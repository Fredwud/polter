// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The command line every Polter game gets for free: play in a window, or run headless for agents.
//! Every command runs a `Session`, so a console cart and a true-colour game answer the same way.
//!
//! A console cart's `main` is one call (a true-colour game uses `run_games` and `GameInfo`):
//!
//! ```no_run
//! # use polter::{Cart, Ctx, Screen};
//! # struct Hello;
//! # impl Cart for Hello { fn update(&mut self, _: &mut Ctx) {} fn draw(&self, _: &mut Screen, _: &Ctx) {} }
//! fn main() -> std::process::ExitCode {
//!     polter_dev::cli::run(&[polter_dev::cli::CartInfo { name: "hello", about: "Says hello.", make: || Box::new(Hello) }])
//! }
//! ```

use crate::ears::ears;
use crate::scenario::{Scenario, parse_state};
use crate::sheet;
use polter::audio::{SAMPLE_RATE, wav};
use polter::haunt::Haunt;
use polter::input::Frame;
use polter::state::quote;
use polter::{Canvas, Cart, Col, Console, Game, Palette, Session, Storage, palettes, png};
use std::panic::{self, AssertUnwindSafe};
use std::process::ExitCode;
use std::sync::Mutex;

mod brief;

/// A console cart the command line can run.
pub struct CartInfo {
    pub name: &'static str,
    pub about: &'static str,
    pub make: fn() -> Box<dyn Cart>,
}

/// A true-colour game the command line can run.
pub struct GameInfo {
    pub name: &'static str,
    pub about: &'static str,
    pub make: fn() -> Box<dyn Game>,
}

/// Either kind, as the commands see it.
enum Maker {
    Cart(fn() -> Box<dyn Cart>),
    Game(fn() -> Box<dyn Game>),
}

struct Entry {
    name: &'static str,
    about: &'static str,
    make: Maker,
}

/// The name this program was started as, e.g. `girder`.
fn program() -> String {
    std::env::args()
        .next()
        .and_then(|p| {
            std::path::Path::new(&p)
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "polter".into())
}

fn help() -> String {
    format!(
        "{p} · a Polter game

USAGE: {p} <command> [cart] [options]   (the cart can be left out if there is only one)

  carts                     list the carts in this program
  play                      play in a window: arrows, Z = A, X = B, mouse and keys if the cart uses them,
                            F1 (or P) = palette or look, Esc or Cmd+Q = quit (desktop carts: Cmd+Q)
  check                     run headless, report panics with file:line, broken invariants and
                            glyphs the font lacks
  probe                     print the cart's state every N frames
  peek                      write frames as PNG plus a numbered contact sheet (no window needed)
  shot --frame N            write one frame as PNG
  clip                      write the run as an animated GIF a web page plays (--frame N: the first
                            frame, default 1; --frames N: how many; --step K: every Kth tick, at
                            least 2, default 2 = 30 frames a second; --scale S; --out FILE)
  twin                      run twice, compare picture hash and state on every frame
  listen                    render the sound as WAV plus a picture (waveform, spectrogram, sound names)
  knobs                     list the game's tuning values: name, value, range, step
  workshop                  a tape deck for the game, to play, rewind and tune it while an agent
                            works on it: the game, a panel (knobs on faders, rules, state) and the
                            deck (the tape to scrub; keys START REW PLAY PAUSE FF STEP REC CUE for
                            the mouse, or Alt+0 Left Space Right Down R M, or F5 to F10; Alt+H
                            folds the panel); it shares a folder with the agent (--dir, default:
                            the game's data folder/workshop)
                            --resume: the last session again, at its end (every session is kept
                            in the folder's sessions/)
                            --continue: the session in the folder goes on, at the tick it was
                            left; the workshop reloads itself this way when its program is built
                            anew (or when an agent writes 'reload' to commands.txt)
                            --out FILE: a picture of the workshop instead of the window (--size W,H)
  assert FILE.scenario …    run scenarios: a start, inputs, and what must be true at which ticks,
                            checked on every tick (--scenarios DIR runs every .scenario in DIR)
  brief                     one page about the game for an agent, measured now: what it is
                            (BRIEF.md), its state, rules, knobs, scenarios, sounds, time per frame,
                            the person's marks, files, recent changes (--project DIR: the game's
                            folder, default here; --frames N: the run, default 600; --out FILE)

OPTIONS
  --frames N     frames to run (default 120; play: unlimited)
  --haunt FILE   input script (.haunt)          --seed N       random seed (default 1)
  --palette P    palette name or index          --json         machine-readable output
  --every N      probe: every Nth frame (default 10)
  --out FILE     probe: save states as JSON lines · shot: PNG path · clip: GIF path
  --dir DIR      peek/listen: output folder (default: new temp folder)
  --step N       peek: every Nth frame (default: at most 24 frames) · clip: every Nth tick (2)
  --all          peek: save every frame, not only the ones on the sheet
  --scale N      peek/shot/clip: scale (default 2) · --columns N  peek: sheet columns (default 4)
  --record FILE  play: save what you play as a .haunt file (saved continuously)
  --perf         check: milliseconds per frame against the 60 FPS budget (use a --release build)
  --storage DIR  where save games live (play: the system's folder for data, e.g.
                 ~/Library/Application Support/<cart>; headless: memory)

Every command stops at the first panic or broken invariant and names the frame.
Exit codes: 0 clean · 1 the game is wrong (panic, broken invariant, twin found a difference)
            · 2 tool problem · 3 bad invocation",
        p = program()
    )
}

#[derive(Default)]
struct Opts {
    command: String,
    cart: Option<String>,
    frames: Option<u64>,
    frame: Option<u64>,
    haunt: Option<String>,
    seed: u64,
    palette: Option<String>,
    every: u64,
    out: Option<String>,
    dir: Option<String>,
    step: Option<usize>,
    all: bool,
    scale: usize,
    columns: usize,
    record: Option<String>,
    json: bool,
    perf: bool,
    storage: Option<String>,
    scenarios: Vec<String>,
    size: Option<(usize, usize)>,
    resume: bool,
    continuing: bool,
    project: Option<String>,
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        seed: 1,
        every: 10,
        scale: 2,
        columns: 4,
        ..Opts::default()
    };
    let mut it = args.iter();
    o.command = it.next().cloned().unwrap_or_default();
    while let Some(a) = it.next() {
        let mut value = |name: &str| it.next().cloned().ok_or(format!("{name} needs a value"));
        let number = |v: String, name: &str| {
            v.parse::<u64>()
                .map_err(|_| format!("{name} needs a number, got {v:?}"))
        };
        match a.as_str() {
            "--frames" => o.frames = Some(number(value(a)?, a)?),
            "--frame" => o.frame = Some(number(value(a)?, a)?),
            "--haunt" => o.haunt = Some(value(a)?),
            "--seed" => o.seed = number(value(a)?, a)?,
            "--palette" => o.palette = Some(value(a)?),
            "--every" => o.every = number(value(a)?, a)?.max(1),
            "--out" => o.out = Some(value(a)?),
            "--dir" => o.dir = Some(value(a)?),
            "--step" => o.step = Some(number(value(a)?, a)?.max(1) as usize),
            "--all" => o.all = true,
            "--scale" => o.scale = number(value(a)?, a)?.clamp(1, 8) as usize,
            "--columns" => o.columns = number(value(a)?, a)?.max(1) as usize,
            "--record" => o.record = Some(value(a)?),
            "--json" => o.json = true,
            "--resume" => o.resume = true,
            "--continue" => o.continuing = true,
            "--project" => o.project = Some(value(a)?),
            "--perf" => o.perf = true,
            "--storage" => o.storage = Some(value(a)?),
            "--size" => {
                let v = value(a)?;
                let (w, h) = v
                    .split_once(',')
                    .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                    .ok_or(format!("--size needs W,H, got {v:?}"))?;
                o.size = Some((w, h));
            }
            "--scenarios" => {
                let dir = value(a)?;
                let mut found: Vec<String> = std::fs::read_dir(&dir)
                    .map_err(|e| format!("--scenarios {dir}: {e}"))?
                    .filter_map(|f| f.ok().map(|f| f.path()))
                    .filter(|p| p.extension().is_some_and(|x| x == "scenario"))
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect();
                found.sort();
                o.scenarios.extend(found);
            }
            s if s.ends_with(".scenario") => o.scenarios.push(s.to_string()),
            "-h" | "--help" => o.command = "help".into(),
            s if s.starts_with("--") => return Err(format!("unknown option {s}")),
            s if o.cart.is_none() => o.cart = Some(s.to_string()),
            s => return Err(format!("unexpected argument {s:?}")),
        }
    }
    Ok(o)
}

// ── Running headless, with panics caught and located ─────────────────────

static LAST_PANIC: Mutex<Option<(String, String)>> = Mutex::new(None);

fn quiet_panics() {
    panic::set_hook(Box::new(|info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".into());
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        *LAST_PANIC.lock().unwrap() = Some((message, location));
    }));
}

/// Why a run stopped: the game panicked, or one of its invariants broke.
enum Fail {
    Panic { message: String, location: String },
    Broken { rules: Vec<String>, state: String },
}

struct Panic {
    frame: u64,
    fail: Fail,
}

impl Panic {
    fn json(&self) -> String {
        match &self.fail {
            Fail::Panic { message, location } => format!(
                "{{\"frame\":{},\"kind\":\"panic\",\"message\":{},\"location\":{}}}",
                self.frame,
                quote(message),
                quote(location)
            ),
            Fail::Broken { rules, state } => format!(
                "{{\"frame\":{},\"kind\":\"invariant\",\"broken\":[{}],\"state\":{}}}",
                self.frame,
                rules.iter().map(|r| quote(r)).collect::<Vec<_>>().join(","),
                state
            ),
        }
    }
}

/// One tick of a session, with a panic caught and located, and broken invariants reported.
fn step_checked(s: &mut Session, input: &Frame) -> Option<Panic> {
    let frame = s.ctx.frame + 1;
    if panic::catch_unwind(AssertUnwindSafe(|| s.step_frame(input))).is_err() {
        let (message, location) = LAST_PANIC.lock().unwrap().take().unwrap_or_default();
        return Some(Panic {
            frame,
            fail: Fail::Panic { message, location },
        });
    }
    (!s.broken.is_empty()).then(|| Panic {
        frame,
        fail: Fail::Broken {
            rules: s.broken.clone(),
            state: s.state_json(),
        },
    })
}

/// Runs a session for `frames` frames, calling `each` after every frame. Stops at the first
/// panic or broken invariant. `look` says which frames' pictures will be looked at; the others
/// need not be painted (`Ctx::picture`).
fn run_frames(
    m: &mut Session,
    frames: u64,
    haunt: &Haunt,
    look: impl Fn(u64) -> bool,
    mut each: impl FnMut(&Session),
) -> Option<Panic> {
    for _ in 0..frames {
        let next = m.ctx.frame + 1;
        m.ctx.picture = look(next);
        if let Some(p) = step_checked(m, &haunt.frame(next)) {
            return Some(p);
        }
        each(m);
    }
    None
}

// ── Commands ─────────────────────────────────────────────────────────────

struct Env<'a> {
    o: &'a Opts,
    cart: &'a Entry,
    haunt: Haunt,
    /// For a console cart, the palette it is shown in (an index into its palettes).
    pal_index: usize,
    /// The palette the tools draw their own pictures with (contact sheets, sound pictures):
    /// the cart's, or the built-in one for a true-colour game.
    pal: Palette,
}

impl Env<'_> {
    fn machine(&self) -> Session {
        let game: Box<dyn Game> = match self.cart.make {
            Maker::Cart(make) => {
                let mut console = Console::new(make());
                console.set_palette(self.pal_index);
                Box::new(console)
            }
            Maker::Game(make) => make(),
        };
        let mut m = Session::new(game, self.o.seed);
        if let Some(dir) = &self.o.storage {
            m.ctx.storage = Storage::dir(dir.into());
        }
        m
    }

    fn frames(&self) -> u64 {
        self.o.frames.unwrap_or(120)
    }

    fn header(&self, command: &str) {
        println!(
            "Polter · {command} · {} · {} frames, seed {}",
            self.cart.name,
            self.frames(),
            self.o.seed
        );
    }
}

fn report_panic(p: &Option<Panic>) {
    match p {
        None => println!("  ✓ No panics, no broken invariants."),
        Some(Panic {
            frame,
            fail: Fail::Panic { message, location },
        }) => println!("  ✗ Panic in frame {frame} at {location}: {message}"),
        Some(Panic {
            frame,
            fail: Fail::Broken { rules, state },
        }) => {
            println!(
                "  ✗ Invariant broken in frame {frame}: {}",
                rules.join("; ")
            );
            println!("    state: {state}");
        }
    }
}

fn panic_json(p: &Option<Panic>) -> String {
    p.as_ref().map_or("null".into(), Panic::json)
}

/// How often each sound played, in order of first appearance, e.g. `jump ×5, land ×7`.
fn sound_counts(names: &[&str]) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for n in names {
        match counts.iter_mut().find(|(k, _)| k == n) {
            Some((_, c)) => *c += 1,
            None => counts.push((n.to_string(), 1)),
        }
    }
    counts
}

struct Perf {
    avg: f64,
    p95: f64,
    max: f64,
    over: usize,
}

/// Average, 95th percentile and maximum of per-frame times; frames over the 60 FPS budget.
fn perf_summary(times: &mut [f64]) -> Option<Perf> {
    if times.is_empty() {
        return None;
    }
    let over = times.iter().filter(|&&t| t > 1000.0 / 60.0).count();
    let avg = times.iter().sum::<f64>() / times.len() as f64;
    times.sort_by(|a, b| a.total_cmp(b));
    let p95 = times[((times.len() as f64 * 0.95) as usize).min(times.len() - 1)];
    Some(Perf {
        avg,
        p95,
        max: times[times.len() - 1],
        over,
    })
}

fn cmd_check(e: &Env) -> u8 {
    let mut m = e.machine();
    let mut played: Vec<&str> = Vec::new();
    let mut times: Vec<f64> = Vec::new();
    let mut last = std::time::Instant::now();
    let last_frame = e.frames();
    let p = run_frames(
        &mut m,
        last_frame,
        &e.haunt,
        |f| f == last_frame,
        |m| {
            played.extend(m.sounds.iter().map(|s| s.name));
            if e.o.perf {
                let now = std::time::Instant::now();
                times.push((now - last).as_secs_f64() * 1000.0);
                last = now;
            }
        },
    );
    let counts = sound_counts(&played);
    let frames_timed = times.len();
    let perf = perf_summary(&mut times);
    if e.o.json {
        let perf_json = perf.as_ref().map_or("null".to_string(), |p| {
            format!(
                "{{\"avg_ms\":{:.3},\"p95_ms\":{:.3},\"max_ms\":{:.3},\"over_budget\":{}}}",
                p.avg, p.p95, p.max, p.over
            )
        });
        println!(
            "{{\"command\":\"check\",\"cart\":{},\"exit\":{},\"frames\":{},\"panic\":{},\"missing_glyphs\":{},\"sounds\":{{{}}},\"perf\":{},\"hash\":\"{:016x}\",\"state\":{}}}",
            quote(e.cart.name),
            p.is_some() as u8,
            m.ctx.frame,
            panic_json(&p),
            quote(&m.game().missing_glyphs().iter().collect::<String>()),
            counts
                .iter()
                .map(|(k, c)| format!("{}:{c}", quote(k)))
                .collect::<Vec<_>>()
                .join(","),
            perf_json,
            m.canvas.hash(),
            m.state_json()
        );
    } else {
        e.header("check");
        report_panic(&p);
        let missing = m.game().missing_glyphs();
        if !missing.is_empty() {
            let list: Vec<String> = missing.iter().map(|c| format!("{c:?}")).collect();
            println!(
                "  ! Drew characters the font doesn't have (they show as boxes): {}",
                list.join(" ")
            );
        }
        if !counts.is_empty() {
            let list: Vec<String> = counts.iter().map(|(k, c)| format!("{k} ×{c}")).collect();
            println!("  Sounds: {}", list.join(", "));
        }
        if let Some(p) = &perf {
            println!(
                "  Frame time: avg {:.2} ms · p95 {:.2} ms · max {:.2} ms · {} of {frames_timed} frames over the 16.7 ms budget{}",
                p.avg,
                p.p95,
                p.max,
                p.over,
                if cfg!(debug_assertions) {
                    " (debug build: measure with --release)"
                } else {
                    ""
                }
            );
        }
        println!("  Final state (frame {}): {}", m.ctx.frame, m.state_json());
    }
    p.is_some() as u8
}

fn cmd_probe(e: &Env) -> u8 {
    let mut m = e.machine();
    let (every, last) = (e.o.every, e.frames());
    let mut states = Vec::new();
    let p = run_frames(
        &mut m,
        last,
        &e.haunt,
        |_| false,
        |m| {
            let f = m.ctx.frame;
            if f == 1 || f % every == 0 || f == last {
                states.push(format!(
                    "{{\"type\":\"state\",\"frame\":{f},\"data\":{}}}",
                    m.state_json()
                ));
            }
        },
    );
    if let Some(path) = &e.o.out
        && let Err(err) = std::fs::write(path, states.join("\n") + "\n")
    {
        eprintln!("cannot write {path}: {err}");
        return 2;
    }
    if e.o.json {
        println!(
            "{{\"command\":\"probe\",\"cart\":{},\"exit\":{},\"panic\":{},\"states\":[{}]}}",
            quote(e.cart.name),
            p.is_some() as u8,
            panic_json(&p),
            states.join(",")
        );
    } else {
        e.header("probe");
        for s in &states {
            println!("  {s}");
        }
        report_panic(&p);
    }
    p.is_some() as u8
}

fn temp_dir(prefix: &str) -> std::io::Result<String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
    std::fs::create_dir_all(&dir)?;
    Ok(dir.to_string_lossy().into_owned())
}

fn save_png(path: &str, canvas: &Canvas, scale: usize) -> std::io::Result<()> {
    std::fs::write(path, canvas.to_png(scale))
}

fn cmd_peek(e: &Env) -> u8 {
    let dir = match &e.o.dir {
        Some(d) => std::fs::create_dir_all(d).map(|_| d.clone()),
        None => temp_dir("polter-peek"),
    };
    let Ok(dir) = dir else {
        eprintln!("cannot create the output folder");
        return 2;
    };
    let frames = e.frames();
    let step =
        e.o.step
            .unwrap_or_else(|| (frames as usize).div_ceil(24).max(1)) as u64;
    let mut picked: Vec<(u64, Canvas)> = Vec::new();
    let mut written = 0;
    let mut io_error = None;
    let mut m = e.machine();
    let all = e.o.all;
    let wanted = |f: u64| all || (f - 1).is_multiple_of(step) || f == frames;
    let p = run_frames(&mut m, frames, &e.haunt, wanted, |m| {
        let f = m.ctx.frame;
        let on_sheet = (f - 1).is_multiple_of(step) || f == frames;
        if on_sheet {
            picked.push((f, m.canvas.clone()));
        }
        if on_sheet || e.o.all {
            let path = format!("{dir}/frame_{f:04}.png");
            match save_png(&path, &m.canvas, e.o.scale) {
                Ok(()) => written += 1,
                Err(err) => io_error = Some(err),
            }
        }
    });
    if let Some(err) = io_error {
        eprintln!("cannot write frames: {err}");
        return 2;
    }
    let sheet_path = format!("{dir}/contact_sheet.png");
    let (bg, ink) = (e.pal.rgb(Col::Black, 0, 0), e.pal.rgb(Col::Shine, 0, 0));
    let (w, h, rgb) = sheet::contact_sheet(&picked, e.o.columns, bg, ink);
    if let Err(err) = std::fs::write(&sheet_path, png::encode(w, h, &rgb)) {
        eprintln!("cannot write the contact sheet: {err}");
        return 2;
    }
    let last = format!("{dir}/frame_{:04}.png", m.ctx.frame);
    if e.o.json {
        println!(
            "{{\"command\":\"peek\",\"cart\":{},\"exit\":{},\"panic\":{},\"dir\":{},\"frames_written\":{written},\"contact_sheet\":{},\"last_frame\":{}}}",
            quote(e.cart.name),
            p.is_some() as u8,
            panic_json(&p),
            quote(&dir),
            quote(&sheet_path),
            quote(&last)
        );
    } else {
        e.header("peek");
        println!(
            "  {written} frames written to {dir} (frame_0001.png = frame 1, scale {}×)",
            e.o.scale
        );
        println!(
            "  Contact sheet: {sheet_path} ({} frames, every {step})",
            picked.len()
        );
        println!("  Last frame:    {last}");
        report_panic(&p);
    }
    p.is_some() as u8
}

fn cmd_shot(e: &Env) -> u8 {
    let Some(target) = e.o.frame else {
        eprintln!("shot needs --frame N");
        return 3;
    };
    let mut m = e.machine();
    let p = run_frames(&mut m, target, &e.haunt, |f| f == target, |_| {});
    let path =
        e.o.out
            .clone()
            .unwrap_or_else(|| format!("{}_{target:04}.png", e.cart.name));
    if let Err(err) = save_png(&path, &m.canvas, e.o.scale) {
        eprintln!("cannot write {path}: {err}");
        return 2;
    }
    if e.o.json {
        println!(
            "{{\"command\":\"shot\",\"cart\":{},\"exit\":{},\"panic\":{},\"file\":{}}}",
            quote(e.cart.name),
            p.is_some() as u8,
            panic_json(&p),
            quote(&path)
        );
    } else {
        println!(
            "Polter · shot · {} · frame {} → {path}",
            e.cart.name, m.ctx.frame
        );
        report_panic(&p);
    }
    p.is_some() as u8
}

fn cmd_clip(e: &Env) -> u8 {
    let first = e.o.frame.unwrap_or(1).max(1);
    let last = first + e.frames() - 1;
    // GIF counts in hundredths of a second, and browsers stretch a delay under two of them:
    // every second tick (30 frames a second) is the finest a page plays as written
    let step = e.o.step.unwrap_or(2).max(2) as u64;
    let wanted = |f: u64| f >= first && f <= last && (f - first).is_multiple_of(step);
    let mut m = e.machine();
    let mut shots: Vec<Vec<u32>> = Vec::new();
    let (mut w, mut h) = (0, 0);
    let p = run_frames(&mut m, last, &e.haunt, wanted, |m| {
        if wanted(m.ctx.frame) {
            let (cw, ch, rgb) = m.canvas.to_rgb(1);
            (w, h) = (cw, ch);
            shots.push(
                rgb.chunks(3)
                    .map(|c| ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32)
                    .collect(),
            );
        }
    });
    if shots.is_empty() {
        eprintln!("the run ended before frame {first}");
        return p.is_some() as u8;
    }
    let delays = crate::gif::centis(shots.len(), step);
    let frames: Vec<crate::gif::Frame> = shots
        .into_iter()
        .zip(delays)
        .map(|(pixels, centis)| crate::gif::Frame { pixels, centis })
        .collect();
    let shown = frames.len();
    let gif = crate::gif::encode(w, h, e.o.scale, &frames);
    let path =
        e.o.out
            .clone()
            .unwrap_or_else(|| format!("{}.gif", e.cart.name));
    if let Err(err) = std::fs::write(&path, &gif.bytes) {
        eprintln!("cannot write {path}: {err}");
        return 2;
    }
    let seconds = (shown as u64 * step) as f64 / 60.0;
    if e.o.json {
        println!(
            "{{\"command\":\"clip\",\"cart\":{},\"exit\":{},\"panic\":{},\"file\":{},\"bytes\":{},\"frames\":{},\"seconds\":{seconds:.2},\"colours\":{},\"exact\":{}}}",
            quote(e.cart.name),
            p.is_some() as u8,
            panic_json(&p),
            quote(&path),
            gif.bytes.len(),
            gif.frames,
            gif.colours,
            gif.exact
        );
    } else {
        println!(
            "Polter · clip · {} · frames {first} to {} → {path}",
            e.cart.name, m.ctx.frame
        );
        println!(
            "  {seconds:.1} s, {shown} frames ({} stored, one every {step} ticks), {}×{} pixels, {} KB",
            gif.frames,
            w * e.o.scale.max(1),
            h * e.o.scale.max(1),
            gif.bytes.len().div_ceil(1024)
        );
        println!(
            "  {} colours{}",
            gif.colours,
            if gif.exact {
                ", all kept"
            } else {
                ", shown with 255"
            }
        );
        report_panic(&p);
    }
    p.is_some() as u8
}

fn cmd_listen(e: &Env) -> u8 {
    let dir = match &e.o.dir {
        Some(d) => std::fs::create_dir_all(d).map(|_| d.clone()),
        None => temp_dir("polter-listen"),
    };
    let Ok(dir) = dir else {
        eprintln!("cannot create the output folder");
        return 2;
    };
    let mut m = e.machine();
    m.enable_audio();
    let (mut samples, mut events) = (Vec::new(), Vec::new());
    let p = run_frames(
        &mut m,
        e.frames(),
        &e.haunt,
        |_| false,
        |m| {
            samples.extend_from_slice(&m.audio);
            events.extend(m.sounds.iter().map(|s| (m.ctx.frame, s.name)));
        },
    );
    let (wav_path, ears_path) = (format!("{dir}/sound.wav"), format!("{dir}/ears.png"));
    let (w, h, rgb) = ears(&samples, &events, &e.pal);
    if let Err(err) = std::fs::write(&wav_path, wav(&samples))
        .and_then(|_| std::fs::write(&ears_path, png::encode(w, h, &rgb)))
    {
        eprintln!("cannot write the sound files: {err}");
        return 2;
    }
    let peak = samples.iter().fold(0f32, |a, s| a.max(s.abs()));
    let clipped = samples.iter().filter(|s| s.abs() >= 0.999).count();
    if e.o.json {
        let list: Vec<String> = events
            .iter()
            .map(|(f, n)| format!("{{\"frame\":{f},\"name\":{}}}", quote(n)))
            .collect();
        println!(
            "{{\"command\":\"listen\",\"cart\":{},\"exit\":{},\"panic\":{},\"events\":[{}],\"peak\":{peak:.3},\"clipped\":{clipped},\"wav\":{},\"ears\":{}}}",
            quote(e.cart.name),
            p.is_some() as u8,
            panic_json(&p),
            list.join(","),
            quote(&wav_path),
            quote(&ears_path)
        );
    } else {
        e.header("listen");
        let list: Vec<String> = events.iter().map(|(f, n)| format!("{f} {n}")).collect();
        println!(
            "  {} sounds: {}",
            events.len(),
            if list.is_empty() {
                "none".into()
            } else {
                list.join(" · ")
            }
        );
        println!(
            "  Peak {peak:.2}, {clipped} clipped samples ({:.1} s at {SAMPLE_RATE} Hz)",
            samples.len() as f32 / SAMPLE_RATE as f32
        );
        println!("  Sound:   {wav_path}");
        println!("  Picture: {ears_path} (waveform, spectrogram from 80 Hz to 8 kHz, sound names)");
        report_panic(&p);
    }
    p.is_some() as u8
}

fn cmd_twin(e: &Env) -> u8 {
    let (mut a, mut b) = (e.machine(), e.machine());
    let mut difference = None;
    let mut panicked = None;
    for f in 1..=e.frames() {
        let input = e.haunt.frame(f);
        if let Some(p) = step_checked(&mut a, &input).or_else(|| step_checked(&mut b, &input)) {
            panicked = Some(p);
            break;
        }
        // both pictures are at hand: compared pixel for pixel, which is faster than hashing
        let (sa, sb) = (a.state_json(), b.state_json());
        let pictures_differ = a.canvas != b.canvas;
        if sa != sb || pictures_differ {
            difference = Some((f, sa, sb, pictures_differ));
            break;
        }
    }
    let code = (panicked.is_some() || difference.is_some()) as u8;
    if e.o.json {
        let diff = difference
            .as_ref()
            .map_or("null".into(), |(f, sa, sb, pix)| {
                format!("{{\"frame\":{f},\"screen_differs\":{pix},\"run1\":{sa},\"run2\":{sb}}}")
            });
        println!(
            "{{\"command\":\"twin\",\"cart\":{},\"exit\":{code},\"identical\":{},\"frames\":{},\"panic\":{},\"difference\":{diff}}}",
            quote(e.cart.name),
            difference.is_none() && panicked.is_none(),
            a.ctx.frame,
            panic_json(&panicked)
        );
    } else {
        e.header("twin");
        match (&difference, &panicked) {
            (None, None) => println!(
                "  ✓ Two runs, {} frames, identical pictures and state. Deterministic.",
                a.ctx.frame
            ),
            (Some((f, sa, sb, pix)), _) => {
                println!(
                    "  ✗ First difference in frame {f}{}",
                    if *pix { " (the picture differs)" } else { "" }
                );
                println!("    run 1: {sa}\n    run 2: {sb}");
            }
            (None, p) => report_panic(p),
        }
    }
    code
}

/// What became of one expectation: first failing tick and the value there, or why it couldn't
/// be judged.
struct Verdict {
    failed_at: Option<(u64, String)>,
    error: Option<String>,
    checked: u64,
}

fn cmd_workshop(e: &Env) -> u8 {
    use crate::workshop::{Bench, Desk, Workshop};
    let dir = e.o.dir.clone().map_or_else(
        || polter::storage::save_dir(e.cart.name).join("workshop"),
        std::path::PathBuf::from,
    );
    let opened = if e.o.continuing {
        Desk::open_continuing(&dir)
    } else {
        Desk::open(&dir)
    };
    let desk = match opened {
        Ok(d) => d,
        Err(err) => {
            eprintln!("cannot open the workshop's folder {}: {err}", dir.display());
            return 2;
        }
    };
    let make: Box<dyn Fn() -> Box<dyn Game>> = match e.cart.make {
        Maker::Cart(make) => {
            let pal = e.pal_index;
            Box::new(move || {
                let mut console = Console::new(make());
                console.set_palette(pal);
                Box::new(console)
            })
        }
        Maker::Game(make) => Box::new(make),
    };
    let mut bench = Bench::new(make, e.o.seed);
    if let Some(dir) = &e.o.storage {
        bench.session.ctx.storage = Storage::dir(dir.into());
    }
    // the session going on after a reload; the last session again, at its end; or a haunt
    // played into the bench first (for pictures, or to start from a scene)
    let mut resume = None;
    if e.o.continuing {
        match desk.load_recording() {
            Some(rec) => {
                let r = desk.take_resume().unwrap_or(crate::workshop::Resume {
                    frame: rec.inputs.len() as u64,
                    panel_open: true,
                    marks: Vec::new(),
                });
                bench.load(rec, r.frame);
                resume = Some(r);
            }
            None => eprintln!(
                "no session in {} to go on with: starting fresh",
                dir.display()
            ),
        }
    } else if e.o.resume {
        match desk.last_session() {
            Some(rec) => {
                let end = rec.inputs.len() as u64;
                bench.load(rec, end);
            }
            None => eprintln!("no earlier session in {}: starting fresh", dir.display()),
        }
    } else if e.o.haunt.is_some() {
        for f in 1..=e.frames() {
            bench.step(&e.haunt.frame(f));
        }
    }
    let size = bench.session.game().size();
    let mut shop = Workshop::new(e.cart.name, bench, desk);
    if let Some(r) = resume {
        shop = shop.resumed(r);
    }
    if let Some(out) = &e.o.out {
        // a picture of the workshop, no window
        let (w, h) = e.o.size.unwrap_or((480, 270));
        let mut canvas = Canvas::new(w, h);
        shop.mode = crate::workshop::Mode::Paused;
        shop.draw(&mut canvas);
        if let Err(err) = std::fs::write(out, canvas.to_png(e.o.scale)) {
            eprintln!("cannot write {out}: {err}");
            return 2;
        }
        println!("{out}");
        return 0;
    }
    println!("Polter · workshop · {}", e.cart.name);
    println!("  shared folder: {}", dir.display());
    println!("  the deck's keys work with the mouse; or Option+R record · Option+Space play/pause");
    println!(
        "  Option+Left/Right back/on 1 s (hold to wind) · Option+Down step · Option+M mark · Option+0 start"
    );
    println!("  Option+H fold the panel · Ctrl+Cmd+F full screen · Cmd+Q quits (saved as you go)");
    if let Ok(exe) = std::env::current_exe() {
        println!(
            "  build {} again and the workshop reloads, at the same tick",
            exe.display()
        );
    }
    let conf = polter::window::Conf {
        title: format!("{} · workshop", e.cart.name),
        size: crate::workshop::Workshop::window_size(size),
        fit: polter::window::Fit::Expand,
        remember: Some(polter::storage::save_dir(e.cart.name).join("workshop_window.txt")),
        hide_cursor: false,
    };
    polter::window::run(conf, Box::new(shop.with_sound().with_reload()));
    0
}

fn cmd_knobs(e: &Env) -> u8 {
    let m = e.machine();
    let knobs = m.game().knobs();
    if e.o.json {
        let list: Vec<String> = knobs
            .iter()
            .map(|k| {
                format!(
                    "{{\"name\":{},\"value\":{},\"min\":{},\"max\":{},\"step\":{}}}",
                    quote(k.name),
                    k.value,
                    k.min,
                    k.max,
                    k.step
                )
            })
            .collect();
        println!(
            "{{\"command\":\"knobs\",\"cart\":{},\"knobs\":[{}]}}",
            quote(e.cart.name),
            list.join(",")
        );
    } else {
        println!("Polter · knobs · {}", e.cart.name);
        if knobs.is_empty() {
            println!("  (none: the game declares no tuning values)");
        }
        for k in &knobs {
            println!(
                "  {:<14} {:<8} {} to {}, steps of {}",
                k.name, k.value, k.min, k.max, k.step
            );
        }
    }
    0
}

/// One scenario, run: what it is, each expectation's verdict, why it stopped early (if it did),
/// and whether it passed.
struct ScenarioRun {
    sc: Scenario,
    verdicts: Vec<Verdict>,
    stop: Option<Panic>,
    ok: bool,
}

/// Runs one scenario file. An error is a message and the exit code it means (3: the file is
/// wrong, 2: the game's state can't be read).
fn run_scenario(e: &Env, path: &str) -> Result<ScenarioRun, (String, u8)> {
    let sc =
        Scenario::load(std::path::Path::new(path)).map_err(|err| (format!("{path}: {err}"), 3))?;
    let mut m = match e.cart.make {
        Maker::Cart(make) => {
            let mut console = Console::new(make());
            console.set_palette(e.pal_index);
            Session::new(Box::new(console), sc.seed)
        }
        Maker::Game(make) => Session::new(make(), sc.seed),
    };
    let mut verdicts: Vec<Verdict> = sc
        .expects
        .iter()
        .map(|_| Verdict {
            failed_at: None,
            error: None,
            checked: 0,
        })
        .collect();
    let mut stop = None;
    for f in 1..=sc.frames {
        // nobody looks at a scenario's pictures (yet): only what the state says counts
        m.ctx.picture = false;
        if let Some(p) = step_checked(&mut m, &sc.haunt.frame(f)) {
            stop = Some(p);
            break;
        }
        let wanted: Vec<usize> = (0..sc.expects.len())
            .filter(|&k| sc.expects[k].when.covers(f))
            .collect();
        if wanted.is_empty() {
            continue;
        }
        let state = parse_state(&m.state_json())
            .map_err(|err| (format!("{path}: the game's state can't be read: {err}"), 2))?;
        for k in wanted {
            let (x, v) = (&sc.expects[k], &mut verdicts[k]);
            if v.failed_at.is_some() || v.error.is_some() {
                continue;
            }
            v.checked += 1;
            match x.holds(&state) {
                Ok(true) => {}
                Ok(false) => {
                    let actual = x.actual(&state).map_or("?".into(), |a| a.to_string());
                    v.failed_at = Some((f, actual));
                }
                Err(why) => v.error = Some(why),
            }
        }
    }
    // an expectation whose ticks were never reached failed too
    let ok = stop.is_none()
        && verdicts
            .iter()
            .all(|v| v.failed_at.is_none() && v.error.is_none() && v.checked > 0);
    Ok(ScenarioRun {
        sc,
        verdicts,
        stop,
        ok,
    })
}

fn cmd_assert(e: &Env) -> u8 {
    if e.o.scenarios.is_empty() {
        eprintln!("assert needs scenario files (FILE.scenario) or --scenarios DIR");
        return 3;
    }
    let mut reports = Vec::new();
    let mut failed = 0;
    for path in &e.o.scenarios {
        match run_scenario(e, path) {
            Ok(run) => {
                failed += (!run.ok) as usize;
                reports.push((path, run));
            }
            Err((message, code)) => {
                eprintln!("{message}");
                return code;
            }
        }
    }
    if e.o.json {
        let list: Vec<String> = reports
            .iter()
            .map(|(path, ScenarioRun { sc, verdicts, stop, ok })| {
                let expects: Vec<String> = sc
                    .expects
                    .iter()
                    .zip(verdicts)
                    .map(|(x, v)| {
                        format!(
                            "{{\"line\":{},\"text\":{},\"passed\":{},\"checked_ticks\":{},\"failed_at\":{},\"actual\":{},\"error\":{}}}",
                            x.line,
                            quote(&x.text),
                            v.failed_at.is_none() && v.error.is_none() && v.checked > 0,
                            v.checked,
                            v.failed_at.as_ref().map_or("null".into(), |f| f.0.to_string()),
                            v.failed_at.as_ref().map_or("null".into(), |f| quote(&f.1)),
                            v.error.as_ref().map_or("null".into(), |w| quote(w)),
                        )
                    })
                    .collect();
                format!(
                    "{{\"file\":{},\"passed\":{ok},\"frames\":{},\"seed\":{},\"failure\":{},\"expects\":[{}]}}",
                    quote(path),
                    sc.frames,
                    sc.seed,
                    panic_json(stop),
                    expects.join(",")
                )
            })
            .collect();
        println!(
            "{{\"command\":\"assert\",\"cart\":{},\"exit\":{},\"passed\":{},\"failed\":{failed},\"scenarios\":[{}]}}",
            quote(e.cart.name),
            (failed > 0) as u8,
            reports.len() - failed,
            list.join(",")
        );
    } else {
        println!("Polter · assert · {}", e.cart.name);
        for (
            path,
            ScenarioRun {
                sc,
                verdicts,
                stop,
                ok,
            },
        ) in &reports
        {
            println!(
                "  {} {path} ({} frames, seed {})",
                if *ok { "✓" } else { "✗" },
                sc.frames,
                sc.seed
            );
            for (x, v) in sc.expects.iter().zip(verdicts) {
                match (&v.failed_at, &v.error) {
                    (Some((f, actual)), _) => println!(
                        "    ✗ line {}: {} · first broken at frame {f}: {} = {actual}",
                        x.line, x.text, x.key
                    ),
                    (None, Some(why)) => println!("    ✗ line {}: {} · {why}", x.line, x.text),
                    (None, None) if v.checked == 0 => println!(
                        "    ✗ line {}: {} · its frames were never reached",
                        x.line, x.text
                    ),
                    (None, None) => println!(
                        "    ✓ line {}: {} ({} frame{} checked)",
                        x.line,
                        x.text,
                        v.checked,
                        if v.checked == 1 { "" } else { "s" }
                    ),
                }
            }
            if stop.is_some() {
                print!("  ");
                report_panic(stop);
            }
        }
        println!(
            "  {} of {} scenarios passed",
            reports.len() - failed,
            reports.len()
        );
    }
    (failed > 0) as u8
}

fn save_recording(path: &str, frames: &[Frame], mouse: bool) -> Result<(), String> {
    std::fs::write(path, Haunt::write_frames(frames, mouse))
        .map_err(|err| format!("cannot write the recording to {path}: {err}"))
}

/// What `play` adds to a game in the window: a haunt file playing along (so `play --haunt`
/// shows a replay), a recording saved as it goes, broken invariants told once, a frame limit.
struct PlayAlong {
    haunt: Haunt,
    record: Option<String>,
    frames: Vec<Frame>,
    desktop: bool,
    save_failed: bool,
    /// Broken invariants already told (on the terminal); the game goes on.
    told: Vec<String>,
    limit: Option<u64>,
}

impl polter::app::Along for PlayAlong {
    fn input(&mut self, frame: u64, from_player: Frame) -> Frame {
        let scripted = self.haunt.frame(frame);
        Frame {
            buttons: from_player.buttons | scripted.buttons,
            mouse: if self.haunt.has_mouse() {
                scripted.mouse
            } else {
                from_player.mouse
            },
            mouse_buttons: from_player.mouse_buttons | scripted.mouse_buttons,
            wheel: from_player.wheel + scripted.wheel,
            keys: from_player.keys | scripted.keys,
            // the pad's buttons together; its sticks and triggers the script's where it moves them
            pad: polter::PadFrame {
                buttons: from_player.pad.buttons | scripted.pad.buttons,
                sticks: if scripted.pad.sticks == [0; 4] {
                    from_player.pad.sticks
                } else {
                    scripted.pad.sticks
                },
                triggers: [0, 1].map(|i| from_player.pad.triggers[i].max(scripted.pad.triggers[i])),
            },
        }
    }

    fn after(&mut self, session: &Session, input: &Frame) -> bool {
        for rule in &session.broken {
            if !self.told.contains(rule) {
                eprintln!("frame {}: invariant broken: {rule}", session.ctx.frame);
                self.told.push(rule.clone());
            }
        }
        self.frames.push(*input);
        // saved twice a second, so a crash loses half a second at most
        if let Some(path) = &self.record
            && session.ctx.frame.is_multiple_of(30)
            && !self.save_failed
            && let Err(err) = save_recording(path, &self.frames, self.desktop)
        {
            eprintln!("{err} (recording stopped, the game goes on)");
            self.save_failed = true;
        }
        !self.limit.is_some_and(|n| session.ctx.frame >= n)
    }

    fn closing(&mut self) {
        if let Some(path) = &self.record {
            match save_recording(path, &self.frames, self.desktop) {
                Ok(()) => println!("Recorded {} frames to {path}.", self.frames.len()),
                Err(err) => eprintln!("{err}"),
            }
        }
    }
}

fn cmd_play(e: &Env) -> u8 {
    // Check the recording target before the game starts: a typo or a protected folder
    // should fail now, not after ten minutes of play.
    if let Some(path) = &e.o.record
        && let Err(err) = save_recording(path, &[], false)
    {
        eprintln!("{err}");
        eprintln!(
            "(macOS can stop the terminal from writing to Desktop or Documents. Try another folder.)"
        );
        return 2;
    }
    let mut m = e.machine();
    if e.o.storage.is_none() {
        m.ctx.storage = Storage::for_cart(e.cart.name);
    }
    // Desktop carts draw their own cursor and may use any key, so P is theirs; F1 always
    // switches palettes (or a game's looks).
    let desktop = m.game().wants_mouse();
    if let Some(path) = &e.o.record {
        println!("Recording to {path}");
        println!(
            "It is saved continuously, so every way of quitting keeps it: Esc, Cmd+Q, closing the window."
        );
        println!(
            "Replay it later: {} peek {} --haunt {path} --frames <frames>",
            program(),
            e.cart.name
        );
    }
    println!("Full screen: Ctrl+Cmd+F on a Mac, F11 or Alt+Enter elsewhere.");
    let along = PlayAlong {
        haunt: e.haunt.clone(),
        record: e.o.record.clone(),
        frames: Vec::new(),
        desktop,
        save_failed: false,
        told: Vec::new(),
        limit: e.o.frames,
    };
    let player = polter::app::Player::new(e.cart.name, m, Some(Box::new(along))).with_sound();
    let conf = player.conf(Some(
        polter::storage::save_dir(e.cart.name).join("window.txt"),
    ));
    // runs until the window closes; on a Mac the process ends there (after `closing`)
    polter::window::run(conf, Box::new(player));
    0
}

/// A palette by index or by (case-insensitive) name prefix; its index.
fn find_palette(pals: &[Palette], query: &str) -> Option<usize> {
    if let Ok(i) = query.parse::<usize>() {
        return (i < pals.len()).then_some(i);
    }
    let q = query.to_lowercase();
    pals.iter()
        .position(|p| p.name.to_lowercase().starts_with(&q))
}

/// Runs the command line for a program containing these console carts. Call it from `main`.
pub fn run(carts: &[CartInfo]) -> ExitCode {
    let entries: Vec<Entry> = carts
        .iter()
        .map(|c| Entry {
            name: c.name,
            about: c.about,
            make: Maker::Cart(c.make),
        })
        .collect();
    run_entries(&entries)
}

/// Runs the command line for a program containing these true-colour games.
pub fn run_games(games: &[GameInfo]) -> ExitCode {
    let entries: Vec<Entry> = games
        .iter()
        .map(|g| Entry {
            name: g.name,
            about: g.about,
            make: Maker::Game(g.make),
        })
        .collect();
    run_entries(&entries)
}

fn run_entries(carts: &[Entry]) -> ExitCode {
    // (older macOS hands an app a process number, -psn_…, which is not ours)
    let args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with("-psn_"))
        .collect();
    let o = match parse(&args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("{}: {err}\n\n{}", program(), help());
            return ExitCode::from(3);
        }
    };
    // started from its Mac app (a double click, no command): play
    let in_app =
        std::env::current_exe().is_ok_and(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"));
    let o = if o.command.is_empty() && in_app {
        Opts {
            command: "play".into(),
            ..o
        }
    } else {
        o
    };
    match o.command.as_str() {
        "" | "help" => {
            println!("{}", help());
            return ExitCode::SUCCESS;
        }
        "carts" => {
            for c in carts {
                println!("{:<10} {}", c.name, c.about);
            }
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
    let cart = match (o.cart.as_deref(), carts) {
        (None, [only]) => Some(only),
        (Some(name), _) => carts.iter().find(|c| c.name.eq_ignore_ascii_case(name)),
        (None, _) => None,
    };
    let Some(cart) = cart else {
        let names: Vec<&str> = carts.iter().map(|c| c.name).collect();
        eprintln!(
            "{}: {} needs one of these carts: {}",
            program(),
            o.command,
            names.join(", ")
        );
        return ExitCode::from(3);
    };
    let haunt = match o.haunt.as_deref().map(Haunt::load).transpose() {
        Ok(h) => h.unwrap_or_default(),
        Err(err) => {
            eprintln!("{}: haunt file: {err}", program());
            return ExitCode::from(3);
        }
    };
    // a console cart's palettes (its own first, then the built-ins); a true-colour game has none
    let pals: Vec<Palette> = match cart.make {
        Maker::Cart(make) => Console::new(make()).palettes().to_vec(),
        Maker::Game(_) => Vec::new(),
    };
    let pal_index = match (o.palette.as_deref(), pals.is_empty()) {
        (None, _) => 0,
        (Some(_), true) => {
            eprintln!(
                "{}: {} draws in true colour and has no palettes (F1 in the window shows its looks)",
                program(),
                cart.name
            );
            return ExitCode::from(3);
        }
        (Some(q), false) => match find_palette(&pals, q) {
            Some(k) => k,
            None => {
                let names: Vec<&str> = pals.iter().map(|p| p.name).collect();
                eprintln!(
                    "{}: unknown palette {q:?}; available: {}",
                    program(),
                    names.join(", ")
                );
                return ExitCode::from(3);
            }
        },
    };
    let pal = pals
        .get(pal_index)
        .cloned()
        .unwrap_or_else(|| palettes()[0].clone());
    let env = Env {
        o: &o,
        cart,
        haunt,
        pal_index,
        pal,
    };
    if o.command != "play" {
        quiet_panics();
    }
    let code = match o.command.as_str() {
        "play" => cmd_play(&env),
        "check" => cmd_check(&env),
        "probe" => cmd_probe(&env),
        "peek" => cmd_peek(&env),
        "shot" => cmd_shot(&env),
        "clip" => cmd_clip(&env),
        "twin" => cmd_twin(&env),
        "listen" => cmd_listen(&env),
        "assert" => cmd_assert(&env),
        "knobs" => cmd_knobs(&env),
        "brief" => brief::cmd_brief(&env),
        "workshop" => cmd_workshop(&env),
        other => {
            eprintln!("{}: unknown command {other:?}\n\n{}", program(), help());
            3
        }
    };
    ExitCode::from(code)
}
