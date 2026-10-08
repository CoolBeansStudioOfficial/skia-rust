// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp (`Program::allocateSlotData`
// and `Program::appendStages`, the `RP::Callbacks` interface).

//! `Program::appendStages`: appends a finished program's stages to a raster pipeline.
//!
//! `Skia` appends to an `SkRasterPipeline*` and calls back into core for child shaders. This
//! crate sits below core, so it declares the three traits it needs ([`StageSink`], [`SlotAlloc`],
//! [`Callbacks`]) and core implements them for its `RasterPipeline` and `ArenaAlloc`. The sink is
//! passed to each callback, so there is one `&mut` at a time.
//!
//! The lane count is `selection().tier.highp_stride()` at append time (Skia's
//! `SkOpts::raster_pipeline_highp_stride`): every byte offset in the stages is computed for it, and
//! the sink records it so that the pipeline can check it runs on the same tier (`docs/design/sksl.md`
//! R8).
//!
//! The slab (`allocateSlotData`) holds, in order, the value slots, the temp stacks, the immutable
//! slots and the uniform block, all as Skia lays them out: the uniform words are scalars in slot
//! memory, so `Addr::Uniform` offsets become slab offsets and the uniform copies read them like
//! Skia's `const int32_t*` sources. Every context is then `'static` and lives in the arena.
//!
//! One departure from Skia's shape: trace ops are not appended. They need a `TraceHook` (S23), so
//! [`Program::append_stages`] returns `false` for a program that has them, as Skia does when it
//! cannot append.

use core::cell::Cell;

#[allow(clippy::wildcard_imports)]
// every context type of the op table appears in `lower_table`
use skia_rust_simd::rp::contexts::*;
use skia_rust_simd::rp::{MemPtr, NUM_HIGHP_OPS, Op, Stage};

use super::ops::BuilderOp;
use super::program::{Addr, Program, SlotData, StageCtx};

/// What [`Program::append_stages`] needs from the pipeline it appends to (Skia's
/// `SkRasterPipeline*`).
pub trait StageSink<'a> {
    /// `append(op, ctx)`: appends a stage.
    fn append(&mut self, stage: Stage<'a>);
    /// `appendStackRewind()`: appends `stack_rewind` (and makes the pipeline highp).
    fn append_stack_rewind(&mut self);
    /// `getNumStages()`: the absolute position of the next stage.
    fn num_stages(&self) -> usize;
    /// Replaces the stage at `index`. Skia patches a branch's context in place once every label
    /// is placed; a stage here is a value, so the branch is appended again with its final offset.
    fn replace_stage(&mut self, index: usize, stage: Stage<'a>);
    /// Records the highp lane count the stages' byte offsets were computed for.
    fn set_lane_count(&mut self, lanes: usize);
}

/// The arena `appendStages` allocates contexts and slot memory from (Skia's `SkArenaAlloc*`).
pub trait SlotAlloc<'a> {
    /// `alloc->make<T>(v)`.
    fn make<T: Copy + 'static>(&'a self, v: T) -> &'a T;
    /// `makeBytesAlignedTo(bytes, align)` for writable pipeline memory, whose first `init.len()`
    /// bytes start as `init` and whose remaining bytes start as zero. Returns a pointer to the
    /// first byte. (`ArenaAlloc::alloc_scratch_init` in core, bound per run to the shader scratch
    /// slot.)
    fn alloc_scratch_init(&'a self, bytes: usize, align: usize, init: &[u8]) -> MemPtr;
}

/// `SkSL::RP::Callbacks`: appends the stages of the children a program invokes.
pub trait Callbacks<'a, P: StageSink<'a>> {
    /// `appendShader(index)`: `false` if the child cannot be appended.
    fn append_shader(&mut self, p: &mut P, index: i32) -> bool;
    /// `appendColorFilter(index)`.
    fn append_color_filter(&mut self, p: &mut P, index: i32) -> bool;
    /// `appendBlender(index)`.
    fn append_blender(&mut self, p: &mut P, index: i32) -> bool;
    /// `toLinearSrgb(color)`: converts the color at `color` in the slot memory.
    fn to_linear_srgb(&mut self, p: &mut P, color: MemPtr);
    /// `fromLinearSrgb(color)`.
    // The name mirrors Skia's callback, which is not a conversion constructor.
    #[allow(clippy::wrong_self_convention)]
    fn from_linear_srgb(&mut self, p: &mut P, color: MemPtr);
}

/// The data a stage's context is built from: the slab's base, where the uniform block starts in
/// the slab, and the arena.
struct Env<'a, A> {
    alloc: &'a A,
    /// The first byte of the slot slab (`SlotData::values`).
    base: MemPtr,
    /// The byte offset of the uniform block within the slab (after the immutable slots).
    uniform_base: isize,
}

impl<A> Env<'_, A> {
    /// The memory an address names: a slab offset, or a uniform byte offset from the uniform
    /// block.
    fn mem(&self, addr: Addr) -> Option<MemPtr> {
        let offset = match addr {
            Addr::Slab(o) => o,
            Addr::Uniform(o) => self.uniform_base.checked_add(o)?,
            Addr::Null => return None,
        };
        Some(self.base.add(u32::try_from(offset).ok()?))
    }
}

/// Builds one simd context from a [`StageCtx`]. `None` when the context does not have the
/// expected shape (a bug in the lowering) or the context is not built here.
trait FromStageCtx<'a, A>: Sized {
    fn from_stage_ctx(ctx: &StageCtx, env: &Env<'a, A>) -> Option<Self>;
}

/// `[T; N]` from the `u32` byte offsets of a swizzle or shuffle, `None` if one does not fit.
fn narrow<T: TryFrom<u32>, const N: usize>(values: [u32; N]) -> Option<[T; N]> {
    let converted = values
        .into_iter()
        .map(|v| T::try_from(v).ok())
        .collect::<Option<Vec<T>>>()?;
    converted.try_into().ok()
}

impl<'a, A: SlotAlloc<'a>> FromStageCtx<'a, A> for MemPtr {
    fn from_stage_ctx(ctx: &StageCtx, env: &Env<'a, A>) -> Option<Self> {
        match *ctx {
            StageCtx::Ptr(addr) => env.mem(addr),
            _ => None,
        }
    }
}

impl<'a, A: SlotAlloc<'a>> FromStageCtx<'a, A> for ConstantCtx {
    fn from_stage_ctx(ctx: &StageCtx, _: &Env<'a, A>) -> Option<Self> {
        match *ctx {
            StageCtx::Constant { dst, value } => Some(ConstantCtx { value, dst }),
            _ => None,
        }
    }
}

impl<'a, A: SlotAlloc<'a>> FromStageCtx<'a, A> for BinaryOpCtx {
    fn from_stage_ctx(ctx: &StageCtx, _: &Env<'a, A>) -> Option<Self> {
        match *ctx {
            StageCtx::BinaryOp { dst, src } => Some(BinaryOpCtx { dst, src }),
            _ => None,
        }
    }
}

impl<'a, A: SlotAlloc<'a>> FromStageCtx<'a, A> for TernaryOpCtx {
    fn from_stage_ctx(ctx: &StageCtx, _: &Env<'a, A>) -> Option<Self> {
        match *ctx {
            StageCtx::TernaryOp { dst, delta } => Some(TernaryOpCtx { dst, delta }),
            _ => None,
        }
    }
}

impl<'a, A: SlotAlloc<'a>> FromStageCtx<'a, A> for MatrixMultiplyCtx {
    fn from_stage_ctx(ctx: &StageCtx, _: &Env<'a, A>) -> Option<Self> {
        match *ctx {
            StageCtx::MatrixMultiply {
                dst,
                left_columns,
                left_rows,
                right_columns,
                right_rows,
            } => Some(MatrixMultiplyCtx {
                dst,
                left_columns: u8::try_from(left_columns).ok()?,
                left_rows: u8::try_from(left_rows).ok()?,
                right_columns: u8::try_from(right_columns).ok()?,
                right_rows: u8::try_from(right_rows).ok()?,
            }),
            _ => None,
        }
    }
}

impl<'a, A: SlotAlloc<'a>> FromStageCtx<'a, A> for SwizzleCtx {
    fn from_stage_ctx(ctx: &StageCtx, _: &Env<'a, A>) -> Option<Self> {
        match *ctx {
            StageCtx::Swizzle { dst, offsets } => Some(SwizzleCtx {
                dst,
                offsets: narrow(offsets)?,
            }),
            _ => None,
        }
    }
}

impl<'a, A: SlotAlloc<'a>> FromStageCtx<'a, A> for &'a ShuffleCtx {
    fn from_stage_ctx(ctx: &StageCtx, env: &Env<'a, A>) -> Option<Self> {
        match *ctx {
            StageCtx::Shuffle {
                ptr,
                count,
                offsets,
            } => Some(env.alloc.make(ShuffleCtx {
                ptr: env.mem(ptr)?,
                count,
                offsets: narrow(offsets)?,
            })),
            _ => None,
        }
    }
}

impl<'a, A: SlotAlloc<'a>> FromStageCtx<'a, A> for &'a SwizzleCopyCtx {
    fn from_stage_ctx(ctx: &StageCtx, env: &Env<'a, A>) -> Option<Self> {
        match *ctx {
            StageCtx::SwizzleCopy { dst, src, offsets } => Some(env.alloc.make(SwizzleCopyCtx {
                dst: env.mem(dst)?,
                src: env.mem(src)?,
                offsets: narrow(offsets)?,
            })),
            _ => None,
        }
    }
}

impl<'a, A: SlotAlloc<'a>> FromStageCtx<'a, A> for &'a SwizzleCopyIndirectCtx {
    fn from_stage_ctx(ctx: &StageCtx, env: &Env<'a, A>) -> Option<Self> {
        match *ctx {
            StageCtx::SwizzleCopyIndirect {
                dst,
                src,
                indirect_offset,
                indirect_limit,
                slots,
                offsets,
            } => Some(env.alloc.make(SwizzleCopyIndirectCtx {
                copy: CopyIndirectCtx {
                    dst: env.mem(dst)?,
                    src: env.mem(src)?,
                    indirect_offset: env.mem(indirect_offset)?,
                    indirect_limit: u32::try_from(indirect_limit).ok()?,
                    slots: u32::try_from(slots).ok()?,
                },
                offsets: narrow(offsets)?,
            })),
            _ => None,
        }
    }
}

impl<'a, A: SlotAlloc<'a>> FromStageCtx<'a, A> for &'a CopyIndirectCtx {
    fn from_stage_ctx(ctx: &StageCtx, env: &Env<'a, A>) -> Option<Self> {
        match *ctx {
            StageCtx::CopyIndirect {
                dst,
                src,
                indirect_offset,
                indirect_limit,
                slots,
            } => Some(env.alloc.make(CopyIndirectCtx {
                dst: env.mem(dst)?,
                src: env.mem(src)?,
                indirect_offset: env.mem(indirect_offset)?,
                indirect_limit: u32::try_from(indirect_limit).ok()?,
                slots: u32::try_from(slots).ok()?,
            })),
            _ => None,
        }
    }
}

// The source of `copy_from_indirect_uniform_unmasked` is either the uniform block or, for an
// immutable array, the immutable slots: both are scalar words in the slab.
impl<'a, A: SlotAlloc<'a>> FromStageCtx<'a, A> for &'a CopyIndirectUniformCtx {
    fn from_stage_ctx(ctx: &StageCtx, env: &Env<'a, A>) -> Option<Self> {
        match *ctx {
            StageCtx::CopyIndirect {
                dst,
                src,
                indirect_offset,
                indirect_limit,
                slots,
            } => Some(env.alloc.make(CopyIndirectUniformCtx {
                dst: env.mem(dst)?,
                src: env.mem(src)?,
                indirect_offset: env.mem(indirect_offset)?,
                indirect_limit: u32::try_from(indirect_limit).ok()?,
                slots: u32::try_from(slots).ok()?,
            })),
            _ => None,
        }
    }
}

impl<'a, A: SlotAlloc<'a>> FromStageCtx<'a, A> for &'a UniformCtx {
    fn from_stage_ctx(ctx: &StageCtx, env: &Env<'a, A>) -> Option<Self> {
        match *ctx {
            StageCtx::Uniform { dst, src } => Some(env.alloc.make(UniformCtx {
                dst: env.mem(dst)?,
                src: env.mem(src)?,
            })),
            _ => None,
        }
    }
}

impl<'a, A: SlotAlloc<'a>> FromStageCtx<'a, A> for BranchCtx {
    fn from_stage_ctx(ctx: &StageCtx, _: &Env<'a, A>) -> Option<Self> {
        match *ctx {
            StageCtx::Branch { offset } => Some(BranchCtx { offset }),
            _ => None,
        }
    }
}

impl<'a, A: SlotAlloc<'a>> FromStageCtx<'a, A> for &'a BranchIfEqualCtx {
    fn from_stage_ctx(ctx: &StageCtx, env: &Env<'a, A>) -> Option<Self> {
        match *ctx {
            StageCtx::BranchIfEqual { offset, value, ptr } => {
                Some(env.alloc.make(BranchIfEqualCtx {
                    offset,
                    value,
                    ptr: env.mem(ptr)?,
                }))
            }
            _ => None,
        }
    }
}

impl<'a, A: SlotAlloc<'a>> FromStageCtx<'a, A> for CaseOpCtx {
    fn from_stage_ctx(ctx: &StageCtx, _: &Env<'a, A>) -> Option<Self> {
        match *ctx {
            StageCtx::CaseOp {
                expected_value,
                offset,
            } => Some(CaseOpCtx {
                expected_value,
                offset,
            }),
            _ => None,
        }
    }
}

/// Contexts that no stage the `SkSL` builder emits uses, or that are not built here: trace
/// contexts need a `TraceHook` (S23), and the rest belong to other stage families, whose
/// contexts the builder never produces. Lowering never reaches them (`append_stages` rejects
/// trace ops first), and a stage of these would panic in [`lower`].
macro_rules! unsupported_ctx {
    ($($ty:ty),* $(,)?) => { $(
        impl<'a, A: SlotAlloc<'a>> FromStageCtx<'a, A> for $ty {
            fn from_stage_ctx(_: &StageCtx, _: &Env<'a, A>) -> Option<Self> {
                None
            }
        }
    )* };
}

unsupported_ctx!(
    MemoryCtx,
    &'a TraceFuncCtx<'a>,
    &'a TraceVarCtx<'a>,
    &'a TraceScopeCtx<'a>,
    &'a TraceLineCtx<'a>,
    &'a SamplerCtx,
    &'a Conical2PtCtx,
    &'a MipmapCtx,
    &'a GradientCtx,
    &'a PerlinNoiseCtx<'a>,
    &'a EvenlySpaced2StopGradientCtx,
    &'a Cell<[u32; MAX_STRIDE_HIGHP]>,
    &'a CallbackCtx<'a>,
    &'a GatherCtx<'a>,
    &'a DecalTileCtx,
    &'a TileCtx,
    &'a CoordClampCtx,
    &'a UniformColorCtx,
    &'a TransferFunction,
    &'a TablesCtx,
    &'a Cell<f32>,
    &'a [f32; 3],
    &'a [f32; 4],
    &'a [f32; 6],
    &'a [f32; 9],
    &'a [f32; 12],
    &'a [f32; 20],
    [f32; 2],
    [u8; 4],
    f32,
    EmbossCtx,
);

/// Generates [`lower_stage`] from the op table: one arm per op, its context built by
/// [`FromStageCtx`].
macro_rules! lower_table {
    ($($name:ident $variant:ident [$($ctx:ty)?] $hk:ident $lk:ident $task:ident;)*) => {
        /// The simd [`Stage`] for `op` with `ctx`, or `None` if the context does not fit.
        fn lower_stage<'a, A: SlotAlloc<'a>>(
            op: Op,
            ctx: &StageCtx,
            env: &Env<'a, A>,
        ) -> Option<Stage<'a>> {
            match op {
                $(Op::$variant => lower_arm!($variant ctx env $($ctx)?),)*
            }
        }
    };
}

/// One arm of [`lower_table`]: a stage without a context, or one whose context is built.
macro_rules! lower_arm {
    ($variant:ident $ctx:ident $env:ident) => {{
        let _ = ($ctx, $env);
        Some(Stage::$variant)
    }};
    ($variant:ident $ctx:ident $env:ident $ty:ty) => {
        <$ty as FromStageCtx<'a, A>>::from_stage_ctx($ctx, $env).map(Stage::$variant)
    };
}

skia_rust_simd::rp_op_table!(lower_table);

/// The native op of a builder op (Skia's `(SkRasterPipelineOp)op`): the builder's first ops are
/// the simd op table, in the same order.
fn native(op: BuilderOp) -> Op {
    let index = op as usize;
    assert!(
        index < NUM_HIGHP_OPS,
        "{} is not a raster pipeline op",
        op.name()
    );
    let native = Op::ALL[index];
    debug_assert_eq!(native.name(), op.name());
    native
}

/// Lowers a stage that is not one of the extended ops.
///
/// # Panics
/// If the context does not have the shape `make_stages` gives its op, or it is a trace op.
fn lower<'a, A: SlotAlloc<'a>>(op: BuilderOp, ctx: &StageCtx, env: &Env<'a, A>) -> Stage<'a> {
    lower_stage(native(op), ctx, env).unwrap_or_else(|| {
        panic!(
            "raster pipeline stage {} has the wrong context {ctx:?}",
            op.name()
        )
    })
}

/// The operations `appendStages` does not lower to a stage of its own: trace ops, which need a
/// `TraceHook` (S23).
fn is_trace_op(op: BuilderOp) -> bool {
    matches!(
        op,
        BuilderOp::TraceLine
            | BuilderOp::TraceScope
            | BuilderOp::TraceEnter
            | BuilderOp::TraceExit
            | BuilderOp::TraceVar
    )
}

/// A branch appended before its target is placed, to be appended again with its offset.
struct PendingBranch {
    /// The branch's position in the pipeline.
    index: usize,
    op: BuilderOp,
    /// The branch's context, with the label ID as its offset.
    ctx: StageCtx,
    label: usize,
}

/// `branchCtx->offset` with `offset` set (Skia's in-place patch).
fn with_offset(ctx: StageCtx, offset: i32) -> StageCtx {
    match ctx {
        StageCtx::Branch { .. } => StageCtx::Branch { offset },
        StageCtx::BranchIfEqual { value, ptr, .. } => {
            StageCtx::BranchIfEqual { offset, value, ptr }
        }
        other => panic!("not a branch context: {other:?}"),
    }
}

/// The label a branch context names (before it is fixed up).
fn branch_label(ctx: StageCtx) -> usize {
    let offset = match ctx {
        StageCtx::Branch { offset } | StageCtx::BranchIfEqual { offset, .. } => offset,
        other => panic!("not a branch context: {other:?}"),
    };
    usize::try_from(offset).expect("label IDs are non-negative")
}

/// The integer a stage's context carries (a label ID or a child index).
fn int_ctx(ctx: StageCtx) -> i32 {
    match ctx {
        StageCtx::Int(v) => v,
        other => panic!("expected an integer context, got {other:?}"),
    }
}

/// The memory a pointer context names.
fn ptr_ctx<A>(ctx: StageCtx, env: &Env<'_, A>) -> MemPtr {
    match ctx {
        StageCtx::Ptr(addr) => env.mem(addr).expect("a slot pointer"),
        other => panic!("expected a pointer context, got {other:?}"),
    }
}

/// The size of the slot slab (`allocateSlotData`, plus the uniform block): the value slots and
/// temp stacks at `4 * lanes` bytes each, then one scalar per immutable slot and one per uniform.
/// `None` if it does not fit an `int`, as Skia's `SkTFitsIn<int>` check requires.
fn slab_bytes(slots: &SlotData, uniforms: usize) -> Option<usize> {
    let vector = 4 * slots.lanes;
    let vectors = slots.num_values.checked_add(slots.num_stack)?;
    let bytes = vector
        .checked_mul(vectors)?
        .checked_add(4 * slots.immutable.len())?
        .checked_add(4 * uniforms)?;
    i32::try_from(bytes).ok()?;
    Some(bytes)
}

impl Program {
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1675-L1695 (chrome/m156)
    // (`allocateSlotData`, with the slab's image in the arena rather than zeroed memory)
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1697-L1819 (chrome/m156)
    // (`appendStages`)
    /// `Program::appendStages`: appends the program's stages to `pipeline`, with its slot memory
    /// (zeroed, with the immutable slots and the uniform block written) in the arena.
    ///
    /// `uniforms` are the float uniforms (Skia's `SkSpan<const float>`); their bit patterns are
    /// copied into the slab. `callbacks` appends the children the program invokes; without it a program that
    /// invokes a child cannot be appended. Returns `false` if the program cannot be appended: a
    /// trace op (S23), a missing callback, or slot memory that does not fit an `int`.
    ///
    /// The stages are built for [`skia_rust_simd::selection`]'s highp stride, and the pipeline
    /// records it ([`StageSink::set_lane_count`]).
    ///
    /// # Panics
    /// If `uniforms` does not have [`num_uniforms`](Self::num_uniforms) entries.
    // Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1697-L1819 (chrome/m156)
    pub fn append_stages<'a, P, A>(
        &self,
        pipeline: &mut P,
        alloc: &'a A,
        mut callbacks: Option<&mut dyn Callbacks<'a, P>>,
        uniforms: &[f32],
    ) -> bool
    where
        P: StageSink<'a>,
        A: SlotAlloc<'a>,
    {
        assert_eq!(
            usize::try_from(self.num_uniforms()).ok(),
            Some(uniforms.len()),
            "appendStages: the uniform count does not match the program"
        );
        let lanes = skia_rust_simd::selection().tier.highp_stride();
        let uniform_bits: Vec<i32> = uniforms.iter().map(|u| u.to_bits().cast_signed()).collect();

        // Convert the instruction list to stages, filling the immutable slots.
        let mut slots = self.slot_data(lanes);
        let stages = self.make_stages(&uniform_bits, &mut slots, false);
        if stages.iter().any(|s| is_trace_op(s.op)) {
            return false;
        }

        // allocateSlotData: values, then the temp stacks, then the immutable slots, then the
        // uniform block (which the uniform copies read as scalars, from slot memory).
        let Some(size) = slab_bytes(&slots, uniform_bits.len()) else {
            return false;
        };
        let vector = 4 * lanes;
        let immutable_base = vector * (slots.num_values + slots.num_stack);
        let uniform_base = immutable_base + 4 * slots.immutable.len();
        let mut image = vec![0_u8; size];
        for (i, bits) in slots.immutable.iter().enumerate() {
            image[immutable_base + 4 * i..][..4].copy_from_slice(&bits.to_ne_bytes());
        }
        for (i, bits) in uniform_bits.iter().enumerate() {
            image[uniform_base + 4 * i..][..4].copy_from_slice(&bits.to_ne_bytes());
        }
        let base = alloc.alloc_scratch_init(size, vector, &image);
        pipeline.set_lane_count(lanes);

        let env = Env {
            alloc,
            base,
            uniform_base: isize::try_from(uniform_base).expect("slab fits an int"),
        };

        let mut label_offsets: Vec<Option<usize>> =
            vec![None; usize::try_from(self.num_labels).unwrap_or(0)];
        let mut branches: Vec<PendingBranch> = Vec::new();

        // Whenever control passes to another shader, it may overwrite the base pointer (SkSL
        // does), so it is reset on return.
        pipeline.append(Stage::SetBasePointer(base));

        for stage in &stages {
            match stage.op {
                BuilderOp::StackRewind => pipeline.append_stack_rewind(),
                BuilderOp::InvokeShader
                | BuilderOp::InvokeColorFilter
                | BuilderOp::InvokeBlender => {
                    let index = int_ctx(stage.ctx);
                    let Some(cb) = callbacks.as_deref_mut() else {
                        return false;
                    };
                    let appended = match stage.op {
                        BuilderOp::InvokeShader => cb.append_shader(pipeline, index),
                        BuilderOp::InvokeColorFilter => cb.append_color_filter(pipeline, index),
                        _ => cb.append_blender(pipeline, index),
                    };
                    if !appended {
                        return false;
                    }
                    pipeline.append(Stage::SetBasePointer(base));
                }
                BuilderOp::InvokeToLinearSrgb | BuilderOp::InvokeFromLinearSrgb => {
                    let color = ptr_ctx(stage.ctx, &env);
                    let Some(cb) = callbacks.as_deref_mut() else {
                        return false;
                    };
                    if stage.op == BuilderOp::InvokeToLinearSrgb {
                        cb.to_linear_srgb(pipeline, color);
                    } else {
                        cb.from_linear_srgb(pipeline, color);
                    }
                    // A color space transform does not change the base pointer.
                }
                BuilderOp::Label => {
                    // The label's absolute position is where the next stage goes.
                    let id =
                        usize::try_from(int_ctx(stage.ctx)).expect("label IDs are non-negative");
                    label_offsets[id] = Some(pipeline.num_stages());
                }
                BuilderOp::Jump
                | BuilderOp::BranchIfAllLanesActive
                | BuilderOp::BranchIfAnyLanesActive
                | BuilderOp::BranchIfNoLanesActive
                | BuilderOp::BranchIfNoActiveLanesEq => {
                    // The context holds the label ID until every label is placed; the branch
                    // is appended with its own position as a placeholder.
                    let label = branch_label(stage.ctx);
                    let index = pipeline.num_stages();
                    let placeholder =
                        lower(stage.op, &with_offset(stage.ctx, index_i32(index)), &env);
                    pipeline.append(placeholder);
                    branches.push(PendingBranch {
                        index,
                        op: stage.op,
                        ctx: stage.ctx,
                        label,
                    });
                }
                _ => pipeline.append(lower(stage.op, &stage.ctx, &env)),
            }
        }

        // Fix up every branch target: the offset from the branch to its label.
        for branch in &branches {
            let target = label_offsets[branch.label].expect("a branch to a label that is placed");
            let offset = index_i32(target) - index_i32(branch.index);
            let fixed = lower(branch.op, &with_offset(branch.ctx, offset), &env);
            pipeline.replace_stage(branch.index, fixed);
        }
        true
    }
}

/// A stage position as the `int` offsets Skia uses.
fn index_i32(index: usize) -> i32 {
    i32::try_from(index).expect("pipeline positions fit an int")
}
