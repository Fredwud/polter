# What Polter still needs

Research and proposals, 19 October 2026, after the owner asked: what does the engine still need, what must it solve cleverly, how does it get everything out of a weak computer (effects nobody would expect from such low tech), what about shaders, a project structure like Godot's, a file browser, and what all of this means for agents. Decisions that follow are in [SPEC.md](SPEC.md); this file keeps the reasoning, the ideas not yet decided and the sources. Estimates are marked as estimates; whatever rests on an experiment not yet made is marked **(spike)**.

Polter's position, in one line: **the picture is computed on the CPU, exactly, and the GPU only presents it.** Everything below keeps that: it is what makes Polter's frames testable byte for byte, its runs reproducible, and its headless checks possible without a GPU.

---

## 1. Low resolution is the strategy, not a limit

A 320 × 180 picture is 57,600 pixels. As 32-bit colour it is 230 KB, which fits in the MacBook Air's 3 MB cache with room to spare; as 8-bit palette indices it is 57.6 KB. Per pixel and frame, at 60 frames a second on two cores, the CPU has roughly 2 × 16.7 ms / 57,600 ≈ 0.58 µs, about a thousand instructions (estimate). Nothing in a 1080p engine gets that budget. So Polter's target is logical resolutions up to about 640 × 360, scaled by whole pixels, and its effects are designed for that budget.

## 2. Tricks: effects nobody expects from such low tech

Ordered by cost.

### 2.1 Palettes and look-up tables: a lookup per pixel

- **Colour cycling.** Pixels stand still while their colours rotate through a range of the palette: water flows, fire flickers, rain falls, neon runs across the whole screen without drawing a single pixel again. Mark Ferrari built entire living landscapes this way from one flat 256-colour image, and smoothed it with "BlendShift" cycling, which fades between the shifting colours. In Polter: **indexed layers** (8-bit pixels plus a palette with cycle ranges, blend or step), composited into the canvas through the palette table. Cost: one lookup per pixel, the animation itself free.
- **Light from a colour map.** Every palette colour gets a ramp of light levels in one table (the trick behind Doom's light and fog). A light map at a quarter of the resolution says how bright each spot is; the table turns colour and light into the lit colour. Darkness, fog, underwater tint, torchlight: one lookup.
- **The print as an engine feature.** SHARDFALL already turns scene light (with glows and a tone curve) into a palette through a 3D table with Bayer dithering, a lookup per pixel. It moves into the engine (E5) as `print`, usable by any game: the dither hides that light was computed at low resolution.
- **Colour-blind modes for free.** The palette table can be swapped for one that keeps colours apart for deuteranopia or protanopia. A setting, not a second set of art.

### 2.2 Per-row effects (as the SNES did with HDMA)

The SNES changed scroll offsets, colours and even its Mode 7 transform for every scanline; whole genres (racing floors, wavy water, parallax skies) came from tables of per-line values. Polter gets the same as **row effects**: a table of per-row parameters the game fills each frame.

- heat haze, underwater wobble, a waving flag: a horizontal offset per row from a sine table;
- parallax: each band of rows scrolls at its own speed, copied with `memmove` speed;
- the water line: a different palette table below the surface;
- **Mode 7**: a perspective floor (racing, a map seen from above, flying), an affine sample per row. At 320 × 180 that is 57,600 texture samples a frame, which a CPU barely notices (estimate).

The tables are data, so an agent sees them in `probe` and can test them.

### 2.3 Pseudo-3D

- **Sprite stacking**: an object as horizontal 2D slices drawn one pixel apart; turning the slices turns the object. Cost grows with slices and objects; caching a few rotations keeps it cheap.
- **Ray casting** (Wolfenstein) and **voxel heightmaps** (Comanche's "Voxel Space"): one column at a time, 320 columns. Both are classic low-end tricks that fit the budget (estimate).
- **Normal-mapped sprites**: pixel art with a second image of normals, lit by 2D lights. Dynamic lighting on hand-drawn sprites, at the light map's resolution.

### 2.4 Light and shadow at a quarter of the resolution

- A **light map at 80 × 45 to 160 × 90**, shadows by marching against a distance field of the walls (built by jump flooding), soft shadows from the same field; upsampled by the print, whose dither hides the coarse grid.
- **2D global illumination with radiance cascades** (Sannikov). Holographic radiance cascades report 1.85 ms for 512 × 512 on an RTX 3080 laptop GPU. The MacBook Air's Intel HD 6000 has very roughly 1/45 of that compute (estimate, from published peak throughput); at 160 × 90, a sixteenth of those pixels, that suggests about 5 ms on its GPU (estimate). On the CPU it is an open question. **(spike)** If it holds, light that bounces off coloured walls in a pixel game on a 2015 laptop is Polter's showcase.
- **Amortised over frames**: half of the light map per frame in a checkerboard, light from things that don't move kept until they do.

### 2.5 Work that isn't done

- **Pictures nobody sees aren't painted** (`ctx.picture`, since E1).
- **Static layers once, then copied**; the picture in 32 × 32 tiles, only changed tiles painted (dirty tiles); scrolling as a copy.
- **Two cores, deterministically**: rows of the picture split in two halves, each painted by its own thread; the cells of a falling-sand world updated in a checkerboard of chunks (as Noita does), so two threads never touch neighbours.
- **The simulation always at 60, the picture at 30 if it must**: on a machine too slow for the picture, every second frame is painted and the game still plays exactly.
- **Run-ahead against input lag** (as emulators do it): Polter is deterministic and keeps snapshots, so it can simulate a frame or two ahead with the input held and show that, taking back the delay a display adds. Worth a spike, not a promise. **(spike)**

### 2.6 Dithering as a feature

Transparency as a screen-door pattern costs no blending; Bayer patterns are fixed in time and don't shimmer. Blue noise looks smoother, but changed every frame it shimmers unless it is designed in time as well (spatiotemporal masks, EA SEED 2022). Polter keeps Bayer as its default and offers blue noise only where nothing moves.

## 3. Shaders

**Two kinds, each where it belongs.**

1. **Pixel programs on the CPU**: functions over pixels, rows or tiles of the canvas (a lookup, a row effect, a small blur on the light map), split across both cores. They are exact, so they belong to the picture that is hashed and tested.
2. **Presentation shaders on the GPU**: one pass while the picture is scaled to the screen: a CRT mask and scanlines, a soft bloom, colour grading through a 3D table, a smoother scaler for those who want it. The HD 6000 has nothing else to do; one full-screen pass costs the CPU nothing.

The catch: an agent can't see what the GPU did, and GPU results differ between drivers. So **every presentation shader has a CPU twin**: the same effect written in Rust. `peek --present` shows what a player sees through the twin, and a **twin test** reads the GPU's result back once and compares it with the twin within a small tolerance. A shader without a twin is allowed but reported as unverified. No other engine I know of makes its shaders checkable by an agent; nothing I read showed one. **(spike: reading back the frame through miniquad)**

Why not draw everything on the GPU: GPU output depends on the driver, headless runs (CI, agents) have no GPU, and the HD 6000's fill rate is the weakest part of the machine.

## 4. A project, like Godot's, but for agents

Godot gives a project a fixed shape: `project.godot`, scenes and resources as text, an import cache, `res://` paths. Polter's equivalent:

```
mygame/
  polter.toml     the game's name, resolution, fit, print and palette, window, icon, version
  BRIEF.md        one page: what the game is, its verbs, its mood (for people and agents)
  Cargo.toml      a crate that depends on polter
  src/            the game (Rust)
  assets/         sources people edit: .png, .aseprite, .ldtk, .wav, .ogg, fonts
  levels/         levels as text, or LDtk files
  scenarios/      .scenario files
  haunts/         recorded play
  golden/         frame hashes
  tests/          bots, feel tests
```

- **`polter new NAME`** writes this shape with a running game in it: an agent never wonders where things go.
- **Assets are named in code and checked when it builds** (`asset!("hero.aseprite")`): a missing or misspelt file is a compiler error with the path, not a blank sprite at run time.
- **Importing**: Aseprite files are read directly (the format is documented: little-endian chunks, cels compressed with zlib), PNG sheets, LDtk levels (plain JSON, from the maker of Dead Cells), WAV and OGG. Imported once into a cache keyed by content; a changed file is imported again and swapped in while the game runs.
- **An asset change is recorded like a knob change**: the workshop's recording notes which version of which file was in use from which tick, so a replay shows exactly what was seen.
- **No scene tree, no inspector**: code and text are what agents write well and what diffs show. The workshop (knobs, state, marks) is where people adjust and point; levels come from LDtk or text.
- **Dependencies**: reading PNG and Aseprite needs inflate. Either a small own implementation or `miniz_oxide` (pure Rust); OGG needs a decoder (`lewton`). Each is measured (build time, binary size) before it comes in, as SPEC 6.6 asks.

## 5. A file browser

In the workshop, a **drawer** beside the deck, by kind: sprites (animated), sounds (playing, with crunch's verdict), levels (as thumbnails), scenarios (passing or failing), marks, takes and earlier sessions. Every entry does something, nothing only decorates: preview it, open it in its editor (Aseprite, LDtk), reload it, load a take or a session onto the deck (takes are kept since today, but can't be played yet).

For agents the same as text: **`polter files --json`**: every asset with kind, size, content hash, import state and **where the code uses it**, so unused and missing files show up.

## 6. What the engine must solve cleverly

1. **Determinism across machines** (built 19 October 2026: `polter::math` and the *clippy.toml*, SPEC 6.3). Basic float operations are the same everywhere (IEEE 754), but `sin`, `exp`, `powf` and their kin come from the platform's maths library and differ in the last bit between macOS, Linux, Windows and the web. Polter therefore needs its own **deterministic maths** (polynomials from basic operations, as `alice-det-math` and Rapier's cross-platform mode do), and its **project template forbids the platform's functions in game code with a *clippy.toml*** (`disallowed-methods`: `f32::sin`, `Instant::now`, `HashMap::new` with a random hasher), so an agent's slip is a warning when it builds, not a broken golden frame on another machine. Only then can CI on three systems compare the same frame hashes (E7).
2. **The CPU picture on two cores**, deterministic: rows or tiles per thread, never a shared accumulation whose order changes.
3. **Hot reload of everything**: code (since today: rebuild, the workshop reloads at the same tick), assets (watched, imported, swapped, recorded), data.
4. **Text**: bitmap fonts with German umlauts and more, kerning, layout for menus; the current 3 × 5 font has capitals only.
5. **Input**: actions with bindings, gamepads (`gilrs`), and as little delay as possible (input read at the last moment, run-ahead as an option).
6. **Saves that survive new versions**: saves as text with a version and migrations; old saves replayed in a test.
7. **The web**: miniquad builds for WebAssembly and WebGL 1, so a Polter game could run in a browser on a Chromebook. It is the cheapest computer most people have. **(spike: sound in the browser)**

## 7. Through an agent's eyes

Polter has what most engines lack: rules checked every tick, scenarios, golden frames, recorded play, `probe`, `peek`, `twin`, `listen`, and the workshop's shared folder. Godot and Unity reach agents through MCP servers bolted on from outside: dozens to hundreds of editor operations, and lately freezing the clock, stepping time, injecting input and digests of state as JSON instead of screenshots (Open Godot MCP, July 2026). Polter has determinism, recordings and exact rewinding by construction. What to add, by value per effort:

1. **`polter brief`** (built 19 October 2026, with the engine's `llms.txt`): one generated page per project in the shape of llms.txt (a title, a summary in a quote, sections, lists of files): BRIEF.md, state keys, rules, knobs, scenarios and whether they pass, assets, open marks. The engine itself gets an `llms.txt` too. A fresh agent reads one page and knows the game.
2. **Shrinking**: when a run breaks a rule, cut its recording down to the shortest one that still breaks it (as property-based testing shrinks a failing case). A 4,000-tick session that breaks becomes a 40-tick repro. I know of no game engine that does this.
3. **Visual diffs**: `peek --diff` between two builds or two seeds: a heat map of changed pixels and their boxes as JSON. "What did my change do to the picture" in one image.
4. **Checks for people agents can't be**: **flashing** (WCAG's rule of no more than three flashes a second over a large area, measured on every frame, the way broadcasters screen for photosensitive epilepsy); **colour blindness** (contact sheets through the colour-blind tables); **contrast** of every text drawn against what is behind it. Agents can't see or feel; numbers make it checkable.
5. **A feel report**: ticks from a press to a reaction, coyote time and jump buffer present or not, hitstop and shake per event, the camera's jerk. My own curriculum's "game feel without eyes", as a command.
6. **The determinism guard** of 6.1 in every new project.
7. **Bisect**: `polter bisect FILE.scenario` finds the commit that broke a scenario (git bisect with `assert`).
8. **Profiling scopes**: `ctx.scope("lights")` in the game, `check --perf --json` reports milliseconds per scope against the budget.
9. **The headless workshop**: an agent plays and tunes through the workshop's commands without a window; its sessions land as takes the person can play back on the deck.
10. **Image economy**: compact text by default, JSON on request, pictures only where needed: contact sheets, crops at whole zoom (`peek --crop x,y,w,h --zoom 4`, today a separate script of mine), diff heat maps.

(Already planned in SPEC 5.6, not repeated here: `trace`, the mutation check, budgets as tests, hidden scenarios, `monkey`, `bot`/`solve`, MCP.)

## 8. Animation

The owner, the same evening: "Was auch noch nicht *unbedingt* beachtet wurde sind Animationen, richtig?" Right. Today a `Sprite` is one still picture from rows of codes; GIRDER animates by switching pictures by hand. What a game needs:

1. **Frames from Aseprite, with everything the artist put in.** Its file keeps each frame's duration, **tags** (a range of frames with a direction, forward, reverse, ping-pong or ping-pong reverse, and a repeat count, 0 for forever), **slices** (named rectangles, with a pivot: hitboxes, the point a weapon hangs from, the feet) and **user data** on frames and cels (text or properties: "step" on frame 3 for a footstep sound). Polter imports all of it, so the artist decides timing and boxes, not code.
2. **A player that counts ticks, not milliseconds.** Aseprite's milliseconds become ticks at 60 a second, the rounding carried from frame to frame so a loop keeps its total length exactly; deterministic, so `twin`, golden frames and the workshop's rewinding hold. `anim.play("run")`, a speed, a choice between switching at once and finishing the loop first, and the events of the frame just reached (`step`, `hit`), which a game turns into sounds (crunch) or damage.
3. **Tweens and easing** for whatever moves without frames: menus, the camera, juice. Easing curves from basic operations (deterministic), tweens in ticks, sequences ("this, then that, wait, these together"), hitstop, screen shake by trauma.
4. **Springs and chains** for what follows: an antenna, hair, a rope, a tail; damped springs and short Verlet chains, cheap at this resolution.
5. **Turning pixel art without mush.** Rotating by anything but quarter turns blurs or breaks pixel art. **RotSprite** (Xenowhirl's algorithm, built into Aseprite) keeps the pixel patterns; it is too slow for every frame, so Polter renders the angles a sprite needs once, when it is imported (16 directions, say), and plays them from that store. Flips cost nothing; scaling only by whole factors.
6. **Animation through the palette** (2.1): a hit flash, a blink, a glow as a swap of the palette table: no frames, no cost.

**Through an agent's eyes**, animation is the part agents get most wrong: they can't watch motion. So:

- `peek --anim FILE TAG`: a sheet of the tag's frames with their durations and slice boxes drawn in, and a strip at real timing (each frame as often as it lasts), so timing becomes a picture.
- Checks when a game builds and when it is imported: every tag and slice the code names exists (`anim!("hero.aseprite", "run")`, a compiler error otherwise); a looping tag's last frame leads back to its first without a jump; no frame lasts zero ticks; a hitbox doesn't jump further between two frames than the body does (a misplaced frame).
- **Foot sliding**: with a *feet* slice in the run cycle, Polter measures how far a planted foot moves over the ground while the character runs, in pixels per frame. Guides describe sliding as running speed that doesn't match the cycle; I found no tool that measures it for 2D sprites. Polter can, because it knows both the speed and the foot.
- **Frame data**, as fighting games publish it: from an attack's tag and its hitbox slices, the frames of startup, active and recovery. Numbers an agent can balance by.
- The current tag and frame are in `state()`, so `probe` and `trace` show them; the workshop's drawer plays an animation with a speed knob, and a mark records which frame was on screen.

## 9. Order

| When | What |
|---|---|
| now | split runtime and tools (SPEC 6.6); deterministic maths and the *clippy.toml* guard; `polter new`, *polter.toml*, *BRIEF.md*, `brief` |
| E3 | indexed layers, palette tables, colour cycling, row effects and Mode 7; sprites from PNG and Aseprite, **animation** (section 8: frames, tags, slices and events, a player in ticks, tweens, springs, RotSprite, the checks), tiles and LDtk, camera, fonts, a small immediate UI (from the workshop's painter); actions, bindings, gamepads; presentation shaders with CPU twins; the drawer and `files --json`. Accepted by a fresh agent with one page of brief |
| E4 | mixer, music, crunch's checks in `listen` |
| E5 | the print, light maps, distance-field shadows; radiance cascades if the spike holds |
| E6 | shrinking, visual diffs, flashing, colour-blind and contrast checks, the feel report, bisect, profiling scopes, MCP |
| E7 | Windows, Linux, the web; CI comparing the same frame hashes on three systems |

**Spikes before building:** radiance cascades at 160 × 90 on the CPU and on the HD 6000; Mode 7 and row effects at 320 × 180 (expected well under a millisecond); rows on two threads (expected near twice as fast); inflate, own or `miniz_oxide`; reading the GPU's frame back through miniquad; run-ahead; sound in the browser.

## Sources

Read (the page itself, 19 October 2026):

- llms.txt v2, Jeremy Howard: [llmstxt.org](https://llmstxt.org/)
- Open Godot MCP announcement, July 2026: [forum.godotengine.org](https://forum.godotengine.org/t/open-godot-mcp-ai-plays-tests-your-game-not-just-edits-files/141986)
- Freeman, Sannikov, Margel, *Holographic Radiance Cascades for 2D Global Illumination* (abstract only; the PDF was too large to fetch): [arXiv 2505.02041](https://arxiv.org/abs/2505.02041)

Found (search summaries, not the pages themselves; worth reading before building on them):

- Colour cycling and Ferrari's BlendShift: [Wikipedia: Color cycling](https://en.wikipedia.org/wiki/Color_cycling), [80.lv on Living Worlds](https://80.lv/articles/color-cycling-from-the-90s-returns-with-html5)
- HDMA and Mode 7: [SNESdev wiki: Mode 7 transform](https://snes.nesdev.org/wiki/Mode_7_transform), [sneslab: HDMA](https://sneslab.net/wiki/HDMA)
- Sprite stacking: [Wee Boats devlog](https://jamespoole.itch.io/wee-boats/devlog/685630/using-sprite-stacking-for-pseudo-3d-in-wee-boats)
- Radiance cascades in a thesis: [Chalmers, 2025](https://odr.chalmers.se/items/3ee9fb4e-1880-46c2-802a-a660e38dc9ee); a Rust implementation: [solis_2d](https://lib.rs/crates/solis_2d)
- Deterministic maths: [alice-det-math](https://docs.rs/alice-det-math), [Rapier: determinism](https://rapier.rs/docs/user_guides/rust/determinism/)
- Blue noise in time: [EA SEED, EGSR 2022](https://www.ea.com/seed/news/egsr-2022-blue-noise)
- Aseprite's file format: [ase-file-specs.md](https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md); LDtk's JSON: [ldtk.io/json](https://ldtk.io/json)
- Flashing: [Harding test](https://en.wikipedia.org/wiki/Harding_test), [W3C technique G15](https://www.w3.org/WAI/WCAG21/Techniques/general/G15)
- MCP servers for engines: [Coding-Solo/godot-mcp](https://github.com/Coding-Solo/godot-mcp), [a 2026 overview](https://codex.danielvaughan.com/2026/04/27/codex-cli-game-development-unity-godot-mcp-agent-driven-workflows/)
- Aseprite's tags and user data: [aseprite.org: tags](https://aseprite.org/docs/tags/), [the format's notes](https://mintlify.com/aseprite/aseprite/development/ase-file-format) (the slice chunk not confirmed there: read the official spec before importing it)
- RotSprite: [Sonic Retro wiki](https://info.sonicretro.org/RotSprite), the [rotsprite crate](https://docs.rs/rotsprite/); CleanEdge as an alternative: [CleanRotate](https://astropulse.itch.io/clean-rotate-for-aseprite)
- Foot sliding as a speed mismatch: [bugnet.io](https://bugnet.io/blog/how-to-fix-animation-foot-sliding), [MonoAGS: walking](https://github.com/tzachshabtay/MonoAGS/blob/master/Docs/articles/walking.md/), [UCLA Games: walk cycles](https://games.ucla.edu/?p=985)

From memory, not checked here: Doom's colour maps, Wolfenstein's ray casting, Comanche's Voxel Space, Noita's checkerboard chunk updates, emulators' run-ahead, normal-mapped sprites in Sprite Lamp and Dead Cells, miniquad's WebAssembly target, clippy's `disallowed-methods`, the HD 6000's and the RTX 3080 laptop's peak throughput. Each is well known; each gets a source before code is built on it.
