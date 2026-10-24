// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The window: shows a game's picture in a window or full screen, and paces it at 60 ticks a
//! second. The picture is computed on the CPU as everywhere else; here it is uploaded as a small
//! texture and the GPU scales it by whole pixels (miniquad; measured in spikes/window: under half
//! a millisecond a frame, full screen included). Showing the picture never changes it.
//!
//! Full screen: Ctrl+Cmd+F on a Mac, F11 or Alt+Enter elsewhere; the window remembers it.
//!
//! For agents, who can't watch a window: `POLTER_CLOSE_AFTER=SECONDS` closes it by itself, and
//! `POLTER_WINDOW_REPORT=FILE` writes, when it closes, how it ran: display frames, ticks, the
//! time between frames and the game's own time per frame.

#![allow(
    clippy::disallowed_methods,
    reason = "the window paces frames by the clock: when a tick runs and a frame shows, never what \
              happens in it"
)]

use crate::canvas::Canvas;
use crate::input::{Frame, Input, Key, Mouse};
use miniquad::{
    Bindings, BufferLayout, BufferSource, BufferType, BufferUsage, EventHandler, FilterMode,
    KeyCode, KeyMods, MipmapFilterMode, MouseButton, PassAction, Pipeline, PipelineParams,
    RenderingBackend, ShaderMeta, ShaderSource, UniformBlockLayout, VertexAttribute, VertexFormat,
    conf, window,
};
use std::path::PathBuf;
use std::time::Instant;

/// How the picture fills the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// The picture keeps its size, scaled by the largest whole number that fits, with bars
    /// around what is left over.
    Fixed,
    /// The picture grows by as many pixels as fill the window at that whole scale: no bars. Its
    /// size is the smallest it may have. (A game's `update` must not depend on the size.)
    Expand,
    /// The picture is the window's size, one picture pixel to one screen pixel.
    Native,
}

/// How to open the window.
pub struct Conf {
    pub title: String,
    /// The picture's size (`Fixed`), or the smallest it may be (`Expand`); unused for `Native`.
    pub size: (usize, usize),
    pub fit: Fit,
    /// A file where the window remembers whether it was full screen; `None`: it doesn't.
    pub remember: Option<PathBuf>,
    /// Hide the system's cursor (the game draws its own).
    pub hide_cursor: bool,
}

/// What the window hands the game each time the display shows a frame.
pub struct Io<'a> {
    /// How many ticks of 1/60 s are due: usually one; none or two when the display's frames
    /// drift against the clock; at most four after a stall.
    pub ticks: u32,
    /// Keys, mouse (in picture pixels) and wheel as they are now: what a session steps with.
    pub frame: Frame,
    /// The same with edges since the last display frame (`pressed`, `released`).
    pub input: &'a Input,
    /// Characters typed since the last display frame.
    pub typed: &'a [char],
    pub full_screen: bool,
}

/// A game in the window.
pub trait Host {
    /// One display frame: run the due ticks and draw into the canvas, whose size follows the
    /// fit. Return false to close the window.
    fn frame(&mut self, io: &Io, canvas: &mut Canvas) -> bool;
    /// The window is closing (Cmd+Q, the close button, or `frame` said so): save now, the
    /// process may end right after this.
    fn closing(&mut self) {}
}

const TICK: f64 = 1.0 / 60.0;

/// Opens the window and runs the game in it until it closes. Must be called on the main thread.
pub fn run(conf: Conf, host: Box<dyn Host>) {
    let full = conf
        .remember
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .is_some_and(|t| t.lines().any(|l| l.trim() == "fullscreen 1"));
    // a window of a whole scale of the picture, not larger than a small laptop screen
    let (w, h) = (conf.size.0.max(1), conf.size.1.max(1));
    let scale = (1280 / w).min(800 / h).max(1);
    let mq = conf::Conf {
        window_title: conf.title.clone(),
        window_width: (w * scale) as i32,
        window_height: (h * scale) as i32,
        high_dpi: true,
        fullscreen: full,
        window_resizable: true,
        ..Default::default()
    };
    miniquad::start(mq, move || Box::new(Stage::new(conf, host, full)));
}

struct Stage {
    ctx: Box<dyn RenderingBackend>,
    pipeline: Pipeline,
    bindings: Bindings,
    tex: (usize, usize),
    rgba: Vec<u8>,
    canvas: Canvas,
    host: Box<dyn Host>,
    fit: Fit,
    base: (usize, usize),
    remember: Option<PathBuf>,
    full: bool,
    keys: u64,
    mouse_px: (f32, f32),
    mouse_buttons: u8,
    wheel: f32,
    typed: Vec<char>,
    input: Input,
    /// Gamepads: they press the console's buttons.
    pads: crate::pad::Pads,
    last: Option<Instant>,
    owed: f64,
    closed: bool,
    /// Where the picture sits in the window, and its scale, for turning the mouse into
    /// picture pixels.
    at: (f32, f32, f32),
    started: Instant,
    close_after: Option<f64>,
    report: Option<PathBuf>,
    /// Per display frame: ms since the last one, ms the game took, ticks run.
    timings: Vec<(f64, f64, u32)>,
    last_frame_ms: Option<f64>,
}

const VERTEX: &str = r#"#version 100
attribute vec2 in_pos;
attribute vec2 in_uv;
varying highp vec2 uv;
void main() {
    gl_Position = vec4(in_pos, 0.0, 1.0);
    uv = in_uv;
}"#;

const FRAGMENT: &str = r#"#version 100
precision highp float;
varying highp vec2 uv;
uniform sampler2D tex;
void main() {
    gl_FragColor = texture2D(tex, uv);
}"#;

impl Stage {
    fn new(conf: Conf, host: Box<dyn Host>, full: bool) -> Stage {
        let mut ctx = window::new_rendering_backend();
        let vb = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Stream,
            BufferSource::slice(&[0.0f32; 16]),
        );
        let ib = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&[0u16, 1, 2, 0, 2, 3]),
        );
        let tex = ctx.new_texture_from_rgba8(1, 1, &[0, 0, 0, 255]);
        let shader = ctx
            .new_shader(
                ShaderSource::Glsl {
                    vertex: VERTEX,
                    fragment: FRAGMENT,
                },
                ShaderMeta {
                    images: vec!["tex".to_string()],
                    uniforms: UniformBlockLayout { uniforms: vec![] },
                },
            )
            .expect("the window's shader");
        let pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("in_pos", VertexFormat::Float2),
                VertexAttribute::new("in_uv", VertexFormat::Float2),
            ],
            shader,
            PipelineParams::default(),
        );
        if conf.hide_cursor {
            window::show_mouse(false);
        }
        Stage {
            ctx,
            pipeline,
            bindings: Bindings {
                vertex_buffers: vec![vb],
                index_buffer: ib,
                images: vec![tex],
            },
            tex: (1, 1),
            rgba: Vec::new(),
            canvas: Canvas::new(conf.size.0.max(1), conf.size.1.max(1)),
            host,
            fit: conf.fit,
            base: (conf.size.0.max(1), conf.size.1.max(1)),
            remember: conf.remember,
            full,
            keys: 0,
            mouse_px: (0.0, 0.0),
            mouse_buttons: 0,
            wheel: 0.0,
            typed: Vec::new(),
            input: Input::default(),
            pads: crate::pad::Pads::open(),
            last: None,
            owed: 0.0,
            closed: false,
            at: (0.0, 0.0, 1.0),
            started: Instant::now(),
            close_after: std::env::var("POLTER_CLOSE_AFTER")
                .ok()
                .and_then(|v| v.parse().ok()),
            report: std::env::var_os("POLTER_WINDOW_REPORT").map(PathBuf::from),
            timings: Vec::new(),
            last_frame_ms: None,
        }
    }

    /// How the window ran, for an agent that couldn't watch it.
    fn write_report(&self) {
        let Some(path) = &self.report else { return };
        let stats = |v: Vec<f64>| {
            if v.is_empty() {
                return "-".to_string();
            }
            let mut v = v;
            v.sort_by(|a, b| a.total_cmp(b));
            let avg = v.iter().sum::<f64>() / v.len() as f64;
            let p95 = v[((v.len() as f64 * 0.95) as usize).min(v.len() - 1)];
            format!(
                "avg {avg:.2} ms, p95 {p95:.2} ms, max {:.2} ms",
                v[v.len() - 1]
            )
        };
        // the first frames settle (the window appearing, a world loading)
        let settled: Vec<&(f64, f64, u32)> = self.timings.iter().skip(30).collect();
        let ticks: u32 = self.timings.iter().map(|t| t.2).sum();
        let text = format!(
            "display frames {}\nticks {ticks}\nseconds {:.2}\nfull screen {}\npicture {}x{} at {}x\nbetween frames: {}\ngame per frame: {}\n",
            self.timings.len(),
            self.started.elapsed().as_secs_f64(),
            self.full,
            self.canvas.width(),
            self.canvas.height(),
            self.at.2,
            stats(settled.iter().map(|t| t.0).collect()),
            stats(settled.iter().map(|t| t.1).collect()),
        );
        if let Err(e) = std::fs::write(path, text) {
            eprintln!("window report {}: {e}", path.display());
        }
    }

    fn remember_full_screen(&self) {
        if let Some(p) = &self.remember {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, format!("fullscreen {}\n", self.full as u8));
        }
    }

    fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.remember_full_screen();
            self.host.closing();
            self.write_report();
        }
    }
}

/// The picture's size and its whole scale, for a window of `w` × `h` pixels.
pub fn layout(fit: Fit, base: (usize, usize), w: usize, h: usize) -> ((usize, usize), usize) {
    let (bw, bh) = (base.0.max(1), base.1.max(1));
    match fit {
        Fit::Fixed => ((bw, bh), (w / bw).min(h / bh).max(1)),
        Fit::Expand => {
            let s = (w / bw).min(h / bh).max(1);
            (((w / s).max(bw), (h / s).max(bh)), s)
        }
        Fit::Native => ((w.max(1), h.max(1)), 1),
    }
}

/// The engine's key for one of miniquad's.
fn key_of(k: KeyCode) -> Option<Key> {
    use KeyCode as K;
    let letters = [
        K::A,
        K::B,
        K::C,
        K::D,
        K::E,
        K::F,
        K::G,
        K::H,
        K::I,
        K::J,
        K::K,
        K::L,
        K::M,
        K::N,
        K::O,
        K::P,
        K::Q,
        K::R,
        K::S,
        K::T,
        K::U,
        K::V,
        K::W,
        K::X,
        K::Y,
        K::Z,
    ];
    let digits = [
        K::Key0,
        K::Key1,
        K::Key2,
        K::Key3,
        K::Key4,
        K::Key5,
        K::Key6,
        K::Key7,
        K::Key8,
        K::Key9,
    ];
    let functions = [
        K::F1,
        K::F2,
        K::F3,
        K::F4,
        K::F5,
        K::F6,
        K::F7,
        K::F8,
        K::F9,
        K::F10,
        K::F11,
        K::F12,
    ];
    if let Some(i) = letters.iter().position(|&l| l == k) {
        return Some(Key::ALL[i]);
    }
    if let Some(i) = digits.iter().position(|&d| d == k) {
        return Some(Key::ALL[26 + i]);
    }
    if let Some(i) = functions.iter().position(|&f| f == k) {
        return Some(Key::ALL[Key::F1 as usize + i]);
    }
    Some(match k {
        K::Space => Key::Space,
        K::Enter | K::KpEnter => Key::Enter,
        K::Tab => Key::Tab,
        K::Backspace => Key::Backspace,
        K::Delete => Key::Delete,
        K::Escape => Key::Escape,
        K::LeftShift | K::RightShift => Key::Shift,
        K::LeftControl | K::RightControl => Key::Ctrl,
        K::LeftAlt | K::RightAlt => Key::Alt,
        K::LeftSuper | K::RightSuper => Key::Super,
        K::Up => Key::Up,
        K::Down => Key::Down,
        K::Left => Key::Left,
        K::Right => Key::Right,
        _ => return None,
    })
}

impl EventHandler for Stage {
    fn update(&mut self) {}

    fn draw(&mut self) {
        if self.closed {
            return;
        }
        // the ticks due: real time in steps of 1/60 s, with a little slack so a display at
        // 60 Hz gives one tick a frame and not none, two, none
        let now = Instant::now();
        let ticks = match self.last {
            None => 1,
            Some(last) => {
                self.owed += (now - last).as_secs_f64();
                if self.owed > 0.25 {
                    // a stall (the window was dragged, the machine was busy): don't race
                    self.owed = TICK;
                }
                let n = ((self.owed + 0.002) / TICK).floor().clamp(0.0, 4.0);
                self.owed = (self.owed - n * TICK).max(0.0);
                n as u32
            }
        };
        self.last_frame_ms = self.last.map(|l| (now - l).as_secs_f64() * 1000.0);
        self.last = Some(now);
        // the picture's size for this window
        let (sw, sh) = window::screen_size();
        let (w, h) = ((sw as usize).max(1), (sh as usize).max(1));
        let ((cw, ch), s) = layout(self.fit, self.base, w, h);
        if (self.canvas.width(), self.canvas.height()) != (cw, ch) {
            self.canvas = Canvas::new(cw, ch);
        }
        let (ox, oy) = (
            (w.saturating_sub(cw * s) / 2) as f32,
            (h.saturating_sub(ch * s) / 2) as f32,
        );
        self.at = (ox, oy, s as f32);
        // input in picture pixels
        let mx = ((self.mouse_px.0 - ox) / s as f32).clamp(0.0, cw as f32 - 1.0) as i32;
        let my = ((self.mouse_px.1 - oy) / s as f32).clamp(0.0, ch as f32 - 1.0) as i32;
        let steps = self.wheel.trunc();
        self.wheel -= steps;
        let (buttons, pad) = self.pads.read();
        let frame = Frame {
            buttons,
            mouse: (mx, my),
            mouse_buttons: self.mouse_buttons,
            wheel: steps as i32,
            keys: self.keys,
            pad,
        };
        self.input.advance(&frame);
        let io = Io {
            ticks,
            frame,
            input: &self.input,
            typed: &self.typed,
            full_screen: self.full,
        };
        let busy = Instant::now();
        let mut go_on = self.host.frame(&io, &mut self.canvas);
        let game_ms = busy.elapsed().as_secs_f64() * 1000.0;
        self.timings
            .push((self.last_frame_ms.take().unwrap_or(0.0), game_ms, ticks));
        if self
            .close_after
            .is_some_and(|t| self.started.elapsed().as_secs_f64() >= t)
        {
            go_on = false;
        }
        self.typed.clear();
        if !go_on {
            self.close();
            window::order_quit();
            return;
        }
        // the picture as a texture of its own size
        self.rgba.clear();
        for &p in self.canvas.pixels() {
            self.rgba
                .extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, 255]);
        }
        if self.tex != (cw, ch) {
            self.ctx.delete_texture(self.bindings.images[0]);
            let tex = self
                .ctx
                .new_texture_from_rgba8(cw as u16, ch as u16, &self.rgba);
            self.ctx
                .texture_set_filter(tex, FilterMode::Nearest, MipmapFilterMode::None);
            self.bindings.images[0] = tex;
            self.tex = (cw, ch);
        } else {
            self.ctx.texture_update(self.bindings.images[0], &self.rgba);
        }
        // a quad of the whole scale, centred
        let (qw, qh) = ((cw * s) as f32 / w as f32, (ch * s) as f32 / h as f32);
        let verts: [f32; 16] = [
            -qw, qh, 0.0, 0.0, //
            qw, qh, 1.0, 0.0, //
            qw, -qh, 1.0, 1.0, //
            -qw, -qh, 0.0, 1.0,
        ];
        self.ctx
            .buffer_update(self.bindings.vertex_buffers[0], BufferSource::slice(&verts));
        self.ctx
            .begin_default_pass(PassAction::clear_color(0.0, 0.0, 0.0, 1.0));
        self.ctx.apply_pipeline(&self.pipeline);
        self.ctx.apply_bindings(&self.bindings);
        self.ctx.draw(0, 6, 1);
        self.ctx.end_render_pass();
        self.ctx.commit_frame();
    }

    fn key_down_event(&mut self, keycode: KeyCode, mods: KeyMods, repeat: bool) {
        let toggle = (keycode == KeyCode::F && mods.ctrl && mods.logo)
            || keycode == KeyCode::F11
            || (keycode == KeyCode::Enter && mods.alt);
        if toggle {
            if !repeat {
                self.full = !self.full;
                window::set_fullscreen(self.full);
                self.remember_full_screen();
            }
            return;
        }
        if let Some(k) = key_of(keycode) {
            self.keys |= k.bit();
        }
    }

    fn key_up_event(&mut self, keycode: KeyCode, _mods: KeyMods) {
        if let Some(k) = key_of(keycode) {
            self.keys &= !k.bit();
        }
    }

    fn char_event(&mut self, character: char, _mods: KeyMods, _repeat: bool) {
        if !character.is_control() {
            self.typed.push(character);
        }
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        self.mouse_px = (x, y);
    }

    fn mouse_wheel_event(&mut self, _x: f32, y: f32) {
        self.wheel += y;
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, x: f32, y: f32) {
        self.mouse_px = (x, y);
        self.mouse_buttons |= mouse_bit(button);
    }

    fn mouse_button_up_event(&mut self, button: MouseButton, x: f32, y: f32) {
        self.mouse_px = (x, y);
        self.mouse_buttons &= !mouse_bit(button);
    }

    fn quit_requested_event(&mut self) {
        self.close();
    }
}

fn mouse_bit(b: MouseButton) -> u8 {
    match b {
        MouseButton::Left => Mouse::LEFT,
        MouseButton::Right => Mouse::RIGHT,
        MouseButton::Middle => Mouse::MIDDLE,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picture_fills_a_macbook_air_by_whole_pixels() {
        // the 2015 MacBook Air's screen: 1440 × 900
        assert_eq!(layout(Fit::Fixed, (320, 180), 1440, 900), ((320, 180), 4));
        assert_eq!(layout(Fit::Expand, (320, 180), 1440, 900), ((360, 225), 4));
        assert_eq!(layout(Fit::Expand, (320, 180), 1920, 1080), ((320, 180), 6));
        assert_eq!(layout(Fit::Native, (320, 180), 1440, 900), ((1440, 900), 1));
        // a window smaller than the picture still shows it, at 1×
        assert_eq!(layout(Fit::Fixed, (320, 180), 200, 100), ((320, 180), 1));
    }

    #[test]
    fn keys_map_to_the_engines_and_unknown_keys_to_nothing() {
        assert_eq!(key_of(KeyCode::A), Some(Key::A));
        assert_eq!(key_of(KeyCode::Key7), Some(Key::Num7));
        assert_eq!(key_of(KeyCode::F12), Some(Key::F12));
        assert_eq!(key_of(KeyCode::RightSuper), Some(Key::Super));
        assert_eq!(key_of(KeyCode::Up), Some(Key::Up));
        assert_eq!(key_of(KeyCode::PageUp), None);
    }
}
