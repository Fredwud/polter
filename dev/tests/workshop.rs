// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The workshop's model: rewinding is exact, knob changes replay at their tick, taking over cuts
//! the future, the recording survives as text, and the desk keeps marks, knobs and commands.

use polter::{Button, Canvas, Ctx, Frozen, Game, Input, Knob, State};
use polter_dev::workshop::{Bench, Desk, Recording};

/// A runner whose speed is a knob; it leaves a trail, so the picture shows its history. It can
/// copy itself for the workshop's snapshots, or not.
#[derive(Clone)]
struct Runner {
    x: f32,
    speed: f32,
    trail: Vec<i32>,
    copies: bool,
    /// It freezes its moments instead (as a big world does), each said to hold this much.
    freezes: Option<usize>,
}

/// A runner's moment, frozen: here simply a copy, with the memory it says it holds.
struct FrozenRunner {
    runner: Runner,
    bytes: usize,
}

impl Frozen for FrozenRunner {
    fn thaw(&self) -> Box<dyn Game> {
        Box::new(self.runner.clone())
    }
    fn bytes(&self) -> usize {
        self.bytes
    }
}

impl Game for Runner {
    fn update(&mut self, ctx: &mut Ctx) {
        let input: &Input = &ctx.input;
        if input.held(Button::Right) {
            self.x += self.speed;
        }
        if input.pressed(Button::A) {
            self.x += ctx.rng.range(0, 5) as f32;
        }
        self.trail.push(self.x as i32);
    }
    fn draw(&self, c: &mut Canvas, _ctx: &Ctx) {
        c.clear(0x101010);
        for (i, &x) in self.trail.iter().rev().take(40).enumerate() {
            c.set(x % 64, i as i32, 0xffcc00);
        }
    }
    fn size(&self) -> (usize, usize) {
        (64, 48)
    }
    fn state(&self) -> State {
        State::new().num("x", self.x).num("speed", self.speed)
    }
    fn knobs(&self) -> Vec<Knob> {
        vec![Knob::new("speed", self.speed, 0.0, 5.0, 0.1)]
    }
    fn set_knob(&mut self, name: &str, value: f32) {
        if name == "speed" {
            self.speed = value;
        }
    }
    fn snapshot(&self) -> Option<Box<dyn Game>> {
        self.copies.then(|| Box::new(self.clone()) as Box<dyn Game>)
    }
    fn freeze(&self) -> Option<Box<dyn Frozen>> {
        self.freezes.map(|bytes| {
            Box::new(FrozenRunner {
                runner: self.clone(),
                bytes,
            }) as Box<dyn Frozen>
        })
    }
}

fn bench_that(copies: bool) -> Bench {
    bench_keeping(copies, None)
}

fn bench_keeping(copies: bool, freezes: Option<usize>) -> Bench {
    Bench::new(
        Box::new(move || {
            Box::new(Runner {
                x: 0.0,
                speed: 1.0,
                trail: Vec::new(),
                copies,
                freezes,
            })
        }),
        7,
    )
}

fn bench() -> Bench {
    bench_that(true)
}

/// Right held throughout; A pressed now and then, and held across every snapshot's tick (a
/// multiple of 120), so the tick after a snapshot only plays right if the snapshot kept what was
/// held before it.
fn input(f: u64) -> polter::Frame {
    let mut b = Button::Right.bit();
    if f.is_multiple_of(17) || f % 120 <= 1 {
        b |= Button::A.bit();
    }
    polter::Frame::buttons(b)
}

#[test]
fn rewinding_is_exact_and_knob_changes_replay_at_their_tick() {
    // copies, frozen moments, and nothing kept (playing again from the start)
    for (copies, freezes) in [(true, None), (false, Some(1)), (false, None)] {
        rewinding_is_exact(copies, freezes);
    }
}

fn rewinding_is_exact(copies: bool, freezes: Option<usize>) {
    let mut b = bench_keeping(copies, freezes);
    let copies = copies || freezes.is_some();
    let mut states = Vec::new();
    for f in 1..=1000 {
        b.step(&input(f));
        // a knob changed after tick 59 is in effect from the end of tick 59 on: that is the
        // moment a rewind to 59 brings back, so the next tick runs with it
        if f == 59 {
            b.set_knob("speed", 2.5);
        }
        // a knob changed right on a snapshot's tick
        if f == 240 {
            b.set_knob("speed", 0.75);
        }
        states.push((b.session.state_json(), b.session.canvas.hash()));
    }
    assert_eq!(b.snapshots().0, if copies { 8 } else { 0 });
    // back, forward, onto snapshots, just after them, just before them
    for target in [
        1u64, 30, 59, 60, 61, 1000, 120, 119, 121, 240, 241, 999, 239, 600, 480,
    ] {
        b.goto(target);
        assert_eq!(b.frame(), target);
        let (state, hash) = &states[target as usize - 1];
        assert_eq!(
            &b.session.state_json(),
            state,
            "state at {target} ({copies})"
        );
        assert_eq!(
            b.session.canvas.hash(),
            *hash,
            "picture at {target} ({copies})"
        );
    }
    // past the end: the end
    b.goto(5000);
    assert_eq!(b.frame(), 1000);
}

#[test]
fn snapshots_thin_out_in_long_sessions_and_go_with_a_cut_future() {
    let mut b = bench();
    for f in 1..=120 * 200 {
        b.step(&input(f));
    }
    let (n, spacing) = b.snapshots();
    assert!(n <= 128, "{n} snapshots");
    assert_eq!(spacing, 240, "the spacing doubled once");
    assert_eq!(n, 100);
    // taking over at 5000 drops the snapshots after it
    b.goto(5000);
    b.step(&polter::Frame::buttons(0));
    assert_eq!(b.snapshots().0, 5000 / 240);
}

#[test]
fn going_back_to_the_start_does_not_make_the_game_again() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let made = Arc::new(AtomicUsize::new(0));
    let count = made.clone();
    let mut b = Bench::new(
        Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
            Box::new(Runner {
                x: 0.0,
                speed: 1.0,
                trail: Vec::new(),
                copies: false,
                freezes: Some(1),
            })
        }),
        7,
    );
    let mut states = Vec::new();
    for f in 1..=300 {
        b.step(&input(f));
        states.push(b.session.state_json());
    }
    for target in [1u64, 60, 119, 300, 2, 1] {
        b.goto(target);
        assert_eq!(
            b.session.state_json(),
            states[target as usize - 1],
            "at {target}"
        );
    }
    assert_eq!(made.load(Ordering::SeqCst), 1, "made once, at the start");
}

#[test]
fn frozen_moments_stay_under_the_memory_budget() {
    // each moment says it holds 40 MB: no more than six fit in the budget
    let mut b = bench_keeping(false, Some(40 << 20));
    for f in 1..=120 * 50 {
        b.step(&input(f));
        assert!(
            b.snapshot_bytes() <= polter_dev::workshop::MOST_BYTES,
            "{} bytes at tick {f}",
            b.snapshot_bytes()
        );
    }
    let (n, spacing) = b.snapshots();
    assert!((3..=6).contains(&n), "{n} moments kept");
    assert!(spacing > 120, "they thinned out: {spacing} ticks apart");
    // and rewinding still lands exactly
    let at = b.session.state_json();
    b.goto(1000);
    b.goto(6000);
    assert_eq!(b.session.state_json(), at);
}

#[test]
fn playing_on_after_a_rewind_replays_then_taking_over_cuts_the_future() {
    let mut b = bench();
    for f in 1..=50 {
        b.step(&input(f));
    }
    let end_state = b.session.state_json();
    b.goto(20);
    // playing back: the recorded ticks again, to the same end
    while b.replay_step() {}
    assert_eq!(b.frame(), 50);
    assert_eq!(b.session.state_json(), end_state);
    // rewind and take over with other input: the old future is gone
    b.goto(20);
    b.step(&polter::Frame::buttons(0));
    assert_eq!(b.len(), 21, "cut at the takeover");
    assert!(!b.replay_step(), "nothing recorded after it");
}

#[test]
fn a_knob_set_after_a_rewind_cuts_the_future_too() {
    let mut b = bench();
    for f in 1..=40 {
        if f == 30 {
            b.set_knob("speed", 3.0);
        }
        b.step(&input(f));
    }
    b.goto(10);
    b.set_knob("speed", 0.5);
    assert_eq!(b.len(), 10);
    assert_eq!(
        b.rec.knobs.len(),
        1,
        "the change at 30 belonged to the cut future"
    );
    b.step(&input(11));
    assert_eq!(b.session.state_json(), r#"{"x":10.5,"speed":0.5}"#);
}

#[test]
fn a_recording_reads_back_as_written() {
    let mut b = bench();
    for f in 1..=90 {
        if f == 45 {
            b.set_knob("speed", 1.7);
        }
        b.step(&input(f));
    }
    let text = b.rec.to_text();
    let back = Recording::parse(&text).expect("parse");
    assert_eq!(back, b.rec);
    // and it plays to the same state in a fresh bench
    let end = b.session.state_json();
    let mut fresh = bench();
    fresh.load(back, 90);
    assert_eq!(fresh.session.state_json(), end);
}

#[test]
fn the_desk_keeps_marks_knobs_commands_and_a_log() {
    let dir = std::env::temp_dir().join(format!("polter-desk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut desk = Desk::open(&dir).expect("desk");
    let mut b = bench();
    for f in 1..=10 {
        b.step(&input(f));
    }
    assert_eq!(desk.mark(&b, "the jump is late here").unwrap(), 1);
    assert_eq!(desk.mark(&b, "").unwrap(), 2);
    assert!(dir.join("marks/mark_1.png").exists());
    let mark = std::fs::read_to_string(dir.join("marks/mark_1.txt")).unwrap();
    assert!(mark.contains("frame 10") && mark.contains("the jump is late here"));
    assert!(mark.contains(r#""speed":1"#), "{mark}");
    // a desk opened again counts on from the marks there
    let mut again = Desk::open(&dir).unwrap();
    assert_eq!(again.mark(&b, "").unwrap(), 3);
    // knobs: written, edited by an agent, read back
    desk.save_knobs(&b.session.game().knobs()).unwrap();
    std::fs::write(dir.join("knobs.txt"), "speed 4.2\n").unwrap();
    assert_eq!(desk.read_knobs(), vec![("speed".to_string(), 4.2)]);
    // commands are taken once
    std::fs::write(dir.join("commands.txt"), "# from the agent\ngoto 5\nmark\n").unwrap();
    assert_eq!(desk.take_commands(), ["goto 5", "mark"]);
    assert!(desk.take_commands().is_empty());
    // the recording, saved and loaded
    desk.save_recording(&b.rec).unwrap();
    assert_eq!(desk.load_recording(), Some(b.rec.clone()));
    let log = std::fs::read_to_string(dir.join("events.log")).unwrap();
    assert_eq!(log.lines().count(), 3);
    assert!(log.contains("frame 10: mark 1 the jump is late here"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_reload_goes_on_with_the_same_session_where_it_was() {
    let dir = std::env::temp_dir().join(format!("polter-desk-reload-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let desk = Desk::open(&dir).unwrap();
    let mut b = bench();
    for f in 1..=50 {
        b.step(&input(f));
    }
    desk.save_recording(&b.rec).unwrap();
    let r = polter_dev::workshop::Resume {
        frame: 30,
        panel_open: false,
        marks: vec![12, 30],
    };
    desk.save_resume(&r).unwrap();
    // the reloaded workshop: the session goes on, it is not put away as an earlier one
    let again = Desk::open_continuing(&dir).unwrap();
    assert_eq!(again.load_recording(), Some(b.rec.clone()));
    assert!(!dir.join("sessions").exists());
    assert_eq!(
        again.take_resume(),
        Some(r),
        "tick, panel and marks come back"
    );
    assert_eq!(again.take_resume(), None, "a note is used up by reading it");
    // a plain start puts the session away instead
    let fresh = Desk::open(&dir).unwrap();
    assert_eq!(fresh.load_recording(), None);
    assert_eq!(fresh.last_session(), Some(b.rec));
    let _ = std::fs::remove_dir_all(&dir);
}
