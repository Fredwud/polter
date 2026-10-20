// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The console's carts, and the machine that runs one: a session of a cart in the console
//! profile (console.rs), with the cart's screen at hand for tests.

use crate::console::Console;
use crate::game::{Ctx, Invariants, Knob, Session};
use crate::palette::Palette;
use crate::screen::Screen;
use crate::state::State;
use std::any::Any;
use std::cell::Ref;

pub use crate::game::FPS;

/// A console game. Polter calls `update` then `draw`, sixty times per second.
pub trait Cart: Any {
    /// All game logic lives here.
    fn update(&mut self, ctx: &mut Ctx);
    /// Draws the current state. It can't change the game: `draw` only gets `&self`.
    fn draw(&self, screen: &mut Screen, ctx: &Ctx);
    /// What `probe` and `twin` see. Report everything that matters for testing.
    fn state(&self) -> State {
        State::new()
    }
    /// The rules that must hold after every frame (see `Invariants`).
    fn invariants(&self, _check: &mut Invariants) {}
    /// The cart's own palettes (up to 256 slots each); the first is the default.
    /// Empty means the built-in palettes.
    fn palettes(&self) -> Vec<Palette> {
        Vec::new()
    }
    /// Whether the cart draws its own mouse cursor; the window then hides the system cursor.
    fn wants_mouse(&self) -> bool {
        false
    }
    /// The tuning values the workshop shows as sliders (see `Knob`).
    fn knobs(&self) -> Vec<Knob> {
        Vec::new()
    }
    /// Sets one tuning value. Unknown names do nothing.
    fn set_knob(&mut self, _name: &str, _value: f32) {}
    /// A copy of the cart as it is now, for the workshop's rewinding (see `Game::snapshot`): a
    /// cart that is `Clone` returns `Some(Box::new(self.clone()))`.
    fn snapshot(&self) -> Option<Box<dyn Cart>> {
        None
    }
}

/// A cart in a running console: a session (its `ctx`, `canvas`, `sounds`, `audio`, `step_frame`,
/// `state_json`), plus the cart's screen.
pub struct Machine {
    session: Session,
}

impl std::ops::Deref for Machine {
    type Target = Session;
    fn deref(&self) -> &Session {
        &self.session
    }
}

impl std::ops::DerefMut for Machine {
    fn deref_mut(&mut self) -> &mut Session {
        &mut self.session
    }
}

impl Machine {
    pub fn new(cart: Box<dyn Cart>, seed: u64) -> Machine {
        Machine {
            session: Session::new(Box::new(Console::new(cart)), seed),
        }
    }

    /// The console the cart runs in (its palettes, its screen).
    pub fn console(&self) -> &Console {
        self.session
            .game_as::<Console>()
            .expect("a machine always runs a console")
    }

    /// The screen as the cart last drew it, in palette slots.
    pub fn screen(&self) -> Ref<'_, Screen> {
        self.console().screen()
    }

    /// The concrete cart, for tools and tests that need more than `state()`.
    pub fn cart_as<T: Cart>(&self) -> Option<&T> {
        self.console().cart_as::<T>()
    }

    /// The session, for tools that run any game.
    pub fn into_session(self) -> Session {
        self.session
    }
}

/// A screen as RGB bytes through a palette, enlarged by a whole number (nearest neighbour).
pub fn to_rgb(screen: &Screen, pal: &Palette, scale: usize) -> (usize, usize, Vec<u8>) {
    let (sw, sh) = (crate::WIDTH as usize, crate::HEIGHT as usize);
    let scale = scale.max(1);
    let (w, h) = (sw * scale, sh * scale);
    let mut out = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (x / scale, y / scale);
            let rgb = pal.rgb(screen.pixels()[sy * sw + sx], sx, sy);
            out.extend_from_slice(&[(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]);
        }
    }
    (w, h, out)
}
