// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The files of a new game: a running game to start from, in the shape every Polter project has.
//! `{name}` is the crate's name, `{Type}` the game's type, `{Title}` what people read, `{engine}`
//! the engine's folder.

pub const POLTER_TOML: &str = r#"# A Polter project: this folder is the game (as project.godot makes a Godot project).
# Run it with: polter play   ·   every command: polter help
name = "{name}"
title = "{Title}"
"#;

pub const CARGO_TOML: &str = r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
polter = { path = "{engine}" }
# The tools (the command line, the workshop, the checks): there while the game is made, left out
# of what a player gets (`polter ship` builds without them).
polter-dev = { path = "{engine}/dev", optional = true }

[features]
default = ["dev"]
dev = ["dep:polter-dev"]

# This game is its own workspace, wherever its folder is.
[workspace]

# The workshop's profile: optimised enough to play, quick to rebuild after a change.
[profile.quick]
inherits = "release"
codegen-units = 16
lto = false
incremental = true

# What a player gets: the fastest build.
[profile.release]
codegen-units = 1
lto = "thin"
"#;

pub const BRIEF_MD: &str = r#"# {Title}

> One sentence: what the game is, and why someone would play it.

- **Verb:** what the player does most (jump, build, sneak, trade …)
- **Mood:** how it should feel
- **Look:** 320 × 180, its palette or print, pictures it should feel like
- **Done when:** what has to be true for a first playable version

This page is for the people and the agents who work on the game: keep it short and true.
"#;

pub const GITIGNORE: &str = r#"/target
/.polter
"#;

pub const MAIN_RS: &str = r#"//! {Title}, a Polter game. `polter help` lists every command; `polter ship` builds it for
//! players, without the tools.

const NAME: &str = "{name}";

/// While the game is made: every command (play, workshop, check, peek …).
#[cfg(feature = "dev")]
fn main() -> std::process::ExitCode {
    polter_dev::cli::run_games(&[polter_dev::cli::GameInfo {
        name: NAME,
        about: "{Title}: say in one sentence what it is.",
        make: || Box::new({crate}::{Type}::new()),
    }])
}

/// What a player gets: the game in its window, nothing else.
#[cfg(not(feature = "dev"))]
fn main() {
    polter::app::play(NAME, Box::new({crate}::{Type}::new()));
}
"#;

/// The engine's own rule for game code (the platform's maths, the wall clock, randomly ordered
/// maps), one file for both.
pub const CLIPPY_TOML: &str = include_str!("../../clippy.toml");

pub const LIB_RS: &str = r#"//! {Title}. It starts as a glowing dot you move with the arrow keys, WASD or a gamepad: make it
//! your game. What the game is meant to be is in BRIEF.md; Polter's example game, LANTERN, shows
//! pictures, animation, tiles and light (the engine's carts/lantern).

use polter::action::Actions;
use polter::text::Font;
use polter::{Canvas, Ctx, Game, Invariants, Knob, State, math};

const WIDTH: f32 = 320.0;
const HEIGHT: f32 = 180.0;
const GROUND: u32 = 0x101820;
const GLOW: u32 = 0xffd447;
/// The keys, as actions: change them here (or read them from a file with `Actions::parse`).
const KEYS: &str = "left: a left button:left\nright: d right button:right\nup: w up button:up\ndown: s down button:down\n";

/// Everything that makes one moment of the game.
#[derive(Clone)]
pub struct {Type} {
    x: f32,
    y: f32,
    speed: f32,
    ticks: u32,
    actions: Actions,
}

impl {Type} {
    pub fn new() -> Self {
        Self {
            x: WIDTH / 2.0,
            y: HEIGHT / 2.0,
            speed: 1.5,
            ticks: 0,
            actions: Actions::parse(KEYS).expect("the keys in KEYS"),
        }
    }
}

impl Default for {Type} {
    fn default() -> Self {
        Self::new()
    }
}

/// A colour between two (t from 0 to 1), channel by channel.
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |s: u32| {
        let (x, y) = (((a >> s) & 255) as f32, ((b >> s) & 255) as f32);
        ((x + (y - x) * t) as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}

impl Game for {Type} {
    fn update(&mut self, ctx: &mut Ctx) {
        self.ticks += 1;
        let (input, keys) = (&ctx.input, &self.actions);
        self.x += keys.axis(input, "left", "right") as f32 * self.speed;
        self.y += keys.axis(input, "up", "down") as f32 * self.speed;
        self.x = self.x.clamp(0.0, WIDTH - 1.0);
        self.y = self.y.clamp(0.0, HEIGHT - 1.0);
    }

    fn draw(&self, c: &mut Canvas, _ctx: &Ctx) {
        c.clear(GROUND);
        // a soft glow, brightest in its middle, breathing (polter::math: the same on every
        // machine; clippy.toml says why)
        let breath = 0.8 + 0.2 * math::sin(self.ticks as f32 / 24.0);
        for dy in -8i32..=8 {
            for dx in -8i32..=8 {
                let d = ((dx * dx + dy * dy) as f32).sqrt() / 8.0;
                if d < 1.0 {
                    let (x, y) = (self.x as i32 + dx, self.y as i32 + dy);
                    c.set(x, y, mix(GROUND, GLOW, (1.0 - d) * (1.0 - d) * breath));
                }
            }
        }
        Font::standard().draw(c, "{Title}", 6, 6, 0xf4e4b8);
    }

    fn state(&self) -> State {
        State::new()
            .num("x", self.x)
            .num("y", self.y)
            .int("ticks", self.ticks as i64)
    }

    fn invariants(&self, check: &mut Invariants) {
        let inside = (0.0..WIDTH).contains(&self.x) && (0.0..HEIGHT).contains(&self.y);
        check.require("the dot stays on the screen", inside);
    }

    fn knobs(&self) -> Vec<Knob> {
        vec![Knob::new("speed", self.speed, 0.5, 4.0, 0.1)]
    }

    fn set_knob(&mut self, name: &str, value: f32) {
        if name == "speed" {
            self.speed = value;
        }
    }

    fn snapshot(&self) -> Option<Box<dyn Game>> {
        Some(Box::new(self.clone()))
    }
}
"#;

pub const SCENARIO: &str = r#"# The dot moves right while Right is held, and never leaves the screen.
seed 1
input 1-60 right
frames 120
at 60: x > 160
always: y == 90
"#;
