// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp (the expression assemblers).

//! Expressions. The `assemble*` functions return the final expression text as a string, and emit
//! any necessary setup code directly into the program. The returned expression may be a
//! `let`-alias that cannot be assigned-into; use `make_lvalue` for an assignable expression.

use super::lvalue::{LValue, SwizzleLValue};
use super::types::{
    SAMPLER_SUFFIX, SK_POINTSIZE_BUILTIN, TEXTURE_SUFFIX, needs_builtin_type_conversion,
    operator_name, to_wgsl_type_simple, type_is_low_precision,
};
use super::{AssembleMode, WgslCodeGenerator};
use crate::analysis::{
    has_side_effects, is_compile_time_constant, is_constant_expression, is_trivial_expression,
};
use crate::constant_folder::get_constant_value_or_null;
use crate::ir::{
    ComponentArray, Expression, ExpressionKind, FieldAccessOwnerKind, FnId, IrPool, Literal,
    Swizzle, TypeId, TypeKind, VarId, VariableStorage,
};
use crate::operator::{Operator, OperatorKind, OperatorPrecedence};
use crate::transform::rewrite_indexed_swizzle;

type Precedence = OperatorPrecedence;

/// A `FunctionCall` copied out of the IR: the callee, the argument expressions and the result
/// type.
#[derive(Clone, Debug)]
pub(super) struct CallInfo {
    pub(super) function: FnId,
    pub(super) args: Vec<crate::ir::ExprId>,
    pub(super) ty: TypeId,
}

/// `all_arguments_constant`: true if all arguments are compile-time constants. If we are calling
/// an intrinsic and all of its inputs are constant, but we didn't constant-fold it, this
/// generally indicates that constant-folding resulted in an infinity or nan. The WGSL compiler
/// will reject such an expression with a compile-time error. We can dodge the error, taking on
/// the risk of indeterminate behavior instead, by replacing one of the constant values with a
/// scratch let-variable. (skbug.com/40045457)
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1019-L1031 (chrome/m156)
fn all_arguments_constant(pool: &IrPool, arguments: &[crate::ir::ExprId]) -> bool {
    arguments
        .iter()
        .all(|&arg| get_constant_value_or_null(pool, arg).is_some())
}

/// `is_nontrivial_expression`: a "trivial expression" is one which we can repeat multiple times
/// in the output without being dangerous or spammy. We avoid emitting temporary variables for
/// very trivial expressions: literals, unadorned variable references, or constant vectors.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1033-L1047 (chrome/m156)
fn is_nontrivial_expression(pool: &IrPool, expr: crate::ir::ExprId) -> bool {
    let e = pool.expression(expr);
    if matches!(
        e.kind,
        ExpressionKind::VariableReference(_) | ExpressionKind::Literal(_)
    ) {
        // Variables and literals are trivial; adding a let-declaration won't simplify anything.
        return false;
    }
    if pool.ty(e.ty).is_vector() && is_constant_expression(pool, expr) {
        // Compile-time constant vectors are also considered trivial; they're short and sweet.
        return false;
    }
    true
}

/// `binary_op_is_ambiguous_in_wgsl`: WGSL always requires parentheses for some operators which
/// are deemed to be ambiguous. (8.19. Operator Precedence and Associativity)
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2778-L2796 (chrome/m156)
pub(super) fn binary_op_is_ambiguous_in_wgsl(op: Operator) -> bool {
    matches!(
        op.kind(),
        OperatorKind::LogicalOr
            | OperatorKind::LogicalAnd
            | OperatorKind::BitwiseOr
            | OperatorKind::BitwiseAnd
            | OperatorKind::BitwiseXor
            | OperatorKind::Shl
            | OperatorKind::Shr
            | OperatorKind::Lt
            | OperatorKind::Gt
            | OperatorKind::LtEq
            | OperatorKind::GtEq
    )
}

impl WgslCodeGenerator<'_> {
    /// The type of expression `e`.
    pub(super) fn expr_type(&self, e: crate::ir::ExprId) -> TypeId {
        self.ctx.pool.expression(e).ty
    }

    /// `assembleExpression(e, parentPrecedence, mode)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2706-L2776 (chrome/m156)
    pub(super) fn assemble_expression(
        &mut self,
        e: crate::ir::ExprId,
        parent_precedence: Precedence,
        mode: AssembleMode,
    ) -> String {
        // Determine if we need to introduce a scratch let for this expression. There are three
        // situations that require this:
        //   1. Explicitly requested by caller.
        //   2. outer context would reference it multiple reasons for *reasons*, and the
        //      expression is not trivial
        //   3. skbug.com/asdf, where the first child const expression needs to be lifted to avoid
        //      WGSL's stricter const eval rules.
        if mode == AssembleMode::ForceLet
            || (mode == AssembleMode::UsedMultipleTimes
                && is_nontrivial_expression(&self.ctx.pool, e))
            || self.needs_const_eval_workaround
        {
            // Allow the scratch let to be considered const as long as we don't have to force it
            // to a let for the const-eval workaround or by `mode`.
            let can_be_const = !self.needs_const_eval_workaround
                && mode != AssembleMode::ForceLet
                && is_compile_time_constant(&self.ctx.pool, e);
            // Setting this to false and not overriding the AssembleMode means this next call to
            // assembleExpression gives us the original expression value.
            self.needs_const_eval_workaround = false;
            let expr = self.assemble_expression(e, Precedence::Assignment, AssembleMode::Auto);
            return self.write_scratch_let(&expr, can_be_const);
        }

        // `AutoConstEvalWorkaround constEval{this, e}`.
        let original_value = self.needs_const_eval_workaround;
        // Each scope can apply the workaround separately from the parent scope, so default to not
        // requiring it.
        self.needs_const_eval_workaround = false;
        self.set_const_eval_workaround_for(e);
        let result = self.assemble_expression_kind(e, parent_precedence);
        self.needs_const_eval_workaround = original_value;
        result
    }

    /// The constructor of `AutoConstEvalWorkaround`: intrinsic function calls and operators can
    /// be const-evaluated by the WGSL compiler, which may cause it complain if said evaluation
    /// would produce non-finite values.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1328-L1371 (chrome/m156)
    fn set_const_eval_workaround_for(&mut self, parent: crate::ir::ExprId) {
        let pool = &self.ctx.pool;
        match &pool.expression(parent).kind {
            ExpressionKind::FunctionCall(call) => {
                // User-defined functions can never be const-evaluated.
                if pool.function(call.function).is_intrinsic()
                    && all_arguments_constant(pool, &call.arguments)
                {
                    self.needs_const_eval_workaround = true;
                }
            }
            ExpressionKind::Binary(op) => {
                // Only ops that produce overflows or non-finite values need to be worked round:
                // +,-,*,/
                self.needs_const_eval_workaround = match op.operator.kind() {
                    OperatorKind::Minus
                    | OperatorKind::Plus
                    | OperatorKind::Star
                    | OperatorKind::Slash => {
                        get_constant_value_or_null(pool, op.left).is_some()
                            && get_constant_value_or_null(pool, op.right).is_some()
                    }
                    _ => false,
                };
            }
            _ => {}
        }
    }

    fn assemble_expression_kind(
        &mut self,
        e: crate::ir::ExprId,
        parent_precedence: Precedence,
    ) -> String {
        let expr: Expression = self.ctx.pool.expression(e).clone();
        match &expr.kind {
            ExpressionKind::Binary(b) => self.assemble_binary_expression(
                b.left,
                b.operator,
                b.right,
                expr.ty,
                parent_precedence,
            ),
            ExpressionKind::ConstructorCompound(c) => {
                self.assemble_constructor_compound(&c.arguments, expr.ty, expr.position)
            }
            ExpressionKind::ConstructorArrayCast(c) => {
                self.assemble_constructor_array_cast(c.argument, expr.ty, parent_precedence)
            }
            ExpressionKind::ConstructorArray(_)
            | ExpressionKind::ConstructorCompoundCast(_)
            | ExpressionKind::ConstructorScalarCast(_)
            | ExpressionKind::ConstructorSplat(_)
            | ExpressionKind::ConstructorStruct(_) => {
                let arguments = expr
                    .any_constructor_arguments()
                    .expect("a constructor has arguments")
                    .to_vec();
                self.assemble_any_constructor(&arguments, expr.ty)
            }
            ExpressionKind::ConstructorDiagonalMatrix(c) => {
                self.assemble_constructor_diagonal_matrix(c.argument, expr.ty)
            }
            ExpressionKind::ConstructorMatrixResize(c) => {
                self.assemble_constructor_matrix_resize(c.argument, expr.ty)
            }
            ExpressionKind::Empty(_) => "false".to_owned(),
            ExpressionKind::FieldAccess(f) => self.assemble_field_access(f),
            ExpressionKind::FunctionCall(c) => {
                let call = CallInfo {
                    function: c.function,
                    args: c.arguments.clone(),
                    ty: expr.ty,
                };
                self.assemble_function_call(&call, parent_precedence)
            }
            ExpressionKind::Index(i) => self.assemble_index_expression(i.base, i.index),
            ExpressionKind::Literal(l) => self.assemble_literal(*l, expr.ty),
            ExpressionKind::Prefix(p) => {
                self.assemble_prefix_expression(p.operator, p.operand, parent_precedence)
            }
            ExpressionKind::Postfix(p) => {
                self.assemble_postfix_expression(p.operator, p.operand, parent_precedence)
            }
            ExpressionKind::Setting(s) => {
                let caps = self.caps;
                let literal = s.to_literal(self.ctx, expr.position, expr.ty, caps);
                self.assemble_expression(literal, parent_precedence, AssembleMode::Auto)
            }
            ExpressionKind::Swizzle(s) => self.assemble_swizzle(s.base, s.components.as_slice()),
            ExpressionKind::Ternary(t) => self.assemble_ternary_expression(
                t.test,
                t.if_true,
                t.if_false,
                expr.ty,
                parent_precedence,
            ),
            ExpressionKind::VariableReference(r) => self.assemble_variable_reference(r.variable),
            // unsupported expression
            ExpressionKind::ChildCall(_)
            | ExpressionKind::FunctionReference(_)
            | ExpressionKind::MethodReference(_)
            | ExpressionKind::Poison(_)
            | ExpressionKind::TypeReference(_) => String::new(),
        }
    }

    /// `binaryOpNeedsComponentwiseMatrixPolyfill(left, right, op)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2798-L2822 (chrome/m156)
    fn binary_op_needs_componentwise_matrix_polyfill(
        &self,
        left: TypeId,
        right: TypeId,
        op: Operator,
    ) -> bool {
        let pool = &self.ctx.pool;
        let (left, right) = (pool.ty(left), pool.ty(right));
        match op.kind() {
            OperatorKind::Slash | OperatorKind::SlashEq => {
                // WGSL does not natively support componentwise matrix-op-matrix for division.
                if left.is_matrix() && right.is_matrix() {
                    return true;
                }
                // WGSL does not natively support componentwise matrix-op-scalar or
                // scalar-op-matrix for addition, subtraction or division.
                (left.is_matrix() && right.is_scalar()) || (left.is_scalar() && right.is_matrix())
            }
            OperatorKind::Plus
            | OperatorKind::PlusEq
            | OperatorKind::Minus
            | OperatorKind::MinusEq => {
                (left.is_matrix() && right.is_scalar()) || (left.is_scalar() && right.is_matrix())
            }
            _ => false,
        }
    }

    /// `assembleBinaryExpression(left, op, right, resultType, parentPrecedence)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2824-L3014 (chrome/m156)
    fn assemble_binary_expression(
        &mut self,
        left: crate::ir::ExprId,
        mut op: Operator,
        right: crate::ir::ExprId,
        result_type: TypeId,
        parent_precedence: Precedence,
    ) -> String {
        // If the operator is && or ||, we need to handle short-circuiting properly. Specifically,
        // we sometimes need to emit extra statements to paper over functionality that WGSL lacks,
        // like assignment in the middle of an expression. We need to guard those extra
        // statements, to ensure that they don't occur if the expression evaluation is
        // short-circuited. Converting the expression into an if-else block keeps the
        // short-circuit property intact even when extra statements are involved.
        // If the RHS doesn't have any side effects, then it's safe to just leave the expression
        // as-is, since we know that any possible extra statements are non-side-effecting.
        let mut expr: String;
        if op.kind() == OperatorKind::LogicalAnd && has_side_effects(&self.ctx.pool, right) {
            // Converts `left_expression && right_expression` into the following block:

            // var _skTemp1: bool;
            // [[ prepare left_expression ]]
            // if left_expression {
            //     [[ prepare right_expression ]]
            //     _skTemp1 = right_expression;
            // } else {
            //     _skTemp1 = false;
            // }

            expr = self.write_scratch_var(result_type, "");

            let left_expr =
                self.assemble_expression(left, Precedence::EXPRESSION, AssembleMode::Auto);
            self.write("if ");
            self.write(&left_expr);
            self.write_line(" {");

            self.indentation += 1;
            let right_expr =
                self.assemble_expression(right, Precedence::Assignment, AssembleMode::Auto);
            self.write(&expr);
            self.write(" = ");
            self.write(&right_expr);
            self.write_line(";");
            self.indentation -= 1;

            self.write_line("} else {");

            self.indentation += 1;
            self.write(&expr);
            self.write_line(" = false;");
            self.indentation -= 1;

            self.write_line("}");
            return expr;
        }

        if op.kind() == OperatorKind::LogicalOr && has_side_effects(&self.ctx.pool, right) {
            // Converts `left_expression || right_expression` into the following block:

            // var _skTemp1: bool;
            // [[ prepare left_expression ]]
            // if left_expression {
            //     _skTemp1 = true;
            // } else {
            //     [[ prepare right_expression ]]
            //     _skTemp1 = right_expression;
            // }

            expr = self.write_scratch_var(result_type, "");

            let left_expr =
                self.assemble_expression(left, Precedence::EXPRESSION, AssembleMode::Auto);
            self.write("if ");
            self.write(&left_expr);
            self.write_line(" {");

            self.indentation += 1;
            self.write(&expr);
            self.write_line(" = true;");
            self.indentation -= 1;

            self.write_line("} else {");

            self.indentation += 1;
            let right_expr =
                self.assemble_expression(right, Precedence::Assignment, AssembleMode::Auto);
            self.write(&expr);
            self.write(" = ");
            self.write(&right_expr);
            self.write_line(";");
            self.indentation -= 1;

            self.write_line("}");
            return expr;
        }

        // Handle comma-expressions.
        if op.kind() == OperatorKind::Comma {
            // The result from the left-expression is ignored, but its side effects must occur.
            self.assemble_expression(left, Precedence::Statement, AssembleMode::Auto);

            // Evaluate the right side normally.
            return self.assemble_expression(right, parent_precedence, AssembleMode::Auto);
        }

        // Handle assignment-expressions.
        let left_type = self.expr_type(left);
        let right_type = self.expr_type(right);
        let comp_matrix_op =
            self.binary_op_needs_componentwise_matrix_polyfill(left_type, right_type, op);

        if op.is_assignment() {
            let Some(lvalue) = self.make_lvalue(left) else {
                return String::new();
            };

            if op.kind() == OperatorKind::Eq {
                // Evaluate the right-hand side of simple assignment (`a = b` --> `b`).
                expr = self.assemble_expression(right, Precedence::Assignment, AssembleMode::Auto);
            } else {
                // Evaluate the right-hand side of compound-assignment (`a += b` --> `a + b`).
                op = op.remove_assignment();

                let lhs = lvalue.load();
                let rhs = self.assemble_expression(
                    right,
                    op.binary_precedence(),
                    if comp_matrix_op {
                        AssembleMode::UsedMultipleTimes
                    } else {
                        AssembleMode::Auto
                    },
                );

                if comp_matrix_op {
                    expr = self.assemble_componentwise_matrix_binary(
                        left_type, right_type, &lhs, &rhs, op,
                    );
                } else {
                    expr = format!("{lhs}{}{rhs}", operator_name(op));
                }
            }

            // Emit the assignment statement (`a = a + b`).
            let store = lvalue.store(self.ctx, &expr);
            self.write_line(&store);

            // Return the lvalue (`a`) as the result, since the value might be used by the caller.
            return lvalue.load();
        }

        if op.is_equality() {
            return self.assemble_equality_expression_exprs(left, right, op, parent_precedence);
        }

        let mut precedence = op.binary_precedence();
        let need_parens = precedence >= parent_precedence;
        if binary_op_is_ambiguous_in_wgsl(op) {
            precedence = Precedence::Parentheses;
        }
        expr = String::new();
        if need_parens {
            expr = "(".to_owned();
        }

        let mode = if comp_matrix_op {
            AssembleMode::UsedMultipleTimes
        } else {
            AssembleMode::Auto
        };
        let lhs = self.assemble_expression(left, precedence, mode);
        let rhs = self.assemble_expression(right, precedence, mode);
        if comp_matrix_op {
            expr +=
                &self.assemble_componentwise_matrix_binary(left_type, right_type, &lhs, &rhs, op);
        } else {
            expr += &lhs;
            expr += operator_name(op);
            expr += &rhs;
        }

        if need_parens {
            expr.push(')');
        }

        expr
    }

    /// The key of the polyfill map for the field `index` of struct type `ty`.
    pub(super) fn field_key(&self, ty: TypeId, index: usize) -> (TypeId, usize) {
        (self.ctx.pool.ty(ty).resolve().id(), index)
    }

    /// `assembleFieldAccess(f)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3016-L3070 (chrome/m156)
    fn assemble_field_access(&mut self, f: &crate::ir::FieldAccess) -> String {
        let base_type = self.expr_type(f.base);
        let key = self.field_key(base_type, f.field_index);
        let field = self.ctx.pool.ty(base_type).fields()[f.field_index].clone();
        let mut expr = String::new();

        if let Some(polyfill_info) = self.field_polyfill_map.get_mut(&key) {
            // We found a matrix uniform. We are required to pass some matrix uniforms as array
            // vectors, since the std140 layout for a matrix assumes 4-column vectors for each
            // row, and WGSL tightly packs 2-column matrices. When emitting code, we replace the
            // field-access expression with a global variable which holds an unpacked version of
            // the uniform.
            polyfill_info.was_accessed = true;
            let replacement_name = polyfill_info.replacement_name.clone();

            // The polyfill can either be based directly onto a uniform in an interface block, or
            // it might be based on an index-expression onto a uniform if the interface block is
            // arrayed.
            let mut index_expr = None;
            if let ExpressionKind::Index(i) = &self.ctx.pool.expression(f.base).kind {
                index_expr = Some(i.index);
            }

            expr = replacement_name;

            // If we had an index expression, we must append the index.
            if let Some(index) = index_expr {
                expr.push('[');
                expr += &self.assemble_expression(index, Precedence::Sequence, AssembleMode::Auto);
                expr.push(']');
            }
            return expr;
        }

        match f.owner_kind {
            FieldAccessOwnerKind::Default => {
                expr =
                    self.assemble_expression(f.base, Precedence::Postfix, AssembleMode::Auto) + ".";
            }
            FieldAccessOwnerKind::AnonymousInterfaceBlock => {
                if let ExpressionKind::VariableReference(r) = &self.ctx.pool.expression(f.base).kind
                    && field.layout.builtin != SK_POINTSIZE_BUILTIN
                {
                    let variable = r.variable;
                    expr = self.variable_prefix(variable);
                }
            }
        }

        expr += &self.assemble_name(&field.name);
        expr
    }

    /// `assembleIndexExpression(i)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3835-L3841 (chrome/m156)
    pub(super) fn assemble_index_expression(
        &mut self,
        base: crate::ir::ExprId,
        index: crate::ir::ExprId,
    ) -> String {
        // Put the index value into a let-expression.
        let idx = self.assemble_expression(
            index,
            Precedence::EXPRESSION,
            AssembleMode::UsedMultipleTimes,
        );
        self.assemble_expression(base, Precedence::Postfix, AssembleMode::Auto) + "[" + &idx + "]"
    }

    /// `assembleLiteral(l)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3843-L3862 (chrome/m156)
    fn assemble_literal(&self, l: Literal, ty: TypeId) -> String {
        let pool = &self.ctx.pool;
        let t = pool.ty(ty);
        if t.is_float() || t.is_boolean() {
            let mut value = l.description(pool, ty);
            if type_is_low_precision(self.ctx, ty) {
                debug_assert!(t.is_float());
                value.push('h');
            }
            return value;
        }
        debug_assert!(t.is_integer());
        if t.matches(TypeId::UINT) {
            format!("{}u", l.int_value() & 0xffff_ffff)
        } else if t.matches(TypeId::USHORT) {
            format!("{}u", l.int_value() & 0xffff)
        } else {
            l.int_value().to_string()
        }
    }

    /// `assembleIncrementExpr(type)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3864-L3877 (chrome/m156)
    fn assemble_increment_expr(&self, ty: TypeId) -> String {
        // `type(`
        let mut expr = to_wgsl_type_simple(self.ctx, ty);
        expr.push('(');

        // `1, 1, 1...)`
        let mut separator = crate::string::Separator::new();
        let mut slots = self.ctx.pool.ty(ty).slot_count();
        while slots > 0 {
            expr += separator.next_str();
            expr += "1";
            slots -= 1;
        }
        expr.push(')');
        expr
    }

    /// `assemblePrefixExpression(p, parentPrecedence)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3879-L3938 (chrome/m156)
    fn assemble_prefix_expression(
        &mut self,
        op: Operator,
        operand: crate::ir::ExprId,
        parent_precedence: Precedence,
    ) -> String {
        // Unary + does nothing, so we omit it from the output.
        if op.kind() == OperatorKind::Plus {
            return self.assemble_expression(operand, Precedence::Prefix, AssembleMode::Auto);
        }

        // Pre-increment/decrement expressions have no direct equivalent in WGSL.
        if op.kind() == OperatorKind::PlusPlus || op.kind() == OperatorKind::MinusMinus {
            let Some(lvalue) = self.make_lvalue(operand) else {
                return String::new();
            };

            // Generate the new value: `lvalue + type(1, 1, 1...)`.
            let operand_type = self.expr_type(operand);
            let new_value = lvalue.load()
                + if op.kind() == OperatorKind::PlusPlus {
                    " + "
                } else {
                    " - "
                }
                + &self.assemble_increment_expr(operand_type);
            let store = lvalue.store(self.ctx, &new_value);
            self.write_line(&store);
            return lvalue.load();
        }

        // WGSL natively supports unary negation/not expressions (!,~,-).
        debug_assert!(matches!(
            op.kind(),
            OperatorKind::LogicalNot | OperatorKind::BitwiseNot | OperatorKind::Minus
        ));

        // The unary negation operator only applies to scalars and vectors. For other mathematical
        // objects (such as matrices) we can express it as a multiplication by -1.
        let mut expr = String::new();
        let operand_type = self.ctx.pool.ty(self.expr_type(operand));
        let needs_negation = op.kind() == OperatorKind::Minus
            && !operand_type.is_scalar()
            && !operand_type.is_vector();
        let need_parens = needs_negation || Precedence::Prefix >= parent_precedence;

        if need_parens {
            expr.push('(');
        }

        if needs_negation {
            expr += "-1.0 * ";
            expr +=
                &self.assemble_expression(operand, Precedence::Multiplicative, AssembleMode::Auto);
        } else {
            expr += op.tight_operator_name();
            expr += &self.assemble_expression(operand, Precedence::Prefix, AssembleMode::Auto);
        }

        if need_parens {
            expr.push(')');
        }

        expr
    }

    /// `assemblePostfixExpression(p, parentPrecedence)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3940-L3970 (chrome/m156)
    fn assemble_postfix_expression(
        &mut self,
        op: Operator,
        operand: crate::ir::ExprId,
        parent_precedence: Precedence,
    ) -> String {
        debug_assert!(matches!(
            op.kind(),
            OperatorKind::PlusPlus | OperatorKind::MinusMinus
        ));

        // Post-increment/decrement expressions have no direct equivalent in WGSL; they do exist
        // as a standalone statement for convenience, but these aren't the same as SkSL's
        // post-increments.
        let Some(lvalue) = self.make_lvalue(operand) else {
            return String::new();
        };

        // If the expression is used, create a let-copy of the original value.
        // (At statement-level precedence, we know the value is unused and can skip this
        // let-copy.)
        let mut original_value = String::new();
        if parent_precedence != Precedence::Statement {
            original_value = self.write_scratch_let(&lvalue.load(), false);
        }
        // Generate the new value: `lvalue + type(1, 1, 1...)`.
        let operand_type = self.expr_type(operand);
        let new_value = lvalue.load()
            + if op.kind() == OperatorKind::PlusPlus {
                " + "
            } else {
                " - "
            }
            + &self.assemble_increment_expr(operand_type);
        let store = lvalue.store(self.ctx, &new_value);
        self.write_line(&store);

        original_value
    }

    /// `assembleSwizzle(swizzle)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3972-L3975 (chrome/m156)
    fn assemble_swizzle(&mut self, base: crate::ir::ExprId, components: &[i8]) -> String {
        self.assemble_expression(base, Precedence::Postfix, AssembleMode::Auto)
            + "."
            + &Swizzle::mask_string(components)
    }

    /// `writeScratchVar(type, value)`: writes a scratch variable into the program and returns its
    /// name (e.g. `_skTemp123`).
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3977-L3990 (chrome/m156)
    pub(super) fn write_scratch_var(&mut self, ty: TypeId, value: &str) -> String {
        let scratch_var_name = format!("_skTemp{}", self.scratch_count);
        self.scratch_count += 1;
        self.write("var ");
        self.write(&scratch_var_name);
        self.write(": ");
        let wgsl_type = to_wgsl_type_simple(self.ctx, ty);
        self.write(&wgsl_type);
        if !value.is_empty() {
            self.write(" = ");
            self.write(value);
        }
        self.write_line(";");
        scratch_var_name
    }

    /// `writeScratchLet(expr, isCompileTimeConstant)`: writes a scratch let-variable into the
    /// program, gives it the value of `expr`, and returns its name (e.g. `_skTemp123`). This can
    /// be `const` instead of `let` when `is_compile_time_constant` is true and at function scope.
    ///
    /// Prefer using `assemble_expression` with an `AssembleMode` to control when scratch lets
    /// should be created automatically.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3992-L4003 (chrome/m156)
    pub(super) fn write_scratch_let(
        &mut self,
        expr: &str,
        is_compile_time_constant: bool,
    ) -> String {
        let scratch_var_name = format!("_skTemp{}", self.scratch_count);
        self.scratch_count += 1;
        self.write(if self.at_function_scope && !is_compile_time_constant {
            "let "
        } else {
            "const "
        });
        self.write(&scratch_var_name);
        self.write(" = ");
        self.write(expr);
        self.write_line(";");
        scratch_var_name
    }

    /// `assembleTernaryExpression(t, parentPrecedence)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4005-L4083 (chrome/m156)
    fn assemble_ternary_expression(
        &mut self,
        test: crate::ir::ExprId,
        if_true: crate::ir::ExprId,
        if_false: crate::ir::ExprId,
        ty: TypeId,
        parent_precedence: Precedence,
    ) -> String {
        let mut expr = String::new();

        // The trivial case is when neither branch has side effects and evaluate to a scalar or
        // vector type. This can be represented with a call to the WGSL `select` intrinsic.
        // Select doesn't support short-circuiting, so we should only use it when both the true-
        // and false-expressions are trivial to evaluate.
        let (is_scalar, is_vector, columns) = {
            let t = self.ctx.pool.ty(ty);
            (
                t.is_scalar(),
                t.is_vector(),
                if t.is_vector() { t.columns() } else { 0 },
            )
        };
        if (is_scalar || is_vector)
            && !has_side_effects(&self.ctx.pool, test)
            && is_trivial_expression(&self.ctx.pool, if_true)
            && is_trivial_expression(&self.ctx.pool, if_false)
        {
            let need_parens = Precedence::Ternary >= parent_precedence;
            if need_parens {
                expr.push('(');
            }
            expr += "select(";
            expr += &self.assemble_expression(if_false, Precedence::Sequence, AssembleMode::Auto);
            expr += ", ";
            expr += &self.assemble_expression(if_true, Precedence::Sequence, AssembleMode::Auto);
            expr += ", ";

            if is_vector {
                // Splat the condition expression into a vector.
                expr += &format!("vec{columns}<bool>(");
            }
            expr += &self.assemble_expression(test, Precedence::Sequence, AssembleMode::Auto);
            if is_vector {
                expr.push(')');
            }
            expr.push(')');
            if need_parens {
                expr.push(')');
            }
        } else {
            // WGSL does not support ternary expressions. Instead, we hoist the expression out
            // into the surrounding block, convert it into an if statement, and write the result
            // to a synthesized variable. Instead of the original expression, we return that
            // variable.
            let true_type = self.expr_type(if_true);
            expr = self.write_scratch_var(true_type, "");

            let test_expr =
                self.assemble_expression(test, Precedence::EXPRESSION, AssembleMode::Auto);
            self.write("if ");
            self.write(&test_expr);
            self.write_line(" {");

            self.indentation += 1;
            let true_expr =
                self.assemble_expression(if_true, Precedence::Assignment, AssembleMode::Auto);
            self.write(&expr);
            self.write(" = ");
            self.write(&true_expr);
            self.write_line(";");
            self.indentation -= 1;

            self.write_line("} else {");

            self.indentation += 1;
            let false_expr =
                self.assemble_expression(if_false, Precedence::Assignment, AssembleMode::Auto);
            self.write(&expr);
            self.write(" = ");
            self.write(&false_expr);
            self.write_line(";");
            self.indentation -= 1;

            self.write_line("}");
        }
        expr
    }

    /// `variablePrefix(v)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4085-L4121 (chrome/m156)
    fn variable_prefix(&self, v: VarId) -> String {
        let pool = &self.ctx.pool;
        let variable = pool.variable(v);
        if variable.storage == VariableStorage::Global {
            // If the field refers to a pipeline IO parameter, then we access it via the
            // synthesized IO structs. We make an explicit exception for `sk_PointSize` which we
            // declare as a placeholder variable in global scope as it is not supported by WebGPU
            // as a pipeline IO parameter (see comments in `writeStageOutputStruct`).
            if variable
                .modifier_flags
                .contains(crate::ir::ModifierFlags::IN)
            {
                return "_stageIn.".to_owned();
            }
            if variable
                .modifier_flags
                .contains(crate::ir::ModifierFlags::OUT)
            {
                return "(*_stageOut).".to_owned();
            }

            // If the field refers to an anonymous-interface-block structure, access it via the
            // synthesized `_uniform0` or `_storage1` global.
            if let Some(ib) = variable.interface_block {
                let ib_type = self.interface_block_struct_type(ib);
                if let Some(ib_name) = self.interface_block_name_map.get(&ib_type) {
                    return format!("{ib_name}.");
                }
            }

            // If the field refers to an top-level uniform, access it via the synthesized
            // `_globalUniforms` global. (Note that this should only occur in test code; Skia will
            // always put uniforms in an interface block.)
            if self.is_in_global_uniforms(variable) {
                return "_globalUniforms.".to_owned();
            }
        }

        String::new()
    }

    /// `variableReferenceNameForLValue(r)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4123-L4161 (chrome/m156)
    fn variable_reference_name_for_lvalue(&mut self, v_id: VarId) -> String {
        let v = self.ctx.pool.variable(v_id).clone();
        let (is_unsized_array, _) = {
            let t = self.ctx.pool.ty(v.ty);
            (t.is_unsized_array(), 0)
        };

        if v.storage == VariableStorage::Parameter
            && (v.modifier_flags.contains(crate::ir::ModifierFlags::OUT) || is_unsized_array)
        {
            // This is an out-parameter or unsized array parameter; it's pointer-typed, so we need
            // to dereference it. We wrap the dereference in parentheses, in case the value is
            // used in an access expression later.
            return format!("(*{})", self.assemble_name(v.mangled_name()));
        }

        if self.is_in_global_uniforms(&v) {
            // Assemble polyfill access (see assembleFieldAccess)
            let block = self
                .synthetic_global_uniforms_block
                .expect("global uniforms have a synthetic interface block");
            let global_struct = self.interface_block_struct_type(block);
            let mut key = None;
            for (index, f) in self.ctx.pool.ty(global_struct).fields().iter().enumerate() {
                if *f.name == *v.name {
                    key = Some(self.field_key(global_struct, index));
                    break;
                }
            }

            if let Some(polyfill_info) = key.and_then(|k| self.field_polyfill_map.get_mut(&k)) {
                // We found a global variable declaration that had to be polyfilled, so switch to
                // its replacement expression. This is simpler than assembleFieldAccess() because
                // we don't need to worry about index expressions.
                polyfill_info.was_accessed = true;
                return polyfill_info.replacement_name.clone();
            }

            // Fall through for a regular global variable access
        }

        self.variable_prefix(v_id) + &self.assemble_name(v.mangled_name())
    }

    /// `assembleVariableReference(r)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4163-L4183 (chrome/m156)
    fn assemble_variable_reference(&mut self, v_id: VarId) -> String {
        // TODO(b/294274678): Correctly handle RTFlip for built-ins.

        // Insert a conversion expression if this is a built-in variable whose type differs from
        // the SkSL.
        let mut expr = String::new();
        let conversion = needs_builtin_type_conversion(self.ctx.pool.variable(v_id));
        if let Some(conversion) = conversion {
            expr += conversion;
            expr.push('(');
        }

        expr += &self.variable_reference_name_for_lvalue(v_id);

        if conversion.is_some() {
            expr.push(')');
        }

        expr
    }

    /// `assembleAnyConstructor(c)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4185-L4195 (chrome/m156)
    fn assemble_any_constructor(&mut self, arguments: &[crate::ir::ExprId], ty: TypeId) -> String {
        let mut expr = to_wgsl_type_simple(self.ctx, ty);
        expr.push('(');
        let mut separator = crate::string::Separator::new();
        for &e in arguments {
            expr += separator.next_str();
            expr += &self.assemble_expression(e, Precedence::Sequence, AssembleMode::Auto);
        }
        expr.push(')');
        expr
    }

    /// `assembleConstructorCompound(c)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4197-L4206 (chrome/m156)
    fn assemble_constructor_compound(
        &mut self,
        arguments: &[crate::ir::ExprId],
        ty: TypeId,
        position: crate::position::Position,
    ) -> String {
        let (is_vector, is_matrix) = {
            let t = self.ctx.pool.ty(ty);
            (t.is_vector(), t.is_matrix())
        };
        if is_vector {
            self.assemble_constructor_compound_vector(arguments, ty)
        } else if is_matrix {
            self.assemble_constructor_compound_matrix(arguments, ty)
        } else {
            self.ctx
                .errors
                .error(position, "unsupported compound constructor");
            String::new()
        }
    }

    /// `assembleConstructorCompoundVector(c)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4208-L4229 (chrome/m156)
    fn assemble_constructor_compound_vector(
        &mut self,
        arguments: &[crate::ir::ExprId],
        ty: TypeId,
    ) -> String {
        // WGSL supports constructing vectors from a mix of scalars and vectors but
        // not matrices (see https://www.w3.org/TR/WGSL/#type-constructor-expr).
        //
        // SkSL supports vec4(mat2x2) which we handle specially.
        if self.ctx.pool.ty(ty).columns() == 4 && arguments.len() == 1 {
            let arg = arguments[0];
            let arg_type = self.expr_type(arg);
            if self.ctx.pool.ty(arg_type).is_matrix() {
                debug_assert!(self.ctx.pool.ty(arg_type).columns() == 2);
                debug_assert!(self.ctx.pool.ty(arg_type).rows() == 2);

                let matrix = self.assemble_expression(
                    arg,
                    Precedence::Postfix,
                    AssembleMode::UsedMultipleTimes,
                );
                return format!(
                    "{}({matrix}[0], {matrix}[1])",
                    to_wgsl_type_simple(self.ctx, ty)
                );
            }
        }
        self.assemble_any_constructor(arguments, ty)
    }

    /// `assembleConstructorCompoundMatrix(ctor)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4231-L4254 (chrome/m156)
    fn assemble_constructor_compound_matrix(
        &mut self,
        arguments: &[crate::ir::ExprId],
        ty: TypeId,
    ) -> String {
        debug_assert!(self.ctx.pool.ty(ty).is_matrix());

        let mut expr = to_wgsl_type_simple(self.ctx, ty) + "(";
        let mut separator = crate::string::Separator::new();
        for &arg in arguments {
            let arg_type = self.expr_type(arg);
            let (is_scalar, num_slots) = {
                let t = self.ctx.pool.ty(arg_type);
                debug_assert!(t.is_scalar() || t.is_vector());
                (t.is_scalar(), t.slot_count())
            };

            if is_scalar {
                expr += separator.next_str();
                expr += &self.assemble_expression(arg, Precedence::Sequence, AssembleMode::Auto);
            } else {
                let inner = self.assemble_expression(
                    arg,
                    Precedence::Sequence,
                    AssembleMode::UsedMultipleTimes,
                );
                for slot in 0..num_slots {
                    expr += &format!("{}{inner}[{slot}]", separator.next_str());
                }
            }
        }
        expr + ")"
    }

    /// `assembleConstructorDiagonalMatrix(c)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4256-L4283 (chrome/m156)
    fn assemble_constructor_diagonal_matrix(
        &mut self,
        argument: crate::ir::ExprId,
        ty: TypeId,
    ) -> String {
        debug_assert!(self.ctx.pool.ty(ty).is_matrix());
        let arg_type = self.expr_type(argument);
        debug_assert!(self.ctx.pool.ty(arg_type).is_scalar());

        // Evaluate the inner-expression, creating a scratch variable if necessary.
        let inner = self.assemble_expression(
            argument,
            Precedence::Assignment,
            AssembleMode::UsedMultipleTimes,
        );

        // Assemble a diagonal-matrix expression.
        let mut expr = to_wgsl_type_simple(self.ctx, ty) + "(";
        let mut separator = crate::string::Separator::new();
        let (columns, rows) = {
            let t = self.ctx.pool.ty(ty);
            (t.columns(), t.rows())
        };
        for col in 0..columns {
            for row in 0..rows {
                expr += separator.next_str();
                if col == row {
                    expr += &inner;
                } else {
                    expr += "0.0";
                }
            }
        }
        expr + ")"
    }

    /// `assembleConstructorMatrixResize(ctor)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4285-L4313 (chrome/m156)
    fn assemble_constructor_matrix_resize(
        &mut self,
        argument: crate::ir::ExprId,
        ty: TypeId,
    ) -> String {
        let source = self.assemble_expression(
            argument,
            Precedence::Sequence,
            AssembleMode::UsedMultipleTimes,
        );
        let (columns, rows) = {
            let t = self.ctx.pool.ty(ty);
            (t.columns(), t.rows())
        };
        let arg_type = self.expr_type(argument);
        let (source_columns, source_rows) = {
            let t = self.ctx.pool.ty(arg_type);
            (t.columns(), t.rows())
        };
        let mut separator = crate::string::Separator::new();
        let mut expr = to_wgsl_type_simple(self.ctx, ty) + "(";

        for c in 0..columns {
            for r in 0..rows {
                expr += separator.next_str();
                if c < source_columns && r < source_rows {
                    expr += &format!("{source}[{c}][{r}]");
                } else if r == c {
                    expr += "1.0";
                } else {
                    expr += "0.0";
                }
            }
        }

        expr + ")"
    }

    /// `assembleConstructorArrayCast(ctor, parentPrecedence)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4315-L4349 (chrome/m156)
    fn assemble_constructor_array_cast(
        &mut self,
        argument: crate::ir::ExprId,
        ty: TypeId,
        parent_precedence: Precedence,
    ) -> String {
        debug_assert!(!self.ctx.pool.ty(ty).is_unsized_array()); // Can't iterate over unbounded elements

        // Our SkSL -> WGSL conversion only distinguishes f16 and f32, so any other array cast
        // turns into equivalent types in WGSL. Check the component type of the ctor and arg,
        // since those are both array types, which has to be peeled away.
        let ctor_component = self.ctx.pool.ty(ty).component_type().id();
        let arg_type = self.expr_type(argument);
        let arg_component = self.ctx.pool.ty(arg_type).component_type().id();
        if to_wgsl_type_simple(self.ctx, ctor_component)
            == to_wgsl_type_simple(self.ctx, arg_component)
        {
            return self.assemble_expression(argument, parent_precedence, AssembleMode::Auto);
        }

        // Construct a new array of the ctor's type
        let mut expr = to_wgsl_type_simple(self.ctx, ty);
        expr += "(";

        let elem_type = to_wgsl_type_simple(self.ctx, ctor_component);
        let array_arg = self.assemble_expression(
            argument,
            Precedence::Postfix,
            AssembleMode::UsedMultipleTimes,
        );

        // Each value of the new array is a cast of an element from the ctor's argument.
        let array_len = self.ctx.pool.ty(ty).columns();
        let mut separator = crate::string::Separator::new();
        for i in 0..array_len {
            expr += separator.next_str();
            expr += &format!("{elem_type}({array_arg}[{i}])");
        }

        expr += ")";
        expr
    }

    /// `assembleEqualityExpression(left, leftName, right, rightName, op, parentPrecedence)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4351-L4456 (chrome/m156)
    fn assemble_equality_expression(
        &mut self,
        left: TypeId,
        left_name: &str,
        right: TypeId,
        right_name: &str,
        op: Operator,
        parent_precedence: Precedence,
    ) -> String {
        debug_assert!(matches!(op.kind(), OperatorKind::EqEq | OperatorKind::Neq));

        let mut expr = String::new();
        let is_equal = op.kind() == OperatorKind::EqEq;
        let combiner = if is_equal { " && " } else { " || " };

        let (is_matrix, is_array, is_struct, is_vector, columns) = {
            let t = self.ctx.pool.ty(left);
            (
                t.is_matrix(),
                t.is_array(),
                t.is_struct(),
                t.is_vector(),
                if t.is_array() { t.columns() } else { 0 },
            )
        };

        if is_matrix {
            // Each matrix column must be compared as if it were an individual vector.
            let (left_rows, left_columns) = {
                let t = self.ctx.pool.ty(left);
                (t.rows(), t.columns())
            };
            {
                let r = self.ctx.pool.ty(right);
                debug_assert!(r.is_matrix());
                debug_assert!(left_rows == r.rows());
                debug_assert!(left_columns == r.columns());
            }
            // `left.columnType(fContext)`.
            let vec_type = self
                .ctx
                .pool
                .ty(left)
                .component_type()
                .to_compound(left_rows, 1);
            let mut separator = "(";
            for index in 0..left_columns {
                expr += separator;
                let suffix = format!("[{index}]");
                expr += &self.assemble_equality_expression(
                    vec_type,
                    &format!("{left_name}{suffix}"),
                    vec_type,
                    &format!("{right_name}{suffix}"),
                    op,
                    Precedence::Parentheses,
                );
                separator = combiner;
            }
            return expr + ")";
        }

        if is_array {
            debug_assert!(self.ctx.pool.ty(right).matches(left));
            let indexed_type = self.ctx.pool.ty(left).component_type().id();
            let mut separator = "(";
            for index in 0..columns {
                expr += separator;
                let suffix = format!("[{index}]");
                expr += &self.assemble_equality_expression(
                    indexed_type,
                    &format!("{left_name}{suffix}"),
                    indexed_type,
                    &format!("{right_name}{suffix}"),
                    op,
                    Precedence::Parentheses,
                );
                separator = combiner;
            }
            return expr + ")";
        }

        if is_struct {
            // Recursively compare every field in the struct.
            debug_assert!(self.ctx.pool.ty(right).matches(left));
            let fields = self.ctx.pool.ty(left).fields().to_vec();

            let mut separator = "(";
            for field in &fields {
                expr += separator;
                let field_name = self.assemble_name(&field.name);
                expr += &self.assemble_equality_expression(
                    field.ty,
                    &format!("{left_name}.{field_name}"),
                    field.ty,
                    &format!("{right_name}.{field_name}"),
                    op,
                    Precedence::Parentheses,
                );
                separator = combiner;
            }
            return expr + ")";
        }

        if is_vector {
            // Compare vectors via `all(x == y)` or `any(x != y)`.
            debug_assert!(self.ctx.pool.ty(right).is_vector());
            debug_assert!(
                self.ctx.pool.ty(left).slot_count() == self.ctx.pool.ty(right).slot_count()
            );

            expr += if is_equal { "all(" } else { "any(" };
            expr += left_name;
            expr += operator_name(op);
            expr += right_name;
            return expr + ")";
        }

        // Compare scalars via `x == y`.
        debug_assert!(self.ctx.pool.ty(right).is_scalar());
        if parent_precedence < Precedence::Sequence {
            expr = "(".to_owned();
        }
        expr += left_name;
        expr += operator_name(op);
        expr += right_name;
        if parent_precedence < Precedence::Sequence {
            expr += ")";
        }
        expr
    }

    /// `assembleEqualityExpression(left, right, op, parentPrecedence)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4458-L4470 (chrome/m156)
    fn assemble_equality_expression_exprs(
        &mut self,
        left: crate::ir::ExprId,
        right: crate::ir::ExprId,
        op: Operator,
        parent_precedence: Precedence,
    ) -> String {
        // WGSL supports scalar and vector comparisons natively. We know the expressions will only
        // be emitted once in that case. Every other case will be poly-filled, resulting in
        // referencing the left and right expressions multiple times.
        let left_type = self.expr_type(left);
        let right_type = self.expr_type(right);
        let mode = {
            let t = self.ctx.pool.ty(left_type);
            if !t.is_scalar() && !t.is_vector() {
                AssembleMode::UsedMultipleTimes
            } else {
                AssembleMode::Auto
            }
        };
        let left_name = self.assemble_expression(left, Precedence::Parentheses, mode);
        let right_name = self.assemble_expression(right, Precedence::Parentheses, mode);
        self.assemble_equality_expression(
            left_type,
            &left_name,
            right_type,
            &right_name,
            op,
            parent_precedence,
        )
    }

    /// `assembleTextureFromImageOrSampler(arg)`: helper for accessing textures from polyfilled
    /// texture and samplers.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3395-L3404 (chrome/m156)
    pub(super) fn assemble_texture_from_image_or_sampler(
        &mut self,
        arg: crate::ir::ExprId,
    ) -> String {
        let kind = self.ctx.pool.ty(self.expr_type(arg)).type_kind;
        debug_assert!(kind == TypeKind::Texture || kind == TypeKind::Sampler);
        let mut expr = self.assemble_expression(arg, Precedence::Sequence, AssembleMode::Auto);
        if kind == TypeKind::Sampler {
            expr += TEXTURE_SUFFIX;
        }
        expr
    }

    /// `assembleComponentwiseMatrixBinary(leftType, rightType, left, right, op)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3223-L3252 (chrome/m156)
    pub(super) fn assemble_componentwise_matrix_binary(
        &self,
        left_type: TypeId,
        right_type: TypeId,
        left: &str,
        right: &str,
        op: Operator,
    ) -> String {
        let left_is_matrix = self.ctx.pool.ty(left_type).is_matrix();
        let right_is_matrix = self.ctx.pool.ty(right_type).is_matrix();
        let matrix_type = if left_is_matrix {
            left_type
        } else {
            right_type
        };

        let mut expr = to_wgsl_type_simple(self.ctx, matrix_type) + "(";
        let mut separator = crate::string::Separator::new();
        let columns = self.ctx.pool.ty(matrix_type).columns();
        for c in 0..columns {
            expr += separator.next_str();
            expr += left;
            if left_is_matrix {
                expr.push('[');
                expr += &c.to_string();
                expr.push(']');
            }
            expr += op.operator_name();
            expr += right;
            if right_is_matrix {
                expr.push('[');
                expr += &c.to_string();
                expr.push(']');
            }
        }
        expr + ")"
    }

    /// `assembleFunctionCall(call, parentPrecedence)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3688-L3833 (chrome/m156)
    fn assemble_function_call(&mut self, call: &CallInfo, parent_precedence: Precedence) -> String {
        let (is_intrinsic, intrinsic_kind, parameters, mangled_name) = {
            let pool = &self.ctx.pool;
            let func = pool.function(call.function);
            (
                func.is_intrinsic(),
                func.intrinsic_kind,
                func.parameters.clone(),
                func.mangled_name(pool),
            )
        };

        // Many intrinsics need to be rewritten in WGSL.
        if is_intrinsic {
            let kind = intrinsic_kind.expect("an intrinsic has a kind");
            let expr = self.assemble_intrinsic_call(call, kind, parent_precedence);
            return self.emit_call(call.ty, parent_precedence, &expr, false);
        }

        // We implement function out-parameters by declaring them as pointers. SkSL follows GLSL's
        // out-parameter semantics, in which out-parameters are only written back to the original
        // variable after the function's execution is complete (see
        // https://www.khronos.org/opengl/wiki/Core_Language_(GLSL)#Parameters).
        //
        // In addition, SkSL supports swizzles and array index expressions to be passed into
        // out-parameters; however, WGSL does not allow taking their address into a pointer.
        //
        // We support these by using LValues to create temporary copies and then pass pointers to
        // the copies. Once the function returns, we copy the values back to the LValue.

        // First detect which arguments are passed to out-parameters.
        let args = &call.args;
        debug_assert!(args.len() == parameters.len());

        let mut writeback: Vec<Option<LValue>> = Vec::with_capacity(args.len());
        let mut substitute_argument: Vec<String> = Vec::with_capacity(args.len());

        let mut needs_writeback = false;
        for (index, &arg) in args.iter().enumerate() {
            let flags = self.ctx.pool.variable(parameters[index]).modifier_flags;
            if flags.contains(crate::ir::ModifierFlags::OUT) {
                if let Some(lvalue) = self.make_lvalue(arg) {
                    let arg_type = self.expr_type(arg);
                    if flags.contains(crate::ir::ModifierFlags::IN) {
                        // Load the lvalue's contents into the substitute argument.
                        let scratch = self.write_scratch_var(arg_type, &lvalue.load());
                        substitute_argument.push(scratch);
                    } else {
                        // Create a substitute argument, but leave it uninitialized.
                        let scratch = self.write_scratch_var(arg_type, "");
                        substitute_argument.push(scratch);
                    }
                    writeback.push(Some(lvalue));
                    needs_writeback = true;
                } else {
                    substitute_argument.push(String::new());
                    writeback.push(None);
                }
            } else {
                substitute_argument.push(String::new());
                writeback.push(None);
            }
        }

        let mut expr = self.assemble_name(&mangled_name);
        expr.push('(');
        let mut separator = crate::string::Separator::new();

        let func_dep_args = self.function_dependency_args(call.function);
        if !func_dep_args.is_empty() {
            expr += &func_dep_args;
            separator.next_str();
        }

        // Pass the function arguments, or any substitutes as needed.
        for (index, &arg) in args.iter().enumerate() {
            expr += separator.next_str();
            let arg_type = self.expr_type(arg);
            let (is_sampler, is_unsized_array) = {
                let t = self.ctx.pool.ty(arg_type);
                (t.is_sampler(), t.is_unsized_array())
            };
            if !substitute_argument[index].is_empty() {
                // We need to take the address of the variable and pass it down as a pointer.
                expr += &format!("&{}", substitute_argument[index]);
            } else if is_sampler {
                // If the argument is a sampler, we need to pass the texture _and_ its associated
                // sampler. (Function parameter lists also convert sampler parameters into a
                // matching texture/sampler parameter pair.)
                expr += &self.assemble_expression(arg, Precedence::Sequence, AssembleMode::Auto);
                expr += TEXTURE_SUFFIX;
                expr += ", ";
                expr += &self.assemble_expression(arg, Precedence::Sequence, AssembleMode::Auto);
                expr += SAMPLER_SUFFIX;
            } else if is_unsized_array {
                // If the array is in the parameter storage space then manually just pass it
                // through since it is already a pointer.
                if let ExpressionKind::VariableReference(r) = &self.ctx.pool.expression(arg).kind {
                    // A variable reference to an unsized array should always be a parameter,
                    // because unsized arrays coming from uniforms will have the `FieldAccess`
                    // expression type.
                    let variable = self.ctx.pool.variable(r.variable);
                    debug_assert!(variable.storage == VariableStorage::Parameter);
                    let mangled = variable.mangled_name().to_owned();
                    expr += &self.assemble_name(&mangled);
                } else {
                    expr += &format!(
                        "&({})",
                        self.assemble_expression(arg, Precedence::Sequence, AssembleMode::Auto)
                    );
                }
            } else {
                expr += &self.assemble_expression(arg, Precedence::Sequence, AssembleMode::Auto);
            }
        }
        expr.push(')');

        // If we have to write substitutions after the function call, we have to force the
        // function call into a scratch let so that variable can be returned as the expression.
        let result = self.emit_call(call.ty, parent_precedence, &expr, needs_writeback);

        if needs_writeback {
            for index in 0..args.len() {
                if !substitute_argument[index].is_empty() {
                    let store = writeback[index]
                        .as_ref()
                        .expect("a substituted argument has an lvalue")
                        .store(self.ctx, &substitute_argument[index]);
                    self.write_line(&store);
                }
            }
        }

        // Return the result of invoking the function.
        result
    }

    /// The `emitCall` lambda of `assembleFunctionCall`: wraps user and intrinsic function calls
    /// that might have side effects.
    fn emit_call(
        &mut self,
        call_type: TypeId,
        parent_precedence: Precedence,
        expr: &str,
        force_let: bool,
    ) -> String {
        if self.ctx.pool.ty(call_type).is_void() || parent_precedence >= Precedence::Statement {
            // If this is true, the assembled expression won't be used directly by the caller, so
            // we need to write it out as its own statement to preserve any side effects.
            debug_assert!(parent_precedence >= Precedence::Sequence);
            self.write(expr);
            self.write_line(";");
            String::new() // Will be unused
        } else if force_let {
            // The function call is used by the caller, but we need to write the actual call *now*
            // before additional statements.
            self.write_scratch_let(expr, false)
        } else {
            // Can inline the call's expression into the caller's expression
            expr.to_owned()
        }
    }

    /// `makeLValue(e)`: synthesizes an `LValue` for an expression.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L2685-L2704 (chrome/m156)
    pub(super) fn make_lvalue(&mut self, e: crate::ir::ExprId) -> Option<LValue> {
        let expr = self.ctx.pool.expression(e).clone();
        match &expr.kind {
            ExpressionKind::VariableReference(r) => Some(LValue::Pointer(
                self.variable_reference_name_for_lvalue(r.variable),
            )),
            ExpressionKind::FieldAccess(f) => Some(LValue::Pointer(self.assemble_field_access(f))),
            ExpressionKind::Index(idx) => {
                let base_type = self.expr_type(idx.base);
                if self.ctx.pool.ty(base_type).is_vector() {
                    // Rewrite indexed-swizzle accesses like `myVec.zyx[i]` into an index onto
                    // `myVec`.
                    if let Some(rewrite) = rewrite_indexed_swizzle(self.ctx, e) {
                        Some(LValue::VectorComponent(self.assemble_expression(
                            rewrite,
                            Precedence::Assignment,
                            AssembleMode::Auto,
                        )))
                    } else {
                        Some(LValue::VectorComponent(
                            self.assemble_index_expression(idx.base, idx.index),
                        ))
                    }
                } else {
                    Some(LValue::Pointer(
                        self.assemble_index_expression(idx.base, idx.index),
                    ))
                }
            }
            ExpressionKind::Swizzle(swizzle) => {
                if swizzle.components.len() == 1 {
                    Some(LValue::VectorComponent(self.assemble_swizzle(
                        swizzle.base,
                        swizzle.components.as_slice(),
                    )))
                } else {
                    let name = self.assemble_expression(
                        swizzle.base,
                        Precedence::Assignment,
                        AssembleMode::Auto,
                    );
                    let base_type = self.expr_type(swizzle.base);
                    let components: &ComponentArray = &swizzle.components;
                    Some(LValue::Swizzle(SwizzleLValue::new(
                        self.ctx,
                        name,
                        base_type,
                        components.as_slice(),
                    )))
                }
            }
            _ => {
                self.ctx
                    .errors
                    .error(expr.position, "unsupported lvalue type");
                None
            }
        }
    }
}
