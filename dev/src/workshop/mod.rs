// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The workshop: where a person plays, rewinds and tunes a game while an agent works on it
//! (SPEC 5.5). This file is the model, without a window: a `Bench` holds the game being played,
//! the recording of everything that made it (inputs and knob changes, tick by tick), and goes to
//! any tick by playing the recording again, which a deterministic game makes exact. A `Desk` is
//! the folder the person and the agent share: marks, knobs, the recording, takes, and a log of
//! events. The window around it is a tape deck: `shop` (what it does) and `look` (how it looks).

mod look;
mod shop;

pub use shop::{DeckKey, Mode, Workshop};

use polter::canvas::Canvas;
use polter::game::{Game, Session, Snapshot};
use polter::haunt::Haunt;
use polter::input::Frame;
use std::path::{Path, PathBuf};

/// A knob set to a value when the game had run `frame` ticks (before the next one).
#[derive(Clone, Debug, PartialEq)]
pub struct KnobChange {
    pub frame: u64,
    pub name: String,
    pub value: f32,
}

/// Everything that made a session: the seed, every tick's input, every knob change.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recording {
    pub seed: u64,
    pub inputs: Vec<Frame>,
    pub knobs: Vec<KnobChange>,
}

impl Recording {
    /// As text: `seed`, `frames`, one `knob FRAME NAME VALUE` line per change, then the inputs
    /// as a haunt (so `peek --haunt` can replay the inputs too).
    pub fn to_text(&self) -> String {
        let mut out = format!(
            "# Polter workshop recording\nseed {}\nframes {}\n",
            self.seed,
            self.inputs.len()
        );
        for k in &self.knobs {
            out += &format!("knob {} {} {}\n", k.frame, k.name, k.value);
        }
        out + &Haunt::write_frames(&self.inputs, true)
    }

    pub fn parse(text: &str) -> Result<Recording, String> {
        let (mut seed, mut frames, mut knobs) = (1, 0u64, Vec::new());
        let mut haunt = String::new();
        for (k, line) in text.lines().enumerate() {
            let words: Vec<&str> = line.split_whitespace().collect();
            match words.as_slice() {
                ["seed", n] => seed = n.parse().map_err(|_| format!("line {}: seed", k + 1))?,
                ["frames", n] => {
                    frames = n.parse().map_err(|_| format!("line {}: frames", k + 1))?
                }
                ["knob", f, name, v] => knobs.push(KnobChange {
                    frame: f
                        .parse()
                        .map_err(|_| format!("line {}: knob frame", k + 1))?,
                    name: name.to_string(),
                    value: v
                        .parse()
                        .map_err(|_| format!("line {}: knob value", k + 1))?,
                }),
                _ => {
                    haunt.push_str(line);
                    haunt.push('\n');
                }
            }
        }
        let h = Haunt::parse(&haunt)?;
        let inputs = (1..=frames).map(|f| h.frame(f)).collect();
        Ok(Recording {
            seed,
            inputs,
            knobs,
        })
    }
}

/// At most this many snapshots are kept, and at most this much memory in them (what frozen
/// moments report: a big world's moments share most of it); past either, every other one goes
/// and the spacing doubles.
const MOST_SNAPSHOTS: usize = 128;
pub const MOST_BYTES: usize = 256 << 20;
/// Snapshots are first taken every this many ticks (two seconds).
const FIRST_SPACING: u64 = 120;

/// The game on the bench: the session at its current tick, and the recording behind it.
pub struct Bench {
    make: Box<dyn Fn() -> Box<dyn Game>>,
    pub session: Session,
    pub rec: Recording,
    /// Copies of the game along the recording, oldest first (when the game can copy itself), so
    /// going back plays a few seconds again instead of the whole session.
    snapshots: Vec<Snapshot>,
    spacing: u64,
    audio: bool,
    /// The game at tick 0, kept with the seed it was made with: going back to the start thaws
    /// it instead of making the game again (a big world takes seconds to make).
    start: Option<(Snapshot, u64)>,
}

impl Bench {
    pub fn new(make: Box<dyn Fn() -> Box<dyn Game>>, seed: u64) -> Bench {
        let session = Session::new(make(), seed);
        let start = session.snapshot().map(|s| (s, seed));
        Bench {
            make,
            session,
            rec: Recording {
                seed,
                ..Recording::default()
            },
            snapshots: Vec::new(),
            spacing: FIRST_SPACING,
            audio: false,
            start,
        }
    }

    /// The game's sound is made from now on (and after every restart), so it can be heard and
    /// measured.
    pub fn enable_audio(&mut self) {
        self.audio = true;
        self.session.enable_audio();
    }

    /// How many snapshots are kept, and how many ticks apart (for measurements and tests).
    pub fn snapshots(&self) -> (usize, u64) {
        (self.snapshots.len(), self.spacing)
    }

    /// The memory the snapshots hold together (the start's moment too), as frozen moments
    /// report it.
    pub fn snapshot_bytes(&self) -> usize {
        self.start
            .iter()
            .map(|(s, _)| s)
            .chain(&self.snapshots)
            .map(Snapshot::bytes)
            .sum()
    }

    /// Plays one tick and keeps a snapshot when one is due.
    fn tick(&mut self, input: &Frame) {
        self.session.step_frame(input);
        let f = self.frame();
        if f.is_multiple_of(self.spacing) && self.snapshots.last().is_none_or(|s| s.frame() < f) {
            let Some(snap) = self.session.snapshot() else {
                return;
            };
            self.snapshots.push(snap);
            while self.snapshots.len() > 2
                && (self.snapshots.len() > MOST_SNAPSHOTS || self.snapshot_bytes() > MOST_BYTES)
            {
                self.spacing *= 2;
                let spacing = self.spacing;
                self.snapshots.retain(|s| s.frame().is_multiple_of(spacing));
            }
        }
    }

    /// Cuts the recording's future after tick `f` (taking over after a rewind).
    fn cut(&mut self, f: u64) {
        self.rec.inputs.truncate(f as usize);
        self.rec.knobs.retain(|k| k.frame <= f);
        self.snapshots.retain(|s| s.frame() <= f);
    }

    /// The tick the game is at.
    pub fn frame(&self) -> u64 {
        self.session.ctx.frame
    }

    /// How many ticks are recorded (more than `frame()` after a rewind).
    pub fn len(&self) -> u64 {
        self.rec.inputs.len() as u64
    }

    pub fn is_empty(&self) -> bool {
        self.rec.inputs.is_empty()
    }

    /// Plays the next tick with new input. After a rewind this takes over: the recording's future
    /// (inputs and knob changes after this tick) is cut, a new branch begins.
    pub fn step(&mut self, input: &Frame) {
        let f = self.frame();
        if f < self.len() {
            self.cut(f);
        }
        self.rec.inputs.push(*input);
        self.tick(input);
    }

    /// Plays the next recorded tick, if there is one (playing a recording back); false at its end.
    pub fn replay_step(&mut self) -> bool {
        let f = self.frame();
        let Some(input) = self.rec.inputs.get(f as usize).copied() else {
            return false;
        };
        self.apply_knobs_at(f);
        self.tick(&input);
        true
    }

    /// Sets a knob now; it is recorded with the tick, so a replay sets it at the same moment.
    pub fn set_knob(&mut self, name: &str, value: f32) {
        let f = self.frame();
        // a change at the same tick replaces the one before it; a rewound future is cut
        self.rec
            .knobs
            .retain(|k| k.frame < f || (k.frame == f && k.name != name));
        if f < self.len() {
            self.cut(f);
        }
        self.rec.knobs.push(KnobChange {
            frame: f,
            name: name.to_string(),
            value,
        });
        self.session.game_mut().set_knob(name, value);
    }

    fn apply_knobs_at(&mut self, frame: u64) {
        let changes: Vec<(String, f32)> = self
            .rec
            .knobs
            .iter()
            .filter(|k| k.frame == frame)
            .map(|k| (k.name.clone(), k.value))
            .collect();
        for (name, value) in changes {
            self.session.game_mut().set_knob(&name, value);
        }
    }

    /// Goes to a tick (clamped to the recording) by playing the recording again, with the knob
    /// changes at their ticks: from the last snapshot before it (or the start, without one), or on
    /// from here when that is closer. Only the last tick's picture is painted.
    pub fn goto(&mut self, frame: u64) {
        let target = frame.min(self.len());
        // the last snapshot before the target (at least one tick before it, which paints the
        // picture again)
        let from = self.snapshots.iter().rposition(|s| s.frame() < target);
        let ahead = target >= self.frame();
        match from {
            Some(i) if !ahead || self.snapshots[i].frame() > self.frame() => {
                if !self.session.restore(&self.snapshots[i]) {
                    self.restart();
                }
            }
            _ if !ahead => self.restart(),
            _ => {}
        }
        while self.frame() < target {
            let f = self.frame();
            self.session.ctx.picture = f + 1 == target;
            self.apply_knobs_at(f);
            let input = self.rec.inputs[f as usize];
            self.tick(&input);
        }
        self.session.ctx.picture = true;
        // a knob changed right at the target tick belongs to the next tick: set it now
        self.apply_knobs_at(target);
    }

    /// Replaces the recording (a resumed or loaded session) and goes to a tick of it.
    pub fn load(&mut self, rec: Recording, frame: u64) {
        self.rec = rec;
        self.snapshots.clear();
        self.spacing = FIRST_SPACING;
        self.restart();
        self.goto(frame);
    }

    /// A fresh game at tick 0, with the recording's seed, the storage and the canvas size kept:
    /// the start's moment thawed when it was made with that seed, the game made again if not.
    fn restart(&mut self) {
        if let Some((start, seed)) = &self.start
            && *seed == self.rec.seed
            && self.session.restore(start)
        {
            return;
        }
        let mut s = Session::new((self.make)(), self.rec.seed);
        s.ctx.storage = std::mem::take(&mut self.session.ctx.storage);
        if s.canvas.width() != self.session.canvas.width()
            || s.canvas.height() != self.session.canvas.height()
        {
            s.resize(self.session.canvas.width(), self.session.canvas.height());
        }
        self.session = s;
        if self.audio {
            self.session.enable_audio();
        }
        self.start = self.session.snapshot().map(|s| (s, self.rec.seed));
    }
}

/// Where a reloaded workshop picks up: the tick, whether the panel was open, and the ticks that
/// were marked (their flags on the tape).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resume {
    pub frame: u64,
    pub panel_open: bool,
    pub marks: Vec<u64>,
}

/// The folder the person and the agent share for one game:
/// `recording.txt` (the session, saved as it goes), `knobs.txt` (the knobs now: the agent may
/// edit it, the workshop picks the change up), `marks/` (pictures and states of marked
/// moments), `commands.txt` (lines from the agent: `goto 1234`, `play`, `pause`, `mark`,
/// `knob NAME VALUE`, `reload`), `events.log` (what happened, one line each, for an agent to watch),
/// `sessions/` (every earlier session's recording), `takes/` (recordings as they were before
/// something was recorded over them).
pub struct Desk {
    pub dir: PathBuf,
    marks: u32,
}

impl Desk {
    /// Opens (or makes) the folder. A recording left from an earlier session is kept, never
    /// overwritten: it moves to `sessions/` under the time it was last saved.
    pub fn open(dir: &Path) -> std::io::Result<Desk> {
        std::fs::create_dir_all(dir)?;
        let old = dir.join("recording.txt");
        if old.exists() {
            let stamp = std::fs::metadata(&old)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            std::fs::create_dir_all(dir.join("sessions"))?;
            let mut to = dir.join("sessions").join(format!("recording_{stamp}.txt"));
            let mut n = 1;
            while to.exists() {
                n += 1;
                to = dir
                    .join("sessions")
                    .join(format!("recording_{stamp}_{n}.txt"));
            }
            std::fs::rename(&old, to)?;
        }
        Desk::open_continuing(dir)
    }

    /// Opens the folder as it is: the session in it goes on (the workshop reloading itself after
    /// a new build).
    pub fn open_continuing(dir: &Path) -> std::io::Result<Desk> {
        std::fs::create_dir_all(dir.join("marks"))?;
        let marks = std::fs::read_dir(dir.join("marks"))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
            .count() as u32;
        Ok(Desk {
            dir: dir.to_path_buf(),
            marks,
        })
    }

    /// Notes where the person is, for the workshop that starts after a reload.
    pub fn save_resume(&self, r: &Resume) -> std::io::Result<()> {
        let panel = if r.panel_open { "open" } else { "folded" };
        let marks: Vec<String> = r.marks.iter().map(u64::to_string).collect();
        std::fs::write(
            self.dir.join("resume.txt"),
            format!(
                "frame {}\npanel {panel}\nmarks {}\n",
                r.frame,
                marks.join(" ")
            ),
        )
    }

    /// Where to resume, if a reload left a note (which is used up by reading it).
    pub fn take_resume(&self) -> Option<Resume> {
        let path = self.dir.join("resume.txt");
        let text = std::fs::read_to_string(&path).ok()?;
        let _ = std::fs::remove_file(&path);
        let mut r = Resume {
            frame: 0,
            panel_open: true,
            marks: Vec::new(),
        };
        for line in text.lines() {
            match line.split_whitespace().collect::<Vec<_>>().as_slice() {
                ["frame", f] => r.frame = f.parse().ok()?,
                ["panel", p] => r.panel_open = *p != "folded",
                ["marks", m @ ..] => r.marks = m.iter().filter_map(|m| m.parse().ok()).collect(),
                _ => {}
            }
        }
        Some(r)
    }

    /// One line in the log: `[seconds since the epoch] frame N: what`.
    pub fn log(&self, frame: u64, what: &str) {
        use std::io::Write;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("events.log"))
        {
            let _ = writeln!(f, "[{stamp}] frame {frame}: {what}");
        }
    }

    pub fn save_recording(&self, rec: &Recording) -> std::io::Result<()> {
        let tmp = self.dir.join("recording.txt.new");
        std::fs::write(&tmp, rec.to_text())?;
        std::fs::rename(tmp, self.dir.join("recording.txt"))
    }

    pub fn load_recording(&self) -> Option<Recording> {
        let text = std::fs::read_to_string(self.dir.join("recording.txt")).ok()?;
        Recording::parse(&text).ok()
    }

    /// The newest recording kept in `sessions/` (to resume where the last session ended).
    pub fn last_session(&self) -> Option<Recording> {
        let newest = std::fs::read_dir(self.dir.join("sessions"))
            .ok()?
            .filter_map(|e| e.ok())
            .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
            .max()?;
        Recording::parse(&std::fs::read_to_string(newest.1).ok()?).ok()
    }

    /// Writes the knobs as they are now (`name value` per line).
    pub fn save_knobs(&self, knobs: &[polter::game::Knob]) -> std::io::Result<()> {
        let text: String = knobs
            .iter()
            .map(|k| format!("{} {}\n", k.name, k.value))
            .collect();
        std::fs::write(self.dir.join("knobs.txt"), text)
    }

    /// The knobs file as `(name, value)` pairs.
    pub fn read_knobs(&self) -> Vec<(String, f32)> {
        std::fs::read_to_string(self.dir.join("knobs.txt"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let (n, v) = l.split_once(char::is_whitespace)?;
                Some((n.trim().to_string(), v.trim().parse().ok()?))
            })
            .collect()
    }

    /// Keeps the recording as it was before its future is cut (recording over it after a rewind),
    /// numbered in `takes/`. Returns the take's number.
    pub fn keep_take(&self, rec: &Recording, at: u64) -> std::io::Result<u32> {
        let dir = self.dir.join("takes");
        std::fs::create_dir_all(&dir)?;
        let n = std::fs::read_dir(&dir)?.count() as u32 + 1;
        std::fs::write(dir.join(format!("take_{n}.txt")), rec.to_text())?;
        self.log(
            at,
            &format!("take {n} kept: the recording as it was before frame {at} was recorded over"),
        );
        Ok(n)
    }

    /// Takes the agent's commands (and empties the file).
    pub fn take_commands(&self) -> Vec<String> {
        let path = self.dir.join("commands.txt");
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        if !text.trim().is_empty() {
            let _ = std::fs::write(&path, "");
        }
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect()
    }

    /// A marked moment: the picture (three times the size), the state and the knobs, and a line
    /// in the log. Returns the mark's number.
    pub fn mark(&mut self, bench: &Bench, note: &str) -> std::io::Result<u32> {
        self.marks += 1;
        let n = self.marks;
        let frame = bench.frame();
        let base = self.dir.join("marks").join(format!("mark_{n}"));
        std::fs::write(base.with_extension("png"), bench.session.canvas.to_png(3))?;
        let knobs: Vec<String> = bench
            .session
            .game()
            .knobs()
            .iter()
            .map(|k| format!("{} {}", k.name, k.value))
            .collect();
        std::fs::write(
            base.with_extension("txt"),
            format!(
                "mark {n}\nframe {frame}\nnote {note}\nstate {}\nknobs {}\n",
                bench.session.state_json(),
                knobs.join(", ")
            ),
        )?;
        self.log(frame, &format!("mark {n} {note}"));
        Ok(n)
    }
}

/// Paints a canvas into another at a whole scale (for the workshop's view of the game).
pub fn blit_scaled(dst: &mut Canvas, src: &Canvas, x0: usize, y0: usize, scale: usize) {
    let (dw, dh) = (dst.width(), dst.height());
    let (sw, sh) = (src.width(), src.height());
    let s = scale.max(1);
    let pixels = src.pixels();
    let out = dst.pixels_mut();
    for sy in 0..sh {
        let row = &pixels[sy * sw..(sy + 1) * sw];
        for r in 0..s {
            let y = y0 + sy * s + r;
            if y >= dh {
                return;
            }
            let line = &mut out[y * dw..(y + 1) * dw];
            for (sx, &p) in row.iter().enumerate() {
                let x = x0 + sx * s;
                if x + s > dw {
                    break;
                }
                line[x..x + s].fill(p);
            }
        }
    }
}
