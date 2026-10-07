// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The Rust replayer: runs a [`Case`] through `skia_rust_simd::rp` on one [`Selection`].
//!
//! [`build_stages`] turns each [`StageSpec`] into a [`Stage`] with a `match` generated from the op
//! table (`skia_rust_simd::rp_op_table!`): the context is converted by [`FromCtx`], implemented
//! once per Rust context type. A context type rp-diff cannot build yet returns an error naming
//! it; supporting it is one `FromCtx` impl here, a [`Ctx`] variant if none fits, and the matching
//! branch in the C++ driver's `make_ctx` (see `docs/PORTING.md` §12).

// The generated `match` names every context type of the op table.
#[allow(clippy::wildcard_imports)]
use skia_rust_simd::rp::contexts::*;

use core::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

use skia_rust_simd::rp::{MemPtr, MemSlot, MemView, MemoryBindings, MemoryCtx, Op, Program, Stage};
use skia_rust_simd::tier::Selection;

use crate::case::{Case, Ctx, StageSpec};

/// Builds a Rust context from its serialized form.
///
/// Borrowed contexts (`&'a T`) are leaked: rp-diff builds each case's stages once per process,
/// so the leak is bounded by the case list.
pub trait FromCtx<'a>: Sized {
    /// The context for `ctx`, or why it cannot be built.
    ///
    /// # Errors
    /// If `ctx` has the wrong kind or shape for this type.
    fn from_ctx(ctx: &Ctx) -> Result<Self, String>;
}

fn leak<'a, T>(v: T) -> &'a T {
    Box::leak(Box::new(v))
}

fn wrong(ty: &str, ctx: &Ctx) -> String {
    format!("context {ctx:?} does not fit {ty}")
}

fn floats<const N: usize>(ctx: &Ctx) -> Result<[f32; N], String> {
    match ctx {
        Ctx::F32(v) => <[f32; N]>::try_from(v.as_slice())
            .map_err(|_| format!("expected {N} floats, got {}", v.len())),
        other => Err(wrong("f32 array", other)),
    }
}

impl FromCtx<'_> for MemPtr {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::Ptr { slot, offset } => Ok(MemPtr::new(MemSlot(slot), offset)),
            ref other => Err(wrong("MemPtr", other)),
        }
    }
}

impl FromCtx<'_> for MemoryCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::Mem { slot } => Ok(MemoryCtx::new(MemSlot(slot))),
            ref other => Err(wrong("MemoryCtx", other)),
        }
    }
}

impl FromCtx<'_> for f32 {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        Ok(floats::<1>(ctx)?[0])
    }
}

impl<const N: usize> FromCtx<'_> for [f32; N] {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        floats::<N>(ctx)
    }
}

impl<'a, const N: usize> FromCtx<'a> for &'a [f32; N] {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        Ok(leak(floats::<N>(ctx)?))
    }
}

impl<'a> FromCtx<'a> for &'a Cell<f32> {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        Ok(leak(Cell::new(floats::<1>(ctx)?[0])))
    }
}

impl<'a> FromCtx<'a> for &'a TransferFunction {
    #[allow(clippy::many_single_char_names)] // skcms_TransferFunction's field names
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        let [g, a, b, c, d, e, f] = floats::<7>(ctx)?;
        Ok(leak(TransferFunction {
            g,
            a,
            b,
            c,
            d,
            e,
            f,
        }))
    }
}

impl FromCtx<'_> for [u8; 4] {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::U8x4(b) => Ok(b),
            ref other => Err(wrong("[u8; 4]", other)),
        }
    }
}

impl FromCtx<'_> for BranchCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::Branch { offset } => Ok(BranchCtx { offset }),
            ref other => Err(wrong("BranchCtx", other)),
        }
    }
}

impl<'a> FromCtx<'a> for &'a BranchIfEqualCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::BranchEq {
                offset,
                value,
                slot,
                byte_offset,
            } => Ok(leak(BranchIfEqualCtx {
                offset,
                value,
                ptr: MemPtr::new(MemSlot(slot), byte_offset),
            })),
            ref other => Err(wrong("BranchIfEqualCtx", other)),
        }
    }
}

impl<'a> FromCtx<'a> for &'a UniformColorCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::UniformColor { rgba, rgba16 } => Ok(leak(UniformColorCtx {
                r: rgba[0],
                g: rgba[1],
                b: rgba[2],
                a: rgba[3],
                rgba: rgba16,
            })),
            ref other => Err(wrong("UniformColorCtx", other)),
        }
    }
}

impl<'a> FromCtx<'a> for &'a GatherCtx<'a> {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match ctx {
            Ctx::Gather {
                pixels,
                stride,
                width,
                height,
                round_down_at_integer,
            } => Ok(leak(GatherCtx {
                pixels: Vec::leak(pixels.clone()),
                stride: *stride,
                width: *width,
                height: *height,
                weights: [0.0; 16],
                round_down_at_integer: *round_down_at_integer,
            })),
            other => Err(wrong("GatherCtx", other)),
        }
    }
}

impl FromCtx<'_> for EmbossCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::Emboss { mul, add } => Ok(EmbossCtx {
                mul: MemoryCtx::new(MemSlot(mul)),
                add: MemoryCtx::new(MemSlot(add)),
            }),
            ref other => Err(wrong("EmbossCtx", other)),
        }
    }
}

impl<'a> FromCtx<'a> for &'a TablesCtx<'a> {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match ctx {
            Ctx::Tables(t) if t.len() == 1024 => {
                let t: &'static [u8] = Box::leak(t.clone().into_boxed_slice());
                let table = |i: usize| -> &'static [u8; 256] {
                    t[256 * i..256 * (i + 1)].try_into().expect("256 bytes")
                };
                Ok(leak(TablesCtx {
                    r: table(0),
                    g: table(1),
                    b: table(2),
                    a: table(3),
                }))
            }
            other => Err(wrong("TablesCtx", other)),
        }
    }
}

/// Context types rp-diff cannot build yet: `from_ctx` always fails, naming the type.
macro_rules! unsupported {
    ($($ty:ty),* $(,)?) => { $(
        impl<'a> FromCtx<'a> for $ty {
            fn from_ctx(_: &Ctx) -> Result<Self, String> {
                Err(format!(
                    "rp-diff cannot build a `{}` context yet: add a `FromCtx` impl in \
                     oracle/rp-diff/src/replay.rs and the C++ side in cpp/driver.cpp \
                     (docs/PORTING.md §12)",
                    stringify!($ty)
                ))
            }
        }
    )* };
}

unsupported!(
    BinaryOpCtx,
    ConstantCtx,
    SwizzleCtx,
    TernaryOpCtx,
    MatrixMultiplyCtx,
    CaseOpCtx,
    &'a SamplerCtx,
    &'a Conical2PtCtx,
    &'a UniformCtx<'a>,
    &'a TileCtx,
    &'a SwizzleCopyCtx,
    &'a DecalTileCtx,
    &'a MipmapCtx,
    &'a TraceFuncCtx<'a>,
    &'a TraceVarCtx<'a>,
    &'a TraceScopeCtx<'a>,
    &'a TraceLineCtx<'a>,
    &'a GradientCtx<'a>,
    &'a CopyIndirectCtx,
    &'a CopyIndirectUniformCtx<'a>,
    &'a SwizzleCopyIndirectCtx,
    &'a ShuffleCtx,
    &'a PerlinNoiseCtx<'a>,
    &'a EvenlySpaced2StopGradientCtx,
    &'a CoordClampCtx,
    &'a Cell<[u32; MAX_STRIDE_HIGHP]>,
    &'a CallbackCtx<'a>,
);

/// Generates [`build_stage`] from the op table: one arm per op.
macro_rules! stage_builder {
    ($($name:ident $variant:ident [$($ctx:ty)?] $hk:ident $lk:ident $task:ident;)*) => {
        /// The [`Stage`] for `op` with the context `ctx`.
        fn build_stage<'a>(op: Op, ctx: &Ctx) -> Result<Stage<'a>, String> {
            match op {
                $(Op::$variant => stage_builder!(@arm $variant ctx $($ctx)?),)*
            }
        }
    };
    (@arm $variant:ident $c:ident) => {
        match $c {
            Ctx::None => Ok(Stage::$variant),
            other => Err(format!("{} takes no context, got {other:?}", Op::$variant.name())),
        }
    };
    (@arm $variant:ident $c:ident $ctx:ty) => {
        <$ctx as FromCtx<'a>>::from_ctx($c).map(Stage::$variant)
    };
}

skia_rust_simd::rp_op_table!(stage_builder);

/// A case's stages, built once.
///
/// # Errors
/// If a context cannot be built (unsupported type or wrong shape), naming the stage.
pub fn build_stages(specs: &[StageSpec]) -> Result<Vec<Stage<'static>>, String> {
    specs
        .iter()
        .enumerate()
        .map(|(i, s)| {
            build_stage(s.op, &s.ctx).map_err(|e| format!("stage {i} ({}): {e}", s.op.name()))
        })
        .collect()
}

/// Runs `case` (whose stages are `stages`, from [`build_stages`]) on `sel` and returns its
/// output: every buffer's bytes after the runs, concatenated in slot order.
///
/// # Errors
/// If the run panics (an unported stage, an out-of-bounds access, ...), with the message.
///
/// # Panics
/// If the case has more than 65536 buffers.
pub fn run_case(case: &Case, stages: &[Stage<'_>], sel: Selection) -> Result<Vec<u8>, String> {
    let mut bufs: Vec<Vec<u8>> = case.buffers.iter().map(|b| b.bytes.clone()).collect();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut mem = MemoryBindings::new();
        for (i, (bytes, b)) in bufs.iter_mut().zip(&case.buffers).enumerate() {
            let slot = MemSlot(u16::try_from(i).expect("fewer than 65536 buffers"));
            mem.bind(
                slot,
                MemView::write(bytes)
                    .with_stride(b.stride)
                    .with_origin(b.origin),
            );
        }
        if case.compiled {
            let mut program = Program::new(stages, sel, case.force_highp);
            for r in &case.runs {
                program.run(r.x, r.y, r.w, r.h, &mut mem);
            }
        } else {
            for r in &case.runs {
                Program::new(stages, sel, case.force_highp).run(r.x, r.y, r.w, r.h, &mut mem);
            }
        }
    }));
    match result {
        Ok(()) => Ok(bufs.concat()),
        Err(e) => Err(e
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_else(|| "panic".to_owned())),
    }
}
