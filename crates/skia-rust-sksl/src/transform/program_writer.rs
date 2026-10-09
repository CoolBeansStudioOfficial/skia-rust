// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/transform/SkSLProgramWriter.h and the `TProgramVisitor` recursion
// (`ProgramWriterTypes` instantiation) in src/sksl/SkSLAnalysis.cpp.

//! [`ProgramWriter`]: traversal that may rewrite the IR in place.

use crate::context::Context;
use crate::ir::{ElemId, ExprId, ExpressionKind, ProgramElementKind, StatementKind, StmtId};

/// `SkSL::ProgramWriter`. Like [`ProgramVisitor`](crate::analysis::ProgramVisitor), but the
/// traversal holds no borrow of the pool while it calls back, so an override may rewrite any
/// local node through `ctx.pool` (Skia: assign to the `unique_ptr&` it was given).
///
/// Skia's `visitExpressionPtr(std::unique_ptr<Expression>& e)` receives the owning slot; here
/// the slot *is* the id, and replacing the node is `ctx.pool.replace_expression(e, new_node)`.
/// The child ids of a node are read before its children are visited, as Skia's range loops do,
/// so rewrite nodes in place rather than editing a parent's child list during its traversal.
#[doc(alias = "SkSL::ProgramWriter")]
pub trait ProgramWriter {
    /// `visitExpression`.
    fn visit_expression(&mut self, ctx: &mut Context, expr: ExprId) -> bool {
        walk_expression_mut(self, ctx, expr)
    }

    /// `visitStatement`.
    fn visit_statement(&mut self, ctx: &mut Context, stmt: StmtId) -> bool {
        walk_statement_mut(self, ctx, stmt)
    }

    /// `visitProgramElement`.
    fn visit_program_element(&mut self, ctx: &mut Context, element: ElemId) -> bool {
        walk_program_element_mut(self, ctx, element)
    }

    /// `visitExpressionPtr(e)`: called for every child expression; override it to replace the
    /// child.
    fn visit_expression_ptr(&mut self, ctx: &mut Context, expr: ExprId) -> bool {
        self.visit_expression(ctx, expr)
    }

    /// `visitStatementPtr(s)`: called for every child statement; override it to replace the
    /// child.
    fn visit_statement_ptr(&mut self, ctx: &mut Context, stmt: StmtId) -> bool {
        self.visit_statement(ctx, stmt)
    }
}

/// `TProgramVisitor<ProgramWriterTypes>::visitExpression`.
// Port of: src/sksl/SkSLAnalysis.cpp#L586-L668 (chrome/m156)
pub fn walk_expression_mut<W: ProgramWriter + ?Sized>(
    w: &mut W,
    ctx: &mut Context,
    expr: ExprId,
) -> bool {
    let children: Vec<ExprId> = match &ctx.pool.expression(expr).kind {
        ExpressionKind::Empty(_)
        | ExpressionKind::FunctionReference(_)
        | ExpressionKind::Literal(_)
        | ExpressionKind::MethodReference(_)
        | ExpressionKind::Poison(_)
        | ExpressionKind::Setting(_)
        | ExpressionKind::TypeReference(_)
        | ExpressionKind::VariableReference(_) => return false,
        ExpressionKind::Binary(b) => vec![b.left, b.right],
        ExpressionKind::ChildCall(c) => c.arguments.clone(),
        ExpressionKind::FunctionCall(c) => c.arguments.clone(),
        ExpressionKind::FieldAccess(f) => vec![f.base],
        ExpressionKind::Index(i) => vec![i.base, i.index],
        ExpressionKind::Postfix(p) => vec![p.operand],
        ExpressionKind::Prefix(p) => vec![p.operand],
        ExpressionKind::Swizzle(s) => vec![s.base],
        ExpressionKind::Ternary(t) => vec![t.test, t.if_true, t.if_false],
        ExpressionKind::ConstructorArray(_)
        | ExpressionKind::ConstructorArrayCast(_)
        | ExpressionKind::ConstructorCompound(_)
        | ExpressionKind::ConstructorCompoundCast(_)
        | ExpressionKind::ConstructorDiagonalMatrix(_)
        | ExpressionKind::ConstructorMatrixResize(_)
        | ExpressionKind::ConstructorScalarCast(_)
        | ExpressionKind::ConstructorSplat(_)
        | ExpressionKind::ConstructorStruct(_) => ctx
            .pool
            .expression(expr)
            .any_constructor_arguments()
            .unwrap_or_default()
            .to_vec(),
    };
    children
        .into_iter()
        .any(|child| w.visit_expression_ptr(ctx, child))
}

/// A child of a statement, in visiting order.
enum Child {
    Expr(ExprId),
    Stmt(StmtId),
}

/// `TProgramVisitor<ProgramWriterTypes>::visitStatement`.
// Port of: src/sksl/SkSLAnalysis.cpp#L670-L730 (chrome/m156)
pub fn walk_statement_mut<W: ProgramWriter + ?Sized>(
    w: &mut W,
    ctx: &mut Context,
    stmt: StmtId,
) -> bool {
    let children: Vec<Child> = match &ctx.pool.statement(stmt).kind {
        StatementKind::Break(_)
        | StatementKind::Continue(_)
        | StatementKind::Discard(_)
        | StatementKind::Nop(_) => return false,
        StatementKind::Block(b) => b.children.iter().map(|&s| Child::Stmt(s)).collect(),
        StatementKind::SwitchCase(sc) => vec![Child::Stmt(sc.statement)],
        StatementKind::Do(d) => vec![Child::Expr(d.test), Child::Stmt(d.statement)],
        StatementKind::Expression(e) => vec![Child::Expr(e.expression)],
        StatementKind::For(f) => f
            .initializer
            .map(Child::Stmt)
            .into_iter()
            .chain(f.test.map(Child::Expr))
            .chain(f.next.map(Child::Expr))
            .chain(std::iter::once(Child::Stmt(f.statement)))
            .collect(),
        StatementKind::If(i) => std::iter::once(Child::Expr(i.test))
            .chain(std::iter::once(Child::Stmt(i.if_true)))
            .chain(i.if_false.map(Child::Stmt))
            .collect(),
        StatementKind::Return(r) => r.expression.map(Child::Expr).into_iter().collect(),
        StatementKind::Switch(sw) => vec![Child::Expr(sw.value), Child::Stmt(sw.case_block)],
        StatementKind::VarDeclaration(d) => d.value.map(Child::Expr).into_iter().collect(),
    };
    children.into_iter().any(|child| match child {
        Child::Expr(e) => w.visit_expression_ptr(ctx, e),
        Child::Stmt(s) => w.visit_statement_ptr(ctx, s),
    })
}

/// `TProgramVisitor<ProgramWriterTypes>::visitProgramElement`.
// Port of: src/sksl/SkSLAnalysis.cpp#L732-L752 (chrome/m156)
pub fn walk_program_element_mut<W: ProgramWriter + ?Sized>(
    w: &mut W,
    ctx: &mut Context,
    element: ElemId,
) -> bool {
    let child = match &ctx.pool.element(element).kind {
        ProgramElementKind::Extension(_)
        | ProgramElementKind::FunctionPrototype(_)
        | ProgramElementKind::InterfaceBlock(_)
        | ProgramElementKind::Modifiers(_)
        | ProgramElementKind::StructDefinition(_) => return false,
        ProgramElementKind::Function(f) => f.body,
        ProgramElementKind::GlobalVar(g) => g.declaration,
    };
    w.visit_statement_ptr(ctx, child)
}
