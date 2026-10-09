// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis/SkSLCheckProgramStructure.cpp (chrome/m156).

//! [`check_program_structure`]: reports recursive call cycles and over-deep call chains.

use std::collections::HashMap;

use super::{ProgramVisitor, walk_expression, walk_program_element};
use crate::context::Context;
use crate::error_reporter::ErrorReporter;
use crate::ir::{ElemId, ExprId, ExpressionKind, FnId, IrPool, ProgramElementKind};

/// `kProgramStackDepthLimit`.
const PROGRAM_STACK_DEPTH_LIMIT: usize = 50;

/// `FunctionState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FunctionState {
    Visiting,
    Visited,
}

/// `ProgramStructureVisitor`.
struct ProgramStructureVisitor<'a> {
    errors: &'a mut ErrorReporter,
    function_map: HashMap<FnId, FunctionState>,
    stack: Vec<FnId>,
}

impl ProgramVisitor for ProgramStructureVisitor<'_> {
    // Port of: src/sksl/analysis/SkSLCheckProgramStructure.cpp#L36-L87 (chrome/m156)
    fn visit_program_element(&mut self, pool: &IrPool, element: ElemId) -> bool {
        let elem = pool.element(element);
        let ProgramElementKind::Function(func_def) = &elem.kind else {
            return walk_program_element(self, pool, element);
        };
        let decl_id = func_def.declaration;
        let decl = pool.function(decl_id);

        // Check the function map first. We don't need to visit this function if we already
        // processed it before.
        if let Some(&state) = self.function_map.get(&decl_id) {
            if state == FunctionState::Visiting {
                // The function is still being processed, so we found a cycle in the code. Unwind
                // our stack into a string.
                let mut msg = format!("\n\t{}", decl.description(pool));
                for &unwind in self.stack.iter().rev() {
                    msg = format!("\n\t{}{}", pool.function(unwind).description(pool), msg);
                    if unwind == decl_id {
                        break;
                    }
                }
                msg = format!("potential recursion (function call cycle) not allowed:{msg}");
                self.errors.error(elem.position, &msg);
                self.function_map.insert(decl_id, FunctionState::Visited);
                return true;
            }
            return false;
        }

        // If the function-call stack has gotten too deep, stop the analysis.
        if self.stack.len() >= PROGRAM_STACK_DEPTH_LIMIT {
            let mut msg = String::from("exceeded max function call depth:");
            for &unwind in &self.stack {
                msg.push_str("\n\t");
                msg.push_str(&pool.function(unwind).description(pool));
            }
            msg.push_str("\n\t");
            msg.push_str(&decl.description(pool));
            self.errors.error(elem.position, &msg);
            self.function_map.insert(decl_id, FunctionState::Visited);
            return true;
        }

        self.function_map.insert(decl_id, FunctionState::Visiting);
        self.stack.push(decl_id);
        let result = walk_program_element(self, pool, element);
        self.function_map.insert(decl_id, FunctionState::Visited);
        self.stack.pop();
        result
    }

    // Port of: src/sksl/analysis/SkSLCheckProgramStructure.cpp#L89-L101 (chrome/m156)
    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        let mut early_exit = false;
        if let ExpressionKind::FunctionCall(call) = &pool.expression(expr).kind {
            let decl = pool.function(call.function);
            if let Some(definition) = decl.definition
                && !decl.is_intrinsic()
            {
                early_exit = self.visit_program_element(pool, definition);
            }
        }
        early_exit || walk_expression(self, pool, expr)
    }
}

/// `Analysis::CheckProgramStructure`: visits every function definition of the program (even
/// unreferenced ones, so that static recursion is always reported) and reports each call cycle
/// and each call chain deeper than the limit. Returns true, as Skia does.
// Port of: src/sksl/analysis/SkSLCheckProgramStructure.cpp#L29-L35 and #L103-L125 (chrome/m156)
pub fn check_program_structure(ctx: &mut Context, owned_elements: &[ElemId]) -> bool {
    let pool: &IrPool = &ctx.pool;
    let mut visitor = ProgramStructureVisitor {
        errors: &mut ctx.errors,
        function_map: HashMap::new(),
        stack: Vec::new(),
    };
    for &element in owned_elements {
        // Visit every function, so that a cycle is reported even in an unreferenced function.
        if matches!(pool.element(element).kind, ProgramElementKind::Function(_)) {
            visitor.visit_program_element(pool, element);
        }
    }
    true
}
