//! The game's help (`Hilfe/anstoss3.rtf`, `Hilfe/anstoss3.cnt`): Windows
//! help source, which the game reads itself (`FUN_0065540c`, the parser
//! around `0x7d63ff`) instead of handing it to WinHelp. Its topics are the
//! RTF's pages, each named by footnotes — `#` its id (a dialog's `IDD_`
//! name, or a word of its own), `$` its title, `K` its index keywords, `+`
//! its place in the browse sequence —; their text is paragraphs of runs,
//! bold or not, links to other topics (double or single underlined text
//! followed by the target as hidden text), pictures (`{bmc file}`, from
//! `Hilfe/Grafik`) and rows of table cells. The contents file lists the
//! topics in two levels.

use std::io;
use std::path::Path;

/// A paragraph's alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Justify,
    Right,
    Centre,
}

/// A piece of a paragraph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Run {
    /// Text in one style; its colour an index into the RTF's colour table.
    Text { text: String, bold: bool, italic: bool, color: Option<u16> },
    /// Text that leads to a topic by its id.
    Link { text: String, target: String, bold: bool },
    /// A picture by its file name.
    Picture(String),
    /// The help's button picture leading to a topic (`{button ,JumpId(file,
    /// topic)}`).
    Button(String),
    Tab,
}

/// A paragraph and its measures, in twips as the RTF gives them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Paragraph {
    pub runs: Vec<Run>,
    pub align: Align,
    pub left: i32,
    pub right: i32,
    pub first: i32,
    pub before: i32,
    pub after: i32,
}

impl Paragraph {
    /// Whether it holds nothing to show: a blank line.
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }
}

/// A table row: where it starts, the gap within its cells, and the cells
/// with their right edges.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub left: i32,
    pub gap: i32,
    pub cells: Vec<Cell>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub right: i32,
    pub paragraphs: Vec<Paragraph>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Paragraph(Paragraph),
    Row(Row),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Topic {
    pub id: String,
    pub title: String,
    pub keywords: Vec<String>,
    pub browse: String,
    pub blocks: Vec<Block>,
}

/// A line of the contents: a heading (level 1) or a topic under one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub level: u8,
    pub title: String,
    pub topic: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Help {
    pub topics: Vec<Topic>,
    pub contents: Vec<Entry>,
    /// The RTF's colours, the empty first entry left out as the game's
    /// reader leaves it — so its `\cf11` is the twelfth listed.
    pub colors: Vec<[u8; 3]>,
}

impl Help {
    pub fn load(dir: &Path) -> io::Result<Self> {
        let rtf = std::fs::read(dir.join("Hilfe/anstoss3.rtf"))?;
        let cnt = std::fs::read(dir.join("Hilfe/anstoss3.cnt"))?;
        let (topics, colors) = parse_rtf(&decode(&rtf));
        Ok(Self { topics, contents: parse_contents(&decode(&cnt)), colors })
    }

    /// A topic's place by its id.
    pub fn topic(&self, id: &str) -> Option<usize> {
        self.topics.iter().position(|t| t.id == id)
    }
}

fn decode(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| crate::sav::cp1252(&[b]).chars().next().unwrap_or(' ')).collect()
}

/// The contents: "1 Heading", "2 Title=topic", after a ":Base" line.
fn parse_contents(text: &str) -> Vec<Entry> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim_end_matches('\r');
            let (level, rest) = line.split_once(' ')?;
            let level: u8 = level.parse().ok()?;
            let (title, topic) = match rest.split_once('=') {
                Some((title, topic)) => (title, Some(topic.to_owned())),
                None => (rest, None),
            };
            Some(Entry { level, title: title.to_owned(), topic })
        })
        .collect()
}

#[derive(Clone, Debug)]
enum Token {
    Open,
    Close,
    Word(String, Option<i32>),
    Text(char),
}

fn tokens(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '{' => out.push(Token::Open),
            '}' => out.push(Token::Close),
            '\r' | '\n' => {}
            '\\' => {
                i += 1;
                let Some(&c) = chars.get(i) else { break };
                if c.is_ascii_alphabetic() {
                    let start = i;
                    while chars.get(i).is_some_and(char::is_ascii_alphabetic) {
                        i += 1;
                    }
                    let word: String = chars[start..i].iter().collect();
                    let num_start = i;
                    if chars.get(i) == Some(&'-') {
                        i += 1;
                    }
                    while chars.get(i).is_some_and(char::is_ascii_digit) {
                        i += 1;
                    }
                    let number = (i > num_start).then(|| chars[num_start..i].iter().collect::<String>());
                    let number = number.and_then(|n| n.parse().ok());
                    // A space ends a control word and is part of it.
                    if chars.get(i) != Some(&' ') {
                        i -= 1;
                    }
                    out.push(Token::Word(word, number));
                } else if c == '\'' {
                    let hex: String = chars.get(i + 1..i + 3).map(|h| h.iter().collect()).unwrap_or_default();
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        out.push(Token::Text(crate::sav::cp1252(&[byte]).chars().next().unwrap_or(' ')));
                    }
                    i += 2;
                } else if c == '~' {
                    out.push(Token::Text('\u{a0}'));
                } else if c == '\r' || c == '\n' {
                    out.push(Token::Word("par".to_owned(), None));
                } else if c != '-' && c != '*' {
                    out.push(Token::Text(c));
                }
            }
            c => out.push(Token::Text(c)),
        }
        i += 1;
    }
    out
}

/// What a group's text goes into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dest {
    Body,
    Footnote,
    Skip,
}

#[derive(Clone, Copy, Debug)]
struct Chars {
    bold: bool,
    italic: bool,
    color: Option<u16>,
    link: bool,
    hidden: bool,
    mark: bool,
    dest: Dest,
}

impl Default for Chars {
    fn default() -> Self {
        Self { bold: false, italic: false, color: None, link: false, hidden: false, mark: false, dest: Dest::Body }
    }
}

#[derive(Default)]
struct Builder {
    topics: Vec<Topic>,
    topic: Topic,
    para: Paragraph,
    text: String,
    text_style: Option<(bool, bool, Option<u16>, bool)>,
    footnote: String,
    row: Row,
    cellx: Vec<i32>,
    in_table: bool,
    cell: Vec<Paragraph>,
}

impl Builder {
    fn flush_text(&mut self) {
        let Some((bold, italic, color, link)) = self.text_style.take() else { return };
        let text = std::mem::take(&mut self.text);
        if text.is_empty() {
            return;
        }
        if link {
            self.para.runs.push(Run::Link { text, target: String::new(), bold });
        } else {
            self.para.runs.push(Run::Text { text, bold, italic, color });
        }
    }

    fn add(&mut self, c: char, chars: &Chars) {
        let style = (chars.bold, chars.italic, chars.color, chars.link);
        if self.text_style != Some(style) {
            self.flush_text();
            self.text_style = Some(style);
        }
        self.text.push(c);
    }

    /// Hidden text after a link names its target.
    fn hidden(&mut self, c: char) {
        self.flush_text();
        if let Some(Run::Link { target, .. }) = self.para.runs.last_mut() {
            target.push(c);
        }
    }

    fn end_paragraph(&mut self) {
        self.flush_text();
        let mut para = std::mem::take(&mut self.para);
        para.runs = pictures(para.runs);
        if self.in_table {
            self.cell.push(para);
        } else {
            self.topic.blocks.push(Block::Paragraph(para));
        }
        // The paragraph's measures stand until `\pard`.
        let p = &mut self.para;
        if let Some(Block::Paragraph(last)) = self.topic.blocks.last() {
            (p.align, p.left, p.right, p.first, p.before, p.after) =
                (last.align, last.left, last.right, last.first, last.before, last.after);
        }
    }

    fn end_cell(&mut self) {
        self.flush_text();
        let mut para = std::mem::take(&mut self.para);
        para.runs = pictures(para.runs);
        self.cell.push(para);
        let right = self.cellx.get(self.row.cells.len()).copied().unwrap_or(0);
        let paragraphs = std::mem::take(&mut self.cell);
        self.row.cells.push(Cell { right, paragraphs });
    }

    /// A row without a `\trowd` of its own keeps the one before's
    /// measures.
    fn end_row(&mut self) {
        let (left, gap) = (self.row.left, self.row.gap);
        let cells = std::mem::take(&mut self.row.cells);
        self.topic.blocks.push(Block::Row(Row { left, gap, cells }));
        self.in_table = false;
    }

    fn end_topic(&mut self) {
        self.flush_text();
        if !self.para.runs.is_empty() {
            self.end_paragraph();
        }
        let topic = std::mem::take(&mut self.topic);
        self.topics.push(topic);
    }

    fn footnote_done(&mut self) {
        let text = std::mem::take(&mut self.footnote);
        let text = text.trim();
        let mut chars = text.chars();
        let Some(mark) = chars.next() else { return };
        let value = chars.as_str().trim().to_owned();
        match mark {
            '#' => self.topic.id = value,
            '$' => self.topic.title = value,
            'K' => self.topic.keywords = value.split(';').map(|k| k.trim().to_owned()).collect(),
            '+' => self.topic.browse = value,
            _ => {}
        }
    }
}

/// Text runs holding `{bmc file}` or `{button …}` split around their
/// pictures. A button leads where the text after its second comma says,
/// without the closing bracket (the game's reader, `\{`).
fn pictures(runs: Vec<Run>) -> Vec<Run> {
    let mut out = Vec::new();
    for run in runs {
        let Run::Text { text, bold, italic, color } = run else {
            out.push(run);
            continue;
        };
        let mut rest = text.as_str();
        while let Some(at) = [rest.find("{bmc"), rest.find("{button")].into_iter().flatten().min() {
            let Some(end) = rest[at..].find('}') else { break };
            if at > 0 {
                out.push(Run::Text { text: rest[..at].to_owned(), bold, italic, color });
            }
            let inner = &rest[at + 1..at + end];
            if inner.starts_with("button") {
                let target = inner.splitn(3, ',').nth(2).unwrap_or("");
                out.push(Run::Button(target.trim().trim_end_matches([' ', ')']).to_owned()));
            } else {
                let file = inner.split_once(' ').map_or("", |(_, f)| f.trim());
                out.push(Run::Picture(file.to_owned()));
            }
            rest = &rest[at + end + 1..];
        }
        if !rest.is_empty() {
            out.push(Run::Text { text: rest.to_owned(), bold, italic, color });
        }
    }
    out
}

const SKIPPED: [&str; 6] = ["fonttbl", "colortbl", "stylesheet", "info", "pntext", "field"];

fn parse_rtf(text: &str) -> (Vec<Topic>, Vec<[u8; 3]>) {
    let tokens = tokens(text);
    let mut colors = Vec::new();
    // The colour table: `\red\green\blue;` each, the first empty.
    if let Some(start) = text.find("{\\colortbl") {
        let end = text[start..].find('}').map_or(text.len(), |e| start + e);
        for entry in text[start + 10..end].split(';') {
            let value = |name: &str| {
                entry.find(&format!("\\{name}")).and_then(|at| {
                    let digits: String =
                        entry[at + name.len() + 1..].chars().take_while(char::is_ascii_digit).collect();
                    digits.parse::<u8>().ok()
                })
            };
            if let (Some(r), Some(g), Some(b)) = (value("red"), value("green"), value("blue")) {
                colors.push([r, g, b]);
            }
        }
    }
    let mut b = Builder::default();
    let mut stack = vec![Chars::default()];
    let mut first_in_group = false;
    for token in tokens {
        let chars = *stack.last().unwrap();
        match token {
            Token::Open => {
                stack.push(chars);
                first_in_group = true;
                continue;
            }
            Token::Close => {
                let closed = stack.pop().unwrap_or_default();
                if closed.dest == Dest::Footnote && stack.last().is_some_and(|c| c.dest != Dest::Footnote) {
                    b.footnote_done();
                }
                if stack.is_empty() {
                    stack.push(Chars::default());
                }
            }
            Token::Word(word, n) => {
                let top = stack.last_mut().unwrap();
                match word.as_str() {
                    w if first_in_group && SKIPPED.contains(&w) => top.dest = Dest::Skip,
                    "footnote" => top.dest = Dest::Footnote,
                    _ if top.dest == Dest::Skip => {}
                    "b" => top.bold = n != Some(0),
                    "i" => top.italic = n != Some(0),
                    "cf" => top.color = n.map(|n| n as u16),
                    "uldb" | "ul" => top.link = n != Some(0),
                    "ulnone" => top.link = false,
                    "v" => top.hidden = n != Some(0),
                    "super" => top.mark = true,
                    "plain" => {
                        (top.bold, top.italic, top.color, top.link, top.hidden, top.mark) =
                            (false, false, None, false, false, false)
                    }
                    _ if top.dest == Dest::Footnote => {}
                    "par" => b.end_paragraph(),
                    "pard" => {
                        b.flush_text();
                        b.para.align = Align::Left;
                        (b.para.left, b.para.right, b.para.first, b.para.before, b.para.after) = (0, 0, 0, 0, 0);
                        b.in_table = false;
                    }
                    "qj" => b.para.align = Align::Justify,
                    "qr" => b.para.align = Align::Right,
                    "qc" => b.para.align = Align::Centre,
                    "ql" => b.para.align = Align::Left,
                    "li" => b.para.left = n.unwrap_or(0),
                    "ri" => b.para.right = n.unwrap_or(0),
                    "fi" => b.para.first = n.unwrap_or(0),
                    "sb" => b.para.before = n.unwrap_or(0),
                    "sa" => b.para.after = n.unwrap_or(0),
                    "intbl" => b.in_table = true,
                    "trowd" => {
                        b.row = Row::default();
                        b.cellx.clear();
                    }
                    "trleft" => b.row.left = n.unwrap_or(0),
                    "trgaph" => b.row.gap = n.unwrap_or(0),
                    "cellx" => b.cellx.push(n.unwrap_or(0)),
                    "cell" => b.end_cell(),
                    "row" => b.end_row(),
                    "page" => b.end_topic(),
                    "tab" => {
                        b.flush_text();
                        b.para.runs.push(Run::Tab);
                    }
                    "endash" => b.add('–', &chars),
                    "emdash" => b.add('—', &chars),
                    "lquote" => b.add('‘', &chars),
                    "rquote" => b.add('’', &chars),
                    "ldblquote" => b.add('“', &chars),
                    "rdblquote" => b.add('”', &chars),
                    "bullet" => b.add('•', &chars),
                    _ => {}
                }
            }
            Token::Text(c) => match chars.dest {
                Dest::Skip => {}
                Dest::Footnote => b.footnote.push(c),
                Dest::Body if chars.mark => {}
                Dest::Body if chars.hidden => b.hidden(c),
                Dest::Body => b.add(c, &chars),
            },
        }
        first_in_group = false;
    }
    b.end_topic();
    (b.topics.into_iter().filter(|t| !t.id.is_empty()).collect(), colors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_topic_keeps_its_footnotes_runs_and_links() {
        let rtf = r"{\rtf1{\colortbl;\red0\green0\blue0;\red128\green0\blue128;}
\pard {\super #{\footnote {\super #} IDD_X}}{\super ${\footnote {\super $} Titel}}{\b Kopf
\par }\pard\qj\li115\sa48 Ein {\uldb\cf2 Link}{\v IDD_Y} und \{bmc M17.bmp\} \{button ,JumpId(A.HLP,IDD_Z)\}.
\par \page {\super #{\footnote {\super #} Zwei}}Text\par}";
        let (topics, colors) = parse_rtf(rtf);
        assert_eq!(colors, [[0, 0, 0], [128, 0, 128]]);
        assert_eq!(topics.len(), 2);
        let t = &topics[0];
        assert_eq!((t.id.as_str(), t.title.as_str()), ("IDD_X", "Titel"));
        let Block::Paragraph(head) = &t.blocks[0] else { panic!() };
        assert_eq!(head.runs, [Run::Text { text: "Kopf".into(), bold: true, italic: false, color: None }]);
        let Block::Paragraph(p) = &t.blocks[1] else { panic!() };
        assert_eq!((p.align, p.left, p.after), (Align::Justify, 115, 48));
        assert_eq!(p.runs[1], Run::Link { text: "Link".into(), target: "IDD_Y".into(), bold: false });
        assert_eq!(p.runs[3], Run::Picture("M17.bmp".into()));
        assert_eq!(p.runs[5], Run::Button("IDD_Z".into()));
        assert_eq!(topics[1].id, "Zwei");
    }
}
