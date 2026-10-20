// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Tiles, a camera, and boxes that collide with tiles. A level can be text, a character a tile,
//! so an agent reads and writes it without an editor:
//!
//! ```
//! use polter::tiles::TileMap;
//! let map = TileMap::from_text("#....#\n#.##.#\n######", 8, &[('#', Some(0)), ('.', None)]).unwrap();
//! assert!(map.solid_at(0, 0) && !map.solid_at(1, 0));
//! // a box 6 × 6 at x = 9 (over column 1, empty in row 1) falls onto row 2, which starts at 16
//! let m = map.move_box(9.0, 0.0, 6.0, 6.0, 0.0, 20.0);
//! assert_eq!((m.y, m.hit_y), (10.0, true));
//! ```

use crate::canvas::Canvas;
use crate::image::Image;

/// A grid of tiles: which tile is where (none, or an index into a tileset), and which tiles
/// block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileMap {
    /// The size in tiles, and a tile's side in pixels.
    pub width: usize,
    pub height: usize,
    pub size: usize,
    /// Row by row: the tile's index plus one, 0 for none.
    cells: Vec<u16>,
    /// By tile index: does it block? (Indices past its end block.)
    pub solid: Vec<bool>,
    /// Whether the world past the map's edges blocks (walls all round) or not.
    pub outside_solid: bool,
}

/// Where a box ended after a move, and what stopped it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moved {
    pub x: f32,
    pub y: f32,
    /// It ran into something sideways, or up or down.
    pub hit_x: bool,
    pub hit_y: bool,
}

impl TileMap {
    /// An empty map; every tile blocks once it is placed, until `solid` says otherwise.
    pub fn new(width: usize, height: usize, size: usize) -> TileMap {
        TileMap {
            width,
            height,
            size: size.max(1),
            cells: vec![0; width * height],
            solid: Vec::new(),
            outside_solid: true,
        }
    }

    /// A map from text: one line a row, one character a tile, the legend saying which tile a
    /// character is (`None`: empty). Lines may differ in length (the rest is empty). Errors name
    /// the line and column of a character the legend lacks.
    pub fn from_text(
        text: &str,
        size: usize,
        legend: &[(char, Option<u16>)],
    ) -> Result<TileMap, String> {
        let lines: Vec<&str> = text.lines().collect();
        let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let mut map = TileMap::new(width, lines.len(), size);
        for (y, line) in lines.iter().enumerate() {
            for (x, ch) in line.chars().enumerate() {
                let Some(&(_, tile)) = legend.iter().find(|(c, _)| *c == ch) else {
                    return Err(format!(
                        "line {}, column {}: {ch:?} is not in the legend",
                        y + 1,
                        x + 1
                    ));
                };
                map.set(x as i32, y as i32, tile);
            }
        }
        Ok(map)
    }

    /// The tile at (tx, ty), in tiles; none outside.
    pub fn get(&self, tx: i32, ty: i32) -> Option<u16> {
        if !self.inside(tx, ty) {
            return None;
        }
        match self.cells[ty as usize * self.width + tx as usize] {
            0 => None,
            k => Some(k - 1),
        }
    }

    /// Places a tile (or clears it with `None`); outside the map nothing happens.
    pub fn set(&mut self, tx: i32, ty: i32, tile: Option<u16>) {
        if self.inside(tx, ty) {
            self.cells[ty as usize * self.width + tx as usize] = tile.map_or(0, |t| t + 1);
        }
    }

    fn inside(&self, tx: i32, ty: i32) -> bool {
        tx >= 0 && ty >= 0 && (tx as usize) < self.width && (ty as usize) < self.height
    }

    /// Does the tile at (tx, ty) block?
    pub fn solid_at(&self, tx: i32, ty: i32) -> bool {
        if !self.inside(tx, ty) {
            return self.outside_solid;
        }
        self.get(tx, ty)
            .is_some_and(|t| self.solid.get(t as usize).copied().unwrap_or(true))
    }

    /// The tiles a span of pixels covers (from `a`, `len` long; touching an edge is not covering).
    fn span(&self, a: f32, len: f32) -> (i32, i32) {
        let s = self.size as f32;
        ((a / s).floor() as i32, ((a + len) / s).ceil() as i32 - 1)
    }

    /// Is any tile under the box solid?
    pub fn overlaps(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        let (x0, x1) = self.span(x, w);
        let (y0, y1) = self.span(y, h);
        (y0..=y1).any(|ty| (x0..=x1).any(|tx| self.solid_at(tx, ty)))
    }

    /// Moves a box (x, y, w, h in pixels) by (dx, dy) against the solid tiles: first sideways,
    /// then up or down, each in steps of at most half a tile so that nothing passes through a
    /// thin wall. It stops flush against what it hits.
    pub fn move_box(&self, x: f32, y: f32, w: f32, h: f32, dx: f32, dy: f32) -> Moved {
        let s = self.size as f32;
        let steps = ((dx.abs().max(dy.abs()) / (s / 2.0)).ceil() as u32).max(1);
        let (sx, sy) = (dx / steps as f32, dy / steps as f32);
        let mut m = Moved {
            x,
            y,
            hit_x: false,
            hit_y: false,
        };
        for _ in 0..steps {
            if !m.hit_x && sx != 0.0 {
                let nx = m.x + sx;
                if self.overlaps(nx, m.y, w, h) {
                    // flush against the tile edge it ran into
                    let (y0, y1) = self.span(m.y, h);
                    let (c0, c1) = self.span(nx, w);
                    let blocked = |tx: i32| (y0..=y1).any(|ty| self.solid_at(tx, ty));
                    m.x = if sx > 0.0 {
                        let col = (c0..=c1).find(|&c| blocked(c)).unwrap_or(c1);
                        col as f32 * s - w
                    } else {
                        let col = (c0..=c1).rev().find(|&c| blocked(c)).unwrap_or(c0);
                        (col + 1) as f32 * s
                    };
                    m.hit_x = true;
                } else {
                    m.x = nx;
                }
            }
            if !m.hit_y && sy != 0.0 {
                let ny = m.y + sy;
                if self.overlaps(m.x, ny, w, h) {
                    let (x0, x1) = self.span(m.x, w);
                    let (r0, r1) = self.span(ny, h);
                    let blocked = |ty: i32| (x0..=x1).any(|tx| self.solid_at(tx, ty));
                    m.y = if sy > 0.0 {
                        let row = (r0..=r1).find(|&r| blocked(r)).unwrap_or(r1);
                        row as f32 * s - h
                    } else {
                        let row = (r0..=r1).rev().find(|&r| blocked(r)).unwrap_or(r0);
                        (row + 1) as f32 * s
                    };
                    m.hit_y = true;
                } else {
                    m.y = ny;
                }
            }
        }
        m
    }

    /// Draws the tiles the view shows: `view` is the camera's top left in pixels, so a tile at
    /// (tx, ty) lands at (tx·size − view.0, ty·size − view.1). Tiles past the tileset's end are
    /// skipped.
    pub fn draw(&self, canvas: &mut Canvas, tileset: &[Image], view: (i32, i32)) {
        let s = self.size as i32;
        let (tx0, ty0) = (view.0.div_euclid(s), view.1.div_euclid(s));
        let tx1 = (view.0 + canvas.width() as i32).div_euclid(s);
        let ty1 = (view.1 + canvas.height() as i32).div_euclid(s);
        for ty in ty0.max(0)..=ty1.min(self.height as i32 - 1) {
            for tx in tx0.max(0)..=tx1.min(self.width as i32 - 1) {
                if let Some(image) = self.get(tx, ty).and_then(|t| tileset.get(t as usize)) {
                    canvas.draw(image, tx * s - view.0, ty * s - view.1);
                }
            }
        }
    }
}

/// A view onto a world bigger than the screen: it follows a point softly and stays inside the
/// world's bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// The view's top left in the world, in pixels.
    pub x: f32,
    pub y: f32,
    /// The view's size.
    pub w: f32,
    pub h: f32,
}

impl Camera {
    pub fn new(w: usize, h: usize) -> Camera {
        Camera {
            x: 0.0,
            y: 0.0,
            w: w as f32,
            h: h as f32,
        }
    }

    /// Moves a share of the way (0: not at all, 1: at once) towards centring (tx, ty).
    pub fn follow(&mut self, tx: f32, ty: f32, share: f32) {
        let k = share.clamp(0.0, 1.0);
        self.x += (tx - self.w / 2.0 - self.x) * k;
        self.y += (ty - self.h / 2.0 - self.y) * k;
    }

    /// Keeps the view inside a world of this size (centred where the world is smaller).
    pub fn keep_inside(&mut self, world_w: f32, world_h: f32) {
        self.x = if world_w <= self.w {
            (world_w - self.w) / 2.0
        } else {
            self.x.clamp(0.0, world_w - self.w)
        };
        self.y = if world_h <= self.h {
            (world_h - self.h) / 2.0
        } else {
            self.y.clamp(0.0, world_h - self.h)
        };
    }

    /// The view's top left in whole pixels, for drawing (`map.draw(canvas, &tiles, cam.view())`,
    /// a sprite at world (x, y) at `x - view.0`).
    pub fn view(&self) -> (i32, i32) {
        (self.x.round() as i32, self.y.round() as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEGEND: &[(char, Option<u16>)] = &[('#', Some(0)), ('.', None), ('=', Some(1))];

    fn room() -> TileMap {
        TileMap::from_text(
            "##########\n\
             #........#\n\
             #...#....#\n\
             #........#\n\
             ##########",
            8,
            LEGEND,
        )
        .unwrap()
    }

    #[test]
    fn a_map_reads_from_text_and_says_what_it_lacks() {
        let m = room();
        assert_eq!((m.width, m.height), (10, 5));
        assert_eq!(
            (m.get(0, 0), m.get(1, 1), m.get(4, 2)),
            (Some(0), None, Some(0))
        );
        assert!(m.solid_at(-1, 2) && m.solid_at(10, 2), "outside blocks");
        let err = TileMap::from_text("#.\n.x", 8, LEGEND).unwrap_err();
        assert!(
            err.contains("line 2, column 2") && err.contains("'x'"),
            "{err}"
        );
        // a tile that doesn't block
        let mut m = TileMap::from_text("=", 8, LEGEND).unwrap();
        m.solid = vec![true, false];
        assert!(!m.solid_at(0, 0));
    }

    #[test]
    fn a_box_stops_flush_against_floors_walls_and_ceilings() {
        let m = room();
        // falling from the air onto the floor (row 4 starts at y = 32)
        let f = m.move_box(10.0, 9.0, 6.0, 6.0, 0.0, 50.0);
        assert_eq!((f.y, f.hit_y, f.hit_x), (26.0, true, false));
        // walking right into the block at column 4 (x = 32), on row 2
        let w = m.move_box(10.0, 17.0, 6.0, 6.0, 40.0, 0.0);
        assert_eq!((w.x, w.hit_x), (26.0, true));
        // and left into the wall
        let w = m.move_box(20.0, 9.0, 6.0, 6.0, -40.0, 0.0);
        assert_eq!((w.x, w.hit_x), (8.0, true));
        // jumping into the ceiling
        let c = m.move_box(10.0, 20.0, 6.0, 6.0, 0.0, -40.0);
        assert_eq!((c.y, c.hit_y), (8.0, true));
        // gliding along the floor while pushed into it
        let g = m.move_box(10.0, 26.0, 6.0, 6.0, 12.0, 3.0);
        assert_eq!((g.x, g.y, g.hit_y), (22.0, 26.0, true));
    }

    #[test]
    fn a_fast_box_does_not_pass_through_a_thin_wall() {
        let m = TileMap::from_text("...#...", 8, LEGEND).unwrap();
        let r = m.move_box(0.0, 0.0, 4.0, 4.0, 200.0, 0.0);
        assert_eq!((r.x, r.hit_x), (20.0, true));
    }

    #[test]
    fn many_random_moves_never_end_inside_a_tile() {
        let m = room();
        let mut rng = crate::rng::Rng::new(7);
        let (mut x, mut y) = (10.0f32, 10.0f32);
        for _ in 0..20_000 {
            let dx = (rng.float() - 0.5) * 40.0;
            let dy = (rng.float() - 0.5) * 40.0;
            let moved = m.move_box(x, y, 5.0, 7.0, dx, dy);
            (x, y) = (moved.x, moved.y);
            assert!(!m.overlaps(x, y, 5.0, 7.0), "inside a tile at ({x}, {y})");
        }
    }

    #[test]
    fn only_what_the_view_shows_is_drawn() {
        let m = room();
        let mut solid = Image::new(8, 8);
        solid.pixels.fill(0xff11_2233);
        let mut c = Canvas::new(16, 16);
        m.draw(&mut c, &[solid], (4, 4));
        // the view's corner is inside the corner tile (0, 0); (1, 1) is empty
        assert_eq!(c.get(0, 0), 0x112233);
        assert_eq!(c.get(8, 8), 0x000000);
    }

    #[test]
    fn the_camera_follows_softly_and_stays_inside() {
        let mut cam = Camera::new(40, 30);
        for _ in 0..200 {
            cam.follow(500.0, 15.0, 0.1);
        }
        assert!(
            (cam.x - 480.0).abs() < 0.01 && cam.y.abs() < 0.01,
            "{cam:?}"
        );
        cam.keep_inside(300.0, 200.0);
        assert_eq!((cam.x, cam.y), (260.0, 0.0));
        cam.keep_inside(20.0, 200.0);
        assert_eq!(cam.x, -10.0, "a world narrower than the view is centred");
        assert_eq!(cam.view(), (-10, 0));
    }
}
