//! Spinners: the tiny up and down arrows next to a number. Most draw small
//! arrows of their own over the dialog's picture, the up arrow at the top,
//! the down arrow at the bottom (`CMyScrBar`'s style 0x40, `FUN_007ad236`),
//! brown from the fourth colour on. One with a track is a tall scrollbar
//! of its own, the help's: round arrows at its ends, a handle in its
//! colour between. Their value, which sliders share.

use bevy_math::{Rect, Vec2};
use image::{RgbaImage, imageops};

use a3_files::rch::{Control, Kind};

use super::{combo, list};
use crate::classic::{Paint, SCROLL_DOWN, SCROLL_UP, paste};
use crate::pointer::{Geometry, Target};

/// The small arrows, up and down, by colour.
pub(crate) const ARROWS: [&str; 4] =
    ["Pfeile/ScPfO1.bmp", "Pfeile/ScPfU1.bmp", "Pfeile/ScPfBraunO1.bmp", "Pfeile/ScPfBraunU1.bmp"];
const SMALL_ARROWS: u16 = 0x40;
const FIRST_BROWN: u16 = 3;
/// A tall scrollbar's track; its arrows' squares.
const WITH_TRACK: u16 = 0x100;
const ARROW: f32 = 20.0;

/// Whether the spinner is a tall scrollbar.
pub(crate) fn tall(control: &Control) -> bool {
    matches!(control.kind, Kind::Spin { style, .. } if style & WITH_TRACK != 0)
}

/// A tall scrollbar's arrows, up and down.
fn arrows(control: &Control) -> (Rect, Rect) {
    let r = control.rect;
    let up = Rect::new(r.min.x, r.min.y, r.min.x + ARROW, r.min.y + ARROW);
    let down = Rect::new(r.min.x, r.max.y - ARROW, r.min.x + ARROW, r.max.y);
    (up, down)
}

/// Where a tall scrollbar's handle sits: as long as the screen set it,
/// else the whole track, as far down as the value is in its range
/// (`FUN_007ae43e`, `FUN_007ad818`).
pub fn thumb(control: &Control, value: &Value) -> Rect {
    let (up, down) = arrows(control);
    let track = (down.min.y - up.max.y) as i32;
    let length = value.handle.unwrap_or(track).min(track);
    let (min, max) = value.range;
    let along = if max > min { (value.value - min) * (track - length) / (max - min) } else { 0 };
    let top = up.max.y + along as f32;
    Rect::new(up.min.x, top, up.max.x, top + length as f32)
}

pub(crate) fn draw(value: &Value, frame: &mut RgbaImage, paint: &Paint) {
    let Kind::Spin { color, style } = paint.control.kind else { return };
    if style & WITH_TRACK != 0 {
        draw_tall(value, usize::from(color), frame, paint);
        return;
    }
    if style & SMALL_ARROWS == 0 {
        return;
    }
    let r = paint.control.rect;
    let brown = usize::from(color >= FIRST_BROWN) * 2;
    if let Some(up) = paint.widget(ARROWS[brown]) {
        paste(frame, up, r.min);
    }
    if let Some(down) = paint.widget(ARROWS[brown + 1]) {
        paste(frame, down, Vec2::new(r.min.x, r.max.y - down.height() as f32));
    }
}

/// A tall scrollbar as a list draws its own: the track, the arrows
/// centred in their squares, the handle cut from its long bar.
fn draw_tall(value: &Value, color: usize, frame: &mut RgbaImage, paint: &Paint) {
    let (up, down) = arrows(paint.control);
    if let Some(track) = paint.widget(list::TRACKS[color.min(list::TRACKS.len() - 1)]) {
        let height = ((down.min.y - up.max.y) as u32).min(track.height());
        let width = list::TRACK_WIDTH.min(track.width());
        let part = imageops::crop_imm(track, 0, 0, width, height).to_image();
        let at = Vec2::new(up.min.x + list::TRACK_LEFT as f32, up.max.y);
        imageops::overlay(frame, &part, at.x as i64, at.y as i64);
    }
    for (name, arrow) in [(SCROLL_UP, up), (SCROLL_DOWN, down)] {
        if let Some(picture) = paint.widget(name) {
            let gap = (arrow.size() - Vec2::new(picture.width() as f32, picture.height() as f32)) / 2.0;
            paste(frame, picture, (arrow.min + gap).floor());
        }
    }
    if let Some(long) = paint.widget(list::HANDLES[color.min(list::HANDLES.len() - 1)]) {
        let thumb = thumb(paint.control, value);
        paste(frame, &combo::cut(long, thumb.height() as u32), thumb.min);
    }
}

/// A spinner's or slider's value, within its range.
#[derive(Clone, Copy, Default, Debug)]
pub struct Value {
    pub value: i32,
    pub range: (i32, i32),
    /// A slider's handle as long as the screen sets it (`FUN_007ae43e`),
    /// where it is not the usual twenty pixels.
    pub handle: Option<i32>,
}

impl Value {
    pub fn step(&mut self, up: bool) {
        self.set(self.value + if up { 1 } else { -1 });
    }

    pub fn set(&mut self, value: i32) {
        let (min, max) = self.range;
        self.value = value.clamp(min, max.max(min));
    }
}

/// The upper part steps up, the lower part down; a tall scrollbar's top
/// arrow back, its bottom arrow on.
pub(crate) fn targets(
    index: usize,
    control: &Control,
    geometry: &dyn Geometry,
    out: &mut Vec<(Target, Rect)>,
) {
    if tall(control) {
        let (up, down) = arrows(control);
        out.push((Target::Spin { control: index, up: false }, geometry.map(up)));
        out.push((Target::Spin { control: index, up: true }, geometry.map(down)));
        return;
    }
    let (up, down) = geometry.spin(index, control);
    out.push((Target::Spin { control: index, up: true }, up));
    out.push((Target::Spin { control: index, up: false }, down));
}
