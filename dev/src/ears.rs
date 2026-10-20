// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Ears for agents: a picture of sound. A waveform on top, a spectrogram below, frame numbers,
//! and the name of every sound where it starts. Timing, loudness and pitch slides at a glance.

use polter::audio::{SAMPLE_RATE, SAMPLES_PER_FRAME};
use polter::font;
use polter::palette::{Col, Palette};

const WINDOW: usize = 1024;
const BINS: usize = 48;
const LOW_HZ: f32 = 80.0;
const HIGH_HZ: f32 = 8_000.0;
const RAMP: [Col; 7] = [
    Col::Black,
    Col::Gunmetal,
    Col::Steel,
    Col::Beret,
    Col::Ember,
    Col::Visor,
    Col::Shine,
];

/// Renders samples (and the frames where sounds started) as an image. Returns (width, height, rgb).
pub fn ears(samples: &[f32], events: &[(u64, &str)], pal: &Palette) -> (usize, usize, Vec<u8>) {
    // About 6 pixels per frame, but never wider than ~1600 pixels.
    let hop = (samples.len() / 1600).max(128);
    let columns = samples.len().div_ceil(hop).max(1);
    let (margin, label_rows, row_h) = (8, 3, 14);
    let ticks_y = margin + label_rows * row_h;
    let wave_y = ticks_y + 14;
    let wave_h = 80;
    let spec_y = wave_y + wave_h + 6;
    let cell = 2;
    let w = margin * 2 + columns;
    let h = spec_y + BINS * cell + margin;
    let colour = |c: Col| pal.rgb(c, 0, 0);
    let mut img = vec![colour(Col::Black); w * h];
    let x_of_frame =
        |frame: u64| margin + ((frame.saturating_sub(1)) as usize * SAMPLES_PER_FRAME) / hop;

    // Waveform: min and max of every column, around a zero line.
    for x in 0..columns {
        img[(wave_y + wave_h / 2) * w + margin + x] = colour(Col::Gunmetal);
        let chunk = &samples[x * hop..((x + 1) * hop).min(samples.len())];
        let (lo, hi) = chunk
            .iter()
            .fold((0f32, 0f32), |(lo, hi), &s| (lo.min(s), hi.max(s)));
        let to_y = |s: f32| wave_y + ((1.0 - s) * 0.5 * (wave_h - 1) as f32).round() as usize;
        for y in to_y(hi)..=to_y(lo) {
            img[y * w + margin + x] = colour(Col::Chrome);
        }
    }

    // Spectrogram: Goertzel power on log-spaced bins, Hann window, relative to the loudest cell.
    let freqs: Vec<f32> = (0..BINS)
        .map(|k| LOW_HZ * (HIGH_HZ / LOW_HZ).powf(k as f32 / (BINS - 1) as f32))
        .collect();
    let hann: Vec<f32> = (0..WINDOW)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / WINDOW as f32).cos())
        .collect();
    let mut db = vec![f32::MIN; columns * BINS];
    for x in 0..columns {
        let centre = x * hop + hop / 2;
        let start = centre.saturating_sub(WINDOW / 2);
        let window: Vec<f32> = (0..WINDOW)
            .map(|i| samples.get(start + i).copied().unwrap_or(0.0) * hann[i])
            .collect();
        for (k, f) in freqs.iter().enumerate() {
            let coeff = 2.0 * (2.0 * std::f32::consts::PI * f / SAMPLE_RATE as f32).cos();
            let (mut s1, mut s2) = (0f32, 0f32);
            for &v in &window {
                let s0 = v + coeff * s1 - s2;
                s2 = s1;
                s1 = s0;
            }
            let power = s1 * s1 + s2 * s2 - coeff * s1 * s2;
            db[x * BINS + k] = 10.0 * (power.max(1e-12)).log10();
        }
    }
    let loudest = db.iter().copied().fold(f32::MIN, f32::max);
    for x in 0..columns {
        for k in 0..BINS {
            let level = ((db[x * BINS + k] - (loudest - 60.0)) / 60.0).clamp(0.0, 1.0);
            let c = colour(RAMP[(level * (RAMP.len() - 1) as f32).round() as usize]);
            for dy in 0..cell {
                img[(spec_y + (BINS - 1 - k) * cell + dy) * w + margin + x] = c;
            }
        }
    }

    // Frame numbers every half second.
    let frames = samples.len().div_ceil(SAMPLES_PER_FRAME) as u64;
    for f in (1..=frames).filter(|f| f % 30 == 0) {
        let x = x_of_frame(f);
        for y in wave_y - 3..wave_y {
            img[y * w + x] = colour(Col::Steel);
        }
        font::draw_into(
            &mut img,
            w,
            &f.to_string(),
            x,
            ticks_y,
            2,
            colour(Col::Steel),
        );
    }

    // Sound names where they start, in three staggered rows.
    for (i, (frame, name)) in events.iter().enumerate() {
        let x = x_of_frame(*frame);
        let row_y = margin + (i % label_rows) * row_h;
        for y in row_y + 11..wave_y + wave_h {
            if y % 2 == 0 {
                img[y * w + x] = colour(Col::Ember);
            }
        }
        font::draw_into(
            &mut img,
            w,
            &name.to_uppercase(),
            x + 2,
            row_y,
            2,
            colour(Col::Shine),
        );
    }

    let rgb = img
        .iter()
        .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
        .collect();
    (w, h, rgb)
}
