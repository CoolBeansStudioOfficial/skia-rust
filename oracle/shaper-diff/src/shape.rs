// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Shaping the corpus with HarfRust, in the way `cpp/shaper_diff.cc` does with HarfBuzz.

use std::fmt::Write as _;

use harfrust::funcs::FontFuncs;
use harfrust::{BufferClusterLevel, FontRef, ShapeOptions, Shaper, ShaperData, UnicodeBuffer};
use sha2::{Digest, Sha256};

use crate::synthetic::{self, SyntheticFuncs};
use crate::tables::{FontTable, skhb_position};
use crate::{Corpus, Row, crate_dir, read_table};

/// The three modes of `docs/design/modules.md` §2.4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// (a) The library's own OpenType font functions.
    Builtin,
    /// (b) Skia's font functions over per-font tables.
    SkiaFuncs,
    /// (c) The empty face with synthetic callbacks.
    EmptyFace,
}

impl Mode {
    /// All modes, in the order of `expected/`.
    pub const ALL: [Mode; 3] = [Mode::Builtin, Mode::SkiaFuncs, Mode::EmptyFace];

    fn from_letter(c: &str) -> Result<Mode, String> {
        match c {
            "a" => Ok(Mode::Builtin),
            "b" => Ok(Mode::SkiaFuncs),
            "c" => Ok(Mode::EmptyFace),
            _ => Err(format!("unknown mode `{c}`")),
        }
    }
}

/// A line of `configs.txt`.
#[derive(Clone, Debug)]
pub struct ShapeConfig {
    /// Name, e.g. `b-12-sub`.
    pub name: String,
    /// Mode.
    pub mode: Mode,
    /// Font size as written (`-` for none), used in table file names.
    pub size_text: String,
    /// Font size; `None` for `-`.
    pub size: Option<f32>,
    /// `SkFont::isSubpixel`.
    pub subpixel: bool,
}

impl ShapeConfig {
    /// All configs of `configs.txt`.
    ///
    /// # Errors
    /// If the file is malformed.
    pub fn load() -> Result<Vec<ShapeConfig>, String> {
        read_table(&crate_dir().join("configs.txt"))?
            .into_iter()
            .map(|r| {
                let [name, mode, size, sub] = &r[..] else {
                    return Err(format!("bad config row {r:?}"));
                };
                Ok(ShapeConfig {
                    name: name.clone(),
                    mode: Mode::from_letter(mode)?,
                    size_text: size.clone(),
                    size: if size == "-" {
                        None
                    } else {
                        Some(size.parse().map_err(|e| format!("{size}: {e}"))?)
                    },
                    subpixel: sub == "1",
                })
            })
            .collect()
    }

    /// The HarfBuzz scale of the config: none, or `skhb_position(size)`.
    #[must_use]
    pub fn scale(&self) -> Option<i32> {
        self.size.map(skhb_position)
    }

    /// The name of this config's table file for `font` (`<font>@<size>@<subpixel>.tbl`).
    #[must_use]
    pub fn table_name(&self, font: &str) -> String {
        format!(
            "{font}@{}@{}.tbl",
            self.size_text,
            if self.subpixel { "1" } else { "0" }
        )
    }
}

/// Shapes `text` as one run and serializes it as `gid:cluster:xa:ya:xo:yo` records.
///
/// The buffer is built as the C++ driver does: UTF-8, monotone-characters cluster level, segment
/// properties guessed. An empty line is not shaped.
#[must_use]
pub fn shape_line(
    shaper: &Shaper<'_>,
    text: &str,
    scale: Option<i32>,
    funcs: Option<&mut dyn FontFuncs>,
) -> String {
    let mut out = String::new();
    if text.is_empty() {
        return out;
    }
    let mut buffer = UnicodeBuffer::new();
    buffer.set_cluster_level(BufferClusterLevel::MonotoneCharacters);
    buffer.push_str(text);
    buffer.guess_segment_properties();
    let glyphs = shaper.shape(buffer, ShapeOptions::new().scale(scale).font_funcs(funcs));
    for (i, (info, pos)) in glyphs
        .glyph_infos()
        .iter()
        .zip(glyphs.glyph_positions())
        .enumerate()
    {
        if i > 0 {
            out.push(' ');
        }
        let _ = write!(
            out,
            "{}:{}:{}:{}:{}:{}",
            info.glyph_id, info.cluster, pos.x_advance, pos.y_advance, pos.x_offset, pos.y_offset
        );
    }
    out
}

/// What a run needs to shape one font under one config.
pub struct Run<'a> {
    font: FontRef<'a>,
    data: ShaperData,
    scale: Option<i32>,
    funcs: Option<Box<dyn FontFuncs>>,
}

impl std::fmt::Debug for Run<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Run")
            .field("scale", &self.scale)
            .finish_non_exhaustive()
    }
}

impl<'a> Run<'a> {
    /// Prepares `font_data` (a font file; face 0) for `config`; `table` is mode b's table.
    ///
    /// # Errors
    /// If the font cannot be parsed.
    pub fn new(
        config: &ShapeConfig,
        font_data: &'a [u8],
        table: Option<FontTable>,
    ) -> Result<Run<'a>, String> {
        let font = FontRef::from_index(font_data, 0).map_err(|e| format!("font: {e}"))?;
        // A fresh `ShaperData` per run: it caches the code point to glyph mapping, which depends
        // on the font functions.
        let data = ShaperData::new(&font);
        let funcs: Option<Box<dyn FontFuncs>> = match config.mode {
            Mode::Builtin => None,
            Mode::SkiaFuncs => Some(Box::new(table.ok_or("mode b needs a table")?)),
            Mode::EmptyFace => Some(Box::new(SyntheticFuncs {
                subpixel: config.subpixel,
            })),
        };
        Ok(Run {
            font,
            data,
            scale: config.scale(),
            funcs,
        })
    }

    /// Shapes every line of `corpus` and returns the rows for `font_name` under `config`.
    pub fn rows(&mut self, config: &ShapeConfig, font_name: &str, corpus: &Corpus) -> Vec<Row> {
        let mut rows = Vec::new();
        let shaper = self.data.shaper(&self.font).build();
        // SHAPER_DIFF_DUMP=1 prints every shaped line (`L <config> <font> <source> <index> <glyphs>`).
        let dump = std::env::var_os("SHAPER_DIFF_DUMP").is_some();
        for (id, lines) in &corpus.sources {
            let mut sha = Sha256::new();
            let mut glyphs = 0;
            for (index, line) in lines.iter().enumerate() {
                let funcs: Option<&mut dyn FontFuncs> = match self.funcs.as_mut() {
                    Some(f) => Some(&mut **f),
                    None => None,
                };
                let mut s = shape_line(&shaper, line, self.scale, funcs);
                if !s.is_empty() {
                    glyphs += 1 + s.bytes().filter(|&b| b == b' ').count();
                }
                if dump {
                    println!("L {} {font_name} {id} {index} {s}", config.name);
                }
                s.push('\n');
                sha.update(s.as_bytes());
            }
            let mut hex = String::with_capacity(64);
            for b in sha.finalize() {
                let _ = write!(hex, "{b:02x}");
            }
            rows.push(Row {
                config: config.name.clone(),
                font: font_name.to_owned(),
                source: id.clone(),
                lines: lines.len(),
                glyphs,
                sha256: hex,
            });
        }
        rows
    }
}

/// The font data of the empty face (mode c).
#[must_use]
pub fn empty_face_data() -> Vec<u8> {
    synthetic::empty_sfnt()
}
