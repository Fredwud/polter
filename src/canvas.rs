// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The canvas: a true-colour picture that every game draws into, once per frame. Pixels are
//! 0RGB (`0x00rrggbb`), computed on the CPU, so a frame is the same bytes with a window and
//! without one, and a picture can be a test.

use crate::image::Image;

/// A true-colour picture, row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canvas {
    w: usize,
    h: usize,
    px: Vec<u32>,
}

impl Canvas {
    /// A black canvas.
    pub fn new(w: usize, h: usize) -> Canvas {
        Canvas {
            w,
            h,
            px: vec![0; w * h],
        }
    }

    pub fn width(&self) -> usize {
        self.w
    }

    pub fn height(&self) -> usize {
        self.h
    }

    /// All pixels, row by row, as 0RGB.
    pub fn pixels(&self) -> &[u32] {
        &self.px
    }

    /// All pixels, to draw into directly (the fastest way, for games that shade every pixel).
    pub fn pixels_mut(&mut self) -> &mut [u32] {
        &mut self.px
    }

    /// Fills the whole canvas with one colour.
    pub fn clear(&mut self, rgb: u32) {
        self.px.fill(rgb);
    }

    /// Sets a pixel; outside the canvas nothing happens.
    pub fn set(&mut self, x: i32, y: i32, rgb: u32) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize] = rgb;
        }
    }

    /// Fills a rectangle (clipped to the canvas).
    pub fn rect(&mut self, x: i32, y: i32, w: usize, h: usize, rgb: u32) {
        let (x0, y0) = (x.max(0) as usize, y.max(0) as usize);
        let x1 = (x + w as i32).clamp(0, self.w as i32) as usize;
        let y1 = (y + h as i32).clamp(0, self.h as i32) as usize;
        for row in y0..y1 {
            if x0 < x1 {
                self.px[row * self.w + x0..row * self.w + x1].fill(rgb);
            }
        }
    }

    /// Draws a picture with its top left at (x, y), clipped to the canvas: see-through pixels
    /// leave what is below them, half see-through ones mix with it (in whole numbers, so the
    /// same on every machine).
    pub fn draw(&mut self, image: &Image, x: i32, y: i32) {
        self.draw_with(image, x, y, false);
    }

    /// The same, mirrored left to right: a figure facing the other way.
    pub fn draw_flipped(&mut self, image: &Image, x: i32, y: i32) {
        self.draw_with(image, x, y, true);
    }

    fn draw_with(&mut self, image: &Image, x: i32, y: i32, flip: bool) {
        let (iw, ih) = (image.width as i32, image.height as i32);
        let (x0, x1) = ((-x).max(0), iw.min(self.w as i32 - x));
        let (y0, y1) = ((-y).max(0), ih.min(self.h as i32 - y));
        for sy in y0..y1 {
            let row = (y + sy) as usize * self.w;
            for sx in x0..x1 {
                let from = if flip { iw - 1 - sx } else { sx };
                let p = image.pixels[(sy * iw + from) as usize];
                let a = p >> 24;
                if a == 0 {
                    continue;
                }
                let at = row + (x + sx) as usize;
                self.px[at] = if a == 255 {
                    p & 0x00ff_ffff
                } else {
                    mix(self.px[at], p, a)
                };
            }
        }
    }

    /// A pixel's colour; outside the canvas, black.
    pub fn get(&self, x: i32, y: i32) -> u32 {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize]
        } else {
            0
        }
    }

    /// FNV-1a over the pixels' red, green and blue bytes, row by row: the same picture gives the
    /// same hash, whatever drew it (the golden frames in tests/golden use this hash).
    pub fn hash(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for &p in &self.px {
            for b in [(p >> 16) as u8, (p >> 8) as u8, p as u8] {
                h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        h
    }

    /// The picture as RGB bytes, enlarged by a whole number (nearest neighbour, of course).
    pub fn to_rgb(&self, scale: usize) -> (usize, usize, Vec<u8>) {
        let scale = scale.max(1);
        let (w, h) = (self.w * scale, self.h * scale);
        let mut out = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            let row = &self.px[(y / scale) * self.w..(y / scale + 1) * self.w];
            for x in 0..w {
                let p = row[x / scale];
                out.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
            }
        }
        (w, h, out)
    }

    /// The picture as a PNG file's bytes.
    pub fn to_png(&self, scale: usize) -> Vec<u8> {
        let (w, h, rgb) = self.to_rgb(scale);
        crate::png::encode(w, h, &rgb)
    }
}

/// `under` with `over` on top at opacity `a` (0 to 255), channel by channel, rounded.
fn mix(under: u32, over: u32, a: u32) -> u32 {
    let ch = |s: u32| {
        let (d, o) = ((under >> s) & 255, (over >> s) & 255);
        ((o * a + d * (255 - a) + 127) / 255) << s
    };
    ch(16) | ch(8) | ch(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_draw_with_their_transparency_clipped_and_mirrored() {
        let mut img = Image::new(3, 2);
        // solid red, see-through, half see-through white; a solid blue row below
        img.pixels = vec![
            0xffff_0000,
            0x0012_3456,
            0x80ff_ffff,
            0xff00_00ff,
            0xff00_00ff,
            0xff00_00ff,
        ];
        let mut c = Canvas::new(4, 3);
        c.clear(0x000000);
        c.draw(&img, 1, 1);
        assert_eq!(c.get(1, 1), 0xff0000);
        assert_eq!(c.get(2, 1), 0x000000, "see-through leaves what is below");
        assert_eq!(c.get(3, 1), 0x808080, "half white on black is middle grey");
        assert_eq!(c.get(1, 2), 0x0000ff);
        assert_eq!(c.get(0, 0), 0x000000);
        // mirrored: the red pixel lands on the right
        let mut m = Canvas::new(3, 2);
        m.draw_flipped(&img, 0, 0);
        assert_eq!(m.get(2, 0), 0xff0000);
        assert_eq!(m.get(0, 0), 0x808080);
        // half outside, on every side: no panic, the inside drawn
        for (x, y) in [(-2, -1), (3, 2), (-5, 0), (0, -5), (4, 3)] {
            let mut e = Canvas::new(4, 3);
            e.draw(&img, x, y);
            e.draw_flipped(&img, x, y);
        }
        let mut e = Canvas::new(4, 3);
        e.draw(&img, -2, -1);
        assert_eq!(
            e.get(0, 0),
            0x0000ff,
            "the blue row's last pixel, at the corner"
        );
        // rectangles clip too
        let mut r = Canvas::new(4, 3);
        r.rect(-1, 1, 3, 9, 0x00ff00);
        assert_eq!(
            (r.get(0, 1), r.get(1, 2), r.get(2, 1), r.get(0, 0)),
            (0x00ff00, 0x00ff00, 0, 0)
        );
    }

    #[test]
    fn pixels_outside_are_ignored_and_read_black() {
        let mut c = Canvas::new(4, 3);
        c.set(-1, 0, 0xffffff);
        c.set(4, 0, 0xffffff);
        c.set(3, 2, 0x123456);
        assert_eq!(c.get(3, 2), 0x123456);
        assert_eq!(c.get(9, 9), 0);
        assert_eq!(c.pixels().iter().filter(|&&p| p != 0).count(), 1);
    }

    #[test]
    fn the_hash_is_fnv_over_rgb_bytes_and_sees_one_pixel() {
        // FNV-1a of no bytes is the offset basis; of three zero bytes, a known value
        assert_eq!(Canvas::new(0, 0).hash(), 0xcbf2_9ce4_8422_2325);
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for _ in 0..3 {
            // a zero byte: the XOR changes nothing, only the multiplication
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        assert_eq!(Canvas::new(1, 1).hash(), h);
        let (a, mut b) = (Canvas::new(8, 8), Canvas::new(8, 8));
        b.set(5, 5, 0x000001);
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn scaling_repeats_each_pixel() {
        let mut c = Canvas::new(2, 1);
        c.set(1, 0, 0x0a0b0c);
        let (w, h, rgb) = c.to_rgb(3);
        assert_eq!((w, h), (6, 3));
        assert_eq!(&rgb[3 * 3..3 * 3 + 3], &[10, 11, 12]);
        assert_eq!(&rgb[(6 * 2 + 5) * 3..(6 * 2 + 5) * 3 + 3], &[10, 11, 12]);
        assert_eq!(&rgb[0..3], &[0, 0, 0]);
    }
}
