// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! LANTERN: carry a lantern through a dark cave, gather the five sparks, and the door at the end
//! opens. Polter's example game, made with the engine alone: pictures from PNG files and an
//! Aseprite sheet (`polter::image`, `polter::anim`), a level as text with a camera
//! (`polter::tiles`), actions bound in a text file (`polter::action`), the built-in font
//! (`polter::text`), maths that is the same on every machine (`polter::math`).
//!
//! The pictures are drawn as text in `tools/art.py`; the level is `assets/level.txt`.

use polter::action::Actions;
use polter::anim::{Anim, Sheet};
use polter::image::Image;
use polter::text::Font;
use polter::tiles::{Camera, TileMap};
use polter::{Canvas, Ctx, Game, Invariants, Knob, Sfx, State, Wave, math};
use std::sync::OnceLock;

const W: usize = 320;
const H: usize = 180;
const TILE: usize = 16;
/// The level: `#` rock, `=` rock with moss, `*` crystals, `D` the door (two high), `o` a spark,
/// `@` where the lantern bearer starts, `.` the open cave.
pub const LEVEL: &str = include_str!("../assets/level.txt");
const KEYS: &str = include_str!("../assets/keys.txt");

// tiles in assets/tiles.png
const ROCK: u16 = 0;
const MOSS: u16 = 1;
const CRYSTAL: u16 = 2;
const DOOR_TOP: u16 = 3;
const DOOR_BOTTOM: u16 = 4;
const OPEN_TOP: u16 = 5;
const OPEN_BOTTOM: u16 = 6;

/// The lantern bearer's box, and where the sprite sits around it.
const BOX: (f32, f32) = (8.0, 13.0);
const SPRITE: (f32, f32) = (-4.0, -3.0);

const JUMP: Sfx = Sfx::new("jump", Wave::Square, 330.0, 620.0, 7, 0.16);
const LAND: Sfx = Sfx::new("land", Wave::Noise, 900.0, 200.0, 6, 0.22);
const SPARK: Sfx = Sfx::new("spark", Wave::Triangle, 1320.0, 1980.0, 14, 0.24);
const DOOR: Sfx = Sfx::new("door", Wave::Saw, 90.0, 60.0, 40, 0.2);
const WIN: Sfx = Sfx::new("win", Wave::Triangle, 660.0, 990.0, 50, 0.22).held();

/// The pictures, read once from the bytes built into the program.
struct Art {
    hero: Sheet,
    tiles: Vec<Image>,
    spark: Vec<Image>,
}

fn art() -> &'static Art {
    static ART: OnceLock<Art> = OnceLock::new();
    ART.get_or_init(|| {
        let picture = |bytes: &[u8], name: &str| {
            Image::from_png(bytes).unwrap_or_else(|e| panic!("assets/{name}: {e}"))
        };
        let hero = picture(include_bytes!("../assets/hero.png"), "hero.png");
        Art {
            hero: Sheet::aseprite(include_str!("../assets/hero.json"), &hero)
                .unwrap_or_else(|e| panic!("assets/hero.json: {e}")),
            tiles: picture(include_bytes!("../assets/tiles.png"), "tiles.png").frames(TILE, TILE),
            spark: picture(include_bytes!("../assets/spark.png"), "spark.png").frames(8, 8),
        }
    })
}

/// The numbers the lantern bearer moves by: knobs in the workshop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tune {
    pub run: f32,
    pub accel: f32,
    pub air: f32,
    pub gravity: f32,
    pub jump: f32,
    pub max_fall: f32,
    /// Ticks after leaving an edge that a jump still works, and ticks a jump pressed early waits.
    pub coyote: f32,
    pub buffer: f32,
}

impl Default for Tune {
    fn default() -> Tune {
        Tune {
            run: 1.5,
            accel: 0.25,
            air: 0.12,
            gravity: 0.24,
            jump: 4.5,
            max_fall: 5.0,
            coyote: 6.0,
            buffer: 6.0,
        }
    }
}

/// A spark to gather: its middle.
#[derive(Clone, Debug, PartialEq)]
pub struct Spark {
    pub x: f32,
    pub y: f32,
    pub taken: bool,
}

/// A mote of light thrown off when a spark is gathered.
#[derive(Clone, Debug, PartialEq)]
struct Mote {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: u32,
}

#[derive(Clone)]
pub struct Lantern {
    pub map: TileMap,
    actions: Actions,
    pub tune: Tune,
    /// The box's top left, and its speed.
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub grounded: bool,
    coyote: u32,
    buffer: u32,
    facing_left: bool,
    anim: Anim,
    pub sparks: Vec<Spark>,
    /// The door's top tile.
    pub door: (i32, i32),
    pub open: bool,
    pub won: bool,
    cam: Camera,
    ticks: u64,
    motes: Vec<Mote>,
    shake: f32,
}

impl Default for Lantern {
    fn default() -> Self {
        Self::new()
    }
}

impl Lantern {
    pub fn new() -> Lantern {
        let legend = [
            ('#', Some(ROCK)),
            ('=', Some(MOSS)),
            ('*', Some(CRYSTAL)),
            ('D', Some(DOOR_TOP)),
            ('.', None),
            ('o', None),
            ('@', None),
        ];
        let mut map = TileMap::from_text(LEVEL, TILE, &legend)
            .unwrap_or_else(|e| panic!("assets/level.txt: {e}"));
        // rock, moss, crystals and the closed door block; the open door doesn't
        map.solid = vec![true, true, true, true, true, false, false];
        let (mut start, mut sparks, mut door) = ((0.0, 0.0), Vec::new(), None);
        for (ty, line) in LEVEL.lines().enumerate() {
            for (tx, ch) in line.chars().enumerate() {
                let (cx, cy) = ((tx * TILE + TILE / 2) as f32, (ty * TILE + TILE / 2) as f32);
                match ch {
                    '@' => start = (cx - BOX.0 / 2.0, (ty * TILE) as f32 + TILE as f32 - BOX.1),
                    'o' => sparks.push(Spark {
                        x: cx,
                        y: cy,
                        taken: false,
                    }),
                    'D' if door.is_none() => door = Some((tx as i32, ty as i32)),
                    _ => {}
                }
            }
        }
        let door = door.expect("assets/level.txt has a door, D, two tiles high");
        map.set(door.0, door.1 + 1, Some(DOOR_BOTTOM));
        let mut cam = Camera::new(W, H);
        cam.follow(start.0, start.1, 1.0);
        cam.keep_inside((map.width * TILE) as f32, (map.height * TILE) as f32);
        Lantern {
            map,
            actions: Actions::parse(KEYS).unwrap_or_else(|e| panic!("assets/keys.txt: {e}")),
            tune: Tune::default(),
            x: start.0,
            y: start.1,
            vx: 0.0,
            vy: 0.0,
            grounded: false,
            coyote: 0,
            buffer: 0,
            facing_left: false,
            anim: Anim::new(),
            sparks,
            door,
            open: false,
            won: false,
            cam,
            ticks: 0,
            motes: Vec::new(),
            shake: 0.0,
        }
    }

    pub fn gathered(&self) -> usize {
        self.sparks.iter().filter(|s| s.taken).count()
    }

    fn touches(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.x < x + w && x < self.x + BOX.0 && self.y < y + h && y < self.y + BOX.1
    }

    /// Where the lantern's light is: the sheet's slice "light", mirrored when facing left.
    fn light(&self) -> (f32, f32) {
        let b = art()
            .hero
            .slice("light", self.anim.frame())
            .map_or((12, 8), |b| (b.x, b.y));
        let lx = if self.facing_left { 15 - b.0 } else { b.0 };
        (
            self.x + SPRITE.0 + lx as f32 + 0.5,
            self.y + SPRITE.1 + b.1 as f32 + 0.5,
        )
    }
}

impl Game for Lantern {
    fn update(&mut self, ctx: &mut Ctx) {
        self.ticks += 1;
        let t = self.tune;
        let input = ctx.input;
        let actions = &self.actions;
        // walking, with less grip in the air
        let dir = if self.won {
            0
        } else {
            actions.axis(&input, "left", "right")
        };
        let grip = if self.grounded { t.accel } else { t.air };
        self.vx += (dir as f32 * t.run - self.vx).clamp(-grip, grip);
        if dir != 0 {
            self.facing_left = dir < 0;
        }
        // jumping: pressed a little early, or a little after leaving an edge, still counts
        self.buffer = if !self.won && actions.pressed(&input, "jump") {
            t.buffer as u32
        } else {
            self.buffer.saturating_sub(1)
        };
        self.coyote = if self.grounded {
            t.coyote as u32
        } else {
            self.coyote.saturating_sub(1)
        };
        if self.buffer > 0 && self.coyote > 0 {
            self.vy = -t.jump;
            self.buffer = 0;
            self.coyote = 0;
            self.grounded = false;
            ctx.play(JUMP);
        }
        // let go early: a shorter jump
        if actions.released(&input, "jump") && self.vy < 0.0 {
            self.vy *= 0.5;
        }
        self.vy = (self.vy + t.gravity).min(t.max_fall);
        let falling = self.vy;
        let m = self
            .map
            .move_box(self.x, self.y, BOX.0, BOX.1, self.vx, self.vy);
        (self.x, self.y) = (m.x, m.y);
        if m.hit_x {
            self.vx = 0.0;
        }
        self.grounded = m.hit_y && falling > 0.0;
        if m.hit_y {
            if self.grounded && falling > 3.6 {
                self.shake = 1.5;
                ctx.play(LAND);
            }
            self.vy = 0.0;
        }
        // the sparks
        for k in 0..self.sparks.len() {
            let s = self.sparks[k].clone();
            if !s.taken && self.touches(s.x - 3.0, s.y - 3.0, 6.0, 6.0) {
                self.sparks[k].taken = true;
                ctx.play(SPARK);
                for _ in 0..14 {
                    let a = ctx.rng.float() * std::f32::consts::TAU;
                    let v = 0.4 + ctx.rng.float() * 1.2;
                    self.motes.push(Mote {
                        x: s.x,
                        y: s.y,
                        vx: math::cos(a) * v,
                        vy: math::sin(a) * v - 0.4,
                        life: 24 + ctx.rng.range(0, 16) as u32,
                    });
                }
            }
        }
        if !self.open && self.gathered() == self.sparks.len() {
            self.open = true;
            self.map.set(self.door.0, self.door.1, Some(OPEN_TOP));
            self.map
                .set(self.door.0, self.door.1 + 1, Some(OPEN_BOTTOM));
            self.shake = 2.5;
            ctx.play(DOOR);
        }
        let (dx, dy) = (
            (self.door.0 * TILE as i32) as f32,
            (self.door.1 * TILE as i32) as f32,
        );
        if self.open && !self.won && self.touches(dx + 4.0, dy, 8.0, 2.0 * TILE as f32) {
            self.won = true;
            ctx.play(WIN);
        }
        // the picture of the lantern bearer
        let tag = if !self.grounded {
            if self.vy < 0.0 { "jump" } else { "fall" }
        } else if self.vx.abs() > 0.3 {
            "run"
        } else {
            "idle"
        };
        self.anim.play(&art().hero, tag);
        self.anim.tick(&art().hero);
        // the camera follows, softly, and stays in the cave
        self.cam
            .follow(self.x + BOX.0 / 2.0, self.y + BOX.1 / 2.0, 0.12);
        self.cam.keep_inside(
            (self.map.width * TILE) as f32,
            (self.map.height * TILE) as f32,
        );
        for m in &mut self.motes {
            m.x += m.vx;
            m.y += m.vy;
            m.vy += 0.03;
            m.vx *= 0.96;
            m.life = m.life.saturating_sub(1);
        }
        self.motes.retain(|m| m.life > 0);
        self.shake *= 0.82;
    }

    fn draw(&self, c: &mut Canvas, ctx: &Ctx) {
        if !ctx.picture {
            return;
        }
        let art = art();
        let (mut vx, mut vy) = self.cam.view();
        if self.shake > 0.3 {
            let k = self.shake.round() as i32;
            vx += [k, -k, 0, k][(self.ticks % 4) as usize];
            vy += [0, k, -k, -k][(self.ticks % 4) as usize];
        }
        // the cave's back wall: rough stones, at half the speed of the rock in front
        for py in 0..H {
            for px in 0..W {
                let (wx, wy) = ((px as i32 + vx / 2) / 3, (py as i32 + vy / 2) / 3);
                let n = hash(wx, wy) & 7;
                let base = [
                    0x10_13_1c, 0x12_15_20, 0x14_18_24, 0x11_14_1e, 0x13_16_22, 0x10_12_1a,
                    0x16_1a_28, 0x12_15_21,
                ][n as usize];
                c.set(px as i32, py as i32, base);
            }
        }
        self.map.draw(c, &art.tiles, (vx, vy));
        // the sparks, turning and bobbing
        for (k, s) in self.sparks.iter().enumerate().filter(|(_, s)| !s.taken) {
            let frame = &art.spark[((self.ticks / 6) as usize + k) % art.spark.len()];
            let bob = math::sin(self.ticks as f32 * 0.07 + k as f32) * 2.0;
            c.draw(frame, s.x as i32 - 4 - vx, (s.y + bob) as i32 - 4 - vy);
        }
        // the lantern bearer
        let (sx, sy) = (
            (self.x + SPRITE.0) as i32 - vx,
            (self.y + SPRITE.1) as i32 - vy,
        );
        let img = self.anim.image(&art.hero);
        if self.facing_left {
            c.draw_flipped(img, sx, sy);
        } else {
            c.draw(img, sx, sy);
        }
        for m in &self.motes {
            let a = (m.life.min(20) as f32 / 20.0 * 255.0) as u32;
            let mut dot = Image::new(1, 1);
            dot.pixels[0] = (a << 24) | 0xfff1b8;
            c.draw(&dot, m.x as i32 - vx, m.y as i32 - vy);
        }
        // light: the cave is dark but for the lantern, the sparks and the open door
        let flicker =
            math::sin(self.ticks as f32 * 0.21) * 2.0 + math::sin(self.ticks as f32 * 0.53);
        let (lx, ly) = self.light();
        let mut lights = vec![(lx, ly, 96.0 + flicker, [1.05, 0.74, 0.42])];
        for (k, s) in self.sparks.iter().enumerate().filter(|(_, s)| !s.taken) {
            let pulse = 28.0 + math::sin(self.ticks as f32 * 0.09 + k as f32) * 3.0;
            lights.push((s.x, s.y, pulse, [1.0, 0.76, 0.34]));
        }
        if self.open {
            let (dx, dy) = (
                (self.door.0 as f32 + 0.5) * TILE as f32,
                (self.door.1 as f32 + 1.0) * TILE as f32,
            );
            lights.push((dx, dy, 60.0, [0.55, 0.8, 1.0]));
        }
        let px = c.pixels_mut();
        for py in 0..H {
            for pxx in 0..W {
                let (wx, wy) = ((pxx as i32 + vx) as f32, (py as i32 + vy) as f32);
                let mut l = [0.13f32, 0.15, 0.22];
                for &(x, y, r, tint) in &lights {
                    let d2 = (wx - x) * (wx - x) + (wy - y) * (wy - y);
                    let f = 1.0 - d2 / (r * r);
                    if f > 0.0 {
                        for (ch, t) in l.iter_mut().zip(tint) {
                            *ch += f * f * t * 1.7;
                        }
                    }
                }
                let i = py * W + pxx;
                let p = px[i];
                let lit = |s: u32, k: f32| ((((p >> s) & 255) as f32 * k).min(255.0) as u32) << s;
                px[i] = lit(16, l[0]) | lit(8, l[1]) | lit(0, l[2]);
            }
        }
        // what the player knows, unlit
        let font = Font::standard();
        let line = format!("Sparks {}/{}", self.gathered(), self.sparks.len());
        c.rect(4, 4, font.width(&line) + 8, 15, 0x0b0d14);
        font.draw(c, &line, 8, 7, 0xffe08a);
        if self.won {
            let lines = ["The door is open.", "You carried the light through."];
            let w = lines.iter().map(|l| font.width(l)).max().unwrap_or(0) + 16;
            let (bx, by) = ((W - w) as i32 / 2, 64);
            c.rect(bx, by, w, 38, 0x0b0d14);
            for (k, l) in lines.iter().enumerate() {
                let x = (W - font.width(l)) as i32 / 2;
                font.draw(
                    c,
                    l,
                    x,
                    by + 8 + 13 * k as i32,
                    if k == 0 { 0xffe08a } else { 0xf4e4b8 },
                );
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        (W, H)
    }

    fn state(&self) -> State {
        State::new()
            .num("x", self.x)
            .num("y", self.y)
            .num("vx", self.vx)
            .num("vy", self.vy)
            .flag("grounded", self.grounded)
            .int("sparks", self.gathered() as i64)
            .flag("open", self.open)
            .flag("won", self.won)
            .int("ticks", self.ticks as i64)
    }

    fn invariants(&self, check: &mut Invariants) {
        check.require(
            "the lantern bearer is never inside the rock",
            !self.map.overlaps(self.x, self.y, BOX.0, BOX.1),
        );
        check.require(
            "the door opens only when every spark is gathered",
            !self.open || self.gathered() == self.sparks.len(),
        );
        check.require("only an open door is gone through", !self.won || self.open);
    }

    fn knobs(&self) -> Vec<Knob> {
        let t = &self.tune;
        vec![
            Knob::new("run", t.run, 0.5, 4.0, 0.05),
            Knob::new("accel", t.accel, 0.02, 1.0, 0.01),
            Knob::new("air", t.air, 0.02, 1.0, 0.01),
            Knob::new("gravity", t.gravity, 0.05, 0.8, 0.01),
            Knob::new("jump", t.jump, 2.0, 8.0, 0.05),
            Knob::new("max_fall", t.max_fall, 1.0, 10.0, 0.1),
            Knob::new("coyote", t.coyote, 0.0, 15.0, 1.0),
            Knob::new("buffer", t.buffer, 0.0, 15.0, 1.0),
        ]
    }

    fn set_knob(&mut self, name: &str, value: f32) {
        let t = &mut self.tune;
        match name {
            "run" => t.run = value,
            "accel" => t.accel = value,
            "air" => t.air = value,
            "gravity" => t.gravity = value,
            "jump" => t.jump = value,
            "max_fall" => t.max_fall = value,
            "coyote" => t.coyote = value,
            "buffer" => t.buffer = value,
            _ => {}
        }
    }

    fn snapshot(&self) -> Option<Box<dyn Game>> {
        Some(Box::new(self.clone()))
    }
}

/// A number from a cell, the same every time.
fn hash(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}
