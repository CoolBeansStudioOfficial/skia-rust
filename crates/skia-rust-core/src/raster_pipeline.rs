// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRasterPipeline.{h,cpp}

//! `SkRasterPipeline`: the builder of raster pipelines (design §2.7).
//!
//! This module holds the core of the builder (task A3): appending stages, `run` and `compile`.
//! The convenience appenders (`appendMatrix`, `appendLoad`, `appendStore`,
//! `appendConstantColor`, `appendTransferFunction`, …), `extend`, `appendStackRewind` and
//! `dump` arrive with task A4.
//!
//! The stages, their contexts and the tier code live in `skia_rust_simd::rp`; writable memory
//! is bound per run with [`MemoryBindings`].

pub use skia_rust_simd::rp::{MemPtr, MemSlot, MemView, MemoryBindings, MemoryCtx, Op, Stage};
use skia_rust_simd::rp::{Program, memory_ctx_infos};

// Port of: src/core/SkRasterPipeline.h#L72-L170 (chrome/m156)
/// `SkRasterPipeline`: a list of [`Stage`]s, built for the current CPU tier when run or
/// compiled.
///
/// Skia's arena (`SkArenaAlloc`) is the lifetime `'a` of the contexts the stages borrow.
#[doc(alias = "SkRasterPipeline")]
#[derive(Clone, Debug, Default)]
pub struct RasterPipeline<'a> {
    stages: Vec<Stage<'a>>,
    /// `gForceHighPrecisionRasterPipeline` (a global in Skia, set by tests).
    force_high_precision: bool,
}

// Port of: src/core/SkRasterPipeline.cpp#L35-L46 (chrome/m156)
impl<'a> RasterPipeline<'a> {
    /// An empty pipeline.
    #[must_use]
    pub fn new() -> RasterPipeline<'a> {
        RasterPipeline {
            stages: Vec::new(),
            force_high_precision: false,
        }
    }

    /// `reset()`: removes every stage.
    pub fn reset(&mut self) {
        self.stages.clear();
    }

    /// Forces highp pipelines (Skia's `gForceHighPrecisionRasterPipeline`).
    pub fn set_force_high_precision(&mut self, force: bool) {
        self.force_high_precision = force;
    }

    // Port of: src/core/SkRasterPipeline.cpp#L48-L62 (chrome/m156)
    /// `append(op, ctx)`: appends a stage.
    ///
    /// Some ops have dedicated appenders that pick the op from their context
    /// (`appendConstantColor`, `appendSetRGB`, `appendTransferFunction`, `appendStackRewind`);
    /// appending those ops directly is a debug assertion failure, as in Skia.
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

    // Port of: src/core/SkRasterPipeline.cpp#L71-L169 (chrome/m156)
    /// `uncheckedAppend`: appends a stage without the `append` checks. (The `MemoryCtx`
    /// registration it does is derived from the stages when the pipeline is built,
    /// [`memory_ctx_infos`]; the tail pointer is interpreter state.)
    pub fn unchecked_append(&mut self, stage: Stage<'a>) {
        self.stages.push(stage);
    }

    /// The stages, in order.
    #[must_use]
    pub fn stages(&self) -> &[Stage<'a>] {
        &self.stages
    }

    /// `empty()`.
    #[must_use]
    pub fn empty(&self) -> bool {
        self.stages.is_empty()
    }

    // Port of: src/core/SkRasterPipeline.cpp#L737-L746 (chrome/m156)
    /// `stagesNeeded()`: the stages plus `just_return`, plus a `stack_checkpoint` if there is a
    /// `stack_rewind`.
    #[must_use]
    pub fn stages_needed(&self) -> usize {
        // Add 1 to budget for a `just_return` stage at the end.
        let mut stages = self.stages.len() + 1;
        // If we have any stack_rewind stages, we will need to inject a stack_checkpoint stage.
        if self.stages.iter().any(|s| matches!(s, Stage::StackRewind)) {
            stages += 1;
        }
        stages
    }

    /// The pipeline built for the current selection (`buildPipeline`).
    fn build(&self) -> Program<'a> {
        Program::new(
            &self.stages,
            skia_rust_simd::selection(),
            self.force_high_precision,
        )
    }

    // Port of: src/core/SkRasterPipeline.cpp#L748-L771 (chrome/m156)
    /// `run(x, y, w, h)`: builds the pipeline (with zeroed tail scratch buffers) and runs it over
    /// the `w × h` pixels at `(x, y)`.
    ///
    /// # Panics
    /// If a stage's memory is not bound in `mem` or an access falls outside it, or a stage is not
    /// ported yet.
    pub fn run(&self, x: usize, y: usize, w: usize, h: usize, mem: &mut MemoryBindings<'_>) {
        if self.empty() {
            return;
        }
        self.build().run(x, y, w, h, mem);
    }

    // Port of: src/core/SkRasterPipeline.cpp#L773-L797 (chrome/m156)
    /// `compile()`: builds the pipeline once. Running the result repeatedly reuses its tail
    /// scratch buffers, whose lanes past the tail keep their bytes between runs.
    #[must_use]
    pub fn compile(&self) -> CompiledPipeline<'a> {
        CompiledPipeline {
            program: (!self.empty()).then(|| self.build()),
        }
    }

    /// The `MemoryCtx`s the pipeline registers (`fMemoryCtxInfos`).
    #[must_use]
    pub fn memory_ctx_infos(&self) -> Vec<skia_rust_simd::rp::MemoryCtxInfo> {
        memory_ctx_infos(&self.stages)
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
