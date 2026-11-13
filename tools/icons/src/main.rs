//! Draws Snowfall's snowflake and writes the tray glyphs and the app icon:
//! `cargo run -p icons` from the workspace root regenerates everything under `windows/assets/`.
//!
//! The snowflake is a signed-distance shape (six arms, each with two pairs of branches, round tips) rendered with
//! supersampling, so every size is drawn from the shape rather than scaled from a bitmap.

use std::f32::consts::PI;
use std::fs;
use std::path::Path;

const TRAY_SIZES: [u32; 7] = [16, 20, 24, 32, 40, 48, 64];
const ICON_SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

fn main() {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../windows/assets");
    fs::create_dir_all(assets.join("tray")).expect("assets folder");
    for size in TRAY_SIZES {
        for (name, color) in [("white", [255, 255, 255]), ("black", [0, 0, 0])] {
            let pixels = render(size, |coverage, _| [color[0], color[1], color[2], to_byte(coverage)], 0.9);
            fs::write(assets.join(format!("tray/snowflake-{name}-{size}.png")), png(size, &pixels)).expect("tray glyph");
        }
    }
    let images: Vec<(u32, Vec<u8>)> = ICON_SIZES.iter().map(|&size| (size, png(size, &app_icon(size)))).collect();
    fs::write(assets.join("Snowfall.ico"), ico(&images)).expect("app icon");
    println!("wrote tray glyphs and Snowfall.ico to {}", assets.display());
}

/// The app icon: a white snowflake on a rounded square of winter-sky blue, lighter at the top.
fn app_icon(size: u32) -> Vec<u8> {
    let flake = render(size, |coverage, _| [255, 255, 255, to_byte(coverage)], 0.74);
    let mut out = Vec::with_capacity(flake.len());
    let n = size as f32;
    for y in 0..size {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            // The tile, antialiased over a pixel.
            let (px, py) = (x as f32 + 0.5 - n / 2.0, y as f32 + 0.5 - n / 2.0);
            let half = n * 0.47;
            let radius = n * 0.2;
            let q = (px.abs() - (half - radius)).max(0.0).hypot((py.abs() - (half - radius)).max(0.0)) - radius;
            let tile = (0.5 - q).clamp(0.0, 1.0);
            let t = y as f32 / n;
            let top = [96.0, 165.0, 235.0];
            let bottom = [40.0, 96.0, 186.0];
            let sky: [f32; 3] = std::array::from_fn(|c| top[c] + (bottom[c] - top[c]) * t);
            let a = flake[i + 3] as f32 / 255.0;
            let rgb: [f32; 3] = std::array::from_fn(|c| sky[c] * (1.0 - a) + 255.0 * a);
            // Straight alpha: the flake only shows where the tile is.
            out.extend([rgb[0] as u8, rgb[1] as u8, rgb[2] as u8, to_byte(tile)]);
        }
    }
    out
}

/// Renders the snowflake, `scale` of the image across, colouring each pixel from its coverage.
fn render(size: u32, color: impl Fn(f32, (u32, u32)) -> [u8; 4], scale: f32) -> Vec<u8> {
    const SAMPLES: u32 = 6;
    let n = size as f32;
    // Strokes no thinner than about a pixel and a quarter, so small glyphs stay legible.
    let stroke = (0.075 * scale).max(1.25 / n);
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let mut hits = 0;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    // Centred, y up, one unit across the image.
                    let u = (x as f32 + (sx as f32 + 0.5) / SAMPLES as f32) / n - 0.5;
                    let v = 0.5 - (y as f32 + (sy as f32 + 0.5) / SAMPLES as f32) / n;
                    if snowflake(u / scale, v / scale, stroke / scale) <= 0.0 {
                        hits += 1;
                    }
                }
            }
            pixels.extend(color(hits as f32 / (SAMPLES * SAMPLES) as f32, (x, y)));
        }
    }
    pixels
}

/// Signed distance to the snowflake, in units of its diameter: six arms, two pairs of branches on each.
fn snowflake(x: f32, y: f32, stroke: f32) -> f32 {
    let half = stroke / 2.0;
    // Fold into one arm's sector, pointing along +x.
    let sector = PI / 3.0;
    let angle = y.atan2(x).rem_euclid(sector * 2.0 * 3.0);
    let a = (angle + sector / 2.0).rem_euclid(sector) - sector / 2.0;
    let r = x.hypot(y);
    let (px, py) = (r * a.cos(), (r * a.sin()).abs());
    let arm = segment(px, py, 0.0, 0.0, 0.46, 0.0) - half;
    let branch_angle = PI / 3.0;
    let mut d = arm;
    for (at, length) in [(0.2, 0.13), (0.33, 0.09)] {
        let (bx, by) = (at + length * branch_angle.cos(), length * branch_angle.sin());
        d = d.min(segment(px, py, at, 0.0, bx, by) - half);
    }
    // A small hexagonal heart.
    d.min(r - stroke * 1.1)
}

fn segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (dx, dy) = (bx - ax, by - ay);
    let t = (((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    (px - ax - dx * t).hypot(py - ay - dy * t)
}

fn to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A PNG of straight-alpha RGBA pixels, deflated with stored blocks: these images are tiny.
fn png(size: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(rgba.len() + size as usize);
    for row in rgba.chunks(size as usize * 4) {
        raw.push(0);
        raw.extend(row);
    }
    let mut zlib = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, block) in blocks.iter().enumerate() {
        zlib.push((i + 1 == blocks.len()) as u8);
        zlib.extend((block.len() as u16).to_le_bytes());
        zlib.extend((!(block.len() as u16)).to_le_bytes());
        zlib.extend(*block);
    }
    zlib.extend(adler32(&raw).to_be_bytes());

    let mut header = Vec::new();
    header.extend(size.to_be_bytes());
    header.extend(size.to_be_bytes());
    header.extend([8, 6, 0, 0, 0]);
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    for (kind, data) in [(b"IHDR", header.as_slice()), (b"IDAT", zlib.as_slice()), (b"IEND", &[][..])] {
        out.extend((data.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend(kind);
        out.extend(data);
        let crc = crc32(&out[start..]);
        out.extend(crc.to_be_bytes());
    }
    out
}

/// An .ico holding each image as a PNG, which Windows reads at every size.
fn ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0, 0, 1, 0];
    out.extend((images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, data) in images {
        let side = if *size >= 256 { 0 } else { *size as u8 };
        out.extend([side, side, 0, 0]);
        out.extend(1u16.to_le_bytes());
        out.extend(32u16.to_le_bytes());
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for (_, data) in images {
        out.extend(data);
    }
    out
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}
