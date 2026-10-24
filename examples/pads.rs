// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Which gamepads Polter sees, and what of them reaches a game: a check with a real pad.
//! `cargo run --release --example pads [SECONDS]` (90 by default). First keep your hands off for a
//! second: it measures where the sticks rest. Then press every button (the four face buttons,
//! L1 R1 L2 R2, Share, Options, the PS button, both sticks pressed in, the four arrows), lean both
//! sticks all the way and a little, and squeeze both triggers slowly. It writes what arrives as it
//! changes, and stops once everything has, or after SECONDS. At the end: what never came, how far
//! the sticks and triggers went, and in how many steps between (a light lean should arrive as a
//! small value, not as nothing or everything).

use polter::input::{Button, Frame, Input, PadButton, PadFrame, Stick};
use polter::pad::Pads;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

/// How far a stick leans, 0 to about 1.
fn lean(input: &Input, s: Stick) -> f32 {
    let (x, y) = input.stick(s);
    (x * x + y * y).sqrt()
}

fn input_of(pad: PadFrame) -> Input {
    let mut i = Input::default();
    i.advance(&Frame {
        pad,
        ..Frame::default()
    });
    i
}

fn names(buttons: u32) -> String {
    let n: Vec<&str> = PadButton::ALL
        .iter()
        .filter(|b| buttons & b.bit() != 0)
        .map(|b| b.name())
        .collect();
    if n.is_empty() {
        "(nothing)".into()
    } else {
        n.join(" ")
    }
}

#[allow(
    clippy::disallowed_methods,
    reason = "a tool that waits for a person's hands, not a game: it may look at the clock"
)]
fn main() {
    let seconds: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(90);
    let mut pads = Pads::open();
    // a pad that is plugged in announces itself in the first events
    std::thread::sleep(Duration::from_millis(300));
    let mut seen_pads = pads.names();
    println!(
        "gamepads: {}",
        if seen_pads.is_empty() {
            "none".to_string()
        } else {
            seen_pads.join(", ")
        }
    );
    // where the sticks rest, hands off
    println!("hands off for a second …");
    let mut rest = [0.0f32; 2];
    let until = Instant::now() + Duration::from_secs(1);
    while Instant::now() < until {
        let i = input_of(pads.read().1);
        for (k, s) in [Stick::Left, Stick::Right].into_iter().enumerate() {
            rest[k] = rest[k].max(lean(&i, s));
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    println!(
        "at rest the sticks lean up to {:.3} (left) and {:.3} (right)",
        rest[0], rest[1]
    );
    println!(
        "up to {seconds} s: press every button, lean both sticks all the way and a little, squeeze both triggers slowly"
    );
    let start = Instant::now();
    let every = PadButton::ALL.iter().fold(0, |a, b| a | b.bit());
    let every_console = Button::ALL.iter().fold(0, |a, b| a | b.bit());
    let (mut seen, mut seen_console) = (0u32, 0u8);
    let mut last = PadFrame::default();
    // tenths shown last, and the steps each stick and trigger has been at (in hundredths)
    let mut shown = [0i32; 6];
    let mut steps: [BTreeSet<i32>; 4] = Default::default();
    let mut most = [0.0f32; 4];
    let complete = |seen: u32, most: &[f32; 4], steps: &[BTreeSet<i32>; 4]| {
        seen == every && most.iter().all(|&m| m > 0.95) && steps.iter().all(|s| s.len() >= 10)
    };
    while start.elapsed() < Duration::from_secs(seconds) && !complete(seen, &most, &steps) {
        let at = start.elapsed().as_secs_f32();
        let now = pads.names();
        if now != seen_pads {
            println!("{at:6.2} s  gamepads now: {}", now.join(", "));
            seen_pads = now;
        }
        let (console, pad) = pads.read();
        let i = input_of(pad);
        if pad.buttons != last.buttons {
            let c = Button::names(console);
            println!(
                "{at:6.2} s  {}   (console: {})",
                names(pad.buttons),
                if c.is_empty() {
                    "nothing".into()
                } else {
                    c.join(" ")
                }
            );
        }
        seen |= pad.buttons;
        seen_console |= console;
        let (lx, ly) = i.stick(Stick::Left);
        let (rx, ry) = i.stick(Stick::Right);
        let values = [
            lx,
            ly,
            rx,
            ry,
            i.trigger(Stick::Left),
            i.trigger(Stick::Right),
        ];
        let tenths = values.map(|v| (v * 10.0).round() as i32);
        if tenths != shown {
            println!(
                "{at:6.2} s  left stick {lx:5.2},{ly:5.2}  right stick {rx:5.2},{ry:5.2}  L2 {:.2}  R2 {:.2}",
                values[4], values[5]
            );
            shown = tenths;
        }
        let amounts = [
            lean(&i, Stick::Left),
            lean(&i, Stick::Right),
            values[4],
            values[5],
        ];
        for k in 0..4 {
            most[k] = most[k].max(amounts[k]);
            // a step counts where it is clear of how the stick rests
            let floor = if k < 2 { rest[k] + 0.02 } else { 0.0 };
            if amounts[k] > floor {
                steps[k].insert((amounts[k] * 100.0) as i32);
            }
        }
        last = pad;
        std::thread::sleep(Duration::from_millis(16));
    }
    println!("pressed at least once: {}", names(seen));
    let missing = every & !seen;
    println!(
        "never pressed: {}",
        if missing == 0 {
            "nothing, every button arrived".to_string()
        } else {
            names(missing)
        }
    );
    let console_missing = every_console & !seen_console;
    println!(
        "console buttons never pressed: {}",
        if console_missing == 0 {
            "nothing".to_string()
        } else {
            Button::names(console_missing).join(" ")
        }
    );
    for (k, name) in ["left stick", "right stick", "L2", "R2"].iter().enumerate() {
        println!(
            "{name:<11} furthest {:.2}, in {} steps of a hundredth{}",
            most[k],
            steps[k].len(),
            steps[k].iter().next().map_or(String::new(), |s| format!(
                ", the least {:.2}",
                *s as f32 / 100.0
            ))
        );
    }
}
