#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
# the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.
"""A game's app icon from a picture of the game: a square shot (for example 128 x 128 pixels,
scaled 8 times to 1024) with rounded corners, as the iconset macOS wants (tools/bundle.sh).

    python3 tools/icon.py SHOT.png OUT.iconset

SHOT.png is a 1024 x 1024 RGB picture (`GAME shot --frame N --scale 8` of a square game, or a
square cut from a bigger shot). The smaller sizes are made with sips; then
`iconutil -c icns OUT.iconset` makes the .icns.
"""

import os
import struct
import subprocess
import sys
import zlib


def load_rgb(path):
    """A PNG as Polter writes it (8-bit RGB, or indexed when it has 256 colours or fewer):
    width, height, rows of RGB bytes."""
    data = open(path, "rb").read()
    i, idat, w, h, kind_of, palette = 8, b"", 0, 0, 2, b""
    while i < len(data):
        n = struct.unpack(">I", data[i:i + 4])[0]
        kind, body = data[i + 4:i + 8], data[i + 8:i + 8 + n]
        if kind == b"IHDR":
            w, h = struct.unpack(">II", body[:8])
            kind_of = body[9]
            assert body[8] == 8 and kind_of in (2, 3), "8-bit RGB or indexed expected"
        elif kind == b"PLTE":
            palette = body
        elif kind == b"IDAT":
            idat += body
        i += 12 + n
    bpp = 3 if kind_of == 2 else 1
    raw, stride, prev, rows = zlib.decompress(idat), w * bpp + 1, bytearray(w * bpp), []
    for y in range(h):
        f, line = raw[y * stride], bytearray(raw[y * stride + 1:(y + 1) * stride])
        for x in range(len(line)):
            a = line[x - bpp] if x >= bpp else 0
            b = prev[x]
            c = prev[x - bpp] if x >= bpp else 0
            if f == 1:
                line[x] = (line[x] + a) & 255
            elif f == 2:
                line[x] = (line[x] + b) & 255
            elif f == 3:
                line[x] = (line[x] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        prev = line
        if bpp == 1:
            rows.append(b"".join(palette[v * 3:v * 3 + 3] for v in line))
        else:
            rows.append(bytes(line))
    return w, h, rows


def save_rgba(path, w, h, rows):
    raw = b"".join(b"\x00" + r for r in rows)

    def chunk(t, c):
        return struct.pack(">I", len(c)) + t + c + struct.pack(">I", zlib.crc32(t + c) & 0xFFFFFFFF)

    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
                + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def rounded(w, h, rows, radius):
    """The picture with its corners cut round (in whole pixels of the art: steps of 8)."""
    out = []
    for y in range(h):
        line = bytearray()
        for x in range(w):
            # the art's own pixel, so the rounding steps like the pixels it cuts
            cx, cy = (x // 8) * 8 + 4, (y // 8) * 8 + 4
            dx = max(radius - cx, cx - (w - radius), 0)
            dy = max(radius - cy, cy - (h - radius), 0)
            inside = dx * dx + dy * dy <= radius * radius
            line += rows[y][x * 3:x * 3 + 3] + bytes([255 if inside else 0])
        out.append(bytes(line))
    return out


def main():
    shot, iconset = sys.argv[1], sys.argv[2]
    w, h, rows = load_rgb(shot)
    assert (w, h) == (1024, 1024), f"a 1024 x 1024 picture, not {w} x {h}"
    os.makedirs(iconset, exist_ok=True)
    big = os.path.join(iconset, "icon_512x512@2x.png")
    save_rgba(big, w, h, rounded(w, h, rows, 184))
    for size in (16, 32, 128, 256, 512):
        for scale in (1, 2):
            px = size * scale
            name = f"icon_{size}x{size}{'@2x' if scale == 2 else ''}.png"
            if px == 1024:
                continue
            subprocess.run(["sips", "-z", str(px), str(px), big, "--out",
                            os.path.join(iconset, name)], check=True, capture_output=True)
    print(iconset)


if __name__ == "__main__":
    main()
