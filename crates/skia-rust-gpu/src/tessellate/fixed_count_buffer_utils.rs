// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/gpu/tessellate/FixedCountBufferUtils.h and FixedCountBufferUtils.cpp (chrome/m156)

//! Fixed-count tessellation operates in three modes, two for filling paths, and one for stroking.
//! These modes may have additional sub-variations, but in terms of vertex buffer management, these
//! three categories are sufficient:
//!
//! - [`FixedCountCurves`]: for filling paths where just the curves are tessellated. Additional
//!   measures to fill space between the inner control points of the paths are needed.
//! - [`FixedCountWedges`]: for filling paths by tessellating the curves and adding an additional
//!   inline triangle with a shared vertex that all verbs connect to. Works with
//!   `PatchAttribs::FAN_POINT`.
//! - [`FixedCountStrokes`]: for stroking a path. Likely paired with
//!   `PatchAttribs::JOIN_CONTROL_POINT` and `PatchAttribs::STROKE_PARAMS`.
//!
//! The three types provide utility functions for heuristics to choose pre-allocation size when
//! accumulating instance attributes with a `PatchWriter`, and functions for creating
//! static/GPU-private vertex and index buffers that are used as the template for instanced
//! rendering.
//!
//! Skia's `VertexWriter` is `skia_rust_gpu::gpu::buffer_writer::VertexWriter`. The debug-only
//! `SkASSERT(vertexWriter.mark() == end)` checks are not ported.

// The C++ converts between the int, size_t and float types implicitly; the casts mirror that.
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_wrap)]
// SkASSERT is a debug_assert!, and the C++ int arithmetic is kept as written.
#![allow(clippy::manual_assert_eq)]
#![allow(clippy::manual_midpoint)]

use skia_rust_core::math_priv::prev_log2;

use crate::gpu::buffer_writer::VertexWriter;
use crate::tessellate::linear_tolerances::LinearTolerances;
use crate::tessellate::tessellation::{
    K_MAX_PARAMETRIC_SEGMENTS, K_MAX_RESOLVE_LEVEL, num_curve_triangles_at_resolve_level,
};

/// `sizeof(SkPoint)`.
const SIZE_OF_SK_POINT: usize = 8;

/// Writes the index buffer of a curve fan, starting at `base_index`. Connects the vertices with a
/// middle-out triangulation; see [`FixedCountCurves::write_vertex_buffer`] for the vertex order.
// Port of: src/gpu/tessellate/FixedCountBufferUtils.cpp#L22-L58 (chrome/m156), `write_curve_index_buffer_base_index`.
fn write_curve_index_buffer_base_index(
    vertex_writer: &mut VertexWriter<'_>,
    buffer_size: usize,
    base_index: u16,
) {
    let triangle_count = (buffer_size / (size_of::<u16>() * 3)) as i32;
    debug_assert!(triangle_count >= 1);
    let mut index_data: Vec<[u16; 3]> = Vec::with_capacity(triangle_count as usize);

    // Connect the vertices with a middle-out triangulation. Refer to
    // FixedCountCurves::write_vertex_buffer for the exact vertex ordering.
    //
    // Resolve level 1 is just a single triangle at T=[0, 1/2, 1].
    index_data.push([base_index, base_index + 2, base_index + 1]);
    // Index into `index_data` of the triangle that neighbors the current resolve level.
    let mut neighbor_in_last_resolve_level = 0_usize;

    // Resolve levels 2..maxResolveLevel
    let max_resolve_level = prev_log2((triangle_count + 1) as u32);
    let mut next_index = base_index + 3;
    debug_assert!(num_curve_triangles_at_resolve_level(max_resolve_level) == triangle_count);
    for resolve_level in 2..=max_resolve_level {
        let num_outer_triangles_in_resolve_level = 1_i32 << (resolve_level - 1);
        debug_assert!(num_outer_triangles_in_resolve_level % 2 == 0);
        let num_triangle_pairs_in_resolve_level = num_outer_triangles_in_resolve_level >> 1;
        for _ in 0..num_triangle_pairs_in_resolve_level {
            let neighbor = index_data[neighbor_in_last_resolve_level];
            // First triangle shares the left edge of "neighborInLastResolveLevel".
            index_data.push([neighbor[0], next_index, neighbor[1]]);
            next_index = next_index.wrapping_add(1);
            // Second triangle shares the right edge of "neighborInLastResolveLevel".
            index_data.push([neighbor[1], next_index, neighbor[2]]);
            next_index = next_index.wrapping_add(1);
            neighbor_in_last_resolve_level += 1;
        }
    }
    debug_assert!(index_data.len() == triangle_count as usize);
    debug_assert!(i32::from(next_index) == i32::from(base_index) + triangle_count + 2);
    vertex_writer.put(index_data.as_slice());
}

/// Fixed-count tessellation of filled curves: the template vertex and index buffers, and the
/// heuristics that size instance buffers for them.
// Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L35-L99 (chrome/m156), class `FixedCountCurves`.
#[derive(Debug)]
pub struct FixedCountCurves;

impl FixedCountCurves {
    /// A heuristic function for reserving instance attribute space before using a `PatchWriter`.
    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L40-L48 (chrome/m156), `PreallocCount`.
    #[must_use]
    pub fn prealloc_count(total_combined_path_verb_cnt: i32) -> i32 {
        // Over-allocate enough curves for 1 in 4 to chop. Every chop introduces 2 new patches:
        // another curve patch and a triangle patch that glues the two chops together,
        // i.e. + 2 * ((count + 3) / 4) == (count + 3) / 2
        let k_max_verb_count = i32::MAX >> 2;
        let total = total_combined_path_verb_cnt.min(k_max_verb_count);
        total + (total + 3) / 2
    }

    /// Converts the accumulated worst-case tolerances into an index count passed into an
    /// instanced, indexed draw function that uses the static vertex and index buffers.
    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L51-L58 (chrome/m156), `VertexCount`.
    #[must_use]
    pub fn vertex_count(tolerances: &LinearTolerances) -> i32 {
        // We should already chopped curves to make sure none needed a higher resolveLevel than
        // kMaxResolveLevel.
        let resolve_level = tolerances.required_resolve_level().min(K_MAX_RESOLVE_LEVEL);
        num_curve_triangles_at_resolve_level(resolve_level) * 3
    }

    /// Number of vertices in the static vertex buffer.
    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L60-L62 (chrome/m156), `VertexBufferVertexCount`.
    #[must_use]
    pub fn vertex_buffer_vertex_count() -> usize {
        (K_MAX_PARAMETRIC_SEGMENTS + 1) as usize
    }

    /// Bytes per vertex of the static vertex buffer.
    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L63-L65 (chrome/m156), `VertexBufferStride`.
    #[must_use]
    pub fn vertex_buffer_stride() -> usize {
        2 * size_of::<f32>()
    }

    /// Number of bytes to allocate for a buffer filled via [`Self::write_vertex_buffer`].
    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L68-L70 (chrome/m156), `VertexBufferSize`.
    #[must_use]
    pub fn vertex_buffer_size() -> usize {
        Self::vertex_buffer_vertex_count() * Self::vertex_buffer_stride()
    }

    /// As above but for the corresponding index buffer, written via [`Self::write_index_buffer`].
    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L73-L75 (chrome/m156), `IndexBufferSize`.
    #[must_use]
    pub fn index_buffer_size() -> usize {
        num_curve_triangles_at_resolve_level(K_MAX_RESOLVE_LEVEL) as usize * 3 * size_of::<u16>()
    }

    /// Writes the vertex buffer: `(resolveLevel, idx)` pairs in middle-out order.
    // Port of: src/gpu/tessellate/FixedCountBufferUtils.cpp#L93-L122 (chrome/m156), `FixedCountCurves::WriteVertexBuffer`.
    pub fn write_vertex_buffer(vertex_writer: &mut VertexWriter<'_>, buffer_size: usize) {
        debug_assert!(buffer_size >= SIZE_OF_SK_POINT * 2);
        let vertex_count = (buffer_size / SIZE_OF_SK_POINT) as i32;
        debug_assert!(vertex_count > 3);

        // Lay out the vertices in "middle-out" order:
        //
        // T= 0/1, 1/1,              ; resolveLevel=0
        //    1/2,                   ; resolveLevel=1  (0/2 and 2/2 are already in resolveLevel 0)
        //    1/4, 3/4,              ; resolveLevel=2  (2/4 is already in resolveLevel 1)
        //    1/8, 3/8, 5/8, 7/8,    ; resolveLevel=3  (2/8 and 6/8 are already in resolveLevel 2)
        //    ...                    ; resolveLevel=...
        //
        // Resolve level 0 is just the beginning and ending vertices.
        vertex_writer.put(&0.0_f32).put(&0.0_f32); // resolveLevel, idx
        vertex_writer.put(&0.0_f32).put(&1.0_f32); // resolveLevel, idx

        // Resolve levels 1..kMaxResolveLevel.
        let max_resolve_level = prev_log2((vertex_count - 1) as u32);
        debug_assert!((1 << max_resolve_level) + 1 == vertex_count);
        for resolve_level in 1..=max_resolve_level {
            let num_segments_in_resolve_level = 1_i32 << resolve_level;
            // Write out the odd vertices in this resolveLevel. The even vertices were already
            // written out in previous resolveLevels and will be indexed from there.
            for i in (1..num_segments_in_resolve_level).step_by(2) {
                vertex_writer.put(&(resolve_level as f32)).put(&(i as f32));
            }
        }
    }

    /// Writes the index buffer of the curve triangles, with a base index of 0.
    // Port of: src/gpu/tessellate/FixedCountBufferUtils.cpp#L124-L126 (chrome/m156), `FixedCountCurves::WriteIndexBuffer`.
    pub fn write_index_buffer(vertex_writer: &mut VertexWriter<'_>, buffer_size: usize) {
        write_curve_index_buffer_base_index(vertex_writer, buffer_size, 0);
    }
}

/// Fixed-count tessellation of filled wedges: like [`FixedCountCurves`] plus one fan vertex and
/// one fan triangle.
// Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L101-L140 (chrome/m156), class `FixedCountWedges`.
#[derive(Debug)]
pub struct FixedCountWedges;

impl FixedCountWedges {
    /// Like [`FixedCountCurves::prealloc_count`], for shaders with `PatchAttribs::FAN_POINT`.
    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L105-L112 (chrome/m156), `PreallocCount`.
    #[must_use]
    pub fn prealloc_count(total_combined_path_verb_cnt: i32) -> i32 {
        // Over-allocate enough wedges for 1 in 4 to chop, i.e., ceil(maxWedges * 5/4).
        let k_max_verb_count = i32::MAX >> 3;
        let total = total_combined_path_verb_cnt.min(k_max_verb_count);
        (total * 5 + 3) / 4
    }

    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L114-L119 (chrome/m156), `VertexCount`.
    #[must_use]
    pub fn vertex_count(tolerances: &LinearTolerances) -> i32 {
        // Emit 3 vertices per curve triangle, plus 3 more for the wedge fan triangle.
        let resolve_level = tolerances.required_resolve_level().min(K_MAX_RESOLVE_LEVEL);
        (num_curve_triangles_at_resolve_level(resolve_level) + 1) * 3
    }

    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L121-L123 (chrome/m156), `VertexBufferVertexCount`.
    #[must_use]
    pub fn vertex_buffer_vertex_count() -> usize {
        // + 1 for the fan vertex.
        (K_MAX_PARAMETRIC_SEGMENTS + 1) as usize + 1
    }

    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L125-L127 (chrome/m156), `VertexBufferStride`.
    #[must_use]
    pub fn vertex_buffer_stride() -> usize {
        2 * size_of::<f32>()
    }

    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L129-L131 (chrome/m156), `VertexBufferSize`.
    #[must_use]
    pub fn vertex_buffer_size() -> usize {
        Self::vertex_buffer_vertex_count() * Self::vertex_buffer_stride()
    }

    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L133-L136 (chrome/m156), `IndexBufferSize`.
    #[must_use]
    pub fn index_buffer_size() -> usize {
        // + 1 for the fan triangle.
        (num_curve_triangles_at_resolve_level(K_MAX_RESOLVE_LEVEL) as usize + 1)
            * 3
            * size_of::<u16>()
    }

    // Port of: src/gpu/tessellate/FixedCountBufferUtils.cpp#L128-L135 (chrome/m156), `FixedCountWedges::WriteVertexBuffer`.
    pub fn write_vertex_buffer(vertex_writer: &mut VertexWriter<'_>, buffer_size: usize) {
        debug_assert!(buffer_size >= SIZE_OF_SK_POINT);
        // Start out with the fan point. A negative resolve level indicates the fan point.
        vertex_writer.put(&-1.0_f32).put(&-1.0_f32); // resolveLevel, idx
        // The rest is the same as for curves.
        FixedCountCurves::write_vertex_buffer(vertex_writer, buffer_size - SIZE_OF_SK_POINT);
    }

    // Port of: src/gpu/tessellate/FixedCountBufferUtils.cpp#L137-L146 (chrome/m156), `FixedCountWedges::WriteIndexBuffer`.
    pub fn write_index_buffer(vertex_writer: &mut VertexWriter<'_>, buffer_size: usize) {
        debug_assert!(buffer_size >= size_of::<u16>() * 3);
        // Start out with the fan triangle.
        vertex_writer.put(&0_u16).put(&1_u16).put(&2_u16);
        // The rest is the same as for curves, with a baseIndex of 1.
        write_curve_index_buffer_base_index(vertex_writer, buffer_size - size_of::<u16>() * 3, 1);
    }
}

/// Fixed-count stroking: the vertex buffer is only needed when vertex IDs are not available as an
/// `SkSL` built-in. Unlike the curve and wedge variants, stroke drawing never relies on an index
/// buffer.
// Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L142-L193 (chrome/m156), class `FixedCountStrokes`.
#[derive(Debug)]
pub struct FixedCountStrokes;

impl FixedCountStrokes {
    /// Don't draw more vertices than can be indexed by a signed short. There are two vertices per
    /// edge, so 2^14 edges make 2^15 vertices.
    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L153 (chrome/m156), `kMaxEdges`.
    pub const K_MAX_EDGES: i32 = (1 << 14) - 1;
    /// `kMaxEdgesNoVertexIDs`.
    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L154 (chrome/m156), `kMaxEdgesNoVertexIDs`.
    pub const K_MAX_EDGES_NO_VERTEX_IDS: usize = 1024;

    /// Over-allocates enough patches for each stroke to chop once, and for 8 extra caps.
    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L156-L165 (chrome/m156), `PreallocCount`.
    #[must_use]
    pub fn prealloc_count(total_combined_path_verb_cnt: i32) -> i32 {
        let k_max_verb_count = i32::MAX >> 2;
        let total = total_combined_path_verb_cnt.min(k_max_verb_count);
        (total * 2) + 8 // caps
    }

    /// Does not account for falling back to [`Self::K_MAX_EDGES_NO_VERTEX_IDS`].
    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L168-L170 (chrome/m156), `VertexCount`.
    #[must_use]
    pub fn vertex_count(tolerances: &LinearTolerances) -> i32 {
        tolerances.required_stroke_edges().min(Self::K_MAX_EDGES) * 2
    }

    /// Each vertex is a single float (explicit id) and each edge is composed of two vertices.
    // Port of: src/gpu/tessellate/FixedCountBufferUtils.h#L172-L176 (chrome/m156), `VertexBufferSize`.
    #[must_use]
    pub fn vertex_buffer_size() -> usize {
        2 * Self::K_MAX_EDGES_NO_VERTEX_IDS * size_of::<f32>()
    }

    /// Initializes the fallback vertex buffer that should be bound when `sk_VertexID` is not
    /// supported.
    // Port of: src/gpu/tessellate/FixedCountBufferUtils.cpp#L148-L153 (chrome/m156), `FixedCountStrokes::WriteVertexBuffer`.
    pub fn write_vertex_buffer(vertex_writer: &mut VertexWriter<'_>, buffer_size: usize) {
        let edge_count = (buffer_size / (size_of::<f32>() * 2)) as i32;
        for i in 0..edge_count {
            vertex_writer.put(&(i as f32)).put(&((-i) as f32));
        }
    }
}
