// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Running the corpus: the Rust counterpart of the C++ driver's main loop.

use std::path::Path;

use skia_rust_core::data::Data;
use skia_rust_core::font::Font;
use skia_rust_core::font_mgr::FontMgr;
use skia_rust_core::typeface::Typeface;
use skia_rust_text::ports::fontations::font_mgr::new_fontations_empty;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

use crate::shape::{Mode, Run, ShapeConfig};
use crate::tables::FontTable;
use crate::{Corpus, Row, font_list, fonts_dir};

/// The name of mode c's only font in the rows and the table files.
const EMPTY_FONT: &str = "(empty)";

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

// The typeface of a font file, from the Fontations manager (face 0): what Skia makes of it.
fn typeface_of(mgr: &FontMgr, name: &str, data: &[u8]) -> Result<Typeface, String> {
    mgr.make_from_data(Some(&Data::new_from_vec(data.to_vec())), 0)
        .ok_or_else(|| format!("{name}: no typeface"))
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
        if font.is_some_and(|f| f != EMPTY_FONT) {
            return Ok(rows);
        }
        let typeface = default_portable_typeface();
        for cfg in &configs {
            let run = Run::Skia(cfg.font(&typeface));
            rows.extend(run.rows(cfg, EMPTY_FONT, &corpus));
        }
        return Ok(rows);
    }
    let fonts = load_fonts(&fonts_dir()?, font)?;
    let mgr = new_fontations_empty();
    for (name, data) in &fonts {
        let typeface = if mode == Mode::SkiaFuncs {
            Some(typeface_of(&mgr, name, data)?)
        } else {
            None
        };
        for cfg in &configs {
            let run = match &typeface {
                Some(typeface) => Run::Skia(cfg.font(typeface)),
                None => Run::builtin(cfg, data)?,
            };
            rows.extend(run.rows(cfg, name, &corpus));
        }
    }
    // The C++ driver loops configs outside fonts.
    rows.sort_by_key(|r| {
        (
            configs.iter().position(|c| c.name == r.config),
            fonts.iter().position(|(n, _)| *n == r.font),
        )
    });
    Ok(rows)
}

/// Writes the table files of modes b and c (`FontTable::write`) for every (font, config) into
/// `out_dir`, for the C++ driver. Returns how many.
///
/// # Errors
/// If an input cannot be read or an output written.
pub fn write_tables(out_dir: &Path) -> Result<usize, String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let fonts = load_fonts(&fonts_dir()?, None)?;
    let mgr = new_fontations_empty();
    let configs = ShapeConfig::load()?;
    let mut n = 0;
    for (name, data) in &fonts {
        let typeface = typeface_of(&mgr, name, data)?;
        let cmap = FontTable::cmap_of(&Font::from_size(typeface.clone(), 12.0));
        for cfg in configs.iter().filter(|c| c.mode == Mode::SkiaFuncs) {
            FontTable::from_font(&cfg.font(&typeface), &cmap)
                .write(&out_dir.join(cfg.table_name(name)))?;
            n += 1;
        }
    }
    let typeface = default_portable_typeface();
    let cmap = FontTable::cmap_of(&Font::from_size(typeface.clone(), 12.0));
    for cfg in configs.iter().filter(|c| c.mode == Mode::EmptyFace) {
        FontTable::from_font(&cfg.font(&typeface), &cmap)
            .write(&out_dir.join(cfg.table_name(EMPTY_FONT)))?;
        n += 1;
    }
    Ok(n)
}
