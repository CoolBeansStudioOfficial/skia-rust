// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/private/SkSLSampleUsage.h, src/sksl/SkSLSampleUsage.cpp, and the
// sample-usage queries of src/sksl/SkSLAnalysis.cpp.

//! How a program samples its children, and which builtins and intrinsics it references.

use super::{ProgramUsage, ProgramVisitor, walk_expression, walk_program_element};
use crate::intrinsic_list::IntrinsicKind;
use crate::ir::{
    ElemId, ExprId, ExpressionKind, IrPool, Program, ProgramElementKind, TypeId, VarId,
};

/// `SkSL::SampleUsage::Kind`. The order is significant: [`SampleUsage::merge`] keeps the greater
/// kind, and Skia's `static_assert`s require `kExplicit > kPassThrough > kNone`.
#[doc(alias = "SkSL::SampleUsage::Kind")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SampleUsageKind {
    /// Child is never sampled.
    #[default]
    None,
    /// Child is only sampled at the same coordinates as the parent.
    PassThrough,
    /// Child is sampled with a matrix whose value is uniform.
    UniformMatrix,
    /// Child is sampled with `sk_FragCoord.xy`.
    FragCoord,
    /// Child is sampled using explicit coordinates.
    Explicit,
}

/// `SkSL::SampleUsage`: every way a fragment processor is sampled by its parent.
// Port of: include/private/SkSLSampleUsage.h (chrome/m156)
#[doc(alias = "SkSL::SampleUsage")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SampleUsage {
    kind: SampleUsageKind,
    // Only valid if `kind` is `UniformMatrix`.
    has_perspective: bool,
}

impl SampleUsage {
    /// `SampleUsage(Kind, bool hasPerspective)`.
    #[must_use]
    pub fn new(kind: SampleUsageKind, has_perspective: bool) -> Self {
        debug_assert!(kind == SampleUsageKind::UniformMatrix || !has_perspective);
        Self {
            kind,
            has_perspective,
        }
    }

    /// `SampleUsage::UniformMatrix(hasPerspective)`: the child is sampled with a matrix whose
    /// value is uniform. The name is fixed.
    #[must_use]
    pub fn uniform_matrix(has_perspective: bool) -> Self {
        Self::new(SampleUsageKind::UniformMatrix, has_perspective)
    }

    /// `SampleUsage::Explicit()`.
    #[must_use]
    pub fn explicit() -> Self {
        Self::new(SampleUsageKind::Explicit, false)
    }

    /// `SampleUsage::PassThrough()`.
    #[must_use]
    pub fn pass_through() -> Self {
        Self::new(SampleUsageKind::PassThrough, false)
    }

    /// `SampleUsage::FragCoord()`.
    #[must_use]
    pub fn frag_coord() -> Self {
        Self::new(SampleUsageKind::FragCoord, false)
    }

    /// `MatrixUniformName()`: the arbitrary name used by all uniform sampling matrices.
    #[must_use]
    pub fn matrix_uniform_name() -> &'static str {
        "matrix"
    }

    /// `merge(other)`: keeps the greater of the two kinds. Skia also returns the merged value,
    /// which none of its callers read, so it is not returned here.
    // Port of: src/sksl/SkSLSampleUsage.cpp#L14-L24 (chrome/m156)
    pub fn merge(&mut self, other: &SampleUsage) {
        // This is only used in the merge of MergeSampleUsageVisitor to determine the combined
        // SampleUsage for a child fp/shader/etc. We should never see matrix sampling here.
        debug_assert!(
            self.kind != SampleUsageKind::UniformMatrix
                && other.kind != SampleUsageKind::UniformMatrix
        );
        self.kind = self.kind.max(other.kind);
    }

    /// `kind()`.
    #[must_use]
    pub fn kind(&self) -> SampleUsageKind {
        self.kind
    }

    /// `hasPerspective()`.
    #[must_use]
    pub fn has_perspective(&self) -> bool {
        self.has_perspective
    }

    /// `isSampled()`.
    #[must_use]
    pub fn is_sampled(&self) -> bool {
        self.kind != SampleUsageKind::None
    }

    /// `isPassThrough()`.
    #[must_use]
    pub fn is_pass_through(&self) -> bool {
        self.kind == SampleUsageKind::PassThrough
    }

    /// `isExplicit()`.
    #[must_use]
    pub fn is_explicit(&self) -> bool {
        self.kind == SampleUsageKind::Explicit
    }

    /// `isUniformMatrix()`.
    #[must_use]
    pub fn is_uniform_matrix(&self) -> bool {
        self.kind == SampleUsageKind::UniformMatrix
    }

    /// `isFragCoord()`.
    #[must_use]
    pub fn is_frag_coord(&self) -> bool {
        self.kind == SampleUsageKind::FragCoord
    }
}

/// `SK_FRAGCOORD_BUILTIN` (`SkSLCompiler.h`): the builtin number of `sk_FragCoord`. It lives in
/// `compiler.rs` in Skia's layout; S11 owns that file, so the constant is kept here.
// Port of: src/sksl/SkSLCompiler.h#L28 (chrome/m156)
pub(crate) const SK_FRAGCOORD_BUILTIN: i32 = 15;

/// `MergeSampleUsageVisitor`: the merged [`SampleUsage`] of one child over a program.
// Port of: src/sksl/SkSLAnalysis.cpp#L70-L151 (chrome/m156)
struct MergeSampleUsageVisitor {
    child: VarId,
    main_coords_param: Option<VarId>,
    writes_to_sample_coords: bool,
    usage: SampleUsage,
    elided_sample_coord_count: i32,
}

impl ProgramVisitor for MergeSampleUsageVisitor {
    fn visit_program_element(&mut self, pool: &IrPool, element: ElemId) -> bool {
        self.main_coords_param = match &pool.element(element).kind {
            ProgramElementKind::Function(def) => {
                pool.function(def.declaration).main_coords_parameter()
            }
            _ => None,
        };
        walk_program_element(self, pool, element)
    }

    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        match &pool.expression(expr).kind {
            ExpressionKind::ChildCall(call) if call.child == self.child => {
                // Determine the type of call at this site, and merge it with the accumulated state.
                let maybe_coords = pool.expression(call.arguments[0]);
                if pool.ty(maybe_coords.ty).matches(TypeId::FLOAT2) {
                    // If the coords are a direct reference to the program's sample-coords, and
                    // those coords are never modified, we can conservatively turn this into
                    // PassThrough sampling. In all other cases, we consider it Explicit.
                    let is_main_coords = matches!(
                        &maybe_coords.kind,
                        ExpressionKind::VariableReference(reference)
                            if Some(reference.variable) == self.main_coords_param
                    );
                    if !self.writes_to_sample_coords && is_main_coords {
                        self.usage.merge(&SampleUsage::pass_through());
                        self.elided_sample_coord_count += 1;
                    } else {
                        self.usage.merge(&SampleUsage::explicit());
                    }
                } else {
                    // child(inputColor) or child(srcColor, dstColor) -> PassThrough
                    self.usage.merge(&SampleUsage::pass_through());
                }
            }
            ExpressionKind::FunctionCall(call) => {
                // If this child effect is ever passed via a function call...
                let passes_child = call.arguments.iter().any(|&arg| {
                    matches!(
                        &pool.expression(arg).kind,
                        ExpressionKind::VariableReference(reference) if reference.variable == self.child
                    )
                });
                if passes_child {
                    // ... we must treat it as explicitly sampled, since the program's sample-coords
                    // only exist as a parameter to `main`.
                    self.usage.merge(&SampleUsage::explicit());
                }
            }
            _ => {}
        }
        walk_expression(self, pool, expr)
    }
}

/// `Analysis::GetSampleUsage`: how `program` samples `child`. By default the sample coordinates
/// might be modified, so `child.eval(sampleCoords)` is treated as Explicit. With
/// `writes_to_sample_coords` false, it is treated as `PassThrough`. `elided_sample_coord_count`,
/// when given, is incremented by the number of sample calls rewritten that way.
// Port of: src/sksl/SkSLAnalysis.cpp#L346-L356 (chrome/m156)
#[must_use]
pub fn get_sample_usage(
    program: &Program,
    child: VarId,
    writes_to_sample_coords: bool,
    elided_sample_coord_count: Option<&mut i32>,
) -> SampleUsage {
    let mut visitor = MergeSampleUsageVisitor {
        child,
        main_coords_param: None,
        writes_to_sample_coords,
        usage: SampleUsage::default(),
        elided_sample_coord_count: 0,
    };
    visitor.visit(program);
    if let Some(count) = elided_sample_coord_count {
        *count += visitor.elided_sample_coord_count;
    }
    visitor.usage
}

/// `Analysis::ReferencesBuiltin`: true if `program` reads a variable with builtin number
/// `builtin`.
// Port of: src/sksl/SkSLAnalysis.cpp#L358-L366 (chrome/m156)
#[must_use]
pub fn references_builtin(program: &Program, usage: &ProgramUsage, builtin: i32) -> bool {
    // Order does not matter: this is an `any` over the table.
    usage.variable_counts.iter().any(|(&var, counts)| {
        counts.read > 0 && program.pool.variable(var).layout.builtin == builtin
    })
}

/// `Analysis::ReferencesSampleCoords`: true if `main` has a coordinate parameter that is read.
// Port of: src/sksl/SkSLAnalysis.cpp#L368-L384 (chrome/m156)
#[must_use]
pub fn references_sample_coords(program: &Program, usage: &ProgramUsage) -> bool {
    // Look for main().
    for &element in &program.owned_elements {
        if let ProgramElementKind::Function(def) = &program.pool.element(element).kind {
            let func = program.pool.function(def.declaration);
            if func.is_main {
                // See if main() has a coords parameter that is read from anywhere.
                if let Some(coords) = func.main_coords_parameter() {
                    return usage.get_variable(coords).read > 0;
                }
            }
        }
    }
    // The program is missing a main().
    false
}

/// `Analysis::ReferencesFragCoords`.
// Port of: src/sksl/SkSLAnalysis.cpp#L386-L388 (chrome/m156)
#[must_use]
pub fn references_frag_coords(program: &Program, usage: &ProgramUsage) -> bool {
    references_builtin(program, usage, SK_FRAGCOORD_BUILTIN)
}

/// `SampleOutsideMainVisitor`: finds a child call in any function other than `main`.
// Port of: src/sksl/SkSLAnalysis.cpp#L153-L171 (chrome/m156)
struct SampleOutsideMainVisitor;

impl ProgramVisitor for SampleOutsideMainVisitor {
    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        if matches!(pool.expression(expr).kind, ExpressionKind::ChildCall(_)) {
            return true;
        }
        walk_expression(self, pool, expr)
    }

    fn visit_program_element(&mut self, pool: &IrPool, element: ElemId) -> bool {
        match &pool.element(element).kind {
            ProgramElementKind::Function(def) if !pool.function(def.declaration).is_main => {
                walk_program_element(self, pool, element)
            }
            _ => false,
        }
    }
}

/// `Analysis::CallsSampleOutsideMain`: true if a function other than `main` calls a child.
// Port of: src/sksl/SkSLAnalysis.cpp#L390-L393 (chrome/m156)
#[must_use]
pub fn calls_sample_outside_main(program: &Program) -> bool {
    SampleOutsideMainVisitor.visit(program)
}

/// `Analysis::CallsColorTransformIntrinsics`: true if `program` calls `toLinearSrgb` or
/// `fromLinearSrgb`.
// Port of: src/sksl/SkSLAnalysis.cpp#L395-L404 (chrome/m156)
#[must_use]
pub fn calls_color_transform_intrinsics(program: &Program, usage: &ProgramUsage) -> bool {
    // Order does not matter: this is an `any` over the call table.
    usage.call_counts.iter().any(|(&func, &count)| {
        let decl = program.pool.function(func);
        count != 0
            && matches!(
                decl.intrinsic_kind,
                Some(IntrinsicKind::ToLinearSrgb | IntrinsicKind::FromLinearSrgb)
            )
    })
}
