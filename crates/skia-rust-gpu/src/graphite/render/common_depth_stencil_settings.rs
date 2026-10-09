// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/CommonDepthStencilSettings.h

//! [`DepthStencilSettings`] shared by several `RenderStep`s: direct depth passes, and the stencil
//! and cover passes of stencil-then-cover algorithms.

use crate::graphite::draw_types::{CompareOp, DepthStencilSettings, Face, StencilOp};

const ALL: u32 = 0xffff_ffff;

/// `kDirectDepthLessPass`: a single pass that shades directly, with a LESS depth test.
// Port of: src/gpu/graphite/render/CommonDepthStencilSettings.h#L15-L36 (chrome/m156)
#[doc(alias = "kDirectDepthLessPass")]
pub const DIRECT_DEPTH_LESS_PASS: DepthStencilSettings = DepthStencilSettings::new(
    Face::new(
        StencilOp::Keep,
        StencilOp::Keep,
        StencilOp::Keep,
        CompareOp::Always,
        ALL,
        ALL,
    ),
    Face::new(
        StencilOp::Keep,
        StencilOp::Keep,
        StencilOp::Keep,
        CompareOp::Always,
        ALL,
        ALL,
    ),
    0,
    false,
    CompareOp::Less,
    true,
    true,
);

/// `kDirectDepthLEqualPass`: a single pass that shades directly, with a LEQUAL depth test.
// Port of: src/gpu/graphite/render/CommonDepthStencilSettings.h#L24-L36 (chrome/m156)
#[doc(alias = "kDirectDepthLEqualPass")]
pub const DIRECT_DEPTH_LEQUAL_PASS: DepthStencilSettings = DepthStencilSettings::new(
    Face::new(
        StencilOp::Keep,
        StencilOp::Keep,
        StencilOp::Keep,
        CompareOp::Always,
        ALL,
        ALL,
    ),
    Face::new(
        StencilOp::Keep,
        StencilOp::Keep,
        StencilOp::Keep,
        CompareOp::Always,
        ALL,
        ALL,
    ),
    0,
    false,
    CompareOp::LEqual,
    true,
    true,
);

/// `kIncrementCW`: the front face of the winding stencil pass.
// Port of: src/gpu/graphite/render/CommonDepthStencilSettings.h#L40-L47 (chrome/m156)
#[doc(alias = "kIncrementCW")]
pub const INCREMENT_CW: Face = Face::new(
    StencilOp::Keep,
    StencilOp::Keep,
    StencilOp::IncWrap,
    CompareOp::Always,
    ALL,
    ALL,
);

/// `kDecrementCCW`: the back face of the winding stencil pass.
// Port of: src/gpu/graphite/render/CommonDepthStencilSettings.h#L49-L56 (chrome/m156)
#[doc(alias = "kDecrementCCW")]
pub const DECREMENT_CCW: Face = Face::new(
    StencilOp::Keep,
    StencilOp::Keep,
    StencilOp::DecWrap,
    CompareOp::Always,
    ALL,
    ALL,
);

/// `kIncrementAlways`: the face used by the increment stencil pass.
// Port of: src/gpu/graphite/render/CommonDepthStencilSettings.h#L58-L65 (chrome/m156)
#[doc(alias = "kIncrementAlways")]
pub const INCREMENT_ALWAYS: Face = Face::new(
    StencilOp::Keep,
    StencilOp::Keep,
    StencilOp::IncClamp,
    CompareOp::Always,
    ALL,
    ALL,
);

/// `kToggle`: the face used by the even-odd stencil pass.
// Port of: src/gpu/graphite/render/CommonDepthStencilSettings.h#L67-L74 (chrome/m156)
#[doc(alias = "kToggle")]
pub const TOGGLE: Face = Face::new(
    StencilOp::Keep,
    StencilOp::Keep,
    StencilOp::Invert,
    CompareOp::Always,
    ALL,
    0x0000_0001,
);

/// `kWindingStencilPass`.
// Port of: src/gpu/graphite/render/CommonDepthStencilSettings.h#L76-L84 (chrome/m156)
#[doc(alias = "kWindingStencilPass")]
pub const WINDING_STENCIL_PASS: DepthStencilSettings = DepthStencilSettings::new(
    INCREMENT_CW,
    DECREMENT_CCW,
    0,
    true,
    CompareOp::Less,
    true,
    // The depth write is handled by the covering pass.
    false,
);

/// `kEvenOddStencilPass`.
// Port of: src/gpu/graphite/render/CommonDepthStencilSettings.h#L86-L94 (chrome/m156)
#[doc(alias = "kEvenOddStencilPass")]
pub const EVEN_ODD_STENCIL_PASS: DepthStencilSettings = DepthStencilSettings::new(
    TOGGLE,
    TOGGLE,
    0,
    true,
    CompareOp::Less,
    true,
    // The depth write is handled by the covering pass.
    false,
);

/// `kIncrementStencilPass`.
// Port of: src/gpu/graphite/render/CommonDepthStencilSettings.h#L96-L104 (chrome/m156)
#[doc(alias = "kIncrementStencilPass")]
pub const INCREMENT_STENCIL_PASS: DepthStencilSettings = DepthStencilSettings::new(
    INCREMENT_ALWAYS,
    INCREMENT_ALWAYS,
    0,
    true,
    CompareOp::Less,
    true,
    // The depth write is handled by the covering pass.
    false,
);

/// `kPassNonZero`: the face of the regular cover pass.
// Port of: src/gpu/graphite/render/CommonDepthStencilSettings.h#L108-L115 (chrome/m156)
#[doc(alias = "kPassNonZero")]
pub const PASS_NON_ZERO: Face = Face::new(
    StencilOp::Keep,
    StencilOp::Zero,
    StencilOp::Zero,
    CompareOp::NotEqual,
    ALL,
    ALL,
);

/// `kPassZero`: the face of the inverse cover pass.
// Port of: src/gpu/graphite/render/CommonDepthStencilSettings.h#L117-L124 (chrome/m156)
#[doc(alias = "kPassZero")]
pub const PASS_ZERO: Face = Face::new(
    StencilOp::Zero,
    StencilOp::Keep,
    StencilOp::Keep,
    CompareOp::Equal,
    ALL,
    ALL,
);

/// `kRegularCoverPass`.
// Port of: src/gpu/graphite/render/CommonDepthStencilSettings.h#L126-L134 (chrome/m156)
#[doc(alias = "kRegularCoverPass")]
pub const REGULAR_COVER_PASS: DepthStencilSettings = DepthStencilSettings::new(
    PASS_NON_ZERO,
    PASS_NON_ZERO,
    0,
    true,
    CompareOp::Less,
    true,
    true,
);

/// `kInverseCoverPass`.
// Port of: src/gpu/graphite/render/CommonDepthStencilSettings.h#L136-L144 (chrome/m156)
#[doc(alias = "kInverseCoverPass")]
pub const INVERSE_COVER_PASS: DepthStencilSettings =
    DepthStencilSettings::new(PASS_ZERO, PASS_ZERO, 0, true, CompareOp::Less, true, true);
