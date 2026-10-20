// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The built-in 3×5 pixel font. Uppercase only; lowercase letters are drawn as uppercase.
//! Each glyph is 3 pixels wide, 5 tall, with one pixel of spacing: a character advances 4 pixels.

use std::collections::BTreeMap;
use std::sync::OnceLock;

/// Horizontal advance of one character in pixels.
pub const ADVANCE: i32 = 4;
/// Height of a line of text in pixels, including spacing.
pub const LINE: i32 = 6;

const GLYPHS: &[(char, [&str; 5])] = &[
    ('A', [".#.", "#.#", "###", "#.#", "#.#"]),
    ('B', ["##.", "#.#", "##.", "#.#", "##."]),
    ('C', [".##", "#..", "#..", "#..", ".##"]),
    ('D', ["##.", "#.#", "#.#", "#.#", "##."]),
    ('E', ["###", "#..", "##.", "#..", "###"]),
    ('F', ["###", "#..", "##.", "#..", "#.."]),
    ('G', [".##", "#..", "#.#", "#.#", ".##"]),
    ('H', ["#.#", "#.#", "###", "#.#", "#.#"]),
    ('I', ["###", ".#.", ".#.", ".#.", "###"]),
    ('J', ["..#", "..#", "..#", "#.#", ".#."]),
    ('K', ["#.#", "#.#", "##.", "#.#", "#.#"]),
    ('L', ["#..", "#..", "#..", "#..", "###"]),
    ('M', ["#.#", "###", "###", "#.#", "#.#"]),
    ('N', ["##.", "#.#", "#.#", "#.#", "#.#"]),
    ('O', [".#.", "#.#", "#.#", "#.#", ".#."]),
    ('P', ["##.", "#.#", "##.", "#..", "#.."]),
    ('Q', [".#.", "#.#", "#.#", "##.", ".##"]),
    ('R', ["##.", "#.#", "##.", "#.#", "#.#"]),
    ('S', [".##", "#..", ".#.", "..#", "##."]),
    ('T', ["###", ".#.", ".#.", ".#.", ".#."]),
    ('U', ["#.#", "#.#", "#.#", "#.#", "###"]),
    ('V', ["#.#", "#.#", "#.#", "#.#", ".#."]),
    ('W', ["#.#", "#.#", "###", "###", "#.#"]),
    ('X', ["#.#", "#.#", ".#.", "#.#", "#.#"]),
    ('Y', ["#.#", "#.#", ".#.", ".#.", ".#."]),
    ('Z', ["###", "..#", ".#.", "#..", "###"]),
    ('0', ["###", "#.#", "#.#", "#.#", "###"]),
    ('1', [".#.", "##.", ".#.", ".#.", "###"]),
    ('2', ["###", "..#", "###", "#..", "###"]),
    ('3', ["###", "..#", "###", "..#", "###"]),
    ('4', ["#.#", "#.#", "###", "..#", "..#"]),
    ('5', ["###", "#..", "###", "..#", "###"]),
    ('6', ["###", "#..", "###", "#.#", "###"]),
    ('7', ["###", "..#", "..#", ".#.", ".#."]),
    ('8', ["###", "#.#", "###", "#.#", "###"]),
    ('9', ["###", "#.#", "###", "..#", "###"]),
    (' ', ["...", "...", "...", "...", "..."]),
    ('.', ["...", "...", "...", "...", ".#."]),
    (',', ["...", "...", "...", ".#.", "#.."]),
    ('!', [".#.", ".#.", ".#.", "...", ".#."]),
    ('?', ["##.", "..#", ".#.", "...", ".#."]),
    (':', ["...", ".#.", "...", ".#.", "..."]),
    (';', ["...", ".#.", "...", ".#.", "#.."]),
    ('-', ["...", "...", "###", "...", "..."]),
    ('+', ["...", ".#.", "###", ".#.", "..."]),
    ('=', ["...", "###", "...", "###", "..."]),
    ('*', ["...", "#.#", ".#.", "#.#", "..."]),
    ('/', ["..#", "..#", ".#.", "#..", "#.."]),
    ('\'', [".#.", ".#.", "...", "...", "..."]),
    ('"', ["#.#", "#.#", "...", "...", "..."]),
    ('(', ["..#", ".#.", ".#.", ".#.", "..#"]),
    (')', ["#..", ".#.", ".#.", ".#.", "#.."]),
    ('[', [".##", ".#.", ".#.", ".#.", ".##"]),
    (']', ["##.", ".#.", ".#.", ".#.", "##."]),
    ('<', ["..#", ".#.", "#..", ".#.", "..#"]),
    ('>', ["#..", ".#.", "..#", ".#.", "#.."]),
    ('%', ["#.#", "..#", ".#.", "#..", "#.#"]),
    ('#', ["#.#", "###", "#.#", "###", "#.#"]),
    ('_', ["...", "...", "...", "...", "###"]),
    ('~', ["...", ".##", "##.", "...", "..."]),
    ('&', [".#.", "#.#", ".#.", "#.#", ".##"]),
    ('@', ["###", "#.#", "#.#", "#..", ".##"]),
    ('·', ["...", "...", ".#.", "...", "..."]),
];

/// The glyph used for characters the font doesn't know: a hollow box, so gaps are visible.
const UNKNOWN: [u8; 5] = [0b111, 0b101, 0b101, 0b101, 0b111];

fn table() -> &'static BTreeMap<char, [u8; 5]> {
    static TABLE: OnceLock<BTreeMap<char, [u8; 5]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        GLYPHS
            .iter()
            .map(|(c, rows)| {
                let mut bits = [0u8; 5];
                for (i, row) in rows.iter().enumerate() {
                    for (j, ch) in row.chars().enumerate() {
                        if ch == '#' {
                            bits[i] |= 0b100 >> j;
                        }
                    }
                }
                (*c, bits)
            })
            .collect()
    })
}

/// The five rows of a glyph; bit 2 is the left column, bit 0 the right one.
pub fn glyph(c: char) -> [u8; 5] {
    let c = c.to_ascii_uppercase();
    table().get(&c).copied().unwrap_or(UNKNOWN)
}

/// Whether the font has a real glyph for this character (after uppercasing).
pub fn has_glyph(c: char) -> bool {
    table().contains_key(&c.to_ascii_uppercase())
}

/// All characters the font knows.
pub fn charset() -> String {
    GLYPHS.iter().map(|(c, _)| *c).collect()
}

/// Draws text into a buffer of 0RGB pixels, each font pixel as a `scale` × `scale` block.
/// Used for labels on contact sheets and sound pictures; clipped at the buffer's edges.
pub fn draw_into(
    buf: &mut [u32],
    width: usize,
    text: &str,
    x: usize,
    y: usize,
    scale: usize,
    color: u32,
) {
    for (i, ch) in text.chars().enumerate() {
        for (row, bits) in glyph(ch).iter().enumerate() {
            for col in 0..3 {
                if bits & (0b100 >> col) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let px = x + i * ADVANCE as usize * scale + col * scale + dx;
                        let py = y + row * scale + dy;
                        if px < width
                            && let Some(p) = buf.get_mut(py * width + px)
                        {
                            *p = color;
                        }
                    }
                }
            }
        }
    }
}
