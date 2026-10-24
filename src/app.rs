// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Playing a game in a window: what a shipped game runs, and what `play` builds on. The console's
//! buttons come from the keys (arrows, Z, X), the window's keys work (Esc quits a game that doesn't
//! want the mouse; F1, or P for those, switches its look), sound goes to the speakers, the picture
//! follows the window's size. Tools hang on through [`Along`]: a script playing along, a recording,
//! reports.

use crate::canvas::Canvas;
use crate::game::{Game, Session};
use crate::input::{Button, Frame, Key};
use crate::storage::Storage;
use crate::window::{Conf, Host, Io};

/// What tools add while a game plays. Every method has a default that does nothing.
pub trait Along {
    /// The input of the next tick (`frame` is the tick it will be), from what the player does: a
    /// script may add to it.
    fn input(&mut self, _frame: u64, from_player: Frame) -> Frame {
        from_player
    }
    /// After every tick, with the input it ran on. False closes the window.
    fn after(&mut self, _session: &Session, _input: &Frame) -> bool {
        true
    }
    /// The window is closing: save now.
    fn closing(&mut self) {}
}

/// Nothing along: a game as a player gets it.
struct Nothing;

impl Along for Nothing {}

/// A game in the window.
pub struct Player {
    session: Session,
    name: String,
    /// It draws its own cursor and may use Escape and every key.
    desktop: bool,
    audio: Option<crate::speaker::Speaker>,
    along: Box<dyn Along>,
}

impl Player {
    pub fn new(name: &str, session: Session, along: Option<Box<dyn Along>>) -> Player {
        let desktop = session.game().wants_mouse();
        Player {
            session,
            name: name.to_string(),
            desktop,
            audio: None,
            along: along.unwrap_or(Box::new(Nothing)),
        }
    }

    /// Sound through the speakers, if there are any (no sound is never an error).
    pub fn with_sound(mut self) -> Player {
        self.audio = crate::speaker::Speaker::open();
        if self.audio.is_some() {
            self.session.enable_audio();
        }
        self
    }

    /// The session being played, for a window that does more around it (pictures, a menu's
    /// quit).
    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    /// How the window should open for this game; its full screen remembered in `remember`.
    pub fn conf(&self, remember: Option<std::path::PathBuf>) -> Conf {
        Conf {
            title: self.name.clone(),
            size: self.session.game().size(),
            fit: self.session.game().fit(),
            remember,
            hide_cursor: self.desktop,
        }
    }
}

impl Host for Player {
    fn frame(&mut self, io: &Io, canvas: &mut Canvas) -> bool {
        // Desktop games get Escape for their own menus; they quit with Cmd+Q or the close button.
        if !self.desktop && io.input.key(Key::Escape) {
            return false;
        }
        if (io.input.key_pressed(Key::F1) || (!self.desktop && io.input.key_pressed(Key::P)))
            && let Some(look) = self.session.game_mut().next_look()
        {
            println!("{} · {look}", self.name);
        }
        // a growing picture follows the window's size, before this frame's ticks draw it
        let resized = (self.session.canvas.width(), self.session.canvas.height())
            != (canvas.width(), canvas.height());
        self.session.resize(canvas.width(), canvas.height());
        let held = |k: Key| io.frame.keys & k.bit() != 0;
        let buttons = [
            (Key::Left, Button::Left),
            (Key::Right, Button::Right),
            (Key::Up, Button::Up),
            (Key::Down, Button::Down),
            (Key::Z, Button::A),
            (Key::X, Button::B),
        ]
        .iter()
        .filter(|(k, _)| held(*k))
        .fold(0, |acc, (_, b)| acc | b.bit());
        for tick in 0..io.ticks {
            let from_player = Frame {
                // the keys' buttons and a gamepad's (the window reads the pads)
                buttons: buttons | io.frame.buttons,
                mouse: io.frame.mouse,
                mouse_buttons: io.frame.mouse_buttons,
                // the wheel turned once in this display frame, however many ticks it has
                wheel: if tick == 0 {
                    io.frame.wheel.clamp(-8, 8)
                } else {
                    0
                },
                keys: io.frame.keys,
                pad: io.frame.pad,
            };
            let input = self.along.input(self.session.ctx.frame + 1, from_player);
            // only the last tick is shown, so only it need be painted (`Ctx::picture`)
            self.session.ctx.picture = tick + 1 == io.ticks;
            self.session.step_frame(&input);
            if let Some(out) = &self.audio {
                out.push(&self.session.audio);
            }
            if !self.along.after(&self.session, &input) {
                return false;
            }
        }
        // shown only once drawn at the new size: a resize without a tick keeps the old picture
        if !(resized && io.ticks == 0)
            && canvas.pixels().len() == self.session.canvas.pixels().len()
        {
            canvas
                .pixels_mut()
                .copy_from_slice(self.session.canvas.pixels());
        }
        true
    }

    fn closing(&mut self) {
        self.along.closing();
    }
}

/// Plays a game in a window until it closes: a shipped game's `main`. Its saves go to the
/// system's folder for the game's data, the window remembers full screen there, the seed comes
/// from the clock (a game played is not a test).
#[allow(
    clippy::disallowed_methods,
    reason = "a shipped game's seed: each play starts differently; tools play with a fixed seed"
)]
pub fn play(name: &str, game: Box<dyn Game>) {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64 | 1);
    let mut session = Session::new(game, seed);
    session.ctx.storage = Storage::for_cart(name);
    let player = Player::new(name, session, None).with_sound();
    let conf = player.conf(Some(crate::storage::save_dir(name).join("window.txt")));
    crate::window::run(conf, Box::new(player));
}

/// The same for a console cart, in its first palette.
pub fn play_cart(name: &str, cart: Box<dyn crate::machine::Cart>) {
    play(name, Box::new(crate::console::Console::new(cart)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Ctx;
    use crate::input::Input;
    use crate::state::State;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A dot that runs right while Right is held.
    struct Dot {
        x: i32,
    }

    impl Game for Dot {
        fn update(&mut self, ctx: &mut Ctx) {
            if ctx.input.held(Button::Right) {
                self.x += 1;
            }
        }
        fn draw(&self, c: &mut Canvas, _ctx: &Ctx) {
            c.clear(0);
            c.set(self.x, 0, 0xffffff);
        }
        fn size(&self) -> (usize, usize) {
            (16, 4)
        }
        fn state(&self) -> State {
            State::new().int("x", self.x as i64)
        }
    }

    /// Notes every input, adds A on tick 2, closes after `stop` ticks.
    struct Spy {
        seen: Rc<RefCell<Vec<Frame>>>,
        stop: u64,
    }

    impl Along for Spy {
        fn input(&mut self, frame: u64, from_player: Frame) -> Frame {
            let mut f = from_player;
            if frame == 2 {
                f.buttons |= Button::A.bit();
            }
            f
        }
        fn after(&mut self, session: &Session, input: &Frame) -> bool {
            self.seen.borrow_mut().push(*input);
            session.ctx.frame < self.stop
        }
    }

    fn frame(player: &mut Player, input: &mut Input, keys: &[Key], ticks: u32) -> bool {
        let f = Frame {
            keys: keys.iter().fold(0, |a, k| a | k.bit()),
            ..Frame::default()
        };
        input.advance(&f);
        let io = Io {
            ticks,
            frame: f,
            input,
            typed: &[],
            full_screen: false,
        };
        let mut canvas = Canvas::new(16, 4);
        let open = player.frame(&io, &mut canvas);
        // a frame that closes the window shows nothing more
        if open {
            assert_eq!(
                canvas.hash(),
                player.session.canvas.hash(),
                "the picture is shown"
            );
        }
        open
    }

    #[test]
    fn a_game_plays_with_keys_as_buttons_and_tools_along() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let spy = Spy {
            seen: seen.clone(),
            stop: 100,
        };
        let session = Session::new(Box::new(Dot { x: 0 }), 1);
        let mut player = Player::new("dot", session, Some(Box::new(spy)));
        let mut input = Input::default();
        // the right arrow is the console's Right; two ticks in one display frame
        assert!(frame(&mut player, &mut input, &[Key::Right], 2));
        assert_eq!(player.session.state_json(), r#"{"x":2}"#);
        let seen = seen.borrow();
        assert_eq!(seen.len(), 2);
        assert!(seen.iter().all(|f| f.buttons & Button::Right.bit() != 0));
        assert!(
            seen[1].buttons & Button::A.bit() != 0,
            "the tool added A on tick 2"
        );
        drop(seen);
        // no tick due: nothing moves
        assert!(frame(&mut player, &mut input, &[Key::Right], 0));
        assert_eq!(player.session.state_json(), r#"{"x":2}"#);
        // Escape closes a game that doesn't want the mouse
        assert!(!frame(&mut player, &mut input, &[Key::Escape], 1));
    }

    /// Counts the wheel and the pictures it paints.
    struct Counter {
        wheel: i32,
        painted: std::cell::Cell<u32>,
    }

    impl Game for Counter {
        fn update(&mut self, ctx: &mut Ctx) {
            self.wheel += ctx.input.wheel();
        }
        fn draw(&self, _c: &mut Canvas, ctx: &Ctx) {
            if ctx.picture {
                self.painted.set(self.painted.get() + 1);
            }
        }
        fn size(&self) -> (usize, usize) {
            (16, 4)
        }
    }

    #[test]
    fn a_display_frame_turns_the_wheel_once_and_paints_its_last_tick() {
        let game = Counter {
            wheel: 0,
            painted: std::cell::Cell::new(0),
        };
        let mut player = Player::new("counter", Session::new(Box::new(game), 1), None);
        let mut input = Input::default();
        let f = Frame {
            wheel: 1,
            ..Frame::default()
        };
        input.advance(&f);
        let io = Io {
            ticks: 3,
            frame: f,
            input: &input,
            typed: &[],
            full_screen: false,
        };
        player.frame(&io, &mut Canvas::new(16, 4));
        let c = player.session().game_as::<Counter>().unwrap();
        assert_eq!(c.wheel, 1, "one turn, not one a tick");
        assert_eq!(c.painted.get(), 1, "three ticks, one picture");
    }

    #[test]
    fn a_gamepads_buttons_reach_the_game() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let spy = Spy {
            seen: seen.clone(),
            stop: 100,
        };
        let mut player = Player::new(
            "dot",
            Session::new(Box::new(Dot { x: 0 }), 1),
            Some(Box::new(spy)),
        );
        // the window puts the pads' buttons in its frame; no key is held
        let f = Frame::buttons(Button::Right.bit());
        let mut input = Input::default();
        input.advance(&f);
        let io = Io {
            ticks: 2,
            frame: f,
            input: &input,
            typed: &[],
            full_screen: false,
        };
        player.frame(&io, &mut Canvas::new(16, 4));
        assert_eq!(
            player.session.state_json(),
            r#"{"x":2}"#,
            "the pad's right moved it"
        );
        assert!(
            seen.borrow()
                .iter()
                .all(|f| f.buttons & Button::Right.bit() != 0)
        );
    }

    #[test]
    fn a_tool_can_close_the_window() {
        let spy = Spy {
            seen: Rc::new(RefCell::new(Vec::new())),
            stop: 3,
        };
        let session = Session::new(Box::new(Dot { x: 0 }), 1);
        let mut player = Player::new("dot", session, Some(Box::new(spy)));
        let mut input = Input::default();
        assert!(frame(&mut player, &mut input, &[], 2));
        assert!(!frame(&mut player, &mut input, &[], 2), "closed at tick 3");
        assert_eq!(player.session.ctx.frame, 3);
    }
}
