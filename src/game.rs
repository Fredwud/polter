// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Games and sessions. A `Game` is whatever Polter runs: it updates one fixed tick at a time,
//! draws into a true-colour `Canvas`, reports its state and declares the rules that must hold on
//! every tick. A `Session` runs a game, with a window or without one; every command of the
//! command line (cli.rs) is a session.

use crate::audio::{Play, SAMPLES_PER_FRAME, Sample, Sfx, Synth};
use crate::canvas::Canvas;
use crate::input::{Frame, Input};
use crate::rng::Rng;
use crate::state::State;
use crate::storage::Storage;
use std::any::Any;

/// Ticks per second. Every tick is exactly 1/60 s of game time, whatever the real clock says.
pub const FPS: u32 = 60;

/// What a game gets every tick.
pub struct Ctx {
    /// The input of this tick.
    pub input: Input,
    /// The current tick (frame) number; the first is 1.
    pub frame: u64,
    /// The only randomness a game should use. Seeded, so runs are reproducible.
    pub rng: Rng,
    /// Save games: files in `play`, memory in headless runs.
    pub storage: Storage,
    /// Whether anyone will look at this tick's picture. In headless checks (`check`, `probe`,
    /// `assert`) most ticks are never looked at: a game may then skip filling its canvas, but
    /// should still run whatever drawing can fail. The window, `twin` and `peek` look at every
    /// picture they show.
    pub picture: bool,
    pub(crate) sounds: Vec<Sfx>,
    pub(crate) plays: Vec<Play>,
    pub(crate) hums: Vec<(&'static str, Play)>,
}

impl Ctx {
    pub(crate) fn new(seed: u64) -> Ctx {
        Ctx {
            input: Input::default(),
            frame: 0,
            rng: Rng::new(seed),
            storage: Storage::memory(),
            picture: true,
            sounds: Vec::new(),
            plays: Vec::new(),
            hums: Vec::new(),
        }
    }

    /// Starts a sound effect. Call it from `update`; up to eight sounds play at once.
    pub fn play(&mut self, sfx: Sfx) {
        self.sounds.push(sfx);
    }

    /// Plays a recording once, as it was recorded, at a volume from 0.0 to 1.0. Call it from
    /// `update`; sixteen recordings play at once, a seventeenth replaces the quietest. It is
    /// heard in the middle unless placed: the `Play` it returns takes a `pan` (where it is
    /// heard, -1.0 left to 1.0 right) and a `cutoff` (how dull, as through a wall).
    pub fn play_sample(&mut self, sample: &Sample, volume: f32) -> &mut Play {
        self.play_sample_at(sample, volume, 1.0)
    }

    /// The same at another speed: 2.0 twice as fast and an octave higher, 0.5 half as fast.
    pub fn play_sample_at(&mut self, sample: &Sample, volume: f32, rate: f32) -> &mut Play {
        let mut play = Play::new(sample, volume);
        play.rate(rate);
        self.plays.push(play);
        let last = self.plays.len() - 1;
        &mut self.plays[last]
    }

    /// Keeps a recording sounding in a loop for as long as it is asked for: call it on every
    /// tick the sound should go on (an engine running, a pump humming), with the volume and
    /// speed it should have now, and its place and dullness through the `Play` it returns. They
    /// glide there, so nothing clicks; a tick without the call fades it out. The key tells the
    /// hums apart; sixteen sound at once.
    pub fn hum(&mut self, key: &'static str, sample: &Sample, volume: f32, rate: f32) -> &mut Play {
        let mut play = Play::new(sample, volume);
        play.rate(rate);
        self.hums.push((key, play));
        let last = self.hums.len() - 1;
        &mut self.hums[last].1
    }
}

/// The rules a game declares to hold on every tick. A broken one stops `check`, `twin`,
/// `assert` and the others at that tick, with its name and the state.
#[derive(Debug, Default)]
pub struct Invariants {
    broken: Vec<String>,
    /// Every rule's name, broken or not, when a tool asks for them (`Session::rules`); on the
    /// ticks of a game only the broken ones are kept, so a rule costs nothing while it holds.
    named: Option<Vec<String>>,
}

impl Invariants {
    /// One rule: its name says what must be true ("money is never negative").
    pub fn require(&mut self, name: &str, holds: bool) {
        if let Some(named) = &mut self.named
            && !named.iter().any(|n| n == name)
        {
            named.push(name.to_string());
        }
        if !holds {
            self.broken.push(name.to_string());
        }
    }

    /// The rules that were broken.
    pub fn broken(&self) -> &[String] {
        &self.broken
    }
}

/// A tuning value a game shows in the workshop: a slider between `min` and `max`, in steps.
/// Changing one changes the game, so the workshop records each change with its frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Knob {
    pub name: &'static str,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

impl Knob {
    pub fn new(name: &'static str, value: f32, min: f32, max: f32, step: f32) -> Knob {
        Knob {
            name,
            value,
            min,
            max,
            step,
        }
    }
}

/// A game: Polter calls `update`, then checks the invariants, then calls `draw`, sixty times
/// a second.
pub trait Game: Any {
    /// All game logic: one tick.
    fn update(&mut self, ctx: &mut Ctx);
    /// Draws the current state. It can't change the game: `draw` only gets `&self`.
    fn draw(&self, canvas: &mut Canvas, ctx: &Ctx);
    /// The picture's size in pixels (the window scales it by whole numbers); with
    /// `Fit::Expand`, the smallest it may be.
    fn size(&self) -> (usize, usize) {
        (crate::WIDTH as usize, crate::HEIGHT as usize)
    }
    /// How the picture fills a window or the screen (window.rs). With `Expand` the canvas
    /// grows to fill it: `draw` reads `canvas.width()` and `height()`, and `update` must not
    /// depend on them, so replays stay the same in any window.
    fn fit(&self) -> crate::window::Fit {
        crate::window::Fit::Fixed
    }
    /// What `probe`, `twin` and scenarios see: everything that matters for testing.
    fn state(&self) -> State {
        State::new()
    }
    /// The rules that must hold after every tick.
    fn invariants(&self, _check: &mut Invariants) {}
    /// Whether the game draws its own mouse cursor; the window then hides the system's.
    fn wants_mouse(&self) -> bool {
        false
    }
    /// Switches to the game's next look (F1 in the window) and names it; `None` if the game
    /// has only one.
    fn next_look(&mut self) -> Option<String> {
        None
    }
    /// Characters the game tried to draw that its font lacks (`check` reports them).
    fn missing_glyphs(&self) -> Vec<char> {
        Vec::new()
    }
    /// The tuning values the workshop shows as sliders, with their current values.
    fn knobs(&self) -> Vec<Knob> {
        Vec::new()
    }
    /// Sets one tuning value (the workshop, an agent, a scenario). Unknown names do nothing.
    fn set_knob(&mut self, _name: &str, _value: f32) {}
    /// A copy of the game as it is now, for the workshop's rewinding: a game that is `Clone`
    /// returns `Some(Box::new(self.clone()))`. Without one the workshop still rewinds, by playing
    /// the recording from the start, which grows slow in long sessions.
    fn snapshot(&self) -> Option<Box<dyn Game>> {
        None
    }
    /// For a game too big to copy whole (a world of millions of cells): the moment as it is,
    /// sharing what didn't change with the moments kept before it (see `Frozen`). When a game
    /// gives one, the workshop keeps it instead of a `snapshot`.
    fn freeze(&self) -> Option<Box<dyn Frozen>> {
        None
    }
}

/// A moment of a game kept to come back to, for games too big to copy whole: the workshop only
/// keeps it and thaws it, it never plays it. It may share what didn't change with the moments
/// before it (unchanged parts of a world behind an `Arc`), so a hundred moments cost little more
/// than one.
pub trait Frozen {
    /// The game again, as it was at that moment.
    fn thaw(&self) -> Box<dyn Game>;
    /// The memory it holds, what it shares divided among those that share it, so the sum over
    /// all moments kept is what they hold together: the workshop keeps that under its budget.
    fn bytes(&self) -> usize;
}

/// How a moment of the game is kept: a copy, or frozen.
enum Kept {
    Copy(Box<dyn Game>),
    Frozen(Box<dyn Frozen>),
}

/// A moment of a session to come back to: the game, kept, and the context that steps it.
pub struct Snapshot {
    game: Kept,
    frame: u64,
    rng: Rng,
    input: Input,
}

impl Snapshot {
    /// The tick it was taken after.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// The memory it holds (a frozen moment's own share; a copy says nothing and counts 0).
    pub fn bytes(&self) -> usize {
        match &self.game {
            Kept::Copy(_) => 0,
            Kept::Frozen(f) => f.bytes(),
        }
    }
}

/// A game being run: its tick context, its picture, its sound.
pub struct Session {
    game: Box<dyn Game>,
    pub ctx: Ctx,
    pub canvas: Canvas,
    /// Sounds the game started in the last tick.
    pub sounds: Vec<Sfx>,
    /// Recordings the game started in the last tick.
    pub plays: Vec<Play>,
    /// The hums that began in the last tick (their samples' names), and the keys asked for.
    hums_begun: Vec<&'static str>,
    hum_keys: Vec<&'static str>,
    /// Samples of the last tick in stereo, left and right in turn (only filled after
    /// `enable_audio`).
    pub audio: Vec<f32>,
    /// The invariants broken in the last tick.
    pub broken: Vec<String>,
    synth: Synth,
    audio_on: bool,
}

impl Session {
    pub fn new(game: Box<dyn Game>, seed: u64) -> Session {
        let (w, h) = game.size();
        Session {
            game,
            ctx: Ctx::new(seed),
            canvas: Canvas::new(w, h),
            sounds: Vec::new(),
            plays: Vec::new(),
            hums_begun: Vec::new(),
            hum_keys: Vec::new(),
            audio: Vec::new(),
            broken: Vec::new(),
            synth: Synth::new(),
            audio_on: false,
        }
    }

    /// The names of every sound that began in the last tick: effects, recordings played once,
    /// hums on their first tick. `check` counts them and `listen` labels them.
    pub fn sound_names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = self.sounds.iter().map(|s| s.name).collect();
        names.extend(self.plays.iter().map(|p| p.sample.name));
        names.extend(&self.hums_begun);
        names
    }

    /// The concrete game, for tools and tests that need more than `state()`.
    pub fn game_as<T: Game>(&self) -> Option<&T> {
        let any: &dyn Any = &*self.game;
        any.downcast_ref::<T>()
    }

    /// The concrete game, to change it from outside (a window's own keys, a menu's quit).
    pub fn game_as_mut<T: Game>(&mut self) -> Option<&mut T> {
        let any: &mut dyn Any = &mut *self.game;
        any.downcast_mut::<T>()
    }

    /// The game, for the window's and the tools' questions (`wants_mouse`, `next_look`, …).
    pub fn game(&self) -> &dyn Game {
        &*self.game
    }

    pub fn game_mut(&mut self) -> &mut dyn Game {
        &mut *self.game
    }

    /// Renders sound from now on. Off by default, so headless runs stay as fast as possible.
    pub fn enable_audio(&mut self) {
        self.audio_on = true;
    }

    /// Runs one tick with the given buttons held (a mask of `Button::bit()` values).
    pub fn step(&mut self, buttons: u8) {
        self.step_frame(&Frame::buttons(buttons));
    }

    /// Runs one tick with full input: buttons, mouse and keyboard.
    pub fn step_frame(&mut self, input: &Frame) {
        self.ctx.frame += 1;
        self.ctx.input.advance(input);
        self.game.update(&mut self.ctx);
        self.sounds.clear();
        self.sounds.append(&mut self.ctx.sounds);
        self.plays.clear();
        self.plays.append(&mut self.ctx.plays);
        let hums = std::mem::take(&mut self.ctx.hums);
        self.hums_begun = hums
            .iter()
            .filter(|(key, _)| !self.hum_keys.contains(key))
            .map(|(_, play)| play.sample.name)
            .collect();
        self.hum_keys = hums.iter().map(|(key, _)| *key).collect();
        if self.audio_on {
            for &sfx in &self.sounds {
                self.synth.trigger(sfx);
            }
            for play in &self.plays {
                self.synth.start(play.clone());
            }
            self.synth.hold(&hums);
            self.audio.resize(SAMPLES_PER_FRAME * 2, 0.0);
            self.synth.render(&mut self.audio);
        }
        let mut check = Invariants::default();
        self.game.invariants(&mut check);
        self.broken = check.broken;
        self.game.draw(&mut self.canvas, &self.ctx);
    }

    /// Gives the picture a new size (the window does, for `Fit::Expand`); the next `draw`
    /// fills it.
    pub fn resize(&mut self, w: usize, h: usize) {
        if (self.canvas.width(), self.canvas.height()) != (w, h) {
            self.canvas = Canvas::new(w.max(1), h.max(1));
        }
    }

    /// The session as it is, if the game can freeze or copy itself (`Game::freeze`,
    /// `Game::snapshot`). The canvas is not kept: `draw` paints the whole picture from the
    /// state, so the next tick paints it again.
    pub fn snapshot(&self) -> Option<Snapshot> {
        let game = match self.game.freeze() {
            Some(frozen) => Kept::Frozen(frozen),
            None => Kept::Copy(self.game.snapshot()?),
        };
        Some(Snapshot {
            game,
            frame: self.ctx.frame,
            rng: self.ctx.rng.clone(),
            input: self.ctx.input,
        })
    }

    /// Back to a snapshot (which stays usable). Storage, sound and the canvas stay as they are.
    pub fn restore(&mut self, s: &Snapshot) -> bool {
        let game = match &s.game {
            Kept::Copy(copy) => match copy.snapshot() {
                Some(game) => game,
                None => return false,
            },
            Kept::Frozen(frozen) => frozen.thaw(),
        };
        self.game = game;
        self.ctx.frame = s.frame;
        self.ctx.rng = s.rng.clone();
        self.ctx.input = s.input;
        true
    }

    /// The names of the rules the game checks as it is now, broken or not (a game may check some
    /// only in some states: ask on several ticks for all of them).
    pub fn rules(&self) -> Vec<String> {
        let mut check = Invariants {
            broken: Vec::new(),
            named: Some(Vec::new()),
        };
        self.game.invariants(&mut check);
        check.named.unwrap_or_default()
    }

    /// The game's state report as JSON.
    pub fn state_json(&self) -> String {
        self.game.state().to_json()
    }
}
