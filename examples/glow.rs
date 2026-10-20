// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The smallest Polter game worth a window: a glow you steer with the arrows, WASD or a gamepad.
//! Polter's README shows the part between the marks; a test checks that it still does.
//! `cargo run --release --example glow`

use polter::action::Actions;
use polter::{Canvas, Ctx, Game, Invariants, State, math, paint};

// readme: start
/// One moment of the game, all of it: so it can be copied, replayed and rewound.
#[derive(Clone)]
struct Glow {
    x: f32,
    y: f32,
    ticks: u32,
    keys: Actions,
}

impl Game for Glow {
    fn update(&mut self, ctx: &mut Ctx) {
        self.ticks += 1;
        let keys = &self.keys;
        let (dx, dy) = (
            keys.axis(&ctx.input, "left", "right"),
            keys.axis(&ctx.input, "up", "down"),
        );
        self.x = (self.x + dx as f32 * 1.5).clamp(0.0, 319.0);
        self.y = (self.y + dy as f32 * 1.5).clamp(0.0, 179.0);
    }

    fn draw(&self, c: &mut Canvas, _ctx: &Ctx) {
        c.clear(0x101820);
        // polter::math, not f32::sin: the same bits on every machine
        let radius = 9.0 + 2.0 * math::sin(self.ticks as f32 / 24.0);
        for dy in -12..=12 {
            for dx in -12..=12 {
                let d = ((dx * dx + dy * dy) as f32).sqrt() / radius;
                let light = (1.0 - d).max(0.0);
                c.set(
                    self.x as i32 + dx,
                    self.y as i32 + dy,
                    paint::mix(0x101820, 0xffd447, light * light),
                );
            }
        }
    }

    fn state(&self) -> State {
        State::new().num("x", self.x).num("y", self.y)
    }

    fn invariants(&self, check: &mut Invariants) {
        check.require(
            "the glow stays on the screen",
            self.x < 320.0 && self.y < 180.0,
        );
    }

    fn snapshot(&self) -> Option<Box<dyn Game>> {
        Some(Box::new(self.clone()))
    }
}
// readme: end

fn main() {
    let keys = "left: a left button:left\nright: d right button:right\nup: w up button:up\ndown: s down button:down\n";
    let keys = Actions::parse(keys).expect("the keys above");
    polter::app::play(
        "glow",
        Box::new(Glow {
            x: 160.0,
            y: 90.0,
            ticks: 0,
            keys,
        }),
    );
}
