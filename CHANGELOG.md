# Changelog

What changed, newest first. Until 1.0 a minor version may change the API; from 1.0 on Polter follows semantic versioning.

## Unreleased

- **Recordings** (`audio`, `game`): `Sample::from_wav` reads WAV files of 8, 16, 24 or 32-bit PCM or 32-bit float, any channels and any rate (mixed down and brought to 44.1 kHz once, on loading); `ctx.play_sample` plays one once (sixteen at a time), `ctx.hum` keeps one looping for as long as the game asks for it every tick, its volume and speed gliding and its seam crossfaded, so the sound is a picture of the state as the canvas is and nothing clicks. `Session::sound_names` names every sound that began in a tick; `check` counts and `listen` labels recordings and hums by those names.
- **The whole gamepad** (`input`, `pad`, `haunt`, `action`): a game reads a pad's own 17 buttons (`Input::pad`, named as a DualShock 4 has them, with the Xbox names beside), both sticks and both triggers with their values (`Input::stick`, `Input::trigger`); several pads play as one. They are part of the tick's `Frame`, so they record and replay exactly; `.haunt` files write them as `pad:NAME`, `lstick X,Y`, `rstick X,Y`, `l2 V`, `r2 V`; actions bind `pad:NAME`. `examples/pads` checks all of it with a pad in hand. The console's six buttons come from a pad as before.

## 0.5.0, 20 October 2026: the first public preview

Everything a small 2D game needs, and the tools an agent needs to make one, proven by an example game made with nothing else ([`carts/lantern`](carts/lantern)).

### The runtime (`polter`)

- **Pictures from PNG files** (`image`, `png`, `inflate`): every colour type and bit depth, interlaced or not, read by Polter's own inflate; sprite sheets cut into frames; pictures drawn with transparency, mirrored, clipped.
- **Animation** (`anim`, `ease`, `json`): sheets from Aseprite's JSON or from a grid, tags that run forward, backward or ping-pong with repeat counts, slices that change per frame, events a frame sends; a player in ticks that is part of the game's state; easing curves, tweens and springs.
- **Tiles, a camera and collision** (`tiles`): levels as text, a box moved against solid tiles in steps that can't pass through a wall, a camera that follows softly and stays in the world.
- **Actions and gamepads** (`action`, `pad`): actions bound to keys, mouse buttons and the console's buttons, written as text; gamepads press the console's buttons (feature `gamepad`, on by default, through gilrs).
- **Text** (`text`): Polter's own proportional pixel font with lower case, umlauts and ß; widths, wrapping, BDF fonts.
- **The same bits on every machine** (`math`): `sin`, `exp`, `pow` and their kin built from operations IEEE 754 fixes exactly, within one last place of the platform's own; a *clippy.toml* that names what would make a game differ between machines.
- **A game in its window** (`app`): what a shipped game runs; tools ride along through `Along`.
- **PNG files are packed** (`deflate`): Polter's own LZ77 with deflate's fixed codes; a picture of the workshop shrinks from 11 MB to 245 KB, a frame of LANTERN from 692 KB to 54 KB.
- Examples: `glow` (the game on the README), `pads` and `pads_raw` (a check with a real gamepad).
- Frozen moments (`Game::freeze`, `Frozen`) for games too big to copy whole: the workshop keeps them sharing what didn't change, under a memory budget.

### The tools (`polter-dev`)

- **The `polter` program**: `new` makes a running game in any folder, `manager` is a window to make, add and open games, `build`, `ship` (what a player gets, without the tools), and every game command on any game's folder.
- **The command line every game gets**: `check`, `probe`, `peek`, `shot`, `twin`, `listen`, `assert` (scenarios checked on every tick), `knobs`, `brief` (one page about a game for an agent, measured when it is made), `workshop`.
- **The workshop**: a tape deck for a game; the person plays, rewinds, marks and tunes knobs on faders, an agent works on the same session through files; it reloads itself after a rebuild, at the same tick.
- **`clip`**: a run as an animated GIF that a web page plays as it is (Polter's own encoder: up to 255 colours exactly, more by median cut; only what changed is stored).
- `llms.txt`: the engine's first page for an agent.

### Known gaps

- Tested by hand on macOS 12 (Intel) only.
- Gamepads: tried with one pad, a DualShock 4 on macOS: it presses all six console buttons (`examples/pads`). Other pads and systems are not yet tried.
- Windows and Linux: the CI's tests and golden frames pass there (LANTERN's 576 frames bit for bit those from an Intel Mac, also on an ARM Mac), but nobody has played on them yet. Analogue sticks reach the game only as directions.
- No kerning; no music yet (sound effects only); no phones or tablets.

## Before 0.5

Polter grew out of EMBER-8, a fantasy console (320 × 180 pixels, eight colours, six buttons, sixty frames a second) with a command line for agents, in October 2026. Its console carts still run unchanged: their golden frames, recorded before the core was rebuilt, still match bit for bit.
