// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis/SkSLProgramUsage.{h,cpp}.

//! [`ProgramUsage`]: how often a program declares, reads, writes and calls each of its symbols.
//!
//! Skia keys these tables by `Variable*`, `FunctionDeclaration*` and `Type*`. The ids are the same
//! identities (`VarId`, `FnId`, `TypeId`), so the tables hold ids. Skia never iterates them in an
//! order that reaches an output: `operator==` and the `any`-style queries in `sample_usage` only
//! produce a bool, so `std::collections::HashMap` is used.

use std::collections::HashMap;

use super::{ProgramVisitor, walk_expression, walk_program_element, walk_statement};
use crate::ir::{
    ElemId, ExprId, ExpressionKind, FnId, IrPool, ModifierFlags, Program, ProgramElementKind,
    StatementKind, StmtId, TypeId, VarId, VariableRefKind,
};
use crate::modules::Module;

/// `ProgramUsage::VariableCounts`.
#[doc(alias = "SkSL::ProgramUsage::VariableCounts")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VariableCounts {
    /// `fVarExists`: zero once the variable's declaration is gone (it might have been deleted).
    pub var_exists: i32,
    /// `fRead`.
    pub read: i32,
    /// `fWrite`. An initial value counts as a write.
    pub write: i32,
}

/// `SkSL::ProgramUsage`.
// Port of: src/sksl/analysis/SkSLProgramUsage.h#L26-L53 (chrome/m156)
#[doc(alias = "SkSL::ProgramUsage")]
#[derive(Clone, Debug, Default)]
pub struct ProgramUsage {
    /// `fStructCounts`: every struct type, keyed by its id. Non-zero entries are the live ones.
    pub struct_counts: HashMap<TypeId, i32>,
    /// `fCallCounts`: how often each function is called.
    pub call_counts: HashMap<FnId, i32>,
    /// `fVariableCounts`.
    pub variable_counts: HashMap<VarId, VariableCounts>,
}

impl ProgramUsage {
    /// `get(const Variable&)`. Skia asserts that the variable is known. A variable that was never
    /// seen reads as all zeros here.
    // Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L164-L168 (chrome/m156)
    #[must_use]
    pub fn get_variable(&self, var: VarId) -> VariableCounts {
        self.variable_counts.get(&var).copied().unwrap_or_default()
    }

    /// `isDead(const Variable&)`: never read and never written (beyond its initial value), and
    /// neither an `in`, `out` or `uniform`, nor an opaque type.
    // Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L170-L183 (chrome/m156)
    #[must_use]
    pub fn is_dead(&self, pool: &IrPool, var: VarId) -> bool {
        let variable = pool.variable(var);
        let flags = variable.modifier_flags;
        let counts = self.get_variable(var);
        if flags.intersects(ModifierFlags::IN | ModifierFlags::OUT | ModifierFlags::UNIFORM) {
            // Never eliminate ins, outs, or uniforms.
            return false;
        }
        if pool.ty(variable.ty).component_type().is_opaque() {
            // Never eliminate samplers, runtime-effect children, or atomics.
            return false;
        }
        // Consider the variable dead if it's never read and never written (besides the
        // initial-value).
        let initial_write = i32::from(variable.initial_value(pool).is_some());
        counts.read == 0 && counts.write <= initial_write
    }

    /// `get(const FunctionDeclaration&)`: the number of calls to `f`.
    // Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L185-L188 (chrome/m156)
    #[must_use]
    pub fn get_call_count(&self, f: FnId) -> i32 {
        self.call_counts.get(&f).copied().unwrap_or(0)
    }

    /// `add(const Expression*)`: counts the references in `expr`.
    // Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L190-L193 (chrome/m156)
    pub fn add_expression(&mut self, pool: &IrPool, expr: ExprId) {
        ProgramUsageVisitor {
            usage: self,
            delta: 1,
        }
        .visit_expression(pool, expr);
    }

    /// `add(const Statement*)`.
    // Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L195-L198 (chrome/m156)
    pub fn add_statement(&mut self, pool: &IrPool, stmt: StmtId) {
        ProgramUsageVisitor {
            usage: self,
            delta: 1,
        }
        .visit_statement(pool, stmt);
    }

    /// `add(const ProgramElement&)`.
    // Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L200-L203 (chrome/m156)
    pub fn add_element(&mut self, pool: &IrPool, element: ElemId) {
        ProgramUsageVisitor {
            usage: self,
            delta: 1,
        }
        .visit_program_element(pool, element);
    }

    /// `remove(const Expression*)`: the inverse of [`add_expression`](Self::add_expression).
    // Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L205-L208 (chrome/m156)
    pub fn remove_expression(&mut self, pool: &IrPool, expr: ExprId) {
        ProgramUsageVisitor {
            usage: self,
            delta: -1,
        }
        .visit_expression(pool, expr);
    }

    /// `remove(const Statement*)`.
    // Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L210-L213 (chrome/m156)
    pub fn remove_statement(&mut self, pool: &IrPool, stmt: StmtId) {
        ProgramUsageVisitor {
            usage: self,
            delta: -1,
        }
        .visit_statement(pool, stmt);
    }

    /// `remove(const ProgramElement&)`.
    // Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L215-L218 (chrome/m156)
    pub fn remove_element(&mut self, pool: &IrPool, element: ElemId) {
        ProgramUsageVisitor {
            usage: self,
            delta: -1,
        }
        .visit_program_element(pool, element);
    }
}

/// `ProgramUsage::operator==`. A dead-stripped entry stays in the maps with a count of zero, while
/// a fresh analysis has no entry at all, so zero entries are skipped and each side is compared
/// against the other.
// Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L285-L297 (chrome/m156)
impl PartialEq for ProgramUsage {
    fn eq(&self, that: &Self) -> bool {
        contains_matching_data(self, that) && contains_matching_data(that, self)
    }
}

// Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L220-L283 (chrome/m156)
fn contains_matching_data(a: &ProgramUsage, b: &ProgramUsage) -> bool {
    for (var_a, counts_a) in &a.variable_counts {
        // Skip variable entries with zero reported usage.
        if counts_a.var_exists == 0 && counts_a.read == 0 && counts_a.write == 0 {
            continue;
        }
        match b.variable_counts.get(var_a) {
            Some(counts_b) if counts_b == counts_a => {}
            _ => return false,
        }
    }

    for (call_a, &call_count_a) in &a.call_counts {
        // Skip function-call entries with zero reported usage.
        if call_count_a == 0 {
            continue;
        }
        if b.call_counts.get(call_a) != Some(&call_count_a) {
            return false;
        }
    }

    for (struct_a, &struct_count_a) in &a.struct_counts {
        // Skip struct entries with zero reported usage.
        if struct_count_a == 0 {
            continue;
        }
        if b.struct_counts.get(struct_a) != Some(&struct_count_a) {
            return false;
        }
    }

    // Every non-zero entry in A has a matching non-zero entry in B.
    true
}

/// `ProgramUsageVisitor`: adds (`delta` = 1) or removes (`delta` = -1) every reference it sees.
// Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L43-L141 (chrome/m156)
struct ProgramUsageVisitor<'a> {
    usage: &'a mut ProgramUsage,
    delta: i32,
}

impl ProgramUsageVisitor<'_> {
    /// `visitType`: arrays count their component type, structs count themselves and their fields.
    fn visit_type(&mut self, pool: &IrPool, ty: TypeId) {
        let t = pool.ty(ty);
        if t.is_array() {
            self.visit_type(pool, t.component_type().id());
            return;
        }
        if t.is_struct() {
            let count = self.usage.struct_counts.entry(ty).or_default();
            *count += self.delta;
            debug_assert!(*count >= 0);
            self.visit_struct_fields(pool, ty);
        }
    }

    /// `visitStructFields`.
    fn visit_struct_fields(&mut self, pool: &IrPool, ty: TypeId) {
        for field in pool.ty(ty).fields() {
            self.visit_type(pool, field.ty);
        }
    }
}

impl ProgramVisitor for ProgramUsageVisitor<'_> {
    // Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L47-L69 (chrome/m156)
    fn visit_program_element(&mut self, pool: &IrPool, element: ElemId) -> bool {
        match &pool.element(element).kind {
            ProgramElementKind::Function(def) => {
                for &param in &pool.function(def.declaration).parameters {
                    // Ensure function-parameter variables exist in the variable usage map. They
                    // aren't otherwise declared, but `get_variable` should find them, even if
                    // they are unread and unwritten.
                    let counts = self.usage.variable_counts.entry(param).or_default();
                    counts.var_exists += self.delta;
                    self.visit_type(pool, pool.variable(param).ty);
                }
            }
            ProgramElementKind::InterfaceBlock(block) => {
                // Ensure interface-block variables exist in the variable usage map.
                self.usage.variable_counts.entry(block.var).or_default();
                self.visit_type(pool, pool.variable(block.var).ty);
            }
            ProgramElementKind::StructDefinition(def) => {
                // Ensure that structs referenced as nested types in other structs are counted as
                // used.
                self.visit_struct_fields(pool, def.ty);
            }
            _ => {}
        }
        walk_program_element(self, pool, element)
    }

    // Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L71-L86 (chrome/m156)
    fn visit_statement(&mut self, pool: &IrPool, stmt: StmtId) -> bool {
        if let StatementKind::VarDeclaration(decl) = &pool.statement(stmt).kind {
            // Add all declared variables to the usage map (even if never otherwise accessed).
            let counts = self.usage.variable_counts.entry(decl.var).or_default();
            counts.var_exists += self.delta;
            debug_assert!(counts.var_exists >= 0 && counts.var_exists <= 1);
            if decl.value.is_some() {
                // The initial-value expression, when present, counts as a write.
                counts.write += self.delta;
            }
            self.visit_type(pool, pool.variable(decl.var).ty);
        }
        walk_statement(self, pool, stmt)
    }

    // Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L88-L113 (chrome/m156)
    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        let e = pool.expression(expr);
        self.visit_type(pool, e.ty);
        match &e.kind {
            ExpressionKind::FunctionCall(call) => {
                let count = self.usage.call_counts.entry(call.function).or_default();
                *count += self.delta;
                debug_assert!(*count >= 0);
            }
            ExpressionKind::VariableReference(reference) => {
                let counts = self
                    .usage
                    .variable_counts
                    .entry(reference.variable)
                    .or_default();
                match reference.ref_kind {
                    VariableRefKind::Read => counts.read += self.delta,
                    VariableRefKind::Write => counts.write += self.delta,
                    VariableRefKind::ReadWrite | VariableRefKind::Pointer => {
                        counts.read += self.delta;
                        counts.write += self.delta;
                    }
                }
                debug_assert!(counts.read >= 0 && counts.write >= 0);
            }
            _ => {}
        }
        walk_expression(self, pool, expr)
    }
}

/// `Analysis::GetUsage(const Program&)`: the usage of every element of `program`. Skia returns a
/// `unique_ptr`; the table is returned by value here.
// Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L145-L150 (chrome/m156)
#[must_use]
pub fn get_usage(program: &Program) -> ProgramUsage {
    let mut usage = ProgramUsage::default();
    ProgramUsageVisitor {
        usage: &mut usage,
        delta: 1,
    }
    .visit(program);
    usage
}

/// `Analysis::GetUsage(const Module&)` for a module that is not frozen yet: the usage of `elements`
/// (in `pool`, which extends `parent`'s pool) and of every module `parent` extends.
// Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L152-L162 (chrome/m156)
#[must_use]
pub fn get_module_parts_usage(pool: &IrPool, elements: &[ElemId], parent: &Module) -> ProgramUsage {
    let mut usage = ProgramUsage::default();
    let mut visitor = ProgramUsageVisitor {
        usage: &mut usage,
        delta: 1,
    };
    for &element in elements {
        visitor.visit_program_element(pool, element);
    }
    let mut current = Some(parent);
    while let Some(m) = current {
        for &element in &m.elements {
            visitor.visit_program_element(&m.pool, element);
        }
        current = m.parent.as_deref();
    }
    usage
}

/// `Analysis::GetUsage(const Module&)`: the usage of a module and of every module it extends.
/// Named apart from [`get_usage`] because Rust has no overloads.
// Port of: src/sksl/analysis/SkSLProgramUsage.cpp#L152-L162 (chrome/m156)
#[must_use]
pub fn get_module_usage(module: &Module) -> ProgramUsage {
    let mut usage = ProgramUsage::default();
    let mut visitor = ProgramUsageVisitor {
        usage: &mut usage,
        delta: 1,
    };
    let mut current = Some(module);
    while let Some(m) = current {
        for &element in &m.elements {
            visitor.visit_program_element(&m.pool, element);
        }
        current = m.parent.as_deref();
    }
    usage
}
