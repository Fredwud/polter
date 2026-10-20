// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! PNG files without dependencies. Writing: images with at most 256 colours are stored as indexed
//! PNGs, others as RGB; rows unfiltered, packed by Polter's own deflate.
//! Reading: every colour type and bit depth PNG has, interlaced or not, into an `Image` (RGBA, 8
//! bits a channel; 16-bit samples keep their high byte).

use std::sync::OnceLock;

/// Encodes RGB bytes (3 per pixel, row by row) as a PNG file.
pub fn encode(width: usize, height: usize, rgb: &[u8]) -> Vec<u8> {
    assert_eq!(
        rgb.len(),
        width * height * 3,
        "rgb buffer does not match {width}x{height}"
    );
    let mut colours: Vec<[u8; 3]> = Vec::new();
    let mut indexed = Vec::with_capacity(width * height);
    for &c in rgb.as_chunks::<3>().0 {
        let i = match colours.iter().position(|&k| k == c) {
            Some(i) => i,
            None => {
                colours.push(c);
                colours.len() - 1
            }
        };
        if colours.len() > 256 {
            return encode_raw(width, height, rgb, 2, 3, &[]);
        }
        indexed.push(i as u8);
    }
    let plte: Vec<u8> = colours.concat();
    encode_raw(width, height, &indexed, 3, 1, &plte)
}

/// Encodes RGBA bytes (4 per pixel, row by row) as a PNG file with transparency: indexed with
/// a transparency table when there are at most 256 different pixels, full RGBA otherwise.
pub fn encode_rgba(width: usize, height: usize, rgba: &[u8]) -> Vec<u8> {
    assert_eq!(
        rgba.len(),
        width * height * 4,
        "rgba buffer does not match {width}x{height}"
    );
    let mut colours: Vec<[u8; 4]> = Vec::new();
    let mut indexed = Vec::with_capacity(width * height);
    for &c in rgba.as_chunks::<4>().0 {
        let i = match colours.iter().position(|&k| k == c) {
            Some(i) => i,
            None => {
                colours.push(c);
                colours.len() - 1
            }
        };
        if colours.len() > 256 {
            return encode_raw_with(width, height, rgba, 6, 4, &[], &[]);
        }
        indexed.push(i as u8);
    }
    let plte: Vec<u8> = colours.iter().flat_map(|c| [c[0], c[1], c[2]]).collect();
    let trns: Vec<u8> = colours.iter().map(|c| c[3]).collect();
    encode_raw_with(width, height, &indexed, 3, 1, &plte, &trns)
}

fn encode_raw(
    width: usize,
    height: usize,
    data: &[u8],
    colour_type: u8,
    bytes_per_px: usize,
    plte: &[u8],
) -> Vec<u8> {
    encode_raw_with(width, height, data, colour_type, bytes_per_px, plte, &[])
}

fn encode_raw_with(
    width: usize,
    height: usize,
    data: &[u8],
    colour_type: u8,
    bytes_per_px: usize,
    plte: &[u8],
    trns: &[u8],
) -> Vec<u8> {
    let mut raw = Vec::with_capacity((width * bytes_per_px + 1) * height);
    for row in data.chunks_exact(width * bytes_per_px) {
        raw.push(0); // filter: none
        raw.extend_from_slice(row);
    }
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, colour_type, 0, 0, 0]);

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &ihdr);
    if !plte.is_empty() {
        chunk(&mut out, b"PLTE", plte);
    }
    if !trns.is_empty() {
        chunk(&mut out, b"tRNS", trns);
    }
    chunk(&mut out, b"IDAT", &crate::deflate::zlib(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

pub fn crc32(data: &[u8]) -> u32 {
    static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (n, slot) in t.iter_mut().enumerate() {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *slot = c;
        }
        t
    });
    !data.iter().fold(!0u32, |c, &b| {
        table[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8)
    })
}

pub(crate) fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &d in data {
        a = (a + d as u32) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

/// The seven passes of Adam7: first column and row, then the steps between them.
const ADAM7: [(usize, usize, usize, usize); 7] = [
    (0, 0, 8, 8),
    (4, 0, 8, 8),
    (0, 4, 4, 8),
    (2, 0, 4, 4),
    (0, 2, 2, 4),
    (1, 0, 2, 2),
    (0, 1, 1, 2),
];

/// The largest side a picture may have when read (a guard against damaged headers).
const MOST_SIDE: usize = 1 << 14;

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let (pa, pb, pc) = (
        (p - a as i16).abs(),
        (p - b as i16).abs(),
        (p - c as i16).abs(),
    );
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// Reads a PNG file into an image: RGBA, 8 bits a channel.
pub fn decode(png: &[u8]) -> Result<crate::image::Image, String> {
    if png.len() < 8 || png[..8] != *b"\x89PNG\r\n\x1a\n" {
        return Err("not a PNG file (it doesn't start with PNG's signature)".into());
    }
    let mut pos = 8;
    let mut header = None;
    let (mut palette, mut trns, mut idat) = (Vec::new(), None, Vec::new());
    loop {
        let Some(len) = png.get(pos..pos + 4) else {
            return Err("the file ends before its IEND chunk".into());
        };
        let len = u32::from_be_bytes([len[0], len[1], len[2], len[3]]) as usize;
        let Some(body) = png.get(pos + 4..pos + 8 + len) else {
            return Err("a chunk runs past the end of the file".into());
        };
        let Some(crc) = png.get(pos + 8 + len..pos + 12 + len) else {
            return Err("a chunk's checksum is missing".into());
        };
        let (kind, data) = (&body[..4], &body[4..]);
        let name = String::from_utf8_lossy(kind).to_string();
        if u32::from_be_bytes([crc[0], crc[1], crc[2], crc[3]]) != crc32(body) {
            return Err(format!(
                "chunk {name}: its checksum doesn't match (the file is damaged)"
            ));
        }
        pos += 12 + len;
        match kind {
            b"IHDR" => {
                if data.len() != 13 {
                    return Err("IHDR has the wrong length".into());
                }
                let w = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
                let h = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
                let (depth, colour, method, filter, interlace) =
                    (data[8], data[9], data[10], data[11], data[12]);
                let depths: &[u8] = match colour {
                    0 => &[1, 2, 4, 8, 16],
                    3 => &[1, 2, 4, 8],
                    2 | 4 | 6 => &[8, 16],
                    _ => return Err(format!("colour type {colour} doesn't exist")),
                };
                if !depths.contains(&depth) {
                    return Err(format!(
                        "colour type {colour} can't have {depth}-bit samples"
                    ));
                }
                if method != 0 || filter != 0 || interlace > 1 {
                    return Err("an unknown compression, filter or interlace method".into());
                }
                if w == 0 || h == 0 || w > MOST_SIDE || h > MOST_SIDE {
                    return Err(format!(
                        "a picture of {w} × {h} (each side must be 1 to {MOST_SIDE})"
                    ));
                }
                header = Some((w, h, depth as usize, colour, interlace == 1));
            }
            b"PLTE" => palette = data.to_vec(),
            b"tRNS" => trns = Some(data.to_vec()),
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            // ancillary chunks (a lower-case first letter) can be skipped; critical ones can't
            _ if kind[0].is_ascii_lowercase() => {}
            _ => return Err(format!("chunk {name} is critical and unknown")),
        }
    }
    let Some((w, h, depth, colour, interlaced)) = header else {
        return Err("no IHDR chunk".into());
    };
    if colour == 3 && palette.is_empty() {
        return Err("an indexed picture without its palette (PLTE)".into());
    }
    let raw = crate::inflate::zlib(&idat).map_err(|e| format!("the pixel data: {e}"))?;
    let channels = match colour {
        0 | 3 => 1,
        4 => 2,
        2 => 3,
        _ => 4,
    };
    let bits = channels * depth;
    let bpp = bits.div_ceil(8);
    let top = (1u32 << depth.min(16)) - 1;
    let to8 = |v: u32| if depth == 16 { v >> 8 } else { v * 255 / top };
    // the colour key of tRNS for grey and RGB pictures, as samples
    let key: Option<Vec<u32>> = match (colour, &trns) {
        (0, Some(t)) if t.len() >= 2 => Some(vec![u16::from_be_bytes([t[0], t[1]]) as u32]),
        (2, Some(t)) if t.len() >= 6 => Some(
            (0..3)
                .map(|k| u16::from_be_bytes([t[2 * k], t[2 * k + 1]]) as u32)
                .collect(),
        ),
        _ => None,
    };
    let mut image = crate::image::Image::new(w, h);
    let passes: &[(usize, usize, usize, usize)] = if interlaced { &ADAM7 } else { &[(0, 0, 1, 1)] };
    let mut at = 0;
    let mut samples = [0u32; 4];
    for &(x0, y0, dx, dy) in passes {
        if x0 >= w || y0 >= h {
            continue;
        }
        let (pw, ph) = ((w - x0).div_ceil(dx), (h - y0).div_ceil(dy));
        let stride = (pw * bits).div_ceil(8);
        let mut prev = vec![0u8; stride];
        let mut line = vec![0u8; stride];
        for row in 0..ph {
            let Some(src) = raw.get(at..at + 1 + stride) else {
                return Err("the pixel data is shorter than the picture".into());
            };
            at += 1 + stride;
            let filter = src[0];
            line.copy_from_slice(&src[1..]);
            for i in 0..stride {
                let a = if i >= bpp { line[i - bpp] } else { 0 };
                let b = prev[i];
                let c = if i >= bpp { prev[i - bpp] } else { 0 };
                let predict = match filter {
                    0 => 0,
                    1 => a,
                    2 => b,
                    3 => ((a as u16 + b as u16) / 2) as u8,
                    4 => paeth(a, b, c),
                    _ => return Err(format!("row filter {filter} doesn't exist")),
                };
                line[i] = line[i].wrapping_add(predict);
            }
            let y = y0 + row * dy;
            for col in 0..pw {
                for (ch, s) in samples.iter_mut().enumerate().take(channels) {
                    let bit = (col * channels + ch) * depth;
                    *s = match depth {
                        16 => u16::from_be_bytes([line[bit / 8], line[bit / 8 + 1]]) as u32,
                        8 => line[bit / 8] as u32,
                        _ => (line[bit / 8] as u32 >> (8 - depth - bit % 8)) & top,
                    };
                }
                let keyed = key.as_ref().is_some_and(|k| k[..] == samples[..channels]);
                let (r, g, b, alpha) = match colour {
                    0 => {
                        let v = to8(samples[0]);
                        (v, v, v, if keyed { 0 } else { 255 })
                    }
                    2 => (
                        to8(samples[0]),
                        to8(samples[1]),
                        to8(samples[2]),
                        if keyed { 0 } else { 255 },
                    ),
                    3 => {
                        let i = samples[0] as usize;
                        let Some(rgb) = palette.get(i * 3..i * 3 + 3) else {
                            return Err(format!("palette index {i} is past the palette's end"));
                        };
                        let a = trns
                            .as_ref()
                            .and_then(|t| t.get(i))
                            .map_or(255, |&a| a as u32);
                        (rgb[0] as u32, rgb[1] as u32, rgb[2] as u32, a)
                    }
                    4 => {
                        let v = to8(samples[0]);
                        (v, v, v, to8(samples[1]))
                    }
                    _ => (
                        to8(samples[0]),
                        to8(samples[1]),
                        to8(samples[2]),
                        to8(samples[3]),
                    ),
                };
                image.pixels[y * w + x0 + col * dx] = (alpha << 24) | (r << 16) | (g << 8) | b;
            }
            std::mem::swap(&mut prev, &mut line);
        }
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The chunks of a PNG file, by kind, with their data.
    fn chunks(png: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        let mut at = 8;
        while at + 8 <= png.len() {
            let len = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
            let kind = String::from_utf8_lossy(&png[at + 4..at + 8]).to_string();
            out.push((kind, png[at + 8..at + 8 + len].to_vec()));
            at += 12 + len;
        }
        out
    }

    #[test]
    fn transparency_is_kept_indexed_and_in_full() {
        // two colours, one see-through: indexed, with a transparency table after the palette
        let rgba = [255, 0, 0, 255, 0, 0, 0, 0, 255, 0, 0, 255, 0, 0, 0, 0];
        let c = chunks(&encode_rgba(2, 2, &rgba));
        let kinds: Vec<&str> = c.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(kinds, ["IHDR", "PLTE", "tRNS", "IDAT", "IEND"]);
        assert_eq!(c[0].1[9], 3, "indexed");
        assert_eq!(c[2].1, [255, 0]);
        // more than 256 different pixels: colour type 6, RGBA, no palette
        let many: Vec<u8> = (0..300u32)
            .flat_map(|i| [i as u8, (i >> 8) as u8, 7, (i % 2) as u8 * 255])
            .collect();
        let c = chunks(&encode_rgba(300, 1, &many));
        assert_eq!(c[0].1[9], 6, "RGBA");
        assert!(c.iter().all(|(k, _)| k != "PLTE" && k != "tRNS"));
    }
}
