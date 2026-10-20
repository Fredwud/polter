// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! LANTERN, a Polter game. `polter help` lists every command; `polter ship` builds it for
//! players, without the tools.

const NAME: &str = "lantern";

/// While the game is made: every command (play, workshop, check, peek …).
#[cfg(feature = "dev")]
fn main() -> std::process::ExitCode {
    polter_dev::cli::run_games(&[polter_dev::cli::GameInfo {
        name: NAME,
        about: "Carry a lantern through a dark cave, gather the five sparks, open the door.",
        make: || Box::new(lantern::Lantern::new()),
    }])
}

/// What a player gets: the game in its window, nothing else.
#[cfg(not(feature = "dev"))]
fn main() {
    polter::app::play(NAME, Box::new(lantern::Lantern::new()));
}
