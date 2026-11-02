//! The whole screen is one row: the scores, the posts, the line and the
//! ball, all in a single line across the middle of an otherwise empty
//! terminal. Everything else is to come, a little at a time.

use crate::app::App;
use crate::game::{Phase, Side};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

const LINE: Color = Color::Rgb(40, 150, 70);
const ZONE: Color = Color::Rgb(150, 100, 0);
const BALL: Color = Color::Rgb(90, 255, 130);
const SCORE: Color = Color::Rgb(60, 200, 100);

pub fn draw(app: &App, area: Rect, buf: &mut Buffer) {
    buf.set_style(area, Style::new().bg(Color::Black).fg(LINE).remove_modifier(Modifier::all()));
    let g = &app.game;
    let y = area.top() + area.height / 2;
    let w = area.width as i32;
    let mut put = |x: i32, ch: char, fg: Color| {
        if x >= 0 && x < w {
            if let Some(c) = buf.cell_mut((area.left() + x as u16, y)) {
                c.set_char(ch).set_fg(fg);
            }
        }
    };

    // "7 ┃━━━━━━━━━━━━━━━━━━━━┃ 3"
    let (l, r) = (g.score[0].to_string(), g.score[1].to_string());
    let x0 = 2 + l.len() as i32 + 2;
    let x1 = w - 3 - r.len() as i32 - 2;
    if x1 - x0 < 8 {
        return;
    }
    for (i, ch) in l.chars().enumerate() {
        put(2 + i as i32, ch, SCORE);
    }
    for (i, ch) in r.chars().enumerate() {
        put(x1 + 3 + i as i32, ch, SCORE);
    }
    put(x0 - 1, '┃', LINE);
    put(x1 + 1, '┃', LINE);

    let n = x1 - x0 + 1;
    let cell = |pos: f32| x0 + (pos.clamp(0.0, 1.0) * (n - 1) as f32).round() as i32;
    let left_end = cell(g.zone(Side::Left));
    let right_start = cell(1.0 - g.zone(Side::Right));
    for x in x0..=x1 {
        put(x, '━', if x <= left_end || x >= right_start { ZONE } else { LINE });
    }

    let blink = |t: f32, hz: f32| (t * hz) as i32 % 2 == 0;
    let show_ball = match g.phase {
        Phase::Point { t, .. } => blink(t, 6.0),
        Phase::Over { .. } => false,
        _ => true,
    };
    if show_ball {
        put(cell(g.pos), '●', BALL);
    }
}
