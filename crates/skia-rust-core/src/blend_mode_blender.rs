// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkBlendModeBlender.{h,cpp}

//! `SkBlendModeBlender`: the [`Blender`] of a [`BlendMode`], one shared instance per mode.

use std::sync::OnceLock;

use crate::blend_mode::BlendMode;
use crate::blend_mode_priv;
use crate::blender::{Blender, BlenderBase, BlenderType};
use crate::effect_priv::StageRec;

/// The blender of a blend mode (`SkBlendModeBlender`).
// Port of: src/core/SkBlendModeBlender.h#L21-L40 (chrome/m156)
#[doc(alias = "SkBlendModeBlender")]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BlendModeBlender {
    mode: BlendMode,
}

impl BlendModeBlender {
    /// A blender for `mode`. Use [`Blender::mode`] for the shared instance.
    #[must_use]
    pub fn new(mode: BlendMode) -> BlendModeBlender {
        BlendModeBlender { mode }
    }

    /// The blend mode (`mode`).
    #[must_use]
    pub fn mode(&self) -> BlendMode {
        self.mode
    }
}

impl BlenderBase for BlendModeBlender {
    fn as_blend_mode(&self) -> Option<BlendMode> {
        Some(self.mode)
    }

    // Port of: src/core/SkBlendModeBlender.cpp#L78-L81 (chrome/m156)
    fn on_append_stages(&self, rec: &mut StageRec<'_, '_>) -> bool {
        blend_mode_priv::append_stages(self.mode, rec.pipeline);
        true
    }

    fn blender_type(&self) -> BlenderType {
        BlenderType::BlendMode
    }
}

/// The shared blender of `mode` (`GetBlendModeSingleton`). (Skia returns a raw pointer; this
/// returns the shared handle, so callers can clone it.)
// Port of: src/core/SkBlendModeBlender.cpp#L20-L63 (chrome/m156)
#[doc(alias = "GetBlendModeSingleton")]
#[must_use]
pub fn get_blend_mode_singleton(mode: BlendMode) -> &'static Blender {
    static BLENDERS: OnceLock<[Blender; BlendMode::COUNT]> = OnceLock::new();
    let blenders = BLENDERS
        .get_or_init(|| BlendMode::VALUES.map(|m| Blender::from_base(BlendModeBlender::new(m))));
    &blenders[mode as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn singletons() {
        for m in BlendMode::VALUES {
            let a = Blender::mode(m);
            let b = Blender::from(m);
            assert!(a.ptr_eq(&b));
            assert_eq!(a, b);
            assert_eq!(a.as_base().as_blend_mode(), Some(m));
            assert_eq!(a.as_base().blender_type(), BlenderType::BlendMode);
        }
        assert_ne!(Blender::mode(BlendMode::Src), Blender::mode(BlendMode::Dst));
        // A separately created blender is a different object.
        let other = Blender::from_base(BlendModeBlender::new(BlendMode::Src));
        assert_ne!(other, Blender::mode(BlendMode::Src));
    }

    #[test]
    fn affects_transparent_black() {
        let affects = |m| Blender::mode(m).as_base().affects_transparent_black();
        // dst coefficient One, ISA or ISC: the destination is kept.
        for m in [
            BlendMode::Dst,
            BlendMode::SrcOver,
            BlendMode::DstOver,
            BlendMode::DstOut,
            BlendMode::SrcATop,
            BlendMode::Xor,
            BlendMode::Plus,
            BlendMode::Screen,
        ] {
            assert!(!affects(m), "{m:?}");
        }
        for m in [
            BlendMode::Clear,
            BlendMode::Src,
            BlendMode::SrcIn,
            BlendMode::DstIn,
            BlendMode::SrcOut,
            BlendMode::DstATop,
            BlendMode::Modulate,
        ] {
            assert!(affects(m), "{m:?}");
        }
        // Advanced modes do not.
        assert!(!affects(BlendMode::Overlay));
        assert!(!affects(BlendMode::Luminosity));
    }
}
