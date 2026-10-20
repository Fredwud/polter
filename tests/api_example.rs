// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The example cart from API.md, verbatim, so the documentation can't drift from the API.

use polter::{Button, Cart, Col, Ctx, Machine, Screen, State};

pub struct Hello {
    x: i32,
}

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

#[test]
fn the_api_example_runs() {
    let mut m = Machine::new(Box::new(Hello { x: 10 }), 1);
    for _ in 0..5 {
        m.step(Button::Right.bit());
    }
    assert_eq!(m.state_json(), r#"{"x":15}"#);
    assert_eq!(
        m.screen().pget(15, 80),
        Col::Shine,
        "the H of HELLO should start at x = 15"
    );
}

// The true-colour example from API.md: the same code, formatted by rustfmt, with the field public
// so the test can make one.
mod glow {
    use polter::{Canvas, Ctx, Game, Invariants, State};

    pub struct Glow {
        pub t: u32,
    }

    impl Game for Glow {
        fn update(&mut self, _ctx: &mut Ctx) {
            self.t += 1;
        }
        fn draw(&self, c: &mut Canvas, _ctx: &Ctx) {
            c.clear(0x101820);
            c.set((self.t % 64) as i32, 8, 0xffd447);
        }
        fn size(&self) -> (usize, usize) {
            (64, 16)
        } // default 320 × 180
        fn state(&self) -> State {
            State::new().int("t", self.t as i64)
        }
        fn invariants(&self, check: &mut Invariants) {
            check.require("t grows", self.t > 0);
        }
    }
}

#[test]
fn the_true_colour_example_runs() {
    let mut s = polter::Session::new(Box::new(glow::Glow { t: 0 }), 1);
    for _ in 0..3 {
        s.step(0);
    }
    assert_eq!(s.state_json(), r#"{"t":3}"#);
    assert_eq!(s.canvas.get(3, 8), 0xffd447);
    assert!(s.broken.is_empty());
}
