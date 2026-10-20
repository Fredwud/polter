// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Polter engine tests: the plumbing every game relies on.

use polter::haunt::Haunt;
use polter::{Button, Col, Palette, Screen, Sprite, png};

#[test]
fn png_is_valid_and_indexed_for_few_colours() {
    let rgb: Vec<u8> = (0..4 * 3).flat_map(|i| [i as u8 * 10, 0, 255]).collect();
    let data = png::encode(4, 3, &rgb);
    assert_eq!(&data[..8], b"\x89PNG\r\n\x1a\n");
    // walk the chunks and verify every CRC
    let mut i = 8;
    let mut kinds = Vec::new();
    while i < data.len() {
        let len = u32::from_be_bytes(data[i..i + 4].try_into().unwrap()) as usize;
        let body = &data[i + 4..i + 8 + len];
        let crc = u32::from_be_bytes(data[i + 8 + len..i + 12 + len].try_into().unwrap());
        let kind = String::from_utf8_lossy(&body[..4]).into_owned();
        assert_eq!(png::crc32(body), crc, "bad CRC in chunk {kind}");
        kinds.push(kind);
        i += 12 + len;
    }
    assert_eq!(kinds, ["IHDR", "PLTE", "IDAT", "IEND"]);
    assert_eq!(data[8 + 8 + 9], 3, "colour type should be indexed");
}

#[test]
fn png_falls_back_to_rgb_for_many_colours() {
    let rgb: Vec<u8> = (0..300u32)
        .flat_map(|i| [(i % 256) as u8, (i / 256) as u8, 7])
        .collect();
    let data = png::encode(300, 1, &rgb);
    assert_eq!(data[8 + 8 + 9], 2, "colour type should be RGB");
}

#[test]
fn haunt_parses_spans_and_reports_bad_lines() {
    let h = Haunt::parse("# comment\n1-3 right\n3 a   # inline comment\n\n10-12\n").unwrap();
    assert_eq!(h.buttons(1), Button::Right.bit());
    assert_eq!(h.buttons(3), Button::Right.bit() | Button::A.bit());
    assert_eq!(h.buttons(4), 0);
    assert_eq!(h.buttons(11), 0);
    let err = Haunt::parse("1-3 right\n5 jump\n").unwrap_err();
    assert!(err.contains("line 2") && err.contains("jump"), "{err}");
    assert!(Haunt::parse("0 a").is_err(), "frames start at 1");
    assert!(Haunt::parse("9-3 a").is_err(), "ranges go upwards");
}

#[test]
fn haunt_recording_round_trips() {
    let (r, a) = (Button::Right.bit(), Button::A.bit());
    let masks = [0, r, r, r | a, r | a, 0, 0, a];
    let h = Haunt::parse(&Haunt::write(&masks)).unwrap();
    for (i, &m) in masks.iter().enumerate() {
        assert_eq!(h.buttons(i as u64 + 1), m, "frame {}", i + 1);
    }
}

#[test]
#[should_panic(expected = "sprite row 1 is 3 wide, expected 4")]
fn sprite_rows_must_be_even() {
    Sprite::from_rows(&["KKKK", "KKK"]);
}

#[test]
fn palette_lookup() {
    assert_eq!(Palette::find("0").unwrap().name, "Chrome & Ember");
    assert_eq!(Palette::find("game").unwrap().name, "Game Boy");
    assert!(Palette::find("nope").is_none());
}

#[test]
fn the_screen_remembers_glyphs_the_font_lacks() {
    let mut s = Screen::new();
    s.text("JUMPS 3 · OK!", 0, 0, Col::Shine);
    assert!(s.missing_glyphs().is_empty());
    s.text("50% § happy §", 0, 10, Col::Shine);
    assert_eq!(
        s.missing_glyphs(),
        ['§'],
        "each missing character is listed once"
    );
}

#[test]
fn the_camera_shifts_drawing_but_not_reading() {
    let mut s = Screen::new();
    s.camera(2, -1);
    s.pset(10, 10, Col::Ember);
    assert_eq!(
        s.pget(8, 11),
        Col::Ember,
        "drawn at (10, 10) minus the camera offset"
    );
    s.rect_fill(10, 20, 1, 1, Col::Visor);
    assert_eq!(
        s.pget(8, 21),
        Col::Visor,
        "rectangles follow the camera too"
    );
    s.camera(0, 0);
    s.pset(10, 10, Col::Shine);
    assert_eq!(s.pget(10, 10), Col::Shine);
}

#[test]
fn scaled_text_is_bigger() {
    let mut s = Screen::new();
    let end = s.text_scaled("I", 0, 0, Col::Shine, 3);
    assert_eq!(end, 12, "one glyph advances 4 px times the scale");
    assert_eq!(Screen::text_width_scaled("II", 3), 21);
    // "I" is ### on top: at scale 3 that's a 9 px wide, 3 px tall bar
    assert_eq!(s.pget(8, 2), Col::Shine);
    assert_eq!(s.pget(9, 2), Col::Black);
}

// ── Input version 2: mouse, wheel, keyboard ──────────────────────────────

use polter::{Cart, Ctx, Frame, Key, Machine, Mouse, State, Storage};

#[test]
fn haunt_mouse_stays_and_glides() {
    let h = Haunt::parse(
        "10 mouse 100,50\n20-30 move 100,50>200,70 click\n40 rclick wheel+2 key:r key:space\n",
    )
    .unwrap();
    assert_eq!(
        h.frame(5).mouse,
        (0, 0),
        "before anything the cursor is at 0,0"
    );
    assert_eq!(
        h.frame(15).mouse,
        (100, 50),
        "the cursor stays where it was put"
    );
    assert_eq!(h.frame(25).mouse, (150, 60), "halfway through the move");
    assert_eq!(h.frame(30).mouse, (200, 70));
    assert_eq!(h.frame(35).mouse, (200, 70), "and stays at the end");
    assert_eq!(h.frame(25).mouse_buttons, Mouse::LEFT);
    let f = h.frame(40);
    assert_eq!((f.mouse_buttons, f.wheel), (Mouse::RIGHT, 2));
    assert_eq!(f.keys, Key::R.bit() | Key::Space.bit());
    assert_eq!(
        h.frame(1000).mouse,
        (200, 70),
        "beyond the script the cursor stays put"
    );
    assert!(Haunt::parse("1 key:nope").is_err());
    assert!(Haunt::parse("1 move 1,2").unwrap_err().contains("X,Y>X,Y"));
}

#[test]
fn full_recordings_round_trip() {
    let mut frames = vec![Frame::default(); 12];
    frames[2].mouse = (10, 20);
    for f in frames.iter_mut().skip(3) {
        f.mouse = (11, 21);
    }
    frames[4].mouse_buttons = Mouse::LEFT;
    frames[5].mouse_buttons = Mouse::LEFT;
    frames[6].wheel = -1;
    frames[7].keys = Key::Num3.bit();
    frames[8].buttons = Button::A.bit();
    let h = Haunt::parse(&Haunt::write_frames(&frames, true)).unwrap();
    for (i, f) in frames.iter().enumerate() {
        assert_eq!(h.frame(i as u64 + 1), *f, "frame {}", i + 1);
    }
}

#[derive(Default)]
struct Echo {
    log: Vec<String>,
}
impl Cart for Echo {
    fn update(&mut self, ctx: &mut Ctx) {
        let i = ctx.input;
        if i.mouse_pressed(Mouse::LEFT) {
            self.log.push(format!("down {:?}", i.mouse()));
        }
        if i.mouse_released(Mouse::LEFT) {
            self.log.push("up".into());
        }
        if i.key_pressed(Key::Q) {
            self.log.push("q".into());
        }
    }
    fn draw(&self, _: &mut Screen, _: &Ctx) {}
    fn state(&self) -> State {
        State::new().text("log", &self.log.join(","))
    }
}

#[test]
fn mouse_and_key_edges_reach_the_cart() {
    let h = Haunt::parse("1 mouse 7,8\n2-3 click\n5-9 key:q\n").unwrap();
    let mut m = Machine::new(Box::<Echo>::default(), 1);
    for f in 1..=10 {
        m.step_frame(&h.frame(f));
    }
    assert_eq!(m.state_json(), r#"{"log":"down (7, 8),up,q"}"#);
    assert_eq!(
        m.cart_as::<Echo>().unwrap().log.len(),
        3,
        "carts can be inspected directly"
    );
}

#[test]
fn storage_keeps_saves_and_refuses_odd_names() {
    let mut s = Storage::memory();
    assert!(s.read("slot1").is_none());
    assert!(s.write("slot1", b"hello"));
    assert_eq!(s.read("slot1").as_deref(), Some(&b"hello"[..]));
    assert!(
        !s.write("../escape", b"x"),
        "no paths outside the save folder"
    );
    assert!(!s.write("", b"x"));
    let dir = std::env::temp_dir().join(format!("polter-storage-test-{}", std::process::id()));
    let mut d = Storage::dir(dir.clone());
    assert!(d.write("save.bin", &[1, 2, 3]));
    assert_eq!(
        Storage::dir(dir.clone()).read("save.bin"),
        Some(vec![1, 2, 3]),
        "files outlive the run"
    );
    let _ = std::fs::remove_dir_all(dir);
}

// ── Games in true colour, sessions, invariants (milestone E1) ─────────────

/// A tiny true-colour game: a square that walks right, and a rule about it.
struct Walker {
    x: i32,
    /// When set, the rule breaks at this x (to see that a broken rule is caught).
    breaks_at: Option<i32>,
}

impl polter::Game for Walker {
    fn update(&mut self, ctx: &mut polter::Ctx) {
        if ctx.input.held(Button::Right) {
            self.x += 1;
        }
    }
    fn draw(&self, canvas: &mut polter::Canvas, _ctx: &polter::Ctx) {
        canvas.clear(0x102030);
        canvas.set(self.x, 5, 0xffcc00);
    }
    fn size(&self) -> (usize, usize) {
        (64, 16)
    }
    fn state(&self) -> polter::State {
        polter::State::new().int("x", self.x as i64)
    }
    fn invariants(&self, check: &mut polter::Invariants) {
        check.require("the square stays left of 50", self.x < 50);
        check.require(
            "the walker is not at its breaking point",
            Some(self.x) != self.breaks_at,
        );
    }
}

#[test]
fn a_true_colour_game_runs_in_a_session_at_its_own_size() {
    let mut s = polter::Session::new(
        Box::new(Walker {
            x: 1,
            breaks_at: None,
        }),
        1,
    );
    assert_eq!((s.canvas.width(), s.canvas.height()), (64, 16));
    for _ in 0..3 {
        s.step(Button::Right.bit());
    }
    assert_eq!(s.state_json(), r#"{"x":4}"#);
    assert_eq!(s.canvas.get(4, 5), 0xffcc00);
    assert_eq!(s.canvas.get(0, 0), 0x102030);
    assert!(s.broken.is_empty());
    assert_eq!(s.game_as::<Walker>().map(|w| w.x), Some(4));
}

#[test]
fn a_broken_invariant_is_named_on_the_tick_it_breaks() {
    let mut s = polter::Session::new(
        Box::new(Walker {
            x: 1,
            breaks_at: Some(3),
        }),
        1,
    );
    s.step(Button::Right.bit());
    assert!(s.broken.is_empty(), "x = 2 breaks nothing");
    s.step(Button::Right.bit());
    assert_eq!(s.broken, ["the walker is not at its breaking point"]);
    s.step(Button::Right.bit());
    assert!(s.broken.is_empty(), "only that tick, not forever after");
}

#[test]
fn a_session_names_its_rules_broken_or_not() {
    let mut s = polter::Session::new(
        Box::new(Walker {
            x: 2,
            breaks_at: Some(3),
        }),
        1,
    );
    let all = [
        "the square stays left of 50",
        "the walker is not at its breaking point",
    ];
    assert_eq!(s.rules(), all, "both, though both hold");
    s.step(Button::Right.bit());
    assert_eq!(s.broken, [all[1]]);
    assert_eq!(s.rules(), all, "and while one is broken");
    // asking changes nothing in the game
    assert_eq!(s.state_json(), r#"{"x":3}"#);
}

#[test]
fn the_console_draws_a_cart_through_its_palette_and_switches_palettes() {
    struct Dot;
    impl polter::Cart for Dot {
        fn update(&mut self, _ctx: &mut polter::Ctx) {}
        fn draw(&self, s: &mut Screen, _ctx: &polter::Ctx) {
            s.cls(Col::Black);
            s.pset(2, 2, Col::Ember);
        }
    }
    let mut m = polter::Machine::new(Box::new(Dot), 1);
    m.step(0);
    let first = m.console().palette().clone();
    assert_eq!(m.canvas.get(2, 2), first.rgb(Col::Ember, 2, 2));
    assert_eq!(m.screen().pget(2, 2), Col::Ember);
    // the window's F1: the next palette, named
    let name = m.game_mut().next_look().expect("a console has palettes");
    assert_ne!(name, first.name);
    m.step(0);
    assert_eq!(
        m.canvas.get(2, 2),
        m.console().palette().rgb(Col::Ember, 2, 2)
    );
}

#[test]
fn old_saves_are_copied_once_and_the_old_folder_stays() {
    let root = std::env::temp_dir().join(format!("polter-saves-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (old, new) = (root.join("old/overburden"), root.join("new/overburden"));
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join("slot1"), b"before").unwrap();
    std::fs::write(old.join("autosave"), b"auto").unwrap();
    assert_eq!(polter::storage::bring_old_saves(&old, &new).unwrap(), 2);
    assert_eq!(std::fs::read(new.join("slot1")).unwrap(), b"before");
    assert_eq!(
        std::fs::read(old.join("slot1")).unwrap(),
        b"before",
        "the old one stays"
    );
    // played since: a second start must not copy the old saves over the new ones
    std::fs::write(new.join("slot1"), b"after").unwrap();
    assert_eq!(polter::storage::bring_old_saves(&old, &new).unwrap(), 0);
    assert_eq!(std::fs::read(new.join("slot1")).unwrap(), b"after");
    assert!(
        !new.with_extension("partial").exists(),
        "no half folder left"
    );
    // nothing old: nothing happens
    assert_eq!(
        polter::storage::bring_old_saves(&root.join("none"), &root.join("new/x")).unwrap(),
        0
    );
    assert!(!root.join("new/x").exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn saves_live_in_the_systems_folder_for_data() {
    let dir = polter::storage::save_dir("girder");
    assert!(dir.ends_with("girder"));
    if cfg!(target_os = "macos") {
        assert!(
            dir.to_string_lossy()
                .contains("Library/Application Support")
        );
    }
}

#[test]
fn every_key_has_one_name_and_old_keys_keep_their_bits() {
    use polter::Key;
    let mut names = std::collections::BTreeSet::new();
    for k in Key::ALL {
        let n = k.name();
        assert!(names.insert(n.clone()), "{n} named twice");
        assert_eq!(Key::from_name(&n), Some(k), "{n} reads back");
    }
    assert_eq!(names.len(), 62);
    // recordings made before E2 keep their meaning
    assert_eq!(Key::Escape.bit(), 1 << 42);
    assert_eq!(Key::A.bit(), 1);
    assert_eq!(Key::F1.name(), "f1");
    assert_eq!(Key::F12.name(), "f12");
    assert_eq!(Key::Super.bit(), 1 << 61);
}

#[test]
fn a_session_resized_draws_at_its_new_size() {
    struct Fill;
    impl polter::Game for Fill {
        fn update(&mut self, _ctx: &mut polter::Ctx) {}
        fn draw(&self, c: &mut polter::Canvas, _ctx: &polter::Ctx) {
            let (w, h) = (c.width() as i32, c.height() as i32);
            c.clear(0x000000);
            c.set(w - 1, h - 1, 0xffffff);
        }
        fn fit(&self) -> polter::window::Fit {
            polter::window::Fit::Expand
        }
    }
    let mut s = polter::Session::new(Box::new(Fill), 1);
    assert_eq!((s.canvas.width(), s.canvas.height()), (320, 180));
    s.resize(360, 225);
    s.step(0);
    assert_eq!((s.canvas.width(), s.canvas.height()), (360, 225));
    assert_eq!(s.canvas.get(359, 224), 0xffffff, "drawn to the new corner");
}
