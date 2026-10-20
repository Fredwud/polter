// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `polter`: makes, lists and opens Polter games in any folder, and runs their commands, the way
//! Godot's project manager and `godot --path` do. A game is a folder with a `polter.toml`; this
//! program finds it (given, or from the current folder upwards), builds it, and hands the command
//! to the game's own program. All games share one build folder, so the engine is built once.

mod manager;
mod template;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "\
polter: Polter games in any folder

  polter new FOLDER           a new game in FOLDER (made if missing, must be empty), ready to run
  polter projects             the games known here, newest first
  polter manager              the same as a window: open, play, make or add a game, folders chosen
                              in it (--out FILE [--size W,H] [--dialog new|add]: a picture instead)
  polter open FOLDER          remember a game made elsewhere
  polter forget FOLDER        stop listing a game (its files stay)
  polter build [FOLDER]       build the game (the quick profile; the workshop reloads when it is done)
  polter ship [FOLDER]        build what a player gets: the fastest build, without the tools
  polter COMMAND [FOLDER] [ARGS…]
                              build the game, then run one of its commands: play, workshop, check,
                              probe, peek, shot, twin, listen, assert, knobs, brief, help

Without FOLDER the game is found from the current folder upwards (its polter.toml), as Godot
finds project.godot, and paths in the command are as typed here. With FOLDER the paths are the
game's own (polter assert FOLDER scenarios/start.scenario). `polter workshop` keeps the
workshop's folder inside the game (.polter/workshop).
";

/// The game commands this program builds for and hands on.
const GAME_COMMANDS: &[&str] = &[
    "play", "workshop", "check", "probe", "peek", "shot", "clip", "twin", "listen", "assert",
    "knobs", "brief", "help", "carts",
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("polter: {e}");
            ExitCode::from(e.code())
        }
    }
}

/// What went wrong, and the exit code it gets: 2 the tool, 3 the way it was called.
#[derive(Debug)]
pub(crate) enum Fail {
    Tool(String),
    Usage(String),
}

impl Fail {
    fn code(&self) -> u8 {
        match self {
            Fail::Tool(_) => 2,
            Fail::Usage(_) => 3,
        }
    }
}

impl std::fmt::Display for Fail {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Fail::Tool(s) | Fail::Usage(s) => write!(f, "{s}"),
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, Fail> {
    let Some(command) = args.first() else {
        print!("{USAGE}");
        list_projects();
        return Ok(ExitCode::SUCCESS);
    };
    let rest = &args[1..];
    match command.as_str() {
        "new" => {
            let folder = rest
                .first()
                .ok_or(Fail::Usage("new needs a FOLDER".into()))?;
            let engine = match rest.iter().position(|a| a == "--engine") {
                Some(i) => absolute(Path::new(
                    rest.get(i + 1)
                        .ok_or(Fail::Usage("--engine needs a FOLDER".into()))?,
                )),
                None => engine_dir(),
            };
            let dir = absolute(Path::new(folder));
            let name = new_game(&dir, &engine)?;
            remember(&dir);
            println!("made {name} in {}", dir.display());
            println!("  play it:    polter play {}", shell_quote(&dir));
            println!("  work on it: polter workshop {}", shell_quote(&dir));
            Ok(ExitCode::SUCCESS)
        }
        "projects" => {
            list_projects();
            Ok(ExitCode::SUCCESS)
        }
        "manager" => manager::run(rest),
        "open" => {
            let folder = rest
                .first()
                .ok_or(Fail::Usage("open needs a FOLDER".into()))?;
            let dir = find_project(&absolute(Path::new(folder)))
                .ok_or(Fail::Usage(format!("no polter.toml in or above {folder}")))?;
            remember(&dir);
            println!("{}", dir.display());
            Ok(ExitCode::SUCCESS)
        }
        "forget" => {
            let folder = rest
                .first()
                .ok_or(Fail::Usage("forget needs a FOLDER".into()))?;
            forget(&absolute(Path::new(folder)));
            Ok(ExitCode::SUCCESS)
        }
        "build" => {
            let (dir, _, _) = project_and_args(rest)?;
            build(&dir)?;
            println!("{}", binary(&dir)?.display());
            Ok(ExitCode::SUCCESS)
        }
        "ship" => {
            let (dir, _, _) = project_and_args(rest)?;
            let program = ship(&dir)?;
            let size = std::fs::metadata(&program).map_or(0, |m| m.len());
            println!("{}", program.display());
            println!(
                "  {:.1} MB, the game alone: no command line, no workshop, no checks",
                size as f64 / 1e6
            );
            Ok(ExitCode::SUCCESS)
        }
        c if GAME_COMMANDS.contains(&c) => {
            let (dir, mut game_args, named) = project_and_args(rest)?;
            build(&dir)?;
            remember(&dir);
            if c == "workshop" && !game_args.iter().any(|a| a == "--dir") {
                game_args.push("--dir".into());
                game_args.push(dir.join(".polter/workshop").display().to_string());
            }
            if c == "brief" && !game_args.iter().any(|a| a == "--project") {
                game_args.push("--project".into());
                game_args.push(dir.display().to_string());
            }
            let mut all = vec![c.to_string()];
            all.extend(game_args);
            // a game named on the command line: its paths are the game's own; a game found from
            // here: paths are as typed here
            hand_on(&binary(&dir)?, &all, named.then_some(dir.as_path()))
        }
        "-h" | "--help" => {
            print!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        other => Err(Fail::Usage(format!(
            "unknown command {other:?} (polter --help lists them)"
        ))),
    }
}

/// The engine's folder: the one this program was built from.
pub(crate) fn engine_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn absolute(p: &Path) -> PathBuf {
    let p = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    };
    // tidy `.` and `..` without asking the file system (the folder may not exist yet)
    let mut out = PathBuf::new();
    for part in p.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// The folder with a `polter.toml`: `start` itself or the nearest above it.
fn find_project(start: &Path) -> Option<PathBuf> {
    let mut p = Some(start);
    while let Some(dir) = p {
        if dir.join("polter.toml").is_file() {
            return Some(dir.to_path_buf());
        }
        p = dir.parent();
    }
    None
}

/// The game a command is for (its folder, if the first argument is one), the rest of the
/// arguments, and whether the folder was named (or found from the current folder).
fn project_and_args(rest: &[String]) -> Result<(PathBuf, Vec<String>, bool), Fail> {
    if let Some(first) = rest.first()
        && !first.starts_with('-')
    {
        let candidate = absolute(Path::new(first));
        if candidate.is_dir() {
            let dir = find_project(&candidate)
                .ok_or(Fail::Usage(format!("no polter.toml in or above {first}")))?;
            return Ok((dir, rest[1..].to_vec(), true));
        }
    }
    let here = std::env::current_dir().map_err(|e| Fail::Tool(e.to_string()))?;
    let dir = find_project(&here).ok_or(Fail::Usage(
        "not inside a Polter game: give its folder (polter projects lists them)".into(),
    ))?;
    Ok((dir, rest.to_vec(), false))
}

/// A path as a shell reads it: quoted when it has spaces or quotes.
fn shell_quote(p: &Path) -> String {
    let s = p.display().to_string();
    if s.contains([' ', '\'', '"', '$', '&', '(', ')']) {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s
    }
}

/// A crate name from a folder name: lower case, letters, digits and dashes, starting with a letter.
fn crate_name(folder: &str) -> String {
    let mut s: String = folder
        .chars()
        .map(|c| {
            let c = c.to_ascii_lowercase();
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-').to_string();
    match s.chars().next() {
        Some(c) if c.is_ascii_alphabetic() => s,
        Some(_) => format!("game-{s}"),
        None => "game".into(),
    }
}

/// A type name from a crate name: `star-pilot` → `StarPilot`.
fn type_name(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or(String::new(), |f| {
                f.to_ascii_uppercase().to_string() + c.as_str()
            })
        })
        .collect()
}

/// Writes a new game into `dir`, which must be missing or empty. Returns its crate name.
pub(crate) fn new_game(dir: &Path, engine: &Path) -> Result<String, Fail> {
    if dir.exists()
        && std::fs::read_dir(dir)
            .map_err(|e| Fail::Tool(e.to_string()))?
            .next()
            .is_some()
    {
        return Err(Fail::Usage(format!(
            "{} is not empty: a new game needs an empty folder",
            dir.display()
        )));
    }
    if !engine.join("Cargo.toml").is_file() {
        return Err(Fail::Usage(format!(
            "no engine in {} (--engine FOLDER)",
            engine.display()
        )));
    }
    let title = dir
        .file_name()
        .map_or_else(|| "game".into(), |n| n.to_string_lossy().to_string());
    let name = crate_name(&title);
    let ty = type_name(&name);
    let krate = name.replace('-', "_");
    let fill = |t: &str| {
        t.replace("{name}", &name)
            .replace("{Type}", &ty)
            .replace("{Title}", &title)
            .replace("{crate}", &krate)
            .replace("{engine}", &engine.display().to_string())
    };
    let files: [(&str, &str); 8] = [
        ("polter.toml", template::POLTER_TOML),
        ("Cargo.toml", template::CARGO_TOML),
        ("clippy.toml", template::CLIPPY_TOML),
        ("BRIEF.md", template::BRIEF_MD),
        (".gitignore", template::GITIGNORE),
        ("src/main.rs", template::MAIN_RS),
        ("src/lib.rs", template::LIB_RS),
        ("scenarios/start.scenario", template::SCENARIO),
    ];
    let io = |e: std::io::Error| Fail::Tool(e.to_string());
    for (path, text) in files {
        let at = dir.join(path);
        if let Some(parent) = at.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        std::fs::write(&at, fill(text)).map_err(io)?;
    }
    for empty in ["assets", "levels", "haunts"] {
        std::fs::create_dir_all(dir.join(empty)).map_err(io)?;
        std::fs::write(dir.join(empty).join(".gitkeep"), "").map_err(io)?;
    }
    Ok(name)
}

/// The crate name of a game, from its Cargo.toml.
fn package_name(dir: &Path) -> Result<String, Fail> {
    let text = std::fs::read_to_string(dir.join("Cargo.toml"))
        .map_err(|e| Fail::Tool(format!("{}: {e}", dir.join("Cargo.toml").display())))?;
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package
            && let Some(v) = line.strip_prefix("name")
            && let Some(v) = v.trim_start().strip_prefix('=')
        {
            return Ok(v.trim().trim_matches('"').to_string());
        }
    }
    Err(Fail::Tool(format!(
        "no package name in {}",
        dir.join("Cargo.toml").display()
    )))
}

/// The build folder every game shares, so the engine is built once.
fn shared_target() -> PathBuf {
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
    if let Some(t) = std::env::var_os("POLTER_TARGET") {
        return PathBuf::from(t);
    }
    if cfg!(target_os = "macos") {
        home.join("Library/Caches/Polter/target")
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map_or_else(|| home.join(".cache"), PathBuf::from)
            .join("polter/target")
    }
}

fn binary(dir: &Path) -> Result<PathBuf, Fail> {
    Ok(shared_target().join("quick").join(package_name(dir)?))
}

fn cargo() -> PathBuf {
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
    let local = home.join(".cargo/bin/cargo");
    if local.is_file() {
        local
    } else {
        PathBuf::from("cargo")
    }
}

fn build(dir: &Path) -> Result<(), Fail> {
    let watch = determinism_watch(dir, &["--profile", "quick"]);
    let status = Command::new(cargo())
        .args(["build", "--quiet", "--profile", "quick", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", shared_target())
        .status()
        .map_err(|e| Fail::Tool(format!("cargo: {e}")))?;
    report_determinism(watch);
    if status.success() {
        Ok(())
    } else {
        Err(Fail::Tool(format!("{} did not build", dir.display())))
    }
}

/// Builds what a player gets: the release profile, without the `dev` feature (and so without
/// polter-dev). Its program.
fn ship(dir: &Path) -> Result<PathBuf, Fail> {
    let watch = determinism_watch(dir, &["--release", "--no-default-features"]);
    let status = Command::new(cargo())
        .args(["build", "--quiet", "--release", "--no-default-features"])
        .arg("--manifest-path")
        .arg(dir.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", shared_target())
        .status()
        .map_err(|e| Fail::Tool(format!("cargo: {e}")))?;
    report_determinism(watch);
    if !status.success() {
        return Err(Fail::Tool(format!("{} did not build", dir.display())));
    }
    Ok(shared_target().join("release").join(package_name(dir)?))
}

/// Starts looking for what would make the game play differently on another machine (the uses
/// its clippy.toml names: the platform's maths, the wall clock, randomly ordered maps), beside
/// the build: in its own folder of the shared target, so the two don't wait for each other's
/// lock (measured on 19 October 2026: 0.6 s after a change, against 0.9 s for the build).
/// Nothing without clippy, or without a clippy.toml (a game made before the rule).
fn determinism_watch(dir: &Path, profile: &[&str]) -> Option<std::process::Child> {
    if !dir.join("clippy.toml").is_file() {
        return None;
    }
    Command::new(cargo())
        .args(["clippy", "--quiet"])
        .args(profile)
        .arg("--manifest-path")
        .arg(dir.join("Cargo.toml"))
        .args(["--", "-A", "warnings"])
        .args(["--force-warn", "clippy::disallowed_methods"])
        .args(["--force-warn", "clippy::disallowed_types"])
        .env("CARGO_TARGET_DIR", shared_target().join("lint"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()
}

/// Shows what the watch found, and nothing else of clippy's. It never stops a build: a game that
/// doesn't compile is left to the build to report.
fn report_determinism(watch: Option<std::process::Child>) {
    let Some(out) = watch.and_then(|w| w.wait_with_output().ok()) else {
        return;
    };
    let said = String::from_utf8_lossy(&out.stderr);
    if out.status.success() && said.contains("disallowed") {
        eprint!("{said}");
        eprintln!(
            "These would make the game play differently on another machine (clippy.toml says why). It still builds."
        );
    }
}

/// Runs the game's program with the command (inside `dir`, if given, so paths in a command are
/// the game's own), in this process where it can (so the terminal and Ctrl+C belong to the
/// game), and passes its exit code on.
fn hand_on(program: &Path, args: &[String], dir: Option<&Path>) -> Result<ExitCode, Fail> {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = command.exec();
        Err(Fail::Tool(format!("{}: {err}", program.display())))
    }
    #[cfg(not(unix))]
    {
        let status = command
            .status()
            .map_err(|e| Fail::Tool(format!("{}: {e}", program.display())))?;
        Ok(ExitCode::from(status.code().unwrap_or(2) as u8))
    }
}

// ── The list of games ────────────────────────────────────────────────────

fn list_file() -> PathBuf {
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
    if let Some(p) = std::env::var_os("POLTER_PROJECTS") {
        return PathBuf::from(p);
    }
    let base = if cfg!(target_os = "macos") {
        home.join("Library/Application Support")
    } else {
        std::env::var_os("XDG_DATA_HOME").map_or_else(|| home.join(".local/share"), PathBuf::from)
    };
    base.join("Polter/projects.txt")
}

fn known() -> Vec<PathBuf> {
    known_in(&list_file())
}

/// The games listed in a file, newest first.
pub(crate) fn known_in(file: &Path) -> Vec<PathBuf> {
    std::fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(PathBuf::from)
        .collect()
}

fn save_list_in(file: &Path, list: &[PathBuf]) {
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let text: String = list.iter().map(|p| format!("{}\n", p.display())).collect();
    let _ = std::fs::write(file, text);
}

/// Puts a game at the top of the list (newest first, each once).
fn remember(dir: &Path) {
    remember_in(&list_file(), dir);
}

pub(crate) fn remember_in(file: &Path, dir: &Path) {
    let mut list = known_in(file);
    list.retain(|p| p != dir);
    list.insert(0, dir.to_path_buf());
    save_list_in(file, &list);
}

fn forget(dir: &Path) {
    if !forget_in(&list_file(), dir) {
        eprintln!("polter: {} was not on the list", dir.display());
    }
}

/// Takes a game off the list (its files stay); false if it wasn't on it.
pub(crate) fn forget_in(file: &Path, dir: &Path) -> bool {
    let mut list = known_in(file);
    let before = list.len();
    list.retain(|p| p != dir);
    save_list_in(file, &list);
    list.len() != before
}

/// A game's title from its polter.toml; None when the folder or the file is gone.
pub(crate) fn title_of(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("polter.toml")).ok()?;
    text.lines()
        .find_map(|l| l.trim().strip_prefix("title").map(str::to_string))
        .and_then(|v| v.split('"').nth(1).map(str::to_string))
}

fn list_projects() {
    let list = known();
    if list.is_empty() {
        println!("no games yet: polter new FOLDER");
        return;
    }
    println!("games (newest first):");
    for dir in list {
        match title_of(&dir) {
            Some(t) => println!("  {t:<20} {}", dir.display()),
            None => println!("  {:<20} {} (missing)", "?", dir.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("polter-dev-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn names_from_folders() {
        assert_eq!(crate_name("Star Pilot"), "star-pilot");
        assert_eq!(crate_name("meinspiel"), "meinspiel");
        assert_eq!(crate_name("3 Bälle!"), "game-3-b-lle");
        assert_eq!(crate_name("---"), "game");
        assert_eq!(type_name("star-pilot"), "StarPilot");
        assert_eq!(type_name("game-3-b-lle"), "Game3BLle");
    }

    #[test]
    fn a_new_game_is_a_complete_folder_and_is_found_from_inside() {
        let dir = temp("new").join("Star Pilot");
        let name = new_game(&dir, &engine_dir()).unwrap();
        assert_eq!(name, "star-pilot");
        for f in [
            "polter.toml",
            "Cargo.toml",
            "clippy.toml",
            "BRIEF.md",
            ".gitignore",
            "src/main.rs",
            "src/lib.rs",
            "scenarios/start.scenario",
            "assets/.gitkeep",
            "levels/.gitkeep",
            "haunts/.gitkeep",
        ] {
            assert!(dir.join(f).is_file(), "{f}");
        }
        let cargo = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(cargo.contains(&format!("path = \"{}\"", engine_dir().display())));
        // the engine's rule, word for word
        let rule = std::fs::read_to_string(engine_dir().join("clippy.toml")).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("clippy.toml")).unwrap(),
            rule
        );
        assert!(rule.contains("\"f32::sin\"") && rule.contains("std::collections::HashMap"));
        assert!(cargo.contains("[workspace]") && cargo.contains("[profile.quick]"));
        let main = std::fs::read_to_string(dir.join("src/main.rs")).unwrap();
        assert!(main.contains("star_pilot::StarPilot::new()"));
        assert!(
            !main.contains("{Type}") && !main.contains("{name}"),
            "{main}"
        );
        assert_eq!(package_name(&dir).unwrap(), "star-pilot");
        // found from a folder inside it
        assert_eq!(find_project(&dir.join("src")), Some(dir.clone()));
        // a folder that isn't empty is refused
        assert!(matches!(new_game(&dir, &engine_dir()), Err(Fail::Usage(_))));
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn the_list_keeps_each_game_once_newest_first() {
        let file = temp("list").join("projects.txt");
        // SAFETY: tests in this binary that touch the list run in this one test only
        unsafe { std::env::set_var("POLTER_PROJECTS", &file) };
        remember(Path::new("/a"));
        remember(Path::new("/b"));
        remember(Path::new("/a"));
        assert_eq!(known(), [PathBuf::from("/a"), PathBuf::from("/b")]);
        forget(Path::new("/a"));
        assert_eq!(known(), [PathBuf::from("/b")]);
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn paths_with_spaces_are_quoted_for_the_shell() {
        assert_eq!(shell_quote(Path::new("/a/b")), "/a/b");
        assert_eq!(shell_quote(Path::new("/a/Star Pilot")), "'/a/Star Pilot'");
        assert_eq!(shell_quote(Path::new("/a/it's")), "'/a/it'\\''s'");
    }

    #[test]
    fn paths_are_tidied_without_the_file_system() {
        assert_eq!(absolute(Path::new("/x/./y/../z")), PathBuf::from("/x/z"));
    }
}
