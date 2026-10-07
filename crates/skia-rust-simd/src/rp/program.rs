// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRasterPipeline.cpp (buildLowpPipeline, buildHighpPipeline,
// stagesNeeded, run, compile)

//! [`Program`]: a pipeline built for a tier (lowp or highp), with its tail scratch buffers.

use super::contexts::MemoryCtxInfo;
use super::memory::{MemoryBindings, MemoryCtxPatch, memory_ctx_infos};
use super::ops::{Op, Stage};
use super::tiers::{self, Instr};
use crate::tier::{Selection, Tier};

/// Whether `op` runs in lowp on `tier` (`SkOpts::ops_lowp[op] != nullptr`): every op of
/// `SK_RASTER_PIPELINE_OPS_LOWP` on a tier with a lowp pipeline; nothing on `Scalar`.
#[must_use]
pub fn has_lowp(tier: Tier, op: Op) -> bool {
    tier.lowp_stride().is_some() && op.has_lowp()
}

/// A raster pipeline built for one tier: the stages as Skia's `buildLowpPipeline` or
/// `buildHighpPipeline` arranges them, plus one zeroed tail scratch buffer per `MemoryCtx`.
///
/// This is what `SkRasterPipeline::compile()` returns; `SkRasterPipeline::run()` builds a fresh
/// one per call. The scratch buffers persist across [`run`](Self::run) calls, so lanes past the
/// tail hold the previous tail's bytes, exactly as in a compiled Skia pipeline (design §1.7).
#[derive(Debug)]
pub struct Program<'a> {
    selection: Selection,
    lowp: bool,
    instrs: Box<[Instr<'a>]>,
    pub(crate) patches: Box<[MemoryCtxPatch]>,
}

impl<'a> Program<'a> {
    /// Builds `stages` for `selection` (Skia's `compile()` with `SkOpts` set to that tier).
    ///
    /// The program is lowp if every stage has a lowp implementation on the tier, no stage is
    /// `stack_rewind` and `force_highp` (Skia's `gForceHighPrecisionRasterPipeline`) is false;
    /// otherwise highp, with a `stack_checkpoint` first if there is a `stack_rewind`.
    #[must_use]
    pub fn new(stages: &[Stage<'a>], selection: Selection, force_highp: bool) -> Program<'a> {
        let selection = tiers::effective(selection);
        let has_rewind = stages.iter().any(|s| matches!(s, Stage::StackRewind));

        // Port of: src/core/SkRasterPipeline.cpp#L588-L604 (chrome/m156)
        let lowp =
            !force_highp && !has_rewind && stages.iter().all(|s| has_lowp(selection.tier, s.op()));

        // Port of: src/core/SkRasterPipeline.cpp#L606-L632 (chrome/m156)
        let mut instrs = Vec::with_capacity(stages.len() + 2);
        if !lowp && has_rewind {
            // stack_checkpoint and stack_rewind are only implemented in highp.
            instrs.push(Instr::Stage(Stage::StackCheckpoint));
        }
        instrs.extend(stages.iter().copied().map(Instr::Stage));
        instrs.push(Instr::Return);

        // Port of: src/core/SkRasterPipeline.cpp#L779-L788 (chrome/m156)
        let patches = memory_ctx_infos(stages)
            .into_iter()
            .map(MemoryCtxPatch::new)
            .collect();

        Program {
            selection,
            lowp,
            instrs: instrs.into_boxed_slice(),
            patches,
        }
    }

    /// The selection the program runs on.
    #[must_use]
    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// Whether the program runs the lowp stages.
    #[must_use]
    pub fn is_lowp(&self) -> bool {
        self.lowp
    }

    /// The `MemoryCtx`s whose tails are patched, in registration order.
    pub fn memory_ctx_infos(&self) -> impl Iterator<Item = MemoryCtxInfo> + '_ {
        self.patches.iter().map(|p| p.info)
    }

    /// The number of program stages including the final `just_return` (Skia's
    /// `stagesNeeded()`).
    #[must_use]
    pub fn len(&self) -> usize {
        self.instrs.len()
    }

    /// Always false: a program ends with `just_return`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.instrs.is_empty()
    }

    // Port of: src/core/SkRasterPipeline.cpp#L790-L796 (chrome/m156) (the compiled closure)
    /// Runs the program over the `w × h` pixels at `(x, y)` with `mem` bound
    /// (`start_pipeline(x, y, x + w, y + h, …)`).
    ///
    /// # Panics
    /// If a stage's memory is not bound in `mem` or an access falls outside it, if a stage is
    /// not ported yet (its task is named), or if the selection cannot run here (see
    /// [`Selection::check`]).
    pub fn run(&mut self, x: usize, y: usize, w: usize, h: usize, mem: &mut MemoryBindings<'_>) {
        tiers::run(
            self.selection,
            self.lowp,
            &self.instrs,
            x,
            y,
            x + w,
            y + h,
            &mut mem.views,
            &mut self.patches,
        );
    }
}
