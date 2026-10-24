// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The console profile: a `Cart` draws into the `Screen` (palette slots), and the
//! console turns it into a `Game` that paints its canvas through the chosen palette. Polter
//! started as this console (EMBER-8); its carts run on the engine unchanged.

use crate::canvas::Canvas;
use crate::game::{Ctx, Game, Invariants, Knob};
use crate::machine::Cart;
use crate::palette::{Palette, palettes};
use crate::screen::{HEIGHT, Screen, WIDTH};
use crate::state::State;
use std::any::Any;
use std::cell::{Ref, RefCell};

/// A cart, its screen and its palettes, as a game.
pub struct Console {
    cart: Box<dyn Cart>,
    screen: RefCell<Screen>,
    pals: Vec<Palette>,
    pal: usize,
    /// The palette in use as a table: both colours of every slot (a slot is a checkerboard of
    /// two colours; a slot the palette lacks shows as its first slot).
    table: Vec<[u32; 2]>,
}

/// Both colours of every slot of a palette, for painting a screen quickly.
fn table(pal: &Palette) -> Vec<[u32; 2]> {
    (0..256)
        .map(|k| {
            let c = crate::palette::Col(k as u8);
            [pal.rgb(c, 0, 0), pal.rgb(c, 1, 0)]
        })
        .collect()
}

impl Console {
    /// A cart with its palettes: its own first, then the built-in ones; the first is in use.
    pub fn new(cart: Box<dyn Cart>) -> Console {
        let pals: Vec<Palette> = cart
            .palettes()
            .into_iter()
            .chain(palettes().iter().cloned())
            .collect();
        let table = table(&pals[0]);
        Console {
            cart,
            screen: RefCell::new(Screen::new()),
            pals,
            pal: 0,
            table,
        }
    }

    /// All palettes the cart can be shown in, its own first.
    pub fn palettes(&self) -> &[Palette] {
        &self.pals
    }

    /// The palette in use.
    pub fn palette(&self) -> &Palette {
        &self.pals[self.pal]
    }

    /// Shows the cart in another of its palettes (an index into `palettes()`).
    pub fn set_palette(&mut self, k: usize) {
        self.pal = k.min(self.pals.len() - 1);
        self.table = table(&self.pals[self.pal]);
    }

    /// The screen as the cart last drew it, in palette slots.
    pub fn screen(&self) -> Ref<'_, Screen> {
        self.screen.borrow()
    }

    /// The concrete cart, for tools and tests.
    pub fn cart_as<T: Cart>(&self) -> Option<&T> {
        let any: &dyn Any = &*self.cart;
        any.downcast_ref::<T>()
    }
}

impl Game for Console {
    fn update(&mut self, ctx: &mut Ctx) {
        self.cart.update(ctx);
    }

    fn draw(&self, canvas: &mut Canvas, ctx: &Ctx) {
        // the cart always draws (a panic or a missing glyph must show in every check) ...
        let mut screen = self.screen.borrow_mut();
        self.cart.draw(&mut screen, ctx);
        // ... but its screen is painted into the canvas only when someone looks: each slot is
        // two colours in a checkerboard, the first where x + y is even
        if !ctx.picture {
            return;
        }
        let w = WIDTH as usize;
        for (y, (out, row)) in canvas
            .pixels_mut()
            .chunks_mut(w)
            .zip(screen.pixels().chunks(w))
            .enumerate()
        {
            for (x, (px, &c)) in out.iter_mut().zip(row).enumerate() {
                *px = self.table[c.0 as usize][(x + y) & 1];
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        (WIDTH as usize, HEIGHT as usize)
    }

    fn state(&self) -> State {
        self.cart.state()
    }

    fn invariants(&self, check: &mut Invariants) {
        self.cart.invariants(check);
    }

    fn wants_mouse(&self) -> bool {
        self.cart.wants_mouse()
    }

    fn next_look(&mut self) -> Option<String> {
        self.set_palette((self.pal + 1) % self.pals.len());
        Some(self.pals[self.pal].name.to_string())
    }

    fn missing_glyphs(&self) -> Vec<char> {
        self.screen.borrow().missing_glyphs().to_vec()
    }

    fn knobs(&self) -> Vec<Knob> {
        self.cart.knobs()
    }

    fn set_knob(&mut self, name: &str, value: f32) {
        self.cart.set_knob(name, value);
    }

    /// The cart's copy in a console with the same palette (the screen is drawn whole each tick).
    fn snapshot(&self) -> Option<Box<dyn Game>> {
        Some(Box::new(Console {
            cart: self.cart.snapshot()?,
            screen: RefCell::new(Screen::new()),
            pals: self.pals.clone(),
            pal: self.pal,
            table: self.table.clone(),
        }))
    }
}
