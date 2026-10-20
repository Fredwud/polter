#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
# the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.
"""LANTERN's pictures, drawn as text and written as PNG files (and the hero's sheet as Aseprite
writes it, JSON with tags), so they can be changed in a text editor and made again:

    python3 tools/art.py            writes assets/*.png, assets/hero.json
    python3 tools/art.py --sheet F  also a sheet of everything, six times the size, to look at
"""
import json
import os
import struct
import sys
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
ASSETS = os.path.join(os.path.dirname(HERE), "assets")

# the palette: a cold cave, a warm lantern
P = {
    ".": None,
    "k": 0x16101a,  # outline
    "h": 0x3a2a22,  # hat
    "H": 0x5a4032,  # hat, lit
    "s": 0xe2a87e,  # skin
    "S": 0xf3c9a0,  # skin, lit
    "c": 0x7a3f2a,  # coat
    "C": 0xa8573a,  # coat, lit
    "p": 0x34304a,  # trousers
    "b": 0x241a16,  # boots
    "m": 0x6f6a72,  # lantern's metal
    "L": 0xffd27a,  # lantern's light
    "W": 0xfff3cf,  # its brightest
    "y": 0xffc04a,  # spark
    "Y": 0xfff1b8,  # spark, bright
    "o": 0xff8a3a,  # spark's edge
    "w": 0x6a4426,  # door: wood
    "v": 0x8f5e35,  # wood, lit
    "i": 0x464a58,  # iron
    "I": 0x6c7183,  # iron, lit
    "d": 0x07060a,  # an open door's dark
}


def png(path, w, h, rgba):
    raw = b"".join(b"\0" + bytes(rgba[y * w * 4:(y + 1) * w * 4]) for y in range(h))

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
    data += chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(data)


class Sheet:
    """A picture made of cells, RGBA."""

    def __init__(self, w, h):
        self.w, self.h = w, h
        self.px = [0, 0, 0, 0] * (w * h)

    def put(self, x, y, rgb, a=255):
        if 0 <= x < self.w and 0 <= y < self.h:
            i = (y * self.w + x) * 4
            self.px[i:i + 4] = [rgb >> 16 & 255, rgb >> 8 & 255, rgb & 255, a]

    def grid(self, ox, oy, rows):
        assert len({len(r) for r in rows}) == 1, f"rows of different widths: {rows}"
        for y, row in enumerate(rows):
            for x, ch in enumerate(row):
                if P[ch] is not None:
                    self.put(ox + x, oy + y, P[ch])

    def save(self, name):
        png(os.path.join(ASSETS, name), self.w, self.h, self.px)


def hash01(x, y, k):
    """A number in 0..1 from a cell, the same every time."""
    h = (x * 374761393 + y * 668265263 + k * 2147483647) & 0xFFFFFFFF
    h = ((h ^ (h >> 13)) * 1274126177) & 0xFFFFFFFF
    return ((h ^ (h >> 16)) & 0xFFFF) / 65535.0


def mix(a, b, t):
    return sum(int(((a >> s & 255) * (1 - t) + (b >> s & 255) * t)) << s for s in (16, 8, 0))


# The hero, 16 × 16 a frame, facing right, the lantern held out in front.
HEAD = [
    "......kkkk......",
    ".....kHHHhk.....",
    "....kHHHhhhk....",
    "...kkkkkkkkkk...",
    ".....kSSsk......",
    ".....kSSkk......",
]
BODY = [
    "....kCCccck.....",
    "...kCCcccckmk...",
    "...kCcccccmWLk..",
    "...kCcccck.mLk..",
    "....kpppk...k...",
]
LEGS = {
    "stand": ["....kp.pk.......", "....kb.bk.......", "....kk.kk......."],
    "run0": ["...kp...pk......", "..kbk...kbk.....", "..kk.....kk....."],
    "run1": ["....kp.pk.......", "....kbkbk.......", ".....kkk........"],
    "run2": [".....kppk.......", "....kbk.kbk.....", "....kk...kk....."],
    "run3": ["....kpp.k.......", "....kbbk........", "....kkk........."],
    "jump": ["...kp..pk.......", "..kbk..kbk......", "..kk....k......."],
    "fall": ["....kp..pk......", "....kb..kb......", "....kk..kk......"],
}


def hero_frame(legs, bob=0):
    rows = ["." * 16] * (2 - bob) + HEAD + BODY + LEGS[legs]
    rows += ["." * 16] * (16 - len(rows))
    return rows[:16]


FRAMES = [
    ("idle", hero_frame("stand"), 500),
    ("idle", hero_frame("stand", 1), 400),
    ("run", hero_frame("run0"), 90),
    ("run", hero_frame("run1"), 90),
    ("run", hero_frame("run2"), 90),
    ("run", hero_frame("run3"), 90),
    ("jump", hero_frame("jump"), 100),
    ("fall", hero_frame("fall"), 100),
]


def hero():
    sheet = Sheet(16 * len(FRAMES), 16)
    frames, tags = {}, []
    for k, (tag, rows, ms) in enumerate(FRAMES):
        sheet.grid(16 * k, 0, rows)
        frames[f"hero {k}.aseprite"] = {
            "frame": {"x": 16 * k, "y": 0, "w": 16, "h": 16},
            "rotated": False,
            "trimmed": False,
            "spriteSourceSize": {"x": 0, "y": 0, "w": 16, "h": 16},
            "sourceSize": {"w": 16, "h": 16},
            "duration": ms,
        }
        if tags and tags[-1]["name"] == tag:
            tags[-1]["to"] = k
        else:
            tags.append({"name": tag, "from": k, "to": k, "direction": "forward", "color": "#000000ff"})
    sheet.save("hero.png")
    data = {
        "frames": frames,
        "meta": {
            "app": "https://www.aseprite.org/ (written by tools/art.py in the same shape)",
            "version": "1.3",
            "image": "hero.png",
            "format": "RGBA8888",
            "size": {"w": sheet.w, "h": 16},
            "scale": "1",
            "frameTags": tags,
            # where the feet touch the ground, and the light in the lantern
            "slices": [
                {"name": "feet", "color": "#0000ffff", "keys": [{"frame": 0, "bounds": {"x": 4, "y": 15, "w": 6, "h": 1}}]},
                {"name": "light", "color": "#ffff00ff", "keys": [{"frame": 0, "bounds": {"x": 12, "y": 8, "w": 1, "h": 1}}]},
            ],
        },
    }
    with open(os.path.join(ASSETS, "hero.json"), "w") as f:
        json.dump(data, f, indent=1)
        f.write("\n")
    return sheet


def rock(sheet, ox, oy, k, top=False, crystal=False):
    """A 16 × 16 block of rock: three tones in patches, cracks, lit edges; moss on top, or
    crystals growing out of it."""
    base, light, dark, crack = 0x2b3146, 0x3c4560, 0x1f2434, 0x161a26
    for y in range(16):
        for x in range(16):
            n = hash01(x // 2 + k * 9, y // 2, 1) * 0.6 + hash01(x, y, 2) * 0.4
            c = dark if n < 0.3 else (light if n > 0.72 else base)
            if hash01(x, y // 3, 3) > 0.93:
                c = crack
            if y == 0 or x == 0:
                c = mix(c, 0x56607e, 0.5)
            if y == 15 or x == 15:
                c = mix(c, 0x0e1018, 0.5)
            sheet.put(ox + x, oy + y, c)
    if top:
        for x in range(16):
            depth = 2 + int(hash01(x, 0, 4) * 3)
            for y in range(depth):
                c = 0x6fa06a if y == 0 else (0x4f7d55 if y < depth - 1 else 0x355a40)
                sheet.put(ox + x, oy + y, c)
            if hash01(x, 1, 5) > 0.7:
                sheet.put(ox + x, oy + depth, 0x355a40)
    if crystal:
        for (cx, cy, h) in [(4, 13, 7), (9, 14, 10), (12, 12, 5)]:
            for d in range(h):
                for w in range(-1, 2):
                    t = 1 - d / h
                    col = mix(0x2f8fa8, 0xbff6ff, t) if w <= 0 else mix(0x1f5f78, 0x7fd6ea, t)
                    if abs(w) <= 1 - (d > h - 2):
                        sheet.put(ox + cx + w, oy + cy - d, col)


DOOR_TOP = [
    "kkkkkkkkkkkkkkkk",
    "kiIIIIIIIIIIIIik",
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkvvik",
    "kIIIIIIIIIIIIIIk",
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkvvik",
    "kIIIIIIIIIIIIIIk",
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkvvik",
]
DOOR_BOTTOM = [
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkIIik",
    "kivvvwkvvvwkIiik",
    "kivvvwkvvvwkvvik",
    "kIIIIIIIIIIIIIIk",
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkvvik",
    "kIIIIIIIIIIIIIIk",
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkvvik",
    "kivvvwkvvvwkvvik",
    "kkkkkkkkkkkkkkkk",
]


def tiles():
    """The tileset, 16 × 16 a tile: 0 rock, 1 rock with moss, 2 rock with crystals, 3 and 4 the
    door (top, bottom), 5 and 6 the door open."""
    sheet = Sheet(16 * 7, 16)
    rock(sheet, 0, 0, 0)
    rock(sheet, 16, 0, 1, top=True)
    rock(sheet, 32, 0, 2, crystal=True)
    sheet.grid(48, 0, DOOR_TOP)
    sheet.grid(64, 0, DOOR_BOTTOM)
    for k in (5, 6):
        rows = [r.replace("v", "d").replace("w", "d").replace("I", "i") for r in (DOOR_TOP if k == 5 else DOOR_BOTTOM)]
        sheet.grid(16 * k, 0, rows)
    sheet.save("tiles.png")
    return sheet


SPARK = [
    ["........", "...y....", "...Y....", ".yYWYy..", "...Y....", "...y....", "........", "........"],
    ["........", "....y...", "..y.Y.y.", "...YWY..", "..y.Y.y.", "....y...", "........", "........"],
    ["........", "...o....", "..oyo...", ".oyWyo..", "..oyo...", "...o....", "........", "........"],
    ["........", "..y...y.", "...yYy..", "...YWY..", "...yYy..", "..y...y.", "........", "........"],
]


def spark():
    sheet = Sheet(8 * len(SPARK), 8)
    for k, rows in enumerate(SPARK):
        sheet.grid(8 * k, 0, rows)
    sheet.save("spark.png")
    return sheet


def look(path, sheets):
    """Everything on one sheet, six times the size, on the cave's dark."""
    scale, pad = 6, 4
    w = max(s.w for s in sheets) + 2 * pad
    h = sum(s.h + pad for s in sheets) + pad
    out = [0x0b, 0x0d, 0x14, 255] * (w * scale * h * scale)
    y0 = pad
    for s in sheets:
        for y in range(s.h):
            for x in range(s.w):
                i = (y * s.w + x) * 4
                if s.px[i + 3]:
                    for dy in range(scale):
                        for dx in range(scale):
                            j = (((y0 + y) * scale + dy) * w * scale + (pad + x) * scale + dx) * 4
                            out[j:j + 4] = s.px[i:i + 4]
        y0 += s.h + pad
    png(path, w * scale, h * scale, out)


def main():
    os.makedirs(ASSETS, exist_ok=True)
    sheets = [hero(), tiles(), spark()]
    if "--sheet" in sys.argv:
        look(sys.argv[sys.argv.index("--sheet") + 1], sheets)
    print("assets written: hero.png, hero.json, tiles.png, spark.png")


if __name__ == "__main__":
    main()
