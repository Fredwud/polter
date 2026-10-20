// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pictures with transparency: what a game draws from its PNG files (sprites, tiles, sheets).
//! Pixels are `0xAARRGGBB`, row by row; alpha 0 is see-through, 255 is solid.

/// A picture: its size and its pixels as `0xAARRGGBB`, row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
}

impl Image {
    /// A see-through picture of this size.
    pub fn new(width: usize, height: usize) -> Image {
        Image {
            width,
            height,
            pixels: vec![0; width * height],
        }
    }

    /// Reads a PNG file's bytes (every colour type and bit depth, interlaced or not).
    pub fn from_png(bytes: &[u8]) -> Result<Image, String> {
        crate::png::decode(bytes)
    }

    /// Reads a PNG file; the error names the file.
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Image, String> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Image::from_png(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The pixel at (x, y), or see-through outside.
    pub fn get(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return 0;
        }
        self.pixels[y as usize * self.width + x as usize]
    }

    /// A part of the picture as a picture of its own (a frame of a sheet, a tile of a set).
    /// What lies outside the picture is see-through.
    pub fn part(&self, x: i32, y: i32, w: usize, h: usize) -> Image {
        let mut out = Image::new(w, h);
        for dy in 0..h {
            for dx in 0..w {
                out.pixels[dy * w + dx] = self.get(x + dx as i32, y + dy as i32);
            }
        }
        out
    }

    /// The picture cut into frames of `w` × `h`, row by row (a sprite sheet laid out in a grid).
    pub fn frames(&self, w: usize, h: usize) -> Vec<Image> {
        if w == 0 || h == 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for row in 0..self.height / h {
            for col in 0..self.width / w {
                out.push(self.part((col * w) as i32, (row * h) as i32, w, h));
            }
        }
        out
    }
}
