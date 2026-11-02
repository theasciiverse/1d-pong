//! The computer player. Each time the ball comes at it, it picks how deep
//! into its zone it means to hit, with a spread that grows with the ball's
//! speed, and presses when the ball gets there. Aim too deep and the ball
//! gets past; too shallow and it whiffs before the zone and is locked out.

use crate::game::{Game, Phase, Side};
use crate::rng::Rng;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Skill {
    Easy,
    Normal,
    Hard,
    King,
}

impl Skill {
    pub const ALL: [Skill; 4] = [Skill::Easy, Skill::Normal, Skill::Hard, Skill::King];

    pub fn key(self) -> &'static str {
        match self {
            Skill::Easy => "easy",
            Skill::Normal => "normal",
            Skill::Hard => "hard",
            Skill::King => "king",
        }
    }

    pub fn parse(s: &str) -> Option<Skill> {
        Skill::ALL.into_iter().find(|k| k.key() == s)
    }

    /// Mean aim depth and its spread at serve speed.
    fn aim(self) -> (f32, f32) {
        match self {
            Skill::Easy => (0.42, 0.30),
            Skill::Normal => (0.58, 0.22),
            Skill::Hard => (0.72, 0.14),
            Skill::King => (0.82, 0.075),
        }
    }
}

pub struct Cpu {
    pub side: Side,
    pub skill: Skill,
    aim: Option<f32>,
    serve_after: Option<f32>,
    rng: Rng,
}

impl Cpu {
    pub fn new(side: Side, skill: Skill, seed: u64) -> Cpu {
        Cpu { side, skill, aim: None, serve_after: None, rng: Rng::new(seed) }
    }

    /// Called every tick; presses through the game.
    pub fn step(&mut self, g: &mut Game) {
        match g.phase {
            Phase::Serve { server, t } if server == self.side => {
                let wait = *self.serve_after.get_or_insert_with(|| 0.6 + self.rng.unit() * 0.9);
                if t >= wait {
                    self.serve_after = None;
                    g.press(self.side);
                }
            }
            Phase::Rally if g.incoming(self.side) => {
                let (mean, spread) = self.skill.aim();
                let speed = g.speed();
                let rng = &mut self.rng;
                let aim = *self.aim.get_or_insert_with(|| {
                    // greedier when the ball is slow, more careful when fast
                    let mean = mean + (0.5 - speed).max(0.0) * 0.3 - (speed - 1.2).max(0.0) * 0.12;
                    let spread = spread * (1.0 + speed * 0.55);
                    (mean + rng.normal() * spread).min(1.15)
                });
                if g.depth(self.side) >= aim && g.lockout[self.side.idx()] == 0.0 {
                    g.press(self.side);
                    if !g.incoming(self.side) {
                        self.aim = None;
                    } else {
                        // whiffed early: try again a little deeper once unlocked
                        self.aim = Some(aim.max(0.0) + 0.2);
                    }
                }
            }
            _ => self.aim = None,
        }
    }
}
