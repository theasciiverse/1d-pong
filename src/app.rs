//! Screens, matches and the effects around the game: everything between
//! the rules in game.rs and what render.rs draws.

use crate::audio::{Audio, Sound};
use crate::cpu::{Cpu, Skill};
use crate::game::{Event, Game, Grade, Phase, Side, MAX_SPEED, MIN_SPEED};
use crate::rng::Rng;
use crate::store::Store;

/// The fixed simulation step.
pub const DT: f32 = 1.0 / 240.0;
/// After a match ends, presses are ignored this long (so a late hit
/// doesn't start the next one).
const OVER_HOLD: f32 = 1.0;
/// Watch mode and the title screen's demo start a new match after this.
const DEMO_RESTART: f32 = 4.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Solo(Skill),
    Duo,
    Watch,
}

impl Mode {
    pub fn labels(self) -> [&'static str; 2] {
        match self {
            Mode::Solo(_) => ["YOU", "CPU"],
            Mode::Duo => ["P1", "P2"],
            Mode::Watch => ["LEFT", "RIGHT"],
        }
    }
}

pub struct Popup {
    pub side: Side,
    pub grade: Grade,
    pub age: f32,
}

/// A spark thrown off a hit. `x` is along the line (0..1, like the ball),
/// `y` in rows from the line (negative is up).
pub struct Spark {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: f32,
    pub max: f32,
    /// the ball's heat when it was thrown, for its colour
    pub heat: f32,
    /// thrown by a point rather than a hit: drawn red
    pub hurt: bool,
}

pub struct Fx {
    pub shake: f32,
    pub popups: Vec<Popup>,
    /// the side that just conceded, and how long ago
    pub conceded: Option<(Side, f32)>,
    pub time: f32,
    /// hit stop: the game holds still this much longer
    pub freeze: f32,
    pub sparks: Vec<Spark>,
    rng: Rng,
}

/// Hit stop for each grade, in seconds: the harder the hit, the longer
/// the moment of impact is held.
fn hit_stop(grade: Grade) -> f32 {
    match grade {
        Grade::Early => 0.025,
        Grade::Good => 0.04,
        Grade::Great => 0.065,
        Grade::Smash => 0.11,
    }
}

const POINT_STOP: f32 = 0.14;

impl Fx {
    fn new(seed: u64) -> Fx {
        Fx { shake: 0.0, popups: Vec::new(), conceded: None, time: 0.0, freeze: 0.0, sparks: Vec::new(), rng: Rng::new(seed) }
    }

    /// Throw `count` sparks from `x`, mostly in direction `dir` (+1 right).
    fn burst(&mut self, x: f32, dir: f32, count: usize, power: f32, heat: f32, hurt: bool) {
        for _ in 0..count {
            let r = &mut self.rng;
            let back = r.unit() < 0.25;
            let vx = dir * (0.04 + r.unit() * 0.30 * power) * if back { -0.5 } else { 1.0 };
            let vy = (r.unit() - 0.5) * 2.0 * (3.0 + 5.0 * power);
            let max = 0.18 + r.unit() * 0.30 * power.max(0.6);
            self.sparks.push(Spark { x, y: 0.0, vx, vy, life: max, max, heat, hurt });
        }
    }

    fn update_sparks(&mut self) {
        let drag = 1.0 - 3.5 * DT;
        for s in &mut self.sparks {
            s.x += s.vx * DT;
            s.y += s.vy * DT;
            s.vx *= drag;
            s.vy *= drag;
            s.life -= DT;
        }
        self.sparks.retain(|s| s.life > 0.0 && (0.0..=1.0).contains(&s.x));
    }
}

pub struct Match {
    pub game: Game,
    pub cpus: Vec<Cpu>,
    pub mode: Mode,
    pub fx: Fx,
}

impl Match {
    pub fn new(mode: Mode, target: u8, rng: &mut Rng) -> Match {
        let first = if rng.unit() < 0.5 { Side::Left } else { Side::Right };
        let cpus = match mode {
            Mode::Solo(skill) => vec![Cpu::new(Side::Right, skill, rng.next())],
            Mode::Duo => vec![],
            Mode::Watch => vec![Cpu::new(Side::Left, Skill::King, rng.next()), Cpu::new(Side::Right, Skill::King, rng.next())],
        };
        Match { game: Game::new(target, first), cpus, mode, fx: Fx::new(rng.next()) }
    }

    /// One fixed step. `juice` off (the pure view) skips the hit stop.
    pub fn step(&mut self, juice: bool) {
        let fx = &mut self.fx;
        fx.time += DT;
        fx.shake = (fx.shake - DT).max(0.0);
        for p in &mut fx.popups {
            p.age += DT;
        }
        fx.popups.retain(|p| p.age < 0.9);
        if !juice {
            fx.freeze = 0.0;
        }
        if fx.freeze > 0.0 {
            // the moment of impact: everything holds still but the flashes
            fx.freeze -= DT;
            return;
        }
        if let Some((_, t)) = &mut fx.conceded {
            *t += DT;
        }
        fx.update_sparks();
        for c in &mut self.cpus {
            c.step(&mut self.game);
        }
        self.game.tick(DT);
    }

    /// The match's own effects for what just happened in the game; sound
    /// and saving are the app's.
    pub fn react(&mut self, events: &[Event]) {
        let g = &self.game;
        let fx = &mut self.fx;
        for e in events {
            match *e {
                Event::Hit { side, grade, speed } => {
                    fx.popups.retain(|p| p.side != side);
                    fx.popups.push(Popup { side, grade, age: 0.0 });
                    fx.freeze = hit_stop(grade);
                    let (count, power) = match grade {
                        Grade::Early => (3, 0.4),
                        Grade::Good => (5, 0.6),
                        Grade::Great => (8, 0.85),
                        Grade::Smash => (16, 1.3),
                    };
                    let dir = if side == Side::Left { 1.0 } else { -1.0 };
                    fx.burst(g.pos, dir, count, power, heat(speed), false);
                    if grade == Grade::Smash {
                        fx.shake = 0.18;
                    }
                }
                Event::Point { winner, .. } => {
                    fx.shake = 0.3;
                    fx.freeze = POINT_STOP;
                    let loser = winner.other();
                    fx.conceded = Some((loser, 0.0));
                    let (x, dir) = if loser == Side::Left { (0.0, 1.0) } else { (1.0, -1.0) };
                    fx.burst(x, dir, 18, 1.0, 1.0, true);
                }
                _ => {}
            }
        }
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
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Title,
    Play,
}

pub const MENU: [&str; 4] = ["1 PLAYER", "2 PLAYERS", "WATCH", "QUIT"];

pub struct App {
    pub screen: Screen,
    pub menu: usize,
    pub skill: Skill,
    pub target: u8,
    pub demo: Match,
    pub play: Option<Match>,
    pub paused: bool,
    pub store: Store,
    pub audio: Audio,
    pub rng: Rng,
    pub quit: bool,
}

impl App {
    pub fn new(audio: Audio, store: Store, target: u8, seed: u64) -> App {
        let mut rng = Rng::new(seed);
        let skill = store.skill.as_deref().and_then(Skill::parse).unwrap_or(Skill::Normal);
        let demo = Match::new(Mode::Watch, target, &mut rng);
        App { screen: Screen::Title, menu: 0, skill, target, demo, play: None, paused: false, store, audio, rng, quit: false }
    }

    pub fn start(&mut self, mode: Mode) {
        log::info!("start match {mode:?} to {}", self.target);
        if let Mode::Solo(skill) = mode {
            self.store.skill = Some(skill.key().to_string());
            self.store.save();
        }
        self.play = Some(Match::new(mode, self.target, &mut self.rng));
        self.screen = Screen::Play;
        self.paused = false;
        self.audio.play(Sound::Serve);
    }

    pub fn to_title(&mut self) {
        log::info!("back to title");
        self.screen = Screen::Title;
        self.play = None;
        self.paused = false;
    }

    /// Advance one fixed step: the title's demo always runs (quietly), a
    /// match unless paused.
    pub fn step(&mut self) {
        let juice = !self.store.pure;
        if self.screen == Screen::Title {
            self.demo.step(true);
            let events = self.demo.game.take_events();
            self.demo.react(&events);
            if self.demo.over_for().is_some_and(|t| t > DEMO_RESTART) {
                self.demo = Match::new(Mode::Watch, self.target, &mut self.rng);
            }
            return;
        }
        if self.paused {
            return;
        }
        let Some(m) = self.play.as_mut() else { return };
        m.step(juice);
        if m.mode == Mode::Watch && m.over_for().is_some_and(|t| t > DEMO_RESTART) {
            *m = Match::new(Mode::Watch, self.target, &mut self.rng);
        }
        let events = m.game.take_events();
        m.react(&events);
        for e in events {
            self.on_event(e);
        }
    }

    fn on_event(&mut self, e: Event) {
        let Some(m) = self.play.as_mut() else { return };
        log::info!("{e:?}");
        let human_left = matches!(m.mode, Mode::Solo(_));
        match e {
            Event::Serve { .. } => self.audio.play(Sound::Serve),
            Event::Hit { grade, speed, .. } => {
                if grade == Grade::Smash {
                    self.audio.play(Sound::Smash);
                } else {
                    self.audio.play(Sound::Hit(heat(speed)));
                }
            }
            Event::Whiff { .. } => self.audio.play(Sound::Whiff),
            Event::Point { winner, .. } => {
                self.audio.play(Sound::Point(!human_left || winner == Side::Left));
            }
            Event::Win { winner, .. } => {
                let best = m.game.best_rally;
                if best > self.store.best_rally && m.mode != Mode::Watch {
                    self.store.best_rally = best;
                }
                if m.mode == Mode::Solo(Skill::King) && winner == Side::Left {
                    self.store.king_beaten = true;
                }
                if m.mode != Mode::Watch {
                    self.store.save();
                }
                self.audio.play(if human_left && winner == Side::Right { Sound::Lose } else { Sound::Win });
            }
        }
    }

    /// A player's button, from the keyboard or a pad.
    pub fn press(&mut self, side: Side) {
        if self.screen != Screen::Play || self.paused {
            return;
        }
        let Some(m) = self.play.as_mut() else { return };
        if let Some(t) = m.over_for() {
            if t >= OVER_HOLD && m.mode != Mode::Watch {
                let mode = m.mode;
                self.start(mode);
            }
            return;
        }
        if m.is_human(side) {
            log::debug!("press {side:?} pos {:.3} depth {:.3} phase {:?}", m.game.pos, m.game.depth(side), m.game.phase);
            m.game.press(side);
        }
    }

    pub fn menu_move(&mut self, d: i32) {
        self.menu = (self.menu as i32 + d).rem_euclid(MENU.len() as i32) as usize;
        self.audio.play(Sound::Menu);
    }

    pub fn skill_move(&mut self, d: i32) {
        if self.menu != 0 {
            return;
        }
        let i = Skill::ALL.iter().position(|&s| s == self.skill).unwrap_or(1) as i32;
        self.skill = Skill::ALL[(i + d).clamp(0, Skill::ALL.len() as i32 - 1) as usize];
        self.audio.play(Sound::Menu);
    }

    pub fn menu_pick(&mut self) {
        match self.menu {
            0 => self.start(Mode::Solo(self.skill)),
            1 => self.start(Mode::Duo),
            2 => self.start(Mode::Watch),
            _ => self.quit = true,
        }
    }

    pub fn snapshot(&self) -> String {
        match (&self.screen, &self.play) {
            (Screen::Play, Some(m)) => {
                let g = &m.game;
                format!(
                    "play {:?} paused {} phase {:?} score {:?} pos {:.3} vel {:.3} rally {} lockout {:?} zones {:.3}/{:.3}",
                    m.mode,
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
            _ => format!("title menu {} skill {:?}", self.menu, self.skill),
        }
    }
}

/// The ball's speed as 0 (slowest) to 1 (fastest).
pub fn heat(speed: f32) -> f32 {
    ((speed - MIN_SPEED) / (MAX_SPEED - MIN_SPEED)).clamp(0.0, 1.0)
}
