// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Painting for Polter's own windows (the workshop, the project manager): rectangles, lines,
//! bevels, brushed metal, text in the built-in font, lamps, display glass, small keys. It became
//! its own module when a second window needed it; games may use it too (a small UI is planned for
//! E3, RESEARCH.md).

use crate::canvas::Canvas;
use crate::font;

/// The colours of Polter's own windows, shared by the workshop and the project manager: edges,
/// ink, print, lamps, display glass.
pub mod colour {
    pub const EDGE_DARK: u32 = 0x08080a;
    pub const EDGE_LIGHT: u32 = 0x3b3d45;
    pub const INK: u32 = 0xdce3ee;
    pub const SILK: u32 = 0x949db0;
    pub const ACCENT: u32 = 0xff6a1f;
    pub const RED: u32 = 0xff3b2f;
    pub const GREEN: u32 = 0x63e86d;
    pub const AMBER: u32 = 0xffb43a;
    pub const TEAL: u32 = 0x7af5e1;
    pub const TEAL_DIM: u32 = 0x2e7a70;
    pub const GLASS: u32 = 0x061312;
}

use colour::*;

/// A rectangle of a picture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn contains(&self, (x, y): (i32, i32)) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

pub fn px(c: &mut Canvas, x: i32, y: i32, rgb: u32) {
    c.set(x, y, rgb);
}

pub fn fill(c: &mut Canvas, r: Rect, rgb: u32) {
    for y in r.y..r.y + r.h {
        hline(c, r.x, r.x + r.w - 1, y, rgb);
    }
}

pub fn hline(c: &mut Canvas, x0: i32, x1: i32, y: i32, rgb: u32) {
    if y < 0 || y >= c.height() as i32 {
        return;
    }
    for x in x0.max(0)..=x1.min(c.width() as i32 - 1) {
        c.set(x, y, rgb);
    }
}

pub fn vline(c: &mut Canvas, x: i32, y0: i32, y1: i32, rgb: u32) {
    if x < 0 || x >= c.width() as i32 {
        return;
    }
    for y in y0.max(0)..=y1.min(c.height() as i32 - 1) {
        c.set(x, y, rgb);
    }
}

/// A one-pixel border: top and left in one colour, bottom and right in the other (dark then
/// light reads as sunk in, light then dark as standing out).
pub fn bevel(c: &mut Canvas, r: Rect, top_left: u32, bottom_right: u32) {
    if r.w <= 0 || r.h <= 0 {
        return;
    }
    hline(c, r.x, r.x + r.w - 1, r.y, top_left);
    vline(c, r.x, r.y, r.y + r.h - 1, top_left);
    hline(c, r.x + 1, r.x + r.w - 1, r.y + r.h - 1, bottom_right);
    vline(c, r.x + r.w - 1, r.y + 1, r.y + r.h - 1, bottom_right);
}

/// Brushed metal: rows a shade apart, faint streaks along them.
pub fn brushed(c: &mut Canvas, r: Rect, base: u32) {
    let (w, h) = (c.width() as i32, c.height() as i32);
    for y in r.y.max(0)..(r.y + r.h).min(h) {
        let row = noise(7, y) / 2;
        for x in r.x.max(0)..(r.x + r.w).min(w) {
            let streak = noise(x / 11 + y * 3, y) / 3;
            c.set(x, y, add(base, row + streak));
        }
    }
}

/// A small silver button with an icon (the panel's handle).
pub fn small_key(c: &mut Canvas, r: Rect, icon: &[&str], hover: bool) {
    for yy in 0..r.h {
        let mut col = mix(0xe9ecf0, 0xa6acb5, yy as f32 / r.h as f32);
        if hover {
            col = mix(col, 0xffffff, 0.15);
        }
        hline(c, r.x, r.x + r.w - 1, r.y + yy, col);
    }
    bevel(c, r, 0xfbfcfd, 0x4a4e56);
    let (iw, ih) = (icon[0].len() as i32, icon.len() as i32);
    bitmap(
        c,
        icon,
        r.x + (r.w - iw) / 2,
        r.y + (r.h - ih) / 2,
        0x232428,
    );
}

/// Where a list stands: a thumb on a one-pixel track from `top` to `bottom`.
pub fn scrollbar(
    c: &mut Canvas,
    x: i32,
    top: i32,
    bottom: i32,
    first: usize,
    shown: usize,
    all: usize,
) {
    if all <= shown || shown == 0 {
        return;
    }
    let track = bottom - top;
    let size = (track * shown as i32 / all as i32).max(3);
    let at = top + (track - size) * first as i32 / (all - shown) as i32;
    vline(c, x, at, at + size, TEAL_DIM);
}

/// A key with a word on it, for Polter's own windows: silver, sunk while held, brighter under
/// the mouse, grey and flat when it can't be pressed.
pub fn key(c: &mut Canvas, r: Rect, label: &str, enabled: bool, hover: bool, down: bool) {
    let sink = if down && enabled { 1 } else { 0 };
    let face = Rect {
        x: r.x,
        y: r.y + sink,
        w: r.w,
        h: r.h - 1,
    };
    let (top, bottom) = if !enabled {
        (0x5a5e66, 0x464a51)
    } else if down {
        (0xc0c5cd, 0x878d96)
    } else {
        (0xe9ecf0, 0xa6acb5)
    };
    for yy in 0..face.h {
        let mut col = mix(top, bottom, yy as f32 / face.h.max(1) as f32);
        if hover && enabled {
            col = mix(col, 0xffffff, 0.15);
        }
        hline(c, face.x, face.x + face.w - 1, face.y + yy, col);
    }
    bevel(c, face, if enabled { 0xfbfcfd } else { 0x6e727a }, 0x34373d);
    if sink == 0 {
        hline(c, r.x, r.x + r.w - 1, r.y + r.h - 1, 0x1a1b1f);
    }
    let ink = if enabled { 0x232428 } else { 0x2c2e33 };
    text(
        c,
        label,
        face.x + (face.w - text_w(label)) / 2,
        face.y + (face.h - 5) / 2,
        ink,
    );
}

/// A small lamp; lit, it glows onto the metal around it.
pub fn led(c: &mut Canvas, x: i32, y: i32, col: u32, lit: bool) {
    if lit {
        for (dx, dy) in [
            (-1, 0),
            (3, 0),
            (-1, 1),
            (3, 1),
            (0, -1),
            (1, -1),
            (2, -1),
            (0, 2),
            (1, 2),
            (2, 2),
        ] {
            let under = c.get(x + dx, y + dy);
            px(c, x + dx, y + dy, mix(under, col, 0.35));
        }
        fill(c, Rect { x, y, w: 3, h: 2 }, col);
        px(c, x, y, mix(col, 0xffffff, 0.6));
    } else {
        fill(c, Rect { x, y, w: 3, h: 2 }, mix(col, 0x000000, 0.75));
    }
}

/// Glass of a display: dark, a fine mesh, a glint along the top, sunk into the metal.
pub fn glass(c: &mut Canvas, r: Rect) {
    fill(c, r, GLASS);
    for y in (r.y + 2..r.y + r.h - 1).step_by(2) {
        hline(c, r.x + 1, r.x + r.w - 2, y, 0x081816);
    }
    bevel(c, r, EDGE_DARK, EDGE_LIGHT);
    hline(c, r.x + 1, r.x + r.w - 2, r.y + 1, 0x12241f);
}

pub fn disc(c: &mut Canvas, cx: i32, cy: i32, r: f32, rgb: u32, clip: Option<Rect>) {
    let n = r.ceil() as i32;
    for dy in -n..=n {
        for dx in -n..=n {
            let d = (dx * dx + dy * dy) as f32;
            if d <= r * r + r * 0.6 && clip.is_none_or(|k| k.contains((cx + dx, cy + dy))) {
                px(c, cx + dx, cy + dy, rgb);
            }
        }
    }
}

pub fn ring(c: &mut Canvas, cx: i32, cy: i32, r: f32, rgb: u32, clip: Option<Rect>) {
    let n = r.ceil() as i32 + 1;
    for dy in -n..=n {
        for dx in -n..=n {
            let d = ((dx * dx + dy * dy) as f32).sqrt();
            if (d - r).abs() < 0.6 && clip.is_none_or(|k| k.contains((cx + dx, cy + dy))) {
                px(c, cx + dx, cy + dy, rgb);
            }
        }
    }
}

/// A straight line, painted only inside `clip`.
pub fn line(c: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32, rgb: u32, clip: Rect) {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    loop {
        if clip.contains((x, y)) {
            px(c, x, y, rgb);
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// A picture from rows of `#` and `.`.
pub fn bitmap(c: &mut Canvas, rows: &[&str], x: i32, y: i32, rgb: u32) {
    for (j, row) in rows.iter().enumerate() {
        for (i, ch) in row.chars().enumerate() {
            if ch == '#' {
                px(c, x + i as i32, y + j as i32, rgb);
            }
        }
    }
}

pub fn text_w(s: &str) -> i32 {
    (s.chars().count() as i32 * font::ADVANCE - 1).max(0)
}

pub fn text(c: &mut Canvas, s: &str, x: i32, y: i32, rgb: u32) {
    let all = Rect {
        x: 0,
        y: 0,
        w: c.width() as i32,
        h: c.height() as i32,
    };
    text_in(c, s, x, y, rgb, 1, all);
}

/// Text in the built-in font, each font pixel `scale` × `scale`, painted only inside `clip`.
pub fn text_in(c: &mut Canvas, s: &str, x: i32, y: i32, rgb: u32, scale: i32, clip: Rect) {
    for (i, ch) in s.chars().enumerate() {
        let gx = x + i as i32 * font::ADVANCE * scale;
        if gx > clip.x + clip.w {
            break;
        }
        for (row, bits) in font::glyph(ch).iter().enumerate() {
            for col in 0..3 {
                if bits & (0b100 >> col) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let (px_, py_) = (gx + col * scale + dx, y + row as i32 * scale + dy);
                        if clip.contains((px_, py_)) {
                            px(c, px_, py_, rgb);
                        }
                    }
                }
            }
        }
    }
}

/// A colour between two (t from 0 to 1).
pub fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |s: u32| {
        let (x, y) = (((a >> s) & 255) as f32, ((b >> s) & 255) as f32);
        ((x + (y - x) * t).round() as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}

/// A colour a few steps lighter or darker on every channel.
pub fn add(rgb: u32, d: i32) -> u32 {
    let ch = |s: u32| ((((rgb >> s) & 255) as i32 + d).clamp(0, 255) as u32) << s;
    ch(16) | ch(8) | ch(0)
}

/// Fixed noise from a position, from -3 to 4.
pub fn noise(x: i32, y: i32) -> i32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h >> 24) & 7) as i32 - 3
}
