# Polter – specification

> *Poltern: to rumble, the way a poltergeist makes a house rumble. Something unseen makes things happen, and here the unseen ones are agents.*

Status: **0.5, 20 October 2026: the first public preview is being made ready** (see "The first release?" below). E0 and E1 done; E2 built and waiting for its proof with SHARDFALL; from E3 the core is there (pictures from PNG and Aseprite, animation, tiles, camera and collision, actions and gamepads, text), the rest of E3 is not (indexed layers, colour cycling, row effects, shaders, the drawer). Polter was called EMBER-8 until 19 October 2026. Whatever a later milestone needs is marked **(E…)** and is not built before.

---

## 1. Pitch

**An open-source 2D game engine that AI agents can drive end to end, made to run smoothly on every computer, old ones included.** Games made with it are free to sell; the engine itself stays open. An agent writes the game, runs it without a window, asks it what is true at every tick, looks at exact pixels, replays a human's session frame by frame, and proves that a level can be finished. Humans get a real window, menus and a double-clickable app.

The verb is *check*: every claim about a game can be checked by a command.

---

## 2. Why another engine

What exists in October 2026 (sources in section 12):

- **The big engines** (Godot, Unity, Unreal, Bevy, Phaser, three.js and others) score at most 49 of 100 for "agent-ready" in an independent ranking. Headless tests reach at most 5 of 10, and diffable project formats and machine-readable API specs are mostly missing. The ranking has no criteria at all for determinism, replay, screenshots, state queries or weak hardware. Agents reach Godot through community MCP servers that drive its editor.
- **Hayao**, "the 2D game engine for AI agents" (TypeScript, MIT, version 0.4), already does much right. The simulation is deterministic and runs headless in Node, with state probes, hashes, input replay, a solver for puzzles, an MCP sidecar and a Claude Code plugin. But it is vector graphics in a browser, has no native build and no GPU path, and by its own words is not for "thousands of sprites at 60 fps with per-pixel effects". It is one author's project with three stars.
- **What agents fail at** (two benchmarks, both on Godot): 74 % of failures are rules that break *during* a run, not at the end. The hardest are decisions that must hold over several frames, spatial reasoning, and timing. Checking only the final state let wrong solutions pass on 34 of 36 tasks. Tick-level assertions with hidden scenarios caught them. Rendered pictures helped with framing, unreadable UI and broken layouts; running more commands without that loop did not help.

**The gap:** a native engine for real-time 2D games, pixel-exact, smooth on every computer down to old ones, where **every pixel and every tick can be checked**. It is meant for action, simulation and pixel art, not only turn-based logic.

---

## 3. Pillars, in order

1. **Agent-first.** Everything a developer does is a command with text or image output and a clear exit code. No task needs a window, an editor or a mouse.
2. **Truth is checkable.** The simulation is deterministic: same build, same seed and same inputs give the same state and the same pixels. Every tick can be asserted on, every frame compared bit for bit, every session replayed.
3. **Smooth everywhere, measured on the slowest.** A 2015 MacBook Air is the floor, not a "minimum spec" that barely runs. Frame, memory, start-up and build budgets are tests.
4. **Humans second, never second-class.** A real window, a pause menu, rebindable keys, an app to double-click, sessions that record themselves.
5. **Small enough to hold in mind.** The whole API fits in one document an agent reads in full each session. There is one way to do each thing, and errors say how to fix them.
6. **Tools never weigh on games.** Whatever helps make a game (the workshop, importers, generators, checks) lives outside the game that ships, and every addition is measured against the budgets before it stays (6.6, 7). Fast and snappy is a feature.
7. **Open, and staying open.** Anyone may make and sell games with it. Whoever changes the engine itself and passes it on must publish the changes under the same license, so nobody can turn it into a closed product of their own (The owner, 19 October 2026; the license in section 11). No telemetry, no network, no accounts.

---

## 4. Scope of 1.0

**Low resolution by design** (decided 19 October 2026): logical pictures up to about 640 × 360, computed on the CPU and scaled by whole pixels, the GPU presenting. Every effect is designed for that budget: at 320 × 180 the two cores have about a thousand instructions per pixel and frame, and the whole picture fits in the cache. The tricks that make it look like more than it is (palette tables, colour cycling, row effects, Mode 7, light at a quarter of the resolution) are in [RESEARCH.md](RESEARCH.md).


**Games it is for:** 2D at the resolution each game chooses, pixel art first (a low resolution scaled by whole pixels), in a window or full screen: platformers, top-down action, shooters, puzzles, roguelikes, falling-sand and cell simulations, strategy, management and card games, with up to a few thousand moving things.

**With it:** the workshop (5.5), where a person and an agent make a game together.

**Not in 1.0:** 3D, networking and multiplayer, phones, consoles, a construction editor (scene trees to click together, asset editors: agents write the code and the data, and the workshop is where people judge, tune and point), a general physics engine (collision is boxes, tiles and cells), a scripting language. Each may come later. Each would be its own milestone, needed by a real game.

**The console is a profile, not the frame.** Polter grew out of a fantasy console (320 × 180, eight colours, six buttons). Its constraints help agents and make a look, so the console stays as a profile for carts. The engine itself is not a console in a window: a game chooses its size, its colours and its actions, and the window follows the screen.

---

## 5. The agent interface

### 5.1 One binary, one vocabulary

Every game is a single binary and gets every command for free (grown out of EMBER-8's command line, which already has most of them):

| Command | What it answers |
|---|---|
| `play [--record FILE]` | the window; records the session as it is played |
| `check` | runs headless: panics with file and line, broken invariants, missing glyphs, sounds played |
| `probe --every N` | the game's state as JSON lines, from a schema the game declares |
| `assert SCENARIO` | starts in a saved state, plays inputs, checks conditions at every tick; hidden scenarios work the same |
| `peek`, `shot`, `sheet` | frames as PNG, contact sheets, crops scaled by whole pixels |
| `twin` | two runs, state hash and screen hash compared every tick |
| `replay FILE` | a recorded session, frame for frame, headless or in the window |
| `listen` | the sound as WAV, with waveform and spectrogram |
| `perf` | milliseconds per stage against the budget, averaged |
| `bot`, `solve` | a scripted or searching player proves a level can be finished, through the player's own inputs |
| `looks` | the same frame through several looks or settings, on one sheet |
| `api` | the engine's and the game's API as JSON (for tools and agents) |

All output is text: JSON with `--json`, PNG and WAV for pictures and sound. Exit codes: 0 clean, 1 the game is wrong, 2 the tool failed, 3 the call was wrong.

### 5.2 Checks that catch what agents get wrong

- **Invariants**, declared by the game (`money never negative`, `the player is never inside rock`, `a cooldown never fires early`), are checked on every tick in `check`, `assert`, `bot` and `twin`. A broken one stops the run and names the tick, the invariant and the state.
- **Scenarios** are text files: a start state (or a seed plus inputs to reach it), an input script, and conditions at ticks. Agents write them, and humans' recorded sessions become them.
- **Golden pictures:** because rendering is on the CPU, a frame is the same bytes in every run, so a picture can be a test.
- **Mutation check (E6):** the engine can break a game's rule on purpose and confirm that its tests notice (L-010 as a feature).

### 5.3 Documents an agent needs

An API document (the whole API, small enough to read in full), `llms.txt`, an agents' guide with the working loop, and the machine-readable API from `api`. Every error message names the fix ("`Sprite::new` expects rows of equal length; row 3 has 7 cells, the others 9").

### 5.4 MCP (E6)

A thin MCP server that wraps the command line, for agents that prefer tools to a shell. The command line stays the source of truth: anything the server can do, a shell can do.

### 5.5 The workshop (E2)

A window where a person plays, tests and tunes a game while an agent works on it. Not a construction kit: a workbench where judgment (the person's) meets measurement and code (the agent's).

```
┌──────────── the game, live ────────────────┐┌ GIRDER ─────── WORKSHOP ┐
│                                            ││ KNOBS                   │
│                                            ││ JUMP_SPEED ──▮──── 4.4  │
│                                            ││ GRAVITY    ─▮───── 0.26 │
│                                            ││ ● RULES HOLD            │
└────────────────────────────────────────────┘│ X 25.1   BOLTS 4  ▒▒▒▒▒ │
════════╤═══════════════⚑═══════════════▮═════════════════════════════════
[cassette] [⏮][◀◀][▶][⏸][▶▶][▶|][●][⚑]   [▶ 0:05.0 TICK 300/300] (LEVEL)(LOAD)
```

Built as a tape deck (The owner's idea, 19 October 2026): the tape is the timeline, the keys latch as a deck's do, and every key and display has a function. The panels follow the content and the window: knobs and state share the panel by what each holds and scroll what does not fit; the panel grows with the window and folds away; the deck keeps the tape and the keys always and drops the rest by worth (cassette, LEVEL, LOAD, display). Built so far: recording, rewinding (with copies of the game along the tape), knobs, marks, takes, the agent's files and commands, reloading after a rebuild (the workshop watches its own program, then starts it again in the same process and replays the session to the tick it was at). Next: proposals, scenarios from the workshop.

- **Play and rewind.** Every session is recorded as it is played. The simulation is deterministic, so the timeline goes back to any frame.
- **Knobs.** A game declares its tuning values (jump height, gravity, colours); the person drags them live; they are saved as data the agent reads and sets.
- **Marks.** M marks a moment: frame, state and picture. "Look at mark 3, the jump is late there" points the agent at exactly that.
- **Code changes without starting over.** When the agent changes the code, the workshop rebuilds and replays the person's recording to the frame they were at, now with the new code: what the change does to *their* scene, in about a second.
- **Proposals.** The agent can show a change as a variant: old and new side by side, on the same recording. The person keeps one.
- **The agent drives it too.** Jump to a frame, set a knob, start a scenario, show the state or hitboxes: every action of the workshop is also a file and a command. The agent is never second-class there.
- **Honest about time.** An agent works in turns and doesn't see the window. The workshop writes marks and events to files; an agent watching them answers within seconds. The two work at the same time, not in the same instant.

### 5.6 What only an engine for agents does

What the benchmarks and BNDR's own work show agents need, and no engine of October 2026 gives them together:

| Need | Polter |
|---|---|
| Rules break *during* a run (74 % of agents' failures in GameLogicBench) | invariants on every tick; scenarios with expectations per frame (E1 ✓) |
| Pictures that can be trusted as tests | CPU rendering: the same bytes with and without a window; golden frames (E1 ✓) |
| Agents can't watch motion or timing | contact sheets, `probe` over time, **`trace`**: a timeline of events per frame to ask "what touched `player.y` between frames 200 and 260" (E6) |
| Agents forget between sessions | **`brief`** (built 19 October 2026): what the game is, its scenarios and invariants and whether they pass, marks from the person, time per frame against the budget, recent changes: a fresh agent's first read, with the engine's `llms.txt` |
| Tests that test nothing | the **mutation check** breaks a rule on purpose and confirms a test notices (E6) |
| A change that quietly makes things slower (F-045) | budgets as tests: frame time, memory, start, binary size, against the last good build (E6) |
| Style is taste, not a number | **variant sheets**: the same frame through several settings, side by side (as SHARDFALL's `looks`; E3) |
| The person's feel is the only judge of fun | the workshop: marks, knobs, proposals on the person's own recording (E2) |
| Hidden acceptance | scenarios the agent never sees, kept by the person or a judge, run as a gate (E6) |
| Random play finds what scripts don't | **`monkey`**: seeded random input for thousands of frames, with every invariant on (E6) |
| A failing run is too long to read | **shrinking**: the recording cut to the shortest one that still breaks the rule (E6) |
| "What did my change do to the picture?" | **visual diffs**: a heat map of changed pixels between two builds or seeds (E6) |
| Agents can't see flashing, colours or contrast as people do | checks for **flashing** (no more than three flashes a second over a large area), **colour blindness** and **text contrast**, on every frame (E6) |
| Agents can't feel a game | a **feel report**: ticks from press to reaction, coyote time, buffers, hitstop, shake, camera jerk (E6) |
| Agents need to know where things go | **`polter new`** with a fixed project shape, *polter.toml*, *BRIEF.md*, assets checked when the game builds, **`files --json`** (now and E3) |

### 5.7 Art and sound (E3, E4)

Agents are weakest at art and presentation (GameCraft-Bench: the lowest scores, and "raw programmatic shapes" are marked down). Polter answers that differently for each side:

- **For the person: no editors of its own, but first-class import.** A pixel editor or a sound studio of Polter's own would cost years and stay worse than the tools people already use. So Polter reads their files and reloads them live in the workshop: Aseprite files (its format is documented) and PNG sheets, WAV and OGG. In the workshop the person tunes what belongs to the game: a palette, a look, a sound's knobs, heard at once.
- **For the agent: art and sound as data, with tools that make it good.** An agent can't hold a pen, but it can describe precisely and measure. Polter gives it:
  - *formats it writes well*: sprites as rows of letters with a legend, palettes as ramps in OkLCH with hue shift, sound effects as synth patches, music as text (a tracker or MML-like notation);
  - *generators with knobs*: ramps and palettes, outlines and shading from a height or normal map, dithering, tiles that join by rule, noise textures; sound patches and their variations;
  - *checks that judge what can be measured*: the greyscale test (do the values read?), contrast between player and background, palette conformance, silhouettes; loudness, clipping and pitch of every sound;
  - *variant sheets for the person's taste*: eight versions of a sprite, a palette or a sound on one sheet (sounds as one WAV with a picture), from which the person picks, as with SHARDFALL's looks.
- **Event sounds: crunch** (by the agent of another game, in Polter's workspace since 19 October 2026; part of a release once `listen` uses it, E4): footsteps, impacts, crunches and rustles as patches, rendered as variants and judged against real recordings by rules (clicks, gaps, loudness from step to step) and guides (octave bands, decay, graininess). A step takes 7 to 30 ms to render on the MacBook Air, so variants are rendered beforehand and played as samples (E4).
- **Continuous sounds and machines** (proposed by the agent of another game on 19 October 2026, from that game's engine sound, about ten rounds with the owner as listener; its written proposal read, that game's sources it names not yet read). Three steps, in this order:
  1. *Checks for continuous sounds* in crunch (**built** by crunch's author, 19 October 2026, and taken into the workspace the same evening; the wiring into `listen` is next): **tones** (narrow peaks above 1 kHz, "digital whistling"), **orders** (the balance of a machine's firing orders, "no power"), **modulation** (envelope depth per band, rough running), **loop seam**; crunch already has clicks and micro gaps. Opt-in per sound through a **profile** (`broadband` for machines and weather, `event` against a reference), so chip sounds may stay tonal. `listen` judges every sound with a profile: verdict lines, `checks` in `--json`, exit 1 when a rule fails. The measures are built in crunch by its author; the wiring into Polter is built here. (E4)
  2. *Machines*: a voice that follows a game value (`ctx.machine(PUMP).set(rpm)` every frame), made of loops rendered beforehand, one per step of the value. Rules, each of which fixed an audible fault in that game: loops a whole number of machine cycles long; one step plays alone, two cross-fade only in a narrow band between them, power-preserving; all steps start at the same sample index; pitch follows the value within a step (0.5 to 2); values pass a smoothing layer (attack, release); below the lowest step the level falls with the value squared; silent machines cost nothing; real happenings are events from the game's logic, never random rattles. Deterministic, so `twin` and golden frames stay valid. Accepted on the first game that needs one. (E4)
  3. *A machine generator in physical units* (cylinders, firing order, rpm, load, muffler), a port of that game's engine voice, and a simpler one for electric pumps and drills: only when a game needs it.
- **Not built in:** generative image or music models. They need large models and a GPU, their licenses are unclear, and their style is hard to hold. Whatever someone makes with them can be imported like any other file.

---

## 6. Architecture

### 6.1 A game

A game is a Rust crate that implements one trait: `update(ctx, input)` one fixed tick, `draw(ctx, frame)`, `state()` (the probe), `invariants()`. Content lives in text data files (TOML), loaded by id, checked by a data test, reloaded live in the window.

**A game is a folder, anywhere** (The owner, 19 October 2026: "Spiele sollten ihren eigenen Pfad und Ordner haben können. So wie bei Godot"). The folder holds a *polter.toml*, its own `Cargo.toml` (its own workspace, with the quick profile), *BRIEF.md*, `src/`, *assets/*, *levels/*, *scenarios/*, *haunts/*, and `.polter/` for what is generated (the workshop's folder). The **`polter` program** (`dev/`, no dependencies) is the project manager: `new`, `projects`, `open`, `forget`, `build`, and every game command on any game's folder, found as given or from the current folder upwards. All games build into one shared folder, so the engine is built once. **The project manager** is a window too (`polter manager`, built the same night): the list, *workshop*, *play*, *show folder*, *remove from list*, *new game* with a name and a folder chosen in Polter's own browser, *add a game* from where it is. It paints with `polter::paint`, which the workshop's look became. The engine's carts in `carts/` stay as the engine's own examples and tests.

### 6.2 Layers

```
core        time (fixed 60 Hz), seeded RNG, hashing, math, ids
platform    window and full screen, resolution rules, input as actions with bindings
            (keyboard, mouse, gamepads), audio out, files and settings, app bundles
gfx         a true-colour framebuffer; sprites, tiles, text; palettes and the print
            (tone curve, palette, Bayer, from SHARDFALL); 2D light (E5)
audio       synth and mixer, reverb by space
ui          menus, notes, rebinding, the pause menu
data        TOML data by id, live reload, the data test
cells       optional: a world of simulated cells in chunks, materials as data,
            the simulated region around the view (from SHARDFALL)
agent       the command line of 5.1, scenarios, invariants, probes, sheets, perf
workshop    the window of 5.5: timeline, knobs, marks, proposals, replay on rebuild
```

Dependencies run downward only. `cells` and the print are optional, so a card game doesn't pay for falling sand.

### 6.3 Determinism

A fixed tick with no wall clock in the simulation. One seeded RNG per stream, and iteration in a fixed order. Floats are allowed: the promise is the same result for the same binary on the same kind of machine, and nothing more. Cosmetic state (particles, screen shake) is kept out of the hash. **Further** (RESEARCH.md 6, built 19 October 2026): `sin`, `exp` and their kin differ between platforms in the last bit, so Polter has its own, `polter::math`: 22 functions from the operations IEEE 754 fixes exactly, worked in `f64` and rounded once, within one unit in the last place of macOS's own (2,086,080 inputs to the one-argument functions, 497,428 pairs to the others, counted from the tests), and a test that hashes their bits (CI on other systems will run it, E7). The runtime keeps its own rule (`clippy.toml`: the platform's maths, the wall clock, `HashMap` and `HashSet`), its sound included, so a WAV from `listen` is the same everywhere too; the window's pacing and a shipped game's seed are the two allowed exceptions, each marked where it is. New games get the same *clippy.toml*, and `polter` shows every use when it builds, beside the build (0.06 s added after a change, measured 19 October 2026). The engine's own carts are older than the rule and keep the platform's maths until E7 gives them new golden frames.

### 6.4 Rendering on the CPU

Every pixel is computed on the CPU into a framebuffer, then shown scaled by whole pixels. This is how SHARDFALL shades 320 × 180 with light and shadows in 12.3 ms a frame on two cores (measured before the print came in; the print adds a table lookup per pixel and has not been measured again). It needs no GPU driver, it is exactly the same with or without a window, and its frames can be tested byte for byte. The GPU presents: it scales the finished frame and may run **presentation shaders** on the way (CRT, bloom, colour grading, a smoother scaler). Each has a **CPU twin**, so `peek --present` shows what a player sees and a twin test compares the GPU's result with it; a shader without a twin is reported as unverified (E3, RESEARCH.md 3).

### 6.5 The window and the screen (E2)

The picture is made first (the canvas, at the game's own size), then shown. Showing it never changes it, so golden frames hold in any window.

- **Resolution rules**, chosen by the game:
  - *fixed*: the canvas keeps its size, scaled by whole pixels, with bars where the screen is left over;
  - *expand*: the game gets as many more pixels as fill the screen at a whole scale (a 2015 MacBook Air's 1440 × 900 is 360 × 225 at four times, with no bars);
  - *native*: the canvas is the window's size, for the workshop and for games with much text.
- **Window or full screen**, switched at any time (Ctrl+Cmd+F on a Mac, F11 elsewhere), resizable, on any monitor, and remembered.
- **Actions, not buttons.** A game names its actions (jump, fire); each has default bindings on keyboard, mouse and gamepad, and the player can change them in the menu. The console's six buttons are the console profile's default actions.
- **The platform: miniquad** (0.4.11), measured against winit with softbuffer and against minifb on the MacBook Air (`spikes/window/`). The game's picture is computed on the CPU as before; miniquad uploads it as a small texture and the GPU scales it by whole pixels: 0.46 ms a frame in full screen, against 15 ms for softbuffer, which copies the whole screen on the CPU. It waits for the display (exactly 16.67 ms a frame, where minifb gives 17.4), opens real full screen, has text input and a clipboard, builds from clean in 18.6 s and weighs 0.74 MB. Gamepads come from **gilrs** (0.11.2). On a Mac it uses OpenGL by default (deprecated by Apple, working on macOS 12); its Metal backend is the way out if that changes.

### 6.6 Keeping it fast and snappy

- **The runtime is small; the tools are apart.** A shipped game links only the runtime (core, platform, gfx, audio, and `cells` if it uses them). The workshop, the importers, the generators and the checks are separate crates or features: built while a game is made, never in the game a player gets.
- **Import while making, not while playing.** Aseprite, PNG, WAV and OGG are turned into the engine's compact data when they change, in the workshop or at build time. The game loads what is ready.
- **Two ways to build.** The release profile (link-time optimisation, one codegen unit) makes the fastest game and takes its time. The workshop builds with the `quick` profile (optimised, 16 codegen units, incremental, no LTO) so that a change is in the replay within seconds. Measured 19 October 2026 after changing one constant, alternately: GIRDER 0.9 to 1.9 s against 10.7 to 11.5 s in release; SHARDFALL 1.5 to 5.5 s against 35.4 to 37.2 s. It runs almost as fast: GIRDER's 36,000 frames 1.15 s against 1.14 s, SHARDFALL's cave 14.9 ms a frame against 14.3 ms.
- **Every dependency pays its way:** it is measured (clean build, binary size) before it comes in, and the list stays short.
- **Measured against the last good build:** frame time, start, memory, binary size and build times are checked after every milestone, alternately against the build before (L-041, L-054). A slower Polter is a bug.

---

## 7. Budgets on the slowest machine (tests, not hopes)

| Measure | Budget on the 2015 MacBook Air (2 cores, Intel HD 6000) | Measured 19 October 2026 (system load about 2) |
|---|---|---|
| Frame | ≤ 16.7 ms average, p95 ≤ 16.7 ms, at 320 × 180 with full-screen light | SHARDFALL's valley 12.3 ms, p95 16.1 (P0, before the print) |
| Memory | ≤ 256 MB for a large game, ≤ 64 MB for a small one | SHARDFALL 126 MB (P0) |
| Start to first frame | ≤ 1 s (a title card while larger worlds form) | GIRDER headless 0.01 s; SHARDFALL's window at once, its valley 5.6 s behind a title card |
| Binary | ≤ 10 MB | GIRDER 0.95 MB, test card 0.93, OVERBURDEN 1.29, SHARDFALL 1.36 |
| Rebuild after a change | ≤ 5 s (the quick profile, for the workshop) | `quick`: GIRDER 0.9 to 1.9 s, SHARDFALL 1.5 to 5.5 s (within budget, SHARDFALL at its edge the first time); release: 10.7 to 37 s, for shipping only |
| Clean build | ≤ 90 s | GIRDER with all dependencies 40.8 s |
| Going back in the workshop | ≤ 16.7 ms (within a frame) | ten minutes of GIRDER, one second back at the end: 15.3 ms with copies of the game every 8 s by then (1827 ms without them) |

A second reference machine (a Raspberry Pi or an old Windows laptop) comes with E7 if one is at hand.

---

## 8. Milestones

Each one ends with a game that runs on it. Nothing counts as done until a game proves it.

| | Milestone | Proof |
|---|---|---|
| **E0** | This spec, decided; the name; the license | ✓ 19 October 2026 (the license: MPL-2.0) |
| **E1** | Core, platform, agent command line: EMBER-8's loop, input, recording, `check`/`probe`/`peek`/`twin`/`listen` moved into the engine, true-colour framebuffer; scenarios and invariants | GIRDER runs on it, and its recorded sessions replay to the same frames as they do today. **✓ 19 October 2026:** 1580 golden frames of GIRDER (title, the bot's run, the tour, a second palette), OVERBURDEN and the test card identical after the change; headless as fast as before (`check` 1.25 s against 1.2 s, `twin` 1.54 s against 1.60 s, alternated). Also in: the license, saves in the system's folder. Still to come from E1's list: nothing; the window, full screen and app bundles are E2, menus E3 |
| **E2** | window and workshop: miniquad; resolution rules, exact 60 frames a second, full screen, app bundles for any game; the quick build profile (rebuild ≤ 5 s); then the workshop (5.5). *Gamepads (gilrs) moved to E3, with actions and bindings, so the mapping is built once.* | GIRDER and SHARDFALL full screen on the MacBook Air with no bars and golden frames unchanged; then BNDR and the owner tune SHARDFALL's prospector together in the workshop: The owner with knobs and marks, BNDR in the code (The owner's choice on 20 October 2026, instead of GIRDER's jump: "Shardfall. Hier sehe ich wirklich potential."). For it SHARDFALL became a Polter game with fifteen knobs and frozen moments that share unchanged chunks (a minute of the valley: 206 MB, one second back in 313 ms) |
| **E3** | gfx and ui: indexed layers, palette tables and colour cycling, row effects and Mode 7, presentation shaders with CPU twins, sprites from PNG and Aseprite, **animation** (Aseprite's frames, tags, slices and events, a player in ticks, tweens, springs, RotSprite, checks for loops, timing, hitboxes, foot sliding and frame data), tiles and LDtk, fonts with umlauts, the workshop's drawer (RESEARCH.md); actions with bindings, gamepads (gilrs) and a rebinding menu; sprites with animation, tiles, text in true colour (with umlauts), palettes and the print, particles, camera, collision of boxes and tiles, menus, variant sheets; art as data with generators and checks, import of Aseprite and PNG (5.7) | a new small top-down game, made from a one-page brief |
| **E4** | audio: mixer, synth patches, music as text, spaces; sound checks and variation sheets, with crunch's checks for continuous sounds in `listen`; machines that follow a game value; import of WAV and OGG (5.7) | the same game gets its sound; `listen` proves it |
| **E5** | cells and 2D light, out of SHARDFALL | SHARDFALL runs on the engine, H0 continues there |
| **E6** | agent layer complete: `bot`/`solve`, `monkey`, `trace`, `brief`, `api`, the mutation check, budgets as tests, hidden scenarios, MCP, `llms.txt` (`assert` came with E1) | **the agent benchmark** (9): a fresh agent builds three small games from briefs using only the docs |
| **E7** | open-source release: docs, five example games in five genres, CI on three systems, contribution guide | the repository goes public, when the owner says so |

---

## 9. How we know it is agent-first

Claims are cheap, as the rankings show, so the engine gets a benchmark of its own. At E6, an agent with a fresh context and only the engine's documents builds three small games from one-page briefs: a platformer, a puzzle and a simulation. Each brief comes with hidden scenarios that are checked at every tick, in the manner of GameLogicBench. Measured: how many scenarios pass, how many runs and turns it took, and where it got stuck. Every place where an agent got stuck becomes a fix in the engine or its documents. The benchmark runs again at every milestone after that.

---

## 10. What becomes of what exists

- **EMBER-8** is renamed Polter (19 October 2026). Its eight-colour console stays as a profile of the engine: 320 × 180, eight colours, its cart API. GIRDER, OVERBURDEN and the test card keep running. Saves moved to the system's place for data in E1 (`~/Library/Application Support/<game>` on a Mac); old ones in `~/.ember8/<game>` are copied there the first time, and the old folder stays as a backup.
- **SHARDFALL** is the flagship and moves onto the engine in E5. Until then it stays playable as it is.
- **Poltergeist** stays the workbench for Godot projects. The engine borrows its vocabulary, so an agent only learns it once.

---

## 11. Decided (19 October 2026)

| Question | Decision |
|---|---|
| 2D only in 1.0? | Yes. 3D on the CPU would be its own engine. |
| Rust for games, no scripting language? | Yes: one binary, compiler errors as feedback, rebuilds in seconds. A scripting language only when a game needs it. |
| The name | **Polter**. The owner's idea, BNDR's decision, after first picking Planchette for an hour. *Poltern* is the rumble a poltergeist makes: something unseen makes things happen. It belongs with Poltergeist, the agents' workbench for Godot, and keeps its vocabulary (`.haunt` files, peek, probe, twin). It is short, says itself, was free on crates.io and has no project of note on GitHub (checked 19 October 2026). "haunt" and "poltergeist" were taken. |
| License | **MPL-2.0** (Mozilla Public License), decided. The owner's wish: games made with Polter may be sold, but nobody may change the engine itself and sell it as their own closed product. MPL-2.0 is copyleft per file. A game's own files can be under any license, closed and sold. Every file of the engine that someone changes and passes on stays under MPL-2.0, with its source, so a closed, paid fork is impossible, and anyone may give the open one away for free. It is open source (OSI), it includes a patent grant, and it works with Rust's static linking, where LGPL would not. What it does not do: forbid selling outright. An absolute ban would make the engine "source-available" and no longer open source, and its wording would be unclear for games. |
| Pause SHARDFALL? | No: E1 to E4 first while SHARDFALL stays as it is, then E5 moves it and H0 goes on there. |
| How high a resolution? | Low by design: logical pictures up to about 640 × 360 on the CPU, the GPU presenting (section 4). HD games with thousands of sprites would ask too much of a 2015 laptop and of "smooth everywhere". |
| The first release? | **Polter 0.5, a public preview** (BNDR's proposal on 20 October 2026, after the owner's goal "Polter as an engine, done and ready for a first release"; the owner can veto). Everything a small 2D game needs, proven by an example game made only with it: sprites from PNG files, animation in ticks, tiles with a camera and collision, actions bound to keys, mouse and gamepads, text with lower case and umlauts; the tools as they are; docs a stranger can start from; a changelog, a guide for contributors, CI for three systems written down, the package checked. It ships only when the owner says so (E2's proof with SHARDFALL is a milestone of its own: SHARDFALL is not part of a release, and the workshop was already tried live with the owner on 19 October 2026). 1.0 is E7 (five example games, CI running on three systems). SHARDFALL is not part of it: its own terms, never published. |
| Phones and tablets? | Last, after the computers (The owner, 19 October 2026: "das können wir gaaaaaanz nach hinten verschieben. Erst Computer"). The way is known: first the web build in a phone's browser (miniquad and cpal both support the web; the owner has an iPhone 17), touch as actions; native apps after that (an iOS app needs Xcode 26, which a 2015 MacBook Air on macOS 12 can't run, so it would be built in CI). |
| What comes next? | Done on 19 October 2026: runtime and tools apart (6.6: `polter` is the runtime, 4,009 lines; `polter-dev` the tools, 4,574 lines; a new game shipped at 1.02 MB against 1.43 MB with the tools; with gamepads, on 20 October, 1.40 MB against 1.90 MB, and 1.04 MB with them turned off), `polter::math` and the *clippy.toml* (6.3), `polter brief` and `llms.txt`. Next: E2's proof with a game the owner cares about, then E3. |

---

## 12. Sources (read 19 October 2026)

The research on effects, shaders, project shape and agent tools of the evening of 19 October 2026 keeps its own sources, read and found, in [RESEARCH.md](RESEARCH.md).

- Hayao: [hayao.dev](https://hayao.dev/), [repository](https://github.com/hellojanpacan/hayao-js), [architecture](https://hayao.dev/docs/architecture), [engine notes](https://hayao.dev/docs/engine)
- Ultrametric, [Best game engines for AI agents (2026)](https://ultrametric.ai/arena/game-engines)
- [GameLogicBench](https://arxiv.org/abs/2609.21562): coding agents on runtime game logic with tick-level state assertions
- [GameCraft-Bench](https://arxiv.org/abs/2606.17861): can agents build playable games end to end in a real engine?
