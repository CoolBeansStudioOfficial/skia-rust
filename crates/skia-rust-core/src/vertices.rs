// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkVertices.h, src/core/SkVertices.cpp, src/core/SkVerticesPriv.h

//! `SkVertices`: an immutable set of vertex data that can be used with
//! [`Canvas::draw_vertices`](crate::canvas::Canvas::draw_vertices).
//!
//! A [`Vertices`] is made by [`Vertices::new_copy`] or by filling in a [`Builder`] and calling
//! [`Builder::detach`]. Triangle fans are converted to indexed triangles when built.
//!
//! skia-rust:
//! * Skia lays the arrays out in one manually sized allocation; here each array is a `Vec`, and
//!   the [`Vertices`] handle is an `Arc` (`sk_sp<SkVertices>`). The size computation of Skia's
//!   `Sizes` (which decides whether a builder is valid) is ported with `checked` arithmetic in
//!   place of `SkSafeMath`.
//! * `SkVerticesPriv` is [`vertices_priv`]; `encode`/`Decode` use the subset of
//!   `SkWriteBuffer`/`SkReadBuffer` that [`crate::write_buffer`] and [`crate::read_buffer`] port.
//! * `skia-safe` makes `new_copy` and `Builder::new` panic when Skia returns `nullptr`; here
//!   they return `Option` / an invalid [`Builder`], because Skia's own tests (`Vertices`,
//!   `Vertices_invalid`) rely on the `nullptr`. See `docs/API_MAPPING.md`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use bitflags::bitflags;

use crate::color::Color;
use crate::point::Point;
use crate::rect::Rect;

/// `SkVertices::VertexMode`: how the vertices are connected into triangles.
// Port of: include/core/SkVertices.h#L30-L36 (chrome/m156)
#[doc(alias = "SkVertices::VertexMode")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum VertexMode {
    /// `kTriangles_VertexMode`: every three vertices (or indices) make a triangle.
    Triangles,
    /// `kTriangleStrip_VertexMode`: each vertex after the first two makes a triangle with the
    /// previous two.
    TriangleStrip,
    /// `kTriangleFan_VertexMode`: each vertex after the first two makes a triangle with the
    /// first and the previous one.
    TriangleFan,
}

impl VertexMode {
    /// `kLast_VertexMode`.
    #[doc(alias = "kLast_VertexMode")]
    pub const LAST: VertexMode = VertexMode::TriangleFan;
}

bitflags! {
    /// `SkVertices::BuilderFlags`: which optional arrays a [`Builder`] allocates.
    // Port of: include/core/SkVertices.h#L62-L65 (chrome/m156)
    #[doc(alias = "SkVertices::BuilderFlags")]
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct BuilderFlags: u32 {
        /// `kHasTexCoords_BuilderFlag`.
        const HAS_TEX_COORDS = 1 << 0;
        /// `kHasColors_BuilderFlag`.
        const HAS_COLORS = 1 << 1;
    }
}

/// `SK_InvalidGenID`.
const INVALID_GEN_ID: u32 = 0;

// Port of: src/core/SkVertices.cpp#L27-L35 (chrome/m156)
fn next_id() -> u32 {
    static NEXT_ID: AtomicU32 = AtomicU32::new(1);

    loop {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        if id != INVALID_GEN_ID {
            return id;
        }
    }
}

/// `sizeof(SkVertices)` on the 64-bit targets Skia is built for: the reference count, the unique
/// ID, four array pointers, the bounds, the counts and the mode (8-byte aligned). It is only
/// used by [`Vertices::approximate_size`].
const SIZEOF_SK_VERTICES: usize = 72;

/// `SkVertices::Desc`.
// Port of: src/core/SkVertices.cpp#L37-L43 (chrome/m156)
#[derive(Copy, Clone)]
struct Desc {
    mode: VertexMode,
    vertex_count: i32,
    index_count: i32,
    has_texs: bool,
    has_colors: bool,
}

/// `SkVertices::Sizes`: the byte sizes of the arrays of a `Desc`.
// Port of: src/core/SkVertices.cpp#L45-L104 (chrome/m156)
#[derive(Copy, Clone, Default)]
struct Sizes {
    /// Size of the entire `SkVertices` allocation (object + arrays).
    total: usize,
    /// Size of all the data arrays (V + T + C + I): just their sum.
    arrays: usize,
    v_size: usize,
    t_size: usize,
    c_size: usize,
    i_size: usize,
    /// For indexed tri-fans this is the amount of space for indices needed in the builder before
    /// conversion to indexed triangles (or zero if not indexed or not a triangle fan).
    builder_tri_fan_i_size: usize,
}

impl Sizes {
    fn new(desc: &Desc) -> Sizes {
        // `SkSafeMath`: any overflow makes the sizes all zero (invalid).
        Self::checked(desc).unwrap_or_default()
    }

    fn checked(desc: &Desc) -> Option<Sizes> {
        const POINT: usize = size_of::<Point>();
        const COLOR: usize = size_of::<Color>();
        const INDEX: usize = size_of::<u16>();

        let vertex_count = usize::try_from(desc.vertex_count).ok()?;
        let index_count = usize::try_from(desc.index_count).ok()?;

        let v_size = vertex_count.checked_mul(POINT)?;
        let t_size = if desc.has_texs {
            vertex_count.checked_mul(POINT)?
        } else {
            0
        };
        let c_size = if desc.has_colors {
            vertex_count.checked_mul(COLOR)?
        } else {
            0
        };

        let mut builder_tri_fan_i_size = 0;
        let mut i_size = index_count.checked_mul(INDEX)?;
        if desc.mode == VertexMode::TriangleFan {
            let num_fan_tris: i64;
            if desc.index_count != 0 {
                builder_tri_fan_i_size = i_size;
                num_fan_tris = i64::from(desc.index_count) - 2;
            } else {
                num_fan_tris = i64::from(desc.vertex_count) - 2;
                // By forcing this to become indexed we are adding a constraint to the maximum
                // number of vertices.
                if desc.vertex_count > i32::from(u16::MAX) + 1 {
                    return None;
                }
            }
            if num_fan_tris <= 0 {
                return None;
            }
            i_size = usize::try_from(num_fan_tris).ok()?.checked_mul(3 * INDEX)?;
        }

        let total = SIZEOF_SK_VERTICES
            .checked_add(v_size.checked_add(t_size.checked_add(c_size.checked_add(i_size)?)?)?)?;

        Some(Sizes {
            total,
            arrays: v_size + t_size + c_size + i_size,
            v_size,
            t_size,
            c_size,
            i_size,
            builder_tri_fan_i_size,
        })
    }

    // Port of: src/core/SkVertices.cpp#L88-L91 (chrome/m156)
    fn is_valid(&self) -> bool {
        debug_assert!(self.total >= self.v_size);
        self.v_size > 0
    }
}

/// The data of a [`Vertices`] (the members of `SkVertices`).
// Port of: include/core/SkVertices.h#L120-L134 (chrome/m156)
#[derive(Debug)]
struct VerticesData {
    unique_id: u32,
    positions: Vec<Point>,
    texs: Option<Vec<Point>>,
    colors: Option<Vec<Color>>,
    /// `fIndices`; `None` (Skia's null) if there are no indices.
    indices: Option<Vec<u16>>,
    bounds: Rect,
    vertex_count: usize,
    index_count: usize,
    mode: VertexMode,
}

/// An immutable set of vertex data that can be used with
/// [`Canvas::draw_vertices`](crate::canvas::Canvas::draw_vertices) (`SkVertices`).
///
/// A cheaply clonable handle (`sk_sp<SkVertices>`).
// Port of: include/core/SkVertices.h#L26-L134 (chrome/m156)
#[doc(alias = "SkVertices")]
#[derive(Clone, Debug)]
pub struct Vertices(Arc<VerticesData>);

impl Vertices {
    /// Creates a vertices by copying the specified arrays. `texs` and `colors` may be `None`;
    /// `indices` is `None` or empty for non-indexed vertices. Returns `None` if the arrays do
    /// not make valid vertices (no vertices, a size overflow, or a triangle fan too small or
    /// large to be rewritten as indexed triangles). Indices larger than the last vertex are
    /// clamped to it.
    ///
    /// The vertex count is `positions.len()` and the index count `indices.len()`; `texs` and
    /// `colors`, if present, must have at least as many elements as `positions`.
    ///
    /// # Panics
    /// If `texs` or `colors` has fewer elements than `positions`.
    // Port of: src/core/SkVertices.cpp#L208-L260 (chrome/m156)
    #[doc(alias = "MakeCopy")]
    #[must_use]
    pub fn new_copy(
        mode: VertexMode,
        positions: &[Point],
        texs: Option<&[Point]>,
        colors: Option<&[Color]>,
        indices: Option<&[u16]>,
    ) -> Option<Vertices> {
        let vertex_count = positions.len();
        if let Some(texs) = texs {
            assert!(texs.len() >= vertex_count);
        }
        if let Some(colors) = colors {
            assert!(colors.len() >= vertex_count);
        }
        let indices = indices.unwrap_or(&[]);
        let index_count = indices.len();

        let desc = Desc {
            mode,
            vertex_count: i32::try_from(vertex_count).ok()?,
            index_count: i32::try_from(index_count).ok()?,
            has_texs: texs.is_some(),
            has_colors: colors.is_some(),
        };
        let mut builder = Builder::from_desc(&desc);
        if !builder.is_valid() {
            return None;
        }

        // `sk_careful_memcpy` of the arrays.
        builder.positions().copy_from_slice(positions);
        if let (Some(dst), Some(src)) = (builder.tex_coords(), texs) {
            dst.copy_from_slice(&src[..vertex_count]);
        }
        if let (Some(dst), Some(src)) = (builder.colors(), colors) {
            dst.copy_from_slice(&src[..vertex_count]);
        }

        // Ensure that indices are valid for the given vertex count. (The builder can update the
        // number of indices: a fan's indices are in the builder's intermediate array.)
        debug_assert!(vertex_count > 0);
        let max_index = u16::try_from(vertex_count - 1).expect("SkToU16(vertexCount - 1)");
        if let Some(dst) = builder.indices() {
            for (dst, &src) in dst.iter_mut().zip(indices) {
                *dst = src.min(max_index);
            }
        }

        builder.detach()
    }

    /// The unique ID of the vertices (`uniqueID`); never 0.
    #[doc(alias = "uniqueID")]
    #[must_use]
    pub fn unique_id(&self) -> u32 {
        self.0.unique_id
    }

    /// The bounds of the positions (`bounds`).
    #[must_use]
    pub fn bounds(&self) -> &Rect {
        &self.0.bounds
    }

    /// The approximate byte size of the vertices object (`approximateSize`).
    #[doc(alias = "approximateSize")]
    #[must_use]
    pub fn approximate_size(&self) -> usize {
        self.sizes().total
    }

    /// `getSizes`.
    // Port of: src/core/SkVertices.cpp#L266-L270 (chrome/m156)
    fn sizes(&self) -> Sizes {
        let sizes = Sizes::new(&Desc {
            mode: self.0.mode,
            // Both counts were checked to fit when the vertices were built.
            vertex_count: i32::try_from(self.0.vertex_count).unwrap_or(i32::MAX),
            index_count: i32::try_from(self.0.index_count).unwrap_or(i32::MAX),
            has_texs: self.0.texs.is_some(),
            has_colors: self.0.colors.is_some(),
        });
        debug_assert!(sizes.is_valid());
        sizes
    }

    /// The vertex mode (`SkVerticesPriv::mode`). Triangle fans are converted to triangles.
    #[must_use]
    pub fn mode(&self) -> VertexMode {
        self.0.mode
    }

    /// The number of vertices.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.0.vertex_count
    }

    /// The number of indices (0 if the vertices are not indexed).
    #[must_use]
    pub fn index_count(&self) -> usize {
        self.0.index_count
    }

    /// The vertex positions.
    #[must_use]
    pub fn positions(&self) -> &[Point] {
        &self.0.positions
    }

    /// The texture coordinates, if there are any.
    #[must_use]
    pub fn tex_coords(&self) -> Option<&[Point]> {
        self.0.texs.as_deref()
    }

    /// The vertex colors, if there are any.
    #[must_use]
    pub fn colors(&self) -> Option<&[Color]> {
        self.0.colors.as_deref()
    }

    /// The indices, if there are any.
    #[must_use]
    pub fn indices(&self) -> Option<&[u16]> {
        self.0.indices.as_deref()
    }

    /// True if the vertices have colors.
    #[must_use]
    pub fn has_colors(&self) -> bool {
        self.0.colors.is_some()
    }

    /// True if the vertices have texture coordinates.
    #[must_use]
    pub fn has_tex_coords(&self) -> bool {
        self.0.texs.is_some()
    }

    /// True if the vertices have indices.
    #[must_use]
    pub fn has_indices(&self) -> bool {
        self.0.indices.is_some()
    }
}

/// The functions of `SkVerticesPriv`, which gives access to `SkVertices` members that are not
/// part of the public API. (`skia-rust`'s [`Vertices`] exposes them as `skia-safe`'s does, so
/// these are the free-function forms the Skia tests use.)
///
/// `encode` and `Decode` serialize the vertices: `[packed (mode and which arrays) | vertex count
/// | index count | positions | texs | colors | indices]`, each array as a byte array.
// Port of: src/core/SkVerticesPriv.h#L23-L56 (chrome/m156)
#[doc(alias = "SkVerticesPriv")]
pub mod vertices_priv {
    use super::{Builder, Desc, Sizes, VertexMode, Vertices};
    use crate::color::Color;
    use crate::point::Point;
    use crate::read_buffer::ReadBuffer;
    use crate::safe_range::SafeRange;
    use crate::write_buffer::BinaryWriteBuffer;

    // Port of: src/core/SkVertices.cpp#L274-L279 (chrome/m156)
    // storage = packed | vertex_count | index_count | attr_count
    //           | pos[] | custom[] | texs[] | colors[] | indices[]
    const MODE_MASK: u32 = 0x0FF;
    const HAS_TEXS_MASK: u32 = 0x100;
    const HAS_COLORS_MASK: u32 = 0x200;

    /// `SkPicturePriv::kVerticesRemoveCustomData_Version`: older pictures wrote a custom data
    /// array.
    // Port of: src/core/SkPicturePriv.h#L153 (chrome/m156)
    const VERTICES_REMOVE_CUSTOM_DATA_VERSION: u32 = 86;

    fn points_to_bytes(points: &[Point]) -> Vec<u8> {
        points
            .iter()
            .flat_map(|p| [p.x, p.y])
            .flat_map(f32::to_ne_bytes)
            .collect()
    }

    /// Writes the vertices to `buffer` (`SkVerticesPriv::encode`).
    ///
    /// # Panics
    /// Never: the sizes of existing vertices fit.
    // Port of: src/core/SkVertices.cpp#L281-L306 (chrome/m156)
    pub fn encode(vertices: &Vertices, buffer: &mut BinaryWriteBuffer) {
        // packed has room for additional flags in the future
        let mut packed = vertices.0.mode as u32;
        debug_assert_eq!(packed & !MODE_MASK, 0); // our mode fits in the mask bits
        if vertices.0.texs.is_some() {
            packed |= HAS_TEXS_MASK;
        }
        if vertices.0.colors.is_some() {
            packed |= HAS_COLORS_MASK;
        }

        debug_assert_eq!(vertices.sizes().builder_tri_fan_i_size, 0);

        // Header
        buffer.write_uint(packed);
        buffer.write_int(i32::try_from(vertices.0.vertex_count).expect("an i32 vertex count"));
        buffer.write_int(i32::try_from(vertices.0.index_count).expect("an i32 index count"));

        // Data arrays
        buffer.write_byte_array(&points_to_bytes(&vertices.0.positions));
        buffer.write_byte_array(
            &vertices
                .0
                .texs
                .as_deref()
                .map_or_else(Vec::new, points_to_bytes),
        );
        buffer.write_byte_array(
            &vertices
                .0
                .colors
                .iter()
                .flatten()
                .flat_map(|&c| u32::from(c).to_ne_bytes())
                .collect::<Vec<u8>>(),
        );
        // if index-count is odd, we won't be 4-bytes aligned, so we call the pad version
        buffer.write_byte_array(
            &vertices
                .0
                .indices
                .iter()
                .flatten()
                .flat_map(|i| i.to_ne_bytes())
                .collect::<Vec<u8>>(),
        );
    }

    /// Reads vertices written by [`encode`], or `None` (and an invalid buffer) if the data is
    /// not valid vertices (`SkVerticesPriv::Decode`).
    // Port of: src/core/SkVertices.cpp#L308-L376 (chrome/m156)
    #[must_use]
    pub fn decode(buffer: &mut ReadBuffer<'_>) -> Option<Vertices> {
        if let Some(verts) = decode_vertices(buffer) {
            return Some(verts);
        }
        buffer.validate(false);
        None
    }

    // Port of: src/core/SkVertices.cpp#L309-L368 (chrome/m156)
    fn decode_vertices(buffer: &mut ReadBuffer<'_>) -> Option<Vertices> {
        let mut safe = SafeRange::new();
        let has_custom_data = buffer.is_version_lt(VERTICES_REMOVE_CUSTOM_DATA_VERSION);

        let packed = buffer.read_uint();
        let vertex_count = safe.check_ge(buffer.read_int(), 0);
        let index_count = safe.check_ge(buffer.read_int(), 0);
        let attr_count = if has_custom_data {
            safe.check_ge(buffer.read_int(), 0)
        } else {
            0
        };
        let mode = match safe.check_le(u64::from(packed & MODE_MASK), VertexMode::LAST as u64) {
            0 => VertexMode::Triangles,
            1 => VertexMode::TriangleStrip,
            _ => VertexMode::TriangleFan,
        };
        let has_texs = packed & HAS_TEXS_MASK != 0;
        let has_colors = packed & HAS_COLORS_MASK != 0;

        // Check that the header fields and buffer are valid. If this is data with the
        // experimental custom attributes feature - we don't support that any more.
        // We also don't support serialized triangle-fan data. We stopped writing that long ago,
        // so it should never appear in valid encoded data.
        if !safe.ok() || !buffer.is_valid() || attr_count != 0 || mode == VertexMode::TriangleFan {
            return None;
        }

        let desc = Desc {
            mode,
            vertex_count,
            index_count,
            has_texs,
            has_colors,
        };
        let sizes = Sizes::new(&desc);
        if !sizes.is_valid() || sizes.arrays > buffer.available() {
            return None;
        }

        let mut builder = Builder::from_desc(&desc);
        if !builder.is_valid() {
            return None;
        }

        let mut bytes = vec![0u8; sizes.v_size];
        buffer.read_byte_array(&mut bytes);
        if bytes.len() == sizes.v_size && buffer.is_valid() {
            for (p, xy) in builder.positions().iter_mut().zip(bytes.as_chunks::<8>().0) {
                *p = Point::new(
                    f32::from_ne_bytes([xy[0], xy[1], xy[2], xy[3]]),
                    f32::from_ne_bytes([xy[4], xy[5], xy[6], xy[7]]),
                );
            }
        }
        if has_custom_data {
            let (_, custom_data_size) = buffer.skip_byte_array();
            if custom_data_size != 0 {
                return None;
            }
        }
        let mut bytes = vec![0u8; sizes.t_size];
        buffer.read_byte_array(&mut bytes);
        if let Some(texs) = builder.tex_coords() {
            for (p, xy) in texs.iter_mut().zip(bytes.as_chunks::<8>().0) {
                *p = Point::new(
                    f32::from_ne_bytes([xy[0], xy[1], xy[2], xy[3]]),
                    f32::from_ne_bytes([xy[4], xy[5], xy[6], xy[7]]),
                );
            }
        }
        let mut bytes = vec![0u8; sizes.c_size];
        buffer.read_byte_array(&mut bytes);
        if let Some(colors) = builder.colors() {
            for (c, argb) in colors.iter_mut().zip(bytes.as_chunks::<4>().0) {
                *c = Color::from(u32::from_ne_bytes(*argb));
            }
        }
        let mut bytes = vec![0u8; sizes.i_size];
        buffer.read_byte_array(&mut bytes);
        if let Some(indices) = builder.indices() {
            for (i, ib) in indices.iter_mut().zip(bytes.as_chunks::<2>().0) {
                *i = u16::from_ne_bytes(*ib);
            }
        }

        if !buffer.is_valid() {
            return None;
        }

        if index_count > 0 {
            // validate that the indices are in range
            let vertex_count = u32::try_from(vertex_count).expect("checked to be non-negative");
            for &index in builder.indices()?.iter() {
                if u32::from(index) >= vertex_count {
                    return None;
                }
            }
        }

        builder.detach()
    }

    /// `SkVerticesPriv::mode`.
    #[must_use]
    pub fn mode(v: &Vertices) -> VertexMode {
        v.mode()
    }

    /// `SkVerticesPriv::hasColors`.
    #[must_use]
    pub fn has_colors(v: &Vertices) -> bool {
        v.has_colors()
    }

    /// `SkVerticesPriv::hasTexCoords`.
    #[must_use]
    pub fn has_tex_coords(v: &Vertices) -> bool {
        v.has_tex_coords()
    }

    /// `SkVerticesPriv::hasIndices`.
    #[must_use]
    pub fn has_indices(v: &Vertices) -> bool {
        v.has_indices()
    }

    /// `SkVerticesPriv::vertexCount`.
    #[must_use]
    pub fn vertex_count(v: &Vertices) -> usize {
        v.vertex_count()
    }

    /// `SkVerticesPriv::indexCount`.
    #[must_use]
    pub fn index_count(v: &Vertices) -> usize {
        v.index_count()
    }

    /// `SkVerticesPriv::positions`.
    #[must_use]
    pub fn positions(v: &Vertices) -> &[Point] {
        v.positions()
    }

    /// `SkVerticesPriv::texCoords`.
    #[must_use]
    pub fn tex_coords(v: &Vertices) -> Option<&[Point]> {
        v.tex_coords()
    }

    /// `SkVerticesPriv::colors`.
    #[must_use]
    pub fn colors(v: &Vertices) -> Option<&[Color]> {
        v.colors()
    }

    /// `SkVerticesPriv::indices`.
    #[must_use]
    pub fn indices(v: &Vertices) -> Option<&[u16]> {
        v.indices()
    }
}

/// Fills in the arrays of a [`Vertices`] (`SkVertices::Builder`).
///
/// A builder is invalid ([`is_valid`](Self::is_valid) is false) if the counts cannot make
/// vertices (see [`Vertices::new_copy`]); an invalid builder has empty arrays.
// Port of: include/core/SkVertices.h#L66-L85 (chrome/m156)
#[doc(alias = "SkVertices::Builder")]
#[derive(Debug)]
pub struct Builder {
    /// `fVertices`: a partially complete object, only completed in [`detach`](Self::detach).
    /// `None` if the builder is invalid.
    vertices: Option<VerticesData>,
    /// `fIntermediateFanIndices`: extra storage for intermediate vertices in the case where the
    /// client specifies indexed triangle fans. These get converted to indexed triangles when
    /// the builder is finalized.
    intermediate_fan_indices: Option<Vec<u16>>,
}

impl Builder {
    /// A builder of `vertex_count` vertices and `index_count` indices, with the optional arrays
    /// in `flags` (`Builder(mode, vertexCount, indexCount, flags)`). The arrays are zeroed.
    ///
    /// Unlike `skia-safe`, this does not panic if the builder is invalid: check
    /// [`is_valid`](Self::is_valid).
    // Port of: src/core/SkVertices.cpp#L106-L115 (chrome/m156)
    #[must_use]
    pub fn new(
        mode: VertexMode,
        vertex_count: usize,
        index_count: usize,
        flags: BuilderFlags,
    ) -> Builder {
        let has_texs = flags.contains(BuilderFlags::HAS_TEX_COORDS);
        let has_colors = flags.contains(BuilderFlags::HAS_COLORS);
        match (i32::try_from(vertex_count), i32::try_from(index_count)) {
            (Ok(vertex_count), Ok(index_count)) => Builder::from_desc(&Desc {
                mode,
                vertex_count,
                index_count,
                has_texs,
                has_colors,
            }),
            _ => Builder {
                vertices: None,
                intermediate_fan_indices: None,
            },
        }
    }

    // Port of: src/core/SkVertices.cpp#L117-L151 (chrome/m156)
    fn from_desc(desc: &Desc) -> Builder {
        let sizes = Sizes::new(desc);
        if !sizes.is_valid() {
            return Builder {
                vertices: None,
                intermediate_fan_indices: None,
            };
        }

        let intermediate_fan_indices = (sizes.builder_tri_fan_i_size != 0)
            .then(|| vec![0u16; sizes.builder_tri_fan_i_size / size_of::<u16>()]);

        let vertex_count = sizes.v_size / size_of::<Point>();
        let vertices = VerticesData {
            unique_id: 0,
            positions: vec![Point::default(); vertex_count],
            texs: (sizes.t_size != 0).then(|| vec![Point::default(); vertex_count]),
            colors: (sizes.c_size != 0).then(|| vec![Color::default(); vertex_count]),
            indices: (sizes.i_size != 0).then(|| vec![0u16; sizes.i_size / size_of::<u16>()]),
            bounds: Rect::new_empty(),
            vertex_count: usize::try_from(desc.vertex_count).unwrap_or(0),
            index_count: usize::try_from(desc.index_count).unwrap_or(0),
            mode: desc.mode,
        };
        // We defer assigning bounds and the unique ID until detach() is called.
        Builder {
            vertices: Some(vertices),
            intermediate_fan_indices,
        }
    }

    /// True if the builder will make vertices (`isValid`).
    #[doc(alias = "isValid")]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.vertices.is_some()
    }

    /// The positions to fill in (`positions`); empty if the builder is invalid.
    // Port of: src/core/SkVertices.cpp#L184-L186 (chrome/m156)
    pub fn positions(&mut self) -> &mut [Point] {
        match &mut self.vertices {
            Some(v) => &mut v.positions,
            None => &mut [],
        }
    }

    /// The indices to fill in (`indices`), or `None` if there are none. For an indexed triangle
    /// fan these are the fan's indices, as passed to the builder (`index_count` of them).
    // Port of: src/core/SkVertices.cpp#L196-L204 (chrome/m156)
    pub fn indices(&mut self) -> Option<&mut [u16]> {
        let v = self.vertices.as_mut()?;
        if let Some(fan) = &mut self.intermediate_fan_indices {
            return Some(fan);
        }
        v.indices.as_deref_mut()
    }

    /// The texture coordinates to fill in (`texCoords`), or `None` if there are none.
    // Port of: src/core/SkVertices.cpp#L188-L190 (chrome/m156)
    #[doc(alias = "texCoords")]
    pub fn tex_coords(&mut self) -> Option<&mut [Point]> {
        self.vertices.as_mut()?.texs.as_deref_mut()
    }

    /// The colors to fill in (`colors`), or `None` if there are none.
    // Port of: src/core/SkVertices.cpp#L192-L194 (chrome/m156)
    pub fn colors(&mut self) -> Option<&mut [Color]> {
        self.vertices.as_mut()?.colors.as_deref_mut()
    }

    /// Makes the vertices (`detach`). `None` if the builder is invalid.
    ///
    /// skia-rust: Skia returns null on later calls; here `detach` consumes the builder.
    ///
    /// # Panics
    /// Never: a triangle fan always has indices, and its indices fit in 16 bits.
    // Port of: src/core/SkVertices.cpp#L153-L182 (chrome/m156)
    #[must_use]
    pub fn detach(mut self) -> Option<Vertices> {
        let mut v = self.vertices.take()?;
        v.bounds = Rect::bounds_or_empty(&v.positions);
        if v.mode == VertexMode::TriangleFan {
            let indices = v.indices.as_mut().expect("a fan always has indices");
            if let Some(temp) = &self.intermediate_fan_indices {
                debug_assert_ne!(v.index_count, 0);
                for t in 0..v.index_count - 2 {
                    indices[3 * t] = temp[0];
                    indices[3 * t + 1] = temp[t + 1];
                    indices[3 * t + 2] = temp[t + 2];
                }
                v.index_count = 3 * (v.index_count - 2);
            } else {
                debug_assert_eq!(v.index_count, 0);
                for t in 0..v.vertex_count - 2 {
                    indices[3 * t] = 0;
                    indices[3 * t + 1] = u16::try_from(t + 1).expect("SkToU16(t + 1)");
                    indices[3 * t + 2] = u16::try_from(t + 2).expect("SkToU16(t + 2)");
                }
                v.index_count = 3 * (v.vertex_count - 2);
            }
            v.mode = VertexMode::Triangles;
        }
        v.unique_id = next_id();
        Some(Vertices(Arc::new(v)))
    }
}

#[cfg(test)]
#[allow(clippy::cast_precision_loss)] // small loop counters
mod tests {
    use super::*;

    fn pts(n: usize) -> Vec<Point> {
        (0..n)
            .map(|i| Point::new(i as f32, 2.0 * i as f32))
            .collect()
    }

    #[test]
    fn builder_validity_of_fans() {
        let b = |vertices, indices| {
            Builder::new(
                VertexMode::TriangleFan,
                vertices,
                indices,
                BuilderFlags::HAS_COLORS,
            )
            .is_valid()
        };
        // The most vertices that can be rewritten as indexed triangles with 16 bit indices.
        assert!(b(usize::from(u16::MAX) + 1, 0));
        assert!(!b(usize::from(u16::MAX) + 2, 0));
        // Two vertices can't make a triangle.
        assert!(!b(2, 0));
        // Minimum number of indices to be rewritten.
        assert!(b(10, 3));
        assert!(!b(10, 2));
        // No vertices at all.
        assert!(!Builder::new(VertexMode::Triangles, 0, 0, BuilderFlags::empty()).is_valid());
    }

    #[test]
    fn fans_become_triangles() {
        let p = pts(5);
        let v = Vertices::new_copy(VertexMode::TriangleFan, &p, None, None, None).unwrap();
        assert_eq!(v.mode(), VertexMode::Triangles);
        assert_eq!(v.index_count(), 9);
        assert_eq!(v.indices().unwrap(), &[0, 1, 2, 0, 2, 3, 0, 3, 4]);

        // An indexed fan: the indices are the fan's, and are clamped to the last vertex.
        let v = Vertices::new_copy(
            VertexMode::TriangleFan,
            &p,
            None,
            None,
            Some(&[4, 3, 100, 1]),
        )
        .unwrap();
        assert_eq!(v.mode(), VertexMode::Triangles);
        assert_eq!(v.index_count(), 6);
        assert_eq!(v.indices().unwrap(), &[4, 3, 4, 4, 4, 1]);
    }

    #[test]
    fn copies_and_reports_bounds() {
        let p = pts(4);
        let t: Vec<Point> = p.iter().map(|p| Point::new(p.x + 1.0, p.y)).collect();
        let c = [Color::RED, Color::GREEN, Color::BLUE, Color::BLACK];
        let v =
            Vertices::new_copy(VertexMode::TriangleStrip, &p, Some(&t), Some(&c), None).unwrap();
        assert_eq!(v.mode(), VertexMode::TriangleStrip);
        assert_eq!(v.vertex_count(), 4);
        assert_eq!(v.index_count(), 0);
        assert!(v.indices().is_none());
        assert_eq!(v.positions(), &p[..]);
        assert_eq!(v.tex_coords().unwrap(), &t[..]);
        assert_eq!(v.colors().unwrap(), &c[..]);
        assert_eq!(*v.bounds(), Rect::new(0.0, 0.0, 3.0, 6.0));
        assert_ne!(v.unique_id(), 0);

        let v2 = Vertices::new_copy(VertexMode::Triangles, &p, None, None, None).unwrap();
        assert_ne!(v.unique_id(), v2.unique_id());
        assert!(v2.tex_coords().is_none() && v2.colors().is_none());
        assert!(v.approximate_size() > v2.approximate_size());
    }

    #[test]
    fn builder_arrays_are_zeroed_and_sized() {
        let mut b = Builder::new(
            VertexMode::Triangles,
            5,
            9,
            BuilderFlags::HAS_COLORS | BuilderFlags::HAS_TEX_COORDS,
        );
        assert!(b.is_valid());
        assert_eq!(b.positions().len(), 5);
        assert_eq!(b.tex_coords().unwrap().len(), 5);
        assert_eq!(b.colors().unwrap().len(), 5);
        assert_eq!(b.indices().unwrap().len(), 9);
        assert!(b.positions().iter().all(|&p| p == Point::default()));

        let mut b = Builder::new(VertexMode::Triangles, 5, 0, BuilderFlags::empty());
        assert!(b.tex_coords().is_none() && b.colors().is_none() && b.indices().is_none());
        assert!(b.detach().is_some());

        let mut invalid = Builder::new(VertexMode::Triangles, 0, 0, BuilderFlags::empty());
        assert!(invalid.positions().is_empty() && invalid.indices().is_none());
        assert!(invalid.detach().is_none());
    }
}
