//! Original terminal-cell artwork. All colours come from the shared theme.
use crate::theme::Theme;
use ratatui::{Frame, layout::Rect, style::Color};

const STAGEHAND: [&str; 18] = [
    "      hhhh      ",
    "     hhhhhhh    ",
    "    hhhhhhhhh   ",
    "   hhhhhhhhhhh  ",
    "    sssssssss   ",
    "    swwsswwss   ",
    "    swpsswpss   ",
    "    sssnnssss   ",
    "     ssmmsss    ",
    "      sssss     ",
    "   bbbbbbbbbb   ",
    "  ssbbbbbbbbss  ",
    "  ssbbwhhwbbss  ",
    "    bbbbbbbb    ",
    "     bb  bb     ",
    "     bb  bb     ",
    "    ppp  ppp    ",
    "                ",
];
const LEAF: [&str; 18] = [
    "     h    h     ",
    "    hhh  hhh    ",
    "      hhhh      ",
    "    ssssssss    ",
    "   ssssssssss   ",
    "   swwsswwsss   ",
    "   swpsswpsss   ",
    "   ssssnnssss   ",
    "    ssmmssss    ",
    "     ssssss     ",
    "  h  bbbbbb  h  ",
    "  hhbbbbbbbbhh  ",
    "    bbbbbbbb    ",
    "     bbbbbb     ",
    "     bb  bb     ",
    "    hbb  bbh    ",
    "   pppp  pppp   ",
    "                ",
];
const MONSTER: [&str; 18] = [
    "     h h h      ",
    "   hhhhhhhhh    ",
    "  hsssssssssh   ",
    " hsssssssssssh  ",
    " hsswwsswwsssh  ",
    " hsswpsswpsssh  ",
    " hssssnnsssssh  ",
    "  hssmmmmsssh   ",
    "   sswwwwsss    ",
    "    sssssss     ",
    " ssbbbbbbbbbss  ",
    " ssbbwwwwwbbss  ",
    "   bbbbbbbbb    ",
    "    bbbbbbb     ",
    "    bb   bb     ",
    "    ss   ss     ",
    "   ppp   ppp    ",
    "                ",
];
const ROBOT: [&str; 18] = [
    "       h        ",
    "      hhh       ",
    "       h        ",
    "    ssssssss    ",
    "   ssssssssss   ",
    "   swwsswwsss   ",
    "   swpsswpsss   ",
    "   ssssssssss   ",
    "    ssmmmsss    ",
    "     ssssss     ",
    "   bbbbbbbbbb   ",
    "  ssbbhwhwbbss  ",
    "  ssbbbbbbbbss  ",
    "    bbbbbbbb    ",
    "     bb  bb     ",
    "     ss  ss     ",
    "    ppp  ppp    ",
    "                ",
];

pub fn seed(id: &str) -> usize {
    id.bytes()
        .fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize))
}
pub fn moppet(f: &mut Frame, area: Rect, t: &Theme, identity: usize, state: &str, frame: usize) {
    let mask = match identity % 4 {
        0 => &STAGEHAND,
        1 => &LEAF,
        2 => &MONSTER,
        _ => &ROBOT,
    };
    let skin = [t.yellow, t.green, t.magenta, t.blue][identity % 4];
    let hair = [t.blue, t.yellow, t.yellow, t.magenta][identity % 4];
    let colour = |ch| match ch {
        's' => skin,
        'h' => hair,
        'b' => t.accent,
        'w' => t.bg,
        'p' | 'm' => t.fg,
        'n' => t.red,
        _ => t.bg,
    };
    let mut pixels = [[b' '; 16]; 20];
    let moving = state == "working" && frame % 2 == 1;
    for (y, line) in mask.iter().enumerate() {
        for (x, ch) in line.bytes().take(16).enumerate() {
            pixels[y + usize::from(moving)][x] = ch;
        }
    }
    if state == "working" && frame.is_multiple_of(2) {
        for (y, x) in [(8, 14), (9, 13), (10, 12)] {
            pixels[y][x] = b's';
        }
    }
    if state == "blocked" {
        pixels[5][5] = b'h';
        pixels[5][9] = b'h';
    }
    let scale = if area.width >= 32 { 2 } else { 1 };
    let height = usize::from(area.height.min(10));
    for row in 0..height {
        let y = row * 20 / height;
        let bottom_y = ((row * 2 + 1) * 20 / (height * 2)).min(19);
        for x in 0..16 {
            if x * scale >= area.width as usize {
                continue;
            }
            let top = pixels[y][x];
            let bottom = pixels[bottom_y][x];
            if top == b' ' && bottom == b' ' {
                continue;
            }
            for dx in 0..scale {
                let col = area.x + (x * scale + dx) as u16;
                if col >= area.right() {
                    continue;
                }
                f.buffer_mut()[(col, area.y + row as u16)]
                    .set_symbol("▀")
                    .set_fg(colour(top as char))
                    .set_bg(colour(bottom as char));
            }
        }
    }
}

fn glyph(c: char) -> [&'static str; 5] {
    match c {
        'A' => ["01110", "11011", "11111", "11011", "11011"],
        'B' => ["11110", "11011", "11110", "11011", "11110"],
        'C' => ["01111", "11000", "11000", "11000", "01111"],
        'E' => ["11111", "11000", "11110", "11000", "11111"],
        'G' => ["01111", "11000", "11011", "11011", "01111"],
        'H' => ["11011", "11011", "11111", "11011", "11011"],
        'K' => ["11011", "11110", "11100", "11110", "11011"],
        'M' => ["10001", "11011", "10101", "10001", "10001"],
        'S' => ["01111", "11000", "01110", "00011", "11110"],
        'T' => ["11111", "00100", "00100", "00100", "00100"],
        'Y' => ["11011", "11011", "01110", "00100", "00100"],
        '0' => ["01110", "11011", "11011", "11011", "01110"],
        '1' => ["00100", "01100", "00100", "00100", "01110"],
        '2' => ["11110", "00011", "01110", "11000", "11111"],
        '3' => ["11110", "00011", "01110", "00011", "11110"],
        '4' => ["11011", "11011", "11111", "00011", "00011"],
        '5' => ["11111", "11000", "11110", "00011", "11110"],
        '6' => ["01110", "11000", "11110", "11011", "01110"],
        '7' => ["11111", "00011", "00110", "01100", "01100"],
        '8' => ["01110", "11011", "01110", "11011", "01110"],
        '9' => ["01110", "11011", "01111", "00011", "01110"],
        _ => ["00000", "00000", "11111", "00000", "00000"],
    }
}
pub fn big(f: &mut Frame, area: Rect, label: &str, ink: Color) {
    let scale = if area.width as usize >= label.len() * 12 {
        2
    } else {
        1
    };
    for (index, c) in label.chars().enumerate() {
        for (y, row) in glyph(c).iter().enumerate() {
            for (x, b) in row.bytes().enumerate() {
                if b != b'1' {
                    continue;
                }
                for dx in 0..scale {
                    let col = index * 6 * scale + x * scale + dx;
                    if col < area.width as usize && y < area.height as usize {
                        f.buffer_mut()[(area.x + col as u16, area.y + y as u16)]
                            .set_symbol("█")
                            .set_fg(ink);
                    }
                }
            }
        }
    }
}
