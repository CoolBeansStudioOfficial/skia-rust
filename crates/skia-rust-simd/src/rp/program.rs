// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRasterPipeline.cpp (buildLowpPipeline, buildHighpPipeline,
// stagesNeeded, run, compile)

//! [`Program`]: a pipeline built for a tier (lowp or highp), with its tail scratch buffers, and
//! [`ProgramDesc::run`], which builds and runs one without allocating.

use super::contexts::{MemSlot, MemoryCtx, MemoryCtxInfo};
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

/// What a pipeline builder hands over to build a [`Program`]: the stages in order plus what
/// `SkRasterPipeline` tracks next to them while appending.
#[derive(Clone, Copy, Debug)]
pub struct ProgramDesc<'s, 'a> {
    /// The stages, in order (`fStages`, oldest first).
    pub stages: &'s [Stage<'a>],
    /// The registered `MemoryCtx`s, in registration order (`fMemoryCtxInfos`).
    pub memory_ctx_infos: &'s [MemoryCtxInfo],
    /// Whether the pipeline has a rewind context (`fRewindCtx != nullptr`, set by
    /// `appendStackRewind`): such a pipeline is highp and starts with `stack_checkpoint`.
    pub has_rewind: bool,
    /// `gForceHighPrecisionRasterPipeline`.
    pub force_highp: bool,
}

/// Program length that [`ProgramDesc::run`] keeps on the stack (`AutoSTMalloc<32, …>` in
/// `SkRasterPipeline::run`).
const STACK_STAGES: usize = 32;
/// Memory contexts that [`ProgramDesc::run`] keeps on the stack (`AutoSTMalloc<2, …>`).
const STACK_PATCHES: usize = 2;

/// A placeholder for unused stack patches.
const UNUSED_INFO: MemoryCtxInfo = MemoryCtxInfo {
    context: MemoryCtx::new(MemSlot(0)),
    bytes_per_pixel: 0,
    load: false,
    store: false,
};

impl<'a> ProgramDesc<'_, 'a> {
    // Port of: src/core/SkRasterPipeline.cpp#L593-L609 (chrome/m156)
    /// Whether `buildLowpPipeline` succeeds on `tier`: not forced highp, no rewind context, and
    /// every stage has a lowp implementation on the tier.
    #[must_use]
    pub fn is_lowp(&self, tier: Tier) -> bool {
        if self.force_highp || self.has_rewind {
            return false;
        }
        self.stages.iter().all(|s| has_lowp(tier, s.op()))
    }

    // Port of: src/core/SkRasterPipeline.cpp#L737-L746 (chrome/m156)
    /// `stagesNeeded()`: the stages, plus `just_return`, plus `stack_checkpoint` with a rewind
    /// context.
    #[must_use]
    pub fn stages_needed(&self) -> usize {
        // Add 1 to budget for a `just_return` stage at the end.
        let mut stages = self.stages.len() + 1;
        // If we have any stack_rewind stages, we will need to inject a stack_checkpoint stage.
        if self.has_rewind {
            stages += 1;
        }
        stages
    }

    // Port of: src/core/SkRasterPipeline.cpp#L593-L627 (chrome/m156)
    /// Writes the program into `instrs` (at least [`stages_needed`](Self::stages_needed)
    /// long), front to back, and returns its length: `buildLowpPipeline` or
    /// `buildHighpPipeline` (which assemble it back to front).
    fn fill(&self, lowp: bool, instrs: &mut [Instr<'a>]) -> usize {
        let mut n = 0;
        // stack_checkpoint and stack_rewind are only implemented in highp. We only need these
        // stages when generating long (or looping) pipelines from SkSL.
        if !lowp && self.has_rewind {
            instrs[n] = Instr::Stage(Stage::StackCheckpoint);
            n += 1;
        }
        for &stage in self.stages {
            instrs[n] = Instr::Stage(stage);
            n += 1;
        }
        instrs[n] = Instr::Return;
        n + 1
    }

    // Port of: src/core/SkRasterPipeline.cpp#L748-L770 (chrome/m156)
    /// `SkRasterPipeline::run`: builds the program for `selection` with zeroed tail scratch
    /// buffers and runs it over the `w × h` pixels at `(x, y)`.
    ///
    /// Like Skia's `AutoSTMalloc`s, this keeps programs of up to 32 stages (including
    /// `just_return`) and up to 2 memory contexts on the stack: it allocates nothing in the
    /// common case.
    ///
    /// # Panics
    /// As [`Program::run`].
    #[allow(clippy::many_single_char_names)] // Skia's run(x, y, w, h)
    pub fn run(
        &self,
        selection: Selection,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        mem: &mut MemoryBindings<'_>,
    ) {
        let lowp = self.is_lowp(selection.tier);
        let needed = self.stages_needed();

        let mut stack_instrs = [Instr::Return; STACK_STAGES];
        let mut heap_instrs = Vec::new();
        let instrs: &mut [Instr<'a>] = if needed <= STACK_STAGES {
            &mut stack_instrs[..needed]
        } else {
            heap_instrs.resize(needed, Instr::Return);
            &mut heap_instrs
        };
        let len = self.fill(lowp, instrs);

        let n = self.memory_ctx_infos.len();
        let mut stack_patches = [const { MemoryCtxPatch::new(UNUSED_INFO) }; STACK_PATCHES];
        let mut heap_patches = Vec::new();
        let patches: &mut [MemoryCtxPatch] = if n <= STACK_PATCHES {
            for (patch, &info) in stack_patches.iter_mut().zip(self.memory_ctx_infos) {
                patch.info = info;
            }
            &mut stack_patches[..n]
        } else {
            heap_patches.extend(
                self.memory_ctx_infos
                    .iter()
                    .map(|&i| MemoryCtxPatch::new(i)),
            );
            &mut heap_patches
        };

        tiers::run(
            selection,
            lowp,
            &instrs[..len],
            x,
            y,
            x + w,
            y + h,
            &mut mem.views,
            patches,
        );
    }
}

/// A raster pipeline built for one tier: the stages as Skia's `buildLowpPipeline` or
/// `buildHighpPipeline` arranges them, plus one zeroed tail scratch buffer per `MemoryCtx`.
///
/// This is what `SkRasterPipeline::compile()` returns. The scratch buffers persist across
/// [`run`](Self::run) calls, so lanes past the tail hold the previous tail's bytes, exactly as in
/// a compiled Skia pipeline (design §1.7).
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
    /// otherwise highp, with a `stack_checkpoint` first if there is a `stack_rewind`. (A
    /// pipeline builder uses [`Program::build`] with what it tracked instead.)
    #[must_use]
    pub fn new(stages: &[Stage<'a>], selection: Selection, force_highp: bool) -> Program<'a> {
        let infos = memory_ctx_infos(stages);
        Program::build(
            &ProgramDesc {
                stages,
                memory_ctx_infos: &infos,
                has_rewind: stages.iter().any(|s| matches!(s, Stage::StackRewind)),
                force_highp,
            },
            selection,
        )
    }

    // Port of: src/core/SkRasterPipeline.cpp#L772-L797 (chrome/m156)
    /// Builds `desc` for `selection` (Skia's `compile()` with `SkOpts` set to that tier), with
    /// zeroed tail scratch buffers.
    #[must_use]
    pub fn build(desc: &ProgramDesc<'_, 'a>, selection: Selection) -> Program<'a> {
        let lowp = desc.is_lowp(selection.tier);
        let mut instrs = vec![Instr::Return; desc.stages_needed()];
        let len = desc.fill(lowp, &mut instrs);
        instrs.truncate(len);
        let patches = desc
            .memory_ctx_infos
            .iter()
            .map(|&info| MemoryCtxPatch::new(info))
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

    /// The number of program stages including the final `just_return` (and a leading
    /// `stack_checkpoint` in a highp program with a rewind context).
    #[must_use]
    pub fn len(&self) -> usize {
        self.instrs.len()
    }

    /// Always false: a program ends with `just_return`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.instrs.is_empty()
    }

    // Port of: src/core/SkRasterPipeline.cpp#L792-L796 (chrome/m156) (the compiled closure)
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
