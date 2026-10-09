// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis/SkSLReturnsInputAlpha.cpp.

//! Whether a color filter returns its input's alpha unchanged.

use super::{ProgramUsage, ProgramVisitor, walk_program_element, walk_statement};
use crate::ir::{
    ElemId, ExprId, ExpressionKind, IrPool, ProgramElementKind, StatementKind, StmtId, VarId,
};

/// `ReturnsInputAlphaVisitor`.
// Port of: src/sksl/analysis/SkSLReturnsInputAlpha.cpp#L37-L122 (chrome/m156)
struct ReturnsInputAlphaVisitor<'a> {
    pool: &'a IrPool,
    usage: &'a ProgramUsage,
    input_var: Option<VarId>,
}

impl ReturnsInputAlphaVisitor<'_> {
    /// `isInputVar`.
    fn is_input_var(&self, expr: ExprId) -> bool {
        matches!(
            &self.pool.expression(expr).kind,
            ExpressionKind::VariableReference(reference) if Some(reference.variable) == self.input_var
        )
    }

    /// `isInputSwizzleEndingWithAlpha`: `input.___a`.
    fn is_input_swizzle_ending_with_alpha(&self, expr: ExprId) -> bool {
        match &self.pool.expression(expr).kind {
            ExpressionKind::Swizzle(swizzle) => {
                self.is_input_var(swizzle.base) && swizzle.components.as_slice().last() == Some(&3)
            }
            _ => false,
        }
    }

    /// `returnsInputAlpha(expr)`: true if `expr` is the input, or a form of it whose alpha is the
    /// input's alpha.
    fn returns_input_alpha_expression(&self, expr: ExprId) -> bool {
        if self.is_input_var(expr) {
            // This expression returns the input value as-is.
            return true;
        }
        let e = self.pool.expression(expr);
        match &e.kind {
            // It's a swizzle: check for `input.___a`.
            ExpressionKind::Swizzle(_) => self.is_input_swizzle_ending_with_alpha(expr),
            // This is a splat or compound constructor; check for `input.a` as its final component.
            ExpressionKind::ConstructorSplat(_) | ExpressionKind::ConstructorCompound(_) => e
                .any_constructor_arguments()
                .and_then(|arguments| arguments.last())
                .is_some_and(|&last| self.returns_input_alpha_expression(last)),
            // Ignore typecasts between float and half.
            ExpressionKind::ConstructorCompoundCast(cast) => {
                self.pool
                    .ty(self.pool.expression(cast.argument).ty)
                    .component_type()
                    .is_float()
                    && self.returns_input_alpha_expression(cast.argument)
            }
            // Both sides of the ternary must preserve input alpha.
            ExpressionKind::Ternary(ternary) => {
                self.returns_input_alpha_expression(ternary.if_true)
                    && self.returns_input_alpha_expression(ternary.if_false)
            }
            // We weren't able to pattern-match here.
            _ => false,
        }
    }
}

impl ProgramVisitor for ReturnsInputAlphaVisitor<'_> {
    fn visit_program_element(&mut self, pool: &IrPool, element: ElemId) -> bool {
        let ProgramElementKind::Function(def) = &pool.element(element).kind else {
            return true;
        };
        let parameters = &pool.function(def.declaration).parameters;

        // We expect a color filter to have a single half4 input.
        let is_color_filter_input = match parameters.as_slice() {
            [param] => {
                let ty = pool.ty(pool.variable(*param).ty);
                ty.columns() == 4 && ty.component_type().is_float()
            }
            _ => false,
        };
        if !is_color_filter_input {
            // This doesn't look like a color filter.
            return true;
        }
        self.input_var = Some(parameters[0]);

        // If the input variable has been written-to, then returning `input.a` isn't sufficient to
        // guarantee that alpha is preserved.
        if self.usage.get_variable(parameters[0]).write != 0 {
            return true;
        }
        walk_program_element(self, pool, element)
    }

    fn visit_statement(&mut self, pool: &IrPool, stmt: StmtId) -> bool {
        if let StatementKind::Return(ret) = &pool.statement(stmt).kind {
            // A return without a value cannot preserve alpha (Skia dereferences the expression
            // here, which is only reachable for a half4 function, so this is never taken there).
            return match ret.expression {
                Some(expr) => !self.returns_input_alpha_expression(expr),
                None => true,
            };
        }
        walk_statement(self, pool, stmt)
    }

    fn visit_expression(&mut self, _pool: &IrPool, _expr: ExprId) -> bool {
        // No need to recurse into expressions; these can never contain return statements.
        false
    }
}

/// `Analysis::ReturnsInputAlpha`: true if the color filter `function` returns the alpha of its
/// input unchanged. This is a very conservative analysis, and only recognizes a swizzle of the
/// input, or a constructor that ends with `input.a`.
// Port of: src/sksl/analysis/SkSLReturnsInputAlpha.cpp#L126-L129 (chrome/m156)
#[must_use]
pub fn returns_input_alpha(pool: &IrPool, function: ElemId, usage: &ProgramUsage) -> bool {
    let mut visitor = ReturnsInputAlphaVisitor {
        pool,
        usage,
        input_var: None,
    };
    !visitor.visit_program_element(pool, function)
}
