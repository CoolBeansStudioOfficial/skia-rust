// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLOperator.{h,cpp}. `determineBinaryType` and `isMatrixMultiply`
// need `Type` and come with the type system (task S6).

//! The `SkSL` operators: their kinds, precedence, printed spelling and the predicates that decide
//! which types they accept.

use crate::context::Context;
use crate::ir::{CoercionCost, IrPool, TypeId};

/// `SkSL::OperatorKind`: every operator, unary, binary, assignment and increment.
#[doc(alias = "SkSL::OperatorKind")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OperatorKind {
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Shl,
    Shr,
    LogicalNot,
    LogicalAnd,
    LogicalOr,
    LogicalXor,
    BitwiseNot,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Eq,
    EqEq,
    Neq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    ShlEq,
    ShrEq,
    BitwiseAndEq,
    BitwiseOrEq,
    BitwiseXorEq,
    PlusPlus,
    MinusMinus,
    Comma,
}

/// `SkSL::OperatorPrecedence`: lower numbers bind tighter. `Expression` is `Sequence`.
#[doc(alias = "SkSL::OperatorPrecedence")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum OperatorPrecedence {
    Parentheses = 1,
    Postfix = 2,
    Prefix = 3,
    Multiplicative = 4,
    Additive = 5,
    Shift = 6,
    Relational = 7,
    Equality = 8,
    BitwiseAnd = 9,
    BitwiseXor = 10,
    BitwiseOr = 11,
    LogicalAnd = 12,
    LogicalXor = 13,
    LogicalOr = 14,
    Ternary = 15,
    Assignment = 16,
    /// A comma-separated sequence.
    Sequence = 17,
    /// A standalone expression-statement.
    Statement = 18,
}

impl OperatorPrecedence {
    /// `kExpression`: a top-level expression, anywhere in a statement (the same as `Sequence`).
    pub const EXPRESSION: Self = Self::Sequence;
}

/// `SkSL::Operator`: one operator, by kind.
#[doc(alias = "SkSL::Operator")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Operator {
    kind: OperatorKind,
}

impl From<OperatorKind> for Operator {
    fn from(kind: OperatorKind) -> Self {
        Self { kind }
    }
}

impl Operator {
    /// `kind()`.
    #[must_use]
    pub fn kind(self) -> OperatorKind {
        self.kind
    }

    /// `isEquality`: `==` or `!=`.
    #[must_use]
    pub fn is_equality(self) -> bool {
        matches!(self.kind, OperatorKind::EqEq | OperatorKind::Neq)
    }

    /// `getBinaryPrecedence`.
    ///
    /// # Panics
    ///
    /// For an operator that is not binary (Skia aborts here).
    // Port of: src/sksl/SkSLOperator.cpp#L21-L56 (chrome/m156)
    #[must_use]
    pub fn binary_precedence(self) -> OperatorPrecedence {
        use OperatorKind as K;
        use OperatorPrecedence as P;
        match self.kind {
            K::Star | K::Slash | K::Percent => P::Multiplicative,
            K::Plus | K::Minus => P::Additive,
            K::Shl | K::Shr => P::Shift,
            K::Lt | K::Gt | K::LtEq | K::GtEq => P::Relational,
            K::EqEq | K::Neq => P::Equality,
            K::BitwiseAnd => P::BitwiseAnd,
            K::BitwiseXor => P::BitwiseXor,
            K::BitwiseOr => P::BitwiseOr,
            K::LogicalAnd => P::LogicalAnd,
            K::LogicalXor => P::LogicalXor,
            K::LogicalOr => P::LogicalOr,
            K::Eq
            | K::PlusEq
            | K::MinusEq
            | K::StarEq
            | K::SlashEq
            | K::PercentEq
            | K::ShlEq
            | K::ShrEq
            | K::BitwiseAndEq
            | K::BitwiseXorEq
            | K::BitwiseOrEq => P::Assignment,
            K::Comma => P::Sequence,
            _ => panic!("unsupported binary operator {:?}", self.kind),
        }
    }

    /// `operatorName`: the spelling with the spaces Skia prints around it.
    // Port of: src/sksl/SkSLOperator.cpp#L58-L108 (chrome/m156)
    #[must_use]
    pub fn operator_name(self) -> &'static str {
        use OperatorKind as K;
        match self.kind {
            K::Plus => " + ",
            K::Minus => " - ",
            K::Star => " * ",
            K::Slash => " / ",
            K::Percent => " % ",
            K::Shl => " << ",
            K::Shr => " >> ",
            K::LogicalNot => "!",
            K::LogicalAnd => " && ",
            K::LogicalOr => " || ",
            K::LogicalXor => " ^^ ",
            K::BitwiseNot => "~",
            K::BitwiseAnd => " & ",
            K::BitwiseOr => " | ",
            K::BitwiseXor => " ^ ",
            K::Eq => " = ",
            K::EqEq => " == ",
            K::Neq => " != ",
            K::Lt => " < ",
            K::Gt => " > ",
            K::LtEq => " <= ",
            K::GtEq => " >= ",
            K::PlusEq => " += ",
            K::MinusEq => " -= ",
            K::StarEq => " *= ",
            K::SlashEq => " /= ",
            K::PercentEq => " %= ",
            K::ShlEq => " <<= ",
            K::ShrEq => " >>= ",
            K::BitwiseAndEq => " &= ",
            K::BitwiseOrEq => " |= ",
            K::BitwiseXorEq => " ^= ",
            K::PlusPlus => "++",
            K::MinusMinus => "--",
            K::Comma => ", ",
        }
    }

    /// `tightOperatorName`: the spelling without its outer spaces.
    #[must_use]
    pub fn tight_operator_name(self) -> &'static str {
        let name = self.operator_name();
        let name = name.strip_prefix(' ').unwrap_or(name);
        name.strip_suffix(' ').unwrap_or(name)
    }

    /// `isAssignment`: `=` and every compound assignment, but not `++` or `--`.
    #[must_use]
    pub fn is_assignment(self) -> bool {
        use OperatorKind as K;
        matches!(
            self.kind,
            K::Eq
                | K::PlusEq
                | K::MinusEq
                | K::StarEq
                | K::SlashEq
                | K::PercentEq
                | K::ShlEq
                | K::ShrEq
                | K::BitwiseOrEq
                | K::BitwiseXorEq
                | K::BitwiseAndEq
        )
    }

    /// `isCompoundAssignment`: an assignment other than `=`.
    #[must_use]
    pub fn is_compound_assignment(self) -> bool {
        self.is_assignment() && self.kind != OperatorKind::Eq
    }

    /// `removeAssignment`: the binary operator a compound assignment applies (`+=` gives `+`).
    /// Other operators are returned unchanged.
    #[must_use]
    pub fn remove_assignment(self) -> Self {
        use OperatorKind as K;
        let kind = match self.kind {
            K::PlusEq => K::Plus,
            K::MinusEq => K::Minus,
            K::StarEq => K::Star,
            K::SlashEq => K::Slash,
            K::PercentEq => K::Percent,
            K::ShlEq => K::Shl,
            K::ShrEq => K::Shr,
            K::BitwiseOrEq => K::BitwiseOr,
            K::BitwiseXorEq => K::BitwiseXor,
            K::BitwiseAndEq => K::BitwiseAnd,
            other => other,
        };
        Self { kind }
    }

    /// `isRelational`: `<`, `<=`, `>` and `>=`.
    #[must_use]
    pub fn is_relational(self) -> bool {
        matches!(
            self.kind,
            OperatorKind::Lt | OperatorKind::Gt | OperatorKind::LtEq | OperatorKind::GtEq
        )
    }

    /// `isOnlyValidForIntegralTypes`: shifts, bitwise operators and `%`, with their assignments.
    #[must_use]
    pub fn is_only_valid_for_integral_types(self) -> bool {
        use OperatorKind as K;
        matches!(
            self.kind,
            K::Shl
                | K::Shr
                | K::BitwiseAnd
                | K::BitwiseOr
                | K::BitwiseXor
                | K::Percent
                | K::ShlEq
                | K::ShrEq
                | K::BitwiseAndEq
                | K::BitwiseOrEq
                | K::BitwiseXorEq
                | K::PercentEq
        )
    }

    /// `isValidForMatrixOrVector`: the arithmetic and bitwise operators, with their assignments
    /// (not the comparisons, logical operators or increments).
    #[must_use]
    pub fn is_valid_for_matrix_or_vector(self) -> bool {
        use OperatorKind as K;
        matches!(
            self.kind,
            K::Plus
                | K::Minus
                | K::Star
                | K::Slash
                | K::Percent
                | K::Shl
                | K::Shr
                | K::BitwiseAnd
                | K::BitwiseOr
                | K::BitwiseXor
                | K::PlusEq
                | K::MinusEq
                | K::StarEq
                | K::SlashEq
                | K::PercentEq
                | K::ShlEq
                | K::ShrEq
                | K::BitwiseAndEq
                | K::BitwiseOrEq
                | K::BitwiseXorEq
        )
    }

    /// `isAllowedInStrictES2Mode`: every operator except the integral-only ones.
    #[must_use]
    pub fn is_allowed_in_strict_es2_mode(self) -> bool {
        !self.is_only_valid_for_integral_types()
    }
}

/// The types of a binary operation's operands and result: Skia's `outLeftType`, `outRightType`
/// and `outResultType` out-parameters of `Operator::determineBinaryType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BinaryTypes {
    /// `outLeftType`.
    pub left: TypeId,
    /// `outRightType`.
    pub right: TypeId,
    /// `outResultType`.
    pub result: TypeId,
}

impl Operator {
    /// `isMatrixMultiply(left, right)`: `*` with a matrix on the left, or a vector on the left and
    /// a matrix on the right.
    // Port of: src/sksl/SkSLOperator.cpp#L209-L217 (chrome/m156)
    #[must_use]
    pub fn is_matrix_multiply(self, pool: &IrPool, left: TypeId, right: TypeId) -> bool {
        if self.kind() != OperatorKind::Star && self.kind() != OperatorKind::StarEq {
            return false;
        }
        let (left, right) = (pool.ty(left), pool.ty(right));
        if left.is_matrix() {
            return right.is_matrix() || right.is_vector();
        }
        left.is_vector() && right.is_matrix()
    }

    /// `determineBinaryType(context, left, right, …)`: the operand and result types of this
    /// operator applied to `left` and `right`, or `None` when the operator does not accept them.
    // Port of: src/sksl/SkSLOperator.cpp#L219-L381 (chrome/m156)
    // Skia's `determineBinaryType` is one function of this length, and its cases run in its order.
    #[allow(clippy::too_many_lines)]
    #[must_use]
    pub fn determine_binary_type(
        self,
        ctx: &Context,
        left: TypeId,
        right: TypeId,
    ) -> Option<BinaryTypes> {
        let allow_narrowing = ctx.config().settings.allow_narrowing_conversions;
        let pool = &ctx.pool;
        let (lt, rt) = (pool.ty(left), pool.ty(right));
        match self.kind() {
            OperatorKind::Eq => {
                // left = right
                if lt.is_void() {
                    return None;
                }
                return rt
                    .can_coerce_to(left, allow_narrowing)
                    .then_some(BinaryTypes {
                        left,
                        right: left,
                        result: left,
                    });
            }
            OperatorKind::EqEq | OperatorKind::Neq => {
                // left == right, left != right
                if lt.is_void() || lt.is_opaque() {
                    return None;
                }
                let right_to_left = rt.coercion_cost(left);
                let left_to_right = lt.coercion_cost(right);
                if right_to_left < left_to_right {
                    if right_to_left.is_possible(allow_narrowing) {
                        return Some(BinaryTypes {
                            left,
                            right: left,
                            result: TypeId::BOOL,
                        });
                    }
                } else if left_to_right.is_possible(allow_narrowing) {
                    return Some(BinaryTypes {
                        left: right,
                        right,
                        result: TypeId::BOOL,
                    });
                }
                return None;
            }
            OperatorKind::LogicalOr | OperatorKind::LogicalAnd | OperatorKind::LogicalXor => {
                // left || right, left && right, left ^^ right
                let bool_ok = lt.can_coerce_to(TypeId::BOOL, allow_narrowing)
                    && rt.can_coerce_to(TypeId::BOOL, allow_narrowing);
                return bool_ok.then_some(BinaryTypes {
                    left: TypeId::BOOL,
                    right: TypeId::BOOL,
                    result: TypeId::BOOL,
                });
            }
            OperatorKind::Comma => {
                // left, right
                if lt.is_opaque() || rt.is_opaque() {
                    return None;
                }
                return Some(BinaryTypes {
                    left,
                    right,
                    result: right,
                });
            }
            _ => {}
        }

        // Boolean types only support the operators listed above.
        let left_component = lt.component_type();
        let right_component = rt.component_type();
        if left_component.is_boolean() || right_component.is_boolean() {
            return None;
        }

        let is_assignment = self.is_assignment();
        if self.is_matrix_multiply(pool, left, right) {
            // `left * right`: determine the final component type first.
            let inner =
                self.determine_binary_type(ctx, left_component.id(), right_component.id())?;
            // Convert the component type to a compound type.
            let scalar_result = pool.ty(inner.result);
            let out_left = scalar_result.to_compound(lt.columns(), lt.rows());
            let out_right = scalar_result.to_compound(rt.columns(), rt.rows());
            let (left_columns, left_rows) = (lt.columns(), lt.rows());
            let (mut right_columns, mut right_rows) = (rt.columns(), rt.rows());
            if rt.is_vector() {
                // `matrix * vector` treats the vector as a column vector: transpose it.
                std::mem::swap(&mut right_columns, &mut right_rows);
                debug_assert_eq!(right_columns, 1);
            }
            let result = if right_columns > 1 {
                scalar_result.to_compound(right_columns, left_rows)
            } else {
                // The result was a column vector. Transpose it back to a row.
                scalar_result.to_compound(left_rows, right_columns)
            };
            let result_ty = pool.ty(result);
            if is_assignment
                && (result_ty.columns() != left_columns || result_ty.rows() != left_rows)
            {
                return None;
            }
            if left_columns != right_rows {
                return None;
            }
            return Some(BinaryTypes {
                left: out_left,
                right: out_right,
                result,
            });
        }

        let left_is_vector_or_matrix = lt.is_vector() || lt.is_matrix();
        let valid_matrix_or_vector_op = self.is_valid_for_matrix_or_vector();
        if left_is_vector_or_matrix && valid_matrix_or_vector_op && rt.is_scalar() {
            // Determine the final component type, then convert it to a compound type.
            let inner = self.determine_binary_type(ctx, left_component.id(), right)?;
            let out_left = pool.ty(inner.left).to_compound(lt.columns(), lt.rows());
            let result = if self.is_relational() {
                inner.result
            } else {
                pool.ty(inner.result).to_compound(lt.columns(), lt.rows())
            };
            return Some(BinaryTypes {
                left: out_left,
                right: inner.right,
                result,
            });
        }

        let right_is_vector_or_matrix = rt.is_vector() || rt.is_matrix();
        if !is_assignment
            && right_is_vector_or_matrix
            && valid_matrix_or_vector_op
            && lt.is_scalar()
        {
            // Determine the final component type, then convert it to a compound type.
            let inner = self.determine_binary_type(ctx, left, right_component.id())?;
            let out_right = pool.ty(inner.right).to_compound(rt.columns(), rt.rows());
            let result = if self.is_relational() {
                inner.result
            } else {
                pool.ty(inner.result).to_compound(rt.columns(), rt.rows())
            };
            return Some(BinaryTypes {
                left: inner.left,
                right: out_right,
                result,
            });
        }

        let right_to_left_cost = rt.coercion_cost(left);
        let left_to_right_cost = if is_assignment {
            CoercionCost::impossible()
        } else {
            lt.coercion_cost(right)
        };
        if (lt.is_scalar() && rt.is_scalar())
            || (left_is_vector_or_matrix && valid_matrix_or_vector_op)
        {
            if self.is_only_valid_for_integral_types()
                && (!left_component.is_integer() || !right_component.is_integer())
            {
                return None;
            }
            let operand = if right_to_left_cost.is_possible(allow_narrowing)
                && right_to_left_cost < left_to_right_cost
            {
                // Right-to-left conversion is possible and cheaper.
                left
            } else if left_to_right_cost.is_possible(allow_narrowing) {
                // Left-to-right conversion is possible, and at least as cheap.
                right
            } else {
                return None;
            };
            let result = if self.is_relational() {
                TypeId::BOOL
            } else {
                operand
            };
            return Some(BinaryTypes {
                left: operand,
                right: operand,
                result,
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{Operator, OperatorKind as K, OperatorPrecedence as P};

    #[test]
    fn binary_precedence_table() {
        assert_eq!(
            Operator::from(K::Star).binary_precedence(),
            P::Multiplicative
        );
        assert_eq!(Operator::from(K::Minus).binary_precedence(), P::Additive);
        assert_eq!(Operator::from(K::Shl).binary_precedence(), P::Shift);
        assert_eq!(Operator::from(K::GtEq).binary_precedence(), P::Relational);
        assert_eq!(Operator::from(K::Neq).binary_precedence(), P::Equality);
        assert_eq!(
            Operator::from(K::LogicalOr).binary_precedence(),
            P::LogicalOr
        );
        assert_eq!(Operator::from(K::ShrEq).binary_precedence(), P::Assignment);
        assert_eq!(Operator::from(K::Comma).binary_precedence(), P::Sequence);
        assert_eq!(P::EXPRESSION, P::Sequence);
        assert!(P::Multiplicative < P::Additive);
    }

    #[test]
    #[should_panic(expected = "unsupported binary operator")]
    fn unary_operators_have_no_binary_precedence() {
        let _ = Operator::from(K::LogicalNot).binary_precedence();
    }

    #[test]
    fn spellings() {
        assert_eq!(Operator::from(K::Plus).operator_name(), " + ");
        assert_eq!(Operator::from(K::Plus).tight_operator_name(), "+");
        assert_eq!(Operator::from(K::LogicalNot).tight_operator_name(), "!");
        assert_eq!(Operator::from(K::LogicalXor).tight_operator_name(), "^^");
        assert_eq!(Operator::from(K::Comma).tight_operator_name(), ",");
        assert_eq!(Operator::from(K::PlusPlus).tight_operator_name(), "++");
    }

    #[test]
    fn assignments() {
        assert!(Operator::from(K::Eq).is_assignment());
        assert!(!Operator::from(K::Eq).is_compound_assignment());
        assert!(Operator::from(K::PlusEq).is_compound_assignment());
        assert!(!Operator::from(K::PlusPlus).is_assignment());
        assert_eq!(
            Operator::from(K::BitwiseXorEq).remove_assignment(),
            Operator::from(K::BitwiseXor)
        );
        assert_eq!(
            Operator::from(K::Eq).remove_assignment(),
            Operator::from(K::Eq)
        );
    }

    #[test]
    fn type_predicates() {
        assert!(Operator::from(K::Percent).is_only_valid_for_integral_types());
        assert!(!Operator::from(K::Slash).is_only_valid_for_integral_types());
        assert!(Operator::from(K::Star).is_valid_for_matrix_or_vector());
        assert!(!Operator::from(K::EqEq).is_valid_for_matrix_or_vector());
        assert!(!Operator::from(K::Shr).is_allowed_in_strict_es2_mode());
        assert!(Operator::from(K::Lt).is_relational());
        assert!(Operator::from(K::EqEq).is_equality());
    }
}
