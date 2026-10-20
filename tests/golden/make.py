#!/usr/bin/env python3
"""Golden frames: every frame a console cart draws, as a hash of its RGB pixels (FNV-1a, 64 bit).

    python3 tests/golden/make.py write    renders the runs below with the carts' `peek --all`
                                          and writes tests/golden/<run>.txt
    python3 tests/golden/make.py check [RUN...]   renders them again and compares, frame by frame

Made before milestone E1 changed the core, so that every console cart can be shown to draw
exactly the same pixels afterwards; LANTERN's run since 20 October 2026. A run whose cart isn't in
this folder is skipped and said so.
"""

import importlib.util
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))

# the PNG reader of tests/png (Python's zlib; no library needed)
_spec = importlib.util.spec_from_file_location("png_reader", os.path.join(ROOT, "tests", "png", "make.py"))
_png = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_png)


def load(path):
    """Width, height and the rows of RGB bytes of a PNG file. Pictures as Polter writes them (8
    bits, not interlaced) take a quick way; any other goes through the full reader."""
    import struct
    import zlib
    data = open(path, "rb").read()
    pos, header, palette, idat = 8, None, b"", b""
    while pos < len(data):
        n = struct.unpack(">I", data[pos:pos + 4])[0]
        kind, body = data[pos + 4:pos + 8], data[pos + 8:pos + 8 + n]
        pos += 12 + n
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", body)
        elif kind == b"PLTE":
            palette = body
        elif kind == b"IDAT":
            idat += body
    w, h, depth, ctype, _, _, interlace = header
    channels = {2: 3, 3: 1, 6: 4}.get(ctype)
    if depth != 8 or interlace or channels is None:
        w, h, rgba = _png.decode(path)
        rgb = bytes(b for i, b in enumerate(rgba) if i % 4 != 3)
        return w, h, [rgb[y * w * 3:(y + 1) * w * 3] for y in range(h)]
    raw = zlib.decompress(idat)
    stride = w * channels
    colours = [palette[i * 3:i * 3 + 3] for i in range(len(palette) // 3)]
    rows, prev = [], bytearray(stride)
    for y in range(h):
        f = raw[y * (stride + 1)]
        line = bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        if f:
            for i in range(stride):
                a = line[i - channels] if i >= channels else 0
                b = prev[i]
                c = prev[i - channels] if i >= channels else 0
                line[i] = (line[i] + [0, a, b, (a + b) // 2, _png.paeth(a, b, c)][f]) & 255
        prev = line
        if ctype == 3:
            rows.append(b"".join(colours[i] for i in line))
        elif ctype == 6:
            rows.append(bytes(v for k, v in enumerate(line) if k % 4 != 3))
        else:
            rows.append(bytes(line))
    return w, h, rows

# name: (cart binary, frames, extra arguments)
RUNS = {
    "testcard": ("testcard", 60, []),
    "lantern_bot_run": ("lantern", 576, ["--haunt", "carts/lantern/haunts/bot_run.haunt"]),
}


def fnv(data):
    h = 0xCBF29CE484222325
    for b in data:
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def render(name):
    binary, frames, extra = RUNS[name]
    with tempfile.TemporaryDirectory() as tmp:
        subprocess.run([os.path.join(ROOT, "target", "release", binary), "peek", "--all", "--scale", "1",
                        "--frames", str(frames), "--dir", tmp] + extra,
                       cwd=ROOT, check=True, capture_output=True)
        hashes = []
        for f in range(1, frames + 1):
            w, h, rows = load(os.path.join(tmp, f"frame_{f:04}.png"))
            hashes.append(f"{f} {w}x{h} {fnv(b''.join(rows)):016x}")
        return hashes


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "check"
    names = sys.argv[2:] or list(RUNS)
    bad = 0
    for name in names:
        cart = RUNS[name][0]
        if not os.path.isdir(os.path.join(ROOT, "carts", cart)):
            print(f"– {name}: skipped (no carts/{cart} here)")
            continue
        got = render(name)
        path = os.path.join(HERE, f"{name}.txt")
        if mode == "write":
            with open(path, "w") as f:
                f.write("\n".join(got) + "\n")
            print(f"{name}: {len(got)} frames written")
            continue
        want = open(path).read().split("\n")[:-1]
        diff = [(w, g) for w, g in zip(want, got) if w != g]
        if diff or len(want) != len(got):
            bad += 1
            first = diff[0] if diff else (f"{len(want)} frames", f"{len(got)} frames")
            print(f"✗ {name}: {len(diff)} frames differ; first: want {first[0]!r}, got {first[1]!r}")
        else:
            print(f"✓ {name}: {len(got)} frames identical")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
