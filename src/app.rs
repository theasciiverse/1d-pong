//! Screens, matches and the effects around the game: everything between
//! the rules in game.rs and what render.rs draws.

use crate::audio::{Audio, Sound};
use crate::cpu::{Cpu, Skill};
use crate::game::{Event, Game, Grade, Phase, Side, MAX_SPEED, MIN_SPEED};
use crate::rng::Rng;
use crate::store::Store;

/// How far the game has unfolded for this player, saved between runs.
/// It starts bare, just the line, and each level arrives the first time
/// its trigger happens, strictly in order.
pub mod level {
    /// the line, the paddles and the ball; nothing else
    pub const BARE: u8 = 0;
    /// your first return
    pub const SOUND: u8 = 1;
    /// the first point: small scores at the ends of the line
    pub const SCORE: u8 = 2;
    /// the first rally of 5: the ball's trail
    pub const TRAIL: u8 = 3;
    /// your first smash, or a rally of 10: hit stop, sparks, grades
    pub const IMPACT: u8 = 4;
    /// your first win: big scores, the rally meter, the title and menu
    pub const FULL: u8 = 5;
    /// a win on Hard: the King of Lineland
    pub const KING: u8 = 6;
    /// beating the King: a second dimension opens for a moment
    pub const PLANE: u8 = 7;
    pub const MAX: u8 = PLANE;

    pub fn caption(l: u8) -> &'static str {
        match l {
            SOUND => "sound",
            SCORE => "score",
            TRAIL => "trail",
            IMPACT => "impact",
            FULL => "the whole game  ·  ESC for the menu",
            KING => "the King of Lineland challenges you",
            PLANE => "upward, not northward",
            _ => "",
        }
    }

    /// How long the caption (and for PLANE the opening) lasts.
    pub fn shown_for(l: u8) -> f32 {
        if l == PLANE {
            7.0
        } else {
            3.5
        }
    }
}

/// Before the full game unfolds, a finished match starts the next by
/// itself after this.
const BARE_RESTART: f32 = 5.0;
/// Before the full game unfolds, matches are this short, so the first
/// win (and with it the whole game) comes soon.
const BARE_TARGET: u8 = 3;
/// The whole game also unfolds after this many bare matches, won or not,
/// so nobody is stuck on the line.
const BARE_MATCHES: u8 = 3;

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

    /// One fixed step. `juice` off (the pure view, or before impact has
    /// unfolded) skips the hit stop.
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
    pub fn react(&mut self, events: &[Event], impact: bool) {
        let g = &self.game;
        let fx = &mut self.fx;
        for e in events {
            match *e {
                Event::Hit { side, grade, speed } if impact => {
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
                    let loser = winner.other();
                    fx.conceded = Some((loser, 0.0));
                    if !impact {
                        continue;
                    }
                    fx.shake = 0.3;
                    fx.freeze = POINT_STOP;
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
    /// the level that just unfolded, and for how long it has been shown
    pub reveal: Option<(u8, f32)>,
    /// whether the player has pressed anything yet this run
    pub hinted: bool,
    /// bare matches finished this run
    pub bare_played: u8,
}

impl App {
    pub fn new(audio: Audio, store: Store, target: u8, seed: u64) -> App {
        let mut rng = Rng::new(seed);
        let skill = store.skill.as_deref().and_then(Skill::parse).unwrap_or(Skill::Normal);
        let demo = Match::new(Mode::Watch, target, &mut rng);
        App {
            screen: Screen::Title,
            menu: 0,
            skill,
            target,
            demo,
            play: None,
            paused: false,
            store,
            audio,
            rng,
            quit: false,
            reveal: None,
            hinted: false,
            bare_played: 0,
        }
    }

    pub fn unfold(&self) -> u8 {
        self.store.unfold
    }

    /// Points to win a match: short until the whole game has unfolded.
    pub fn match_target(&self) -> u8 {
        if self.unfold() < level::FULL {
            self.target.min(BARE_TARGET)
        } else {
            self.target
        }
    }

    /// Sounds only once sound has unfolded.
    fn sound(&self, s: Sound) {
        if self.unfold() >= level::SOUND {
            self.audio.play(s);
        }
    }

    /// Unfold `l` if it is the next level.
    fn unlock(&mut self, l: u8) {
        if self.store.unfold + 1 != l {
            return;
        }
        self.store.unfold = l;
        if l == level::KING {
            self.skill = Skill::King;
            self.store.skill = Some(Skill::King.key().to_string());
        }
        self.store.save();
        self.reveal = Some((l, 0.0));
        log::info!("unfolded level {l}: {}", level::caption(l));
        self.sound(Sound::Point(true));
    }

    /// Set the level by hand (`--unfold N`), saved; its moment is shown
    /// again, so each one can be seen on demand.
    pub fn set_unfold(&mut self, l: u8) {
        self.store.unfold = l.min(level::MAX);
        self.store.save();
        if l > level::BARE {
            self.reveal = Some((self.store.unfold, 0.0));
        }
    }

    pub fn start(&mut self, mode: Mode) {
        let target = self.match_target();
        log::info!("start match {mode:?} to {target}");
        if let Mode::Solo(skill) = mode {
            if self.unfold() >= level::FULL {
                self.store.skill = Some(skill.key().to_string());
                self.store.save();
            }
        }
        self.play = Some(Match::new(mode, target, &mut self.rng));
        self.screen = Screen::Play;
        self.paused = false;
        self.sound(Sound::Serve);
    }

    pub fn to_title(&mut self) {
        if self.unfold() < level::FULL {
            // there is no title yet: ESC leaves
            self.quit = true;
            return;
        }
        log::info!("back to title");
        self.screen = Screen::Title;
        self.play = None;
        self.paused = false;
    }

    /// Advance one fixed step: the title's demo always runs (quietly), a
    /// match unless paused.
    pub fn step(&mut self) {
        if let Some((l, t)) = &mut self.reveal {
            *t += DT;
            if *t > level::shown_for(*l) {
                self.reveal = None;
            }
        }
        let impact = !self.store.pure && self.unfold() >= level::IMPACT;
        if self.screen == Screen::Title {
            self.demo.step(true);
            let events = self.demo.game.take_events();
            self.demo.react(&events, true);
            if self.demo.over_for().is_some_and(|t| t > DEMO_RESTART) {
                self.demo = Match::new(Mode::Watch, self.target, &mut self.rng);
            }
            return;
        }
        if self.paused {
            return;
        }
        let bare = self.unfold() < level::FULL;
        let target = self.match_target();
        let Some(m) = self.play.as_mut() else { return };
        m.step(impact);
        let restart = if m.mode == Mode::Watch { DEMO_RESTART } else if bare { BARE_RESTART } else { f32::INFINITY };
        if m.over_for().is_some_and(|t| t > restart) {
            let mode = m.mode;
            *m = Match::new(mode, target, &mut self.rng);
            log::info!("next match {mode:?} to {target}");
        }
        let events = m.game.take_events();
        m.react(&events, impact);
        for e in events {
            self.on_event(e);
        }
    }

    fn on_event(&mut self, e: Event) {
        let loud = self.unfold() >= level::SOUND;
        let Some(m) = self.play.as_mut() else { return };
        log::info!("{e:?}");
        let audio = &self.audio;
        let play = |s: Sound| {
            if loud {
                audio.play(s)
            }
        };
        let human_left = matches!(m.mode, Mode::Solo(_));
        let mut unlocks = Vec::new();
        match e {
            Event::Serve { .. } => play(Sound::Serve),
            Event::Hit { side, grade, speed } => {
                if grade == Grade::Smash {
                    play(Sound::Smash);
                } else {
                    play(Sound::Hit(heat(speed)));
                }
                if m.is_human(side) {
                    unlocks.push(level::SOUND);
                    if grade == Grade::Smash {
                        unlocks.push(level::IMPACT);
                    }
                }
                if m.mode != Mode::Watch {
                    if m.game.rally >= 5 {
                        unlocks.push(level::TRAIL);
                    }
                    if m.game.rally >= 10 {
                        unlocks.push(level::IMPACT);
                    }
                }
            }
            Event::Whiff { .. } => play(Sound::Whiff),
            Event::Point { winner, .. } => {
                play(Sound::Point(!human_left || winner == Side::Left));
                if m.mode != Mode::Watch {
                    unlocks.push(level::SCORE);
                }
            }
            Event::Win { winner, .. } => {
                let best = m.game.best_rally;
                if best > self.store.best_rally && m.mode != Mode::Watch {
                    self.store.best_rally = best;
                }
                let human_won = match m.mode {
                    Mode::Solo(_) => winner == Side::Left,
                    Mode::Duo => true,
                    Mode::Watch => false,
                };
                if human_won {
                    unlocks.push(level::FULL);
                }
                if m.mode != Mode::Watch && self.store.unfold < level::FULL {
                    self.bare_played += 1;
                    if self.bare_played >= BARE_MATCHES {
                        unlocks.push(level::FULL);
                    }
                }
                if m.mode == Mode::Solo(Skill::Hard) && human_won {
                    unlocks.push(level::KING);
                }
                if m.mode == Mode::Solo(Skill::King) && human_won {
                    self.store.king_beaten = true;
                    unlocks.push(level::PLANE);
                }
                if m.mode != Mode::Watch {
                    self.store.save();
                }
                play(if human_left && winner == Side::Right { Sound::Lose } else { Sound::Win });
            }
        }
        unlocks.sort();
        for l in unlocks {
            self.unlock(l);
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
            self.hinted = true;
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
        // the King only once he has challenged you
        let top = if self.unfold() >= level::KING { Skill::ALL.len() } else { Skill::ALL.len() - 1 };
        self.skill = Skill::ALL[(i + d).clamp(0, top as i32 - 1) as usize];
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
                    "play {:?} unfold {} reveal {:?} paused {} phase {:?} score {:?} pos {:.3} vel {:.3} rally {} lockout {:?} zones {:.3}/{:.3}",
                    m.mode,
                    self.store.unfold,
                    self.reveal,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Grade;

    fn fresh() -> App {
        let dir = std::env::temp_dir().join(format!("1d-pong-test-{}", std::process::id()));
        std::env::set_var("XDG_DATA_HOME", &dir);
        App::new(Audio::silent(), Store::default(), 7, 42)
    }

    fn hit(side: Side, grade: Grade) -> Event {
        Event::Hit { side, grade, speed: 0.5 }
    }

    #[test]
    fn the_game_unfolds_in_order_as_you_play() {
        let mut app = fresh();
        app.start(Mode::Solo(Skill::Easy));
        assert_eq!(app.unfold(), level::BARE);
        // a point before any return: the score has to wait for sound
        app.on_event(Event::Point { winner: Side::Right, score: [0, 1], rally: 0 });
        assert_eq!(app.unfold(), level::BARE);
        app.on_event(hit(Side::Left, Grade::Good));
        assert_eq!(app.unfold(), level::SOUND);
        app.on_event(Event::Point { winner: Side::Left, score: [1, 1], rally: 1 });
        assert_eq!(app.unfold(), level::SCORE);
        app.play.as_mut().unwrap().game.rally = 5;
        app.on_event(hit(Side::Right, Grade::Good));
        assert_eq!(app.unfold(), level::TRAIL);
        app.on_event(hit(Side::Left, Grade::Smash));
        assert_eq!(app.unfold(), level::IMPACT);
        app.on_event(Event::Win { winner: Side::Left, score: [7, 3] });
        assert_eq!(app.unfold(), level::FULL);
        // a win on Normal is not a win on Hard
        app.start(Mode::Solo(Skill::Normal));
        app.on_event(Event::Win { winner: Side::Left, score: [7, 3] });
        assert_eq!(app.unfold(), level::FULL);
        app.start(Mode::Solo(Skill::Hard));
        app.on_event(Event::Win { winner: Side::Left, score: [7, 6] });
        assert_eq!(app.unfold(), level::KING);
        assert_eq!(app.skill, Skill::King);
        app.start(Mode::Solo(Skill::King));
        app.on_event(Event::Win { winner: Side::Left, score: [7, 6] });
        assert_eq!(app.unfold(), level::PLANE);
        assert!(app.store.king_beaten);
        assert_eq!(app.reveal.map(|r| r.0), Some(level::PLANE));
    }

    #[test]
    fn the_computer_playing_itself_unfolds_nothing() {
        let mut app = fresh();
        app.start(Mode::Watch);
        app.on_event(hit(Side::Left, Grade::Smash));
        app.on_event(Event::Point { winner: Side::Left, score: [1, 0], rally: 12 });
        app.on_event(Event::Win { winner: Side::Left, score: [7, 0] });
        assert_eq!(app.unfold(), level::BARE);
    }

    #[test]
    fn bare_matches_are_short_and_three_of_them_unfold_the_game() {
        let mut app = fresh();
        app.store.unfold = level::IMPACT;
        app.start(Mode::Solo(Skill::Easy));
        assert_eq!(app.play.as_ref().unwrap().game.target, 3);
        for _ in 0..2 {
            app.on_event(Event::Win { winner: Side::Right, score: [1, 3] });
            assert_eq!(app.unfold(), level::IMPACT);
        }
        app.on_event(Event::Win { winner: Side::Right, score: [2, 3] });
        assert_eq!(app.unfold(), level::FULL);
        app.start(Mode::Solo(Skill::Easy));
        assert_eq!(app.play.as_ref().unwrap().game.target, 7);
    }

    #[test]
    fn a_rally_of_ten_brings_impact_without_a_smash() {
        let mut app = fresh();
        app.store.unfold = level::TRAIL;
        app.start(Mode::Solo(Skill::Easy));
        app.play.as_mut().unwrap().game.rally = 10;
        app.on_event(hit(Side::Right, Grade::Good));
        assert_eq!(app.unfold(), level::IMPACT);
    }
}
