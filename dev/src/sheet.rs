// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
// the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Contact sheets: many frames on one image, each labelled with its frame number.
//! An agent can't watch the game run; a contact sheet shows motion, arcs and timing at a glance.

use polter::canvas::Canvas;
use polter::font;

/// Lays out frames (frame number and picture) in a grid, on `bg` with labels in `ink`.
/// Returns (width, height, rgb bytes).
pub fn contact_sheet(
    frames: &[(u64, Canvas)],
    columns: usize,
    bg: u32,
    ink: u32,
) -> (usize, usize, Vec<u8>) {
    let columns = columns.max(1);
    let rows = frames.len().div_ceil(columns).max(1);
    let (margin, label) = (6usize, 14usize);
    let (fw, fh) = frames
        .first()
        .map_or((0, 0), |(_, c)| (c.width(), c.height()));
    let w = margin + columns * (fw + margin);
    let h = margin + rows * (label + fh + margin);
    let mut img = vec![bg; w * h];

    for (n, (frame, canvas)) in frames.iter().enumerate() {
        let x0 = margin + (n % columns) * (fw + margin);
        let y0 = margin + (n / columns) * (label + fh + margin);
        font::draw_into(&mut img, w, &frame.to_string(), x0, y0 + 2, 2, ink);
        for (y, row) in canvas.pixels().chunks(canvas.width()).enumerate().take(fh) {
            let at = (y0 + label + y) * w + x0;
            img[at..at + row.len().min(fw)].copy_from_slice(&row[..row.len().min(fw)]);
        }
    }
    let rgb = img
        .iter()
        .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
        .collect();
    (w, h, rgb)
}
