// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: the `SkFlattenable::Register` calls of the effects, collected in
// `SkGlobalInitialization_default.cpp#L84-L97` (chrome/m156)

//! The flattenable registry of the effects: the names under which Skia registers each path effect
//! and mask filter, mapped to the factory that reads it back.
//!
//! Skia fills a process-wide table during static initialization. Here the table is a compile-time
//! constant, [`REGISTRY`], which a caller passes to the `read_path_effect` and `read_mask_filter`
//! methods of `ReadBuffer`. The names are the ones C++ registers, including the ones it registers
//! for effects whose `getTypeName` differs (for example `SkPath1DPathEffectImpl`), so a read of
//! those effects fails as it does in C++.

use skia_rust_core::blur_mask_filter_impl::blur_create_proc;
use skia_rust_core::flattenable::{FlattenableRegistry, MaskFilterFactory, PathEffectFactory};
use skia_rust_core::path_effect::{compose_create_proc, sum_create_proc};

use crate::corner_path_effect::create_proc as corner_path_effect;
use crate::dash_impl::create_proc as dash_impl;
use crate::discrete_path_effect::create_proc as discrete_path_effect;
use crate::emboss_mask_filter::create_proc as emboss_mask_filter;
use crate::line_2d_path_effect::create_proc as line_2d_path_effect;
use crate::path_1d_path_effect::create_proc as path_1d_path_effect;
use crate::path_2d_path_effect::create_proc as path_2d_path_effect;
use crate::table_mask_filter::create_proc as table_mask_filter;
use crate::trim_path_effect::create_proc as trim_path_effect;

/// The path effects registered by Skia, by name (`SkFlattenable::Register`).
// Port of: src/ports/SkGlobalInitialization_default.cpp#L90-L97 (chrome/m156), and the
// `RegisterFlattenables` of the path effects
const PATH_EFFECTS: &[(&str, PathEffectFactory)] = &[
    ("SkComposePathEffect", compose_create_proc),
    ("SkSumPathEffect", sum_create_proc),
    ("SkCornerPathEffect", corner_path_effect),
    ("SkDashImpl", dash_impl),
    ("SkDiscretePathEffect", discrete_path_effect),
    ("SkPath1DPathEffectImpl", path_1d_path_effect),
    ("SkLine2DPathEffectImpl", line_2d_path_effect),
    ("SkPath2DPathEffectImpl", path_2d_path_effect),
    ("SkTrimPE", trim_path_effect),
];

/// The mask filters registered by Skia, by name (`SkFlattenable::Register`).
// Port of: src/ports/SkGlobalInitialization_default.cpp#L84-L87 (chrome/m156), and the
// `RegisterFlattenables` of the mask filters
const MASK_FILTERS: &[(&str, MaskFilterFactory)] = &[
    ("SkBlurMaskFilterImpl", blur_create_proc),
    ("SkEmbossMaskFilter", emboss_mask_filter),
    ("SkTableMaskFilterImpl", table_mask_filter),
    // The previous name of the table mask filter.
    ("SkTableMF", table_mask_filter),
];

/// The flattenables of the effects, for `ReadBuffer::read_path_effect` and
/// `ReadBuffer::read_mask_filter`.
#[doc(alias = "SkFlattenable::Register")]
pub const REGISTRY: FlattenableRegistry = FlattenableRegistry {
    path_effects: PATH_EFFECTS,
    mask_filters: MASK_FILTERS,
};
