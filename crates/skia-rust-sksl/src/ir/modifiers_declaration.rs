// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLModifiersDeclaration.{h,cpp} (`Convert` and `Make`).

//! The factories of [`ModifiersDeclaration`].

use super::{
    ModifierFlags, Modifiers, ModifiersDeclaration, ProgramElement, ProgramElementKind, ids::ElemId,
};
use crate::context::Context;
use crate::program_settings::ProgramConfig;

impl ModifiersDeclaration {
    /// `ModifiersDeclaration::Convert`: a declaration of layout qualifiers only, such as
    /// `layout(local_size_x = 8) in;`. Allowed in fragment, vertex and compute programs.
    // Port of: src/sksl/ir/SkSLModifiersDeclaration.cpp#L16-L51 (chrome/m156)
    #[must_use]
    pub fn convert(ctx: &mut Context, modifiers: &Modifiers) -> Option<ElemId> {
        let kind = ctx.config().kind;
        if !ProgramConfig::is_fragment(kind)
            && !ProgramConfig::is_vertex(kind)
            && !ProgramConfig::is_compute(kind)
        {
            ctx.errors.error(
                modifiers.position,
                "layout qualifiers are not allowed in this kind of program",
            );
            return None;
        }
        let layout = &modifiers.layout;
        if layout.local_size_x >= 0 || layout.local_size_y >= 0 || layout.local_size_z >= 0 {
            if layout.local_size_x == 0 || layout.local_size_y == 0 || layout.local_size_z == 0 {
                ctx.errors
                    .error(modifiers.position, "local size qualifiers cannot be zero");
                return None;
            }
            if !ProgramConfig::is_compute(kind) {
                ctx.errors.error(
                    modifiers.position,
                    "local size layout qualifiers are only allowed in a compute program",
                );
                return None;
            }
            if modifiers.flags != ModifierFlags::IN {
                ctx.errors.error(
                    modifiers.position,
                    "local size layout qualifiers must be defined using an 'in' declaration",
                );
                return None;
            }
        }
        Some(Self::make(ctx, modifiers))
    }

    /// `ModifiersDeclaration::Make`: the caller has checked the program kind.
    // Port of: src/sksl/ir/SkSLModifiersDeclaration.cpp#L53-L61 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, modifiers: &Modifiers) -> ElemId {
        let kind = ctx.config().kind;
        debug_assert!(
            ProgramConfig::is_fragment(kind)
                || ProgramConfig::is_vertex(kind)
                || ProgramConfig::is_compute(kind)
        );
        ctx.pool.add_element(ProgramElement::new(
            modifiers.position,
            ProgramElementKind::Modifiers(Self {
                layout: modifiers.layout,
                flags: modifiers.flags,
            }),
        ))
    }
}
