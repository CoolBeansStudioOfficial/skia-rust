// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFMakeCIDGlyphWidthsArray.{h,cpp} (chrome/m156)

//! `SkPDFMakeCIDGlyphWidthsArray`: the `W` array of a CID font, the advances of the glyphs a
//! document used.

#![allow(clippy::cast_precision_loss)] // SkIntToScalar-style casts mirror the C++
#![allow(clippy::missing_panics_doc)] // the SkTo checks of the C++

use skia_rust_core::font_types::GlyphId;
use skia_rust_core::scalar::scalar;
use skia_rust_core::strike_spec::BulkGlyphMetricsAndPaths;

use crate::font::PdfStrikeSpec;
use crate::glyph_use::PdfGlyphUse;
use crate::types::PdfArray;

/// Scale from em-units to 1000-units.
// Port of: src/pdf/SkPDFMakeCIDGlyphWidthsArray.cpp#L28-L34 (from_font_units, chrome/m156)
fn from_font_units(scaled: scalar, em_size: i32) -> scalar {
    if em_size == 1000 {
        scaled
    } else {
        scaled * 1000.0 / em_size as scalar
    }
}

// Port of: src/pdf/SkPDFMakeCIDGlyphWidthsArray.cpp#L36-L60 (find_mode_or_0, chrome/m156)
fn find_mode_or_0(advances: &[scalar]) -> scalar {
    if advances.is_empty() {
        return 0.0;
    }

    let mut current_advance = advances[0];
    let mut current_mode_advance = advances[0];
    let mut current_count = 1usize;
    let mut current_mode_count = 1usize;

    for &advance in &advances[1..] {
        #[allow(clippy::float_cmp)] // exact comparison, as in Skia
        if advance == current_advance {
            current_count += 1;
        } else {
            if current_count > current_mode_count {
                current_mode_advance = current_advance;
                current_mode_count = current_count;
            }
            current_advance = advance;
            current_count = 1;
        }
    }
    if current_count > current_mode_count {
        current_advance
    } else {
        current_mode_advance
    }
}

/// `SkPDFMakeCIDGlyphWidthsArray`: the `W` array of the glyphs in `subset`, and the default
/// advance (`DW`).
// Port of: src/pdf/SkPDFMakeCIDGlyphWidthsArray.cpp#L64-L201 (chrome/m156)
#[doc(alias = "SkPDFMakeCIDGlyphWidthsArray")]
#[allow(clippy::cast_possible_truncation)] // `(int32_t)currentAdvance`, as in the C++
#[allow(clippy::float_cmp)] // exact comparisons, as in Skia
#[allow(clippy::too_many_lines)] // one function in the C++, kept as is
pub fn make_cid_glyph_widths_array(
    pdf_strike_spec: &PdfStrikeSpec,
    subset: &PdfGlyphUse,
    default_advance: &mut i32,
) -> PdfArray {
    // There are two ways of expressing advances
    //
    // range: " gfid [adv.ances adv.ances ... adv.ances]"
    //   run: " gfid gfid adv.ances"
    //
    // Assuming that on average
    // the ASCII representation of an advance plus a space is 10 characters
    // the ASCII representation of a glyph id plus a space is 4 characters
    // the ASCII representation of unused gid plus a space in a range is 2 characters
    //
    // When not in a range or run
    //  a. Skipping don't cares or defaults is a win (trivial)
    //  b. Run wins for 2+ repeats " gid gid adv.ances"
    //                             " gid [adv.ances adv.ances]"
    //     rule: 2+ repeats create run as long as possible, else start range
    //
    // When in a range
    // Cost of stopping and starting a range is 8 characters  "] gid ["
    //  c. Skipping defaults is always a win                  " adv.ances"
    //     rule: end range if default seen
    //  d. Skipping 4+ don't cares is a win                   " 0 0 0 0"
    //     rule: end range if 4+ don't cares
    // Cost of stop and start range plus run is 28 characters "] gid gid adv.ances gid ["
    //  e. Switching for 2+ repeats and 4+ don't cares wins   " 0 0 adv.ances 0 0 adv.ances"
    //     rule: end range for 2+ repeats with 4+ don't cares
    //  f. Switching for 3+ repeats wins                      " adv.ances adv.ances adv.ances"
    //     rule: end range for 3+ repeats

    let em_size = pdf_strike_spec.units_per_em as i32;
    let paths = BulkGlyphMetricsAndPaths::new(&pdf_strike_spec.strike_spec);

    let mut result = PdfArray::new();

    let mut glyph_ids: Vec<GlyphId> = Vec::new();
    subset.get_set_values(|index| {
        glyph_ids.push(GlyphId::try_from(index).expect("SkTo<SkGlyphID>"));
    });
    let glyphs = paths.glyphs(&glyph_ids);

    // Find the pdf integer mode (most common pdf integer advance).
    // Unfortunately, poppler enforces DW (default width) must be an integer,
    // so only consider integer pdf advances when finding the mode.
    let mut int_advances: Vec<scalar> = Vec::new();
    for glyph in &glyphs {
        let current_advance = from_font_units(glyph.advance_x(), em_size);
        if (current_advance as i32) as scalar == current_advance {
            int_advances.push(current_advance);
        }
    }
    int_advances.sort_by(scalar::total_cmp);
    let mode_advance = find_mode_or_0(&int_advances) as i32;
    *default_advance = mode_advance;
    let mode_advance_f = mode_advance as scalar;

    // Pre-convert to pdf advances.
    let advances: Vec<scalar> = glyphs
        .iter()
        .map(|glyph| from_font_units(glyph.advance_x(), em_size))
        .collect();

    let n = glyphs.len();
    let mut i = 0usize;
    while i < n {
        let mut advance = advances[i];

        // a. Skipping don't cares or defaults is a win (trivial)
        if advance == mode_advance_f {
            i += 1;
            continue;
        }

        // b. 2+ repeats create run as long as possible, else start range
        {
            let mut j = i + 1; // j is always one past the last known repeat
            while j < n {
                let next_advance = advances[j];
                if advance != next_advance {
                    break;
                }
                j += 1;
            }
            if j - i >= 2 {
                result.append_int(i32::from(glyphs[i].glyph_id()));
                result.append_int(i32::from(glyphs[j - 1].glyph_id()));
                result.append_scalar(advance);
                i = j; // `i = j - 1; continue;` with the loop's `++i`
                continue;
            }
        }

        {
            result.append_int(i32::from(glyphs[i].glyph_id()));
            let mut advance_array = PdfArray::new();
            advance_array.append_scalar(advance);
            let mut j = i + 1; // j is always one past the last output
            while j < n {
                advance = advances[j];

                // c. end range if default seen
                if advance == mode_advance_f {
                    break;
                }

                let mut dont_cares =
                    i32::from(glyphs[j].glyph_id()) - i32::from(glyphs[j - 1].glyph_id()) - 1;
                // d. end range if 4+ don't cares
                if dont_cares >= 4 {
                    break;
                }

                let mut next_advance: scalar = 0.0;
                // e. end range for 2+ repeats with 4+ don't cares
                if j + 1 < n {
                    next_advance = advances[j + 1];
                    let next_dont_cares =
                        i32::from(glyphs[j + 1].glyph_id()) - i32::from(glyphs[j].glyph_id()) - 1;
                    if advance == next_advance && dont_cares + next_dont_cares >= 4 {
                        break;
                    }
                }

                // f. end range for 3+ repeats
                if j + 2 < n && advance == next_advance {
                    next_advance = advances[j + 2];
                    if advance == next_advance {
                        break;
                    }
                }

                while dont_cares > 0 {
                    dont_cares -= 1;
                    advance_array.append_scalar(0.0);
                }
                advance_array.append_scalar(advance);
                j += 1;
            }
            result.append_object(Box::new(advance_array));
            i = j; // `i = j - 1` with the loop's `++i`
        }
    }

    result
}
