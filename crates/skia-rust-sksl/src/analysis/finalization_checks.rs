// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/analysis/SkSLFinalizationChecks.cpp (chrome/m156).

//! [`do_finalization_checks`]: the last-minute checks that run when a program is finalized.

use std::collections::HashSet;

use super::s9b_shims::{WriteCounts, saturating_add_size};
use super::{ProgramVisitor, walk_expression, walk_program_element};
use crate::context::Context;
use crate::defines::VARIABLE_SLOT_LIMIT;
use crate::error_reporter::ErrorReporter;
use crate::ir::{
    ElemId, ExprId, ExpressionKind, FunctionDefinition, GlobalVarDeclaration, InterfaceBlock,
    IrPool, ModifierFlags, ModifiersDeclaration, ProgramElementKind, StatementKind, TypeId,
};
use crate::position::Position;
use crate::program_settings::{ProgramConfig, ProgramKind};

/// `FinalizationVisitor`.
struct FinalizationVisitor<'a> {
    kind: ProgramKind,
    errors: &'a mut ErrorReporter,
    usage: &'a WriteCounts,
    global_slots_used: usize,
    // We pack the set/binding pair into a single 64-bit key.
    bindings: HashSet<u64>,
    // Compute programs must at least specify the X dimension of the local size. The other
    // dimensions have a default value of "1".
    local_size_x: i32,
    local_size_y: i32,
    local_size_z: i32,
}

impl<'a> FinalizationVisitor<'a> {
    fn new(kind: ProgramKind, errors: &'a mut ErrorReporter, usage: &'a WriteCounts) -> Self {
        Self {
            kind,
            errors,
            usage,
            global_slots_used: 0,
            bindings: HashSet::new(),
            local_size_x: -1,
            local_size_y: -1,
            local_size_z: -1,
        }
    }

    // Port of: src/sksl/analysis/SkSLFinalizationChecks.cpp#L74-L89 (chrome/m156)
    fn check_global_variable_size_limit(&mut self, pool: &IrPool, global: &GlobalVarDeclaration) {
        if !ProgramConfig::is_runtime_effect(self.kind) {
            return;
        }
        let StatementKind::VarDeclaration(decl) = &pool.statement(global.declaration).kind else {
            unreachable!("GlobalVarDeclaration holds a VarDeclaration");
        };
        let decl_position = pool.statement(global.declaration).position;
        let var = pool.variable(decl.var);
        let slot_limit = usize::try_from(VARIABLE_SLOT_LIMIT).unwrap_or(usize::MAX);

        let prev_slots_used = self.global_slots_used;
        self.global_slots_used =
            saturating_add_size(self.global_slots_used, pool.ty(var.ty).slot_count());
        // To avoid overzealous error reporting, only trigger the error at the first place where
        // the global limit is exceeded.
        if prev_slots_used < slot_limit && self.global_slots_used >= slot_limit {
            self.errors.error(
                decl_position,
                &format!("global variable '{}' exceeds the size limit", var.name),
            );
        }
    }

    // Port of: src/sksl/analysis/SkSLFinalizationChecks.cpp#L91-L115 (chrome/m156)
    // The sign-extending casts mirror the C++ unsigned conversions of the packed key.
    #[allow(clippy::cast_sign_loss)]
    fn check_bind_uniqueness(&mut self, pool: &IrPool, position: Position, block: &InterfaceBlock) {
        let var = pool.variable(block.var);
        let set = var.layout.set;
        let binding = var.layout.binding;
        if binding == -1 {
            return;
        }
        // `((uint64_t)set << 32) + binding`: both operands sign-extend to 64 bits.
        let key = (i64::from(set) as u64)
            .wrapping_shl(32)
            .wrapping_add(i64::from(binding) as u64);
        if self.bindings.insert(key) {
            return;
        }
        if set == -1 {
            self.errors.error(
                position,
                &format!("layout(binding={binding}) has already been defined"),
            );
        } else {
            self.errors.error(
                position,
                &format!("layout(set={set}, binding={binding}) has already been defined"),
            );
        }
    }

    // Port of: src/sksl/analysis/SkSLFinalizationChecks.cpp#L117-L135 (chrome/m156)
    fn check_out_params_are_assigned(&mut self, pool: &IrPool, func_def: &FunctionDefinition) {
        let func_decl = pool.function(func_def.declaration);
        // Searches for `out` parameters that are not written to. According to the GLSL spec, the
        // value of an out-param that's never assigned to is unspecified, so report it.
        for &param in &func_decl.parameters {
            let var = pool.variable(param);
            let param_inout = var.modifier_flags & (ModifierFlags::IN | ModifierFlags::OUT);
            if param_inout == ModifierFlags::OUT && self.usage.writes(param) <= 0 {
                self.errors.error(
                    var.position,
                    &format!(
                        "function '{}' never assigns a value to out parameter '{}'",
                        func_decl.name, var.name
                    ),
                );
            }
        }
    }

    // Port of: src/sksl/analysis/SkSLFinalizationChecks.cpp#L137-L159 (chrome/m156)
    fn check_workgroup_local_size(&mut self, position: Position, d: &ModifiersDeclaration) {
        if d.layout.local_size_x >= 0 {
            if self.local_size_x >= 0 {
                self.errors
                    .error(position, "'local_size_x' was specified more than once");
            } else {
                self.local_size_x = d.layout.local_size_x;
            }
        }
        if d.layout.local_size_y >= 0 {
            if self.local_size_y >= 0 {
                self.errors
                    .error(position, "'local_size_y' was specified more than once");
            } else {
                self.local_size_y = d.layout.local_size_y;
            }
        }
        if d.layout.local_size_z >= 0 {
            if self.local_size_z >= 0 {
                self.errors
                    .error(position, "'local_size_z' was specified more than once");
            } else {
                self.local_size_z = d.layout.local_size_z;
            }
        }
    }

    fn defines_local_size(&self) -> bool {
        self.local_size_x >= 0 || self.local_size_y >= 0 || self.local_size_z >= 0
    }
}

impl ProgramVisitor for FinalizationVisitor<'_> {
    // Port of: src/sksl/analysis/SkSLFinalizationChecks.cpp#L52-L72 (chrome/m156)
    fn visit_program_element(&mut self, pool: &IrPool, element: ElemId) -> bool {
        let elem = pool.element(element);
        match &elem.kind {
            ProgramElementKind::GlobalVar(global) => {
                self.check_global_variable_size_limit(pool, global);
            }
            // TODO(skbug.com/40044753) in Skia: enforce duplicate checks universally.
            ProgramElementKind::InterfaceBlock(block) => {
                self.check_bind_uniqueness(pool, elem.position, block);
            }
            ProgramElementKind::Function(func_def) => {
                self.check_out_params_are_assigned(pool, func_def);
            }
            ProgramElementKind::Modifiers(modifiers) => {
                self.check_workgroup_local_size(elem.position, modifiers);
            }
            _ => {}
        }
        walk_program_element(self, pool, element)
    }

    // Port of: src/sksl/analysis/SkSLFinalizationChecks.cpp#L161-L184 (chrome/m156)
    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        let e = pool.expression(expr);
        match &e.kind {
            ExpressionKind::FunctionCall(call) => {
                let decl = pool.function(call.function);
                if !decl.is_builtin() && decl.definition.is_none() {
                    self.errors.error(
                        e.position,
                        &format!("function '{}' is not defined", decl.description(pool)),
                    );
                }
            }
            ExpressionKind::FunctionReference(_)
            | ExpressionKind::MethodReference(_)
            | ExpressionKind::TypeReference(_) => {
                // Skia: SkDEBUGFAIL, because coerce() should have reported these already.
                debug_assert!(
                    false,
                    "invalid reference-expr, should have been reported by coerce()"
                );
                self.errors.error(e.position, "invalid expression");
            }
            _ => {
                if pool.ty(e.ty).matches(TypeId::INVALID) {
                    self.errors.error(e.position, "invalid expression");
                }
            }
        }
        walk_expression(self, pool, expr)
    }
}

/// `Analysis::DoFinalizationChecks`: reports dangling references, uncalled functions, duplicate
/// bindings and local sizes, `out` parameters that are never written, and missing workgroup
/// sizes. `usage` holds the write counts of the program (Skia's `program.usage()`), and
/// `owned_elements` are the program's own elements; built-in elements are assumed valid.
// Port of: src/sksl/analysis/SkSLFinalizationChecks.cpp#L207-L217 (chrome/m156)
pub fn do_finalization_checks(ctx: &mut Context, usage: &WriteCounts, owned_elements: &[ElemId]) {
    let kind = ctx.config().kind;
    let pool: &IrPool = &ctx.pool;
    let errors = &mut ctx.errors;
    let defines_local_size = {
        let mut visitor = FinalizationVisitor::new(kind, &mut *errors, usage);
        for &element in owned_elements {
            visitor.visit_program_element(pool, element);
        }
        visitor.defines_local_size()
    };
    if ProgramConfig::is_compute(kind) && !defines_local_size {
        errors.error(
            Position::default(),
            "compute programs must specify a workgroup size",
        );
    }
}
