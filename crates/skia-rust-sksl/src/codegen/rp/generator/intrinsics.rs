// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp (`Generator`'s
// `pushIntrinsic` members).

//! Intrinsic calls: the `pushIntrinsic` overloads and the op selection behind them.
//!
//! Skia overloads `pushIntrinsic` on (`IntrinsicKind`, args), (`TypedOps`, args) and
//! (`BuilderOp`, args). The Rust names say which: `push_intrinsic_kind{1,2,3}` for the first,
//! `push_intrinsic_typed{1,2}` and `push_intrinsic_op{1,2}` for the others.

// Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3040-L3500 (chrome/m156)

use super::{Generator, Res, TypedOps, unsupported};
use crate::codegen::rp::ops::BuilderOp;
use crate::intrinsic_list::IntrinsicKind;
use crate::ir::{ExprId, FunctionCall, TypeId};
use crate::operator::OperatorKind;
use crate::position::Position;

/// `FLT_MAX`.
const FLT_MAX: f64 = f32::MAX as f64;

impl Generator<'_> {
    /// `pushIntrinsic(const FunctionCall&)`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3040-L3057 (chrome/m156)
    pub(super) fn push_intrinsic_call(&mut self, c: &FunctionCall) -> Res {
        let kind = self
            .ctx
            .pool
            .function(c.function)
            .intrinsic_kind
            .expect("an intrinsic function has an intrinsic kind");
        match c.arguments.len() {
            1 => self.push_intrinsic_kind1(kind, c.arguments[0]),
            2 => self.push_intrinsic_kind2(kind, c.arguments[0], c.arguments[1]),
            3 => self.push_intrinsic_kind3(kind, c.arguments[0], c.arguments[1], c.arguments[2]),
            _ => unsupported(),
        }
    }

    /// `pushLengthIntrinsic`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3059-L3069 (chrome/m156)
    fn push_length_intrinsic(&mut self, slot_count: i32) -> Res {
        if slot_count == 1 {
            // `length(scalar)` is `sqrt(x^2)`, which is equivalent to `abs(x)`.
            return self.push_abs_float_intrinsic(1);
        }
        // Implement `length(vec)` as `sqrt(dot(x, x))`.
        self.builder.push_clone(slot_count, 0);
        self.builder.dot_floats(slot_count);
        self.builder.unary_op(BuilderOp::SqrtFloat, 1);
        Ok(())
    }

    /// `pushAbsFloatIntrinsic`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3071-L3076 (chrome/m156)
    fn push_abs_float_intrinsic(&mut self, slots: i32) -> Res {
        // Perform abs(float) by masking off the sign bit.
        self.builder.push_constant_u(0x7FFF_FFFF, slots);
        self.builder.binary_op(BuilderOp::BitwiseAndNInts, slots);
        Ok(())
    }

    /// `pushIntrinsic(const TypedOps&, arg0)`.
    fn push_intrinsic_typed1(&mut self, ops: TypedOps, arg0: ExprId) -> Res {
        self.push_expression(arg0, true)?;
        let ty = self.expr_type(arg0);
        self.unary_op(ty, ops)
    }

    /// `pushIntrinsic(BuilderOp, arg0)`.
    fn push_intrinsic_op1(&mut self, builder_op: BuilderOp, arg0: ExprId) -> Res {
        self.push_expression(arg0, true)?;
        let slots = self.expr_slots(arg0);
        self.builder.unary_op(builder_op, slots);
        Ok(())
    }

    /// `pushIntrinsic(IntrinsicKind, arg0)`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3097-L3287 (chrome/m156)
    fn push_intrinsic_kind1(&mut self, intrinsic: IntrinsicKind, arg0: ExprId) -> Res {
        let arg0_type = self.expr_type(arg0);
        let arg0_slots = self.type_slots(arg0_type);
        let arg0_component = self.ctx.pool.ty(arg0_type).component_type().id();
        match intrinsic {
            IntrinsicKind::Abs => {
                if self.ctx.pool.ty(arg0_component).is_float() {
                    // Perform abs(float) by masking off the sign bit.
                    self.push_expression(arg0, true)?;
                    return self.push_abs_float_intrinsic(arg0_slots);
                }
                // We have a dedicated op for abs(int).
                self.push_intrinsic_op1(BuilderOp::AbsInt, arg0)
            }

            IntrinsicKind::Any => {
                self.push_expression(arg0, true)?;
                self.fold_with_multi_op(BuilderOp::BitwiseOrNInts, arg0_slots);
                Ok(())
            }

            IntrinsicKind::All => {
                self.push_expression(arg0, true)?;
                self.fold_with_multi_op(BuilderOp::BitwiseAndNInts, arg0_slots);
                Ok(())
            }

            IntrinsicKind::Acos => self.push_intrinsic_op1(BuilderOp::AcosFloat, arg0),

            IntrinsicKind::Asin => self.push_intrinsic_op1(BuilderOp::AsinFloat, arg0),

            IntrinsicKind::Atan => self.push_intrinsic_op1(BuilderOp::AtanFloat, arg0),

            IntrinsicKind::Ceil => self.push_intrinsic_op1(BuilderOp::CeilFloat, arg0),

            IntrinsicKind::Cos => self.push_intrinsic_op1(BuilderOp::CosFloat, arg0),

            IntrinsicKind::Degrees => {
                let lit_180_over_pi = self.make_literal(
                    Position::default(),
                    f64::from(57.295_779_513_1_f32),
                    arg0_component,
                );
                self.push_binary_expression(arg0, OperatorKind::Star.into(), lit_180_over_pi)
            }

            IntrinsicKind::FloatBitsToInt
            | IntrinsicKind::FloatBitsToUint
            | IntrinsicKind::IntBitsToFloat
            | IntrinsicKind::UintBitsToFloat => self.push_expression(arg0, true),

            IntrinsicKind::Exp => self.push_intrinsic_op1(BuilderOp::ExpFloat, arg0),

            IntrinsicKind::Exp2 => self.push_intrinsic_op1(BuilderOp::Exp2Float, arg0),

            IntrinsicKind::Floor => self.push_intrinsic_op1(BuilderOp::FloorFloat, arg0),

            IntrinsicKind::Fract => {
                // Implement fract as `x - floor(x)`.
                self.push_expression(arg0, true)?;
                self.builder.push_clone(arg0_slots, 0);
                self.builder.unary_op(BuilderOp::FloorFloat, arg0_slots);
                self.binary_op(arg0_type, TypedOps::SUBTRACT)
            }

            IntrinsicKind::Inverse => {
                debug_assert!(self.ctx.pool.ty(arg0_type).is_matrix());
                debug_assert_eq!(
                    self.ctx.pool.ty(arg0_type).rows(),
                    self.ctx.pool.ty(arg0_type).columns()
                );
                self.push_expression(arg0, true)?;
                let rows = self.ctx.pool.ty(arg0_type).rows();
                self.builder.inverse_matrix(rows);
                Ok(())
            }

            IntrinsicKind::Inversesqrt => self.push_intrinsic_typed1(TypedOps::INVERSE_SQRT, arg0),

            IntrinsicKind::Length => {
                self.push_expression(arg0, true)?;
                self.push_length_intrinsic(arg0_slots)
            }

            IntrinsicKind::Log => {
                self.push_expression(arg0, true)?;
                self.builder.unary_op(BuilderOp::LogFloat, arg0_slots);
                Ok(())
            }

            IntrinsicKind::Log2 => {
                self.push_expression(arg0, true)?;
                self.builder.unary_op(BuilderOp::Log2Float, arg0_slots);
                Ok(())
            }

            IntrinsicKind::Normalize => {
                // Implement normalize as `x / length(x)`. First, push the expression.
                self.push_expression(arg0, true)?;
                let slot_count = arg0_slots;
                if slot_count > 1 {
                    // TODO (upstream): We can get roughly the same result in less time by using
                    // `invsqrt`, but that leads to more variance across architectures, which
                    // Chromium layout tests do not handle nicely. (`SK_USE_RSQRT_IN_RP_NORMALIZE`
                    // is not defined.)
                    self.builder.push_clone(slot_count, 0);
                    self.builder.push_clone(slot_count, 0);
                    self.builder.dot_floats(slot_count);

                    // Compute `vec(sqrt(dot(x, x)))`.
                    self.builder.unary_op(BuilderOp::SqrtFloat, 1);
                    self.builder.push_duplicates(slot_count - 1);

                    // Return `x / vec(sqrt(dot(x, x)))`.
                    self.binary_op(arg0_type, TypedOps::DIVIDE)
                } else {
                    // For single-slot normalization, we can simplify `sqrt(x * x)` into `abs(x)`.
                    self.builder.push_clone(slot_count, 0);
                    self.push_abs_float_intrinsic(1)?;
                    self.binary_op(arg0_type, TypedOps::DIVIDE)
                }
            }

            IntrinsicKind::Not => {
                self.push_prefix_expression(OperatorKind::LogicalNot.into(), arg0)
            }

            IntrinsicKind::Radians => {
                let lit_pi_over_180 = self.make_literal(
                    Position::default(),
                    f64::from(0.017_453_292_51_f32),
                    arg0_component,
                );
                self.push_binary_expression(arg0, OperatorKind::Star.into(), lit_pi_over_180)
            }

            IntrinsicKind::Saturate => {
                // Implement saturate as clamp(arg, 0, 1).
                let zero_literal = self.make_literal(Position::default(), 0.0, arg0_component);
                let one_literal = self.make_literal(Position::default(), 1.0, arg0_component);
                self.push_intrinsic_kind3(IntrinsicKind::Clamp, arg0, zero_literal, one_literal)
            }

            IntrinsicKind::Sign => {
                // Implement floating-point sign() as `clamp(arg * FLT_MAX, -1, 1)`.
                // FLT_MIN * FLT_MAX evaluates to 4, so multiplying any float value against
                // FLT_MAX is sufficient to ensure that |value| is always 1 or greater (excluding
                // zero and nan). Integer sign() doesn't need to worry about fractional values or
                // nans, and can simply be `clamp(arg, -1, 1)`.
                self.push_expression(arg0, true)?;
                if self.ctx.pool.ty(arg0_component).is_float() {
                    let flt_max_literal =
                        self.make_literal(Position::default(), FLT_MAX, arg0_component);
                    self.push_vectorized_expression(flt_max_literal, arg0_type)?;
                    self.binary_op(arg0_type, TypedOps::MULTIPLY)?;
                }
                let neg1_literal = self.make_literal(Position::default(), -1.0, arg0_component);
                self.push_vectorized_expression(neg1_literal, arg0_type)?;
                self.binary_op(arg0_type, TypedOps::MAX)?;
                let pos1_literal = self.make_literal(Position::default(), 1.0, arg0_component);
                self.push_vectorized_expression(pos1_literal, arg0_type)?;
                self.binary_op(arg0_type, TypedOps::MIN)
            }

            IntrinsicKind::Sin => self.push_intrinsic_op1(BuilderOp::SinFloat, arg0),

            IntrinsicKind::Sqrt => self.push_intrinsic_op1(BuilderOp::SqrtFloat, arg0),

            IntrinsicKind::Tan => self.push_intrinsic_op1(BuilderOp::TanFloat, arg0),

            IntrinsicKind::Transpose => {
                debug_assert!(self.ctx.pool.ty(arg0_type).is_matrix());
                self.push_expression(arg0, true)?;
                let (columns, rows) = {
                    let t = self.ctx.pool.ty(arg0_type);
                    (t.columns(), t.rows())
                };
                self.builder.transpose(columns, rows);
                Ok(())
            }

            IntrinsicKind::Trunc => {
                // Implement trunc as `float(int(x))`, since float-to-int rounds toward zero.
                self.push_expression(arg0, true)?;
                self.builder
                    .unary_op(BuilderOp::CastToIntFromFloat, arg0_slots);
                self.builder
                    .unary_op(BuilderOp::CastToFloatFromInt, arg0_slots);
                Ok(())
            }

            IntrinsicKind::FromLinearSrgb | IntrinsicKind::ToLinearSrgb => {
                // The argument must be a half3.
                debug_assert!(self.ctx.pool.ty(arg0_type).matches(TypeId::HALF3));
                self.push_expression(arg0, true)?;

                if intrinsic == IntrinsicKind::FromLinearSrgb {
                    self.builder.invoke_from_linear_srgb();
                } else {
                    self.builder.invoke_to_linear_srgb();
                }
                Ok(())
            }

            _ => unsupported(),
        }
    }

    /// `pushIntrinsic(const TypedOps&, arg0, arg1)`.
    fn push_intrinsic_typed2(&mut self, ops: TypedOps, arg0: ExprId, arg1: ExprId) -> Res {
        self.push_expression(arg0, true)?;
        let arg0_type = self.expr_type(arg0);
        self.push_vectorized_expression(arg1, arg0_type)?;
        self.binary_op(arg0_type, ops)
    }

    /// `pushIntrinsic(BuilderOp, arg0, arg1)`.
    fn push_intrinsic_op2(&mut self, builder_op: BuilderOp, arg0: ExprId, arg1: ExprId) -> Res {
        self.push_expression(arg0, true)?;
        let arg0_type = self.expr_type(arg0);
        self.push_vectorized_expression(arg1, arg0_type)?;
        let slots = self.type_slots(arg0_type);
        self.builder.binary_op(builder_op, slots);
        Ok(())
    }

    /// `pushIntrinsic(IntrinsicKind, arg0, arg1)`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3326-L3495 (chrome/m156)
    fn push_intrinsic_kind2(
        &mut self,
        intrinsic: IntrinsicKind,
        arg0: ExprId,
        arg1: ExprId,
    ) -> Res {
        let arg0_type = self.expr_type(arg0);
        let arg1_type = self.expr_type(arg1);
        match intrinsic {
            IntrinsicKind::Atan => self.push_intrinsic_op2(BuilderOp::Atan2NFloats, arg0, arg1),

            IntrinsicKind::Cross => {
                // Implement cross as `arg0.yzx * arg1.zxy - arg0.zxy * arg1.yzx`. We use two
                // stacks so that each subexpression can be multiplied separately.
                debug_assert!(self.ctx.pool.ty(arg0_type).matches(arg1_type));
                debug_assert_eq!(self.type_slots(arg0_type), 3);
                debug_assert_eq!(self.type_slots(arg1_type), 3);

                // Push `arg0.yzx` onto this stack and `arg0.zxy` onto a separate subexpression
                // stack.
                let subexpression_stack = self.new_auto_stack();
                self.stack_enter(subexpression_stack);
                self.push_expression(arg0, true)?;
                self.stack_exit(subexpression_stack);
                self.stack_push_clone(subexpression_stack, 3);

                self.builder.swizzle(3, &[1, 2, 0]);
                self.stack_enter(subexpression_stack);
                self.builder.swizzle(3, &[2, 0, 1]);
                self.stack_exit(subexpression_stack);

                // Push `arg1.zxy` onto this stack and `arg1.yzx` onto the next stack. Perform the
                // multiply on each subexpression (`arg0.yzx * arg1.zxy` on the first stack, and
                // `arg0.zxy * arg1.yzx` on the next).
                self.stack_enter(subexpression_stack);
                self.push_expression(arg1, true)?;
                self.stack_exit(subexpression_stack);
                self.stack_push_clone(subexpression_stack, 3);

                self.builder.swizzle(3, &[2, 0, 1]);
                self.builder.binary_op(BuilderOp::MulNFloats, 3);

                self.stack_enter(subexpression_stack);
                self.builder.swizzle(3, &[1, 2, 0]);
                self.builder.binary_op(BuilderOp::MulNFloats, 3);
                self.stack_exit(subexpression_stack);

                // Migrate the result of the second subexpression (`arg0.zxy * arg1.yzx`) back
                // onto the main stack and subtract it from the first subexpression
                // (`arg0.yzx * arg1.zxy`).
                self.stack_push_clone(subexpression_stack, 3);
                self.builder.binary_op(BuilderOp::SubNFloats, 3);

                // Now that the calculation is complete, discard the subexpression on the next
                // stack.
                self.stack_enter(subexpression_stack);
                self.discard_expression(3);
                self.stack_exit(subexpression_stack);
                self.drop_auto_stack(subexpression_stack);
                Ok(())
            }

            IntrinsicKind::Distance => {
                // Implement distance as `length(a - b)`.
                debug_assert_eq!(self.type_slots(arg0_type), self.type_slots(arg1_type));
                self.push_binary_expression(arg0, OperatorKind::Minus.into(), arg1)?;
                let slots = self.type_slots(arg0_type);
                self.push_length_intrinsic(slots)
            }

            IntrinsicKind::Dot => {
                debug_assert!(self.ctx.pool.ty(arg0_type).matches(arg1_type));
                self.push_expression(arg0, true)?;
                self.push_expression(arg1, true)?;
                let slots = self.type_slots(arg0_type);
                self.builder.dot_floats(slots);
                Ok(())
            }

            IntrinsicKind::Equal => {
                debug_assert!(self.ctx.pool.ty(arg0_type).matches(arg1_type));
                self.push_intrinsic_typed2(TypedOps::EQUAL, arg0, arg1)
            }

            IntrinsicKind::NotEqual => {
                debug_assert!(self.ctx.pool.ty(arg0_type).matches(arg1_type));
                self.push_intrinsic_typed2(TypedOps::NOT_EQUAL, arg0, arg1)
            }

            IntrinsicKind::LessThan => {
                debug_assert!(self.ctx.pool.ty(arg0_type).matches(arg1_type));
                self.push_intrinsic_typed2(TypedOps::LESS_THAN, arg0, arg1)
            }

            IntrinsicKind::GreaterThan => {
                debug_assert!(self.ctx.pool.ty(arg0_type).matches(arg1_type));
                self.push_intrinsic_typed2(TypedOps::LESS_THAN, arg1, arg0)
            }

            IntrinsicKind::LessThanEqual => {
                debug_assert!(self.ctx.pool.ty(arg0_type).matches(arg1_type));
                self.push_intrinsic_typed2(TypedOps::LESS_THAN_EQUAL, arg0, arg1)
            }

            IntrinsicKind::GreaterThanEqual => {
                debug_assert!(self.ctx.pool.ty(arg0_type).matches(arg1_type));
                self.push_intrinsic_typed2(TypedOps::LESS_THAN_EQUAL, arg1, arg0)
            }

            IntrinsicKind::Min => self.push_intrinsic_typed2(TypedOps::MIN, arg0, arg1),

            IntrinsicKind::MatrixCompMult => {
                debug_assert!(self.ctx.pool.ty(arg0_type).matches(arg1_type));
                self.push_intrinsic_typed2(TypedOps::MULTIPLY, arg0, arg1)
            }

            IntrinsicKind::Max => self.push_intrinsic_typed2(TypedOps::MAX, arg0, arg1),

            IntrinsicKind::Mod => self.push_intrinsic_typed2(TypedOps::MOD, arg0, arg1),

            IntrinsicKind::Pow => {
                debug_assert!(self.ctx.pool.ty(arg0_type).matches(arg1_type));
                self.push_intrinsic_op2(BuilderOp::PowNFloats, arg0, arg1)
            }

            IntrinsicKind::Reflect => {
                // Implement reflect as `I - (N * dot(I,N) * 2)`.
                debug_assert!(self.ctx.pool.ty(arg0_type).matches(arg1_type));
                debug_assert_eq!(self.type_slots(arg0_type), self.type_slots(arg1_type));
                let slot_count = self.type_slots(arg0_type);

                // Stack: I, N.
                self.push_expression(arg0, true)?;
                self.push_expression(arg1, true)?;
                // Stack: I, N, I, N.
                self.builder.push_clone(2 * slot_count, 0);
                // Stack: I, N, dot(I,N)
                self.builder.dot_floats(slot_count);
                // Stack: I, N, dot(I,N), 2
                self.builder.push_constant_f(2.0);
                // Stack: I, N, dot(I,N) * 2
                self.builder.binary_op(BuilderOp::MulNFloats, 1);
                // Stack: I, N * dot(I,N) * 2
                self.builder.push_duplicates(slot_count - 1);
                self.builder.binary_op(BuilderOp::MulNFloats, slot_count);
                // Stack: I - (N * dot(I,N) * 2)
                self.builder.binary_op(BuilderOp::SubNFloats, slot_count);
                Ok(())
            }

            IntrinsicKind::Step => {
                // Compute step as `float(lessThanEqual(edge, x))`. We convert from boolean 0/~0
                // to floating point zero/one by using a bitwise-and against the bit-pattern of
                // 1.0.
                self.push_vectorized_expression(arg0, arg1_type)?;
                self.push_expression(arg1, true)?;
                self.binary_op(arg1_type, TypedOps::LESS_THAN_EQUAL)?;
                let component = self.ctx.pool.ty(arg1_type).component_type().id();
                let pos1_literal = self.make_literal(Position::default(), 1.0, component);
                self.push_vectorized_expression(pos1_literal, arg1_type)?;
                let slots = self.type_slots(arg1_type);
                self.builder.binary_op(BuilderOp::BitwiseAndNInts, slots);
                Ok(())
            }

            _ => unsupported(),
        }
    }

    /// `pushIntrinsic(IntrinsicKind, arg0, arg1, arg2)`.
    // Port of: src/sksl/codegen/SkSLRasterPipelineCodeGenerator.cpp#L3497-L3612 (chrome/m156)
    fn push_intrinsic_kind3(
        &mut self,
        intrinsic: IntrinsicKind,
        arg0: ExprId,
        arg1: ExprId,
        arg2: ExprId,
    ) -> Res {
        let arg0_type = self.expr_type(arg0);
        let arg2_type = self.expr_type(arg2);
        match intrinsic {
            IntrinsicKind::Clamp => {
                // Implement clamp as min(max(arg, low), high).
                self.push_expression(arg0, true)?;
                self.push_vectorized_expression(arg1, arg0_type)?;
                self.binary_op(arg0_type, TypedOps::MAX)?;
                self.push_vectorized_expression(arg2, arg0_type)?;
                self.binary_op(arg0_type, TypedOps::MIN)
            }

            IntrinsicKind::Faceforward => {
                // Implement faceforward as `N ^ ((0 <= dot(I, NRef)) & 0x80000000)`.
                // In other words, flip the sign bit of N if `0 <= dot(I, NRef)`.
                debug_assert!(self.ctx.pool.ty(arg0_type).matches(self.expr_type(arg1)));
                debug_assert!(self.ctx.pool.ty(arg0_type).matches(arg2_type));
                let slot_count = self.type_slots(arg0_type);

                // Stack: N, 0, I, Nref
                self.push_expression(arg0, true)?;
                self.builder.push_constant_f(0.0);
                self.push_expression(arg1, true)?;
                self.push_expression(arg2, true)?;
                // Stack: N, 0, dot(I,NRef)
                self.builder.dot_floats(slot_count);
                // Stack: N, (0 <= dot(I,NRef))
                self.builder.binary_op(BuilderOp::CmpleNFloats, 1);
                // Stack: N, (0 <= dot(I,NRef)), 0x80000000
                self.builder.push_constant_u(0x8000_0000, 1);
                // Stack: N, (0 <= dot(I,NRef)) & 0x80000000)
                self.builder.binary_op(BuilderOp::BitwiseAndNInts, 1);
                // Stack: N, vec(0 <= dot(I,NRef)) & 0x80000000)
                self.builder.push_duplicates(slot_count - 1);
                // Stack: N ^ vec((0 <= dot(I,NRef)) & 0x80000000)
                self.builder
                    .binary_op(BuilderOp::BitwiseXorNInts, slot_count);
                Ok(())
            }

            IntrinsicKind::Mix => {
                // Note: our SkRP mix op takes the interpolation point first, not the
                // interpolants.
                debug_assert!(self.ctx.pool.ty(arg0_type).matches(self.expr_type(arg1)));
                let arg2_component = self.ctx.pool.ty(arg2_type).component_type().id();
                if self.ctx.pool.ty(arg2_component).is_float() {
                    self.push_vectorized_expression(arg2, arg0_type)?;
                    self.push_expression(arg0, true)?;
                    self.push_expression(arg1, true)?;
                    return self.ternary_op(arg0_type, TypedOps::MIX);
                }
                if self.ctx.pool.ty(arg2_component).is_boolean() {
                    self.push_expression(arg2, true)?;
                    self.push_expression(arg0, true)?;
                    self.push_expression(arg1, true)?;
                    // The `mix_int` op isn't doing a lerp; it uses the third argument to select
                    // values from the first and second arguments. It's safe for use with any type
                    // in arguments 0 and 1.
                    let slots = self.type_slots(arg0_type);
                    self.builder.ternary_op(BuilderOp::MixNInts, slots);
                    return Ok(());
                }
                unsupported()
            }

            IntrinsicKind::Refract => {
                // We always calculate refraction using vec4s, so we pad out unused N/I slots with
                // zero.
                let padding = 4 - self.type_slots(arg0_type);
                self.push_expression(arg0, true)?;
                self.builder.push_zeros(padding);

                self.push_expression(arg1, true)?;
                self.builder.push_zeros(padding);

                // eta is always a scalar and doesn't need padding.
                self.push_expression(arg2, true)?;
                self.builder.refract_floats();

                // The result vector was returned as a vec4, so discard the extra columns.
                self.builder.discard_stack(padding);
                Ok(())
            }

            IntrinsicKind::Smoothstep => {
                self.push_vectorized_expression(arg0, arg2_type)?;
                self.push_vectorized_expression(arg1, arg2_type)?;
                self.push_expression(arg2, true)?;
                let slots = self.type_slots(arg2_type);
                self.builder.ternary_op(BuilderOp::SmoothstepNFloats, slots);
                Ok(())
            }

            _ => unsupported(),
        }
    }
}
