// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLExtension.{h,cpp} (`Convert` and `Make`).

//! The factories of [`Extension`].

use super::{Extension, ProgramElement, ProgramElementKind, ids::ElemId};
use crate::context::Context;
use crate::position::Position;
use crate::program_settings::ProgramConfig;

impl Extension {
    /// `Extension::Convert`: `#extension name : behavior`. `disable` is accepted and does nothing,
    /// so it produces no element.
    // Port of: src/sksl/ir/SkSLExtension.cpp#L14-L35 (chrome/m156)
    #[must_use]
    pub fn convert(
        ctx: &mut Context,
        pos: Position,
        name: &str,
        behavior_text: &str,
    ) -> Option<ElemId> {
        if ProgramConfig::is_runtime_effect(ctx.config().kind) {
            // Runtime Effects do not allow any #extensions.
            ctx.errors.error(pos, "unsupported directive '#extension'");
            return None;
        }
        if behavior_text == "disable" {
            // We allow `#extension <name> : disable`, but it is a no-op.
            return None;
        }
        if behavior_text != "require" && behavior_text != "enable" && behavior_text != "warn" {
            ctx.errors
                .error(pos, "expected 'require', 'enable', 'warn', or 'disable'");
            return None;
        }
        // We don't currently do anything different between `require`, `enable`, and `warn`.
        Some(Self::make(ctx, pos, name))
    }

    /// `Extension::Make`: the caller has checked that this is not a runtime effect.
    // Port of: src/sksl/ir/SkSLExtension.cpp#L37-L42 (chrome/m156)
    #[must_use]
    pub fn make(ctx: &mut Context, pos: Position, name: &str) -> ElemId {
        debug_assert!(!ProgramConfig::is_runtime_effect(ctx.config().kind));
        ctx.pool.add_element(ProgramElement::new(
            pos,
            ProgramElementKind::Extension(Self { name: name.into() }),
        ))
    }
}
