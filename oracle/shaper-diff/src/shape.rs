// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Shaping the corpus with HarfRust, in the way `cpp/shaper_diff.cc` does with HarfBuzz.

use std::fmt::Write as _;

use harfrust::{
    BufferClusterLevel, FontRef, GlyphBuffer, ShapeOptions, Shaper, ShaperData, UnicodeBuffer,
};
use sha2::{Digest, Sha256};
use skia_rust_core::font::Font;
use skia_rust_core::typeface::Typeface;
use skia_rust_shaper::harfbuzz::{shape_buffer, skhb_position};

use crate::{Corpus, Row, crate_dir, read_table};

/// The three modes of `docs/design/modules.md` §2.4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// (a) The library's own OpenType font functions.
    Builtin,
    /// (b) Skia's font functions, over the `Font` of a Fontations typeface of the test font.
    SkiaFuncs,
    /// (c) The empty face of the portable `TestTypeface`, with Skia's font functions.
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

    /// The `Font` of a typeface under this config (modes b and c).
    ///
    /// # Panics
    /// If the config has no size.
    #[must_use]
    pub fn font(&self, typeface: &Typeface) -> Font {
        let mut font = Font::from_size(
            typeface.clone(),
            self.size.expect("modes b and c have a size"),
        );
        font.set_subpixel(self.subpixel);
        font
    }
}

fn serialize(glyphs: &GlyphBuffer) -> String {
    let mut out = String::new();
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

fn buffer_of(text: &str) -> UnicodeBuffer {
    let mut buffer = UnicodeBuffer::new();
    buffer.set_cluster_level(BufferClusterLevel::MonotoneCharacters);
    buffer.push_str(text);
    buffer.guess_segment_properties();
    buffer
}

/// Mode a: shapes `text` as one run and serializes it as `gid:cluster:xa:ya:xo:yo` records.
///
/// The buffer is built as the C++ driver does: UTF-8, monotone-characters cluster level, segment
/// properties guessed. An empty line is not shaped.
#[must_use]
pub fn shape_line(shaper: &Shaper<'_>, text: &str, scale: Option<i32>) -> String {
    if text.is_empty() {
        return String::new();
    }
    serialize(&shaper.shape(buffer_of(text), ShapeOptions::new().scale(scale)))
}

/// Modes b and c: the same, through `skia-rust-shaper`: the cached face of the typeface of `font`,
/// Skia's font functions and the scale of the font size.
#[must_use]
pub fn shape_line_skia(font: &Font, text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    shape_buffer(font, buffer_of(text), &[]).map_or_else(String::new, |g| serialize(&g))
}

/// What a run needs to shape one font under one config.
pub enum Run<'a> {
    /// Mode a: HarfRust over the font file, with its own font functions.
    Builtin {
        /// The font.
        font: FontRef<'a>,
        /// Its shaper data.
        data: Box<ShaperData>,
        /// The scale, if any.
        scale: Option<i32>,
    },
    /// Modes b and c: Skia's `Font`.
    Skia(Font),
}

impl std::fmt::Debug for Run<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Run").finish_non_exhaustive()
    }
}

impl<'a> Run<'a> {
    /// Mode a over `font_data` (face 0).
    ///
    /// # Errors
    /// If the font cannot be parsed.
    pub fn builtin(config: &ShapeConfig, font_data: &'a [u8]) -> Result<Run<'a>, String> {
        let font = FontRef::from_index(font_data, 0).map_err(|e| format!("font: {e}"))?;
        let data = Box::new(ShaperData::new(&font));
        Ok(Run::Builtin {
            font,
            data,
            scale: config.scale(),
        })
    }

    /// Shapes every line of `corpus` and returns the rows for `font_name` under `config`.
    #[must_use]
    pub fn rows(&self, config: &ShapeConfig, font_name: &str, corpus: &Corpus) -> Vec<Row> {
        let mut rows = Vec::new();
        let shaper = match self {
            Run::Builtin { font, data, .. } => Some(data.shaper(font).build()),
            Run::Skia(_) => None,
        };
        // SHAPER_DIFF_DUMP=1 prints every shaped line (`L <config> <font> <source> <index> <glyphs>`).
        let dump = std::env::var_os("SHAPER_DIFF_DUMP").is_some();
        for (id, lines) in &corpus.sources {
            let mut sha = Sha256::new();
            let mut glyphs = 0;
            for (index, line) in lines.iter().enumerate() {
                let mut s = match (self, &shaper) {
                    (Run::Builtin { scale, .. }, Some(shaper)) => shape_line(shaper, line, *scale),
                    (Run::Skia(font), _) => shape_line_skia(font, line),
                    (Run::Builtin { .. }, None) => unreachable!("a builtin run has a shaper"),
                };
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
