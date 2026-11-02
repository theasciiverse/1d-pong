//! The rules. The court is one line, 0.0 at the left end and 1.0 at the
//! right, so the game plays the same at any terminal width. Each side owns a
//! hit zone at its end; pressing while the ball is in your zone and coming
//! at you sends it back. The deeper into the zone (the closer to your own
//! end) you hit it, the faster it goes back: hitting late is the risk you
//! take to win the point.

use std::collections::VecDeque;

/// Each side's hit zone, as a share of the line, before any shrinking.
pub const ZONE: f32 = 0.15;
/// The leader's zone shrinks this much per point ahead, down to MIN_ZONE.
pub const ZONE_SHRINK: f32 = 0.012;
pub const MIN_ZONE: f32 = 0.08;
/// Ball speeds, in line lengths per second.
pub const SERVE_SPEED: f32 = 0.42;
pub const MIN_SPEED: f32 = 0.34;
pub const MAX_SPEED: f32 = 2.7;
/// After a press that hits nothing, that side can't hit for this long.
pub const LOCKOUT: f32 = 0.32;
/// The server has this long to serve before the ball goes by itself.
pub const AUTO_SERVE: f32 = 3.0;
/// How long a point is shown before the next serve.
pub const POINT_PAUSE: f32 = 1.6;
/// How much ball history the renderer gets for the trail, in seconds.
const TRAIL_SECS: f32 = 0.16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
    pub fn idx(self) -> usize {
        self as usize
    }
    /// The direction the ball travels when this side sends it away.
    fn away(self) -> f32 {
        match self {
            Side::Left => 1.0,
            Side::Right => -1.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grade {
    Early,
    Good,
    Great,
    Smash,
}

impl Grade {
    fn of(depth: f32) -> Grade {
        if depth >= 0.92 {
            Grade::Smash
        } else if depth >= 0.70 {
            Grade::Great
        } else if depth >= 0.35 {
            Grade::Good
        } else {
            Grade::Early
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    /// The ball rests at the server's end; `t` counts up.
    Serve { server: Side, t: f32 },
    Rally,
    /// Someone just won a point.
    Point { winner: Side, t: f32 },
    /// Someone won the match.
    Over { winner: Side, t: f32 },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Event {
    Serve { side: Side },
    Hit { side: Side, grade: Grade, speed: f32 },
    Whiff { side: Side },
    Point { winner: Side, score: [u8; 2], rally: u32 },
    Win { winner: Side, score: [u8; 2] },
}

pub struct Game {
    pub pos: f32,
    pub vel: f32,
    pub phase: Phase,
    pub score: [u8; 2],
    pub target: u8,
    pub lockout: [f32; 2],
    pub rally: u32,
    pub best_rally: u32,
    /// Seconds since each side last hit the ball (for the zone flash).
    pub since_hit: [f32; 2],
    pub last_grade: [Option<Grade>; 2],
    /// Recent ball positions, newest first, one per tick.
    pub trail: VecDeque<f32>,
    pub events: Vec<Event>,
}

impl Game {
    pub fn new(target: u8, first_server: Side) -> Game {
        let mut g = Game {
            pos: 0.0,
            vel: 0.0,
            phase: Phase::Serve { server: first_server, t: 0.0 },
            score: [0, 0],
            target,
            lockout: [0.0; 2],
            rally: 0,
            best_rally: 0,
            since_hit: [9.0; 2],
            last_grade: [None; 2],
            trail: VecDeque::new(),
            events: Vec::new(),
        };
        g.place_for_serve(first_server);
        g
    }

    /// The zone length for a side: it shrinks while that side is ahead.
    pub fn zone(&self, side: Side) -> f32 {
        let lead = self.score[side.idx()].saturating_sub(self.score[side.other().idx()]) as f32;
        (ZONE - ZONE_SHRINK * lead).max(MIN_ZONE)
    }

    /// How deep into `side`'s zone the ball is: 0 at the zone's inner edge,
    /// 1 at that side's end of the line. Below 0 means not there yet, above
    /// 1 means it has gone past.
    pub fn depth(&self, side: Side) -> f32 {
        let z = self.zone(side);
        match side {
            Side::Left => (z - self.pos) / z,
            Side::Right => (self.pos - (1.0 - z)) / z,
        }
    }

    /// Whether the ball is travelling toward `side`.
    pub fn incoming(&self, side: Side) -> bool {
        self.phase == Phase::Rally && self.vel * side.away() < 0.0
    }

    pub fn speed(&self) -> f32 {
        self.vel.abs()
    }

    fn place_for_serve(&mut self, server: Side) {
        let z = self.zone(server) * 0.5;
        self.pos = match server {
            Side::Left => z,
            Side::Right => 1.0 - z,
        };
        self.vel = 0.0;
        self.trail.clear();
    }

    /// One player pressed their button.
    pub fn press(&mut self, side: Side) {
        let i = side.idx();
        match self.phase {
            Phase::Serve { server, t } if server == side && t >= 0.35 => self.serve(side),
            Phase::Rally => {
                if self.lockout[i] > 0.0 {
                    return;
                }
                let d = self.depth(side);
                if self.incoming(side) && (0.0..=1.0).contains(&d) {
                    self.hit(side, d);
                } else {
                    self.lockout[i] = LOCKOUT;
                    self.events.push(Event::Whiff { side });
                }
            }
            _ => {}
        }
    }

    fn serve(&mut self, side: Side) {
        self.phase = Phase::Rally;
        self.vel = SERVE_SPEED * side.away();
        self.rally = 0;
        self.lockout = [0.0; 2];
        self.events.push(Event::Serve { side });
    }

    fn hit(&mut self, side: Side, depth: f32) {
        let grade = Grade::of(depth);
        let mut factor = 0.84 + 0.42 * depth;
        if grade == Grade::Smash {
            factor += 0.10;
        }
        let speed = (self.speed() * factor + 0.015).clamp(MIN_SPEED, MAX_SPEED);
        self.vel = speed * side.away();
        self.rally += 1;
        self.best_rally = self.best_rally.max(self.rally);
        self.since_hit[side.idx()] = 0.0;
        self.last_grade[side.idx()] = Some(grade);
        self.events.push(Event::Hit { side, grade, speed });
    }

    pub fn tick(&mut self, dt: f32) {
        for l in &mut self.lockout {
            *l = (*l - dt).max(0.0);
        }
        for s in &mut self.since_hit {
            *s += dt;
        }
        match &mut self.phase {
            Phase::Serve { server, t } => {
                *t += dt;
                if *t >= AUTO_SERVE {
                    let s = *server;
                    self.serve(s);
                }
            }
            Phase::Rally => {
                self.pos += self.vel * dt;
                if self.pos < 0.0 {
                    self.point(Side::Right);
                } else if self.pos > 1.0 {
                    self.point(Side::Left);
                }
            }
            Phase::Point { winner, t } => {
                *t += dt;
                if *t >= POINT_PAUSE {
                    // the side that lost the point serves
                    let server = winner.other();
                    self.phase = Phase::Serve { server, t: 0.0 };
                    self.place_for_serve(server);
                }
            }
            Phase::Over { t, .. } => *t += dt,
        }
        if self.phase == Phase::Rally {
            self.trail.push_front(self.pos);
            let keep = (TRAIL_SECS / dt).ceil() as usize;
            self.trail.truncate(keep);
        } else if !matches!(self.phase, Phase::Point { .. } | Phase::Over { .. }) {
            self.trail.clear();
        }
    }

    fn point(&mut self, winner: Side) {
        self.pos = self.pos.clamp(0.0, 1.0);
        self.score[winner.idx()] += 1;
        let rally = self.rally;
        self.events.push(Event::Point { winner, score: self.score, rally });
        if self.score[winner.idx()] >= self.target {
            self.phase = Phase::Over { winner, t: 0.0 };
            self.events.push(Event::Win { winner, score: self.score });
        } else {
            self.phase = Phase::Point { winner, t: 0.0 };
        }
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 240.0;

    fn rally_toward_right() -> Game {
        let mut g = Game::new(7, Side::Left);
        g.tick(0.5);
        g.press(Side::Left);
        assert_eq!(g.phase, Phase::Rally);
        g
    }

    fn run_until(g: &mut Game, mut f: impl FnMut(&Game) -> bool) {
        for _ in 0..100_000 {
            if f(g) {
                return;
            }
            g.tick(DT);
        }
        panic!("never happened");
    }

    #[test]
    fn late_hits_go_back_faster_than_early_ones() {
        let mut early = rally_toward_right();
        run_until(&mut early, |g| g.depth(Side::Right) >= 0.05);
        early.press(Side::Right);
        let mut late = rally_toward_right();
        run_until(&mut late, |g| g.depth(Side::Right) >= 0.95);
        late.press(Side::Right);
        assert!(early.vel < 0.0 && late.vel < 0.0);
        assert!(late.speed() > SERVE_SPEED && early.speed() < SERVE_SPEED);
        assert!(matches!(late.events.last(), Some(Event::Hit { grade: Grade::Smash, .. })));
    }

    #[test]
    fn a_press_outside_the_zone_locks_you_out() {
        let mut g = rally_toward_right();
        g.press(Side::Right);
        assert!(g.lockout[1] > 0.0);
        assert!(g.vel > 0.0, "the ball kept going");
    }

    #[test]
    fn a_missed_ball_scores_for_the_other_side_and_the_loser_serves() {
        let mut g = rally_toward_right();
        run_until(&mut g, |g| g.phase != Phase::Rally);
        assert_eq!(g.score, [1, 0]);
        run_until(&mut g, |g| matches!(g.phase, Phase::Serve { .. }));
        assert!(matches!(g.phase, Phase::Serve { server: Side::Right, .. }));
    }

    #[test]
    fn the_leader_has_the_smaller_zone() {
        let mut g = Game::new(7, Side::Left);
        g.score = [3, 0];
        assert!(g.zone(Side::Left) < g.zone(Side::Right));
        g.score = [7, 0];
        assert!(g.zone(Side::Left) >= MIN_ZONE);
    }

    #[test]
    fn reaching_the_target_ends_the_match() {
        let mut g = Game::new(1, Side::Left);
        g.tick(0.5);
        g.press(Side::Left);
        run_until(&mut g, |g| g.phase != Phase::Rally);
        assert!(matches!(g.phase, Phase::Over { winner: Side::Left, .. }));
    }
}
