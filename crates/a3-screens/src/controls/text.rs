//! Statics: a line of text, or a block that wraps where it has room for more
//! lines, which the flow may fill in and make clickable.

use bevy_math::Rect;
use image::{Rgba, RgbaImage};

use a3_files::bitmap_font::BitmapFont;
use a3_files::rch::{Align, Control};

use crate::classic::{Paint, TEXT_HOVER, centred};
use crate::pointer::{Geometry, Target};

#[derive(Clone, Default, Debug)]
pub struct StaticState {
    /// What it shows, placeholders filled in.
    pub text: String,
    /// Statics only answer the pointer when the flow says so.
    pub clickable: bool,
    /// One that answers to be dragged does not light up.
    pub quiet: bool,
    /// A colour the screen gives it instead of the dialog file's.
    pub ink: Option<[u8; 3]>,
}

impl StaticState {
    pub(crate) fn draw(&self, frame: &mut RgbaImage, paint: &Paint) {
        let Some(font) = paint.font_of(paint.control.font) else { return };
        let pointed = paint.pointed(Target::Control(paint.index)) && !self.quiet;
        let [r, g, b] = self.ink.unwrap_or(paint.control.ink);
        let color = if pointed { TEXT_HOVER } else { Rgba([r, g, b, 255]) };
        draw_static(font, frame, paint.control, &self.text, color);
    }

    pub(crate) fn targets(
        &self,
        index: usize,
        control: &Control,
        geometry: &dyn Geometry,
        out: &mut Vec<(Target, Rect)>,
    ) {
        if self.clickable {
            out.push((Target::Control(index), geometry.rect(control)));
        }
    }
}

/// Aligned within its rectangle, wrapped, each line aligned on its own,
/// and only what falls within the rectangle shown (`DrawText` with
/// `DT_WORDBREAK`, as `CMyStatic` paints, `FUN_007ae930`).
fn draw_static(
    font: &BitmapFont,
    frame: &mut RgbaImage,
    control: &Control,
    text: &str,
    color: Rgba<u8>,
) {
    let r = control.rect;
    let (x, y) = (r.min.x as i32, r.min.y as i32);
    // Wrapped always: what does not fit on the lines its height has room
    // for falls below and does not show ("Ersten Torschützen tippen" in
    // 411's 120 pixels).
    let lines = font.wrap(text, r.width() as u32, false);
    let placed: Vec<(i32, &String)> = lines
        .iter()
        .map(|line| {
            let width = font.width(line, false) as i32;
            let left = match control.align {
                Align::Left => x,
                Align::Centre => centred(x, r.width() as i32, width),
                Align::Right => r.max.x as i32 - width,
            };
            (left, line)
        })
        .collect();
    // A static is a window of its own: what it draws shows only within it.
    let left = placed.iter().map(|&(l, _)| l).min().unwrap_or(x);
    let right = placed.iter().map(|&(l, line)| l + font.width(line, false) as i32 + 1).max().unwrap_or(x);
    let bottom = y + (lines.len() as i32 + 1) * font.height() as i32;
    within(frame, r, (left, y, right, bottom), |frame| {
        for (i, (left, line)) in placed.iter().enumerate() {
            font.draw(frame, *left, y + i as i32 * font.height() as i32, line, color, false);
        }
    });
}

/// Draws, keeping what lands outside `rect` from showing: the pixels
/// `reach` covers outside it are put back as they were — a control's
/// window clips what it draws.
pub(crate) fn within(
    frame: &mut RgbaImage,
    rect: Rect,
    reach: (i32, i32, i32, i32),
    draw: impl FnOnce(&mut RgbaImage),
) {
    let (w, h) = (frame.width() as i32, frame.height() as i32);
    let (x0, y0) = (reach.0.min(rect.min.x as i32).clamp(0, w), reach.1.min(rect.min.y as i32).clamp(0, h));
    let (x1, y1) = (reach.2.clamp(0, w), reach.3.clamp(0, h));
    let inside = |px: i32, py: i32| {
        (rect.min.x as i32..rect.max.x as i32).contains(&px) && (rect.min.y as i32..rect.max.y as i32).contains(&py)
    };
    let outside: Vec<(u32, u32, Rgba<u8>)> = (y0..y1)
        .flat_map(|py| (x0..x1).map(move |px| (px, py)))
        .filter(|&(px, py)| !inside(px, py))
        .map(|(px, py)| (px as u32, py as u32, *frame.get_pixel(px as u32, py as u32)))
        .collect();
    draw(frame);
    for (px, py, pixel) in outside {
        frame.put_pixel(px, py, pixel);
    }
}
