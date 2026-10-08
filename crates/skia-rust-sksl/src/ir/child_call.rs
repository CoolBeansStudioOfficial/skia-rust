// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLChildCall.{h,cpp} (data and `description`). `Make` comes
// with task S7c.

//! [`ChildCall`]: `child.eval(args…)` on a shader, color filter or blender.

use super::{
    IrPool,
    ids::{ExprId, VarId},
};
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

impl ChildCall {
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
