//! A 3x5 block font for scores and titles, drawn either full height (five
//! rows of █) or squeezed into three rows of half blocks.

use ratatui::buffer::Buffer;
use ratatui::style::Color;

fn glyph(c: char) -> [&'static str; 5] {
    match c.to_ascii_uppercase() {
        '0' => ["###", "# #", "# #", "# #", "###"],
        '1' => [" # ", "## ", " # ", " # ", "###"],
        '2' => ["###", "  #", "###", "#  ", "###"],
        '3' => ["###", "  #", " ##", "  #", "###"],
        '4' => ["# #", "# #", "###", "  #", "  #"],
        '5' => ["###", "#  ", "###", "  #", "###"],
        '6' => ["###", "#  ", "###", "# #", "###"],
        '7' => ["###", "  #", "  #", " # ", " # "],
        '8' => ["###", "# #", "###", "# #", "###"],
        '9' => ["###", "# #", "###", "  #", "###"],
        'A' => [" # ", "# #", "###", "# #", "# #"],
        'B' => ["## ", "# #", "## ", "# #", "## "],
        'C' => [" ##", "#  ", "#  ", "#  ", " ##"],
        'D' => ["## ", "# #", "# #", "# #", "## "],
        'E' => ["###", "#  ", "## ", "#  ", "###"],
        'F' => ["###", "#  ", "## ", "#  ", "#  "],
        'G' => [" ##", "#  ", "# #", "# #", " ##"],
        'H' => ["# #", "# #", "###", "# #", "# #"],
        'I' => ["###", " # ", " # ", " # ", "###"],
        'J' => ["  #", "  #", "  #", "# #", " # "],
        'K' => ["# #", "# #", "## ", "# #", "# #"],
        'L' => ["#  ", "#  ", "#  ", "#  ", "###"],
        'M' => ["# #", "###", "###", "# #", "# #"],
        'N' => ["## ", "# #", "# #", "# #", "# #"],
        'O' => [" # ", "# #", "# #", "# #", " # "],
        'P' => ["## ", "# #", "## ", "#  ", "#  "],
        'Q' => [" # ", "# #", "# #", "## ", " ##"],
        'R' => ["## ", "# #", "## ", "# #", "# #"],
        'S' => [" ##", "#  ", " # ", "  #", "## "],
        'T' => ["###", " # ", " # ", " # ", " # "],
        'U' => ["# #", "# #", "# #", "# #", "###"],
        'V' => ["# #", "# #", "# #", "# #", " # "],
        'W' => ["# #", "# #", "###", "###", "# #"],
        'X' => ["# #", "# #", " # ", "# #", "# #"],
        'Y' => ["# #", "# #", " # ", " # ", " # "],
        'Z' => ["###", "  #", " # ", "#  ", "###"],
        '!' => [" # ", " # ", " # ", "   ", " # "],
        '-' => ["   ", "   ", "###", "   ", "   "],
        ':' => ["   ", " # ", "   ", " # ", "   "],
        _ => ["   "; 5],
    }
}

/// Width in cells of `text` at horizontal scale `sx`.
pub fn width(text: &str, sx: u16) -> u16 {
    let n = text.chars().count() as u16;
    if n == 0 {
        0
    } else {
        n * 3 * sx + (n - 1) * sx
    }
}

/// Rows the text takes: 5 full, 3 squeezed.
pub fn height(half: bool) -> u16 {
    if half {
        3
    } else {
        5
    }
}

pub fn draw(buf: &mut Buffer, x: i32, y: i32, text: &str, sx: u16, half: bool, fg: Color) {
    let area = buf.area;
    let mut put = |cx: i32, cy: i32, ch: char| {
        if cx >= area.x as i32 && cy >= area.y as i32 && cx < area.right() as i32 && cy < area.bottom() as i32 {
            if let Some(cell) = buf.cell_mut((cx as u16, cy as u16)) {
                cell.set_char(ch).set_fg(fg);
            }
        }
    };
    let mut gx = x;
    for c in text.chars() {
        let g = glyph(c);
        let on = |col: usize, row: usize| row < 5 && g[row].as_bytes()[col] == b'#';
        for col in 0..3 {
            for s in 0..sx as i32 {
                let cx = gx + col as i32 * sx as i32 + s;
                if half {
                    for r in 0..3 {
                        let (top, bot) = (on(col, r * 2), on(col, r * 2 + 1));
                        let ch = match (top, bot) {
                            (true, true) => '█',
                            (true, false) => '▀',
                            (false, true) => '▄',
                            _ => continue,
                        };
                        put(cx, y + r as i32, ch);
                    }
                } else {
                    for r in 0..5 {
                        if on(col, r) {
                            put(cx, y + r as i32, '█');
                        }
                    }
                }
            }
        }
        gx += 4 * sx as i32;
    }
}
