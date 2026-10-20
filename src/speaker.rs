// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The speaker: the default sound output, fed frame by frame (cpal). Any problem means silence,
//! never a crash.

use std::sync::Mutex;

/// Sound output for a window: frames push samples into a queue, the audio thread pulls them.
pub struct Speaker {
    _stream: cpal::Stream,
    out: std::sync::Arc<Mutex<crate::audio::Resampler>>,
}

impl Speaker {
    /// Opens the default output device. Any problem means silence, never a crash.
    pub fn open() -> Option<Speaker> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let device = cpal::default_host().default_output_device()?;
        let supported = device.default_output_config().ok()?;
        if supported.sample_format() != cpal::SampleFormat::F32 {
            eprintln!(
                "audio: the output device wants {:?} samples; playing without sound",
                supported.sample_format()
            );
            return None;
        }
        let config: cpal::StreamConfig = supported.into();
        let (channels, rate) = (config.channels as usize, config.sample_rate);
        let out = std::sync::Arc::new(Mutex::new(crate::audio::Resampler::new()));
        let source = out.clone();
        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| match source.lock() {
                    Ok(mut r) => r.pull(data, channels, rate),
                    Err(_) => data.fill(0.0),
                },
                |err| eprintln!("audio: {err}"),
                None,
            )
            .map_err(|err| eprintln!("audio: {err}; playing without sound"))
            .ok()?;
        stream
            .play()
            .map_err(|err| eprintln!("audio: {err}; playing without sound"))
            .ok()?;
        Some(Speaker {
            _stream: stream,
            out,
        })
    }

    pub fn push(&self, samples: &[f32]) {
        if let Ok(mut r) = self.out.lock() {
            r.push(samples);
        }
    }
}
