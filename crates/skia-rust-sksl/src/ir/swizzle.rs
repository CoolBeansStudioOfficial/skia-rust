// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLSwizzle.{h,cpp} (data, `MaskString`, `IsIdentity` and
// `description`). `Convert`, `Make` and `MakeExact` come with task S7b.

//! [`Swizzle`]: `base.xyzw`.

use super::{IrPool, ids::ExprId};
use crate::operator::OperatorPrecedence;

/// `SkSL::SwizzleComponent`: the component codes of a swizzle mask. `ZERO` and `ONE` are the
/// constant components (`.x0`, `.1y`).
#[doc(alias = "SkSL::SwizzleComponent")]
#[allow(missing_docs)]
pub mod swizzle_component {
    pub const X: i8 = 0;
    pub const Y: i8 = 1;
    pub const Z: i8 = 2;
    pub const W: i8 = 3;
    pub const R: i8 = 4;
    pub const G: i8 = 5;
    pub const B: i8 = 6;
    pub const A: i8 = 7;
    pub const S: i8 = 8;
    pub const T: i8 = 9;
    pub const P: i8 = 10;
    pub const Q: i8 = 11;
    pub const UL: i8 = 12;
    pub const UT: i8 = 13;
    pub const UR: i8 = 14;
    pub const UB: i8 = 15;
    pub const ZERO: i8 = 16;
    pub const ONE: i8 = 17;
}

/// `SkSL::ComponentArray`: one to four swizzle components (`FixedArray<4, int8_t>`).
#[doc(alias = "SkSL::ComponentArray")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ComponentArray {
    len: u8,
    components: [i8; 4],
}

impl ComponentArray {
    /// An array holding `components`.
    ///
    /// # Panics
    ///
    /// If there are more than four components.
    #[must_use]
    pub fn from_slice(components: &[i8]) -> Self {
        assert!(components.len() <= 4, "a swizzle has at most 4 components");
        let mut result = Self::default();
        for &c in components {
            result.push(c);
        }
        result
    }

    /// Appends a component.
    ///
    /// # Panics
    ///
    /// If the array already holds four components.
    pub fn push(&mut self, component: i8) {
        assert!(self.len < 4, "a swizzle has at most 4 components");
        self.components[usize::from(self.len)] = component;
        self.len += 1;
    }

    /// The components.
    #[must_use]
    pub fn as_slice(&self) -> &[i8] {
        &self.components[..usize::from(self.len)]
    }

    /// `size()`.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    /// `empty()`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl std::ops::Deref for ComponentArray {
    type Target = [i8];

    fn deref(&self) -> &[i8] {
        self.as_slice()
    }
}

/// `SkSL::Swizzle`. Its type is the base's component type widened to the mask's length.
// Port of: src/sksl/ir/SkSLSwizzle.h#L45-L128 (chrome/m156)
#[doc(alias = "SkSL::Swizzle")]
#[derive(Clone, Debug, PartialEq)]
pub struct Swizzle {
    /// `base()`.
    pub base: ExprId,
    /// `components()`.
    pub components: ComponentArray,
}

/// `mask_char`: the letter of a component.
// Port of: src/sksl/ir/SkSLSwizzle.cpp#L89-L111 (chrome/m156)
fn mask_char(component: i8) -> char {
    use swizzle_component as c;
    match component {
        c::X => 'x',
        c::Y => 'y',
        c::Z => 'z',
        c::W => 'w',
        c::R => 'r',
        c::G => 'g',
        c::B => 'b',
        c::A => 'a',
        c::S => 's',
        c::T => 't',
        c::P => 'p',
        c::Q => 'q',
        c::UL => 'L',
        c::UT => 'T',
        c::UR => 'R',
        c::UB => 'B',
        c::ZERO => '0',
        c::ONE => '1',
        _ => unreachable!("invalid swizzle component {component}"),
    }
}

impl Swizzle {
    /// `MaskString(components)`: `"xyz"`, `"x0"`, …
    // Port of: src/sksl/ir/SkSLSwizzle.cpp#L113-L119 (chrome/m156)
    #[must_use]
    pub fn mask_string(components: &[i8]) -> String {
        components.iter().map(|&c| mask_char(c)).collect()
    }

    /// `IsIdentity(components)`: `.x`, `.xy`, `.xyz` or `.xyzw`.
    #[must_use]
    pub fn is_identity(components: &[i8]) -> bool {
        components
            .iter()
            .enumerate()
            .all(|(index, &c)| i8::try_from(index).is_ok_and(|i| i == c))
    }

    /// `description()`: `base.mask`.
    // Port of: src/sksl/ir/SkSLSwizzle.cpp#L551-L554 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        format!(
            "{}.{}",
            pool.expression_description_with(self.base, OperatorPrecedence::Postfix),
            Self::mask_string(&self.components)
        )
    }
}
