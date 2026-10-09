// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRasterPipelineOpContexts.h

//! The op contexts (`SkRasterPipelineContexts`), as Rust types.
//!
//! Skia passes every context as a `void*`. Here each [`Stage`](super::Stage) variant holds its
//! context by value, and every context type is `Copy`:
//!
//! - **Constant data** (matrices, colors, tables, gather sources, uniforms) is borrowed: `&'a T`,
//!   or held by value when it is at most 8 bytes.
//! - **Pixel memory** accessed at `(dx, dy)` and patched for the tail (`MemoryCtx`) is a
//!   [`MemoryCtx`]: a [`MemSlot`] whose bytes, row stride and origin are bound per run in
//!   [`MemoryBindings`](super::MemoryBindings). Skia's `MemoryCtx { pixels, stride }` lives in
//!   the binding (`MemView`), so a compiled program can be reused with new memory, as blitters
//!   do.
//! - **Other writable memory** (`load_src`/`store_src` buffers, `SkSL` slots, clip-shader
//!   coverage buffers) is a [`MemPtr`]: a byte offset into a bound slot. `SkSL`'s `SkRPOffset`s
//!   stay `u32` byte offsets from the program's base pointer (`set_base_pointer`), as in Skia.
//! - **Scratch state written by stages inside a context** (decal masks, sampler coordinates,
//!   mipmap colors, conical masks) uses [`Cell`], and contexts that a caller changes between
//!   runs of a compiled program (`scale_1_float`/`lerp_1_float` coverage) are `&'a Cell<T>`.
//!
//! The contexts of ops not ported yet (wave B, Phase 3) follow the C++ field by field; their
//! task may adjust the Rust representation of its own ops (the [`Stage`](super::Stage) variant
//! and the stage functions are the only users).

use core::cell::Cell;
use core::fmt;
use std::sync::Arc;

// Port of: src/core/SkRasterPipelineOpContexts.h#L27-L33 (chrome/m156)
/// `kMaxStride`: the largest number of pixels handled at a time (lowp `N` on Ml3/Ml4).
#[doc(alias = "kMaxStride")]
pub const MAX_STRIDE: usize = 16;
/// `kMaxStride_highp`: the largest highp `N` (Ml4).
#[doc(alias = "kMaxStride_highp")]
pub const MAX_STRIDE_HIGHP: usize = 16;
/// `kMaxScratchPerPatch`: bytes of scratch per `MemoryCtx` for tail pixels: room for
/// [`MAX_STRIDE_HIGHP`] pixels of the widest highp format (RGBA F32, 16 bytes) and
/// [`MAX_STRIDE`] pixels of the widest lowp format (RGBA 8888, 4 bytes).
#[doc(alias = "kMaxScratchPerPatch")]
pub const MAX_SCRATCH_PER_PATCH: usize = if MAX_STRIDE_HIGHP * 16 > MAX_STRIDE * 4 {
    MAX_STRIDE_HIGHP * 16
} else {
    MAX_STRIDE * 4
};

/// A memory binding slot: an index into [`MemoryBindings`](super::MemoryBindings).
///
/// Stages name writable memory by slot; the memory itself is bound when the program runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MemSlot(pub u16);

// Port of: src/core/SkRasterPipelineOpContexts.h#L36-L39 (chrome/m156)
/// `MemoryCtx`: pixel memory accessed at the current `(dx, dy)`, with tail patching.
///
/// Skia's `pixels` and `stride` are the bound [`MemView`](super::MemView)'s bytes, origin and
/// stride; two stages share a `MemoryCtx` (and its tail scratch buffer) exactly when they name
/// the same slot.
#[doc(alias = "SkRasterPipelineContexts::MemoryCtx")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MemoryCtx {
    pub slot: MemSlot,
}

impl MemoryCtx {
    /// The context bound to `slot`.
    #[must_use]
    pub const fn new(slot: MemSlot) -> MemoryCtx {
        MemoryCtx { slot }
    }
}

/// A pointer to writable memory that is not patched for the tail (Skia's `float* ptr`,
/// `int32_t* dst`, `std::byte* base`, …): `offset` bytes into the memory bound to `slot`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MemPtr {
    pub slot: MemSlot,
    /// Byte offset from the start of the bound bytes.
    pub offset: u32,
}

impl MemPtr {
    /// `offset` bytes into `slot`.
    #[must_use]
    pub const fn new(slot: MemSlot, offset: u32) -> MemPtr {
        MemPtr { slot, offset }
    }

    /// `ptr + bytes`: this pointer moved forward by `bytes`.
    ///
    /// # Panics
    /// If the offset overflows `u32`.
    #[must_use]
    pub const fn add(self, bytes: u32) -> MemPtr {
        MemPtr {
            slot: self.slot,
            offset: self
                .offset
                .checked_add(bytes)
                .expect("MemPtr offset overflow"),
        }
    }
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L56-L62 (chrome/m156)
/// `MemoryCtxInfo`: how a pipeline uses one [`MemoryCtx`], which decides how its tail is
/// patched (`load`: copy the tail pixels into scratch first; `store`: copy them back after).
#[doc(alias = "SkRasterPipelineContexts::MemoryCtxInfo")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryCtxInfo {
    pub context: MemoryCtx,
    pub bytes_per_pixel: usize,
    pub load: bool,
    pub store: bool,
}

/// The bytes a [`GatherCtx`] samples, borrowed from the caller or kept alive by a shared owner.
///
/// skia-rust: `GatherCtx::pixels` is a `const void*` in Skia. A borrowed slice cannot be put in
/// an `ArenaAlloc` (arenas hold `'static` values), which is where a shader puts the contexts it
/// appends, so a shader hands the context a shared owner of the bytes ([`PixelBytes`]: a pixel
/// ref, a mipmap, ...) instead.
#[derive(Clone, Debug)]
pub enum GatherPixels<'a> {
    /// Bytes borrowed for `'a`.
    Slice(&'a [u8]),
    /// Bytes kept alive by a shared owner.
    Shared(Arc<dyn PixelBytes>),
}

impl GatherPixels<'_> {
    /// The pixels, from the pixel at `(0, 0)`.
    #[inline]
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        match self {
            GatherPixels::Slice(bytes) => bytes,
            GatherPixels::Shared(owner) => owner.bytes(),
        }
    }
}

impl<'a> From<&'a [u8]> for GatherPixels<'a> {
    fn from(bytes: &'a [u8]) -> GatherPixels<'a> {
        GatherPixels::Slice(bytes)
    }
}

impl<'a> From<&'a Vec<u8>> for GatherPixels<'a> {
    fn from(bytes: &'a Vec<u8>) -> GatherPixels<'a> {
        GatherPixels::Slice(bytes)
    }
}

/// A shared owner of the bytes of a [`GatherCtx`] (see [`GatherPixels::Shared`]).
pub trait PixelBytes: Send + Sync + fmt::Debug {
    /// The bytes, from the pixel at `(0, 0)` of the context.
    fn bytes(&self) -> &[u8];
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L71-L79 (chrome/m156)
/// `GatherCtx`: read-only pixels sampled at arbitrary coordinates (never patched).
#[doc(alias = "SkRasterPipelineContexts::GatherCtx")]
#[derive(Clone, Debug)]
pub struct GatherCtx<'a> {
    /// The pixels, from the pixel at `(0, 0)`.
    pub pixels: GatherPixels<'a>,
    /// Row stride in pixels.
    pub stride: i32,
    pub width: f32,
    pub height: f32,
    /// For `bicubic` and `bicubic_clamp_8888`.
    pub weights: [f32; 16],
    /// Controls whether pixel `i-1` or `i` is selected when the sample position is exactly `i`.
    pub round_down_at_integer: bool,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L82-L94 (chrome/m156)
/// `SamplerCtx`: state shared by `save_xy`, `accumulate`, `bilinear_*` and `bicubic_*`
/// (written by the stages within a chunk).
#[doc(alias = "SkRasterPipelineContexts::SamplerCtx")]
#[derive(Debug, Default)]
pub struct SamplerCtx {
    pub x: Cell<[f32; MAX_STRIDE_HIGHP]>,
    pub y: Cell<[f32; MAX_STRIDE_HIGHP]>,
    pub fx: Cell<[f32; MAX_STRIDE_HIGHP]>,
    pub fy: Cell<[f32; MAX_STRIDE_HIGHP]>,
    pub scalex: Cell<[f32; MAX_STRIDE_HIGHP]>,
    pub scaley: Cell<[f32; MAX_STRIDE_HIGHP]>,
    /// For `bicubic_[np][13][xy]`.
    pub weights: [f32; 16],
    pub wx: Cell<[[f32; MAX_STRIDE_HIGHP]; 4]>,
    pub wy: Cell<[[f32; MAX_STRIDE_HIGHP]; 4]>,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L96-L104 (chrome/m156)
/// `TileCtx`: `repeat_*`/`mirror_*` parameters.
#[doc(alias = "SkRasterPipelineContexts::TileCtx")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileCtx {
    pub scale: f32,
    /// Cache of `1/scale`.
    pub inv_scale: f32,
    /// `1` if `GatherCtx::round_down_at_integer`, otherwise `-1`.
    pub mirror_bias_dir: i32,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L106-L115 (chrome/m156)
/// `DecalTileCtx`: `decal_*` parameters; `mask` is written by `decal_*` and read by
/// `check_decal_mask` in the same chunk.
#[doc(alias = "SkRasterPipelineContexts::DecalTileCtx")]
#[derive(Debug, Default)]
pub struct DecalTileCtx {
    pub mask: Cell<[u32; MAX_STRIDE]>,
    pub limit_x: f32,
    pub limit_y: f32,
    /// Which edge of the interval is included (`limit_x` if `round_down_at_integer`, else 0).
    pub inclusive_edge_x: f32,
    pub inclusive_edge_y: f32,
}

/// `SkPerlinNoiseShaderType`.
#[doc(alias = "SkPerlinNoiseShaderType")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerlinNoiseShaderType {
    FractalNoise,
    Turbulence,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L117-L125 (chrome/m156)
/// `PerlinNoiseCtx`.
///
/// skia-rust: the lattice selector and noise tables are owned (Skia points into the shader's
/// painting data), so the context is `'static` and lives in `ArenaAlloc` like the others.
#[doc(alias = "SkRasterPipelineContexts::PerlinNoiseCtx")]
#[derive(Clone, Copy, Debug)]
pub struct PerlinNoiseCtx {
    pub noise_type: PerlinNoiseShaderType,
    pub base_frequency_x: f32,
    pub base_frequency_y: f32,
    pub stitch_data_in_x: f32,
    pub stitch_data_in_y: f32,
    pub stitching: bool,
    pub num_octaves: i32,
    /// `[256 values]`.
    pub lattice_selector: [u8; 256],
    /// `[4 channels][256 elements][vector of 2]`, flattened.
    pub noise_data: [u16; 4 * 256 * 2],
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L128-L144 (chrome/m156)
/// `MipmapCtx`: state used by `mipmap_linear_*`.
#[doc(alias = "SkRasterPipelineContexts::MipmapCtx")]
#[derive(Debug, Default)]
pub struct MipmapCtx {
    /// Original coordinates, saved before the base level logic.
    pub x: Cell<[f32; MAX_STRIDE_HIGHP]>,
    pub y: Cell<[f32; MAX_STRIDE_HIGHP]>,
    /// Base level color.
    pub r: Cell<[f32; MAX_STRIDE_HIGHP]>,
    pub g: Cell<[f32; MAX_STRIDE_HIGHP]>,
    pub b: Cell<[f32; MAX_STRIDE_HIGHP]>,
    pub a: Cell<[f32; MAX_STRIDE_HIGHP]>,
    /// Scale factors to transform base level coordinates to lower level coordinates.
    pub scale_x: f32,
    pub scale_y: f32,
    pub lower_weight: f32,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L146-L149 (chrome/m156)
/// `CoordClampCtx`.
#[doc(alias = "SkRasterPipelineContexts::CoordClampCtx")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoordClampCtx {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

/// The function of a [`CallbackCtx`]: called with the active pixels' `r,g,b,a` (interleaved per pixel,
/// `N` lanes each, as `store4` writes them) and the number of active pixels; whatever it leaves
/// in the array is loaded back.
pub type CallbackFn<'a> = dyn Fn(&mut [f32; 4 * MAX_STRIDE_HIGHP], usize) + 'a;

// Port of: src/core/SkRasterPipelineOpContexts.h#L151-L158 (chrome/m156)
/// `CallbackCtx`. (Skia's `read_from` pointer, which lets the callback point the pipeline at
/// another buffer, is replaced by writing the results into the array.)
#[doc(alias = "SkRasterPipelineContexts::CallbackCtx")]
#[derive(Clone, Copy)]
pub struct CallbackCtx<'a> {
    pub callback: &'a CallbackFn<'a>,
}

impl fmt::Debug for CallbackCtx<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallbackCtx").finish_non_exhaustive()
    }
}

// (`RewindCtx`, src/core/SkRasterPipelineOpContexts.h#L160-L171, has no Rust counterpart:
// `stack_checkpoint`/`stack_rewind` only bound the C++ stack, see `rp::tiers::highp`.)

/// `kRGBAChannels`.
pub const RGBA_CHANNELS: usize = 4;

// Port of: src/core/SkRasterPipelineOpContexts.h#L175-L180 (chrome/m156)
/// `GradientCtx`.
///
/// skia-rust: the factor, bias and `t` tables are owned (Skia points them into one arena
/// allocation), so the context is `'static` and can live in `skia_rust_core`'s `ArenaAlloc` like
/// the other contexts the shaders allocate. Each table has at least `max(stop_count + 1, 8)`
/// entries (the AVX2 gather of Skia's `gradient_lookup` reads eight).
#[doc(alias = "SkRasterPipelineContexts::GradientCtx")]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GradientCtx {
    pub stop_count: usize,
    pub factors: [Vec<f32>; RGBA_CHANNELS],
    pub biases: [Vec<f32>; RGBA_CHANNELS],
    pub ts: Vec<f32>,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L182-L185 (chrome/m156)
/// `EvenlySpaced2StopGradientCtx`.
#[doc(alias = "SkRasterPipelineContexts::EvenlySpaced2StopGradientCtx")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EvenlySpaced2StopGradientCtx {
    pub factor: [f32; RGBA_CHANNELS],
    pub bias: [f32; RGBA_CHANNELS],
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L187-L191 (chrome/m156)
/// `Conical2PtCtx`; `mask` is written by `mask_2pt_conical_*` and read by `apply_vector_mask`
/// (which takes the mask itself as its context).
#[doc(alias = "SkRasterPipelineContexts::Conical2PtCtx")]
#[derive(Debug, Default)]
pub struct Conical2PtCtx {
    pub mask: Cell<[u32; MAX_STRIDE_HIGHP]>,
    pub p0: f32,
    pub p1: f32,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L193-L196 (chrome/m156)
/// `UniformColorCtx`.
#[doc(alias = "SkRasterPipelineContexts::UniformColorCtx")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UniformColorCtx {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
    /// `[0,255]` in a 16-bit lane.
    pub rgba: [u16; 4],
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L198-L200 (chrome/m156)
/// `EmbossCtx`: two A8 memory contexts.
#[doc(alias = "SkRasterPipelineContexts::EmbossCtx")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmbossCtx {
    pub mul: MemoryCtx,
    pub add: MemoryCtx,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L202-L204 (chrome/m156)
/// `TablesCtx`.
///
/// skia-rust: the four tables are owned (copied in), not borrowed, so the context can live in a
/// pipeline's arena, which only holds `'static` values. The tables are 1 KiB in all.
#[doc(alias = "SkRasterPipelineContexts::TablesCtx")]
#[derive(Clone, Copy, Debug)]
pub struct TablesCtx {
    pub r: [u8; 256],
    pub g: [u8; 256],
    pub b: [u8; 256],
    pub a: [u8; 256],
}

/// `skcms_TransferFunction` (the context of `parametric`, `PQish`, `HLGish`, `HLGinvish`):
/// `g, a, b, c, d, e, f`. (A plain copy, since this crate sits below `skia-rust-skcms`.)
#[doc(alias = "skcms_TransferFunction")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TransferFunction {
    pub g: f32,
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

/// `SkRPOffset`: a byte offset from the `SkSL` base pointer.
#[doc(alias = "SkRPOffset")]
pub type RpOffset = u32;

// (`InitLaneMasksCtx`, src/core/SkRasterPipelineOpContexts.h#L208-L210, holds only the
// pipeline's tail pointer; the tail is part of the interpreter state here, so `init_lane_masks`
// has no context.)

// Port of: src/core/SkRasterPipelineOpContexts.h#L212-L215 (chrome/m156)
/// `ConstantCtx`.
#[doc(alias = "SkRasterPipelineContexts::ConstantCtx")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstantCtx {
    pub value: i32,
    pub dst: RpOffset,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L217-L220 (chrome/m156)
/// `UniformCtx`. `src` is Skia's `const int32_t*`: the first of the uniform words, which live in
/// slot memory (the `SkSL` builder's uniform block is copied there by `appendStages`), so the
/// context borrows nothing and can live in an arena.
#[doc(alias = "SkRasterPipelineContexts::UniformCtx")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UniformCtx {
    pub dst: MemPtr,
    pub src: MemPtr,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L222-L225 (chrome/m156)
/// `BinaryOpCtx`.
#[doc(alias = "SkRasterPipelineContexts::BinaryOpCtx")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BinaryOpCtx {
    pub dst: RpOffset,
    pub src: RpOffset,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L227-L230 (chrome/m156)
/// `TernaryOpCtx`.
#[doc(alias = "SkRasterPipelineContexts::TernaryOpCtx")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TernaryOpCtx {
    pub dst: RpOffset,
    pub delta: RpOffset,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L232-L235 (chrome/m156)
/// `MatrixMultiplyCtx`.
#[doc(alias = "SkRasterPipelineContexts::MatrixMultiplyCtx")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatrixMultiplyCtx {
    pub dst: RpOffset,
    pub left_columns: u8,
    pub left_rows: u8,
    pub right_columns: u8,
    pub right_rows: u8,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L237-L244 (chrome/m156)
/// `SwizzleCtx`.
#[doc(alias = "SkRasterPipelineContexts::SwizzleCtx")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwizzleCtx {
    pub dst: RpOffset,
    /// Byte offsets (`4 * highp-stride * component-index`).
    pub offsets: [u8; 4],
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L246-L250 (chrome/m156)
/// `ShuffleCtx`.
#[doc(alias = "SkRasterPipelineContexts::ShuffleCtx")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShuffleCtx {
    pub ptr: MemPtr,
    pub count: i32,
    /// Byte offsets (`4 * highp-stride * component-index`).
    pub offsets: [u16; 16],
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L252-L256 (chrome/m156)
/// `SwizzleCopyCtx`.
#[doc(alias = "SkRasterPipelineContexts::SwizzleCopyCtx")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwizzleCopyCtx {
    pub dst: MemPtr,
    /// Must not overlap `dst`.
    pub src: MemPtr,
    /// Byte offsets (`4 * highp-stride * component-index`).
    pub offsets: [u16; 4],
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L258-L264 (chrome/m156)
/// `CopyIndirectCtx`.
#[doc(alias = "SkRasterPipelineContexts::CopyIndirectCtx")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CopyIndirectCtx {
    pub dst: MemPtr,
    pub src: MemPtr,
    /// Applies to `src` or `dst` depending on the op.
    pub indirect_offset: MemPtr,
    /// The indirect offset is clamped to this upper bound.
    pub indirect_limit: u32,
    /// The number of slots to copy.
    pub slots: u32,
}

/// `CopyIndirectCtx` as used by `copy_from_indirect_uniform_unmasked` (the source is uniform
/// data: `src` is the first of its scalar words, in slot memory, as for [`UniformCtx`]).
#[doc(alias = "SkRasterPipelineContexts::CopyIndirectCtx")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CopyIndirectUniformCtx {
    pub dst: MemPtr,
    pub src: MemPtr,
    pub indirect_offset: MemPtr,
    pub indirect_limit: u32,
    pub slots: u32,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L266-L268 (chrome/m156)
/// `SwizzleCopyIndirectCtx` (C++: derives from `CopyIndirectCtx`).
#[doc(alias = "SkRasterPipelineContexts::SwizzleCopyIndirectCtx")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwizzleCopyIndirectCtx {
    pub copy: CopyIndirectCtx,
    /// Byte offsets (`4 * highp-stride * component-index`).
    pub offsets: [u16; 4],
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L270-L272 (chrome/m156)
/// `BranchCtx`: the program-counter offset of a branch (relative to the branch stage).
///
/// Also the context of `branch_if_all_lanes_active` (Skia's `BranchIfAllLanesActiveCtx` adds
/// the pipeline's tail pointer, which is interpreter state here).
#[doc(alias = "SkRasterPipelineContexts::BranchCtx")]
#[doc(alias = "SkRasterPipelineContexts::BranchIfAllLanesActiveCtx")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BranchCtx {
    pub offset: i32,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L278-L281 (chrome/m156)
/// `BranchIfEqualCtx`.
#[doc(alias = "SkRasterPipelineContexts::BranchIfEqualCtx")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BranchIfEqualCtx {
    pub offset: i32,
    pub value: i32,
    /// `N` lanes of `int32_t`.
    pub ptr: MemPtr,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L283-L286 (chrome/m156)
/// `CaseOpCtx`.
#[doc(alias = "SkRasterPipelineContexts::CaseOpCtx")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaseOpCtx {
    pub expected_value: i32,
    /// Points to a pair of adjacent `I32`s: `{actualValue, defaultMask}`.
    pub offset: RpOffset,
}

/// `SkSL::TraceHook`: receives the trace ops' events.
#[doc(alias = "`SkSL`::TraceHook")]
pub trait TraceHook {
    /// `var(slot, val)`.
    fn var(&self, slot: i32, val: i32);
    /// `line(lineNum)`.
    fn line(&self, line_num: i32);
    /// `enter(fnIdx)`.
    fn enter(&self, fn_idx: i32);
    /// `exit(fnIdx)`.
    fn exit(&self, fn_idx: i32);
    /// `scope(delta)`.
    fn scope(&self, delta: i32);
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L288-L292 (chrome/m156)
/// `TraceFuncCtx`.
///
/// skia-rust: the trace hook is an `Arc` rather than the `SkSL::TraceHook*` of Skia. A context is
/// arena-allocated (`'static`), and its hook is owned by the program that appended it, so sharing
/// the handle keeps the context free of a lifetime without unsafe code.
#[doc(alias = "SkRasterPipelineContexts::TraceFuncCtx")]
#[derive(Clone)]
pub struct TraceFuncCtx {
    pub trace_mask: MemPtr,
    pub trace_hook: Arc<dyn TraceHook>,
    pub func_idx: i32,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L294-L298 (chrome/m156)
/// `TraceScopeCtx`.
#[doc(alias = "SkRasterPipelineContexts::TraceScopeCtx")]
#[derive(Clone)]
pub struct TraceScopeCtx {
    pub trace_mask: MemPtr,
    pub trace_hook: Arc<dyn TraceHook>,
    pub delta: i32,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L300-L304 (chrome/m156)
/// `TraceLineCtx`.
#[doc(alias = "SkRasterPipelineContexts::TraceLineCtx")]
#[derive(Clone)]
pub struct TraceLineCtx {
    pub trace_mask: MemPtr,
    pub trace_hook: Arc<dyn TraceHook>,
    pub line_number: i32,
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L306-L313 (chrome/m156)
/// `TraceVarCtx`.
#[doc(alias = "SkRasterPipelineContexts::TraceVarCtx")]
#[derive(Clone)]
pub struct TraceVarCtx {
    pub trace_mask: MemPtr,
    pub trace_hook: Arc<dyn TraceHook>,
    pub slot_idx: i32,
    pub num_slots: i32,
    pub data: MemPtr,
    /// If set, an offset applied to `data`.
    pub indirect_offset: Option<MemPtr>,
    /// The indirect offset is clamped to this upper bound.
    pub indirect_limit: u32,
}

macro_rules! debug_trace_ctx {
    ($($name:ident),*) => {$(
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("trace_mask", &self.trace_mask)
                    .finish_non_exhaustive()
            }
        }
    )*};
}
debug_trace_ctx!(TraceFuncCtx, TraceScopeCtx, TraceLineCtx, TraceVarCtx);
