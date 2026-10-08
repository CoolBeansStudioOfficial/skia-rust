// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis/SkSLProgramVisitor.h and the `TProgramVisitor` recursion in
// src/sksl/SkSLAnalysis.cpp.

//! [`ProgramVisitor`]: read-only traversal of a program's IR, in Skia's order.

use crate::ir::{
    ElemId, ExprId, ExpressionKind, IrPool, Program, ProgramElementKind, StatementKind, StmtId,
};

/// `SkSL::ProgramVisitor`. Each `visit_*` returns true to stop the traversal. Override a method
/// and call the matching `walk_*` function for Skia's `INHERITED::visitX` (the default
/// recursion into children).
#[doc(alias = "SkSL::ProgramVisitor")]
pub trait ProgramVisitor {
    /// `visitExpression`.
    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        walk_expression(self, pool, expr)
    }

    /// `visitStatement`.
    fn visit_statement(&mut self, pool: &IrPool, stmt: StmtId) -> bool {
        walk_statement(self, pool, stmt)
    }

    /// `visitProgramElement`.
    fn visit_program_element(&mut self, pool: &IrPool, element: ElemId) -> bool {
        walk_program_element(self, pool, element)
    }

    /// `visit(program)`: visits every element (shared ones first), stopping at the first that
    /// returns true.
    // Port of: src/sksl/SkSLAnalysis.cpp#L577-L584 (chrome/m156)
    fn visit(&mut self, program: &Program) -> bool {
        program
            .elements()
            .any(|e| self.visit_program_element(&program.pool, e))
    }
}

/// `TProgramVisitor::visitExpression`: visits the children of `expr`. Leaf expressions return
/// false. `ChildCall` visits its arguments, not the child variable; `MethodReference` is a leaf.
// Port of: src/sksl/SkSLAnalysis.cpp#L586-L668 (chrome/m156)
pub fn walk_expression<V: ProgramVisitor + ?Sized>(v: &mut V, pool: &IrPool, expr: ExprId) -> bool {
    match &pool.expression(expr).kind {
        ExpressionKind::Empty(_)
        | ExpressionKind::FunctionReference(_)
        | ExpressionKind::Literal(_)
        | ExpressionKind::MethodReference(_)
        | ExpressionKind::Poison(_)
        | ExpressionKind::Setting(_)
        | ExpressionKind::TypeReference(_)
        | ExpressionKind::VariableReference(_) => false,
        ExpressionKind::Binary(b) => {
            v.visit_expression(pool, b.left) || v.visit_expression(pool, b.right)
        }
        ExpressionKind::ChildCall(c) => c.arguments.iter().any(|&a| v.visit_expression(pool, a)),
        ExpressionKind::FunctionCall(c) => c.arguments.iter().any(|&a| v.visit_expression(pool, a)),
        ExpressionKind::FieldAccess(f) => v.visit_expression(pool, f.base),
        ExpressionKind::Index(i) => {
            v.visit_expression(pool, i.base) || v.visit_expression(pool, i.index)
        }
        ExpressionKind::Postfix(p) => v.visit_expression(pool, p.operand),
        ExpressionKind::Prefix(p) => v.visit_expression(pool, p.operand),
        ExpressionKind::Swizzle(s) => v.visit_expression(pool, s.base),
        ExpressionKind::Ternary(t) => {
            v.visit_expression(pool, t.test)
                || v.visit_expression(pool, t.if_true)
                || v.visit_expression(pool, t.if_false)
        }
        ExpressionKind::ConstructorArray(_)
        | ExpressionKind::ConstructorArrayCast(_)
        | ExpressionKind::ConstructorCompound(_)
        | ExpressionKind::ConstructorCompoundCast(_)
        | ExpressionKind::ConstructorDiagonalMatrix(_)
        | ExpressionKind::ConstructorMatrixResize(_)
        | ExpressionKind::ConstructorScalarCast(_)
        | ExpressionKind::ConstructorSplat(_)
        | ExpressionKind::ConstructorStruct(_) => pool
            .expression(expr)
            .any_constructor_arguments()
            .unwrap_or_default()
            .iter()
            .any(|&a| v.visit_expression(pool, a)),
    }
}

/// `TProgramVisitor::visitStatement`: visits the children of `stmt`. Leaf statements return
/// false.
// Port of: src/sksl/SkSLAnalysis.cpp#L670-L730 (chrome/m156)
pub fn walk_statement<V: ProgramVisitor + ?Sized>(v: &mut V, pool: &IrPool, stmt: StmtId) -> bool {
    match &pool.statement(stmt).kind {
        StatementKind::Break(_)
        | StatementKind::Continue(_)
        | StatementKind::Discard(_)
        | StatementKind::Nop(_) => false,
        StatementKind::Block(b) => b.children.iter().any(|&s| v.visit_statement(pool, s)),
        StatementKind::SwitchCase(sc) => v.visit_statement(pool, sc.statement),
        StatementKind::Do(d) => {
            v.visit_expression(pool, d.test) || v.visit_statement(pool, d.statement)
        }
        StatementKind::Expression(e) => v.visit_expression(pool, e.expression),
        StatementKind::For(f) => {
            f.initializer.is_some_and(|s| v.visit_statement(pool, s))
                || f.test.is_some_and(|e| v.visit_expression(pool, e))
                || f.next.is_some_and(|e| v.visit_expression(pool, e))
                || v.visit_statement(pool, f.statement)
        }
        StatementKind::If(i) => {
            v.visit_expression(pool, i.test)
                || v.visit_statement(pool, i.if_true)
                || i.if_false.is_some_and(|s| v.visit_statement(pool, s))
        }
        StatementKind::Return(r) => r.expression.is_some_and(|e| v.visit_expression(pool, e)),
        StatementKind::Switch(sw) => {
            v.visit_expression(pool, sw.value) || v.visit_statement(pool, sw.case_block)
        }
        StatementKind::VarDeclaration(d) => d.value.is_some_and(|e| v.visit_expression(pool, e)),
    }
}

/// `TProgramVisitor::visitProgramElement`: functions visit their body, globals their
/// declaration; the other elements are leaves.
// Port of: src/sksl/SkSLAnalysis.cpp#L732-L752 (chrome/m156)
pub fn walk_program_element<V: ProgramVisitor + ?Sized>(
    v: &mut V,
    pool: &IrPool,
    element: ElemId,
) -> bool {
    match &pool.element(element).kind {
        ProgramElementKind::Extension(_)
        | ProgramElementKind::FunctionPrototype(_)
        | ProgramElementKind::InterfaceBlock(_)
        | ProgramElementKind::Modifiers(_)
        | ProgramElementKind::StructDefinition(_) => false,
        ProgramElementKind::Function(f) => v.visit_statement(pool, f.body),
        ProgramElementKind::GlobalVar(g) => v.visit_statement(pool, g.declaration),
    }
}
