// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRasterPipeline.{h,cpp}

//! `SkRasterPipeline`: the builder of raster pipelines (design §2.7).
//!
//! A [`RasterPipeline<'a>`] records [`Stage`]s whose contexts live for `'a`: constant data the
//! caller owns, or data the appenders allocate in an [`ArenaAlloc`] that outlives the pipeline
//! (Skia's `SkArenaAlloc*` arguments). Writable memory is never borrowed; it is named by a
//! [`MemoryCtx`] / [`MemPtr`] and bound per run with [`MemoryBindings`].
//!
//! The stages, their contexts and the tier code live in `skia_rust_simd::rp`.
//!
//! ```
//! use skia_rust_core::arena_alloc::ArenaAlloc;
//! use skia_rust_core::color_type::ColorType;
//! use skia_rust_core::raster_pipeline::{MemSlot, MemoryCtx, RasterPipeline, Stage};
//!
//! let alloc = ArenaAlloc::new();
//! let dst = MemoryCtx::new(MemSlot(0));
//! let mut p = RasterPipeline::new();
//! p.append_constant_color(&alloc, &[0.25, 0.5, 0.75, 1.0]);
//! p.append_load_dst(ColorType::BGRA8888, dst);
//! p.append(Stage::Srcover);
//! p.append_store(ColorType::BGRA8888, dst);
//! assert_eq!(
//!     p.to_string(), // `dump()`'s text
//!     "SkRasterPipeline, 6 stages\n\tuniform_color\n\tload_8888_dst\n\tswap_rb_dst\n\
//!      \tsrcover\n\tswap_rb\n\tstore_8888\n\n",
//! );
//! ```

use core::fmt;

use crate::arena_alloc::ArenaAlloc;
use crate::color::Color4f;
use crate::color_type::ColorType;
use crate::image_info::ImageInfo;
use crate::image_info_priv::color_type_is_normalized;
use crate::matrix::{Matrix, TypeMask};
use skia_rust_simd::rp::{MemoryCtxInfo, Program, ProgramDesc, add_memory_context};
use skia_rust_skcms::TfType;

pub use skia_rust_simd::rp::contexts::{self, TransferFunction, UniformColorCtx};
pub use skia_rust_simd::rp::{MemPtr, MemSlot, MemView, MemoryBindings, MemoryCtx, Op, Stage};

/// The sRGB transfer function as a stage context (`*skcms_sRGB_TransferFunction()`).
#[allow(clippy::cast_possible_truncation)] // mirrors skcms's (float)(double expression) casts
pub static SRGB_TRANSFER_FUNCTION: TransferFunction = TransferFunction {
    g: 2.4,
    a: (1.0f64 / 1.055) as f32,
    b: (0.055f64 / 1.055) as f32,
    c: (1.0f64 / 12.92) as f32,
    d: 0.04045,
    e: 0.0,
    f: 0.0,
};

/// The inverse sRGB transfer function as a stage context
/// (`*skcms_sRGB_Inverse_TransferFunction()`).
#[allow(clippy::excessive_precision)] // skcms's literals kept verbatim
pub static SRGB_INVERSE_TRANSFER_FUNCTION: TransferFunction = TransferFunction {
    g: 0.416_666_657,
    a: 1.137_283_325,
    b: -0.0,
    c: 12.920_000_076,
    d: 0.003_130_805,
    e: -0.054_969_788,
    f: -0.0,
};

/// A stage context copy of a skcms transfer function (the two types have the same fields; the
/// stage crate sits below skcms).
#[must_use]
pub fn transfer_function_ctx(tf: &skia_rust_skcms::TransferFunction) -> TransferFunction {
    TransferFunction {
        g: tf.g,
        a: tf.a,
        b: tf.b,
        c: tf.c,
        d: tf.d,
        e: tf.e,
        f: tf.f,
    }
}

// Port of: src/core/SkRasterPipeline.h#L72-L172 (chrome/m156)
/// `SkRasterPipeline`: a list of [`Stage`]s, built for the current CPU tier when run or
/// compiled.
///
/// Skia's arena (`SkArenaAlloc`) is the lifetime `'a` of the contexts the stages borrow; the
/// appenders that allocate contexts take the arena as Skia's do. Skia's linked `StageList` is a
/// `Vec` here, so [`reset`](Self::reset) keeps its storage and rebuilding a pipeline in the
/// same `RasterPipeline` does not allocate.
#[doc(alias = "SkRasterPipeline")]
#[derive(Clone, Debug, Default)]
pub struct RasterPipeline<'a> {
    /// `fStages`, oldest first.
    stages: Vec<Stage<'a>>,
    /// `fMemoryCtxInfos`.
    memory_ctx_infos: Vec<MemoryCtxInfo>,
    /// `fRewindCtx != nullptr`. (The context itself has no counterpart: `stack_rewind` only
    /// bounds the C++ stack.)
    has_rewind: bool,
    /// `gForceHighPrecisionRasterPipeline` (a global in Skia, set by tests).
    force_high_precision: bool,
}

// Port of: src/core/SkRasterPipeline.cpp#L36-L47 (chrome/m156)
impl<'a> RasterPipeline<'a> {
    /// An empty pipeline.
    #[must_use]
    pub fn new() -> RasterPipeline<'a> {
        RasterPipeline {
            stages: Vec::new(),
            memory_ctx_infos: Vec::new(),
            has_rewind: false,
            force_high_precision: false,
        }
    }

    /// `reset()`: removes every stage (keeping the storage).
    pub fn reset(&mut self) {
        // We intentionally leave the alloc alone here; we don't own it.
        self.has_rewind = false;
        self.stages.clear();
        self.memory_ctx_infos.clear();
    }

    /// Forces highp pipelines (Skia's `gForceHighPrecisionRasterPipeline`).
    pub fn set_force_high_precision(&mut self, force: bool) {
        self.force_high_precision = force;
    }

    // Port of: src/core/SkRasterPipeline.cpp#L49-L62 (chrome/m156)
    /// `append(op, ctx)`: appends a stage.
    ///
    /// Some ops have dedicated appenders that pick the op from their context
    /// ([`append_constant_color`](Self::append_constant_color),
    /// [`append_set_rgb`](Self::append_set_rgb),
    /// [`append_transfer_function`](Self::append_transfer_function),
    /// [`append_stack_rewind`](Self::append_stack_rewind)); appending those ops directly is a
    /// debug assertion failure, as in Skia.
    pub fn append(&mut self, stage: Stage<'a>) {
        debug_assert!(!matches!(stage, Stage::UniformColor(_))); // Please use appendConstantColor().
        debug_assert!(!matches!(stage, Stage::UnboundedUniformColor(_))); // Please use appendConstantColor().
        debug_assert!(!matches!(stage, Stage::SetRgb(_))); // Please use appendSetRGB().
        debug_assert!(!matches!(stage, Stage::UnboundedSetRgb(_))); // Please use appendSetRGB().
        debug_assert!(!matches!(stage, Stage::Parametric(_))); // Please use appendTransferFunction().
        debug_assert!(!matches!(stage, Stage::Gamma(_))); // Please use appendTransferFunction().
        debug_assert!(!matches!(stage, Stage::PQish(_))); // Please use appendTransferFunction().
        debug_assert!(!matches!(stage, Stage::HLGish(_))); // Please use appendTransferFunction().
        debug_assert!(!matches!(stage, Stage::HLGinvish(_))); // Please use appendTransferFunction().
        debug_assert!(!matches!(stage, Stage::StackCheckpoint)); // Please use appendStackRewind().
        debug_assert!(!matches!(stage, Stage::StackRewind)); // Please use appendStackRewind().
        self.unchecked_append(stage);
    }

    // Port of: src/core/SkRasterPipeline.cpp#L72-L183 (chrome/m156)
    /// `uncheckedAppend`: appends a stage without the `append` checks, registering the
    /// [`MemoryCtx`]s it loads from or stores to (for tail patching). (The tail pointer that
    /// Skia hands `init_lane_masks` and `branch_if_all_lanes_active` is interpreter state.)
    pub fn unchecked_append(&mut self, stage: Stage<'a>) {
        self.stages.push(stage);
        skia_rust_simd::rp::register_memory_ctxs(&mut self.memory_ctx_infos, &stage);
    }

    // Port of: src/core/SkRasterPipeline.cpp#L191-L239 (chrome/m156)
    /// `extend(src)`: appends all of `src`'s stages.
    ///
    /// A rewind context in `src` gives this pipeline one too (Skia rewrites `src`'s rewind,
    /// lane-mask and branch contexts to this pipeline's; here those are interpreter state).
    pub fn extend(&mut self, src: &RasterPipeline<'a>) {
        if src.empty() {
            return;
        }
        // Create a rewind context if `src` has one already, but we don't.
        if src.has_rewind {
            self.has_rewind = true;
        }
        self.stages.extend_from_slice(&src.stages);
        for info in &src.memory_ctx_infos {
            add_memory_context(
                &mut self.memory_ctx_infos,
                info.context,
                info.bytes_per_pixel,
                info.load,
                info.store,
            );
        }
    }

    // Port of: src/core/SkRasterPipeline.cpp#L241-L249 (chrome/m156)
    /// `GetOpName(op)`: Skia's name for `op`.
    #[doc(alias = "GetOpName")]
    #[must_use]
    pub fn get_op_name(op: Op) -> &'static str {
        op.name()
    }

    /// `getStageList()`: the stages, oldest first (Skia's list runs newest first).
    #[doc(alias = "getStageList")]
    #[must_use]
    pub fn stages(&self) -> &[Stage<'a>] {
        &self.stages
    }

    /// `getNumStages()`.
    #[doc(alias = "getNumStages")]
    #[must_use]
    pub fn num_stages(&self) -> usize {
        self.stages.len()
    }

    // Port of: src/core/SkRasterPipeline.cpp#L251-L262 (chrome/m156)
    /// `dump()`: prints the stage list (the [`Display`](fmt::Display) output) to stderr, as
    /// `SkDebugf` does.
    pub fn dump(&self) {
        eprint!("{self}");
    }

    // Port of: src/core/SkRasterPipeline.cpp#L264-L279 (chrome/m156)
    /// `appendSetRGB(alloc, rgb)`: sets `r, g, b` to a constant, keeping `a` (`set_rgb` if all
    /// three are in `[0, 1]`, otherwise the highp-only `unbounded_set_rgb`).
    #[doc(alias = "appendSetRGB")]
    pub fn append_set_rgb(&mut self, alloc: &'a ArenaAlloc, rgb: &[f32; 3]) {
        let arg = alloc.make([rgb[0], rgb[1], rgb[2]]);

        let in_range = 0.0 <= rgb[0]
            && rgb[0] <= 1.0
            && 0.0 <= rgb[1]
            && rgb[1] <= 1.0
            && 0.0 <= rgb[2]
            && rgb[2] <= 1.0;
        let stage = if in_range {
            Stage::SetRgb(arg)
        } else {
            Stage::UnboundedSetRgb(arg)
        };

        self.unchecked_append(stage);
    }

    /// `appendSetRGB(alloc, const SkColor4f&)`: [`append_set_rgb`](Self::append_set_rgb) with
    /// `color`'s `r, g, b`.
    #[doc(alias = "appendSetRGB")]
    pub fn append_set_rgb_color4f(&mut self, alloc: &'a ArenaAlloc, color: &Color4f) {
        self.append_set_rgb(alloc, &[color.r, color.g, color.b]);
    }

    // Port of: src/core/SkRasterPipeline.cpp#L281-L310 (chrome/m156)
    /// `appendConstantColor(alloc, rgba)`: sets the source color to a constant: `black_color` /
    /// `white_color` for opaque black / white, `uniform_color` (lowp capable) for a premultiplied
    /// color in range, otherwise the highp-only `unbounded_uniform_color`.
    #[doc(alias = "appendConstantColor")]
    #[allow(clippy::float_cmp)] // Skia compares the components exactly
    pub fn append_constant_color(&mut self, alloc: &'a ArenaAlloc, rgba: &[f32; 4]) {
        // r,g,b might be outside [0,1], but alpha should probably always be in [0,1].
        debug_assert!(0.0 <= rgba[3] && rgba[3] <= 1.0);

        if rgba[0] == 0.0 && rgba[1] == 0.0 && rgba[2] == 0.0 && rgba[3] == 1.0 {
            self.append(Stage::BlackColor);
        } else if rgba[0] == 1.0 && rgba[1] == 1.0 && rgba[2] == 1.0 && rgba[3] == 1.0 {
            self.append(Stage::WhiteColor);
        } else {
            let mut ctx = UniformColorCtx {
                r: rgba[0],
                g: rgba[1],
                b: rgba[2],
                a: rgba[3],
                rgba: [0; 4],
            };

            // uniform_color requires colors in range and can go lowp,
            // while unbounded_uniform_color supports out-of-range colors too but not lowp.
            if 0.0 <= rgba[0]
                && rgba[0] <= rgba[3]
                && 0.0 <= rgba[1]
                && rgba[1] <= rgba[3]
                && 0.0 <= rgba[2]
                && rgba[2] <= rgba[3]
            {
                // To make loads more direct, we store 8-bit values in 16-bit slots.
                // (skvx: `color * 255.0f + 0.5f`, two roundings, then `(uint16_t)` truncation.)
                ctx.rgba = rgba.map(|c| to_u16(c * 255.0 + 0.5));
                self.unchecked_append(Stage::UniformColor(alloc.make(ctx)));
            } else {
                self.unchecked_append(Stage::UnboundedUniformColor(alloc.make(ctx)));
            }
        }
    }

    /// `appendConstantColor(alloc, const SkColor4f&)`:
    /// [`append_constant_color`](Self::append_constant_color) with `color`'s components.
    #[doc(alias = "appendConstantColor")]
    pub fn append_constant_color4f(&mut self, alloc: &'a ArenaAlloc, color: &Color4f) {
        self.append_constant_color(alloc, &[color.r, color.g, color.b, color.a]);
    }

    // Port of: src/core/SkRasterPipeline.cpp#L312-L341 (chrome/m156)
    /// `appendMatrix(alloc, matrix)`: maps the `(x, y)` coordinates in `r, g` by `matrix`, with
    /// the cheapest stage for its type (nothing for the identity).
    #[doc(alias = "appendMatrix")]
    pub fn append_matrix(&mut self, alloc: &'a ArenaAlloc, matrix: &Matrix) {
        let mt = matrix.get_type();

        if mt == TypeMask::IDENTITY {
            return;
        }
        if mt == TypeMask::TRANSLATE {
            // (The two floats fit in the stage: no arena copy.)
            self.append(Stage::MatrixTranslate([
                matrix.translate_x(),
                matrix.translate_y(),
            ]));
        } else if (mt | (TypeMask::SCALE | TypeMask::TRANSLATE))
            == (TypeMask::SCALE | TypeMask::TRANSLATE)
        {
            let scale_trans = alloc.make([
                matrix.scale_x(),
                matrix.scale_y(),
                matrix.translate_x(),
                matrix.translate_y(),
            ]);
            self.append(Stage::MatrixScaleTranslate(scale_trans));
        } else {
            let mut storage = [0.0; 9];
            matrix.get_9(&mut storage);
            if matrix.has_perspective() {
                self.append(Stage::MatrixPerspective(alloc.make(storage)));
            } else {
                // note: asAffine and the 2x3 stage really only need 6 entries
                // (so only those are kept: the stage context is `&[f32; 6]`)
                let affine: [f32; 6] = core::array::from_fn(|i| storage[i]);
                self.append(Stage::Matrix2x3(alloc.make(affine)));
            }
        }
    }

    // Port of: src/core/SkRasterPipeline.cpp#L343-L411 (chrome/m156)
    /// `appendLoad(ct, ctx)`: loads pixels of color type `ct` into `r, g, b, a`.
    #[doc(alias = "appendLoad")]
    pub fn append_load(&mut self, ct: ColorType, ctx: MemoryCtx) {
        use ColorType as C;
        use Stage as S;
        match ct {
            C::Unknown => debug_assert!(false, "appendLoad: kUnknown_SkColorType"),

            C::Alpha8 => self.append(S::LoadA8(ctx)),
            C::A16UNorm => self.append(S::LoadA16(ctx)),
            C::A16Float => self.append(S::LoadAf16(ctx)),
            C::RGB565 => self.append(S::Load565(ctx)),
            C::ARGB4444 => self.append(S::Load4444(ctx)),
            C::R8G8UNorm => self.append(S::LoadRg88(ctx)),
            C::R16UNorm => self.append(S::LoadR16(ctx)),
            C::R16Float => self.append(S::LoadRf16(ctx)),
            C::R16G16UNorm => self.append(S::LoadRg1616(ctx)),
            C::R16G16Float => self.append(S::LoadRgf16(ctx)),
            C::RGBA8888 => self.append(S::Load8888(ctx)),
            C::RGBA1010102 => self.append(S::Load1010102(ctx)),
            C::R16G16B16A16UNorm => self.append(S::Load16161616(ctx)),
            C::RGBAF16Norm | C::RGBAF16 => self.append(S::LoadF16(ctx)),
            C::RGBAF32 => self.append(S::LoadF32(ctx)),
            C::RGBA10x6 => self.append(S::Load10x6(ctx)),

            C::Gray8 => {
                self.append(S::LoadA8(ctx));
                self.append(S::AlphaToGray);
            }
            C::R8UNorm => {
                self.append(S::LoadA8(ctx));
                self.append(S::AlphaToRed);
            }
            C::RGB888x => {
                self.append(S::Load8888(ctx));
                self.append(S::ForceOpaque);
            }
            C::BGRA1010102 => {
                self.append(S::Load1010102(ctx));
                self.append(S::SwapRb);
            }
            C::RGB101010x => {
                self.append(S::Load1010102(ctx));
                self.append(S::ForceOpaque);
            }
            C::BGR101010x => {
                self.append(S::Load1010102(ctx));
                self.append(S::ForceOpaque);
                self.append(S::SwapRb);
            }
            C::BGRA10101010XR => {
                self.append(S::Load10101010Xr(ctx));
                self.append(S::SwapRb);
            }
            C::BGR101010xXR => {
                self.append(S::Load1010102Xr(ctx));
                self.append(S::ForceOpaque);
                self.append(S::SwapRb);
            }
            C::RGBF16F16F16x => {
                self.append(S::LoadF16(ctx));
                self.append(S::ForceOpaque);
            }
            C::BGRA8888 => {
                self.append(S::Load8888(ctx));
                self.append(S::SwapRb);
            }
            C::SRGBA8888 => {
                self.append(S::Load8888(ctx));
                self.append_transfer_function(&SRGB_TRANSFER_FUNCTION);
            }
        }
    }

    // Port of: src/core/SkRasterPipeline.cpp#L413-L485 (chrome/m156)
    /// `appendLoadDst(ct, ctx)`: loads pixels of color type `ct` into `dr, dg, db, da`.
    #[doc(alias = "appendLoadDst")]
    pub fn append_load_dst(&mut self, ct: ColorType, ctx: MemoryCtx) {
        use ColorType as C;
        use Stage as S;
        match ct {
            C::Unknown => debug_assert!(false, "appendLoadDst: kUnknown_SkColorType"),

            C::Alpha8 => self.append(S::LoadA8Dst(ctx)),
            C::A16UNorm => self.append(S::LoadA16Dst(ctx)),
            C::A16Float => self.append(S::LoadAf16Dst(ctx)),
            C::RGB565 => self.append(S::Load565Dst(ctx)),
            C::ARGB4444 => self.append(S::Load4444Dst(ctx)),
            C::R8G8UNorm => self.append(S::LoadRg88Dst(ctx)),
            C::R16UNorm => self.append(S::LoadR16Dst(ctx)),
            C::R16Float => self.append(S::LoadRf16Dst(ctx)),
            C::R16G16UNorm => self.append(S::LoadRg1616Dst(ctx)),
            C::R16G16Float => self.append(S::LoadRgf16Dst(ctx)),
            C::RGBA8888 => self.append(S::Load8888Dst(ctx)),
            C::RGBA1010102 => self.append(S::Load1010102Dst(ctx)),
            C::R16G16B16A16UNorm => self.append(S::Load16161616Dst(ctx)),
            C::RGBAF16Norm | C::RGBAF16 => self.append(S::LoadF16Dst(ctx)),
            C::RGBAF32 => self.append(S::LoadF32Dst(ctx)),
            C::RGBA10x6 => self.append(S::Load10x6Dst(ctx)),

            C::Gray8 => {
                self.append(S::LoadA8Dst(ctx));
                self.append(S::AlphaToGrayDst);
            }
            C::R8UNorm => {
                self.append(S::LoadA8Dst(ctx));
                self.append(S::AlphaToRedDst);
            }
            C::RGB888x => {
                self.append(S::Load8888Dst(ctx));
                self.append(S::ForceOpaqueDst);
            }
            C::BGRA1010102 => {
                self.append(S::Load1010102Dst(ctx));
                self.append(S::SwapRbDst);
            }
            C::RGB101010x => {
                self.append(S::Load1010102Dst(ctx));
                self.append(S::ForceOpaqueDst);
            }
            C::BGR101010x => {
                self.append(S::Load1010102Dst(ctx));
                self.append(S::ForceOpaqueDst);
                self.append(S::SwapRbDst);
            }
            C::BGR101010xXR => {
                self.append(S::Load1010102XrDst(ctx));
                self.append(S::ForceOpaqueDst);
                self.append(S::SwapRbDst);
            }
            C::BGRA10101010XR => {
                self.append(S::Load10101010XrDst(ctx));
                self.append(S::SwapRbDst);
            }
            C::RGBF16F16F16x => {
                self.append(S::LoadF16Dst(ctx));
                self.append(S::ForceOpaqueDst);
            }
            C::BGRA8888 => {
                self.append(S::Load8888Dst(ctx));
                self.append(S::SwapRbDst);
            }
            C::SRGBA8888 => {
                // TODO: We could remove the double-swap if we had _dst versions of all the TF
                // stages
                self.append(S::Load8888Dst(ctx));
                self.append(S::SwapSrcDst);
                self.append_transfer_function(&SRGB_TRANSFER_FUNCTION);
                self.append(S::SwapSrcDst);
            }
        }
    }

    // Port of: src/core/SkRasterPipeline.cpp#L487-L552 (chrome/m156)
    /// `appendStore(ct, ctx)`: stores `r, g, b, a` as pixels of color type `ct`.
    #[doc(alias = "appendStore")]
    pub fn append_store(&mut self, ct: ColorType, ctx: MemoryCtx) {
        use ColorType as C;
        use Stage as S;
        match ct {
            C::Unknown => debug_assert!(false, "appendStore: kUnknown_SkColorType"),

            C::Alpha8 => self.append(S::StoreA8(ctx)),
            C::R8UNorm => self.append(S::StoreR8(ctx)),
            C::A16UNorm => self.append(S::StoreA16(ctx)),
            C::A16Float => self.append(S::StoreAf16(ctx)),
            C::RGB565 => self.append(S::Store565(ctx)),
            C::ARGB4444 => self.append(S::Store4444(ctx)),
            C::R8G8UNorm => self.append(S::StoreRg88(ctx)),
            C::R16UNorm => self.append(S::StoreR16(ctx)),
            C::R16Float => self.append(S::StoreRf16(ctx)),
            C::R16G16UNorm => self.append(S::StoreRg1616(ctx)),
            C::R16G16Float => self.append(S::StoreRgf16(ctx)),
            C::RGBA8888 => self.append(S::Store8888(ctx)),
            C::RGBA1010102 => self.append(S::Store1010102(ctx)),
            C::R16G16B16A16UNorm => self.append(S::Store16161616(ctx)),
            C::RGBAF16Norm | C::RGBAF16 => self.append(S::StoreF16(ctx)),
            C::RGBAF32 => self.append(S::StoreF32(ctx)),
            C::RGBA10x6 => self.append(S::Store10x6(ctx)),

            C::RGB888x => {
                self.append(S::ForceOpaque);
                self.append(S::Store8888(ctx));
            }
            C::BGRA1010102 => {
                self.append(S::SwapRb);
                self.append(S::Store1010102(ctx));
            }
            C::RGB101010x => {
                self.append(S::ForceOpaque);
                self.append(S::Store1010102(ctx));
            }
            C::BGR101010x => {
                self.append(S::ForceOpaque);
                self.append(S::SwapRb);
                self.append(S::Store1010102(ctx));
            }
            C::BGR101010xXR => {
                self.append(S::ForceOpaque);
                self.append(S::SwapRb);
                self.append(S::Store1010102Xr(ctx));
            }
            C::RGBF16F16F16x => {
                self.append(S::ForceOpaque);
                self.append(S::StoreF16(ctx));
            }
            C::BGRA10101010XR => {
                self.append(S::SwapRb);
                self.append(S::Store10101010Xr(ctx));
            }
            C::Gray8 => {
                self.append(S::Bt709LuminanceOrLumaToAlpha);
                self.append(S::StoreA8(ctx));
            }
            C::BGRA8888 => {
                self.append(S::SwapRb);
                self.append(S::Store8888(ctx));
            }
            C::SRGBA8888 => {
                self.append_transfer_function(&SRGB_INVERSE_TRANSFER_FUNCTION);
                self.append(S::Store8888(ctx));
            }
        }
    }

    // Port of: src/core/SkRasterPipeline.cpp#L554-L570 (chrome/m156)
    /// `appendTransferFunction(tf)`: applies `tf` to `r, g, b`, with the stage for its skcms
    /// type (`gamma_` for a pure power function). An invalid `tf` is a debug assertion failure
    /// and appends nothing.
    #[doc(alias = "appendTransferFunction")]
    #[allow(clippy::float_cmp)] // Skia compares the coefficients exactly
    pub fn append_transfer_function(&mut self, tf: &'a TransferFunction) {
        let skcms_tf =
            skia_rust_skcms::TransferFunction::new(tf.g, tf.a, tf.b, tf.c, tf.d, tf.e, tf.f);
        match skcms_tf.tf_type() {
            TfType::SRGBish => {
                if tf.a == 1.0
                    && tf.b == 0.0
                    && tf.c == 0.0
                    && tf.d == 0.0
                    && tf.e == 0.0
                    && tf.f == 0.0
                {
                    // (`gamma_` reads only `tf.g`, held by value.)
                    self.unchecked_append(Stage::Gamma(tf.g));
                } else {
                    self.unchecked_append(Stage::Parametric(tf));
                }
            }
            TfType::PQish => self.unchecked_append(Stage::PQish(tf)),
            TfType::HLGish => self.unchecked_append(Stage::HLGish(tf)),
            TfType::HLGinvish => self.unchecked_append(Stage::HLGinvish(tf)),
            TfType::Invalid | TfType::PQ | TfType::HLG => {
                debug_assert!(
                    false,
                    "appendTransferFunction: unsupported transfer function"
                );
            }
        }
    }

    // Port of: src/core/SkRasterPipeline.cpp#L572-L578 (chrome/m156)
    /// `appendClampIfNormalized(info)`: GPUs clamp all color channels to the limits of the
    /// format just before the blend step. To match that auto-clamp, the RP blitter uses this
    /// helper immediately before appending blending stages.
    #[doc(alias = "appendClampIfNormalized")]
    pub fn append_clamp_if_normalized(&mut self, info: &ImageInfo) {
        if color_type_is_normalized(info.color_type()) {
            self.unchecked_append(Stage::Clamp01);
        }
    }

    // Port of: src/core/SkRasterPipeline.cpp#L580-L585 (chrome/m156)
    /// `appendStackRewind()`: appends `stack_rewind` (and makes the pipeline highp, with a
    /// leading `stack_checkpoint`).
    #[doc(alias = "appendStackRewind")]
    pub fn append_stack_rewind(&mut self) {
        self.has_rewind = true;
        self.unchecked_append(Stage::StackRewind);
    }

    /// `empty()`.
    #[must_use]
    pub fn empty(&self) -> bool {
        self.stages.is_empty()
    }

    /// What [`Program`] needs to build this pipeline.
    fn desc(&self) -> ProgramDesc<'_, 'a> {
        ProgramDesc {
            stages: &self.stages,
            memory_ctx_infos: &self.memory_ctx_infos,
            has_rewind: self.has_rewind,
            force_highp: self.force_high_precision,
        }
    }

    // Port of: src/core/SkRasterPipeline.cpp#L737-L746 (chrome/m156)
    /// `stagesNeeded()`: the stages plus `just_return`, plus a `stack_checkpoint` if there is a
    /// rewind context.
    #[must_use]
    pub fn stages_needed(&self) -> usize {
        self.desc().stages_needed()
    }

    // Port of: src/core/SkRasterPipeline.cpp#L593-L609 (chrome/m156)
    /// Whether the pipeline would be built in lowp on `tier` (`buildLowpPipeline` succeeds):
    /// not forced highp, no rewind context, and every stage has a lowp implementation there.
    #[must_use]
    pub fn is_lowp(&self, tier: skia_rust_simd::Tier) -> bool {
        self.desc().is_lowp(tier)
    }

    // Port of: src/core/SkRasterPipeline.cpp#L748-L770 (chrome/m156)
    /// `run(x, y, w, h)`: builds the pipeline for the current CPU tier (with zeroed tail scratch
    /// buffers) and runs it over the `w × h` pixels at `(x, y)`. Allocation-free for pipelines
    /// of up to 31 stages and 2 memory contexts, like Skia's.
    ///
    /// # Panics
    /// If a stage's memory is not bound in `mem` or an access falls outside it, or a stage is not
    /// ported yet.
    pub fn run(&self, x: usize, y: usize, w: usize, h: usize, mem: &mut MemoryBindings<'_>) {
        if self.empty() {
            return;
        }
        self.desc()
            .run(skia_rust_simd::selection(), x, y, w, h, mem);
    }

    // Port of: src/core/SkRasterPipeline.cpp#L772-L797 (chrome/m156)
    /// `compile()`: builds the pipeline once. Running the result repeatedly reuses its tail
    /// scratch buffers, whose lanes past the tail keep their bytes between runs.
    #[must_use]
    pub fn compile(&self) -> CompiledPipeline<'a> {
        CompiledPipeline {
            program: (!self.empty())
                .then(|| Program::build(&self.desc(), skia_rust_simd::selection())),
        }
    }

    /// The `MemoryCtx`s the pipeline registers (`fMemoryCtxInfos`), in registration order.
    #[must_use]
    pub fn memory_ctx_infos(&self) -> &[MemoryCtxInfo] {
        &self.memory_ctx_infos
    }
}

/// `(uint16_t)f` for the `uniform_color` channels, which are in `[0.5, 255.5]` here (the
/// in-range test passed and alpha is in `[0, 1]`); an out-of-range alpha (a failed Skia debug
/// assertion, UB in C++) saturates.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // the C++ (uint16_t) cast
fn to_u16(f: f32) -> u16 {
    f as u16
}

// Port of: src/core/SkRasterPipeline.cpp#L251-L262 (chrome/m156)
impl fmt::Display for RasterPipeline<'_> {
    /// `dump()`'s text: `SkRasterPipeline, <n> stages`, then one tab-indented op name per stage,
    /// then an empty line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "SkRasterPipeline, {} stages", self.stages.len())?;
        for stage in &self.stages {
            writeln!(f, "\t{}", stage.op().name())?;
        }
        writeln!(f)
    }
}

/// The result of [`RasterPipeline::compile`] (Skia's `std::function<void(x, y, w, h)>`).
#[derive(Debug)]
pub struct CompiledPipeline<'a> {
    /// `None` for an empty pipeline (Skia returns a no-op function).
    program: Option<Program<'a>>,
}

impl CompiledPipeline<'_> {
    /// Runs the compiled pipeline over the `w × h` pixels at `(x, y)`.
    ///
    /// # Panics
    /// As [`RasterPipeline::run`].
    pub fn run(&mut self, x: usize, y: usize, w: usize, h: usize, mem: &mut MemoryBindings<'_>) {
        if let Some(program) = &mut self.program {
            program.run(x, y, w, h, mem);
        }
    }

    /// Whether the compiled pipeline runs its lowp stages.
    #[must_use]
    pub fn is_lowp(&self) -> bool {
        self.program.as_ref().is_some_and(Program::is_lowp)
    }
}

#[cfg(test)]
mod tests;
