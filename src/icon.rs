//! Procedural app icon: a stone-green rounded square with a white "branch" glyph.
//!
//! The same artwork feeds the window icon (`rgba`) and the packaging icon file
//! (`write_png`), so the desktop entry can never drift from what the app shows.

const SAMPLES: u32 = 4;

/// Premultiplied-free RGBA8 buffer, `size * size * 4` bytes.
pub fn rgba(size: u32) -> Vec<u8> {
    let mut out = vec![0u8; (size * size * 4) as usize];
    let s = size as f32;

    for y in 0..size {
        for x in 0..size {
            let mut bg_hits = 0u32;
            let mut fg_hits = 0u32;
            let mut color_acc = [0f32; 3];

            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let px = (x as f32 + (sx as f32 + 0.5) / SAMPLES as f32) / s;
                    let py = (y as f32 + (sy as f32 + 0.5) / SAMPLES as f32) / s;

                    if in_rounded_rect(px, py, 0.02, 0.98, 0.02, 0.98, 0.235) {
                        bg_hits += 1;
                        let t = ((px + py) * 0.5).clamp(0.0, 1.0);
                        let c = lerp3([0.157, 0.800, 0.580], [0.031, 0.333, 0.251], t);
                        color_acc[0] += c[0];
                        color_acc[1] += c[1];
                        color_acc[2] += c[2];
                    }
                    if in_glyph(px, py) {
                        fg_hits += 1;
                    }
                }
            }

            let total = (SAMPLES * SAMPLES) as f32;
            let bg_a = bg_hits as f32 / total;
            let fg_a = fg_hits as f32 / total;

            let bg_rgb = if bg_hits > 0 {
                [
                    color_acc[0] / bg_hits as f32,
                    color_acc[1] / bg_hits as f32,
                    color_acc[2] / bg_hits as f32,
                ]
            } else {
                [0.0, 0.0, 0.0]
            };

            // White glyph over the green plate; alpha is the union of both.
            let alpha = bg_a.max(fg_a);
            let rgb = if alpha > 0.0 {
                let w = (fg_a / alpha).clamp(0.0, 1.0);
                lerp3(bg_rgb, [1.0, 1.0, 1.0], w)
            } else {
                [0.0, 0.0, 0.0]
            };

            let i = ((y * size + x) * 4) as usize;
            out[i] = to_u8(rgb[0]);
            out[i + 1] = to_u8(rgb[1]);
            out[i + 2] = to_u8(rgb[2]);
            out[i + 3] = to_u8(alpha);
        }
    }

    out
}

fn in_glyph(x: f32, y: f32) -> bool {
    const TRUNK_X: f32 = 0.34;
    const TOP_Y: f32 = 0.27;
    const BOTTOM_Y: f32 = 0.73;
    const LEAF_X: f32 = 0.70;
    const MID_Y: f32 = 0.50;
    const NODE_R: f32 = 0.088;
    const BAR: f32 = 0.038;

    in_circle(x, y, TRUNK_X, TOP_Y, NODE_R)
        || in_circle(x, y, TRUNK_X, BOTTOM_Y, NODE_R)
        || in_circle(x, y, LEAF_X, MID_Y, NODE_R)
        || in_capsule(x, y, TRUNK_X, TOP_Y, TRUNK_X, BOTTOM_Y, BAR)
        || in_capsule(x, y, TRUNK_X, MID_Y, LEAF_X, MID_Y, BAR)
}

fn in_rounded_rect(x: f32, y: f32, x0: f32, x1: f32, y0: f32, y1: f32, r: f32) -> bool {
    if x < x0 || x > x1 || y < y0 || y > y1 {
        return false;
    }
    let cx = x.clamp(x0 + r, x1 - r);
    let cy = y.clamp(y0 + r, y1 - r);
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= r * r
}

fn in_circle(x: f32, y: f32, cx: f32, cy: f32, r: f32) -> bool {
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= r * r
}

fn in_capsule(x: f32, y: f32, ax: f32, ay: f32, bx: f32, by: f32, r: f32) -> bool {
    let (vx, vy) = (bx - ax, by - ay);
    let (wx, wy) = (x - ax, y - ay);
    let len2 = vx * vx + vy * vy;
    let t = if len2 <= f32::EPSILON {
        0.0
    } else {
        ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0)
    };
    let dx = wx - vx * t;
    let dy = wy - vy * t;
    dx * dx + dy * dy <= r * r
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Writes the icon as a PNG without pulling in an encoder: the zlib stream uses
/// stored (uncompressed) deflate blocks, which every PNG reader accepts.
pub fn write_png(path: &str, size: u32) -> std::io::Result<()> {
    let pixels = rgba(size);

    let mut raw = Vec::with_capacity((size * (size * 4 + 1)) as usize);
    for y in 0..size {
        raw.push(0u8); // filter type: none
        let start = (y * size * 4) as usize;
        raw.extend_from_slice(&pixels[start..start + (size * 4) as usize]);
    }

    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&size.to_be_bytes());
    ihdr.extend_from_slice(&size.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    write_chunk(&mut png, b"IHDR", &ihdr);
    write_chunk(&mut png, b"IDAT", &zlib_stored(&raw));
    write_chunk(&mut png, b"IEND", &[]);

    std::fs::write(path, png)
}

fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01]; // deflate, default window, no dictionary
    let mut chunks = data.chunks(0xFFFF).peekable();
    if data.is_empty() {
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    }
    while let Some(chunk) = chunks.next() {
        let last = chunks.peek().is_none();
        out.push(if last { 1 } else { 0 });
        let len = chunk.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(chunk);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_has_expected_size_and_opaque_center() {
        let size = 64;
        let buf = rgba(size);
        assert_eq!(buf.len() as u32, size * size * 4);
        let center = ((size / 2 * size + size / 2) * 4) as usize;
        assert_eq!(buf[center + 3], 255, "icon center must be opaque");
    }

    #[test]
    fn corners_are_transparent() {
        let buf = rgba(64);
        assert_eq!(buf[3], 0, "top-left corner must be rounded away");
    }

    #[test]
    fn png_has_signature_and_iend() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("icon.png");
        write_png(path.to_str().unwrap(), 32).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        );
        assert!(bytes.ends_with(&[0xAE, 0x42, 0x60, 0x82]), "IEND crc tail");
    }
}
