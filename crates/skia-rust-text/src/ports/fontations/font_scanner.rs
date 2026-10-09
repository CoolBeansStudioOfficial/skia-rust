// Copyright 2024 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file.
// Port of: src/ports/SkFontScanner_fontations.cpp#L1-L216 (chrome/m156)

//! The Fontations font scanner (`SkFontScanner_Fontations`).

use std::sync::Arc;

use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_arguments::variation_position::Coordinate;
use skia_rust_core::font_descriptor::FactoryId;
use skia_rust_core::font_parameters::variation::Axis;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::stream::StreamAsset;
use skia_rust_core::typeface::Typeface;

use crate::font_scanner::{FontScanner, InstanceInfo, read_all};

use super::base::{
    coordinates_for_shifted_named_instance_index, font_or_collection, font_ref_is_valid,
    get_font_style, make_font_ref, num_axes, num_named_instances, populate_axes,
    resolve_into_normalized_coords,
};
use super::names;
use super::typeface::{FACTORY_ID, font_style_from_bridge, make_from_stream};

/// `SkFontScanner_Fontations`: scans font data with read-fonts.
// Port of: src/ports/SkFontScanner_fontations_priv.h#L14-L37 (chrome/m156)
#[derive(Debug, Default)]
pub struct FontScannerFontations;

impl FontScanner for FontScannerFontations {
    /// `SkFontScanner_Fontations::scanFile`: one face for a single font, one per face in a
    /// collection.
    // Port of: src/ports/SkFontScanner_fontations.cpp#L38-L50 (chrome/m156)
    fn scan_file(&self, stream: &mut dyn StreamAsset) -> Option<i32> {
        let data = read_all(stream);
        let num_fonts = font_or_collection(data.as_bytes())?;
        // A single font is one face. A collection has far fewer than 2^31 fonts.
        #[allow(clippy::cast_possible_wrap)]
        Some(if num_fonts == 0 { 1 } else { num_fonts as i32 })
    }

    /// `SkFontScanner_Fontations::scanFace`: the named instances of the face.
    // Port of: src/ports/SkFontScanner_fontations.cpp#L52-L64 (chrome/m156)
    fn scan_face(&self, stream: &mut dyn StreamAsset, face_index: i32) -> Option<i32> {
        let data = read_all(stream);
        #[allow(clippy::cast_sign_loss)] // the face index is a uint32 in C++
        let font_ref = make_font_ref(data.as_bytes(), face_index as u32);
        if !font_ref_is_valid(&font_ref) {
            return None;
        }
        Some(i32::try_from(num_named_instances(&font_ref)).unwrap_or(i32::MAX))
    }

    /// `SkFontScanner_Fontations::scanInstance`. The face index and the instance index are
    /// packed into one collection index, as `FreeType` does (`face + (instance << 16)`).
    // Port of: src/ports/SkFontScanner_fontations.cpp#L66-L173 (chrome/m156)
    fn scan_instance(
        &self,
        stream: &mut dyn StreamAsset,
        face_index: i32,
        instance_index: i32,
    ) -> Option<InstanceInfo> {
        let data = read_all(stream);
        // C++ computes `faceIndex + (instanceIndex << 16)` as an int, then passes it as uint32.
        #[allow(clippy::cast_sign_loss)] // the packed index is a uint32 in C++
        let packed = face_index.wrapping_add(instance_index.wrapping_shl(16)) as u32;
        let font_ref = make_font_ref(data.as_bytes(), packed);
        if !font_ref_is_valid(&font_ref) {
            return None;
        }

        let name = names::family_name(&font_ref);
        let is_fixed_pitch = false; // TODO in C++: not computed by the Fontations scanner.

        // The axis values of the named instance `instance_index`, or `None` when they cannot be
        // read (C++ then leaves the position empty).
        #[allow(clippy::cast_sign_loss)] // the shifted index is a uint32 in C++
        let shifted = instance_index.wrapping_shl(16) as u32;
        let instance_position = || -> Option<Vec<Coordinate>> {
            let count = usize::try_from(coordinates_for_shifted_named_instance_index(
                &font_ref,
                shifted,
                &mut [],
            ))
            .unwrap_or(0);
            let mut coordinates = vec![Coordinate::default(); count];
            let retrieved =
                coordinates_for_shifted_named_instance_index(&font_ref, shifted, &mut coordinates);
            (usize::try_from(retrieved).ok() == Some(count)).then_some(coordinates)
        };

        // The style: the default instance (0) at the default location, else the instance's.
        // An index past the last instance has no style.
        let instance_count = i32::try_from(num_named_instances(&font_ref)).unwrap_or(i32::MAX);
        if instance_index > instance_count {
            return None;
        }
        let style_at = |coordinates: &[Coordinate]| {
            let normalized = resolve_into_normalized_coords(&font_ref, coordinates);
            get_font_style(&font_ref, &normalized)
                .map_or_else(FontStyle::default, font_style_from_bridge)
        };
        let style = if instance_index == 0 {
            style_at(&[])
        } else {
            instance_position().map_or_else(FontStyle::default, |position| style_at(&position))
        };

        // The axes, with the hidden flag left clear as C++ does.
        let axis_count = num_axes(&font_ref);
        let mut axes = vec![Axis::default(); axis_count];
        if populate_axes(&font_ref, &mut axes) >= 0 {
            for axis in &mut axes {
                *axis = Axis::new(axis.tag, axis.min, axis.def, axis.max, false);
            }
        }

        let position = instance_position().unwrap_or_default();

        Some(InstanceInfo {
            name,
            style,
            is_fixed_pitch,
            axes,
            position,
        })
    }

    /// `SkFontScanner_Fontations::MakeFromStream`.
    // Port of: src/ports/SkFontScanner_fontations.cpp#L209-L211 (chrome/m156)
    fn make_from_stream(
        &self,
        stream: Box<dyn StreamAsset>,
        args: &FontArguments<'_, '_>,
    ) -> Option<Typeface> {
        make_from_stream(stream, args)
    }

    /// `SkFontScanner_Fontations::getFactoryId`.
    // Port of: src/ports/SkFontScanner_fontations.cpp#L213-L215 (chrome/m156)
    fn factory_id(&self) -> FactoryId {
        FACTORY_ID
    }
}

/// `SkFontScanner_Make_Fontations()`: a Fontations scanner.
// Port of: src/ports/SkFontScanner_fontations.cpp#L212-L214 (chrome/m156)
#[must_use]
pub fn new_fontations() -> Arc<dyn FontScanner> {
    Arc::new(FontScannerFontations)
}
