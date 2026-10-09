// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file.
// Port of: src/ports/fontations/src/colr.rs (chrome/m156). Only `resolve_palette` is ported here,
// because the typeface needs palettes (T19a). The COLR glyph painting follows with T21.

//! The colour palettes of a font (`CPAL`), with the overrides of `SkFontArguments`.

use skia_rust_core::font_arguments::palette::Override;

use super::base::BridgeFontRef;

/// Port of `resolve_palette`: the palette `base_palette` of the font (or palette 0 when that one
/// does not exist), with `palette_overrides` applied. Each entry is `0xAARRGGBB`. An empty
/// result means the font has no palette.
// Port of: src/ports/fontations/src/colr.rs#L317-L356 (chrome/m156)
#[must_use]
pub fn resolve_palette(
    font_ref: &BridgeFontRef<'_>,
    base_palette: u16,
    palette_overrides: &[Override],
) -> Vec<u32> {
    let cpal_to_vector =
        |cpal: &read_fonts::tables::cpal::Cpal<'_>, palette_index| -> Option<Vec<u32>> {
            let start_index: usize = cpal
                .color_record_indices()
                .get(usize::from(palette_index))?
                .get()
                .into();
            let num_entries: usize = cpal.num_palette_entries().into();
            let color_records = cpal.color_records_array()?.ok()?;
            Some(
                color_records
                    .get(start_index..start_index + num_entries)?
                    .iter()
                    .map(|record| {
                        u32::from_be_bytes([record.alpha, record.red, record.green, record.blue])
                    })
                    .collect(),
            )
        };

    font_ref
        .with_font(|f| {
            use read_fonts::TableProvider;
            let cpal = f.cpal().ok()?;

            let mut palette = cpal_to_vector(&cpal, base_palette).or(cpal_to_vector(&cpal, 0))?;

            for override_entry in palette_overrides {
                let index = usize::from(override_entry.index);
                if index < palette.len() {
                    palette[index] = u32::from(override_entry.color);
                }
            }
            Some(palette)
        })
        .unwrap_or_default()
}
