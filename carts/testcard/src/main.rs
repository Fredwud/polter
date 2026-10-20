// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! TESTCARD: every colour, every glyph, every button. The first thing to look at on a new machine.

use polter::{Button, Cart, Col, Ctx, Key, Mouse, Screen, State, font};

#[derive(Default)]
pub struct Testcard {
    mask: u8,
    frame: u64,
    mouse: (i32, i32),
    mouse_buttons: u8,
    keys: Vec<String>,
}

impl Cart for Testcard {
    fn update(&mut self, ctx: &mut Ctx) {
        self.mask = ctx.input.mask();
        self.frame = ctx.frame;
        self.mouse = ctx.input.mouse();
        self.mouse_buttons = [Mouse::LEFT, Mouse::RIGHT, Mouse::MIDDLE]
            .iter()
            .filter(|&&b| ctx.input.mouse_held(b))
            .fold(0, |a, b| a | b);
        self.keys = Key::ALL
            .into_iter()
            .filter(|&k| ctx.input.key(k))
            .map(|k| k.name())
            .collect();
    }

    fn draw(&self, s: &mut Screen, ctx: &Ctx) {
        s.cls(Col::Gunmetal);
        s.text("POLTER TESTCARD", 8, 6, Col::Shine);
        s.text(&format!("FRAME {}", ctx.frame), 260, 6, Col::Visor);

        for (i, &c) in Col::ALL.iter().enumerate() {
            let x = 8 + i as i32 * 19;
            s.rect_fill(x, 16, 17, 40, c);
            s.rect(x, 16, 17, 40, Col::Black);
            let ink = if matches!(c, Col::Black | Col::Gunmetal | Col::Bark | Col::Deep) {
                Col::Shine
            } else {
                Col::Black
            };
            s.text(&format!("{i}"), x + 2, 18, ink);
        }

        let chars: Vec<char> = font::charset().chars().collect();
        for (row, chunk) in chars.chunks(36).enumerate() {
            s.text(
                &chunk.iter().collect::<String>(),
                8,
                70 + row as i32 * 8,
                Col::Shine,
            );
        }
        s.text(
            "The quick brown fox jumps over the lazy dog.",
            8,
            96,
            Col::Chrome,
        );
        s.text("Unknown glyphs look like this: § ^", 8, 104, Col::Steel);

        for (i, b) in Button::ALL.iter().enumerate() {
            let x = 8 + i as i32 * 30;
            let on = self.mask & b.bit() != 0;
            s.rect_fill(x, 124, 26, 11, if on { Col::Ember } else { Col::Steel });
            s.text(&b.name().to_uppercase(), x + 3, 127, Col::Black);
        }

        s.line(196, 118, 312, 172, Col::Visor);
        s.circ(250, 150, 18, Col::Shine);
        s.circ_fill(250, 150, 9, Col::Ember);
        s.rect(200, 120, 30, 20, Col::Beret);
        for y in 0..12 {
            for x in 0..24 {
                if (x + y) % 2 == 0 {
                    s.pset(282 + x, 120 + y, Col::Chrome);
                }
            }
        }

        // Mouse and keyboard.
        let mb = ["L", "R", "M"]
            .iter()
            .zip([Mouse::LEFT, Mouse::RIGHT, Mouse::MIDDLE])
            .filter(|(_, b)| self.mouse_buttons & b != 0)
            .map(|(n, _)| *n)
            .collect::<String>();
        s.text(
            &format!("MOUSE {},{} {mb}", self.mouse.0, self.mouse.1),
            8,
            140,
            Col::Sky,
        );
        s.text(&format!("KEYS {}", self.keys.join(" ")), 8, 148, Col::Sky);
        let (mx, my) = self.mouse;
        let c = if self.mouse_buttons != 0 {
            Col::Ember
        } else {
            Col::Shine
        };
        s.line(mx - 4, my, mx - 2, my, c);
        s.line(mx + 2, my, mx + 4, my, c);
        s.line(mx, my - 4, mx, my - 2, c);
        s.line(mx, my + 2, mx, my + 4, c);
    }

    fn wants_mouse(&self) -> bool {
        true
    }

    fn state(&self) -> State {
        State::new()
            .int("frame", self.frame as i64)
            .text("held", &Button::names(self.mask).join(" "))
            .int("mouse_x", self.mouse.0 as i64)
            .int("mouse_y", self.mouse.1 as i64)
            .int("mouse_buttons", self.mouse_buttons as i64)
            .text("keys", &self.keys.join(" "))
    }
}

fn main() -> std::process::ExitCode {
    polter_dev::cli::run(&[polter_dev::cli::CartInfo {
        name: "testcard",
        about: "Palette, font and button test pattern.",
        make: || Box::new(Testcard::default()),
    }])
}
