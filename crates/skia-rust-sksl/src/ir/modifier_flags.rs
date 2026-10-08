// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLModifierFlags.{h,cpp} and src/sksl/ir/SkSLModifiers.h.
// `checkPermittedFlags` comes with the type system (task S6).

//! [`ModifierFlags`] (`const`, `uniform`, `in`, `$pure`, …) and [`Modifiers`].

use super::Layout;
use crate::position::Position;

bitflags::bitflags! {
    /// `SkSL::ModifierFlag` / `ModifierFlags`.
    #[doc(alias = "SkSL::ModifierFlag")]
    #[doc(alias = "SkSL::ModifierFlags")]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct ModifierFlags: i32 {
        // Real GLSL modifiers
        const FLAT = 1 << 0;
        const NO_PERSPECTIVE = 1 << 1;
        const CONST = 1 << 2;
        const UNIFORM = 1 << 3;
        const IN = 1 << 4;
        const OUT = 1 << 5;
        const HIGHP = 1 << 6;
        const MEDIUMP = 1 << 7;
        const LOWP = 1 << 8;
        const READ_ONLY = 1 << 9;
        const WRITE_ONLY = 1 << 10;
        const BUFFER = 1 << 11;
        const PIXEL_LOCAL = 1 << 12;
        /// The GLSL `shared` modifier. Only allowed in a compute program.
        const WORKGROUP = 1 << 13;
        // SkSL extensions, not present in GLSL
        const EXPORT = 1 << 14;
        const ES3 = 1 << 15;
        const PURE = 1 << 16;
        const INLINE = 1 << 17;
        const NO_INLINE = 1 << 18;
    }
}

impl ModifierFlags {
    /// `paddedDescription`: each modifier followed by a space, in Skia's fixed order.
    // Port of: src/sksl/ir/SkSLModifierFlags.cpp#L16-L86 (chrome/m156)
    #[must_use]
    pub fn padded_description(self) -> String {
        let mut result = String::new();
        // SkSL extensions
        let extensions = [
            (Self::EXPORT, "$export "),
            (Self::ES3, "$es3 "),
            (Self::PURE, "$pure "),
            (Self::INLINE, "inline "),
            (Self::NO_INLINE, "noinline "),
            // Real GLSL qualifiers (must be specified in order in GLSL 4.1 and below)
            (Self::FLAT, "flat "),
            (Self::NO_PERSPECTIVE, "noperspective "),
            (Self::CONST, "const "),
            (Self::UNIFORM, "uniform "),
        ];
        for (flag, text) in extensions {
            if self.intersects(flag) {
                result.push_str(text);
            }
        }
        if self.contains(Self::IN) && self.contains(Self::OUT) {
            result.push_str("inout ");
        } else if self.contains(Self::IN) {
            result.push_str("in ");
        } else if self.contains(Self::OUT) {
            result.push_str("out ");
        }
        let rest = [
            (Self::HIGHP, "highp "),
            (Self::MEDIUMP, "mediump "),
            (Self::LOWP, "lowp "),
            (Self::READ_ONLY, "readonly "),
            (Self::WRITE_ONLY, "writeonly "),
            (Self::BUFFER, "buffer "),
            // We're using non-GLSL names for these.
            (Self::PIXEL_LOCAL, "pixel_local "),
            (Self::WORKGROUP, "workgroup "),
        ];
        for (flag, text) in rest {
            if self.intersects(flag) {
                result.push_str(text);
            }
        }
        result
    }

    /// `description`: [`ModifierFlags::padded_description`] without the trailing space.
    // Port of: src/sksl/ir/SkSLModifierFlags.cpp#L88-L94 (chrome/m156)
    #[must_use]
    pub fn description(self) -> String {
        let mut s = self.padded_description();
        s.pop();
        s
    }

    /// `isConst`.
    #[must_use]
    pub fn is_const(self) -> bool {
        self.contains(Self::CONST)
    }

    /// `isUniform`.
    #[must_use]
    pub fn is_uniform(self) -> bool {
        self.contains(Self::UNIFORM)
    }

    /// `isReadOnly`.
    #[must_use]
    pub fn is_read_only(self) -> bool {
        self.contains(Self::READ_ONLY)
    }

    /// `isWriteOnly`.
    #[must_use]
    pub fn is_write_only(self) -> bool {
        self.contains(Self::WRITE_ONLY)
    }

    /// `isBuffer`.
    #[must_use]
    pub fn is_buffer(self) -> bool {
        self.contains(Self::BUFFER)
    }

    /// `isPixelLocal`.
    #[must_use]
    pub fn is_pixel_local(self) -> bool {
        self.contains(Self::PIXEL_LOCAL)
    }

    /// `isWorkgroup`.
    #[must_use]
    pub fn is_workgroup(self) -> bool {
        self.contains(Self::WORKGROUP)
    }

    /// `isExport`.
    #[must_use]
    pub fn is_export(self) -> bool {
        self.contains(Self::EXPORT)
    }

    /// `isES3`.
    #[must_use]
    pub fn is_es3(self) -> bool {
        self.contains(Self::ES3)
    }

    /// `isPure`.
    #[must_use]
    pub fn is_pure(self) -> bool {
        self.contains(Self::PURE)
    }

    /// `isInline`.
    #[must_use]
    pub fn is_inline(self) -> bool {
        self.contains(Self::INLINE)
    }

    /// `isNoInline`.
    #[must_use]
    pub fn is_no_inline(self) -> bool {
        self.contains(Self::NO_INLINE)
    }

    /// `isFlat`.
    #[must_use]
    pub fn is_flat(self) -> bool {
        self.contains(Self::FLAT)
    }

    /// `isNoPerspective`.
    #[must_use]
    pub fn is_no_perspective(self) -> bool {
        self.contains(Self::NO_PERSPECTIVE)
    }
}

/// `SkSL::Modifiers`: the modifiers of a declaration as the parser saw them.
// Port of: src/sksl/ir/SkSLModifiers.h#L17-L21 (chrome/m156)
#[doc(alias = "SkSL::Modifiers")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    /// `fPosition`.
    pub position: Position,
    /// `fLayout`.
    pub layout: Layout,
    /// `fFlags`.
    pub flags: ModifierFlags,
}

#[cfg(test)]
mod tests {
    use super::ModifierFlags;

    #[test]
    fn descriptions_follow_skia_order() {
        assert_eq!(ModifierFlags::empty().description(), "");
        assert_eq!(
            (ModifierFlags::IN | ModifierFlags::OUT).padded_description(),
            "inout "
        );
        assert_eq!(
            (ModifierFlags::UNIFORM | ModifierFlags::PURE | ModifierFlags::HIGHP).description(),
            "$pure uniform highp"
        );
        assert_eq!(ModifierFlags::OUT.description(), "out");
    }
}
