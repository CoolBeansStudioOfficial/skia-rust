// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h (namespace lowp: start_pipeline, the
// LOWP_STAGE_GG/GP/PP stage ABI, just_return, and the stages in the submodules)

//! The lowp stages and interpreter (`namespace lowp`), stamped once per tier that has a lowp
//! pipeline (every SIMD tier; `Scalar` has none). Like `highp`, the code names the tier's lowp
//! lane types (`U16`, `F`, …, `N` = the lowp stride) unqualified.
//!
//! # Writing a stage
//! The op table (`rp::ops`) gives each lowp op one of Skia's three stage shapes:
//!
//! ```text
//! pp: pub(super) fn name([ctx,] p: &mut Regs, e: &mut Params<'_, '_>)            // pixels in/out
//! gg: pub(super) fn name([ctx,] x: &mut F, y: &mut F, e: &mut Params<'_, '_>)    // geometry
//! gp: pub(super) fn name([ctx,] x: F, y: F, p: &mut Regs, e: &mut Params<'_, '_>) // gather
//! ```
//!
//! For `gg`/`gp` the interpreter joins `x` from `r,g` and `y` from `b,a` (and splits them back
//! after a `gg` stage), exactly like Skia's `LOWP_STAGE_GG`/`_GP` wrappers.

// Stage code uses the tier's lane names unqualified, as Skia's stages do.
#[allow(clippy::wildcard_imports)]
use super::lanes::lowp::*;
#[allow(clippy::wildcard_imports)]
use crate::rp::contexts::*;
use crate::rp::memory::{MemView, MemoryCtxPatch, NO_TAIL, Params};
use crate::rp::ops::Stage;
use crate::rp::tiers::Instr;
use crate::vx::Vec;
#[allow(unused_imports)] // used by some tiers' stage sets only
use core::cell::Cell;

mod basic;
mod blend;
mod color;
mod geometry;
mod image_sampling;
mod memory;
mod sampling;

#[allow(clippy::wildcard_imports)] // the dispatch calls every stage by its op's name
use self::{basic::*, blend::*, color::*, geometry::*, image_sampling::*, memory::*, sampling::*};

/// Panics for a stage whose task has not ported it yet.
macro_rules! not_ported {
    ($name:literal, $task:literal) => {
        unimplemented!(concat!(
            "raster pipeline stage `",
            $name,
            "` (lowp) is not ported yet (task ",
            $task,
            ")"
        ))
    };
}
use not_ported;

/// The eight registers every lowp stage receives (Skia's `U16 r, g, b, a, dr, dg, db, da`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Regs {
    pub r: U16,
    pub g: U16,
    pub b: U16,
    pub a: U16,
    pub dr: U16,
    pub dg: U16,
    pub db: U16,
    pub da: U16,
}

/// `0.5, 1.5, 2.5, …`: pixel centers relative to `dx`.
pub(crate) const IOTA_F: [f32; MAX_STRIDE] = [
    0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5, 8.5, 9.5, 10.5, 11.5, 12.5, 13.5, 14.5, 15.5,
];

si! {
    /// `join<F>(lo, hi)`: the float register whose bytes are `lo`'s then `hi`'s.
    fn join_f(lo: U16, hi: U16) -> F {
        // Half by half, so each half stays one register.
        let (lo, hi): (Vec<{ N / 2 }, f32>, Vec<{ N / 2 }, f32>) = (lo.bit_cast(), hi.bit_cast());
        crate::vx::join(lo, hi)
    }

    /// `split(v, &lo, &hi)`: the inverse of `join_f`.
    fn split_f(v: F) -> (U16, U16) {
        (v.lo().bit_cast(), v.hi().bit_cast())
    }
}

/// One dispatch arm (see `highp_arm!` in the highp interpreter): runs the stage on a copy of
/// the registers, joining/splitting the geometry registers for `gg`/`gp` stages.
macro_rules! lowp_arm {
    (@params $dx:ident, $dy:ident, $tail:ident, $views:ident, $patches:ident) => {
        Params {
            dx: $dx,
            dy: $dy,
            tail: $tail,
            base: None,
            views: &mut *$views,
            patches: &mut *$patches,
        }
    };
    (pp $name:ident $s:ident $variant:ident [], $regs:ident, $($p:ident),*) => {{
        let _ = $s;
        let mut t = $regs;
        let mut e = lowp_arm!(@params $($p),*);
        $name(&mut t, &mut e);
        $regs = t;
    }};
    (pp $name:ident $s:ident $variant:ident [$ctx:ty], $regs:ident, $($p:ident),*) => {{
        let Stage::$variant(c) = $s else { unreachable!() };
        let mut t = $regs;
        let mut e = lowp_arm!(@params $($p),*);
        $name(*c, &mut t, &mut e);
        $regs = t;
    }};
    (gg $name:ident $s:ident $variant:ident [$($ctx:ty)?], $regs:ident, $($p:ident),*) => {{
        let mut x = join_f($regs.r, $regs.g);
        let mut y = join_f($regs.b, $regs.a);
        let mut e = lowp_arm!(@params $($p),*);
        lowp_arm!(@call_gg $name $s $variant [$($ctx)?], x, y, e);
        ($regs.r, $regs.g) = split_f(x);
        ($regs.b, $regs.a) = split_f(y);
    }};
    (@call_gg $name:ident $s:ident $variant:ident [], $x:ident, $y:ident, $e:ident) => {{
        let _ = $s;
        $name(&mut $x, &mut $y, &mut $e);
    }};
    (@call_gg $name:ident $s:ident $variant:ident [$ctx:ty], $x:ident, $y:ident, $e:ident) => {{
        let Stage::$variant(c) = $s else { unreachable!() };
        $name(*c, &mut $x, &mut $y, &mut $e);
    }};
    (gp $name:ident $s:ident $variant:ident [$ctx:ty], $regs:ident, $($p:ident),*) => {{
        let Stage::$variant(c) = $s else { unreachable!() };
        let x = join_f($regs.r, $regs.g);
        let y = join_f($regs.b, $regs.a);
        let mut t = $regs;
        let mut e = lowp_arm!(@params $($p),*);
        $name(*c, x, y, &mut t, &mut e);
        $regs = t;
    }};
    (hi $name:ident $s:ident $variant:ident [$($ctx:ty)?], $regs:ident, $($p:ident),*) => {{
        let _ = $s;
        unreachable!(concat!("highp-only stage `", stringify!($name), "` in a lowp program"))
    }};
}

/// Defines `row`, the lowp interpreter for one row, from the op table.
macro_rules! lowp_interpreter {
    ($($name:ident $variant:ident [$($ctx:ty)?] $hk:ident $lk:ident $task:ident;)*) => {
        tier_fn! {
            // Port of: src/opts/SkRasterPipeline_opts.h#L5533-L5570 (chrome/m156) (one row)
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
                    let zero = U16::splat(0);
                    let mut regs = Regs {
                        r: zero, g: zero, b: zero, a: zero, dr: zero, dg: zero, db: zero, da: zero,
                    };
                    let mut pc = 0usize;
                    loop {
                        match &prog[pc] {
                            // just_return() ends the chain, returning back up to start_pipeline().
                            Instr::Return => break,
                            $(
                            Instr::Stage(s @ Stage::$variant { .. }) => lowp_arm!(
                                $lk $name s $variant [$($ctx)?], regs, dx, dy, tail, views,
                                patches
                            ),
                            )*
                        }
                        pc += 1;
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

crate::rp::ops::rp_ops!(lowp_interpreter);

tier_fn! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L5533-L5570 (chrome/m156)
    /// `lowp::start_pipeline`: as the highp `run`, in chunks of the lowp `N`.
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
