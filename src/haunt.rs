// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `.haunt` input scripts: what is held, pressed and pointed at on which frames.
//! Same format as Poltergeist for the buttons, plus mouse and keyboard (version 2).
//!
//! ```text
//! # frame(s)  inputs
//! 1-30        right                   # console buttons: left right up down a b
//! 31          a
//! 40          mouse 120,64            # cursor position from this frame on
//! 41-60       move 120,64>200,64 click   # glide while holding the left button: a drag
//! 61          rclick                  # right (also: click, mclick)
//! 62          wheel+1                 # wheel steps (wheel-2 …)
//! 70-72       key:r key:shift         # keyboard keys
//! 80-90       pad:south pad:l1        # a gamepad's own buttons (south east west north l1 r1 l2 r2
//!                                     # select start mode l3 r3 up down left right)
//! 80          lstick 0.5,-0.25 r2 0.8 # its sticks (x right, y up) and triggers, from this frame on
//! ```
//!
//! Frame numbers are the frames the cart sees (the first frame is 1). Everything not listed for a
//! frame is up; the cursor, the sticks and the triggers stay where they were last put.

use crate::input::{Button, Frame, Key, Mouse, PadButton, PadFrame};

#[derive(Clone, Debug, PartialEq)]
enum Act {
    Buttons(u8),
    MouseButtons(u8),
    At(i32, i32),
    Move((i32, i32), (i32, i32)),
    Wheel(i32),
    Keys(u64),
    PadButtons(u32),
    /// A stick (0 left, 1 right) or a trigger (2 left, 3 right) put somewhere, from this frame on.
    Lean(usize, i8, i8),
}

/// A parsed script, expanded into one `Frame` per frame for fast replay.
#[derive(Clone, Debug, Default)]
pub struct Haunt {
    frames: Vec<Frame>,
    has_mouse: bool,
}

fn point(s: &str) -> Option<(i32, i32)> {
    let (x, y) = s.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

fn act(word: &str) -> Result<Act, String> {
    if let Some(b) = Button::from_name(word) {
        return Ok(Act::Buttons(b.bit()));
    }
    match word {
        "click" => return Ok(Act::MouseButtons(Mouse::LEFT)),
        "rclick" => return Ok(Act::MouseButtons(Mouse::RIGHT)),
        "mclick" => return Ok(Act::MouseButtons(Mouse::MIDDLE)),
        _ => {}
    }
    if let Some(k) = word.strip_prefix("key:") {
        return Key::from_name(k)
            .map(|k| Act::Keys(k.bit()))
            .ok_or(format!("unknown key {k:?}"));
    }
    if let Some(b) = word.strip_prefix("pad:") {
        return PadButton::from_name(b)
            .map(|b| Act::PadButtons(b.bit()))
            .ok_or(format!("unknown pad button {b:?}"));
    }
    if let Some(n) = word.strip_prefix("wheel") {
        return n
            .parse::<i32>()
            .map(Act::Wheel)
            .map_err(|_| format!("wheel needs a number like wheel+1, got {word:?}"));
    }
    Err(format!(
        "unknown input {word:?} (buttons left right up down a b, click rclick mclick, mouse X,Y, move X,Y>X,Y, wheel+N, key:NAME, pad:NAME, lstick X,Y, rstick X,Y, l2 V, r2 V)"
    ))
}

impl Haunt {
    pub fn parse(text: &str) -> Result<Haunt, String> {
        let mut spans: Vec<(u64, u64, Vec<Act>)> = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let content = line.split('#').next().unwrap_or("").trim();
            if content.is_empty() {
                continue;
            }
            let err = |e: String| format!("line {}: {e}", i + 1);
            let mut words = content.split_whitespace();
            let span = words.next().unwrap_or("");
            let (from, to) = match span.split_once('-') {
                Some((a, b)) => (a.parse::<u64>(), b.parse::<u64>()),
                None => (span.parse::<u64>(), span.parse::<u64>()),
            };
            let (Ok(from), Ok(to)) = (from, to) else {
                return Err(err(format!(
                    "expected a frame or range like 5 or 5-20, got {span:?}"
                )));
            };
            if from == 0 || to < from {
                return Err(err(format!(
                    "frames start at 1 and ranges go upwards, got {span:?}"
                )));
            }
            let mut acts = Vec::new();
            while let Some(w) = words.next() {
                match w {
                    "mouse" => {
                        let p = words
                            .next()
                            .and_then(point)
                            .ok_or_else(|| err("mouse needs X,Y".into()))?;
                        acts.push(Act::At(p.0, p.1));
                    }
                    "move" => {
                        let arg = words.next().unwrap_or("");
                        let (a, b) = arg
                            .split_once('>')
                            .ok_or_else(|| err("move needs X,Y>X,Y".into()))?;
                        let (a, b) = (point(a), point(b));
                        let (Some(a), Some(b)) = (a, b) else {
                            return Err(err("move needs X,Y>X,Y".into()));
                        };
                        acts.push(Act::Move(a, b));
                    }
                    "lstick" | "rstick" => {
                        let (x, y) = words
                            .next()
                            .and_then(|a| a.split_once(','))
                            .and_then(|(x, y)| {
                                Some((x.parse::<f32>().ok()?, y.parse::<f32>().ok()?))
                            })
                            .ok_or_else(|| err(format!("{w} needs X,Y")))?;
                        let which = usize::from(w == "rstick");
                        acts.push(Act::Lean(which, PadFrame::step(x), PadFrame::step(y)));
                    }
                    "l2" | "r2" => {
                        let v = words
                            .next()
                            .and_then(|v| v.parse::<f32>().ok())
                            .ok_or_else(|| err(format!("{w} needs a value from 0 to 1")))?;
                        let which = 2 + usize::from(w == "r2");
                        acts.push(Act::Lean(which, PadFrame::trigger_step(v) as i8, 0));
                    }
                    w => acts.push(act(w).map_err(err)?),
                }
            }
            spans.push((from, to, acts));
        }
        Ok(Haunt::expand(&spans))
    }

    fn expand(spans: &[(u64, u64, Vec<Act>)]) -> Haunt {
        let last = spans.iter().map(|s| s.1).max().unwrap_or(0) as usize;
        let mut frames = vec![Frame::default(); last + 1];
        let mut cursor: Vec<Option<(i32, i32)>> = vec![None; last + 1];
        // the sticks' and triggers' places, where a frame puts them
        let mut leans: Vec<[Option<(i8, i8)>; 4]> = vec![[None; 4]; last + 1];
        let mut has_mouse = false;
        for (from, to, acts) in spans {
            for f in *from as usize..=*to as usize {
                let fr = &mut frames[f];
                for a in acts {
                    match *a {
                        Act::Buttons(b) => fr.buttons |= b,
                        Act::MouseButtons(b) => fr.mouse_buttons |= b,
                        Act::Keys(k) => fr.keys |= k,
                        Act::PadButtons(b) => fr.pad.buttons |= b,
                        Act::Lean(which, x, y) => leans[f][which] = Some((x, y)),
                        Act::Wheel(n) => fr.wheel += n,
                        Act::At(x, y) => {
                            cursor[f] = Some((x, y));
                            has_mouse = true;
                        }
                        Act::Move(a, b) => {
                            let len = (*to - *from).max(1) as f32;
                            let t = (f as u64 - *from) as f32 / len;
                            let lerp =
                                |p: i32, q: i32| (p as f32 + (q - p) as f32 * t).round() as i32;
                            cursor[f] = Some((lerp(a.0, b.0), lerp(a.1, b.1)));
                            has_mouse = true;
                        }
                    }
                }
            }
        }
        // The cursor, the sticks and the triggers stay where they were last put.
        let mut at = (0, 0);
        let mut lean = [(0i8, 0i8); 4];
        for f in 0..=last {
            if let Some(p) = cursor[f] {
                at = p;
            }
            frames[f].mouse = at;
            for (k, l) in leans[f].iter().enumerate() {
                if let Some(v) = l {
                    lean[k] = *v;
                }
            }
            let pad = &mut frames[f].pad;
            pad.sticks = [lean[0].0, lean[0].1, lean[1].0, lean[1].1];
            pad.triggers = [lean[2].0 as u8, lean[3].0 as u8];
        }
        Haunt { frames, has_mouse }
    }

    pub fn load(path: &str) -> Result<Haunt, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
        Haunt::parse(&text)
    }

    /// Everything that comes in on a frame.
    pub fn frame(&self, frame: u64) -> Frame {
        match self.frames.get(frame as usize) {
            Some(f) => *f,
            None => {
                let last = self.frames.last().copied().unwrap_or_default();
                Frame {
                    mouse: last.mouse,
                    pad: PadFrame {
                        buttons: 0,
                        ..last.pad
                    },
                    ..Frame::default()
                }
            }
        }
    }

    /// The console buttons held on a frame.
    pub fn buttons(&self, frame: u64) -> u8 {
        self.frame(frame).buttons
    }

    /// Whether the script ever points the mouse anywhere.
    pub fn has_mouse(&self) -> bool {
        self.has_mouse
    }

    /// Writes a recording of button masks (frame 1 first) as `.haunt` text.
    pub fn write(masks: &[u8]) -> String {
        let frames: Vec<Frame> = masks.iter().map(|&m| Frame::buttons(m)).collect();
        Haunt::write_frames(&frames, false)
    }

    /// Writes a recording of full frames (frame 1 first) as compact `.haunt` text. With `mouse`,
    /// cursor positions are written whenever they change.
    pub fn write_frames(frames: &[Frame], mouse: bool) -> String {
        let mut out =
            String::from("# Recorded with Polter. Frame numbers are the frames the cart saw.\n");
        let held = |f: &Frame| (f.buttons, f.mouse_buttons, f.keys, f.pad.buttons);
        let mut lines: Vec<(usize, String)> = Vec::new();
        let mut i = 0;
        while i < frames.len() {
            let mut j = i;
            while j + 1 < frames.len() && held(&frames[j + 1]) == held(&frames[i]) {
                j += 1;
            }
            let f = &frames[i];
            if held(f) != (0, 0, 0, 0) {
                let mut words: Vec<String> = Button::names(f.buttons)
                    .into_iter()
                    .map(String::from)
                    .collect();
                for (bit, name) in [
                    (Mouse::LEFT, "click"),
                    (Mouse::RIGHT, "rclick"),
                    (Mouse::MIDDLE, "mclick"),
                ] {
                    if f.mouse_buttons & bit != 0 {
                        words.push(name.into());
                    }
                }
                words.extend(
                    Key::ALL
                        .into_iter()
                        .filter(|k| f.keys & k.bit() != 0)
                        .map(|k| format!("key:{}", k.name())),
                );
                words.extend(
                    PadButton::ALL
                        .into_iter()
                        .filter(|b| f.pad.buttons & b.bit() != 0)
                        .map(|b| format!("pad:{}", b.name())),
                );
                let span = if i == j {
                    format!("{}", i + 1)
                } else {
                    format!("{}-{}", i + 1, j + 1)
                };
                lines.push((i, format!("{span:<10} {}", words.join(" "))));
            }
            i = j + 1;
        }
        for (k, f) in frames.iter().enumerate() {
            if f.wheel != 0 {
                lines.push((k, format!("{:<10} wheel{:+}", k + 1, f.wheel)));
            }
            // the sticks and triggers where they change (three decimals bring every step back)
            let before = if k == 0 {
                PadFrame::default()
            } else {
                frames[k - 1].pad
            };
            let mut leans = Vec::new();
            for (s, name) in [(0, "lstick"), (2, "rstick")] {
                let (x, y) = (f.pad.sticks[s], f.pad.sticks[s + 1]);
                if (x, y) != (before.sticks[s], before.sticks[s + 1]) {
                    leans.push(format!(
                        "{name} {:.3},{:.3}",
                        x as f32 / 127.0,
                        y as f32 / 127.0
                    ));
                }
            }
            for (t, name) in [(0, "l2"), (1, "r2")] {
                if f.pad.triggers[t] != before.triggers[t] {
                    leans.push(format!("{name} {:.3}", f.pad.triggers[t] as f32 / 255.0));
                }
            }
            if !leans.is_empty() {
                lines.push((k, format!("{:<10} {}", k + 1, leans.join(" "))));
            }
            if mouse && (k == 0 || f.mouse != frames[k - 1].mouse) {
                lines.push((
                    k,
                    format!("{:<10} mouse {},{}", k + 1, f.mouse.0, f.mouse.1),
                ));
            }
        }
        lines.sort_by_key(|(k, _)| *k);
        for (_, line) in lines {
            out.push_str(&line);
            out.push('\n');
        }
        out
    }
}
