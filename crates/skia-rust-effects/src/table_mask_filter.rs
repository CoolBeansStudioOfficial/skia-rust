// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/SkTableMaskFilter.h, src/effects/SkTableMaskFilter.cpp

//! `SkTableMaskFilter`: mask filters that apply a table lookup to the alpha values of a mask.
//!
//! skia-rust: flattening and `asImageFilter` (which needs `SkColorFilters::TableARGB`) are not
//! ported yet.

use skia_rust_core::align::align4;
use skia_rust_core::fixed::{Fixed, fixed_round_to_int};
use skia_rust_core::flattenable::FlattenableRegistry;
use skia_rust_core::floating_point::float_round2int;
use skia_rust_core::mask::{AllocType, Mask, MaskBuilder, MaskFormat};
use skia_rust_core::mask_filter::{MaskFilter, MaskFilterBase, MaskFilterType};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::IPoint;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::scalar::scalar;
use skia_rust_core::t_pin::t_pin;
use skia_rust_core::write_buffer::BinaryWriteBuffer;

// Port of: src/effects/SkTableMaskFilter.cpp#L33-L62 (chrome/m156)
#[derive(Clone, Debug)]
struct TableMaskFilterImpl {
    table: [u8; 256],
}

impl TableMaskFilterImpl {
    // Port of: src/effects/SkTableMaskFilter.cpp#L64-L66 (chrome/m156)
    fn new(table: &[u8; 256]) -> Self {
        TableMaskFilterImpl { table: *table }
    }
}

/// `SkTableMaskFilterImpl::CreateProc`: the 256-entry table.
// Port of: src/effects/SkTableMaskFilter.cpp#L134-L140 (chrome/m156)
pub fn create_proc(
    buffer: &mut ReadBuffer<'_>,
    _registry: &FlattenableRegistry,
) -> Option<MaskFilter> {
    let mut table = [0u8; 256];
    if !buffer.read_byte_array(&mut table) {
        return None;
    }
    Some(new(&table))
}

impl MaskFilterBase for TableMaskFilterImpl {
    // Port of: src/effects/SkTableMaskFilter.cpp#L53 (chrome/m156), SK_FLATTENABLE_HOOKS
    fn type_name(&self) -> &'static str {
        "SkTableMaskFilterImpl"
    }

    // Port of: src/effects/SkTableMaskFilter.cpp#L130-L132 (chrome/m156)
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        buffer.write_byte_array(&self.table);
    }

    // Port of: src/effects/SkTableMaskFilter.cpp#L70-L116 (chrome/m156)
    fn filter_mask(
        &self,
        dst: &mut MaskBuilder,
        src: &Mask<'_>,
        _matrix: &Matrix,
        margin: Option<&mut IPoint>,
    ) -> bool {
        // SkAlign4 overflows when too close to INT32_MAX, so reject when too big.
        const MAX_WIDTH: i32 = 1 << 30;

        if src.format != MaskFormat::A8 {
            return false;
        }
        if src.bounds.width() > MAX_WIDTH {
            return false;
        }
        dst.bounds = src.bounds;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // width is positive
        {
            dst.row_bytes = align4(dst.bounds.width() as usize) as u32;
        }
        dst.format = MaskFormat::A8;
        dst.image = Vec::new();

        if !src.image.is_empty() {
            let img_size = dst.compute_image_size();
            if img_size == 0 {
                return false;
            }
            dst.image = MaskBuilder::alloc_image(img_size, AllocType::Uninit);

            let dst_width = usize::try_from(dst.bounds.width()).expect("positive");
            let dst_height = usize::try_from(dst.bounds.height()).expect("positive");
            let dst_rb = dst.row_bytes as usize;
            let src_rb = src.row_bytes as usize;
            for y in 0..dst_height {
                let src_row = &src.image[y * src_rb..y * src_rb + dst_width];
                let dst_row = &mut dst.image[y * dst_rb..y * dst_rb + dst_rb];
                for (d, &s) in dst_row.iter_mut().zip(src_row) {
                    *d = self.table[usize::from(s)];
                }
                // we can't just inc dstP by rowbytes, because if it has any
                // padding between its width and its rowbytes, we need to zero those
                // so that the bitters can read those safely if that is faster for
                // them
                dst_row[dst_width..].fill(0);
            }
        }

        if let Some(margin) = margin {
            margin.set(0, 0);
        }
        true
    }

    // Port of: src/effects/SkTableMaskFilter.cpp#L118-L120 (chrome/m156)
    fn format(&self) -> MaskFormat {
        MaskFormat::A8
    }

    fn filter_type(&self) -> MaskFilterType {
        MaskFilterType::Table
    }
}

/// A mask filter that maps each coverage value through `table`
/// (`SkTableMaskFilter::Create`).
// Port of: src/effects/SkTableMaskFilter.cpp#L144-L146 (chrome/m156)
#[doc(alias = "SkTableMaskFilter::Create")]
#[must_use]
pub fn new(table: &[u8; 256]) -> MaskFilter {
    MaskFilter::from_base(TableMaskFilterImpl::new(table))
}

/// A mask filter with the gamma table of `gamma` (`SkTableMaskFilter::CreateGamma`).
// Port of: src/effects/SkTableMaskFilter.cpp#L148-L152 (chrome/m156)
#[doc(alias = "SkTableMaskFilter::CreateGamma")]
#[must_use]
pub fn new_gamma(gamma: scalar) -> MaskFilter {
    let table = new_gamma_table(gamma);
    new(&table)
}

/// A mask filter with the clip table of `min` and `max` (`SkTableMaskFilter::CreateClip`).
// Port of: src/effects/SkTableMaskFilter.cpp#L154-L158 (chrome/m156)
#[doc(alias = "SkTableMaskFilter::CreateClip")]
#[must_use]
pub fn new_clip(min: u8, max: u8) -> MaskFilter {
    let table = new_clip_table(min, max);
    new(&table)
}

/// The gamma table of `gamma` (`SkTableMaskFilter::MakeGammaTable`).
// Port of: src/effects/SkTableMaskFilter.cpp#L160-L170 (chrome/m156)
#[doc(alias = "SkTableMaskFilter::MakeGammaTable")]
#[must_use]
pub fn new_gamma_table(gamma: scalar) -> [u8; 256] {
    let mut table = [0u8; 256];
    let dx: f32 = 1.0 / 255.0;
    let g: f32 = gamma;

    let mut x: f32 = 0.0;
    for t in &mut table {
        // float ee = powf(x, g) * 255;
        // skia-rust: libm (`f32::powf`)
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // pinned to 0..=255
        {
            *t = t_pin(float_round2int(x.powf(g) * 255.0), 0, 255) as u8;
        }
        x += dx;
    }
    table
}

/// The clip table of `min` and `max` (`SkTableMaskFilter::MakeClipTable`).
// Port of: src/effects/SkTableMaskFilter.cpp#L172-L200 (chrome/m156)
#[doc(alias = "SkTableMaskFilter::MakeClipTable")]
#[must_use]
pub fn new_clip_table(mut min: u8, mut max: u8) -> [u8; 256] {
    let mut table = [0u8; 256];
    if 0 == max {
        max = 1;
    }
    if min >= max {
        min = max - 1;
    }
    debug_assert!(min < max);

    let scale: Fixed = (1 << 16) * 255 / (i32::from(max) - i32::from(min));
    // memset(table, 0, min + 1) is a no-op on the zeroed table
    for i in min + 1..max {
        let value = fixed_round_to_int(scale * i32::from(i - min));
        debug_assert!(value <= 255);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // SkASSERT(value <= 255)
        {
            table[usize::from(i)] = value as u8;
        }
    }
    table[usize::from(max)..].fill(255);
    table
}

/// Provides the deprecated `MaskFilter::table`, `MaskFilter::gamma` and `MaskFilter::clip` of
/// `skia-safe` (Rust does not allow inherent impls outside the defining crate).
pub trait TableMaskFilterExt {
    /// See [`new`].
    fn table(table: &[u8; 256]) -> MaskFilter;
    /// See [`new_gamma`].
    fn gamma(gamma: scalar) -> MaskFilter;
    /// See [`new_clip`].
    fn clip(min: u8, max: u8) -> MaskFilter;
}

impl TableMaskFilterExt for MaskFilter {
    fn table(table: &[u8; 256]) -> MaskFilter {
        new(table)
    }

    fn gamma(gamma: scalar) -> MaskFilter {
        new_gamma(gamma)
    }

    fn clip(min: u8, max: u8) -> MaskFilter {
        new_clip(min, max)
    }
}
