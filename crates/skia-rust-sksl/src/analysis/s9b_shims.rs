// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: minimal faithful copies of helpers that the S9b analyses call and that belong
// to other tasks. Each item names its owner. When that task lands, its version replaces this one
// and the call sites switch to it:
//   - `ConstantFolder::GetConstantValue` and its helpers, `Analysis::IsCompileTimeConstant`
//     (S8: SkSLConstantFolder.cpp, SkSLAnalysis.cpp)
//   - `BinaryExpression::Make` without constant folding (S7b)
//   - `Analysis::IsSameExpressionTree`, the `ConstantExpressionVisitor` of
//     `Analysis::IsConstantExpression` with loop indices, and the write counts of `ProgramUsage`
//     (S9a: analysis/SkSLIsSameExpressionTree.cpp, IsConstantExpression.cpp, SkSLProgramUsage.cpp)
//   - `Analysis::StatementWritesToVariable` (S9a: the helpers of SkSLAnalysis.cpp)
//   - the integer `SkSafeMath` operations and `sk_double_saturate2int` (skia-rust-base)

// The casts below mirror the C++ `static_cast`s and truncations of the ported helpers.
#![allow(clippy::cast_possible_truncation)]

use std::collections::{HashMap, HashSet};

use super::{ProgramVisitor, walk_expression, walk_statement};
use crate::context::Context;
use crate::error_reporter::{ErrorReporter, ErrorSink};
use crate::ir::{
    BinaryExpression, ElemId, ExprId, Expression, ExpressionKind, IrPool, StatementKind, StmtId,
    VarId, VariableRefKind, VariableStorage,
};
use crate::operator::{Operator, OperatorKind};
use crate::position::Position;

/// `ConstantFolder::GetConstantValueOrNull`: the expression a constant stands for (a literal, or
/// a const variable whose initial value is a compile-time constant), or `None`.
// Port of: src/sksl/SkSLConstantFolder.cpp#L441-L460 (chrome/m156) (shim, owner S8)
fn get_constant_value_or_null(pool: &IrPool, expr: ExprId) -> Option<ExprId> {
    let mut expr = expr;
    while let ExpressionKind::VariableReference(var_ref) = &pool.expression(expr).kind {
        if var_ref.ref_kind != VariableRefKind::Read {
            return None;
        }
        let var = pool.variable(var_ref.variable);
        if !var.modifier_flags.is_const() {
            return None;
        }
        // Function parameters can be const but have no initial value.
        expr = var.initial_value(pool)?;
    }
    is_compile_time_constant(pool, expr).then_some(expr)
}

/// `ConstantFolder::GetConstantValueForVariable`: the constant `expr` stands for, or `expr`.
// Port of: src/sksl/SkSLConstantFolder.cpp#L462-L465 (chrome/m156) (shim, owner S8)
#[must_use]
pub fn get_constant_value_for_variable(pool: &IrPool, expr: ExprId) -> ExprId {
    get_constant_value_or_null(pool, expr).unwrap_or(expr)
}

/// `ConstantFolder::GetConstantValue(expr, &out)`: the value of a literal, looking through const
/// variables first.
// Port of: src/sksl/SkSLConstantFolder.cpp#L335-L342 (chrome/m156) (shim, owner S8)
#[must_use]
pub fn get_constant_value(pool: &IrPool, expr: ExprId) -> Option<f64> {
    let expr = get_constant_value_for_variable(pool, expr);
    match &pool.expression(expr).kind {
        ExpressionKind::Literal(literal) => Some(literal.value),
        _ => None,
    }
}

/// `Analysis::IsCompileTimeConstantVisitor`.
struct IsCompileTimeConstantVisitor {
    is_constant: bool,
}

impl ProgramVisitor for IsCompileTimeConstantVisitor {
    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        match &pool.expression(expr).kind {
            // Literals are compile-time constants.
            ExpressionKind::Literal(_) => false,
            // Constructors are constants when they are built entirely from literals and
            // constructors.
            ExpressionKind::ConstructorArray(_)
            | ExpressionKind::ConstructorCompound(_)
            | ExpressionKind::ConstructorDiagonalMatrix(_)
            | ExpressionKind::ConstructorMatrixResize(_)
            | ExpressionKind::ConstructorSplat(_)
            | ExpressionKind::ConstructorStruct(_) => walk_expression(self, pool, expr),
            _ => {
                self.is_constant = false;
                true
            }
        }
    }
}

/// `Analysis::IsCompileTimeConstant`.
// Port of: src/sksl/SkSLAnalysis.cpp (IsCompileTimeConstant, chrome/m156) (shim, owner S9a)
#[must_use]
pub fn is_compile_time_constant(pool: &IrPool, expr: ExprId) -> bool {
    let mut visitor = IsCompileTimeConstantVisitor { is_constant: true };
    visitor.visit_expression(pool, expr);
    visitor.is_constant
}

/// `BinaryExpression::Make(context, pos, left, op, right)`, without the constant folding that
/// `ConstantFolder::Simplify` would attempt. The loop rewrite that calls it compares a
/// non-constant loop index with a constant, and `Simplify` has nothing to fold there.
// Port of: src/sksl/ir/SkSLBinaryExpression.cpp#L97-L139 (chrome/m156) (shim, owner S7b)
pub fn binary_expression_make(
    ctx: &mut Context,
    pos: Position,
    left: ExprId,
    op: Operator,
    right: ExprId,
) -> ExprId {
    let left_type = ctx.pool.expression(left).ty;
    let right_type = ctx.pool.expression(right).ty;
    let Some(types) = op.determine_binary_type(ctx, left_type, right_type) else {
        panic!("BinaryExpression::Make: the operands have no binary type");
    };
    ctx.pool.add_expression(Expression::new(
        pos,
        types.result,
        ExpressionKind::Binary(BinaryExpression {
            left,
            operator: op,
            right,
        }),
    ))
}

/// `ConstantExpressionVisitor(loopIndices).visitExpression(expr)` of `IsConstantExpression.cpp`:
/// true when the expression is *not* a constant (index) expression.
// Port of: src/sksl/analysis/SkSLIsConstantExpression.cpp#L23-L110 (chrome/m156) (shim, owner S9a)
#[must_use]
pub fn visit_constant_expression(
    pool: &IrPool,
    expr: ExprId,
    loop_indices: Option<&HashSet<VarId>>,
) -> bool {
    let mut visitor = ConstantExpressionVisitor { loop_indices };
    visitor.visit_expression(pool, expr)
}

/// `ConstantExpressionVisitor`: see [`visit_constant_expression`].
struct ConstantExpressionVisitor<'a> {
    loop_indices: Option<&'a HashSet<VarId>>,
}

impl ProgramVisitor for ConstantExpressionVisitor<'_> {
    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        match &pool.expression(expr).kind {
            // A literal value, or a setting (which resolves when compiled).
            ExpressionKind::Literal(_) | ExpressionKind::Setting(_) => false,
            // A `const` global or local, or a loop index.
            ExpressionKind::VariableReference(var_ref) => {
                let var = pool.variable(var_ref.variable);
                if var.modifier_flags.is_const()
                    && matches!(
                        var.storage,
                        VariableStorage::Global | VariableStorage::Local
                    )
                {
                    return false;
                }
                match self.loop_indices {
                    None => true,
                    Some(indices) => !indices.contains(&var_ref.variable),
                }
            }
            // A sequence expression is not a constant.
            ExpressionKind::Binary(b) if b.operator.kind() == OperatorKind::Comma => true,
            ExpressionKind::Binary(_)
            | ExpressionKind::ConstructorArray(_)
            | ExpressionKind::ConstructorArrayCast(_)
            | ExpressionKind::ConstructorCompound(_)
            | ExpressionKind::ConstructorCompoundCast(_)
            | ExpressionKind::ConstructorDiagonalMatrix(_)
            | ExpressionKind::ConstructorMatrixResize(_)
            | ExpressionKind::ConstructorScalarCast(_)
            | ExpressionKind::ConstructorSplat(_)
            | ExpressionKind::ConstructorStruct(_)
            | ExpressionKind::FieldAccess(_)
            | ExpressionKind::Index(_)
            | ExpressionKind::Prefix(_)
            | ExpressionKind::Postfix(_)
            | ExpressionKind::Swizzle(_)
            | ExpressionKind::Ternary(_) => walk_expression(self, pool, expr),
            // Calls are never constant; the rest should not appear in a valid program.
            ExpressionKind::FunctionCall(_)
            | ExpressionKind::ChildCall(_)
            | ExpressionKind::Poison(_)
            | ExpressionKind::FunctionReference(_)
            | ExpressionKind::MethodReference(_)
            | ExpressionKind::TypeReference(_)
            | ExpressionKind::Empty(_) => true,
        }
    }
}

/// `Analysis::IsSameExpressionTree`: a conservative structural equality of two expressions.
// Port of: src/sksl/analysis/SkSLIsSameExpressionTree.cpp#L28-L84 (chrome/m156) (shim, owner S9a)
#[must_use]
pub fn is_same_expression_tree(pool: &IrPool, left: ExprId, right: ExprId) -> bool {
    let (l, r) = (pool.expression(left), pool.expression(right));
    if std::mem::discriminant(&l.kind) != std::mem::discriminant(&r.kind)
        || !pool.ty(l.ty).matches(r.ty)
    {
        return false;
    }
    match (&l.kind, &r.kind) {
        #[allow(clippy::float_cmp)] // Mirrors the C++ `==` on the literal values.
        (ExpressionKind::Literal(a), ExpressionKind::Literal(b)) => a.value == b.value,
        (ExpressionKind::FieldAccess(a), ExpressionKind::FieldAccess(b)) => {
            a.field_index == b.field_index && is_same_expression_tree(pool, a.base, b.base)
        }
        (ExpressionKind::Index(a), ExpressionKind::Index(b)) => {
            is_same_expression_tree(pool, a.index, b.index)
                && is_same_expression_tree(pool, a.base, b.base)
        }
        (ExpressionKind::Prefix(a), ExpressionKind::Prefix(b)) => {
            a.operator.kind() == b.operator.kind()
                && is_same_expression_tree(pool, a.operand, b.operand)
        }
        (ExpressionKind::Swizzle(a), ExpressionKind::Swizzle(b)) => {
            a.components == b.components && is_same_expression_tree(pool, a.base, b.base)
        }
        (ExpressionKind::VariableReference(a), ExpressionKind::VariableReference(b)) => {
            a.variable == b.variable
        }
        _ if l.is_any_constructor() => {
            // The kinds match, so both sides are constructors of the same kind.
            match (l.any_constructor_arguments(), r.any_constructor_arguments()) {
                (Some(la), Some(ra)) => {
                    la.len() == ra.len()
                        && la
                            .iter()
                            .zip(ra)
                            .all(|(&a, &b)| is_same_expression_tree(pool, a, b))
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// The write counts of `ProgramUsage::get(v).fWrite`: how many times each variable is written in
/// the elements it is built from. A declaration with an initial value is one write; a reference
/// is a write when it is `kWrite`, `kReadWrite` or `kPointer`.
// Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L40-L118 (chrome/m156), the `fWrite` part only
// (shim, owner S9a: `ProgramUsage` replaces it)
#[derive(Debug, Default)]
pub struct WriteCounts {
    writes: HashMap<VarId, i32>,
}

impl WriteCounts {
    /// Counts the writes in `elements` (shared and owned, as `ProgramUsage` does).
    #[must_use]
    pub fn for_elements(pool: &IrPool, elements: &[ElemId]) -> Self {
        let mut counts = Self::default();
        for &element in elements {
            counts.visit_program_element(pool, element);
        }
        counts
    }

    /// `ProgramUsage::get(v).fWrite`.
    #[must_use]
    pub fn writes(&self, var: VarId) -> i32 {
        self.writes.get(&var).copied().unwrap_or(0)
    }

    fn add_write(&mut self, var: VarId) {
        *self.writes.entry(var).or_insert(0) += 1;
    }
}

impl ProgramVisitor for WriteCounts {
    fn visit_statement(&mut self, pool: &IrPool, stmt: StmtId) -> bool {
        if let StatementKind::VarDeclaration(decl) = &pool.statement(stmt).kind
            && decl.value.is_some()
        {
            // The initial-value expression, when present, counts as a write.
            self.add_write(decl.var);
        }
        walk_statement(self, pool, stmt)
    }

    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        if let ExpressionKind::VariableReference(var_ref) = &pool.expression(expr).kind {
            match var_ref.ref_kind {
                VariableRefKind::Read => {}
                VariableRefKind::Write | VariableRefKind::ReadWrite | VariableRefKind::Pointer => {
                    self.add_write(var_ref.variable);
                }
            }
        }
        walk_expression(self, pool, expr)
    }
}

/// `Analysis::StatementWritesToVariable`: true if `stmt` writes to `var`.
// Port of: src/sksl/SkSLAnalysis.cpp#L230-L256 and #L547-L549 (chrome/m156)
// (shim, owner S9a: the helpers of SkSLAnalysis.cpp)
#[must_use]
pub fn statement_writes_to_variable(pool: &IrPool, stmt: StmtId, var: VarId) -> bool {
    let mut visitor = VariableWriteVisitor { var };
    visitor.visit_statement(pool, stmt)
}

/// `VariableWriteVisitor`.
struct VariableWriteVisitor {
    var: VarId,
}

impl ProgramVisitor for VariableWriteVisitor {
    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        if let ExpressionKind::VariableReference(var_ref) = &pool.expression(expr).kind
            && var_ref.variable == self.var
            && matches!(
                var_ref.ref_kind,
                VariableRefKind::Write | VariableRefKind::ReadWrite | VariableRefKind::Pointer
            )
        {
            return true;
        }
        walk_expression(self, pool, expr)
    }
}

/// `SkSafeMath`'s integer operations. Each records overflow in `ok` and returns the C++ result.
// Port of: src/core/SkSafeMath.h#L24-L60 (chrome/m156) (shim, owner skia-rust-base)
#[derive(Clone, Copy, Debug)]
pub struct SafeMath {
    ok: bool,
}

impl Default for SafeMath {
    fn default() -> Self {
        Self::new()
    }
}

impl SafeMath {
    /// A checker with no overflow yet.
    #[must_use]
    pub fn new() -> Self {
        Self { ok: true }
    }

    /// `ok()`: no operation overflowed.
    #[must_use]
    pub fn ok(self) -> bool {
        self.ok
    }

    /// `addInt`.
    pub fn add_int(&mut self, a: i32, b: i32) -> i32 {
        let result = i64::from(a) + i64::from(b);
        if i32::try_from(result).is_err() {
            self.ok = false;
        }
        result as i32
    }

    /// `subInt`.
    pub fn sub_int(&mut self, a: i32, b: i32) -> i32 {
        let result = i64::from(a) - i64::from(b);
        if i32::try_from(result).is_err() {
            self.ok = false;
        }
        result as i32
    }

    /// `mulInt`.
    pub fn mul_int(&mut self, a: i32, b: i32) -> i32 {
        let result = i64::from(a) * i64::from(b);
        if i32::try_from(result).is_err() {
            self.ok = false;
        }
        result as i32
    }

    /// `divInt`: flags a zero divisor and `INT_MIN / -1`, and returns `a` for them.
    pub fn div_int(&mut self, a: i32, b: i32) -> i32 {
        if b == 0 || (a == i32::MIN && b == -1) {
            self.ok = false;
            return a;
        }
        a / b
    }

    /// `modInt`: the remainder, with the checks of [`SafeMath::div_int`].
    pub fn mod_int(&mut self, a: i32, b: i32) -> i32 {
        if b == 0 || (a == i32::MIN && b == -1) {
            self.ok = false;
            return a;
        }
        a % b
    }
}

/// `SkSafeMath::Add(size_t, size_t)`: the sum, saturated to the maximum on overflow.
// Port of: src/core/SkSafeMath.cpp#L10-L14 (chrome/m156) (shim, owner skia-rust-base)
#[must_use]
pub fn saturating_add_size(x: usize, y: usize) -> usize {
    x.saturating_add(y)
}

/// `sk_double_saturate2int`: clamps to the `int` range. NaN gives `INT_MAX`, as the C++
/// comparisons do.
// Port of: include/private/SkFloatingPoint.h#L99-L104 (chrome/m156) (shim, owner skia-rust-base)
#[must_use]
pub fn double_saturate2int(x: f64) -> i32 {
    let x = if x < f64::from(i32::MAX) {
        x
    } else {
        f64::from(i32::MAX)
    };
    let x = if x > f64::from(i32::MIN) {
        x
    } else {
        f64::from(i32::MIN)
    };
    x as i32
}

/// Forwards the errors that an [`ErrorSink::Forwarding`] reporter recorded to `to`, in order.
/// The analyses with an optional error reporter record into a forwarding reporter first, so
/// that they can borrow the context mutably at the same time.
pub fn forward_errors(from: &ErrorReporter, to: &mut ErrorReporter) {
    if let ErrorSink::Forwarding { errors } = from.sink() {
        for (msg, position) in errors {
            to.error(*position, msg);
        }
    }
}
