// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Which gamepads Polter sees, and which of the console's buttons they press: a check with a real
//! pad. `cargo run --release --example pads [SECONDS]`, then press the pad's four directions, lean
//! the left stick, and press the lower and the right face button. It stops once all six buttons
//! have arrived, or after SECONDS (30 by default).

use polter::input::Button;
use polter::pad::Pads;
use std::time::{Duration, Instant};

#[allow(
    clippy::disallowed_methods,
    reason = "a tool that waits for a person's hands, not a game: it may look at the clock"
)]
fn main() {
    let seconds: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    let mut pads = Pads::open();
    // a pad that is plugged in announces itself in the first events
    std::thread::sleep(Duration::from_millis(300));
    let mut names = pads.names();
    println!(
        "gamepads: {}",
        if names.is_empty() {
            "none".to_string()
        } else {
            names.join(", ")
        }
    );
    println!(
        "up to {seconds} s: press the directions, lean the stick, press the lower and the right face button"
    );
    let start = Instant::now();
    let (mut last, mut seen) = (0u8, 0u8);
    let every = Button::ALL.iter().fold(0, |a, b| a | b.bit());
    while start.elapsed() < Duration::from_secs(seconds) && seen != every {
        let now = pads.names();
        if now != names {
            println!(
                "{:6.2} s  gamepads now: {}",
                start.elapsed().as_secs_f32(),
                now.join(", ")
            );
            names = now;
        }
        let b = pads.buttons();
        if b != last {
            let shown = Button::names(b);
            println!(
                "{:6.2} s  {}",
                start.elapsed().as_secs_f32(),
                if shown.is_empty() {
                    "(nothing)".to_string()
                } else {
                    shown.join(" ")
                }
            );
            last = b;
            seen |= b;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    let missing: Vec<&str> = Button::ALL
        .iter()
        .filter(|b| seen & b.bit() == 0)
        .map(|b| b.name())
        .collect();
    println!("pressed at least once: {}", Button::names(seen).join(" "));
    println!(
        "never pressed: {}",
        if missing.is_empty() {
            "nothing, every button arrived".to_string()
        } else {
            missing.join(" ")
        }
    );
}
