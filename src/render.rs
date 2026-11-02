//! Drawing. The game itself is one row; everything around it is there to
//! make that row feel like a stadium: big scores, the ball's glow spilling
//! out of Lineland, a trail that heats up with speed, and a shake when it
//! counts.

use crate::app::{heat, App, Fx, Match, Mode, Screen, DT, MENU};
use crate::cpu::Skill;
use crate::font;
use crate::game::{Game, Grade, Phase, Side};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

pub const MIN_W: u16 = 40;
pub const MIN_H: u16 = 12;

const BG: (u8, u8, u8) = (0, 0, 0);
const SCAN: (u8, u8, u8) = (5, 11, 7);
const GREEN: (u8, u8, u8) = (60, 255, 110);
const DIM: (u8, u8, u8) = (22, 92, 44);
const FAINT: (u8, u8, u8) = (14, 52, 26);
const AMBER: (u8, u8, u8) = (255, 176, 0);
const DIM_AMBER: (u8, u8, u8) = (120, 78, 0);
const RED: (u8, u8, u8) = (255, 56, 48);
const HOT: (u8, u8, u8) = (255, 250, 225);

fn rgb(c: (u8, u8, u8)) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

fn mix(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    (l(a.0, b.0), l(a.1, b.1), l(a.2, b.2))
}

/// Green when slow, through amber and red, to white hot at full speed.
fn heat_color(h: f32) -> (u8, u8, u8) {
    if h < 0.45 {
        mix(GREEN, AMBER, h / 0.45)
    } else if h < 0.8 {
        mix(AMBER, RED, (h - 0.45) / 0.35)
    } else {
        mix(RED, HOT, (h - 0.8) / 0.2)
    }
}

struct Canvas<'a> {
    buf: &'a mut Buffer,
    dx: i32,
}

impl Canvas<'_> {
    fn put(&mut self, x: i32, y: i32, ch: char, fg: (u8, u8, u8)) {
        let (x, y) = (x + self.dx, y);
        let a = self.buf.area;
        if x < a.x as i32 || y < a.y as i32 || x >= a.right() as i32 || y >= a.bottom() as i32 {
            return;
        }
        if let Some(c) = self.buf.cell_mut((x as u16, y as u16)) {
            c.set_char(ch).set_fg(rgb(fg));
        }
    }

    fn text(&mut self, x: i32, y: i32, s: &str, fg: (u8, u8, u8)) {
        for (i, ch) in s.chars().enumerate() {
            self.put(x + i as i32, y, ch, fg);
        }
    }

    fn center(&mut self, y: i32, s: &str, fg: (u8, u8, u8)) {
        let w = self.buf.area.width as i32;
        self.text((w - s.chars().count() as i32) / 2, y, s, fg);
    }

    fn big(&mut self, x: i32, y: i32, s: &str, sx: u16, half: bool, fg: (u8, u8, u8)) {
        font::draw(self.buf, x + self.dx, y, s, sx, half, rgb(fg));
    }

    fn big_center(&mut self, y: i32, s: &str, sx: u16, half: bool, fg: (u8, u8, u8)) {
        let w = self.buf.area.width as i32;
        self.big((w - font::width(s, sx) as i32) / 2, y, s, sx, half, fg);
    }
}

pub fn draw(app: &App, area: Rect, buf: &mut Buffer) {
    // black page with faint scanlines
    for y in area.top()..area.bottom() {
        let bg = if y % 2 == 1 { SCAN } else { BG };
        for x in area.left()..area.right() {
            if let Some(c) = buf.cell_mut((x, y)) {
                c.set_char(' ').set_bg(rgb(bg)).set_fg(rgb(GREEN)).set_style(Modifier::empty());
            }
        }
    }
    let mut cv = Canvas { buf, dx: 0 };
    if area.width < MIN_W || area.height < MIN_H {
        let y = area.height as i32 / 2;
        cv.center(y - 1, "1D-PONG needs a little more room", GREEN);
        cv.center(y + 1, &format!("{MIN_W}x{MIN_H} or more, this is {}x{}", area.width, area.height), DIM);
        return;
    }
    match (app.screen, &app.play) {
        (Screen::Play, Some(m)) => play(&mut cv, app, m),
        _ => title(&mut cv, app),
    }
}

fn title(cv: &mut Canvas, app: &App) {
    let (w, h) = (cv.buf.area.width as i32, cv.buf.area.height as i32);
    let roomy = h >= 26 && w >= 64;
    let sx = if roomy { 2 } else { 1 };
    let half = h < 20;
    let tw = font::width("1D-PONG", sx) as i32;
    let x = (w - tw) / 2;
    let top = if roomy { 2 } else { 1 };
    // "1D" in amber, the rest in green
    cv.big(x, top, "1D", sx, half, AMBER);
    cv.big(x + font::width("1D", sx) as i32 + sx as i32, top, "-PONG", sx, half, GREEN);
    let mut y = top + font::height(half) as i32 + 1;
    cv.center(y, "a game in one dimension", DIM);
    y += if roomy { 3 } else { 2 };

    // the demo match, playing itself
    let x0 = 6.max(w / 8);
    let x1 = w - 1 - x0;
    court(cv, &app.demo.game, &app.demo.fx, x0, x1, y, true);
    y += if roomy { 4 } else { 2 };

    for (i, item) in MENU.iter().enumerate() {
        let sel = i == app.menu;
        let label = if i == 0 { format!("{item}   ‹ {} ›", app.skill.name()) } else { item.to_string() };
        let s = if sel { format!("▸ {label} ◂") } else { label };
        cv.center(y, &s, if sel { AMBER } else { DIM });
        y += if roomy { 2 } else { 1 };
    }
    y += 1;
    let hint = match app.menu {
        0 => "Hit late for a faster return. Too late and it's gone.",
        1 => "Left player: A   Right player: L   (or two gamepads)",
        2 => "Two kings of Lineland, playing each other",
        _ => "",
    };
    cv.center(y, hint, GREEN);
    if app.store.best_rally > 0 || app.store.king_beaten {
        let mut s = format!("best rally {}", app.store.best_rally);
        if app.store.king_beaten {
            s.push_str("   ·   the King of Lineland has fallen");
        }
        if y + 2 < h - 1 {
            cv.center(y + 2, &s, DIM_AMBER);
        }
    }
    let muted = if app.audio.muted { "   ·   muted" } else { "" };
    cv.center(h - 1, &format!("↑↓ choose  ·  ←→ difficulty  ·  ENTER start  ·  V pure  ·  M mute  ·  Q quit{muted}"), FAINT);
}

fn play(cv: &mut Canvas, app: &App, m: &Match) {
    if app.store.pure {
        pure(cv, &m.game);
        return;
    }
    let (w, h) = (cv.buf.area.width as i32, cv.buf.area.height as i32);
    let g = &m.game;
    let labels = m.mode.labels();
    if m.fx.shake > 0.0 {
        cv.dx = if (m.fx.time / DT) as i64 / 3 % 2 == 0 { 1 } else { -1 };
    }

    // header
    let desc = match m.mode {
        Mode::Solo(skill) => format!("YOU  vs  CPU · {}", skill.name()),
        Mode::Duo => "P1 (A)  vs  P2 (L)".to_string(),
        Mode::Watch => "KING  vs  KING".to_string(),
    };
    cv.center(0, &format!("1D-PONG  ·  {desc}  ·  first to {}", g.target), DIM);

    // big scores
    let big_h = h >= 24;
    let sx = if w >= 80 && big_h { 2 } else { 1 };
    let score_y = if big_h { 2 } else { 1 };
    for side in [Side::Left, Side::Right] {
        let s = g.score[side.idx()].to_string();
        let cx = if side == Side::Left { w / 4 } else { w * 3 / 4 };
        let sw = font::width(&s, sx) as i32;
        let fresh = matches!(m.fx.conceded, Some((c, t)) if c == side.other() && t < 1.2);
        let col = if fresh && (m.fx.time * 8.0) as i32 % 2 == 0 { HOT } else if fresh { AMBER } else { GREEN };
        cv.big(cx - sw / 2, score_y, &s, sx, !big_h, col);
        let lab = labels[side.idx()];
        let ly = score_y + font::height(!big_h) as i32 + 1;
        cv.text(cx - lab.chars().count() as i32 / 2, ly, lab, DIM);
    }

    // the line
    let ly = (h / 2 + 2).min(h - 7);
    let x0 = 4;
    let x1 = w - 5;
    court(cv, g, &m.fx, x0, x1, ly, false);

    // grade popups above the hitter's zone
    for p in &m.fx.popups {
        let word = p.grade.word();
        let zone_mid = match p.side {
            Side::Left => x0 + ((x1 - x0) as f32 * g.zone(Side::Left) * 0.5) as i32,
            Side::Right => x1 - ((x1 - x0) as f32 * g.zone(Side::Right) * 0.5) as i32,
        };
        let x = (zone_mid - word.len() as i32 / 2).clamp(1, w - 1 - word.len() as i32);
        let rise = if p.age > 0.35 { 1 } else { 0 };
        let fade = 1.0 - (p.age / 0.9);
        let col = match p.grade {
            Grade::Early => DIM,
            Grade::Good => GREEN,
            Grade::Great => AMBER,
            Grade::Smash => {
                if (p.age * 20.0) as i32 % 2 == 0 {
                    HOT
                } else {
                    RED
                }
            }
        };
        cv.text(x, ly - 3 - rise, word, mix(BG, col, fade * 1.3));
    }

    // below the line: what's going on
    let info_y = ly + 3;
    match g.phase {
        Phase::Serve { server, t } => {
            if m.is_human(server) {
                if (t * 3.0) as i32 % 2 == 0 || t < 0.35 {
                    let who = if m.mode == Mode::Duo { format!("{} to serve", labels[server.idx()]) } else { "your serve".into() };
                    cv.center(info_y, &who, AMBER);
                }
            } else {
                cv.center(info_y, &format!("{} serves", labels[server.idx()]), DIM);
            }
        }
        Phase::Rally => {
            let heat = heat(g.speed());
            let bars = 16;
            let lit = (heat * bars as f32).ceil() as usize;
            let meter: String = (0..bars).map(|i| if i < lit { '▮' } else { '▯' }).collect();
            let s = format!("RALLY {:<3}  {meter}", g.rally);
            let x = (w - s.chars().count() as i32) / 2;
            cv.text(x, info_y, &format!("RALLY {:<3}  ", g.rally), GREEN);
            let mx = x + 12;
            for (i, ch) in meter.chars().enumerate() {
                let c = if i < lit { heat_color(i as f32 / bars as f32) } else { FAINT };
                cv.put(mx + i as i32, info_y, ch, c);
            }
        }
        Phase::Point { winner, .. } => {
            let word = format!("POINT {}", labels[winner.idx()]);
            cv.big_center(info_y, &word, 1, true, AMBER);
            if g.rally > 1 {
                cv.center(info_y + 4, &format!("after a rally of {}", g.rally), DIM);
            }
        }
        Phase::Over { winner, t } => {
            let word = match m.mode {
                Mode::Solo(_) if winner == Side::Left => "YOU WIN!".to_string(),
                Mode::Solo(_) => "CPU WINS".to_string(),
                _ => format!("{} WINS", labels[winner.idx()]),
            };
            let flash = if (t * 4.0) as i32 % 2 == 0 { AMBER } else { HOT };
            cv.big_center(info_y, &word, 1, h < 26, flash);
            let below = info_y + font::height(h < 26) as i32 + 1;
            cv.center(below, &format!("{} - {}   ·   longest rally {}", g.score[0], g.score[1], g.best_rally), GREEN);
            if m.mode == Mode::Solo(Skill::King) && winner == Side::Left {
                cv.center(below + 1, "Lineland has a new king", AMBER);
            }
        }
    }

    if app.paused {
        cv.big_center(ly - 6, "PAUSED", 1, true, AMBER);
    }

    // help row
    let help = if app.paused {
        "P resume  ·  ESC menu  ·  Q quit".to_string()
    } else if m.over_for().is_some() && m.mode != Mode::Watch {
        "SPACE rematch  ·  ESC menu  ·  Q quit".to_string()
    } else {
        match m.mode {
            Mode::Solo(_) => "SPACE hit  ·  P pause  ·  M mute  ·  ESC menu",
            Mode::Duo => "P1: A   P2: L  ·  P pause  ·  M mute  ·  ESC menu",
            Mode::Watch => "ESC menu  ·  M mute  ·  Q quit",
        }
        .to_string()
    };
    cv.dx = 0;
    cv.center(h - 1, &help, FAINT);
}

/// The court: one row from x0 to x1 at row y, goal posts outside it, hit
/// zones at both ends, the ball, its trail and its glow. `quiet` dims it
/// for the title screen.
fn court(cv: &mut Canvas, g: &Game, fx: &Fx, x0: i32, x1: i32, y: i32, quiet: bool) {
    let n = (x1 - x0 + 1).max(2);
    let cell = |pos: f32| x0 + (pos.clamp(0.0, 1.0) * (n - 1) as f32).round() as i32;
    let dim = |c: (u8, u8, u8)| if quiet { mix(BG, c, 0.75) } else { c };

    // posts: red flash on the side that just conceded
    for (side, x) in [(Side::Left, x0 - 2), (Side::Right, x1 + 2)] {
        let hurt = matches!(fx.conceded, Some((s, t)) if s == side && t < 0.8);
        let col = if hurt { if (fx.time * 12.0) as i32 % 2 == 0 { RED } else { HOT } } else { DIM };
        for dy in -1..=1 {
            cv.put(x, y + dy, '┃', dim(col));
        }
    }

    // the line and the zones
    let left_end = cell(g.zone(Side::Left));
    let right_start = cell(1.0 - g.zone(Side::Right));
    for x in x0..=x1 {
        let side = if x <= left_end {
            Some(Side::Left)
        } else if x >= right_start {
            Some(Side::Right)
        } else {
            None
        };
        let col = match side {
            Some(s) => {
                let i = s.idx();
                if g.lockout[i] > 0.0 {
                    mix(DIM_AMBER, RED, 0.6)
                } else if g.since_hit[i] < 0.15 {
                    mix(AMBER, HOT, 1.0 - g.since_hit[i] / 0.15)
                } else if g.incoming(s) {
                    mix(DIM_AMBER, AMBER, 0.45)
                } else {
                    DIM_AMBER
                }
            }
            None => DIM,
        };
        let ch = if x == (x0 + x1) / 2 && side.is_none() { '╋' } else { '━' };
        cv.put(x, y, ch, dim(col));
    }

    let h = heat(g.speed());
    let ball_col = if g.phase == Phase::Rally { heat_color(h) } else { GREEN };

    // trail, oldest first so the newest wins
    let len = g.trail.len().max(1) as f32;
    for (i, &p) in g.trail.iter().enumerate().rev() {
        let age = i as f32 / len;
        cv.put(cell(p), y, '━', dim(mix(DIM, ball_col, (1.0 - age) * (0.35 + h * 0.65))));
    }

    // the ball and the light it spills out of its one dimension
    let bx = cell(g.pos);
    let visible = match g.phase {
        Phase::Point { t, .. } => (t * 10.0) as i32 % 2 == 0 && t < 0.8,
        _ => true,
    };
    if visible {
        let reach = if quiet { 1 } else { 2 + (h * 3.0) as i32 };
        for d in -reach..=reach {
            let k = 1.0 - d.abs() as f32 / (reach + 1) as f32;
            let c = dim(mix(BG, ball_col, k * if quiet { 0.4 } else { 0.55 + h * 0.3 }));
            cv.put(bx + d, y - 1, '▄', c);
            cv.put(bx + d, y + 1, '▀', c);
        }
        cv.put(bx, y, '█', dim(ball_col));
    }
}

/// The pure view, where 1D-Pong began (0.1.0): one row across the middle of
/// an empty screen, "7 ┃━━━━━●━━━━━┃ 3", and nothing else at all.
fn pure(cv: &mut Canvas, g: &Game) {
    let (w, h) = (cv.buf.area.width as i32, cv.buf.area.height as i32);
    // the scanlines go too
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = cv.buf.cell_mut((x as u16, y as u16)) {
                c.set_bg(Color::Black);
            }
        }
    }
    let y = h / 2;
    let (l, r) = (g.score[0].to_string(), g.score[1].to_string());
    let x0 = 2 + l.len() as i32 + 2;
    let x1 = w - 3 - r.len() as i32 - 2;
    cv.text(2, y, &l, GREEN);
    cv.text(x1 + 3, y, &r, GREEN);
    cv.put(x0 - 1, y, '┃', DIM);
    cv.put(x1 + 1, y, '┃', DIM);
    let n = x1 - x0 + 1;
    let cell = |pos: f32| x0 + (pos.clamp(0.0, 1.0) * (n - 1) as f32).round() as i32;
    let left_end = cell(g.zone(Side::Left));
    let right_start = cell(1.0 - g.zone(Side::Right));
    for x in x0..=x1 {
        cv.put(x, y, '━', if x <= left_end || x >= right_start { DIM_AMBER } else { DIM });
    }
    let show = match g.phase {
        Phase::Point { t, .. } => (t * 6.0) as i32 % 2 == 0,
        Phase::Over { .. } => false,
        _ => true,
    };
    if show {
        cv.put(cell(g.pos), y, '●', GREEN);
    }
}
