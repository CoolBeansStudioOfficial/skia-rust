// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLProgram.{h,cpp}. `fUsage` (`ProgramUsage`) is added by task
// S9a; `Program` is created by `Compiler::convertProgram` (task S11).

//! [`Program`]: a compiled program and its IR.

use std::sync::Arc;

use super::{
    IrPool, NumberKind, SymbolId,
    ids::{ElemId, FnId, SymTabId},
};
use crate::program_settings::ProgramConfig;

/// One uniform of `UniformInfo`.
#[doc(alias = "UniformInfo::Uniform")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uniform {
    /// `fName`.
    pub name: String,
    /// `fKind`.
    pub kind: NumberKind,
    /// `fColumns`.
    pub columns: i32,
    /// `fRows`.
    pub rows: i32,
    /// `fSlot`.
    pub slot: i32,
}

/// `SkSL::UniformInfo`: the uniforms a program reads, by slot.
// Port of: src/sksl/ir/SkSLProgram.h#L37-L48 (chrome/m156)
#[doc(alias = "SkSL::UniformInfo")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UniformInfo {
    /// `fUniforms`.
    pub uniforms: Vec<Uniform>,
    /// `fUniformSlotCount`.
    pub uniform_slot_count: i32,
}

bitflags::bitflags! {
    /// `ProgramInterface::RTFlip`: which features need the render-target flip uniform.
    #[doc(alias = "ProgramInterface::RTFlip")]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct RtFlip: u8 {
        const FRAG_COORD = 0b0000_0001;
        const CLOCKWISE = 0b0000_0010;
        const DERIVATIVE = 0b0000_0100;
    }
}

/// `SkSL::ProgramInterface`: what a program needs from its pipeline.
// Port of: src/sksl/ir/SkSLProgram.h#L50-L68 (chrome/m156)
#[doc(alias = "SkSL::ProgramInterface")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProgramInterface {
    /// `fRTFlipUniform`.
    pub rt_flip_uniform: RtFlip,
    /// `fUseLastFragColor`.
    pub use_last_frag_color: bool,
    /// `fOutputSecondaryColor`.
    pub output_secondary_color: bool,
}

/// `SkSL::Program`: a compiled program.
///
/// The program's IR lives in `pool`, which extends the frozen pool of the module the program was
/// compiled against. `owned_elements` are local to `pool`; `shared_elements` are built-in
/// elements of the module chain that the program uses (Skia's `fSharedElements`), and are
/// immutable. Skia's `fContext` and `fPool` members are replaced by `pool`; the compiler lends
/// `pool` to its [`Context`](crate::context::Context) while it optimizes the program.
// Port of: src/sksl/ir/SkSLProgram.h#L73-L170 (chrome/m156)
#[doc(alias = "SkSL::Program")]
#[derive(Debug)]
pub struct Program {
    /// `fSource`: the text the program was compiled from. Positions are byte offsets into it.
    /// Bytes, because a fuzzer's input need not be UTF-8 (`Ossfuzz519154489`).
    pub source: Arc<[u8]>,
    /// `fConfig`.
    pub config: ProgramConfig,
    /// The program's IR (Skia: `fPool` and the nodes it holds).
    pub pool: IrPool,
    /// `fSymbols`: the program's top-level symbol table (in `pool`).
    pub symbols: SymTabId,
    /// `fOwnedElements`: elements that belong to this program only.
    pub owned_elements: Vec<ElemId>,
    /// `fSharedElements`: built-in elements (from the module chain) the program includes.
    pub shared_elements: Vec<ElemId>,
    /// `fInterface`.
    pub interface: ProgramInterface,
}

impl Program {
    /// `elements()`: every element, shared (built-in) ones first, then owned ones.
    pub fn elements(&self) -> impl Iterator<Item = ElemId> + '_ {
        self.shared_elements
            .iter()
            .chain(self.owned_elements.iter())
            .copied()
    }

    /// `getFunction(name)`: the function of that name, if it exists and has a definition.
    // Port of: src/sksl/ir/SkSLProgram.cpp#L56-L61 (chrome/m156)
    #[must_use]
    pub fn get_function(&self, function_name: &str) -> Option<FnId> {
        match self.pool.find_symbol(self.symbols, function_name)? {
            SymbolId::FunctionDeclaration(f) if self.pool.function(f).definition.is_some() => {
                Some(f)
            }
            _ => None,
        }
    }

    /// `description()`: the `#version` line, then every element's description.
    // Port of: src/sksl/ir/SkSLProgram.cpp#L48-L54 (chrome/m156)
    #[must_use]
    pub fn description(&self) -> String {
        let mut result = self.config.version_description().to_owned();
        for e in self.elements() {
            result.push_str(&self.pool.element_description(e));
        }
        result
    }
}
