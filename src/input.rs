// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Input: six console buttons for every cart, plus mouse and keyboard for carts that want them.

/// A button of the console. In the window: arrow keys, Z = A, X = B.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Up,
    Down,
    A,
    B,
}

impl Button {
    pub const ALL: [Button; 6] = [
        Button::Left,
        Button::Right,
        Button::Up,
        Button::Down,
        Button::A,
        Button::B,
    ];

    /// The bit of this button in a button mask.
    pub fn bit(self) -> u8 {
        1 << self as u8
    }

    /// The name used in `.haunt` files: `left right up down a b`.
    pub fn name(self) -> &'static str {
        match self {
            Button::Left => "left",
            Button::Right => "right",
            Button::Up => "up",
            Button::Down => "down",
            Button::A => "a",
            Button::B => "b",
        }
    }

    pub fn from_name(s: &str) -> Option<Button> {
        Button::ALL
            .into_iter()
            .find(|b| b.name().eq_ignore_ascii_case(s))
    }

    /// Names of all buttons set in a mask, e.g. `["right", "a"]`.
    pub fn names(mask: u8) -> Vec<&'static str> {
        Button::ALL
            .into_iter()
            .filter(|b| mask & b.bit() != 0)
            .map(Button::name)
            .collect()
    }
}

/// Keyboard keys a game can read. `.haunt` files write them as `key:r`, `key:space`, `key:f1`.
/// New keys are only ever added at the end, so a recording keeps its meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Key {
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    Space, Enter, Tab, Backspace, Shift, Delete, Escape,
    Up, Down, Left, Right,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Ctrl, Alt, Super,
}

impl Key {
    #[rustfmt::skip]
    pub const ALL: [Key; 62] = [
        Key::A, Key::B, Key::C, Key::D, Key::E, Key::F, Key::G, Key::H, Key::I, Key::J, Key::K, Key::L, Key::M,
        Key::N, Key::O, Key::P, Key::Q, Key::R, Key::S, Key::T, Key::U, Key::V, Key::W, Key::X, Key::Y, Key::Z,
        Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9,
        Key::Space, Key::Enter, Key::Tab, Key::Backspace, Key::Shift, Key::Delete, Key::Escape,
        Key::Up, Key::Down, Key::Left, Key::Right,
        Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6, Key::F7, Key::F8, Key::F9, Key::F10, Key::F11, Key::F12,
        Key::Ctrl, Key::Alt, Key::Super,
    ];

    pub fn bit(self) -> u64 {
        1 << self as u64
    }

    /// `a`–`z`, `0`–`9`, `space`, `enter`, `tab`, `backspace`, `shift`, `delete`, `escape`,
    /// `up`, `down`, `left`, `right`, `f1`–`f12`, `ctrl`, `alt`, `super` (Cmd on a Mac).
    /// Escape reaches only games that want the mouse; for the others it closes the window.
    pub fn name(self) -> String {
        let i = self as u8;
        match self {
            Key::Up => "up".into(),
            Key::Down => "down".into(),
            Key::Left => "left".into(),
            Key::Right => "right".into(),
            Key::Ctrl => "ctrl".into(),
            Key::Alt => "alt".into(),
            Key::Super => "super".into(),
            _ if (Key::F1 as u8..=Key::F12 as u8).contains(&i) => {
                format!("f{}", i - Key::F1 as u8 + 1)
            }
            Key::Space => "space".into(),
            Key::Enter => "enter".into(),
            Key::Tab => "tab".into(),
            Key::Backspace => "backspace".into(),
            Key::Shift => "shift".into(),
            Key::Delete => "delete".into(),
            Key::Escape => "escape".into(),
            _ if i < 26 => ((b'a' + i) as char).to_string(),
            _ => ((b'0' + i - 26) as char).to_string(),
        }
    }

    pub fn from_name(s: &str) -> Option<Key> {
        Key::ALL
            .into_iter()
            .find(|k| k.name().eq_ignore_ascii_case(s))
    }
}

/// Mouse button bits.
pub struct Mouse;

impl Mouse {
    pub const LEFT: u8 = 1;
    pub const RIGHT: u8 = 2;
    pub const MIDDLE: u8 = 4;
}

/// Everything that came in for one frame: buttons, mouse and keyboard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Frame {
    /// Console buttons (`Button::bit()` values).
    pub buttons: u8,
    /// Mouse position in screen pixels.
    pub mouse: (i32, i32),
    /// Mouse buttons (`Mouse::LEFT` …).
    pub mouse_buttons: u8,
    /// Wheel steps this frame; positive is up.
    pub wheel: i32,
    /// Held keys (`Key::bit()` values).
    pub keys: u64,
}

impl Frame {
    /// A frame with only console buttons.
    pub fn buttons(mask: u8) -> Frame {
        Frame {
            buttons: mask,
            ..Frame::default()
        }
    }
}

/// The input of the current frame, with the previous frame remembered for edges.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    now: Frame,
    before: Frame,
}

impl Input {
    /// Held down this frame.
    pub fn held(&self, b: Button) -> bool {
        self.now.buttons & b.bit() != 0
    }

    /// Went down this frame (was up last frame).
    pub fn pressed(&self, b: Button) -> bool {
        self.held(b) && self.before.buttons & b.bit() == 0
    }

    /// Went up this frame (was down last frame).
    pub fn released(&self, b: Button) -> bool {
        !self.held(b) && self.before.buttons & b.bit() != 0
    }

    /// -1, 0 or 1 from two opposite buttons, e.g. `axis(Button::Left, Button::Right)`.
    pub fn axis(&self, neg: Button, pos: Button) -> i32 {
        self.held(pos) as i32 - self.held(neg) as i32
    }

    /// The raw mask of held buttons.
    pub fn mask(&self) -> u8 {
        self.now.buttons
    }

    /// Mouse position in screen pixels.
    pub fn mouse(&self) -> (i32, i32) {
        self.now.mouse
    }

    /// A mouse button (`Mouse::LEFT` …) is down this frame.
    pub fn mouse_held(&self, b: u8) -> bool {
        self.now.mouse_buttons & b != 0
    }

    /// A mouse button went down this frame.
    pub fn mouse_pressed(&self, b: u8) -> bool {
        self.mouse_held(b) && self.before.mouse_buttons & b == 0
    }

    /// A mouse button went up this frame.
    pub fn mouse_released(&self, b: u8) -> bool {
        !self.mouse_held(b) && self.before.mouse_buttons & b != 0
    }

    /// Wheel steps this frame; positive is up.
    pub fn wheel(&self) -> i32 {
        self.now.wheel
    }

    /// A key is down this frame.
    pub fn key(&self, k: Key) -> bool {
        self.now.keys & k.bit() != 0
    }

    /// A key went down this frame.
    pub fn key_pressed(&self, k: Key) -> bool {
        self.key(k) && self.before.keys & k.bit() == 0
    }

    /// A key went up this frame.
    pub fn key_released(&self, k: Key) -> bool {
        !self.key(k) && self.before.keys & k.bit() != 0
    }

    /// The next frame's input: what was held becomes "before", so `pressed` and `released` see
    /// the edges. Sessions and windows call it; so may a test that plays a person.
    pub fn advance(&mut self, f: &Frame) {
        self.before = self.now;
        self.now = *f;
    }
}
