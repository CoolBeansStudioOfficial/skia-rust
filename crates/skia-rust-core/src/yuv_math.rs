// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkYUVMath.cpp (the tables and `SkColorMatrix_RGB2YUV` /
// `SkColorMatrix_YUV2RGB`).
//
// The tables are the default branch of SkYUVMath.cpp (without SK_YUV_COLOR_SPACE_HIGH_PRECISION).
// Each literal is the same decimal text as the C++ (`0.299000f` is `0.299000_f32`), which both
// compilers round to the same nearest f32.

//! The YUV <-> RGB colour matrices for each [`YUVColorSpace`] (`SkColorMatrix_RGB2YUV` and
//! `SkColorMatrix_YUV2RGB`), row-major 4x5 like [`ColorMatrix`](crate::color_matrix::ColorMatrix).

use crate::image_info::YUVColorSpace;

// Port of: src/core/SkYUVMath.cpp#L749-L776 (the `static_assert`s on the colour space order)
const _: () = {
    assert!(YUVColorSpace::JPEGFull as i32 == 0);
    assert!(YUVColorSpace::Rec601Limited as i32 == 1);
    assert!(YUVColorSpace::Rec709Full as i32 == 2);
    assert!(YUVColorSpace::Rec709Limited as i32 == 3);
    assert!(YUVColorSpace::BT2020_8BitFull as i32 == 4);
    assert!(YUVColorSpace::BT2020_8BitLimited as i32 == 5);
    assert!(YUVColorSpace::BT2020_10BitFull as i32 == 6);
    assert!(YUVColorSpace::BT2020_10BitLimited as i32 == 7);
    assert!(YUVColorSpace::BT2020_12BitFull as i32 == 8);
    assert!(YUVColorSpace::BT2020_12BitLimited as i32 == 9);
    assert!(YUVColorSpace::BT2020_16BitFull as i32 == 10);
    assert!(YUVColorSpace::BT2020_16BitLimited as i32 == 11);
    assert!(YUVColorSpace::FCCFull as i32 == 12);
    assert!(YUVColorSpace::FCCLimited as i32 == 13);
    assert!(YUVColorSpace::SMPTE240Full as i32 == 14);
    assert!(YUVColorSpace::SMPTE240Limited as i32 == 15);
    assert!(YUVColorSpace::YDZDXFull as i32 == 16);
    assert!(YUVColorSpace::YDZDXLimited as i32 == 17);
    assert!(YUVColorSpace::GBRFull as i32 == 18);
    assert!(YUVColorSpace::GBRLimited as i32 == 19);
    assert!(YUVColorSpace::YCgCo_8BitFull as i32 == 20);
    assert!(YUVColorSpace::YCgCo_8BitLimited as i32 == 21);
    assert!(YUVColorSpace::YCgCo_10BitFull as i32 == 22);
    assert!(YUVColorSpace::YCgCo_10BitLimited as i32 == 23);
    assert!(YUVColorSpace::YCgCo_12BitFull as i32 == 24);
    assert!(YUVColorSpace::YCgCo_12BitLimited as i32 == 25);
    assert!(YUVColorSpace::YCgCo_16BitFull as i32 == 26);
    assert!(YUVColorSpace::YCgCo_16BitLimited as i32 == 27);
};

// Port of: src/core/SkYUVMath.cpp#L20-L31 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const JPEG_FULL_RGB_TO_YUV: [f32; 20] = [
    0.299000_f32,
    0.587000_f32,
    0.114000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.168736_f32,
    -0.331264_f32,
    0.500000_f32,
    0.000000_f32,
    0.501961_f32,
    0.500000_f32,
    -0.418688_f32,
    -0.081312_f32,
    0.000000_f32,
    0.501961_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L33-L44 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const JPEG_FULL_YUV_TO_RGB: [f32; 20] = [
    1.000000_f32,
    -0.000000_f32,
    1.402000_f32,
    0.000000_f32,
    -0.703749_f32,
    1.000000_f32,
    -0.344136_f32,
    -0.714136_f32,
    0.000000_f32,
    0.531211_f32,
    1.000000_f32,
    1.772000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.889475_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L46-L57 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const REC601_LIMITED_RGB_TO_YUV: [f32; 20] = [
    0.256788_f32,
    0.504129_f32,
    0.097906_f32,
    0.000000_f32,
    0.062745_f32,
    -0.148223_f32,
    -0.290993_f32,
    0.439216_f32,
    0.000000_f32,
    0.501961_f32,
    0.439216_f32,
    -0.367788_f32,
    -0.071427_f32,
    0.000000_f32,
    0.501961_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L59-L70 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const REC601_LIMITED_YUV_TO_RGB: [f32; 20] = [
    1.164384_f32,
    -0.000000_f32,
    1.596027_f32,
    0.000000_f32,
    -0.874202_f32,
    1.164384_f32,
    -0.391762_f32,
    -0.812968_f32,
    0.000000_f32,
    0.531668_f32,
    1.164384_f32,
    2.017232_f32,
    0.000000_f32,
    0.000000_f32,
    -1.085631_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L72-L83 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const REC709_FULL_RGB_TO_YUV: [f32; 20] = [
    0.212600_f32,
    0.715200_f32,
    0.072200_f32,
    0.000000_f32,
    0.000000_f32,
    -0.114572_f32,
    -0.385428_f32,
    0.500000_f32,
    0.000000_f32,
    0.501961_f32,
    0.500000_f32,
    -0.454153_f32,
    -0.045847_f32,
    0.000000_f32,
    0.501961_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L85-L96 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const REC709_FULL_YUV_TO_RGB: [f32; 20] = [
    1.000000_f32,
    -0.000000_f32,
    1.574800_f32,
    0.000000_f32,
    -0.790488_f32,
    1.000000_f32,
    -0.187324_f32,
    -0.468124_f32,
    0.000000_f32,
    0.329010_f32,
    1.000000_f32,
    1.855600_f32,
    -0.000000_f32,
    0.000000_f32,
    -0.931439_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L98-L109 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const REC709_LIMITED_RGB_TO_YUV: [f32; 20] = [
    0.182586_f32,
    0.614231_f32,
    0.062007_f32,
    0.000000_f32,
    0.062745_f32,
    -0.100644_f32,
    -0.338572_f32,
    0.439216_f32,
    0.000000_f32,
    0.501961_f32,
    0.439216_f32,
    -0.398942_f32,
    -0.040274_f32,
    0.000000_f32,
    0.501961_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L111-L122 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const REC709_LIMITED_YUV_TO_RGB: [f32; 20] = [
    1.164384_f32,
    -0.000000_f32,
    1.792741_f32,
    0.000000_f32,
    -0.972945_f32,
    1.164384_f32,
    -0.213249_f32,
    -0.532909_f32,
    0.000000_f32,
    0.301483_f32,
    1.164384_f32,
    2.112402_f32,
    -0.000000_f32,
    0.000000_f32,
    -1.133402_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L124-L135 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const BT2020_8BIT_FULL_RGB_TO_YUV: [f32; 20] = [
    0.262700_f32,
    0.678000_f32,
    0.059300_f32,
    0.000000_f32,
    0.000000_f32,
    -0.139630_f32,
    -0.360370_f32,
    0.500000_f32,
    0.000000_f32,
    0.501961_f32,
    0.500000_f32,
    -0.459786_f32,
    -0.040214_f32,
    0.000000_f32,
    0.501961_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L137-L148 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const BT2020_8BIT_FULL_YUV_TO_RGB: [f32; 20] = [
    1.000000_f32,
    -0.000000_f32,
    1.474600_f32,
    0.000000_f32,
    -0.740191_f32,
    1.000000_f32,
    -0.164553_f32,
    -0.571353_f32,
    0.000000_f32,
    0.369396_f32,
    1.000000_f32,
    1.881400_f32,
    -0.000000_f32,
    0.000000_f32,
    -0.944389_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L150-L161 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const BT2020_8BIT_LIMITED_RGB_TO_YUV: [f32; 20] = [
    0.225613_f32,
    0.582282_f32,
    0.050928_f32,
    0.000000_f32,
    0.062745_f32,
    -0.122655_f32,
    -0.316560_f32,
    0.439216_f32,
    0.000000_f32,
    0.501961_f32,
    0.439216_f32,
    -0.403890_f32,
    -0.035326_f32,
    0.000000_f32,
    0.501961_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L163-L174 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const BT2020_8BIT_LIMITED_YUV_TO_RGB: [f32; 20] = [
    1.164384_f32,
    -0.000000_f32,
    1.678674_f32,
    0.000000_f32,
    -0.915688_f32,
    1.164384_f32,
    -0.187326_f32,
    -0.650424_f32,
    0.000000_f32,
    0.347458_f32,
    1.164384_f32,
    2.141772_f32,
    -0.000000_f32,
    0.000000_f32,
    -1.148145_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L176-L187 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const BT2020_10BIT_FULL_RGB_TO_YUV: [f32; 20] = [
    0.262700_f32,
    0.678000_f32,
    0.059300_f32,
    0.000000_f32,
    0.000000_f32,
    -0.139630_f32,
    -0.360370_f32,
    0.500000_f32,
    0.000000_f32,
    0.500489_f32,
    0.500000_f32,
    -0.459786_f32,
    -0.040214_f32,
    0.000000_f32,
    0.500489_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L189-L200 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const BT2020_10BIT_FULL_YUV_TO_RGB: [f32; 20] = [
    1.000000_f32,
    -0.000000_f32,
    1.474600_f32,
    0.000000_f32,
    -0.738021_f32,
    1.000000_f32,
    -0.164553_f32,
    -0.571353_f32,
    0.000000_f32,
    0.368313_f32,
    1.000000_f32,
    1.881400_f32,
    -0.000000_f32,
    0.000000_f32,
    -0.941620_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L202-L213 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const BT2020_10BIT_LIMITED_RGB_TO_YUV: [f32; 20] = [
    0.224951_f32,
    0.580575_f32,
    0.050779_f32,
    0.000000_f32,
    0.062561_f32,
    -0.122296_f32,
    -0.315632_f32,
    0.437928_f32,
    0.000000_f32,
    0.500489_f32,
    0.437928_f32,
    -0.402706_f32,
    -0.035222_f32,
    0.000000_f32,
    0.500489_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L215-L226 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const BT2020_10BIT_LIMITED_YUV_TO_RGB: [f32; 20] = [
    1.167808_f32,
    -0.000000_f32,
    1.683611_f32,
    0.000000_f32,
    -0.915688_f32,
    1.167808_f32,
    -0.187877_f32,
    -0.652337_f32,
    0.000000_f32,
    0.347458_f32,
    1.167808_f32,
    2.148072_f32,
    -0.000000_f32,
    0.000000_f32,
    -1.148145_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L228-L239 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const BT2020_12BIT_FULL_RGB_TO_YUV: [f32; 20] = [
    0.262700_f32,
    0.678000_f32,
    0.059300_f32,
    0.000000_f32,
    0.000000_f32,
    -0.139630_f32,
    -0.360370_f32,
    0.500000_f32,
    0.000000_f32,
    0.500122_f32,
    0.500000_f32,
    -0.459786_f32,
    -0.040214_f32,
    0.000000_f32,
    0.500122_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L241-L252 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const BT2020_12BIT_FULL_YUV_TO_RGB: [f32; 20] = [
    1.000000_f32,
    -0.000000_f32,
    1.474600_f32,
    0.000000_f32,
    -0.737480_f32,
    1.000000_f32,
    -0.164553_f32,
    -0.571353_f32,
    0.000000_f32,
    0.368043_f32,
    1.000000_f32,
    1.881400_f32,
    -0.000000_f32,
    0.000000_f32,
    -0.940930_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L254-L265 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const BT2020_12BIT_LIMITED_RGB_TO_YUV: [f32; 20] = [
    0.224787_f32,
    0.580149_f32,
    0.050742_f32,
    0.000000_f32,
    0.062515_f32,
    -0.122206_f32,
    -0.315401_f32,
    0.437607_f32,
    0.000000_f32,
    0.500122_f32,
    0.437607_f32,
    -0.402411_f32,
    -0.035196_f32,
    0.000000_f32,
    0.500122_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L267-L278 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const BT2020_12BIT_LIMITED_YUV_TO_RGB: [f32; 20] = [
    1.168664_f32,
    -0.000000_f32,
    1.684846_f32,
    0.000000_f32,
    -0.915688_f32,
    1.168664_f32,
    -0.188015_f32,
    -0.652816_f32,
    0.000000_f32,
    0.347458_f32,
    1.168664_f32,
    2.149647_f32,
    -0.000000_f32,
    0.000000_f32,
    -1.148145_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L280-L291 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const BT2020_16BIT_FULL_RGB_TO_YUV: [f32; 20] = [
    0.262700_f32,
    0.678000_f32,
    0.059300_f32,
    0.000000_f32,
    0.000000_f32,
    -0.139630_f32,
    -0.360370_f32,
    0.500000_f32,
    0.000000_f32,
    0.500008_f32,
    0.500000_f32,
    -0.459786_f32,
    -0.040214_f32,
    0.000000_f32,
    0.500008_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L293-L304 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const BT2020_16BIT_FULL_YUV_TO_RGB: [f32; 20] = [
    1.000000_f32,
    -0.000000_f32,
    1.474600_f32,
    0.000000_f32,
    -0.737311_f32,
    1.000000_f32,
    -0.164553_f32,
    -0.571353_f32,
    0.000000_f32,
    0.367959_f32,
    1.000000_f32,
    1.881400_f32,
    -0.000000_f32,
    0.000000_f32,
    -0.940714_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L306-L317 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const BT2020_16BIT_LIMITED_RGB_TO_YUV: [f32; 20] = [
    0.224735_f32,
    0.580017_f32,
    0.050730_f32,
    0.000000_f32,
    0.062501_f32,
    -0.122178_f32,
    -0.315329_f32,
    0.437507_f32,
    0.000000_f32,
    0.500008_f32,
    0.437507_f32,
    -0.402319_f32,
    -0.035188_f32,
    0.000000_f32,
    0.500008_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L319-L330 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const BT2020_16BIT_LIMITED_YUV_TO_RGB: [f32; 20] = [
    1.168932_f32,
    0.000000_f32,
    1.685231_f32,
    0.000000_f32,
    -0.915688_f32,
    1.168932_f32,
    -0.188058_f32,
    -0.652965_f32,
    0.000000_f32,
    0.347458_f32,
    1.168932_f32,
    2.150139_f32,
    -0.000000_f32,
    0.000000_f32,
    -1.148145_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L332-L343 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const FCC_FULL_RGB_TO_YUV: [f32; 20] = [
    0.300000_f32,
    0.590000_f32,
    0.110000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.168539_f32,
    -0.331461_f32,
    0.500000_f32,
    0.000000_f32,
    0.501961_f32,
    0.500000_f32,
    -0.421429_f32,
    -0.078571_f32,
    0.000000_f32,
    0.501961_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L345-L356 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const FCC_FULL_YUV_TO_RGB: [f32; 20] = [
    1.000000_f32,
    0.000000_f32,
    1.400000_f32,
    0.000000_f32,
    -0.702745_f32,
    1.000000_f32,
    -0.331864_f32,
    -0.711864_f32,
    0.000000_f32,
    0.523911_f32,
    1.000000_f32,
    1.780000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.893490_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L358-L369 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const FCC_LIMITED_RGB_TO_YUV: [f32; 20] = [
    0.257647_f32,
    0.506706_f32,
    0.094471_f32,
    0.000000_f32,
    0.062745_f32,
    -0.148050_f32,
    -0.291165_f32,
    0.439216_f32,
    0.000000_f32,
    0.501961_f32,
    0.439216_f32,
    -0.370196_f32,
    -0.069020_f32,
    0.000000_f32,
    0.501961_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L371-L382 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const FCC_LIMITED_YUV_TO_RGB: [f32; 20] = [
    1.164384_f32,
    -0.000000_f32,
    1.593750_f32,
    0.000000_f32,
    -0.873059_f32,
    1.164384_f32,
    -0.377792_f32,
    -0.810381_f32,
    0.000000_f32,
    0.523357_f32,
    1.164384_f32,
    2.026339_f32,
    0.000000_f32,
    0.000000_f32,
    -1.090202_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L384-L395 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const SMPTE240_FULL_RGB_TO_YUV: [f32; 20] = [
    0.212000_f32,
    0.701000_f32,
    0.087000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.116101_f32,
    -0.383899_f32,
    0.500000_f32,
    0.000000_f32,
    0.501961_f32,
    0.500000_f32,
    -0.444797_f32,
    -0.055203_f32,
    0.000000_f32,
    0.501961_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L397-L408 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const SMPTE240_FULL_YUV_TO_RGB: [f32; 20] = [
    1.000000_f32,
    0.000000_f32,
    1.576000_f32,
    0.000000_f32,
    -0.791090_f32,
    1.000000_f32,
    -0.226622_f32,
    -0.476622_f32,
    0.000000_f32,
    0.353001_f32,
    1.000000_f32,
    1.826000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.916580_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L410-L421 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const SMPTE240_LIMITED_RGB_TO_YUV: [f32; 20] = [
    0.182071_f32,
    0.602035_f32,
    0.074718_f32,
    0.000000_f32,
    0.062745_f32,
    -0.101987_f32,
    -0.337229_f32,
    0.439216_f32,
    0.000000_f32,
    0.501961_f32,
    0.439216_f32,
    -0.390724_f32,
    -0.048492_f32,
    0.000000_f32,
    0.501961_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L423-L434 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const SMPTE240_LIMITED_YUV_TO_RGB: [f32; 20] = [
    1.164384_f32,
    -0.000000_f32,
    1.794107_f32,
    0.000000_f32,
    -0.973631_f32,
    1.164384_f32,
    -0.257985_f32,
    -0.542583_f32,
    0.000000_f32,
    0.328794_f32,
    1.164384_f32,
    2.078705_f32,
    0.000000_f32,
    0.000000_f32,
    -1.116488_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L436-L447 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YDZDX_FULL_RGB_TO_YUV: [f32; 20] = [
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.500000_f32,
    0.493283_f32,
    0.000000_f32,
    0.501961_f32,
    0.500000_f32,
    -0.495951_f32,
    0.000000_f32,
    0.000000_f32,
    0.501961_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L449-L460 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YDZDX_FULL_YUV_TO_RGB: [f32; 20] = [
    0.991902_f32,
    -0.000000_f32,
    2.000000_f32,
    0.000000_f32,
    -1.003922_f32,
    1.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.013617_f32,
    2.027234_f32,
    0.000000_f32,
    0.000000_f32,
    -1.017592_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L462-L473 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YDZDX_LIMITED_RGB_TO_YUV: [f32; 20] = [
    0.000000_f32,
    0.858824_f32,
    0.000000_f32,
    0.000000_f32,
    0.062745_f32,
    0.000000_f32,
    -0.439216_f32,
    0.433315_f32,
    0.000000_f32,
    0.501961_f32,
    0.439216_f32,
    -0.435659_f32,
    0.000000_f32,
    0.000000_f32,
    0.501961_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L475-L486 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YDZDX_LIMITED_YUV_TO_RGB: [f32; 20] = [
    1.154954_f32,
    -0.000000_f32,
    2.276786_f32,
    0.000000_f32,
    -1.215325_f32,
    1.164384_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.073059_f32,
    1.180239_f32,
    2.307788_f32,
    0.000000_f32,
    0.000000_f32,
    -1.232474_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L488-L499 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const GBR_FULL_RGB_TO_YUV: [f32; 20] = [
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L501-L512 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const GBR_FULL_YUV_TO_RGB: [f32; 20] = [
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L514-L525 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const GBR_LIMITED_RGB_TO_YUV: [f32; 20] = [
    0.000000_f32,
    0.858824_f32,
    0.000000_f32,
    0.000000_f32,
    0.062745_f32,
    0.000000_f32,
    0.000000_f32,
    0.858824_f32,
    0.000000_f32,
    0.062745_f32,
    0.858824_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    0.062745_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L527-L538 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const GBR_LIMITED_YUV_TO_RGB: [f32; 20] = [
    0.000000_f32,
    0.000000_f32,
    1.164384_f32,
    0.000000_f32,
    -0.073059_f32,
    1.164384_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.073059_f32,
    0.000000_f32,
    1.164384_f32,
    0.000000_f32,
    0.000000_f32,
    -0.073059_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L540-L551 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YCGCO_8BIT_FULL_RGB_TO_YUV: [f32; 20] = [
    0.250000_f32,
    0.500000_f32,
    0.250000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.250000_f32,
    0.500000_f32,
    -0.250000_f32,
    0.000000_f32,
    0.501961_f32,
    0.500000_f32,
    0.000000_f32,
    -0.500000_f32,
    0.000000_f32,
    0.501961_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L553-L564 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YCGCO_8BIT_FULL_YUV_TO_RGB: [f32; 20] = [
    1.000000_f32,
    -1.000000_f32,
    1.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    1.000000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.501961_f32,
    1.000000_f32,
    -1.000000_f32,
    -1.000000_f32,
    0.000000_f32,
    1.003922_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L566-L577 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YCGCO_8BIT_LIMITED_RGB_TO_YUV: [f32; 20] = [
    0.214706_f32,
    0.429412_f32,
    0.214706_f32,
    0.000000_f32,
    0.062745_f32,
    -0.214706_f32,
    0.429412_f32,
    -0.214706_f32,
    0.000000_f32,
    0.501961_f32,
    0.429412_f32,
    0.000000_f32,
    -0.429412_f32,
    0.000000_f32,
    0.501961_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L579-L590 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YCGCO_8BIT_LIMITED_YUV_TO_RGB: [f32; 20] = [
    1.164384_f32,
    -1.164384_f32,
    1.164384_f32,
    0.000000_f32,
    -0.073059_f32,
    1.164384_f32,
    1.164384_f32,
    0.000000_f32,
    0.000000_f32,
    -0.657534_f32,
    1.164384_f32,
    -1.164384_f32,
    -1.164384_f32,
    0.000000_f32,
    1.095891_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L592-L603 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YCGCO_10BIT_FULL_RGB_TO_YUV: [f32; 20] = [
    0.250000_f32,
    0.500000_f32,
    0.250000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.250000_f32,
    0.500000_f32,
    -0.250000_f32,
    0.000000_f32,
    0.500489_f32,
    0.500000_f32,
    0.000000_f32,
    -0.500000_f32,
    0.000000_f32,
    0.500489_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L605-L616 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YCGCO_10BIT_FULL_YUV_TO_RGB: [f32; 20] = [
    1.000000_f32,
    -1.000000_f32,
    1.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    1.000000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.500489_f32,
    1.000000_f32,
    -1.000000_f32,
    -1.000000_f32,
    0.000000_f32,
    1.000978_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L618-L629 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YCGCO_10BIT_LIMITED_RGB_TO_YUV: [f32; 20] = [
    0.214076_f32,
    0.428153_f32,
    0.214076_f32,
    0.000000_f32,
    0.062561_f32,
    -0.214076_f32,
    0.428153_f32,
    -0.214076_f32,
    0.000000_f32,
    0.500489_f32,
    0.428153_f32,
    0.000000_f32,
    -0.428153_f32,
    0.000000_f32,
    0.500489_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L631-L642 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YCGCO_10BIT_LIMITED_YUV_TO_RGB: [f32; 20] = [
    1.167808_f32,
    -1.167808_f32,
    1.167808_f32,
    0.000000_f32,
    -0.073059_f32,
    1.167808_f32,
    1.167808_f32,
    0.000000_f32,
    0.000000_f32,
    -0.657534_f32,
    1.167808_f32,
    -1.167808_f32,
    -1.167808_f32,
    0.000000_f32,
    1.095890_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L644-L655 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YCGCO_12BIT_FULL_RGB_TO_YUV: [f32; 20] = [
    0.250000_f32,
    0.500000_f32,
    0.250000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.250000_f32,
    0.500000_f32,
    -0.250000_f32,
    0.000000_f32,
    0.500122_f32,
    0.500000_f32,
    0.000000_f32,
    -0.500000_f32,
    0.000000_f32,
    0.500122_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L657-L668 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YCGCO_12BIT_FULL_YUV_TO_RGB: [f32; 20] = [
    1.000000_f32,
    -1.000000_f32,
    1.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    1.000000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.500122_f32,
    1.000000_f32,
    -1.000000_f32,
    -1.000000_f32,
    0.000000_f32,
    1.000244_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L670-L681 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YCGCO_12BIT_LIMITED_RGB_TO_YUV: [f32; 20] = [
    0.213919_f32,
    0.427839_f32,
    0.213919_f32,
    0.000000_f32,
    0.062515_f32,
    -0.213919_f32,
    0.427839_f32,
    -0.213919_f32,
    0.000000_f32,
    0.500122_f32,
    0.427839_f32,
    0.000000_f32,
    -0.427839_f32,
    0.000000_f32,
    0.500122_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L683-L694 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YCGCO_12BIT_LIMITED_YUV_TO_RGB: [f32; 20] = [
    1.168664_f32,
    -1.168664_f32,
    1.168664_f32,
    0.000000_f32,
    -0.073059_f32,
    1.168664_f32,
    1.168664_f32,
    0.000000_f32,
    0.000000_f32,
    -0.657534_f32,
    1.168664_f32,
    -1.168664_f32,
    -1.168664_f32,
    0.000000_f32,
    1.095891_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L696-L707 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YCGCO_16BIT_FULL_RGB_TO_YUV: [f32; 20] = [
    0.250000_f32,
    0.500000_f32,
    0.250000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.250000_f32,
    0.500000_f32,
    -0.250000_f32,
    0.000000_f32,
    0.500008_f32,
    0.500000_f32,
    0.000000_f32,
    -0.500000_f32,
    0.000000_f32,
    0.500008_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L709-L720 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YCGCO_16BIT_FULL_YUV_TO_RGB: [f32; 20] = [
    1.000000_f32,
    -1.000000_f32,
    1.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    1.000000_f32,
    0.000000_f32,
    0.000000_f32,
    -0.500008_f32,
    1.000000_f32,
    -1.000000_f32,
    -1.000000_f32,
    0.000000_f32,
    1.000015_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L722-L733 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YCGCO_16BIT_LIMITED_RGB_TO_YUV: [f32; 20] = [
    0.213870_f32,
    0.427741_f32,
    0.213870_f32,
    0.000000_f32,
    0.062501_f32,
    -0.213870_f32,
    0.427741_f32,
    -0.213870_f32,
    0.000000_f32,
    0.500008_f32,
    0.427741_f32,
    0.000000_f32,
    -0.427741_f32,
    0.000000_f32,
    0.500008_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];
// Port of: src/core/SkYUVMath.cpp#L735-L746 (chrome/m156), the default (not SK_YUV_COLOR_SPACE_HIGH_PRECISION) branch
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const YCGCO_16BIT_LIMITED_YUV_TO_RGB: [f32; 20] = [
    1.168932_f32,
    -1.168932_f32,
    1.168932_f32,
    0.000000_f32,
    -0.073059_f32,
    1.168932_f32,
    1.168932_f32,
    0.000000_f32,
    0.000000_f32,
    -0.657534_f32,
    1.168932_f32,
    -1.168932_f32,
    -1.168932_f32,
    0.000000_f32,
    1.095890_f32,
    0.000000_f32,
    0.000000_f32,
    0.000000_f32,
    1.000000_f32,
    0.000000_f32,
];

// Port of: src/core/SkYUVMath.cpp#L778-L807 (chrome/m156), `yuv_to_rgb_array` and `rgb_to_yuv_array`
// (indexed by `SkYUVColorSpace`)
static YUV_TO_RGB_ARRAY: [&[f32; 20]; 28] = [
    &JPEG_FULL_YUV_TO_RGB,
    &REC601_LIMITED_YUV_TO_RGB,
    &REC709_FULL_YUV_TO_RGB,
    &REC709_LIMITED_YUV_TO_RGB,
    &BT2020_8BIT_FULL_YUV_TO_RGB,
    &BT2020_8BIT_LIMITED_YUV_TO_RGB,
    &BT2020_10BIT_FULL_YUV_TO_RGB,
    &BT2020_10BIT_LIMITED_YUV_TO_RGB,
    &BT2020_12BIT_FULL_YUV_TO_RGB,
    &BT2020_12BIT_LIMITED_YUV_TO_RGB,
    &BT2020_16BIT_FULL_YUV_TO_RGB,
    &BT2020_16BIT_LIMITED_YUV_TO_RGB,
    &FCC_FULL_YUV_TO_RGB,
    &FCC_LIMITED_YUV_TO_RGB,
    &SMPTE240_FULL_YUV_TO_RGB,
    &SMPTE240_LIMITED_YUV_TO_RGB,
    &YDZDX_FULL_YUV_TO_RGB,
    &YDZDX_LIMITED_YUV_TO_RGB,
    &GBR_FULL_YUV_TO_RGB,
    &GBR_LIMITED_YUV_TO_RGB,
    &YCGCO_8BIT_FULL_YUV_TO_RGB,
    &YCGCO_8BIT_LIMITED_YUV_TO_RGB,
    &YCGCO_10BIT_FULL_YUV_TO_RGB,
    &YCGCO_10BIT_LIMITED_YUV_TO_RGB,
    &YCGCO_12BIT_FULL_YUV_TO_RGB,
    &YCGCO_12BIT_LIMITED_YUV_TO_RGB,
    &YCGCO_16BIT_FULL_YUV_TO_RGB,
    &YCGCO_16BIT_LIMITED_YUV_TO_RGB,
];

// Port of: src/core/SkYUVMath.cpp#L809-L838 (chrome/m156), `rgb_to_yuv_array`
static RGB_TO_YUV_ARRAY: [&[f32; 20]; 28] = [
    &JPEG_FULL_RGB_TO_YUV,
    &REC601_LIMITED_RGB_TO_YUV,
    &REC709_FULL_RGB_TO_YUV,
    &REC709_LIMITED_RGB_TO_YUV,
    &BT2020_8BIT_FULL_RGB_TO_YUV,
    &BT2020_8BIT_LIMITED_RGB_TO_YUV,
    &BT2020_10BIT_FULL_RGB_TO_YUV,
    &BT2020_10BIT_LIMITED_RGB_TO_YUV,
    &BT2020_12BIT_FULL_RGB_TO_YUV,
    &BT2020_12BIT_LIMITED_RGB_TO_YUV,
    &BT2020_16BIT_FULL_RGB_TO_YUV,
    &BT2020_16BIT_LIMITED_RGB_TO_YUV,
    &FCC_FULL_RGB_TO_YUV,
    &FCC_LIMITED_RGB_TO_YUV,
    &SMPTE240_FULL_RGB_TO_YUV,
    &SMPTE240_LIMITED_RGB_TO_YUV,
    &YDZDX_FULL_RGB_TO_YUV,
    &YDZDX_LIMITED_RGB_TO_YUV,
    &GBR_FULL_RGB_TO_YUV,
    &GBR_LIMITED_RGB_TO_YUV,
    &YCGCO_8BIT_FULL_RGB_TO_YUV,
    &YCGCO_8BIT_LIMITED_RGB_TO_YUV,
    &YCGCO_10BIT_FULL_RGB_TO_YUV,
    &YCGCO_10BIT_LIMITED_RGB_TO_YUV,
    &YCGCO_12BIT_FULL_RGB_TO_YUV,
    &YCGCO_12BIT_LIMITED_RGB_TO_YUV,
    &YCGCO_16BIT_FULL_RGB_TO_YUV,
    &YCGCO_16BIT_LIMITED_RGB_TO_YUV,
];

// The identity matrix, as `memset(m, 0); m[0] = m[6] = m[12] = m[18] = 1` makes it.
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the literals are the C++ tables, verbatim
const IDENTITY_20: [f32; 20] = [
    1.0, 0.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 0.0, 1.0, 0.0,
];

/// Port of `SkColorMatrix_RGB2YUV`: the RGB to YUV matrix of `cs`, row-major.
// Port of: src/core/SkYUVMath.cpp#L842-L849 (chrome/m156)
#[doc(alias = "SkColorMatrix_RGB2YUV")]
#[must_use]
pub fn color_matrix_rgb2yuv(cs: YUVColorSpace) -> [f32; 20] {
    // `(unsigned)cs < kIdentity_SkYUVColorSpace`: a negative value is never below it, and the
    // table has one entry per colour space before the identity, so `get` is that test.
    usize::try_from(cs as i32)
        .ok()
        .and_then(|idx| RGB_TO_YUV_ARRAY.get(idx))
        .map_or(IDENTITY_20, |m| **m)
}

/// Port of `SkColorMatrix_YUV2RGB`: the YUV to RGB matrix of `cs`, row-major.
// Port of: src/core/SkYUVMath.cpp#L851-L858 (chrome/m156)
#[doc(alias = "SkColorMatrix_YUV2RGB")]
#[must_use]
pub fn color_matrix_yuv2rgb(cs: YUVColorSpace) -> [f32; 20] {
    // See `color_matrix_rgb2yuv`.
    usize::try_from(cs as i32)
        .ok()
        .and_then(|idx| YUV_TO_RGB_ARRAY.get(idx))
        .map_or(IDENTITY_20, |m| **m)
}
