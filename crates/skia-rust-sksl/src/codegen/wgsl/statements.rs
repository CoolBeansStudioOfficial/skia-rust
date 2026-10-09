// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp (the statement writers).

//! Statements: blocks, control flow, switch emulation and variable declarations.

use std::mem;

use super::types::to_wgsl_type;
use super::{AssembleMode, WgslCodeGenerator};
use crate::analysis::{is_compile_time_constant, switch_case_contains_unconditional_exit};
use crate::ir::{ExprId, ExpressionKind, PrefixExpression, StatementKind, StmtId};
use crate::operator::{Operator, OperatorKind, OperatorPrecedence};

type Precedence = OperatorPrecedence;

/// A switch case, copied out of the IR: its statement, whether it is the default and its value.
#[derive(Clone, Copy)]
struct CaseInfo {
    statement: StmtId,
    is_default: bool,
    value: i64,
}

impl WgslCodeGenerator<'_> {
    fn case_info(&self, case: StmtId) -> CaseInfo {
        match &self.ctx.pool.statement(case).kind {
            StatementKind::SwitchCase(sc) => CaseInfo {
                statement: sc.statement,
                is_default: sc.is_default,
                value: sc.value,
            },
            _ => unreachable!("a switch holds only switch cases"),
        }
    }

    /// `writeStatement(s)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2118-L2162 (chrome/m156)
    pub(super) fn write_statement(&mut self, s: StmtId) {
        let kind = self.ctx.pool.statement(s).kind.clone();
        match kind {
            StatementKind::Block(_) => self.write_block(s),
            StatementKind::Break(_) => self.write_line("break;"),
            StatementKind::Continue(_) => self.write_line("continue;"),
            StatementKind::Discard(_) => self.write_line("discard;"),
            StatementKind::Do(d) => self.write_do_statement(d.statement, d.test),
            StatementKind::Expression(e) => self.write_expression_statement(e.expression),
            StatementKind::For(_) => self.write_for_statement(s),
            StatementKind::If(i) => self.write_if_statement(i.test, i.if_true, i.if_false),
            StatementKind::Nop(_) => self.write_line(";"),
            StatementKind::Return(r) => self.write_return_statement(r.expression),
            StatementKind::Switch(_) => self.write_switch_statement(s),
            StatementKind::SwitchCase(_) => {
                // switch-case statements should only be present inside a switch
            }
            StatementKind::VarDeclaration(_) => self.write_var_declaration(s),
        }
    }

    /// `writeStatements(statements)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2164-L2171 (chrome/m156)
    fn write_statements(&mut self, statements: &[StmtId]) {
        for &s in statements {
            if !self.ctx.pool.statement(s).is_empty(&self.ctx.pool) {
                self.write_statement(s);
                self.finish_line();
            }
        }
    }

    /// `writeBlock(b)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2173-L2186 (chrome/m156)
    pub(super) fn write_block(&mut self, block: StmtId) {
        let StatementKind::Block(b) = self.ctx.pool.statement(block).kind.clone() else {
            unreachable!("writeBlock takes a block");
        };
        // Write scope markers if this block is a scope, or if the block is empty (since we need
        // to emit something here to make the code valid).
        let is_scope = b.is_scope() || b.is_empty(&self.ctx.pool);
        if is_scope {
            self.write_line("{");
            self.indentation += 1;
        }
        self.write_statements(&b.children);
        if is_scope {
            self.indentation -= 1;
            self.write_line("}");
        }
    }

    /// `writeExpressionStatement(expr)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2188-L2195 (chrome/m156)
    pub(super) fn write_expression_statement(&mut self, expr: ExprId) {
        // Any expression-related side effects must be emitted as separate statements when
        // `assembleExpression` is called.
        // The final result of the expression will be a variable, let-reference, or an expression
        // with no side effects (`foo + bar`). Discarding this result is safe, as the program
        // never uses it.
        let _ = self.assemble_expression(expr, Precedence::Statement, AssembleMode::Auto);
    }

    /// `writeDoStatement(s)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2197-L2229 (chrome/m156)
    fn write_do_statement(&mut self, statement: StmtId, test: ExprId) {
        // Generate a loop structure like this:
        //   loop {
        //       body-statement;
        //       continuing {
        //           break if inverted-test-expression;
        //       }
        //   }

        self.conditional_scope_depth += 1;

        let test_position = self.ctx.pool.expression(test).position;
        let cloned_test = self.ctx.pool.clone_expression(test);
        let inverted_test_expr = PrefixExpression::make(
            self.ctx,
            test_position,
            Operator::from(OperatorKind::LogicalNot),
            cloned_test,
        );

        self.write_line("loop {");
        self.indentation += 1;
        self.write_statement(statement);
        self.finish_line();

        self.write_line("continuing {");
        self.indentation += 1;
        let break_if_expr = self.assemble_expression(
            inverted_test_expr,
            Precedence::EXPRESSION,
            AssembleMode::Auto,
        );
        self.write("break if ");
        self.write(&break_if_expr);
        self.write_line(";");
        self.indentation -= 1;
        self.write_line("}");
        self.indentation -= 1;
        self.write_line("}");

        self.conditional_scope_depth -= 1;
    }

    /// `writeForStatement(s)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2231-L2394 (chrome/m156)
    fn write_for_statement(&mut self, stmt: StmtId) {
        let StatementKind::For(s) = self.ctx.pool.statement(stmt).kind.clone() else {
            unreachable!("writeForStatement takes a for statement");
        };
        // When the next expression is a single expression and the condition needs no scratch
        // variables, emit a native for-loop:
        //
        //   {
        //       init;
        //       for (; cond; next) {
        //           body
        //       }
        //   }
        //
        // Otherwise (comma next expression, or condition assembly emits preparatory 'let'
        // statements), fall back to:
        //
        //   {
        //       init;
        //       loop {
        //           cond-prep-stmts;   // scratch lets for the condition, if any
        //           if (cond) {
        //               body
        //           } else {
        //               break;
        //           }
        //           continuing {
        //               next;
        //           }
        //       }
        //   }
        //
        // The outer block scopes any init variable declarations.

        self.conditional_scope_depth += 1;

        // If there is an initializer, wrap in an outer block to scope any variable declarations.
        // This must be emitted regardless of unroll count.
        if let Some(initializer) = s.initializer {
            self.write_line("{");
            self.indentation += 1;
            self.write_statement(initializer);
            self.finish_line();
        }

        if s.unroll_info.as_ref().is_some_and(|info| info.count <= 0) {
            // Loops which are known to never execute don't need to be emitted at all.
            // (However, the front end should have already replaced this loop with a Nop.)
        } else {
            let has_comma_next = s.next.is_some_and(|next| {
                matches!(
                    &self.ctx.pool.expression(next).kind,
                    ExpressionKind::Binary(b) if b.operator.kind() == OperatorKind::Comma
                )
            });

            // Assemble cond and next into temporary buffers. If either assembly allocates
            // scratch variables (_skTemp), those must run on every loop iteration and the
            // native for-loop form cannot be used; fall back to loop{} in that case.
            let scratch_before = self.scratch_count;

            let mut cond_buf = String::new();
            let mut cond_str = String::new();
            if let Some(test) = s.test {
                // `AutoOutputStream captureOutput(this, &condBuf, &fIndentation)`.
                let old_out = mem::take(&mut self.out);
                let old_indentation = mem::replace(&mut self.indentation, 0);
                cond_str =
                    self.assemble_expression(test, Precedence::EXPRESSION, AssembleMode::Auto);
                cond_buf = mem::replace(&mut self.out, old_out);
                self.indentation = old_indentation;
            }

            let mut next_buf = String::new();
            if let Some(next) = s.next {
                let old_out = mem::take(&mut self.out);
                let old_indentation = mem::replace(&mut self.indentation, 0);
                self.write_expression_statement(next);
                next_buf = mem::replace(&mut self.out, old_out);
                self.indentation = old_indentation;
            }

            let needs_loop_form = has_comma_next || (self.scratch_count != scratch_before);

            if needs_loop_form {
                // Comma next, or scratch variables needed: use
                // loop { cond-scratch; if (cond) { body } else { break; } continuing { next; } }.
                self.write_line("loop {");
                self.indentation += 1;

                // Inline cond prep statements on the same line as the if-check by replacing
                // newlines with spaces (avoids indentation issues with a pre-captured string).
                let cond_prep = cond_buf.replace('\n', " ");

                self.write(&cond_prep);

                if cond_str.is_empty() {
                    self.write_statement(s.statement);
                    self.finish_line();
                } else {
                    self.write("if ");
                    self.write(&cond_str);
                    self.write_line(" {");
                    self.indentation += 1;
                    self.write_statement(s.statement);
                    self.finish_line();
                    self.indentation -= 1;
                    self.write_line("} else {");
                    self.indentation += 1;
                    self.write_line("break;");
                    self.indentation -= 1;
                    self.write_line("}");
                }

                if s.next.is_some() {
                    self.write_line("continuing {");
                    self.indentation += 1;
                    let next_str = next_buf.replace('\n', " ");
                    self.write(&next_str);
                    self.finish_line();
                    self.indentation -= 1;
                    self.write_line("}");
                }

                self.indentation -= 1;
                self.write_line("}");
            } else {
                // Single next expression, no scratch variables: emit a native for-loop.
                // Strip the trailing ';' and newline from nextBuf to get the for-update
                // expression.
                let mut update_str = next_buf;
                while update_str.ends_with(['\n', '\r']) {
                    update_str.pop();
                }
                if update_str.ends_with(';') {
                    update_str.pop();
                }

                self.write("for (;");
                if !cond_str.is_empty() {
                    self.write(" ");
                    self.write(&cond_str);
                }
                self.write("; ");
                self.write(&update_str);
                self.write_line(") {");

                self.indentation += 1;
                self.write_statement(s.statement);
                self.finish_line();
                self.indentation -= 1;
                self.write_line("}");
            }
        }

        if s.initializer.is_some() {
            self.indentation -= 1;
            self.write_line("}");
        }

        self.conditional_scope_depth -= 1;
    }

    /// `writeIfStatement(s)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2396-L2415 (chrome/m156)
    fn write_if_statement(&mut self, test: ExprId, if_true: StmtId, if_false: Option<StmtId>) {
        self.conditional_scope_depth += 1;

        let test_expr = self.assemble_expression(test, Precedence::EXPRESSION, AssembleMode::Auto);
        self.write("if ");
        self.write(&test_expr);
        self.write_line(" {");
        self.indentation += 1;
        self.write_statement(if_true);
        self.finish_line();
        self.indentation -= 1;
        if let Some(if_false) = if_false {
            self.write_line("} else {");
            self.indentation += 1;
            self.write_statement(if_false);
            self.finish_line();
            self.indentation -= 1;
        }
        self.write_line("}");

        self.conditional_scope_depth -= 1;
    }

    /// `writeReturnStatement(s)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2417-L2426 (chrome/m156)
    fn write_return_statement(&mut self, expression: Option<ExprId>) {
        self.has_unconditional_return |= self.conditional_scope_depth == 0;

        let expr = match expression {
            Some(e) => self.assemble_expression(e, Precedence::EXPRESSION, AssembleMode::Auto),
            None => String::new(),
        };
        self.write("return ");
        self.write(&expr);
        self.write(";");
    }

    /// `writeSwitchCaseList(cases)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2428-L2438 (chrome/m156)
    fn write_switch_case_list(&mut self, cases: &[StmtId]) {
        let mut separator = crate::string::Separator::new();
        for &sc in cases {
            self.write(separator.next_str());
            let info = self.case_info(sc);
            if info.is_default {
                self.write("default");
            } else {
                self.write(&info.value.to_string());
            }
        }
    }

    /// `writeSwitchCases(cases)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2440-L2461 (chrome/m156)
    fn write_switch_cases(&mut self, cases: &[StmtId]) {
        if let Some(&last) = cases.last() {
            // Only the last switch-case should have a non-empty statement.
            debug_assert!(cases[..cases.len() - 1].iter().all(|&sc| {
                let stmt = self.case_info(sc).statement;
                self.ctx.pool.statement(stmt).is_empty(&self.ctx.pool)
            }));

            // Emit the cases in a comma-separated list.
            self.write("case ");
            self.write_switch_case_list(cases);
            self.write_line(" {");
            self.indentation += 1;

            // Emit the switch-case body.
            let body = self.case_info(last).statement;
            self.write_statement(body);
            self.finish_line();

            self.indentation -= 1;
            self.write_line("}");
        }
    }

    /// `writeEmulatedSwitchFallthroughCases(cases, switchValue)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2463-L2542 (chrome/m156)
    fn write_emulated_switch_fallthrough_cases(&mut self, cases: &[StmtId], switch_value: &str) {
        // There's no need for fallthrough handling unless we actually have multiple case blocks.
        if cases.len() < 2 {
            self.write_switch_cases(cases);
            return;
        }

        // Match against the entire case group.
        self.write("case ");
        self.write_switch_case_list(cases);
        self.write_line(" {");
        self.indentation += 1;

        let fallthrough_var = self.write_scratch_var(crate::ir::TypeId::BOOL, "false");
        let second_to_last_case_index = cases.len() - 2;
        let last_case_index = cases.len() - 1;

        for (index, &case) in cases.iter().enumerate() {
            let sc = self.case_info(case);
            if index < last_case_index {
                // The default case must come last in SkSL, and this case isn't the last one, so
                // it can't possibly be the default.
                debug_assert!(!sc.is_default);

                self.write("if ");
                if index > 0 {
                    self.write(&fallthrough_var);
                    self.write(" || ");
                }
                self.write(switch_value);
                self.write(" == ");
                self.write(&sc.value.to_string());
                self.write_line(" {");
                self.indentation += 1;

                // We write the entire case-block statement here, and then set
                // `switchFallthrough` to 1. If the case-block had a break statement in it, we
                // break out of the outer for-loop entirely, meaning the `switchFallthrough`
                // assignment never occurs, nor does any code after it inside the switch. We've
                // forbidden `continue` statements inside switch case-blocks entirely, so we
                // don't need to consider their effect on control flow; see the Finalizer in
                // FunctionDefinition::Convert.
                self.write_statement(sc.statement);
                self.finish_line();

                if index < second_to_last_case_index {
                    // Set a variable to indicate falling through to the next block. The very last
                    // case-block is reached by process of elimination and doesn't need this
                    // variable, so we don't actually need to set it if we are on the
                    // second-to-last case block.
                    self.write(&fallthrough_var);
                    self.write(" = true;  ");
                }
                // (Skia adds `// fallthrough` here in SK_DEBUG builds only; the goldens are not
                // made by one.)

                self.indentation -= 1;
                self.write_line("}");
            } else {
                // This is the final case. Since it's always last, we can just dump in the code.
                // (If we didn't match any of the other values, we must have matched this one by
                // process of elimination. If we did match one of the other values, we either hit
                // a `break` statement earlier--and won't get this far--or we're falling
                // through.)
                self.write_statement(sc.statement);
                self.finish_line();
            }
        }

        self.indentation -= 1;
        self.write_line("}");
    }

    /// `writeSwitchStatement(s)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2544-L2653 (chrome/m156)
    fn write_switch_statement(&mut self, stmt: StmtId) {
        let StatementKind::Switch(s) = self.ctx.pool.statement(stmt).kind.clone() else {
            unreachable!("writeSwitchStatement takes a switch statement");
        };
        // WGSL supports the `switch` statement in a limited capacity. A default case must always
        // be specified. Each switch-case must be scoped inside braces. Fallthrough is not
        // supported; a trailing break is implied at the end of each switch-case block. (Explicit
        // breaks are also allowed.)  One minor improvement over a traditional switch is that
        // switch-cases take a list of values to match, instead of a single value:
        //   case 1, 2       { foo(); }
        //   case 3, default { bar(); }
        //
        // We will use the native WGSL switch statement for any switch-cases in the SkSL which can
        // be made to conform to these limitations. The remaining cases which cannot conform will
        // be emulated with if-else blocks (similar to our GLSL ES2 switch-statement emulation
        // path). This should give us good performance in the common case, since most switches
        // naturally conform.

        // First, let's emit the switch itself.
        let value_expr = self.assemble_expression(
            s.value,
            Precedence::EXPRESSION,
            AssembleMode::UsedMultipleTimes,
        );
        self.write("switch ");
        self.write(&value_expr);
        self.write_line(" {");
        self.indentation += 1;

        // Now let's go through the switch-cases, and emit the ones that don't fall through.
        let mut native_cases: Vec<StmtId> = Vec::new();
        let mut fallthrough_cases: Vec<StmtId> = Vec::new();
        let mut previous_case_fell_through = false;
        let mut found_native_default = false;
        let mut found_fallthrough_default = false;

        let all_cases: Vec<StmtId> = s.cases(&self.ctx.pool).to_vec();
        let last_switch_case_idx = all_cases.len() - 1;
        for (index, &case) in all_cases.iter().enumerate() {
            let sc = self.case_info(case);

            if self
                .ctx
                .pool
                .statement(sc.statement)
                .is_empty(&self.ctx.pool)
            {
                // This is a `case X:` that immediately falls through to the next case.
                // If we aren't already falling through, we can handle this via a comma-separated
                // list.
                if previous_case_fell_through {
                    fallthrough_cases.push(case);
                    found_fallthrough_default |= sc.is_default;
                } else {
                    native_cases.push(case);
                    found_native_default |= sc.is_default;
                }
                continue;
            }

            if index == last_switch_case_idx
                || switch_case_contains_unconditional_exit(&self.ctx.pool, case)
            {
                // This is a `case X:` that never falls through.
                if previous_case_fell_through {
                    // Because the previous cases fell through, we can't use a native switch-case
                    // here.
                    fallthrough_cases.push(case);
                    found_fallthrough_default |= sc.is_default;

                    self.write_emulated_switch_fallthrough_cases(&fallthrough_cases, &value_expr);
                    fallthrough_cases.clear();

                    // Fortunately, we're no longer falling through blocks, so we might be able to
                    // use a native switch-case list again.
                    previous_case_fell_through = false;
                } else {
                    // Emit a native switch-case block with a comma-separated case list.
                    native_cases.push(case);
                    found_native_default |= sc.is_default;

                    self.write_switch_cases(&native_cases);
                    native_cases.clear();
                }
                continue;
            }

            // This case falls through, so it will need to be handled via emulation.
            // If we have put together a collection of "native" cases (cases that fall through
            // with no actual case-body), we will need to slide them over into the
            // fallthrough-case list.
            fallthrough_cases.append(&mut native_cases);

            fallthrough_cases.push(case);
            found_fallthrough_default |= sc.is_default;
            previous_case_fell_through = true;
        }

        // Finish out the remaining switch-cases.
        self.write_switch_cases(&native_cases);
        native_cases.clear();

        self.write_emulated_switch_fallthrough_cases(&fallthrough_cases, &value_expr);
        fallthrough_cases.clear();

        // WGSL requires a default case.
        if !found_native_default && !found_fallthrough_default {
            self.write_line("case default {}");
        }

        self.indentation -= 1;
        self.write_line("}");
    }

    /// `writeVarDeclaration(varDecl)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2655-L2683 (chrome/m156)
    fn write_var_declaration(&mut self, stmt: StmtId) {
        let StatementKind::VarDeclaration(var_decl) = self.ctx.pool.statement(stmt).kind.clone()
        else {
            unreachable!("writeVarDeclaration takes a variable declaration");
        };
        let initial_value = match var_decl.value {
            Some(value) => {
                self.assemble_expression(value, Precedence::Assignment, AssembleMode::Auto)
            }
            None => String::new(),
        };

        let var = self.ctx.pool.variable(var_decl.var).clone();
        if var.modifier_flags.is_const()
            || (self.usage.get_variable(var_decl.var).write == 1 && var_decl.value.is_some())
        {
            // Use `const` at global scope, or if the value is a compile-time constant.
            let value = var_decl
                .value
                .expect("an immutable variable must specify a value");
            let use_const =
                !self.at_function_scope || is_compile_time_constant(&self.ctx.pool, value);
            self.write(if use_const { "const " } else { "let " });
        } else {
            self.write("var ");
        }
        let name = self.assemble_name(var.mangled_name());
        self.write(&name);
        self.write(": ");
        let ty = to_wgsl_type(self.ctx, var.ty, Some(&var.layout), false);
        self.write(&ty);

        if var_decl.value.is_some() {
            self.write(" = ");
            self.write(&initial_value);
        }

        self.write(";");
    }
}
