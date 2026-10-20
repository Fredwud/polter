// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Animated GIF files, written by Polter itself: a run of a game as a clip that a web page plays
//! as it is (a README, a store page, an issue). One palette for the whole clip: the clip's own
//! colours when there are at most 255, else 255 chosen by median cut, weighted so that a rare
//! colour of a light or a spark keeps its place next to a wide dark wall. After the first frame
//! only the rectangle that changed is stored, its unchanged pixels transparent. Frames that change
//! nothing lengthen the one before.

use std::collections::HashMap;

/// The palette index that stands for "as before".
const SEE_THROUGH: u8 = 255;

/// One frame of a clip: its pixels as `0xRRGGBB`, row by row, and how long it shows, in
/// hundredths of a second.
pub struct Frame {
    pub pixels: Vec<u32>,
    pub centis: u16,
}

/// What became of a clip.
pub struct Written {
    pub bytes: Vec<u8>,
    /// Frames stored (frames that changed nothing are folded into the one before).
    pub frames: usize,
    /// Colours in the clip, and whether the palette holds all of them.
    pub colours: usize,
    pub exact: bool,
}

/// The delays, in hundredths of a second, of `n` frames that are `ticks` ticks apart at sixty
/// ticks a second: rounded so that they add up to the real time (3, 3, 4, 3, 3, 4 … for two).
pub fn centis(n: usize, ticks: u64) -> Vec<u16> {
    let at = |i: usize| (i as u64 * ticks * 100 + 30) / 60;
    (0..n).map(|i| (at(i + 1) - at(i)) as u16).collect()
}

/// A clip of `w` × `h` pixels, each drawn as `scale` × `scale`, played once and again forever.
pub fn encode(w: usize, h: usize, scale: usize, frames: &[Frame]) -> Written {
    let scale = scale.max(1);
    let (palette, colours, exact) = palette_of(frames);
    let mut nearest: HashMap<u32, u8> = HashMap::new();
    let mut index = |c: u32| {
        *nearest
            .entry(c)
            .or_insert_with(|| closest(&palette, c) as u8)
    };
    // the frames as palette indices, then as the rectangles that changed
    struct Part {
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        indices: Vec<u8>,
        centis: u16,
    }
    let mut parts: Vec<Part> = Vec::new();
    let mut shown: Vec<u8> = Vec::new();
    for f in frames {
        assert_eq!(f.pixels.len(), w * h, "a frame of another size");
        let now: Vec<u8> = f.pixels.iter().map(|&c| index(c)).collect();
        if shown.is_empty() {
            parts.push(Part {
                x: 0,
                y: 0,
                w,
                h,
                indices: now.clone(),
                centis: f.centis,
            });
            shown = now;
            continue;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        for y in 0..h {
            for x in 0..w {
                if now[y * w + x] != shown[y * w + x] {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
                }
            }
        }
        if x0 >= x1 {
            // nothing changed: the frame before stays longer
            let last = parts.last_mut().expect("the first frame is always stored");
            last.centis = last.centis.saturating_add(f.centis);
            continue;
        }
        let mut indices = Vec::with_capacity((x1 - x0) * (y1 - y0));
        for y in y0..y1 {
            for x in x0..x1 {
                let i = y * w + x;
                indices.push(if now[i] == shown[i] {
                    SEE_THROUGH
                } else {
                    now[i]
                });
            }
        }
        parts.push(Part {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
            indices,
            centis: f.centis,
        });
        shown = now;
    }

    let mut out = Vec::new();
    out.extend_from_slice(b"GIF89a");
    let size = |v: usize| (v as u16).to_le_bytes();
    out.extend_from_slice(&size(w * scale));
    out.extend_from_slice(&size(h * scale));
    // a global colour table of 256, eight bits a channel; no background, square pixels
    out.extend_from_slice(&[0xf7, 0, 0]);
    for i in 0..256 {
        let c = palette.get(i).copied().unwrap_or(0);
        out.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8]);
    }
    // play forever
    out.extend_from_slice(b"\x21\xff\x0bNETSCAPE2.0\x03\x01\x00\x00\x00");
    for p in &parts {
        // leave each frame in place, index 255 lets the one before show through
        out.extend_from_slice(&[0x21, 0xf9, 4, 0x05]);
        out.extend_from_slice(&p.centis.max(2).to_le_bytes());
        out.extend_from_slice(&[SEE_THROUGH, 0]);
        out.push(0x2c);
        for v in [p.x, p.y, p.w, p.h] {
            out.extend_from_slice(&size(v * scale));
        }
        out.push(0);
        out.push(8);
        let mut big = Vec::with_capacity(p.indices.len() * scale * scale);
        for row in p.indices.chunks(p.w) {
            let line: Vec<u8> = row
                .iter()
                .flat_map(|&i| std::iter::repeat_n(i, scale))
                .collect();
            for _ in 0..scale {
                big.extend_from_slice(&line);
            }
        }
        for block in lzw(&big).chunks(255) {
            out.push(block.len() as u8);
            out.extend_from_slice(block);
        }
        out.push(0);
    }
    out.push(0x3b);
    Written {
        bytes: out,
        frames: parts.len(),
        colours,
        exact,
    }
}

/// The clip's palette: its colours, or 255 of them by median cut. And how many colours it has.
fn palette_of(frames: &[Frame]) -> (Vec<u32>, usize, bool) {
    let mut count: HashMap<u32, u64> = HashMap::new();
    for f in frames {
        for &c in &f.pixels {
            *count.entry(c & 0xff_ffff).or_default() += 1;
        }
    }
    let mut all: Vec<(u32, u64)> = count.into_iter().collect();
    // the same palette from the same clip, whatever order the map kept
    all.sort_unstable();
    let colours = all.len();
    if colours <= 255 {
        return (all.into_iter().map(|(c, _)| c).collect(), colours, true);
    }
    // weight by the square root of the count: a colour on a hundred pixels is not a hundred
    // times as important as one on a single pixel
    let weighted: Vec<([u8; 3], f64)> = all
        .iter()
        .map(|&(c, n)| {
            (
                [(c >> 16) as u8, (c >> 8) as u8, c as u8],
                (n as f64).sqrt(),
            )
        })
        .collect();
    let mut boxes: Vec<Vec<([u8; 3], f64)>> = vec![weighted];
    while boxes.len() < 255 {
        // split the box whose widest channel spans most, times its weight
        let spread = |b: &Vec<([u8; 3], f64)>| {
            let mut best = (0, 0u8);
            for ch in 0..3 {
                let lo = b.iter().map(|e| e.0[ch]).min().unwrap_or(0);
                let hi = b.iter().map(|e| e.0[ch]).max().unwrap_or(0);
                if hi - lo > best.1 {
                    best = (ch, hi - lo);
                }
            }
            best
        };
        let pick = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .max_by(|(_, a), (_, b)| {
                let score = |b: &Vec<([u8; 3], f64)>| {
                    spread(b).1 as f64 * b.iter().map(|e| e.1).sum::<f64>().sqrt()
                };
                score(a).total_cmp(&score(b))
            })
            .map(|(i, _)| i);
        let Some(i) = pick else { break };
        let mut b = boxes.swap_remove(i);
        let (ch, _) = spread(&b);
        b.sort_by_key(|e| (e.0[ch], e.0));
        let half = b.iter().map(|e| e.1).sum::<f64>() / 2.0;
        let (mut acc, mut cut) = (0.0, 1);
        for (k, e) in b.iter().enumerate() {
            acc += e.1;
            if acc >= half {
                cut = (k + 1).clamp(1, b.len() - 1);
                break;
            }
        }
        let rest = b.split_off(cut);
        boxes.push(b);
        boxes.push(rest);
    }
    let mut palette: Vec<u32> = boxes
        .iter()
        .map(|b| {
            let total: f64 = b.iter().map(|e| e.1).sum();
            let mean = |ch: usize| {
                (b.iter().map(|e| e.0[ch] as f64 * e.1).sum::<f64>() / total).round() as u32
            };
            (mean(0) << 16) | (mean(1) << 8) | mean(2)
        })
        .collect();
    palette.sort_unstable();
    palette.dedup();
    (palette, colours, false)
}

/// The palette entry nearest to `c`, by distance weighted the way eyes weigh red, green, blue.
fn closest(palette: &[u32], c: u32) -> usize {
    let ch = |v: u32, s: u32| ((v >> s) & 0xff) as i32;
    let mut best = (0, i32::MAX);
    for (i, &p) in palette.iter().enumerate() {
        let (dr, dg, db) = (
            ch(p, 16) - ch(c, 16),
            ch(p, 8) - ch(c, 8),
            ch(p, 0) - ch(c, 0),
        );
        let d = 3 * dr * dr + 4 * dg * dg + 2 * db * db;
        if d < best.1 {
            best = (i, d);
        }
    }
    best.0
}

/// GIF's LZW with eight-bit symbols: codes from 9 to 12 bits, least significant bit first, a
/// clear code when the table is full.
fn lzw(data: &[u8]) -> Vec<u8> {
    const CLEAR: u16 = 256;
    const END: u16 = 257;
    let mut out = Vec::new();
    let (mut bits, mut nbits) = (0u32, 0u32);
    let mut emit = |code: u16, size: u32, out: &mut Vec<u8>| {
        bits |= (code as u32) << nbits;
        nbits += size;
        while nbits >= 8 {
            out.push(bits as u8);
            bits >>= 8;
            nbits -= 8;
        }
    };
    // table[prefix * 256 + symbol] = the code for prefix followed by symbol, 0 if none yet
    let mut table = vec![0u16; 4096 * 256];
    let (mut size, mut next) = (9u32, 258u16);
    emit(CLEAR, size, &mut out);
    let mut it = data.iter();
    if let Some(&first) = it.next() {
        let mut prefix = first as u16;
        for &k in it {
            let slot = prefix as usize * 256 + k as usize;
            if table[slot] != 0 {
                prefix = table[slot];
                continue;
            }
            emit(prefix, size, &mut out);
            if next < 4096 {
                table[slot] = next;
                next += 1;
                if next as u32 > 1 << size && size < 12 {
                    size += 1;
                }
            } else {
                emit(CLEAR, size, &mut out);
                table.iter_mut().for_each(|t| *t = 0);
                (size, next) = (9, 258);
            }
            prefix = k as u16;
        }
        emit(prefix, size, &mut out);
    }
    emit(END, size, &mut out);
    if nbits > 0 {
        out.push(bits as u8);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plain GIF decoder, written from the format's description, for these tests only.
    fn unlzw(data: &[u8]) -> Vec<u8> {
        let mut table: Vec<Vec<u8>> = Vec::new();
        let reset = |t: &mut Vec<Vec<u8>>| {
            t.clear();
            t.extend((0..=255u8).map(|i| vec![i]));
            t.push(Vec::new());
            t.push(Vec::new());
        };
        reset(&mut table);
        let (mut size, mut pos) = (9u32, 0usize);
        let mut prev: Option<Vec<u8>> = None;
        let mut out = Vec::new();
        loop {
            let mut code = 0usize;
            for b in 0..size as usize {
                let bit = (data[(pos + b) / 8] >> ((pos + b) % 8)) & 1;
                code |= (bit as usize) << b;
            }
            pos += size as usize;
            match code {
                256 => {
                    reset(&mut table);
                    size = 9;
                    prev = None;
                    continue;
                }
                257 => return out,
                _ => {}
            }
            let entry = if code < table.len() {
                table[code].clone()
            } else {
                let p = prev.clone().expect("a new code right after a clear");
                let mut e = p.clone();
                e.push(p[0]);
                e
            };
            if let Some(p) = prev
                && table.len() < 4096
            {
                let mut e = p;
                e.push(entry[0]);
                table.push(e);
                if table.len() == 1 << size && size < 12 {
                    size += 1;
                }
            }
            out.extend_from_slice(&entry);
            prev = Some(entry);
        }
    }

    /// The frames a GIF shows, as palette colours, each composed over the one before.
    fn decode(gif: &[u8]) -> (usize, usize, Vec<Vec<u32>>, Vec<u16>) {
        assert_eq!(&gif[..6], b"GIF89a");
        let rd = |i: usize| u16::from_le_bytes([gif[i], gif[i + 1]]) as usize;
        let (w, h) = (rd(6), rd(8));
        let palette: Vec<u32> = (0..256)
            .map(|i| {
                let o = 13 + i * 3;
                ((gif[o] as u32) << 16) | ((gif[o + 1] as u32) << 8) | gif[o + 2] as u32
            })
            .collect();
        let (mut i, mut screen) = (13 + 768, vec![0u32; w * h]);
        let (mut frames, mut delays, mut see_through) = (Vec::new(), Vec::new(), None);
        loop {
            match gif[i] {
                0x21 => {
                    if gif[i + 1] == 0xf9 {
                        delays.push(rd(i + 4) as u16);
                        see_through = (gif[i + 3] & 1 == 1).then_some(gif[i + 6]);
                    }
                    i += 2;
                    while gif[i] != 0 {
                        i += gif[i] as usize + 1;
                    }
                    i += 1;
                }
                0x2c => {
                    let (x, y, fw, fh) = (rd(i + 1), rd(i + 3), rd(i + 5), rd(i + 7));
                    assert_eq!(gif[i + 9], 0, "no local palette, no interlacing");
                    assert_eq!(gif[i + 10], 8);
                    i += 11;
                    let mut data = Vec::new();
                    while gif[i] != 0 {
                        data.extend_from_slice(&gif[i + 1..i + 1 + gif[i] as usize]);
                        i += gif[i] as usize + 1;
                    }
                    i += 1;
                    let px = unlzw(&data);
                    assert_eq!(px.len(), fw * fh);
                    for (k, &p) in px.iter().enumerate() {
                        if Some(p) != see_through {
                            screen[(y + k / fw) * w + x + k % fw] = palette[p as usize];
                        }
                    }
                    frames.push(screen.clone());
                }
                0x3b => return (w, h, frames, delays),
                other => panic!("unexpected block {other:#x} at {i}"),
            }
        }
    }

    fn noise(seed: u64, n: usize, kinds: u64) -> Vec<u8> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                s = s
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((s >> 33) % kinds) as u8
            })
            .collect()
    }

    #[test]
    fn lzw_gives_back_what_it_was_given() {
        // runs, few symbols, many symbols (the table fills and clears several times), nothing
        let mut cases = vec![
            vec![],
            vec![7],
            vec![0; 100_000],
            noise(1, 50_000, 3),
            noise(2, 200_000, 256),
        ];
        cases.push((0..=255u8).cycle().take(70_000).collect());
        for data in cases {
            assert_eq!(unlzw(&lzw(&data)), data, "{} bytes", data.len());
        }
    }

    #[test]
    fn a_clip_shows_its_frames_in_its_colours() {
        let (w, h) = (16, 9);
        let frame = |t: usize| {
            let pixels = (0..w * h)
                .map(|i| {
                    let (x, y) = (i % w, i / w);
                    if (x + t).is_multiple_of(7) || y == t % h {
                        0xffd070
                    } else {
                        0x101830 + (x as u32) * 0x10
                    }
                })
                .collect();
            Frame { pixels, centis: 3 }
        };
        let mut frames: Vec<Frame> = (0..12).map(frame).collect();
        // a frame that changes nothing: folded into the one before
        frames.insert(5, frame(4));
        let gif = encode(w, h, 3, &frames);
        assert!(gif.exact);
        assert_eq!(gif.frames, 12);
        let (gw, gh, shown, delays) = decode(&gif.bytes);
        assert_eq!((gw, gh), (w * 3, h * 3));
        let wanted: Vec<&Frame> = frames
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 5)
            .map(|(_, f)| f)
            .collect();
        for (k, f) in wanted.iter().enumerate() {
            for y in 0..gh {
                for x in 0..gw {
                    assert_eq!(
                        shown[k][y * gw + x],
                        f.pixels[(y / 3) * w + x / 3],
                        "frame {k} at {x},{y}"
                    );
                }
            }
        }
        assert_eq!(
            delays[4], 6,
            "the frame that changed nothing shows as part of the one before"
        );
        assert!(delays.iter().enumerate().all(|(i, &d)| i == 4 || d == 3));
    }

    #[test]
    fn many_colours_come_close() {
        // a light's falloff: 4096 colours, more than a palette holds
        let (w, h) = (64, 64);
        let pixels: Vec<u32> = (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as u32, (i / w) as u32);
                ((x * 4) << 16) | ((y * 4) << 8) | ((x + y) * 2)
            })
            .collect();
        let gif = encode(
            w,
            h,
            1,
            &[Frame {
                pixels: pixels.clone(),
                centis: 10,
            }],
        );
        assert!(!gif.exact);
        assert_eq!(gif.colours, 4096);
        let (_, _, shown, _) = decode(&gif.bytes);
        let worst = pixels
            .iter()
            .zip(&shown[0])
            .map(|(&a, &b)| {
                (0..3)
                    .map(|s| ((a >> (s * 8)) & 0xff).abs_diff((b >> (s * 8)) & 0xff))
                    .max()
                    .unwrap()
            })
            .max()
            .unwrap();
        assert!(worst <= 16, "a channel off by {worst}");
        // the same clip gives the same file
        assert_eq!(
            encode(w, h, 1, &[Frame { pixels, centis: 10 }]).bytes,
            gif.bytes
        );
    }

    #[test]
    fn delays_add_up_to_the_real_time() {
        assert_eq!(centis(6, 2), vec![3, 4, 3, 3, 4, 3]);
        assert_eq!(centis(60, 2).iter().map(|&c| c as u32).sum::<u32>(), 200);
        assert_eq!(centis(4, 3), vec![5, 5, 5, 5]);
    }
}
