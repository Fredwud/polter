// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Everything for making a game with Polter, and nothing a player needs: the command line every
//! game gets (`check`, `probe`, `peek`, `twin`, `listen`, `assert`, `knobs`, `workshop`, `play`
//! with recording), scenarios, contact sheets, sound pictures, the workshop. A game links it while
//! it is made (the `dev` feature) and leaves it out when it ships, so the runtime stays small
//! (SPEC 6.6).

pub mod cli;
pub mod ears;
pub mod gif;
pub mod scenario;
pub mod sheet;
pub mod workshop;
