// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Actions: a game asks for "jump", not for the space bar. Each action is bound to keys, mouse
//! buttons, the console's buttons (which a gamepad presses too: its pad and left stick are
//! the directions, its lower face button A, its right one B) and a gamepad's own buttons
//! (`pad:south`, `pad:r1`, `pad:r2` …; `PadButton`). Bindings are text, one action a line, so a
//! player or an agent changes them in a file:
//!
//! ```text
//! # actions
//! left:   a left button:left
//! right:  d right button:right
//! jump:   space w up button:a
//! fire:   mouse:left button:b pad:r2
//! sprint: shift pad:l3
//! ```
//!
//! How far a stick leans or a trigger is pressed is no action; a game reads it from the `Input`
//! (`stick`, `trigger`).
//!
//! Actions are plain data and read the tick's `Input`, so they record and replay with the game.

use crate::input::{Button, Input, Key, Mouse, PadButton};

/// One thing that can trigger an action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bind {
    Key(Key),
    /// A mouse button (`Mouse::LEFT`, `Mouse::RIGHT`, `Mouse::MIDDLE`).
    Mouse(u8),
    /// A console button: arrows, Z and X on the keyboard, and the gamepad.
    Button(Button),
    /// A gamepad's own button.
    Pad(PadButton),
}

impl Bind {
    fn held(self, input: &Input) -> bool {
        match self {
            Bind::Key(k) => input.key(k),
            Bind::Mouse(m) => input.mouse_held(m),
            Bind::Button(b) => input.held(b),
            Bind::Pad(b) => input.pad(b),
        }
    }

    fn pressed(self, input: &Input) -> bool {
        match self {
            Bind::Key(k) => input.key_pressed(k),
            Bind::Mouse(m) => input.mouse_pressed(m),
            Bind::Button(b) => input.pressed(b),
            Bind::Pad(b) => input.pad_pressed(b),
        }
    }

    fn released(self, input: &Input) -> bool {
        match self {
            Bind::Key(k) => input.key_released(k),
            Bind::Mouse(m) => input.mouse_released(m),
            Bind::Button(b) => input.released(b),
            Bind::Pad(b) => input.pad_released(b),
        }
    }

    /// The bind as it is written: `space`, `mouse:left`, `button:a`, `pad:south`.
    pub fn name(self) -> String {
        match self {
            Bind::Key(k) => k.name(),
            Bind::Mouse(m) => format!(
                "mouse:{}",
                match m {
                    Mouse::RIGHT => "right",
                    Mouse::MIDDLE => "middle",
                    _ => "left",
                }
            ),
            Bind::Button(b) => format!("button:{}", b.name()),
            Bind::Pad(b) => format!("pad:{}", b.name()),
        }
    }

    /// A bind from how it is written (case does not matter).
    pub fn from_name(s: &str) -> Option<Bind> {
        let s = s.to_lowercase();
        if let Some(m) = s.strip_prefix("mouse:") {
            return match m {
                "left" => Some(Bind::Mouse(Mouse::LEFT)),
                "right" => Some(Bind::Mouse(Mouse::RIGHT)),
                "middle" => Some(Bind::Mouse(Mouse::MIDDLE)),
                _ => None,
            };
        }
        if let Some(b) = s.strip_prefix("button:") {
            return Button::from_name(b).map(Bind::Button);
        }
        if let Some(b) = s.strip_prefix("pad:") {
            return PadButton::from_name(b).map(Bind::Pad);
        }
        Key::from_name(&s).map(Bind::Key)
    }
}

/// Actions by name, each with its binds, in the order they were made.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Actions {
    binds: Vec<(String, Vec<Bind>)>,
}

impl Actions {
    pub fn new() -> Actions {
        Actions::default()
    }

    /// Adds an action (or replaces its binds).
    pub fn bind(mut self, name: &str, binds: &[Bind]) -> Actions {
        self.rebind(name, binds);
        self
    }

    /// Replaces an action's binds (adds it if it is new).
    pub fn rebind(&mut self, name: &str, binds: &[Bind]) {
        match self.binds.iter_mut().find(|(n, _)| n == name) {
            Some((_, b)) => *b = binds.to_vec(),
            None => self.binds.push((name.into(), binds.to_vec())),
        }
    }

    /// Reads bindings written as text: `name: bind bind …` a line, `#` starts a comment. Errors
    /// name the line and what it can't read.
    pub fn parse(text: &str) -> Result<Actions, String> {
        let mut out = Actions::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let Some((name, rest)) = line.split_once(':') else {
                return Err(format!("line {}: wants `name: binds`, got {line:?}", n + 1));
            };
            let name = name.trim();
            if name.is_empty() || name.contains(char::is_whitespace) {
                return Err(format!(
                    "line {}: an action's name is one word, got {name:?}",
                    n + 1
                ));
            }
            let mut binds = Vec::new();
            for word in rest.split_whitespace() {
                binds.push(Bind::from_name(word).ok_or(format!(
                    "line {}: {word:?} is no key, mouse:left/right/middle, button:left/right/up/down/a/b or pad:south/east/west/north/l1/r1/l2/r2/select/start/mode/l3/r3/up/down/left/right",
                    n + 1
                ))?);
            }
            out.rebind(name, &binds);
        }
        Ok(out)
    }

    /// The bindings as text, as `parse` reads them.
    pub fn to_text(&self) -> String {
        self.binds
            .iter()
            .map(|(name, binds)| {
                let b: Vec<String> = binds.iter().map(|b| b.name()).collect();
                format!("{name}: {}\n", b.join(" "))
            })
            .collect()
    }

    fn binds(&self, name: &str) -> &[Bind] {
        self.binds
            .iter()
            .find(|(n, _)| n == name)
            .map_or(&[], |(_, b)| b)
    }

    /// Is the action held (any of its binds)? An unknown action is never held.
    pub fn held(&self, input: &Input, name: &str) -> bool {
        self.binds(name).iter().any(|b| b.held(input))
    }

    /// Did the action start this tick (a bind pressed while none was held before)?
    pub fn pressed(&self, input: &Input, name: &str) -> bool {
        let binds = self.binds(name);
        binds.iter().any(|b| b.pressed(input))
            && !binds.iter().any(|b| b.held(input) && !b.pressed(input))
    }

    /// Did the action end this tick (its last bind let go)?
    pub fn released(&self, input: &Input, name: &str) -> bool {
        let binds = self.binds(name);
        binds.iter().any(|b| b.released(input)) && !binds.iter().any(|b| b.held(input))
    }

    /// -1, 0 or 1 from two opposite actions (left, right): both or neither held is 0.
    pub fn axis(&self, input: &Input, negative: &str, positive: &str) -> i32 {
        self.held(input, positive) as i32 - self.held(input, negative) as i32
    }

    /// The actions' names, in order.
    pub fn names(&self) -> Vec<&str> {
        self.binds.iter().map(|(n, _)| n.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Frame;

    const TEXT: &str = "# a platformer's keys\n\
        left: a left button:left\n\
        right: d right button:right\n\
        jump: space W button:a   # W in capitals reads too\n\
        fire: mouse:left button:b pad:R2\n\
        sprint: shift pad:l3\n";

    fn input_after(frames: &[Frame]) -> Input {
        let mut input = Input::default();
        for f in frames {
            input.advance(f);
        }
        input
    }

    #[test]
    fn bindings_read_from_text_and_write_back_the_same() {
        let a = Actions::parse(TEXT).unwrap();
        assert_eq!(a.names(), ["left", "right", "jump", "fire", "sprint"]);
        assert_eq!(Actions::parse(&a.to_text()).unwrap(), a);
        for (text, says) in [
            ("jump space", "wants `name: binds`"),
            ("jump: spacebar", "\"spacebar\" is no key"),
            ("big jump: space", "one word"),
            ("fire: mouse:thumb", "is no key"),
            ("fire: pad:r4", "is no key"),
        ] {
            let err = Actions::parse(text).unwrap_err();
            assert!(
                err.contains("line 1") && err.contains(says),
                "{text:?}: {err}"
            );
        }
    }

    #[test]
    fn an_action_is_held_pressed_and_released_through_any_of_its_binds() {
        let a = Actions::parse(TEXT).unwrap();
        let space = Frame {
            keys: Key::Space.bit(),
            ..Frame::default()
        };
        let pad_a = Frame::buttons(Button::A.bit());
        let both = Frame {
            buttons: Button::A.bit(),
            ..space
        };
        // the key, then the pad's A as well, then only the pad, then nothing
        let i = input_after(&[Frame::default(), space]);
        assert!(a.held(&i, "jump") && a.pressed(&i, "jump"));
        let i = input_after(&[space, both]);
        assert!(
            a.held(&i, "jump") && !a.pressed(&i, "jump"),
            "already held: not pressed again"
        );
        let i = input_after(&[both, pad_a]);
        assert!(
            a.held(&i, "jump") && !a.released(&i, "jump"),
            "one bind still held"
        );
        let i = input_after(&[pad_a, Frame::default()]);
        assert!(!a.held(&i, "jump") && a.released(&i, "jump"));
        // the mouse
        let click = Frame {
            mouse_buttons: Mouse::LEFT,
            ..Frame::default()
        };
        assert!(a.pressed(&input_after(&[click]), "fire"));
        // an axis from two actions; both held is 0
        let left_right = Frame {
            keys: Key::A.bit() | Key::D.bit(),
            ..Frame::default()
        };
        assert_eq!(a.axis(&input_after(&[left_right]), "left", "right"), 0);
        assert_eq!(
            a.axis(
                &input_after(&[Frame::buttons(Button::Left.bit())]),
                "left",
                "right"
            ),
            -1
        );
        // a gamepad's own buttons: the trigger fires, the stick pressed in sprints
        let r2 = Frame {
            pad: crate::input::PadFrame {
                buttons: PadButton::R2.bit(),
                ..Default::default()
            },
            ..Frame::default()
        };
        assert!(a.pressed(&input_after(&[Frame::default(), r2]), "fire"));
        assert!(
            a.held(&input_after(&[r2, r2]), "fire") && !a.pressed(&input_after(&[r2, r2]), "fire"),
            "held on: not pressed again"
        );
        assert!(a.released(&input_after(&[r2, Frame::default()]), "fire"));
        assert!(!a.held(&input_after(&[r2]), "sprint"));
        // unknown actions are never anything
        assert!(!a.held(&input_after(&[space]), "dance"));
    }
}
