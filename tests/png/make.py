#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
# the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.
"""Test pictures for Polter's PNG decoder, with what they must decode to.

    python3 tests/png/make.py

writes tests/png/*.png: every colour type at every bit depth PNG allows, with and without
transparency, plain and interlaced (Adam7), every row filter, stored and compressed. Each is
made from known pixels, so what it must decode to is known without decoding it. It also
decodes every other PNG in the engine's folder with Python's zlib (an implementation that is
not Polter's) and writes the expected pixels of all of them to tests/png/expected.txt:

    path width height fnv64-of-the-RGBA-bytes

Decoded pixels are RGBA, 8 bits a channel: 16-bit samples keep their high byte, grey levels
below 8 bits are scaled to 0..255, grey is R = G = B, alpha is 255 unless tRNS says otherwise.
"""
import os
import struct
import sys
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
ADAM7 = [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)]
CHANNELS = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}


def fnv(data):
    h = 0xCBF29CE484222325
    for b in data:
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)


def paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


def filter_rows(rows, bpp, first_filter):
    """Filters each row with filter (first_filter + row index) % 5, as an encoder would."""
    out = bytearray()
    prev = bytes(len(rows[0])) if rows else b""
    for k, row in enumerate(rows):
        f = (first_filter + k) % 5
        out.append(f)
        for i, x in enumerate(row):
            a = row[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            pred = [0, a, b, (a + b) // 2, paeth(a, b, c)][f]
            out.append((x - pred) & 255)
        prev = row
    return bytes(out)


def pack_samples(samples, depth):
    """A row of samples at a bit depth, packed big-endian as PNG wants."""
    if depth == 8:
        return bytes(samples)
    if depth == 16:
        return b"".join(struct.pack(">H", s) for s in samples)
    out, acc, n = bytearray(), 0, 0
    for s in samples:
        acc = (acc << depth) | s
        n += depth
        if n == 8:
            out.append(acc)
            acc, n = 0, 0
    if n:
        out.append(acc << (8 - n))
    return bytes(out)


def synthetic(name, w, h, ct, depth, interlace, trns, level):
    """A picture of known pixels, written in one PNG variant; its expected RGBA."""
    top = (1 << depth) - 1
    palette = [((i * 37) & 255, (i * 91 + 13) & 255, (255 - i * 7) & 255) for i in range(min(256, 1 << depth))]

    def sample(x, y, c):
        # every sample value of the depth turns up somewhere; 16-bit ones vary in both bytes
        if depth == 16:
            return (x * 4099 + y * 7919 + c * 30011 + x * y * 97) % 65536
        return (x * 7 + y * 13 + c * 29 + (x * y) % 11) % (top + 1)

    raw = {}  # (x, y) -> the samples of the pixel
    expect = bytearray()
    key = None
    for y in range(h):
        for x in range(w):
            s = [sample(x, y, c) for c in range(CHANNELS[ct])]
            raw[(x, y)] = s
    if trns and ct == 0:
        key = raw[(1, 0)][0]
    if trns and ct == 2:
        key = tuple(raw[(1, 0)])
    alphas = [(255 - i * 23) & 255 for i in range(len(palette))] if trns and ct == 3 else None

    def to8(v):
        if depth == 16:
            return v >> 8
        return v * 255 // top

    for y in range(h):
        for x in range(w):
            s = raw[(x, y)]
            if ct == 0:
                g = to8(s[0])
                a = 0 if key is not None and s[0] == key else 255
                expect += bytes((g, g, g, a))
            elif ct == 2:
                a = 0 if key is not None and tuple(s) == key else 255
                expect += bytes((to8(s[0]), to8(s[1]), to8(s[2]), a))
            elif ct == 3:
                r, g, b = palette[s[0]]
                a = alphas[s[0]] if alphas else 255
                expect += bytes((r, g, b, a))
            elif ct == 4:
                g = to8(s[0])
                expect += bytes((g, g, g, to8(s[1])))
            else:
                expect += bytes(to8(v) for v in s)
    bits = CHANNELS[ct] * depth
    bpp = max(1, bits // 8)

    def rows_of(xs, ys):
        return [pack_samples([v for x in xs for v in raw[(x, y)]], depth) for y in ys]

    data = b""
    if interlace:
        for k, (x0, y0, dx, dy) in enumerate(ADAM7):
            xs, ys = list(range(x0, w, dx)), list(range(y0, h, dy))
            if xs and ys:
                data += filter_rows(rows_of(xs, ys), bpp, k)
    else:
        data = filter_rows(rows_of(range(w), range(h)), bpp, 0)
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, depth, ct, 0, 0, interlace))
    if ct == 3:
        png += chunk(b"PLTE", b"".join(bytes(p) for p in palette))
        if alphas:
            png += chunk(b"tRNS", bytes(alphas))
    if key is not None:
        png += chunk(b"tRNS", struct.pack(">H", key) if ct == 0 else struct.pack(">HHH", *key))
    z = zlib.compress(data, level)
    # IDAT split in two, as encoders do
    half = len(z) // 2
    png += chunk(b"IDAT", z[:half]) + chunk(b"IDAT", z[half:]) + chunk(b"IEND", b"")
    with open(os.path.join(HERE, name), "wb") as f:
        f.write(png)
    return w, h, bytes(expect)


def decode(path):
    """Python's own decoding of a PNG (zlib for the inflate), to RGBA bytes."""
    b = open(path, "rb").read()
    pos, idat, plte, trns = 8, b"", None, None
    while pos < len(b):
        n = struct.unpack(">I", b[pos:pos + 4])[0]
        kind, data = b[pos + 4:pos + 8], b[pos + 8:pos + 8 + n]
        pos += 12 + n
        if kind == b"IHDR":
            w, h, depth, ct, _, _, inter = struct.unpack(">IIBBBBB", data)
        elif kind == b"PLTE":
            plte = data
        elif kind == b"tRNS":
            trns = data
        elif kind == b"IDAT":
            idat += data
    raw = zlib.decompress(idat)
    bits = CHANNELS[ct] * depth
    bpp = max(1, bits // 8)
    passes = ADAM7 if inter else [(0, 0, 1, 1)]
    img = {}
    off = 0
    for x0, y0, dx, dy in passes:
        xs, ys = list(range(x0, w, dx)), list(range(y0, h, dy))
        if not xs or not ys:
            continue
        stride = (len(xs) * bits + 7) // 8
        prev = bytearray(stride)
        for y in ys:
            f, line = raw[off], bytearray(raw[off + 1:off + 1 + stride])
            off += 1 + stride
            for i in range(stride):
                a = line[i - bpp] if i >= bpp else 0
                c = prev[i - bpp] if i >= bpp else 0
                line[i] = (line[i] + [0, a, prev[i], (a + prev[i]) // 2, paeth(a, prev[i], c)][f]) & 255
            prev = line
            for k, x in enumerate(xs):
                vals = []
                for ch in range(CHANNELS[ct]):
                    bit = (k * CHANNELS[ct] + ch) * depth
                    if depth == 16:
                        vals.append(line[bit // 8] * 256 + line[bit // 8 + 1])
                    elif depth == 8:
                        vals.append(line[bit // 8])
                    else:
                        vals.append((line[bit // 8] >> (8 - depth - bit % 8)) & ((1 << depth) - 1))
                img[(x, y)] = vals
    top = (1 << depth) - 1
    to8 = (lambda v: v >> 8) if depth == 16 else (lambda v: v * 255 // top)
    out = bytearray()
    for y in range(h):
        for x in range(w):
            s = img[(x, y)]
            if ct == 3:
                r, g, bl = plte[s[0] * 3:s[0] * 3 + 3]
                a = trns[s[0]] if trns and s[0] < len(trns) else 255
                out += bytes((r, g, bl, a))
            elif ct == 0:
                a = 0 if trns and struct.unpack(">H", trns[:2])[0] == s[0] else 255
                out += bytes((to8(s[0]),) * 3 + (a,))
            elif ct == 2:
                a = 0 if trns and struct.unpack(">HHH", trns[:6]) == tuple(s) else 255
                out += bytes([to8(v) for v in s] + [a])
            elif ct == 4:
                out += bytes((to8(s[0]),) * 3 + (to8(s[1]),))
            else:
                out += bytes(to8(v) for v in s)
    return w, h, bytes(out)


def main():
    lines = []
    cases = [(0, d) for d in (1, 2, 4, 8, 16)] + [(2, 8), (2, 16)] + [(3, d) for d in (1, 2, 4, 8)] + \
            [(4, 8), (4, 16), (6, 8), (6, 16)]
    for ct, depth in cases:
        for interlace in (0, 1):
            for trns in ((False, True) if ct in (0, 2, 3) else (False,)):
                for level in ((0, 9) if (ct, depth) == (6, 8) else (6,)):
                    name = f"t{ct}_d{depth}{'_adam7' if interlace else ''}{'_trns' if trns else ''}_z{level}.png"
                    # odd sizes, so rows end inside a byte and passes come out uneven
                    w, h, rgba = synthetic(name, 13, 11, ct, depth, interlace, trns, level)
                    lines.append(f"tests/png/{name} {w} {h} {fnv(rgba):016x}")
    # every other PNG in the engine, decoded by Python
    for dirpath, dirs, files in os.walk(ROOT):
        dirs[:] = [d for d in dirs if d not in ("target", ".git", ".polter")]
        for f in sorted(files):
            path = os.path.join(dirpath, f)
            rel = os.path.relpath(path, ROOT)
            if f.endswith(".png") and not rel.startswith("tests/png/"):
                w, h, rgba = decode(path)
                lines.append(f"{rel} {w} {h} {fnv(rgba):016x}")
    # the synthetic ones decoded by Python too: its decoder agrees with what they were made from
    for line in lines:
        rel, w, h, want = line.split()
        if rel.startswith("tests/png/"):
            _, _, got = decode(os.path.join(ROOT, rel))
            assert f"{fnv(got):016x}" == want, f"Python's decoding of {rel} disagrees with its pixels"
    with open(os.path.join(HERE, "expected.txt"), "w") as f:
        f.write("\n".join(sorted(lines)) + "\n")
    print(f"{len(lines)} pictures in tests/png/expected.txt")


if __name__ == "__main__":
    sys.exit(main())
