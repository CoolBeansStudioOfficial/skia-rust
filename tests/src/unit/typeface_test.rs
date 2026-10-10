// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/TypefaceTest.cpp (chrome/m156), the cases that need only the typeface core and the
// portable and Fontations test managers

#![cfg(test)]

use skia_rust_core::data::Data;
use skia_rust_core::fixed::scalar_to_fixed;
use skia_rust_core::font::Font;
use skia_rust_core::font_arguments::variation_position::Coordinate;
use skia_rust_core::font_arguments::{FontArguments, VariationPosition};
use skia_rust_core::font_descriptor::FontDescriptor;
use skia_rust_core::font_parameters::variation::Axis;
use skia_rust_core::font_priv::glyphs_to_unichars;
use skia_rust_core::font_style::{FontStyle, Slant, Weight, Width};
use skia_rust_core::font_types::GlyphId;
use skia_rust_core::font_types::{FourByteTag, set_four_byte_tag};
use skia_rust_core::path::Path;
use skia_rust_core::rect::Rect;
use skia_rust_core::stream::{DynamicMemoryWStream, MemoryStream, StreamAsset};
use skia_rust_core::typeface::{SerializeBehavior, Typeface};
use skia_rust_core::typeface_cache::TypefaceCache;
use skia_rust_core::utf::{Unichar, count_utf8, next_utf8};
use skia_rust_text::utils::custom_typeface::CustomTypefaceBuilder;
use skia_rust_tools::font_tool_utils::{
    create_test_typeface, create_typeface_from_resource, default_typeface, emoji_sample_default,
    test_font_mgr,
};
use skia_rust_tools::fonts::test_empty_typeface::TestEmptyTypeface;

use crate::resources::get_resource_as_data;
use crate::{Reporter, def_font_test, def_test, errorf, reporter_assert};

/// `GetResourceAsStream(path)`: the resource as a memory stream, or `None` if it is missing.
fn resource_stream(path: &str) -> Option<Box<dyn StreamAsset>> {
    let data = get_resource_as_data(path)?;
    Some(MemoryStream::make_copy(&data))
}

/// `ToolUtils::CreateTypefaceFromResource(path)`: the typeface of a resource (`None` in the
/// portable configuration, docs/design/text.md §1.2).
fn create_typeface_from_resource_path(path: &str) -> Option<Typeface> {
    create_typeface_from_resource(resource_stream(path), 0)
}

/// `SkEndian_SwapBE16` / `SkEndian_SwapBE32` reads of the big-endian SFNT fields.
fn be16(bytes: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([bytes[at], bytes[at + 1]])
}

fn be32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// `sizeof(SkSFNTHeader)`: the table directory entries start right after it.
const SFNT_HEADER_SIZE: usize = 12;
/// `sizeof(SkSFNTHeader::TableDirectoryEntry)`.
const TABLE_DIRECTORY_ENTRY_SIZE: usize = 16;

/// The four characters of an axis tag, as the C++ messages print them.
fn tag_to_string(tag: FourByteTag) -> String {
    tag.to_be_bytes().iter().map(|&b| char::from(b)).collect()
}

/// `SkFontArguments` with one variation position.
fn position_args(position: &[Coordinate]) -> FontArguments<'_, '_> {
    let mut args = FontArguments::new();
    args.set_variation_design_position(VariationPosition {
        coordinates: position,
    });
    args
}

// Port of: tests/TypefaceTest.cpp#L52-L109 (chrome/m156)
fn typeface_style_test(reporter: &mut Reporter, weight: u16, width: u16, data: &[u8]) {
    // C++ rewrites the table in place (copying the data first if it is shared). The bytes that
    // reach the font manager are the same either way.
    let mut bytes = data.to_vec();
    let num_tables = usize::from(be16(&bytes, 4));
    let os2_entry = (0..num_tables)
        .map(|index| SFNT_HEADER_SIZE + index * TABLE_DIRECTORY_ENTRY_SIZE)
        .find(|&entry| &bytes[entry..entry + 4] == b"OS/2")
        .expect("the OS/2 table is in the table directory (SkASSERT_RELEASE)");
    let os2_offset = be32(&bytes, os2_entry + 8) as usize;
    // SkOTTableOS2_V0: version (u16), xAvgCharWidth (i16), usWeightClass, usWidthClass.
    bytes[os2_offset + 4..os2_offset + 6].copy_from_slice(&weight.to_be_bytes());
    bytes[os2_offset + 6..os2_offset + 8].copy_from_slice(&width.to_be_bytes());

    let Some(new_typeface) = test_font_mgr().make_from_data(Some(&Data::new_from_vec(bytes)), 0)
    else {
        // Not all SkFontMgr can MakeFromStream().
        return;
    };
    let new_style = new_typeface.font_style();
    let new_weight = *new_style.weight();
    let weight = i32::from(weight);
    // Some back-ends (CG, GDI, DW) support OS/2 version A which uses 0 - 10 (but all differently).
    reporter_assert!(
        reporter,
        new_weight == weight
            || (weight <= 10 && new_weight == 100 * weight)
            || (weight == 4 && new_weight == 350) // GDI weirdness
            || (weight == 5 && new_weight == 400) // GDI weirdness
            || (weight == 0 && new_weight == 1) // DW weirdness
            || (weight == 1000 && new_weight == 999), // DW weirdness
        "newStyle.weight(): {} weight: {}",
        new_weight,
        weight
    );

    // Some back-ends (GDI) don't support width, ensure these always report 'normal'.
    let width = i32::from(width);
    reporter_assert!(
        reporter,
        *new_style.width() == width || new_style.width() == Width::NORMAL,
        "newStyle.width(): {} width: {}",
        *new_style.width(),
        width
    );
}

// Port of: tests/TypefaceTest.cpp#L110-L125 (chrome/m156)
def_font_test!(TypefaceStyle, |reporter| {
    crate::skip_without_resources!();
    let Some(data) = get_resource_as_data("fonts/Em.ttf") else {
        errorf!(reporter, "Cannot load resource");
        return;
    };

    for weight in 0..=1000 {
        typeface_style_test(reporter, weight, 5, &data);
    }
    for width in 1..=9 {
        typeface_style_test(reporter, 400, width, &data);
    }
});

/// The expected glyph count and codepoints of `TypefaceGlyphToUnicode`.
const EXPECTED_GLYPHS: i32 = 6;
const EXPECTED_CODEPOINTS: [Unichar; 6] = [0, 0, 0, 9747, 11035, 11036];
/// The names that `TypefaceNameIter` must find.
const EXPECTED_NAMES: [&str; 2] = ["SpiderSymbol", "Symbole de l'Araignée"];

/// One `TypefaceStyleVariable` case: the variation position, and the expected styles.
struct TestCase {
    position: [Coordinate; 2],
    expected: FontStyle,
    // On Mac10.15 and earlier, the wdth affected the style using the old gx ranges.
    // On macOS 11 and later, the wdth affects the style using the new OpenType ranges.
    // Allow old CoreText to report the wrong width values.
    mac1015expected: FontStyle,
}

// Port of: tests/TypefaceTest.cpp#L131-L159 (chrome/m156)
def_font_test!(TypefaceGlyphToUnicode, |reporter| {
    crate::skip_without_resources!();
    let Some(stream) = resource_stream("fonts/Em.ttf") else {
        errorf!(reporter, "Cannot load resource");
        return;
    };
    let Some(typeface) = test_font_mgr().make_from_stream(Some(stream), 0) else {
        // Not all SkFontMgr can MakeFromStream().
        return;
    };

    let actual_glyphs = typeface.count_glyphs();
    if actual_glyphs != EXPECTED_GLYPHS {
        reporter_assert!(
            reporter,
            actual_glyphs == EXPECTED_GLYPHS,
            "{} != {}",
            actual_glyphs,
            EXPECTED_GLYPHS
        );
        return;
    }
    let mut codepoints: [Unichar; 6] = [0; 6];
    // TestSkTypefaceGlyphToUnicodeMap(*typeface, codepoints)
    typeface.glyph_to_unicode_map(&mut codepoints);
    for (i, (&codepoint, &expected)) in codepoints.iter().zip(&EXPECTED_CODEPOINTS).enumerate() {
        // CoreText before macOS 11 sometimes infers space (0x20) for empty glyphs.
        reporter_assert!(
            reporter,
            codepoint == expected || (codepoint == 32 && expected == 0),
            "codepoints[{}] == {} != {}",
            i,
            codepoint,
            expected
        );
    }
});

// Port of: tests/TypefaceTest.cpp#L161-L248 (chrome/m156)
def_font_test!(TypefaceStyleVariable, |reporter| {
    crate::skip_without_resources!();
    let fm = test_font_mgr();

    let Some(stream) = resource_stream("fonts/Variable.ttf") else {
        errorf!(reporter, "Cannot load resource");
        return;
    };
    let Some(typeface) = fm.make_from_stream(Some(stream), 0) else {
        // Not all SkFontMgr can MakeFromStream().
        return;
    };

    // Creating Variable.ttf without any extra parameters should have a normal font style.
    let fs = typeface.font_style();
    reporter_assert!(reporter, fs == FontStyle::normal(), "fs: {:?}", fs);

    // Ensure that the font supports variable stuff
    let mut var_pos = [Coordinate::default(); 2];
    let Some(num_axes) = typeface.get_variation_design_position(&mut var_pos) else {
        // Not all SkTypeface can get the variation.
        return;
    };
    if num_axes == 0 {
        // Not all SkTypeface can get the variation.
        return;
    }
    if num_axes != 2 {
        // Variable.ttf has two axes.
        reporter_assert!(reporter, num_axes == 2);
        return;
    }

    // If a fontmgr or typeface can do variations, ensure the variation affects the reported style.
    let wght = set_four_byte_tag(b'w', b'g', b'h', b't');
    let wdth = set_four_byte_tag(b'w', b'd', b't', b'h');
    let coord = |axis, value| Coordinate { axis, value };
    let test_cases = [
        // In range but non-default
        TestCase {
            position: [coord(wght, 200.0), coord(wdth, 75.0)],
            expected: FontStyle::new(Weight::from(200), Width::from(3), Slant::Upright),
            mac1015expected: FontStyle::new(Weight::from(200), Width::from(9), Slant::Upright),
        },
        // Out of range low, should clamp
        TestCase {
            position: [coord(wght, 0.0), coord(wdth, 75.0)],
            expected: FontStyle::new(Weight::from(100), Width::from(3), Slant::Upright),
            mac1015expected: FontStyle::new(Weight::from(100), Width::from(9), Slant::Upright),
        },
        // Out of range high, should clamp
        TestCase {
            position: [coord(wght, 10000.0), coord(wdth, 75.0)],
            expected: FontStyle::new(Weight::from(900), Width::from(3), Slant::Upright),
            mac1015expected: FontStyle::new(Weight::from(900), Width::from(9), Slant::Upright),
        },
    ];

    // static const constexpr bool isMac = SK_BUILD_FOR_MAC
    let is_mac = cfg!(target_os = "macos");
    for test in &test_cases {
        let args = position_args(&test.position);

        let Some(stream) = resource_stream("fonts/Variable.ttf") else {
            return;
        };
        let Some(non_default_typeface) = fm.make_from_stream_args(Some(stream), &args) else {
            errorf!(reporter, "makeFromStream returned null for the variation");
            continue;
        };
        let ndfs = non_default_typeface.font_style();
        reporter_assert!(
            reporter,
            ndfs == test.expected || (is_mac && ndfs == test.mac1015expected),
            "ndfs: {} {} {:?}",
            *ndfs.weight(),
            *ndfs.width(),
            ndfs.slant()
        );

        let clone_typeface = typeface.make_clone(&args);
        let cfs = clone_typeface.font_style();
        reporter_assert!(
            reporter,
            cfs == test.expected || (is_mac && cfs == test.mac1015expected),
            "cfs: {} {} {:?}",
            *cfs.weight(),
            *cfs.width(),
            cfs.slant()
        );
    }
});

// Port of: tests/TypefacePostScriptName (tests/TypefaceTest.cpp#L250-L264, chrome/m156)
def_font_test!(TypefacePostScriptName, |reporter| {
    let Some(typeface) = create_typeface_from_resource_path("fonts/Em.ttf") else {
        // Not all SkFontMgr can MakeFromStream().
        return;
    };

    let post_script_name = typeface.post_script_name();
    let has_name = post_script_name.is_some();
    // `getPostScriptName(nullptr)`: the same query, without copying the name out.
    let has_name2 = typeface.post_script_name().is_some();
    reporter_assert!(reporter, has_name == has_name2);
    if let Some(name) = post_script_name {
        reporter_assert!(reporter, name == "Em");
    }
});

// Port of: tests/TypefaceTest.cpp#L266-L292 (chrome/m156)
def_font_test!(TypefaceNameIter, |reporter| {
    let Some(typeface) = create_typeface_from_resource_path("fonts/SpiderSymbol.ttf") else {
        // Not all SkFontMgr can MakeFromStream().
        return;
    };

    let mut found = [false; EXPECTED_NAMES.len()];
    for other_name in typeface.new_family_name_iterator() {
        for (i, expected) in EXPECTED_NAMES.iter().enumerate() {
            if other_name.string == *expected {
                found[i] = true;
                break;
            }
        }
    }
    for (i, expected) in EXPECTED_NAMES.iter().enumerate() {
        reporter_assert!(reporter, found[i], "Missing: {}", expected);
    }
});

// Port of: tests/TypefaceTest.cpp#L294-L307 (chrome/m156)
def_font_test!(TypefaceRoundTrip, |reporter| {
    let Some(typeface) = create_typeface_from_resource_path("fonts/7630.otf") else {
        // Not all SkFontMgr can MakeFromStream().
        return;
    };

    let Some((stream, font_index)) = typeface.open_stream() else {
        errorf!(reporter, "openStream returned null for a resource typeface");
        return;
    };
    let typeface2 = test_font_mgr().make_from_stream(Some(stream), font_index);
    reporter_assert!(reporter, typeface2.is_some());
});

/// The `test` lambda of `TypefaceAxes`. `also_accepted_axis_tag_count` is a count the typeface
/// may report instead of the expected one (-1 for none).
// Port of: tests/TypefaceTest.cpp#L335-L380 (chrome/m156)
fn typeface_axes_test(
    reporter: &mut Reporter,
    typeface: Option<&Typeface>,
    expected: &[Coordinate],
    also_accepted_axis_tag_count: Option<usize>,
) {
    let Some(typeface) = typeface else {
        return; // Not all SkFontMgr can makeFromStream().
    };
    let Some(actual_count) = typeface.get_variation_design_position(&mut []) else {
        return; // The number of axes is unknown.
    };
    reporter_assert!(
        reporter,
        actual_count == expected.len() || Some(actual_count) == also_accepted_axis_tag_count
    );
    // Variable font conservative bounds don't vary, so ensure they aren't reported.
    reporter_assert!(reporter, typeface.get_bounds().is_empty());
    let mut actual = vec![Coordinate::default(); actual_count];
    let Some(actual_count) = typeface.get_variation_design_position(&mut actual) else {
        return; // The position cannot be determined.
    };
    reporter_assert!(
        reporter,
        actual_count == expected.len() || Some(actual_count) == also_accepted_axis_tag_count
    );
    // Every actual must be expected.
    let mut expected_used = vec![false; expected.len()];
    for actual_coordinate in actual.iter().take(actual_count) {
        let mut actual_found = false;
        for (expected_idx, expected_coordinate) in expected.iter().enumerate() {
            if expected_used[expected_idx] {
                continue;
            }
            if actual_coordinate.axis != expected_coordinate.axis {
                continue;
            }
            // Convert to fixed for "almost equal".
            let fixed_read = scalar_to_fixed(actual_coordinate.value);
            let fixed_original = scalar_to_fixed(expected_coordinate.value);
            if (fixed_read - fixed_original).abs() >= 2 {
                continue;
            }
            // This actual matched an unused expected.
            actual_found = true;
            expected_used[expected_idx] = true;
            break;
        }
        reporter_assert!(
            reporter,
            actual_found,
            "Actual axis '{}' with value '{}' not expected",
            tag_to_string(actual_coordinate.axis),
            actual_coordinate.value
        );
    }
}

// Port of: tests/TypefaceTest.cpp#L333-L462 (chrome/m156)
def_font_test!(TypefaceAxes, |reporter| {
    crate::skip_without_resources!();
    let fm = test_font_mgr();
    let wght = set_four_byte_tag(b'w', b'g', b'h', b't');
    let wdth = set_four_byte_tag(b'w', b'd', b't', b'h');

    // Not specifying a position should produce the default.
    {
        let Some(variable) = resource_stream("fonts/Variable.ttf") else {
            errorf!(reporter, "variable");
            return;
        };
        let default_position = [
            Coordinate {
                axis: wght,
                value: 400.0,
            },
            Coordinate {
                axis: wdth,
                value: 100.0,
            },
        ];
        let typeface = fm.make_from_stream(Some(variable), 0);
        typeface_axes_test(reporter, typeface.as_ref(), &default_position, None);
    }
    // Multiple axes with the same tag (and min, max, default) works.
    {
        let Some(dup_tags) = resource_stream("fonts/VaryAlongQuads.ttf") else {
            errorf!(reporter, "dupTags");
            return;
        };
        // The position may be over specified. If there are multiple values for a given axis,
        // ensure the last one since that's what css-fonts-4 requires.
        let position = [
            Coordinate {
                axis: wght,
                value: 700.0,
            },
            Coordinate {
                axis: wght,
                value: 600.0,
            },
            Coordinate {
                axis: wght,
                value: 600.0,
            },
        ];
        let params = position_args(&position);
        let typeface = fm.make_from_stream_args(Some(dup_tags), &params);
        typeface_axes_test(reporter, typeface.as_ref(), &position[1..3], Some(1));
    }
    // Overspecifying an axis tag value applies the last one in the list.
    {
        let Some(distortable) = resource_stream("fonts/Distortable.ttf") else {
            errorf!(reporter, "distortable");
            return;
        };
        // The position may be over specified. If there are multiple values for a given axis,
        // ensure the last one since that's what css-fonts-4 requires.
        let position = [
            // `1.618033988749895f` rounds to the f32 golden ratio.
            Coordinate {
                axis: wght,
                value: std::f32::consts::GOLDEN_RATIO,
            },
            // SK_ScalarSqrt2
            Coordinate {
                axis: wght,
                value: std::f32::consts::SQRT_2,
            },
        ];
        let params = position_args(&position);
        let typeface = fm.make_from_stream_args(Some(distortable), &params);
        typeface_axes_test(reporter, typeface.as_ref(), &position[1..2], None);
        if let Some(typeface) = &typeface {
            // Cloning without specifying any parameters should produce an equivalent variation.
            let clone = typeface.make_clone(&FontArguments::new());
            typeface_axes_test(reporter, Some(&clone), &position[1..2], None);
        }
    }
});

// Port of: tests/TypefaceTest.cpp#L464-L500 (chrome/m156)
def_font_test!(
    #[allow(clippy::float_cmp)]
    TypefaceVariationIndex,
    |reporter| {
        crate::skip_without_resources!();
        // The C++ compares the float with `==`; 0.5 is exact in both.
        let Some(distortable) = resource_stream("fonts/Distortable.ttf") else {
            errorf!(reporter, "distortable");
            return;
        };
        let fm = test_font_mgr();
        let mut params = FontArguments::new();
        // The first named variation position in Distortable is 'Thin'.
        params.set_collection_index(0x0001_0000);
        let Some(typeface) = fm.make_from_stream_args(Some(distortable), &params) else {
            // FreeType is the only weird thing that supports this, Skia just needs to make sure if it
            // gets one of these things make sense.
            return;
        };
        let Some(count) = typeface.get_variation_design_position(&mut []) else {
            errorf!(reporter, "count == 1");
            return;
        };
        if count != 1 {
            errorf!(reporter, "count == 1");
            return;
        }
        let mut position_read = [Coordinate::default(); 1];
        let Some(count) = typeface.get_variation_design_position(&mut position_read) else {
            return;
        };
        if count != 1 {
            errorf!(reporter, "count == 1");
            return;
        }
        reporter_assert!(
            reporter,
            position_read[0].axis == set_four_byte_tag(b'w', b'g', b'h', b't')
        );
        reporter_assert!(
            reporter,
            position_read[0].value == 0.5,
            "positionRead[0].value: {}",
            position_read[0].value
        );
    }
);

/// The `test` lambda of `TypefaceAxesParameters`.
// Port of: tests/TypefaceTest.cpp#L516-L653 (chrome/m156)
fn typeface_axes_parameters_test(
    reporter: &mut Reporter,
    typeface: Option<&Typeface>,
    expected: &[Axis],
    also_accepted_axis_tag_count: Option<usize>,
) {
    let Some(typeface) = typeface else {
        return; // Not all SkFontMgr can makeFromStream().
    };
    let Some(actual_count) = typeface.get_variation_design_parameters(&mut []) else {
        return; // The number of axes is unknown.
    };
    reporter_assert!(
        reporter,
        actual_count == expected.len() || Some(actual_count) == also_accepted_axis_tag_count
    );
    let mut actual = vec![Axis::new(0, 0.0, 0.0, 0.0, false); actual_count];
    let Some(actual_count) = typeface.get_variation_design_parameters(&mut actual) else {
        return; // The position cannot be determined.
    };
    reporter_assert!(
        reporter,
        actual_count == expected.len() || Some(actual_count) == also_accepted_axis_tag_count
    );
    // Every actual must be expected.
    let mut expected_used = vec![false; expected.len()];
    for actual_axis in actual.iter().take(actual_count) {
        let mut actual_found = false;
        for (expected_idx, expected_axis) in expected.iter().enumerate() {
            if expected_used[expected_idx] {
                continue;
            }
            if actual_axis.tag != expected_axis.tag {
                continue;
            }
            // Convert to fixed for "almost equal".
            let fixed_actual_min = scalar_to_fixed(actual_axis.min);
            let fixed_expected_min = scalar_to_fixed(expected_axis.min);
            if (fixed_actual_min - fixed_expected_min).abs() >= 2 {
                continue;
            }
            let fixed_actual_max = scalar_to_fixed(actual_axis.max);
            let fixed_expected_max = scalar_to_fixed(expected_axis.max);
            if (fixed_actual_max - fixed_expected_max).abs() >= 2 {
                continue;
            }
            let fixed_actual_default = scalar_to_fixed(actual_axis.def);
            let fixed_expected_default = scalar_to_fixed(expected_axis.def);
            if (fixed_actual_default - fixed_expected_default).abs() >= 2 {
                continue;
            }
            // This seems silly, but allows MSAN to ensure that isHidden is initialized.
            if actual_axis.is_hidden() && actual_axis.is_hidden() != expected_axis.is_hidden() {
                continue;
            }
            // This actual matched an unused expected.
            actual_found = true;
            expected_used[expected_idx] = true;
            break;
        }
        reporter_assert!(
            reporter,
            actual_found,
            "Actual axis '{}' with min {} max {} default {} hidden {} not expected",
            tag_to_string(actual_axis.tag),
            actual_axis.min,
            actual_axis.max,
            actual_axis.def,
            if actual_axis.is_hidden() {
                "true"
            } else {
                "false"
            }
        );
    }
}

// Port of: tests/TypefaceTest.cpp#L516-L653 (chrome/m156)
def_font_test!(TypefaceAxesParameters, |reporter| {
    crate::skip_without_resources!();
    let fm = test_font_mgr();

    // Two axis OpenType variable font.
    {
        let Some(variable) = resource_stream("fonts/Variable.ttf") else {
            errorf!(reporter, "variable");
            return;
        };
        let expected = [
            Axis::new(
                set_four_byte_tag(b'w', b'g', b'h', b't'),
                100.0,
                400.0,
                900.0,
                true,
            ),
            Axis::new(
                set_four_byte_tag(b'w', b'd', b't', b'h'),
                50.0,
                100.0,
                200.0,
                false,
            ),
        ];
        let typeface = fm.make_from_stream(Some(variable), 0);
        typeface_axes_parameters_test(reporter, typeface.as_ref(), &expected, None);
    }
    // Multiple axes with the same tag (and min, max, default) works.
    {
        let Some(dup_tags) = resource_stream("fonts/VaryAlongQuads.ttf") else {
            errorf!(reporter, "dupTags");
            return;
        };
        let expected = [
            Axis::new(
                set_four_byte_tag(b'w', b'g', b'h', b't'),
                100.0,
                400.0,
                900.0,
                false,
            ),
            Axis::new(
                set_four_byte_tag(b'w', b'g', b'h', b't'),
                100.0,
                400.0,
                900.0,
                false,
            ),
        ];
        let typeface = fm.make_from_stream(Some(dup_tags), 0);
        typeface_axes_parameters_test(reporter, typeface.as_ref(), &expected, Some(1));
    }
    // Simple single axis GX variable font.
    {
        let Some(distortable) = resource_stream("fonts/Distortable.ttf") else {
            errorf!(reporter, "distortable");
            return;
        };
        let expected = [Axis::new(
            set_four_byte_tag(b'w', b'g', b'h', b't'),
            0.5,
            1.0,
            2.0,
            true,
        )];
        let typeface = fm.make_from_stream(Some(distortable), 0);
        typeface_axes_parameters_test(reporter, typeface.as_ref(), &expected, None);
    }
});

// Port of: tests/TypefaceTest.cpp#L667-L686 (chrome/m156)
def_test!(TypefaceCache, |reporter| {
    let t1 = TestEmptyTypeface::make();
    {
        let mut cache = TypefaceCache::new();
        reporter_assert!(reporter, count(reporter, &cache) == 0);
        {
            let t0 = TestEmptyTypeface::make();
            cache.add(t0.clone());
            reporter_assert!(reporter, count(reporter, &cache) == 1);
            cache.add(t1.clone());
            reporter_assert!(reporter, count(reporter, &cache) == 2);
            cache.purge_all();
            reporter_assert!(reporter, count(reporter, &cache) == 2);
        }
        reporter_assert!(reporter, count(reporter, &cache) == 2);
        cache.purge_all();
        reporter_assert!(reporter, count(reporter, &cache) == 1);
    }
    reporter_assert!(reporter, t1.is_unique());
});

/// `check_serialize_behaviors` (tests/TypefaceTest.cpp).
// Port of: tests/TypefaceTest.cpp#L688-L708 (chrome/m156)
fn check_serialize_behaviors(reporter: &mut Reporter, typeface: Option<Typeface>) {
    let Some(typeface) = typeface else {
        return;
    };
    let (_desc, serialize) = typeface.font_descriptor();
    let data0 = typeface.serialize(SerializeBehavior::DoIncludeData);
    let data1 = typeface.serialize(SerializeBehavior::DontIncludeData);
    let data2 = typeface.serialize(SerializeBehavior::IncludeDataIfLocal);
    let (Some(data0), Some(data1), Some(data2)) = (data0, data1, data2) else {
        errorf!(reporter, "serialize returned null");
        return;
    };

    reporter_assert!(reporter, data0.size() >= data1.size());
    if serialize {
        reporter_assert!(reporter, data0.equals(Some(&data2)));
    } else {
        reporter_assert!(reporter, data1.equals(Some(&data2)));
    }
}

// Port of: tests/TypefaceTest.cpp#L710-L715 (chrome/m156)
def_font_test!(Typeface_serialize, |reporter| {
    check_serialize_behaviors(reporter, Some(default_typeface()));
    check_serialize_behaviors(
        reporter,
        resource_stream("fonts/Distortable.ttf")
            .and_then(|stream| test_font_mgr().make_from_stream(Some(stream), 0)),
    );
});

// Port of: tests/TypefaceTest.cpp#L760-L782 (chrome/m156)
// This test makes sure the legacy typeface creation does not lose its specified
// style. See https://bugs.chromium.org/p/skia/issues/detail?id=8447 for more
// context.
def_font_test!(LegacyMakeTypeface, |reporter| {
    let fm = test_font_mgr();
    let typeface1 = fm.legacy_make_typeface(None, FontStyle::italic());
    let typeface2 = fm.legacy_make_typeface(None, FontStyle::bold());
    let typeface3 = fm.legacy_make_typeface(None, FontStyle::bold_italic());
    if typeface1.is_some() || typeface2.is_some() || typeface3.is_some() {
        // The C++ condition is `typeface1 && typeface2 && typeface1`.
        reporter_assert!(
            reporter,
            typeface1.is_some() && typeface2.is_some() && typeface1.is_some()
        );
    }
    if let Some(typeface1) = &typeface1 {
        reporter_assert!(reporter, typeface1.is_italic());
        reporter_assert!(reporter, !typeface1.is_bold());
    }
    if let Some(typeface2) = &typeface2 {
        reporter_assert!(reporter, !typeface2.is_italic());
        reporter_assert!(reporter, typeface2.is_bold());
    }
    if let Some(typeface3) = &typeface3 {
        reporter_assert!(reporter, typeface3.is_italic());
        reporter_assert!(reporter, typeface3.is_bold());
    }
});

/// `count_proc` and `count`: the number of typefaces the cache holds, counted with a search that
/// never matches.
// Port of: tests/TypefaceTest.cpp#L655-L665 (chrome/m156)
fn count(reporter: &mut Reporter, cache: &TypefaceCache) -> i32 {
    let mut count = 0;
    let none = cache.find_by_proc_and_ref(|_| {
        count += 1;
        false
    });
    reporter_assert!(reporter, none.is_none());
    count
}

// Port of: tests/TypefaceTest.cpp#L309-L331 (chrome/m156)
// The C++ test compares the float with `==`; the value is exact, so the comparison is too.
def_test!(
    #[allow(clippy::float_cmp)]
    FontDescriptorNegativeVariationSerialize,
    |reporter| {
        let mut desc = FontDescriptor::new();
        let style = FontStyle::new(Weight::from(2), Width::from(9), Slant::Oblique);
        desc.set_style(style);
        let postscript_name = "postscript";
        desc.set_postscript_name(postscript_name);
        let variation = desc.set_variation_coordinates(1);
        variation[0] = Coordinate {
            axis: 0,
            value: -1.0,
        };

        let mut stream = DynamicMemoryWStream::new();
        reporter_assert!(reporter, desc.serialize(&mut stream));
        let mut read_back = stream.detach_as_stream();
        // C++ leaves the output descriptor default-constructed when Deserialize fails, and the
        // assertions below then fail; `unwrap_or_default` keeps that behavior.
        let desc_d = FontDescriptor::deserialize(read_back.as_mut(), None).unwrap_or_default();

        reporter_assert!(reporter, desc_d.style() == style);
        reporter_assert!(reporter, desc.postscript_name() == postscript_name);
        if desc_d.variation().len() == 1 {
            reporter_assert!(reporter, desc_d.variation()[0].value == -1.0);
        } else {
            errorf!(reporter, "descD.getVariationCoordinateCount() != 1");
        }
    }
);

/// `SkTypeface::Equal(a, b)`: two nulls are equal, a null and a typeface are not.
// Port of: src/core/SkTypeface.cpp#L148-L156 (chrome/m156)
fn typeface_equal(a: Option<&Typeface>, b: Option<&Typeface>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

// Port of: tests/TypefaceTest.cpp#L502-L514 (chrome/m156)
def_font_test!(Typeface, |reporter| {
    let t1 = create_test_typeface(None, FontStyle::default());
    let t2 = default_typeface();

    reporter_assert!(reporter, typeface_equal(Some(&t1), Some(&t2)));
    reporter_assert!(reporter, typeface_equal(None, None));

    reporter_assert!(reporter, !typeface_equal(None, Some(&t1)));
    reporter_assert!(reporter, !typeface_equal(None, Some(&t2)));
    reporter_assert!(reporter, !typeface_equal(Some(&t1), None));
    reporter_assert!(reporter, !typeface_equal(Some(&t2), None));
});

// Port of: tests/TypefaceTest.cpp#L784-L798 (chrome/m156), CustomTypeface_invalid_glyphid
def_test!(CustomTypeface_invalid_glyphid, |reporter| {
    let glyph_path = Path::rect(Rect::from_ltrb(10.0, 20.0, 30.0, 40.0), None);

    let mut builder = CustomTypefaceBuilder::new();
    builder.set_glyph(0, 42.0, &glyph_path);

    let Some(typeface) = builder.detach() else {
        errorf!(reporter, "the builder has a glyph");
        return;
    };
    let custom_font = Font::from_size(typeface, 1.0);

    let glyph_ids: [GlyphId; 2] = [0, 1];
    let mut widths = [0.0; 2];
    let mut bounds = [Rect::default(); 2];
    custom_font.get_widths_bounds(&glyph_ids, &mut widths, &mut bounds, None);

    reporter_assert!(
        reporter,
        bounds[0] == Rect::from_ltrb(10.0, 20.0, 30.0, 40.0)
    );
    reporter_assert!(reporter, bounds[1] == Rect::from_ltrb(0.0, 0.0, 0.0, 0.0));
});

// Port of: tests/TypefaceTest.cpp#L717-L755 (chrome/m156)
def_test!(Typeface_glyph_to_char, |reporter| {
    let emoji_sample = emoji_sample_default();
    let typeface = emoji_sample.typeface.expect("an emoji typeface");
    let font = Font::from_size(typeface, 12.0);
    let text = emoji_sample.sample_text.as_bytes();
    let family_name = font.typeface().family_name();

    let codepoint_count = usize::try_from(count_utf8(text)).unwrap();
    let mut remaining = text;
    let original_codepoints: Vec<Unichar> = (0..codepoint_count)
        .map(|_| next_utf8(&mut remaining))
        .collect();
    let mut glyphs = vec![0 as GlyphId; codepoint_count];
    font.unichars_to_glyphs(&original_codepoints, &mut glyphs);
    if glyphs.contains(&0) {
        errorf!(
            reporter,
            "Unexpected typeface \"{}\". Expected full support for emoji_sample_text.",
            family_name
        );
        return;
    }

    let mut new_codepoints = vec![0 as Unichar; codepoint_count];
    glyphs_to_unichars(&font, &glyphs, &mut new_codepoints);

    for i in 0..codepoint_count {
        // GDI does not support character to glyph mapping outside BMP. The font manager is never
        // GDI here (ToolUtils::FontMgrIsGDI is false), so that skip never applies.
        // If two codepoints map to the same glyph then this assert is not valid.
        // However, the emoji test font should never have multiple characters map to the same glyph.
        reporter_assert!(
            reporter,
            original_codepoints[i] == new_codepoints[i],
            "name:{} i:{} original:{} new:{} glyph:{}",
            family_name,
            i,
            original_codepoints[i],
            new_codepoints[i],
            glyphs[i]
        );
    }
});
