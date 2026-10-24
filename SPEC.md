# Polter – specification

Status, 24 October 2026: **0.5 is out, a public preview** ([CHANGELOG.md](CHANGELOG.md)), checked by CI on Windows, Linux and macOS. E0 and E1 are done, E2 is built and waits for its proof, and the core of E3 is built. This file is the plan from here: what Polter is for, the principles every change keeps, and what each milestone still has to build. What is built is described in [API.md](API.md) and [README.md](README.md); reasoning and sources for what comes next are in [RESEARCH.md](RESEARCH.md). Whatever a later milestone needs is marked **(E…)** and is not built before.

---

## 1. Pitch

**An open-source 2D game engine that AI agents can drive end to end, made to run smoothly on every computer, old ones included.** Games made with it are free to sell; the engine itself stays open. An agent writes the game, runs it without a window, asks it what is true at every tick, looks at exact pixels, replays a human's session frame by frame, and proves that a level can be finished. Humans get a real window, menus and a double-clickable app.

The verb is *check*: every claim about a game can be checked by a command.

---

## 2. Why another engine

What exists in October 2026 (sources in section 11):

- **The big engines** (Godot, Unity, Unreal, Bevy, Phaser, three.js and others) score at most 49 of 100 for "agent-ready" in an independent ranking. Headless tests reach at most 5 of 10, and diffable project formats and machine-readable API specs are mostly missing. The ranking has no criteria at all for determinism, replay, screenshots, state queries or weak hardware. Agents reach Godot through community MCP servers that drive its editor.
- **Hayao**, "the 2D game engine for AI agents" (TypeScript, MIT, version 0.4), already does much right: a deterministic simulation headless in Node, state probes, hashes, input replay, a puzzle solver, an MCP sidecar and a Claude Code plugin. But it is vector graphics in a browser, has no native build and no GPU path, and by its own words is not for "thousands of sprites at 60 fps with per-pixel effects".
- **What agents fail at** (two benchmarks, both on Godot): 74 % of failures are rules that break *during* a run, not at the end. The hardest are decisions that must hold over several frames, spatial reasoning, and timing. Checking only the final state let wrong solutions pass on 34 of 36 tasks; tick-level assertions with hidden scenarios caught them. Rendered pictures helped with framing, unreadable UI and broken layouts; running more commands without that loop did not help.

**The gap:** a native engine for real-time 2D games, pixel-exact, smooth on every computer down to old ones, where **every pixel and every tick can be checked**. It is meant for action, simulation and pixel art, not only turn-based logic.

---

## 3. Pillars, in order

1. **Agent-first.** Everything a developer does is a command with text or image output and a clear exit code. No task needs a window, an editor or a mouse.
2. **Truth is checkable.** The simulation is deterministic: same build, same seed and same inputs give the same state and the same pixels. Every tick can be asserted on, every frame compared bit for bit, every session replayed.
3. **Smooth everywhere, measured on the slowest.** A 2015 MacBook Air is the floor, not a "minimum spec" that barely runs. Frame, memory, start-up and build budgets are tests.
4. **Humans second, never second-class.** A real window, a pause menu, rebindable keys, an app to double-click, sessions that record themselves.
5. **Small enough to hold in mind.** The whole API fits in one document an agent reads in full each session. There is one way to do each thing, and errors say how to fix them.
6. **Tools never weigh on games.** Whatever helps make a game (the workshop, importers, generators, checks) lives outside the game that ships, and every addition is measured against the budgets before it stays (6.6, 7).
7. **Open, and staying open.** Anyone may make and sell games with it. Whoever changes the engine itself and passes it on must publish the changes under the same licence (section 10). No telemetry, no network, no accounts.

---

## 4. Scope of 1.0

**Low resolution by design:** logical pictures up to about 640 × 360, computed on the CPU and scaled by whole pixels, the GPU presenting. Every effect is designed for that budget: at 320 × 180 the two cores have about a thousand instructions per pixel and frame (an estimate, RESEARCH.md 1), and the whole picture fits in the cache. HD games with thousands of sprites would ask too much of a 2015 laptop. The tricks that make it look like more (palette tables, colour cycling, row effects, Mode 7, light at a quarter of the resolution) are in RESEARCH.md.

**Games it is for:** 2D at the resolution each game chooses, pixel art first, in a window or full screen: platformers, top-down action, shooters, puzzles, roguelikes, falling-sand and cell simulations, strategy, management and card games, with up to a few thousand moving things.

**With it:** the workshop (5.5), where a person and an agent make a game together.

**Not in 1.0:** 3D, networking and multiplayer, phones, consoles, a construction editor (scene trees to click together, asset editors: agents write the code and the data, and the workshop is where people judge, tune and point), a general physics engine (collision is boxes, tiles and cells), a scripting language. Each may come later, as its own milestone, needed by a real game.

**The console is a profile, not the frame.** Polter grew out of a fantasy console (320 × 180, a fixed palette, six buttons). Its constraints help agents and make a look, so the console stays as a profile for carts. The engine itself is not a console in a window: a game chooses its size, its colours and its actions, and the window follows the screen.

---

## 5. The agent interface

### 5.1 One program, one vocabulary

Every game gets the same command line, and the `polter` program runs it on any game's folder. Built (API.md, *The command line every game gets*): `play [--record FILE]`, `check [--perf]`, `probe`, `peek`, `shot`, `clip`, `twin`, `listen`, `assert`, `knobs`, `brief`, `workshop`; and `polter new`, `projects`, `manager`, `open`, `forget`, `build`, `ship`. Output is text (JSON with `--json`), PNG, GIF and WAV. Exit codes: 0 clean, 1 the game is wrong, 2 the tool failed, 3 the call was wrong.

Planned commands are in 5.6 and in the milestones (8).

### 5.2 Checks that catch what agents get wrong

- **Invariants**, declared by the game (`money never negative`, `the player is never inside rock`, `a cooldown never fires early`), checked after every tick by every command. A broken one stops the run and names the tick, the invariant and the state.
- **Scenarios**: text files with a seed, inputs and conditions at ticks. Agents write them; a human's recording becomes one. **Hidden scenarios**, kept by the person or a judge and run as a gate (E6).
- **Golden pictures:** rendering is on the CPU, so a frame is the same bytes in every run and on every system, and a picture can be a test.
- **Mutation check (E6):** the engine breaks a game's rule on purpose and confirms that its tests notice.

### 5.3 Documents an agent needs

Built: API.md (the whole API, small enough to read in full), `llms.txt` (the first page), and `brief` (one page per game, measured now). Planned: the machine-readable API from `api` (E6). Every error message names the fix ("`Sprite::new` expects rows of equal length; row 3 has 7 cells, the others 9").

### 5.4 MCP (E6)

A thin MCP server that wraps the command line, for agents that prefer tools to a shell. The command line stays the source of truth: anything the server can do, a shell can do.

### 5.5 The workshop (E2)

A window where a person plays, tests and tunes a game while an agent works on it: a workbench where judgment (the person's) meets measurement and code (the agent's), not a construction kit.

Built (API.md, *The workshop*): a tape deck whose tape is the timeline; recording, rewinding (with copies or frozen moments of the game along the tape), knobs on faders, marks, takes, the agent's files and commands, and reloading after a rebuild at the same tick. The rules it keeps: every key and display has a function; panels share their room by what each holds and scroll what doesn't fit; when the window narrows, what repeats something else goes first.

- **The agent drives it too.** Every action of the workshop is also a file and a command.
- **Honest about time.** An agent works in turns and doesn't see the window. The workshop writes marks and events to files; an agent watching them answers within seconds. The two work at the same time, not in the same instant.

Next:

- **Proposals.** The agent shows a change as a variant: old and new side by side, on the same recording. The person keeps one.
- **Scenarios from the workshop**: a recorded session turned into a scenario.
- **The drawer** (E3, RESEARCH.md 5): sprites, sounds, levels, scenarios, marks, takes and sessions, each to preview, open, reload or load onto the deck.
- **The headless workshop** (E6): an agent plays and tunes through the workshop's commands without a window; its sessions land as takes the person can play back.

### 5.6 What only an engine for agents does

What the benchmarks and the work on games with agents show agents need, and no engine of October 2026 gives them together:

| Need | Polter |
|---|---|
| Rules break *during* a run (74 % of agents' failures in GameLogicBench) | invariants on every tick; scenarios with expectations per frame (built) |
| Pictures that can be trusted as tests | CPU rendering: the same bytes with and without a window, on every system; golden frames (built) |
| Agents can't watch motion or timing | contact sheets, `probe` over time, `clip` (built); `trace` (E6) |
| Agents forget between sessions | `brief`: what the game is, its scenarios and invariants and whether they pass, marks from the person, time per frame, recent changes (built) |
| Tests that test nothing | the mutation check (E6) |
| A change that quietly makes things slower | budgets as tests: frame time, memory, start, binary size, against the last good build (E6) |
| Style is taste, not a number | variant sheets (`looks`): the same frame through several settings, side by side (E3) |
| The person's feel is the only judge of fun | the workshop: marks and knobs on the person's own recording (built), proposals (next) |
| Hidden acceptance | scenarios the agent never sees, run as a gate (E6) |
| Random play finds what scripts don't | `monkey` (E6) |
| A failing run is too long to read | shrinking: the recording cut to the shortest one that still breaks the rule (E6) |
| "What did my change do to the picture?" | visual diffs: a heat map of changed pixels between two builds or seeds (E6) |
| Agents can't see flashing, colours or contrast as people do | checks for flashing (no more than three flashes a second over a large area), colour blindness and text contrast, on every frame (E6) |
| Agents can't feel a game | a feel report: ticks from press to reaction, coyote time, buffers, hitstop, shake, camera jerk (E6) |
| A level must be finishable | LANTERN's bot searches with copies of the game and proves it as a test (built); `bot` and `solve` as commands (E6) |
| Agents need to know where things go | `polter new` with a fixed project shape, *polter.toml*, *BRIEF.md* (built); assets checked when the game builds, `files --json` (E3) |

### 5.7 Art and sound (E3, E4)

Agents are weakest at art and presentation (GameCraft-Bench: the lowest scores, and "raw programmatic shapes" are marked down). Polter answers that differently for each side:

- **For the person: no editors of its own, but first-class import.** A pixel editor or a sound studio of Polter's own would cost years and stay worse than the tools people already use. So Polter reads their files: PNG and Aseprite's sheets (built), Aseprite files themselves, WAV and OGG (E3, E4), reloaded live in the workshop when they change (E3). In the workshop the person tunes what belongs to the game: a palette, a look, a sound's knobs.
- **For the agent: art and sound as data, with tools that make it good.** An agent can't hold a pen, but it can describe precisely and measure:
  - *formats it writes well*: sprites as rows of letters with a legend (built), palettes as ramps (built for carts; in OkLCH with hue shift, E3), sound effects as synth patches, music as text (E4);
  - *generators with knobs*: ramps and palettes, outlines and shading from a height or normal map, dithering, tiles that join by rule, noise textures; sound patches and their variations;
  - *checks that judge what can be measured*: the greyscale test (do the values read?), contrast between player and background, palette conformance, silhouettes; loudness, clipping and pitch of every sound;
  - *variant sheets for the person's taste*: eight versions of a sprite, a palette or a sound on one sheet, from which the person picks.
- **Event sounds and sound checks come from crunch**, a crate that is not yet in the public tree (it joins when `listen` uses it, E4): footsteps, impacts, crunches and rustles as patches, rendered as variants beforehand and played as samples, and checks that tell whether a sound will be heard as fake (8.1).
- **Not built in:** generative image or music models. They need large models and a GPU, their licences are unclear, and their style is hard to hold. Whatever someone makes with them can be imported like any other file.

---

## 6. Architecture

### 6.1 A game

A game is a Rust crate that implements one trait, `Game`: `update(&mut self, ctx)` runs one fixed tick, `draw(&self, canvas, ctx)` paints it, `state()` reports what matters, `invariants(check)` declares the rules; everything else is optional (API.md). A console cart implements `Cart` and runs through `Console`.

**A game is a folder, anywhere**, with a *polter.toml* (as a Godot project is a folder with *project.godot*): its own `Cargo.toml` (its own workspace, with the quick profile), *BRIEF.md*, `src/`, *assets/*, *levels/*, *scenarios/*, *haunts/*, and `.polter/` for what is generated (the workshop's folder). The `polter` program (`dev/`) is the project manager, on the command line and as a window (`polter manager`, painted with `polter::paint` like the workshop). All games build into one shared folder, so the engine is built once. The engine's carts in `carts/` are its own examples and tests.

Planned: content in text data files, loaded by id, checked by a data test and reloaded live; assets named in code and checked when the game builds (`asset!("hero.aseprite")`, a compiler error with the path when one is missing) (E3).

### 6.2 Layers

```
core        fixed 60 Hz ticks, seeded RNG, hashing, polter::math
platform    window and full screen, fit rules, input as actions with bindings (keyboard,
            mouse, gamepads), sound out, saves in the system's folder
gfx         the true-colour canvas and the console's palette screen; PNG pictures, animation,
            tiles and camera, text; later palette tables, indexed layers, row effects,
            presentation shaders (E3), the print and 2D light (E5)
audio       a synth of eight voices; later a mixer, patches, music, spaces, machines (E4)
ui          the workshop's painter (polter::paint); later menus, the pause menu, rebinding (E3)
data        later: text data by id, live reload, the data test (E3)
cells       later, optional: a world of simulated cells in chunks, materials as data (E5)
agent       polter-dev: the command line, scenarios, contact sheets, sound pictures, brief
workshop    polter-dev: the window of 5.5
```

Dependencies run downward only. `cells` and the print are optional, so a card game doesn't pay for falling sand.

### 6.3 Determinism

A fixed tick with no wall clock in the simulation, one seeded RNG, iteration in a fixed order. The promise: the same build, seed and input give the same state and the same pixels on every machine. Floats are allowed: their basic operations are exact by IEEE 754, but `sin`, `exp` and their kin come from the platform's maths library and differ in the last bit, so Polter has its own, `polter::math`: 22 functions built from exactly rounded operations, worked in `f64` and rounded once, within one unit in the last place of the platform's own. A test hashes their bits, and CI runs it on all three systems.

`clippy.toml` names what would break the promise (the platform's maths, the wall clock, `HashMap` and `HashSet`). The runtime keeps that rule, its sound included, so a WAV from `listen` is the same everywhere too. New games get the same *clippy.toml*, and `polter` shows every use when it builds a game. The window's pacing and a shipped game's seed are the two allowed exceptions, each marked where it is.

### 6.4 Rendering on the CPU

Every pixel is computed on the CPU into a framebuffer, then shown scaled by whole pixels. LANTERN lights every pixel of its 320 × 180 picture in about 1.4 ms a frame (2015 MacBook Air, 20 October 2026). It needs no GPU driver, it is exactly the same with or without a window, and its frames can be tested byte for byte.

The GPU presents: it scales the finished frame, and later may run **presentation shaders** on the way (CRT, bloom, colour grading, a smoother scaler). Each gets a **CPU twin**, so `peek --present` shows what a player sees and a twin test compares the GPU's result with it; a shader without a twin is reported as unverified (E3, RESEARCH.md 3).

### 6.5 The window and the screen

The picture is made first (the canvas, at the game's own size), then shown. Showing it never changes it, so golden frames hold in any window.

- **Fit rules**, chosen by the game: *fixed* (the canvas keeps its size, scaled by whole pixels, with bars), *expand* (as many more pixels as fill the screen at a whole scale: a 2015 MacBook Air's 1440 × 900 is 360 × 225 at four times, with no bars), *native* (the canvas is the window's size, for the workshop and for games with much text).
- **Window or full screen**, switched at any time, resizable, remembered.
- **Actions, not buttons.** A game names its actions (jump, fire), each with default bindings on keyboard, mouse and gamepad; the console's six buttons are the console profile's default actions. A menu where the player changes the bindings comes with the UI (E3).
- **The platform: miniquad** (0.4.11), chosen over winit with softbuffer and over minifb by measurement ([spikes/window](spikes/window/README.md)): it lets the GPU scale the CPU picture, waits for the display and opens real full screen. Gamepads come from **gilrs** (0.11.2). On a Mac miniquad uses OpenGL (deprecated by Apple, working on macOS 12); its Metal backend is the way out if that changes.

### 6.6 Keeping it fast and snappy

- **The runtime is small; the tools are apart.** A shipped game links only the runtime (`polter`); the command line, the workshop and the checks are `polter-dev`, behind the feature `dev`, never in the game a player gets. Later importers and generators follow the same rule.
- **Import while making, not while playing**, a rule for the importers to come: Aseprite, PNG, WAV and OGG are turned into the engine's compact data when they change, in the workshop or at build time. The game loads what is ready.
- **Two ways to build.** The release profile (link-time optimisation, one codegen unit) makes the fastest game and takes its time. The workshop builds with the `quick` profile (optimised, 16 codegen units, incremental, no LTO), so a change is in the replay within seconds, and runs almost as fast.
- **Every dependency pays its way:** it is measured (clean build, binary size) before it comes in, and the list stays short.
- **Measured against the last good build:** frame time, start, memory, binary size and build times are checked after every milestone, alternately against the build before. A slower Polter is a bug.


---

## 7. Budgets on the slowest machine

On the 2015 MacBook Air (2 cores, Intel HD 6000). Today they are checked by hand; budgets as tests come with E6.

| Measure | Budget |
|---|---|
| Frame | ≤ 16.7 ms average, p95 ≤ 16.7 ms, at 320 × 180 with full-screen light |
| Memory | ≤ 256 MB for a large game, ≤ 64 MB for a small one |
| Start to first frame | ≤ 1 s (a title card while larger worlds form) |
| Binary | ≤ 10 MB |
| Rebuild after a change | ≤ 5 s (the quick profile, for the workshop) |
| Clean build | ≤ 90 s |
| Going back in the workshop | ≤ 16.7 ms (within a frame) |

Measured on 20 October 2026: LANTERN's frame with light per pixel about 1.4 ms; a game fresh from `polter new`, shipped, 1.40 MB (1.04 MB without gamepads). A second reference machine (a Raspberry Pi or an old Windows laptop) comes with E7 if one is at hand.


---

## 8. Milestones

Each one ends with a game that runs on it. Nothing counts as done until a game proves it.

| | Milestone | State | Proof |
|---|---|---|---|
| **E0** | this spec, the name, the licence | done | |
| **E1** | core, platform, the agent command line, the true-colour canvas, scenarios and invariants, saves in the system's folder | done: the console carts' recorded runs replay to the same frames as before the core was rebuilt | |
| **E2** | the window (miniquad): fit rules, exactly 60 frames a second, full screen, Mac apps for the engine's carts; the quick build profile; the workshop (5.5) | built | a person and an agent tune a game together in the workshop, the person with knobs and marks, the agent in the code |
| **E3** | gfx and ui (below) | the core built in 0.5 | a fresh agent builds a new small top-down game from a one-page brief, with only the docs |
| **E4** | sound (8.1) | | the same game gets its sound; `listen` proves it |
| **E5** | cells and 2D light, from SHARDFALL (a game with its own terms, not part of Polter): the print, light maps, shadows from distance fields; radiance cascades if the spike holds (RESEARCH.md 2.4) | | SHARDFALL's cells and light come from the engine |
| **E6** | the agent layer: `bot`/`solve`, `monkey`, `trace`, `api`, the mutation check, budgets as tests, hidden scenarios, shrinking, visual diffs, checks for flashing, colour blindness and contrast, the feel report, bisect, profiling scopes, the headless workshop, MCP | | the agent benchmark (section 9) |
| **E7** | 1.0: five example games in five genres; Windows, Linux and the web played by people, not only checked by CI; a second reference machine | | 1.0 released |


**E3, what is built** (0.5): pictures from PNG files, sheets and animation from Aseprite's JSON (tags, slices, events, a player in ticks), easing, tweens and springs; tiles from text, a camera, boxes that collide with tiles; actions bound to keys, mouse, buttons and gamepads; text with lower case and umlauts, and BDF fonts.

**E3, what is left:** indexed layers, palette tables and colour cycling; row effects and Mode 7 (their cost at 320 × 180 measured first); presentation shaders with CPU twins (first: reading a frame back from the GPU); Aseprite files read directly, LDtk levels; RotSprite and checks for animation (loops, timing, hitboxes, foot sliding, frame data); kerning; a small UI from the workshop's painter (menus, the pause menu, rebinding); particles; variant sheets (`looks`); art generators and checks (5.7); the workshop's drawer and `files --json`; asset changes noted in the recording; data files by id.

### 8.1 E4: sound

In this order:

1. **crunch's checks in `listen`.** crunch already measures what a listener hears as fake: **tones** (narrow peaks between 1 and 7 kHz, "digital whistling"), **clicks** and **micro gaps** per second, the **orders** of a machine's shaft (its firing balance, "no power"), **modulation** per octave band (rough running), and the **loop seam**. A **profile** says which rules a sound must pass: `Broadband` (machines, wind, rain: no tones above 1 kHz, at most one click and one micro gap a second, a clean seam), or `Event` against a reference recording. A sound without a profile is not judged, so chip sounds may stay tonal. The wiring: an optional `profile` on `Sfx` (and on `Machine`); `listen` judges every sound that has one, with verdict lines, a `checks` array in `--json`, exit 1 when a rule fails, and a mark in the sound picture. Accepted when a noise sound such as a furnace's roar passes `Broadband`, a square-wave sound forced through it is flagged as tonal, and `listen --json` carries the checks. Then crunch joins the public tree.
2. **Machines: voices that follow a game value.** A `Machine` is a set of loops rendered beforehand, one per step of a value (rpm, load, speed), mixed from a value the game sets every tick (`ctx.machine(PUMP).set(rpm)`). Rules, each of which fixed an audible fault in another game's engine sound:
   - loops are a whole number of machine cycles long, so they wrap without a seam;
   - one step plays alone; only in a narrow band between two steps (35 % of the gap) do two cross-fade, keeping the power (cos and sin): a wide linear cross-fade combs and sounds like two machines;
   - all steps of a machine start at the same sample index, so a cross-fade never phases;
   - within a step the pitch follows the value (rate = value / step value, 0.5 to 2);
   - values pass a smoothing layer with attack and release: raw values jump, ears don't;
   - below the lowest step the level falls with the value squared: a machine running down gets quiet, not only low;
   - silent machines cost nothing; slowly changing ones update every third tick;
   - real happenings are events from the game's logic, never random rattles or whistles.

   Deterministic (loops are bytes from a patch and a seed, mixed by the synth), so `twin` and golden frames hold; `listen` draws each machine's value as a line. Accepted on the first game that needs one: its pump's rpm follows its load, `listen` passes `Broadband` while it speeds up and slows down, `probe` shows the smoothed value, and a listener hears the load change.
3. **A machine generator in physical units**, only when a game needs it: cylinders, firing order, rpm, load, pipe lengths, muffler chambers (a four-cylinder four-stroke at 1500 rpm fires at 50 Hz, which `orders` then confirms); per-cylinder pulses that vary a little (a perfectly even engine sounds synthetic), jet noise on a continuous mean flow (noise gated to silence between pulses sounds choppy), the casing and valve ticks; output as the stepped loops of step 2. Electric pumps, drills and fans get a simpler voice: blade or pole passing over broadband noise. Accepted when a diesel generator and an electric pump render to loops that pass `Broadband` and `orders` shows the firing order where physics puts it.
4. Then a mixer, synth patches, music as text, spaces, import of WAV and OGG, and variant sheets for sounds.

What carries over regardless of code: one emitter per physical source (exhaust, casing, intake, fan), not one mixed sound; levels from documented values against one measured reference, never by feel; judge recordings of the running game, made with gain (quiet sounds in 16 bits show false gaps); verify the measuring tool before the sound; near sounds at 44.1 kHz; take the listener's words literally.


---

## 9. How we know it is agent-first

Claims are cheap, as the rankings show, so the engine gets a benchmark of its own. At E6, an agent with a fresh context and only the engine's documents builds three small games from one-page briefs: a platformer, a puzzle and a simulation. Each brief comes with hidden scenarios that are checked at every tick, in the manner of GameLogicBench. Measured: how many scenarios pass, how many runs and turns it took, and where it got stuck. Every place where an agent got stuck becomes a fix in the engine or its documents. The benchmark runs again at every milestone after that.

---

## 10. Decided

| Question | Decision |
|---|---|
| 2D only in 1.0? | Yes. 3D on the CPU would be its own engine. |
| Rust for games, no scripting language? | Yes: one binary, compiler errors as feedback, rebuilds in seconds. A scripting language only when a game needs it. |
| Licence | **MPL-2.0.** A game's own files can be under any licence, closed and sold. Every file of the engine that someone changes and passes on stays MPL-2.0, with its source, so a closed, paid fork of the engine is impossible, and anyone may give the open one away. MPL-2.0 is copyleft per file, open source (OSI), includes a patent grant and works with Rust's static linking, where the LGPL would not. It does not forbid selling the engine outright: a ban would make it source-available rather than open source, and its wording would be unclear for games. |
| Phones and tablets? | Last, after the computers. First the web build in a phone's browser (miniquad and cpal both support the web), touch as actions; native apps after that (an iOS app needs Xcode 26, which a 2015 MacBook Air on macOS 12 can't run, so it would be built in CI). |

---

## 11. Sources (read 19 October 2026)

The research on effects, shaders, project shape and agent tools keeps its own sources, read and found, in [RESEARCH.md](RESEARCH.md).

- Hayao: [hayao.dev](https://hayao.dev/), [repository](https://github.com/hellojanpacan/hayao-js), [architecture](https://hayao.dev/docs/architecture), [engine notes](https://hayao.dev/docs/engine)
- Ultrametric, [Best game engines for AI agents (2026)](https://ultrametric.ai/arena/game-engines)
- [GameLogicBench](https://arxiv.org/abs/2609.21562): coding agents on runtime game logic with tick-level state assertions
- [GameCraft-Bench](https://arxiv.org/abs/2606.17861): can agents build playable games end to end in a real engine?
