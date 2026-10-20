// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The 320 × 180 screen and everything that draws on it. All drawing is clipped, so carts can
//! draw partly off-screen without checking bounds.

use crate::font;
use crate::palette::Col;
use crate::sprite::Sprite;

/// Screen width in pixels.
pub const WIDTH: i32 = 320;
/// Screen height in pixels.
pub const HEIGHT: i32 = 180;

/// The framebuffer: one palette slot per pixel. It keeps its contents between frames;
/// call [`Screen::cls`] at the start of `draw` to start fresh.
#[derive(Clone)]
pub struct Screen {
    px: Vec<Col>,
    missing: Vec<char>,
    cam: (i32, i32),
}

impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen {
    pub fn new() -> Screen {
        Screen {
            px: vec![Col::Black; (WIDTH * HEIGHT) as usize],
            missing: Vec::new(),
            cam: (0, 0),
        }
    }

    /// Shifts everything drawn afterwards by (-x, -y), like a camera. Whole pixels only, so a
    /// screen shake never smears. `camera(0, 0)` resets it; `cls` and `pget` ignore it.
    pub fn camera(&mut self, x: i32, y: i32) {
        self.cam = (x, y);
    }

    /// Fills the whole screen with one colour.
    pub fn cls(&mut self, c: Col) {
        self.px.fill(c);
    }

    /// Sets one pixel. Off-screen coordinates are ignored.
    ///
    /// Only palette colours exist, so raw RGB values don't compile:
    ///
    /// ```compile_fail
    /// let mut s = polter::Screen::new();
    /// s.pset(0, 0, 0xff00ff);
    /// ```
    #[inline]
    pub fn pset(&mut self, x: i32, y: i32, c: Col) {
        let (x, y) = (x - self.cam.0, y - self.cam.1);
        if (0..WIDTH).contains(&x) && (0..HEIGHT).contains(&y) {
            self.px[(y * WIDTH + x) as usize] = c;
        }
    }

    /// Reads one pixel. Off-screen coordinates read as [`Col::Black`].
    pub fn pget(&self, x: i32, y: i32) -> Col {
        if (0..WIDTH).contains(&x) && (0..HEIGHT).contains(&y) {
            self.px[(y * WIDTH + x) as usize]
        } else {
            Col::Black
        }
    }

    /// A filled rectangle with its top-left corner at (x, y).
    pub fn rect_fill(&mut self, x: i32, y: i32, w: i32, h: i32, c: Col) {
        let (x, y) = (x - self.cam.0, y - self.cam.1);
        let (x0, x1) = (x.max(0), (x + w).min(WIDTH));
        let (y0, y1) = (y.max(0), (y + h).min(HEIGHT));
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.px[(yy * WIDTH + xx) as usize] = c;
            }
        }
    }

    /// A one-pixel rectangle outline.
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Col) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.rect_fill(x, y, w, 1, c);
        self.rect_fill(x, y + h - 1, w, 1, c);
        self.rect_fill(x, y, 1, h, c);
        self.rect_fill(x + w - 1, y, 1, h, c);
    }

    /// A line from (x0, y0) to (x1, y1), both ends included (Bresenham).
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Col) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.pset(x, y, c);
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

    /// A circle outline around (cx, cy).
    pub fn circ(&mut self, cx: i32, cy: i32, r: i32, c: Col) {
        let (mut x, mut y, mut err) = (r, 0, 1 - r);
        while x >= y {
            for (px, py) in [
                (x, y),
                (y, x),
                (-y, x),
                (-x, y),
                (-x, -y),
                (-y, -x),
                (y, -x),
                (x, -y),
            ] {
                self.pset(cx + px, cy + py, c);
            }
            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }
    }

    /// A filled circle around (cx, cy).
    pub fn circ_fill(&mut self, cx: i32, cy: i32, r: i32, c: Col) {
        // Measuring against r + 0.5 gives flat caps instead of a lone pixel on top and bottom.
        let reach = (r as f32 + 0.5) * (r as f32 + 0.5);
        for dy in -r..=r {
            let half = (reach - (dy * dy) as f32).sqrt().floor() as i32;
            self.rect_fill(cx - half, cy + dy, 2 * half + 1, 1, c);
        }
    }

    /// Draws text with the built-in 3×5 font and returns the x position after the last character.
    pub fn text(&mut self, s: &str, x: i32, y: i32, c: Col) -> i32 {
        let mut cx = x;
        for ch in s.chars() {
            if !font::has_glyph(ch) && !self.missing.contains(&ch) {
                self.missing.push(ch);
            }
            let g = font::glyph(ch);
            for (row, bits) in g.iter().enumerate() {
                for col in 0..3 {
                    if bits & (0b100 >> col) != 0 {
                        self.pset(cx + col, y + row as i32, c);
                    }
                }
            }
            cx += font::ADVANCE;
        }
        cx
    }

    /// Draws text with every font pixel as a `scale` × `scale` block, for titles.
    /// Returns the x position after the last character.
    pub fn text_scaled(&mut self, s: &str, x: i32, y: i32, c: Col, scale: i32) -> i32 {
        let mut cx = x;
        for ch in s.chars() {
            if !font::has_glyph(ch) && !self.missing.contains(&ch) {
                self.missing.push(ch);
            }
            for (row, bits) in font::glyph(ch).iter().enumerate() {
                for col in 0..3 {
                    if bits & (0b100 >> col) != 0 {
                        self.rect_fill(cx + col * scale, y + row as i32 * scale, scale, scale, c);
                    }
                }
            }
            cx += font::ADVANCE * scale;
        }
        cx
    }

    /// Width of a scaled string in pixels.
    pub fn text_width_scaled(s: &str, scale: i32) -> i32 {
        Self::text_width(s) * scale
    }

    /// Width of a string in pixels (without the trailing spacing).
    pub fn text_width(s: &str) -> i32 {
        (s.chars().count() as i32 * font::ADVANCE - 1).max(0)
    }

    /// Draws a sprite with its top-left corner at (x, y); transparent pixels are skipped.
    pub fn sprite(&mut self, spr: &Sprite, x: i32, y: i32, flip_x: bool) {
        for sy in 0..spr.h {
            for sx in 0..spr.w {
                let src = if flip_x { spr.w - 1 - sx } else { sx };
                if let Some(c) = spr.get(src, sy) {
                    self.pset(x + sx, y + sy, c);
                }
            }
        }
    }

    /// Characters that were drawn but aren't in the font (they show as boxes).
    /// `check` reports them, so a typo in a text never slips through unseen.
    pub fn missing_glyphs(&self) -> &[char] {
        &self.missing
    }

    /// All pixels, row by row.
    pub fn pixels(&self) -> &[Col] {
        &self.px
    }

    /// FNV-1a hash of the screen: two runs that draw the same frame produce the same hash.
    pub fn hash(&self) -> u64 {
        self.px.iter().fold(0xcbf29ce484222325u64, |h, &c| {
            (h ^ c.0 as u64).wrapping_mul(0x100000001b3)
        })
    }
}
