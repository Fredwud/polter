// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `brief`: one page about a game, for an agent that has never seen it (or has forgotten it, as
//! agents do between sessions). In the shape of llms.txt: a title, a summary in a quote, then
//! sections. Everything on it is measured when it is made, from the game itself and its folder:
//! what the game is meant to be (BRIEF.md), how to work on it, its state, its rules, its knobs,
//! its scenarios and whether they pass, its sounds, its time per frame, the moments the person
//! marked in the workshop, its files and its recent changes.

use super::{Env, Fail, Panic, ScenarioRun, perf_summary, run_frames, run_scenario, sound_counts};
use crate::scenario::parse_state;
use std::path::{Path, PathBuf};

/// The engine's folder, as this program was built from it.
fn engine_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// Today as YYYY-MM-DD (UTC), from the days since 1970 (Howard Hinnant's civil-from-days).
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    format!("{y:04}-{m:02}-{d:02}")
}

/// A value from a line `key = "value"` of a small TOML file.
fn toml_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == key).then(|| v.trim().trim_matches('"').to_string())
    })
}

/// A path as it is typed in a shell: in single quotes when it needs them.
fn shell_quote(p: &Path) -> String {
    let s = p.display().to_string();
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-~".contains(c))
    {
        s
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// A table cell: one line, no column breaks.
fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

/// A path for the page: inside the project as it is there, elsewhere in full.
fn shown(p: &Path, project: &Path) -> String {
    p.strip_prefix(project)
        .map_or_else(|_| p.display().to_string(), |r| r.display().to_string())
}

/// What BRIEF.md says: its title, its one sentence, the rest.
struct Pitch {
    title: Option<String>,
    sentence: Option<String>,
    rest: String,
    template: bool,
}

fn pitch(text: &str) -> Pitch {
    let mut title = None;
    let mut sentence = None;
    let mut rest = Vec::new();
    for line in text.lines() {
        if title.is_none() && rest.is_empty() && line.starts_with("# ") {
            title = Some(line[2..].trim().to_string());
        } else if sentence.is_none() && line.starts_with("> ") {
            sentence = Some(line[2..].trim().to_string());
        } else if !(rest.is_empty() && line.trim().is_empty()) {
            rest.push(line);
        }
    }
    Pitch {
        title,
        sentence,
        rest: rest.join("\n").trim_end().to_string(),
        template: text.contains("One sentence: what the game is"),
    }
}

/// Files under a folder (not hidden ones), sorted.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            if e.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if path.is_dir() {
                todo.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// The moments the person marked in the workshop: number, frame, note, picture.
fn marks(project: &Path) -> Vec<(u32, String, String, PathBuf)> {
    let dir = project.join(".polter/workshop/marks");
    let mut out: Vec<(u32, String, String, PathBuf)> = files_under(&dir)
        .into_iter()
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).ok()?;
            let field = |k: &str| {
                text.lines()
                    .find_map(|l| l.strip_prefix(&format!("{k} ")).map(str::to_string))
            };
            let n = field("mark")?.parse().ok()?;
            Some((
                n,
                field("frame").unwrap_or_default(),
                field("note").unwrap_or_default(),
                p.with_extension("png"),
            ))
        })
        .collect();
    out.sort_by_key(|m| m.0);
    out
}

/// The last commits that touched the folder, if it is in a git repository.
fn recent_changes(project: &Path) -> Option<Vec<String>> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(project)
        .args(["log", "--oneline", "-8", "--", "."])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    out.status
        .success()
        .then(|| text.lines().map(str::to_string).collect())
}

/// The page.
pub(super) fn page(e: &Env, project: &Path) -> String {
    let mut out: Vec<String> = Vec::new();
    let is_project = project.join("polter.toml").is_file();
    let toml = std::fs::read_to_string(project.join("polter.toml")).unwrap_or_default();
    let brief = std::fs::read_to_string(project.join("BRIEF.md")).ok();
    let pitch = brief.as_deref().map(pitch);
    let title = toml_value(&toml, "title")
        .or_else(|| pitch.as_ref().and_then(|p| p.title.clone()))
        .unwrap_or_else(|| e.cart.name.to_string());
    let sentence = pitch
        .as_ref()
        .and_then(|p| p.sentence.clone())
        .filter(|_| !pitch.as_ref().is_some_and(|p| p.template))
        .unwrap_or_else(|| e.cart.about.to_string());
    let engine = engine_dir();
    let folder = shell_quote(project);

    out.push(format!("# {title}"));
    out.push(String::new());
    out.push(format!("> {sentence}"));
    out.push(String::new());
    let place = if is_project {
        format!("in `{}`", project.display())
    } else {
        format!("the program `{}`", super::program())
    };
    out.push(format!(
        "A game made with Polter, {place}. `polter brief` made this page on {} from the game itself: every number on it was measured just now, on this machine. Read it first; then the code.",
        today()
    ));
    out.push(String::new());

    // what it is meant to be
    out.push("## What it is".into());
    out.push(String::new());
    match &pitch {
        Some(p) if p.template => out.push(
            "BRIEF.md is still the template's: fill it in (verb, mood, look, when it's done) before the game grows."
                .into(),
        ),
        Some(p) if p.rest.is_empty() => out.push("BRIEF.md has only its title and sentence.".into()),
        Some(p) => out.push(p.rest.clone()),
        None if is_project => {
            out.push("There is no BRIEF.md: write one. It is the first thing an agent reads.".into())
        }
        None => out.push(format!(
            "`{}` is not a game's folder (no polter.toml), so its files are left out of this page. Give the folder with `--project DIR`.",
            project.display()
        )),
    }
    out.push(String::new());

    // how to work on it
    out.push("## Working on it".into());
    out.push(String::new());
    let polter = engine.join("target/release/polter");
    let program = if is_project {
        out.push(format!(
            "From the game's folder, `polter` finds the game by itself (as Godot finds project.godot):\n\n    cd {folder}\n"
        ));
        if polter.is_file() {
            out.push(format!(
                "`polter` is the program `{}` (on your PATH, or typed in full).\n",
                polter.display()
            ));
        }
        "polter".to_string()
    } else {
        super::program()
    };
    for (command, what) in [
        (
            "check",
            "runs it headless: panics with file:line, broken rules, sounds (`--perf`: time per frame)",
        ),
        (
            "assert",
            "runs scenarios (`--scenarios scenarios`, or one FILE.scenario): a start, inputs, what must be true at which ticks",
        ),
        ("probe", "prints the state every N frames"),
        (
            "peek",
            "pictures of frames and a contact sheet, no window needed",
        ),
        ("listen", "the sound as WAV and a picture"),
        (
            "twin",
            "runs twice and compares every frame: is it deterministic?",
        ),
        (
            "workshop",
            "the person's tape deck: play, rewind, tune; it shares `.polter/workshop` with you (`commands.txt`, `events.log`, `marks/`)",
        ),
        ("brief", "this page again"),
    ] {
        out.push(format!("- `{program} {command}`: {what}"));
    }
    out.push(String::new());
    out.push("Rules for the code: all logic in `update` (`draw` gets `&self`); randomness only from `ctx.rng`; `polter::math` instead of `x.sin()`, `exp`, `powf`, and `BTreeMap` instead of `HashMap` (the same bits on every machine; `clippy.toml` says why); everything that matters in `state()`; what must always hold in `invariants()`.".into());
    out.push(String::new());
    out.push(format!(
        "The engine: [API.md]({}) (the whole API: read it all before writing code) · [llms.txt]({})",
        engine.join("API.md").display(),
        engine.join("llms.txt").display()
    ));
    out.push(String::new());

    // a run, measured
    let frames = e.o.frames.unwrap_or(600);
    let mut m = e.machine();
    let mut rules: Vec<String> = Vec::new();
    let mut played: Vec<&str> = Vec::new();
    let mut times: Vec<f64> = Vec::new();
    let mut first = None;
    let mut last = std::time::Instant::now();
    let stop = run_frames(
        &mut m,
        frames,
        &e.haunt,
        |_| true,
        |m| {
            let now = std::time::Instant::now();
            times.push((now - last).as_secs_f64() * 1000.0);
            for r in m.rules() {
                if !rules.contains(&r) {
                    rules.push(r);
                }
            }
            played.extend(m.sounds.iter().map(|s| s.name));
            if first.is_none() {
                first = Some(m.state_json());
            }
            last = std::time::Instant::now();
        },
    );
    let run = match &e.o.haunt {
        Some(h) => format!(
            "{frames} frames from seed {} with the input of `{h}`",
            e.o.seed
        ),
        None => format!("{frames} frames from seed {} without input", e.o.seed),
    };

    out.push("## State".into());
    out.push(String::new());
    let start = first
        .as_deref()
        .and_then(|s| parse_state(s).ok())
        .unwrap_or_default();
    let end = parse_state(&m.state_json()).unwrap_or_default();
    if end.is_empty() {
        out.push(
            "The game reports no state: report what matters in `state()`, so it can be tested."
                .into(),
        );
    } else {
        out.push(format!(
            "What `state()` reports after frame 1 and after frame {} ({run}). Scenarios and `probe` read these keys.",
            m.ctx.frame
        ));
        out.push(String::new());
        out.push(format!("| key | frame 1 | frame {} |", m.ctx.frame));
        out.push("|---|---|---|".into());
        for (key, value) in &end {
            let at_start = start
                .iter()
                .find(|(k, _)| k == key)
                .map_or("".into(), |(_, v)| v.to_string());
            out.push(format!(
                "| `{}` | {} | {} |",
                cell(key),
                cell(&at_start),
                cell(&value.to_string())
            ));
        }
    }
    out.push(String::new());

    out.push("## Rules".into());
    out.push(String::new());
    if rules.is_empty() && stop.is_none() {
        out.push("None: the game declares no rules. Say in `invariants()` what must always hold; every command then checks it on every tick.".into());
    }
    let broken: Vec<String> = match &stop {
        Some(Panic {
            fail: Fail::Broken { rules, .. },
            ..
        }) => rules.clone(),
        _ => Vec::new(),
    };
    for r in &rules {
        if !broken.contains(r) {
            out.push(format!("- ✓ {r}"));
        }
    }
    match &stop {
        None if !rules.is_empty() => {
            out.push(String::new());
            out.push(format!("Each held on every tick of the run ({run})."));
        }
        Some(Panic {
            frame,
            fail: Fail::Broken { rules, state },
        }) => {
            for r in rules {
                out.push(format!(
                    "- ✗ {r}: broken at frame {frame}, with the state {state}"
                ));
            }
        }
        Some(Panic {
            frame,
            fail: Fail::Panic { message, location },
        }) => {
            out.push(String::new());
            out.push(format!(
                "✗ The run stopped: the game panicked at frame {frame}, at {location}: {message}"
            ));
        }
        None => {}
    }
    out.push(String::new());

    out.push("## Knobs".into());
    out.push(String::new());
    let knobs = m.game().knobs();
    if knobs.is_empty() {
        out.push("None: the game has no tuning values for the workshop's faders.".into());
    } else {
        out.push("Tuning values: the person moves them on the workshop's faders, you through `knobs.txt` there.".into());
        out.push(String::new());
        out.push("| knob | value | from | to | step |".into());
        out.push("|---|---|---|---|---|".into());
        for k in &knobs {
            out.push(format!(
                "| `{}` | {} | {} | {} | {} |",
                k.name, k.value, k.min, k.max, k.step
            ));
        }
    }
    out.push(String::new());

    if is_project {
        out.push("## Scenarios".into());
        out.push(String::new());
        let scenarios: Vec<PathBuf> = files_under(&project.join("scenarios"))
            .into_iter()
            .filter(|p| p.extension().is_some_and(|x| x == "scenario"))
            .collect();
        if scenarios.is_empty() {
            out.push("None in `scenarios/`: a scenario is a start, inputs and what must be true at which ticks.".into());
        }
        let mut passed = 0;
        for path in &scenarios {
            let name = shown(path, project);
            match run_scenario(e, &path.display().to_string()) {
                Ok(ScenarioRun {
                    sc,
                    verdicts,
                    stop,
                    ok,
                }) => {
                    passed += ok as usize;
                    out.push(format!(
                        "- {} `{name}` ({} frames, seed {})",
                        if ok { "✓" } else { "✗" },
                        sc.frames,
                        sc.seed
                    ));
                    for (x, v) in sc.expects.iter().zip(&verdicts) {
                        let verdict = match (&v.failed_at, &v.error) {
                            (Some((f, actual)), _) => {
                                format!("✗ first broken at frame {f}: {} = {actual}", x.key)
                            }
                            (None, Some(why)) => format!("✗ {why}"),
                            (None, None) if v.checked == 0 => {
                                "✗ its frames were never reached".into()
                            }
                            (None, None) => "✓".into(),
                        };
                        out.push(format!("  - `{}` {verdict}", x.text));
                    }
                    if let Some(p) = &stop {
                        out.push(format!("  - ✗ stopped at frame {}", p.frame));
                    }
                }
                Err((message, _)) => out.push(format!("- ✗ `{name}`: {message}")),
            }
        }
        if !scenarios.is_empty() {
            out.push(String::new());
            out.push(format!("{passed} of {} pass.", scenarios.len()));
        }
        out.push(String::new());
    }

    out.push("## Sounds".into());
    out.push(String::new());
    let counts = sound_counts(&played);
    if counts.is_empty() {
        out.push(format!("None played in the run ({run})."));
    } else {
        let list: Vec<String> = counts.iter().map(|(k, c)| format!("`{k}` ×{c}")).collect();
        out.push(format!("In the run ({run}): {}.", list.join(", ")));
    }
    out.push(String::new());

    out.push("## Time per frame".into());
    out.push(String::new());
    let timed = times.len();
    match perf_summary(&mut times) {
        Some(p) => out.push(format!(
            "Update and draw, on this machine: average {:.2} ms, 95 % within {:.2} ms, at most {:.2} ms; {} of {timed} frames over the 16.7 ms a frame has at 60 FPS{}.",
            p.avg,
            p.p95,
            p.max,
            p.over,
            if cfg!(debug_assertions) {
                " (a debug build: slower than the game is)"
            } else {
                ""
            }
        )),
        None => out.push("Not measured: the run stopped at its first frame.".into()),
    }
    out.push(String::new());

    if is_project {
        out.push("## Marks from the person".into());
        out.push(String::new());
        let marks = marks(project);
        if marks.is_empty() {
            out.push("None: in the workshop, the person marks a moment with CUE (Alt+M or F9), and it lands here with its picture and state.".into());
        }
        for (n, frame, note, picture) in &marks {
            let note = if note.is_empty() { "(no note)" } else { note };
            out.push(format!(
                "- mark {n} at frame {frame}: {note} · [picture]({})",
                shown(picture, project)
            ));
        }
        out.push(String::new());

        out.push("## Files".into());
        out.push(String::new());
        for path in files_under(&project.join("src")) {
            let lines = std::fs::read_to_string(&path).map_or(0, |t| t.lines().count());
            out.push(format!("- `{}`: {lines} lines", shown(&path, project)));
        }
        for folder in ["assets", "levels", "haunts"] {
            let found: Vec<PathBuf> = files_under(&project.join(folder));
            let shown_files: Vec<String> = found
                .iter()
                .take(12)
                .map(|p| format!("`{}`", shown(p, project)))
                .collect();
            out.push(match found.len() {
                0 => format!("- `{folder}/`: empty"),
                n if n <= 12 => format!("- `{folder}/`: {}", shown_files.join(", ")),
                n => format!(
                    "- `{folder}/`: {} and {} more",
                    shown_files.join(", "),
                    n - 12
                ),
            });
        }
        out.push(String::new());

        out.push("## Recent changes".into());
        out.push(String::new());
        match recent_changes(project) {
            Some(lines) if !lines.is_empty() => {
                for l in lines {
                    out.push(format!("- {l}"));
                }
            }
            Some(_) => out.push("None committed yet.".into()),
            None => {
                out.push("Not in a git repository: `git init` keeps what changed, and why.".into())
            }
        }
        out.push(String::new());
    }
    out.join("\n")
}

/// `brief`: the page on standard output, or in `--out FILE`.
pub(super) fn cmd_brief(e: &Env) -> u8 {
    let project = PathBuf::from(e.o.project.as_deref().unwrap_or("."));
    let project = std::fs::canonicalize(&project).unwrap_or(project);
    let text = page(e, &project);
    match &e.o.out {
        Some(path) => match std::fs::write(path, &text) {
            Ok(()) => {
                println!("{path}");
                0
            }
            Err(err) => {
                eprintln!("cannot write {path}: {err}");
                2
            }
        },
        None => {
            print!("{text}");
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Entry, Maker, Opts};
    use super::*;
    use polter::audio::{Sfx, Wave};
    use polter::haunt::Haunt;
    use polter::{Canvas, Ctx, Game, Invariants, Knob, State};

    const RING: Sfx = Sfx::new("ring", Wave::Triangle, 880.0, 660.0, 10, 0.2);

    /// Rings every 100 ticks; one rule always, one only after tick 50, and with `CRACKS` one
    /// that breaks at tick 20.
    struct Bell<const CRACKS: bool> {
        t: u32,
    }

    impl<const CRACKS: bool> Game for Bell<CRACKS> {
        fn update(&mut self, ctx: &mut Ctx) {
            self.t += 1;
            if self.t.is_multiple_of(100) {
                ctx.play(RING);
            }
        }
        fn draw(&self, c: &mut Canvas, _ctx: &Ctx) {
            c.clear(0);
        }
        fn size(&self) -> (usize, usize) {
            (8, 8)
        }
        fn state(&self) -> State {
            State::new().int("t", self.t as i64)
        }
        fn invariants(&self, check: &mut Invariants) {
            check.require("t never passes 10000", self.t < 10_000);
            if self.t > 50 {
                check.require("rings come every 100 ticks", true);
            }
            if CRACKS {
                check.require("the bell never cracks", self.t != 20);
            }
        }
        fn knobs(&self) -> Vec<Knob> {
            vec![Knob::new("loud", 0.3, 0.0, 1.0, 0.1)]
        }
    }

    /// A game's folder with a brief, two scenarios (one that fails), a mark and a source file.
    fn bell_tower(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("polter-brief-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let files = [
            ("polter.toml", "name = \"bell\"\ntitle = \"Bell Tower\"\n"),
            (
                "BRIEF.md",
                "# Bell Tower\n\n> Ring the bells on time.\n\n- **Verb:** ring\n",
            ),
            (
                "scenarios/good.scenario",
                "seed 1\nframes 120\nat 100: t == 100\n",
            ),
            (
                "scenarios/bad.scenario",
                "seed 1\nframes 50\nat 40: t == 41\n",
            ),
            (
                ".polter/workshop/marks/mark_1.txt",
                "mark 1\nframe 77\nnote the bell is too quiet\nstate {}\nknobs loud 0.3\n",
            ),
            ("src/lib.rs", "//! The bell.\n\npub struct Bell;\n"),
        ];
        for (path, text) in files {
            let at = dir.join(path);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, text).unwrap();
        }
        dir
    }

    fn page_for(make: fn() -> Box<dyn Game>, dir: &Path) -> String {
        let o = Opts {
            seed: 1,
            frames: Some(300),
            project: Some(dir.display().to_string()),
            ..Opts::default()
        };
        let entry = Entry {
            name: "bell",
            about: "A bell.",
            make: Maker::Game(make),
        };
        let env = Env {
            o: &o,
            cart: &entry,
            haunt: Haunt::default(),
            pal_index: 0,
            pal: polter::palettes()[0].clone(),
        };
        page(&env, dir)
    }

    #[test]
    fn a_page_says_what_the_game_is_and_how_it_stands() {
        let dir = bell_tower("whole");
        let page = page_for(|| Box::new(Bell::<false> { t: 0 }), &dir);
        let has = |s: &str| assert!(page.contains(s), "missing {s:?} in\n{page}");
        assert!(
            page.starts_with("# Bell Tower\n\n> Ring the bells on time.\n"),
            "{page}"
        );
        has("- **Verb:** ring");
        has("    cd ");
        has("| `t` | 1 | 300 |");
        // a rule the game checks only later is found too
        has("- ✓ t never passes 10000");
        has("- ✓ rings come every 100 ticks");
        has("Each held on every tick of the run (300 frames from seed 1 without input).");
        has("| `loud` | 0.3 | 0 | 1 | 0.1 |");
        has("- ✓ `scenarios/good.scenario` (120 frames, seed 1)");
        has("- ✗ `scenarios/bad.scenario` (50 frames, seed 1)");
        has("  - `at 40: t == 41` ✗ first broken at frame 40: t = 40");
        has("1 of 2 pass.");
        has("`ring` ×3");
        has(
            "- mark 1 at frame 77: the bell is too quiet · [picture](.polter/workshop/marks/mark_1.png)",
        );
        has("- `src/lib.rs`: 3 lines");
        has("- `assets/`: empty");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_broken_rule_is_on_the_page_with_its_frame() {
        let dir = bell_tower("cracked");
        let page = page_for(|| Box::new(Bell::<true> { t: 0 }), &dir);
        assert!(page.contains("- ✓ t never passes 10000"), "{page}");
        assert!(
            page.contains(
                "- ✗ the bell never cracks: broken at frame 20, with the state {\"t\":20}"
            ),
            "{page}"
        );
        assert!(!page.contains("Each held"), "{page}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_that_is_no_game_leaves_the_files_out() {
        let dir = std::env::temp_dir().join(format!("polter-brief-none-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let page = page_for(|| Box::new(Bell::<false> { t: 0 }), &dir);
        assert!(page.starts_with("# bell\n\n> A bell.\n"), "{page}");
        assert!(
            page.contains("is not a game's folder (no polter.toml)"),
            "{page}"
        );
        assert!(
            !page.contains("## Scenarios") && !page.contains("## Files"),
            "{page}"
        );
        assert!(
            page.contains("## State") && page.contains("## Rules"),
            "{page}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_pitch_is_read_from_brief_md() {
        let p = pitch("# Lantern\n\n> Carry light through a cave.\n\n- **Verb:** carry\n");
        assert_eq!(p.title.as_deref(), Some("Lantern"));
        assert_eq!(p.sentence.as_deref(), Some("Carry light through a cave."));
        assert_eq!(p.rest, "- **Verb:** carry");
        assert!(!p.template);
        let t = pitch("# X\n\n> One sentence: what the game is, and why someone would play it.\n");
        assert!(t.template);
    }

    #[test]
    fn small_helpers() {
        assert_eq!(
            toml_value("name = \"x\"\ntitle = \"Star Pilot\"\n", "title").as_deref(),
            Some("Star Pilot")
        );
        assert_eq!(cell("a|b\nc"), "a\\|b c");
        assert_eq!(shell_quote(Path::new("/a/b")), "/a/b");
        assert_eq!(shell_quote(Path::new("/a/Star Pilot")), "'/a/Star Pilot'");
        assert_eq!(shell_quote(Path::new("/it's")), "'/it'\\''s'");
        let d = today();
        assert_eq!(d.len(), 10);
        assert!(d.starts_with("20"), "{d}");
    }
}
