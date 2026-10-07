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

use core::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use skia_rust_simd::rp::{MemPtr, MemSlot, MemView, MemoryBindings, MemoryCtx, Op, Program, Stage};
use skia_rust_simd::tier::{Selection, Tier};

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
            Ctx::SkslPtr { buf, slot } => Ok(sksl_ptr(buf, slot)),
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

thread_local! {
    /// Bytes of one `SkSL` slot on the tier being built (`4 * N`, `N` the highp stride): `SkSL`
    /// offsets are slot indices in the case text (see [`Ctx::SkslPtr`]).
    static SLOT_BYTES: Cell<u32> = const { Cell::new(16) };
    /// The `DecalTileCtx`s of the case being built, by [`Ctx::Decal`] id (`decal_*` and
    /// `check_decal_mask` stages share one).
    static DECALS: RefCell<HashMap<u32, &'static DecalTileCtx>> = RefCell::new(HashMap::new());
}

/// Bytes of one `SkSL` slot on the tier being built.
fn slot_bytes() -> u32 {
    SLOT_BYTES.with(Cell::get)
}

/// The byte offset of `SkSL` slot `index`.
fn slot_offset(index: u32) -> u32 {
    index * slot_bytes()
}

fn sksl_ptr(buf: u16, slot: u32) -> MemPtr {
    MemPtr::new(MemSlot(buf), slot_offset(slot))
}

fn u16_offsets<const N: usize>(comps: &[u16]) -> [u16; N] {
    core::array::from_fn(|i| {
        u16::try_from(u32::from(comps[i]) * slot_bytes()).expect("swizzle offset fits u16")
    })
}

impl FromCtx<'_> for BinaryOpCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::SkslBinary { dst, src } => Ok(BinaryOpCtx {
                dst: slot_offset(dst),
                src: slot_offset(src),
            }),
            ref other => Err(wrong("BinaryOpCtx", other)),
        }
    }
}

impl FromCtx<'_> for TernaryOpCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::SkslTernary { dst, delta } => Ok(TernaryOpCtx {
                dst: slot_offset(dst),
                delta: slot_offset(delta),
            }),
            ref other => Err(wrong("TernaryOpCtx", other)),
        }
    }
}

impl FromCtx<'_> for ConstantCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::SkslConstant { value, dst } => Ok(ConstantCtx {
                value,
                dst: slot_offset(dst),
            }),
            ref other => Err(wrong("ConstantCtx", other)),
        }
    }
}

impl FromCtx<'_> for MatrixMultiplyCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::SkslMatmul { dst, dims } => Ok(MatrixMultiplyCtx {
                dst: slot_offset(dst),
                left_columns: dims[0],
                left_rows: dims[1],
                right_columns: dims[2],
                right_rows: dims[3],
            }),
            ref other => Err(wrong("MatrixMultiplyCtx", other)),
        }
    }
}

impl FromCtx<'_> for SwizzleCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::SkslSwizzle { dst, comps } => Ok(SwizzleCtx {
                dst: slot_offset(dst),
                offsets: comps.map(|c| {
                    u8::try_from(u32::from(c) * slot_bytes()).expect("swizzle offset fits u8")
                }),
            }),
            ref other => Err(wrong("SwizzleCtx", other)),
        }
    }
}

impl FromCtx<'_> for CaseOpCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::SkslCase { expected, slot } => Ok(CaseOpCtx {
                expected_value: expected,
                offset: slot_offset(slot),
            }),
            ref other => Err(wrong("CaseOpCtx", other)),
        }
    }
}

impl<'a> FromCtx<'a> for &'a ShuffleCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match ctx {
            Ctx::SkslShuffle {
                buf,
                slot,
                count,
                comps,
            } if comps.len() == 16 => Ok(leak(ShuffleCtx {
                ptr: sksl_ptr(*buf, *slot),
                count: *count,
                offsets: u16_offsets::<16>(comps),
            })),
            other => Err(wrong("ShuffleCtx", other)),
        }
    }
}

impl<'a> FromCtx<'a> for &'a SwizzleCopyCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::SkslSwizzleCopy {
                buf,
                dst,
                src,
                comps,
            } => Ok(leak(SwizzleCopyCtx {
                dst: sksl_ptr(buf, dst),
                src: sksl_ptr(buf, src),
                offsets: u16_offsets::<4>(&comps),
            })),
            ref other => Err(wrong("SwizzleCopyCtx", other)),
        }
    }
}

fn copy_indirect(ctx: &Ctx) -> Result<(CopyIndirectCtx, [u16; 4], Vec<i32>), String> {
    match ctx {
        Ctx::SkslIndirect {
            buf,
            dst,
            src,
            indirect,
            limit,
            slots,
            comps,
            uniform,
        } => Ok((
            CopyIndirectCtx {
                dst: sksl_ptr(*buf, *dst),
                src: sksl_ptr(*buf, *src),
                indirect_offset: sksl_ptr(*buf, *indirect),
                indirect_limit: *limit,
                slots: *slots,
            },
            u16_offsets::<4>(comps),
            uniform.clone(),
        )),
        other => Err(wrong("CopyIndirectCtx", other)),
    }
}

impl<'a> FromCtx<'a> for &'a CopyIndirectCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        Ok(leak(copy_indirect(ctx)?.0))
    }
}

impl<'a> FromCtx<'a> for &'a SwizzleCopyIndirectCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        let (copy, offsets, _) = copy_indirect(ctx)?;
        Ok(leak(SwizzleCopyIndirectCtx { copy, offsets }))
    }
}

impl<'a> FromCtx<'a> for &'a CopyIndirectUniformCtx<'a> {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        let (c, _, uniform) = copy_indirect(ctx)?;
        Ok(leak(CopyIndirectUniformCtx {
            dst: c.dst,
            src: Vec::leak(uniform),
            indirect_offset: c.indirect_offset,
            indirect_limit: c.indirect_limit,
            slots: c.slots,
        }))
    }
}

impl<'a> FromCtx<'a> for &'a UniformCtx<'a> {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match ctx {
            Ctx::SkslUniform { buf, dst, values } => Ok(leak(UniformCtx {
                dst: sksl_ptr(*buf, *dst),
                src: Vec::leak(values.clone()),
            })),
            other => Err(wrong("UniformCtx", other)),
        }
    }
}

impl<'a> FromCtx<'a> for &'a TileCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::Tile {
                scale,
                inv_scale,
                mirror_bias_dir,
            } => Ok(leak(TileCtx {
                scale,
                inv_scale,
                mirror_bias_dir,
            })),
            ref other => Err(wrong("TileCtx", other)),
        }
    }
}

impl<'a> FromCtx<'a> for &'a CoordClampCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::CoordClamp([min_x, min_y, max_x, max_y]) => Ok(leak(CoordClampCtx {
                min_x,
                min_y,
                max_x,
                max_y,
            })),
            ref other => Err(wrong("CoordClampCtx", other)),
        }
    }
}

impl<'a> FromCtx<'a> for &'a DecalTileCtx {
    fn from_ctx(ctx: &Ctx) -> Result<Self, String> {
        match *ctx {
            Ctx::Decal { id, limit, edge } => Ok(DECALS.with(|d| {
                *d.borrow_mut().entry(id).or_insert_with(|| {
                    leak(DecalTileCtx {
                        mask: Cell::default(),
                        limit_x: limit[0],
                        limit_y: limit[1],
                        inclusive_edge_x: edge[0],
                        inclusive_edge_y: edge[1],
                    })
                })
            })),
            ref other => Err(wrong("DecalTileCtx", other)),
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
    &'a SamplerCtx,
    &'a Conical2PtCtx,
    &'a MipmapCtx,
    &'a TraceFuncCtx<'a>,
    &'a TraceVarCtx<'a>,
    &'a TraceScopeCtx<'a>,
    &'a TraceLineCtx<'a>,
    &'a GradientCtx<'a>,
    &'a PerlinNoiseCtx<'a>,
    &'a EvenlySpaced2StopGradientCtx,
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

/// A case's stages for `tier`, built once (`SkSL` slot indices become byte offsets of the tier's
/// highp stride; the stages are valid on every selection of the tier).
///
/// # Errors
/// If a context cannot be built (unsupported type or wrong shape), naming the stage.
///
/// # Panics
/// Never for valid cases (a swizzle offset that does not fit its field panics).
pub fn build_stages(specs: &[StageSpec], tier: Tier) -> Result<Vec<Stage<'static>>, String> {
    let stride = u32::try_from(tier.highp_stride()).expect("small");
    SLOT_BYTES.with(|b| b.set(4 * stride));
    DECALS.with(|d| d.borrow_mut().clear());
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
