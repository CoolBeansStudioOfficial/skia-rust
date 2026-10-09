// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/UniformManager.h, src/gpu/graphite/UniformManager.cpp

//! [`UniformOffsetCalculator`] and [`UniformManager`]: the std140, std430 and Metal uniform
//! layouts, with half-precision packing.
//!
//! The manager appends uniform values to a byte buffer, inserting the padding each layout needs.
//! Skia's raw pointers become byte offsets into the buffer, and the `SK_DEBUG` checks that verify
//! each write against the expected uniform declarations are compiled in when `debug_assertions`
//! are enabled (`SkDEBUGCODE` and `SkASSERT` in Skia).
//!
//! Skia's `write(const void*)` takes a pointer to the source values. Here the source is a byte
//! slice holding the values in native byte order: 4-byte `f32`/`i32` elements, or the `f32`
//! values that half-precision uniforms convert. Each typed `write_*` method builds those bytes.

use skia_rust_core::align::align_to;
use skia_rust_core::color::PMColor4f;
use skia_rust_core::m44::M44;
use skia_rust_core::math::is_pow2;
use skia_rust_core::math_priv::next_pow2;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::matrix_priv::m44_col_major;
use skia_rust_core::rect::Rect;
use skia_rust_simd::vx::{Vec as VxVec, to_half};

use crate::graphite::resource_types::Layout;
use crate::graphite::uniform::{K_NON_ARRAY, Uniform};
use crate::sksl_type_shared::SkSLType;

/// The three diverging behaviours of the layouts, as `LayoutRules` in Skia.
// Port of: src/gpu/graphite/UniformManager.h#L163-L170 (chrome/m156)
#[doc(alias = "LayoutRules")]
pub mod layout_rules {
    use super::Layout;

    /// Whether a non-array `vec3` takes the size of a `vec4` (Metal only).
    // Port of: src/gpu/graphite/UniformManager.h#L165 (chrome/m156)
    #[doc(alias = "PadVec3Size")]
    #[must_use]
    pub const fn pad_vec3_size(layout: Layout) -> bool {
        matches!(layout, Layout::Metal)
    }

    /// Whether arrays are aligned and strided to 16 bytes (std140 and its f16 variant).
    // Port of: src/gpu/graphite/UniformManager.h#L166 (chrome/m156)
    #[doc(alias = "AlignArraysTo16")]
    #[must_use]
    pub const fn align_arrays_to_16(layout: Layout) -> bool {
        matches!(layout, Layout::Std140 | Layout::Std140F16)
    }

    /// Whether half-precision uniforms are stored in full precision (std140 and std430).
    // Port of: src/gpu/graphite/UniformManager.h#L167-L168 (chrome/m156)
    #[doc(alias = "UseFullPrecision")]
    #[must_use]
    pub const fn use_full_precision(layout: Layout) -> bool {
        matches!(layout, Layout::Std140 | Layout::Std430)
    }
}

// Converts a non-negative Skia `int` offset or size to an index.
fn to_usize(x: i32) -> usize {
    usize::try_from(x).expect("uniform offsets and sizes are non-negative")
}

// Converts a byte count or index to Skia's `int`.
fn to_i32(x: usize) -> i32 {
    i32::try_from(x).expect("uniform storage fits in an int, as in Skia")
}

// Port of: src/gpu/graphite/UniformManager.h#L392-L402 (chrome/m156), the `IsHalfVector` check
// (`kHalf` through `kHalf4`).
fn is_half_vector(ty: SkSLType) -> bool {
    matches!(
        ty,
        SkSLType::Half | SkSLType::Half2 | SkSLType::Half3 | SkSLType::Half4
    )
}

// Port of: src/gpu/graphite/UniformManager.cpp#L108-L124 (chrome/m156)
fn adjust_for_matrix_type(ty: SkSLType, count: i32) -> (SkSLType, i32) {
    // All Layouts flatten matrices and arrays of matrices into arrays of columns, so update
    // `ty` to be the column type and either multiply `count` by the number of columns for arrays
    // of matrices, or set to exactly the number of columns for a "non-array" matrix.
    match ty {
        SkSLType::Float2x2 => (SkSLType::Float2, 2 * count.max(1)),
        SkSLType::Float3x3 => (SkSLType::Float3, 3 * count.max(1)),
        SkSLType::Float4x4 => (SkSLType::Float4, 4 * count.max(1)),

        SkSLType::Half2x2 => (SkSLType::Half2, 2 * count.max(1)),
        SkSLType::Half3x3 => (SkSLType::Half3, 3 * count.max(1)),
        SkSLType::Half4x4 => (SkSLType::Half4, 4 * count.max(1)),

        // Otherwise leave type and count alone.
        _ => (ty, count),
    }
}

// The vector type of `len` floats (or halves when `half`), as `write<Type>` dispatches on it.
fn float_vector_type(len: usize, half: bool) -> SkSLType {
    match (len, half) {
        (1, false) => SkSLType::Float,
        (2, false) => SkSLType::Float2,
        (3, false) => SkSLType::Float3,
        (4, false) => SkSLType::Float4,
        (1, true) => SkSLType::Half,
        (2, true) => SkSLType::Half2,
        (3, true) => SkSLType::Half3,
        (4, true) => SkSLType::Half4,
        _ => unreachable!("uniform vectors have 1 to 4 components"),
    }
}

// Writes `values` as native-endian bytes into the start of `out`.
fn f32s_to_bytes(values: &[f32], out: &mut [u8]) {
    for (i, value) in values.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&value.to_ne_bytes());
    }
}

/// Describes the offset of uniforms in a uniform block, according to a [`Layout`]
/// (`UniformOffsetCalculator`).
///
/// Unlike [`UniformManager`], it stores no values: it only computes where each uniform starts.
// Port of: src/gpu/graphite/UniformManager.h#L172-L216 (chrome/m156)
#[doc(alias = "skgpu::graphite::UniformOffsetCalculator")]
#[derive(Clone, Copy, Debug)]
pub struct UniformOffsetCalculator {
    layout: Layout,
    offset: i32,
    req_alignment: i32,
}

impl Default for UniformOffsetCalculator {
    // Port of: src/gpu/graphite/UniformManager.h#L176 (chrome/m156), `= default`
    fn default() -> Self {
        Self {
            layout: Layout::Invalid,
            offset: 0,
            req_alignment: 1,
        }
    }
}

impl UniformOffsetCalculator {
    /// A calculator for a top-level uniform block, starting at `offset`.
    // Port of: src/gpu/graphite/UniformManager.h#L178-L180 (chrome/m156)
    #[doc(alias = "ForTopLevel")]
    #[must_use]
    pub const fn for_top_level(layout: Layout, offset: i32) -> Self {
        Self {
            layout,
            offset,
            req_alignment: 1,
        }
    }

    /// A calculator for the fields of a struct. Its base alignment comes from its fields.
    // Port of: src/gpu/graphite/UniformManager.h#L182-L186 (chrome/m156)
    #[doc(alias = "ForStruct")]
    #[must_use]
    pub const fn for_struct(layout: Layout) -> Self {
        let req_alignment = if layout_rules::align_arrays_to_16(layout) {
            16
        } else {
            1
        };
        Self {
            layout,
            offset: 0,
            req_alignment,
        }
    }

    /// The layout this calculator uses.
    // Port of: src/gpu/graphite/UniformManager.h#L188 (chrome/m156)
    #[must_use]
    pub const fn layout(&self) -> Layout {
        self.layout
    }

    /// The last consumed byte. Inside a struct, round it up to [`Self::required_alignment`].
    // Port of: src/gpu/graphite/UniformManager.h#L193-L194 (chrome/m156)
    #[must_use]
    pub const fn size(&self) -> i32 {
        self.offset
    }

    /// The alignment every uniform added so far requires.
    // Port of: src/gpu/graphite/UniformManager.h#L195 (chrome/m156)
    #[must_use]
    pub const fn required_alignment(&self) -> i32 {
        self.req_alignment
    }

    /// Returns the correctly aligned offset to accommodate `count` instances of `ty` and advances
    /// the internal offset. Pass [`K_NON_ARRAY`] for a non-array.
    // Port of: src/gpu/graphite/UniformManager.h#L202 and src/gpu/graphite/UniformManager.cpp#L19-L48
    // (chrome/m156)
    #[doc(alias = "advanceOffset")]
    pub fn advance_offset(&mut self, ty: SkSLType, mut count: i32) -> i32 {
        debug_assert!(ty.can_be_uniform_value());

        let mut dimension = ty.matrix_size();
        if dimension > 0 {
            // All SkSL matrices are square and can be interpreted as an array of column vectors.
            count = count.max(1) * dimension;
        } else {
            dimension = ty.vec_length();
        }
        debug_assert!((1..=4).contains(&dimension));

        // Bump dimension up to 4 if the array or vec3 consumes 4 primitives per element.
        // NOTE: This affects the size; the alignment already rounds up to a power of 2.
        let is_array = count > K_NON_ARRAY;
        let force_align16 = is_array && layout_rules::align_arrays_to_16(self.layout);
        if force_align16
            || (dimension == 3 && (is_array || layout_rules::pad_vec3_size(self.layout)))
        {
            dimension = 4;
        }

        let primitive_size = if layout_rules::use_full_precision(self.layout)
            || ty.is_full_precision_numeric_type()
            || force_align16
        {
            4
        } else {
            2
        };
        let align = next_pow2(dimension) * primitive_size;
        let aligned_offset = align_to(self.offset, align);
        self.offset = aligned_offset + dimension * primitive_size * count.max(1);
        self.req_alignment = self.req_alignment.max(align);

        aligned_offset
    }

    /// Returns the correctly aligned offset to accommodate `count` instances of a struct whose
    /// fields were advanced in `substruct`. The size of the struct includes the padding of
    /// layout rule 9.
    // Port of: src/gpu/graphite/UniformManager.h#L209 and src/gpu/graphite/UniformManager.cpp#L50-L70
    // (chrome/m156)
    #[doc(alias = "advanceStruct")]
    pub fn advance_struct(&mut self, substruct: &Self, count: i32) -> i32 {
        debug_assert_eq!(substruct.layout, self.layout);

        // If array element strides are forced to 16-byte alignment, structs must also have their
        // base alignment rounded up to 16-byte alignment, which should have been accounted for in
        // `substruct`'s constructor.
        let base_alignment = substruct.required_alignment();
        debug_assert!(!layout_rules::align_arrays_to_16(self.layout) || base_alignment % 16 == 0);

        // Per layout rule #9, the struct size must be padded to its base alignment.
        let aligned_size = align_to(substruct.size(), base_alignment);

        let aligned_offset = align_to(self.offset, base_alignment);
        self.offset = aligned_offset + aligned_size * count.max(1);
        self.req_alignment = self.req_alignment.max(base_alignment);

        aligned_offset
    }
}

// The expectations that `SK_DEBUG` builds check each write against (the `fOffsetCalculator`,
// `fExpectedUniforms` and related members of `UniformManager`).
// Port of: src/gpu/graphite/UniformManager.h#L600-L632 (chrome/m156), the `SK_DEBUG` members
#[cfg(debug_assertions)]
#[derive(Clone, Debug)]
struct Expectations {
    offset_calculator: UniformOffsetCalculator,
    marked_offset_calculator: UniformOffsetCalculator,
    substruct_calculator: UniformOffsetCalculator,
    substruct_starting_offset: i32,
    expected_uniforms: Vec<Uniform>,
    expected_uniform_index: usize,
}

#[cfg(debug_assertions)]
impl Default for Expectations {
    fn default() -> Self {
        Self {
            offset_calculator: UniformOffsetCalculator::default(),
            marked_offset_calculator: UniformOffsetCalculator::default(),
            substruct_calculator: UniformOffsetCalculator::default(),
            substruct_starting_offset: -1,
            expected_uniforms: Vec::new(),
            expected_uniform_index: 0,
        }
    }
}

/// Packs uniform values into a byte buffer with the padding a [`Layout`] requires
/// (`UniformManager`).
///
/// The buffer is zeroed in its padding, so the finished bytes can be hashed to de-duplicate
/// uniform data before upload.
// Port of: src/gpu/graphite/UniformManager.h#L218-L398 (chrome/m156)
#[doc(alias = "skgpu::graphite::UniformManager")]
#[derive(Debug)]
pub struct UniformManager {
    storage: Vec<u8>,
    storage_high_water_mark: i32,

    layout: Layout,

    req_alignment: i32,
    end_paint_alignment: i32,
    struct_base_alignment: i32,

    end_paint_offset: i32,
    non_shading_offset: i32,

    wrote_paint_color: bool,

    #[cfg(debug_assertions)]
    expect: Expectations,
}

impl UniformManager {
    /// Creates a manager that writes with `layout`.
    // Port of: src/gpu/graphite/UniformManager.h#L219 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout) -> Self {
        let mut manager = Self {
            storage: Vec::new(),
            storage_high_water_mark: 0,
            layout,
            req_alignment: 1,
            end_paint_alignment: 1,
            struct_base_alignment: 0,
            end_paint_offset: 0,
            non_shading_offset: 0,
            wrote_paint_color: false,
            #[cfg(debug_assertions)]
            expect: Expectations::default(),
        };
        manager.reset_with_new_layout(layout);
        manager
    }

    /// Marks the end of the paint uniforms, so a render step can be written after them.
    // Port of: src/gpu/graphite/UniformManager.h#L222-L226 (chrome/m156)
    #[doc(alias = "markOffset")]
    pub fn mark_offset(&mut self) {
        self.end_paint_offset = self.storage_len();
        self.end_paint_alignment = self.req_alignment;
        #[cfg(debug_assertions)]
        {
            self.expect.marked_offset_calculator = self.expect.offset_calculator;
        }
    }

    /// Aligns the non-shading render step uniforms to `required_alignment`.
    // Port of: src/gpu/graphite/UniformManager.h#L228-L243 (chrome/m156)
    #[doc(alias = "alignForNonShading")]
    pub fn align_for_non_shading(&mut self, required_alignment: i32) {
        self.align_to_storage(required_alignment);
        self.non_shading_offset = self.storage_len();
        debug_assert!(is_pow2(required_alignment));
        self.req_alignment = self.req_alignment.max(required_alignment);

        #[cfg(debug_assertions)]
        {
            self.expect.offset_calculator = UniformOffsetCalculator::for_top_level(self.layout, 0);
            self.expect.expected_uniforms.clear();
            self.expect.expected_uniform_index = 0;
        }
        // If we're rewinding, we shouldn't be using substructs.
        #[cfg(debug_assertions)]
        debug_assert_eq!(self.expect.substruct_starting_offset, -1);
        // Any struct should be closed.
        debug_assert_eq!(self.struct_base_alignment, 0);
    }

    /// Aligns the storage and returns the bytes written so far, from the start.
    // Port of: src/gpu/graphite/UniformManager.h#L245-L250 (chrome/m156), `finish(0)`
    pub fn finish(&mut self) -> &[u8] {
        self.finish_from(0)
    }

    /// Aligns the storage and returns the bytes written so far, from `subspan_start`.
    // Port of: src/gpu/graphite/UniformManager.h#L245-L250 (chrome/m156)
    pub fn finish_from(&mut self, subspan_start: usize) -> &[u8] {
        self.align_to_storage(self.req_alignment);
        self.storage_high_water_mark = self.storage_high_water_mark.max(self.storage_len());
        if self.storage.is_empty() {
            &[]
        } else {
            &self.storage[subspan_start..]
        }
    }

    /// Returns the bytes of the non-shading render step uniforms, after the paint uniforms.
    // Port of: src/gpu/graphite/UniformManager.h#L252-L254 (chrome/m156), `finishMarked`
    #[doc(alias = "finishMarked")]
    pub fn finish_marked(&mut self) -> &[u8] {
        let start = to_usize(self.non_shading_offset);
        self.finish_from(start)
    }

    /// Resets the manager to `layout`, with no uniforms written.
    // Port of: src/gpu/graphite/UniformManager.cpp#L72-L89 (chrome/m156)
    #[doc(alias = "resetWithNewLayout")]
    pub fn reset_with_new_layout(&mut self, layout: Layout) {
        self.storage.clear();
        self.layout = layout;
        self.req_alignment = 1;
        self.end_paint_alignment = 1;
        self.end_paint_offset = 0;
        self.non_shading_offset = 0;
        self.struct_base_alignment = 0;
        self.wrote_paint_color = false;

        #[cfg(debug_assertions)]
        {
            self.expect.offset_calculator = UniformOffsetCalculator::for_top_level(layout, 0);
            self.expect.marked_offset_calculator = self.expect.offset_calculator;
            self.expect.substruct_calculator = UniformOffsetCalculator::default();
            self.expect.expected_uniforms.clear();
            self.expect.expected_uniform_index = 0;
        }
    }

    /// Resets the manager to its current layout.
    // Port of: src/gpu/graphite/UniformManager.h#L255 (chrome/m156)
    pub fn reset(&mut self) {
        self.reset_with_new_layout(self.layout);
    }

    /// Rewinds the storage to the end of the paint uniforms (see [`Self::mark_offset`]).
    // Port of: src/gpu/graphite/UniformManager.cpp#L91-L106 (chrome/m156)
    #[doc(alias = "rewindToMark")]
    pub fn rewind_to_mark(&mut self) {
        // Prepare the storage parameters such that:
        //  1) If the render step is shading, the size and alignment can grow directly from the
        //     state at the end of the paint uniforms.
        //  2) If the render step is non-shading, the storage can be aligned to the render step's
        //     uniform alignment requirements.
        self.storage.truncate(to_usize(self.end_paint_offset));
        self.req_alignment = self.end_paint_alignment;
        self.non_shading_offset = 0;
        #[cfg(debug_assertions)]
        {
            self.expect.offset_calculator = self.expect.marked_offset_calculator;
        }

        // If we're rewinding, we shouldn't be using substructs.
        #[cfg(debug_assertions)]
        debug_assert_eq!(self.expect.substruct_starting_offset, -1);
        // Any struct should be closed.
        debug_assert_eq!(self.struct_base_alignment, 0);
    }

    /// The layout this manager writes with.
    // Port of: src/gpu/graphite/UniformManager.h#L257 (chrome/m156)
    #[must_use]
    pub const fn layout(&self) -> Layout {
        self.layout
    }

    /// The number of bytes written so far.
    // Port of: src/gpu/graphite/UniformManager.h#L258 (chrome/m156)
    #[must_use]
    pub fn size(&self) -> i32 {
        self.storage_len()
    }

    /// Gives back capacity when the high-water mark has fallen to half of it.
    // Port of: src/gpu/graphite/UniformManager.h#L260-L270 (chrome/m156)
    #[doc(alias = "tryShrinkCapacity")]
    pub fn try_shrink_capacity(&mut self) {
        let half_capacity = self.storage.capacity() / 2;
        if self.storage_high_water_mark < to_i32(half_capacity) {
            self.storage_high_water_mark = 0;
            debug_assert_eq!(self.storage.len(), 0);
            // Skia's `reserve_exact` on an empty array; `shrink_to` does the same here.
            self.storage.shrink_to(half_capacity);
        }
    }

    /// Writes a float (`write(float)`).
    // Port of: src/gpu/graphite/UniformManager.h#L274 (chrome/m156)
    pub fn write_f32(&mut self, f: f32) {
        self.write_typed(&f.to_ne_bytes(), SkSLType::Float);
    }

    /// Writes an int (`write(int32_t)`).
    // Port of: src/gpu/graphite/UniformManager.h#L275 (chrome/m156)
    pub fn write_i32(&mut self, i: i32) {
        self.write_typed(&i.to_ne_bytes(), SkSLType::Int);
    }

    /// Writes an unsigned int (`write(uint32_t)`).
    // Port of: src/gpu/graphite/UniformManager.h#L276 (chrome/m156)
    pub fn write_u32(&mut self, u: u32) {
        self.write_typed(&u.to_ne_bytes(), SkSLType::UInt);
    }

    /// Writes a float as a half-precision value (`writeHalf(float)`).
    // Port of: src/gpu/graphite/UniformManager.h#L277 (chrome/m156)
    pub fn write_half(&mut self, f: f32) {
        self.write_typed(&f.to_ne_bytes(), SkSLType::Half);
    }

    /// Writes an `N`-component float vector (`write` of an `SkV2`, `SkV3`, `SkV4`, `SkPoint`,
    /// `SkSize`, ...).
    // Port of: src/gpu/graphite/UniformManager.h#L280-L357 (chrome/m156), the vector overloads
    pub fn write_vec<const N: usize>(&mut self, v: [f32; N]) {
        let mut bytes = [0u8; 16];
        f32s_to_bytes(&v, &mut bytes[..4 * N]);
        self.write_typed(&bytes[..4 * N], float_vector_type(N, false));
    }

    /// Writes an `N`-component vector as half-precision values (`writeHalf` of a vector).
    // Port of: src/gpu/graphite/UniformManager.h#L286-L300 (chrome/m156), the half vector overloads
    pub fn write_half_vec<const N: usize>(&mut self, v: [f32; N]) {
        let mut bytes = [0u8; 16];
        f32s_to_bytes(&v, &mut bytes[..4 * N]);
        self.write_typed(&bytes[..4 * N], float_vector_type(N, true));
    }

    /// Writes an `N`-component int vector (`write` of an `SkISize` or `SkIRect`).
    // Port of: src/gpu/graphite/UniformManager.h#L296-L303 (chrome/m156), the int vector overloads
    pub fn write_ivec<const N: usize>(&mut self, v: [i32; N]) {
        let mut bytes = [0u8; 16];
        for (i, value) in v.iter().enumerate() {
            bytes[4 * i..4 * i + 4].copy_from_slice(&value.to_ne_bytes());
        }
        let ty = match N {
            1 => SkSLType::Int,
            2 => SkSLType::Int2,
            3 => SkSLType::Int3,
            4 => SkSLType::Int4,
            _ => unreachable!("uniform vectors have 1 to 4 components"),
        };
        self.write_typed(&bytes[..4 * N], ty);
    }

    /// Writes a color with premultiplied components (`write(SkPMColor4f)`).
    // Port of: src/gpu/graphite/UniformManager.h#L280 (chrome/m156)
    pub fn write_color(&mut self, c: &PMColor4f) {
        self.write_vec([c.r, c.g, c.b, c.a]);
    }

    /// Writes a rectangle as `vec4(left, top, right, bottom)` (`write(const SkRect&)`).
    // Port of: src/gpu/graphite/UniformManager.h#L281 (chrome/m156)
    pub fn write_rect(&mut self, r: &Rect) {
        self.write_vec([r.left, r.top, r.right, r.bottom]);
    }

    /// Writes a 4x4 matrix as four column vectors (`write(const SkM44&)`).
    // Port of: src/gpu/graphite/UniformManager.h#L329-L333 (chrome/m156)
    pub fn write_m44(&mut self, m: &M44) {
        let mut bytes = [0u8; 64];
        f32s_to_bytes(&m44_col_major(m), &mut bytes);
        self.write_array_typed(&bytes, 4, SkSLType::Float4);
    }

    /// Writes a 4x4 matrix as half-precision column vectors (`writeHalf(const SkM44&)`).
    // Port of: src/gpu/graphite/UniformManager.h#L335-L337 (chrome/m156)
    pub fn write_half_m44(&mut self, m: &M44) {
        let mut bytes = [0u8; 64];
        f32s_to_bytes(&m44_col_major(m), &mut bytes);
        self.write_array_typed(&bytes, 4, SkSLType::Half4);
    }

    /// Writes a 3x3 matrix as three column vectors, transposing the row-major `SkMatrix`
    /// (`write(const SkMatrix&)`).
    // Port of: src/gpu/graphite/UniformManager.h#L339-L345 (chrome/m156)
    pub fn write_matrix(&mut self, m: &Matrix) {
        let col_major = matrix_column_major(m);
        let mut bytes = [0u8; 36];
        f32s_to_bytes(&col_major, &mut bytes);
        self.write_array_typed(&bytes, 3, SkSLType::Float3);
    }

    /// Writes a 3x3 matrix as half-precision column vectors (`writeHalf(const SkMatrix&)`).
    // Port of: src/gpu/graphite/UniformManager.h#L347-L352 (chrome/m156)
    pub fn write_half_matrix(&mut self, m: &Matrix) {
        let col_major = matrix_column_major(m);
        let mut bytes = [0u8; 36];
        f32s_to_bytes(&col_major, &mut bytes);
        self.write_array_typed(&bytes, 3, SkSLType::Half3);
    }

    /// Writes an array of floats (`writeArray(SkSpan<const float>)`).
    // Port of: src/gpu/graphite/UniformManager.h#L290-L292 (chrome/m156)
    pub fn write_array_f32(&mut self, f: &[f32]) {
        let mut bytes = vec![0u8; 4 * f.len()];
        f32s_to_bytes(f, &mut bytes);
        self.write_array_typed(&bytes, to_i32(f.len()), SkSLType::Float);
    }

    /// Writes an array of `N`-component float vectors (`writeArray` of `SkV2`, `SkV4`, ...).
    // Port of: src/gpu/graphite/UniformManager.h#L293-L302 (chrome/m156)
    pub fn write_array_vec<const N: usize>(&mut self, v: &[[f32; N]]) {
        let flat: Vec<f32> = v.iter().flatten().copied().collect();
        let mut bytes = vec![0u8; 4 * flat.len()];
        f32s_to_bytes(&flat, &mut bytes);
        self.write_array_typed(&bytes, to_i32(v.len()), float_vector_type(N, false));
    }

    /// Writes an array of `N`-component vectors as half-precision values.
    // Port of: src/gpu/graphite/UniformManager.h#L303-L306 (chrome/m156)
    pub fn write_half_array_vec<const N: usize>(&mut self, v: &[[f32; N]]) {
        let flat: Vec<f32> = v.iter().flatten().copied().collect();
        let mut bytes = vec![0u8; 4 * flat.len()];
        f32s_to_bytes(&flat, &mut bytes);
        self.write_array_typed(&bytes, to_i32(v.len()), float_vector_type(N, true));
    }

    /// Writes the paint color, once per uniform block (`writePaintColor`).
    // Port of: src/gpu/graphite/UniformManager.h#L359-L370 (chrome/m156)
    #[doc(alias = "writePaintColor")]
    pub fn write_paint_color(&mut self, color: &PMColor4f) {
        if self.wrote_paint_color {
            // Validate expected uniforms, but don't write a second copy since the paint color
            // uniform can only ever be declared once in the final SkSL program.
            #[cfg(debug_assertions)]
            self.check_expected(None, SkSLType::Half4, K_NON_ARRAY);
        } else {
            self.write_typed(&color_bytes(color), SkSLType::Half4);
            self.wrote_paint_color = true;
        }
    }

    /// Copies the value at `data` for the uniform `u`, using the array-count semantics of `u`
    /// (`write(const Uniform&, const void*)`). `data` holds the values in native byte order.
    // Port of: src/gpu/graphite/UniformManager.h#L372 and src/gpu/graphite/UniformManager.cpp#L126-L169
    // (chrome/m156)
    #[doc(alias = "write")]
    pub fn write_uniform(&mut self, u: &Uniform, data: &[u8]) {
        debug_assert!(u.ty().can_be_uniform_value());
        debug_assert!(!u.is_paint_color()); // Must go through write_paint_color()

        let (ty, count) = adjust_for_matrix_type(u.ty(), u.count());
        debug_assert!(ty.matrix_size() < 0); // Matrix types should have been flattened

        let full_precision = layout_rules::use_full_precision(self.layout) || !is_half_vector(ty);
        let n = to_usize(ty.vec_length());
        let half = !full_precision;

        if count == K_NON_ARRAY {
            self.write_n(data, n, half, ty);
        } else {
            let align16 = layout_rules::align_arrays_to_16(self.layout);
            self.write_array_n(data, count, n, half, align16, ty);
        }
    }

    /// Starts a struct whose base alignment is `base_alignment`; the next writes are its fields
    /// (`beginStruct`).
    // Port of: src/gpu/graphite/UniformManager.h#L383-L388 (chrome/m156)
    #[doc(alias = "beginStruct")]
    pub fn begin_struct(&mut self, base_alignment: i32) {
        #[cfg(debug_assertions)]
        self.check_begin_struct(base_alignment);

        self.align_to_storage(base_alignment);
        self.struct_base_alignment = base_alignment;
        self.req_alignment = self.req_alignment.max(base_alignment);
    }

    /// Ends the struct started by [`Self::begin_struct`] (`endStruct`).
    // Port of: src/gpu/graphite/UniformManager.h#L390-L394 (chrome/m156)
    #[doc(alias = "endStruct")]
    pub fn end_struct(&mut self) {
        debug_assert!(self.struct_base_alignment >= 1); // Must have started a struct
        self.align_to_storage(self.struct_base_alignment);
        #[cfg(debug_assertions)]
        self.check_end_struct();
        self.struct_base_alignment = 0;
    }

    /// Whether nothing has been written since the last reset (`isReset`).
    // Port of: src/gpu/graphite/UniformManager.cpp#L278-L280 (chrome/m156)
    #[cfg(debug_assertions)]
    #[must_use]
    pub fn is_reset(&self) -> bool {
        self.storage.is_empty()
    }

    /// The bytes written so far, without aligning them (`fStorage`). The debug check
    /// `PipelineDataGatherer::checkEquivalent` compares these.
    // Port of: src/gpu/graphite/UniformManager.h#L300 (chrome/m156), `fStorage` (debug access)
    #[cfg(debug_assertions)]
    #[must_use]
    pub fn storage(&self) -> &[u8] {
        &self.storage
    }

    /// Declares the uniforms the next writes must match (`setExpectedUniforms`).
    // Port of: src/gpu/graphite/UniformManager.cpp#L282-L294 (chrome/m156)
    #[cfg(debug_assertions)]
    #[doc(alias = "setExpectedUniforms")]
    pub fn set_expected_uniforms(&mut self, expected: &[Uniform], is_substruct: bool) {
        self.expect.expected_uniforms = expected.to_vec();
        self.expect.expected_uniform_index = 0;

        if is_substruct {
            // Start collecting the subsequent uniforms with a 0-based offset to determine their
            // relative layout and required base alignment of the entire struct.
            self.expect.substruct_calculator = UniformOffsetCalculator::for_struct(self.layout);
        } else {
            // Expected uniforms will advance `offset_calculator` directly.
            debug_assert_eq!(self.expect.substruct_calculator.layout(), Layout::Invalid);
        }
    }

    /// Ends the declaration started by [`Self::set_expected_uniforms`]
    /// (`doneWithExpectedUniforms`).
    // Port of: src/gpu/graphite/UniformManager.cpp#L296-L302 (chrome/m156)
    #[cfg(debug_assertions)]
    #[doc(alias = "doneWithExpectedUniforms")]
    pub fn done_with_expected_uniforms(&mut self) {
        debug_assert_eq!(
            self.expect.expected_uniform_index,
            self.expect.expected_uniforms.len()
        );
        // Any expected substruct should have been ended and validated inside end_struct(); if
        // this fails it means there is a missing end_struct().
        debug_assert_eq!(self.expect.substruct_calculator.layout(), Layout::Invalid);
        self.expect.expected_uniforms.clear();
    }

    // The number of bytes written, as Skia's `int` size.
    fn storage_len(&self) -> i32 {
        to_i32(self.storage.len())
    }

    // Port of: src/gpu/graphite/UniformManager.h#L600-L605 (chrome/m156), `alignTo`
    fn align_to_storage(&mut self, alignment: i32) {
        debug_assert!(alignment >= 1 && is_pow2(alignment));
        if (self.storage_len() & (alignment - 1)) != 0 {
            self.append(alignment, 0);
        }
    }

    // Appends `size` bytes after the padding that `alignment` requires, and returns the index of
    // the first byte of the new data. The bytes are zeroed; the caller writes the data.
    // Port of: src/gpu/graphite/UniformManager.h#L607-L625 (chrome/m156), `append`
    fn append(&mut self, alignment: i32, size: i32) -> usize {
        // The base alignment for a struct should have been calculated for the current layout
        // using UniformOffsetCalculator, so every field appended within the struct should have
        // an alignment less than or equal to that base alignment.
        debug_assert!(self.struct_base_alignment <= 0 || alignment <= self.struct_base_alignment);

        let offset = self.storage_len();
        let padding = align_to(offset, alignment) - offset;

        // These are just asserts, not aborts, because SkSL compilation imposes limits on the size
        // of runtime effect arrays, and internal shaders should not be using excessive lengths.
        debug_assert!(i32::MAX - alignment >= offset);
        debug_assert!(i32::MAX - size >= padding);

        let dst = self.storage.len() + to_usize(padding);
        self.storage.resize(dst + to_usize(size), 0);

        // For pow of 2, max is LCM. If that assumption changes, this should change as well.
        self.req_alignment = self.req_alignment.max(alignment);
        dst
    }

    // Port of: src/gpu/graphite/UniformManager.h#L541-L556 (chrome/m156), `write<N, Half>`
    fn write_n(&mut self, src: &[u8], n: usize, half: bool, ty: SkSLType) {
        // `ty` is read only by the debug checks below.
        let _ = ty;
        let traits = LayoutTraits { n, half };
        let pad_vec3 = n == 3 && layout_rules::pad_vec3_size(self.layout);

        // Layouts diverge in how vec3 size is determined for non-array usage.
        let size = if pad_vec3 {
            traits.size() + traits.elem_size()
        } else {
            traits.size()
        };
        let dst = self.append(to_i32(traits.align()), to_i32(size));
        #[cfg(debug_assertions)]
        self.check_expected(Some(dst), ty, K_NON_ARRAY);

        traits.copy(src, &mut self.storage[dst..dst + traits.size()]);
        if pad_vec3 {
            self.storage[dst + traits.size()..dst + size].fill(0);
        }
    }

    // Port of: src/gpu/graphite/UniformManager.h#L558-L598 (chrome/m156), `writeArray<N, Half, Align16>`
    fn write_array_n(
        &mut self,
        src: &[u8],
        count: i32,
        n: usize,
        half: bool,
        align16: bool,
        ty: SkSLType,
    ) {
        // `ty` is read only by the debug checks below.
        let _ = ty;
        let traits = LayoutTraits { n, half };
        let src_stride = 4 * n; // Source data is always in multiples of 4 bytes.

        debug_assert!(count > 0);
        debug_assert_eq!(align16, layout_rules::align_arrays_to_16(self.layout));

        if half || n == 3 || (n != 4 && align16) {
            //         Size (H|F)  Align (H|F)  Padding (H|F) Align16-Padding (H|F)
            // N = 1   2|4         2|4          0|0(*)        14|12
            // N = 2   4|8         4|8          0|0(*)        12|8
            // N = 3   6|12        8|16         2|4           10|4
            // N = 4   8|16        8|16         0|0(*)         8|0(*)
            // Padding entries marked with (*) represent cases that fall into the else block below.
            // All other cases need per-element half conversion and/or per-element padding added.
            let stride = if align16 { 16 } else { traits.align() };

            let first = self.append(to_i32(stride), to_i32(stride) * count);
            #[cfg(debug_assertions)]
            self.check_expected(Some(first), ty, count);

            for i in 0..to_usize(count) {
                let dst = first + i * stride;
                traits.copy(
                    &src[i * src_stride..],
                    &mut self.storage[dst..dst + traits.size()],
                );
                if stride > traits.size() {
                    self.storage[dst + traits.size()..dst + stride].fill(0);
                }
            }
        } else {
            // A dense array with no type conversion, so copy in one go.
            debug_assert!(traits.align() == traits.size() && src_stride == traits.size());
            let total = traits.size() * to_usize(count);
            let dst = self.append(to_i32(traits.align()), to_i32(total));
            #[cfg(debug_assertions)]
            self.check_expected(Some(dst), ty, count);

            self.storage[dst..dst + total].copy_from_slice(&src[..total]);
        }
    }

    // Port of: src/gpu/graphite/UniformManager.h#L505-L509 (chrome/m156), the `write<Type>` and
    // `writeArray<Type>` dispatch on full precision
    fn write_typed(&mut self, src: &[u8], ty: SkSLType) {
        let n = to_usize(ty.vec_length());
        let half = is_half_vector(ty) && !layout_rules::use_full_precision(self.layout);
        self.write_n(src, n, half, ty);
    }

    // Port of: src/gpu/graphite/UniformManager.h#L511-L525 (chrome/m156), `writeArray<Type>`
    fn write_array_typed(&mut self, src: &[u8], count: i32, ty: SkSLType) {
        let n = to_usize(ty.vec_length());
        let half = is_half_vector(ty) && !layout_rules::use_full_precision(self.layout);
        let align16 = layout_rules::align_arrays_to_16(self.layout);
        self.write_array_n(src, count, n, half, align16, ty);
    }

    // Port of: src/gpu/graphite/UniformManager.cpp#L173-L196 (chrome/m156), `checkBeginStruct`
    #[cfg(debug_assertions)]
    fn check_begin_struct(&mut self, base_alignment: i32) {
        // Wrote a struct field before the struct was started.
        debug_assert_eq!(self.expect.expected_uniform_index, 0);

        // Not expecting to start a struct (layout must be valid).
        debug_assert_ne!(self.expect.substruct_calculator.layout(), Layout::Invalid);

        // Somehow already started a substruct (base alignment should be <= 0 initially).
        debug_assert!(self.struct_base_alignment <= 0);

        // Empty substructs are not allowed.
        debug_assert_ne!(self.expect.expected_uniforms.len(), 0);

        // Assume the expected uniforms describe the whole substruct.
        let mut struct_calculator = UniformOffsetCalculator::for_struct(self.layout);
        for f in &self.expect.expected_uniforms {
            struct_calculator.advance_offset(f.ty(), f.count());
        }

        // Calculated alignment must match the passed base alignment.
        debug_assert_eq!(base_alignment, struct_calculator.required_alignment());

        self.expect.substruct_starting_offset = self
            .expect
            .offset_calculator
            .advance_struct(&struct_calculator, K_NON_ARRAY);
    }

    // Port of: src/gpu/graphite/UniformManager.cpp#L198-L222 (chrome/m156), `checkEndStruct`
    #[cfg(debug_assertions)]
    fn check_end_struct(&mut self) {
        // Didn't write all the expected fields before ending the struct.
        debug_assert_eq!(
            self.expect.expected_uniform_index,
            self.expect.expected_uniforms.len()
        );

        // Not expecting a struct (layout must be valid).
        debug_assert_ne!(self.expect.substruct_calculator.layout(), Layout::Invalid);

        // Missing a begin_struct() (base alignment must be > 0 if we are in a struct).
        debug_assert!(self.struct_base_alignment > 0);

        // `substruct_calculator` should now have been advanced equivalently to the substruct
        // calculator used in check_begin_struct() to calculate the expected starting offset.
        let sub = self.expect.substruct_calculator;
        let struct_size = align_to(sub.size(), sub.required_alignment());

        // Somehow didn't end on the correct boundary.
        debug_assert_eq!(
            self.storage_len(),
            self.expect.substruct_starting_offset + struct_size
        );

        // UniformManager's alignment got out of sync with expected alignment.
        debug_assert_eq!(
            self.req_alignment,
            self.expect.offset_calculator.required_alignment()
        );
        debug_assert!(self.req_alignment >= sub.required_alignment());

        // Reset the substruct calculator to mark that the struct has been completed.
        self.expect.substruct_calculator = UniformOffsetCalculator::default();
    }

    // Port of: src/gpu/graphite/UniformManager.cpp#L224-L276 (chrome/m156), `checkExpected`
    #[cfg(debug_assertions)]
    fn check_expected(&mut self, dst: Option<usize>, ty: SkSLType, count: i32) {
        // A write() outside of a UniformExpectationsVisitor or too many uniforms written for what
        // is expected.
        debug_assert!(self.expect.expected_uniform_index < self.expect.expected_uniforms.len());

        if self.expect.substruct_calculator.layout() == Layout::Invalid {
            // A substruct was started when it shouldn't have been.
            debug_assert!(self.struct_base_alignment <= 0);
        } else {
            // A write() that should be inside a struct, but missing a call to begin_struct().
            debug_assert!(self.struct_base_alignment > 0);
        }

        let expected = &self.expect.expected_uniforms[self.expect.expected_uniform_index];
        self.expect.expected_uniform_index += 1;
        // Not all types are supported as uniforms or supported by UniformManager.
        debug_assert!(expected.ty().can_be_uniform_value());

        let (expected_type, expected_count) =
            adjust_for_matrix_type(expected.ty(), expected.count());
        debug_assert!(expected_type == ty && expected_count == count);

        if let Some(dst) = dst {
            // `dst` is the aligned starting offset of the uniform being checked, relative to the
            // start of the storage, so subtracting the non-shading offset gives the offset.
            let offset = to_i32(dst) - self.non_shading_offset;

            if self.expect.substruct_calculator.layout() == Layout::Invalid {
                // Pass the original expected type and count to the offset calculator.
                let expected_offset = self
                    .expect
                    .offset_calculator
                    .advance_offset(expected.ty(), expected.count());
                debug_assert_eq!(offset, expected_offset);
                debug_assert_eq!(
                    self.req_alignment,
                    self.expect.offset_calculator.required_alignment()
                );

                // And if it is the paint color uniform, we should not have already written it.
                debug_assert!(!(self.wrote_paint_color && expected.is_paint_color()));
            } else {
                let rel_offset = self
                    .expect
                    .substruct_calculator
                    .advance_offset(expected.ty(), expected.count());
                debug_assert_eq!(offset, self.expect.substruct_starting_offset + rel_offset);

                // The overall required alignment might already be higher from prior fields, but
                // should be at least what's required by the substruct.
                debug_assert!(
                    self.req_alignment >= self.expect.substruct_calculator.required_alignment()
                );

                // And it should not be a paint color uniform within a substruct.
                debug_assert!(!expected.is_paint_color());
            }
        } else {
            // If `dst` is None, it's an already-visited paint color uniform, so it's not being
            // written and not changing the offset, and should not be part of a substruct.
            debug_assert!(self.wrote_paint_color);
            debug_assert_eq!(self.expect.substruct_calculator.layout(), Layout::Invalid);
            debug_assert!(expected.is_paint_color());
        }
    }
}

// The matrix's members in column-major order: `SkMatrix` is row-major, so the columns are its
// (0, 3, 6), (1, 4, 7) and (2, 5, 8) members.
// Port of: src/gpu/graphite/UniformManager.h#L340-L344 (chrome/m156)
fn matrix_column_major(m: &Matrix) -> [f32; 9] {
    [m[0], m[3], m[6], m[1], m[4], m[7], m[2], m[5], m[8]]
}

// The bytes of a premultiplied color's four components, in order.
fn color_bytes(c: &PMColor4f) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    f32s_to_bytes(&[c.r, c.g, c.b, c.a], &mut bytes);
    bytes
}

/// `LayoutTraits<N, Half>`: the sizes of `N` values of a layout's element type, and the copy from
/// the source values to the uniform block.
// Port of: src/gpu/graphite/UniformManager.h#L487-L539 (chrome/m156)
#[derive(Clone, Copy)]
struct LayoutTraits {
    n: usize,
    half: bool,
}

impl LayoutTraits {
    // Port of: src/gpu/graphite/UniformManager.h#L492 (chrome/m156), `kElemSize`
    const fn elem_size(self) -> usize {
        if self.half { 2 } else { 4 }
    }

    // Port of: src/gpu/graphite/UniformManager.h#L493 (chrome/m156), `kSize`
    const fn size(self) -> usize {
        self.n * self.elem_size()
    }

    // Port of: src/gpu/graphite/UniformManager.h#L494 (chrome/m156), `kAlign`
    fn align(self) -> usize {
        to_usize(next_pow2(to_i32(self.n))) * self.elem_size()
    }

    // Reads `size()` bytes from `src` (as `N` floats when converting to half) and copies or
    // converts (float to half) the `N` values into `dst`. Does not add any padding.
    // Port of: src/gpu/graphite/UniformManager.h#L498-L518 (chrome/m156), `Copy`
    fn copy(self, src: &[u8], dst: &mut [u8]) {
        if self.half {
            let read = |i: usize| {
                let j = 4 * i;
                f32::from_ne_bytes([src[j], src[j + 1], src[j + 2], src[j + 3]])
            };
            // The lanes are padded to the next power of two, as skvx::Vec<NextPow2(N), float>;
            // the 4th lane of a vec3 is 0 and is not copied.
            let halves: [u16; 4] = match self.n {
                1 => {
                    let h = to_half(VxVec::<1, f32>([read(0)])).0;
                    [h[0], 0, 0, 0]
                }
                2 => {
                    let h = to_half(VxVec::<2, f32>([read(0), read(1)])).0;
                    [h[0], h[1], 0, 0]
                }
                3 => to_half(VxVec::<4, f32>([read(0), read(1), read(2), 0.0])).0,
                _ => to_half(VxVec::<4, f32>([read(0), read(1), read(2), read(3)])).0,
            };
            for (i, h) in halves.iter().take(self.n).enumerate() {
                dst[2 * i..2 * i + 2].copy_from_slice(&h.to_ne_bytes());
            }
        } else {
            let size = self.size();
            dst[..size].copy_from_slice(&src[..size]);
        }
    }
}
