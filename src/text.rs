// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Text for true-colour games: a proportional pixel font with lower case, umlauts and ß, the
//! width of a text, wrapping to a width, and fonts of one's own from BDF files (the format free
//! pixel fonts come in). The built-in font is Polter's own, drawn for it: 7 pixels from the top of
//! a capital to the baseline, 2 below it for g, j, p, q and y, a pixel between letters.
//!
//! ```
//! use polter::text::Font;
//! let font = Font::standard();
//! assert_eq!(font.width("Hi"), 5 + 1 + 1);
//! let lines = font.wrap("Über den Fluss gehen", 50);
//! assert!(lines.len() > 1 && lines.iter().all(|l| font.width(l) <= 50));
//! ```

use crate::canvas::Canvas;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// One character: its width in pixels and its rows (a bit per column, the leftmost highest).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Glyph {
    width: usize,
    rows: Vec<u64>,
}

/// A bitmap font.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Font {
    glyphs: BTreeMap<char, Glyph>,
    /// Rows of a glyph, and the row below the last one that stands on the baseline.
    pub height: usize,
    pub baseline: usize,
    /// From one line's top to the next's.
    pub line_height: usize,
    /// Empty columns between two characters.
    pub spacing: usize,
}

/// Polter's font: per character its rows, top to bottom, `#` set, `.` not. Seven rows reach the
/// baseline; glyphs with a descender have nine.
#[rustfmt::skip]
const STANDARD: &[(char, &[&str])] = &[
    (' ', &["...", "...", "...", "...", "...", "...", "..."]),
    ('0', &[".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."]),
    ('1', &["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."]),
    ('2', &[".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"]),
    ('3', &["#####", "...#.", "..#..", "...#.", "....#", "#...#", ".###."]),
    ('4', &["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."]),
    ('5', &["#####", "#....", "####.", "....#", "....#", "#...#", ".###."]),
    ('6', &["..##.", ".#...", "#....", "####.", "#...#", "#...#", ".###."]),
    ('7', &["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."]),
    ('8', &[".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."]),
    ('9', &[".###.", "#...#", "#...#", ".####", "....#", "...#.", ".##.."]),
    ('A', &[".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('B', &["####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."]),
    ('C', &[".###.", "#...#", "#....", "#....", "#....", "#...#", ".###."]),
    ('D', &["###..", "#..#.", "#...#", "#...#", "#...#", "#..#.", "###.."]),
    ('E', &["#####", "#....", "#....", "####.", "#....", "#....", "#####"]),
    ('F', &["#####", "#....", "#....", "####.", "#....", "#....", "#...."]),
    ('G', &[".###.", "#...#", "#....", "#.###", "#...#", "#...#", ".####"]),
    ('H', &["#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('I', &["###", ".#.", ".#.", ".#.", ".#.", ".#.", "###"]),
    ('J', &["..###", "...#.", "...#.", "...#.", "...#.", "#..#.", ".##.."]),
    ('K', &["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"]),
    ('L', &["#....", "#....", "#....", "#....", "#....", "#....", "#####"]),
    ('M', &["#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#"]),
    ('N', &["#...#", "#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#"]),
    ('O', &[".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('P', &["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."]),
    ('Q', &[".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"]),
    ('R', &["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"]),
    ('S', &[".####", "#....", "#....", ".###.", "....#", "....#", "####."]),
    ('T', &["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."]),
    ('U', &["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('V', &["#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#.."]),
    ('W', &["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "#.#.#", ".#.#."]),
    ('X', &["#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"]),
    ('Y', &["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."]),
    ('Z', &["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"]),
    ('Ä', &["#...#", ".....", ".###.", "#...#", "#####", "#...#", "#...#"]),
    ('Ö', &["#...#", ".....", ".###.", "#...#", "#...#", "#...#", ".###."]),
    ('Ü', &["#...#", ".....", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('a', &["....", "....", ".##.", "...#", ".###", "#..#", ".###"]),
    ('b', &["#...", "#...", "###.", "#..#", "#..#", "#..#", "###."]),
    ('c', &["....", "....", ".##.", "#...", "#...", "#...", ".##."]),
    ('d', &["...#", "...#", ".###", "#..#", "#..#", "#..#", ".###"]),
    ('e', &["....", "....", ".##.", "#..#", "####", "#...", ".##."]),
    ('f', &[".##", "#..", "###", "#..", "#..", "#..", "#.."]),
    ('g', &["....", "....", ".###", "#..#", "#..#", "#..#", ".###", "...#", ".##."]),
    ('h', &["#...", "#...", "###.", "#..#", "#..#", "#..#", "#..#"]),
    ('i', &["#", ".", "#", "#", "#", "#", "#"]),
    ('j', &[".#", "..", ".#", ".#", ".#", ".#", ".#", ".#", "#."]),
    ('k', &["#...", "#...", "#..#", "#.#.", "##..", "#.#.", "#..#"]),
    ('l', &["#.", "#.", "#.", "#.", "#.", "#.", ".#"]),
    ('m', &[".....", ".....", "##.#.", "#.#.#", "#.#.#", "#.#.#", "#.#.#"]),
    ('n', &["....", "....", "###.", "#..#", "#..#", "#..#", "#..#"]),
    ('o', &["....", "....", ".##.", "#..#", "#..#", "#..#", ".##."]),
    ('p', &["....", "....", "###.", "#..#", "#..#", "#..#", "###.", "#...", "#..."]),
    ('q', &["....", "....", ".###", "#..#", "#..#", "#..#", ".###", "...#", "...#"]),
    ('r', &["....", "....", "#.##", "##..", "#...", "#...", "#..."]),
    ('s', &["....", "....", ".###", "#...", ".##.", "...#", "###."]),
    ('t', &[".#.", ".#.", "###", ".#.", ".#.", ".#.", "..#"]),
    ('u', &["....", "....", "#..#", "#..#", "#..#", "#..#", ".###"]),
    ('v', &[".....", ".....", "#...#", "#...#", ".#.#.", ".#.#.", "..#.."]),
    ('w', &[".....", ".....", "#...#", "#...#", "#.#.#", "#.#.#", ".#.#."]),
    ('x', &["....", "....", "#..#", "#..#", ".##.", "#..#", "#..#"]),
    ('y', &["....", "....", "#..#", "#..#", "#..#", "#..#", ".###", "...#", ".##."]),
    ('z', &["....", "....", "####", "...#", ".##.", "#...", "####"]),
    ('ä', &["#..#", "....", ".##.", "...#", ".###", "#..#", ".###"]),
    ('ö', &["#..#", "....", ".##.", "#..#", "#..#", "#..#", ".##."]),
    ('ü', &["#..#", "....", "#..#", "#..#", "#..#", "#..#", ".###"]),
    ('ß', &[".##.", "#..#", "#..#", "###.", "#..#", "#..#", "#.#."]),
    ('!', &["#", "#", "#", "#", "#", ".", "#"]),
    ('"', &["#.#", "#.#", "...", "...", "...", "...", "..."]),
    ('#', &[".#.#.", ".#.#.", "#####", ".#.#.", "#####", ".#.#.", ".#.#."]),
    ('$', &["..#..", ".####", "#.#..", ".###.", "..#.#", "####.", "..#.."]),
    ('%', &["##...", "##..#", "...#.", "..#..", ".#...", "#..##", "...##"]),
    ('&', &[".##..", "#..#.", "#.#..", ".#...", "#.#.#", "#..#.", ".##.#"]),
    ('\'', &["#", "#", ".", ".", ".", ".", "."]),
    ('(', &[".#", "#.", "#.", "#.", "#.", "#.", ".#"]),
    (')', &["#.", ".#", ".#", ".#", ".#", ".#", "#."]),
    ('*', &[".....", "..#..", "#.#.#", ".###.", "#.#.#", "..#..", "....."]),
    ('+', &[".....", "..#..", "..#..", "#####", "..#..", "..#..", "....."]),
    (',', &["..", "..", "..", "..", "..", ".#", ".#", "#.", ".."]),
    ('-', &["....", "....", "....", "####", "....", "....", "...."]),
    ('.', &[".", ".", ".", ".", ".", ".", "#"]),
    ('/', &["....#", "....#", "...#.", "..#..", ".#...", "#....", "#...."]),
    (':', &[".", ".", "#", ".", ".", ".", "#"]),
    (';', &["..", "..", ".#", "..", "..", "..", ".#", "#.", ".."]),
    ('<', &["...#", "..#.", ".#..", "#...", ".#..", "..#.", "...#"]),
    ('=', &["....", "....", "####", "....", "####", "....", "...."]),
    ('>', &["#...", ".#..", "..#.", "...#", "..#.", ".#..", "#..."]),
    ('?', &[".###.", "#...#", "....#", "...#.", "..#..", ".....", "..#.."]),
    ('@', &[".###.", "#...#", "#.###", "#.#.#", "#.###", "#....", ".###."]),
    ('[', &["##", "#.", "#.", "#.", "#.", "#.", "##"]),
    ('\\', &["#....", "#....", ".#...", "..#..", "...#.", "....#", "....#"]),
    (']', &["##", ".#", ".#", ".#", ".#", ".#", "##"]),
    ('^', &["..#..", ".#.#.", "#...#", ".....", ".....", ".....", "....."]),
    ('_', &[".....", ".....", ".....", ".....", ".....", ".....", ".....", "#####", "....."]),
    ('`', &["#.", ".#", "..", "..", "..", "..", ".."]),
    ('{', &["..#", ".#.", ".#.", "#..", ".#.", ".#.", "..#"]),
    ('|', &["#", "#", "#", "#", "#", "#", "#", "#", "#"]),
    ('}', &["#..", ".#.", ".#.", "..#", ".#.", ".#.", "#.."]),
    ('~', &[".....", ".....", ".#...", "#.#.#", "...#.", ".....", "....."]),
    ('°', &[".#.", "#.#", ".#.", "...", "...", "...", "..."]),
    ('€', &["..###", ".#...", "####.", ".#...", "####.", ".#...", "..###"]),
    ('·', &[".", ".", ".", "#", ".", ".", "."]),
    ('–', &[".....", ".....", ".....", "#####", ".....", ".....", "....."]),
    ('×', &[".....", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "....."]),
    ('…', &[".....", ".....", ".....", ".....", ".....", ".....", "#.#.#"]),
];

/// What stands for a character the font lacks: a hollow box, so the gap shows.
fn missing_glyph(height: usize, baseline: usize) -> Glyph {
    let mut rows = vec![0u64; height];
    for (r, row) in rows.iter_mut().enumerate().take(baseline) {
        *row = if r == 0 || r + 1 == baseline {
            0b111
        } else {
            0b101
        };
    }
    Glyph { width: 3, rows }
}

impl Font {
    /// Polter's own font (built once).
    pub fn standard() -> &'static Font {
        static FONT: OnceLock<Font> = OnceLock::new();
        FONT.get_or_init(|| {
            let mut glyphs = BTreeMap::new();
            for (ch, rows) in STANDARD {
                let width = rows[0].len();
                let mut bits = vec![0u64; 9];
                for (r, row) in rows.iter().enumerate() {
                    bits[r] = row.bytes().fold(0, |a, b| (a << 1) | (b == b'#') as u64);
                }
                glyphs.insert(*ch, Glyph { width, rows: bits });
            }
            Font {
                glyphs,
                height: 9,
                baseline: 7,
                line_height: 11,
                spacing: 1,
            }
        })
    }

    /// A font from a BDF file's text (the format of most free pixel fonts). Its glyphs stand on
    /// its baseline (FONT_ASCENT, or the bounding box's); the advance of each is its DWIDTH, so
    /// `spacing` is 0. Errors name what is wrong.
    pub fn from_bdf(text: &str) -> Result<Font, String> {
        let mut ascent = None;
        let mut descent = None;
        let mut bbox = None;
        let mut glyphs = BTreeMap::new();
        let mut lines = text.lines().enumerate();
        let num = |s: Option<&str>, n: usize| -> Result<i64, String> {
            s.and_then(|v| v.parse().ok())
                .ok_or(format!("BDF, line {}: a number is missing", n + 1))
        };
        while let Some((n, line)) = lines.next() {
            let mut w = line.split_whitespace();
            match w.next() {
                Some("FONT_ASCENT") => ascent = Some(num(w.next(), n)?),
                Some("FONT_DESCENT") => descent = Some(num(w.next(), n)?),
                Some("FONTBOUNDINGBOX") => {
                    let v: Vec<i64> = w.filter_map(|x| x.parse().ok()).collect();
                    if v.len() != 4 {
                        return Err(format!(
                            "BDF, line {}: FONTBOUNDINGBOX wants four numbers",
                            n + 1
                        ));
                    }
                    bbox = Some((v[0], v[1], v[2], v[3]));
                }
                Some("STARTCHAR") => {
                    let (mut code, mut advance, mut gbox, mut bitmap) =
                        (None, None, None, Vec::new());
                    let mut in_bitmap = false;
                    for (m, l) in lines.by_ref() {
                        let mut w = l.split_whitespace();
                        match w.next() {
                            Some("ENCODING") => code = Some(num(w.next(), m)?),
                            Some("DWIDTH") => advance = Some(num(w.next(), m)?),
                            Some("BBX") => {
                                let v: Vec<i64> = w.filter_map(|x| x.parse().ok()).collect();
                                if v.len() != 4 {
                                    return Err(format!(
                                        "BDF, line {}: BBX wants four numbers",
                                        m + 1
                                    ));
                                }
                                gbox = Some((v[0], v[1], v[2], v[3]));
                            }
                            Some("BITMAP") => in_bitmap = true,
                            Some("ENDCHAR") => break,
                            Some(hex) if in_bitmap => {
                                bitmap.push(u64::from_str_radix(hex, 16).map_err(|_| {
                                    format!("BDF, line {}: {hex:?} is no hex row", m + 1)
                                })?)
                            }
                            _ => {}
                        }
                    }
                    if let (Some(code), Some(gbox)) = (code, gbox) {
                        glyphs.insert(code, (advance.unwrap_or(gbox.0), gbox, bitmap));
                    }
                }
                _ => {}
            }
        }
        let (bw, bh, _, by) = bbox.ok_or("BDF: no FONTBOUNDINGBOX")?;
        let ascent = ascent.unwrap_or(bh + by).max(1) as usize;
        let descent = descent.unwrap_or(-by).max(0) as usize;
        let height = ascent + descent;
        let mut out = BTreeMap::new();
        for (code, (advance, (gw, gh, gx, gy), bitmap)) in glyphs {
            let Some(ch) = u32::try_from(code).ok().and_then(char::from_u32) else {
                continue;
            };
            if gw > 63 || advance > 63 || bw > 63 {
                return Err(format!("BDF: glyph {code} is wider than 63 pixels"));
            }
            let width = advance.max(0) as usize;
            let mut rows = vec![0u64; height];
            // each BITMAP row is padded to whole bytes, the glyph's pixels leftmost
            let row_bits = (gw as usize).div_ceil(8) * 8;
            let top = ascent as i64 - (gh + gy);
            for (k, &bits) in bitmap.iter().enumerate() {
                let r = top + k as i64;
                if r < 0 || r as usize >= height {
                    continue;
                }
                for col in 0..gw.max(0) as usize {
                    if bits >> (row_bits - 1 - col) & 1 == 1 {
                        let x = gx + col as i64;
                        if (0..width as i64).contains(&x) {
                            rows[r as usize] |= 1 << (width - 1 - x as usize);
                        }
                    }
                }
            }
            out.insert(ch, Glyph { width, rows });
        }
        if out.is_empty() {
            return Err("BDF: no glyphs".into());
        }
        Ok(Font {
            glyphs: out,
            height,
            baseline: ascent,
            line_height: height + 2,
            spacing: 0,
        })
    }

    fn glyph(&self, ch: char) -> std::borrow::Cow<'_, Glyph> {
        match self.glyphs.get(&ch) {
            Some(g) => std::borrow::Cow::Borrowed(g),
            None => std::borrow::Cow::Owned(missing_glyph(self.height, self.baseline)),
        }
    }

    /// Does the font have this character?
    pub fn has(&self, ch: char) -> bool {
        self.glyphs.contains_key(&ch)
    }

    /// The characters of a text the font lacks (each once, in order): they show as boxes.
    pub fn missing(&self, text: &str) -> Vec<char> {
        let mut out = Vec::new();
        for ch in text.chars() {
            if ch != '\n' && !self.has(ch) && !out.contains(&ch) {
                out.push(ch);
            }
        }
        out
    }

    /// The width of a line of text in pixels (of the widest line, if it has several).
    pub fn width(&self, text: &str) -> usize {
        text.split('\n')
            .map(|line| {
                let w: usize = line
                    .chars()
                    .map(|c| self.glyph(c).width + self.spacing)
                    .sum();
                w.saturating_sub(self.spacing)
            })
            .max()
            .unwrap_or(0)
    }

    /// Draws text with its top left at (x, y), line by line (`\n` starts a new one), clipped to
    /// the canvas. Returns the size it took.
    pub fn draw(
        &self,
        canvas: &mut Canvas,
        text: &str,
        x: i32,
        y: i32,
        rgb: u32,
    ) -> (usize, usize) {
        let mut lines = 0;
        for (n, line) in text.split('\n').enumerate() {
            lines = n + 1;
            let top = y + (n * self.line_height) as i32;
            let mut left = x;
            for ch in line.chars() {
                let g = self.glyph(ch);
                for (r, &bits) in g.rows.iter().enumerate() {
                    if bits == 0 {
                        continue;
                    }
                    for col in 0..g.width {
                        if bits >> (g.width - 1 - col) & 1 == 1 {
                            canvas.set(left + col as i32, top + r as i32, rgb);
                        }
                    }
                }
                left += (g.width + self.spacing) as i32;
            }
        }
        let height = if lines == 0 {
            0
        } else {
            (lines - 1) * self.line_height + self.height
        };
        (self.width(text), height)
    }

    /// Breaks text into lines no wider than `max` pixels, at spaces; a word wider than that is
    /// cut. `\n` always breaks.
    pub fn wrap(&self, text: &str, max: usize) -> Vec<String> {
        let mut out = Vec::new();
        for para in text.split('\n') {
            let mut line = String::new();
            for word in para.split(' ') {
                let candidate = if line.is_empty() {
                    word.to_string()
                } else {
                    format!("{line} {word}")
                };
                if self.width(&candidate) <= max {
                    line = candidate;
                    continue;
                }
                if !line.is_empty() {
                    out.push(std::mem::take(&mut line));
                }
                // a word too long for a line on its own is cut where it must be
                let mut rest = word.to_string();
                while self.width(&rest) > max && rest.chars().count() > 1 {
                    let mut cut = rest.chars().count() - 1;
                    while cut > 1 && self.width(&rest.chars().take(cut).collect::<String>()) > max {
                        cut -= 1;
                    }
                    out.push(rest.chars().take(cut).collect());
                    rest = rest.chars().skip(cut).collect();
                }
                line = rest;
            }
            out.push(line);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_is_a_rectangle_standing_on_the_baseline() {
        let mut seen = Vec::new();
        for (ch, rows) in STANDARD {
            assert!(!seen.contains(ch), "{ch:?} twice");
            seen.push(*ch);
            assert!(
                rows.len() == 7 || rows.len() == 9,
                "{ch:?} has {} rows",
                rows.len()
            );
            let w = rows[0].len();
            assert!((1..=5).contains(&w), "{ch:?} is {w} wide");
            for r in rows.iter() {
                assert_eq!(r.len(), w, "{ch:?}: a row of another width");
                assert!(r.bytes().all(|b| b == b'#' || b == b'.'), "{ch:?}: {r}");
            }
        }
        // every printable ASCII character, and German's
        let font = Font::standard();
        for c in (32u8..127).map(char::from).chain("ÄÖÜäöüß€°·–×…".chars()) {
            assert!(font.has(c), "{c:?} is missing");
        }
    }

    #[test]
    fn widths_wrapping_and_missing_characters() {
        let f = Font::standard();
        assert_eq!(f.width("i"), 1);
        assert_eq!(f.width("ab"), 4 + 1 + 4);
        assert_eq!(f.width("ab\nmmm"), 5 * 3 + 2, "the widest line");
        assert_eq!(f.width(""), 0);
        let text = "Der Prospektor gräbt sich durch Sandstein, Kohle und Bleiglanz.";
        for max in [40, 60, 100, 200] {
            let lines = f.wrap(text, max);
            assert!(lines.iter().all(|l| f.width(l) <= max), "{max}: {lines:?}");
            // no letter lost; where every word fits a line, the text comes back as it was
            let letters = |t: &str| t.replace(' ', "");
            assert_eq!(letters(&lines.concat()), letters(text), "at {max}");
            if text.split(' ').all(|w| f.width(w) <= max) {
                assert_eq!(lines.join(" "), text, "at {max}");
            }
        }
        let cut = f.wrap("Donaudampfschifffahrt", 30);
        assert!(
            cut.len() > 1 && cut.iter().all(|l| f.width(l) <= 30),
            "{cut:?}"
        );
        assert_eq!(cut.concat(), "Donaudampfschifffahrt");
        assert_eq!(f.missing("Ψ ok Ψ ☃"), ['Ψ', '☃']);
        // a missing character draws its box
        let mut c = Canvas::new(8, 9);
        f.draw(&mut c, "Ψ", 0, 0, 0xffffff);
        assert_eq!(
            (c.get(0, 0), c.get(1, 1), c.get(0, 6)),
            (0xffffff, 0, 0xffffff)
        );
    }

    #[test]
    fn text_draws_on_its_baseline_with_descenders_below() {
        let f = Font::standard();
        let mut c = Canvas::new(40, 24);
        let (w, h) = f.draw(&mut c, "Hg\nI", 1, 1, 0xff0000);
        assert_eq!((w, h), (5 + 1 + 4, 11 + 9));
        // H's left stem from row 0 to row 6, nothing below; g's tail in rows 7 and 8
        assert_eq!(c.get(1, 1), 0xff0000);
        assert_eq!(c.get(1, 7), 0xff0000);
        assert_eq!(c.get(1, 8), 0);
        assert_eq!(c.get(7 + 3, 1 + 7), 0xff0000, "g's descender");
        // the second line starts a line height down
        assert_eq!(c.get(1, 12), 0xff0000, "I's top bar");
    }

    #[test]
    fn a_bdf_font_reads_and_draws() {
        let bdf = "STARTFONT 2.1\nFONT test\nSIZE 6 75 75\nFONTBOUNDINGBOX 4 6 0 -1\n\
                   STARTPROPERTIES 2\nFONT_ASCENT 5\nFONT_DESCENT 1\nENDPROPERTIES\nCHARS 1\n\
                   STARTCHAR T\nENCODING 84\nSWIDTH 500 0\nDWIDTH 4 0\nBBX 3 5 0 0\nBITMAP\n\
                   E0\n40\n40\n40\n40\nENDCHAR\nENDFONT\n";
        let f = Font::from_bdf(bdf).unwrap();
        assert_eq!((f.height, f.baseline, f.spacing), (6, 5, 0));
        assert_eq!(f.width("TT"), 8);
        let mut c = Canvas::new(8, 6);
        f.draw(&mut c, "T", 0, 0, 0xffffff);
        let row = |y: i32| (0..4).map(|x| (c.get(x, y) != 0) as u8).collect::<Vec<_>>();
        assert_eq!(row(0), [1, 1, 1, 0]);
        assert_eq!(row(4), [0, 1, 0, 0]);
        assert_eq!(row(5), [0, 0, 0, 0], "below the baseline: empty");
        assert!(
            Font::from_bdf("STARTFONT 2.1\nENDFONT\n")
                .unwrap_err()
                .contains("FONTBOUNDINGBOX")
        );
    }
}

#[cfg(test)]
mod sheet {
    /// The font on a sheet, three times its size, in `POLTER_LOOK_DIR` when that is set (to look
    /// at it; not a check).
    #[test]
    fn the_font_on_a_sheet() {
        let Some(dir) = std::env::var_os("POLTER_LOOK_DIR") else {
            return;
        };
        let f = super::Font::standard();
        let text = "ABCDEFGHIJKLMNOPQRSTUVWXYZ ÄÖÜ\nabcdefghijklmnopqrstuvwxyz äöüß\n0123456789 !\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~ °€·–×…\nDer Prospektor gräbt sich durch Sandstein, Kohle und Bleiglanz.\nQuick brown fox jumps over the lazy dog: 320 × 180, 60 FPS.";
        let (w, h) = (
            f.width(text) + 8,
            f.draw(&mut crate::Canvas::new(1, 1), text, 0, 0, 0).1 + 8,
        );
        let mut c = crate::Canvas::new(w, h);
        c.clear(0x1a1c22);
        f.draw(&mut c, text, 4, 4, 0xf4e4b8);
        std::fs::write(std::path::Path::new(&dir).join("font.png"), c.to_png(3)).unwrap();
    }
}
