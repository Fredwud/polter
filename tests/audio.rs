// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Sound tests: the synth must be deterministic, stop on time, and slide in the right direction.

use polter::audio::{SAMPLE_RATE, SAMPLES_PER_FRAME, Synth, wav};
use polter::{Cart, Ctx, Machine, Screen, Sfx, Wave};

const UP: Sfx = Sfx::new("up", Wave::Square, 200.0, 800.0, 20, 0.3);

/// The left side of a sound effect (effects are heard in the middle: both sides alike).
fn render(sfx: Sfx, frames: usize) -> Vec<f32> {
    let mut synth = Synth::new();
    synth.trigger(sfx);
    let mut out = vec![0.0; frames * SAMPLES_PER_FRAME * 2];
    synth.render(&mut out);
    assert!(
        out.as_chunks::<2>().0.iter().all(|pair| pair[0] == pair[1]),
        "an effect sounds the same on both sides"
    );
    out.into_iter().step_by(2).collect()
}

/// The left and the right side of stereo samples.
fn sides(stereo: &[f32]) -> (Vec<f32>, Vec<f32>) {
    (
        stereo.iter().step_by(2).copied().collect(),
        stereo.iter().skip(1).step_by(2).copied().collect(),
    )
}

/// Renders a synth for some frames, both sides.
fn frames(synth: &mut Synth, n: usize) -> Vec<f32> {
    let mut out = vec![0.0; n * SAMPLES_PER_FRAME * 2];
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
    assert_eq!(loud.audio.len(), SAMPLES_PER_FRAME * 2, "stereo samples");
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
            let mut samples = vec![0.0; SAMPLES_PER_FRAME * 2];
            for pair in samples.as_chunks_mut::<2>().0 {
                phase += 220.0 / SAMPLE_RATE as f32;
                pair.fill(0.3 * polter::math::sin(phase * std::f32::consts::TAU));
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

// ---- recordings ----

use polter::audio::{OPEN, Play};
use polter::{Game, Sample, Session};

/// A WAV file's bytes from its parts, for the formats `wav` does not write.
fn wav_bytes(format: u16, channels: u16, rate: u32, bits: u16, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&format.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    let block = channels as u32 * bits as u32 / 8;
    out.extend_from_slice(&(rate * block).to_le_bytes());
    out.extend_from_slice(&(block as u16).to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    out
}

fn sine(freq: f32, seconds: f32, volume: f32) -> Vec<f32> {
    let n = (seconds * SAMPLE_RATE as f32) as usize;
    (0..n)
        .map(|i| {
            volume * polter::math::sin(i as f32 / SAMPLE_RATE as f32 * freq * std::f32::consts::TAU)
        })
        .collect()
}

/// The largest jump between two neighbouring samples: a click shows as a jump.
fn largest_step(s: &[f32]) -> f32 {
    s.windows(2).fold(0.0f32, |m, w| m.max((w[1] - w[0]).abs()))
}

#[test]
fn a_wav_loads_in_every_common_format() {
    let want = [0.0f32, 0.5, -0.5, 0.25];
    let sixteen = Sample::from_wav("16", &wav(&want)).expect("16-bit mono");
    for (i, w) in want.iter().enumerate() {
        let got = sixteen_at(&sixteen, i);
        assert!((got - w).abs() < 1e-3, "16-bit sample {i}: {got} for {w}");
    }
    // 24-bit stereo stays stereo; its value is the middle of left and right: left 0.5, right
    // -0.5 gives 0; left and right 0.5 give 0.5
    let mut data = Vec::new();
    for (l, r) in [(0.5f32, -0.5f32), (0.5, 0.5)] {
        for v in [l, r] {
            let i = (v * 8_388_607.0) as i32;
            data.extend_from_slice(&i.to_le_bytes()[..3]);
        }
    }
    let stereo = Sample::from_wav("24", &wav_bytes(1, 2, SAMPLE_RATE, 24, &data)).expect("24-bit");
    assert_eq!((stereo.len(), stereo.channels()), (2, 2));
    assert!(sixteen_at(&stereo, 0).abs() < 1e-3 && (sixteen_at(&stereo, 1) - 0.5).abs() < 1e-3);
    // 32-bit float and 8-bit
    let floats: Vec<u8> = [0.75f32, -0.25]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let float = Sample::from_wav("f", &wav_bytes(3, 1, SAMPLE_RATE, 32, &floats)).expect("float");
    assert!((sixteen_at(&float, 0) - 0.75).abs() < 1e-6);
    let eight =
        Sample::from_wav("8", &wav_bytes(1, 1, SAMPLE_RATE, 8, &[128, 192])).expect("8-bit");
    assert!(sixteen_at(&eight, 0).abs() < 1e-6 && (sixteen_at(&eight, 1) - 0.5).abs() < 1e-6);
}

/// One of a recording's values.
fn sixteen_at(s: &Sample, i: usize) -> f32 {
    s.value(i)
}

#[test]
fn a_wav_at_another_rate_is_brought_to_the_engines() {
    let half: Vec<u8> = (0..1000)
        .flat_map(|i| ((i as f32 / 1000.0 * 16_000.0) as i16).to_le_bytes())
        .collect();
    let s = Sample::from_wav("slow", &wav_bytes(1, 1, SAMPLE_RATE / 2, 16, &half)).expect("loads");
    assert!(
        (s.len() as i32 - 1999).abs() <= 1,
        "twice the samples: {}",
        s.len()
    );
    assert!((s.seconds() - 1000.0 / 22_050.0).abs() < 0.001);
}

#[test]
fn a_file_that_is_no_wav_is_refused_with_a_reason() {
    let err = Sample::from_wav("junk", b"definitely not a wav file").unwrap_err();
    assert!(err.contains("WAV"), "{err}");
    let odd = wav_bytes(2, 1, SAMPLE_RATE, 4, &[0, 0]);
    assert!(
        Sample::from_wav("adpcm", &odd)
            .unwrap_err()
            .contains("format 2")
    );
}

#[test]
fn a_recording_plays_once_at_its_volume_then_falls_silent() {
    // 110.3 periods: it ends in the middle of a wave, where a cut would click
    let tone = Sample::new("tone", sine(440.0, 0.2507, 0.8));
    let mut synth = Synth::new();
    synth.start(Play::new(&tone, 0.5));
    let mut both = vec![0.0; SAMPLE_RATE as usize];
    synth.render(&mut both);
    let (out, right) = sides(&both);
    assert_eq!(out, right, "in the middle, both sides alike");
    let peak = |s: &[f32]| s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(
        (peak(&out[1000..5000]) - 0.4).abs() < 0.02,
        "0.8 at half volume: {}",
        peak(&out[1000..5000])
    );
    assert!(
        out[tone.len()..].iter().all(|v| *v == 0.0),
        "silent after its end"
    );
    assert!(
        largest_step(&out) < 0.06,
        "no click at its start or end: {}",
        largest_step(&out)
    );
}

#[test]
fn a_hum_glides_in_holds_and_fades_out_without_a_click() {
    let tone = Sample::new("hum", sine(220.0, 0.5, 0.6));
    let mut synth = Synth::new();
    let mut all = Vec::new();
    for frame in 0..90 {
        let asked = if frame < 50 {
            vec![("motor", Play::new(&tone, 0.5))]
        } else {
            Vec::new()
        };
        synth.hold(&asked);
        all.extend(sides(&frames(&mut synth, 1)).0);
    }
    let peak = |s: &[f32]| s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let f = SAMPLES_PER_FRAME;
    assert!(
        peak(&all[..f / 4]) < 0.15,
        "it fades in: {}",
        peak(&all[..f / 4])
    );
    assert!(
        (peak(&all[30 * f..40 * f]) - 0.3).abs() < 0.02,
        "it holds 0.6 at half volume"
    );
    assert!(
        peak(&all[80 * f..]) < 0.01,
        "gone within half a second of the last call"
    );
    assert!(
        largest_step(&all) < 0.03,
        "never a click: {}",
        largest_step(&all)
    );
}

#[test]
fn a_loop_comes_round_without_a_click_at_its_seam() {
    // 0.3 s of 233 Hz does not end where it begins: looped raw, the seam would jump
    let tone = Sample::new("seam", sine(233.0, 0.3, 0.8));
    let mut synth = Synth::new();
    let mut all = Vec::new();
    for _ in 0..80 {
        synth.hold(&[("drone", Play::new(&tone, 1.0))]);
        all.extend(sides(&frames(&mut synth, 1)).0);
    }
    // three times round, and no step larger than the tone's own
    let own = 0.8 * 233.0 * std::f32::consts::TAU / SAMPLE_RATE as f32;
    assert!(
        largest_step(&all[2000..]) < own * 1.6,
        "{} against {own}",
        largest_step(&all[2000..])
    );
}

struct Player;

impl Game for Player {
    fn update(&mut self, ctx: &mut Ctx) {
        thread_local! {
            static CLICK: Sample = Sample::new("click", sine(1000.0, 0.05, 0.5));
            static HUM: Sample = Sample::new("engine", sine(110.0, 0.4, 0.5));
        }
        if ctx.frame == 2 {
            CLICK.with(|s| ctx.play_sample(s, 0.8));
        }
        HUM.with(|s| ctx.hum("engine", s, 0.4, 1.0));
    }
    fn draw(&self, _: &mut polter::Canvas, _: &Ctx) {}
}

#[test]
fn games_play_recordings_and_hums_and_the_tools_hear_their_names() {
    let mut s = Session::new(Box::new(Player), 1);
    s.step(0);
    assert_eq!(
        s.sound_names(),
        ["engine"],
        "the hum begins on the first tick"
    );
    s.step(0);
    assert_eq!(
        s.sound_names(),
        ["click"],
        "the click in the second; the hum goes on unnamed"
    );
    let mut loud = Session::new(Box::new(Player), 1);
    loud.enable_audio();
    for _ in 0..10 {
        loud.step(0);
    }
    assert!(
        loud.audio.iter().any(|v| v.abs() > 0.1),
        "enabled audio carries the hum"
    );
}

fn rms(s: &[f32]) -> f32 {
    (s.iter().map(|v| v * v).sum::<f32>() / s.len().max(1) as f32).sqrt()
}

#[test]
fn a_recording_is_heard_where_it_is_placed_and_as_loud_wherever_it_stands() {
    let tone = Sample::new("tone", sine(330.0, 0.5, 0.4));
    let heard = |pan: f32| {
        let mut synth = Synth::new();
        let mut play = Play::new(&tone, 0.5);
        play.pan(pan);
        synth.start(play);
        let (l, r) = sides(&frames(&mut synth, 20));
        (rms(&l[2000..14000]), rms(&r[2000..14000]))
    };
    let (ml, mr) = heard(0.0);
    assert!((ml - mr).abs() < 1e-6, "in the middle: {ml} and {mr}");
    let (ll, lr) = heard(-0.8);
    assert!(ll > 5.0 * lr, "left: {ll} against {lr} on the right");
    let (rl, rr) = heard(1.0);
    assert!(rl < 1e-4 && rr > 0.1, "hard right: {rl} left, {rr} right");
    for (pan, (l, r)) in [
        (0.0, (ml, mr)),
        (-0.8, (ll, lr)),
        (1.0, (rl, rr)),
        (0.4, heard(0.4)),
    ] {
        let power = l * l + r * r;
        let middle = ml * ml + mr * mr;
        assert!(
            (power / middle - 1.0).abs() < 0.02,
            "equal power at {pan}: {power} against {middle} in the middle"
        );
    }
}

#[test]
fn a_cutoff_dulls_the_high_pitches_and_keeps_the_low_ones() {
    let kept = |freq: f32, cutoff: f32| {
        let tone = Sample::new("tone", sine(freq, 0.5, 0.4));
        let mut synth = Synth::new();
        let mut play = Play::new(&tone, 1.0);
        play.cutoff(cutoff);
        synth.start(play);
        let (l, _) = sides(&frames(&mut synth, 20));
        rms(&l[4000..14000]) / (0.4 / std::f32::consts::SQRT_2)
    };
    assert!(kept(5000.0, polter::audio::OPEN) > 0.97, "open keeps all");
    let high = kept(5000.0, 500.0);
    assert!(
        high < 0.03,
        "5 kHz through a 500 Hz cutoff: {high} of it is left"
    );
    let low = kept(100.0, 500.0);
    assert!(
        low > 0.9,
        "100 Hz through a 500 Hz cutoff: {low} of it is left"
    );
}

#[test]
fn a_stereo_recording_keeps_its_left_and_right() {
    // left a 0.5 tone, right silent: as a WAV, loaded and played in the middle
    let tone = sine(440.0, 0.2, 0.5);
    let mut data = Vec::new();
    for v in &tone {
        data.extend_from_slice(&((v * 32_767.0) as i16).to_le_bytes());
        data.extend_from_slice(&0i16.to_le_bytes());
    }
    let s = Sample::from_wav("left", &wav_bytes(1, 2, SAMPLE_RATE, 16, &data)).expect("stereo");
    assert_eq!(s.channels(), 2);
    let mut synth = Synth::new();
    synth.start(Play::new(&s, 1.0));
    let (l, r) = sides(&frames(&mut synth, 10));
    assert!(rms(&l[1000..7000]) > 0.3, "the left side sounds");
    assert!(rms(&r) < 1e-6, "the right side stays silent: {}", rms(&r));
    // a stereo WAV of the engine's own writing reads back as it was
    let back = Sample::from_wav("back", &polter::audio::stereo_wav(&[0.5, -0.5, 0.25, 0.0]))
        .expect("loads");
    assert_eq!((back.len(), back.channels()), (2, 2));
}

#[test]
fn a_hum_moving_from_one_side_to_the_other_glides_without_a_click() {
    let tone = Sample::new("hum", sine(220.0, 0.5, 0.6));
    let mut synth = Synth::new();
    let mut all = Vec::new();
    for frame in 0..60 {
        let mut play = Play::new(&tone, 0.6);
        play.pan(if frame < 30 { -1.0 } else { 1.0 })
            .cutoff(if frame < 30 { OPEN } else { 300.0 });
        synth.hold(&[("motor", play)]);
        all.extend(frames(&mut synth, 1));
    }
    let (l, r) = sides(&all);
    let f = SAMPLES_PER_FRAME;
    assert!(
        rms(&l[20 * f..29 * f]) > 0.2 && rms(&r[20 * f..29 * f]) < 0.01,
        "first on the left"
    );
    assert!(
        rms(&r[50 * f..]) > 3.0 * rms(&l[50 * f..]),
        "then on the right"
    );
    assert!(
        largest_step(&l) < 0.03 && largest_step(&r) < 0.03,
        "no click on the way: {} and {}",
        largest_step(&l),
        largest_step(&r)
    );
}
