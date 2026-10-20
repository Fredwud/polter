//! Mini-experiment for Polter E2: can winit + softbuffer show a CPU picture scaled by whole
//! pixels, in a window and full screen, within the frame budget on a 2015 MacBook Air?
//! 4 s window (320 × 180 at 4×, "fixed"), then 4 s borderless full screen ("expand": the picture
//! grows so that a whole scale fills the screen), then it quits by itself.

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowId};

const BASE: (u32, u32) = (320, 180);

/// CPU time this thread has used, in ms (wall time includes waiting; this doesn't).
fn cpu_ms() -> f64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    ts.tv_sec as f64 * 1000.0 + ts.tv_nsec as f64 / 1e6
}

/// Without pacing: as fast as `present` lets frames go (POLTER_POLL=1).
fn poll() -> bool {
    std::env::var_os("POLTER_POLL").is_some()
}
const PHASE: f64 = 4.0;

#[derive(Default)]
struct Phase {
    name: &'static str,
    size: (u32, u32),
    canvas: (u32, u32),
    scale: u32,
    draw: Vec<f64>,
    upscale: Vec<f64>,
    present: Vec<f64>,
    present_cpu: Vec<f64>,
    interval: Vec<f64>,
}

struct App {
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    start: Instant,
    last: Option<Instant>,
    next: Instant,
    phases: Vec<Phase>,
    full: bool,
    canvas: Vec<u32>,
    frame: u32,
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

impl App {
    fn report(&self) -> String {
        let mut out = String::new();
        for p in &self.phases {
            out += &format!(
                "{}: window {}x{} px, canvas {}x{} at {}x, {} frames\n  draw the canvas: {}\n  scale up:        {}\n  present (wall):  {}\n  present (CPU):   {}\n  frame interval:  {}\n",
                p.name,
                p.size.0,
                p.size.1,
                p.canvas.0,
                p.canvas.1,
                p.scale,
                p.draw.len(),
                stats(&p.draw),
                stats(&p.upscale),
                stats(&p.present),
                stats(&p.present_cpu),
                stats(&p.interval)
            );
        }
        out
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = Rc::new(
            el.create_window(
                Window::default_attributes()
                    .with_title("Polter · window test (closes by itself)")
                    .with_inner_size(LogicalSize::new(1280.0, 720.0)),
            )
            .expect("window"),
        );
        let context = softbuffer::Context::new(window.clone()).expect("context");
        self.surface = Some(softbuffer::Surface::new(&context, window.clone()).expect("surface"));
        self.window = Some(window);
        self.phases.push(Phase {
            name: "window (fixed)",
            ..Phase::default()
        });
        self.start = Instant::now();
        self.next = Instant::now();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::RedrawRequested => self.redraw(el),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if poll() {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            el.set_control_flow(ControlFlow::Poll);
            return;
        }
        // 60 frames a second, paced by the clock
        let now = Instant::now();
        if now >= self.next {
            self.next = now.max(self.next) + Duration::from_micros(16_667);
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
        el.set_control_flow(ControlFlow::WaitUntil(self.next));
    }
}

impl App {
    fn redraw(&mut self, el: &ActiveEventLoop) {
        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else {
            return;
        };
        let t = self.start.elapsed().as_secs_f64();
        if t > 2.0 * PHASE + 0.5 {
            el.exit();
            return;
        }
        if t > PHASE && !self.full {
            self.full = true;
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
            self.phases.push(Phase {
                name: "full screen (expand)",
                ..Phase::default()
            });
            self.last = None;
            return;
        }
        let size = window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));
        // fixed: the base picture at the largest whole scale; expand: as many more pixels as
        // that scale can fill
        let scale = (w / BASE.0).min(h / BASE.1).max(1);
        let (cw, ch) = if self.full {
            (w / scale, h / scale)
        } else {
            BASE
        };
        let now = Instant::now();
        // draw a moving test picture into the canvas (stands in for a game's frame)
        let a = Instant::now();
        self.canvas.resize((cw * ch) as usize, 0);
        self.frame += 1;
        let f = self.frame;
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
        let draw_ms = a.elapsed().as_secs_f64() * 1000.0;
        // scale up by whole pixels into the window, bars around
        let b = Instant::now();
        surface
            .resize(NonZeroU32::new(w).unwrap(), NonZeroU32::new(h).unwrap())
            .expect("resize");
        let mut buf = surface.buffer_mut().expect("buffer");
        let (ox, oy) = ((w - cw * scale) / 2, (h - ch * scale) / 2);
        buf.fill(0);
        let mut row = vec![0u32; (cw * scale) as usize];
        for sy in 0..ch {
            let src = &self.canvas[(sy * cw) as usize..((sy + 1) * cw) as usize];
            for (k, &p) in src.iter().enumerate() {
                row[k * scale as usize..(k + 1) * scale as usize].fill(p);
            }
            for r in 0..scale {
                let at = ((oy + sy * scale + r) * w + ox) as usize;
                buf[at..at + row.len()].copy_from_slice(&row);
            }
        }
        let upscale_ms = b.elapsed().as_secs_f64() * 1000.0;
        let (c, cc) = (Instant::now(), cpu_ms());
        buf.present().expect("present");
        let present_ms = c.elapsed().as_secs_f64() * 1000.0;
        let present_cpu = cpu_ms() - cc;
        let phase = self.phases.last_mut().expect("phase");
        // the first frames of a phase settle (window appearing, the full-screen animation)
        let settled = if self.full { t > PHASE + 1.5 } else { t > 0.5 };
        if settled {
            phase.size = (w, h);
            phase.canvas = (cw, ch);
            phase.scale = scale;
            phase.draw.push(draw_ms);
            phase.upscale.push(upscale_ms);
            phase.present.push(present_ms);
            phase.present_cpu.push(present_cpu);
            if let Some(last) = self.last {
                phase.interval.push((now - last).as_secs_f64() * 1000.0);
            }
        }
        self.last = Some(now);
    }
}

fn main() {
    let el = EventLoop::new().expect("event loop");
    let mut app = App {
        window: None,
        surface: None,
        start: Instant::now(),
        last: None,
        next: Instant::now(),
        phases: Vec::new(),
        full: false,
        canvas: Vec::new(),
        frame: 0,
    };
    el.run_app(&mut app).expect("run");
    let report = app.report();
    print!("{report}");
    let out = std::env::args().nth(1).unwrap_or_else(|| "wintest_result.txt".into());
    std::fs::write(&out, &report).expect("write the result");
}
