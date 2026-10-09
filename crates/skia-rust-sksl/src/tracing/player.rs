// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/tracing/SkSLDebugTracePlayer.{h,cpp}.

//! `SkSLDebugTracePlayer`: plays back a debug trace, so that it can be viewed like a debugger.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::bit_set::BitSet;
use super::{DebugTracePriv, TraceInfo, TraceOp};

/// `SkSLDebugTracePlayer::VariableData`: one variable that a stack frame (or the global scope)
/// displays.
#[doc(alias = "SkSLDebugTracePlayer::VariableData")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VariableData {
    /// `fSlotIndex`.
    pub slot_index: i32,
    /// `fDirty`: has this slot been written-to since the last step call?
    pub dirty: bool,
    /// `fValue`: the value in the slot, with the type conversion applied.
    pub value: f64,
}

/// `SkSLDebugTracePlayer::Slot`.
#[derive(Clone, Copy, Debug)]
struct Slot {
    /// The value in this slot.
    value: i32,
    /// The scope value of this slot.
    scope: i32,
    /// When was the variable in this slot most recently written? (by cursor position)
    write_time: usize,
}

/// `SkSLDebugTracePlayer::StackFrame`.
#[derive(Clone, Debug)]
struct StackFrame {
    /// From `fFuncInfo`.
    function: i32,
    /// Our current line number within the function.
    line: i32,
    /// The variable slots which have been touched in this function.
    display_mask: BitSet,
}

/// `SkSLDebugTracePlayer`: plays back a `SkSL` debug trace, allowing its contents to be viewed like
/// a traditional debugger.
#[doc(alias = "SkSLDebugTracePlayer")]
#[derive(Debug, Default)]
pub struct DebugTracePlayer {
    debug_trace: Option<Arc<DebugTracePriv>>,
    /// The position of the read head.
    cursor: usize,
    /// The current scope depth (as tracked by `trace_scope`).
    scope: i32,
    /// The array of all slots.
    slots: Vec<Slot>,
    /// The execution stack.
    stack: Vec<StackFrame>,
    /// Variable slots touched during the most-recently executed step.
    dirty_mask: BitSet,
    /// Variable slots containing return values.
    return_values: BitSet,
    /// `[line number, the remaining number of times to reach this line during the trace]`.
    line_numbers: HashMap<i32, i32>,
    /// All breakpoints set by `set_breakpoints`.
    breakpoint_lines: HashSet<i32>,
}

impl DebugTracePlayer {
    /// `reset`: resets playback to the start of the trace. Breakpoints are not cleared.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L15-L44 (chrome/m156)
    pub fn reset(&mut self, debug_trace: Option<Arc<DebugTracePriv>>) {
        let nslots = debug_trace.as_ref().map_or(0, |t| t.slot_info.len());
        self.debug_trace = debug_trace;
        self.cursor = 0;
        self.scope = 0;
        self.slots.clear();
        self.slots.resize(
            nslots,
            Slot {
                value: 0,
                scope: i32::MAX,
                write_time: 0,
            },
        );
        self.stack.clear();
        self.stack.push(StackFrame {
            function: -1,
            line: -1,
            display_mask: BitSet::new(nslots),
        });
        self.dirty_mask = BitSet::new(nslots);
        self.return_values = BitSet::new(nslots);
        if let Some(trace) = &self.debug_trace {
            for (slot_idx, info) in trace.slot_info.iter().enumerate() {
                if info.fn_return_value >= 0 {
                    self.return_values.set(slot_idx);
                }
            }
            for trace_info in trace.trace_ops() {
                if trace_info.op == TraceOp::Line {
                    *self.line_numbers.entry(trace_info.data[0]).or_insert(0) += 1;
                }
            }
        }
    }

    /// `step`: advances the simulation to the next Line op.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L46-L53 (chrome/m156)
    pub fn step(&mut self) {
        self.tidy_state();
        while !self.trace_has_completed() {
            let position = self.cursor;
            self.cursor += 1;
            if self.execute(position) {
                break;
            }
        }
    }

    /// `stepOver`: advances to the next Line op, skipping past matched Enter/Exit pairs.
    /// Breakpoints will also stop the simulation even if we haven't reached an Exit.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L55-L68 (chrome/m156)
    pub fn step_over(&mut self) {
        self.tidy_state();
        let initial_stack_depth = self.stack.len();
        while !self.trace_has_completed() {
            let can_escape_from_this_stack_depth = self.stack.len() <= initial_stack_depth;
            let position = self.cursor;
            self.cursor += 1;
            if self.execute(position) && (can_escape_from_this_stack_depth || self.at_breakpoint())
            {
                break;
            }
        }
    }

    /// `stepOut`: advances until we exit from the current stack frame. Breakpoints will also stop
    /// the simulation even if we haven't left the stack frame.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L70-L82 (chrome/m156)
    pub fn step_out(&mut self) {
        self.tidy_state();
        let initial_stack_depth = self.stack.len();
        while !self.trace_has_completed() {
            let position = self.cursor;
            self.cursor += 1;
            if self.execute(position) {
                let has_escaped_from_initial_stack_depth = self.stack.len() < initial_stack_depth;
                if has_escaped_from_initial_stack_depth || self.at_breakpoint() {
                    break;
                }
            }
        }
    }

    /// `run`: advances the simulation until we hit a breakpoint, or the trace completes.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L84-L94 (chrome/m156)
    pub fn run(&mut self) {
        self.tidy_state();
        while !self.trace_has_completed() {
            let position = self.cursor;
            self.cursor += 1;
            if self.execute(position) && self.at_breakpoint() {
                break;
            }
        }
    }

    /// `tidyState`: cleans up temporary state between steps, such as the dirty mask and function
    /// return values.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L96-L103 (chrome/m156)
    fn tidy_state(&mut self) {
        self.dirty_mask.reset_all();
        // Conceptually this is `fStack.back().fDisplayMask &= ~fReturnValues`, but the bit set
        // does not support masking one set against another.
        let return_slots = self.return_values.set_indices();
        let frame = self
            .stack
            .last_mut()
            .expect("the global frame is always present");
        for slot in return_slots {
            frame.display_mask.reset(slot);
        }
    }

    /// `traceHasCompleted`: returns true if we have reached the end of the trace.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L105-L107 (chrome/m156)
    #[must_use]
    pub fn trace_has_completed(&self) -> bool {
        self.debug_trace
            .as_ref()
            .is_none_or(|trace| self.cursor >= trace.trace_len())
    }

    /// `getCurrentLine`: the current line.
    ///
    /// # Panics
    /// If there is no stack (a player that was never reset).
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L109-L112 (chrome/m156)
    #[must_use]
    pub fn get_current_line(&self) -> i32 {
        self.stack.last().expect("the stack is not empty").line
    }

    /// `getCurrentLineInStackFrame`: the current line for a given stack frame.
    ///
    /// # Panics
    /// If the stack frame does not exist.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L114-L123 (chrome/m156)
    #[must_use]
    pub fn get_current_line_in_stack_frame(&self, stack_frame_index: i32) -> i32 {
        // The first entry on the stack is the "global" frame before we enter main, so offset our
        // index by one to account for it.
        let index = usize::try_from(stack_frame_index + 1).expect("a stack frame index");
        assert!(index < self.stack.len(), "the stack frame exists");
        self.stack[index].line
    }

    /// `atBreakpoint`: returns true if there is a breakpoint set at the current line.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L125-L127 (chrome/m156)
    #[must_use]
    pub fn at_breakpoint(&self) -> bool {
        self.breakpoint_lines.contains(&self.get_current_line())
    }

    /// `cursor`: the cursor position.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// `getBreakpoints`.
    #[must_use]
    pub fn get_breakpoints(&self) -> &HashSet<i32> {
        &self.breakpoint_lines
    }

    /// `setBreakpoints`: breakpoints force the simulation to stop whenever a desired line is
    /// reached.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L129-L131 (chrome/m156)
    pub fn set_breakpoints(&mut self, breakpoint_lines: HashSet<i32>) {
        self.breakpoint_lines = breakpoint_lines;
    }

    /// `addBreakpoint`.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L133-L135 (chrome/m156)
    pub fn add_breakpoint(&mut self, line: i32) {
        self.breakpoint_lines.insert(line);
    }

    /// `removeBreakpoint`.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L137-L139 (chrome/m156)
    pub fn remove_breakpoint(&mut self, line: i32) {
        self.breakpoint_lines.remove(&line);
    }

    /// `getCallStack`: the call stack as an array of `FunctionInfo` indices.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L141-L150 (chrome/m156)
    #[must_use]
    pub fn get_call_stack(&self) -> Vec<i32> {
        self.stack.iter().skip(1).map(|f| f.function).collect()
    }

    /// `getStackDepth`: the size of the call stack.
    ///
    /// # Panics
    /// If the player was never reset.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L152-L155 (chrome/m156)
    #[must_use]
    pub fn get_stack_depth(&self) -> i32 {
        i32::try_from(self.stack.len() - 1).expect("the stack depth fits an int")
    }

    /// `getLineNumbersReached`: every line number reached inside this debug trace, along with the
    /// remaining number of times that this trace will reach it.
    #[must_use]
    pub fn get_line_numbers_reached(&self) -> &HashMap<i32, i32> {
        &self.line_numbers
    }

    /// `getVariablesForDisplayMask`: the indices and values of each slot enabled in `bits`, the
    /// most recently written first.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L157-L170 (chrome/m156)
    fn get_variables_for_display_mask(&self, display_mask: &BitSet) -> Vec<VariableData> {
        assert_eq!(display_mask.size(), self.slots.len());
        // Only a displayed slot needs the trace (a player with no trace displays none).
        let mut vars: Vec<VariableData> = display_mask
            .set_indices()
            .into_iter()
            .map(|slot| {
                let trace = self
                    .debug_trace
                    .as_ref()
                    .expect("a displayed slot has a trace");
                VariableData {
                    slot_index: i32::try_from(slot).expect("a slot index fits an int"),
                    dirty: self.dirty_mask.test(slot),
                    value: trace.interpret_value_bits(slot, self.slots[slot].value),
                }
            })
            .collect();

        // Order the variable list so that the most recently-written variables are shown at the
        // top. (`stable_sort` is `std::stable_sort`.)
        vars.sort_by(|a, b| {
            let write_a = self.slots[usize::try_from(a.slot_index).unwrap_or(0)].write_time;
            let write_b = self.slots[usize::try_from(b.slot_index).unwrap_or(0)].write_time;
            write_b.cmp(&write_a)
        });
        vars
    }

    /// `getLocalVariables`: the variables of a stack frame.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L172-L183 (chrome/m156)
    #[must_use]
    pub fn get_local_variables(&self, stack_frame_index: i32) -> Vec<VariableData> {
        // The first entry on the stack is the "global" frame before we enter main, so offset our
        // index by one to account for it.
        let index = stack_frame_index + 1;
        match usize::try_from(index) {
            Ok(index) if index > 0 && index < self.stack.len() => {
                self.get_variables_for_display_mask(&self.stack[index].display_mask)
            }
            _ => {
                debug_assert!(false, "stack frame {stack_frame_index} doesn't exist");
                Vec::new()
            }
        }
    }

    /// `getGlobalVariables`: the variables of the global scope.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L185-L190 (chrome/m156)
    #[must_use]
    pub fn get_global_variables(&self) -> Vec<VariableData> {
        match self.stack.first() {
            None => Vec::new(),
            Some(frame) => self.get_variables_for_display_mask(&frame.display_mask),
        }
    }

    /// `updateVariableWriteTime`: updates the write time of the entire variable in a given slot.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L192-L212 (chrome/m156)
    fn update_variable_write_time(&mut self, slot_idx: usize, cursor: usize) {
        // The slot could point to any slot within a variable. We want to update the write time on
        // EVERY slot associated with this variable. The group index gives us the affected range.
        let trace = self
            .debug_trace
            .clone()
            .expect("a variable is written in a trace");
        let slot_infos = &trace.slot_info;
        let group = isize::try_from(slot_infos[slot_idx].group_index).expect("group index");
        let mut index = isize::try_from(slot_idx).expect("slot fits") - group;
        assert!(index >= 0, "the group starts inside the slot table");
        loop {
            let index_usize = usize::try_from(index).expect("slot index is non-negative");
            self.slots[index_usize].write_time = cursor;
            index += 1;
            // Stop if we've reached the final slot.
            if index >= isize::try_from(slot_infos.len()).expect("slot count fits") {
                break;
            }
            // Each separate variable-group starts with a groupIndex of 0; stop when we detect
            // this.
            if slot_infos[usize::try_from(index).expect("in range")].group_index == 0 {
                break;
            }
        }
    }

    /// `execute`: executes the trace op at the passed-in cursor position. Returns true if we've
    /// reached a line or exit trace op, which indicate a stopping point.
    // Port of: src/sksl/tracing/SkSLDebugTracePlayer.cpp#L214-L283 (chrome/m156)
    fn execute(&mut self, position: usize) -> bool {
        let trace = self.debug_trace.clone().expect("a trace is being played");
        let Some(trace_info) = trace.trace_op_at(position) else {
            debug_assert!(false, "position {position} out of range");
            return true;
        };
        self.execute_op(&trace, position, trace_info)
    }

    fn execute_op(&mut self, trace: &DebugTracePriv, position: usize, info: TraceInfo) -> bool {
        let [data0, data1] = info.data;
        match info.op {
            TraceOp::Line => {
                // data: line number, (unused)
                assert!(!self.stack.is_empty());
                assert!(data0 >= 0);
                assert!(
                    usize::try_from(data0).expect("line") < trace.source.len(),
                    "a line number is inside the source"
                );
                self.stack.last_mut().expect("not empty").line = data0;
                *self.line_numbers.entry(data0).or_insert(0) -= 1;
                true
            }
            TraceOp::Var => {
                // data: slot, value
                let slot_idx = usize::try_from(data0).expect("a variable slot is non-negative");
                assert!(slot_idx < trace.slot_info.len());
                self.slots[slot_idx].value = data1;
                self.slots[slot_idx].scope = self.slots[slot_idx].scope.min(self.scope);
                self.update_variable_write_time(slot_idx, position);
                if trace.slot_info[slot_idx].fn_return_value < 0 {
                    // Normal variables are associated with the current function.
                    let n = self.stack.len();
                    self.stack[n - 1].display_mask.set(slot_idx);
                } else {
                    // Return values are associated with the parent function (since the current
                    // function is exiting and we won't see them there).
                    let n = self.stack.len();
                    assert!(n > 1);
                    self.stack[n - 2].display_mask.set(slot_idx);
                }
                self.dirty_mask.set(slot_idx);
                false
            }
            TraceOp::Enter => {
                // data: function index, (unused)
                assert!(data0 >= 0);
                assert!(usize::try_from(data0).expect("fn") < trace.func_info.len());
                self.stack.push(StackFrame {
                    function: data0,
                    line: -1,
                    display_mask: BitSet::new(trace.slot_info.len()),
                });
                false
            }
            TraceOp::Exit => {
                // data: function index, (unused)
                assert!(!self.stack.is_empty());
                assert_eq!(self.stack.last().expect("not empty").function, data0);
                self.stack.pop();
                true
            }
            TraceOp::Scope => {
                // data: scope delta, (unused)
                assert!(!self.stack.is_empty());
                self.scope += data0;
                if data0 < 0 {
                    // If the scope is being reduced, discard variables that are now out of scope.
                    for slot_idx in 0..self.slots.len() {
                        if self.scope < self.slots[slot_idx].scope {
                            self.slots[slot_idx].scope = i32::MAX;
                            self.stack
                                .last_mut()
                                .expect("not empty")
                                .display_mask
                                .reset(slot_idx);
                        }
                    }
                }
                false
            }
        }
    }
}
