// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Sound: a tiny deterministic synth, and recordings. Carts describe sound effects or play
//! recordings (once, or looped for as long as they ask), each at its place between left and
//! right, Polter mixes them into stereo samples, the same way in a window and headless, so agents
//! can check sound with `listen`.

/// Samples per second.
pub const SAMPLE_RATE: u32 = 44_100;
/// Samples rendered per frame (44 100 / 60).
pub const SAMPLES_PER_FRAME: usize = (SAMPLE_RATE / crate::machine::FPS) as usize;
const VOICES: usize = 8;
/// Recordings playing once at the same time, and recordings held in a loop.
const PLAYERS: usize = 16;
const LOOPS: usize = 16;
/// How far a held recording's volume and speed move towards what is asked, per sample: about
/// 30 ms to get most of the way, so a change never clicks.
const GLIDE: f32 = 1.0 / (0.03 * SAMPLE_RATE as f32);
/// A loop's seam: its last samples are faded into its first ones over this many (about 23 ms),
/// so a recording that does not end as it begins loops without a click.
const SEAM: usize = 1024;
/// A recording played once fades in over its first samples and out over its last ones.
const EDGE_IN: f64 = 64.0;
const EDGE_OUT: f64 = 256.0;
/// A `Play::cutoff` at or above this keeps every pitch: the recording sounds as it was made.
pub const OPEN: f32 = 20_000.0;

/// The shape of a tone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wave {
    /// Hollow and loud, the classic chip sound.
    Square,
    /// Soft and round.
    Triangle,
    /// Buzzy, good for creaks and engines.
    Saw,
    /// Hiss and rumble; the pitch sets how fast the noise changes.
    Noise,
}

/// A sound effect: a tone that slides from one pitch to another while it fades out.
///
/// ```
/// use polter::{Sfx, Wave};
/// const JUMP: Sfx = Sfx::new("jump", Wave::Square, 260.0, 520.0, 7, 0.25);
/// assert_eq!(JUMP.frames, 7);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sfx {
    /// Shown by `check` and `listen`, so agents can tell sounds apart.
    pub name: &'static str,
    pub wave: Wave,
    /// Start pitch in Hz.
    pub from: f32,
    /// End pitch in Hz.
    pub to: f32,
    /// Length in frames (1/60 s).
    pub frames: u16,
    /// 0.0 to 1.0. Several sounds at full volume would clip, so stay around 0.15 to 0.35.
    pub volume: f32,
    /// Holds its volume and fades only at the end (drones, wind, whistles) instead of fading
    /// out from the start.
    pub hold: bool,
    /// Pitch wobble, as a fraction of the pitch (0.02 is a gentle vibrato), six times a second.
    pub vibrato: f32,
}

impl Sfx {
    pub const fn new(
        name: &'static str,
        wave: Wave,
        from: f32,
        to: f32,
        frames: u16,
        volume: f32,
    ) -> Sfx {
        Sfx {
            name,
            wave,
            from,
            to,
            frames,
            volume,
            hold: false,
            vibrato: 0.0,
        }
    }

    /// The same sound, held at full volume until it fades at the end.
    ///
    /// ```
    /// use polter::{Sfx, Wave};
    /// const DRONE: Sfx = Sfx::new("drone", Wave::Triangle, 55.0, 55.0, 240, 0.1).held();
    /// assert!(DRONE.hold);
    /// ```
    pub const fn held(mut self) -> Sfx {
        self.hold = true;
        self
    }

    /// The same sound with a vibrato of `depth` (a fraction of the pitch).
    pub const fn vibrato(mut self, depth: f32) -> Sfx {
        self.vibrato = depth;
        self
    }

    fn samples(&self) -> u32 {
        self.frames as u32 * SAMPLES_PER_FRAME as u32
    }
}

/// A recording, ready to play: mono or stereo samples at `SAMPLE_RATE`, shared, so copies cost
/// nothing. Load it from a WAV file's bytes, for instance a file in the game's folder taken in
/// with `include_bytes!`:
///
/// ```
/// use polter::audio::{wav, Sample};
/// let bytes = wav(&[0.0, 0.5, -0.5, 0.25]); // any WAV: 8, 16, 24 or 32 bits, or float
/// let tick = Sample::from_wav("tick", &bytes).unwrap();
/// assert_eq!(tick.len(), 4);
/// ```
#[derive(Clone, Debug)]
pub struct Sample {
    /// Shown by `check` and `listen`, as an `Sfx`'s name is.
    pub name: &'static str,
    /// The values; in stereo, left and right in turn.
    data: std::sync::Arc<[f32]>,
    stereo: bool,
}

/// Values held to -1.0..1.0, and anything not a number made silent.
fn clean(samples: Vec<f32>) -> std::sync::Arc<[f32]> {
    samples
        .into_iter()
        .map(|v| {
            if v.is_finite() {
                v.clamp(-1.0, 1.0)
            } else {
                0.0
            }
        })
        .collect()
}

impl Sample {
    /// From samples already at `SAMPLE_RATE`, mono, -1.0 to 1.0 (louder ones are held there).
    pub fn new(name: &'static str, samples: Vec<f32>) -> Sample {
        Sample {
            name,
            data: clean(samples),
            stereo: false,
        }
    }

    /// From stereo samples already at `SAMPLE_RATE`, left and right in turn (a last value
    /// without its partner is dropped).
    pub fn stereo(name: &'static str, mut samples: Vec<f32>) -> Sample {
        samples.truncate(samples.len() / 2 * 2);
        Sample {
            name,
            data: clean(samples),
            stereo: true,
        }
    }

    /// From a WAV file: PCM of 8, 16, 24 or 32 bits or 32-bit float (also in the extensible
    /// format), mono or stereo (more channels are mixed down to one), any rate (brought to
    /// `SAMPLE_RATE` once, here, along straight lines between the samples).
    pub fn from_wav(name: &'static str, bytes: &[u8]) -> Result<Sample, String> {
        let (rate, channels, values) = read_wav(bytes)?;
        let frames = values.len() / channels;
        let values = if rate == SAMPLE_RATE || frames < 2 {
            values
        } else {
            let step = rate as f64 / SAMPLE_RATE as f64;
            let n = ((frames - 1) as f64 / step).floor() as usize + 1;
            let mut out = Vec::with_capacity(n * channels);
            for j in 0..n {
                let pos = j as f64 * step;
                let i = pos as usize;
                let t = (pos - i as f64) as f32;
                let next = (i + 1).min(frames - 1);
                for c in 0..channels {
                    let (a, b) = (values[i * channels + c], values[next * channels + c]);
                    out.push(a + (b - a) * t);
                }
            }
            out
        };
        Ok(if channels == 2 {
            Sample::stereo(name, values)
        } else {
            Sample::new(name, values)
        })
    }

    /// Its length in samples (in stereo, pairs of them).
    pub fn len(&self) -> usize {
        self.data.len() / self.channels()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// 1 for mono, 2 for stereo.
    pub fn channels(&self) -> usize {
        if self.stereo { 2 } else { 1 }
    }

    /// How long it plays at its own speed.
    pub fn seconds(&self) -> f32 {
        self.len() as f32 / SAMPLE_RATE as f32
    }

    /// One of its values (in stereo, the middle of left and right; 0.0 past its end).
    pub fn value(&self, i: usize) -> f32 {
        self.frame(i).map_or(0.0, |(l, r)| (l + r) * 0.5)
    }

    /// Its values at sample `i`, left and right (the same twice in mono).
    fn frame(&self, i: usize) -> Option<(f32, f32)> {
        if self.stereo {
            Some((*self.data.get(2 * i)?, *self.data.get(2 * i + 1)?))
        } else {
            self.data.get(i).map(|&v| (v, v))
        }
    }

    /// The values between two samples, along the straight line; silence outside.
    fn at(&self, pos: f64) -> (f32, f32) {
        if pos < 0.0 {
            return (0.0, 0.0);
        }
        let i = pos as usize;
        let Some((al, ar)) = self.frame(i) else {
            return (0.0, 0.0);
        };
        let (bl, br) = self.frame(i + 1).unwrap_or((0.0, 0.0));
        let t = (pos - i as f64) as f32;
        (al + (bl - al) * t, ar + (br - ar) * t)
    }

    /// The seam's length for this recording, and the loop's length without it.
    fn seam(&self) -> (usize, f64) {
        let seam = SEAM.min(self.len() / 4);
        (seam, (self.len() - seam) as f64)
    }

    /// Its values at `pos` in a loop: once it has come round, the start is faded in from the end.
    fn looped(&self, pos: f64, wrapped: bool) -> (f32, f32) {
        let (seam, len) = self.seam();
        let (l, r) = self.at(pos);
        if wrapped && seam > 0 && pos < seam as f64 {
            let w = (pos / seam as f64) as f32;
            let (el, er) = self.at(pos + len);
            (l * w + el * (1.0 - w), r * w + er * (1.0 - w))
        } else {
            (l, r)
        }
    }
}

/// A recording to start or keep sounding: how loud (0.0 to 1.0), how fast (1.0 as recorded; 2.0
/// twice as fast and an octave higher, 0.5 half as fast and an octave lower), where it is heard
/// and how dull. `Ctx::play_sample` and `Ctx::hum` hand it back, so a game can place it there:
///
/// ```ignore
/// ctx.hum("motors", &MOTORS, 0.5, 1.0).pan(0.6).cutoff(900.0); // aft, behind a bulkhead
/// ```
#[derive(Clone, Debug)]
pub struct Play {
    pub sample: Sample,
    pub volume: f32,
    pub rate: f32,
    /// Where it is heard: -1.0 left, 0.0 in the middle, 1.0 right. Equal power: as loud
    /// wherever it stands, so a sound passing from one side to the other does not dip. A stereo
    /// recording keeps its own left and right, shifted this way.
    pub pan: f32,
    /// The highest pitch it keeps, in Hz, as through a wall or a closed door (`OPEN` keeps all).
    pub cutoff: f32,
}

impl Play {
    /// As recorded, in the middle, at a volume.
    pub fn new(sample: &Sample, volume: f32) -> Play {
        Play {
            sample: sample.clone(),
            volume,
            rate: 1.0,
            pan: 0.0,
            cutoff: OPEN,
        }
    }

    pub fn rate(&mut self, rate: f32) -> &mut Play {
        self.rate = rate;
        self
    }

    pub fn pan(&mut self, pan: f32) -> &mut Play {
        self.pan = pan;
        self
    }

    pub fn cutoff(&mut self, hz: f32) -> &mut Play {
        self.cutoff = hz;
        self
    }

    /// The gains for left and right: equal power, 1.0 each in the middle.
    fn gains(&self) -> (f32, f32) {
        if self.pan == 0.0 {
            return (1.0, 1.0);
        }
        let a = (self.pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
        let (sin, cos) = crate::math::sin_cos(a);
        (
            cos * std::f32::consts::SQRT_2,
            sin * std::f32::consts::SQRT_2,
        )
    }

    /// The low-pass filter's coefficient for its cutoff: 1.0 lets everything through.
    fn dullness(&self) -> f32 {
        if self.cutoff >= OPEN {
            1.0
        } else {
            1.0 - crate::math::exp(
                -std::f32::consts::TAU * self.cutoff.max(20.0) / SAMPLE_RATE as f32,
            )
        }
    }
}

/// Two gentle low-passes in a row for each side (12 dB less per octave above the cutoff), with
/// their coefficient: what a wall leaves of a sound.
#[derive(Clone, Copy, Debug)]
struct Dull {
    k: f32,
    left: [f32; 2],
    right: [f32; 2],
}

impl Dull {
    fn new(k: f32) -> Dull {
        Dull {
            k,
            left: [0.0; 2],
            right: [0.0; 2],
        }
    }

    fn pass(&mut self, (l, r): (f32, f32)) -> (f32, f32) {
        if self.k >= 1.0 {
            self.left = [l; 2];
            self.right = [r; 2];
            return (l, r);
        }
        for (s, v) in [(&mut self.left, l), (&mut self.right, r)] {
            s[0] += (v - s[0]) * self.k;
            s[1] += (s[0] - s[1]) * self.k;
        }
        (self.left[1], self.right[1])
    }
}

/// A WAV file's rate, its channels (1 or 2) and its values, in stereo left and right in turn;
/// more than two channels are mixed down to one.
fn read_wav(bytes: &[u8]) -> Result<(u32, usize, Vec<f32>), String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAV file (no RIFF/WAVE header)".into());
    }
    let u16_at = |b: &[u8], i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |b: &[u8], i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let (mut fmt, mut data) = (None, None);
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let size = u32_at(bytes, pos + 4) as usize;
        let body = &bytes[pos + 8..(pos + 8).saturating_add(size).min(bytes.len())];
        match &bytes[pos..pos + 4] {
            b"fmt " => fmt = Some(body),
            b"data" => data = Some(body),
            _ => {}
        }
        // chunks are padded to an even length
        pos = pos.saturating_add(8 + size + (size & 1));
    }
    let fmt = fmt.ok_or("no fmt chunk")?;
    let data = data.ok_or("no data chunk")?;
    if fmt.len() < 16 {
        return Err("the fmt chunk is too short".into());
    }
    let mut format = u16_at(fmt, 0);
    let channels = u16_at(fmt, 2).max(1) as usize;
    let rate = u32_at(fmt, 4);
    let bits = u16_at(fmt, 14);
    if format == 0xfffe && fmt.len() >= 26 {
        format = u16_at(fmt, 24); // the extensible format names its subformat here
    }
    let width = match (format, bits) {
        (1, 8) => 1,
        (1, 16) => 2,
        (1, 24) => 3,
        (1, 32) | (3, 32) => 4,
        _ => {
            return Err(format!(
                "format {format} with {bits} bits: only PCM of 8, 16, 24 or 32 bits and 32-bit float"
            ));
        }
    };
    if rate == 0 {
        return Err("a rate of 0".into());
    }
    let read = |i: usize| -> f32 {
        let b = &data[i..i + width];
        match (format, bits) {
            (1, 8) => (b[0] as f32 - 128.0) / 128.0,
            (1, 16) => i16::from_le_bytes([b[0], b[1]]) as f32 / 32_768.0,
            (1, 24) => (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0,
            (1, _) => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
            _ => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        }
    };
    let frame = channels * width;
    let frames = data.len() / frame;
    if channels == 2 {
        let values = (0..frames * 2).map(|k| read(k * width)).collect();
        return Ok((rate, 2, values));
    }
    let mono = (0..frames)
        .map(|k| {
            (0..channels)
                .map(|c| read(k * frame + c * width))
                .sum::<f32>()
                / channels as f32
        })
        .collect();
    Ok((rate, 1, mono))
}

/// A recording playing once.
#[derive(Clone)]
struct Player {
    play: Play,
    pos: f64,
    gains: (f32, f32),
    dull: Dull,
}

/// A recording held in a loop while the game asks for it.
#[derive(Clone)]
struct Held {
    key: &'static str,
    sample: Sample,
    pos: f64,
    wrapped: bool,
    volume: f32,
    rate: f32,
    gains: (f32, f32),
    dull: Dull,
    goal_volume: f32,
    goal_rate: f32,
    goal_gains: (f32, f32),
    goal_dull: f32,
    asked: bool,
}

/// One step of a glide from `now` towards `goal`; close enough, it arrives.
fn glide(now: f32, goal: f32) -> f32 {
    if (goal - now).abs() < 1e-5 {
        goal
    } else {
        now + (goal - now) * GLIDE
    }
}

#[derive(Clone, Copy)]
struct Voice {
    sfx: Sfx,
    pos: u32,
    phase: f32,
    /// Noise: the random value at the start and at the end of the current step.
    noise_a: f32,
    noise_b: f32,
    /// A gentle low-pass per voice: takes the sharpest edges off squares, saws and noise.
    smooth: f32,
}

impl Voice {
    /// Where the envelope stands (0..1), for choosing which voice to give up.
    fn loudness(&self) -> f32 {
        let t = self.pos as f32 / self.sfx.samples().max(1) as f32;
        let fade = if self.sfx.hold {
            ((1.0 - t) / 0.15).min(1.0)
        } else {
            1.0 - t
        };
        fade * self.sfx.volume
    }
}

/// Low-pass coefficient for about 7 kHz at 44.1 kHz.
const SMOOTH: f32 = 0.63;

/// Mixes up to eight sounds at once. When all voices are busy, the quietest sound gives way
/// (cutting off a loud one would click).
#[derive(Clone)]
pub struct Synth {
    voices: [Option<Voice>; VOICES],
    rng: u32,
    triggered: u64,
    players: Vec<Player>,
    held: Vec<Held>,
}

impl Default for Synth {
    fn default() -> Self {
        Self::new()
    }
}

impl Synth {
    pub fn new() -> Synth {
        Synth {
            voices: [None; VOICES],
            rng: 0x9e37_79b9,
            triggered: 0,
            players: Vec::new(),
            held: Vec::new(),
        }
    }

    /// Starts a recording once. When sixteen are playing, the quietest gives way.
    pub fn start(&mut self, play: Play) {
        if play.sample.is_empty() {
            return;
        }
        if self.players.len() >= PLAYERS
            && let Some(quiet) = (0..self.players.len()).min_by(|&a, &b| {
                self.players[a]
                    .play
                    .volume
                    .total_cmp(&self.players[b].play.volume)
            })
        {
            self.players.remove(quiet);
        }
        let (gains, dull) = (play.gains(), Dull::new(play.dullness()));
        self.players.push(Player {
            play,
            pos: 0.0,
            gains,
            dull,
        });
    }

    /// The recordings asked to keep sounding in this tick, by key: a new one starts and fades in,
    /// a known one glides to its new volume, speed, place and dullness, one not asked for fades
    /// out.
    pub fn hold(&mut self, hums: &[(&'static str, Play)]) {
        for h in &mut self.held {
            h.asked = false;
        }
        for (key, play) in hums {
            if let Some(h) = self.held.iter_mut().find(|h| h.key == *key) {
                if !std::sync::Arc::ptr_eq(&h.sample.data, &play.sample.data) {
                    h.sample = play.sample.clone();
                    h.pos = 0.0;
                    h.wrapped = false;
                }
                h.asked = true;
                h.goal_volume = play.volume;
                h.goal_rate = play.rate;
                h.goal_gains = play.gains();
                h.goal_dull = play.dullness();
            } else if self.held.len() < LOOPS && play.sample.len() > 1 {
                self.held.push(Held {
                    key,
                    sample: play.sample.clone(),
                    pos: 0.0,
                    wrapped: false,
                    volume: 0.0,
                    rate: play.rate,
                    gains: play.gains(),
                    dull: Dull::new(play.dullness()),
                    goal_volume: play.volume,
                    goal_rate: play.rate,
                    goal_gains: play.gains(),
                    goal_dull: play.dullness(),
                    asked: true,
                });
            }
        }
    }

    pub fn trigger(&mut self, sfx: Sfx) {
        self.triggered += 1;
        let voice = Voice {
            sfx,
            pos: 0,
            phase: 0.0,
            noise_a: 0.0,
            noise_b: self.random(),
            smooth: 0.0,
        };
        let slot = self
            .voices
            .iter()
            .position(Option::is_none)
            .unwrap_or_else(|| {
                (0..VOICES)
                    .min_by(|&a, &b| {
                        let la = self.voices[a].map_or(0.0, |v| v.loudness());
                        let lb = self.voices[b].map_or(0.0, |v| v.loudness());
                        la.total_cmp(&lb)
                    })
                    .unwrap_or(0)
            });
        self.voices[slot] = Some(voice);
    }

    /// A random value in -1..1 (xorshift; deterministic, so headless runs hear the same).
    fn random(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x as i32) as f32 / i32::MAX as f32
    }

    /// Fills `out` with the next samples in stereo, left and right in turn, mixed and gently
    /// limited to -1.0..1.0. Sound effects are heard in the middle.
    pub fn render(&mut self, out: &mut [f32]) {
        let attack = SAMPLE_RATE as f32 * 0.002; // 2 ms, so tones don't click on
        for pair in out.as_chunks_mut::<2>().0 {
            let mut mix = 0.0;
            for k in 0..VOICES {
                let Some(mut v) = self.voices[k] else {
                    continue;
                };
                let len = v.sfx.samples();
                if v.pos >= len {
                    self.voices[k] = None;
                    continue;
                }
                let t = v.pos as f32 / len as f32;
                let mut freq = v.sfx.from * crate::math::pow(v.sfx.to / v.sfx.from, t);
                if v.sfx.vibrato != 0.0 {
                    let secs = v.pos as f32 / SAMPLE_RATE as f32;
                    freq *=
                        1.0 + v.sfx.vibrato * crate::math::sin(secs * 6.0 * std::f32::consts::TAU);
                }
                v.phase += freq / SAMPLE_RATE as f32;
                if v.phase >= 1.0 {
                    v.phase -= 1.0;
                    v.noise_a = v.noise_b;
                    v.noise_b = self.random();
                }
                let raw = match v.sfx.wave {
                    Wave::Square => {
                        if v.phase < 0.5 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                    Wave::Triangle => 4.0 * (v.phase - 0.5).abs() - 1.0,
                    Wave::Saw => 2.0 * v.phase - 1.0,
                    // noise glides from one random value to the next: a low pitch rumbles
                    // instead of crackling
                    Wave::Noise => v.noise_a + (v.noise_b - v.noise_a) * v.phase,
                };
                v.smooth += (raw - v.smooth) * SMOOTH;
                let fade = if v.sfx.hold {
                    ((1.0 - t) / 0.15).min(1.0)
                } else {
                    1.0 - t
                };
                let envelope = (v.pos as f32 / attack).min(1.0) * fade;
                mix += v.smooth * v.sfx.volume * envelope;
                v.pos += 1;
                self.voices[k] = Some(v);
            }
            let (mut left, mut right) = (mix, mix);
            for p in &mut self.players {
                let len = p.play.sample.len() as f64;
                let edge = (p.pos / EDGE_IN)
                    .min((len - p.pos) / EDGE_OUT)
                    .clamp(0.0, 1.0) as f32;
                let (l, r) = p.dull.pass(p.play.sample.at(p.pos));
                let k = p.play.volume * edge;
                left += l * p.gains.0 * k;
                right += r * p.gains.1 * k;
                p.pos += p.play.rate.max(0.0) as f64;
            }
            for h in &mut self.held {
                let goal = if h.asked { h.goal_volume } else { 0.0 };
                h.volume = glide(h.volume, goal);
                h.rate = glide(h.rate, h.goal_rate);
                h.gains = (
                    glide(h.gains.0, h.goal_gains.0),
                    glide(h.gains.1, h.goal_gains.1),
                );
                h.dull.k = glide(h.dull.k, h.goal_dull);
                let (l, r) = h.dull.pass(h.sample.looped(h.pos, h.wrapped));
                left += l * h.gains.0 * h.volume;
                right += r * h.gains.1 * h.volume;
                h.pos += h.rate.max(0.0) as f64;
                let (_, len) = h.sample.seam();
                if h.pos >= len {
                    h.pos -= len;
                    h.wrapped = true;
                }
            }
            pair[0] = limit(left);
            pair[1] = limit(right);
        }
        self.players
            .retain(|p| p.pos < p.play.sample.len() as f64 && p.play.rate > 0.0);
        self.held.retain(|h| h.asked || h.volume > 1e-4);
    }
}

/// Leaves quiet signals alone and rounds off loud ones instead of cutting them flat.
fn limit(x: f32) -> f32 {
    const KNEE: f32 = 0.6;
    let a = x.abs();
    if a <= KNEE {
        x
    } else {
        x.signum() * (KNEE + (1.0 - KNEE) * crate::math::tanh((a - KNEE) / (1.0 - KNEE)))
    }
}

/// Carries the synth's stereo samples to a sound card that pulls them at its own pace and rate.
///
/// The game delivers one frame of samples at a time, not quite evenly. A buffer of about
/// 67 ms absorbs that; instead of dropping or repeating samples (which clicks), playback runs
/// a little faster or slower (at most 3 %) to keep the buffer near its target. Samples are
/// interpolated to the card's rate. If the buffer ever runs dry, the sound fades out within a
/// few milliseconds and fades back in when it is full again: a gap, never a click.
#[derive(Clone, Debug)]
pub struct Resampler {
    /// Left and right.
    queue: std::collections::VecDeque<[f32; 2]>,
    pos: f32,
    prev: [f32; 2],
    next: [f32; 2],
    gain: f32,
    started: bool,
    /// Samples the card asked for while the buffer was empty (for tests and diagnosis).
    pub starved: u64,
}

impl Default for Resampler {
    fn default() -> Self {
        Self::new()
    }
}

impl Resampler {
    pub const TARGET: usize = SAMPLES_PER_FRAME * 4;

    pub fn new() -> Resampler {
        Resampler {
            queue: std::collections::VecDeque::with_capacity(Self::TARGET * 4),
            pos: 0.0,
            prev: [0.0; 2],
            next: [0.0; 2],
            gain: 0.0,
            started: false,
            starved: 0,
        }
    }

    /// The game hands over a frame's samples, left and right in turn (`Session::audio`).
    pub fn push(&mut self, samples: &[f32]) {
        self.queue
            .extend(samples.as_chunks::<2>().0.iter().copied());
        // far ahead (the window was dragged, the machine stalled): catch up, rarely
        if self.queue.len() > Self::TARGET * 8 {
            let drop = self.queue.len() - Self::TARGET * 2;
            self.queue.drain(..drop);
        }
    }

    pub fn buffered(&self) -> usize {
        self.queue.len()
    }

    /// The card asks for `out.len() / channels` frames at `rate` Hz: one channel gets the middle
    /// of left and right, two get left and right, any more the middle again.
    pub fn pull(&mut self, out: &mut [f32], channels: usize, rate: u32) {
        if !self.started && self.queue.len() >= Self::TARGET {
            self.started = true;
        }
        let fill = self.queue.len() as f32 / Self::TARGET as f32;
        let nudge = ((fill - 1.0) * 0.03).clamp(-0.03, 0.03);
        let step = SAMPLE_RATE as f32 / rate as f32 * (1.0 + nudge);
        // about 6 ms to fade in or out at 44.1 to 48 kHz
        let ramp = 1.0 / 280.0;
        for frame in out.chunks_mut(channels.max(1)) {
            if self.started {
                self.pos += step;
                while self.pos >= 1.0 {
                    self.pos -= 1.0;
                    match self.queue.pop_front() {
                        Some(v) => {
                            self.prev = self.next;
                            self.next = v;
                        }
                        None => {
                            // dry: hold still, fade out, and wait for a full buffer
                            self.starved += 1;
                            self.started = false;
                            self.pos = 0.0;
                            break;
                        }
                    }
                }
            }
            self.gain = if self.started {
                (self.gain + ramp).min(1.0)
            } else {
                (self.gain - ramp).max(0.0)
            };
            let side =
                |c: usize| (self.prev[c] + (self.next[c] - self.prev[c]) * self.pos) * self.gain;
            let (l, r) = (side(0), side(1));
            match frame {
                [one] => *one = (l + r) * 0.5,
                [left, right, rest @ ..] => {
                    *left = l;
                    *right = r;
                    rest.fill((l + r) * 0.5);
                }
                [] => {}
            }
        }
    }
}

/// A 16-bit mono WAV file.
pub fn wav(samples: &[f32]) -> Vec<u8> {
    encode(samples, 1)
}

/// A 16-bit stereo WAV file, from samples given left and right in turn (`Session::audio`).
pub fn stereo_wav(samples: &[f32]) -> Vec<u8> {
    encode(samples, 2)
}

fn encode(samples: &[f32], channels: u16) -> Vec<u8> {
    let data_len = samples.len() as u32 * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2 * channels as u32).to_le_bytes()); // bytes per second
    out.extend_from_slice(&(2 * channels).to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32_767.0) as i16).to_le_bytes());
    }
    out
}
