// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Diff images for golden mismatches, written to `target/gm-diffs/<oracle tier>/<config>/`:
//!
//! - `<name>.png`: three panels side by side: the golden, ours, and the difference (magenta
//!   where the stored bytes of a pixel differ, the golden darkened elsewhere). Colors are shown
//!   as stored (premultiplied, i.e. composited over black; 565 and f16 converted to 8 bits).
//! - `<name>.raw`: our bytes, in the oracle's layout (`cargo xtask oracle compare` reads these).
//! - `<name>.golden.raw`: the golden bytes.

use std::path::{Path, PathBuf};

use skia_rust_core::color_type::ColorType;
use skia_rust_core::half::half_to_float;

use crate::sink::Config;

/// Where diff images go: `target/gm-diffs`, or `$SKIA_RUST_GM_DIFFS`.
#[must_use]
pub fn diff_root() -> PathBuf {
    std::env::var_os("SKIA_RUST_GM_DIFFS").map_or_else(
        || crate::goldens::target_dir().join("gm-diffs"),
        PathBuf::from,
    )
}

/// Converts one pixel result to 8-bit RGB (alpha forced opaque), as stored.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped to [0, 255.5)
fn to_rgb8(config: Config, bytes: &[u8]) -> Vec<[u8; 3]> {
    let bpp = config.color_type().bytes_per_pixel();
    bytes
        .chunks_exact(bpp)
        .map(|p| match config {
            Config::N32 if config.color_type() == ColorType::BGRA8888 => [p[2], p[1], p[0]],
            Config::N32 => [p[0], p[1], p[2]],
            Config::Rgb565 => {
                let v = u16::from_le_bytes([p[0], p[1]]);
                let (r, g, b) = ((v >> 11) & 0x1f, (v >> 5) & 0x3f, v & 0x1f);
                [
                    ((r << 3) | (r >> 2)) as u8,
                    ((g << 2) | (g >> 4)) as u8,
                    ((b << 3) | (b >> 2)) as u8,
                ]
            }
            Config::F16 => {
                let c = |i: usize| {
                    let f = half_to_float(u16::from_le_bytes([p[2 * i], p[2 * i + 1]]));
                    (f.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
                };
                [c(0), c(1), c(2)]
            }
        })
        .collect()
}

/// Writes the diff image and raw files for result `name` (see the module docs) under
/// `root/<oracle_tier>/<config>/`, and returns the PNG's path.
///
/// # Errors
/// If the byte counts do not match the size, or a file cannot be written.
#[allow(clippy::cast_sign_loss)] // width/height of a rendered result are positive
pub fn write_diff(
    root: &Path,
    oracle_tier: &str,
    config: Config,
    name: &str,
    (width, height): (i32, i32),
    ours: &[u8],
    golden: &[u8],
) -> Result<PathBuf, String> {
    let (w, h) = (width as usize, height as usize);
    let bpp = config.color_type().bytes_per_pixel();
    if ours.len() != w * h * bpp || golden.len() != w * h * bpp {
        return Err(format!(
            "{name}: {}x{} {config} needs {} bytes; ours {}, golden {}",
            w,
            h,
            w * h * bpp,
            ours.len(),
            golden.len()
        ));
    }
    let dir = root.join(oracle_tier).join(config.tag());
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let write = |file: &str, bytes: &[u8]| {
        let path = dir.join(file);
        std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))
    };
    write(&format!("{name}.raw"), ours)?;
    write(&format!("{name}.golden.raw"), golden)?;

    let g = to_rgb8(config, golden);
    let o = to_rgb8(config, ours);
    let mut rgb = Vec::with_capacity(w * 3 * h * 3);
    for y in 0..h {
        let row = y * w..(y + 1) * w;
        for px in &g[row.clone()] {
            rgb.extend_from_slice(px);
        }
        for px in &o[row.clone()] {
            rgb.extend_from_slice(px);
        }
        for x in row {
            let at = x * bpp..(x + 1) * bpp;
            if ours[at.clone()] == golden[at] {
                rgb.extend(g[x].map(|c| c / 4));
            } else {
                rgb.extend_from_slice(&[255, 0, 255]);
            }
        }
    }

    let path = dir.join(format!("{name}.png"));
    let file = std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let png_width = u32::try_from(w * 3).map_err(|e| e.to_string())?;
    let png_height = u32::try_from(h).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), png_width, png_height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    writer
        .write_image_data(&rgb)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    writer
        .finish()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_each_config() {
        assert_eq!(
            to_rgb8(Config::N32, &[1, 2, 3, 255]),
            [if Config::N32.color_type() == ColorType::BGRA8888 {
                [3, 2, 1]
            } else {
                [1, 2, 3]
            }]
        );
        // 565 red, green, blue.
        assert_eq!(
            to_rgb8(Config::Rgb565, &[0x00, 0xf8, 0xe0, 0x07, 0x1f, 0x00]),
            [[255, 0, 0], [0, 255, 0], [0, 0, 255]]
        );
        // f16 (1.0, 0.5, 0.0, 1.0).
        assert_eq!(
            to_rgb8(Config::F16, &[0x00, 0x3c, 0x00, 0x38, 0, 0, 0x00, 0x3c]),
            [[255, 128, 0]]
        );
    }

    #[test]
    fn writes_png_and_raws() {
        let root = std::env::temp_dir().join(format!("skia-rust-gm-diff-{}", std::process::id()));
        let golden = [0xffu8; 2 * 2 * 4];
        let mut ours = golden;
        ours[4] = 0;
        let png = write_diff(&root, "tier", Config::N32, "g", (2, 2), &ours, &golden).unwrap();
        assert!(png.is_file());
        let dir = root.join("tier").join("8888");
        assert_eq!(std::fs::read(dir.join("g.raw")).unwrap(), ours);
        assert_eq!(std::fs::read(dir.join("g.golden.raw")).unwrap(), golden);
        assert!(write_diff(&root, "tier", Config::N32, "g", (3, 2), &ours, &golden).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
