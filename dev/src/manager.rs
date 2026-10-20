// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `polter manager`: the project manager as a window, as Godot opens with one. The games known
//! here, newest first; a game opens in the workshop or plays; a new game is made in a folder chosen
//! in the window, an existing one is added from where it is. Every key does something: nothing on
//! it is decoration.

use crate::{Fail, engine_dir, forget_in, known_in, new_game, remember_in, title_of};
use polter::Canvas;
use polter::input::{Key, Mouse};
use polter::paint::{self, Rect, colour::*};
use polter::window::{Conf, Fit, Host, Io};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const ROW: i32 = 11;
const SIDE: i32 = 122;
const KEY_H: i32 = 16;
const PLATE: u32 = 0x202227;
const BODY: u32 = 0x1c1d21;

/// A game on the list: its folder, and its title (none when the folder is gone).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Game {
    pub dir: PathBuf,
    pub title: Option<String>,
}

/// A folder in the browser.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Folder {
    pub name: String,
    pub path: PathBuf,
    /// It holds a game (a polter.toml).
    pub game: bool,
}

/// Choosing a folder inside the window: the folder shown, its subfolders, the one picked.
pub(crate) struct Browser {
    pub dir: PathBuf,
    pub folders: Vec<Folder>,
    pub picked: Option<usize>,
    pub scroll: usize,
}

impl Browser {
    pub(crate) fn at(dir: &Path) -> Browser {
        let mut folders: Vec<Folder> = std::fs::read_dir(dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| e.path().is_dir())
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        (!name.starts_with('.')).then(|| Folder {
                            game: e.path().join("polter.toml").is_file(),
                            path: e.path(),
                            name,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        folders.sort_by_key(|f| f.name.to_lowercase());
        Browser {
            dir: dir.to_path_buf(),
            folders,
            picked: None,
            scroll: 0,
        }
    }

    fn up(&mut self) {
        if let Some(parent) = self.dir.parent() {
            *self = Browser::at(parent);
        }
    }
}

/// What the window shows over the list, if anything.
pub(crate) enum Dialog {
    None,
    /// A new game: its name, typed, and the folder it goes into.
    New {
        name: String,
        browser: Browser,
    },
    /// An existing game, found in the browser.
    Add {
        browser: Browser,
    },
}

/// What the mouse can be on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hit {
    Game(usize),
    Workshop,
    Play,
    Open,
    Remove,
    New,
    Add,
    Folder(usize),
    Up,
    Home,
    Projects,
    Confirm,
    Cancel,
}

/// Where things sit.
pub(crate) struct Layout {
    pub list: Rect,
    pub keys: Vec<(Hit, Rect)>,
    pub status: Rect,
    pub dialog: Rect,
    pub name_field: Rect,
    pub folders: Rect,
    pub dialog_keys: Vec<(Hit, Rect)>,
}

impl Layout {
    fn new(w: usize, h: usize) -> Layout {
        let (w, h) = (w as i32, h as i32);
        let list = Rect {
            x: 6,
            y: 24,
            w: (w - SIDE - 18).max(10),
            h: (h - 24 - 26).max(ROW),
        };
        let kx = w - SIDE - 6;
        let mut keys = Vec::new();
        let mut y = 24;
        for (i, hit) in [
            Hit::Workshop,
            Hit::Play,
            Hit::Open,
            Hit::Remove,
            Hit::New,
            Hit::Add,
        ]
        .into_iter()
        .enumerate()
        {
            if i == 4 {
                y += 10;
            }
            keys.push((
                hit,
                Rect {
                    x: kx,
                    y,
                    w: SIDE,
                    h: KEY_H,
                },
            ));
            y += KEY_H + 4;
        }
        let status = Rect {
            x: 6,
            y: h - 20,
            w: w - 12,
            h: 15,
        };
        let dialog = Rect {
            x: 16,
            y: 12,
            w: (w - 32).max(40),
            h: (h - 30).max(40),
        };
        let name_field = Rect {
            x: dialog.x + 44,
            y: dialog.y + 22,
            w: dialog.w - 52,
            h: 13,
        };
        let folders = Rect {
            x: dialog.x + 8,
            y: dialog.y + 56,
            w: dialog.w - 16 - SIDE / 2 - 8,
            h: (dialog.h - 56 - 26).max(ROW),
        };
        let dk = SIDE / 2 + 8;
        let mut dialog_keys = Vec::new();
        let mut y = folders.y;
        for hit in [Hit::Up, Hit::Home, Hit::Projects] {
            dialog_keys.push((
                hit,
                Rect {
                    x: dialog.x + dialog.w - 8 - (dk - 8),
                    y,
                    w: dk - 8,
                    h: KEY_H,
                },
            ));
            y += KEY_H + 4;
        }
        let by = dialog.y + dialog.h - KEY_H - 6;
        for (i, hit) in [Hit::Cancel, Hit::Confirm].into_iter().enumerate() {
            dialog_keys.push((
                hit,
                Rect {
                    x: dialog.x + dialog.w - 8 - (2 - i as i32) * 64 + 4,
                    y: by,
                    w: 60,
                    h: KEY_H,
                },
            ));
        }
        Layout {
            list,
            keys,
            status,
            dialog,
            name_field,
            folders,
            dialog_keys,
        }
    }

    fn rows(r: Rect) -> usize {
        (r.h / ROW).max(1) as usize
    }
}

/// Starts a command of the polter program on a game (in its own process); tests replace it.
type Launch = Box<dyn FnMut(&str, &Path) -> Result<(), String>>;

pub(crate) struct Manager {
    list_file: PathBuf,
    pub games: Vec<Game>,
    pub picked: usize,
    scroll: usize,
    pub dialog: Dialog,
    note: Option<(String, u32)>,
    hover: Option<Hit>,
    held: Option<Hit>,
    last_click: Option<(Hit, u32)>,
    frame: u32,
    start_dir: PathBuf,
    launch: Launch,
}

impl Manager {
    pub(crate) fn new(list_file: PathBuf, start_dir: PathBuf, launch: Launch) -> Manager {
        let mut m = Manager {
            list_file,
            games: Vec::new(),
            picked: 0,
            scroll: 0,
            dialog: Dialog::None,
            note: None,
            hover: None,
            held: None,
            last_click: None,
            frame: 0,
            start_dir,
            launch,
        };
        m.reload();
        m
    }

    fn reload(&mut self) {
        self.games = known_in(&self.list_file)
            .into_iter()
            .map(|dir| Game {
                title: title_of(&dir),
                dir,
            })
            .collect();
        self.picked = self.picked.min(self.games.len().saturating_sub(1));
    }

    fn tell(&mut self, s: impl Into<String>) {
        self.note = Some((s.into().to_uppercase(), 240));
    }

    fn picked_game(&self) -> Option<&Game> {
        self.games.get(self.picked)
    }

    fn enabled(&self, hit: Hit) -> bool {
        let present = self.picked_game().is_some_and(|g| g.title.is_some());
        match hit {
            Hit::Workshop | Hit::Play | Hit::Open => present,
            Hit::Remove => self.picked_game().is_some(),
            Hit::Confirm => match &self.dialog {
                Dialog::New { name, browser } => {
                    !name.trim().is_empty() && empty_or_missing(&browser.dir.join(name.trim()))
                }
                Dialog::Add { browser } => self.add_target(browser).is_some(),
                Dialog::None => false,
            },
            Hit::Projects => projects_dir().is_dir(),
            _ => true,
        }
    }

    /// The game a press of ADD would add: the picked folder if it is one, or the folder shown.
    fn add_target(&self, browser: &Browser) -> Option<PathBuf> {
        if let Some(f) = browser.picked.and_then(|i| browser.folders.get(i))
            && f.game
        {
            return Some(f.path.clone());
        }
        browser
            .dir
            .join("polter.toml")
            .is_file()
            .then(|| browser.dir.clone())
    }

    fn press(&mut self, hit: Hit, double: bool) {
        if !self.enabled(hit) && !matches!(hit, Hit::Game(_) | Hit::Folder(_)) {
            return;
        }
        match hit {
            Hit::Game(i) => {
                self.picked = i;
                if double && self.enabled(Hit::Workshop) {
                    self.press(Hit::Workshop, false);
                }
            }
            Hit::Workshop | Hit::Play => {
                let what = if hit == Hit::Workshop {
                    "workshop"
                } else {
                    "play"
                };
                if let Some(g) = self.picked_game().cloned() {
                    let title = g.title.clone().unwrap_or_default();
                    match (self.launch)(what, &g.dir) {
                        Ok(()) => {
                            remember_in(&self.list_file, &g.dir);
                            self.reload();
                            self.picked = 0;
                            self.tell(format!("{what}: {title} (built first, it opens by itself)"));
                        }
                        Err(e) => self.tell(format!("cannot start: {e}")),
                    }
                }
            }
            Hit::Open => {
                if let Some(g) = self.picked_game().cloned() {
                    let opener = if cfg!(target_os = "macos") {
                        "open"
                    } else {
                        "xdg-open"
                    };
                    let shown = std::process::Command::new(opener).arg(&g.dir).spawn();
                    if let Err(e) = shown {
                        self.tell(format!("cannot show the folder: {e}"));
                    }
                }
            }
            Hit::Remove => {
                if let Some(g) = self.picked_game().cloned() {
                    forget_in(&self.list_file, &g.dir);
                    self.reload();
                    let title = g.title.unwrap_or_else(|| "it".into());
                    self.tell(format!(
                        "{title} is off the list; its files are still there"
                    ));
                }
            }
            Hit::New => {
                self.dialog = Dialog::New {
                    name: String::new(),
                    browser: Browser::at(&self.start_dir),
                };
            }
            Hit::Add => {
                self.dialog = Dialog::Add {
                    browser: Browser::at(&self.start_dir),
                };
            }
            Hit::Cancel => self.dialog = Dialog::None,
            Hit::Up | Hit::Home | Hit::Projects | Hit::Folder(_) => self.browse(hit, double),
            Hit::Confirm => self.confirm(),
        }
    }

    fn browse(&mut self, hit: Hit, double: bool) {
        let target_of_add = match &self.dialog {
            Dialog::Add { browser } => match hit {
                Hit::Folder(i) => browser
                    .folders
                    .get(i)
                    .filter(|f| f.game)
                    .map(|f| f.path.clone()),
                _ => None,
            },
            _ => None,
        };
        let browser = match &mut self.dialog {
            Dialog::New { browser, .. } | Dialog::Add { browser } => browser,
            Dialog::None => return,
        };
        match hit {
            Hit::Up => browser.up(),
            Hit::Home => *browser = Browser::at(&home()),
            Hit::Projects => *browser = Browser::at(&projects_dir()),
            Hit::Folder(i) => {
                if double {
                    if target_of_add.is_some() {
                        // a game double-clicked in "add": add it
                        browser.picked = Some(i);
                        self.confirm();
                        return;
                    }
                    if let Some(f) = browser.folders.get(i).cloned() {
                        *browser = Browser::at(&f.path);
                    }
                } else {
                    browser.picked = Some(i);
                }
            }
            _ => {}
        }
    }

    fn confirm(&mut self) {
        match std::mem::replace(&mut self.dialog, Dialog::None) {
            Dialog::New { name, browser } => {
                let dir = browser.dir.join(name.trim());
                match new_game(&dir, &engine_dir()) {
                    Ok(_) => {
                        remember_in(&self.list_file, &dir);
                        self.reload();
                        self.picked = 0;
                        self.tell(format!(
                            "made {}: WORKSHOP opens it (the first build takes a moment)",
                            name.trim()
                        ));
                    }
                    Err(Fail::Tool(e) | Fail::Usage(e)) => {
                        self.tell(format!("not made: {e}"));
                        self.dialog = Dialog::New { name, browser };
                    }
                }
            }
            Dialog::Add { browser } => match self.add_target(&browser) {
                Some(dir) => {
                    remember_in(&self.list_file, &dir);
                    self.reload();
                    self.picked = 0;
                    let title = title_of(&dir).unwrap_or_default();
                    self.tell(format!("added {title}"));
                }
                None => self.dialog = Dialog::Add { browser },
            },
            Dialog::None => {}
        }
    }

    fn hit_at(&self, lay: &Layout, p: (i32, i32)) -> Option<Hit> {
        match &self.dialog {
            Dialog::None => {
                if let Some((hit, _)) = lay.keys.iter().find(|(_, r)| r.contains(p)) {
                    return Some(*hit);
                }
                if lay.list.contains(p) {
                    let i = self.scroll + ((p.1 - lay.list.y) / ROW) as usize;
                    return (i < self.games.len()).then_some(Hit::Game(i));
                }
                None
            }
            Dialog::New { browser, .. } | Dialog::Add { browser } => {
                if let Some((hit, _)) = lay.dialog_keys.iter().find(|(_, r)| r.contains(p)) {
                    return Some(*hit);
                }
                if lay.folders.contains(p) {
                    let i = browser.scroll + ((p.1 - lay.folders.y) / ROW) as usize;
                    return (i < browser.folders.len()).then_some(Hit::Folder(i));
                }
                None
            }
        }
    }

    /// One display frame: keys, typing, the mouse.
    pub(crate) fn step(&mut self, io: &Io, w: usize, h: usize) {
        self.frame += 1;
        let lay = Layout::new(w, h);
        let input = io.input;
        // typing a new game's name
        if let Dialog::New { name, .. } = &mut self.dialog {
            for &ch in io.typed {
                if !ch.is_control() && !['/', '\\', ':'].contains(&ch) && name.chars().count() < 40
                {
                    name.push(ch);
                }
            }
            if input.key_pressed(Key::Backspace) {
                name.pop();
            }
        }
        if input.key_pressed(Key::Escape) {
            self.dialog = Dialog::None;
        }
        if input.key_pressed(Key::Enter) {
            match self.dialog {
                Dialog::None => self.press(Hit::Workshop, false),
                _ => self.press(Hit::Confirm, false),
            }
        }
        if matches!(self.dialog, Dialog::None) && !self.games.is_empty() {
            if input.key_pressed(Key::Down) {
                self.picked = (self.picked + 1).min(self.games.len() - 1);
            }
            if input.key_pressed(Key::Up) {
                self.picked = self.picked.saturating_sub(1);
            }
        }
        let p = input.mouse();
        self.hover = self.hit_at(&lay, p);
        if input.mouse_pressed(Mouse::LEFT)
            && let Some(hit) = self.hover
        {
            let double = self
                .last_click
                .is_some_and(|(h, f)| h == hit && self.frame - f <= 25);
            self.last_click = Some((hit, self.frame));
            self.held = Some(hit);
            self.press(hit, double);
        }
        if !input.mouse_held(Mouse::LEFT) {
            self.held = None;
        }
        // the wheel scrolls what it is over
        let wheel = input.wheel();
        if wheel != 0 {
            match &mut self.dialog {
                Dialog::None => {
                    let most = self.games.len().saturating_sub(Layout::rows(lay.list));
                    self.scroll = (self.scroll as i32 - wheel).clamp(0, most as i32) as usize;
                }
                Dialog::New { browser, .. } | Dialog::Add { browser } => {
                    let most = browser
                        .folders
                        .len()
                        .saturating_sub(Layout::rows(lay.folders));
                    browser.scroll = (browser.scroll as i32 - wheel).clamp(0, most as i32) as usize;
                }
            }
        }
        // keep the picked game in view
        let rows = Layout::rows(lay.list);
        if self.picked < self.scroll {
            self.scroll = self.picked;
        } else if self.picked >= self.scroll + rows {
            self.scroll = self.picked + 1 - rows;
        }
        if let Some((_, left)) = &mut self.note {
            *left = left.saturating_sub(1);
            if *left == 0 {
                self.note = None;
            }
        }
    }

    /// Paints the window.
    pub(crate) fn draw(&self, c: &mut Canvas) {
        let (w, h) = (c.width() as i32, c.height() as i32);
        let lay = Layout::new(c.width(), c.height());
        paint::brushed(c, Rect { x: 0, y: 0, w, h }, BODY);
        // the nameplate
        let plate = Rect {
            x: 6,
            y: 5,
            w: w - 12,
            h: 13,
        };
        paint::fill(c, plate, 0x101114);
        paint::bevel(c, plate, EDGE_DARK, EDGE_LIGHT);
        paint::text(c, "POLTER", plate.x + 5, plate.y + 4, ACCENT);
        paint::text(c, "GAMES", plate.x + 33, plate.y + 4, SILK);
        let n = format!("{} KNOWN HERE", self.games.len());
        paint::text(
            c,
            &n,
            plate.x + plate.w - 5 - paint::text_w(&n),
            plate.y + 4,
            SILK,
        );
        // the list
        let l = lay.list;
        paint::fill(c, l, 0x121317);
        paint::bevel(c, l, EDGE_DARK, EDGE_LIGHT);
        if self.games.is_empty() {
            paint::text(
                c,
                "NO GAMES YET: NEW GAME MAKES ONE",
                l.x + 8,
                l.y + 8,
                SILK,
            );
        }
        for (row, (i, g)) in self
            .games
            .iter()
            .enumerate()
            .skip(self.scroll)
            .take(Layout::rows(l))
            .enumerate()
        {
            let y = l.y + 2 + row as i32 * ROW;
            let r = Rect {
                x: l.x + 2,
                y,
                w: l.w - 4,
                h: ROW - 1,
            };
            if i == self.picked {
                paint::fill(c, r, 0x2a2d35);
                paint::vline(c, r.x, r.y, r.y + r.h - 1, ACCENT);
            } else if self.hover == Some(Hit::Game(i)) {
                paint::fill(c, r, 0x1d1f25);
            }
            paint::led(
                c,
                r.x + 4,
                y + 4,
                if g.title.is_some() { GREEN } else { RED },
                true,
            );
            let title = g.title.clone().unwrap_or_else(|| "MISSING".into());
            let clip = Rect {
                x: r.x + 10,
                y: r.y,
                w: 92,
                h: r.h,
            };
            paint::text_in(
                c,
                &title.to_uppercase(),
                r.x + 10,
                y + 3,
                if g.title.is_some() { INK } else { RED },
                1,
                clip,
            );
            let path = left_trim(&shown(&g.dir), ((r.w - 110) / 4).max(4) as usize);
            paint::text(c, &path.to_uppercase(), r.x + 106, y + 3, SILK);
        }
        // the keys
        for (hit, r) in &lay.keys {
            let label = match hit {
                Hit::Workshop => "WORKSHOP",
                Hit::Play => "PLAY",
                Hit::Open => "SHOW FOLDER",
                Hit::Remove => "REMOVE FROM LIST",
                Hit::New => "NEW GAME",
                Hit::Add => "ADD A GAME",
                _ => "",
            };
            let active = matches!(self.dialog, Dialog::None);
            paint::key(
                c,
                *r,
                label,
                active && self.enabled(*hit),
                active && self.hover == Some(*hit),
                self.held == Some(*hit),
            );
        }
        // the status line
        paint::glass(c, lay.status);
        let line = match &self.note {
            Some((s, _)) => s.clone(),
            None => "DOUBLE-CLICK OR ENTER: WORKSHOP · UP/DOWN: CHOOSE · CMD+Q: QUIT".into(),
        };
        let clip = Rect {
            x: lay.status.x + 3,
            y: lay.status.y + 2,
            w: lay.status.w - 6,
            h: lay.status.h - 4,
        };
        paint::text_in(c, &line, lay.status.x + 5, lay.status.y + 5, TEAL, 1, clip);
        self.draw_dialog(c, &lay);
    }

    fn draw_dialog(&self, c: &mut Canvas, lay: &Layout) {
        let (title, browser, name) = match &self.dialog {
            Dialog::None => return,
            Dialog::New { name, browser } => ("NEW GAME", browser, Some(name)),
            Dialog::Add { browser } => ("ADD A GAME", browser, None),
        };
        let d = lay.dialog;
        // the window under it, darkened
        for y in 0..c.height() as i32 {
            for x in 0..c.width() as i32 {
                let p = c.get(x, y);
                c.set(x, y, paint::mix(p, 0x000000, 0.55));
            }
        }
        paint::brushed(c, d, PLATE);
        paint::bevel(c, d, EDGE_LIGHT, EDGE_DARK);
        paint::text(c, title, d.x + 8, d.y + 7, ACCENT);
        let f = lay.name_field;
        match name {
            Some(name) => {
                paint::text(c, "NAME", d.x + 8, f.y + 4, SILK);
                paint::glass(c, f);
                let cursor = if (self.frame / 30).is_multiple_of(2) {
                    "_"
                } else {
                    ""
                };
                paint::text_in(
                    c,
                    &format!("{}{cursor}", name.to_uppercase()),
                    f.x + 4,
                    f.y + 4,
                    TEAL,
                    1,
                    f,
                );
            }
            None => paint::text(
                c,
                "PICK A FOLDER WITH A GAME IN IT (A LAMP) AND ADD IT",
                d.x + 8,
                f.y + 4,
                SILK,
            ),
        }
        // where it goes
        paint::text(c, "IN", d.x + 8, d.y + 43, SILK);
        let width = ((d.w - 52) / 4).max(4) as usize;
        let place = left_trim(&shown(&browser.dir), width);
        paint::text(c, &place.to_uppercase(), d.x + 44, d.y + 43, INK);
        // the folders
        let fr = lay.folders;
        paint::fill(c, fr, 0x121317);
        paint::bevel(c, fr, EDGE_DARK, EDGE_LIGHT);
        if browser.folders.is_empty() {
            paint::text(c, "NO FOLDERS HERE", fr.x + 8, fr.y + 6, SILK);
        }
        for (row, (i, folder)) in browser
            .folders
            .iter()
            .enumerate()
            .skip(browser.scroll)
            .take(Layout::rows(fr))
            .enumerate()
        {
            let y = fr.y + 2 + row as i32 * ROW;
            let r = Rect {
                x: fr.x + 2,
                y,
                w: fr.w - 4,
                h: ROW - 1,
            };
            if browser.picked == Some(i) {
                paint::fill(c, r, 0x2a2d35);
                paint::vline(c, r.x, r.y, r.y + r.h - 1, ACCENT);
            } else if self.hover == Some(Hit::Folder(i)) {
                paint::fill(c, r, 0x1d1f25);
            }
            if folder.game {
                paint::led(c, r.x + 4, y + 4, GREEN, true);
            }
            paint::text_in(
                c,
                &folder.name.to_uppercase(),
                r.x + 10,
                y + 3,
                if folder.game { INK } else { SILK },
                1,
                r,
            );
        }
        // its keys
        for (hit, r) in &lay.dialog_keys {
            let label = match hit {
                Hit::Up => "UP A FOLDER",
                Hit::Home => "HOME",
                Hit::Projects => "PROJECTS",
                Hit::Cancel => "CANCEL",
                Hit::Confirm => {
                    if name.is_some() {
                        "MAKE IT"
                    } else {
                        "ADD"
                    }
                }
                _ => "",
            };
            paint::key(
                c,
                *r,
                label,
                self.enabled(*hit),
                self.hover == Some(*hit),
                self.held == Some(*hit),
            );
        }
        // the folder the new game will be, or why not
        if let Some(name) = name {
            let target = browser.dir.join(name.trim());
            let (line, col) = if name.trim().is_empty() {
                ("TYPE A NAME".to_string(), SILK)
            } else if empty_or_missing(&target) {
                (format!("MAKES {}", left_trim(&shown(&target), 40)), GREEN)
            } else {
                (
                    "A FOLDER OF THAT NAME IS HERE AND NOT EMPTY".to_string(),
                    RED,
                )
            };
            paint::text(c, &line.to_uppercase(), d.x + 8, d.y + d.h - 15, col);
        }
    }
}

impl Host for Manager {
    fn frame(&mut self, io: &Io, canvas: &mut Canvas) -> bool {
        self.step(io, canvas.width(), canvas.height());
        self.draw(canvas);
        true
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

fn projects_dir() -> PathBuf {
    home().join("Projects")
}

fn empty_or_missing(dir: &Path) -> bool {
    !dir.exists() || std::fs::read_dir(dir).is_ok_and(|mut d| d.next().is_none())
}

/// A path as people read it: the home folder as `~`.
fn shown(p: &Path) -> String {
    let home = home();
    match p.strip_prefix(&home) {
        Ok(rest) if home != Path::new("/") => format!("~/{}", rest.display()),
        _ => p.display().to_string(),
    }
}

/// The end of a long text, `…` in front, at most `max` letters.
fn left_trim(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        s.to_string()
    } else {
        let tail: String = s.chars().skip(n + 3 - max.max(4)).collect();
        format!("...{tail}")
    }
}

/// `polter manager`: the window, or with `--out FILE` a picture of it.
pub(crate) fn run(args: &[String]) -> Result<ExitCode, Fail> {
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let start = if projects_dir().is_dir() {
        projects_dir()
    } else {
        home()
    };
    let launch: Launch = Box::new(|command, dir| {
        let me = std::env::current_exe().map_err(|e| e.to_string())?;
        std::process::Command::new(me)
            .arg(command)
            .arg(dir)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    });
    let mut m = Manager::new(crate::list_file(), start, launch);
    if let Some(out) = value("--out") {
        let (w, h) = value("--size")
            .and_then(|s| {
                let (w, h) = s.split_once(',')?;
                Some((w.parse().ok()?, h.parse().ok()?))
            })
            .unwrap_or((480, 270));
        match value("--dialog").as_deref() {
            Some("new") => m.press(Hit::New, false),
            Some("add") => m.press(Hit::Add, false),
            _ => {}
        }
        let mut c = Canvas::new(w, h);
        m.draw(&mut c);
        std::fs::write(&out, c.to_png(2)).map_err(|e| Fail::Tool(format!("{out}: {e}")))?;
        println!("{out}");
        return Ok(ExitCode::SUCCESS);
    }
    let conf = Conf {
        title: "Polter · games".into(),
        size: (480, 270),
        fit: Fit::Expand,
        remember: Some(crate::list_file().with_file_name("manager_window.txt")),
        hide_cursor: false,
    };
    polter::window::run(conf, Box::new(m));
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polter::input::{Frame, Input};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A person at the manager: clicks, keys and typing, one display frame at a time.
    struct Person {
        m: Manager,
        input: Input,
        canvas: Canvas,
        launched: Rc<RefCell<Vec<(String, PathBuf)>>>,
    }

    impl Person {
        fn new(dir: &Path) -> Person {
            let launched = Rc::new(RefCell::new(Vec::new()));
            let log = launched.clone();
            let m = Manager::new(
                dir.join("projects.txt"),
                dir.to_path_buf(),
                Box::new(move |c, d| {
                    log.borrow_mut().push((c.to_string(), d.to_path_buf()));
                    Ok(())
                }),
            );
            Person {
                m,
                input: Input::default(),
                canvas: Canvas::new(480, 270),
                launched,
            }
        }
        fn frame(&mut self, keys: &[Key], mouse: (i32, i32), click: bool, typed: &[char]) {
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
                typed,
                full_screen: false,
            };
            self.m.frame(&io, &mut self.canvas);
        }
        fn lay(&self) -> Layout {
            Layout::new(self.canvas.width(), self.canvas.height())
        }
        fn click(&mut self, hit: Hit) {
            let lay = self.lay();
            let r = lay
                .keys
                .iter()
                .chain(&lay.dialog_keys)
                .find(|(h, _)| *h == hit)
                .map(|(_, r)| *r)
                .unwrap_or_else(|| panic!("no key {hit:?}"));
            let at = (r.x + r.w / 2, r.y + r.h / 2);
            self.frame(&[], at, true, &[]);
            self.frame(&[], at, false, &[]);
        }
        fn click_folder(&mut self, name: &str, double: bool) {
            let lay = self.lay();
            let (Dialog::New { browser, .. } | Dialog::Add { browser }) = &self.m.dialog else {
                panic!("no browser open");
            };
            let i = browser
                .folders
                .iter()
                .position(|f| f.name == name)
                .expect(name);
            let at = (
                lay.folders.x + 20,
                lay.folders.y + (i as i32 - browser.scroll as i32) * ROW + 5,
            );
            for _ in 0..if double { 2 } else { 1 } {
                self.frame(&[], at, true, &[]);
                self.frame(&[], at, false, &[]);
            }
        }
    }

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("polter-manager-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_person_makes_a_game_in_a_chosen_folder_opens_it_and_takes_it_off_the_list() {
        let dir = temp("make");
        std::fs::create_dir_all(dir.join("games")).unwrap();
        let mut p = Person::new(&dir);
        assert!(p.m.games.is_empty());
        // NEW GAME, into the folder "games", by double-clicking it
        p.click(Hit::New);
        assert!(matches!(p.m.dialog, Dialog::New { .. }));
        p.click_folder("games", true);
        let Dialog::New { browser, .. } = &p.m.dialog else {
            panic!()
        };
        assert_eq!(browser.dir, dir.join("games"));
        // MAKE IT stays shut until there is a name
        assert!(!p.m.enabled(Hit::Confirm));
        p.frame(
            &[],
            (0, 0),
            false,
            &"Star Pilot!".chars().collect::<Vec<_>>(),
        );
        p.frame(&[Key::Backspace], (0, 0), false, &[]);
        let Dialog::New { name, .. } = &p.m.dialog else {
            panic!()
        };
        assert_eq!(name, "Star Pilot");
        p.click(Hit::Confirm);
        assert!(matches!(p.m.dialog, Dialog::None));
        let made = dir.join("games/Star Pilot");
        assert!(made.join("polter.toml").is_file() && made.join("src/lib.rs").is_file());
        assert_eq!(
            p.m.games,
            [Game {
                dir: made.clone(),
                title: Some("Star Pilot".into())
            }]
        );
        // the list is kept in its file
        assert_eq!(
            known_in(&dir.join("projects.txt")),
            std::slice::from_ref(&made)
        );
        // WORKSHOP starts it (here: noted), and so do Enter and a double-click on its row
        p.click(Hit::Workshop);
        p.frame(&[Key::Enter], (0, 0), false, &[]);
        let row = (p.lay().list.x + 30, p.lay().list.y + 6);
        for _ in 0..2 {
            p.frame(&[], row, true, &[]);
            p.frame(&[], row, false, &[]);
        }
        let workshop = ("workshop".to_string(), made.clone());
        assert_eq!(
            p.launched.borrow().as_slice(),
            [workshop.clone(), workshop.clone(), workshop]
        );
        p.click(Hit::Play);
        assert_eq!(p.launched.borrow().last().unwrap().0, "play");
        // a name that is taken: MAKE IT shuts
        p.click(Hit::New);
        p.click_folder("games", true);
        p.frame(
            &[],
            (0, 0),
            false,
            &"Star Pilot".chars().collect::<Vec<_>>(),
        );
        assert!(
            !p.m.enabled(Hit::Confirm),
            "a folder of that name is there and not empty"
        );
        p.frame(&[Key::Escape], (0, 0), false, &[]);
        assert!(matches!(p.m.dialog, Dialog::None));
        // REMOVE takes it off the list; the files stay
        p.click(Hit::Remove);
        assert!(p.m.games.is_empty());
        assert!(made.join("polter.toml").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_person_adds_a_game_from_where_it_is() {
        let dir = temp("add");
        new_game(&dir.join("old/Lantern"), &engine_dir()).unwrap();
        std::fs::create_dir_all(dir.join("old/notes")).unwrap();
        let mut p = Person::new(&dir);
        p.click(Hit::Add);
        p.click_folder("old", true);
        let Dialog::Add { browser } = &p.m.dialog else {
            panic!()
        };
        let lantern = browser
            .folders
            .iter()
            .find(|f| f.name == "Lantern")
            .unwrap();
        assert!(lantern.game, "a game's folder has its lamp");
        assert!(
            !browser
                .folders
                .iter()
                .find(|f| f.name == "notes")
                .unwrap()
                .game
        );
        // ADD is shut until a game is picked; a plain folder doesn't open it
        assert!(!p.m.enabled(Hit::Confirm));
        p.click_folder("notes", false);
        assert!(!p.m.enabled(Hit::Confirm));
        p.click_folder("Lantern", false);
        assert!(p.m.enabled(Hit::Confirm));
        p.click(Hit::Confirm);
        assert_eq!(p.m.games[0].dir, dir.join("old/Lantern"));
        assert_eq!(p.m.games[0].title.as_deref(), Some("Lantern"));
        // a game whose folder went away is shown as missing and can't be opened
        std::fs::remove_dir_all(dir.join("old/Lantern")).unwrap();
        p.m.reload();
        assert_eq!(p.m.games[0].title, None);
        assert!(!p.m.enabled(Hit::Workshop) && p.m.enabled(Hit::Remove));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_manager_draws_at_any_size_with_and_without_a_dialog() {
        let dir = temp("draw");
        let mut p = Person::new(&dir);
        for (w, h) in [(1, 1), (60, 40), (480, 270), (900, 500)] {
            p.canvas = Canvas::new(w, h);
            p.frame(&[], (w as i32 / 2, h as i32 / 2), false, &[]);
            p.m.press(Hit::New, false);
            p.frame(&[], (w as i32 / 2, h as i32 / 2), false, &['x']);
            p.m.dialog = Dialog::None;
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn home_is_shown_as_a_tilde() {
        assert_eq!(shown(&home().join("Projects/x")), "~/Projects/x");
        assert_eq!(shown(Path::new("/tmp/x")), "/tmp/x");
    }

    #[test]
    fn long_paths_keep_their_end() {
        assert_eq!(left_trim("/a/b", 10), "/a/b");
        assert_eq!(left_trim("/Users/someone/Projects/star", 10), "...ts/star");
    }
}
