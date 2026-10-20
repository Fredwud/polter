// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Deflate, the other way round from `inflate`: data packed into a zlib stream, for the PNG files
//! Polter writes. Repeats are found through hash chains (LZ77) and coded with deflate's fixed
//! Huffman codes, in one block. Pixel art packs well this way: a row drawn twice for a scaled
//! picture, a wall of one colour, a frame much like the one before it. The same data gives the
//! same bytes.

use crate::inflate::{DISTANCE_BASE, DISTANCE_EXTRA, LENGTH_BASE, LENGTH_EXTRA};

/// How far back a repeat may start, and how long it may be (deflate's limits).
const WINDOW: usize = 32_768;
const LONGEST: usize = 258;
/// How many earlier places with the same three bytes are tried: more packs tighter, slower.
const TRIES: usize = 48;
const HASH_BITS: u32 = 15;

/// The data as a zlib stream (RFC 1950 around RFC 1951).
pub fn zlib(data: &[u8]) -> Vec<u8> {
    let mut w = Bits {
        out: vec![0x78, 0x01],
        acc: 0,
        n: 0,
    };
    // one block, the last, with the fixed codes
    w.put(1, 1);
    w.put(1, 2);
    let mut head = vec![usize::MAX; 1 << HASH_BITS];
    let mut prev = vec![usize::MAX; WINDOW];
    let mut i = 0;
    while i < data.len() {
        let (mut best, mut back) = (0, 0);
        if i + 2 < data.len() {
            let most = LONGEST.min(data.len() - i);
            let mut at = head[hash(data, i)];
            let mut tries = 0;
            while at != usize::MAX && i - at <= WINDOW && tries < TRIES {
                let len = data[at..at + most]
                    .iter()
                    .zip(&data[i..i + most])
                    .take_while(|(a, b)| a == b)
                    .count();
                if len > best {
                    (best, back) = (len, i - at);
                    if len == most {
                        break;
                    }
                }
                let next = prev[at % WINDOW];
                if next == usize::MAX || next >= at {
                    break;
                }
                at = next;
                tries += 1;
            }
        }
        if best >= 3 {
            w.length(best);
            w.distance(back);
            for k in i..i + best {
                insert(data, k, &mut head, &mut prev);
            }
            i += best;
        } else {
            w.literal(data[i] as u16);
            insert(data, i, &mut head, &mut prev);
            i += 1;
        }
    }
    w.literal(256);
    if w.n > 0 {
        w.out.push(w.acc as u8);
    }
    w.out
        .extend_from_slice(&crate::png::adler32(data).to_be_bytes());
    w.out
}

/// Where the three bytes at `i` are filed.
fn hash(data: &[u8], i: usize) -> usize {
    let v = (data[i] as u32) << 16 | (data[i + 1] as u32) << 8 | data[i + 2] as u32;
    (v.wrapping_mul(2_654_435_761) >> (32 - HASH_BITS)) as usize
}

/// Files the place `i` under its three bytes, in front of the places filed there before.
fn insert(data: &[u8], i: usize, head: &mut [usize], prev: &mut [usize]) {
    if i + 2 < data.len() {
        let h = hash(data, i);
        prev[i % WINDOW] = head[h];
        head[h] = i;
    }
}

struct Bits {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl Bits {
    /// `count` bits of `value`, least significant first (how deflate stores numbers).
    fn put(&mut self, value: u32, count: u32) {
        self.acc |= value << self.n;
        self.n += count;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }

    /// A Huffman code, most significant bit first (how deflate stores codes).
    fn code(&mut self, code: u32, len: u32) {
        let reversed = code.reverse_bits() >> (32 - len);
        self.put(reversed, len);
    }

    /// A literal byte or a length symbol (256 to 287) in the fixed code.
    fn literal(&mut self, sym: u16) {
        let s = sym as u32;
        match sym {
            0..=143 => self.code(0x30 + s, 8),
            144..=255 => self.code(0x190 + s - 144, 9),
            256..=279 => self.code(s - 256, 7),
            _ => self.code(0xc0 + s - 280, 8),
        }
    }

    fn length(&mut self, len: usize) {
        let k = LENGTH_BASE
            .iter()
            .rposition(|&b| b as usize <= len)
            .expect("3 to 258");
        self.literal(257 + k as u16);
        self.put(
            (len - LENGTH_BASE[k] as usize) as u32,
            LENGTH_EXTRA[k] as u32,
        );
    }

    fn distance(&mut self, back: usize) {
        let k = DISTANCE_BASE
            .iter()
            .rposition(|&b| b as usize <= back)
            .expect("1 to 32768");
        self.code(k as u32, 5);
        self.put(
            (back - DISTANCE_BASE[k] as usize) as u32,
            DISTANCE_EXTRA[k] as u32,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(seed: u64, n: usize, kinds: u64) -> Vec<u8> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                s = s
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                ((s >> 33) % kinds) as u8
            })
            .collect()
    }

    #[test]
    fn inflate_gives_back_what_deflate_packed() {
        let mut row: Vec<u8> = noise(3, 960, 256);
        let picture: Vec<u8> = (0..200).flat_map(|_| row.clone()).collect();
        row.reverse();
        let cases = [
            vec![],
            vec![42],
            vec![0, 0],
            vec![9; 100_000],
            noise(1, 70_000, 256),
            noise(2, 70_000, 4),
            picture,
            (0..=255u8).cycle().take(300_000).collect(),
        ];
        for data in cases {
            let packed = zlib(&data);
            assert_eq!(
                crate::inflate::zlib(&packed).unwrap(),
                data,
                "{} bytes",
                data.len()
            );
        }
    }

    #[test]
    fn pictures_pack_small() {
        // a scaled picture: every pixel twice, every row twice
        let small = noise(4, 320 * 3, 6);
        let mut big = Vec::new();
        for _ in 0..180 {
            let line: Vec<u8> = small.chunks(3).flat_map(|p| [p, p].concat()).collect();
            big.extend_from_slice(&line);
            big.extend_from_slice(&line);
        }
        let packed = zlib(&big);
        assert!(
            packed.len() * 20 < big.len(),
            "{} of {} bytes",
            packed.len(),
            big.len()
        );
        // the same data, the same bytes
        assert_eq!(zlib(&big), packed);
    }
}
