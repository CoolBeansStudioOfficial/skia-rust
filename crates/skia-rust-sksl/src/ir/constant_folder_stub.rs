// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Stand-ins for `ConstantFolder` (`SkSLConstantFolder.cpp`), which is task S8 and is not ported
//! yet. S7b's operator expressions call these functions where Skia calls `ConstantFolder`. When
//! S8 lands, `crate::constant_folder` provides the same names, and the call sites switch to it.
//!
//! The stand-ins match Skia where folding has no effect: `GetConstantValueForVariable` returns
//! the expression itself, as it does for a non-constant variable, and `Simplify` folds nothing.
//! The trees built by S7b therefore differ from Skia's only where Skia would fold them.

use super::{ExprId, IrPool, TypeId};
use crate::context::Context;
use crate::defines::SkslInt;
use crate::operator::Operator;
use crate::position::Position;

/// `ConstantFolder::Simplify(context, pos, left, op, right, resultType)`: a folded replacement for
/// the binary expression, or `None` when there is none.
///
/// Stand-in: never folds.
// Port of: src/sksl/SkSLConstantFolder.cpp (`Simplify`), stand-in until S8
pub(crate) fn simplify(
    ctx: &mut Context,
    pos: Position,
    left: ExprId,
    op: Operator,
    right: ExprId,
    result_type: TypeId,
) -> Option<ExprId> {
    let _ = (ctx, pos, left, op, right, result_type);
    None
}

/// `ConstantFolder::GetConstantValueForVariable(expr)`: the constant an expression stands for, or
/// the expression itself.
///
/// Stand-in: returns `expr`, which is what Skia returns for anything but a `const` variable with
/// a constant initializer.
// Port of: src/sksl/SkSLConstantFolder.cpp (`GetConstantValueForVariable`), stand-in until S8
pub(crate) fn get_constant_value_for_variable(pool: &IrPool, expr: ExprId) -> ExprId {
    let _ = pool;
    expr
}

/// `ConstantFolder::GetConstantInt(expr, &out)`: the integer `expr` is a literal of, if it is one.
// Port of: src/sksl/SkSLConstantFolder.cpp#L326-L333 (chrome/m156)
pub(crate) fn get_constant_int(pool: &IrPool, expr: ExprId) -> Option<SkslInt> {
    let id = get_constant_value_for_variable(pool, expr);
    let e = pool.expression(id);
    if !e.is_int_literal(pool) {
        return None;
    }
    e.as_literal().map(|lit| lit.int_value())
}
