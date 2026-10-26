# Polter API

Everything a game can use, on one page. If you are an agent: read this file completely before writing a game. It is short on purpose, so you never have to guess an API.

A game is one of two kinds:
- a **cart** for the console profile: 320 × 180 pixels in up to 256 palette slots, drawn with `Screen` (most of this page; start here);
- a **true-colour game**: any size, every pixel 0RGB, drawn into a `Canvas` (see *A true-colour game*).

Both get the same command line, the same invariants and the same scenarios.

A game lives in its own folder, anywhere. The `polter` program (README.md, *Start*) makes one that runs with `polter new FOLDER`, and `polter COMMAND [FOLDER]` builds it and runs any command below on it.

## The machine

| | |
|---|---|
| Screen | 320 × 180 pixels, `WIDTH` / `HEIGHT` |
| Colours | up to 256 palette slots (`Col`), nothing else can be drawn; 16 named slots in the built-in palettes |
| Buttons | `Left Right Up Down A B` (window: arrows, Z, X) |
| Mouse and keys | optional, for desktop carts: cursor in screen pixels, three buttons, wheel, 62 keys |
| Gamepad | presses the six buttons; for carts that want it whole: 17 buttons of its own, two sticks and two triggers with their values (`PadButton`, `Stick`) |
| Time | 60 frames per second (`FPS`), fixed: every frame is exactly 1/60 s |
| Randomness | `ctx.rng`, seeded: same seed and inputs give the same game |

## A cart

```rust
use polter::{Button, Cart, Col, Ctx, Screen, State};

pub struct Hello { x: i32 }

impl Cart for Hello {
    fn update(&mut self, ctx: &mut Ctx) {
        self.x += ctx.input.axis(Button::Left, Button::Right);
    }
    fn draw(&self, s: &mut Screen, _ctx: &Ctx) {
        s.cls(Col::Gunmetal);
        s.text("HELLO", self.x, 80, Col::Shine);
    }
    fn state(&self) -> State {
        State::new().int("x", self.x as i64)
    }
}
```

**Every game is its own crate**, using the engine as a library; its `main` is one call, and the game gets the whole command line for free. `polter new star-pilot` writes:

```text
polter.toml               name and title: this folder is a Polter game
Cargo.toml                its own workspace: polter by path to the engine, polter-dev behind the
                          feature `dev` (on by default); the profiles `quick` and `release`
clippy.toml               what would make the game play differently on another machine
BRIEF.md                  one page: what the game is meant to be
src/lib.rs                the game (a library, so tests in tests/ can use it)
src/main.rs               with `dev`:    polter_dev::cli::run_games(&[polter_dev::cli::GameInfo {
                                             name: NAME, about: "…",
                                             make: || Box::new(star_pilot::StarPilot::new()),
                                         }])
                          without it:    polter::app::play(NAME, Box::new(star_pilot::StarPilot::new()))
scenarios/start.scenario  its first scenario
assets/ levels/ haunts/   empty, for pictures, levels and recorded play
```

A cart's `main` calls `polter_dev::cli::run(&[polter_dev::cli::CartInfo { name, about, make }])` instead. All games build into one shared folder (`~/Library/Caches/Polter/target` on a Mac, `~/.cache/polter/target` elsewhere, or `POLTER_TARGET`), so the engine is built once. The engine's own carts (`carts/lantern`, `carts/testcard`) are crates in the engine's workspace instead.

**The runtime and the tools are apart.** `polter` is what a player gets: the canvas, the console, sessions, input, sound, the window, `polter::app` (a game in its window). `polter-dev` (in `dev/`) is what making a game needs: the command line, scenarios, contact sheets, sound pictures, the workshop. `polter ship` builds a game without the feature `dev`, so without the tools. Tools that ride along with a game in its window implement `polter::app::Along` (the command line's `play --record` does).

**Read `carts/lantern`**: everything this page describes in one small game in true colour (PNG pictures and an Aseprite sheet, animation, tiles from text, a camera, actions, text, light), a bot that proves it can be done, and scenarios. `carts/testcard` is the smallest console cart.


**Rules**
- All game logic lives in `update`. `draw` only gets `&self`, so it cannot change the game.
- Use only `ctx.rng` for randomness, never the system clock or thread RNGs. Otherwise `twin` fails, and replays stop being tests.
- Use `polter::math` for `sin`, `exp`, `pow` and their kin, never the platform's (`x.sin()`), and `BTreeMap` rather than `HashMap`. The platform's maths differs between machines in the last bit, and a `HashMap` iterates in a different order every run. A game made by `polter new` has a *clippy.toml* that names all of these; `polter` shows each use when it builds the game.
- Report everything that matters in `state()`. That's how agents test your cart.
- Declare the rules that must always hold in `invariants()`. Every command checks them on every frame.

## `Ctx`

| Field | |
|---|---|
| `ctx.input` | buttons of this frame (`Input`) |
| `ctx.frame` | frame number, the first frame is 1 |
| `ctx.rng` | the cart's random numbers (`Rng`) |
| `ctx.play(sfx)` | start a sound effect (call it from `update`) |
| `ctx.play_sample(&sample, volume)` · `ctx.hum(key, &sample, volume, speed)` | play a recording once · keep one looping while it is asked for, every tick |
| `ctx.storage` | save games: `read(name)`, `write(name, bytes)`; files in `play`, memory in headless runs |
| `ctx.picture` | whether anyone looks at this frame's picture; headless checks look only at some. A true-colour game may skip filling its canvas when it is `false` (a cart needn't care: the console handles it) |

## `Input`

| Method | |
|---|---|
| `held(b)` | button is down this frame |
| `pressed(b)` | went down this frame |
| `released(b)` | went up this frame |
| `axis(neg, pos)` | -1, 0 or 1, e.g. `axis(Button::Left, Button::Right)` |
| `mask()` | raw bit mask |
| `mouse()` | cursor position in screen pixels |
| `mouse_held(b)` / `mouse_pressed(b)` / `mouse_released(b)` | `b` is `Mouse::LEFT`, `Mouse::RIGHT` or `Mouse::MIDDLE` |
| `wheel()` | wheel steps this frame, positive is up |
| `key(k)` / `key_pressed(k)` / `key_released(k)` | `k` is a `Key`: `A`–`Z`, `Num0`–`Num9`, `Space`, `Enter`, `Tab`, `Backspace`, `Shift`, `Delete`, `Escape`, `Up`, `Down`, `Left`, `Right`, `F1`–`F12`, `Ctrl`, `Alt`, `Super` (Cmd on a Mac) |
| `pad(b)` / `pad_pressed(b)` / `pad_released(b)` | a gamepad's own button, `b` a `PadButton`: `South East West North` (on a DualShock 4 cross, circle, square, triangle; on an Xbox pad A B X Y), `L1 R1 L2 R2`, `Select Start Mode` (Share, Options, PS), `L3 R3` (the sticks pressed in), `Up Down Left Right` (the arrows) |
| `stick(s)` | where a stick leans, `(x, y)` from -1 to 1, right and up positive; `s` is `Stick::Left` or `Stick::Right`. Raw, without a dead zone: a pad at rest reads a little off centre (`examples/pads` measures how much), so a game picks its own |
| `trigger(s)` | how far L2 (`Stick::Left`) or R2 (`Stick::Right`) is pressed, 0 to 1 |

A cart that uses the mouse says so with `fn wants_mouse(&self) -> bool { true }`: the window then hides the system cursor (draw your own) and leaves P and Escape to the cart (F1 cycles palettes; quit with Cmd+Q or by closing the window).

**The window** shows the picture scaled by whole pixels, with bars where the screen is left over, and switches to full screen with Ctrl+Cmd+F on a Mac (F11 or Alt+Enter elsewhere); it remembers the choice. It runs the game at exactly 60 frames a second, paced by the display. For agents, who can't watch it: `POLTER_CLOSE_AFTER=SECONDS` closes the window by itself, and `POLTER_WINDOW_REPORT=FILE` writes how it ran (display frames, ticks, time between frames, the game's time per frame).

## `Screen`

All drawing is clipped. Coordinates are `i32`, the origin is top left.

| Method | |
|---|---|
| `cls(c)` | fill the screen |
| `camera(x, y)` | shift everything drawn afterwards by (-x, -y), whole pixels only (screen shake); `camera(0, 0)` resets |
| `pset(x, y, c)` / `pget(x, y)` | one pixel |
| `rect(x, y, w, h, c)` / `rect_fill(…)` | rectangle outline / filled |
| `line(x0, y0, x1, y1, c)` | line, both ends included |
| `circ(cx, cy, r, c)` / `circ_fill(…)` | circle outline / filled |
| `text(s, x, y, c) -> i32` | 3×5 font, uppercase; returns the x after the text |
| `text_scaled(s, x, y, c, scale) -> i32` | the same font, every pixel as a `scale` × `scale` block, for titles |
| `Screen::text_width(s)` / `text_width_scaled(s, scale)` | width of a text in pixels |
| `sprite(&spr, x, y, flip_x)` | draw a sprite, transparent pixels skipped |
| `hash()` | fingerprint of the screen (used by `twin`) |

The font: `A–Z 0–9` and `. , ! ? : ; - + = * / ' " ( ) [ ] < > % # _ & @ ·` plus space. Advance 4 px, line height 6 px. Unknown characters draw as a hollow box.

## `Col`

| Slot | Code | Default colour | Use |
|---|---|---|---|
| `Black` | K | `#0D0E13` | outlines |
| `Gunmetal` | G | `#232733` | deep shadow, background |
| `Steel` | S | `#4A5263` | mid shadow |
| `Chrome` | C | `#8C97AB` | base tone |
| `Shine` | W | `#DCE3EE` | highlights, text |
| `Visor` | Y | `#FFD447` | lights, eyes |
| `Ember` | O | `#FF6A1F` | the one loud accent |
| `Beret` | R | `#A3312A` | warm dark |
| `Sand` | D | `#C9A26B` | warm light earth |
| `Clay` | L | `#7A5136` | warm mid earth |
| `Bark` | B | `#4A2F22` | dark brown |
| `Moss` | M | `#2F7A4F` | green |
| `Deep` | P | `#2A4F7A` | dark blue |
| `Water` | U | `#4F8FD1` | mid blue |
| `Sky` | I | `#8FB7D9` | light blue |
| `Paper` | E | `#E9DFC4` | warm off-white |

`Col` is a slot index (`Col(0)` to `Col(255)`); the names above are constants for the first sixteen. Built-in palettes restyle those sixteen: `Chrome & Ember` (default), `PICO-8`, `Game Boy`, `Parchment 1-bit`. A palette slot may be a two-colour dither.

**Your own palette:** implement `fn palettes(&self) -> Vec<Palette>` (the first is the default). `Palette::ramps(name, &[(dark, light), …], shades)` builds ramps of shades with cool shadows and warm lights: slot = ramp × shades + shade. That's how lighting works with an indexed palette: pick a darker shade of the same ramp.

## `Sprite`

```rust
let robot = Sprite::from_rows(&[
    ".KK.",
    "KYYK",
    "KCCK",
]);
s.sprite(&robot, 10, 20, false);
```

Codes are the letters from the `Col` table, `.` is transparent. Uneven rows or unknown codes panic with the row and column. For any of the 256 slots, give the sprite its own legend: `Sprite::from_rows_with(&rows, &[('a', Col(40)), ('b', Col(41))])`.

## Sound

```rust
use polter::{Sfx, Wave};
const JUMP: Sfx = Sfx::new("jump", Wave::Square, 260.0, 520.0, 7, 0.22);
// in update:
ctx.play(JUMP);
```

`Sfx::new(name, wave, from_hz, to_hz, frames, volume)`: a tone that slides from one pitch to the other while it fades out. Waves: `Square` (hollow, loud), `Triangle` (soft), `Saw` (buzzy), `Noise` (hiss and rumble; the pitch sets how fast it changes). Eight sounds play at once; a ninth replaces the oldest. Keep volumes around 0.15 to 0.35.

The `name` matters: `check` counts sounds by name and `listen` labels them, so agents can verify sound without hearing it.

### Recordings

```rust
use polter::Sample;
use std::sync::LazyLock;
static CREAK: LazyLock<Sample> =
    LazyLock::new(|| Sample::from_wav("creak", include_bytes!("../assets/creak.wav")).unwrap());
static ENGINE: LazyLock<Sample> =
    LazyLock::new(|| Sample::from_wav("engine", include_bytes!("../assets/engine.wav")).unwrap());
// in update:
ctx.play_sample(&CREAK, 0.4);                  // once
ctx.hum("engine", &ENGINE, 0.3 * load, 0.8 + 0.4 * load); // every tick it should sound
```

`Sample::from_wav(name, bytes)` reads WAV files of 8, 16, 24 or 32-bit PCM or 32-bit float, any channels (mixed down to one) and any rate (brought to 44.1 kHz once, on loading); `Sample::new(name, samples)` takes samples made by the game. `ctx.play_sample(&s, volume)` plays it once (`play_sample_at` with a speed: 0.5 an octave lower and twice as long); sixteen play at once, the quietest gives way. `ctx.hum(key, &s, volume, speed)` keeps it sounding in a loop as long as it is called every tick: volume and speed glide to the new values within about 30 ms, a tick without the call fades it out, and the loop's seam is crossfaded, so nothing clicks. Sound is a picture of the state, like the canvas: the game says each tick what should be heard. `check` and `listen` name a recording when it starts and a hum on its first tick (`Session::sound_names`).

## `Rng`

`next_u32()` · `range(lo, hi)` (hi excluded) · `float()` in 0..1 · `chance(p)`

## `math`

The same bits on every machine: built from +, −, ×, ÷ and `sqrt` (which IEEE 754 fixes exactly), worked in `f64` and rounded once, within one unit in the last place of the platform's own.

`sin` · `cos` · `sin_cos` · `tan` · `asin` · `acos` · `atan` · `atan2(y, x)` · `exp` · `exp2` · `exp_m1` · `ln` · `ln_1p` · `log2` · `log10` · `pow(x, y)` · `powi(x, n)` · `cbrt` · `hypot(x, y)` · `sinh` · `cosh` · `tanh`, all on `f32`.

```rust
let (s, c) = polter::math::sin_cos(angle);
let fade = polter::math::exp(-t / 30.0);
```

`sqrt`, `abs`, `floor`, `round`, `min`, `max`, `clamp` and `mul_add` are exact everywhere already: use them as they are.

## `State`

```rust
State::new().num("x", 1.5).int("lives", 3).flag("on_ground", true).text("mode", "play")
```

Numbers are rounded to 4 decimals. The result is a flat JSON object.

## Invariants

The rules that must hold after every frame, named by what must be true:

```rust
fn invariants(&self, check: &mut Invariants) {
    check.require("lives are never negative", self.lives >= 0);
    check.require("the player is never inside rock", !self.stuck());
}
```

`check`, `probe`, `peek`, `shot`, `twin`, `listen` and `assert` check them after every frame, and stop at the first broken one with the frame, the rule and the state (exit 1). In the window, a broken rule is printed once and the game goes on. Write the rules that a game can break *during* a run, not only at the end: a cooldown that fires early, a bolt counted twice, money below zero. Those are the errors final-state tests miss.

## Knobs

A knob is a tuning value the person drags in the workshop as a fader (jump height, gravity, a colour). A game (or a cart: the same two methods) reports its knobs with their current values and sets one by name:

```rust
use polter::Knob;

fn knobs(&self) -> Vec<Knob> {
    vec![Knob::new("speed", self.speed, 0.5, 4.0, 0.1)]   // name, value, min, max, step
}
fn set_knob(&mut self, name: &str, value: f32) {
    if name == "speed" { self.speed = value; }             // unknown names do nothing
}
```

`set_knob` is called from a fader, from `knobs.txt` or from a `knob NAME VALUE` line in `commands.txt` (*The workshop*); the workshop records every change with its tick, so a replay stays exact. The command `knobs` lists them.

## A true-colour game

```rust
use polter::{Canvas, Ctx, Game, Invariants, State};

pub struct Glow { t: u32 }

impl Game for Glow {
    fn update(&mut self, _ctx: &mut Ctx) { self.t += 1; }
    fn draw(&self, c: &mut Canvas, _ctx: &Ctx) {
        c.clear(0x101820);
        c.set((self.t % 64) as i32, 8, 0xffd447);
    }
    fn size(&self) -> (usize, usize) { (64, 16) }               // default 320 × 180
    fn state(&self) -> State { State::new().int("t", self.t as i64) }
    fn invariants(&self, check: &mut Invariants) { check.require("t grows", self.t > 0); }
}
// main: polter_dev::cli::run_games(&[polter_dev::cli::GameInfo { name: "glow", about: "…", make: || Box::new(Glow { t: 0 }) }])
```

| `Canvas` | |
|---|---|
| `clear(rgb)` | fill everything |
| `set(x, y, rgb)` / `get(x, y)` | one pixel, clipped (outside reads black) |
| `rect(x, y, w, h, rgb)` | a filled rectangle, clipped |
| `draw(&image, x, y)` / `draw_flipped(&image, x, y)` | a picture with its top left at (x, y), clipped; see-through pixels leave what is below, half see-through ones mix with it (in whole numbers); flipped is mirrored left to right |
| `pixels()` / `pixels_mut()` | all pixels, row by row: the fastest way to shade every pixel |
| `width()` / `height()` | the size the game asked for |
| `hash()` | fingerprint of the picture (FNV-1a over RGB bytes; used by `twin` and golden tests) |
| `to_rgb(scale)` / `to_png(scale)` | the picture enlarged by whole pixels (PNG packed by Polter's own deflate, `polter::deflate`) |

**Pictures from PNG files** (`polter::image::Image`): `Image::load("assets/hero.png")?` reads any PNG (every colour type and bit depth, interlaced or not) into RGBA pixels, `0xAARRGGBB`; the error names the file and what is wrong with it. `image.frames(w, h)` cuts a sprite sheet laid out in a grid into its frames, row by row; `image.part(x, y, w, h)` takes one rectangle. Load pictures once (in the game's `new`), not in `draw`.

Optional `Game` methods: `fit()` (how the picture fills the window, `polter::window::Fit`: `Fixed` keeps the size from `size()` with bars around; `Expand` grows the canvas so a whole scale fills the screen with no bars, at least `size()`; `Native` makes the canvas the window's size, one picture pixel to one screen pixel; with `Expand` and `Native`, `draw` reads `canvas.width()`, and `update` must not depend on it), `wants_mouse()`, `next_look()` (F1 in the window switches to the next look and names it), `missing_glyphs()`, `knobs()` and `set_knob()` (*Knobs*), `snapshot()` and `freeze()` (*The workshop*). A cart becomes a `Game` through the console (`Console::new(cart)`), which paints its screen through the chosen palette.

## Animation

A `Sheet` holds frames, how many ticks each shows, tags (a named run of frames), slices (named boxes per frame) and events (names a frame sends when it starts). An `Anim` plays it: plain data, so it belongs in the game's state and rewinds with it.

```rust
use polter::anim::{Anim, Direction, Sheet};
use polter::image::Image;

// from Aseprite: File › Export Sprite Sheet, with "JSON Data" (array or hash), tags and slices on
let hero = Sheet::load_aseprite("assets/hero.json")?;          // its picture: meta.image, beside it
// or from a picture laid out in a grid, tags and events added in code
let coin = Sheet::grid(&Image::load("assets/coin.png")?, 8, 8, 6)  // 8 × 8 frames, 6 ticks each
    .tag("spin", 0, 5, Direction::Forward)
    .event(3, "glint");

// in update
self.anim.play(&hero, if moving { "run" } else { "idle" }); // goes on if already playing
for event in self.anim.tick(&hero) {                         // the events of a frame just begun
    if event == "step" { ctx.play(STEP); }
}
// in draw
canvas.draw(self.anim.image(&hero), x, y);
let hitbox = hero.slice("hitbox", self.anim.frame());        // Option<Bounds>
```

Durations come in milliseconds from Aseprite and become ticks (100 ms: 6). Directions: forward, reverse, ping-pong (the end frames not twice), ping-pong reverse; a tag with a repeat count plays that many times and stays on its last frame (`finished()`). Trimmed frames are put back in their place; rotated ones are refused.

**Easing** (`polter::ease`): `Ease::OutBack.at(t)` for t in 0..1 (linear, quad, cubic and sine in, out and in-out; back; bounce; elastic). `Tween::new(from, to, ticks, Ease::OutQuad)` runs a value in ticks and ends exactly on `to`; `Spring::new(value, stiffness)` follows its `target` (critically damped). All in `polter::math`, all plain data.

## Tiles, camera, collision

A level can be text, a character a tile, so an agent reads and writes it without an editor (`polter::tiles`):

```rust
use polter::tiles::{Camera, TileMap};

let map = TileMap::from_text(LEVEL, 16, &[('#', Some(0)), ('=', Some(1)), ('.', None)])?;
let tiles = Image::load("assets/tiles.png")?.frames(16, 16);    // tile k is frame k
// in update: move the hero's box against the solid tiles
let m = map.move_box(self.x, self.y, 12.0, 14.0, self.vx, self.vy);
(self.x, self.y) = (m.x, m.y);
if m.hit_y { self.vy = 0.0; }                                     // landed, or bumped a ceiling
self.cam.follow(self.x, self.y, 0.15);
self.cam.keep_inside((map.width * map.size) as f32, (map.height * map.size) as f32);
// in draw
let view = self.cam.view();
map.draw(canvas, &tiles, view);
canvas.draw(&hero, self.x as i32 - view.0, self.y as i32 - view.1);
```

`move_box` goes sideways first, then up or down, in steps of at most half a tile (nothing passes through a thin wall), and stops flush against what it hits (`hit_x`, `hit_y`). Every tile blocks unless `map.solid[k] = false`; past the edges is solid (`outside_solid`). `from_text` names the line and column of a character its legend lacks; `get`, `set` and `solid_at` work in tiles, `overlaps` in pixels. `Camera::follow(x, y, share)` moves a share of the way towards centring a point; `keep_inside` keeps the view in the world (centred where the world is smaller); `view()` is its top left in whole pixels.

## Text

`polter::text::Font::standard()` is Polter's own proportional pixel font: every printable ASCII character, ÄÖÜäöüß, € ° · – × …; 7 pixels from a capital's top to the baseline, 2 below it for g, j, p, q, y, a pixel between letters, 11 from line to line.

```rust
use polter::text::Font;
let font = Font::standard();
font.draw(canvas, "Bleiglanz: 12 kg", 8, 8, 0xf4e4b8);          // returns the size it took
let lines = font.wrap("A long sentence for a narrow box.", 80);   // at spaces, cuts long words
let w = font.width("Ü");                                          // the widest line, in pixels
let gaps = font.missing("Ψ");                                     // characters it lacks: boxes
```

`Font::from_bdf(&text)` reads a BDF font (the format most free pixel fonts come in): its glyphs stand on its baseline, each advances by its DWIDTH. No kerning yet.

## Actions and gamepads

A game can ask for "jump" instead of the space bar (`polter::action::Actions`). An action is bound to keys, mouse buttons and the console's buttons; the bindings are text, so a player or an agent changes them in a file:

```rust
use polter::action::Actions;

const KEYS: &str = "left: a left button:left\nright: d right button:right\njump: space w button:a\nfire: mouse:left button:b\n";
let actions = Actions::parse(KEYS)?;      // errors name the line and the word it can't read
// in update
let run = actions.axis(&ctx.input, "left", "right");   // -1, 0 or 1
if actions.pressed(&ctx.input, "jump") { /* … */ }     // also held(), released()
```

**Gamepads** press the console's buttons: the pad and the left stick (past `pad::STICK`, 0.4) are the directions, the lower face button A, the right one B. So a game made for the keyboard plays with a pad as it is. A game that wants the whole pad reads its own buttons, both sticks and both triggers from the `Input` (`pad`, `stick`, `trigger`), and binds the buttons to actions as `pad:NAME` (`fire: mouse:left pad:r2`). A stick's lean is a value, so a light touch can be a slow walk. With several pads plugged in they play as one: their buttons together, of each stick and trigger the one leant furthest. Whatever a pad does is recorded and replayed like the keys (the window puts it in the tick's `Frame`, sticks in steps of 1/127, triggers in steps of 1/255, so a replay is exact); a pad's press takes over a playback in the workshop like a key's. They are read through gilrs, with the feature `gamepad`, on by default; `polter = { …, default-features = false }` leaves them out (a shipped game 0.36 MB smaller). Not through gilrs on a Mac: the touchpad, the gyro, the light bar and rumble. `cargo run --release --example pads` checks a pad in your hands.

## The command line every game gets

Run it as `polter COMMAND [FOLDER] [OPTIONS]`, which builds the game first. The game's program takes the same commands (`GAME COMMAND`); an engine cart runs as `cargo run --release -p NAME -- COMMAND` from the engine's folder. A program can hold several carts (`carts` lists them): the cart's name goes after the command, and can be left out when there is only one.

| Command | |
|---|---|
| `carts` | list the carts in this program |
| `play` | window. `--record FILE` saves what was played as `.haunt`, continuously, so Cmd+Q keeps it |
| `check` | headless run: panics reported as file:line, broken invariants, glyphs the font lacks, and how often each sound played |
| `probe --every N` | state as JSON lines |
| `peek` | PNG frames plus a numbered contact sheet, no window needed |
| `shot --frame N` | one frame as PNG |
| `clip` | the run as an animated GIF that a web page plays as it is: `--frame N` the first frame (default 1), `--frames N` how many, `--step K` every Kth tick (at least 2: browsers stretch shorter delays; default 2, thirty frames a second), `--scale S`, `--out FILE`. Up to 255 colours are kept exactly; more are shown with 255 chosen by median cut |
| `twin` | two runs, compare picture hash and state on every frame |
| `listen` | render the sound as WAV plus a picture: waveform, spectrogram (80 Hz to 8 kHz), and the name of every sound where it starts |
| `assert FILE.scenario …` | run scenarios and check their expectations on every frame they cover (`--scenarios DIR` runs every `.scenario` in a folder) |
| `knobs` | the game's tuning values: name, value, range, step |
| `brief` | one page about the game for an agent, in the shape of llms.txt, measured now: BRIEF.md, how to work on it, the state after frame 1 and after the run, every rule and whether it held, knobs, every scenario in the game's *scenarios* folder and whether it passes, sounds, time per frame, the person's marks from the workshop, files, recent commits. `--project DIR` is the game's folder (`polter brief FOLDER` passes it), `--frames N` the run (default 600), `--out FILE` |
| `workshop` | the workshop window (below). `--resume` continues the last session; `--dir DIR` picks the shared folder; `--out FILE --size W,H` writes a picture of it instead of opening a window |

Common options: `--frames N` · `--haunt FILE` · `--seed N` · `--palette NAME` (carts) · `--storage DIR` · `--json`. `check --perf` measures milliseconds per frame against the 60 FPS budget (use a `--release` build). Exit codes: 0 clean · 1 the game is wrong (a panic, a broken invariant, a twin difference, a failed scenario) · 2 tool problem · 3 bad invocation (also: a scenario that doesn't parse).

Saves (`ctx.storage`) live in the system's folder for program data when a person plays: `~/Library/Application Support/<game>` on a Mac. Headless runs keep them in memory.

`.haunt` files: `FRAME[-FRAME] inputs…` per line, `#` starts a comment. Frame numbers are the frames the cart sees.

| Input | Meaning |
|---|---|
| `left right up down a b` | console buttons held |
| `click` `rclick` `mclick` | mouse buttons held |
| `mouse X,Y` | the cursor is at X,Y from this frame on |
| `move X,Y>X,Y` | the cursor glides over the frame range (with `click`: a drag) |
| `wheel+N` / `wheel-N` | wheel steps on each frame of the range |
| `key:NAME` | a key held, e.g. `key:r`, `key:space` |
| `pad:NAME` | a gamepad's own button held, e.g. `pad:south`, `pad:r2` |
| `lstick X,Y` / `rstick X,Y` | a stick leans so from this frame on, e.g. `lstick 0.3,0` (a light lean right) |
| `l2 V` / `r2 V` | a trigger pressed so far (0 to 1) from this frame on |

`play --record` writes all of these, so a human's session replays exactly.

## Scenarios

A scenario is a start, the inputs, and what must be true at which frames. Agents write them to prove a rule; a human's recording becomes one; a judge can keep some hidden.

```text
# LANTERN: the bot's way through gathers every spark, then the door opens
seed 1
haunt ../haunts/bot_run.haunt       # inputs, relative to this file
frames 600                          # how long (default: the last frame named below)
at 560: sparks == 5                 # at one frame
at 560: won == true
always: sparks <= 5                 # on every frame
```

Two more kinds of line: `input 30-60 key:space` adds input, a line as in a `.haunt` file; `from 60 to 300: vx == 0` checks every frame of a range.

An expectation compares a key of `state()` with a number, `true`, `false`, `null` or a `"text"` by `==`, `!=`, `<`, `<=`, `>`, `>=` (only numbers can be ordered). `assert` reports each one: passed, with how many frames it checked, or the first frame it broke and the value there. A key that isn't in the state, or a frame after an explicit `frames`, is reported with the line. `carts/lantern/scenarios/` has examples.

## Testing a game in Rust

| | |
|---|---|
| `Machine::new(Box::new(cart), seed)` | a cart in the console, without a window |
| `Session::new(Box::new(game), seed)` | any game |
| `.step(buttons)` / `.step_frame(&frame)` | one frame: a mask of `Button::bit()`, or full input (a `haunt.frame(n)`) |
| `.state_json()` · `.canvas` · `.broken` · `.sounds` · `.ctx.frame` | what the frame left |
| `machine.screen()` · `machine.cart_as::<MyCart>()` · `session.game_as::<MyGame>()` | the screen in palette slots; the concrete cart or game |

`carts/lantern/tests/bot.rs` is an example: a bot that searches with copies of the game for the way through, and the way it finds played from the start as keys.


## The workshop

`polter workshop FOLDER` (or `GAME workshop`) opens a tape deck for the game: the person plays, everything played is recorded, and the tape winds to any tick. A panel beside the game holds the knobs as faders, the rules and the state; an agent works on the same session through files.

| Key or mouse | |
|---|---|
| REC | record from here (the person's play goes onto the tape; after a rewind it records over the rest) |
| PLAY · PAUSE | play the tape back · stop time (Alt+Space or F5 toggles) |
| REW · FF | one second back or on; held, they wind at thirty times the speed (Alt+Left/Right, F7/F8) |
| START · STEP | to the beginning · one tick (Alt+0 or F10 · Alt+Down or F6) |
| CUE | mark the moment for the agent (Alt+M, F9) |
| the tape | click or drag to go to a tick; flags are marks, dots are knob changes |
| faders | drag to set a knob; the wheel scrolls them when there are more than fit |
| the panel's handle | fold the panel away for a larger game, and back (Alt+H) |

**Code changes without starting over:** when the game's program is built anew (the same file the workshop runs from), the workshop reloads itself and replays the session to the tick it was at, paused there, now with the new code. `polter build FOLDER` rebuilds in the `quick` profile, the one `polter workshop` runs from, in seconds (an engine cart: `cargo build --profile quick -p NAME`, the workshop started from `target/quick/`). `workshop --continue` is the same by hand: the session in the folder goes on.

While Alt (Option) is held the game gets no keys. A game key or a click into the game during playback takes over: the rest of the tape is recorded over. Nothing is lost by that: what was there is kept as a take first.

The deck shows what works first: when the window narrows, the cassette goes (it repeats what the tape shows), then the LEVEL meter, then LOAD, then the display; the keys get narrower but stay. LEVEL is the game's sound; LOAD is the time a tick takes against its 16.7 ms.

The shared folder: `FOLDER/.polter/workshop` when `polter workshop FOLDER` starts it, `workshop` in the game's data folder when the game's own program does (`~/Library/Application Support/<game>/workshop` on a Mac), or `--dir DIR`:

| File | |
|---|---|
| `recording.txt` | the session: seed, knob changes with their ticks, the inputs as a haunt; saved every two seconds and on closing |
| `knobs.txt` | `name value` per line; edit it and the workshop sets the knob (within a quarter of a second) |
| `commands.txt` | lines for the workshop: `goto 1234`, `play`, `pause`, `mark NOTE`, `knob NAME VALUE`, `reload` |
| `events.log` | one line per event (marks, knob changes, commands, takes), to watch |
| `marks` (folder) | per mark a picture (three times the size) and its tick, state, knobs and note |
| `sessions` · `takes` (folders) | earlier sessions; recordings as they were before something was recorded over them |

Going back plays the recording again from the start, which is exact for a deterministic game but slow in long sessions. A game that can copy itself makes it fast: the workshop keeps copies along the tape (every two seconds, at most 128, thinned as the tape grows) and plays only from the last one.

```rust
#[derive(Clone)]
struct MyGame { /* … */ }

impl Game for MyGame {                                 // for a cart: impl Cart, the same method
    fn snapshot(&self) -> Option<Box<dyn Game>> {      //   returning Option<Box<dyn Cart>>
        Some(Box::new(self.clone()))
    }
    // …
}
```

`draw` must paint the whole picture from the state (copies keep no picture).

A game too big to copy whole (a world of millions of cells) freezes its moments instead: `fn freeze(&self) -> Option<Box<dyn Frozen>>`. A `Frozen` only has to `thaw()` into the game again and say how many `bytes()` it holds, counting what it shares with other moments (unchanged parts of the world behind an `Arc`) as a share. The workshop keeps frozen moments under 256 MB together (`MOST_BYTES`) and thins them out like copies. When a game freezes, `snapshot` is not asked. The moment at tick 0 is always kept (frozen or copied), so going back to the start never makes the game again.
