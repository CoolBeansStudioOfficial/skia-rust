// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/tracing/SkSLDebugTracePriv.{h,cpp} and
// src/sksl/tracing/SkSLTraceHook.{h,cpp} (the `Tracer` hook). The player is in `tracing::player`.

//! Debug-trace data: the slot and function tables a program was compiled with, the trace the
//! trace ops record while it runs, and the hook that records it.

pub mod bit_set;
pub mod player;

use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_simd::rp::contexts::TraceHook;

use crate::ir::NumberKind;
use crate::position::Position;

/// `SkSL::TraceInfo::Op`: what a trace op recorded.
#[doc(alias = "SkSL::TraceInfo::Op")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceOp {
    /// `kLine`: data: line number, (unused).
    Line,
    /// `kVar`: data: slot, value.
    Var,
    /// `kEnter`: data: function index, (unused).
    Enter,
    /// `kExit`: data: function index, (unused).
    Exit,
    /// `kScope`: data: scope delta, (unused).
    Scope,
}

/// `SkSL::TraceInfo`: one recorded trace op.
#[doc(alias = "SkSL::TraceInfo")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceInfo {
    pub op: TraceOp,
    pub data: [i32; 2],
}

/// `SkSL::SlotDebugInfo`: what a value slot holds.
#[doc(alias = "SkSL::SlotDebugInfo")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotDebugInfo {
    /// The full name of this variable (without component), e.g. `myArray[3].myStruct.myVector`.
    pub name: String,
    /// The dimensions of this variable: 1x1 is a scalar, Nx1 is a vector, `NxM` is a matrix.
    pub columns: u8,
    /// See [`columns`](Self::columns).
    pub rows: u8,
    /// Which component of the variable is this slot? (e.g. `vec4.z` is component 2)
    pub component_index: u8,
    /// Complex types (arrays/structs) can be tracked as a "group" of adjacent slots.
    pub group_index: i32,
    /// What kind of numbers belong in this slot?
    pub number_kind: NumberKind,
    /// Where is this variable located in the program?
    pub line: i32,
    /// The source position of the variable's declaration.
    pub pos: Position,
    /// If this slot holds a function's return value, contains 1; if not, -1.
    pub fn_return_value: i32,
}

impl Default for SlotDebugInfo {
    /// The C++ member initializers: `columns = 1, rows = 1, componentIndex = 0, groupIndex = 0,
    /// numberKind = kNonnumeric, line = 0, pos = {}, fnReturnValue = -1`.
    fn default() -> Self {
        Self {
            name: String::new(),
            columns: 1,
            rows: 1,
            component_index: 0,
            group_index: 0,
            number_kind: NumberKind::Nonnumeric,
            line: 0,
            pos: Position::default(),
            fn_return_value: -1,
        }
    }
}

impl SlotDebugInfo {
    /// A slot named `name` with the default fields.
    #[must_use]
    pub fn named(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            ..Self::default()
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

/// `SkSL::DebugTracePriv`: the debug information a program was compiled with, and the trace it
/// records.
///
/// skia-rust: `fTraceInfo` is behind a `Mutex`, because the trace hook writes it while the program
/// runs, and the program holds a shared handle to this object (`Arc<DebugTracePriv>`).
#[doc(alias = "SkSL::DebugTracePriv")]
#[derive(Debug)]
pub struct DebugTracePriv {
    /// `fTraceCoord`: the device coordinate to trace.
    pub trace_coord: (i32, i32),
    /// `fUniformInfo`: one entry per uniform slot.
    pub uniform_info: Vec<SlotDebugInfo>,
    /// `fSlotInfo`: one entry per value slot.
    pub slot_info: Vec<SlotDebugInfo>,
    /// `fFuncInfo`: one entry per function.
    pub func_info: Vec<FunctionDebugInfo>,
    /// `fTraceInfo`: the trace ops recorded so far.
    pub trace_info: Mutex<Vec<TraceInfo>>,
    /// `fSource`: the `SkSL` source, split line by line.
    pub source: Vec<String>,
}

impl Default for DebugTracePriv {
    fn default() -> Self {
        Self {
            trace_coord: (0, 0),
            uniform_info: Vec::new(),
            slot_info: Vec::new(),
            func_info: Vec::new(),
            trace_info: Mutex::new(Vec::new()),
            source: Vec::new(),
        }
    }
}

impl DebugTracePriv {
    /// `setTraceCoord`: sets the device-coordinate pixel to trace.
    pub fn set_trace_coord(&mut self, x: i32, y: i32) {
        self.trace_coord = (x, y);
    }

    /// `setSource`: splits `source` into lines on `'\n'`. As with Skia's `std::getline` loop, a
    /// trailing newline produces a final empty line.
    // Port of: src/sksl/tracing/SkSLDebugTracePriv.cpp#L85-L92 (chrome/m156)
    pub fn set_source(&mut self, source: &[u8]) {
        self.source = String::from_utf8_lossy(source)
            .split('\n')
            .map(str::to_owned)
            .collect();
    }

    /// A snapshot of the trace recorded so far (`fTraceInfo`).
    #[must_use]
    pub fn trace_ops(&self) -> Vec<TraceInfo> {
        self.trace_info
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// `fTraceInfo.size()`.
    #[must_use]
    pub fn trace_len(&self) -> usize {
        self.trace_info
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// `fTraceInfo[position]`, if there is one.
    #[must_use]
    pub fn trace_op_at(&self, position: usize) -> Option<TraceInfo> {
        self.trace_info
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(position)
            .copied()
    }

    /// `getSlotComponentSuffix`: a slot's component as a variable-name suffix, e.g. ".x" or
    /// "[2][2]".
    // Port of: src/sksl/tracing/SkSLDebugTracePriv.cpp#L15-L33 (chrome/m156)
    #[must_use]
    pub fn get_slot_component_suffix(&self, slot_index: usize) -> String {
        let slot = &self.slot_info[slot_index];
        let rows = u32::from(slot.rows);
        let component = u32::from(slot.component_index);
        if rows > 1 {
            return format!("[{}][{}]", component / rows, component % rows);
        }
        if slot.columns > 1 {
            return match slot.component_index {
                0 => ".x",
                1 => ".y",
                2 => ".z",
                3 => ".w",
                _ => "[???]",
            }
            .to_owned();
        }
        String::new()
    }

    /// `interpretValueBits`: bit-casts a slot's value, honoring the slot's `NumberKind`.
    // Port of: src/sksl/tracing/SkSLDebugTracePriv.cpp#L35-L56 (chrome/m156)
    #[must_use]
    pub fn interpret_value_bits(&self, slot_index: usize, value_bits: i32) -> f64 {
        match self.slot_info[slot_index].number_kind {
            NumberKind::Unsigned => f64::from(value_bits.cast_unsigned()),
            NumberKind::Float => f64::from(f32::from_bits(value_bits.cast_unsigned())),
            _ => f64::from(value_bits),
        }
    }

    /// `slotValueToString`: converts a numeric value into text, based on the slot's `NumberKind`.
    // Port of: src/sksl/tracing/SkSLDebugTracePriv.cpp#L58-L69 (chrome/m156)
    #[must_use]
    pub fn slot_value_to_string(&self, slot_index: usize, value: f64) -> String {
        match self.slot_info[slot_index].number_kind {
            NumberKind::Boolean => (if value == 0.0 { "false" } else { "true" }).to_owned(),
            _ => format_g8(value),
        }
    }

    /// `getSlotValue`: bit-casts a slot's value, then converts it to text, e.g. "3.14" or "true".
    // Port of: src/sksl/tracing/SkSLDebugTracePriv.cpp#L71-L74 (chrome/m156)
    #[must_use]
    pub fn get_slot_value(&self, slot_index: usize, value: i32) -> String {
        self.slot_value_to_string(slot_index, self.interpret_value_bits(slot_index, value))
    }

    /// `dump`: a human-readable dump of the debug trace.
    ///
    /// # Panics
    /// If a trace op names a slot, function or source line that the tables do not have.
    // Port of: src/sksl/tracing/SkSLDebugTracePriv.cpp#L95-L193 (chrome/m156)
    #[must_use]
    // Each piece is appended in order, as Skia writes each one to the stream in turn.
    #[allow(clippy::format_push_string)]
    pub fn dump(&self) -> String {
        let mut o = String::new();
        for (index, info) in self.slot_info.iter().enumerate() {
            o += &format!("${index} = {} (", info.name);
            o += match info.number_kind {
                NumberKind::Float => "float",
                NumberKind::Signed => "int",
                NumberKind::Unsigned => "uint",
                NumberKind::Boolean => "bool",
                NumberKind::Nonnumeric => "???",
            };
            let total = u32::from(info.rows) * u32::from(info.columns);
            if total > 1 {
                o += &format!("{}", info.columns);
                if info.rows != 1 {
                    o += &format!("x{}", info.rows);
                }
                o += &format!(" : slot {}/{}", u32::from(info.component_index) + 1, total);
            }
            o += &format!(", L{})\n", info.line);
        }

        for (index, info) in self.func_info.iter().enumerate() {
            o += &format!("F{index} = {}\n", info.name);
        }

        o.push('\n');

        let mut indent = String::new();
        for trace_info in self.trace_ops() {
            let [data0, data1] = trace_info.data;
            match trace_info.op {
                TraceOp::Line => {
                    o += &format!("{indent}line {data0}");
                }
                TraceOp::Var => {
                    let slot = usize::try_from(data0).expect("a variable slot is non-negative");
                    o += &format!(
                        "{indent}{}{} = {}",
                        self.slot_info[slot].name,
                        self.get_slot_component_suffix(slot),
                        self.get_slot_value(slot, data1)
                    );
                }
                TraceOp::Enter => {
                    let func = usize::try_from(data0).expect("a function index is non-negative");
                    o += &format!("{indent}enter {}", self.func_info[func].name);
                    indent += "  ";
                }
                TraceOp::Exit => {
                    let len = indent.len().saturating_sub(2);
                    indent.truncate(len);
                    let func = usize::try_from(data0).expect("a function index is non-negative");
                    o += &format!("{indent}exit {}", self.func_info[func].name);
                }
                TraceOp::Scope => {
                    for _ in data0..0 {
                        indent.pop();
                    }
                    let sign = if data0 >= 0 { "+" } else { "" };
                    o += &format!("{indent}scope {sign}{data0}");
                    for _ in 0..data0 {
                        indent.push(' ');
                    }
                }
            }
            o.push('\n');
        }
        o
    }
}

/// `SkSL::Tracer`: the trace hook of a program. It appends each trace op to the `fTraceInfo` of
/// its debug trace.
#[doc(alias = "SkSL::Tracer")]
#[derive(Debug)]
pub struct Tracer {
    debug: Arc<DebugTracePriv>,
}

impl Tracer {
    /// `Tracer::Make`: a hook that records into `debug`.
    // Port of: src/sksl/tracing/SkSLTraceHook.cpp#L11-L15 (chrome/m156)
    #[must_use]
    pub fn new(debug: Arc<DebugTracePriv>) -> Self {
        Self { debug }
    }

    fn push(&self, op: TraceOp, data0: i32, data1: i32) {
        self.debug
            .trace_info
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(TraceInfo {
                op,
                data: [data0, data1],
            });
    }
}

impl TraceHook for Tracer {
    // Port of: src/sksl/tracing/SkSLTraceHook.cpp#L17-L31 (chrome/m156)
    fn var(&self, slot: i32, val: i32) {
        self.push(TraceOp::Var, slot, val);
    }
    fn line(&self, line_num: i32) {
        self.push(TraceOp::Line, line_num, 0);
    }
    fn enter(&self, fn_idx: i32) {
        self.push(TraceOp::Enter, fn_idx, 0);
    }
    fn exit(&self, fn_idx: i32) {
        self.push(TraceOp::Exit, fn_idx, 0);
    }
    fn scope(&self, delta: i32) {
        self.push(TraceOp::Scope, delta, 0);
    }
}

/// `snprintf(buffer, n, "%.8g", value)`: `%e` or `%f` at 8 significant digits, whichever C picks,
/// with the trailing zeros of the fraction removed.
///
/// # Panics
/// The `expect`s check the layout of Rust's own `{:e}` output, so they cannot fail.
// Port of: src/sksl/tracing/SkSLDebugTracePriv.cpp#L58-L69 (chrome/m156)
#[must_use]
pub fn format_g8(value: f64) -> String {
    const PRECISION: i32 = 8;
    if value.is_nan() {
        return if value.is_sign_negative() {
            "-nan"
        } else {
            "nan"
        }
        .to_owned();
    }
    if value.is_infinite() {
        return if value < 0.0 { "-inf" } else { "inf" }.to_owned();
    }
    // The exponent comes from `%e` at precision P-1, so that rounding can carry into it.
    let sci = format!("{:.*e}", (PRECISION - 1) as usize, value);
    let (mantissa_e, exp) = sci.split_once('e').expect("%e has an exponent");
    let exponent: i32 = exp.parse().expect("%e exponent is an integer");
    let text = if (-4..PRECISION).contains(&exponent) {
        let decimals = usize::try_from(PRECISION - 1 - exponent).expect("decimals are positive");
        format!("{value:.decimals$}")
    } else {
        let sign = if exponent < 0 { '-' } else { '+' };
        format!("{mantissa_e}e{sign}{:02}", exponent.unsigned_abs())
    };
    // `%g` without `#` removes the trailing zeros of the fraction, and then the point.
    let (mantissa, suffix) = match text.find('e') {
        Some(at) => (&text[..at], &text[at..]),
        None => (text.as_str(), ""),
    };
    let trimmed = if mantissa.contains('.') {
        mantissa.trim_end_matches('0').trim_end_matches('.')
    } else {
        mantissa
    };
    format!("{trimmed}{suffix}")
}
