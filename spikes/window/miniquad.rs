//! The same measurement with miniquad: the CPU picture is uploaded as a small texture and the GPU
//! scales it by whole pixels (nearest neighbour). 4 s a window (320 × 180 at 4×), then 4 s real
//! full screen ("expand"), then it quits by itself and writes the result.

use miniquad::*;
use std::time::Instant;

const BASE: (u32, u32) = (320, 180);
const PHASE: f64 = 4.0;

fn cpu_ms() -> f64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    ts.tv_sec as f64 * 1000.0 + ts.tv_nsec as f64 / 1e6
}

fn stats(v: &[f64]) -> String {
    if v.is_empty() {
        return "-".into();
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let avg = s.iter().sum::<f64>() / s.len() as f64;
    let p95 = s[((s.len() as f64 * 0.95) as usize).min(s.len() - 1)];
    format!("avg {avg:.2} ms, p95 {p95:.2} ms, max {:.2} ms", s[s.len() - 1])
}

#[derive(Default)]
struct Phase {
    name: &'static str,
    size: (u32, u32),
    canvas: (u32, u32),
    scale: u32,
    show_cpu: Vec<f64>,
    show_wall: Vec<f64>,
    interval: Vec<f64>,
}

struct Stage {
    ctx: Box<dyn RenderingBackend>,
    pipeline: Pipeline,
    bindings: Bindings,
    tex_size: (u32, u32),
    canvas: Vec<u32>,
    rgba: Vec<u8>,
    start: Instant,
    last: Option<Instant>,
    full: bool,
    frame: u32,
    phases: Vec<Phase>,
    out: String,
    done: bool,
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
    fn new() -> Stage {
        let mut ctx = window::new_rendering_backend();
        let verts = [0.0f32; 16];
        let vb = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Stream,
            BufferSource::slice(&verts),
        );
        let ib = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&[0u16, 1, 2, 0, 2, 3]),
        );
        let tex = ctx.new_texture_from_rgba8(BASE.0 as u16, BASE.1 as u16, &vec![0u8; (BASE.0 * BASE.1 * 4) as usize]);
        ctx.texture_set_filter(tex, FilterMode::Nearest, MipmapFilterMode::None);
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
            .expect("shader");
        let pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("in_pos", VertexFormat::Float2),
                VertexAttribute::new("in_uv", VertexFormat::Float2),
            ],
            shader,
            PipelineParams::default(),
        );
        Stage {
            ctx,
            pipeline,
            bindings: Bindings {
                vertex_buffers: vec![vb],
                index_buffer: ib,
                images: vec![tex],
            },
            tex_size: BASE,
            canvas: Vec::new(),
            rgba: Vec::new(),
            start: Instant::now(),
            last: None,
            full: false,
            frame: 0,
            phases: vec![Phase {
                name: "window (fixed)",
                ..Phase::default()
            }],
            out: std::env::args().nth(1).unwrap_or("mq_result.txt".into()),
            done: false,
        }
    }

    fn report(&self) -> String {
        let mut s = format!("backend: {:?}\n", self.ctx.info().backend);
        for p in &self.phases {
            s += &format!(
                "{}: window {}x{} px, canvas {}x{} at {}x, {} frames\n  upload + draw + present (CPU):  {}\n  upload + draw + present (wall): {}\n  frame interval:                 {}\n",
                p.name, p.size.0, p.size.1, p.canvas.0, p.canvas.1, p.scale, p.show_cpu.len(),
                stats(&p.show_cpu), stats(&p.show_wall), stats(&p.interval)
            );
        }
        s
    }
}

impl EventHandler for Stage {
    fn update(&mut self) {}

    fn draw(&mut self) {
        if self.done {
            return;
        }
        let t = self.start.elapsed().as_secs_f64();
        if t > 2.0 * PHASE + 0.5 {
            self.done = true;
            let r = self.report();
            print!("{r}");
            std::fs::write(&self.out, r).expect("write the result");
            window::quit();
            return;
        }
        if t > PHASE && !self.full {
            self.full = true;
            window::set_fullscreen(true);
            self.phases.push(Phase {
                name: "full screen (expand)",
                ..Phase::default()
            });
            self.last = None;
        }
        let now = Instant::now();
        let (sw, sh) = window::screen_size();
        let (w, h) = ((sw as u32).max(1), (sh as u32).max(1));
        let scale = (w / BASE.0).min(h / BASE.1).max(1);
        let (cw, ch) = if self.full { (w / scale, h / scale) } else { BASE };
        // the game's picture, drawn on the CPU (a moving test pattern)
        self.frame += 1;
        let f = self.frame;
        self.canvas.resize((cw * ch) as usize, 0);
        for y in 0..ch {
            for x in 0..cw {
                let v = ((x + f) ^ y) & 31;
                let edge = x == 0 || y == 0 || x == cw - 1 || y == ch - 1;
                self.canvas[(y * cw + x) as usize] = if edge {
                    0xffd447
                } else {
                    (v * 6) << 16 | (v * 3) << 8 | (40 + v * 4)
                };
            }
        }
        let (c0, w0) = (cpu_ms(), Instant::now());
        // 0RGB to RGBA bytes, uploaded as a texture of the picture's own size
        self.rgba.clear();
        for &p in &self.canvas {
            self.rgba.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, 255]);
        }
        if self.tex_size != (cw, ch) {
            self.ctx.delete_texture(self.bindings.images[0]);
            let tex = self.ctx.new_texture_from_rgba8(cw as u16, ch as u16, &self.rgba);
            self.ctx.texture_set_filter(tex, FilterMode::Nearest, MipmapFilterMode::None);
            self.bindings.images[0] = tex;
            self.tex_size = (cw, ch);
        } else {
            self.ctx.texture_update(self.bindings.images[0], &self.rgba);
        }
        // a quad of whole-pixel scale, centred
        let (qw, qh) = ((cw * scale) as f32 / w as f32, (ch * scale) as f32 / h as f32);
        let verts: [f32; 16] = [
            -qw, qh, 0.0, 0.0, //
            qw, qh, 1.0, 0.0, //
            qw, -qh, 1.0, 1.0, //
            -qw, -qh, 0.0, 1.0,
        ];
        self.ctx
            .buffer_update(self.bindings.vertex_buffers[0], BufferSource::slice(&verts));
        self.ctx.begin_default_pass(PassAction::clear_color(0.0, 0.0, 0.0, 1.0));
        self.ctx.apply_pipeline(&self.pipeline);
        self.ctx.apply_bindings(&self.bindings);
        self.ctx.draw(0, 6, 1);
        self.ctx.end_render_pass();
        self.ctx.commit_frame();
        let (show_cpu, show_wall) = (cpu_ms() - c0, w0.elapsed().as_secs_f64() * 1000.0);
        let settled = if self.full { t > PHASE + 1.5 } else { t > 0.7 };
        let phase = self.phases.last_mut().expect("phase");
        if settled {
            phase.size = (w, h);
            phase.canvas = (cw, ch);
            phase.scale = scale;
            phase.show_cpu.push(show_cpu);
            phase.show_wall.push(show_wall);
            if let Some(l) = self.last {
                phase.interval.push((now - l).as_secs_f64() * 1000.0);
            }
        }
        self.last = Some(now);
    }
}

fn main() {
    let conf = conf::Conf {
        window_title: "Polter · window test (closes by itself)".to_string(),
        window_width: 1280,
        window_height: 720,
        ..Default::default()
    };
    miniquad::start(conf, || Box::new(Stage::new()));
}
