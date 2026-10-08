// Copyright 2024 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file.
// Ported from Skia: include/core/SkFontScanner.h (chrome/m156)

//! [`FontScanner`]: reads the faces, instances and axes of font data without making typefaces
//! of all of them (`SkFontScanner`).

use std::fmt;

use skia_rust_core::font_arguments::FontArguments;
use skia_rust_core::font_arguments::variation_position::Coordinate;
use skia_rust_core::font_descriptor::FactoryId;
use skia_rust_core::font_parameters::variation::Axis;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::stream::StreamAsset;
use skia_rust_core::typeface::Typeface;

/// What [`FontScanner::scan_instance`] reads about one instance of a face.
// Port of: include/core/SkFontScanner.h#L49-L70 (chrome/m156), the outputs of scanInstance
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InstanceInfo {
    /// The family name of the face (`SkString* name`).
    pub name: String,
    /// The style of the instance (`SkFontStyle* style`).
    pub style: FontStyle,
    /// Whether the face is fixed pitch (`bool* isFixedPitch`).
    pub is_fixed_pitch: bool,
    /// The variation axes of the face (`AxisDefinitions* axes`).
    pub axes: Vec<Axis>,
    /// The axis values of the instance (`VariationPosition* position`).
    pub position: Vec<Coordinate>,
}

/// A scanner for one kind of font data (`SkFontScanner`): it tells how many faces and instances
/// the data has, and their names, styles and axes.
///
/// The stream arguments are rewound before and after each call, as in C++.
// Port of: include/core/SkFontScanner.h#L24-L77 (chrome/m156)
#[doc(alias = "SkFontScanner")]
pub trait FontScanner: Send + Sync + fmt::Debug {
    /// `scanFile`: the number of faces in the data, or `None` if it is not a font.
    // Port of: include/core/SkFontScanner.h#L34 (chrome/m156)
    #[doc(alias = "scanFile")]
    fn scan_file(&self, stream: &mut dyn StreamAsset) -> Option<i32>;

    /// `scanFace`: the number of named instances of face `face_index`, or `None` if the face
    /// cannot be read.
    // Port of: include/core/SkFontScanner.h#L36 (chrome/m156)
    #[doc(alias = "scanFace")]
    fn scan_face(&self, stream: &mut dyn StreamAsset, face_index: i32) -> Option<i32>;

    /// `scanInstance`: what face `face_index` says about instance `instance_index` (0 is the
    /// default instance). `None` if the face or the instance cannot be read.
    // Port of: include/core/SkFontScanner.h#L38-L45 (chrome/m156)
    #[doc(alias = "scanInstance")]
    fn scan_instance(
        &self,
        stream: &mut dyn StreamAsset,
        face_index: i32,
        instance_index: i32,
    ) -> Option<InstanceInfo>;

    /// `makeFromStream`: a typeface of the stream, with the arguments.
    // Port of: include/core/SkFontScanner.h#L47 (chrome/m156)
    #[doc(alias = "makeFromStream")]
    fn make_from_stream(
        &self,
        stream: Box<dyn StreamAsset>,
        args: &FontArguments<'_, '_>,
    ) -> Option<Typeface>;

    /// `getFactoryId`: the id that the typefaces of this scanner carry in their descriptors.
    // Port of: include/core/SkFontScanner.h#L51 (chrome/m156)
    #[doc(alias = "getFactoryId")]
    fn factory_id(&self) -> FactoryId;
}

/// Reads the rest of a stream into memory, for a scanner that works on bytes. Keeps the
/// stream's position where C++ does: it rewinds before and after.
pub(crate) fn read_all(stream: &mut dyn StreamAsset) -> skia_rust_core::data::Data {
    stream.rewind();
    let length = stream.get_length();
    let data = skia_rust_core::data::Data::from_stream(stream, length).unwrap_or_default();
    stream.rewind();
    data
}
