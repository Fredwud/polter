// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Where games keep save games: files in the system's folder for a program's data when a person
//! plays, memory for headless runs, so a headless run never depends on what happens to lie on
//! the disk.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct Storage {
    dir: Option<PathBuf>,
    memory: BTreeMap<String, Vec<u8>>,
}

/// Keeps names to safe file names: letters, digits, `-`, `_` and `.`.
fn clean(name: &str) -> Option<String> {
    let ok = !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    ok.then(|| name.to_string())
}

impl Storage {
    /// Nothing on disk: everything lives as long as the run.
    pub fn memory() -> Storage {
        Storage::default()
    }

    /// Files in a folder (created on first write).
    pub fn dir(path: PathBuf) -> Storage {
        Storage {
            dir: Some(path),
            memory: BTreeMap::new(),
        }
    }

    /// A game's saves when a person plays, in the system's folder for a program's data
    /// (`save_dir`). Saves from before (`~/.ember8/<game>`, when the engine was EMBER-8) are
    /// copied there the first time; the old folder stays as it was, a backup.
    pub fn for_cart(cart: &str) -> Storage {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let dir = save_dir(cart);
        match bring_old_saves(&home.join(".ember8").join(cart), &dir) {
            Ok(0) => {}
            Ok(n) => eprintln!(
                "{n} save file(s) copied from ~/.ember8/{cart} to {} (the old folder stays as a backup)",
                dir.display()
            ),
            Err(e) => eprintln!("could not copy the old saves of {cart}: {e}"),
        }
        Storage::dir(dir)
    }

    pub fn read(&self, name: &str) -> Option<Vec<u8>> {
        let name = clean(name)?;
        match &self.dir {
            Some(dir) => std::fs::read(dir.join(name)).ok(),
            None => self.memory.get(&name).cloned(),
        }
    }

    /// Stores data under a name; false if the name is not allowed or the disk refused.
    pub fn write(&mut self, name: &str, data: &[u8]) -> bool {
        let Some(name) = clean(name) else {
            return false;
        };
        match &self.dir {
            Some(dir) => std::fs::create_dir_all(dir)
                .and_then(|_| std::fs::write(dir.join(name), data))
                .is_ok(),
            None => {
                self.memory.insert(name, data.to_vec());
                true
            }
        }
    }
}

/// The system's folder for a game's data: on a Mac `~/Library/Application Support/<game>`, on
/// Linux `$XDG_DATA_HOME/<game>` (or `~/.local/share/<game>`), on Windows `%APPDATA%\<game>`.
pub fn save_dir(game: &str) -> PathBuf {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let home = env("HOME").unwrap_or_else(|| PathBuf::from("."));
    let base = if cfg!(target_os = "macos") {
        home.join("Library/Application Support")
    } else if cfg!(windows) {
        env("APPDATA").unwrap_or(home)
    } else {
        env("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"))
    };
    base.join(game)
}

/// Copies save files from an old folder into a new one, once: only while the new folder
/// doesn't exist yet, so nothing played since is ever overwritten. The old folder is left as
/// it was. Returns how many files were copied.
pub fn bring_old_saves(old: &Path, new: &Path) -> std::io::Result<usize> {
    if new.exists() || !old.is_dir() {
        return Ok(0);
    }
    let files: Vec<PathBuf> = std::fs::read_dir(old)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .collect();
    if files.is_empty() {
        return Ok(0);
    }
    // into a folder beside it first, then renamed: a copy cut short leaves no half folder
    let partial = new.with_extension("partial");
    let _ = std::fs::remove_dir_all(&partial);
    std::fs::create_dir_all(&partial)?;
    for f in &files {
        if let Some(name) = f.file_name() {
            std::fs::copy(f, partial.join(name))?;
        }
    }
    std::fs::rename(&partial, new)?;
    Ok(files.len())
}
