// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLLayout.{h,cpp}. `checkPermittedLayout` comes with the type
// system (task S6).

//! [`Layout`]: a `layout (…)` qualifier.

use crate::string::Separator;

bitflags::bitflags! {
    /// `SkSL::LayoutFlag` / `LayoutFlags`.
    #[doc(alias = "SkSL::LayoutFlag")]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct LayoutFlags: i32 {
        const ORIGIN_UPPER_LEFT = 1 << 0;
        const PUSH_CONSTANT = 1 << 1;
        const BLEND_SUPPORT_ALL_EQUATIONS = 1 << 2;
        const COLOR = 1 << 3;
        // These flags indicate if the qualifier appeared, regardless of the accompanying value.
        const LOCATION = 1 << 4;
        const OFFSET = 1 << 5;
        const BINDING = 1 << 6;
        const TEXTURE = 1 << 7;
        const SAMPLER = 1 << 8;
        const INDEX = 1 << 9;
        const SET = 1 << 10;
        const BUILTIN = 1 << 11;
        const INPUT_ATTACHMENT_INDEX = 1 << 12;
        // These flags indicate the backend type; only one at most can be set.
        const VULKAN = 1 << 13;
        const METAL = 1 << 14;
        const WEB_GPU = 1 << 15;
        const DIRECT3D = 1 << 16;
        const ALL_BACKENDS = Self::VULKAN.bits() | Self::METAL.bits() | Self::WEB_GPU.bits()
            | Self::DIRECT3D.bits();
        // These flags indicate the pixel format; only one at most can be set.
        const RGBA8 = 1 << 17;
        const RGBA32F = 1 << 18;
        const R32F = 1 << 19;
        const ALL_PIXEL_FORMATS = Self::RGBA8.bits() | Self::RGBA32F.bits() | Self::R32F.bits();
        // The local invocation size of a compute program.
        const LOCAL_SIZE_X = 1 << 20;
        const LOCAL_SIZE_Y = 1 << 21;
        const LOCAL_SIZE_Z = 1 << 22;
    }
}

/// `SkSL::Layout`: the qualifiers of a `layout (…)` clause. Integer values are -1 when absent.
// Port of: src/sksl/ir/SkSLLayout.h#L62-L141 (chrome/m156)
#[doc(alias = "SkSL::Layout")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Layout {
    /// `fFlags`.
    pub flags: LayoutFlags,
    /// `fLocation`.
    pub location: i32,
    /// `fOffset`.
    pub offset: i32,
    /// `fBinding`.
    pub binding: i32,
    /// `fTexture`.
    pub texture: i32,
    /// `fSampler`.
    pub sampler: i32,
    /// `fIndex`.
    pub index: i32,
    /// `fSet`.
    pub set: i32,
    /// `fBuiltin`: the SPIR-V builtin this object represents.
    pub builtin: i32,
    /// `fInputAttachmentIndex`.
    pub input_attachment_index: i32,
    /// `fLocalSizeX`.
    pub local_size_x: i32,
    /// `fLocalSizeY`.
    pub local_size_y: i32,
    /// `fLocalSizeZ`.
    pub local_size_z: i32,
}

impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}

impl Layout {
    /// An empty layout (`Layout()`): no flags, every value -1.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            flags: LayoutFlags::empty(),
            location: -1,
            offset: -1,
            binding: -1,
            texture: -1,
            sampler: -1,
            index: -1,
            set: -1,
            builtin: -1,
            input_attachment_index: -1,
            local_size_x: -1,
            local_size_y: -1,
            local_size_z: -1,
        }
    }

    /// `Layout::builtin(builtin)`: a layout that only names a SPIR-V builtin.
    #[must_use]
    pub const fn with_builtin(builtin: i32) -> Self {
        let mut result = Self::new();
        result.builtin = builtin;
        result
    }

    /// `paddedDescription`: `layout (…) ` with a trailing space, or "" for an empty layout.
    // Port of: src/sksl/ir/SkSLLayout.cpp#L18-L95 (chrome/m156)
    #[must_use]
    pub fn padded_description(&self) -> String {
        let mut result = String::new();
        let mut separator = Separator::new();
        let mut add = |result: &mut String, text: &str| {
            result.push_str(separator.next_str());
            result.push_str(text);
        };
        let flags = self.flags;
        if flags.contains(LayoutFlags::VULKAN) {
            add(&mut result, "vulkan");
        }
        if flags.contains(LayoutFlags::METAL) {
            add(&mut result, "metal");
        }
        if flags.contains(LayoutFlags::WEB_GPU) {
            add(&mut result, "webgpu");
        }
        if flags.contains(LayoutFlags::DIRECT3D) {
            add(&mut result, "direct3d");
        }
        if flags.contains(LayoutFlags::RGBA8) {
            add(&mut result, "rgba8");
        }
        if flags.contains(LayoutFlags::RGBA32F) {
            add(&mut result, "rgba32f");
        }
        if flags.contains(LayoutFlags::R32F) {
            add(&mut result, "r32f");
        }
        let values = [
            (self.location, "location = "),
            (self.offset, "offset = "),
            (self.binding, "binding = "),
            (self.texture, "texture = "),
            (self.sampler, "sampler = "),
            (self.index, "index = "),
            (self.set, "set = "),
            (self.builtin, "builtin = "),
            (self.input_attachment_index, "input_attachment_index = "),
        ];
        for (value, label) in values {
            if value >= 0 {
                add(&mut result, &format!("{label}{value}"));
            }
        }
        if flags.contains(LayoutFlags::ORIGIN_UPPER_LEFT) {
            add(&mut result, "origin_upper_left");
        }
        if flags.contains(LayoutFlags::BLEND_SUPPORT_ALL_EQUATIONS) {
            add(&mut result, "blend_support_all_equations");
        }
        if flags.contains(LayoutFlags::PUSH_CONSTANT) {
            add(&mut result, "push_constant");
        }
        if flags.contains(LayoutFlags::COLOR) {
            add(&mut result, "color");
        }
        let local_sizes = [
            (self.local_size_x, "local_size_x = "),
            (self.local_size_y, "local_size_y = "),
            (self.local_size_z, "local_size_z = "),
        ];
        for (value, label) in local_sizes {
            if value >= 0 {
                add(&mut result, &format!("{label}{value}"));
            }
        }
        if !result.is_empty() {
            result = format!("layout ({result}) ");
        }
        result
    }

    /// `description`: [`Layout::padded_description`] without the trailing space.
    // Port of: src/sksl/ir/SkSLLayout.cpp#L97-L103 (chrome/m156)
    #[must_use]
    pub fn description(&self) -> String {
        let mut s = self.padded_description();
        s.pop();
        s
    }
}

#[cfg(test)]
mod tests {
    use super::{Layout, LayoutFlags};

    #[test]
    fn descriptions() {
        assert_eq!(Layout::new().padded_description(), "");
        assert_eq!(Layout::new().description(), "");
        let mut layout = Layout::with_builtin(15);
        layout.flags = LayoutFlags::COLOR | LayoutFlags::METAL;
        layout.binding = 2;
        assert_eq!(
            layout.padded_description(),
            "layout (metal, binding = 2, builtin = 15, color) "
        );
        assert_eq!(
            layout.description(),
            "layout (metal, binding = 2, builtin = 15, color)"
        );
    }
}
