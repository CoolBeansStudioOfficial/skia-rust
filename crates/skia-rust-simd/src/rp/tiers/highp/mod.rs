// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h (highp: start_pipeline, the stage ABI,
// just_return, stack_checkpoint/stack_rewind, and the stages in the submodules)

//! The highp stages and interpreter, **stamped once per tier** (design §2.5): every tier module
//! in `rp::tiers` mounts this directory with `#[path]`, after bringing its lane module into
//! scope as `lanes` and defining `si!`/`tier_fn!` with its target features. The code here names
//! `F`, `mad`, `rcp_fast`, … unqualified, exactly like `SkRasterPipeline_opts.h` under
//! `SK_OPTS_NS`.
//!
//! # Writing a stage
//! A stage is a function named after its op, in the submodule of its task, with the signature
//! fixed by the op table (`rp::ops`):
//!
//! ```text
//! si! {
//!     // Port of: src/opts/SkRasterPipeline_opts.h#Lx-Ly (chrome/m156)
//!     pub(super) fn name([ctx: Ctx,] p: &mut Regs, e: &mut Params<'_, '_>) { … }
//! }
//! ```
//!
//! `p` holds Skia's `r, g, b, a, dr, dg, db, da`; `e` holds `dx`, `dy`, `base`, the tail byte and
//! the memory (`e.ptr_at_xy(ctx, bpp)`, `e.ptr(mem_ptr)`). Branch stages (`HIGHP_BRANCH_STAGE`)
//! return the program-counter offset as `i32`. The interpreter's `match` is generated from the
//! op table, so adding a stage never touches it.

// Stage code uses the tier's lane names unqualified, as Skia's stages do.
#[allow(clippy::wildcard_imports)]
use super::lanes::*;
#[allow(clippy::wildcard_imports)]
use crate::rp::contexts::*;
use crate::rp::memory::{MemView, MemoryCtxPatch, NO_TAIL, Params};
use crate::rp::ops::Stage;
use crate::rp::tiers::Instr;
#[allow(unused_imports)] // used by some tiers' stage sets only
use core::cell::Cell;

mod basic;
mod blend;
mod branch;
mod color;
mod geometry;
mod image_sampling;
mod memory;
mod memory_wide;
mod sampling;
mod sksl_arith;
mod sksl_masks;
mod sksl_math;
mod sksl_trace;

#[allow(clippy::wildcard_imports)] // the dispatch calls every stage by its op's name
use self::{
    basic::*, blend::*, branch::*, color::*, geometry::*, image_sampling::*, memory::*,
    memory_wide::*, sampling::*, sksl_arith::*, sksl_masks::*, sksl_math::*, sksl_trace::*,
};

/// The eight registers every highp stage receives (Skia's `F r, g, b, a, dr, dg, db, da`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Regs {
    pub r: F,
    pub g: F,
    pub b: F,
    pub a: F,
    pub dr: F,
    pub dg: F,
    pub db: F,
    pub da: F,
}

/// `0.5, 1.5, 2.5, …`: pixel centers relative to `dx` (`seed_shader`, `store_device_xy01`).
pub(crate) const IOTA_F: [f32; MAX_STRIDE_HIGHP] = [
    0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5, 8.5, 9.5, 10.5, 11.5, 12.5, 13.5, 14.5, 15.5,
];

/// `0, 1, 2, …`: lane indices (`dither`, `init_lane_masks`, `branch_if_all_lanes_active`).
pub(crate) const IOTA_U32: [u32; MAX_STRIDE_HIGHP] =
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

/// One dispatch arm: runs a stage on copies of the registers and the base pointer (so the
/// loop-carried registers never have their address taken, whether or not LLVM inlines the
/// stage), then advances the program counter.
macro_rules! highp_arm {
    (@call $name:ident ($($c:expr)?), $regs:ident, $base:ident, $dx:ident, $dy:ident, $tail:ident,
     $views:ident, $patches:ident) => {{
        let mut t = $regs;
        let mut e = Params {
            dx: $dx,
            dy: $dy,
            tail: $tail,
            base: $base,
            views: &mut *$views,
            patches: &mut *$patches,
        };
        let r = $name($($c,)? &mut t, &mut e);
        $base = e.base;
        $regs = t;
        r
    }};
    (n $name:ident $s:ident $variant:ident [], $pc:ident, $($rest:tt)*) => {{
        let _ = $s;
        highp_arm!(@call $name (), $($rest)*);
        $pc += 1;
    }};
    (n $name:ident $s:ident $variant:ident [$ctx:ty], $pc:ident, $($rest:tt)*) => {{
        let Stage::$variant(c) = $s else { unreachable!() };
        highp_arm!(@call $name (*c), $($rest)*);
        $pc += 1;
    }};
    (br $name:ident $s:ident $variant:ident [$ctx:ty], $pc:ident, $($rest:tt)*) => {{
        let Stage::$variant(c) = $s else { unreachable!() };
        let offset: i32 = highp_arm!(@call $name (*c), $($rest)*);
        // Skia: `program += offset` (a negative offset jumps back; out of range panics below).
        $pc = $pc.wrapping_add_signed(offset as isize);
    }};
}

/// Defines `row`, the interpreter for one row, from the op table.
macro_rules! highp_interpreter {
    ($($name:ident $variant:ident [$($ctx:ty)?] $hk:ident $lk:ident $task:ident;)*) => {
        tier_fn! {
            // Port of: src/opts/SkRasterPipeline_opts.h#L1779-L1828 (chrome/m156) (one row)
            /// Runs the program over row `dy` from `x0` to `xlimit` (one iteration of
            /// `start_pipeline`'s row loop): full chunks of `N` pixels, then the tail chunk with
            /// every `MemoryCtx` patched to its scratch buffer. Each chunk starts with zeroed
            /// registers (Skia calls the first stage with `F0`s) and ends at `just_return`.
            ///
            /// The chunk loop and the dispatch `match` share one function so the stage registers
            /// stay in vector registers and the prologue is paid once per row.
            #[inline(never)]
            #[allow(clippy::too_many_lines)] // one arm per op
            fn row(
                prog: &[Instr<'_>],
                x0: usize,
                xlimit: usize,
                dy: usize,
                views: &mut [Option<MemView<'_>>],
                patches: &mut [MemoryCtxPatch],
            ) {
                let mut dx = x0;
                loop {
                    let tail = if dx + N <= xlimit {
                        NO_TAIL
                    } else if dx < xlimit {
                        let tail = xlimit - dx;
                        crate::rp::memory::patch_memory_contexts(views, patches, dx, dy, tail);
                        // `*tailPointer = tail`; tail < N <= 16.
                        #[allow(clippy::cast_possible_truncation)]
                        let tail = tail as u8;
                        tail
                    } else {
                        return;
                    };
                    let zero = F::splat(0.0);
                    let mut regs = Regs {
                        r: zero, g: zero, b: zero, a: zero, dr: zero, dg: zero, db: zero, da: zero,
                    };
                    // Skia's `base` starts as nullptr in every chunk.
                    let mut base: Option<MemPtr> = None;
                    let mut pc = 0usize;
                    loop {
                        match &prog[pc] {
                            // just_return() ends the chain, returning back up to start_pipeline().
                            Instr::Return => break,
                            $(
                            Instr::Stage(s @ Stage::$variant { .. }) => highp_arm!(
                                $hk $name s $variant [$($ctx)?], pc, regs, base, dx, dy, tail,
                                views, patches
                            ),
                            )*
                        }
                    }
                    if tail != NO_TAIL {
                        crate::rp::memory::restore_memory_contexts(
                            views,
                            patches,
                            dx,
                            dy,
                            usize::from(tail),
                        );
                        return;
                    }
                    dx += N;
                }
            }
        }
    };
}

crate::rp::ops::rp_ops!(highp_interpreter);

tier_fn! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L1779-L1828 (chrome/m156)
    /// `start_pipeline`: runs `prog` over the rectangle `[x0, xlimit) × [y0, ylimit)` in chunks
    /// of `N` pixels, then one tail chunk per row with every `MemoryCtx` patched to its scratch
    /// buffer (`patch_memory_contexts`/`restore_memory_contexts`).
    pub(crate) fn run(
        prog: &[Instr<'_>],
        x0: usize,
        y0: usize,
        xlimit: usize,
        ylimit: usize,
        views: &mut [Option<MemView<'_>>],
        patches: &mut [MemoryCtxPatch],
    ) {
        for dy in y0..ylimit {
            row(prog, x0, xlimit, dy, views, patches);
        }
    }
}
