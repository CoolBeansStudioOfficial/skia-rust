// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/HdrMetadataTest.cpp (chrome/m156), the parse and serialize cases of the content
// light level and mastering display colour volume metadata. The ColorVolumeTransform (AGTM) cases
// are not ported (see the manifest reasons).
#![cfg(test)]

use skia_rust_core::color_space::ColorSpacePrimaries;
use skia_rust_core::data::Data;
use skia_rust_core::hdr_metadata::{ContentLightLevelInformation, MasteringDisplayColorVolume};

use crate::{def_test, reporter_assert};

// Port of: tests/HdrMetadataTest.cpp#L20-L47 (chrome/m156), HdrMetadata_ParseSerialize_ContentLightLevelInformation
def_test!(
    HdrMetadata_ParseSerialize_ContentLightLevelInformation,
    |r| {
        let data = Data::new_copy(&[0x03, 0xE8, 0x00, 0xFA]);
        // Data taken from:
        // https://www.w3.org/TR/png-3/#example-13
        // https://www.w3.org/TR/png-3/#example-14
        let data_png = Data::new_copy(&[0x00, 0x98, 0x96, 0x80, 0x00, 0x26, 0x25, 0xA0]);
        let clli_expected = ContentLightLevelInformation {
            max_cll: 1000.0,
            max_fall: 250.0,
        };

        let clli = ContentLightLevelInformation::parse(&data);
        reporter_assert!(r, clli == Some(clli_expected));
        reporter_assert!(
            r,
            clli.is_some_and(|clli| data.equals(Some(&clli.serialize())))
        );

        let clli_png = ContentLightLevelInformation::parse_png_chunk(&data_png);
        reporter_assert!(r, clli_png == Some(clli_expected));
        reporter_assert!(
            r,
            clli.is_some_and(|clli| data_png.equals(Some(&clli.serialize_png_chunk())))
        );
    }
);

// Port of: tests/HdrMetadataTest.cpp#L49-L73 (chrome/m156), HdrMetadata_ParseSerialize_MasteringDisplayColorVolume
def_test!(
    HdrMetadata_ParseSerialize_MasteringDisplayColorVolume,
    |r| {
        // Data taken from:
        // https://www.w3.org/TR/png-3/#example-5
        // https://www.w3.org/TR/png-3/#example-6
        // https://www.w3.org/TR/png-3/#example-7
        // https://www.w3.org/TR/png-3/#example-8
        let data = Data::new_copy(&[
            0x8A, 0x48, 0x39, 0x08, // Red
            0x21, 0x34, 0x9B, 0xAA, // Green
            0x19, 0x96, 0x08, 0xFC, // Blue
            0x3D, 0x13, 0x40, 0x42, // White
            0x02, 0x62, 0x5A, 0x00, // Maximum luminance
            0x00, 0x00, 0x00, 0x05, // Minimum luminance
        ]);
        let mdcv_expected = MasteringDisplayColorVolume {
            display_primaries: ColorSpacePrimaries {
                rx: 0.708,
                ry: 0.292,
                gx: 0.17,
                gy: 0.797,
                bx: 0.131,
                by: 0.046,
                wx: 0.3127,
                wy: 0.329,
            },
            maximum_display_mastering_luminance: 4000.0,
            minimum_display_mastering_luminance: 0.0005,
        };

        let mdcv = MasteringDisplayColorVolume::parse(&data);
        reporter_assert!(r, mdcv.as_ref() == Some(&mdcv_expected));
        reporter_assert!(
            r,
            mdcv.is_some_and(|mdcv| data.equals(Some(&mdcv.serialize())))
        );
    }
);
