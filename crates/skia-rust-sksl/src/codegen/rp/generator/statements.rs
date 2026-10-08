// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp (`Generator`'s function,
// global, statement and immutable-data members).

//! Functions, globals and statements.

// Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1391-L2186 (chrome/m156)

use super::{
    ContinueMask, Generator, ImmutableBits, Res, SK_FRAGCOORD_BUILTIN, SlotKey, unsupported,
};
use crate::analysis;
use crate::codegen::rp::builder::SlotRange;
use crate::constant_folder;
use crate::ir::{
    BlockKind, ElemId, ExprId, ExpressionKind, NumberKind, ProgramElementKind, StatementKind,
    StmtId, SwitchStatement, VarId,
};

/// `(int32_t)value` for a `double`, as x86 compiles it: values that do not fit give `INT_MIN`.
fn double_to_i32(value: f64) -> i32 {
    if value.is_nan() || value >= 2_147_483_648.0 || value < -2_147_483_648.0 {
        return i32::MIN;
    }
    // In range: truncation toward zero.
    #[allow(clippy::cast_possible_truncation)]
    {
        value as i32
    }
}

/// `(uint32_t)value` for a `double`, as x86 compiles it: a 64-bit truncation, then the low half.
fn double_to_u32(value: f64) -> u32 {
    let wide = if value.is_nan()
        || value >= 9_223_372_036_854_775_808.0
        || value < -9_223_372_036_854_775_808.0
    {
        i64::MIN
    } else {
        // In range: truncation toward zero.
        #[allow(clippy::cast_possible_truncation)]
        {
            value as i64
        }
    };
    // The low 32 bits of the 64-bit result.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        wide as u32
    }
}

impl Generator<'_> {
    /// `writeFunction`: converts an `SkSL` function into a set of instructions. Returns the slots
    /// of its result, or `Err` if the function contained unsupported statements or expressions.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1391-L1493 (chrome/m156)
    pub(super) fn write_function(
        &mut self,
        call_site: SlotKey,
        function: ElemId,
        arguments: &[ExprId],
    ) -> Res<SlotRange> {
        let (decl_id, body) = self.function_parts(function);

        // Generate debug information and emit a trace-enter op.
        let mut func_index = -1;
        if self.debug_trace.is_some() {
            func_index = self.get_function_debug_info(decl_id);
            debug_assert!(func_index >= 0);
            if self.should_write_trace_ops() {
                self.builder
                    .trace_enter(self.trace_mask_stack_id(), func_index);
            }
        }

        // Handle parameter lvalues.
        let parameters = self.ctx.pool.function(decl_id).parameters.clone();
        let is_main = self.ctx.pool.function(decl_id).is_main;
        let mut lvalues: Vec<Option<super::lvalue::LvId>> = Vec::new();
        let mut remapped_slot_ranges: Vec<(VarId, Option<SlotRange>)> = Vec::new();

        if is_main {
            // For main(), the parameter slots have already been populated by `writeProgram`, but
            // we still need to explicitly emit trace ops for the variables in main(), since they
            // are initialized before it is safe to use trace-var. (We can't invoke
            // init-lane-masks until after we've copied the inputs from main into slots, because
            // dst.rgba is used to pass in a blend-destination color, but we clobber it and put in
            // the execution mask instead.)
            if self.should_write_trace_ops() {
                for &var in &parameters {
                    let slots = self.get_variable_slots(var);
                    self.builder.trace_var(self.trace_mask_stack_id(), slots);
                }
            }
        } else {
            // Write all the arguments into their parameter's variable slots. Because we never
            // allow recursion, we don't need to worry about overwriting any existing values in
            // those slots. (In fact, we don't even need to apply the write mask.)
            lvalues.resize(arguments.len(), None);

            for index in 0..arguments.len() {
                let arg = arguments[index];
                let param = parameters[index];
                let arg_type = self.expr_type(arg);

                // If we are passing a child effect to a function, we need to add its mapping to
                // our child map.
                if self.ctx.pool.ty(arg_type).is_effect_child() {
                    if let ExpressionKind::VariableReference(var_ref) =
                        &self.ctx.pool.expression(arg).kind
                    {
                        if let Some(&child_index) = self.child_effect_map.get(&var_ref.variable) {
                            debug_assert!(!self.child_effect_map.contains_key(&param));
                            self.child_effect_map.insert(param, child_index);
                        }
                    }
                    continue;
                }

                // Use LValues for out-parameters and inout-parameters, so we can store back to
                // them later.
                if self.is_inout_parameter(param) || self.is_out_parameter(param) {
                    let Some(lvalue) = self.make_lvalue(arg, false) else {
                        return unsupported();
                    };
                    lvalues[index] = Some(lvalue);
                    // There are no guarantees on the starting value of an out-parameter, so we
                    // only need to store the lvalues associated with an inout parameter.
                    if self.is_inout_parameter(param) {
                        self.push_lvalue(lvalue)?;
                        let slots = self.get_variable_slots(param);
                        self.pop_to_slot_range_unmasked(slots);
                    }
                    continue;
                }

                // If a parameter is never read by the function, we don't need to populate its
                // slots.
                let param_counts = self.usage.get_variable(param);
                if param_counts.read == 0 {
                    // Honor the expression's side effects, if any.
                    if analysis::has_side_effects(&self.ctx.pool, arg) {
                        self.push_expression(arg, false)?;
                        let slots = self.expr_slots(arg);
                        self.discard_expression(slots);
                    }
                    continue;
                }

                // If the expression is a plain variable and the parameter is never written to, we
                // don't need to copy it; we can just share the slots from the existing variable.
                if param_counts.write == 0 {
                    if let ExpressionKind::VariableReference(var_ref) =
                        &self.ctx.pool.expression(arg).kind
                    {
                        let var = var_ref.variable;
                        if self.has_variable_slots(var) {
                            let range = self.get_variable_slots(var);
                            let original_range = self.program_slots.map_variable_to_slots(
                                &self.ctx.pool,
                                param,
                                range,
                            );
                            remapped_slot_ranges.push((param, original_range));
                            continue;
                        }
                    }
                }

                // Copy input arguments into their respective parameter slots.
                self.push_expression(arg, true)?;
                let slots = self.get_variable_slots(param);
                self.pop_to_slot_range_unmasked(slots);
            }
        }

        // Set up a slot range dedicated to this function's return value.
        let last_function_result = self.current_function_result;
        self.current_function_result = self.get_function_slots(call_site, decl_id);

        // Save off the return mask.
        if self.needs_return_mask(function) {
            self.builder.enable_execution_mask_writes();
            if !is_main {
                self.builder.push_return_mask();
            }
        }

        // Emit the function body.
        self.write_statement(body)?;

        // Restore the original return mask.
        if self.needs_return_mask(function) {
            if !is_main {
                self.builder.pop_return_mask();
            }
            self.builder.disable_execution_mask_writes();
        }

        // Restore the function-result slot range.
        let function_result = self.current_function_result;
        self.current_function_result = last_function_result;

        // Emit a trace-exit op.
        if self.should_write_trace_ops() {
            self.builder
                .trace_exit(self.trace_mask_stack_id(), func_index);
        }

        // Copy out-parameters and inout-parameters back to their homes.
        for (index, lvalue) in lvalues.iter().enumerate() {
            if let Some(lvalue) = *lvalue {
                // Only out- and inout-parameters should have an associated lvalue.
                let param = parameters[index];
                debug_assert!(self.is_inout_parameter(param) || self.is_out_parameter(param));

                // Copy the parameter's slots directly into the lvalue.
                let slots = self.get_variable_slots(param);
                self.builder.push_slots(slots);
                self.store_lvalue(lvalue)?;
                let param_slots = self.type_slots(self.ctx.pool.variable(param).ty);
                self.discard_expression(param_slots);
            }
        }

        // Restore any remapped parameter slot ranges to their original values.
        for (variable, slot_range) in remapped_slot_ranges {
            if let Some(range) = slot_range {
                self.program_slots
                    .map_variable_to_slots(&self.ctx.pool, variable, range);
            } else {
                self.program_slots.unmap_variable_slots(variable);
            }
        }

        // Remove any child-effect mappings that were made for this call.
        for (index, &arg) in arguments.iter().enumerate() {
            let arg_type = self.expr_type(arg);
            if self.ctx.pool.ty(arg_type).is_effect_child() {
                self.child_effect_map.remove(&parameters[index]);
            }
        }

        // `lvalues` is destroyed when the function returns, front to back.
        for lvalue in lvalues.into_iter().flatten() {
            self.free_lvalue(lvalue);
        }

        Ok(function_result)
    }

    /// `writeGlobals`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1537-L1594 (chrome/m156)
    pub(super) fn write_globals(&mut self) -> Res {
        for element in self.elements.clone() {
            let ProgramElementKind::GlobalVar(gvd) = &self.ctx.pool.element(element).kind else {
                continue;
            };
            let declaration = gvd.declaration;
            let StatementKind::VarDeclaration(decl) = &self.ctx.pool.statement(declaration).kind
            else {
                panic!("a global variable declaration holds a VarDeclaration");
            };
            let var = decl.var;
            let decl_value = decl.value;
            let var_type = self.ctx.pool.variable(var).ty;

            if self.ctx.pool.ty(var_type).is_effect_child() {
                // Associate each child effect variable with its numeric index.
                debug_assert!(!self.child_effect_map.contains_key(&var));
                let child_effect_index =
                    i32::try_from(self.child_effect_map.len()).expect("child count fits i32");
                self.child_effect_map.insert(var, child_effect_index);
                continue;
            }

            // Opaque types include child processors and GL objects (samplers, textures, etc).
            // Of those, only child processors are legal variables.
            debug_assert!(!self.ctx.pool.ty(var_type).is_void());
            debug_assert!(!self.ctx.pool.ty(var_type).is_opaque());

            // Builtin variables are system-defined, with special semantics.
            let builtin = self.ctx.pool.variable(var).layout().builtin;
            if builtin >= 0 {
                if builtin == SK_FRAGCOORD_BUILTIN {
                    let slots = self.get_variable_slots(var);
                    self.builder.store_device_xy01(slots);
                    continue;
                }
                // The only builtin variable exposed to runtime effects is sk_FragCoord.
                return unsupported();
            }

            if self.is_uniform(var) {
                // Create the uniform slot map in first-to-last order.
                let uniform_slot_range = self.get_uniform_slots(var);

                if self.should_write_trace_ops() {
                    // We expect uniform values to show up in the debug trace. To make this happen
                    // without updating the file format, we synthesize a value-slot range for the
                    // uniform here, and copy the uniform data into the value slots. This allows
                    // trace_var to work naturally. This wastes a bit of memory, but debug traces
                    // don't need to be hyper-efficient.
                    let copy_range = self.program_slots.get_variable_slots(&self.ctx.pool, var);
                    self.builder.push_uniform(uniform_slot_range);
                    self.pop_to_slot_range_unmasked(copy_range);
                }

                continue;
            }

            // Other globals are treated as normal variable declarations.
            self.write_var_declaration(var, decl_value)?;
        }

        Ok(())
    }

    /// `writeStatement`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1596-L1653 (chrome/m156)
    pub(super) fn write_statement(&mut self, s: StmtId) -> Res {
        let statement = self.ctx.pool.statement(s);
        let position = statement.position;
        let kind = statement.kind.clone();

        match &kind {
            // The debugger will stop on statements inside Blocks; there's no need for an
            // additional stop on the block's initial open-brace. Likewise, the debugger will stop
            // on the init-statement of a for statement, so we don't need to stop on the outer
            // for-statement itself as well.
            StatementKind::Block(_) | StatementKind::For(_) => {}
            // The debugger should stop on other statements.
            _ => self.emit_trace_line(position),
        }

        match kind {
            StatementKind::Block(b) => self.write_block(position, b.block_kind, &b.children),
            StatementKind::Break(_) => self.write_break_statement(),
            StatementKind::Continue(_) => self.write_continue_statement(),
            StatementKind::Do(d) => self.write_do_statement(d.statement, d.test),
            StatementKind::Expression(e) => self.write_expression_statement(e.expression),
            StatementKind::For(f) => self.write_for_statement(position, &f),
            StatementKind::If(i) => self.write_if_statement(i.test, i.if_true, i.if_false),
            StatementKind::Nop(_) => Ok(()),
            StatementKind::Return(r) => self.write_return_statement(r.expression),
            StatementKind::Switch(_) => self.write_switch_statement(s),
            StatementKind::VarDeclaration(v) => self.write_var_declaration(v.var, v.value),
            _ => unsupported(),
        }
    }

    /// `writeBlock`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1655-L1680 (chrome/m156)
    fn write_block(
        &mut self,
        position: crate::position::Position,
        block_kind: BlockKind,
        children: &[StmtId],
    ) -> Res {
        if block_kind == BlockKind::CompoundStatement {
            self.emit_trace_line(position);
            self.inside_compound_statement += 1;
        } else {
            self.push_trace_scope_mask();
            self.emit_trace_scope(1);
        }

        for &stmt in children {
            self.write_statement(stmt)?;
        }

        if block_kind == BlockKind::CompoundStatement {
            self.inside_compound_statement -= 1;
        } else {
            self.emit_trace_scope(-1);
            self.discard_trace_scope_mask();
        }

        Ok(())
    }

    /// `writeBreakStatement`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1682-L1688 (chrome/m156)
    fn write_break_statement(&mut self) -> Res {
        // If all lanes have reached this break, we can just branch straight to the break target
        // instead of updating masks.
        self.builder
            .branch_if_all_lanes_active(self.current_break_target);
        self.builder.mask_off_loop_mask();
        Ok(())
    }

    /// `writeContinueStatement`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1690-L1693 (chrome/m156)
    fn write_continue_statement(&mut self) -> Res {
        let mask = self
            .current_continue_mask
            .expect("a continue is inside a loop with a continue mask");
        self.builder.continue_op(mask.stack_id());
        Ok(())
    }

    /// `writeDoStatement`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1695-L1748 (chrome/m156)
    fn write_do_statement(&mut self, statement: StmtId, test: ExprId) -> Res {
        // Set up a break target.
        let previous_break_target = self.current_break_target;
        let break_target = self.builder.next_label_id();
        self.current_break_target = break_target;

        // Save off the original loop mask.
        self.builder.enable_execution_mask_writes();
        self.builder.push_loop_mask();

        // If `continue` is used in the loop...
        let loop_info = analysis::get_loop_control_flow_info(&self.ctx.pool, statement);
        let mut auto_continue_mask = ContinueMask::default();
        if loop_info.has_continue {
            // ... create a temporary slot for continue-mask storage.
            self.continue_mask_enable(&mut auto_continue_mask);
        }

        // Write the do-loop body.
        let label_id = self.builder.next_label_id();
        self.builder.label(label_id);

        self.continue_mask_enter_loop_body(&auto_continue_mask);

        self.write_statement(statement)?;

        self.continue_mask_exit_loop_body(&auto_continue_mask);

        // Point the debugger at the do-statement's test-expression before we run it.
        let test_position = self.ctx.pool.expression(test).position;
        self.emit_trace_line(test_position);

        // Emit the test-expression, in order to combine it with the loop mask.
        self.push_expression(test, true)?;

        // Mask off any lanes in the loop mask where the test-expression is false; this breaks the
        // loop. We don't use the test expression for anything else, so jettison it.
        self.builder.merge_loop_mask();
        self.discard_expression(1);

        // If any lanes are still running, go back to the top and run the loop body again.
        self.builder.branch_if_any_lanes_active(label_id);

        // If we hit a break statement on all lanes, we will branch here to escape from the loop.
        self.builder.label(break_target);

        // Restore the loop mask.
        self.builder.pop_loop_mask();
        self.builder.disable_execution_mask_writes();

        // End of scope: `autoContinueMask`, then `breakTarget`, are destroyed.
        self.continue_mask_drop(auto_continue_mask);
        self.current_break_target = previous_break_target;
        Ok(())
    }

    /// `writeMasklessForStatement`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1750-L1834 (chrome/m156)
    fn write_maskless_for_statement(&mut self, f: &crate::ir::ForStatement) -> Res {
        let unroll_info = f.unroll_info.expect("a maskless for loop has unroll info");
        debug_assert!(unroll_info.count > 0);
        let initializer = f
            .initializer
            .expect("a maskless for loop has an initializer");
        let test = f.test.expect("a maskless for loop has a test");
        let next = f.next.expect("a maskless for loop has a next-expression");

        // We want the loop index to disappear at the end of the loop, so wrap the for statement
        // in a trace scope.
        self.push_trace_scope_mask();
        self.emit_trace_scope(1);

        // If no lanes are active, skip over the loop entirely. This guards against looping
        // forever; with no lanes active, we wouldn't be able to write the loop variable back to
        // its slot, so we'd never make forward progress.
        let loop_exit_id = self.builder.next_label_id();
        let loop_body_id = self.builder.next_label_id();
        self.builder.branch_if_no_lanes_active(loop_exit_id);

        // Run the loop initializer.
        self.write_statement(initializer)?;

        // Write the for-loop body. We know the for-loop has a standard ES2 unrollable structure,
        // and that it runs for at least one iteration, so we can plow straight ahead into the
        // loop body instead of running the loop-test first.
        self.builder.label(loop_body_id);

        self.write_statement(f.statement)?;

        // Point the debugger at the for-statement's next-expression before we run it, or as
        // close as we can reasonably get.
        let next_position = self.ctx.pool.expression(next).position;
        self.emit_trace_line(next_position);

        // If the loop only runs for a single iteration, we are already done. If not...
        if unroll_info.count > 1 {
            // ... run the next-expression, and immediately discard its result.
            self.push_expression(next, false)?;
            let slots = self.expr_slots(next);
            self.discard_expression(slots);

            // Run the test-expression, and repeat the loop until the test-expression evaluates
            // false.
            self.push_expression(test, true)?;
            self.builder
                .branch_if_no_active_lanes_on_stack_top_equal(0, loop_body_id);

            // Jettison the test-expression.
            self.discard_expression(1);
        }

        self.builder.label(loop_exit_id);

        self.emit_trace_scope(-1);
        self.discard_trace_scope_mask();
        Ok(())
    }

    /// `writeForStatement`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1836-L1953 (chrome/m156)
    fn write_for_statement(
        &mut self,
        position: crate::position::Position,
        f: &crate::ir::ForStatement,
    ) -> Res {
        // If we've determined that the loop does not run, omit its code entirely.
        if f.unroll_info.is_some_and(|info| info.count == 0) {
            return Ok(());
        }

        // If the loop doesn't escape early due to a `continue`, `break` or `return`, and the loop
        // conforms to ES2 structure, we know that we will run the full number of iterations
        // across all lanes and don't need to use a loop mask.
        let loop_info = analysis::get_loop_control_flow_info(&self.ctx.pool, f.statement);
        if !loop_info.has_continue
            && !loop_info.has_break
            && !loop_info.has_return
            && f.unroll_info.is_some()
        {
            return self.write_maskless_for_statement(f);
        }

        // We want the loop index to disappear at the end of the loop, so wrap the for statement
        // in a trace scope.
        self.push_trace_scope_mask();
        self.emit_trace_scope(1);

        // Set up a break target.
        let previous_break_target = self.current_break_target;
        let break_target = self.builder.next_label_id();
        self.current_break_target = break_target;

        // Run the loop initializer.
        if let Some(initializer) = f.initializer {
            self.write_statement(initializer)?;
        } else {
            self.emit_trace_line(position);
        }

        let mut auto_continue_mask = ContinueMask::default();
        if loop_info.has_continue {
            // Acquire a temporary slot for continue-mask storage.
            self.continue_mask_enable(&mut auto_continue_mask);
        }

        // Save off the original loop mask.
        self.builder.enable_execution_mask_writes();
        self.builder.push_loop_mask();

        let loop_test_id = self.builder.next_label_id();
        let loop_body_id = self.builder.next_label_id();

        // Jump down to the loop test so we can fall out of the loop immediately if it's
        // zero-iteration.
        self.builder.jump(loop_test_id);

        // Write the for-loop body.
        self.builder.label(loop_body_id);

        self.continue_mask_enter_loop_body(&auto_continue_mask);

        self.write_statement(f.statement)?;

        self.continue_mask_exit_loop_body(&auto_continue_mask);

        // Point the debugger at the for-statement's next-expression before we run it, or as close
        // as we can reasonably get.
        if let Some(next) = f.next {
            let next_position = self.ctx.pool.expression(next).position;
            self.emit_trace_line(next_position);
        } else if let Some(test) = f.test {
            let test_position = self.ctx.pool.expression(test).position;
            self.emit_trace_line(test_position);
        } else {
            self.emit_trace_line(position);
        }

        // Run the next-expression. Immediately discard its result.
        if let Some(next) = f.next {
            self.push_expression(next, false)?;
            let slots = self.expr_slots(next);
            self.discard_expression(slots);
        }

        self.builder.label(loop_test_id);
        if let Some(test) = f.test {
            // Emit the test-expression, in order to combine it with the loop mask.
            self.push_expression(test, true)?;
            // Mask off any lanes in the loop mask where the test-expression is false; this breaks
            // the loop. We don't use the test expression for anything else, so jettison it.
            self.builder.merge_loop_mask();
            self.discard_expression(1);
        }

        // If any lanes are still running, go back to the top and run the loop body again.
        self.builder.branch_if_any_lanes_active(loop_body_id);

        // If we hit a break statement on all lanes, we will branch here to escape from the loop.
        self.builder.label(break_target);

        // Restore the loop mask.
        self.builder.pop_loop_mask();
        self.builder.disable_execution_mask_writes();

        self.emit_trace_scope(-1);
        self.discard_trace_scope_mask();

        // End of scope: `autoContinueMask`, then `breakTarget`, are destroyed.
        self.continue_mask_drop(auto_continue_mask);
        self.current_break_target = previous_break_target;
        Ok(())
    }

    /// `writeExpressionStatement`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1955-L1961 (chrome/m156)
    fn write_expression_statement(&mut self, expression: ExprId) -> Res {
        self.push_expression(expression, false)?;
        let slots = self.expr_slots(expression);
        self.discard_expression(slots);
        Ok(())
    }

    /// `writeDynamicallyUniformIfStatement`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L1963-L2001 (chrome/m156)
    fn write_dynamically_uniform_if_statement(
        &mut self,
        test: ExprId,
        if_true: StmtId,
        if_false: Option<StmtId>,
    ) -> Res {
        debug_assert!(analysis::is_dynamically_uniform_expression(
            &self.ctx.pool,
            test
        ));

        let false_label_id = self.builder.next_label_id();
        let exit_label_id = self.builder.next_label_id();

        self.push_expression(test, true)?;

        self.builder
            .branch_if_no_active_lanes_on_stack_top_equal(!0, false_label_id);

        self.write_statement(if_true)?;

        if let Some(if_false) = if_false {
            // We do have an if-false condition. We've just completed the if-true block, so we
            // need to jump past the if-false block to avoid executing it.
            self.builder.jump(exit_label_id);

            // The if-false block starts here.
            self.builder.label(false_label_id);

            self.write_statement(if_false)?;

            self.builder.label(exit_label_id);
        } else {
            // We don't have an if-false condition at all.
            self.builder.label(false_label_id);
        }

        // Jettison the test-expression.
        self.discard_expression(1);
        Ok(())
    }

    /// `writeIfStatement`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2003-L2036 (chrome/m156)
    fn write_if_statement(
        &mut self,
        test: ExprId,
        if_true: StmtId,
        if_false: Option<StmtId>,
    ) -> Res {
        // If the test condition is known to be uniform, we can skip over the untrue portion
        // entirely.
        if analysis::is_dynamically_uniform_expression(&self.ctx.pool, test) {
            return self.write_dynamically_uniform_if_statement(test, if_true, if_false);
        }

        // Save the current condition-mask.
        self.builder.enable_execution_mask_writes();
        self.builder.push_condition_mask();

        // Push the test condition mask.
        self.push_expression(test, true)?;

        // Merge the current condition-mask with the test condition, then run the if-true branch.
        self.builder.merge_condition_mask();
        self.write_statement(if_true)?;

        if let Some(if_false) = if_false {
            // Apply the inverse condition-mask. Then run the if-false branch.
            self.builder.merge_inv_condition_mask();
            self.write_statement(if_false)?;
        }

        // Jettison the test-expression, and restore the the condition-mask.
        self.discard_expression(1);
        self.builder.pop_condition_mask();
        self.builder.disable_execution_mask_writes();

        Ok(())
    }

    /// `writeReturnStatement`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2038-L2050 (chrome/m156)
    fn write_return_statement(&mut self, expression: Option<ExprId>) -> Res {
        let current = self.current_function.expect("a current function");
        if let Some(expression) = expression {
            self.push_expression(expression, true)?;
            if self.needs_function_result_slots(current) {
                let result = self.current_function_result;
                self.pop_to_slot_range(result);
            }
        }
        if self.builder.execution_mask_writes_are_enabled() && self.needs_return_mask(current) {
            self.builder.mask_off_return_mask();
        }
        Ok(())
    }

    /// `writeSwitchStatement`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2052-L2117 (chrome/m156)
    fn write_switch_statement(&mut self, s: StmtId) -> Res {
        let StatementKind::Switch(switch) = self.ctx.pool.statement(s).kind.clone() else {
            panic!("writeSwitchStatement: not a switch");
        };
        let cases: Vec<StmtId> = SwitchStatement::cases(&switch, &self.ctx.pool).to_vec();

        // Set up a break target.
        let previous_break_target = self.current_break_target;
        let break_target = self.builder.next_label_id();
        self.current_break_target = break_target;

        // Save off the original loop mask.
        self.builder.enable_execution_mask_writes();
        self.builder.push_loop_mask();

        // Push the switch-case value, and write a default-mask that enables every lane which
        // already has an active loop mask. As we match cases, the default mask will get pared
        // down.
        self.push_expression(switch.value, true)?;
        self.builder.push_loop_mask();

        // Zero out the loop mask; each case op will re-enable it as we go.
        self.builder.mask_off_loop_mask();

        // Write each switch-case.
        let mut found_default_case = false;
        for &stmt in &cases {
            let skip_label_id = self.builder.next_label_id();

            let StatementKind::SwitchCase(sc) = self.ctx.pool.statement(stmt).kind.clone() else {
                panic!("a switch's case block holds only SwitchCases");
            };
            if sc.is_default {
                found_default_case = true;
                if Some(&stmt) != cases.last() {
                    // We only support a default case when it is the very last case. If that
                    // changes, this logic will need to be updated.
                    return unsupported();
                }
                // Keep whatever lanes are executing now, and also enable any lanes in the default
                // mask.
                self.builder.pop_and_reenable_loop_mask();
                // Execute the switch-case block, if any lanes are alive to see it.
                self.builder.branch_if_no_lanes_active(skip_label_id);
                self.write_statement(sc.statement)?;
            } else {
                // The case-op will enable the loop mask if the switch-value matches, and mask off
                // lanes from the default-mask. (`int` narrows the `SKSL_INT`.)
                #[allow(clippy::cast_possible_truncation)]
                self.builder.case_op(sc.value as i32);
                // Execute the switch-case block, if any lanes are alive to see it.
                self.builder.branch_if_no_lanes_active(skip_label_id);
                self.write_statement(sc.statement)?;
            }
            self.builder.label(skip_label_id);
        }

        // Jettison the switch value, and the default case mask if it was never consumed above.
        self.discard_expression(if found_default_case { 1 } else { 2 });

        // If we hit a break statement on all lanes, we will branch here to escape from the
        // switch.
        self.builder.label(break_target);

        // Restore the loop mask.
        self.builder.pop_loop_mask();
        self.builder.disable_execution_mask_writes();
        self.current_break_target = previous_break_target;
        Ok(())
    }

    /// `writeImmutableVarDeclaration`: `false` if the variable cannot be immutable.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2119-L2160 (chrome/m156)
    fn write_immutable_var_declaration(&mut self, var: VarId, value: ExprId) -> bool {
        // In a debugging session, we expect debug traces for a variable declaration to appear,
        // even if it's constant, so we don't use immutable slots for variables when tracing is
        // on.
        if self.should_write_trace_ops() {
            return false;
        }

        // Find the constant value for this variable.
        let initial_value = constant_folder::get_constant_value_for_variable(&self.ctx.pool, value);

        // For a variable to be immutable, it cannot be written-to besides its initial
        // declaration.
        let counts = self.usage.get_variable(var);
        if counts.write != 1 {
            return false;
        }

        let mut immutable_values: Vec<ImmutableBits> = Vec::new();
        if !self.get_immutable_value_for_expression(initial_value, &mut immutable_values) {
            return false;
        }

        self.immutable_variables.insert(var);

        let preexisting_slots = self.find_preexisting_immutable_data(&immutable_values);
        if let Some(preexisting) = preexisting_slots {
            // Associate this variable with a preexisting range of immutable data (no new data or
            // code).
            self.immutable_slots
                .map_variable_to_slots(&self.ctx.pool, var, preexisting);
        } else {
            // Write out the constant value back to immutable slots. (This generates data, but no
            // runtime code.)
            let slots = self.get_immutable_slots(var);
            self.store_immutable_value_to_slots(&immutable_values, slots);
        }

        true
    }

    /// `writeVarDeclaration`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2162-L2178 (chrome/m156)
    pub(super) fn write_var_declaration(&mut self, var: VarId, value: Option<ExprId>) -> Res {
        if let Some(value) = value {
            // If a variable never actually changes, we can make it immutable.
            if self.write_immutable_var_declaration(var, value) {
                return Ok(());
            }
            // This is a real variable which can change over the course of execution.
            self.push_expression(value, true)?;
            let slots = self.get_variable_slots(var);
            self.pop_to_slot_range_unmasked(slots);
        } else {
            let slots = self.get_variable_slots(var);
            self.zero_slot_range_unmasked(slots);
        }
        Ok(())
    }

    // ---- immutable data ---------------------------------------------------------------------

    /// `getImmutableBitsForSlot`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2601-L2629 (chrome/m156)
    pub(super) fn get_immutable_bits_for_slot(
        &self,
        expr: ExprId,
        slot: usize,
    ) -> Option<ImmutableBits> {
        // Determine the constant-value of the slot; bail if it isn't constant.
        let pool = &self.ctx.pool;
        let e = pool.expression(expr);
        let value = e.get_constant_value(pool, slot)?;
        // Determine the number-kind of the slot, and convert the value to its bit-representation.
        let kind = pool.ty(e.ty).slot_type(slot).number_kind();
        match kind {
            // `sk_bit_cast<ImmutableBits>((float)value)`.
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            NumberKind::Float => Some((value as f32).to_bits() as i32),
            NumberKind::Signed => Some(double_to_i32(value)),
            // `sk_bit_cast<ImmutableBits>((uint32_t)value)`.
            #[allow(clippy::cast_possible_wrap)]
            NumberKind::Unsigned => Some(double_to_u32(value) as i32),
            NumberKind::Boolean => Some(if value == 0.0 { 0 } else { !0 }),
            NumberKind::Nonnumeric => None,
        }
    }

    /// `getImmutableValueForExpression`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2631-L2646 (chrome/m156)
    pub(super) fn get_immutable_value_for_expression(
        &self,
        expr: ExprId,
        immutable_values: &mut Vec<ImmutableBits>,
    ) -> bool {
        if !self.ctx.pool.expression(expr).supports_constant_values() {
            return false;
        }
        let num_slots = self.ctx.pool.ty(self.expr_type(expr)).slot_count();
        immutable_values.reserve_exact(num_slots);
        for index in 0..num_slots {
            let Some(bits) = self.get_immutable_bits_for_slot(expr, index) else {
                return false;
            };
            immutable_values.push(bits);
        }
        true
    }

    /// `storeImmutableValueToSlots`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2648-L2660 (chrome/m156)
    pub(super) fn store_immutable_value_to_slots(
        &mut self,
        immutable_values: &[ImmutableBits],
        mut slots: SlotRange,
    ) {
        for &bits in immutable_values
            .iter()
            .take(usize::try_from(slots.count).unwrap_or(0))
        {
            // Store the immutable value in its slot.
            let slot = slots.index;
            slots.index += 1;
            self.builder.store_immutable_value_i(slot, bits);

            // Keep track of every stored immutable value for potential later reuse.
            self.immutable_slot_map
                .get_or_insert_default(&bits)
                .add(slot);
        }
    }

    /// `findPreexistingImmutableData`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2662-L2707 (chrome/m156)
    pub(super) fn find_preexisting_immutable_data(
        &self,
        immutable_values: &[ImmutableBits],
    ) -> Option<SlotRange> {
        let mut slot_array = Vec::with_capacity(immutable_values.len());

        // Find all the slots associated with each immutable-value bit representation. If a given
        // bit-pattern doesn't exist anywhere in our program yet, we can stop searching.
        for immutable_value in immutable_values {
            let slots_for_value = self.immutable_slot_map.find(immutable_value)?;
            slot_array.push(slots_for_value);
        }
        if slot_array.is_empty() {
            return None;
        }

        // Look for the group with the fewest number of entries, since that can be searched in the
        // least amount of effort.
        let mut least_slot_index = 0;
        let mut least_slot_count = i32::MAX;
        for (index, slots) in slot_array.iter().enumerate() {
            let current_count = slots.count();
            if current_count < least_slot_count {
                least_slot_index = index;
                least_slot_count = current_count;
            }
        }

        // See if we can reconstitute the value that we want with any of the data we've already
        // got.
        let least_slot_index_i32 = i32::try_from(least_slot_index).expect("index fits i32");
        for &slot in slot_array[least_slot_index].iter() {
            let first_slot = slot - least_slot_index_i32;
            let mut found = true;
            for (index, slots) in slot_array.iter().enumerate() {
                let probe = first_slot + i32::try_from(index).expect("index fits i32");
                if !slots.contains(&probe) {
                    found = false;
                    break;
                }
            }
            if found {
                // We've found an exact match for the input value; return its slot-range.
                return Some(SlotRange {
                    index: first_slot,
                    count: i32::try_from(slot_array.len()).expect("count fits i32"),
                });
            }
        }

        // We didn't find any reusable slot ranges.
        None
    }

    /// `pushImmutableData`: `false` if the expression has no immutable representation.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L2709-L2726 (chrome/m156)
    pub(super) fn push_immutable_data(&mut self, e: ExprId) -> bool {
        let mut immutable_values: Vec<ImmutableBits> = Vec::new();
        if !self.get_immutable_value_for_expression(e, &mut immutable_values) {
            return false;
        }
        let preexisting_data = self.find_preexisting_immutable_data(&immutable_values);
        if let Some(preexisting) = preexisting_data {
            self.builder.push_immutable(preexisting);
            return true;
        }
        let (ty, pos) = {
            let node = self.ctx.pool.expression(e);
            (node.ty, node.position)
        };
        let pool = &self.ctx.pool;
        let range =
            self.immutable_slots
                .create_slots(pool, || pool.expression_description(e), ty, pos);
        self.store_immutable_value_to_slots(&immutable_values, range);
        self.builder.push_immutable(range);
        true
    }
}
