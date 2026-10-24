// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Gamepads press the console's buttons: the pad and the left stick are the directions, the
//! lower face button is A, the right one B. So a game made for the keyboard plays with a pad as
//! it is. And a game that wants the whole pad reads its own buttons, both sticks and both triggers
//! (`Input::pad`, `stick`, `trigger`): all of a DualShock 4, the left stick pressed lightly to
//! walk slowly, as the owner asked on 22 October 2026. What a pad does is recorded
//! and replayed like any other input (the window turns it into the tick's `Frame`). Read through
//! `gilrs`, with the feature `gamepad` (on by default). Not through gilrs on a Mac: the touchpad,
//! the gyro, the light bar and rumble.

use crate::input::{Button, PadButton, PadFrame};

/// What a gamepad is doing, as far as the console's buttons care.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadState {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    /// The lower and the right face button (A and B on most pads, cross and circle on some).
    pub south: bool,
    pub east: bool,
    /// The left stick, x to the right and y up, each from −1 to 1.
    pub stick: (f32, f32),
}

/// How far the stick must lean before it counts as a direction.
pub const STICK: f32 = 0.4;

/// The console buttons a pad frame presses (the same as `buttons_of` for the state it holds).
pub fn console_of(p: &PadFrame) -> u8 {
    let held = |b: PadButton| p.buttons & b.bit() != 0;
    buttons_of(&PadState {
        left: held(PadButton::Left),
        right: held(PadButton::Right),
        up: held(PadButton::Up),
        down: held(PadButton::Down),
        south: held(PadButton::South),
        east: held(PadButton::East),
        stick: (p.sticks[0] as f32 / 127.0, p.sticks[1] as f32 / 127.0),
    })
}

/// Several pads' states as one: their buttons together, of each stick and trigger the one leant
/// furthest.
pub fn together(states: &[PadFrame]) -> PadFrame {
    let mut all = PadFrame::default();
    for s in states {
        all.buttons |= s.buttons;
        for (a, &v) in all.sticks.iter_mut().zip(&s.sticks) {
            if v.unsigned_abs() > a.unsigned_abs() {
                *a = v;
            }
        }
        for (a, &v) in all.triggers.iter_mut().zip(&s.triggers) {
            *a = (*a).max(v);
        }
    }
    all
}

/// The console buttons a pad's state presses.
pub fn buttons_of(s: &PadState) -> u8 {
    let mut b = 0;
    let mut press = |on: bool, button: Button| {
        if on {
            b |= button.bit();
        }
    };
    press(s.left || s.stick.0 < -STICK, Button::Left);
    press(s.right || s.stick.0 > STICK, Button::Right);
    press(s.up || s.stick.1 > STICK, Button::Up);
    press(s.down || s.stick.1 < -STICK, Button::Down);
    press(s.south, Button::A);
    press(s.east, Button::B);
    b
}

/// The gamepads plugged in (none, if they can't be read here).
pub struct Pads {
    #[cfg(feature = "gamepad")]
    gilrs: Option<gilrs::Gilrs>,
}

impl Pads {
    pub fn open() -> Pads {
        Pads {
            #[cfg(feature = "gamepad")]
            gilrs: gilrs::Gilrs::new().ok(),
        }
    }

    /// The names of the pads plugged in, as they call themselves.
    pub fn names(&mut self) -> Vec<String> {
        #[cfg(feature = "gamepad")]
        if let Some(g) = &mut self.gilrs {
            while g.next_event().is_some() {}
            return g
                .gamepads()
                .map(|(_, pad)| pad.name().to_string())
                .collect();
        }
        Vec::new()
    }

    /// What all pads do together now: the console buttons they press, and their own buttons,
    /// sticks and triggers.
    pub fn read(&mut self) -> (u8, PadFrame) {
        #[cfg(feature = "gamepad")]
        if let Some(g) = &mut self.gilrs {
            use gilrs::{Axis, Button as B};
            while g.next_event().is_some() {}
            let own = [
                (B::South, PadButton::South),
                (B::East, PadButton::East),
                (B::West, PadButton::West),
                (B::North, PadButton::North),
                (B::LeftTrigger, PadButton::L1),
                (B::RightTrigger, PadButton::R1),
                (B::LeftTrigger2, PadButton::L2),
                (B::RightTrigger2, PadButton::R2),
                (B::Select, PadButton::Select),
                (B::Start, PadButton::Start),
                (B::Mode, PadButton::Mode),
                (B::LeftThumb, PadButton::L3),
                (B::RightThumb, PadButton::R3),
                (B::DPadUp, PadButton::Up),
                (B::DPadDown, PadButton::Down),
                (B::DPadLeft, PadButton::Left),
                (B::DPadRight, PadButton::Right),
            ];
            let states: Vec<PadFrame> = g
                .gamepads()
                .map(|(_, pad)| {
                    let buttons = own
                        .iter()
                        .filter(|(b, _)| pad.is_pressed(*b))
                        .fold(0, |m, (_, p)| m | p.bit());
                    // a trigger is a button with a value, or an axis, as the pad's mapping has it
                    let trigger = |b: B, a: Axis| {
                        pad.button_data(b)
                            .map_or(0.0, |d| d.value())
                            .max(pad.value(a))
                    };
                    PadFrame {
                        buttons,
                        sticks: [
                            PadFrame::step(pad.value(Axis::LeftStickX)),
                            PadFrame::step(pad.value(Axis::LeftStickY)),
                            PadFrame::step(pad.value(Axis::RightStickX)),
                            PadFrame::step(pad.value(Axis::RightStickY)),
                        ],
                        triggers: [
                            PadFrame::trigger_step(trigger(B::LeftTrigger2, Axis::LeftZ)),
                            PadFrame::trigger_step(trigger(B::RightTrigger2, Axis::RightZ)),
                        ],
                    }
                })
                .collect();
            let pad = together(&states);
            return (console_of(&pad), pad);
        }
        (0, PadFrame::default())
    }

    /// The console buttons all pads press together now.
    pub fn buttons(&mut self) -> u8 {
        self.read().0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pad_presses_the_console_buttons() {
        assert_eq!(buttons_of(&PadState::default()), 0);
        let pad = PadState {
            left: true,
            south: true,
            ..PadState::default()
        };
        assert_eq!(buttons_of(&pad), Button::Left.bit() | Button::A.bit());
        // the stick: up is positive; a light touch is nothing
        let lean = PadState {
            stick: (0.9, 0.5),
            east: true,
            ..PadState::default()
        };
        assert_eq!(
            buttons_of(&lean),
            Button::Right.bit() | Button::Up.bit() | Button::B.bit()
        );
        let light = PadState {
            stick: (-0.2, -0.39),
            ..PadState::default()
        };
        assert_eq!(buttons_of(&light), 0);
        let down_left = PadState {
            stick: (-0.5, -0.8),
            ..PadState::default()
        };
        assert_eq!(
            buttons_of(&down_left),
            Button::Left.bit() | Button::Down.bit()
        );
    }

    #[test]
    fn two_pads_play_as_one_and_a_pad_frame_presses_the_same_console_buttons() {
        let a = PadFrame {
            buttons: PadButton::South.bit(),
            sticks: [100, -20, 0, 5],
            triggers: [10, 200],
        };
        let b = PadFrame {
            buttons: PadButton::Left.bit(),
            sticks: [-120, 10, 0, -6],
            triggers: [40, 0],
        };
        let both = together(&[a, b]);
        assert_eq!(both.buttons, PadButton::South.bit() | PadButton::Left.bit());
        assert_eq!(
            both.sticks,
            [-120, -20, 0, -6],
            "each the one leant furthest"
        );
        assert_eq!(both.triggers, [40, 200]);
        assert_eq!(together(&[]), PadFrame::default());
        // the arrows, the left stick past STICK, south and east: as before
        assert_eq!(console_of(&a), Button::Right.bit() | Button::A.bit());
        assert_eq!(
            console_of(&both),
            Button::Left.bit() | Button::A.bit(),
            "the stick leans left, and the arrow says left too"
        );
        let light = PadFrame {
            sticks: [PadFrame::step(0.39), PadFrame::step(-0.39), 127, 127],
            ..PadFrame::default()
        };
        assert_eq!(
            console_of(&light),
            0,
            "a light lean and the right stick press nothing"
        );
    }

    #[test]
    fn every_pad_button_has_one_name_and_keeps_its_bit() {
        let mut names = std::collections::BTreeSet::new();
        for b in PadButton::ALL {
            assert!(names.insert(b.name()), "{} named twice", b.name());
            assert_eq!(PadButton::from_name(&b.name().to_uppercase()), Some(b));
        }
        assert_eq!(names.len(), 17);
        // recordings keep their meaning: new buttons only ever at the end
        assert_eq!(PadButton::South.bit(), 1);
        assert_eq!(PadButton::R2.bit(), 1 << 7);
        assert_eq!(PadButton::Right.bit(), 1 << 16);
    }

    #[test]
    fn the_pads_answer_with_console_buttons_only() {
        // whatever is plugged in (none, or a pad someone holds), only the six buttons come back
        let all = Button::ALL.iter().fold(0, |a, b| a | b.bit());
        let mut pads = Pads::open();
        assert_eq!(pads.buttons() & !all, 0);
        let (buttons, pad) = pads.read();
        assert_eq!(buttons & !all, 0);
        assert_eq!(
            buttons,
            console_of(&pad),
            "the console buttons are the pad's"
        );
        let _ = pads.names();
    }
}
