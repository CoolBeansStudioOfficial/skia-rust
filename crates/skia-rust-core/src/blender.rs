// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkBlender.h, src/core/SkBlenderBase.h,
// src/core/SkBlendModeBlender.cpp (SkBlenderBase::affectsTransparentBlack)

//! `SkBlender`: a blend function, combining a source color (the result of the paint) and a
//! destination color (from the canvas) into a final color.
//!
//! [`Blender`] is the shared, cheaply clonable handle (`sk_sp<SkBlender>`); [`BlenderBase`] is
//! the trait implementations provide (`SkBlenderBase`'s virtuals). The blend-mode blenders are
//! in [`crate::blend_mode_blender`].

use core::any::Any;
use core::fmt;
use std::sync::Arc;

use crate::blend_mode::{BlendMode, BlendModeCoeff};
use crate::effect_priv::StageRec;
use crate::write_buffer::BinaryWriteBuffer;

/// The kinds of blenders (`SkBlenderBase::BlenderType`, from `SK_ALL_BLENDERS`).
// Port of: src/core/SkBlenderBase.h#L30-L32 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlenderType {
    /// `kBlendMode`.
    BlendMode,
    /// `kRuntime`.
    Runtime,
}

/// The virtual interface of a blender (`SkBlenderBase`).
///
/// Implementations are wrapped in a [`Blender`] with [`Blender::from_base`]. The non-virtual
/// members of `SkBlenderBase` are inherent methods of `dyn BlenderBase`
/// ([`affects_transparent_black`](trait.BlenderBase.html#method.affects_transparent_black),
/// [`append_stages`](trait.BlenderBase.html#method.append_stages)).
// Port of: src/core/SkBlenderBase.h#L39-L67 (chrome/m156)
#[doc(alias = "SkBlenderBase")]
pub trait BlenderBase: Any + fmt::Debug + Send + Sync {
    /// The blend mode this blender represents, if it represents one (`asBlendMode`).
    #[doc(alias = "asBlendMode")]
    fn as_blend_mode(&self) -> Option<BlendMode> {
        None
    }

    /// Appends the stages of the blend to `rec`'s pipeline; false on failure
    /// (`onAppendStages`).
    #[doc(alias = "onAppendStages")]
    fn on_append_stages(&self, rec: &mut StageRec<'_, '_>) -> bool;

    /// The kind of blender (`type`).
    #[doc(alias = "type")]
    fn blender_type(&self) -> BlenderType;

    /// The name the blender is flattened under (`getTypeName`), which the registry maps back to
    /// its factory. The empty name, the default, marks a blender that cannot be flattened.
    #[doc(alias = "getTypeName")]
    fn type_name(&self) -> &'static str {
        ""
    }

    /// Writes the parameters of the blender (`flatten`). Writes nothing by default.
    fn flatten(&self, _buffer: &mut BinaryWriteBuffer) {}
}

impl dyn BlenderBase {
    /// True if blending with a transparent black source can change the destination
    /// (`affectsTransparentBlack`).
    // Port of: src/core/SkBlendModeBlender.cpp#L83-L101 (chrome/m156)
    #[doc(alias = "affectsTransparentBlack")]
    #[must_use]
    pub fn affects_transparent_black(&self) -> bool {
        if let Some(blend_mode) = self.as_blend_mode() {
            if let Some((_src, dst)) = blend_mode.as_coeff() {
                // If the source is (0,0,0,0), then dst is preserved as long as its coefficient
                // evaluates to 1.0. This is true for kOne, kISA, and kISC. Anything else means the
                // blend mode affects transparent black.
                dst != BlendModeCoeff::One
                    && dst != BlendModeCoeff::ISA
                    && dst != BlendModeCoeff::ISC
            } else {
                // An advanced blend mode, which do not affect transparent black
                false
            }
        } else {
            // Blenders that aren't blend modes are assumed to modify transparent black.
            true
        }
    }

    /// Appends the stages of the blend to `rec`'s pipeline; false on failure (`appendStages`).
    // Port of: src/core/SkBlenderBase.h#L49-L51 (chrome/m156)
    #[doc(alias = "appendStages")]
    #[must_use]
    pub fn append_stages(&self, rec: &mut StageRec<'_, '_>) -> bool {
        self.on_append_stages(rec)
    }
}

/// A shared blend function (`sk_sp<SkBlender>`): a cheaply clonable handle to a
/// [`BlenderBase`].
///
/// Equality is identity, as Skia compares `sk_sp`s ([`Blender::ptr_eq`]).
// Port of: include/core/SkBlender.h#L18-L28 (chrome/m156)
#[doc(alias = "SkBlender")]
#[derive(Clone)]
pub struct Blender(Arc<dyn BlenderBase>);

impl Blender {
    /// A blender that implements `mode` (`SkBlender::Mode`). Every call with the same mode
    /// returns the same shared blender.
    // Port of: src/core/SkBlendModeBlender.cpp#L65-L67 (chrome/m156)
    #[doc(alias = "Mode")]
    #[must_use]
    pub fn mode(mode: BlendMode) -> Blender {
        crate::blend_mode_blender::get_blend_mode_singleton(mode).clone()
    }

    /// Wraps a blender implementation.
    #[must_use]
    pub fn from_base(base: impl BlenderBase) -> Blender {
        Blender(Arc::new(base))
    }

    /// The implementation (`as_BB`).
    #[doc(alias = "as_BB")]
    #[must_use]
    pub fn as_base(&self) -> &dyn BlenderBase {
        &*self.0
    }

    /// True if `self` and `other` are the same blender (Skia's `sk_sp` comparison).
    #[must_use]
    pub fn ptr_eq(&self, other: &Blender) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl PartialEq for Blender {
    /// Identity, as Skia's `sk_sp<SkBlender>` `operator==`.
    fn eq(&self, other: &Blender) -> bool {
        self.ptr_eq(other)
    }
}

impl From<BlendMode> for Blender {
    fn from(mode: BlendMode) -> Blender {
        Blender::mode(mode)
    }
}

impl fmt::Debug for Blender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Blender").field(&self.0).finish()
    }
}
