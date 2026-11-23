//! Reader for the RIFF `AVI ` videos of ANSTOSS 3 (`videos/*.AVI`: the extro,
//! the draw and the promotion). Each holds one video stream — Intel Indeo 3
//! (`IV32`) — and one stream of plain PCM sound. This reads the container
//! only: the stream headers, every video chunk in order and the sound joined
//! into one WAV file; decoding the frames is the codec's business.
//!
//! Layout: `RIFF AVI ` with a `LIST hdrl` (the `avih` main header, then one
//! `LIST strl` per stream: `strh` with type, handler, scale, rate and length,
//! and `strf` with the format — a `BITMAPINFOHEADER` for video, a
//! `WAVEFORMATEX` for sound), then `LIST movi` with chunks named by stream
//! number and kind (`00dc`, `01wb`), possibly grouped in `LIST rec `.

use std::io;
use std::ops::Range;
use std::path::Path;

use crate::cpr::{invalid, u32_at};

pub struct Avi {
    pub width: u32,
    pub height: u32,
    /// The video codec's four characters, `IV32` for every game video.
    pub codec: [u8; 4],
    /// Frames a second as the stream header's rate over its scale.
    pub rate: u32,
    pub scale: u32,
    data: Vec<u8>,
    /// One range per video chunk, in order; an empty one repeats the frame
    /// before.
    frames: Vec<Range<usize>>,
    /// The sound's `WAVEFORMATEX`, if the file has a sound stream.
    sound_format: Option<Range<usize>>,
    sound: Vec<Range<usize>>,
}

struct Stream {
    kind: [u8; 4],
    handler: [u8; 4],
    scale: u32,
    rate: u32,
    format: Range<usize>,
}

fn four(data: &[u8], pos: usize) -> io::Result<[u8; 4]> {
    data.get(pos..pos + 4).map(|b| b.try_into().unwrap()).ok_or_else(|| invalid("truncated file"))
}

/// The chunks within `range` as `(id, body)`, each padded to an even length.
fn chunks(data: &[u8], range: Range<usize>) -> io::Result<Vec<([u8; 4], Range<usize>)>> {
    let mut out = Vec::new();
    let mut pos = range.start;
    while pos + 8 <= range.end {
        let id = four(data, pos)?;
        let size = u32_at(data, pos + 4)? as usize;
        let body = pos + 8..pos + 8 + size;
        if body.end > range.end {
            return Err(invalid("chunk runs past its list"));
        }
        out.push((id, body.clone()));
        pos = body.end + (size & 1);
    }
    Ok(out)
}

impl Avi {
    pub fn open(path: &Path) -> io::Result<Self> {
        Self::parse(std::fs::read(path)?)
    }

    pub fn parse(data: Vec<u8>) -> io::Result<Self> {
        if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"AVI " {
            return Err(invalid("not an AVI file"));
        }
        let riff_end = (8 + u32_at(&data, 4)? as usize).min(data.len());
        let lists = chunks(&data, 12..riff_end)?;
        let list = |name: &[u8; 4]| {
            lists.iter().find_map(|(id, body)| {
                (id == b"LIST" && data.get(body.start..body.start + 4) == Some(name.as_slice()))
                    .then(|| body.start + 4..body.end)
            })
        };
        let hdrl = list(b"hdrl").ok_or_else(|| invalid("AVI without header list"))?;
        let movi = list(b"movi").ok_or_else(|| invalid("AVI without movie list"))?;

        let mut streams = Vec::new();
        for (id, body) in chunks(&data, hdrl)? {
            if id != *b"LIST" || four(&data, body.start)? != *b"strl" {
                continue;
            }
            let parts = chunks(&data, body.start + 4..body.end)?;
            let find = |name: &[u8; 4]| parts.iter().find(|(id, _)| id == name).map(|(_, b)| b.clone());
            let strh = find(b"strh").ok_or_else(|| invalid("stream without header"))?;
            let format = find(b"strf").ok_or_else(|| invalid("stream without format"))?;
            streams.push(Stream {
                kind: four(&data, strh.start)?,
                handler: four(&data, strh.start + 4)?,
                scale: u32_at(&data, strh.start + 20)?,
                rate: u32_at(&data, strh.start + 24)?,
                format,
            });
        }
        let video = streams
            .iter()
            .position(|s| &s.kind == b"vids")
            .ok_or_else(|| invalid("AVI without video stream"))?;
        let sound = streams.iter().position(|s| &s.kind == b"auds");
        let v = &streams[video];
        if v.format.len() < 20 || v.scale == 0 || v.rate == 0 {
            return Err(invalid("video stream without size or rate"));
        }
        let (width, height) = (u32_at(&data, v.format.start + 4)?, u32_at(&data, v.format.start + 8)?);
        // BITMAPINFOHEADER's biCompression names the codec where the
        // handler is left blank.
        let codec = if v.handler == [0; 4] { four(&data, v.format.start + 16)? } else { v.handler };
        let codec = codec.map(|b| b.to_ascii_uppercase());

        // Chunk ids start with the stream's number in two decimal digits; the
        // game's Indeo videos name their frames `00iv`.
        let stream_of = |id: &[u8; 4]| -> Option<usize> {
            let (a, b) = (id[0].wrapping_sub(b'0'), id[1].wrapping_sub(b'0'));
            (a < 10 && b < 10).then_some(usize::from(a) * 10 + usize::from(b))
        };
        let (mut frames, mut sound_chunks) = (Vec::new(), Vec::new());
        let mut pending = vec![movi];
        while let Some(range) = pending.pop() {
            // Walked depth first, so a `rec ` list is taken in place.
            let mut inner = chunks(&data, range)?;
            inner.reverse();
            let mut stack = inner;
            while let Some((id, body)) = stack.pop() {
                if id == *b"LIST" {
                    let mut nested = chunks(&data, body.start + 4..body.end)?;
                    nested.reverse();
                    stack.extend(nested);
                } else if stream_of(&id) == Some(video) && matches!(&id[2..], b"dc" | b"db" | b"iv") {
                    frames.push(body);
                } else if stream_of(&id) == sound && sound.is_some() && id[2..] == *b"wb" {
                    sound_chunks.push(body);
                }
            }
        }
        if frames.is_empty() {
            return Err(invalid("video has no frames"));
        }
        Ok(Self {
            width,
            height,
            codec,
            rate: v.rate,
            scale: v.scale,
            sound_format: sound.map(|s| streams[s].format.clone()),
            data,
            frames,
            sound: sound_chunks,
        })
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Frame `i`'s compressed bytes; empty where it repeats the frame before.
    pub fn frame(&self, i: usize) -> &[u8] {
        &self.data[self.frames[i].clone()]
    }

    /// Seconds per frame.
    pub fn frame_secs(&self) -> f64 {
        f64::from(self.scale) / f64::from(self.rate)
    }

    /// The whole sound stream as a WAV file in memory, its format as the
    /// AVI states it; `None` for a silent video.
    pub fn sound_wav(&self) -> Option<Vec<u8>> {
        let format = &self.data[self.sound_format.clone()?];
        let len: usize = self.sound.iter().map(|r| r.len()).sum();
        let mut out = Vec::with_capacity(28 + format.len() + len);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&((20 + format.len() + len) as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&(format.len() as u32).to_le_bytes());
        out.extend_from_slice(format);
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(len as u32).to_le_bytes());
        for r in &self.sound {
            out.extend_from_slice(&self.data[r.clone()]);
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn reads_the_extro() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let avi = Avi::open(&dir.join("videos/EXTRO.AVI")).unwrap();
        assert_eq!((avi.width, avi.height, &avi.codec), (400, 300, b"IV32"));
        // The stream header's length: 600 frames, 80 seconds.
        assert_eq!(avi.frame_count(), 600);
        assert!((avi.frame_secs() - 1.0 / 7.5).abs() < 1e-6);
        let wav = avi.sound_wav().unwrap();
        // Mono 16-bit PCM at 22050 Hz.
        assert_eq!(&wav[20..24], &[1, 0, 1, 0]);
        assert_eq!(u32_at(&wav, 24).unwrap(), 22050);
        assert!(wav.len() > 1_000_000);
        // Every Indeo 3 frame: 16 bytes of frame header, then version 32 and
        // the picture's height and width at +28 and +30.
        let first = avi.frame(0);
        let le16 = |at: usize| u16::from_le_bytes([first[at], first[at + 1]]);
        assert_eq!((le16(16), le16(28), le16(30)), (32, 300, 400));
    }

    #[test]
    fn walks_a_minimal_file() {
        let mut body = Vec::new();
        let chunk = |out: &mut Vec<u8>, id: &[u8], data: &[u8]| {
            out.extend_from_slice(id);
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(data);
            if data.len() % 2 == 1 {
                out.push(0);
            }
        };
        let mut strh = vec![0u8; 56];
        strh[0..4].copy_from_slice(b"vids");
        strh[4..8].copy_from_slice(b"iv32");
        strh[20..24].copy_from_slice(&2u32.to_le_bytes());
        strh[24..28].copy_from_slice(&15u32.to_le_bytes());
        let mut strf = vec![0u8; 40];
        strf[4..8].copy_from_slice(&8u32.to_le_bytes());
        strf[8..12].copy_from_slice(&6u32.to_le_bytes());
        let mut strl = b"strl".to_vec();
        chunk(&mut strl, b"strh", &strh);
        chunk(&mut strl, b"strf", &strf);
        let mut hdrl = b"hdrl".to_vec();
        chunk(&mut hdrl, b"avih", &[0; 56]);
        chunk(&mut hdrl, b"LIST", &strl);
        let mut rec = b"rec ".to_vec();
        chunk(&mut rec, b"00dc", b"abc");
        let mut movi = b"movi".to_vec();
        chunk(&mut movi, b"LIST", &rec);
        chunk(&mut movi, b"00dc", b"");
        chunk(&mut body, b"LIST", &hdrl);
        chunk(&mut body, b"LIST", &movi);
        let mut file = b"RIFF".to_vec();
        file.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
        file.extend_from_slice(b"AVI ");
        file.extend_from_slice(&body);

        let avi = Avi::parse(file).unwrap();
        assert_eq!((avi.width, avi.height, &avi.codec), (8, 6, b"IV32"));
        assert_eq!(avi.frame_count(), 2);
        assert_eq!((avi.frame(0), avi.frame(1)), (&b"abc"[..], &b""[..]));
        assert!(avi.sound_wav().is_none());
    }
}
