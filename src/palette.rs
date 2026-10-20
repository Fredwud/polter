// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Colours: up to 256 palette slots.
//!
//! The screen stores palette slots, never raw RGB. A colour outside the palette cannot be drawn,
//! because it cannot even be expressed. The first sixteen slots have names and make up the
//! built-in palettes; a cart can bring its own palette with up to 256 slots, for example ramps of
//! shades that make lighting possible (the way Doom and Quake lit their 256-colour worlds).

use std::fmt;
use std::sync::OnceLock;

/// A palette slot, 0 to 255. Every pixel on the screen is one of these.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Col(pub u8);

/// Names for the first sixteen slots, as in the default palette.
#[allow(non_upper_case_globals)]
impl Col {
    /// Outlines, darkest shadows.
    pub const Black: Col = Col(0);
    /// Deep shadows, the usual background.
    pub const Gunmetal: Col = Col(1);
    /// Mid shadows.
    pub const Steel: Col = Col(2);
    /// Base metal tone.
    pub const Chrome: Col = Col(3);
    /// Highlights, text.
    pub const Shine: Col = Col(4);
    /// Eyes, lights.
    pub const Visor: Col = Col(5);
    /// The one accent that screams.
    pub const Ember: Col = Col(6);
    /// Oxblood: berets, cigars, warm darks.
    pub const Beret: Col = Col(7);
    /// Warm light earth.
    pub const Sand: Col = Col(8);
    /// Warm mid earth.
    pub const Clay: Col = Col(9);
    /// Dark brown.
    pub const Bark: Col = Col(10);
    /// Green.
    pub const Moss: Col = Col(11);
    /// Dark blue.
    pub const Deep: Col = Col(12);
    /// Mid blue.
    pub const Water: Col = Col(13);
    /// Light blue.
    pub const Sky: Col = Col(14);
    /// Off-white, warm.
    pub const Paper: Col = Col(15);

    /// The sixteen named slots in index order.
    pub const ALL: [Col; 16] = [
        Col::Black,
        Col::Gunmetal,
        Col::Steel,
        Col::Chrome,
        Col::Shine,
        Col::Visor,
        Col::Ember,
        Col::Beret,
        Col::Sand,
        Col::Clay,
        Col::Bark,
        Col::Moss,
        Col::Deep,
        Col::Water,
        Col::Sky,
        Col::Paper,
    ];

    const NAMES: [&'static str; 16] = [
        "Black", "Gunmetal", "Steel", "Chrome", "Shine", "Visor", "Ember", "Beret", "Sand", "Clay",
        "Bark", "Moss", "Deep", "Water", "Sky", "Paper",
    ];

    pub fn from_index(i: u8) -> Col {
        Col(i)
    }

    /// The single-character code used in sprite rows:
    /// `K G S C W Y O R` for the first eight, `D L B M P U I E` for the second eight.
    /// Sprites that need more colours use `Sprite::from_rows_with` and their own legend.
    pub fn from_code(c: char) -> Option<Col> {
        Some(match c {
            'K' => Col::Black,
            'G' => Col::Gunmetal,
            'S' => Col::Steel,
            'C' => Col::Chrome,
            'W' => Col::Shine,
            'Y' => Col::Visor,
            'O' => Col::Ember,
            'R' => Col::Beret,
            'D' => Col::Sand,
            'L' => Col::Clay,
            'B' => Col::Bark,
            'M' => Col::Moss,
            'P' => Col::Deep,
            'U' => Col::Water,
            'I' => Col::Sky,
            'E' => Col::Paper,
            _ => return None,
        })
    }
}

impl fmt::Debug for Col {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match Col::NAMES.get(self.0 as usize) {
            Some(name) => f.write_str(name),
            None => write!(f, "Col({})", self.0),
        }
    }
}

/// How the slots look on screen. Each slot is a pair of RGB colours that alternate in a
/// checkerboard; a solid colour simply uses the same value twice. Swapping the palette restyles
/// a whole game at once. Slots beyond the palette's length draw as its first slot.
#[derive(Clone, Debug)]
pub struct Palette {
    pub name: &'static str,
    pub slots: Vec<(u32, u32)>,
}

const fn solid(rgb: u32) -> (u32, u32) {
    (rgb, rgb)
}

/// The built-in palettes (16 slots each). The first one is the default.
pub fn palettes() -> &'static [Palette] {
    static ALL: OnceLock<Vec<Palette>> = OnceLock::new();
    ALL.get_or_init(|| {
        vec![
            Palette {
                name: "Chrome & Ember",
                slots: vec![
                    solid(0x0d0e13),
                    solid(0x232733),
                    solid(0x4a5263),
                    solid(0x8c97ab),
                    solid(0xdce3ee),
                    solid(0xffd447),
                    solid(0xff6a1f),
                    solid(0xa3312a),
                    solid(0xc9a26b),
                    solid(0x7a5136),
                    solid(0x4a2f22),
                    solid(0x2f7a4f),
                    solid(0x2a4f7a),
                    solid(0x4f8fd1),
                    solid(0x8fb7d9),
                    solid(0xe9dfc4),
                ],
            },
            Palette {
                name: "PICO-8",
                slots: vec![
                    solid(0x000000),
                    solid(0x1d2b53),
                    solid(0x5f574f),
                    solid(0xc2c3c7),
                    solid(0xfff1e8),
                    solid(0xffec27),
                    solid(0xffa300),
                    solid(0xab5236),
                    solid(0xffccaa),
                    solid(0xab5236),
                    solid(0x7e2553),
                    solid(0x008751),
                    solid(0x1d2b53),
                    solid(0x29adff),
                    solid(0x29adff),
                    solid(0xfff1e8),
                ],
            },
            Palette {
                name: "Game Boy",
                slots: vec![
                    solid(0x0f380f),
                    solid(0x9bbc0f),
                    (0x306230, 0x8bac0f),
                    solid(0x8bac0f),
                    solid(0x9bbc0f),
                    solid(0x9bbc0f),
                    solid(0x306230),
                    solid(0x306230),
                    solid(0x8bac0f),
                    solid(0x306230),
                    solid(0x0f380f),
                    solid(0x306230),
                    solid(0x0f380f),
                    (0x306230, 0x8bac0f),
                    solid(0x9bbc0f),
                    solid(0x9bbc0f),
                ],
            },
            Palette {
                name: "Parchment 1-bit",
                slots: vec![
                    solid(0x1b1b1e),
                    solid(0xf2e8d5),
                    (0x1b1b1e, 0xf2e8d5),
                    solid(0xf2e8d5),
                    solid(0xf2e8d5),
                    solid(0xf2e8d5),
                    (0x1b1b1e, 0xf2e8d5),
                    solid(0x1b1b1e),
                    (0x1b1b1e, 0xf2e8d5),
                    (0x1b1b1e, 0xf2e8d5),
                    solid(0x1b1b1e),
                    (0x1b1b1e, 0xf2e8d5),
                    solid(0x1b1b1e),
                    (0x1b1b1e, 0xf2e8d5),
                    solid(0xf2e8d5),
                    solid(0xf2e8d5),
                ],
            },
        ]
    })
}

impl Palette {
    /// The on-screen RGB value of a slot at a pixel position (dithered slots alternate).
    #[inline]
    pub fn rgb(&self, c: Col, x: usize, y: usize) -> u32 {
        let (a, b) = self
            .slots
            .get(c.0 as usize)
            .copied()
            .unwrap_or(self.slots[0]);
        if (x + y) & 1 == 0 { a } else { b }
    }

    /// Finds a built-in palette by index or by (case-insensitive) name prefix.
    pub fn find(query: &str) -> Option<&'static Palette> {
        let all = palettes();
        if let Ok(i) = query.parse::<usize>() {
            return all.get(i);
        }
        let q = query.to_lowercase();
        all.iter().find(|p| p.name.to_lowercase().starts_with(&q))
    }

    /// A palette of ramps: for each (dark, light) pair, `shades` steps from dark to light, with the
    /// hue drifting cooler in the shadows and warmer in the lights. Slot = ramp × shades + shade.
    pub fn ramps(name: &'static str, ramps: &[(u32, u32)], shades: usize) -> Palette {
        let mut slots = Vec::with_capacity(ramps.len() * shades);
        for &(dark, light) in ramps {
            for s in 0..shades {
                let t = s as f32 / (shades - 1).max(1) as f32;
                slots.push(solid(mix(dark, light, t)));
            }
        }
        Palette { name, slots }
    }
}

/// Blends two colours; on the way from dark to light the blue channel leads a little early and
/// the red channel a little late, which reads as cool shadows and warm highlights.
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |c: u32, shift: u32| ((c >> shift) & 0xff) as f32;
    let lerp = |x: f32, y: f32, t: f32| x + (y - x) * t.clamp(0.0, 1.0);
    let r = lerp(ch(a, 16), ch(b, 16), t * t * 0.4 + t * 0.6);
    let g = lerp(ch(a, 8), ch(b, 8), t);
    let bl = lerp(ch(a, 0), ch(b, 0), (t * 1.15).min(1.0));
    ((r.round() as u32) << 16) | ((g.round() as u32) << 8) | bl.round() as u32
}
