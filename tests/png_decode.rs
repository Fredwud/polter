// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Polter's PNG reader against pixels known another way: test pictures in every colour type and
//! bit depth, interlaced and not, every row filter, stored and compressed (made from known
//! pixels), and every other PNG in the engine (decoded by Python's zlib). `tests/png/make.py`
//! writes them and `tests/png/expected.txt`.

use polter::image::Image;
use std::path::Path;

fn fnv(image: &Image) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &p in &image.pixels {
        for b in [(p >> 16) as u8, (p >> 8) as u8, p as u8, (p >> 24) as u8] {
            h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
    }
    h
}

#[test]
fn every_picture_reads_as_it_was_made() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expected = std::fs::read_to_string(root.join("tests/png/expected.txt")).unwrap();
    let mut wrong = Vec::new();
    let mut n = 0;
    for line in expected.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let [path, w, h, hash] = parts[..] else {
            panic!("a line of expected.txt: {line}");
        };
        // the engine's other pictures are checked where they are (not in a package, say)
        if !path.starts_with("tests/png/") && !root.join(path).exists() {
            continue;
        }
        n += 1;
        match Image::load(root.join(path)) {
            Ok(image) => {
                let got = format!("{} {} {:016x}", image.width, image.height, fnv(&image));
                if got != format!("{w} {h} {hash}") {
                    wrong.push(format!("{path}: {got}, wanted {w} {h} {hash}"));
                }
            }
            Err(e) => wrong.push(format!("{path}: {e}")),
        }
    }
    assert!(n >= 54, "only {n} pictures");
    assert!(
        wrong.is_empty(),
        "{} of {n} wrong:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

#[test]
fn what_polter_writes_it_reads_back() {
    // few colours (an indexed file) and many (RGBA), with see-through pixels
    for colours in [7u32, 900] {
        let (w, h) = (37usize, 23usize);
        let mut rgba = Vec::new();
        let mut want = Image::new(w, h);
        for i in 0..w * h {
            let c = (i as u32 * 2_654_435_761) % colours;
            let (r, g, b) = ((c * 37) as u8, (c * 11) as u8, (c * 3) as u8);
            let a = if i % 5 == 0 { 0 } else { 255 };
            rgba.extend_from_slice(&[r, g, b, a]);
            // a see-through pixel's colour is not kept: it reads as see-through black
            want.pixels[i] = if a == 0 {
                0
            } else {
                0xff00_0000 | (r as u32) << 16 | (g as u32) << 8 | b as u32
            };
        }
        let mut got = Image::from_png(&polter::png::encode_rgba(w, h, &rgba)).unwrap();
        for p in got.pixels.iter_mut() {
            if *p >> 24 == 0 {
                *p = 0;
            }
        }
        assert_eq!(got, want, "{colours} colours");
    }
}

#[test]
fn damaged_files_say_what_is_wrong() {
    let good = polter::png::encode_rgba(4, 3, &[200; 48]);
    assert!(Image::from_png(&good).is_ok());
    let cases: [(&str, Vec<u8>); 4] = [
        ("PNG's signature", b"GIF89a, not a PNG".to_vec()),
        // cut inside the header's data, wherever the packed pixels end
        ("past the end", good[..8 + 8 + 6].to_vec()),
        ("checksum", {
            let mut bad = good.clone();
            bad[20] ^= 1; // inside IHDR
            bad
        }),
        ("IEND", good[..good.len() - 12].to_vec()),
    ];
    for (says, bytes) in cases {
        let err = Image::from_png(&bytes).unwrap_err();
        assert!(err.contains(says), "{err:?} should say {says:?}");
    }
    // a missing file is named
    let err = Image::load("no/such/picture.png").unwrap_err();
    assert!(err.contains("no/such/picture.png"), "{err}");
}

#[test]
fn a_sheet_cuts_into_frames() {
    let mut sheet = Image::new(6, 4);
    for (i, p) in sheet.pixels.iter_mut().enumerate() {
        *p = 0xff00_0000 | i as u32;
    }
    let frames = sheet.frames(3, 2);
    assert_eq!(frames.len(), 4);
    assert_eq!(
        frames[1].pixels,
        [
            0xff00_0003,
            0xff00_0004,
            0xff00_0005,
            0xff00_0009,
            0xff00_000a,
            0xff00_000b
        ]
    );
    assert_eq!(
        sheet.part(5, 3, 2, 2).pixels,
        [0xff00_0017, 0, 0, 0],
        "outside is see-through"
    );
}
