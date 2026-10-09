// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/render/DynamicInstancesPatchAllocator.h

//! [`DynamicInstancesPatchAllocator`]: appends one patch instance per curve or wedge, with the
//! instance count of the template decided by the tessellation tolerances of all the instances.
//! Used by the tessellation steps (G7b).

use crate::gpu::buffer_writer::VertexWriter;
use crate::graphite::buffer::BindBufferInfo;
use crate::graphite::draw_writer::{DrawWriter, DynamicInstances, VertexCountProxy};
use crate::tessellate::linear_tolerances::LinearTolerances;
use crate::tessellate::patch_writer::PatchAllocator;

/// A fixed-count patch variant: how many vertices one instance of the template draws, given the
/// accumulated tolerances (the static `FixedCountVariant::VertexCount` of the C++ template).
// Port of: src/gpu/graphite/render/DynamicInstancesPatchAllocator.h#L15-L18 (chrome/m156), the
// FixedCountVariant protocol
pub trait FixedCountVariant {
    /// `FixedCountVariant::VertexCount(tolerances)`.
    fn vertex_count(tolerances: &LinearTolerances) -> u32;
}

/// The `LinearToleranceProxy` of `DynamicInstancesPatchAllocator`: accumulates the tolerances of
/// the appended patches, and reports the vertex count of the accumulated result.
// Port of: src/gpu/graphite/render/DynamicInstancesPatchAllocator.h#L43-L51 (chrome/m156)
pub struct LinearToleranceProxy<V: FixedCountVariant> {
    tolerances: LinearTolerances,
    variant: std::marker::PhantomData<V>,
}

impl<V: FixedCountVariant> std::fmt::Debug for LinearToleranceProxy<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinearToleranceProxy")
            .finish_non_exhaustive()
    }
}

impl<V: FixedCountVariant> Default for LinearToleranceProxy<V> {
    fn default() -> Self {
        Self {
            tolerances: LinearTolerances::default(),
            variant: std::marker::PhantomData,
        }
    }
}

impl<V: FixedCountVariant> VertexCountProxy for LinearToleranceProxy<V> {
    type Input = LinearTolerances;

    // `void operator <<(const tess::LinearTolerances& t) { fTolerances.accumulate(t); }`
    fn accumulate(&mut self, value: &LinearTolerances) {
        self.tolerances.accumulate(value);
    }

    // `operator uint32_t() const { return FixedCountVariant::VertexCount(fTolerances); }`
    fn count(&self) -> u32 {
        V::vertex_count(&self.tolerances)
    }
}

/// `DynamicInstancesPatchAllocator`: one `append` per patch, with the template count resolved
/// when the allocator is dropped.
// Port of: src/gpu/graphite/render/DynamicInstancesPatchAllocator.h#L20-L51 (chrome/m156)
#[doc(alias = "skgpu::graphite::DynamicInstancesPatchAllocator")]
pub struct DynamicInstancesPatchAllocator<'w, 'a, V: FixedCountVariant> {
    instances: DynamicInstances<'w, 'a, LinearToleranceProxy<V>>,
}

impl<V: FixedCountVariant> std::fmt::Debug for DynamicInstancesPatchAllocator<'_, '_, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicInstancesPatchAllocator")
            .field("instances", &self.instances)
            .finish()
    }
}

impl<'w, 'a, V: FixedCountVariant> DynamicInstancesPatchAllocator<'w, 'a, V> {
    /// `DynamicInstancesPatchAllocator(stride, writer, fixedVertexBuffer, fixedIndexBuffer,
    /// reserveCount)`. `stride` is the patch stride from the `PatchWriter`.
    // Port of: src/gpu/graphite/render/DynamicInstancesPatchAllocator.h#L23-L36 (chrome/m156)
    #[must_use]
    pub fn new(
        stride: usize,
        writer: &'w mut DrawWriter<'a>,
        fixed_vertex_buffer: BindBufferInfo,
        fixed_index_buffer: BindBufferInfo,
        reserve_count: u32,
    ) -> Self {
        debug_assert_eq!(stride, writer.append_stride() as usize);
        let mut instances = DynamicInstances::new(writer, fixed_vertex_buffer, fixed_index_buffer);
        // TODO (Skia): is it worth re-reserving large chunks after this preallocation is used up?
        // Appending one at a time may be fine, since it comes from a large vertex buffer anyway.
        instances.reserve(reserve_count);
        Self { instances }
    }

    /// `append(tolerances)`: appends one patch instance.
    // Port of: src/gpu/graphite/render/DynamicInstancesPatchAllocator.h#L38-L40 (chrome/m156)
    pub fn append(&mut self, tolerances: &LinearTolerances) -> VertexWriter<'_> {
        self.instances.append(tolerances, 1)
    }
}

impl<V: FixedCountVariant> PatchAllocator for DynamicInstancesPatchAllocator<'_, '_, V> {
    // `PatchWriter` always has space in a dynamic-instance buffer, so this never returns `None`.
    // Port of: src/gpu/tessellate/PatchWriter.h#L213-L220 (chrome/m156), `PatchAllocator::append`
    fn append(&mut self, tolerances: &LinearTolerances) -> Option<VertexWriter<'_>> {
        Some(DynamicInstancesPatchAllocator::append(self, tolerances))
    }
}
