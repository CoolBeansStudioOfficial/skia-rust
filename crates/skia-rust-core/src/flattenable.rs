// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkFlattenable.h, src/core/SkFlattenable.cpp (the name-to-factory
// lookup, chrome/m156)

//! `SkFlattenable` registration: the factories that rebuild a path effect or mask filter from its
//! name and flattened body.
//!
//! Skia keeps a process-wide table filled by `SkFlattenable::Register` during static
//! initialization. skia-rust has no global mutable state: a [`FlattenableRegistry`] is a pair of
//! static tables, built at compile time, that the caller passes to
//! [`ReadBuffer::read_path_effect`](crate::read_buffer::ReadBuffer::read_path_effect) and
//! [`ReadBuffer::read_mask_filter`](crate::read_buffer::ReadBuffer::read_mask_filter). The
//! effects of `skia-rust-effects` publish the complete table; the core effects (`SkComposePathEffect`,
//! `SkSumPathEffect`, `SkBlurMaskFilterImpl`) are in it too. A name missing from the table makes
//! the buffer invalid, as an unregistered name does in C++.

use crate::mask_filter::MaskFilter;
use crate::path_effect::PathEffect;
use crate::read_buffer::ReadBuffer;

/// Rebuilds a path effect from the body that its `flatten` wrote (`SkFlattenable::Factory` for
/// `SkPathEffect`). Nested path effects are read with the same registry.
// Port of: include/core/SkFlattenable.h#L41 (chrome/m156), `Factory`
#[doc(alias = "SkFlattenable::Factory")]
pub type PathEffectFactory = fn(&mut ReadBuffer<'_>, &FlattenableRegistry) -> Option<PathEffect>;

/// Rebuilds a mask filter from the body that its `flatten` wrote (`SkFlattenable::Factory` for
/// `SkMaskFilter`).
// Port of: include/core/SkFlattenable.h#L41 (chrome/m156), `Factory`
#[doc(alias = "SkFlattenable::Factory")]
pub type MaskFilterFactory = fn(&mut ReadBuffer<'_>, &FlattenableRegistry) -> Option<MaskFilter>;

/// The names and factories of the flattenables that can be read (`SkFlattenable::Register` and
/// `NameToFactory`), as two tables. Each entry is the name a flattenable is registered under;
/// the same factory may be registered under more than one name, as C++ does for legacy names.
// Port of: src/core/SkFlattenable.cpp (the registry, chrome/m156)
#[doc(alias = "SkFlattenable::Register")]
#[derive(Clone, Copy, Debug)]
pub struct FlattenableRegistry {
    /// The path effects, by name.
    pub path_effects: &'static [(&'static str, PathEffectFactory)],
    /// The mask filters, by name.
    pub mask_filters: &'static [(&'static str, MaskFilterFactory)],
}

impl FlattenableRegistry {
    /// A registry with nothing in it: every name is unknown.
    pub const EMPTY: FlattenableRegistry = FlattenableRegistry {
        path_effects: &[],
        mask_filters: &[],
    };

    /// The factory of the path effect registered as `name` (`NameToFactory`).
    #[must_use]
    pub fn path_effect_factory(&self, name: &str) -> Option<PathEffectFactory> {
        lookup(self.path_effects, name)
    }

    /// The factory of the mask filter registered as `name` (`NameToFactory`).
    #[must_use]
    pub fn mask_filter_factory(&self, name: &str) -> Option<MaskFilterFactory> {
        lookup(self.mask_filters, name)
    }
}

/// The factory registered as `name` in `table`.
fn lookup<T: Copy>(table: &[(&'static str, T)], name: &str) -> Option<T> {
    table
        .iter()
        .find(|(registered, _)| *registered == name)
        .map(|&(_, factory)| factory)
}
