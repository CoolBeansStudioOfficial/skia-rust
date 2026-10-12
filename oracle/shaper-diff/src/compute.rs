// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Running the corpus: the Rust counterpart of the C++ driver's main loop.

use std::path::Path;

use crate::shape::{self, Mode, ShapeConfig};
use crate::tables::FontTable;
use crate::{Corpus, Row, font_list, fonts_dir};

/// Verifies the fonts of `fonts.txt` in `dir` and returns their data.
fn load_fonts(dir: &Path, only: Option<&str>) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut out = Vec::new();
    for (name, sha) in font_list()? {
        if only.is_some_and(|o| o != name) {
            continue;
        }
        let path = dir.join(&name);
        let data = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let got = skia_rust_text_deps::sha256_hex(&data);
        if got != sha {
            return Err(format!(
                "{} has SHA-256 {got}, fonts.txt says {sha}",
                path.display()
            ));
        }
        out.push((name, data));
    }
    Ok(out)
}

fn table_for(cfg: &ShapeConfig, name: &str, data: &[u8]) -> Result<FontTable, String> {
    let face = read_fonts::FontRef::from_index(data, 0).map_err(|e| format!("{name}: {e}"))?;
    Ok(FontTable::build(
        &face,
        cfg.size.ok_or("mode b needs a size")?,
        cfg.subpixel,
    ))
}

/// Shapes the corpus under every config of `mode` (or only `config`), with every font of
/// `fonts.txt` (or only `font`; mode c has the single font `(empty)`), and returns the rows in the
/// C++ driver's order.
///
/// # Errors
/// If an input cannot be read or a font does not match `fonts.txt`.
pub fn compute(mode: Mode, config: Option<&str>, font: Option<&str>) -> Result<Vec<Row>, String> {
    let corpus = Corpus::load()?;
    let configs: Vec<ShapeConfig> = ShapeConfig::load()?
        .into_iter()
        .filter(|c| c.mode == mode && config.is_none_or(|n| n == c.name))
        .collect();
    let mut rows = Vec::new();
    if mode == Mode::EmptyFace {
        let data = shape::empty_face_data();
        for cfg in &configs {
            if font.is_some_and(|f| f != "(empty)") {
                continue;
            }
            let mut run = shape::Run::new(cfg, &data, None)?;
            rows.extend(run.rows(cfg, "(empty)", &corpus));
        }
        return Ok(rows);
    }
    let fonts = load_fonts(&fonts_dir()?, font)?;
    for cfg in &configs {
        for (name, data) in &fonts {
            let table = if mode == Mode::SkiaFuncs {
                Some(table_for(cfg, name, data)?)
            } else {
                None
            };
            let mut run = shape::Run::new(cfg, data, table)?;
            rows.extend(run.rows(cfg, name, &corpus));
        }
    }
    Ok(rows)
}

/// Writes mode b's table files (`FontTable::write`) for every (font, config) into `out_dir`, for
/// the C++ driver. Returns how many.
///
/// # Errors
/// If an input cannot be read or an output written.
pub fn write_tables(out_dir: &Path) -> Result<usize, String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let fonts = load_fonts(&fonts_dir()?, None)?;
    let mut n = 0;
    for cfg in ShapeConfig::load()?
        .into_iter()
        .filter(|c| c.mode == Mode::SkiaFuncs)
    {
        for (name, data) in &fonts {
            table_for(&cfg, name, data)?.write(&out_dir.join(cfg.table_name(name)))?;
            n += 1;
        }
    }
    Ok(n)
}
