// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis/SkSLHasSideEffects.cpp, SkSLIsConstantExpression.cpp,
// SkSLIsTrivialExpression.cpp, SkSLIsDynamicallyUniformExpression.cpp,
// SkSLIsSameExpressionTree.cpp, and the expression helpers of src/sksl/SkSLAnalysis.cpp.

//! Predicates on expression trees. Each one is a [`ProgramVisitor`] or a recursion over the
//! node kinds it cares about, as in Skia.

use std::collections::HashSet;

use super::{ProgramVisitor, walk_expression};
use crate::ir::{ExprId, ExpressionKind, IrPool, VarId, VariableStorage};
use crate::operator::OperatorKind;

/// The name of the render-target adjust uniform (`SkSL::Compiler::RTADJUST_NAME`, which lives in
/// `SkSLCompiler.h`; S11 owns `compiler.rs`, so the constant is kept here).
// Port of: src/sksl/SkSLCompiler.h#L69 (chrome/m156)
pub(crate) const RTADJUST_NAME: &str = "sk_RTAdjust";

/// `Analysis::HasSideEffects`: true if `expr` writes, increments, decrements, or calls a function
/// that is not `pure`.
// Port of: src/sksl/analysis/SkSLHasSideEffects.cpp#L22-L63 (chrome/m156)
#[must_use]
pub fn has_side_effects(pool: &IrPool, expr: ExprId) -> bool {
    struct HasSideEffectsVisitor;
    impl ProgramVisitor for HasSideEffectsVisitor {
        fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
            match &pool.expression(expr).kind {
                ExpressionKind::FunctionCall(call) => {
                    if !pool.function(call.function).modifier_flags.is_pure() {
                        return true;
                    }
                }
                ExpressionKind::Prefix(prefix) => {
                    if matches!(
                        prefix.operator.kind(),
                        OperatorKind::PlusPlus | OperatorKind::MinusMinus
                    ) {
                        return true;
                    }
                }
                ExpressionKind::Binary(binary) => {
                    if binary.operator.is_assignment() {
                        return true;
                    }
                }
                ExpressionKind::Postfix(_) => return true,
                _ => {}
            }
            walk_expression(self, pool, expr)
        }
    }
    HasSideEffectsVisitor.visit_expression(pool, expr)
}

/// `Analysis::IsCompileTimeConstant`: true if `expr` is composed only of literals and
/// constructors.
// Port of: src/sksl/SkSLAnalysis.cpp#L473-L508 (chrome/m156)
#[must_use]
pub fn is_compile_time_constant(pool: &IrPool, expr: ExprId) -> bool {
    struct IsCompileTimeConstantVisitor {
        is_constant: bool,
    }
    impl ProgramVisitor for IsCompileTimeConstantVisitor {
        fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
            match &pool.expression(expr).kind {
                // Literals are compile-time constants.
                ExpressionKind::Literal(_) => false,
                // Constructors might be compile-time constants, if they are composed entirely of
                // literals and constructors. (Casting constructors are intentionally omitted here.
                // If the value inside was a compile-time constant, we would have not have
                // generated a cast at all.)
                ExpressionKind::ConstructorArray(_)
                | ExpressionKind::ConstructorCompound(_)
                | ExpressionKind::ConstructorDiagonalMatrix(_)
                | ExpressionKind::ConstructorMatrixResize(_)
                | ExpressionKind::ConstructorSplat(_)
                | ExpressionKind::ConstructorStruct(_) => walk_expression(self, pool, expr),
                // This expression isn't a compile-time constant.
                _ => {
                    self.is_constant = false;
                    true
                }
            }
        }
    }
    let mut visitor = IsCompileTimeConstantVisitor { is_constant: true };
    visitor.visit_expression(pool, expr);
    visitor.is_constant
}

/// Checks the ES2 constant-expression rules. With `loop_indices` set, it also accepts the loop
/// indices of a for-loop (the constant-index-expression rules; the ES2 indexing check of S9b
/// passes them).
// Port of: src/sksl/analysis/SkSLIsConstantExpression.cpp#L37-L114 (chrome/m156)
struct ConstantExpressionVisitor<'a> {
    loop_indices: Option<&'a HashSet<VarId>>,
}

impl ProgramVisitor for ConstantExpressionVisitor<'_> {
    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        let e = pool.expression(expr);
        match &e.kind {
            // A literal value is constant. Settings can appear in fragment processors; they will
            // resolve when compiled.
            ExpressionKind::Literal(_) | ExpressionKind::Setting(_) => false,
            // A global or local variable qualified as 'const', excluding function parameters.
            // Loop indices as defined in section 4 of the GLSL ES 1.00 appendix A.
            ExpressionKind::VariableReference(reference) => {
                let var = pool.variable(reference.variable);
                if var.modifier_flags.is_const()
                    && matches!(var.storage, VariableStorage::Global | VariableStorage::Local)
                {
                    return false;
                }
                !self
                    .loop_indices
                    .is_some_and(|indices| indices.contains(&reference.variable))
            }
            // Not a sequence expression (skbug.com/40044392).
            ExpressionKind::Binary(binary) if binary.operator.kind() == OperatorKind::Comma => true,
            // Expressions composed of constant-expressions.
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
            // Function calls are completely disallowed in SkSL constant-(index)-expressions. GLSL
            // does mandate that calling a built-in function where the arguments are all
            // constant-expressions should result in a constant-expression. SkSL handles this by
            // optimizing fully-constant function calls into literals in FunctionCall::Make.
            ExpressionKind::FunctionCall(_)
            | ExpressionKind::ChildCall(_)
            // These shouldn't appear in a valid program at all, and definitely aren't
            // constant-(index)-expressions.
            | ExpressionKind::Poison(_)
            | ExpressionKind::FunctionReference(_)
            | ExpressionKind::MethodReference(_)
            | ExpressionKind::TypeReference(_)
            | ExpressionKind::Empty(_) => true,
        }
    }
}

/// `Analysis::IsConstantExpression`: a constant-expression as GLSL 1.0, section 5.10 defines it.
// Port of: src/sksl/analysis/SkSLIsConstantExpression.cpp#L157-L159 (chrome/m156)
#[must_use]
pub fn is_constant_expression(pool: &IrPool, expr: ExprId) -> bool {
    !ConstantExpressionVisitor { loop_indices: None }.visit_expression(pool, expr)
}

/// `ConstantExpressionVisitor` with loop indices: true when `expr` is not a constant expression,
/// where the loop indices in `loop_indices` count as constant.
// Port of: src/sksl/analysis/SkSLIsConstantExpression.cpp#L23-L110 (chrome/m156)
#[must_use]
pub fn is_constant_expression_with_loop_indices(
    pool: &IrPool,
    expr: ExprId,
    loop_indices: &HashSet<VarId>,
) -> bool {
    !ConstantExpressionVisitor {
        loop_indices: Some(loop_indices),
    }
    .visit_expression(pool, expr)
}

/// `Analysis::IsTrivialExpression`: an expression that is cheap enough to clone several times.
// Port of: src/sksl/analysis/SkSLIsTrivialExpression.cpp#L25-L84 (chrome/m156)
#[must_use]
pub fn is_trivial_expression(pool: &IrPool, expr: ExprId) -> bool {
    let e = pool.expression(expr);
    match &e.kind {
        ExpressionKind::Literal(_) | ExpressionKind::VariableReference(_) => true,
        // All swizzles are considered to be trivial.
        ExpressionKind::Swizzle(swizzle) => is_trivial_expression(pool, swizzle.base),
        ExpressionKind::Prefix(prefix) => match prefix.operator.kind() {
            OperatorKind::Plus
            | OperatorKind::Minus
            | OperatorKind::LogicalNot
            | OperatorKind::BitwiseNot => is_trivial_expression(pool, prefix.operand),
            _ => false,
        },
        // Accessing a field is trivial.
        ExpressionKind::FieldAccess(access) => is_trivial_expression(pool, access.base),
        // Accessing a constant array index is trivial.
        ExpressionKind::Index(index) => {
            pool.expression(index.index).is_int_literal(pool)
                && is_trivial_expression(pool, index.base)
        }
        // Only consider small arrays/structs of compile-time-constants to be trivial.
        ExpressionKind::ConstructorArray(_) | ExpressionKind::ConstructorStruct(_) => {
            pool.ty(e.ty).slot_count() <= 4 && is_compile_time_constant(pool, expr)
        }
        // Only compile-time-constant compound constructors are considered to be trivial.
        ExpressionKind::ConstructorCompound(_) => is_compile_time_constant(pool, expr),
        // Single-argument constructors are trivial when their inner expression is trivial.
        ExpressionKind::ConstructorCompoundCast(_)
        | ExpressionKind::ConstructorScalarCast(_)
        | ExpressionKind::ConstructorSplat(_)
        | ExpressionKind::ConstructorDiagonalMatrix(_) => {
            let arguments = e.any_constructor_arguments().unwrap_or_default();
            debug_assert_eq!(arguments.len(), 1);
            arguments
                .first()
                .is_some_and(|&inner| is_trivial_expression(pool, inner))
        }
        // Array casts and matrix resizes require function calls in Metal, so they are never
        // trivial, and neither is anything else.
        _ => false,
    }
}

/// `Analysis::IsSameExpressionTree`: true if both trees are the same. Used by the optimizer to
/// spot self-assignment and self-comparison; it does not catch every case, and it rejects
/// expressions that may have side effects.
// Port of: src/sksl/analysis/SkSLIsSameExpressionTree.cpp#L28-L95 (chrome/m156)
#[must_use]
pub fn is_same_expression_tree(pool: &IrPool, left: ExprId, right: ExprId) -> bool {
    let l = pool.expression(left);
    let r = pool.expression(right);
    if std::mem::discriminant(&l.kind) != std::mem::discriminant(&r.kind)
        || !pool.ty(l.ty).matches(r.ty)
    {
        return false;
    }
    // This isn't a fully exhaustive list of expressions by any stretch of the imagination; for
    // instance, `x[y+1] = x[y+1]` isn't detected because we don't look at BinaryExpressions.
    // Since this is intended to be used for optimization purposes, handling the common cases is
    // sufficient.
    match (&l.kind, &r.kind) {
        #[allow(clippy::float_cmp)] // Skia compares the two doubles exactly.
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
            a.components.as_slice() == b.components.as_slice()
                && is_same_expression_tree(pool, a.base, b.base)
        }
        (ExpressionKind::VariableReference(a), ExpressionKind::VariableReference(b)) => {
            a.variable == b.variable
        }
        _ => {
            // The constructor kinds: same kind (checked above), and the same arguments.
            match (l.any_constructor_arguments(), r.any_constructor_arguments()) {
                (Some(left_args), Some(right_args)) => {
                    left_args.len() == right_args.len()
                        && left_args
                            .iter()
                            .zip(right_args)
                            .all(|(&a, &b)| is_same_expression_tree(pool, a, b))
                }
                _ => false,
            }
        }
    }
}

/// `Analysis::IsDynamicallyUniformExpression`: true if `expr` could be evaluated at compile time
/// were the uniform values known.
// Port of: src/sksl/analysis/SkSLIsDynamicallyUniformExpression.cpp#L21-L84 (chrome/m156)
#[must_use]
pub fn is_dynamically_uniform_expression(pool: &IrPool, expr: ExprId) -> bool {
    struct IsDynamicallyUniformExpressionVisitor {
        is_dynamically_uniform: bool,
    }
    impl ProgramVisitor for IsDynamicallyUniformExpressionVisitor {
        fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
            match &pool.expression(expr).kind {
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
                | ExpressionKind::Postfix(_)
                | ExpressionKind::Prefix(_)
                | ExpressionKind::Swizzle(_)
                | ExpressionKind::Ternary(_) => {
                    // These expressions might be dynamically uniform, if they are composed
                    // entirely of constants and uniforms.
                }
                ExpressionKind::VariableReference(reference) => {
                    // Verify that variable references are const or uniform.
                    let flags = pool.variable(reference.variable).modifier_flags;
                    if flags.is_const() || flags.is_uniform() {
                        // Fine, keep looking at the rest of the tree (there are no children).
                    } else {
                        self.is_dynamically_uniform = false;
                        return true;
                    }
                }
                ExpressionKind::FunctionCall(call) => {
                    // Verify that function calls are pure.
                    if !pool.function(call.function).modifier_flags.is_pure() {
                        self.is_dynamically_uniform = false;
                        return true;
                    }
                }
                // Literals are compile-time constants.
                ExpressionKind::Literal(_) => return false,
                // This expression isn't dynamically uniform.
                _ => {
                    self.is_dynamically_uniform = false;
                    return true;
                }
            }
            walk_expression(self, pool, expr)
        }
    }
    let mut visitor = IsDynamicallyUniformExpressionVisitor {
        is_dynamically_uniform: true,
    };
    visitor.visit_expression(pool, expr);
    visitor.is_dynamically_uniform
}

/// `Analysis::ContainsRTAdjust`: true if `expr` mentions `sk_RTAdjust`.
// Port of: src/sksl/SkSLAnalysis.cpp#L411-L427 (chrome/m156)
#[must_use]
pub fn contains_rt_adjust(pool: &IrPool, expr: ExprId) -> bool {
    struct ContainsRtAdjustVisitor;
    impl ProgramVisitor for ContainsRtAdjustVisitor {
        fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
            if matches!(
                &pool.expression(expr).kind,
                ExpressionKind::VariableReference(reference)
                    if pool.variable(reference.variable).name.as_ref() == RTADJUST_NAME
            ) {
                return true;
            }
            walk_expression(self, pool, expr)
        }
    }
    ContainsRtAdjustVisitor.visit_expression(pool, expr)
}

/// `Analysis::ContainsVariable`: true if `expr` mentions `var`.
// Port of: src/sksl/SkSLAnalysis.cpp#L429-L448 (chrome/m156)
#[must_use]
pub fn contains_variable(pool: &IrPool, expr: ExprId, var: VarId) -> bool {
    struct ContainsVariableVisitor {
        variable: VarId,
    }
    impl ProgramVisitor for ContainsVariableVisitor {
        fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
            if matches!(
                &pool.expression(expr).kind,
                ExpressionKind::VariableReference(reference) if reference.variable == self.variable
            ) {
                return true;
            }
            walk_expression(self, pool, expr)
        }
    }
    ContainsVariableVisitor { variable: var }.visit_expression(pool, expr)
}

/// `Analysis::GetRootVariable`: unwraps indexes, field accesses and swizzles to the variable
/// being referenced, if any.
// Port of: src/sksl/SkSLAnalysis.cpp#L450-L471 (chrome/m156)
#[must_use]
pub fn get_root_variable(pool: &IrPool, expr: ExprId) -> Option<VarId> {
    let mut current = expr;
    loop {
        match &pool.expression(current).kind {
            ExpressionKind::VariableReference(reference) => return Some(reference.variable),
            ExpressionKind::Index(index) => current = index.base,
            ExpressionKind::FieldAccess(access) => current = access.base,
            ExpressionKind::Swizzle(swizzle) => current = swizzle.base,
            _ => return None,
        }
    }
}
