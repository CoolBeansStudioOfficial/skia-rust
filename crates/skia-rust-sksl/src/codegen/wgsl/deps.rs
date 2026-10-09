// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp (`FunctionDependencyResolver` and
// the program-requirement helpers).

//! Which functions need the pipeline stage input and output parameters passed in.

use std::collections::HashMap;
use std::ops::BitOrAssign;

use crate::analysis::{ProgramVisitor, walk_expression};
use crate::ir::{
    ElemId, ExprId, ExpressionKind, FnId, IrPool, ModifierFlags, ProgramElementKind, VarId,
    VariableStorage,
};

/// `WGSLFunctionDependency`: a function's dependencies that are not accessible in global scope.
/// For instance, pipeline stage input and output parameters must be passed in as an argument.
/// Skia's `WGSLFunctionDependencies` is a bitmask of these.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Deps(u8);

impl Deps {
    /// `kNone`.
    pub(super) const NONE: Self = Self(0);
    /// `kPipelineInputs`.
    pub(super) const PIPELINE_INPUTS: Self = Self(1 << 0);
    /// `kPipelineOutputs`.
    pub(super) const PIPELINE_OUTPUTS: Self = Self(1 << 1);

    /// True if every bit of `other` is set (and, for the empty mask, any value).
    pub(super) fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// True if no dependency is set.
    pub(super) fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl BitOrAssign for Deps {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// `ProgramRequirements::DepsMap`.
pub(super) type DepsMap = HashMap<FnId, Deps>;

/// `WGSLCodeGenerator::ProgramRequirements`.
#[derive(Debug, Default)]
pub(super) struct ProgramRequirements {
    /// Mappings used to synthesize function parameters according to dependencies on pipeline
    /// input/output variables. Skia's `THashMap` is only looked up, never iterated.
    pub(super) dependencies: DepsMap,
    /// These flags track extensions that will need to be enabled.
    pub(super) pixel_local_extension: bool,
}

/// `FunctionDependencyResolver`: visits the IR tree rooted at a particular function definition and
/// computes that function's dependencies on pipeline stage IO parameters. These are later used to
/// synthesize arguments when writing out function definitions.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1058-L1147 (chrome/m156)
struct FunctionDependencyResolver<'a> {
    elements: &'a [ElemId],
    function: FnId,
    dependency_map: &'a mut DepsMap,
    deps: Deps,
}

impl<'a> FunctionDependencyResolver<'a> {
    fn new(elements: &'a [ElemId], function: FnId, dependency_map: &'a mut DepsMap) -> Self {
        Self {
            elements,
            function,
            dependency_map,
            deps: Deps::NONE,
        }
    }

    fn resolve(&mut self, pool: &IrPool) -> Deps {
        self.deps = Deps::NONE;
        // `this->visit(*fProgram)`.
        let elements = self.elements;
        for &e in elements {
            if self.visit_program_element(pool, e) {
                break;
            }
        }
        self.deps
    }
}

impl ProgramVisitor for FunctionDependencyResolver<'_> {
    fn visit_program_element(&mut self, pool: &IrPool, p: ElemId) -> bool {
        // Only visit the program that matches the requested function.
        if let ProgramElementKind::Function(f) = &pool.element(p).kind
            && f.declaration == self.function
        {
            return crate::analysis::walk_program_element(self, pool, p);
        }
        // Continue visiting other program elements.
        false
    }

    fn visit_expression(&mut self, pool: &IrPool, e: ExprId) -> bool {
        match &pool.expression(e).kind {
            ExpressionKind::VariableReference(v) => {
                let variable = pool.variable(v.variable);
                if variable.storage == VariableStorage::Global {
                    let flags = variable.modifier_flags;
                    if flags.contains(ModifierFlags::IN) {
                        self.deps |= Deps::PIPELINE_INPUTS;
                    }
                    if flags.contains(ModifierFlags::OUT) {
                        self.deps |= Deps::PIPELINE_OUTPUTS;
                    }
                }
            }
            ExpressionKind::FunctionCall(callee) => {
                // The current function that we're processing (`fFunction`) inherits the
                // dependencies of functions that it makes calls to, because the pipeline stage IO
                // parameters need to be passed down as an argument.
                let callee = callee.function;

                // Don't process a function again if we have already resolved it.
                if let Some(found) = self.dependency_map.get(&callee) {
                    self.deps |= *found;
                } else {
                    // Store the dependencies that have been discovered for the current function so
                    // far. If `callee` directly or indirectly calls the current function, then
                    // this value will prevent an infinite recursion.
                    self.dependency_map.insert(self.function, self.deps);

                    // Separately traverse the called function's definition and determine its
                    // dependencies.
                    let callee_deps = {
                        let mut resolver = FunctionDependencyResolver::new(
                            self.elements,
                            callee,
                            &mut *self.dependency_map,
                        );
                        resolver.resolve(pool)
                    };

                    // Store the callee's dependencies in the global map to avoid processing the
                    // function again for future calls.
                    self.dependency_map.insert(callee, callee_deps);

                    // Add to the current function's dependencies.
                    self.deps |= callee_deps;
                }
            }
            _ => {}
        }
        walk_expression(self, pool, e)
    }
}

/// `resolve_program_requirements`.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1149-L1174 (chrome/m156)
pub(super) fn resolve_program_requirements(
    pool: &IrPool,
    elements: &[ElemId],
) -> ProgramRequirements {
    let mut requirements = ProgramRequirements::default();

    for &e in elements {
        match &pool.element(e).kind {
            ProgramElementKind::Function(f) => {
                let decl = f.declaration;
                let deps = {
                    let mut resolver = FunctionDependencyResolver::new(
                        elements,
                        decl,
                        &mut requirements.dependencies,
                    );
                    resolver.resolve(pool)
                };
                requirements.dependencies.insert(decl, deps);
            }
            ProgramElementKind::GlobalVar(g) => {
                let decl = g.var_declaration(pool);
                if pool.variable(decl.var).modifier_flags.is_pixel_local() {
                    requirements.pixel_local_extension = true;
                }
            }
            _ => {}
        }
    }

    requirements
}

/// `collect_pipeline_io_vars`.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1176-L1192 (chrome/m156)
pub(super) fn collect_pipeline_io_vars(
    pool: &IrPool,
    elements: &[ElemId],
    io_type: ModifierFlags,
) -> Vec<VarId> {
    let mut io_vars = Vec::new();
    for &e in elements {
        match &pool.element(e).kind {
            ProgramElementKind::GlobalVar(g) => {
                let v = g.var_declaration(pool).var;
                if pool.variable(v).modifier_flags.intersects(io_type) {
                    io_vars.push(v);
                }
            }
            ProgramElementKind::InterfaceBlock(ib) => {
                let v = ib.var;
                if pool.variable(v).modifier_flags.intersects(io_type) {
                    io_vars.push(v);
                }
            }
            _ => {}
        }
    }
    io_vars
}
