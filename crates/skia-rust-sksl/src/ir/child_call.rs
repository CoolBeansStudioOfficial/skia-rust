// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLChildCall.{h,cpp} (data, `description` and `Make`).

//! [`ChildCall`]: `child.eval(args…)` on a shader, color filter or blender.

use super::{
    Expression, ExpressionKind, IrPool, TypeKind,
    ids::{ExprId, TypeId, VarId},
};
use crate::context::Context;
use crate::position::Position;
use crate::string::Separator;

/// `SkSL::ChildCall`.
// Port of: src/sksl/ir/SkSLChildCall.h#L27-L71 (chrome/m156)
#[doc(alias = "SkSL::ChildCall")]
#[derive(Clone, Debug, PartialEq)]
pub struct ChildCall {
    /// `child()`: the `shader`/`colorFilter`/`blender` variable.
    pub child: VarId,
    /// `arguments()`.
    pub arguments: Vec<ExprId>,
}

/// `call_signature_is_valid(context, child, arguments)`: the argument types a child's `eval` takes
/// (`half4, half4` for a blender, `half4` for a color filter, `float2` for a shader).
// Port of: src/sksl/ir/SkSLChildCall.cpp#L39-L64 (chrome/m156)
fn call_signature_is_valid(ctx: &Context, child: VarId, arguments: &[ExprId]) -> bool {
    let child_ty = ctx.pool.variable(child).ty;
    let params: &[TypeId] = match ctx.pool.type_node(child_ty).type_kind {
        TypeKind::Blender => &[TypeId::HALF4, TypeId::HALF4],
        TypeKind::ColorFilter => &[TypeId::HALF4],
        TypeKind::Shader => &[TypeId::FLOAT2],
        _ => unreachable!("a child is a shader, color filter or blender"),
    };
    if params.len() != arguments.len() {
        return false;
    }
    arguments
        .iter()
        .zip(params)
        .all(|(&arg, &param)| ctx.pool.ty(ctx.pool.expression(arg).ty).matches(param))
}

impl ChildCall {
    /// `Make(context, pos, returnType, child, arguments)`: a call of `child` returning
    /// `return_type`. The arguments must match the child's signature.
    // Port of: src/sksl/ir/SkSLChildCall.cpp#L66-L73 (chrome/m156)
    #[must_use]
    pub fn make(
        ctx: &mut Context,
        pos: Position,
        return_type: TypeId,
        child: VarId,
        arguments: Vec<ExprId>,
    ) -> ExprId {
        debug_assert!(call_signature_is_valid(ctx, child, &arguments));
        ctx.pool.add_expression(Expression::new(
            pos,
            return_type,
            ExpressionKind::ChildCall(Self { child, arguments }),
        ))
    }

    /// `description()`: `child.eval(a, b)`.
    // Port of: src/sksl/ir/SkSLChildCall.cpp#L28-L37 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        let mut result = format!("{}.eval(", pool.variable(self.child).name);
        let mut separator = Separator::new();
        for &arg in &self.arguments {
            result.push_str(separator.next_str());
            result.push_str(
                &pool.expression_description_with(
                    arg,
                    crate::operator::OperatorPrecedence::Sequence,
                ),
            );
        }
        result.push(')');
        result
    }
}
