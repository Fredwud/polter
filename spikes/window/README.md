# Spike: showing the CPU picture, in a window and full screen

Milestone E2 needs full screen, which minifb (used until then) doesn't offer. Three ways to show Polter's CPU-drawn picture were measured on the reference machine, a MacBook Air 13" (2015, Intel HD 6000, macOS 12, display 1440 × 900), on 19 October 2026, system load 3 to 6. Each ran 4 s in a 1280 × 720 window (a 320 × 180 picture at 4×), then 4 s full screen with the picture grown to fill it at a whole scale (360 × 225 at 4×). Times are per frame, after the first frames settled.

| | Showing a frame, CPU time | Frame interval | Full screen | Clean build | Binary |
|---|---|---|---|---|---|
| winit 0.30.13 + softbuffer 0.4.8 (`softbuffer.rs`) | 11.2 ms in the window, 15.0 ms full screen (wall time the same: it computes, it doesn't wait) | 17.3 to 17.6 ms | yes | 53 s | 1.0 MB |
| minifb 0.29.0 (`minifb.rs`) | 0.52 ms, 0.58 ms stretched over 1440 × 900 (the GPU scales) | 17.4 ms | no API for it | (in use) | 0.95 MB (GIRDER) |
| **miniquad 0.4.11, OpenGL** (`miniquad.rs`) | **0.43 ms in the window, 0.46 ms full screen** (the picture is uploaded as a small texture, the GPU scales it) | **16.67 ms** (vsync) | **yes, real full screen** | **18.6 s** | **0.74 MB** |

**Decision:** miniquad. Softbuffer copies the whole screen-sized buffer on the CPU every frame; on this machine that alone is most of a frame. minifb shows through the GPU but has no full screen, and its frame limiter runs at 57 instead of 60 frames a second. miniquad uploads only the game's small picture, lets the GPU scale it by whole pixels, waits for the display, opens real full screen, and builds fastest.

**Still to know:** miniquad uses OpenGL on a Mac by default (deprecated by Apple, still working on macOS 12); its Metal backend is the fallback if OpenGL goes. It has no gamepads (gilrs adds them) and owns the main loop (the headless path doesn't use it).

To run a spike again: copy the file into a new crate as its main source file with the dependency named in its header (plus `libc`), build with `--release`, start it. It quits by itself after about 9 s and writes its numbers to the file given as its first argument.
