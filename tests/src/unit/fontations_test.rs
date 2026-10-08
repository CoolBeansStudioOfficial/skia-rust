// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/FontationsTest.cpp (chrome/m156), the Fontations typeface cases (T19a) and the
// outline cases that need the Fontations scaler context (T19b).

#![cfg(test)]

use skia_rust_core::font::Font;
use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_arguments::VariationPosition;
use skia_rust_core::font_arguments::variation_position::Coordinate;
use skia_rust_core::font_parameters::variation::Axis;
use skia_rust_core::font_types::set_four_byte_tag;
use skia_rust_core::stream::{MemoryStream, StreamAsset};
use skia_rust_core::typeface::Typeface;
use skia_rust_core::utf::Unichar;
use skia_rust_text::ports::fontations::typeface::make_from_stream;

use crate::resources::get_resource_as_data;
use crate::{def_test, reporter_assert, skip_missing_resource};

const FONT_RESOURCE: &str = "fonts/ahem.ttf";
const TTC_RESOURCE: &str = "fonts/test.ttc";
const VARIABLE_RESOURCE: &str = "fonts/test_glyphs-glyf_colr_1_variable.ttf";
const NUM_VARIABLE_AXES: usize = 44;
const NO_CAP_HEIGHT_RESOURCE: &str = "fonts/DejaVuSans.subset.ttf";
const NO_CAP_HEIGHT_NO_HX_RESOURCE: &str = "fonts/DejaVuSans.subset_noHx.ttf";

/// `GetResourceAsStream(path)`: the resource as a memory stream, or `None` if it is missing.
fn resource_stream(path: &str) -> Option<Box<dyn StreamAsset>> {
    let data = get_resource_as_data(path)?;
    Some(MemoryStream::make_copy(&data))
}

/// The typeface of a stream that the test needs: a test whose font does not make a typeface
/// fails here (C++ would dereference the null typeface).
fn typeface_for(stream: Box<dyn StreamAsset>, args: &FontArguments<'_, '_>) -> Typeface {
    make_from_stream(stream, args).expect("the resource is a font that makes a typeface")
}

/// One row of `axisExpectations`: the tag, and the min, default and max of the axis.
struct AxisExpectation {
    tag: u32,
    min_value: f32,
    def_value: f32,
    max_value: f32,
}

/// `axisExpectations` (tests/FontationsTest.cpp): the axes of the variable test font, in order.
// Port of: tests/FontationsTest.cpp#L18-L69 (chrome/m156)
// The literals are the C++ `float` values of the expectations: Rust rounds them to f32 as C++ does.
#[allow(clippy::excessive_precision, clippy::unreadable_literal)]
#[rustfmt::skip]
const AXIS_EXPECTATIONS: [AxisExpectation; NUM_VARIABLE_AXES] = [
    AxisExpectation { tag: set_four_byte_tag(b'S', b'W', b'P', b'S'), min_value: -90.0, def_value: 0.0, max_value: 90.0 },
    AxisExpectation { tag: set_four_byte_tag(b'S', b'W', b'P', b'E'), min_value: -90.0, def_value: 0.0, max_value: 90.0 },
    AxisExpectation { tag: set_four_byte_tag(b'S', b'W', b'C', b'1'), min_value: -2.0, def_value: 0.0, max_value: 2.0 },
    AxisExpectation { tag: set_four_byte_tag(b'S', b'W', b'C', b'2'), min_value: -2.0, def_value: 0.0, max_value: 2.0 },
    AxisExpectation { tag: set_four_byte_tag(b'S', b'W', b'C', b'3'), min_value: -2.0, def_value: 0.0, max_value: 2.0 },
    AxisExpectation { tag: set_four_byte_tag(b'S', b'W', b'C', b'4'), min_value: -2.0, def_value: 0.0, max_value: 2.0 },
    AxisExpectation { tag: set_four_byte_tag(b'S', b'C', b'O', b'X'), min_value: -200., def_value: 0.0, max_value: 200. },
    AxisExpectation { tag: set_four_byte_tag(b'S', b'C', b'O', b'Y'), min_value: -200., def_value: 0.0, max_value: 200. },
    AxisExpectation { tag: set_four_byte_tag(b'S', b'C', b'S', b'X'), min_value: -2.0, def_value: 0.0, max_value: 1.9999389648437 },
    AxisExpectation { tag: set_four_byte_tag(b'S', b'C', b'S', b'Y'), min_value: -2.0, def_value: 0.0, max_value: 1.9999389648437 },
    AxisExpectation { tag: set_four_byte_tag(b'G', b'R', b'X', b'0'), min_value: -1000.0, def_value: 0.0, max_value: 1000.0 },
    AxisExpectation { tag: set_four_byte_tag(b'G', b'R', b'Y', b'0'), min_value: -1000.0, def_value: 0.0, max_value: 1000.0 },
    AxisExpectation { tag: set_four_byte_tag(b'G', b'R', b'X', b'1'), min_value: -1000.0, def_value: 0.0, max_value: 1000.0 },
    AxisExpectation { tag: set_four_byte_tag(b'G', b'R', b'Y', b'1'), min_value: -1000.0, def_value: 0.0, max_value: 1000.0 },
    AxisExpectation { tag: set_four_byte_tag(b'G', b'R', b'X', b'2'), min_value: -1000.0, def_value: 0.0, max_value: 1000.0 },
    AxisExpectation { tag: set_four_byte_tag(b'G', b'R', b'Y', b'2'), min_value: -1000.0, def_value: 0.0, max_value: 1000.0 },
    AxisExpectation { tag: set_four_byte_tag(b'G', b'R', b'R', b'0'), min_value: -1000.0, def_value: 0.0, max_value: 1000.0 },
    AxisExpectation { tag: set_four_byte_tag(b'G', b'R', b'R', b'1'), min_value: -1000.0, def_value: 0.0, max_value: 1000.0 },
    AxisExpectation { tag: set_four_byte_tag(b'C', b'O', b'L', b'1'), min_value: -2.0, def_value: 0.0, max_value: 2.0 },
    AxisExpectation { tag: set_four_byte_tag(b'C', b'O', b'L', b'2'), min_value: -2.0, def_value: 0.0, max_value: 2.0 },
    AxisExpectation { tag: set_four_byte_tag(b'C', b'O', b'L', b'3'), min_value: -2.0, def_value: 0.0, max_value: 2.0 },
    AxisExpectation { tag: set_four_byte_tag(b'R', b'O', b'T', b'A'), min_value: 0.0, def_value: 0.0, max_value: 539.989013671875 },
    AxisExpectation { tag: set_four_byte_tag(b'R', b'O', b'T', b'X'), min_value: -500.0, def_value: 0.0, max_value: 500.0 },
    AxisExpectation { tag: set_four_byte_tag(b'R', b'O', b'T', b'Y'), min_value: -500.0, def_value: 0.0, max_value: 500.0 },
    AxisExpectation { tag: set_four_byte_tag(b'S', b'K', b'X', b'A'), min_value: -90.0, def_value: 0.0, max_value: 90.0 },
    AxisExpectation { tag: set_four_byte_tag(b'S', b'K', b'Y', b'A'), min_value: -90.0, def_value: 0.0, max_value: 90.0 },
    AxisExpectation { tag: set_four_byte_tag(b'S', b'K', b'C', b'X'), min_value: -500.0, def_value: 0.0, max_value: 500.0 },
    AxisExpectation { tag: set_four_byte_tag(b'S', b'K', b'C', b'Y'), min_value: -500.0, def_value: 0.0, max_value: 500.0 },
    AxisExpectation { tag: set_four_byte_tag(b'T', b'R', b'X', b'X'), min_value: -2.0, def_value: 0.0, max_value: 2.0 },
    AxisExpectation { tag: set_four_byte_tag(b'T', b'R', b'Y', b'X'), min_value: -2.0, def_value: 0.0, max_value: 2.0 },
    AxisExpectation { tag: set_four_byte_tag(b'T', b'R', b'X', b'Y'), min_value: -2.0, def_value: 0.0, max_value: 2.0 },
    AxisExpectation { tag: set_four_byte_tag(b'T', b'R', b'Y', b'Y'), min_value: -2.0, def_value: 0.0, max_value: 2.0 },
    AxisExpectation { tag: set_four_byte_tag(b'T', b'R', b'D', b'X'), min_value: -500.0, def_value: 0.0, max_value: 500.0 },
    AxisExpectation { tag: set_four_byte_tag(b'T', b'R', b'D', b'Y'), min_value: -500.0, def_value: 0.0, max_value: 500.0 },
    AxisExpectation { tag: set_four_byte_tag(b'T', b'L', b'D', b'X'), min_value: -500.0, def_value: 0.0, max_value: 500.0 },
    AxisExpectation { tag: set_four_byte_tag(b'T', b'L', b'D', b'Y'), min_value: -500.0, def_value: 0.0, max_value: 500.0 },
    AxisExpectation { tag: set_four_byte_tag(b'C', b'L', b'X', b'I'), min_value: -500.0, def_value: 0.0, max_value: 500.0 },
    AxisExpectation { tag: set_four_byte_tag(b'C', b'L', b'Y', b'I'), min_value: -500.0, def_value: 0.0, max_value: 500.0 },
    AxisExpectation { tag: set_four_byte_tag(b'C', b'L', b'X', b'A'), min_value: -500.0, def_value: 0.0, max_value: 500.0 },
    AxisExpectation { tag: set_four_byte_tag(b'C', b'L', b'Y', b'A'), min_value: -500.0, def_value: 0.0, max_value: 500.0 },
    AxisExpectation { tag: set_four_byte_tag(b'C', b'L', b'I', b'O'), min_value: -500.0, def_value: 0.0, max_value: 500.0 },
    AxisExpectation { tag: set_four_byte_tag(b'A', b'P', b'H', b'1'), min_value: -1.0, def_value: 0.0, max_value: 0.0 },
    AxisExpectation { tag: set_four_byte_tag(b'A', b'P', b'H', b'2'), min_value: -1.0, def_value: 0.0, max_value: 0.0 },
    AxisExpectation { tag: set_four_byte_tag(b'A', b'P', b'H', b'3'), min_value: -1.0, def_value: 0.0, max_value: 0.0 },
];

// Port of: tests/FontationsTest.cpp#L71-L76 (chrome/m156)
def_test!(Fontations_DoNotMakeFromNull, |reporter| {
    // SkMemoryStream::MakeDirect(nullptr, 0): an empty stream.
    let null_stream: Box<dyn StreamAsset> = MemoryStream::make_copy(&[]);
    let probe_typeface = make_from_stream(null_stream, &FontArguments::new());
    reporter_assert!(reporter, probe_typeface.is_none());
});

// Port of: tests/FontationsTest.cpp#L78-L84 (chrome/m156)
def_test!(Fontations_DoNotMakeFromNonSfnt, |reporter| {
    // `char notAnSfnt[] = "I_AM_NOT_AN_SFNT";` includes the terminating NUL in `std::size`.
    let not_an_sfnt: &[u8] = b"I_AM_NOT_AN_SFNT\0";
    let not_sfnt_stream: Box<dyn StreamAsset> = MemoryStream::make_copy(not_an_sfnt);
    let probe_typeface = make_from_stream(not_sfnt_stream, &FontArguments::new());
    reporter_assert!(reporter, probe_typeface.is_none());
});

// Port of: tests/FontationsTest.cpp#L86-L91 (chrome/m156)
def_test!(Fontations_MakeFromFont, |reporter| {
    let stream = skip_missing_resource!(resource_stream(FONT_RESOURCE), FONT_RESOURCE);
    let probe_typeface = make_from_stream(stream, &FontArguments::new());
    reporter_assert!(reporter, probe_typeface.is_some());
});

// Port of: tests/FontationsTest.cpp#L93-L98 (chrome/m156)
def_test!(Fontations_MakeFromCollection, |reporter| {
    let stream = skip_missing_resource!(resource_stream(TTC_RESOURCE), TTC_RESOURCE);
    let probe_typeface = make_from_stream(stream, &FontArguments::new());
    reporter_assert!(reporter, probe_typeface.is_some());
});

// Port of: tests/FontationsTest.cpp#L100-L107 (chrome/m156)
def_test!(Fontations_MakeFromCollectionNonNullIndex, |reporter| {
    let mut args = FontArguments::new();
    args.set_collection_index(1);
    let stream = skip_missing_resource!(resource_stream(TTC_RESOURCE), TTC_RESOURCE);
    let probe_typeface = make_from_stream(stream, &args);
    reporter_assert!(reporter, probe_typeface.is_some());
});

// Port of: tests/FontationsTest.cpp#L109-L116 (chrome/m156)
def_test!(
    Fontations_DoNotMakeFromCollection_Invalid_Index,
    |reporter| {
        let mut args = FontArguments::new();
        args.set_collection_index(1000);
        let stream = skip_missing_resource!(resource_stream(TTC_RESOURCE), TTC_RESOURCE);
        let probe_typeface = make_from_stream(stream, &args);
        reporter_assert!(reporter, probe_typeface.is_none());
    }
);

// Port of: tests/FontationsTest.cpp#L118-L154 (chrome/m156)
def_test!(Fontations_TableData, |reporter| {
    const NAME_TABLE_SIZE: usize = 11310;
    const TEST_OFFSET: usize = 1310;
    const TEST_LENGTH: usize = 500;
    let mut dest_buffer = [0_u8; NAME_TABLE_SIZE];
    let test_typeface = typeface_for(
        skip_missing_resource!(resource_stream(FONT_RESOURCE), FONT_RESOURCE),
        &FontArguments::new(),
    );
    let name_table_tag = set_four_byte_tag(b'n', b'a', b'm', b'e');
    let non_existant_tag = set_four_byte_tag(b'0', b'X', b'0', b'X');

    // Getting size without buffer.
    reporter_assert!(
        reporter,
        test_typeface.get_table_data(name_table_tag, 0, NAME_TABLE_SIZE, None) == NAME_TABLE_SIZE
    );
    // Reading full table.
    reporter_assert!(
        reporter,
        test_typeface.get_table_data(name_table_tag, 0, NAME_TABLE_SIZE, Some(&mut dest_buffer))
            == NAME_TABLE_SIZE
    );
    // Reading restricted length.
    reporter_assert!(
        reporter,
        test_typeface.get_table_data(name_table_tag, 0, TEST_LENGTH, Some(&mut dest_buffer))
            == TEST_LENGTH
    );
    reporter_assert!(
        reporter,
        test_typeface.get_table_data(
            name_table_tag,
            TEST_OFFSET,
            TEST_LENGTH,
            Some(&mut dest_buffer)
        ) == TEST_LENGTH
    );
    // Reading at an offset.
    reporter_assert!(
        reporter,
        test_typeface.get_table_data(
            name_table_tag,
            TEST_OFFSET,
            NAME_TABLE_SIZE,
            Some(&mut dest_buffer)
        ) == NAME_TABLE_SIZE - TEST_OFFSET
    );

    // Reading from offset past table.
    reporter_assert!(
        reporter,
        test_typeface.get_table_data(
            name_table_tag,
            NAME_TABLE_SIZE,
            NAME_TABLE_SIZE,
            Some(&mut dest_buffer)
        ) == 0
    );
    reporter_assert!(
        reporter,
        test_typeface.get_table_data(name_table_tag, NAME_TABLE_SIZE, 0, None) == 0
    );
    // Reading one byte before end of table.
    reporter_assert!(
        reporter,
        test_typeface.get_table_data(
            name_table_tag,
            NAME_TABLE_SIZE - 1,
            NAME_TABLE_SIZE,
            Some(&mut dest_buffer)
        ) == 1
    );
    // Trying to start reading at an offset past table start.
    reporter_assert!(
        reporter,
        test_typeface.get_table_data(
            name_table_tag,
            0,
            NAME_TABLE_SIZE + 10,
            Some(&mut dest_buffer)
        ) == NAME_TABLE_SIZE
    );
    // Restricting length without target buffer.
    reporter_assert!(
        reporter,
        test_typeface.get_table_data(name_table_tag, 0, TEST_LENGTH, None) == TEST_LENGTH
    );

    // Trying to access non-existant table.
    reporter_assert!(
        reporter,
        test_typeface.get_table_data(non_existant_tag, 0, NAME_TABLE_SIZE, Some(&mut dest_buffer))
            == 0
    );
    reporter_assert!(
        reporter,
        test_typeface.get_table_data(non_existant_tag, 0, 0, None) == 0
    );
    reporter_assert!(
        reporter,
        test_typeface.get_table_data(non_existant_tag, TEST_OFFSET, 0, None) == 0
    );
});

// Port of: tests/FontationsTest.cpp#L156-L171 (chrome/m156)
def_test!(Fontations_TableTags, |reporter| {
    const NUM_TAGS: usize = 11;
    let mut tags_buffer = [0_u32; NUM_TAGS];
    let test_typeface = typeface_for(
        skip_missing_resource!(resource_stream(FONT_RESOURCE), FONT_RESOURCE),
        &FontArguments::new(),
    );
    let first_tag = set_four_byte_tag(b'O', b'S', b'/', b'2');
    let last_tag = set_four_byte_tag(b'p', b'o', b's', b't');

    reporter_assert!(reporter, test_typeface.count_tables() == NUM_TAGS);

    reporter_assert!(
        reporter,
        test_typeface.read_table_tags(&mut tags_buffer) == NUM_TAGS
    );
    reporter_assert!(reporter, tags_buffer[0] == first_tag);
    reporter_assert!(reporter, tags_buffer[NUM_TAGS - 1] == last_tag);
});

// Port of: tests/FontationsTest.cpp#L173-L205 (chrome/m156)
// The values are the ones set through the clone, which are copied exactly.
def_test!(
    #[allow(clippy::float_cmp)]
    Fontations_VariationPosition,
    |reporter| {
        let variable_typeface = typeface_for(
            skip_missing_resource!(resource_stream(VARIABLE_RESOURCE), VARIABLE_RESOURCE),
            &FontArguments::new(),
        );
        // Everything at default.
        let num_axes = variable_typeface.get_variation_design_position(&mut []);
        reporter_assert!(
            reporter,
            num_axes == Some(NUM_VARIABLE_AXES),
            "numAxes: {num_axes:?}"
        );

        let swps_coordinate_first = Coordinate {
            axis: set_four_byte_tag(b'S', b'W', b'P', b'S'),
            value: 25.0,
        };
        let swps_coordinate_second = Coordinate {
            axis: set_four_byte_tag(b'S', b'W', b'P', b'S'),
            value: 55.0,
        };
        let swpe_coordinate = Coordinate {
            axis: set_four_byte_tag(b'S', b'W', b'P', b'E'),
            value: 45.0,
        };
        let invalid_coordinate = Coordinate {
            axis: set_four_byte_tag(b'_', b'_', b'_', b'_'),
            value: 0.0,
        };

        // 'SWPS' and 'SWPE' exist. Second 'SWPS' should override first, invalid tag should be stripped.
        let clone_coordinates = [
            swps_coordinate_first,
            swps_coordinate_second,
            swpe_coordinate,
            invalid_coordinate,
        ];
        let mut clone_args = FontArguments::new();
        clone_args.set_variation_design_position(VariationPosition {
            coordinates: &clone_coordinates,
        });

        let clone_typeface = variable_typeface.make_clone(&clone_args);
        let clone_num_axes = clone_typeface.get_variation_design_position(&mut []);
        reporter_assert!(
            reporter,
            clone_num_axes == Some(NUM_VARIABLE_AXES),
            "clonedNumAxes: {clone_num_axes:?}"
        );

        let mut retrieve_coordinates = [Coordinate::default(); NUM_VARIABLE_AXES];

        // Error when providing too little space.
        let bad_cloned_num_axes =
            clone_typeface.get_variation_design_position(&mut retrieve_coordinates[..1]);
        reporter_assert!(
            reporter,
            bad_cloned_num_axes.is_none(),
            "badClonedNumAxes: {bad_cloned_num_axes:?}"
        );

        let retrieved_cloned_num_axes =
            clone_typeface.get_variation_design_position(&mut retrieve_coordinates);
        reporter_assert!(
            reporter,
            retrieved_cloned_num_axes == Some(NUM_VARIABLE_AXES),
            "retrievedClonedNumAxes: {retrieved_cloned_num_axes:?}"
        );
        reporter_assert!(
            reporter,
            retrieve_coordinates[0].axis == swps_coordinate_second.axis
                && retrieve_coordinates[0].value == swps_coordinate_second.value
        );
        reporter_assert!(
            reporter,
            retrieve_coordinates[1].axis == swpe_coordinate.axis
                && retrieve_coordinates[1].value == swpe_coordinate.value
        );
    }
);

// Port of: tests/FontationsTest.cpp#L207-L221 (chrome/m156)
// The axis values are copied from the font, not computed, so the exact comparisons are the test.
def_test!(
    #[allow(clippy::float_cmp)]
    Fontations_VariationParameters,
    |reporter| {
        let variable_typeface = typeface_for(
            skip_missing_resource!(resource_stream(VARIABLE_RESOURCE), VARIABLE_RESOURCE),
            &FontArguments::new(),
        );
        reporter_assert!(
            reporter,
            variable_typeface.get_variation_design_parameters(&mut []) == Some(NUM_VARIABLE_AXES)
        );

        let mut axes = [Axis::default(); NUM_VARIABLE_AXES];
        reporter_assert!(
            reporter,
            variable_typeface.get_variation_design_parameters(&mut axes) == Some(NUM_VARIABLE_AXES)
        );

        for (axis, expectation) in axes.iter().zip(&AXIS_EXPECTATIONS) {
            reporter_assert!(reporter, axis.tag == expectation.tag);
            reporter_assert!(reporter, axis.min == expectation.min_value);
            reporter_assert!(reporter, axis.def == expectation.def_value);
            reporter_assert!(reporter, axis.max == expectation.max_value);
        }
    }
);

// Port of: tests/FontationsTest.cpp#L223-L234 (chrome/m156)
def_test!(Fontations_VariationParameters_BufferTooSmall, |reporter| {
    const ARRAY_TOO_SMALL: usize = 3;
    let variable_typeface = typeface_for(
        skip_missing_resource!(resource_stream(VARIABLE_RESOURCE), VARIABLE_RESOURCE),
        &FontArguments::new(),
    );
    reporter_assert!(
        reporter,
        variable_typeface.get_variation_design_parameters(&mut []) == Some(NUM_VARIABLE_AXES)
    );

    let mut axes = [Axis::default(); ARRAY_TOO_SMALL];
    reporter_assert!(
        reporter,
        variable_typeface
            .get_variation_design_parameters(&mut axes)
            .is_none()
    );
});

// Port of: tests/FontationsTest.cpp#L303-L334 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp)] // exact comparisons, as the C++ test makes them
    Fontations_SyntheticXHeight,
    |reporter| {
        let stream = skip_missing_resource!(
            resource_stream(NO_CAP_HEIGHT_RESOURCE),
            NO_CAP_HEIGHT_RESOURCE
        );
        let no_x_height_typeface = typeface_for(stream, &FontArguments::new());
        let stream = skip_missing_resource!(
            resource_stream(NO_CAP_HEIGHT_NO_HX_RESOURCE),
            NO_CAP_HEIGHT_NO_HX_RESOURCE
        );
        let no_x_height_no_hx_typeface = typeface_for(stream, &FontArguments::new());

        let x_height_font = Font::from_size(no_x_height_typeface.clone(), 12.0);
        let x_height_font_no_hx = Font::from_size(no_x_height_no_hx_typeface.clone(), 12.0);

        let (_, metrics) = x_height_font.metrics();
        let x_char_height: f32 = 7.0;
        reporter_assert!(
            reporter,
            metrics.x_height == x_char_height,
            "Expected: {} vs actual: {}",
            x_char_height,
            metrics.x_height
        );

        let (_, metrics) = x_height_font_no_hx.metrics();
        let glyph_id = no_x_height_no_hx_typeface.unichar_to_glyph('x' as Unichar);
        reporter_assert!(
            reporter,
            glyph_id == 0,
            "Glyph lookup for x should fail, but was: {}",
            glyph_id
        );

        // xHeight falls back to ascent as well.
        let expected: f32 = 11.138_672;
        reporter_assert!(
            reporter,
            metrics.x_height == expected,
            "Metrics mismatch: {} vs. {}",
            expected,
            metrics.x_height
        );
    }
);

// Port of: tests/FontationsTest.cpp#L275-L302 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp)] // exact comparisons, as the C++ test makes them
    Fontations_SyntheticCapHeight,
    |reporter| {
        let stream = skip_missing_resource!(
            resource_stream(NO_CAP_HEIGHT_RESOURCE),
            NO_CAP_HEIGHT_RESOURCE
        );
        let no_cap_height_typeface = typeface_for(stream, &FontArguments::new());
        let stream = skip_missing_resource!(
            resource_stream(NO_CAP_HEIGHT_NO_HX_RESOURCE),
            NO_CAP_HEIGHT_NO_HX_RESOURCE
        );
        let no_cap_height_no_hx_typeface = typeface_for(stream, &FontArguments::new());

        let cap_height_font = Font::from_size(no_cap_height_typeface.clone(), 12.0);
        let cap_height_font_no_hx = Font::from_size(no_cap_height_no_hx_typeface.clone(), 12.0);

        let (_, metrics) = cap_height_font.metrics();
        let h_char_height: f32 = 9.0;
        reporter_assert!(reporter, metrics.cap_height == h_char_height);

        let (_, metrics) = cap_height_font_no_hx.metrics();
        let glyph_id = no_cap_height_no_hx_typeface.unichar_to_glyph('H' as Unichar);
        reporter_assert!(
            reporter,
            glyph_id == 0,
            "Glyph lookup for H should fail, but was: {}",
            glyph_id
        );

        let expected: f32 = 11.138_672;
        reporter_assert!(
            reporter,
            metrics.cap_height == expected,
            "Metrics mismatch: {} vs. {}",
            expected,
            metrics.cap_height
        );
    }
);
