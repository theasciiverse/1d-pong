//! One match at a time, and nothing around it.

use crate::cpu::{Cpu, Skill};
use crate::game::{Game, Phase, Side};
use crate::rng::Rng;

/// The fixed simulation step.
pub const DT: f32 = 1.0 / 240.0;
/// After a match ends, presses are ignored this long (so a late hit
/// doesn't start the next one).
const OVER_HOLD: f32 = 1.0;
/// Watch mode starts a new match after this.
const WATCH_RESTART: f32 = 4.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Solo(Skill),
    Duo,
    Watch,
}

pub struct App {
    pub game: Game,
    pub cpus: Vec<Cpu>,
    pub mode: Mode,
    pub target: u8,
    pub paused: bool,
    pub quit: bool,
    pub rng: Rng,
}

impl App {
    pub fn new(mode: Mode, target: u8, seed: u64) -> App {
        let mut app = App { game: Game::new(target, Side::Left), cpus: Vec::new(), mode, target, paused: false, quit: false, rng: Rng::new(seed) };
        app.restart();
        app
    }

    pub fn restart(&mut self) {
        let first = if self.rng.unit() < 0.5 { Side::Left } else { Side::Right };
        self.game = Game::new(self.target, first);
        self.cpus = match self.mode {
            Mode::Solo(skill) => vec![Cpu::new(Side::Right, skill, self.rng.next())],
            Mode::Duo => vec![],
            Mode::Watch => vec![Cpu::new(Side::Left, Skill::King, self.rng.next()), Cpu::new(Side::Right, Skill::King, self.rng.next())],
        };
        self.paused = false;
        log::info!("new match {:?} to {} first serve {first:?}", self.mode, self.target);
    }

    pub fn is_human(&self, side: Side) -> bool {
        match self.mode {
            Mode::Solo(_) => side == Side::Left,
            Mode::Duo => true,
            Mode::Watch => false,
        }
    }

    pub fn over_for(&self) -> Option<f32> {
        match self.game.phase {
            Phase::Over { t, .. } => Some(t),
            _ => None,
        }
    }

    pub fn step(&mut self) {
        if self.paused {
            return;
        }
        for c in &mut self.cpus {
            c.step(&mut self.game);
        }
        self.game.tick(DT);
        for e in self.game.take_events() {
            log::info!("{e:?}");
        }
        if self.mode == Mode::Watch && self.over_for().is_some_and(|t| t > WATCH_RESTART) {
            self.restart();
        }
    }

    /// A player's button, from the keyboard or a pad.
    pub fn press(&mut self, side: Side) {
        if self.paused {
            return;
        }
        if let Some(t) = self.over_for() {
            if t >= OVER_HOLD && self.mode != Mode::Watch {
                self.restart();
            }
            return;
        }
        if self.is_human(side) {
            let g = &self.game;
            log::debug!("press {side:?} pos {:.3} depth {:.3} phase {:?}", g.pos, g.depth(side), g.phase);
            self.game.press(side);
        }
    }

    pub fn snapshot(&self) -> String {
        let g = &self.game;
        format!(
            "{:?} paused {} phase {:?} score {:?} pos {:.3} vel {:.3} rally {} lockout {:?} zones {:.3}/{:.3}",
            self.mode,
            self.paused,
            g.phase,
            g.score,
            g.pos,
            g.vel,
            g.rally,
            g.lockout,
            g.zone(Side::Left),
            g.zone(Side::Right)
        )
    }
}
