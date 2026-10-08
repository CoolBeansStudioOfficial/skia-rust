// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis/SkSLGetLoopUnrollInfo.cpp (chrome/m156) and
// src/sksl/analysis/SkSLGetLoopControlFlowInfo.cpp (chrome/m156).

//! [`get_loop_unroll_info`], which decides how many times a strict-ES2 `for` loop runs, and
//! [`get_loop_control_flow_info`], which finds the `break`, `continue` and `return` that affect
//! a loop.

use super::statement_queries::statement_writes_to_variable;
use super::{ProgramVisitor, walk_statement};
use crate::base_helpers::{SafeMath, double_saturate2int};
use crate::constant_folder::get_constant_value;
use crate::context::Context;
use crate::error_reporter::ErrorReporter;
use crate::error_reporter::forward_errors;
use crate::ir::{
    BinaryExpression, ExprId, ExpressionKind, IrPool, LoopUnrollInfo, StatementKind, StmtId, VarId,
};
use crate::operator::{Operator, OperatorKind};
use crate::position::{ForLoopPositions, Position};

/// `kLoopTerminationLimit`: loops that run for more iterations than this are rejected (the
/// non-fuzzer build's value).
// Port of: src/sksl/analysis/SkSLGetLoopUnrollInfo.cpp#L30-L34 (chrome/m156)
const LOOP_TERMINATION_LIMIT: i32 = 100_000;

/// `Direction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Backwards,
    Forwards,
}

/// `Inclusive`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Inclusive {
    No,
    Yes,
}

/// `LoopType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoopType {
    Float,
    Int,
}

// Port of: src/sksl/analysis/SkSLGetLoopUnrollInfo.cpp#L46-L66 (chrome/m156)
// The float-to-int casts mirror the C++ `(int32_t)` conversions of the loop bounds.
#[allow(clippy::float_cmp)]
fn calculate_count_float(start: f64, end: f64, delta: f64, inclusive: Inclusive) -> i32 {
    let iterations = (end - start) / delta;
    let mut count = iterations.ceil();
    if inclusive == Inclusive::Yes && count == iterations {
        count += 1.0;
    }
    if count > f64::from(LOOP_TERMINATION_LIMIT) || !count.is_finite() {
        // The loop runs for more iterations than we can safely unroll.
        return LOOP_TERMINATION_LIMIT;
    }
    double_saturate2int(count)
}

// Port of: src/sksl/analysis/SkSLGetLoopUnrollInfo.cpp#L68-L89 (chrome/m156)
fn calculate_count_int(start: i32, end: i32, delta: i32, inclusive: Inclusive) -> i32 {
    if delta == 0 {
        return LOOP_TERMINATION_LIMIT;
    }
    let mut math = SafeMath::new();
    let round_up = if delta > 0 {
        math.sub_int(delta, 1)
    } else {
        math.add_int(delta, 1)
    };
    let width = math.sub_int(end, start);
    // The overflow flag is sticky, so the order of these checks does not change the result.
    let sum = math.add_int(width, round_up);
    let mut iterations = math.div_int(sum, delta);
    if inclusive == Inclusive::Yes && math.mod_int(width, delta) == 0 {
        iterations = math.add_int(iterations, 1);
    }
    // Check that we won't overflow while looping.
    let product = math.mul_int(delta, iterations);
    math.add_int(start, product);
    if !math.ok() || !(0..=LOOP_TERMINATION_LIMIT).contains(&iterations) {
        return LOOP_TERMINATION_LIMIT;
    }
    iterations
}

// Port of: src/sksl/analysis/SkSLGetLoopUnrollInfo.cpp#L91-L110 (chrome/m156)
// The casts mirror the C++ `(int32_t)` conversions of the loop bounds.
#[allow(clippy::cast_possible_truncation)]
fn calculate_count(
    start: f64,
    end: f64,
    delta: f64,
    dir: Direction,
    inclusive: Inclusive,
    loop_type: LoopType,
) -> i32 {
    if (dir == Direction::Forwards && start > end) || (dir == Direction::Backwards && start < end) {
        // The loop starts in a completed state (the start has already advanced past the end).
        return 0;
    }
    #[allow(clippy::float_cmp)] // Mirrors the C++ `delta == 0.0`.
    let delta_is_zero = delta == 0.0;
    if delta_is_zero
        || (delta > 0.0 && dir == Direction::Backwards)
        || (delta < 0.0 && dir == Direction::Forwards)
    {
        // The loop does not progress toward a completed state, and will never terminate.
        return LOOP_TERMINATION_LIMIT;
    }
    if loop_type == LoopType::Int {
        return calculate_count_int(start as i32, end as i32, delta as i32, inclusive);
    }
    calculate_count_float(start, end, delta, inclusive)
}

// Port of: src/sksl/analysis/SkSLGetLoopUnrollInfo.cpp#L112-L121 (chrome/m156)
fn calculate_count_neq_int(start: i32, end: i32, delta: i32) -> i32 {
    if delta == 0 {
        return LOOP_TERMINATION_LIMIT;
    }
    let mut math = SafeMath::new();
    let difference = math.sub_int(end, start);
    let iterations = math.div_int(difference, delta);
    // Check that we won't overflow while looping and that we actually hit `end`.
    let product = math.mul_int(delta, iterations);
    let last_value = math.add_int(start, product);
    if !math.ok() || last_value != end || !(0..=LOOP_TERMINATION_LIMIT).contains(&iterations) {
        return LOOP_TERMINATION_LIMIT;
    }
    iterations
}

// Port of: src/sksl/analysis/SkSLGetLoopUnrollInfo.cpp#L123-L134 (chrome/m156)
#[allow(clippy::float_cmp, clippy::cast_possible_truncation)]
fn calculate_count_neq_float(start: f64, end: f64, delta: f64) -> i32 {
    if delta == 0.0 {
        return LOOP_TERMINATION_LIMIT;
    }
    let iterations = (end - start) / delta;
    let count = iterations.ceil();
    if count < 0.0 || count != iterations || !iterations.is_finite() {
        // The loop doesn't reach the exact endpoint and so will never terminate.
        return LOOP_TERMINATION_LIMIT;
    }
    double_saturate2int(count)
}

// Port of: src/sksl/analysis/SkSLGetLoopUnrollInfo.cpp#L136-L143 (chrome/m156)
#[allow(clippy::cast_possible_truncation)]
fn calculate_count_neq(start: f64, end: f64, delta: f64, loop_type: LoopType) -> i32 {
    if loop_type == LoopType::Int {
        return calculate_count_neq_int(start as i32, end as i32, delta as i32);
    }
    calculate_count_neq_float(start, end, delta)
}

/// `Analysis::GetLoopUnrollInfo`: checks that a `for` loop has the strict-ES2 form
/// `for (T i = constant; i <relop> constant; i++ / i-- / i += constant / i -= constant)` with
/// a body that does not write `i`, and that it runs fewer than the termination limit times.
///
/// `loop_test` is the loop's condition slot: a `x != n` test on a float index is rewritten in
/// place to `x < n` or `x > n`. Errors go to `ctx.errors` when `report_errors` is set, and are
/// dropped otherwise.
// Port of: src/sksl/analysis/SkSLGetLoopUnrollInfo.cpp#L145-L393 (chrome/m156)
#[allow(clippy::too_many_arguments)] // Mirrors the C++ signature, one argument per loop part.
#[allow(clippy::too_many_lines)] // One function in Skia; its checks run in its order.
#[must_use]
pub fn get_loop_unroll_info(
    ctx: &mut Context,
    loop_pos: Position,
    positions: &ForLoopPositions,
    loop_initializer: Option<StmtId>,
    loop_test: &mut Option<ExprId>,
    loop_next: Option<ExprId>,
    loop_statement: StmtId,
    report_errors: bool,
) -> Option<LoopUnrollInfo> {
    // The checks report into a buffer, so that the context can be borrowed at the same time.
    let mut errors = ErrorReporter::forwarding();
    let info = unroll_info_checks(
        ctx,
        &mut errors,
        loop_pos,
        positions,
        loop_initializer,
        loop_test,
        loop_next,
        loop_statement,
    );
    if report_errors {
        forward_errors(&errors, &mut ctx.errors);
    }
    info
}

/// The checks of [`get_loop_unroll_info`], reporting into `errors`.
// The float comparisons mirror the C++ `==`/`!=` on the loop bounds.
#[allow(clippy::too_many_arguments)] // See `get_loop_unroll_info`.
#[allow(clippy::too_many_lines)] // See `get_loop_unroll_info`.
#[allow(clippy::float_cmp)]
fn unroll_info_checks(
    ctx: &mut Context,
    errors: &mut ErrorReporter,
    loop_pos: Position,
    positions: &ForLoopPositions,
    loop_initializer: Option<StmtId>,
    loop_test: &mut Option<ExprId>,
    loop_next: Option<ExprId>,
    loop_statement: StmtId,
) -> Option<LoopUnrollInfo> {
    //
    // init_declaration has the form: type_specifier identifier = constant_expression
    //
    let Some(initializer) = loop_initializer else {
        let pos = if positions.init_position.valid() {
            positions.init_position
        } else {
            loop_pos
        };
        errors.error(pos, "missing init declaration");
        return None;
    };
    let init_position = ctx.pool.statement(initializer).position;
    let StatementKind::VarDeclaration(init_decl) = &ctx.pool.statement(initializer).kind else {
        errors.error(init_position, "invalid init declaration");
        return None;
    };
    let (index, base_type, array_size, init_value) = (
        init_decl.var,
        init_decl.base_type,
        init_decl.array_size,
        init_decl.value,
    );
    if !ctx.pool.ty(base_type).is_number() {
        errors.error(init_position, "invalid type for loop index");
        return None;
    }
    if array_size != 0 {
        errors.error(init_position, "invalid type for loop index");
        return None;
    }
    let Some(init_value) = init_value else {
        errors.error(init_position, "missing loop index initializer");
        return None;
    };
    let Some(start) = get_constant_value(&ctx.pool, init_value) else {
        errors.error(
            init_position,
            "loop index initializer must be a constant expression",
        );
        return None;
    };
    let mut loop_info = LoopUnrollInfo {
        index,
        start,
        delta: 0.0,
        count: 0,
    };

    //
    // condition has the form: loop_index relational_operator constant_expression
    //
    let Some(test) = *loop_test else {
        let pos = if positions.condition_position.valid() {
            positions.condition_position
        } else {
            loop_pos
        };
        errors.error(pos, "missing condition");
        return None;
    };
    let test_expr = ctx.pool.expression(test);
    let ExpressionKind::Binary(cond) = &test_expr.kind else {
        errors.error(test_expr.position, "invalid condition");
        return None;
    };
    let (cond_pos, cond_left, cond_op, cond_right) =
        (test_expr.position, cond.left, cond.operator, cond.right);
    if !is_loop_index(ctx, cond_left, index) {
        errors.error(
            cond_pos,
            "expected loop index on left hand side of condition",
        );
        return None;
    }
    // relational_operator is one of: > >= < <= == or !=
    match cond_op.kind() {
        OperatorKind::Gt
        | OperatorKind::GtEq
        | OperatorKind::Lt
        | OperatorKind::LtEq
        | OperatorKind::EqEq
        | OperatorKind::Neq => {}
        _ => {
            errors.error(cond_pos, "invalid relational operator");
            return None;
        }
    }
    let Some(loop_end) = get_constant_value(&ctx.pool, cond_right) else {
        errors.error(
            cond_pos,
            "loop index must be compared with a constant expression",
        );
        return None;
    };

    //
    // expression has one of the following forms:
    //   loop_index++
    //   loop_index--
    //   loop_index += constant_expression
    //   loop_index -= constant_expression
    // The spec doesn't mention prefix increment and decrement, but there is some consensus that
    // it's an oversight, so we allow those as well.
    //
    let Some(next) = loop_next else {
        let pos = if positions.next_position.valid() {
            positions.next_position
        } else {
            loop_pos
        };
        errors.error(pos, "missing loop expression");
        return None;
    };
    let next_expr = ctx.pool.expression(next);
    let next_pos = next_expr.position;
    match &next_expr.kind {
        ExpressionKind::Binary(b) => {
            if !is_loop_index(ctx, b.left, index) {
                errors.error(next_pos, "expected loop index in loop expression");
                return None;
            }
            let Some(delta) = get_constant_value(&ctx.pool, b.right) else {
                errors.error(
                    next_pos,
                    "loop index must be modified by a constant expression",
                );
                return None;
            };
            loop_info.delta = delta;
            match b.operator.kind() {
                OperatorKind::PlusEq => {}
                OperatorKind::MinusEq => loop_info.delta = -loop_info.delta,
                _ => {
                    errors.error(next_pos, "invalid operator in loop expression");
                    return None;
                }
            }
        }
        ExpressionKind::Prefix(p) => {
            if !is_loop_index(ctx, p.operand, index) {
                errors.error(next_pos, "expected loop index in loop expression");
                return None;
            }
            match p.operator.kind() {
                OperatorKind::PlusPlus => loop_info.delta = 1.0,
                OperatorKind::MinusMinus => loop_info.delta = -1.0,
                _ => {
                    errors.error(next_pos, "invalid operator in loop expression");
                    return None;
                }
            }
        }
        ExpressionKind::Postfix(p) => {
            if !is_loop_index(ctx, p.operand, index) {
                errors.error(next_pos, "expected loop index in loop expression");
                return None;
            }
            match p.operator.kind() {
                OperatorKind::PlusPlus => loop_info.delta = 1.0,
                OperatorKind::MinusMinus => loop_info.delta = -1.0,
                _ => {
                    errors.error(next_pos, "invalid operator in loop expression");
                    return None;
                }
            }
        }
        _ => {
            errors.error(next_pos, "invalid loop expression");
            return None;
        }
    }

    //
    // Within the body of the loop, the loop index is not statically assigned to, nor is it used
    // as argument to a function 'out' or 'inout' parameter.
    //
    if statement_writes_to_variable(&ctx.pool, loop_statement, index) {
        errors.error(
            ctx.pool.statement(loop_statement).position,
            "loop index must not be modified within body of the loop",
        );
        return None;
    }

    // Finally, compute the iteration count, based on the bounds, and the termination operator.
    // For integer variables, we simulate the loop using 32-bit signed math to correctly detect
    // the integer wraparound behavior that would occur at runtime on the GPU. (For 'float'
    // variables, the existing double-precision calculation is sufficient.)
    let loop_type = if ctx.pool.ty(base_type).is_integer() {
        LoopType::Int
    } else {
        debug_assert!(ctx.pool.ty(base_type).is_float());
        LoopType::Float
    };
    let (start, delta) = (loop_info.start, loop_info.delta);
    loop_info.count = match cond_op.kind() {
        OperatorKind::Lt => calculate_count(
            start,
            loop_end,
            delta,
            Direction::Forwards,
            Inclusive::No,
            loop_type,
        ),
        OperatorKind::Gt => calculate_count(
            start,
            loop_end,
            delta,
            Direction::Backwards,
            Inclusive::No,
            loop_type,
        ),
        OperatorKind::LtEq => calculate_count(
            start,
            loop_end,
            delta,
            Direction::Forwards,
            Inclusive::Yes,
            loop_type,
        ),
        OperatorKind::GtEq => calculate_count(
            start,
            loop_end,
            delta,
            Direction::Backwards,
            Inclusive::Yes,
            loop_type,
        ),
        OperatorKind::Neq => {
            let count = calculate_count_neq(start, loop_end, delta, loop_type);
            let index_is_float = {
                let index_ty = ctx.pool.variable(index).ty;
                ctx.pool.ty(index_ty).component_type().is_float()
            };
            if index_is_float {
                // Rewrite `x != n` tests as `x < n` or `x > n` depending on the loop direction.
                // Less-than and greater-than tests avoid infinite loops caused by rounding error.
                let rewritten = if loop_info.delta > 0.0 {
                    OperatorKind::Lt
                } else {
                    OperatorKind::Gt
                };
                let left = ctx.pool.clone_expression(cond_left);
                let right = ctx.pool.clone_expression(cond_right);
                *loop_test = Some(BinaryExpression::make(
                    ctx,
                    cond_pos,
                    left,
                    Operator::from(rewritten),
                    right,
                ));
            }
            count
        }
        OperatorKind::EqEq => {
            if start == loop_end {
                // Start and end begin in the same place, so we can run one iteration...
                if delta == 0.0 {
                    // ... but they never diverge, so the loop runs forever.
                    LOOP_TERMINATION_LIMIT
                } else {
                    // ... and then they diverge, so the loop terminates.
                    1
                }
            } else {
                // Start never equals end, so the loop will not run a single iteration.
                0
            }
        }
        _ => unreachable!("the relational operator was checked above"),
    };

    debug_assert!(loop_info.count >= 0);
    if loop_info.count >= LOOP_TERMINATION_LIMIT {
        errors.error(
            loop_pos,
            "loop must guarantee termination in fewer iterations",
        );
        return None;
    }
    Some(loop_info)
}

/// `expr->is<VariableReference>() && expr->as<VariableReference>().variable() == index`.
fn is_loop_index(ctx: &Context, expr: ExprId, index: VarId) -> bool {
    matches!(
        &ctx.pool.expression(expr).kind,
        ExpressionKind::VariableReference(var_ref) if var_ref.variable == index
    )
}

/// `LoopControlFlowInfo`: which control-flow statements inside a loop affect the loop.
// Port of: src/sksl/analysis/SkSLGetLoopControlFlowInfo.cpp and SkSLAnalysis.h#L134-L140
// (chrome/m156)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // Mirrors `LoopControlFlowInfo`'s three flags.
pub struct LoopControlFlowInfo {
    /// `fHasContinue`: a `continue` that is not nested in another loop or switch.
    pub has_continue: bool,
    /// `fHasBreak`: a `break` that is not nested in another loop or switch.
    pub has_break: bool,
    /// `fHasReturn`: a `return`, however deeply nested.
    pub has_return: bool,
}

/// `LoopControlFlowVisitor`.
struct LoopControlFlowVisitor {
    result: LoopControlFlowInfo,
    depth: i32,
}

impl ProgramVisitor for LoopControlFlowVisitor {
    fn visit_expression(&mut self, _pool: &IrPool, _expr: ExprId) -> bool {
        // We can avoid processing expressions entirely.
        false
    }

    // Port of: src/sksl/analysis/SkSLGetLoopControlFlowInfo.cpp#L20-L58 (chrome/m156)
    fn visit_statement(&mut self, pool: &IrPool, stmt: StmtId) -> bool {
        match &pool.statement(stmt).kind {
            StatementKind::Continue(_) => {
                // A continue only affects the loop if it's not nested inside another looping
                // structure. (Inside a switch, SkSL disallows continue entirely.)
                self.result.has_continue |= self.depth == 0;
            }
            StatementKind::Break(_) => {
                // A break only affects the loop if it's not nested inside another loop/switch.
                self.result.has_break |= self.depth == 0;
            }
            StatementKind::Return(_) => {
                // A return aborts the loop's control flow no matter how deeply it is nested.
                self.result.has_return = true;
            }
            StatementKind::For(_) | StatementKind::Do(_) | StatementKind::Switch(_) => {
                self.depth += 1;
                let done = walk_statement(self, pool, stmt);
                self.depth -= 1;
                return done;
            }
            _ => return walk_statement(self, pool, stmt),
        }
        // If we've already found everything we're hunting for, we can stop searching early.
        self.result.has_continue && self.result.has_break && self.result.has_return
    }
}

/// `Analysis::GetLoopControlFlowInfo`: the control flow in `stmt` that affects its enclosing
/// loop.
// Port of: src/sksl/analysis/SkSLGetLoopControlFlowInfo.cpp#L69-L74 (chrome/m156)
#[must_use]
pub fn get_loop_control_flow_info(pool: &IrPool, stmt: StmtId) -> LoopControlFlowInfo {
    let mut visitor = LoopControlFlowVisitor {
        result: LoopControlFlowInfo::default(),
        depth: 0,
    };
    visitor.visit_statement(pool, stmt);
    visitor.result
}
