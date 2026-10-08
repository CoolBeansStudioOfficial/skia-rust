// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/tracing/SkSLDebugTracePriv.h (the data half: slot, uniform and
// function debug info). The trace hook and the player are not ported yet (task S23).

//! Debug-trace data that the Raster Pipeline dumper needs to name slots and functions.

// Port of: src/sksl/tracing/SkSLDebugTracePriv.h#L39-L67 (chrome/m156)

/// `SkSL::SlotDebugInfo`: what a value slot holds.
#[doc(alias = "SkSL::SlotDebugInfo")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlotDebugInfo {
    /// The full name of this variable (without component), e.g. `myArray[3].myStruct.myVector`.
    pub name: String,
    /// The dimensions of this variable: 1x1 is a scalar, Nx1 is a vector, `NxM` is a matrix.
    pub columns: u8,
    /// See [`columns`](Self::columns).
    pub rows: u8,
    /// Which component of the variable is this slot? (e.g. `vec4.z` is component 2)
    pub component_index: u8,
    /// Start offset of the variable's declaration in the source, when the position is valid
    /// (`SkSL::Position::valid()`). The dumper uses it to disambiguate same-named variables.
    pub pos_start: Option<usize>,
}

impl SlotDebugInfo {
    /// A scalar slot with no source position, named `name`.
    #[must_use]
    pub fn named(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            columns: 1,
            rows: 1,
            component_index: 0,
            pos_start: None,
        }
    }
}

/// `SkSL::FunctionDebugInfo`.
#[doc(alias = "SkSL::FunctionDebugInfo")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FunctionDebugInfo {
    /// Full function declaration: `float myFunction(half4 color)`.
    pub name: String,
}

/// `SkSL::DebugTracePriv`: the debug information a program was compiled with.
#[doc(alias = "SkSL::DebugTracePriv")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugTracePriv {
    /// `fSlotInfo`: one entry per value slot.
    pub slot_info: Vec<SlotDebugInfo>,
    /// `fUniformInfo`: one entry per uniform slot.
    pub uniform_info: Vec<SlotDebugInfo>,
    /// `fFuncInfo`: one entry per function.
    pub func_info: Vec<FunctionDebugInfo>,
}
