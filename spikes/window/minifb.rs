//! The same measurement with minifb (which shows its buffer through the GPU on a Mac):
//! 4 s a 320 × 180 picture at 4× in a 1280 × 720 window, then 4 s a 360 × 225 picture
//! stretched over a borderless 1440 × 900 window (as full screen would be).

use minifb::{Scale, ScaleMode, Window, WindowOptions};
use std::time::Instant;

fn cpu_ms() -> f64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    ts.tv_sec as f64 * 1000.0 + ts.tv_nsec as f64 / 1e6
}

fn stats(v: &[f64]) -> String {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let avg = s.iter().sum::<f64>() / s.len() as f64;
    let p95 = s[((s.len() as f64 * 0.95) as usize).min(s.len() - 1)];
    format!("avg {avg:.2} ms, p95 {p95:.2} ms, max {:.2} ms", s[s.len() - 1])
}

fn run(name: &str, cw: usize, ch: usize, opts: WindowOptions, out: &mut String) {
    let mut win = Window::new(name, cw, ch, opts).expect("window");
    win.set_target_fps(60);
    let mut canvas = vec![0u32; cw * ch];
    let (mut wall, mut cpu, mut interval) = (Vec::new(), Vec::new(), Vec::new());
    let start = Instant::now();
    let mut last: Option<Instant> = None;
    let mut f = 0u32;
    while win.is_open() && start.elapsed().as_secs_f64() < 4.0 {
        f += 1;
        for y in 0..ch {
            for x in 0..cw {
                let v = ((x as u32 + f) ^ y as u32) & 31;
                canvas[y * cw + x] = (v * 6) << 16 | (v * 3) << 8 | (40 + v * 4);
            }
        }
        let (t, c) = (Instant::now(), cpu_ms());
        win.update_with_buffer(&canvas, cw, ch).expect("update");
        if start.elapsed().as_secs_f64() > 0.7 {
            wall.push(t.elapsed().as_secs_f64() * 1000.0);
            cpu.push(cpu_ms() - c);
            if let Some(l) = last {
                interval.push((t - l).as_secs_f64() * 1000.0);
            }
        }
        last = Some(t);
    }
    let (w, h) = win.get_size();
    *out += &format!(
        "{name}: window {w}x{h}, canvas {cw}x{ch}, {} frames\n  update_with_buffer (wall): {}\n  update_with_buffer (CPU):  {}\n  frame interval:            {}\n",
        wall.len(), stats(&wall), stats(&cpu), stats(&interval)
    );
}

fn main() {
    let mut out = String::new();
    run("minifb window 4x", 320, 180, WindowOptions { scale: Scale::X4, ..WindowOptions::default() }, &mut out);
    run("minifb borderless 1440x900 stretched", 360, 225, WindowOptions {
        borderless: true,
        resize: true,
        scale: Scale::X4,
        scale_mode: ScaleMode::AspectRatioStretch,
        ..WindowOptions::default()
    }, &mut out);
    print!("{out}");
    std::fs::write(std::env::args().nth(1).unwrap_or("mfb_result.txt".into()), out).expect("write");
}
