// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The workshop as a window: a tape deck for a game. The game plays in its bay and everything
//! played is recorded (REC); the deck plays it back (PLAY), winds it (REW, FF, START), steps it
//! (STEP) and marks moments for the agent (CUE). The panel beside it holds the knobs, the rules and
//! the state. Mouse on the keys, the tape and the faders; keyboard: Alt with a key, or F5 to F10.

use super::look::{self, Content, Layout};
use super::{Bench, Desk, Resume};
use polter::canvas::Canvas;
use polter::input::{Button, Frame, Input, Key, Mouse, PadButton};
use polter::window::{Host, Io};
use std::path::PathBuf;
use std::time::{Instant, SystemTime};

/// The program the workshop runs from. When it is built anew, the workshop reloads into it.
pub(super) struct Watch {
    path: PathBuf,
    first: Option<(SystemTime, u64)>,
    last: Option<(SystemTime, u64)>,
}

impl Watch {
    pub(super) fn new(path: PathBuf) -> Watch {
        let now = signature(&path);
        Watch {
            path,
            first: now,
            last: now,
        }
    }

    /// True once the file is not what it was at the start and has stayed the same since the
    /// last look (a build still being written is not taken).
    pub(super) fn changed(&mut self) -> bool {
        let now = signature(&self.path);
        let settled = now.is_some() && now == self.last;
        self.last = now;
        settled && now != self.first
    }
}

/// A file's time and length: what changes when it is written anew.
fn signature(path: &std::path::Path) -> Option<(SystemTime, u64)> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.modified().ok()?, m.len()))
}

/// What the deck is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The person plays and every tick is recorded (REC and PLAY are down). Recording after a
    /// rewind records over the rest of the tape; what was there is kept as a take first.
    Live,
    /// Time stands; the tape can be wound and scrubbed, STEP plays one tick.
    Paused,
    /// The recording plays back; a game key or a click into the game takes over from there.
    Replay,
}

/// The deck's keys, left to right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeckKey {
    Start,
    Rew,
    Play,
    Pause,
    Ff,
    Step,
    Rec,
    Cue,
}

impl DeckKey {
    pub const ALL: [DeckKey; 8] = [
        DeckKey::Start,
        DeckKey::Rew,
        DeckKey::Play,
        DeckKey::Pause,
        DeckKey::Ff,
        DeckKey::Step,
        DeckKey::Rec,
        DeckKey::Cue,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DeckKey::Start => "START",
            DeckKey::Rew => "REW",
            DeckKey::Play => "PLAY",
            DeckKey::Pause => "PAUSE",
            DeckKey::Ff => "FF",
            DeckKey::Step => "STEP",
            DeckKey::Rec => "REC",
            DeckKey::Cue => "CUE",
        }
    }

    /// Keys that keep going while held: winding, stepping.
    fn repeats(self) -> bool {
        matches!(self, DeckKey::Rew | DeckKey::Ff | DeckKey::Step)
    }

    /// Its keyboard keys: an F key, or Alt with a key (F keys need Fn on a Mac).
    fn keys(self) -> (Option<Key>, Key) {
        match self {
            DeckKey::Start => (Some(Key::F10), Key::Num0),
            DeckKey::Rew => (Some(Key::F7), Key::Left),
            DeckKey::Play => (None, Key::P),
            DeckKey::Pause => (None, Key::Space),
            DeckKey::Ff => (Some(Key::F8), Key::Right),
            DeckKey::Step => (Some(Key::F6), Key::Down),
            DeckKey::Rec => (None, Key::R),
            DeckKey::Cue => (Some(Key::F9), Key::M),
        }
    }
}

/// F5 (and Alt+Space) toggles between playing and pausing.
const KEY_TOGGLE: Key = Key::F5;
/// Frames a winding key is held before it keeps winding.
const HOLD: u32 = 18;
/// Ticks wound per frame while REW or FF is held: thirty times as fast as playing.
const WIND: u64 = 30;

/// What the mouse drags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Drag {
    Knob(usize),
    Tape,
}

/// A line in the display, running through it when it is longer than the glass.
pub(super) struct Note {
    pub text: String,
    pub age: u32,
    pub life: u32,
}

/// The meters' needles, with some inertia, as needles have.
#[derive(Default)]
pub(super) struct Meters {
    /// The sound's level: 0 at -36 dB and below, 1 at full scale.
    pub level: f32,
    /// Time a tick takes, as a share of its 16.7 ms.
    pub load: f32,
}

/// The workshop as a window host: the game on its bench, the desk it shares with an agent.
pub struct Workshop {
    pub(super) name: String,
    pub bench: Bench,
    pub(super) desk: Desk,
    pub mode: Mode,
    speaker: Option<polter::speaker::Speaker>,
    pub(super) note: Option<Note>,
    pub(super) drag: Option<Drag>,
    /// The deck key under the mouse, and the one the mouse holds down.
    pub(super) hover: Option<DeckKey>,
    pub(super) mouse_key: Option<DeckKey>,
    /// A repeating key being held, and for how many frames.
    held: Option<(DeckKey, u32)>,
    /// Ticks of this recording that were marked (CUE), shown on the tape.
    pub(super) marks: Vec<u64>,
    pub(super) meters: Meters,
    /// How far the state display and the faders are scrolled.
    pub(super) state_scroll: usize,
    pub(super) knob_scroll: usize,
    /// Whether the panel is open, and whether the mouse is on its handle.
    pub(super) panel_open: bool,
    pub(super) hover_handle: bool,
    /// What the workshop last wrote to knobs.txt (a different file means the agent changed it).
    knobs_written: String,
    /// The program to reload into when it is built anew, and the words it was started with.
    watch: Option<Watch>,
    args: Vec<String>,
    /// A fader in the hand: its last value, logged once when it is let go (a drag is one event
    /// for an agent reading the log, not one per pixel; the recording keeps every step).
    fader_note: Option<(String, f32)>,
    ticks_since_save: u32,
    polls: u32,
}

impl Workshop {
    pub fn new(name: &str, mut bench: Bench, desk: Desk) -> Workshop {
        // the sound is made even without speakers: the level meter shows it
        bench.enable_audio();
        let mut w = Workshop {
            name: name.to_string(),
            bench,
            desk,
            mode: Mode::Live,
            speaker: None,
            note: None,
            drag: None,
            hover: None,
            mouse_key: None,
            held: None,
            marks: Vec::new(),
            meters: Meters::default(),
            state_scroll: 0,
            knob_scroll: 0,
            panel_open: true,
            hover_handle: false,
            knobs_written: String::new(),
            fader_note: None,
            watch: None,
            args: Vec::new(),
            ticks_since_save: 0,
            polls: 0,
        };
        w.save();
        w.desk.log(w.bench.frame(), "workshop opened");
        w
    }

    /// The window's size for a game of this size: the game at its own scale beside the panel,
    /// above the deck.
    pub fn window_size(game: (usize, usize)) -> (usize, usize) {
        Layout::window_size(game)
    }

    /// Sound through the speakers (the window's workshop; headless pictures stay silent).
    pub fn with_sound(mut self) -> Workshop {
        self.speaker = polter::speaker::Speaker::open();
        self
    }

    /// Reloads when its own program is built anew: the session goes on at the same tick, with
    /// the new code (SPEC 5.5, "code changes without starting over").
    pub fn with_reload(mut self) -> Workshop {
        if let Ok(exe) = std::env::current_exe() {
            self.watch = Some(Watch::new(exe));
            self.args = std::env::args().skip(1).collect();
        }
        self
    }

    /// Picks up after a reload: paused at the tick the person was at, the panel as it was.
    pub fn resumed(mut self, r: Resume) -> Workshop {
        self.mode = Mode::Paused;
        self.panel_open = r.panel_open;
        self.marks = r.marks.clone();
        self.desk
            .log(r.frame, "reloaded: the new build, replayed to here");
        self.tell(format!("new build, back at {}", r.frame));
        self
    }

    /// Saves everything, notes where the person is, and starts the program again in this very
    /// process (the window closes and opens; the terminal stays the same).
    fn reload(&mut self, why: &str) {
        let frame = self.bench.frame();
        let Some(watch) = &self.watch else {
            self.tell("no reload here");
            return;
        };
        let path = watch.path.clone();
        self.log_fader();
        self.save();
        let resume = Resume {
            frame,
            panel_open: self.panel_open,
            marks: self.marks.clone(),
        };
        if let Err(e) = self.desk.save_resume(&resume) {
            self.tell(format!("no reload: {e}"));
            return;
        }
        self.desk.log(frame, &format!("reloading ({why})"));
        let mut args = self.args.clone();
        if !args.iter().any(|a| a == "--continue") {
            args.push("--continue".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // returns only if it failed
            let err = std::process::Command::new(&path).args(&args).exec();
            self.desk.log(frame, &format!("reload failed: {err}"));
            self.tell(format!("reload failed: {err}"));
        }
        #[cfg(not(unix))]
        {
            let _ = (path, args);
            self.tell("start the workshop again with --continue");
        }
    }

    /// Draws the whole workshop onto a canvas (the window's, or a picture's).
    pub fn draw(&self, canvas: &mut Canvas) {
        look::draw(self, canvas);
    }

    /// Whether a key stays down by itself, as a deck's keys latch: PLAY while playing or
    /// recording, REC while recording, PAUSE while paused.
    pub(super) fn engaged(&self, k: DeckKey) -> bool {
        match k {
            DeckKey::Play => self.mode != Mode::Paused,
            DeckKey::Rec => self.mode == Mode::Live,
            DeckKey::Pause => self.mode == Mode::Paused,
            _ => false,
        }
    }

    pub(super) fn layout(&self, w: usize, h: usize) -> Layout {
        let g = &self.bench.session.canvas;
        let knobs = self.bench.session.game().knobs();
        let state_lines =
            crate::scenario::parse_state(&self.bench.session.state_json()).map_or(0, |s| s.len());
        Layout::new(
            w,
            h,
            &Content {
                game: (g.width(), g.height()),
                knobs: knobs.len(),
                name_chars: knobs.iter().map(|k| k.name.len()).max().unwrap_or(0),
                state_lines,
                knob_scroll: self.knob_scroll,
                panel_open: self.panel_open,
            },
        )
    }

    fn tell(&mut self, text: impl Into<String>) {
        let text = text.into().to_uppercase();
        let life = 150 + 8 * text.len() as u32;
        self.note = Some(Note { text, age: 0, life });
    }

    /// Saves the recording and the knobs to the desk.
    fn save(&mut self) {
        let _ = self.desk.save_recording(&self.bench.rec);
        let knobs = self.bench.session.game().knobs();
        let _ = self.desk.save_knobs(&knobs);
        self.knobs_written =
            std::fs::read_to_string(self.desk.dir.join("knobs.txt")).unwrap_or_default();
    }

    /// Before the tape's future is recorded over: keep it as a take.
    fn before_cut(&mut self) {
        let f = self.bench.frame();
        if f < self.bench.len() {
            match self.desk.keep_take(&self.bench.rec, f) {
                Ok(n) => self.tell(format!("take {n} kept")),
                Err(e) => self.tell(format!("no take kept: {e}")),
            }
            self.marks.retain(|&m| m <= f);
        }
    }

    /// The fader just let go, as one line in the log.
    fn log_fader(&mut self) {
        if let Some((name, value)) = self.fader_note.take() {
            self.desk
                .log(self.bench.frame(), &format!("knob {name} {value} (fader)"));
        }
    }

    fn set_knob(&mut self, name: &str, value: f32, from: &str) {
        self.before_cut();
        self.bench.set_knob(name, value);
        if from == "fader" {
            self.fader_note = Some((name.to_string(), value));
        } else {
            self.desk
                .log(self.bench.frame(), &format!("knob {name} {value} ({from})"));
        }
        if self.mode == Mode::Replay {
            // a knob set during playback records over the rest: play on live
            self.mode = Mode::Live;
        }
    }

    fn goto(&mut self, frame: u64) {
        self.bench.goto(frame);
        self.mode = Mode::Paused;
    }

    fn play(&mut self) {
        self.mode = if self.bench.frame() < self.bench.len() {
            Mode::Replay
        } else {
            Mode::Live
        };
    }

    fn mark(&mut self, note: &str) {
        match self.desk.mark(&self.bench, note) {
            Ok(n) => {
                self.marks.push(self.bench.frame());
                self.tell(format!("mark {n} kept"));
            }
            Err(e) => self.tell(format!("no mark: {e}")),
        }
    }

    /// What a deck key does when it goes down.
    fn press(&mut self, k: DeckKey) {
        match k {
            DeckKey::Start => self.goto(0),
            DeckKey::Rew => self.goto(self.bench.frame().saturating_sub(60)),
            DeckKey::Ff => self.goto((self.bench.frame() + 60).min(self.bench.len())),
            DeckKey::Play => self.play(),
            DeckKey::Pause => {
                if self.mode == Mode::Paused {
                    self.play();
                } else {
                    self.save();
                    self.mode = Mode::Paused;
                }
            }
            DeckKey::Step => self.step_once(),
            DeckKey::Rec => {
                self.mode = Mode::Live;
                if self.bench.frame() < self.bench.len() {
                    self.tell("recording from here");
                }
            }
            DeckKey::Cue => self.mark(""),
        }
        if k.repeats() {
            self.held = Some((k, 0));
        }
    }

    /// What a winding key does while it stays down.
    fn repeat(&mut self, k: DeckKey) {
        let f = self.bench.frame();
        match k {
            DeckKey::Rew => self.goto(f.saturating_sub(WIND)),
            DeckKey::Ff => self.goto((f + WIND).min(self.bench.len())),
            DeckKey::Step => self.step_once(),
            _ => {}
        }
    }

    fn step_once(&mut self) {
        self.mode = Mode::Paused;
        if !self.bench.replay_step() {
            self.bench.step(&Frame::default());
        }
    }

    /// The agent's side: commands from the desk, and knobs changed in its file.
    fn poll_desk(&mut self) {
        for c in self.desk.take_commands() {
            let words: Vec<&str> = c.split_whitespace().collect();
            let done = match words.as_slice() {
                ["goto", f] => f.parse().map(|f| self.goto(f)).is_ok(),
                ["play"] => {
                    self.play();
                    true
                }
                ["pause"] => {
                    self.mode = Mode::Paused;
                    true
                }
                ["reload"] => {
                    self.reload("the agent asked");
                    true
                }
                ["mark", note @ ..] => {
                    self.mark(&note.join(" "));
                    true
                }
                ["knob", name, v] => v
                    .parse()
                    .map(|v| self.set_knob(name, v, "the agent"))
                    .is_ok(),
                _ => false,
            };
            let what = if done { "done" } else { "not understood" };
            self.desk
                .log(self.bench.frame(), &format!("command \"{c}\": {what}"));
            self.tell(format!("agent: {c}"));
        }
        let text = std::fs::read_to_string(self.desk.dir.join("knobs.txt")).unwrap_or_default();
        if text != self.knobs_written && !text.is_empty() {
            let now: Vec<(String, f32)> = self
                .bench
                .session
                .game()
                .knobs()
                .iter()
                .map(|k| (k.name.to_string(), k.value))
                .collect();
            for (name, value) in self.desk.read_knobs() {
                if now.iter().any(|(n, v)| *n == name && *v != value) {
                    self.set_knob(&name, value, "knobs.txt");
                    self.tell(format!("{name} = {value} (from the file)"));
                }
            }
            self.knobs_written = text;
        }
    }

    /// The game's own input from the window's: console buttons from arrows, Z and X, the
    /// workshop's F keys taken out (and every key while Alt is held), the mouse in the game's
    /// pixels (its buttons only inside the game). Also whether the person just pressed something
    /// for the game: that takes over a playback.
    fn game_input(&self, io: &Io, lay: &Layout) -> (Frame, bool) {
        let workshop_keys = DeckKey::ALL
            .iter()
            .filter_map(|k| k.keys().0)
            .chain([KEY_TOGGLE])
            .fold(0u64, |a, f| a | f.bit());
        let keys = if io.input.key(Key::Alt) {
            0
        } else {
            io.frame.keys & !workshop_keys
        };
        let held = |k: Key| keys & k.bit() != 0;
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
        .fold(0, |a, (_, b)| a | b.bit());
        let (vx, vy, s) = lay.view;
        let (gw, gh) = lay.game;
        let (mx, my) = io.frame.mouse;
        let inside = mx >= vx && my >= vy && mx < vx + gw * s && my < vy + gh * s;
        let mouse = (
            ((mx - vx).max(0) / s).min(gw - 1),
            ((my - vy).max(0) / s).min(gh - 1),
        );
        let frame = Frame {
            // the keys' buttons and a gamepad's (the window reads the pads)
            buttons: buttons | io.frame.buttons,
            mouse,
            mouse_buttons: if inside { io.frame.mouse_buttons } else { 0 },
            wheel: if inside { io.frame.wheel } else { 0 },
            keys,
            pad: io.frame.pad,
        };
        let pressed = Key::ALL
            .iter()
            .any(|k| keys & k.bit() != 0 && io.input.key_pressed(*k))
            || (inside && io.input.mouse_pressed(Mouse::LEFT))
            // a gamepad's button pressed now (the window's input holds the pads' buttons)
            || Button::ALL
                .iter()
                .any(|b| io.frame.buttons & b.bit() != 0 && io.input.pressed(*b))
            || PadButton::ALL.iter().any(|b| io.input.pad_pressed(*b));
        (frame, pressed)
    }

    /// The keyboard's deck keys: pressed now, and which are held.
    fn keyboard(&mut self, input: &Input) -> Vec<DeckKey> {
        let alt = input.key(Key::Alt);
        if input.key_pressed(KEY_TOGGLE) || (alt && input.key_pressed(Key::Space)) {
            self.press(DeckKey::Pause);
        }
        if alt && input.key_pressed(Key::H) {
            self.panel_open = !self.panel_open;
        }
        let mut held = Vec::new();
        for k in DeckKey::ALL {
            if k == DeckKey::Pause {
                continue;
            }
            let (f, with_alt) = k.keys();
            if f.is_some_and(|f| input.key_pressed(f)) || (alt && input.key_pressed(with_alt)) {
                self.press(k);
            }
            if f.is_some_and(|f| input.key(f)) || (alt && input.key(with_alt)) {
                held.push(k);
            }
        }
        held
    }

    /// The mouse on the deck's keys, the tape, the faders and the state display.
    fn mouse(&mut self, input: &Input, lay: &Layout) {
        let p = input.mouse();
        self.hover = lay.key_at(p);
        self.hover_handle = lay.handle.contains(p);
        if !input.mouse_held(Mouse::LEFT) {
            self.drag = None;
            self.mouse_key = None;
            self.log_fader();
        } else if input.mouse_pressed(Mouse::LEFT) {
            if self.hover_handle {
                self.panel_open = !self.panel_open;
            } else if let Some(k) = self.hover {
                self.mouse_key = Some(k);
                self.press(k);
            } else if lay.tape.contains(p) {
                self.drag = Some(Drag::Tape);
            } else if let Some(i) = lay.fader_at(p) {
                self.drag = Some(Drag::Knob(i));
            }
        }
        match self.drag {
            Some(Drag::Tape) => {
                let target = lay.tape_frame(p.0, self.bench.len());
                if target != self.bench.frame() || self.mode != Mode::Paused {
                    self.goto(target);
                }
            }
            Some(Drag::Knob(i)) => {
                if let Some(knob) = self.bench.session.game().knobs().get(i) {
                    let (x0, x1) = lay.slot();
                    let t = (p.0.clamp(x0, x1) - x0) as f32 / (x1 - x0).max(1) as f32;
                    let raw = knob.min + t * (knob.max - knob.min);
                    let v = (knob.min + ((raw - knob.min) / knob.step).round() * knob.step)
                        .clamp(knob.min, knob.max);
                    // rounded, so files and logs read clean
                    let v = (v * 10_000.0).round() / 10_000.0;
                    if (v - knob.value).abs() > f32::EPSILON {
                        let name = knob.name;
                        self.set_knob(name, v, "fader");
                    }
                }
            }
            None => {}
        }
        if input.wheel() != 0 && lay.state.contains(p) {
            let lines = crate::scenario::parse_state(&self.bench.session.state_json())
                .map_or(0, |s| s.len());
            let most = lines.saturating_sub(lay.state_rows());
            self.state_scroll =
                (self.state_scroll as i32 - input.wheel()).clamp(0, most as i32) as usize;
        }
        if input.wheel() != 0 && lay.faders_area().contains(p) {
            let most = lay.knobs.saturating_sub(lay.faders);
            self.knob_scroll = (lay.first as i32 - input.wheel()).clamp(0, most as i32) as usize;
        }
    }

    /// One display frame: the desk, the keys, the mouse, the game's ticks, the meters.
    fn run(&mut self, io: &Io, w: usize, h: usize) {
        let lay = self.layout(w, h);
        self.polls += 1;
        if self.polls.is_multiple_of(15) {
            self.poll_desk();
        }
        if self.polls.is_multiple_of(30) && self.watch.as_mut().is_some_and(|w| w.changed()) {
            self.reload("a new build");
        }
        let input = io.input;
        let keys_held = self.keyboard(input);
        self.mouse(input, &lay);
        // a winding key held down keeps winding
        if let Some((k, n)) = self.held {
            if keys_held.contains(&k) || self.mouse_key == Some(k) {
                self.held = Some((k, n + 1));
                if n + 1 > HOLD {
                    self.repeat(k);
                }
            } else {
                self.held = None;
            }
        }
        let (frame, pressed) = self.game_input(io, &lay);
        let started = Instant::now();
        let (mut played, mut level) = (0u32, 0f32);
        for _ in 0..io.ticks {
            match self.mode {
                Mode::Paused => break,
                Mode::Replay if pressed => {
                    self.mode = Mode::Live;
                    self.desk.log(self.bench.frame(), "taken over");
                    self.before_cut();
                    self.bench.step(&frame);
                }
                Mode::Replay => {
                    if !self.bench.replay_step() {
                        self.mode = Mode::Live;
                        self.bench.step(&frame);
                    }
                }
                Mode::Live => {
                    self.before_cut();
                    self.bench.step(&frame);
                }
            }
            played += 1;
            let audio = &self.bench.session.audio;
            if !audio.is_empty() {
                let rms = (audio.iter().map(|s| s * s).sum::<f32>() / audio.len() as f32).sqrt();
                level = level.max(((20.0 * rms.max(1e-6).log10() + 36.0) / 36.0).clamp(0.0, 1.1));
            }
            if let Some(sp) = &self.speaker {
                sp.push(audio);
            }
            self.ticks_since_save += 1;
        }
        let load = if played > 0 {
            started.elapsed().as_secs_f32() * 1000.0 / played as f32 / (1000.0 / 60.0)
        } else {
            0.0
        };
        self.meters.level += (level - self.meters.level) * 0.3;
        self.meters.load += (load.min(1.2) - self.meters.load) * 0.2;
        if self.ticks_since_save >= 120 {
            self.ticks_since_save = 0;
            self.save();
        }
        if let Some(note) = &mut self.note {
            note.age += 1;
            if note.age >= note.life {
                self.note = None;
            }
        }
    }
}

impl Host for Workshop {
    fn frame(&mut self, io: &Io, canvas: &mut Canvas) -> bool {
        self.run(io, canvas.width(), canvas.height());
        self.draw(canvas);
        true
    }

    fn closing(&mut self) {
        self.log_fader();
        self.save();
        self.desk.log(self.bench.frame(), "workshop closed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polter::game::{Ctx, Game, Knob};
    use polter::state::State;
    use std::path::Path;

    /// A dot that runs right while Right is held, at a speed that is a knob.
    struct Dot {
        x: f32,
        speed: f32,
    }

    impl Game for Dot {
        fn update(&mut self, ctx: &mut Ctx) {
            if ctx.input.held(Button::Right) {
                self.x += self.speed;
            }
        }
        fn draw(&self, c: &mut Canvas, _ctx: &Ctx) {
            c.clear(0x000000);
            c.set(self.x as i32 % 64, 10, 0xffffff);
        }
        fn size(&self) -> (usize, usize) {
            (64, 48)
        }
        fn state(&self) -> State {
            State::new().num("x", self.x)
        }
        fn knobs(&self) -> Vec<Knob> {
            vec![Knob::new("speed", self.speed, 0.0, 4.0, 0.5)]
        }
        fn set_knob(&mut self, name: &str, value: f32) {
            if name == "speed" {
                self.speed = value;
            }
        }
    }

    /// A person at the workshop: holds keys, moves and clicks the mouse, one display frame at a
    /// time (one tick each), on a 480 × 270 picture.
    struct Person {
        shop: Workshop,
        input: Input,
        canvas: Canvas,
    }

    impl Person {
        fn frame(&mut self, keys: &[Key], mouse: (i32, i32), click: bool) {
            let frame = Frame {
                keys: keys.iter().fold(0, |a, k| a | k.bit()),
                mouse,
                mouse_buttons: if click { Mouse::LEFT } else { 0 },
                ..Frame::default()
            };
            self.input.advance(&frame);
            let io = Io {
                ticks: 1,
                frame,
                input: &self.input,
                typed: &[],
                full_screen: false,
            };
            self.shop.frame(&io, &mut self.canvas);
        }
        fn frames(&mut self, n: u32, keys: &[Key]) {
            for _ in 0..n {
                self.frame(keys, (0, 0), false);
            }
        }
        /// Clicks a deck key: down for one frame, up the next.
        fn click(&mut self, k: DeckKey) {
            let r = self.lay().keys[DeckKey::ALL.iter().position(|d| *d == k).unwrap()];
            let at = (r.x + r.w / 2, r.y + r.h / 2);
            self.frame(&[], at, true);
            self.frame(&[], at, false);
        }
        fn lay(&self) -> Layout {
            self.shop.layout(self.canvas.width(), self.canvas.height())
        }
        fn x(&self) -> f32 {
            let state = self.shop.bench.session.state_json();
            let pairs = crate::scenario::parse_state(&state).unwrap();
            let (_, x) = pairs.iter().find(|(k, _)| k == "x").unwrap();
            x.to_string().parse().unwrap()
        }
        fn at(&self) -> (u64, u64) {
            (self.shop.bench.frame(), self.shop.bench.len())
        }
    }

    fn person(dir: &Path) -> Person {
        let _ = std::fs::remove_dir_all(dir);
        let bench = Bench::new(Box::new(|| Box::new(Dot { x: 0.0, speed: 1.0 })), 1);
        let shop = Workshop::new("dot", bench, Desk::open(dir).unwrap());
        Person {
            shop,
            input: Input::default(),
            canvas: Canvas::new(480, 270),
        }
    }

    #[test]
    fn a_person_plays_rewinds_records_over_and_tunes() {
        let dir = std::env::temp_dir().join(format!("polter-shop-{}", std::process::id()));
        let mut p = person(&dir);
        // play: two seconds holding right, all of it recorded
        p.frames(120, &[Key::Right]);
        assert_eq!((p.at(), p.x()), ((120, 120), 120.0));
        assert_eq!(p.shop.mode, Mode::Live);
        // F5 pauses: time stands
        p.frame(&[KEY_TOGGLE], (0, 0), false);
        p.frames(30, &[Key::Right]);
        assert_eq!((p.shop.mode, p.at()), (Mode::Paused, (120, 120)));
        // START, then FF held down: a second on the press, thirty ticks a frame once held long
        p.click(DeckKey::Start);
        assert_eq!(p.at().0, 0);
        let ff = p.lay().keys[4];
        for _ in 0..=HOLD {
            p.frame(&[], (ff.x + 5, ff.y + 5), true);
        }
        p.frame(&[], (0, 0), false);
        assert_eq!(p.at().0, 60 + WIND, "one frame past holding");
        // a click in the middle of the tape goes to the middle of the recording
        let lay = p.lay();
        let (x0, x1) = lay.tape_x();
        let tape = ((x0 + x1) / 2, lay.tape.y + lay.tape.h / 2);
        p.frame(&[], tape, true);
        p.frame(&[], tape, false);
        assert_eq!((p.at(), p.x()), ((60, 120), 60.0));
        // Alt alone does nothing; Alt+Space plays the recording back from there (the deck's keys
        // act before the frame's tick, so the frame of the press plays already)
        p.frame(&[Key::Alt], (0, 0), false);
        assert_eq!((p.shop.mode, p.at().0), (Mode::Paused, 60));
        p.frame(&[Key::Alt, Key::Space], (0, 0), false);
        assert_eq!(p.shop.mode, Mode::Replay);
        p.frames(9, &[]);
        assert_eq!(
            (p.at().0, p.x()),
            (70, 70.0),
            "the recording, not the hands"
        );
        // pressing a game key takes over: the old future is kept as a take, then cut
        p.frames(5, &[Key::Right]);
        assert_eq!((p.shop.mode, p.at()), (Mode::Live, (75, 75)));
        assert!(dir.join("takes/take_1.txt").exists());
        // the knob's fader, dragged to its right end: speed 4 from now on
        let row = p.lay().fader_row(0).unwrap();
        let (s0, s1) = p.lay().slot();
        p.frame(&[], (s0 + 1, row.y + 3), true);
        p.frame(&[], (s1 + 20, row.y + 3), true);
        p.frame(&[], (s1 + 20, row.y + 3), false);
        assert_eq!(p.shop.bench.session.game().knobs()[0].value, 4.0);
        // one line in the log for the whole drag, with where it ended
        let log = std::fs::read_to_string(dir.join("events.log")).unwrap();
        let faders: Vec<&str> = log.lines().filter(|l| l.ends_with("(fader)")).collect();
        assert_eq!(faders.len(), 1, "{log}");
        assert!(faders[0].ends_with("knob speed 4 (fader)"), "{log}");
        let before = p.x();
        p.frames(10, &[Key::Right]);
        assert_eq!(p.x(), before + 40.0);
        // F9 keeps a mark for the agent, shown on the tape (at the tick before the frame's own)
        let at = p.at().0;
        p.frame(&[Key::F9], (0, 0), false);
        assert!(dir.join("marks/mark_1.png").exists());
        assert_eq!(p.shop.marks, [at]);
        // the deck's keys with the mouse: PAUSE latches, REW winds back a second, PLAY plays,
        // REC records over the rest (kept as a take first)
        p.click(DeckKey::Pause);
        assert_eq!(p.shop.mode, Mode::Paused);
        assert!(p.shop.engaged(DeckKey::Pause));
        let (end, _) = p.at();
        p.click(DeckKey::Rew);
        assert_eq!(p.at(), (end - 60, end));
        p.click(DeckKey::Play);
        assert_eq!(p.shop.mode, Mode::Replay);
        assert_eq!(
            p.at(),
            (end - 58, end),
            "a tick on the press, one on the release"
        );
        p.click(DeckKey::Rec);
        assert_eq!(p.shop.mode, Mode::Live);
        assert_eq!(p.at(), (end - 56, end - 56), "recorded over");
        assert!(dir.join("takes/take_2.txt").exists());
        assert!(
            p.shop.marks.is_empty(),
            "the mark was on the part recorded over"
        );
        // the agent: a command, and a knob changed in its file
        std::fs::write(dir.join("commands.txt"), "goto 30\n").unwrap();
        p.frames(15, &[]);
        assert_eq!((p.at().0, p.shop.mode), (30, Mode::Paused));
        std::fs::write(dir.join("knobs.txt"), "speed 2.5\n").unwrap();
        p.frames(15, &[]);
        assert_eq!(p.shop.bench.session.game().knobs()[0].value, 2.5);
        assert!(
            dir.join("takes/take_3.txt").exists(),
            "set at 30: recorded over"
        );
        let log = std::fs::read_to_string(dir.join("events.log")).unwrap();
        assert!(log.contains("command \"goto 30\": done"), "{log}");
        assert!(log.contains("knob speed 2.5 (knobs.txt)"), "{log}");
        // closing saves; the next session keeps this one and can resume it
        p.shop.closing();
        let saved = p.shop.bench.rec.clone();
        let desk = Desk::open(&dir).unwrap();
        assert_eq!(desk.last_session(), Some(saved));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The deck in its states, as pictures in `POLTER_LOOK_DIR` when it is set (to look at
    /// before trusting the drawing), drawn in any case.
    #[test]
    fn pictures_of_the_deck() {
        let dir = std::env::temp_dir().join(format!("polter-shop-look-{}", std::process::id()));
        let mut p = person(&dir);
        let out = std::env::var_os("POLTER_LOOK_DIR").map(std::path::PathBuf::from);
        let shot = |p: &Person, name: &str| {
            if let Some(out) = &out {
                std::fs::write(out.join(format!("{name}.png")), p.canvas.to_png(3)).unwrap();
            }
        };
        // recording, loud
        p.frames(200, &[Key::Right]);
        p.shop.meters = Meters {
            level: 0.85,
            load: 0.2,
        };
        p.shop.draw(&mut p.canvas);
        shot(&p, "deck_rec");
        // playing back mid-tape, with marks, a knob change and a note running through
        p.frame(&[Key::F9], (0, 0), false);
        p.click(DeckKey::Pause);
        p.click(DeckKey::Start);
        p.frames(1, &[]);
        p.click(DeckKey::Play);
        p.frames(80, &[]);
        p.shop
            .tell("agent: knob gravity 0.31, the jump felt floaty");
        p.frames(60, &[]);
        p.shop.meters.level = 0.45;
        p.shop.hover = Some(DeckKey::Ff);
        p.shop.draw(&mut p.canvas);
        shot(&p, "deck_play");
        // a broken rule, a fader in the hand, a larger window
        p.shop.bench.session.broken = vec!["x stays on the screen".into()];
        p.shop.drag = Some(Drag::Knob(0));
        p.canvas = Canvas::new(480, 300);
        p.shop.draw(&mut p.canvas);
        shot(&p, "deck_broken");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_build_is_seen_once_it_has_settled() {
        let path = std::env::temp_dir().join(format!("polter-watch-{}", std::process::id()));
        std::fs::write(&path, b"the old build").unwrap();
        let mut w = Watch::new(path.clone());
        assert!(!w.changed(), "nothing new");
        assert!(!w.changed(), "still nothing");
        // a build being written: different now, not yet settled
        std::fs::write(&path, b"the new build, longer").unwrap();
        assert!(!w.changed(), "just written: wait one look");
        assert!(w.changed(), "the same since the last look: reload");
        std::fs::write(&path, b"the old build").unwrap();
        // gone for a moment (a build replacing the file): no reload
        std::fs::remove_file(&path).unwrap();
        assert!(!w.changed());
        assert!(!w.changed());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_workshop_draws_at_any_size() {
        let dir = std::env::temp_dir().join(format!("polter-shop-sizes-{}", std::process::id()));
        let mut p = person(&dir);
        p.frames(20, &[Key::Right]);
        for (w, h) in [(1, 1), (40, 20), (149, 300), (480, 270), (1440, 900)] {
            let mut c = Canvas::new(w, h);
            p.shop.draw(&mut c);
            p.canvas = Canvas::new(w, h);
            p.frame(&[], (w as i32 - 1, h as i32 - 1), true);
            p.frame(&[], (0, 0), false);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
