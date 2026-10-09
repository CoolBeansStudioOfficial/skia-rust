// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file.
// Port of: src/ports/fontations/src/hinting.rs (chrome/m156), with the `cxx` FFI replaced by a
// plain Rust API: the `Box` handles become values and the `unsafe` FFI markers are dropped.

//! Hinting instances: which hinting engine a glyph is drawn with, at which size and location.

use std::sync::OnceLock;

use skrifa::{
    outline::{
        Engine, GlyphStyles, HintingInstance, HintingOptions, OutlineGlyphFormat, SmoothMode,
        Target,
    },
    prelude::Size,
};

use super::base::{BridgeNormalizedCoords, BridgeOutlineCollection};

/// Whether the bridge should force the autohinter (`AutoHintingControl` in `ffi.rs`).
// Port of: src/ports/fontations/src/ffi.rs (AutoHintingControl, chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoHintingControl {
    /// Autohint TrueType (glyf) outlines, nothing else.
    ForceForGlyf,
    /// Autohint TrueType and CFF outlines.
    ForceForGlyfAndCff,
    /// Never autohint.
    ForceOff,
    /// Let the hinting engine choose, as `FreeType` does.
    Fallback,
}

// Rust side lazily created GlyphStyles that are computed when first needed.
// This helps optimize make_hinting_instance in the Fontations ScalerContext.
// Port of: src/ports/fontations/src/hinting.rs#L13-L22 (chrome/m156)
#[derive(Default, Debug)]
pub struct BridgeGlyphStyles {
    glyph_styles: OnceLock<GlyphStyles>,
}

/// A hinting instance, or `None` when the glyph is drawn without hinting (`BridgeHintingInstance`).
// Port of: src/ports/fontations/src/hinting.rs#L24 (chrome/m156)
pub struct BridgeHintingInstance(pub Option<HintingInstance>);

// skrifa's `HintingInstance` is not `Debug`, so the debug output shows only whether there is one.
impl std::fmt::Debug for BridgeHintingInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("BridgeHintingInstance")
            .field(&self.0.is_some())
            .finish()
    }
}

// Port of: src/ports/fontations/src/hinting.rs#L26-L28 (chrome/m156)
#[must_use]
pub fn get_bridge_glyph_styles() -> BridgeGlyphStyles {
    BridgeGlyphStyles::default()
}

// Port of: src/ports/fontations/src/hinting.rs#L30-L36 (chrome/m156)
#[must_use]
pub fn hinting_reliant(font_ref: &BridgeOutlineCollection<'_>) -> bool {
    if let Some(outlines) = &font_ref.0 {
        outlines.require_interpreter()
    } else {
        false
    }
}

// Port of: src/ports/fontations/src/hinting.rs#L38-L40 (chrome/m156)
#[must_use]
pub fn no_hinting_instance() -> BridgeHintingInstance {
    BridgeHintingInstance(None)
}

/// Port of `make_hinting_instance`: the hinting instance for `outlines` at `size`, with the
/// engine and target the flags select. The flags are the bridge's parameters, in the C++ order.
// Port of: src/ports/fontations/src/hinting.rs#L42-L112 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the parameters of the C++ bridge function
#[must_use]
pub fn make_hinting_instance(
    outlines: &BridgeOutlineCollection<'_>,
    bridge_glyph_styles: &BridgeGlyphStyles,
    size: f32,
    coords: &BridgeNormalizedCoords,
    do_light_hinting: bool,
    do_lcd_antialiasing: bool,
    lcd_orientation_vertical: bool,
    autohinting_control: AutoHintingControl,
) -> BridgeHintingInstance {
    let hinting_instance = match &outlines.0 {
        Some(outlines) => {
            let smooth_mode = match (
                do_light_hinting,
                do_lcd_antialiasing,
                lcd_orientation_vertical,
            ) {
                (true, _, _) => SmoothMode::Light,
                (false, true, false) => SmoothMode::Lcd,
                (false, true, true) => SmoothMode::VerticalLcd,
                _ => SmoothMode::Normal,
            };

            let hinting_target = Target::Smooth {
                mode: smooth_mode,
                // See https://docs.rs/skrifa/latest/skrifa/outline/enum.Target.html#variant.Smooth.field.mode
                // Configure additional params to match FreeType.
                symmetric_rendering: true,
                preserve_linear_metrics: false,
            };

            // Do not force-autohint for CFF to match FreeType, compare
            // https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/base/ftobjs.c#L1001
            // Engine::AutoFallback (see Skrifa docs) means:
            // "Specifically, PostScript (CFF/CFF2) fonts will always use the hinting engine in the
            // PostScript interpreter and TrueType fonts will use the interpreter for TrueType
            // instructions if one of the fpgm or prep tables is non-empty, falling back to the
            // automatic hinter otherwise."
            // So Engine::AutoFallback does not engage autohinting for CFF.
            let engine_type = match (autohinting_control, outlines.format()) {
                (AutoHintingControl::ForceForGlyf, Some(OutlineGlyphFormat::Glyf))
                | (AutoHintingControl::ForceForGlyfAndCff, _) => {
                    let glyph_styles = Some(
                        bridge_glyph_styles
                            .glyph_styles
                            .get_or_init(|| GlyphStyles::new(outlines)),
                    );
                    Engine::Auto(glyph_styles.cloned())
                }
                (AutoHintingControl::ForceOff, _) => Engine::Interpreter,
                _ => Engine::AutoFallback,
            };

            HintingInstance::new(
                outlines,
                Size::new(size),
                &coords.normalized_coords,
                HintingOptions {
                    engine: engine_type,
                    target: hinting_target,
                },
            )
            .ok()
        }
        _ => None,
    };
    BridgeHintingInstance(hinting_instance)
}

/// Port of `make_mono_hinting_instance`: the strong (monochrome) hinting instance, or `None`
/// when the font has no outlines or the instance fails.
// Port of: src/ports/fontations/src/hinting.rs#L114-L129 (chrome/m156)
#[must_use]
pub fn make_mono_hinting_instance(
    outlines: &BridgeOutlineCollection<'_>,
    size: f32,
    coords: &BridgeNormalizedCoords,
) -> BridgeHintingInstance {
    let hinting_instance = outlines.0.as_ref().and_then(|outlines| {
        HintingInstance::new(
            outlines,
            Size::new(size),
            &coords.normalized_coords,
            skrifa::outline::HintingMode::Strong,
        )
        .ok()
    });
    BridgeHintingInstance(hinting_instance)
}
