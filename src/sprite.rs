// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Sprites, written as rows of colour codes.

use crate::palette::Col;

/// A sprite: a small image with transparent pixels.
#[derive(Clone, Debug)]
pub struct Sprite {
    pub w: i32,
    pub h: i32,
    px: Vec<Option<Col>>,
}

impl Sprite {
    /// Builds a sprite from rows of colour codes: `K G S C W Y O R` for the eight palette slots,
    /// `.` for transparent.
    ///
    /// ```
    /// let s = polter::Sprite::from_rows(&[".KK.", "KOOK", ".KK."]);
    /// assert_eq!((s.w, s.h), (4, 3));
    /// ```
    ///
    /// Sprites are drawn by hand, so a typo panics with a clear message instead of drawing garbage.
    pub fn from_rows(rows: &[&str]) -> Sprite {
        let w = rows.first().map_or(0, |r| r.chars().count());
        let mut px = Vec::with_capacity(w * rows.len());
        for (y, row) in rows.iter().enumerate() {
            assert_eq!(
                row.chars().count(),
                w,
                "sprite row {y} is {} wide, expected {w}: {row:?}",
                row.chars().count()
            );
            for (x, ch) in row.chars().enumerate() {
                px.push(match ch {
                    '.' => None,
                    _ => Some(Col::from_code(ch).unwrap_or_else(|| {
                        panic!("sprite row {y}, column {x}: unknown colour code {ch:?}")
                    })),
                });
            }
        }
        Sprite {
            w: w as i32,
            h: rows.len() as i32,
            px,
        }
    }

    /// Like `from_rows`, with a legend for any of the 256 palette slots: each listed character
    /// stands for its colour; unlisted characters fall back to the standard codes.
    ///
    /// ```
    /// use polter::{Col, Sprite};
    /// let s = Sprite::from_rows_with(&["ab", "bK"], &[('a', Col(40)), ('b', Col(41))]);
    /// assert_eq!(s.get(0, 0), Some(Col(40)));
    /// assert_eq!(s.get(1, 1), Some(Col::Black));
    /// ```
    pub fn from_rows_with(rows: &[&str], legend: &[(char, Col)]) -> Sprite {
        let w = rows.first().map_or(0, |r| r.chars().count());
        let mut px = Vec::with_capacity(w * rows.len());
        for (y, row) in rows.iter().enumerate() {
            assert_eq!(
                row.chars().count(),
                w,
                "sprite row {y} is {} wide, expected {w}: {row:?}",
                row.chars().count()
            );
            for (x, ch) in row.chars().enumerate() {
                px.push(match ch {
                    '.' => None,
                    _ => Some(legend.iter().find(|(k, _)| *k == ch).map(|(_, c)| *c).or_else(|| Col::from_code(ch)).unwrap_or_else(|| {
                        panic!("sprite row {y}, column {x}: {ch:?} is neither in the legend nor a colour code")
                    })),
                });
            }
        }
        Sprite {
            w: w as i32,
            h: rows.len() as i32,
            px,
        }
    }

    /// The pixel at (x, y), or `None` if it is transparent or outside the sprite.
    pub fn get(&self, x: i32, y: i32) -> Option<Col> {
        if (0..self.w).contains(&x) && (0..self.h).contains(&y) {
            self.px[(y * self.w + x) as usize]
        } else {
            None
        }
    }
}
