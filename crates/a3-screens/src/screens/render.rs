//! The event window over a 3D picture (dialog 477, `CRender` on
//! `CRenderNeu`), here for a record the manager broke: the caption
//! "ANSTOSS 3 - Ereignis für" with his first and last name
//! (`FUN_00536655`), "Herzlichen Glückwunsch" and the record's lines
//! (`FUN_005350c6`, case 5), each centred in the bold font across the
//! area right of the picture (`FUN_00536c26`'s `SetRect(310, 35, 640,
//! 400)`, the line table at `0x886b3c`, painted by `FUN_00537259`). The
//! picture's video is 3D and stays out; so do the static over it and
//! cancel, which the record's window hides (`FUN_005374f4`). OK closes, as
//! does the window itself after 58 seconds (`FUN_0053656d` posting
//! `WM_ENDE_DIALOG`). Which records open it is [`a3_core::render`].

use std::collections::HashMap;

use a3_core::render::{self, CAPTION, CONGRATULATIONS, RECORD_TEXTS, RENDER};
use a3_files::rch::{Align, BOLD_FONT, Control, Dialog, Kind};
use bevy_math::Rect;

use super::discipline::CAPTION_ID;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::Target;
use crate::view::View;

const OK: u16 = 1;
const CANCEL: u16 = 2;
/// The static over the picture (`IDC_…` 6048, the dialog's +0x788).
const PICTURE: u16 = 6048;
/// The lines, by ids of the replica's own: each twice, the second a pixel
/// to the left, for a line of odd width — the window centres at
/// `left + (width − text) / 2`, a static at `left + width / 2 − text / 2`.
const LINES: [u16; 5] = [9941, 9942, 9943, 9944, 9945];
const ODD_LINES: [u16; 5] = [9951, 9952, 9953, 9954, 9955];
/// The area the lines are centred across: left, top, right.
const AREA: (f32, f32, f32) = (310.0, 35.0, 640.0);
/// Each line's top below the area's (the table's +0xc).
const LINE_TOPS: [f32; 5] = [0.0, 140.0, 160.0, 180.0, 200.0];
const LINE_HEIGHT: f32 = 20.0;
/// The lines' colour (the table's +0x10, 0x0a0a0a).
const INK: [u8; 3] = [10, 10, 10];
/// How long the window stays without OK (the dialog's +0x890, 58000 ms).
const CLOSES_AFTER: f32 = 58.0;

/// The caption bar and the lines; the picture's static and cancel out.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&RENDER) else { return };
    dialog.controls.retain(|c| c.id != Some(CANCEL) && c.id != Some(PICTURE));
    let caption = super::caption(dialog.frame, CAPTION_ID, String::new());
    dialog.controls.insert(0, caption);
    let (left, top, right) = AREA;
    for (ids, shift) in [(LINES, 0.0), (ODD_LINES, 1.0)] {
        for (&id, &y) in ids.iter().zip(&LINE_TOPS) {
            dialog.controls.push(Control {
                kind: Kind::Static,
                rect: Rect::new(left - shift, top + y, right - shift, top + y + LINE_HEIGHT),
                align: Align::Centre,
                text: String::new(),
                id: Some(id),
                font: BOLD_FONT,
                ink: INK,
            });
        }
    }
}

#[derive(Default)]
pub struct Render {
    /// How long it has been open.
    open_for: f32,
}

impl Screen for Render {
    fn ids(&self) -> &[u16] {
        &[RENDER]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        self.open_for = 0.0;
        let dialog = cx.dialog;
        let Args::Render(broken) = args else { return };
        let Some(records) = cx.state.records.as_ref() else { return };
        if let Some(m) = records.managers.get(usize::from(broken.slot)) {
            let caption = format!("{} {} {}", cx.game.text(CAPTION), m.first_name(), m.last_name());
            set(view, dialog, CAPTION_ID, caption);
        }
        let text = RECORD_TEXTS.get(usize::from(broken.record)).map_or("", |&id| cx.game.text(id));
        let fonts = cx.game.fonts();
        let width = |line: &str| fonts.get(BOLD_FONT).map_or(0, |f| f.width(line, false));
        let mut lines = render::lines(cx.game.text(CONGRATULATIONS), text).into_iter();
        for (&even, &odd) in LINES.iter().zip(&ODD_LINES) {
            let line = lines.next().unwrap_or_default();
            let (shown, unused) = if width(&line) % 2 == 1 { (odd, even) } else { (even, odd) };
            if let Some(c) = control(dialog, unused) {
                view.controls[c].hidden = true;
            }
            set(view, dialog, shown, line);
        }
        mark_inert(view, dialog, |t| match t {
            Target::Button(c) => dialog.controls[c].id == Some(OK),
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if control(cx.dialog, OK) == Some(c) => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }

    fn tick(&mut self, _: &mut Cx, _: &mut View, seconds: f32) -> Outcome {
        self.open_for += seconds;
        if self.open_for >= CLOSES_AFTER { Outcome::Close(Reply::Ok) } else { Outcome::Stay }
    }
}
