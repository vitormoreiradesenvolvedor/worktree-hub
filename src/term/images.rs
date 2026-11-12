//! Pictures drawn on top of the terminal grid.
//!
//! A terminal has no cell for a picture: a program sends one in an escape
//! sequence and the emulator paints it over the text. This module reads the
//! kitty graphics protocol (`ESC _ G …控制; base64 …ESC \`) out of the PTY
//! stream, decodes the picture and remembers which line of the scrollback it
//! was dropped on, so it scrolls with the text that came with it.

use std::sync::Mutex;

use egui::{Context, TextureHandle, TextureOptions};

/// A picture that arrived whole.
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// `i=` in the protocol: the id the sender wants to be acknowledged with.
    pub id: Option<u32>,
}

/// What the scanner hands back, in the order it was read.
pub enum Piece {
    /// Ordinary terminal bytes, for the emulator.
    Bytes(Vec<u8>),
    /// A picture to drop at the cursor.
    Picture(Box<Picture>),
    /// `a=d`: the sender took its pictures back.
    Clear,
}

/// Pulls kitty graphics commands out of a byte stream, passing everything else
/// through untouched. It survives chunk boundaries: a sequence may be split
/// anywhere, and a picture may arrive over many sequences (`m=1`).
#[derive(Default)]
pub struct Scanner {
    state: State,
    /// Payload of the sequence being read.
    apc: Vec<u8>,
    /// Base64 collected so far, across `m=1` sequences.
    payload: Vec<u8>,
    /// Keys of the sequence that started the transfer.
    control: Control,
    collecting: bool,
}

#[derive(Default, PartialEq)]
enum State {
    #[default]
    Text,
    /// Saw `ESC`.
    Escape,
    /// Inside `ESC _ … `.
    Apc,
    /// Inside the sequence and saw `ESC`, which may start the terminator.
    ApcEscape,
}

#[derive(Clone, Copy, Default)]
struct Control {
    action: u8,
    format: u32,
    width: u32,
    height: u32,
    zlib: bool,
    more: bool,
    id: Option<u32>,
    quiet: u32,
}

impl Scanner {
    pub fn feed(&mut self, input: &[u8]) -> Vec<Piece> {
        let mut out: Vec<Piece> = Vec::new();
        let mut text: Vec<u8> = Vec::with_capacity(input.len());

        for &byte in input {
            match self.state {
                State::Text => {
                    if byte == 0x1b {
                        self.state = State::Escape;
                    } else {
                        text.push(byte);
                    }
                }
                State::Escape => {
                    if byte == b'_' {
                        self.state = State::Apc;
                        self.apc.clear();
                    } else if byte == 0x1b {
                        // Two in a row: the first was not the start of anything.
                        text.push(0x1b);
                    } else {
                        // Not ours: hand the escape back as it came.
                        text.push(0x1b);
                        text.push(byte);
                        self.state = State::Text;
                    }
                }
                State::Apc => match byte {
                    0x1b => self.state = State::ApcEscape,
                    0x07 => {
                        flush(&mut text, &mut out);
                        self.finish(&mut out);
                        self.state = State::Text;
                    }
                    _ => self.apc.push(byte),
                },
                State::ApcEscape => {
                    if byte == b'\\' {
                        flush(&mut text, &mut out);
                        self.finish(&mut out);
                        self.state = State::Text;
                    } else {
                        self.apc.push(0x1b);
                        self.apc.push(byte);
                        self.state = State::Apc;
                    }
                }
            }
        }

        flush(&mut text, &mut out);
        out
    }

    /// Reads one finished `ESC _ … ESC \` sequence.
    fn finish(&mut self, out: &mut Vec<Piece>) {
        let apc = std::mem::take(&mut self.apc);
        let Some((&b'G', rest)) = apc.split_first() else {
            // Some other APC sequence: the emulator ignores those anyway.
            return;
        };
        let (control, data) = match rest.iter().position(|&b| b == b';') {
            Some(at) => (&rest[..at], &rest[at + 1..]),
            None => (rest, &[][..]),
        };
        let control = parse_control(control);

        if control.action == b'd' {
            out.push(Piece::Clear);
            return;
        }
        if !matches!(control.action, b't' | b'T' | 0) {
            // Queries and placements of pictures we never stored: nothing to do.
            return;
        }

        if !self.collecting {
            self.control = control;
            self.collecting = true;
            self.payload.clear();
        }
        self.payload.extend_from_slice(data);
        if control.more {
            return;
        }

        self.collecting = false;
        let control = self.control;
        let payload = std::mem::take(&mut self.payload);
        if let Some(picture) = decode(control, &payload) {
            out.push(Piece::Picture(Box::new(picture)));
        }
    }
}

/// The text read so far goes out before the picture that follows it: the
/// cursor has to be where the program left it when the picture lands.
fn flush(text: &mut Vec<u8>, out: &mut Vec<Piece>) {
    if !text.is_empty() {
        out.push(Piece::Bytes(std::mem::take(text)));
    }
}

fn parse_control(text: &[u8]) -> Control {
    let mut control = Control {
        format: 32,
        ..Default::default()
    };
    for pair in text.split(|&b| b == b',') {
        let Some(at) = pair.iter().position(|&b| b == b'=') else {
            continue;
        };
        let key = &pair[..at];
        let value = String::from_utf8_lossy(&pair[at + 1..]).into_owned();
        match key {
            b"a" => control.action = value.as_bytes().first().copied().unwrap_or(0),
            b"f" => control.format = value.parse().unwrap_or(32),
            b"s" => control.width = value.parse().unwrap_or(0),
            b"v" => control.height = value.parse().unwrap_or(0),
            b"o" => control.zlib = value == "z",
            b"m" => control.more = value == "1",
            b"i" => control.id = value.parse().ok(),
            b"q" => control.quiet = value.parse().unwrap_or(0),
            _ => {}
        }
    }
    control
}

fn decode(control: Control, payload: &[u8]) -> Option<Picture> {
    let raw = base64(payload)?;
    let raw = if control.zlib {
        miniz_oxide::inflate::decompress_to_vec_zlib(&raw).ok()?
    } else {
        raw
    };

    match control.format {
        100 => {
            let decoded =
                image::load_from_memory_with_format(&raw, image::ImageFormat::Png).ok()?;
            let rgba = decoded.to_rgba8();
            Some(Picture {
                width: rgba.width(),
                height: rgba.height(),
                rgba: rgba.into_raw(),
                id: control.id,
            })
        }
        24 | 32 => {
            let (width, height) = (control.width, control.height);
            if width == 0 || height == 0 {
                return None;
            }
            let pixels = (width as usize) * (height as usize);
            let rgba = if control.format == 32 {
                if raw.len() < pixels * 4 {
                    return None;
                }
                raw
            } else {
                if raw.len() < pixels * 3 {
                    return None;
                }
                let mut out = Vec::with_capacity(pixels * 4);
                for chunk in raw.chunks_exact(3).take(pixels) {
                    out.extend_from_slice(chunk);
                    out.push(255);
                }
                out
            };
            Some(Picture {
                width,
                height,
                rgba,
                id: control.id,
            })
        }
        _ => None,
    }
}

fn base64(input: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for &byte in input {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\r' | b'\n' | b' ' | b'\t' => continue,
            _ => return None,
        } as u32;
        acc = (acc << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// A picture sitting on a line of the scrollback.
pub struct Placed {
    /// Counted from the oldest line the scrollback still holds.
    pub line: i64,
    pub col: usize,
    pub width: f32,
    pub height: f32,
    size: [usize; 2],
    /// Dropped once the GPU has a copy.
    rgba: Vec<u8>,
    texture: Option<TextureHandle>,
}

impl Placed {
    /// Uploads on first use and keeps the handle afterwards.
    pub fn texture(&mut self, ctx: &Context) -> TextureHandle {
        if let Some(texture) = &self.texture {
            return texture.clone();
        }
        let image = egui::ColorImage::from_rgba_unmultiplied(self.size, &self.rgba);
        let texture = ctx.load_texture("term-image", image, TextureOptions::LINEAR);
        self.rgba = Vec::new();
        self.texture = Some(texture.clone());
        texture
    }
}

/// Every picture a pane is showing. The reader thread fills it, the widget
/// paints from it.
#[derive(Default)]
pub struct Album {
    pub items: Vec<Placed>,
}

/// More than this and the oldest picture goes, so a program spraying images
/// cannot grow the pane's memory without bound.
const MAX_PICTURES: usize = 24;

impl Album {
    pub fn place(&mut self, picture: &Picture, line: i64, col: usize) {
        if picture.width == 0 || picture.height == 0 {
            return;
        }
        self.items.push(Placed {
            line,
            col,
            width: picture.width as f32,
            height: picture.height as f32,
            size: [picture.width as usize, picture.height as usize],
            rgba: picture.rgba.clone(),
            texture: None,
        });
        if self.items.len() > MAX_PICTURES {
            self.items.remove(0);
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Forgets the pictures whose line has fallen off the end of the scrollback.
    pub fn cull(&mut self, oldest: i64) {
        self.items.retain(|placed| placed.line >= oldest);
    }
}

pub type SharedAlbum = std::sync::Arc<Mutex<Album>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn kitty(control: &str, payload: &str) -> Vec<u8> {
        format!("\x1b_G{control};{payload}\x1b\\").into_bytes()
    }

    #[test]
    fn plain_text_passes_through_untouched() {
        let mut scanner = Scanner::default();
        let pieces = scanner.feed(b"hello \x1b[31mworld\x1b[m");
        assert_eq!(pieces.len(), 1);
        match &pieces[0] {
            Piece::Bytes(bytes) => assert_eq!(bytes, b"hello \x1b[31mworld\x1b[m"),
            _ => panic!("the stream was not handed back"),
        }
    }

    #[test]
    fn a_raw_rgba_picture_is_decoded() {
        // Two pixels, red then green, base64 of the RGBA bytes.
        let data = [255u8, 0, 0, 255, 0, 255, 0, 255];
        let mut scanner = Scanner::default();
        let pieces = scanner.feed(&kitty("a=T,f=32,s=2,v=1", &to_base64(&data)));
        let picture = pieces
            .iter()
            .find_map(|piece| match piece {
                Piece::Picture(picture) => Some(picture),
                _ => None,
            })
            .expect("no picture came out");
        assert_eq!((picture.width, picture.height), (2, 1));
        assert_eq!(picture.rgba, data);
    }

    #[test]
    fn a_picture_split_over_chunks_still_arrives() {
        let data = [1u8, 2, 3, 4];
        let encoded = to_base64(&data);
        let (head, tail) = encoded.split_at(4);
        let mut scanner = Scanner::default();

        let first = scanner.feed(&kitty("a=T,f=32,s=1,v=1,m=1", head));
        assert!(!first.iter().any(|p| matches!(p, Piece::Picture(_))));

        // And the terminator lands in a later read, as it does over a PTY.
        let rest = kitty("m=0", tail);
        let (a, b) = rest.split_at(rest.len() - 3);
        let mut pieces = scanner.feed(a);
        pieces.extend(scanner.feed(b));
        let picture = pieces
            .iter()
            .find_map(|piece| match piece {
                Piece::Picture(picture) => Some(picture),
                _ => None,
            })
            .expect("the split picture never completed");
        assert_eq!(picture.rgba, data);
    }

    #[test]
    fn delete_clears_the_album() {
        let mut scanner = Scanner::default();
        let pieces = scanner.feed(b"\x1b_Ga=d\x1b\\");
        assert!(pieces.iter().any(|piece| matches!(piece, Piece::Clear)));
    }

    #[test]
    fn pictures_older_than_the_scrollback_are_forgotten() {
        let mut album = Album::default();
        let picture = Picture {
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 255],
            id: None,
        };
        album.place(&picture, 5, 0);
        album.place(&picture, 50, 0);
        album.cull(10);
        assert_eq!(album.items.len(), 1);
        assert_eq!(album.items[0].line, 50);
    }

    fn to_base64(data: &[u8]) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in data.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            for shift in [18, 12, 6, 0] {
                out.push(ALPHABET[((n >> shift) & 63) as usize] as char);
            }
            let pad = 3 - chunk.len();
            if pad > 0 {
                out.truncate(out.len() - pad);
                out.extend(std::iter::repeat_n('=', pad));
            }
        }
        out
    }
}
