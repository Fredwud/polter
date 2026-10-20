// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! How the workshop looks: a tape deck. A bay for the game; a panel of brushed metal with a
//! nameplate, faders for the knobs, a lamp for the rules and a display for the state; below, the
//! deck: the tape as the timeline, the cassette with its reels, piano keys with lamps, a display
//! for the time, and meters for the sound and for the time a tick takes. Everything is drawn from
//! the shop's state; `Layout` says where things sit, for the drawing and for the mouse.
//!
//! Every key and every display does something or shows something ("only buttons and displays
//! that work", the person who first played it asked): nothing that looks like a control is only
//! decoration.

use super::shop::{DeckKey, Drag, Mode, Workshop};
use crate::scenario::{Value, parse_state};
use polter::canvas::Canvas;
use polter::game::Knob;
use polter::paint::colour::*;
use polter::paint::*;
use std::f32::consts::TAU;

/// The panel's width beside a game in the window it opens with, its narrowest and widest when
/// the window grows or shrinks, and folded away; the deck's height. In pixels of the workshop's
/// picture.
pub(super) const PANEL: i32 = 150;
const PANEL_MIN: i32 = 130;
const PANEL_MAX: i32 = 220;
const FOLDED: i32 = 12;
pub(super) const DECK: i32 = 64;
pub(super) const FADER_ROW: i32 = 9;
const STATE_ROW: i32 = 7;
/// A key's width (narrower only when the window is too narrow for the eight), and what each
/// thing beside the keys' well takes with its gap.
const KEY_W: i32 = 22;
const KEY_MIN: i32 = 12;
const CASSETTE_W: i32 = 72 + 5;
const DISPLAY_W: i32 = 72 + 5;
const METER_W: i32 = 54;
/// A knob's value takes eight letters.
const VALUE_W: i32 = 8 * 4 - 1;

const BODY: u32 = 0x1c1d21;
const PLATE: u32 = 0x202227;
const DECK_PLATE: u32 = 0x24252b;
const BAY: u32 = 0x0b0c0f;
const TAPE: u32 = 0x553821;
const CREAM: u32 = 0xeadfc6;
const PEN: u32 = 0x2a2420;

/// What the layout depends on besides the picture's size: the game, the panel's content, how far
/// the knobs are scrolled, whether the panel is open.
pub(super) struct Content {
    pub game: (usize, usize),
    pub knobs: usize,
    /// The longest knob name, in letters.
    pub name_chars: usize,
    pub state_lines: usize,
    pub knob_scroll: usize,
    pub panel_open: bool,
}

/// Where everything sits. The panel shares its height between knobs and state by what each
/// holds (scrolling what does not fit) and grows with the window; the deck keeps the tape and the
/// keys always and the rest as room allows: the display, then LOAD, then LEVEL, then the
/// cassette (which only repeats what the tape and the display show).
pub(super) struct Layout {
    pub w: i32,
    pub h: i32,
    /// The game's picture: top left and whole scale; and its size.
    pub view: (i32, i32, i32),
    pub game: (i32, i32),
    pub bay: Rect,
    pub panel: Rect,
    pub panel_open: bool,
    /// The button that folds the panel away and back.
    pub handle: Rect,
    pub deck: Rect,
    /// The tape: the timeline (click and drag to wind).
    pub tape: Rect,
    pub keys: [Rect; 8],
    pub cassette: Option<Rect>,
    pub display: Option<Rect>,
    /// LEVEL and LOAD.
    pub meters: [Option<Rect>; 2],
    pub faders_top: i32,
    /// The knobs with a fader on show: `faders` of them from `first` (the wheel scrolls).
    pub knobs: usize,
    pub first: usize,
    pub faders: usize,
    pub name_w: i32,
    pub rules_y: i32,
    pub state: Rect,
}

impl Layout {
    pub(super) fn new(w: usize, h: usize, c: &Content) -> Layout {
        let (w, h) = (w as i32, h as i32);
        let game = (c.game.0.max(1) as i32, c.game.1.max(1) as i32);
        let deck = Rect {
            x: 0,
            y: h - DECK,
            w,
            h: DECK,
        };
        // the panel: a third of the width within its bounds, never taking the game's room
        let panel_w = if c.panel_open {
            (w / 3)
                .clamp(PANEL_MIN, PANEL_MAX)
                .min((w - game.0 - 10).max(PANEL_MIN))
        } else {
            FOLDED
        };
        let panel = Rect {
            x: w - panel_w,
            y: 0,
            w: panel_w,
            h: h - DECK,
        };
        let handle = Rect {
            x: panel.x + if c.panel_open { 4 } else { 3 },
            y: panel.y + 4,
            w: 7,
            h: 13,
        };
        let bay = Rect {
            x: 3,
            y: 3,
            w: (panel.x - 6).max(1),
            h: (h - DECK - 6).max(1),
        };
        let s = ((bay.w - 4) / game.0).min((bay.h - 4) / game.1).max(1);
        let view = (
            bay.x + (bay.w - game.0 * s).max(0) / 2,
            bay.y + (bay.h - game.1 * s).max(0) / 2,
            s,
        );
        // the deck, by what each part is worth
        let tape = Rect {
            x: 6,
            y: deck.y + 3,
            w: w - 12,
            h: 12,
        };
        let row = deck.y + 19;
        let kw = if w - 12 >= 8 * (KEY_W + 2) + 4 {
            KEY_W
        } else {
            ((w - 12 - 4) / 8 - 2).max(KEY_MIN)
        };
        let room = w - 12 - (8 * (kw + 2) + 4);
        let mut need = 0;
        let mut fits = |more: i32| {
            let ok = room >= need + more;
            if ok {
                need += more;
            }
            ok
        };
        let display_on = fits(DISPLAY_W);
        let load_on = display_on && fits(METER_W + 6);
        let level_on = load_on && fits(METER_W + 4);
        let cassette_on = level_on && fits(CASSETTE_W);
        let keys_x = 6 + if cassette_on { CASSETTE_W } else { 0 } + 3;
        let keys = std::array::from_fn(|i| Rect {
            x: keys_x + (kw + 2) * i as i32,
            y: row + 6,
            w: kw,
            h: 18,
        });
        let mut right = w - 6;
        let at = |right: i32, width: i32| Rect {
            x: right - width,
            y: row,
            w: width,
            h: 42,
        };
        let load = load_on.then(|| at(right, METER_W));
        if load_on {
            right -= METER_W + if level_on { 4 } else { 6 };
        }
        let level = level_on.then(|| at(right, METER_W));
        if level_on {
            right -= METER_W + 6;
        }
        let display = display_on.then(|| at(right, 72));
        // the panel inside: knobs and state share the height by what they hold
        let faders_top = panel.y + if c.knobs > 0 { 31 } else { 22 };
        let avail = (panel.h - 6 - 11 - faders_top).max(0);
        let state_need = if c.state_lines > 0 {
            c.state_lines as i32 * STATE_ROW + 8
        } else {
            0
        };
        let faders = if !c.panel_open {
            0
        } else if c.knobs as i32 * FADER_ROW + state_need <= avail {
            c.knobs
        } else {
            let keep = state_need.min(3 * STATE_ROW + 8);
            (((avail - keep) / FADER_ROW).max(0) as usize)
                .max(2)
                .min(c.knobs)
        };
        let first = c.knob_scroll.min(c.knobs - faders);
        let rules_y = faders_top + faders as i32 * FADER_ROW + if faders > 0 { 4 } else { 0 };
        let state = Rect {
            x: panel.x + 6,
            y: rules_y + 11,
            w: panel.w - 12,
            h: if c.panel_open {
                (panel.y + panel.h - 6 - (rules_y + 11)).max(0)
            } else {
                0
            },
        };
        let name_w = ((c.name_chars.min(16) as i32) * 4 - 1)
            .min(panel.w - 12 - VALUE_W - 12 - 24)
            .max(0);
        Layout {
            w,
            h,
            view,
            game,
            bay,
            panel,
            panel_open: c.panel_open,
            handle,
            deck,
            tape,
            keys,
            cassette: cassette_on.then_some(Rect {
                x: 6,
                y: row,
                w: 72,
                h: 42,
            }),
            display,
            meters: [level, load],
            faders_top,
            knobs: c.knobs,
            first,
            faders,
            name_w,
            rules_y,
            state,
        }
    }

    /// The window for a game: the game at its own scale beside the panel and above the deck,
    /// at least 480 × 270.
    pub(super) fn window_size(game: (usize, usize)) -> (usize, usize) {
        (
            (game.0 + PANEL as usize + 10).max(480),
            (game.1 + DECK as usize + 10).max(270),
        )
    }

    pub(super) fn key_at(&self, p: (i32, i32)) -> Option<DeckKey> {
        self.keys
            .iter()
            .position(|r| Rect { h: r.h + 2, ..*r }.contains(p))
            .map(|i| DeckKey::ALL[i])
    }

    /// Knob `i`'s fader row, if it is on show.
    pub(super) fn fader_row(&self, i: usize) -> Option<Rect> {
        (i >= self.first && i < self.first + self.faders).then(|| Rect {
            x: self.panel.x + 4,
            y: self.faders_top + (i - self.first) as i32 * FADER_ROW,
            w: self.panel.w - 8,
            h: FADER_ROW,
        })
    }

    pub(super) fn fader_at(&self, p: (i32, i32)) -> Option<usize> {
        (self.first..self.first + self.faders)
            .find(|&i| self.fader_row(i).is_some_and(|r| r.contains(p)))
    }

    /// Where the faders are (the wheel scrolls them there).
    pub(super) fn faders_area(&self) -> Rect {
        Rect {
            x: self.panel.x,
            y: self.faders_top,
            w: self.panel.w,
            h: self.faders as i32 * FADER_ROW,
        }
    }

    /// A fader's travel: after the longest knob name, before the value.
    pub(super) fn slot(&self) -> (i32, i32) {
        (
            self.panel.x + 6 + self.name_w + 6,
            self.panel.x + self.panel.w - 6 - VALUE_W - 6,
        )
    }

    /// The tape's ends: frame 0 and the end of the recording (never crossed, however narrow).
    pub(super) fn tape_x(&self) -> (i32, i32) {
        let x0 = self.tape.x + 10;
        (x0, (self.tape.x + self.tape.w - 11).max(x0 + 1))
    }

    pub(super) fn tape_at(&self, frame: u64, len: u64) -> i32 {
        let (x0, x1) = self.tape_x();
        x0 + ((frame as f64 / len.max(1) as f64) * (x1 - x0) as f64).round() as i32
    }

    pub(super) fn tape_frame(&self, x: i32, len: u64) -> u64 {
        let (x0, x1) = self.tape_x();
        let t = (x.clamp(x0, x1) - x0) as f64 / (x1 - x0) as f64;
        (t * len as f64).round() as u64
    }

    /// How many lines of state the display shows.
    pub(super) fn state_rows(&self) -> usize {
        ((self.state.h - 6) / STATE_ROW).max(0) as usize
    }
}

/// Paints the workshop.
pub(super) fn draw(shop: &Workshop, c: &mut Canvas) {
    let game = &shop.bench.session.canvas;
    let knobs = shop.bench.session.game().knobs();
    let lay = shop.layout(c.width(), c.height());
    brushed(
        c,
        Rect {
            x: 0,
            y: 0,
            w: lay.w,
            h: lay.h,
        },
        BODY,
    );
    bay(c, &lay, game);
    panel(c, &lay, shop, &knobs);
    deck(c, &lay, shop);
}

// ── The bay ─────────────────────────────────────────────────────────────

fn bay(c: &mut Canvas, lay: &Layout, game: &Canvas) {
    fill(c, lay.bay, BAY);
    bevel(c, lay.bay, EDGE_DARK, EDGE_LIGHT);
    let (vx, vy, s) = lay.view;
    let frame = Rect {
        x: vx - 1,
        y: vy - 1,
        w: lay.game.0 * s + 2,
        h: lay.game.1 * s + 2,
    };
    bevel(c, frame, 0x000000, 0x1d1f24);
    super::blit_scaled(c, game, vx.max(0) as usize, vy.max(0) as usize, s as usize);
}

// ── The panel ───────────────────────────────────────────────────────────

fn panel(c: &mut Canvas, lay: &Layout, shop: &Workshop, knobs: &[Knob]) {
    let p = lay.panel;
    brushed(c, p, PLATE);
    vline(c, p.x, p.y, p.y + p.h - 1, EDGE_DARK);
    vline(c, p.x + 1, p.y, p.y + p.h - 1, EDGE_LIGHT);
    // the handle: folds the panel away (and back)
    let arrow: &[&str] = if lay.panel_open {
        &["#..", ".#.", "..#", ".#.", "#.."]
    } else {
        &["..#", ".#.", "#..", ".#.", "..#"]
    };
    small_key(c, lay.handle, arrow, shop.hover_handle);
    if !lay.panel_open {
        return;
    }
    // the nameplate
    let plate = Rect {
        x: p.x + 15,
        y: p.y + 4,
        w: p.w - 21,
        h: 13,
    };
    fill(c, plate, 0x101114);
    bevel(c, plate, EDGE_DARK, EDGE_LIGHT);
    let name: String = shop.name.to_uppercase().chars().take(16).collect();
    text(c, &name, plate.x + 4, plate.y + 4, ACCENT);
    let tag = "WORKSHOP";
    if text_w(&name) + text_w(tag) + 14 <= plate.w {
        text(
            c,
            tag,
            plate.x + plate.w - 4 - text_w(tag),
            plate.y + 4,
            SILK,
        );
    }
    // the faders
    if lay.faders > 0 {
        text(c, "KNOBS", p.x + 6, lay.faders_top - 9, SILK);
        hline(c, p.x + 30, p.x + p.w - 7, lay.faders_top - 7, 0x34363d);
    }
    let (s0, s1) = lay.slot();
    for (i, k) in knobs.iter().enumerate() {
        let Some(r) = lay.fader_row(i) else {
            continue;
        };
        let active = shop.drag == Some(Drag::Knob(i));
        let name = k.name.to_uppercase();
        let clip = Rect {
            x: p.x + 6,
            y: r.y,
            w: lay.name_w + 1,
            h: r.h,
        };
        text_in(
            c,
            &name,
            p.x + 6,
            r.y + 2,
            if active { ACCENT } else { SILK },
            1,
            clip,
        );
        // the groove, and its end stops
        hline(c, s0, s1, r.y + 3, EDGE_DARK);
        hline(c, s0, s1, r.y + 4, 0x040405);
        hline(c, s0, s1, r.y + 5, EDGE_LIGHT);
        for x in [s0 - 1, s1 + 1] {
            vline(c, x, r.y + 2, r.y + 6, 0x4a4d55);
        }
        let t = ((k.value - k.min) / (k.max - k.min).max(f32::EPSILON)).clamp(0.0, 1.0);
        let fx = s0 + (t * (s1 - s0) as f32).round() as i32;
        let line = if active { ACCENT } else { 0x26282d };
        let cap = [
            0xf1f3f6, 0xd6dbe1, 0xc2c8d0, line, 0xaab0b9, 0x959ba4, 0x6c727b,
        ];
        for (j, col) in cap.iter().enumerate() {
            hline(c, fx - 2, fx + 2, r.y + 1 + j as i32, *col);
        }
        let value: String = k.value.to_string().chars().take(8).collect();
        text(c, &value, s1 + 7, r.y + 2, AMBER);
    }
    if lay.faders < lay.knobs {
        let area = lay.faders_area();
        scrollbar(
            c,
            p.x + p.w - 3,
            area.y,
            area.y + area.h - 2,
            lay.first,
            lay.faders,
            lay.knobs,
        );
    }
    // the rules' lamp
    let broken = &shop.bench.session.broken;
    let ry = lay.rules_y;
    if broken.is_empty() {
        led(c, p.x + 6, ry + 1, GREEN, true);
        text(c, "RULES HOLD", p.x + 13, ry, INK);
    } else {
        led(c, p.x + 6, ry + 1, RED, true);
        let what: String = format!("BROKEN: {}", broken[0])
            .to_uppercase()
            .chars()
            .take(32)
            .collect();
        text(c, &what, p.x + 13, ry, RED);
    }
    // the state, in a display of its own (the wheel scrolls it)
    let st = lay.state;
    if st.h >= 14 {
        glass(c, st);
        let pairs = parse_state(&shop.bench.session.state_json()).unwrap_or_default();
        let rows = lay.state_rows();
        let skip = shop.state_scroll.min(pairs.len().saturating_sub(rows));
        let clip = Rect {
            x: st.x + 2,
            y: st.y + 2,
            w: st.w - 6,
            h: st.h - 4,
        };
        for (i, (k, v)) in pairs.iter().skip(skip).take(rows).enumerate() {
            let y = st.y + 4 + i as i32 * STATE_ROW;
            let v = match v {
                Value::Text(t) => t.clone(),
                other => other.to_string(),
            };
            text_in(c, &k.to_uppercase(), st.x + 4, y, TEAL_DIM, 1, clip);
            let vx = st.x + 4 + 15 * 4;
            text_in(c, &v.to_uppercase(), vx, y, TEAL, 1, clip);
        }
        if pairs.len() > rows && rows > 0 {
            vline(c, st.x + st.w - 3, st.y + 3, st.y + st.h - 4, 0x0d201d);
            scrollbar(
                c,
                st.x + st.w - 3,
                st.y + 3,
                st.y + st.h - 4,
                skip,
                rows,
                pairs.len(),
            );
        }
    }
}

// ── The deck ────────────────────────────────────────────────────────────

fn deck(c: &mut Canvas, lay: &Layout, shop: &Workshop) {
    let d = lay.deck;
    brushed(c, d, DECK_PLATE);
    hline(c, d.x, d.x + d.w - 1, d.y, EDGE_DARK);
    hline(c, d.x, d.x + d.w - 1, d.y + 1, EDGE_LIGHT);
    tape(c, lay, shop);
    if let Some(r) = lay.cassette {
        cassette(c, r, shop);
    }
    keys(c, lay, shop);
    if let Some(r) = lay.display {
        display(c, r, shop);
    }
    let lit = shop.mode != Mode::Paused;
    if let Some(r) = lay.meters[0] {
        meter(c, r, shop.meters.level, "VU", "LEVEL", 0.8, lit);
    }
    if let Some(r) = lay.meters[1] {
        meter(c, r, shop.meters.load, "MS", "LOAD", 0.75, lit);
    }
}

/// The tape across the deck: the recording from its start to its end (a line marks each), with
/// the head where the game is, the marks as flags above and knob changes as dots below.
fn tape(c: &mut Canvas, lay: &Layout, shop: &Workshop) {
    let t = lay.tape;
    let cy = t.y + 6;
    let len = shop.bench.len();
    let (x0, x1) = lay.tape_x();
    let band = [0x7a5232, 0x5d3d24, TAPE, 0x45301c, 0x2a1c10];
    for (i, col) in band.iter().enumerate() {
        hline(c, x0 - 3, x1 + 3, cy - 2 + i as i32, *col);
    }
    for x in [x0, x1] {
        vline(c, x, cy - 4, cy + 4, SILK);
    }
    for k in &shop.bench.rec.knobs {
        px(c, lay.tape_at(k.frame, len), cy + 4, AMBER);
    }
    for &m in &shop.marks {
        let x = lay.tape_at(m, len);
        vline(c, x, t.y - 1, cy - 3, CREAM);
        fill(
            c,
            Rect {
                x: x + 1,
                y: t.y - 1,
                w: 3,
                h: 2,
            },
            CREAM,
        );
    }
    // the head
    let hx = lay.tape_at(shop.bench.frame(), len);
    let head = [0x4f545c, 0xc9ced6, ACCENT, 0xaeb3bb, 0x4a4e56];
    for (i, col) in head.iter().enumerate() {
        vline(c, hx - 2 + i as i32, t.y + 1, t.y + t.h - 2, *col);
    }
    hline(c, hx - 2, hx + 2, t.y, 0xe8ebef);
}

/// A cassette: smoky shell, a label with the game's name, a window on the two reels. The tape
/// goes from the left reel to the right one as the recording plays; the hubs turn with it.
fn cassette(c: &mut Canvas, r: Rect, shop: &Workshop) {
    let (f, len) = (shop.bench.frame(), shop.bench.len());
    let t = if len == 0 { 0.0 } else { f as f32 / len as f32 };
    for yy in 0..r.h {
        for xx in 0..r.w {
            let corner = xx.min(r.w - 1 - xx) + yy.min(r.h - 1 - yy) < 2;
            if corner {
                continue;
            }
            let edge = xx == 0 || yy == 0 || xx == r.w - 1 || yy == r.h - 1;
            let col = if edge {
                0x0e0e11
            } else if yy == 1 {
                0x4a4b53
            } else {
                mix(0x36373e, 0x222328, yy as f32 / r.h as f32)
            };
            px(c, r.x + xx, r.y + yy, col);
        }
    }
    // the label
    let label = Rect {
        x: r.x + 5,
        y: r.y + 3,
        w: r.w - 10,
        h: 25,
    };
    fill(c, label, CREAM);
    hline(
        c,
        label.x,
        label.x + label.w - 1,
        label.y + label.h - 1,
        0xcbbf9f,
    );
    hline(c, label.x, label.x + label.w - 1, label.y + 2, ACCENT);
    hline(c, label.x, label.x + label.w - 1, label.y + 3, ACCENT);
    hline(c, label.x, label.x + label.w - 1, label.y + 4, 0xc8322a);
    let name: String = shop.name.to_uppercase().chars().take(14).collect();
    text(c, &name, label.x + 3, label.y + 7, PEN);
    // the window, the tape on the reels, the hubs
    let win = Rect {
        x: r.x + 10,
        y: r.y + 15,
        w: r.w - 20,
        h: 12,
    };
    fill(c, win, 0x141317);
    bevel(c, win, 0x0a090b, 0x6d675d);
    let inner = Rect {
        x: win.x + 1,
        y: win.y + 1,
        w: win.w - 2,
        h: win.h - 2,
    };
    let pack = |amount: f32| (4.5f32 * 4.5 + amount * (12.5 * 12.5 - 4.5 * 4.5)).sqrt();
    let hubs = [
        (r.x + 22, r.y + 21, pack(1.0 - t)),
        (r.x + r.w - 23, r.y + 21, pack(t)),
    ];
    for (hx, hy, radius) in hubs {
        // the wound tape: layers, a bright rim where the light catches it
        let mut layer = radius;
        let mut odd = false;
        while layer > 4.0 {
            disc(
                c,
                hx,
                hy,
                layer,
                if odd { 0x5a3a22 } else { 0x684429 },
                Some(inner),
            );
            layer -= 1.5;
            odd = !odd;
        }
        ring(c, hx, hy, radius, 0x9a6a40, Some(inner));
        disc(c, hx, hy, 4.0, 0xe4e4e8, Some(inner));
        disc(c, hx, hy, 1.5, 0x141317, Some(inner));
        let turn = f as f32 * 0.9 / radius;
        for k in 0..3 {
            let a = turn + k as f32 * TAU / 3.0;
            let (x, y) = (
                hx + (3.0 * a.cos()).round() as i32,
                hy + (3.0 * a.sin()).round() as i32,
            );
            if inner.contains((x, y)) {
                px(c, x, y, 0x55585f);
            }
        }
    }
    // the bottom edge, where the head meets the tape
    for yy in 0..10 {
        let inset = 9 + yy / 3;
        let col = if yy == 0 { 0x44454c } else { 0x1d1e22 };
        hline(c, r.x + inset, r.x + r.w - 1 - inset, r.y + 31 + yy, col);
    }
}

/// The piano keys in their well, lamps above PLAY, PAUSE and REC, names printed below.
fn keys(c: &mut Canvas, lay: &Layout, shop: &Workshop) {
    let (first, last) = (lay.keys[0], lay.keys[7]);
    let well = Rect {
        x: first.x - 3,
        y: first.y - 2,
        w: last.x + last.w - first.x + 6,
        h: first.h + 5,
    };
    fill(c, well, 0x0b0b0d);
    bevel(c, well, EDGE_DARK, EDGE_LIGHT);
    for (i, k) in DeckKey::ALL.iter().enumerate() {
        let r = lay.keys[i];
        let down = shop.mouse_key == Some(*k) || shop.engaged(*k);
        piano_key(c, r, *k, down, shop.hover == Some(*k));
        let lamp = match k {
            DeckKey::Play => Some(GREEN),
            DeckKey::Pause => Some(AMBER),
            DeckKey::Rec => Some(RED),
            _ => None,
        };
        if let Some(col) = lamp {
            led(c, r.x + r.w / 2 - 1, r.y - 5, col, shop.engaged(*k));
        }
        let l = k.label();
        if text_w(l) <= r.w + 1 {
            text(c, l, r.x + (r.w - text_w(l)) / 2, r.y + r.h + 5, SILK);
        }
    }
}

fn piano_key(c: &mut Canvas, r: Rect, k: DeckKey, down: bool, hover: bool) {
    let sink = if down { 2 } else { 0 };
    let face = Rect {
        x: r.x,
        y: r.y + sink,
        w: r.w,
        h: r.h - 2,
    };
    let (top, bottom) = if down {
        (0xc0c5cd, 0x878d96)
    } else {
        (0xe9ecf0, 0xa6acb5)
    };
    for yy in 0..face.h {
        let mut col = mix(top, bottom, yy as f32 / face.h as f32);
        if hover {
            col = mix(col, 0xffffff, 0.12);
        }
        hline(c, face.x, face.x + face.w - 1, face.y + yy, col);
    }
    hline(
        c,
        face.x,
        face.x + face.w - 1,
        face.y,
        if down { 0xd4d8de } else { 0xfbfcfd },
    );
    vline(c, face.x, face.y, face.y + face.h - 1, 0x80868f);
    vline(
        c,
        face.x + face.w - 1,
        face.y,
        face.y + face.h - 1,
        0x737982,
    );
    if !down {
        hline(c, r.x, r.x + r.w - 1, r.y + r.h - 2, 0x5d626b);
        hline(c, r.x, r.x + r.w - 1, r.y + r.h - 1, 0x34373d);
    }
    let rows = icon(k);
    let (iw, ih) = (rows[0].len() as i32, rows.len() as i32);
    let ink = if k == DeckKey::Rec {
        0xc9302a
    } else {
        0x232428
    };
    bitmap(
        c,
        rows,
        face.x + (face.w - iw) / 2,
        face.y + (face.h - ih) / 2,
        ink,
    );
}

fn icon(k: DeckKey) -> &'static [&'static str] {
    match k {
        DeckKey::Start => &[
            "#...#...#",
            "#..##..##",
            "#.###.###",
            "#########",
            "#.###.###",
            "#..##..##",
            "#...#...#",
        ],
        DeckKey::Rew => &[
            "...#...#", "..##..##", ".###.###", "########", ".###.###", "..##..##", "...#...#",
        ],
        DeckKey::Play => &["#...", "##..", "###.", "####", "###.", "##..", "#..."],
        DeckKey::Pause => &[
            "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##",
        ],
        DeckKey::Ff => &[
            "#...#...", "##..##..", "###.###.", "########", "###.###.", "##..##..", "#...#...",
        ],
        DeckKey::Step => &[
            "#...#", "##..#", "###.#", "#####", "###.#", "##..#", "#...#",
        ],
        DeckKey::Rec => &[
            "..###..", ".#####.", "#######", "#######", "#######", ".#####.", "..###..",
        ],
        DeckKey::Cue => &[
            "#.....", "####..", "######", "####..", "#.....", "#.....", "#.....",
        ],
    }
}

/// The display: what the deck does (or a note running through), the time, the ticks.
fn display(c: &mut Canvas, r: Rect, shop: &Workshop) {
    glass(c, r);
    let inner = Rect {
        x: r.x + 3,
        y: r.y + 3,
        w: r.w - 6,
        h: r.h - 6,
    };
    if let Some(note) = &shop.note {
        let w = text_w(&note.text);
        if w <= inner.w {
            text_in(c, &note.text, inner.x + 1, r.y + 4, TEAL, 1, inner);
        } else {
            let gap = 24;
            let run = (note.age.saturating_sub(30) / 2) as i32 % (w + gap);
            for x in [inner.x + 1 - run, inner.x + 1 - run + w + gap] {
                text_in(c, &note.text, x, r.y + 4, TEAL, 1, inner);
            }
        }
    } else {
        let (sign, word, col): (&[&str], &str, u32) = match shop.mode {
            Mode::Live => (&[".###.", "#####", "#####", "#####", ".###."], "REC", RED),
            Mode::Replay => (&["#....", "###..", "#####", "###..", "#...."], "PLAY", TEAL),
            Mode::Paused => (
                &["##.##", "##.##", "##.##", "##.##", "##.##"],
                "PAUSE",
                TEAL,
            ),
        };
        bitmap(c, sign, inner.x + 1, r.y + 4, col);
        text(c, word, inner.x + 8, r.y + 4, col);
    }
    let (f, len) = (shop.bench.frame(), shop.bench.len());
    let tenths = f * 10 / 60;
    let time = format!("{}:{:02}.{}", tenths / 600, (tenths / 10) % 60, tenths % 10);
    let tw = text_w(&time) * 2 + 1;
    text_in(c, &time, r.x + (r.w - tw) / 2, r.y + 14, TEAL, 2, inner);
    let ticks = format!("{f}/{len}");
    let line = if text_w(&ticks) + 20 <= inner.w {
        text(c, "TICK", inner.x + 1, r.y + 32, TEAL_DIM);
        inner.x + inner.w - text_w(&ticks)
    } else {
        inner.x + 1
    };
    text_in(c, &ticks, line, r.y + 32, TEAL, 1, inner);
}

/// A moving-coil meter: a lit cream face, a scale with a red end, a needle on a pivot below.
fn meter(c: &mut Canvas, m: Rect, value: f32, face_word: &str, label: &str, red: f32, lit: bool) {
    fill(c, m, 0x15161a);
    bevel(c, m, 0x9aa1ab, 0x4d525a);
    let face = Rect {
        x: m.x + 2,
        y: m.y + 2,
        w: m.w - 4,
        h: m.h - 12,
    };
    for yy in 0..face.h {
        for xx in 0..face.w {
            let t = yy as f32 / face.h as f32;
            let mut col = mix(0xf6eac6, 0xdcc895, t);
            let dx = (xx as f32 / face.w as f32 - 0.5) * 2.0;
            let dy = t - 0.35;
            col = mix(col, 0x9c8a5c, (dx * dx * 0.5 + dy * dy).min(1.0) * 0.5);
            if !lit {
                col = mix(col, 0x4e473a, 0.3);
            }
            px(c, face.x + xx, face.y + yy, col);
        }
    }
    let (cx, py) = (face.x + face.w / 2, face.y + face.h + 8);
    let radius = 28.0f32;
    let span = 48f32.to_radians();
    let at = |t: f32, r: f32| {
        let a = -span + t * 2.0 * span;
        (
            (cx as f32 + r * a.sin()).round() as i32,
            (py as f32 - r * a.cos()).round() as i32,
        )
    };
    for i in 0..=96 {
        let t = i as f32 / 96.0;
        let hot = t >= red;
        let (x, y) = at(t, radius);
        px(c, x, y, if hot { 0xc8322a } else { PEN });
        if hot {
            let (x, y) = at(t, radius - 1.0);
            px(c, x, y, 0xc8322a);
        }
    }
    for i in 0..=6 {
        let t = i as f32 / 6.0;
        for d in 1..3 {
            let (x, y) = at(t, radius + d as f32);
            px(c, x, y, if t >= red { 0xc8322a } else { PEN });
        }
    }
    let fw = text_w(face_word);
    text(c, face_word, cx - fw / 2, face.y + face.h - 9, 0x5a4e3c);
    // the needle, seen only on the face
    let (tx, ty) = at(value.clamp(0.0, 1.06), radius + 3.0);
    line(c, cx, py, tx, ty, 0x1b1a19, face);
    let base = Rect {
        x: m.x + 2,
        y: face.y + face.h,
        w: m.w - 4,
        h: 8,
    };
    fill(c, base, 0x1a1b1f);
    hline(c, base.x, base.x + base.w - 1, base.y, 0x2e3036);
    let lw = text_w(label);
    text(c, label, base.x + (base.w - lw) / 2, base.y + 2, SILK);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(knobs: usize, state_lines: usize) -> Content {
        Content {
            game: (320, 180),
            knobs,
            name_chars: 12,
            state_lines,
            knob_scroll: 0,
            panel_open: true,
        }
    }

    fn apart(a: Rect, b: Rect) -> bool {
        a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y
    }

    #[test]
    fn knobs_and_state_share_the_panel_by_what_they_hold() {
        // few knobs: all on show, the state takes the rest
        let lay = Layout::new(480, 270, &content(1, 30));
        assert_eq!(lay.faders, 1);
        assert!(lay.state_rows() >= 15, "{} rows", lay.state_rows());
        // many knobs: as many as leave the state three lines, the rest a scroll away
        let mut c = content(40, 30);
        let lay = Layout::new(480, 270, &c);
        assert!(lay.faders < 40 && lay.faders >= 2);
        assert!(lay.state_rows() >= 3, "{} rows", lay.state_rows());
        assert!(lay.fader_row(39).is_none());
        c.knob_scroll = 1000;
        let lay = Layout::new(480, 270, &c);
        assert_eq!(
            lay.first + lay.faders,
            40,
            "scrolled to the end, never past it"
        );
        assert!(lay.fader_row(39).is_some(), "every knob can be reached");
        // a short state leaves the knobs the room
        let lay = Layout::new(480, 270, &content(15, 2));
        assert_eq!(lay.faders, 15);
    }

    #[test]
    fn the_deck_keeps_what_works_first_as_it_narrows() {
        let parts = |w: usize| {
            let lay = Layout::new(w, 270, &content(3, 5));
            (
                lay.display.is_some(),
                lay.meters[1].is_some(),
                lay.meters[0].is_some(),
                lay.cassette.is_some(),
            )
        };
        assert_eq!(parts(480), (true, true, true, true));
        assert_eq!(
            parts(470),
            (true, true, true, false),
            "the cassette goes first"
        );
        assert_eq!(parts(400), (true, true, false, false), "then LEVEL");
        assert_eq!(parts(340), (true, false, false, false), "then LOAD");
        assert_eq!(parts(220), (false, false, false, false), "the keys stay");
        let small = Layout::new(140, 200, &content(3, 5));
        assert!(
            small.keys[0].w < KEY_W,
            "narrower keys rather than keys off the edge"
        );
        // nothing overlaps, at any width
        for w in (130..=1200).step_by(7) {
            let lay = Layout::new(w, 300, &content(3, 5));
            let well = Rect {
                x: lay.keys[0].x - 3,
                w: lay.keys[7].x + lay.keys[7].w + 3 - (lay.keys[0].x - 3),
                ..lay.keys[0]
            };
            let mut all = vec![well];
            all.extend(lay.cassette);
            all.extend(lay.display);
            all.extend(lay.meters.iter().flatten());
            for (i, a) in all.iter().enumerate() {
                assert!(a.x >= 0 && a.x + a.w <= w as i32, "{a:?} at width {w}");
                for b in &all[i + 1..] {
                    assert!(apart(*a, *b), "{a:?} and {b:?} at width {w}");
                }
            }
        }
    }

    #[test]
    fn the_panel_grows_with_the_window_and_folds_away() {
        let narrow = Layout::new(480, 270, &content(3, 5));
        let wide = Layout::new(900, 500, &content(3, 5));
        assert_eq!(narrow.panel.w, PANEL);
        assert!(wide.panel.w > narrow.panel.w && wide.panel.w <= PANEL_MAX);
        assert!(wide.slot().1 - wide.slot().0 > narrow.slot().1 - narrow.slot().0);
        // the game keeps its room beside the panel
        assert!(narrow.bay.w >= 320 + 4);
        let mut c = content(3, 5);
        c.panel_open = false;
        let folded = Layout::new(480, 270, &c);
        assert_eq!(folded.panel.w, FOLDED);
        assert!(folded.bay.w > narrow.bay.w);
        assert!(
            folded
                .handle
                .contains((folded.handle.x + 2, folded.handle.y + 2))
        );
        assert_eq!((folded.faders, folded.state_rows()), (0, 0));
    }
}
