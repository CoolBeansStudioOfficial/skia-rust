// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLModule.h (`ModuleType`, `SKSL_MODULE_LIST`) and
// src/sksl/SkSLModuleDataDefault.cpp (`GetModuleData`).

//! The embedded texts of Skia's built-in `SkSL` modules, in both variants (`docs/design/sksl.md`
//! §3). The files under `src/modules/` are copies of the pinned tree, written by
//! `cargo xtask sksl sync-modules`, and `cargo xtask sksl sync-modules --check` fails when they
//! drift from it.

use std::sync::Arc;

use crate::flavor::ModuleSource;
use crate::ir::{ElemId, IrPool, SymTabId};

/// `SkSL::ModuleType`: `program` (code that is not in a module), `unknown`, then the built-in
/// modules in `SKSL_MODULE_LIST` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModuleType {
    /// `program`: the code is not in a module at all.
    Program,
    /// `unknown`: the code is in a module outside of `SKSL_MODULE_LIST`.
    Unknown,
    /// `sksl_shared`: root intrinsics (`genType`, `sin`, …).
    SkslShared,
    /// `sksl_compute`: compute program kind.
    SkslCompute,
    /// `sksl_frag`: fragment program kind.
    SkslFrag,
    /// `sksl_gpu`: GPU-only intrinsics.
    SkslGpu,
    /// `sksl_public`: the public runtime-effect intrinsics (`toLinearSrgb`, `$eval`, …).
    SkslPublic,
    /// `sksl_rt_shader`: runtime-shader helpers (`sk_luma`, `sk_decal`, …).
    SkslRtShader,
    /// `sksl_vert`: vertex program kind.
    SkslVert,
    /// `sksl_graphite_frag`: Graphite's fragment modules, loaded only on request.
    SkslGraphiteFrag,
    /// `sksl_graphite_vert`: Graphite's vertex modules, loaded only on request.
    SkslGraphiteVert,
}

impl ModuleType {
    /// Every module, in `SKSL_MODULE_LIST` order.
    pub const ALL: [Self; 9] = [
        Self::SkslShared,
        Self::SkslCompute,
        Self::SkslFrag,
        Self::SkslGpu,
        Self::SkslPublic,
        Self::SkslRtShader,
        Self::SkslVert,
        Self::SkslGraphiteFrag,
        Self::SkslGraphiteVert,
    ];

    /// The Skia name of the module (`sksl_shared`, …), which is also its file stem
    /// (`ModuleTypeToString`, which names `program` and `unknown` both "unknown").
    #[doc(alias = "ModuleTypeToString")]
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Program | Self::Unknown => "unknown",
            Self::SkslShared => "sksl_shared",
            Self::SkslCompute => "sksl_compute",
            Self::SkslFrag => "sksl_frag",
            Self::SkslGpu => "sksl_gpu",
            Self::SkslPublic => "sksl_public",
            Self::SkslRtShader => "sksl_rt_shader",
            Self::SkslVert => "sksl_vert",
            Self::SkslGraphiteFrag => "sksl_graphite_frag",
            Self::SkslGraphiteVert => "sksl_graphite_vert",
        }
    }

    /// Whether the module belongs to Graphite (`SkSLGraphiteModules`): compiled in, but loaded
    /// only when a Graphite backend asks for it.
    #[must_use]
    pub fn is_graphite(self) -> bool {
        matches!(self, Self::SkslGraphiteFrag | Self::SkslGraphiteVert)
    }

    /// The module's source text in the given variant. Every module is non-empty.
    ///
    /// # Panics
    ///
    /// For [`ModuleType::Program`] and [`ModuleType::Unknown`], which have no module text.
    #[must_use]
    pub fn text(self, source: ModuleSource) -> &'static str {
        match (source, self) {
            (_, Self::Program | Self::Unknown) => {
                panic!("ModuleType::{self:?} has no module text")
            }
            (ModuleSource::Minified, Self::SkslShared) => {
                include_str!("modules/minified/sksl_shared.sksl")
            }
            (ModuleSource::Minified, Self::SkslCompute) => {
                include_str!("modules/minified/sksl_compute.sksl")
            }
            (ModuleSource::Minified, Self::SkslFrag) => {
                include_str!("modules/minified/sksl_frag.sksl")
            }
            (ModuleSource::Minified, Self::SkslGpu) => {
                include_str!("modules/minified/sksl_gpu.sksl")
            }
            (ModuleSource::Minified, Self::SkslPublic) => {
                include_str!("modules/minified/sksl_public.sksl")
            }
            (ModuleSource::Minified, Self::SkslRtShader) => {
                include_str!("modules/minified/sksl_rt_shader.sksl")
            }
            (ModuleSource::Minified, Self::SkslVert) => {
                include_str!("modules/minified/sksl_vert.sksl")
            }
            (ModuleSource::Minified, Self::SkslGraphiteFrag) => {
                include_str!("modules/minified/sksl_graphite_frag.sksl")
            }
            (ModuleSource::Minified, Self::SkslGraphiteVert) => {
                include_str!("modules/minified/sksl_graphite_vert.sksl")
            }
            (ModuleSource::Original, Self::SkslShared) => {
                include_str!("modules/original/sksl_shared.sksl")
            }
            (ModuleSource::Original, Self::SkslCompute) => {
                include_str!("modules/original/sksl_compute.sksl")
            }
            (ModuleSource::Original, Self::SkslFrag) => {
                include_str!("modules/original/sksl_frag.sksl")
            }
            (ModuleSource::Original, Self::SkslGpu) => {
                include_str!("modules/original/sksl_gpu.sksl")
            }
            (ModuleSource::Original, Self::SkslPublic) => {
                include_str!("modules/original/sksl_public.sksl")
            }
            (ModuleSource::Original, Self::SkslRtShader) => {
                include_str!("modules/original/sksl_rt_shader.sksl")
            }
            (ModuleSource::Original, Self::SkslVert) => {
                include_str!("modules/original/sksl_vert.sksl")
            }
            (ModuleSource::Original, Self::SkslGraphiteFrag) => {
                include_str!("modules/original/sksl_graphite_frag.sksl")
            }
            (ModuleSource::Original, Self::SkslGraphiteVert) => {
                include_str!("modules/original/sksl_graphite_vert.sksl")
            }
        }
    }
}

/// `SkSL::Module`: a compiled built-in module. Its IR lives in a frozen pool that extends its
/// parent module's pool, and programs compiled against it extend that pool in turn
/// (`docs/design/sksl.md` §4.1).
// Port of: src/sksl/SkSLModule.h#L37-L42 (chrome/m156)
#[doc(alias = "SkSL::Module")]
#[derive(Debug)]
pub struct Module {
    /// `fParent`: the module this one inherits symbols from.
    pub parent: Option<Arc<Module>>,
    /// The module's frozen IR. Its parent pool is `parent`'s pool.
    pub pool: Arc<IrPool>,
    /// `fSymbols`: the module's symbol table (in `pool`); its parent is `parent`'s table.
    pub symbols: SymTabId,
    /// `fElements`: the module's program elements (in `pool`), in source order.
    pub elements: Vec<ElemId>,
    /// `fModuleType`.
    pub module_type: ModuleType,
    /// The source text the module was compiled from (Skia: `takeOwnershipOfString`). It is kept
    /// alive with the module, as the module's positions index into it.
    pub source: Arc<[u8]>,
}

#[cfg(test)]
mod tests {
    use super::ModuleType;
    use crate::flavor::ModuleSource;

    #[test]
    fn every_module_has_text_in_both_variants() {
        for module in ModuleType::ALL {
            assert!(
                !module.text(ModuleSource::Minified).is_empty(),
                "{}",
                module.name()
            );
            assert!(
                !module.text(ModuleSource::Original).is_empty(),
                "{}",
                module.name()
            );
        }
    }

    #[test]
    fn the_two_variants_differ() {
        // The minified text drops comments and renames parameters (`s` becomes `a`).
        let original = ModuleType::SkslPublic.text(ModuleSource::Original);
        let minified = ModuleType::SkslPublic.text(ModuleSource::Minified);
        assert!(original.contains("toLinearSrgb(half3 color)"));
        assert!(minified.contains("toLinearSrgb(half3);"));
    }

    #[test]
    fn graphite_modules_are_the_last_two() {
        let graphite: Vec<_> = ModuleType::ALL
            .iter()
            .filter(|m| m.is_graphite())
            .map(|m| m.name())
            .collect();
        assert_eq!(graphite, ["sksl_graphite_frag", "sksl_graphite_vert"]);
    }
}
