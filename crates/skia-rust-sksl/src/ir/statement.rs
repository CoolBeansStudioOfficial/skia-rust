// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLStatement.h, src/sksl/ir/SkSLIRNode.h (`StatementKind`) and
// the `description`/`isEmpty` overrides of every statement class.

//! [`Statement`]: a statement node, and [`StatementKind`], its subclass and data.

use super::{
    Block, BreakStatement, ContinueStatement, DiscardStatement, DoStatement, ExpressionStatement,
    ForStatement, IfStatement, IrPool, Nop, ReturnStatement, SwitchCase, SwitchStatement,
    VarDeclaration, ids::StmtId,
};
use crate::position::Position;

/// `SkSL::Statement`.
// Port of: src/sksl/ir/SkSLStatement.h#L21-L39 (chrome/m156)
#[doc(alias = "SkSL::Statement")]
#[derive(Clone, Debug, PartialEq)]
pub struct Statement {
    /// `fPosition`.
    pub position: Position,
    /// The subclass and its data (`kind()`).
    pub kind: StatementKind,
}

/// `Statement::Kind`, carrying each subclass's data, in Skia's `StatementKind` order.
// Port of: src/sksl/ir/SkSLIRNode.h#L43-L57 (chrome/m156)
#[doc(alias = "SkSL::StatementKind")]
#[derive(Clone, Debug, PartialEq)]
pub enum StatementKind {
    Block(Block),
    Break(BreakStatement),
    Continue(ContinueStatement),
    Discard(DiscardStatement),
    Do(DoStatement),
    Expression(ExpressionStatement),
    For(ForStatement),
    If(IfStatement),
    Nop(Nop),
    Return(ReturnStatement),
    Switch(SwitchStatement),
    SwitchCase(SwitchCase),
    VarDeclaration(VarDeclaration),
}

impl Statement {
    /// A node of `kind` at `position`.
    #[must_use]
    pub fn new(position: Position, kind: StatementKind) -> Self {
        Self { position, kind }
    }

    /// `isEmpty()`: a `Nop`, or a block of empty statements.
    #[must_use]
    pub fn is_empty(&self, pool: &IrPool) -> bool {
        match &self.kind {
            StatementKind::Nop(_) => true,
            StatementKind::Block(b) => b.is_empty(pool),
            _ => false,
        }
    }

    /// The block payload, if this is a `Block`.
    #[must_use]
    pub fn as_block(&self) -> Option<&Block> {
        match &self.kind {
            StatementKind::Block(b) => Some(b),
            _ => None,
        }
    }

    /// The `VarDeclaration` payload, if this is one.
    #[must_use]
    pub fn as_var_declaration(&self) -> Option<&VarDeclaration> {
        match &self.kind {
            StatementKind::VarDeclaration(v) => Some(v),
            _ => None,
        }
    }

    /// `description()`: the statement as `SkSL` text.
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        match &self.kind {
            StatementKind::Block(b) => b.description(pool),
            StatementKind::Break(b) => b.description(),
            StatementKind::Continue(c) => c.description(),
            StatementKind::Discard(d) => d.description(),
            StatementKind::Do(d) => d.description(pool),
            StatementKind::Expression(e) => e.description(pool),
            StatementKind::For(f) => f.description(pool),
            StatementKind::If(i) => i.description(pool),
            StatementKind::Nop(n) => n.description(),
            StatementKind::Return(r) => r.description(pool),
            StatementKind::Switch(s) => s.description(pool),
            StatementKind::SwitchCase(s) => s.description(pool),
            StatementKind::VarDeclaration(v) => v.description(pool),
        }
    }
}

impl IrPool {
    /// `stmt->description()` for the statement at `id`.
    #[must_use]
    pub fn statement_description(&self, id: StmtId) -> String {
        self.statement(id).description(self)
    }
}
