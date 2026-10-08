// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis/SkSLGetReturnComplexity.cpp (chrome/m156).

//! [`get_return_complexity`]: how many `return`s a function has, and where they are. The inliner
//! uses it to choose how to inline a call.

use super::{ProgramVisitor, walk_statement};
use crate::ir::{ExprId, FunctionDefinition, IrPool, StatementKind, StmtId};

/// `Analysis::ReturnComplexity`. The variants are ordered from simplest to most complex, so
/// `<=` and `>` compare complexity as Skia does.
// Port of: src/sksl/SkSLAnalysis.h (ReturnComplexity, chrome/m156)
#[doc(alias = "SkSL::Analysis::ReturnComplexity")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReturnComplexity {
    /// One `return`, at the end of the function's control flow.
    SingleSafeReturn,
    /// Several `return`s, or a `return` inside a scope with variables declared in it.
    ScopedReturns,
    /// A `return` before the end of the function's control flow.
    EarlyReturns,
}

/// `CountReturnsAtEndOfControlFlow`: counts the `return`s that end the function's control flow.
struct CountReturnsAtEndOfControlFlow {
    num_returns: i32,
}

impl ProgramVisitor for CountReturnsAtEndOfControlFlow {
    fn visit_expression(&mut self, _pool: &IrPool, _expr: ExprId) -> bool {
        // Do not recurse into expressions.
        false
    }

    // Port of: src/sksl/analysis/SkSLGetReturnComplexity.cpp#L13-L46 (chrome/m156)
    fn visit_statement(&mut self, pool: &IrPool, stmt: StmtId) -> bool {
        match &pool.statement(stmt).kind {
            StatementKind::Block(block) => {
                // Check only the last statement of a block.
                match block.children.last() {
                    Some(&last) => self.visit_statement(pool, last),
                    None => false,
                }
            }
            // Don't introspect switches or loop structures at all.
            StatementKind::Switch(_) | StatementKind::Do(_) | StatementKind::For(_) => false,
            StatementKind::Return(_) => {
                self.num_returns += 1;
                walk_statement(self, pool, stmt)
            }
            _ => walk_statement(self, pool, stmt),
        }
    }
}

/// `CountReturnsWithLimit`: counts the `return`s up to a limit, and the scope depth of the
/// deepest one, and whether a variable is declared inside a nested scope.
struct CountReturnsWithLimit {
    num_returns: i32,
    deepest_return: i32,
    limit: i32,
    scoped_block_depth: i32,
    variables_in_blocks: bool,
}

impl ProgramVisitor for CountReturnsWithLimit {
    fn visit_expression(&mut self, _pool: &IrPool, _expr: ExprId) -> bool {
        // Do not recurse into expressions.
        false
    }

    // Port of: src/sksl/analysis/SkSLGetReturnComplexity.cpp#L48-L89 (chrome/m156)
    fn visit_statement(&mut self, pool: &IrPool, stmt: StmtId) -> bool {
        match &pool.statement(stmt).kind {
            StatementKind::Return(_) => {
                self.num_returns += 1;
                self.deepest_return = self.deepest_return.max(self.scoped_block_depth);
                (self.num_returns >= self.limit) || walk_statement(self, pool, stmt)
            }
            StatementKind::VarDeclaration(_) => {
                if self.scoped_block_depth > 1 {
                    self.variables_in_blocks = true;
                }
                walk_statement(self, pool, stmt)
            }
            StatementKind::Block(block) => {
                let depth_increment = i32::from(block.is_scope());
                self.scoped_block_depth += depth_increment;
                let result = walk_statement(self, pool, stmt);
                self.scoped_block_depth -= depth_increment;
                if self.num_returns == 0 && self.scoped_block_depth <= 1 {
                    // Closing this block puts us back at the top level, and no `return` has been
                    // seen yet. Any variables declared so far are out of scope and were never
                    // used by a `return`, so they can be ignored.
                    self.variables_in_blocks = false;
                }
                result
            }
            _ => walk_statement(self, pool, stmt),
        }
    }
}

/// `Analysis::GetReturnComplexity`: classifies the `return`s of a function definition.
// Port of: src/sksl/analysis/SkSLGetReturnComplexity.cpp#L116-L130 (chrome/m156)
#[doc(alias = "SkSL::Analysis::GetReturnComplexity")]
#[must_use]
pub fn get_return_complexity(pool: &IrPool, func_def: &FunctionDefinition) -> ReturnComplexity {
    let mut at_end = CountReturnsAtEndOfControlFlow { num_returns: 0 };
    at_end.visit_statement(pool, func_def.body);
    let returns_at_end_of_control_flow = at_end.num_returns;

    let mut counter = CountReturnsWithLimit {
        num_returns: 0,
        deepest_return: 0,
        limit: returns_at_end_of_control_flow + 1,
        scoped_block_depth: 0,
        variables_in_blocks: false,
    };
    counter.visit_statement(pool, func_def.body);
    if counter.num_returns > returns_at_end_of_control_flow {
        return ReturnComplexity::EarlyReturns;
    }
    if counter.num_returns > 1 {
        return ReturnComplexity::ScopedReturns;
    }
    if counter.variables_in_blocks && counter.deepest_return > 1 {
        return ReturnComplexity::ScopedReturns;
    }
    ReturnComplexity::SingleSafeReturn
}
