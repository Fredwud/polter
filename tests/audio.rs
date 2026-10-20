// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Sound tests: the synth must be deterministic, stop on time, and slide in the right direction.

use polter::audio::{SAMPLE_RATE, SAMPLES_PER_FRAME, Synth, wav};
use polter::{Cart, Ctx, Machine, Screen, Sfx, Wave};

const UP: Sfx = Sfx::new("up", Wave::Square, 200.0, 800.0, 20, 0.3);

fn render(sfx: Sfx, frames: usize) -> Vec<f32> {
    let mut synth = Synth::new();
    synth.trigger(sfx);
    let mut out = vec![0.0; frames * SAMPLES_PER_FRAME];
    synth.render(&mut out);
    out
}

fn zero_crossings(s: &[f32]) -> usize {
    s.windows(2)
        .filter(|w| (w[0] > 0.0) != (w[1] > 0.0))
        .count()
}

#[test]
fn the_synth_is_deterministic() {
    for wave in [Wave::Square, Wave::Triangle, Wave::Saw, Wave::Noise] {
        let sfx = Sfx { wave, ..UP };
        assert_eq!(
            render(sfx, 25),
            render(sfx, 25),
            "{wave:?} differs between two renders"
        );
    }
}

#[test]
fn a_sound_stops_after_its_length() {
    let s = render(UP, 30);
    let end = UP.frames as usize * SAMPLES_PER_FRAME;
    assert!(
        s[..end].iter().any(|v| v.abs() > 0.05),
        "the sound should be audible while it plays"
    );
    assert!(
        s[end..].iter().all(|v| *v == 0.0),
        "the sound should be silent after {} frames",
        UP.frames
    );
}

#[test]
fn a_rising_sound_rises_and_a_falling_one_falls() {
    // A 4x exponential slide doubles the pitch from one half to the next (measured: 97 vs 192
    // zero crossings), so 1.5x is a clear verdict on the direction without sitting on the edge.
    let half = UP.frames as usize * SAMPLES_PER_FRAME / 2;
    let up = render(UP, 20);
    let (a, b) = (zero_crossings(&up[..half]), zero_crossings(&up[half..]));
    assert!(
        b * 2 > a * 3,
        "pitch should rise: {a} then {b} zero crossings"
    );
    let down = render(
        Sfx {
            from: 800.0,
            to: 200.0,
            ..UP
        },
        20,
    );
    let (a, b) = (zero_crossings(&down[..half]), zero_crossings(&down[half..]));
    assert!(
        a * 2 > b * 3,
        "pitch should fall: {a} then {b} zero crossings"
    );
}

#[test]
fn the_wav_header_is_right() {
    let w = wav(&[0.0, 1.0, -1.0]);
    assert_eq!(&w[..4], b"RIFF");
    assert_eq!(&w[8..16], b"WAVEfmt ");
    assert_eq!(
        u32::from_le_bytes(w[24..28].try_into().unwrap()),
        SAMPLE_RATE
    );
    assert_eq!(
        u32::from_le_bytes(w[40..44].try_into().unwrap()),
        6,
        "three 16-bit samples"
    );
    assert_eq!(i16::from_le_bytes(w[46..48].try_into().unwrap()), 32_767);
}

struct Beeper;
impl Cart for Beeper {
    fn update(&mut self, ctx: &mut Ctx) {
        if ctx.frame == 2 {
            ctx.play(UP);
        }
    }
    fn draw(&self, _: &mut Screen, _: &Ctx) {}
}

#[test]
fn carts_play_sounds_through_the_machine() {
    let mut m = Machine::new(Box::new(Beeper), 1);
    m.step(0);
    assert!(
        m.sounds.is_empty() && m.audio.is_empty(),
        "no sound yet, and no audio unless enabled"
    );
    m.step(0);
    assert_eq!(m.sounds, [UP], "the sound started in frame 2 is reported");
    let mut loud = Machine::new(Box::new(Beeper), 1);
    loud.enable_audio();
    loud.step(0);
    loud.step(0);
    assert_eq!(loud.audio.len(), SAMPLES_PER_FRAME);
    assert!(
        loud.audio.iter().any(|v| v.abs() > 0.05),
        "enabled audio renders the sound"
    );
}

#[test]
fn held_sounds_keep_their_volume() {
    let fading = render(Sfx::new("fade", Wave::Square, 220.0, 220.0, 20, 0.3), 20);
    let held = render(
        Sfx::new("hold", Wave::Square, 220.0, 220.0, 20, 0.3).held(),
        20,
    );
    let peak = |s: &[f32]| s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let mid = held.len() / 2;
    let quarter = held.len() / 4;
    assert!(
        peak(&held[mid..mid + 400]) > 0.29,
        "a held sound is still loud halfway"
    );
    assert!(
        peak(&fading[mid..mid + 400]) < 0.17,
        "a plain sound has faded to half by then"
    );
    assert!(peak(&held[quarter..quarter + 400]) > 0.29);
}

/// The way to the sound card: the game delivers a frame of samples at uneven moments, the card
/// pulls at its own steady pace and rate. The signal must arrive without jumps (a jump is a
/// click), and after the start the buffer must not run dry.
#[test]
fn the_output_carries_sound_without_clicks_despite_an_uneven_frame_pace() {
    use polter::audio::{Resampler, SAMPLE_RATE, SAMPLES_PER_FRAME};
    let mut r = Resampler::new();
    let (rate, block) = (48_000u32, 512usize);
    let mut game_clock = 0.0f64;
    let mut card_clock = 0.0f64;
    let mut phase = 0.0f32;
    let mut jitter = 12345u32;
    let mut out: Vec<f32> = Vec::new();
    let mut frame = 0u64;
    // twenty seconds; the game runs at 59.6 fps on average, with jitter and two long hitches
    while card_clock < 20.0 {
        if game_clock <= card_clock {
            let mut samples = vec![0.0; SAMPLES_PER_FRAME];
            for s in samples.iter_mut() {
                phase += 220.0 / SAMPLE_RATE as f32;
                *s = 0.3 * polter::math::sin(phase * std::f32::consts::TAU);
            }
            r.push(&samples);
            frame += 1;
            jitter ^= jitter << 13;
            jitter ^= jitter >> 17;
            jitter ^= jitter << 5;
            let wobble = (jitter % 1000) as f64 / 1000.0 * 0.008 - 0.004;
            let hitch = if frame == 400 || frame == 900 {
                0.045
            } else {
                0.0
            };
            game_clock += 1.0 / 59.6 + wobble + hitch;
        } else {
            let mut buf = vec![0.0; block * 2];
            r.pull(&mut buf, 2, rate);
            out.extend(buf.iter().step_by(2));
            card_clock += block as f64 / rate as f64;
        }
    }
    // after the first half second: no jumps larger than a 220 Hz sine can make between samples
    let settled = &out[rate as usize / 2..];
    let max_step = settled
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max);
    let silent = settled.iter().filter(|v| v.abs() < 1e-4).count();
    assert!(
        max_step < 0.02,
        "a jump of {max_step} between two samples: that clicks"
    );
    assert!(
        r.starved < 2000,
        "the buffer ran dry for {} samples",
        r.starved
    );
    assert!(
        silent < settled.len() / 100,
        "{silent} silent samples in a steady tone"
    );
}

#[test]
fn noise_at_a_low_pitch_glides_instead_of_crackling() {
    let rumble = render(Sfx::new("rumble", Wave::Noise, 120.0, 120.0, 20, 0.3), 20);
    let jumps = rumble
        .windows(2)
        .filter(|w| (w[1] - w[0]).abs() > 0.05)
        .count();
    assert!(
        jumps < rumble.len() / 200,
        "{jumps} hard steps in a low rumble"
    );
}
