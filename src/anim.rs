// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Animation in ticks. A `Sheet` holds the frames, how many ticks each one shows, the tags that
//! name a run of frames (run, jump, swing), the boxes named per frame (a hitbox, the feet) and
//! the events a frame sends when it starts (a footstep, the moment a swing hits). `Anim` plays a
//! sheet's tags: it is plain data, part of the game's state, so it rewinds and replays exactly.
//!
//! Sheets come from Aseprite (File › Export Sprite Sheet, with JSON data: frames, tags, slices;
//! array or hash) or from a picture laid out in a grid.

use crate::image::Image;
use crate::json::Json;

/// Which way a tag runs through its frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Reverse,
    /// There and back (the end frames are not shown twice).
    PingPong,
    /// Back and there.
    PingPongReverse,
}

/// A run of frames under a name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    /// The first and the last frame (both in the run).
    pub from: usize,
    pub to: usize,
    pub direction: Direction,
    /// How many times it plays, then stays on its last frame; `None`: again and again.
    pub repeat: Option<u32>,
}

/// A box in a frame, in the frame's pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// A named box that may change from frame to frame: each key holds from its frame on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slice {
    pub name: String,
    pub keys: Vec<(usize, Bounds)>,
}

/// The frames of an animated picture and what is known about them.
#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    pub frames: Vec<Image>,
    /// How many ticks (a 60th of a second each) every frame shows.
    pub ticks: Vec<u32>,
    pub tags: Vec<Tag>,
    pub slices: Vec<Slice>,
    /// Events by frame: the names a frame sends on the tick it starts.
    pub events: Vec<(usize, String)>,
}

/// Milliseconds as ticks, at least one.
fn ticks_of(ms: f64) -> u32 {
    ((ms * 60.0 / 1000.0).round() as u32).max(1)
}

impl Sheet {
    /// A picture laid out in a grid of `w` × `h` frames, row by row, each showing `ticks` ticks.
    /// Tags and events are added with `tag` and `event`.
    pub fn grid(image: &Image, w: usize, h: usize, ticks: u32) -> Sheet {
        let frames = image.frames(w, h);
        Sheet {
            ticks: vec![ticks.max(1); frames.len()],
            frames,
            tags: Vec::new(),
            slices: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Names a run of frames.
    pub fn tag(mut self, name: &str, from: usize, to: usize, direction: Direction) -> Sheet {
        self.tags.push(Tag {
            name: name.into(),
            from,
            to,
            direction,
            repeat: None,
        });
        self
    }

    /// An event the frame sends on the tick it starts.
    pub fn event(mut self, frame: usize, name: &str) -> Sheet {
        self.events.push((frame, name.into()));
        self
    }

    /// Reads Aseprite's JSON for a sheet, with the sheet's picture. Trimmed frames are put back
    /// in their place; durations in milliseconds become ticks (100 ms: 6 ticks).
    pub fn aseprite(json: &str, sheet: &Image) -> Result<Sheet, String> {
        let j = Json::parse(json)?;
        let num = |v: &Json, key: &str, what: &str| -> Result<i64, String> {
            v.get(key)
                .and_then(Json::as_i64)
                .ok_or(format!("{what} has no whole number {key:?}"))
        };
        let rect = |v: Option<&Json>, what: &str| -> Result<Bounds, String> {
            let v = v.ok_or(format!("{what} is missing"))?;
            Ok(Bounds {
                x: num(v, "x", what)? as i32,
                y: num(v, "y", what)? as i32,
                w: num(v, "w", what)? as i32,
                h: num(v, "h", what)? as i32,
            })
        };
        let frames_json = j.get("frames").ok_or("no \"frames\"")?.items();
        let (mut frames, mut ticks) = (Vec::new(), Vec::new());
        for (k, f) in frames_json.iter().enumerate() {
            let what = format!("frame {k}");
            if f.get("rotated").and_then(Json::as_bool) == Some(true) {
                return Err(format!(
                    "{what} is rotated: export the sheet without rotation"
                ));
            }
            let at = rect(f.get("frame"), &format!("{what}'s \"frame\""))?;
            let size = f
                .get("sourceSize")
                .ok_or(format!("{what} has no \"sourceSize\""))?;
            let (sw, sh) = (
                num(size, "w", &what)? as usize,
                num(size, "h", &what)? as usize,
            );
            let offset = match f.get("spriteSourceSize") {
                Some(s) => rect(Some(s), &what)?,
                None => Bounds {
                    x: 0,
                    y: 0,
                    w: at.w,
                    h: at.h,
                },
            };
            let part = sheet.part(at.x, at.y, at.w.max(0) as usize, at.h.max(0) as usize);
            let mut frame = Image::new(sw, sh);
            for y in 0..part.height {
                for x in 0..part.width {
                    let (tx, ty) = (x as i32 + offset.x, y as i32 + offset.y);
                    if tx >= 0 && ty >= 0 && (tx as usize) < sw && (ty as usize) < sh {
                        frame.pixels[ty as usize * sw + tx as usize] =
                            part.pixels[y * part.width + x];
                    }
                }
            }
            frames.push(frame);
            ticks.push(ticks_of(
                f.get("duration").and_then(Json::as_f64).unwrap_or(100.0),
            ));
        }
        let meta = j.get("meta");
        let mut tags = Vec::new();
        for t in meta
            .and_then(|m| m.get("frameTags"))
            .map(Json::items)
            .unwrap_or_default()
        {
            let name = t
                .get("name")
                .and_then(Json::as_str)
                .ok_or("a tag without a name")?;
            let what = format!("tag {name:?}");
            let (from, to) = (
                num(t, "from", &what)? as usize,
                num(t, "to", &what)? as usize,
            );
            if from > to || to >= frames.len() {
                return Err(format!(
                    "{what} runs from frame {from} to {to}, past the sheet's {} frames",
                    frames.len()
                ));
            }
            let direction = match t
                .get("direction")
                .and_then(Json::as_str)
                .unwrap_or("forward")
            {
                "forward" => Direction::Forward,
                "reverse" => Direction::Reverse,
                "pingpong" => Direction::PingPong,
                "pingpong_reverse" => Direction::PingPongReverse,
                other => return Err(format!("{what} has an unknown direction {other:?}")),
            };
            // Aseprite writes the repeat count as a text ("2"), sometimes as a number
            let repeat = t
                .get("repeat")
                .and_then(|r| {
                    r.as_str()
                        .and_then(|s| s.parse().ok())
                        .or(r.as_i64().map(|n| n as u32))
                })
                .filter(|&n| n > 0);
            tags.push(Tag {
                name: name.into(),
                from,
                to,
                direction,
                repeat,
            });
        }
        let mut slices = Vec::new();
        for s in meta
            .and_then(|m| m.get("slices"))
            .map(Json::items)
            .unwrap_or_default()
        {
            let name = s
                .get("name")
                .and_then(Json::as_str)
                .ok_or("a slice without a name")?;
            let mut keys = Vec::new();
            for k in s.get("keys").map(Json::items).unwrap_or_default() {
                let what = format!("slice {name:?}");
                keys.push((
                    num(k, "frame", &what)? as usize,
                    rect(k.get("bounds"), &what)?,
                ));
            }
            keys.sort_by_key(|k| k.0);
            slices.push(Slice {
                name: name.into(),
                keys,
            });
        }
        if frames.is_empty() {
            return Err("a sheet without frames".into());
        }
        Ok(Sheet {
            frames,
            ticks,
            tags,
            slices,
            events: Vec::new(),
        })
    }

    /// Reads an Aseprite sheet from its JSON file; the picture is the one its `meta.image`
    /// names, next to the JSON file. Errors name the file.
    pub fn load_aseprite(json_path: impl AsRef<std::path::Path>) -> Result<Sheet, String> {
        let path = json_path.as_ref();
        let named = |e: String| format!("{}: {e}", path.display());
        let text = std::fs::read_to_string(path).map_err(|e| named(e.to_string()))?;
        let j = Json::parse(&text).map_err(named)?;
        let image_name = j
            .get("meta")
            .and_then(|m| m.get("image"))
            .and_then(Json::as_str)
            .ok_or_else(|| {
                named("no \"meta\": {\"image\": …} naming the sheet's picture".into())
            })?;
        let image = Image::load(path.with_file_name(image_name))?;
        Sheet::aseprite(&text, &image).map_err(named)
    }

    /// The tag of this name, by index.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.tags.iter().position(|t| t.name == name)
    }

    /// The box of a slice in a frame (its last key at or before the frame), if it has one there.
    pub fn slice(&self, name: &str, frame: usize) -> Option<Bounds> {
        let s = self.slices.iter().find(|s| s.name == name)?;
        s.keys
            .iter()
            .rev()
            .find(|(f, _)| *f <= frame)
            .map(|(_, b)| *b)
    }
}

/// Plays a sheet's tags a tick at a time. Plain data: keep it in the game's state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Anim {
    /// The tag playing (its index), if any.
    tag: Option<usize>,
    /// The frame shown (its index in the sheet).
    frame: usize,
    /// Ticks the frame has been shown, and whether it is new (its events not sent yet).
    shown: u32,
    fresh: bool,
    /// Where in the tag's run the frame is (an index into its sequence), and the plays done.
    step: usize,
    plays: u32,
    done: bool,
}

/// The frames a tag shows, in order, for one play.
fn sequence(t: &Tag) -> Vec<usize> {
    let up: Vec<usize> = (t.from..=t.to).collect();
    let down: Vec<usize> = up.iter().rev().copied().collect();
    let there_and_back = |a: &[usize], b: &[usize]| {
        let mut v = a.to_vec();
        if b.len() > 2 {
            v.extend_from_slice(&b[1..b.len() - 1]);
        }
        v
    };
    match t.direction {
        Direction::Forward => up,
        Direction::Reverse => down,
        Direction::PingPong => there_and_back(&up, &down),
        Direction::PingPongReverse => there_and_back(&down, &up),
    }
}

impl Anim {
    pub fn new() -> Anim {
        Anim::default()
    }

    /// Plays a tag from its start, unless it is playing already (so calling this every tick is
    /// fine). False if the sheet has no such tag (nothing changes then).
    pub fn play(&mut self, sheet: &Sheet, tag: &str) -> bool {
        let Some(k) = sheet.find(tag) else {
            return false;
        };
        if self.tag != Some(k) {
            self.start(sheet, k);
        }
        true
    }

    /// Plays a tag from its start, even when it is playing.
    pub fn restart(&mut self, sheet: &Sheet, tag: &str) -> bool {
        let Some(k) = sheet.find(tag) else {
            return false;
        };
        self.start(sheet, k);
        true
    }

    fn start(&mut self, sheet: &Sheet, k: usize) {
        let first = sequence(&sheet.tags[k])[0];
        *self = Anim {
            tag: Some(k),
            frame: first,
            shown: 0,
            fresh: true,
            step: 0,
            plays: 0,
            done: false,
        };
    }

    /// One tick. Returns the events of a frame shown for the first time this tick.
    pub fn tick(&mut self, sheet: &Sheet) -> Vec<String> {
        let Some(k) = self.tag else {
            return Vec::new();
        };
        if self.fresh {
            self.fresh = false;
            self.shown = 1;
            return self.events(sheet);
        }
        self.shown += 1;
        let ticks = sheet.ticks.get(self.frame).copied().unwrap_or(1);
        if self.shown <= ticks || self.done {
            return Vec::new();
        }
        let tag = &sheet.tags[k];
        let seq = sequence(tag);
        if self.step + 1 < seq.len() {
            self.step += 1;
        } else {
            self.plays += 1;
            if tag.repeat.is_some_and(|n| self.plays >= n) {
                // played out: stays on its last frame
                self.done = true;
                return Vec::new();
            }
            self.step = 0;
        }
        self.frame = seq[self.step];
        self.shown = 1;
        self.events(sheet)
    }

    fn events(&self, sheet: &Sheet) -> Vec<String> {
        sheet
            .events
            .iter()
            .filter(|(f, _)| *f == self.frame)
            .map(|(_, e)| e.clone())
            .collect()
    }

    /// The frame to show, as an index into the sheet's frames.
    pub fn frame(&self) -> usize {
        self.frame
    }

    /// The picture to show.
    pub fn image<'a>(&self, sheet: &'a Sheet) -> &'a Image {
        &sheet.frames[self.frame.min(sheet.frames.len() - 1)]
    }

    /// The tag playing.
    pub fn playing<'a>(&self, sheet: &'a Sheet) -> Option<&'a str> {
        self.tag.map(|k| sheet.tags[k].name.as_str())
    }

    /// A tag with a repeat count that has played out (it stays on its last frame).
    pub fn finished(&self) -> bool {
        self.done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sheet of `n` frames of 2 × 1, frame k filled with colour k.
    fn strip(n: usize) -> Image {
        let mut img = Image::new(2 * n, 1);
        for (i, p) in img.pixels.iter_mut().enumerate() {
            *p = 0xff00_0000 | (i / 2) as u32;
        }
        img
    }

    /// The frames shown on ticks 1..=n.
    fn shown(anim: &mut Anim, sheet: &Sheet, n: usize) -> Vec<usize> {
        (0..n)
            .map(|_| {
                anim.tick(sheet);
                anim.frame()
            })
            .collect()
    }

    #[test]
    fn tags_run_their_way_for_their_ticks() {
        let sheet = Sheet::grid(&strip(4), 2, 1, 2)
            .tag("fwd", 0, 2, Direction::Forward)
            .tag("rev", 1, 3, Direction::Reverse)
            .tag("pp", 0, 3, Direction::PingPong);
        let mut a = Anim::new();
        assert!(a.play(&sheet, "fwd"));
        assert_eq!(shown(&mut a, &sheet, 8), [0, 0, 1, 1, 2, 2, 0, 0]);
        a.play(&sheet, "rev");
        assert_eq!(shown(&mut a, &sheet, 7), [3, 3, 2, 2, 1, 1, 3]);
        a.play(&sheet, "pp");
        assert_eq!(
            shown(&mut a, &sheet, 14),
            [0, 0, 1, 1, 2, 2, 3, 3, 2, 2, 1, 1, 0, 0]
        );
        assert!(!a.play(&sheet, "nope"));
        assert_eq!(
            a.playing(&sheet),
            Some("pp"),
            "an unknown tag changes nothing"
        );
    }

    #[test]
    fn playing_the_same_tag_goes_on_and_restart_starts_over() {
        let sheet = Sheet::grid(&strip(3), 2, 1, 1).tag("a", 0, 2, Direction::Forward);
        let mut a = Anim::new();
        a.play(&sheet, "a");
        shown(&mut a, &sheet, 2);
        a.play(&sheet, "a");
        assert_eq!(shown(&mut a, &sheet, 1), [2], "not started over");
        a.restart(&sheet, "a");
        assert_eq!(shown(&mut a, &sheet, 1), [0]);
    }

    #[test]
    fn a_repeat_count_plays_out_and_stays_on_the_last_frame() {
        let mut sheet = Sheet::grid(&strip(3), 2, 1, 1).tag("swing", 0, 2, Direction::Forward);
        sheet.tags[0].repeat = Some(2);
        let mut a = Anim::new();
        a.play(&sheet, "swing");
        assert_eq!(shown(&mut a, &sheet, 8), [0, 1, 2, 0, 1, 2, 2, 2]);
        assert!(a.finished());
    }

    #[test]
    fn events_fire_on_the_first_tick_of_their_frame() {
        let sheet = Sheet::grid(&strip(4), 2, 1, 3)
            .tag("run", 0, 3, Direction::Forward)
            .event(1, "step")
            .event(3, "step")
            .event(0, "start");
        let mut a = Anim::new();
        a.play(&sheet, "run");
        let fired: Vec<(usize, Vec<String>)> = (1..=13)
            .map(|t| (t, a.tick(&sheet)))
            .filter(|(_, e)| !e.is_empty())
            .collect();
        assert_eq!(
            fired,
            [
                (1, vec!["start".to_string()]),
                (4, vec!["step".to_string()]),
                (10, vec!["step".to_string()]),
                (13, vec!["start".to_string()]),
            ]
        );
    }

    #[test]
    fn an_aseprite_sheet_reads_with_its_trims_durations_tags_and_slices() {
        // frame 1 is trimmed: 1 × 1 at (1, 0) of a 2 × 1 frame
        let json = r#"{ "frames": {
            "hero 0.aseprite": { "frame": {"x":0,"y":0,"w":2,"h":1}, "rotated": false, "trimmed": false,
                "spriteSourceSize": {"x":0,"y":0,"w":2,"h":1}, "sourceSize": {"w":2,"h":1}, "duration": 100 },
            "hero 1.aseprite": { "frame": {"x":2,"y":0,"w":1,"h":1}, "rotated": false, "trimmed": true,
                "spriteSourceSize": {"x":1,"y":0,"w":1,"h":1}, "sourceSize": {"w":2,"h":1}, "duration": 250 },
            "hero 2.aseprite": { "frame": {"x":4,"y":0,"w":2,"h":1}, "rotated": false, "trimmed": false,
                "spriteSourceSize": {"x":0,"y":0,"w":2,"h":1}, "sourceSize": {"w":2,"h":1}, "duration": 16 }
          },
          "meta": { "app": "https://www.aseprite.org/", "image": "hero.png",
            "frameTags": [ { "name": "idle", "from": 0, "to": 1, "direction": "pingpong", "repeat": "3" },
                           { "name": "hit", "from": 2, "to": 2, "direction": "forward" } ],
            "slices": [ { "name": "hitbox", "keys": [ { "frame": 1, "bounds": {"x":0,"y":0,"w":2,"h":1} },
                                                       { "frame": 0, "bounds": {"x":1,"y":0,"w":1,"h":1} } ] } ] } }"#;
        let sheet = Sheet::aseprite(json, &strip(3)).unwrap();
        assert_eq!(sheet.frames.len(), 3);
        assert_eq!(sheet.ticks, [6, 15, 1], "100 ms, 250 ms, 16 ms");
        assert_eq!(
            sheet.frames[1].pixels,
            [0, 0xff00_0001],
            "the trimmed pixel back in its place"
        );
        assert_eq!(sheet.tags[0].direction, Direction::PingPong);
        assert_eq!(sheet.tags[0].repeat, Some(3));
        assert_eq!(sheet.find("hit"), Some(1));
        assert_eq!(sheet.slice("hitbox", 0).map(|b| b.x), Some(1));
        assert_eq!(
            sheet.slice("hitbox", 2).map(|b| b.w),
            Some(2),
            "a key holds from its frame on"
        );
        assert_eq!(sheet.slice("nope", 0), None);
        // broken files say what is wrong
        let err = Sheet::aseprite(&json.replace("\"to\": 1", "\"to\": 9"), &strip(3)).unwrap_err();
        assert!(err.contains("past the sheet"), "{err}");
        let err = Sheet::aseprite(
            &json.replace(
                "\"rotated\": false, \"trimmed\": true",
                "\"rotated\": true, \"trimmed\": true",
            ),
            &strip(3),
        )
        .unwrap_err();
        assert!(err.contains("rotated"), "{err}");
    }
}
