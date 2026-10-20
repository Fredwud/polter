// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Gamepads press the console's buttons: the pad and the left stick are the directions, the
//! lower face button is A, the right one B. So a game made for the keyboard plays with a pad as
//! it is, and what a pad does is recorded and replayed like any other input (the window turns it
//! into the tick's `Frame`). Read through `gilrs`, with the feature `gamepad` (on by default).

use crate::input::Button;

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

    /// The console buttons all pads press together now.
    pub fn buttons(&mut self) -> u8 {
        #[cfg(feature = "gamepad")]
        if let Some(g) = &mut self.gilrs {
            use gilrs::{Axis, Button as B};
            // the events keep the pads' states up to date
            while g.next_event().is_some() {}
            return g
                .gamepads()
                .map(|(_, pad)| {
                    buttons_of(&PadState {
                        left: pad.is_pressed(B::DPadLeft),
                        right: pad.is_pressed(B::DPadRight),
                        up: pad.is_pressed(B::DPadUp),
                        down: pad.is_pressed(B::DPadDown),
                        south: pad.is_pressed(B::South),
                        east: pad.is_pressed(B::East),
                        stick: (pad.value(Axis::LeftStickX), pad.value(Axis::LeftStickY)),
                    })
                })
                .fold(0, |a, b| a | b);
        }
        0
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
    fn the_pads_answer_with_console_buttons_only() {
        // whatever is plugged in (none, or a pad someone holds), only the six buttons come back
        let all = Button::ALL.iter().fold(0, |a, b| a | b.bit());
        let mut pads = Pads::open();
        assert_eq!(pads.buttons() & !all, 0);
        let _ = pads.names();
    }
}
