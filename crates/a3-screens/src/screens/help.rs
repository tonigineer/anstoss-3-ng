//! The help (dialog 423, `CDiHelpPage`), behind the question mark in a
//! window's caption (`CDialogH::OnClicked_IDC_HILFE`, `FUN_004c5687`): the
//! topic of the window it was opened from — found by the window's name in
//! the game's resources —, else the contents. The game reads the help's
//! RTF itself into one column of lines 434 pixels wide (`CParagraph`),
//! every line justified but a paragraph's last, a table's row framed cells
//! on a line of its own; the page shows the whole lines that fit in 397
//! pixels from the first at or under where its bar stands, a hundred
//! pixels a step (`FUN_006c80d3`). Its links lead to other topics;
//! "Inhalt" shows the contents, "Index" the topics' keywords, "Historie"
//! goes back the way the links took, "Blättern" through the topics in
//! their order.

use std::collections::HashMap;

use a3_files::help::{Block, Help, Paragraph, Run};
use a3_files::rch::{Align, Control, Dialog, Kind};
use bevy_math::Rect;
use image::{Rgba, RgbaImage, imageops};

use crate::controls::bitmap::Picture;
use crate::game::Game;
use crate::pointer::Target;
use crate::screens::{Args, Cx, Outcome, Reply, Screen, caption, control, mark_inert, set};
use crate::view::View;

pub(crate) const HELP_PAGE: u16 = 423;
const CAPTION_ID: u16 = 9947;
/// The page and the bar beside it, drawn by the screen.
const PAGE_ID: u16 = 9960;
const PAGE_AT: Rect = Rect { min: bevy_math::Vec2::new(21.0, 64.0), max: bevy_math::Vec2::new(455.0, 461.0) };
const WIDTH: i32 = 434;
const HEIGHT: i32 = 397;
/// The bar's steps.
const STEP: i32 = 100;
const SCROLLBAR: u16 = 5142;
/// Its arrows' squares, and the shortest its handle gets.
const BAR_ARROW: i32 = 20;
const LEAST_HANDLE: i32 = 8;
/// "Inhalt", "Historie" and "Blättern" back and on, "Index"; the topic's
/// title.
const CONTENTS: u16 = 5143;
const WAYS: u16 = 6164;
const INDEX: u16 = 5139;
const TITLE: u16 = 6476;
const BACK: usize = 0;
const PREVIOUS: usize = 2;
const NEXT: usize = 3;
/// The caption: the dialog's title, the help's version and the
/// simulation's (`Simuversion`).
const VERSION: &str = "Version 1.40/1.30";
/// What the contents and the index are called in the title.
const CONTENTS_TITLE: &str = "Inhalt";
const INDEX_TITLE: &str = "Index";
/// A line of text; a line holds 12 pixels before anything is on it
/// (`CParagraphLine`).
const LINE: i32 = 13;
const EMPTY: i32 = 12;
/// The gap between words where a line is not justified, and the frame
/// of a table's row.
const SPACE: i32 = 3;
const BORDER: i32 = 2;
/// A table's `\cellx` in twips to pixels, its cells 3 pixels narrower
/// (`FUN_006c7e97`); without one a cell is 100 wide, the second the rest
/// but 110 (`FUN_006c7cc4`).
const TWIPS: f64 = 15.4;
const NO_CELLS: [i32; 2] = [100, WIDTH - 110];
/// What a contents' book leads to.
const NO_JUMP: &str = "NO JUMP";
/// Black text, the contents' and the index's green entries (0x8000), and
/// links where the RTF names no colour.
const BLACK: [u8; 3] = [0, 0, 0];
const ENTRY: [u8; 3] = [0, 128, 0];
const LINK: [u8; 3] = [128, 0, 128];
/// The pictures' folder, and the button that leads to another topic.
const PICTURES: &str = "Hilfe/Grafik";
const BUTTON: &str = "HelpJumpButton.bmp";
/// A link's target that runs a WinHelp macro instead.
const MACRO: char = '%';
/// The start menu opens the contents, though a topic bears its name (seen
/// on the original).
const NO_TOPIC: [u16; 1] = [9];

/// What the page shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Page {
    #[default]
    Contents,
    Index,
    Topic(usize),
}

/// Something to draw on the page, from its top left.
#[derive(Clone, Debug)]
enum Draw {
    Text { x: i32, y: i32, text: String, bold: bool, color: [u8; 3] },
    Picture { x: i32, y: i32, name: String },
    /// A table's border (`FUN_0079c51c`).
    Fill { x: i32, y: i32, w: i32, h: i32 },
}

/// One piece of a line: a word, a link whole, a picture, a table's cell.
#[derive(Clone, Debug)]
enum Item {
    Text { text: String, bold: bool, color: [u8; 3], link: Option<Page>, width: i32 },
    Picture { name: String, width: i32, link: Option<Page> },
    Cell(Column),
}

impl Item {
    fn width(&self) -> i32 {
        match self {
            Item::Text { width, .. } | Item::Picture { width, .. } => *width,
            Item::Cell(cell) => cell.width,
        }
    }
}

/// A line as the game builds it (`CParagraphLine`).
#[derive(Clone, Debug)]
struct Line {
    items: Vec<Item>,
    /// The pieces' widths together, and how far they may run.
    used: i32,
    width: i32,
    height: i32,
    /// The gap between the pieces where the line is not justified.
    space: i32,
    /// A table row's frame.
    border: i32,
    justify: bool,
    /// A paragraph's last line, never justified.
    last: bool,
}

impl Line {
    fn new(width: i32, justify: bool) -> Self {
        Line { items: Vec::new(), used: 0, width, height: EMPTY, space: SPACE, border: 0, justify, last: false }
    }

    /// Whether a piece still fits, counting a gap fewer than the pieces
    /// it follows (`FUN_006c685c`).
    fn fits(&self, width: i32) -> bool {
        (self.items.len() as i32 - 1) * self.space + self.used + width < self.width - 1
    }

    fn push(&mut self, item: Item, height: i32) {
        self.used += item.width();
        self.height = self.height.max(height);
        self.items.push(item);
    }

    /// A table's cell, framed, its frame shared with the cell before
    /// (`FUN_006c6e07`); one that does not fit is left out.
    fn push_cell(&mut self, cell: Column) {
        let mut width = cell.width + 2 * self.border;
        if self.used != 0 {
            width -= self.border - 1;
        }
        if self.used + width > self.width {
            return;
        }
        self.used += width;
        self.height = self.height.max(cell.height() + 2 * self.border);
        self.items.push(Item::Cell(cell));
    }

    /// The line drawn with its top left at (x, y), the pieces a gap
    /// apart that adds up in floats and is cut to whole pixels
    /// (`FUN_006c6b59`); returns how far down it moves the next line.
    fn place(&self, x: i32, y: i32, out: &mut Placed) -> i32 {
        let border = self.border;
        let mut pen = border as f32;
        let count = self.items.len() as i32;
        let gap = if self.justify && !self.last && count > 1 {
            (self.width - self.used) as f32 / (count - 1) as f32
        } else {
            self.space as f32
        };
        for item in &self.items {
            let (left, top) = (x + pen as i32, y + border);
            match item {
                Item::Text { text, bold, color, link, width } => {
                    out.draws.push(Draw::Text { x: left, y: top, text: text.clone(), bold: *bold, color: *color });
                    if let Some(page) = link {
                        let at = Rect::new(left as f32, top as f32, (left + width) as f32, (top + LINE) as f32);
                        out.links.push((at, *page));
                    }
                }
                Item::Picture { name, width, link } => {
                    out.draws.push(Draw::Picture { x: left, y: top, name: name.clone() });
                    if let Some(page) = link {
                        let at = Rect::new(left as f32, top as f32, (left + width) as f32, (top + self.height) as f32);
                        out.links.push((at, *page));
                    }
                }
                Item::Cell(cell) => cell.place(left, top, out),
            }
            pen = (f64::from(item.width()) + f64::from(gap) + f64::from(border) + f64::from(pen)) as f32;
            if border > 1 {
                let at = (f64::from(x) + f64::from(pen) + 1.0 - f64::from(border)) as i32;
                out.draws.push(Draw::Fill { x: at, y, w: border - 1, h: self.height });
                pen = (f64::from(border - 1) + f64::from(pen)) as f32;
            }
        }
        if border < 2 {
            return self.height;
        }
        let across = (f64::from(pen) - 2.0 * f64::from(border) + 2.0) as i32;
        out.draws.push(Draw::Fill { x, y, w: across, h: border - 1 });
        out.draws.push(Draw::Fill { x, y: y + self.height + 1 - border, w: across, h: border - 1 });
        out.draws.push(Draw::Fill { x, y, w: border - 1, h: self.height });
        self.height - border + 1
    }
}

/// Lines one under another (`CParagraph`): the page, or a table's cell.
#[derive(Clone, Debug, Default)]
struct Column {
    lines: Vec<Line>,
    width: i32,
    justify: bool,
}

impl Column {
    fn new(width: i32, justify: bool) -> Self {
        Column { lines: Vec::new(), width, justify }
    }

    fn current(&mut self) -> &mut Line {
        if self.lines.is_empty() {
            self.lines.push(Line::new(self.width, self.justify));
        }
        self.lines.last_mut().unwrap()
    }

    /// A paragraph's end: the line ends, a new one begins
    /// (`FUN_006c78ba`).
    fn new_line(&mut self) {
        if let Some(line) = self.lines.last_mut() {
            line.last = true;
        }
        self.lines.push(Line::new(self.width, self.justify));
    }

    /// A piece on the line, else on a new one (`FUN_006c71d3`).
    fn add(&mut self, item: Item, height: i32) {
        if !self.current().fits(item.width()) {
            self.lines.push(Line::new(self.width, self.justify));
        }
        self.current().push(item, height);
    }

    fn end(&mut self) {
        if let Some(line) = self.lines.last_mut() {
            line.last = true;
        }
    }

    fn height(&self) -> i32 {
        self.lines.iter().map(|l| l.height).sum()
    }

    fn place(&self, x: i32, mut y: i32, out: &mut Placed) {
        for line in &self.lines {
            y += line.place(x, y, out);
        }
    }
}

/// What a page shows where it stands: the draws and the links.
#[derive(Default)]
struct Placed {
    draws: Vec<Draw>,
    links: Vec<(Rect, Page)>,
}

/// The caption bar, and the page's picture over the dialog's.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&HELP_PAGE) else { return };
    let title = format!("{} {VERSION}", dialog.title);
    dialog.controls.insert(0, caption(dialog.frame, CAPTION_ID, title));
    dialog.controls.insert(
        1,
        Control {
            kind: Kind::Bitmap,
            rect: PAGE_AT,
            align: Align::Left,
            text: String::new(),
            id: Some(PAGE_ID),
            font: 0,
            ink: [0; 3],
        },
    );
}

/// The topic a window's help opens on.
pub(crate) fn topic_of(game: &Game, dialog: u16) -> Option<usize> {
    if NO_TOPIC.contains(&dialog) {
        return None;
    }
    game.help()?.topic(&game.dialogs.get(&dialog)?.name)
}

/// A picture of the help, its name as the RTF spells it, found whatever
/// its case in the help's folder, else in the game's graphics.
fn picture(game: &Game, name: &str) -> Option<RgbaImage> {
    let dir = game.dir.join(PICTURES);
    let file = std::fs::read_dir(&dir)
        .ok()
        .and_then(|d| d.filter_map(Result::ok).find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name)));
    match file {
        Some(file) => game.picture(&format!("{PICTURES}/{}", file.file_name().to_string_lossy())).ok(),
        None => game.picture(name).ok(),
    }
}

/// The screen: what it shows, how far down, the way it took there.
#[derive(Default)]
pub struct HelpPage {
    page: Page,
    scroll: i32,
    /// The pages links led away from, each as far down as it stood.
    history: Vec<(Page, i32)>,
    laid: Column,
    placed: Placed,
    drawn: u64,
    pictures: HashMap<String, Option<RgbaImage>>,
}

impl HelpPage {
    /// The page's paragraphs: a topic's, or the contents' and the index's
    /// as the game makes them.
    fn blocks(&self, help: &Help) -> Vec<Block> {
        let line = |run: Run| Block::Paragraph(Paragraph { runs: vec![run], ..Paragraph::default() });
        match self.page {
            Page::Topic(t) => help.topics.get(t).map_or_else(Vec::new, |t| t.blocks.clone()),
            // Each entry one piece, set in by two blanks a level; a book
            // bold, black and leading nowhere (`FUN_007d6124`).
            Page::Contents => help
                .contents
                .iter()
                .map(|e| {
                    let text = format!("{}{}", "  ".repeat(usize::from(e.level)), e.title);
                    line(match &e.topic {
                        Some(topic) => Run::Link { text, target: topic.clone(), bold: false },
                        None => Run::Link { text, target: NO_JUMP.to_owned(), bold: true },
                    })
                })
                .collect(),
            // The keywords in their order, case aside as the C library
            // sees it, under the first character they share, a blank line
            // between (`FUN_007d438b`).
            Page::Index => {
                let mut keywords: Vec<(String, String)> = help
                    .topics
                    .iter()
                    .flat_map(|t| t.keywords.iter().map(move |k| (k.clone(), t.id.clone())))
                    .filter(|(k, _)| !k.is_empty())
                    .collect();
                keywords.sort_by_cached_key(|(k, _)| k.to_ascii_lowercase());
                keywords.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));
                let mut out = Vec::new();
                let mut group = None;
                for (keyword, topic) in keywords {
                    let first = keyword.chars().next();
                    if group != first {
                        if group.is_some() {
                            out.push(Block::Paragraph(Paragraph::default()));
                        }
                        group = first;
                        let head = first.map(String::from).unwrap_or_default();
                        out.push(line(Run::Text { text: head, bold: true, italic: false, color: None }));
                    }
                    out.push(line(Run::Link { text: keyword, target: topic, bold: false }));
                }
                out
            }
        }
    }

    /// The page laid out as the game does it: one column of lines 434
    /// wide, justified but where a paragraph ends (`FUN_0065564d`).
    fn lay_out(&mut self, game: &Game) {
        let mut page = Column::new(WIDTH, true);
        let Some(help) = game.help() else {
            self.laid = page;
            return;
        };
        for block in self.blocks(help) {
            match block {
                Block::Paragraph(p) => {
                    self.paragraph(game, help, &p, &mut page);
                    page.new_line();
                }
                Block::Row(row) => {
                    // The cells framed side by side on a line of their
                    // own, the first a pixel narrower (`FUN_006c7cc4`,
                    // `FUN_006c7ef2`).
                    let mut widths = Vec::new();
                    let mut before = 0;
                    for cell in &row.cells {
                        let right = ((f64::from(cell.right) / TWIPS) as i32).min(WIDTH);
                        widths.push(right - 3 - before);
                        before = right;
                    }
                    if widths.is_empty() {
                        widths.extend(NO_CELLS);
                    }
                    let line = page.current();
                    line.space = 0;
                    line.border = BORDER;
                    for (i, cell) in row.cells.iter().enumerate() {
                        let width = widths.get(i).copied().unwrap_or(100) - i32::from(i == 0);
                        let mut column = Column::new(width, false);
                        for (j, p) in cell.paragraphs.iter().enumerate() {
                            if j > 0 {
                                column.new_line();
                            }
                            self.paragraph(game, help, p, &mut column);
                        }
                        column.end();
                        page.current().push_cell(column);
                    }
                    page.new_line();
                }
            }
        }
        page.end();
        self.laid = page;
    }

    /// A paragraph's words onto the column, each its own piece; a link
    /// one piece whole (`FUN_006c76b4`).
    fn paragraph(&mut self, game: &Game, help: &Help, p: &Paragraph, column: &mut Column) {
        let fonts = game.fonts();
        let Some(plain) = fonts.get(a3_files::rch::PLAIN_FONT) else { return };
        let color_of = |c: Option<u16>| c.and_then(|c| help.colors.get(usize::from(c))).copied();
        for run in &p.runs {
            match run {
                Run::Text { text, bold, color, .. } => {
                    let color = color_of(*color).unwrap_or(BLACK);
                    for word in text.split(' ').filter(|w| !w.is_empty()) {
                        let width = plain.width(word, false) as i32;
                        let item = Item::Text { text: word.to_owned(), bold: *bold, color, link: None, width };
                        column.add(item, LINE);
                    }
                }
                Run::Link { text, target, bold } => {
                    if text.is_empty() {
                        continue;
                    }
                    // A macro's (`%!JumpId(…)`, a button before it) is
                    // no topic's name: black, and it leads nowhere.
                    let link = help.topic(target).map(Page::Topic);
                    let color = match self.page {
                        _ if target.starts_with(MACRO) || target == NO_JUMP => BLACK,
                        Page::Contents | Page::Index => ENTRY,
                        _ => LINK,
                    };
                    let width = plain.width(text, false) as i32;
                    column.add(Item::Text { text: text.clone(), bold: *bold, color, link, width }, LINE);
                }
                Run::Picture(name) => {
                    let pic = self.picture(game, name);
                    let (width, height) = pic.map_or((0, 0), |p| (p.width() as i32, p.height() as i32));
                    column.add(Item::Picture { name: name.clone(), width, link: None }, height);
                }
                Run::Button(target) => {
                    // The game's own button picture (`CMyHelpJumpButton`).
                    let pic = self.picture(game, BUTTON);
                    let (width, height) = pic.map_or((0, 0), |p| (p.width() as i32, p.height() as i32));
                    let link = help.topic(target).map(Page::Topic);
                    column.add(Item::Picture { name: BUTTON.to_owned(), width, link }, height);
                }
                Run::Tab => {}
            }
        }
    }

    fn picture(&mut self, game: &Game, name: &str) -> Option<&RgbaImage> {
        self.pictures.entry(name.to_lowercase()).or_insert_with(|| picture(game, name)).as_ref()
    }

    /// The page from the first line at or under where the bar stands,
    /// as many whole lines as fit (`FUN_006c80d3`), on the dialog's
    /// picture.
    fn draw(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Ok(background) = game.graphic(dialog.background) else { return };
        let mut page =
            imageops::crop_imm(&background, PAGE_AT.min.x as u32, PAGE_AT.min.y as u32, WIDTH as u32, HEIGHT as u32)
                .to_image();
        let mut placed = Placed::default();
        let (mut top, mut y) = (0, 0);
        for line in &self.laid.lines {
            if self.scroll <= top {
                if HEIGHT - 1 < y + line.height {
                    break;
                }
                y += line.place(0, y, &mut placed);
            }
            top += line.height;
        }
        let fonts = game.fonts();
        for d in &placed.draws {
            match d {
                Draw::Text { x, y, text, bold, color } => {
                    // Bold is the plain face struck again a pixel to the
                    // right (`FUN_006c6500`).
                    if let Some(font) = fonts.get(a3_files::rch::PLAIN_FONT) {
                        let [r, g, b] = *color;
                        for dx in 0..=i32::from(*bold) {
                            font.draw(&mut page, x + dx, *y, text, Rgba([r, g, b, 255]), false);
                        }
                    }
                }
                Draw::Picture { x, y, name } => {
                    if let Some(p) = self.picture(game, name) {
                        imageops::overlay(&mut page, p, i64::from(*x), i64::from(*y));
                    }
                }
                Draw::Fill { x, y, w, h } => crate::classic::fill(&mut page, *x, *y, *w, *h, Rgba([0, 0, 0, 255])),
            }
        }
        self.placed = placed;
        self.drawn += 1;
        if let Some(bitmap) = control(dialog, PAGE_ID).and_then(|c| view.bitmap_mut(c)) {
            bitmap.picture = Some(Picture::drawn((1 << 40) + self.drawn, page));
            bitmap.pointed = true;
        }
        // The bar a step for every hundred pixels the page runs over
        // 396, its handle as long as 396 are of the page (`FUN_004c59c9`,
        // `FUN_007ae43e`).
        let (height, shown) = (self.laid.height(), HEIGHT - 1);
        let steps = if height > shown { (height - shown) / STEP + 1 } else { 0 };
        if let Some(bar) = control(dialog, SCROLLBAR) {
            let track = dialog.controls[bar].rect.height() as i32 - 2 * BAR_ARROW;
            if let Some(bar) = view.value_mut(bar) {
                bar.range = (0, steps);
                bar.set(self.scroll / STEP);
                bar.handle = (shown < height).then(|| (track * shown / height).max(LEAST_HANDLE));
            }
        }
    }

    /// Another page shown, the bar where it is put (`FUN_004c5c4c`); the
    /// page shown already stays as it is.
    fn show(&mut self, cx: &Cx, view: &mut View, page: Page, scroll: i32) {
        if page != self.page {
            self.open_page(cx, view, page, scroll);
        }
    }

    fn open_page(&mut self, cx: &Cx, view: &mut View, page: Page, scroll: i32) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.page = page;
        self.scroll = scroll;
        let title = match page {
            Page::Contents => CONTENTS_TITLE.to_owned(),
            Page::Index => INDEX_TITLE.to_owned(),
            Page::Topic(t) => game.help().and_then(|h| h.topics.get(t)).map_or_else(String::new, |t| t.title.clone()),
        };
        set(view, dialog, TITLE, title);
        self.lay_out(game);
        self.draw(cx, view);
    }

    /// On to a link's page, the one left kept for "Historie" with where
    /// its bar stood (`FUN_006556dc`).
    fn follow(&mut self, cx: &Cx, view: &mut View, page: Page) {
        if page != self.page {
            self.history.push((self.page, self.scroll));
        }
        self.show(cx, view, page, 0);
    }
}

impl Screen for HelpPage {
    fn ids(&self) -> &[u16] {
        &[HELP_PAGE]
    }

    /// `FUN_004c59c9`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        *self = Self::default();
        let page = match args {
            Args::Record(dialog) => topic_of(cx.game, dialog).map_or(Page::Contents, Page::Topic),
            _ => Page::Contents,
        };
        if let Some(options) = control(cx.dialog, WAYS).and_then(|c| view.options_mut(c)) {
            options.actions = true;
        }
        self.open_page(cx, view, page, 0);
        mark_inert(view, cx.dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        match target {
            // A link on the page (`OnLButtonDown`, `FUN_006556dc`).
            Target::Point { x, y, .. } => {
                let at = bevy_math::Vec2::new(f32::from(x), f32::from(y));
                if let Some(&(_, page)) = self.placed.links.iter().find(|(r, _)| r.contains(at)) {
                    self.follow(cx, view, page);
                }
            }
            Target::Spin { control: c, .. } if id(c) == SCROLLBAR => {
                let step = view.value(c).map_or(0, |v| v.value);
                self.scroll = step * STEP;
                self.draw(cx, view);
            }
            // The contents, the index and "Blättern" keep no history
            // (`OnClicked_IDC_CONTENT`, `OnClicked_IDC_INDEX`, `FUN_004c5e1d`,
            // `FUN_004c5e43`); "Historie" back returns to where the bar
            // stood (`FUN_004c5dea`), on to nothing.
            Target::Button(c) if id(c) == CONTENTS => self.show(cx, view, Page::Contents, 0),
            Target::Button(c) if id(c) == INDEX => self.show(cx, view, Page::Index, 0),
            Target::Option { control: c, item } if id(c) == WAYS => {
                let topics = cx.game.help().map_or(0, |h| h.topics.len());
                match (item, self.page) {
                    (BACK, _) => {
                        if let Some((page, scroll)) = self.history.pop() {
                            self.show(cx, view, page, scroll);
                        }
                    }
                    (PREVIOUS, Page::Topic(t)) if t > 0 => self.show(cx, view, Page::Topic(t - 1), 0),
                    (NEXT, Page::Topic(t)) if t + 1 < topics => self.show(cx, view, Page::Topic(t + 1), 0),
                    _ => {}
                }
            }
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                return Outcome::Close(Reply::Ok);
            }
            _ => {}
        }
        Outcome::Stay
    }
}
