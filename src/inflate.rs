// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Inflate (RFC 1951) inside the zlib wrapper (RFC 1950): what a PNG file's pixels are packed
//! with. Small and without dependencies, after the shape of Mark Adler's `puff`: canonical
//! Huffman codes decoded a bit at a time. Not the fastest way, but short and easy to check; a
//! game's pictures are small.

/// The bits of a stream, least significant first, as deflate packs them.
struct Bits<'a> {
    data: &'a [u8],
    /// The next byte to take.
    pos: usize,
    buffer: u64,
    count: u32,
}

impl Bits<'_> {
    fn need(&mut self, n: u32) -> Result<(), String> {
        while self.count < n {
            let Some(&b) = self.data.get(self.pos) else {
                return Err("the compressed data ends early".into());
            };
            self.buffer |= (b as u64) << self.count;
            self.pos += 1;
            self.count += 8;
        }
        Ok(())
    }

    fn bits(&mut self, n: u32) -> Result<u32, String> {
        self.need(n)?;
        let v = (self.buffer & ((1u64 << n) - 1)) as u32;
        self.buffer >>= n;
        self.count -= n;
        Ok(v)
    }

    /// Drops the bits left in the current byte (before a stored block).
    fn align(&mut self) {
        let drop = self.count % 8;
        self.buffer >>= drop;
        self.count -= drop;
    }

    /// The first byte not read (whole bytes held in the buffer are given back).
    fn consumed(&self) -> usize {
        self.pos - (self.count / 8) as usize
    }
}

/// A canonical Huffman code: how many codes there are of each length, and the symbols in code
/// order.
struct Huffman {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Result<Huffman, String> {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[l as usize] += 1;
        }
        // more codes of a length than the shorter ones leave room for: not a code
        let mut left = 1i32;
        for &count in &counts[1..] {
            left = left * 2 - count as i32;
            if left < 0 {
                return Err("a Huffman code is over-subscribed".into());
            }
        }
        let mut offsets = [0u16; 16];
        for l in 1..15 {
            offsets[l + 1] = offsets[l] + counts[l];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (symbol, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[offsets[l as usize] as usize] = symbol as u16;
                offsets[l as usize] += 1;
            }
        }
        Ok(Huffman { counts, symbols })
    }

    fn decode(&self, bits: &mut Bits) -> Result<u16, String> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= bits.bits(1)? as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                return Ok(self.symbols[(index + code - first) as usize]);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err("a code that isn't in the Huffman table".into())
    }
}

pub(crate) const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
pub(crate) const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
pub(crate) const DISTANCE_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
pub(crate) const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
/// The order the lengths of the code-length code come in.
const CODE_LENGTH_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// One block's literals, lengths and distances, until its end code.
fn codes(
    bits: &mut Bits,
    out: &mut Vec<u8>,
    lengths: &Huffman,
    distances: &Huffman,
) -> Result<(), String> {
    loop {
        let symbol = lengths.decode(bits)? as usize;
        if symbol < 256 {
            out.push(symbol as u8);
            continue;
        }
        if symbol == 256 {
            return Ok(());
        }
        let k = symbol - 257;
        if k >= LENGTH_BASE.len() {
            return Err(format!("length code {symbol} doesn't exist"));
        }
        let len = LENGTH_BASE[k] as usize + bits.bits(LENGTH_EXTRA[k] as u32)? as usize;
        let d = distances.decode(bits)? as usize;
        if d >= DISTANCE_BASE.len() {
            return Err(format!("distance code {d} doesn't exist"));
        }
        let dist = DISTANCE_BASE[d] as usize + bits.bits(DISTANCE_EXTRA[d] as u32)? as usize;
        if dist > out.len() {
            return Err(format!(
                "a distance of {dist} reaches before the start (only {} bytes so far)",
                out.len()
            ));
        }
        // byte by byte: a copy may overlap what it is writing
        let from = out.len() - dist;
        for i in 0..len {
            out.push(out[from + i]);
        }
    }
}

/// The codes of a block with fixed Huffman codes.
fn fixed() -> (Huffman, Huffman) {
    let mut l = [0u8; 288];
    l[..144].fill(8);
    l[144..256].fill(9);
    l[256..280].fill(7);
    l[280..].fill(8);
    (
        Huffman::new(&l).expect("the fixed code"),
        Huffman::new(&[5; 30]).expect("the fixed distances"),
    )
}

/// The codes a block with dynamic Huffman codes describes at its start.
fn dynamic(bits: &mut Bits) -> Result<(Huffman, Huffman), String> {
    let nlen = bits.bits(5)? as usize + 257;
    let ndist = bits.bits(5)? as usize + 1;
    let ncode = bits.bits(4)? as usize + 4;
    if nlen > 286 || ndist > 30 {
        return Err("a block has too many codes".into());
    }
    let mut code_lengths = [0u8; 19];
    for &k in &CODE_LENGTH_ORDER[..ncode] {
        code_lengths[k] = bits.bits(3)? as u8;
    }
    let code = Huffman::new(&code_lengths)?;
    let mut lengths = Vec::with_capacity(nlen + ndist);
    while lengths.len() < nlen + ndist {
        let symbol = code.decode(bits)?;
        let (value, repeat) = match symbol {
            0..=15 => (symbol as u8, 1),
            16 => {
                let Some(&last) = lengths.last() else {
                    return Err("a repeat with nothing before it".into());
                };
                (last, 3 + bits.bits(2)? as usize)
            }
            17 => (0, 3 + bits.bits(3)? as usize),
            _ => (0, 11 + bits.bits(7)? as usize),
        };
        if lengths.len() + repeat > nlen + ndist {
            return Err("code lengths run past their count".into());
        }
        lengths.extend(std::iter::repeat_n(value, repeat));
    }
    if lengths[256] == 0 {
        return Err("a block without an end code".into());
    }
    Ok((
        Huffman::new(&lengths[..nlen])?,
        Huffman::new(&lengths[nlen..])?,
    ))
}

/// Inflates a raw deflate stream; also the first byte after it.
pub fn inflate(data: &[u8]) -> Result<(Vec<u8>, usize), String> {
    let mut bits = Bits {
        data,
        pos: 0,
        buffer: 0,
        count: 0,
    };
    let mut out = Vec::new();
    loop {
        let last = bits.bits(1)? == 1;
        match bits.bits(2)? {
            0 => {
                bits.align();
                let len = bits.bits(16)?;
                let nlen = bits.bits(16)?;
                if len != !nlen & 0xffff {
                    return Err("a stored block's length doesn't match its complement".into());
                }
                for _ in 0..len {
                    out.push(bits.bits(8)? as u8);
                }
            }
            1 => {
                let (l, d) = fixed();
                codes(&mut bits, &mut out, &l, &d)?;
            }
            2 => {
                let (l, d) = dynamic(&mut bits)?;
                codes(&mut bits, &mut out, &l, &d)?;
            }
            _ => return Err("a block of type 3, which doesn't exist".into()),
        }
        if last {
            return Ok((out, bits.consumed()));
        }
    }
}

/// Unpacks a zlib stream (its header, the deflate data, the Adler-32 checksum, which is checked).
pub fn zlib(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 6 {
        return Err("the compressed data is too short".into());
    }
    let (cmf, flg) = (data[0], data[1]);
    if cmf & 0x0f != 8 || !(cmf as u16 * 256 + flg as u16).is_multiple_of(31) {
        return Err("not a zlib stream (its header is wrong)".into());
    }
    if flg & 0x20 != 0 {
        return Err("a zlib stream with a preset dictionary, which PNG doesn't use".into());
    }
    let (out, used) = inflate(&data[2..])?;
    let at = 2 + used;
    let Some(sum) = data.get(at..at + 4) else {
        return Err("the checksum after the compressed data is missing".into());
    };
    if u32::from_be_bytes([sum[0], sum[1], sum[2], sum[3]]) != crate::png::adler32(&out) {
        return Err("the checksum doesn't match: the data is damaged".into());
    }
    Ok(out)
}
