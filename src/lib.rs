// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! # Polter
//!
//! A 2D game engine that AI agents can drive end to end, made to run smoothly on every computer,
//! old ones included (formerly EMBER-8; the plan is in SPEC.md).
//!
//! A `Game` updates one fixed tick at a time and draws into a true-colour `Canvas`; a
//! `Session` runs it, with a window or without one. The fantasy console Polter started as
//! (320 × 180 pixels, palette slots with sixteen named colours, six buttons) is a profile: a `Cart` runs as a `Console`.
//! Everything is rendered on the CPU, so running without a window is the same code path as
//! playing in one: what an agent sees in `peek` is exactly what a player sees. Every tick checks
//! the game's invariants, and scenarios check what must be true when. The full API is in
//! `API.md`.

pub mod action;
pub mod anim;
pub mod app;
pub mod audio;
pub mod canvas;
pub mod console;
pub mod deflate;
pub mod ease;
pub mod font;
pub mod game;
pub mod haunt;
pub mod image;
pub mod inflate;
pub mod input;
pub mod json;
pub mod machine;
pub mod math;
pub mod pad;
pub mod paint;
pub mod palette;
pub mod png;
pub mod rng;
pub mod screen;
pub mod speaker;
pub mod sprite;
pub mod state;
pub mod storage;
pub mod text;
pub mod tiles;
pub mod window;

pub use audio::{Play, Sample, Sfx, Wave};
pub use canvas::Canvas;
pub use console::Console;
pub use game::{Ctx, FPS, Frozen, Game, Invariants, Knob, Session};
pub use input::{Button, Frame, Input, Key, Mouse, PadButton, PadFrame, Stick};
pub use machine::{Cart, Machine};
pub use palette::{Col, Palette, palettes};
pub use rng::Rng;
pub use screen::{HEIGHT, Screen, WIDTH};
pub use sprite::Sprite;
pub use state::State;
pub use storage::Storage;
