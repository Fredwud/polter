// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Everything gilrs hears from the gamepads, unfiltered: for when `examples/pads` sees a pad but
//! none of its buttons. `cargo run --release --example pads_raw [SECONDS]` (10 by default) writes
//! each pad's identity and where its mapping comes from, then every event with its time and raw
//! code, then how many events of each kind came.

use gilrs::{EventType, GilrsBuilder};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[allow(
    clippy::disallowed_methods,
    reason = "a tool that waits for a person's hands, not a game: it may look at the clock"
)]
fn main() {
    let seconds: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let mut g = match GilrsBuilder::new().with_default_filters(false).build() {
        Ok(g) => g,
        Err(e) => {
            println!("gilrs could not start: {e}");
            return;
        }
    };
    let start = Instant::now();
    let mut kinds: BTreeMap<String, u32> = BTreeMap::new();
    let mut shown = 0;
    while start.elapsed() < Duration::from_secs(seconds) {
        while let Some(ev) = g.next_event() {
            let t = start.elapsed().as_secs_f32();
            let kind = format!("{:?}", ev.event)
                .split(['(', ' '])
                .next()
                .unwrap_or("")
                .to_string();
            *kinds.entry(kind).or_default() += 1;
            if let EventType::Connected = ev.event {
                let pad = g.gamepad(ev.id);
                println!(
                    "{t:6.2} s  connected: {:?} (os: {:?}), vendor {:04x?}, product {:04x?}, mapping {:?}, power {:?}",
                    pad.name(),
                    pad.os_name(),
                    pad.vendor_id(),
                    pad.product_id(),
                    pad.mapping_source(),
                    pad.power_info()
                );
            } else if shown < 400 {
                println!("{t:6.2} s  pad {}: {:?}", ev.id, ev.event);
                shown += 1;
            }
        }
        std::thread::sleep(Duration::from_millis(4));
    }
    for (_, pad) in g.gamepads() {
        println!(
            "at the end: {:?} connected {}, {} buttons and {} axes known",
            pad.name(),
            pad.is_connected(),
            pad.state().buttons().count(),
            pad.state().axes().count()
        );
    }
    println!(
        "events in {seconds} s: {}",
        if kinds.is_empty() {
            "none".to_string()
        } else {
            kinds
                .iter()
                .map(|(k, n)| format!("{k} {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
}
