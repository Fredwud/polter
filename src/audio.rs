// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Sound: a tiny deterministic synth. Carts describe sound effects, Polter renders them into
//! samples, the same way in a window and headless, so agents can check sound with `listen`.

/// Samples per second.
pub const SAMPLE_RATE: u32 = 44_100;
/// Samples rendered per frame (44 100 / 60).
pub const SAMPLES_PER_FRAME: usize = (SAMPLE_RATE / crate::machine::FPS) as usize;
const VOICES: usize = 8;

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

    /// Fills `out` with the next samples, mixed and gently limited to -1.0..1.0.
    pub fn render(&mut self, out: &mut [f32]) {
        let attack = SAMPLE_RATE as f32 * 0.002; // 2 ms, so tones don't click on
        for sample in out.iter_mut() {
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
            *sample = limit(mix);
        }
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

/// Carries the synth's samples to a sound card that pulls them at its own pace and rate.
///
/// The game delivers one frame of samples at a time, not quite evenly. A buffer of about
/// 67 ms absorbs that; instead of dropping or repeating samples (which clicks), playback runs
/// a little faster or slower (at most 3 %) to keep the buffer near its target. Samples are
/// interpolated to the card's rate. If the buffer ever runs dry, the sound fades out within a
/// few milliseconds and fades back in when it is full again: a gap, never a click.
#[derive(Clone, Debug)]
pub struct Resampler {
    queue: std::collections::VecDeque<f32>,
    pos: f32,
    prev: f32,
    next: f32,
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
            prev: 0.0,
            next: 0.0,
            gain: 0.0,
            started: false,
            starved: 0,
        }
    }

    /// The game hands over a frame's samples.
    pub fn push(&mut self, samples: &[f32]) {
        self.queue.extend(samples);
        // far ahead (the window was dragged, the machine stalled): catch up, rarely
        if self.queue.len() > Self::TARGET * 8 {
            let drop = self.queue.len() - Self::TARGET * 2;
            self.queue.drain(..drop);
        }
    }

    pub fn buffered(&self) -> usize {
        self.queue.len()
    }

    /// The card asks for `out.len() / channels` frames at `rate` Hz.
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
            frame.fill((self.prev + (self.next - self.prev) * self.pos) * self.gain);
        }
    }
}

/// A 16-bit mono WAV file.
pub fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() as u32 * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // bytes per second
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32_767.0) as i16).to_le_bytes());
    }
    out
}
