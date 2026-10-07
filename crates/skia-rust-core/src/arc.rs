// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkArc.h, src/core/SkPathPriv.cpp (`CreateDrawArcPath`)

//! `SkArc`: an oval, a start angle and a sweep, drawn as an arc or a pie wedge.

use crate::path::Path;
use crate::path_builder::PathBuilder;
use crate::path_enums::{PathConvexity, PathFirstDirection};
use crate::path_priv;
use crate::path_types::PathFillType;
use crate::rect::Rect;
use crate::scalar::{scalar, scalar_abs};

/// Whether an [`Arc`] is just the arc or the wedge closed through the oval's center
/// (`SkArc::Type`).
// Port of: include/core/SkArc.h#L16-L19 (chrome/m156)
#[doc(alias = "SkArc::Type")]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum ArcType {
    /// `kArc`.
    Arc,
    /// `kWedge`.
    Wedge,
}

/// An arc of an oval (`SkArc`).
// Port of: include/core/SkArc.h#L14-L61 (chrome/m156)
#[doc(alias = "SkArc")]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Arc {
    /// The oval the arc is cut from (`fOval`).
    pub oval: Rect,
    /// Where the arc starts, in degrees clockwise from 3 o'clock (`fStartAngle`).
    pub start_angle: scalar,
    /// How far the arc sweeps, in degrees (`fSweepAngle`).
    pub sweep_angle: scalar,
    /// Arc or wedge (`fType`).
    pub kind: ArcType,
}

impl Arc {
    /// `SkArc::Make(oval, start, sweep, useCenter)`.
    // Port of: include/core/SkArc.h#L38-L42 (chrome/m156)
    #[must_use]
    pub fn new(oval: Rect, start_angle: scalar, sweep_angle: scalar, use_center: bool) -> Arc {
        Arc {
            oval,
            start_angle,
            sweep_angle,
            kind: if use_center {
                ArcType::Wedge
            } else {
                ArcType::Arc
            },
        }
    }

    /// True for a pie wedge (`isWedge`).
    #[doc(alias = "isWedge")]
    #[must_use]
    pub fn is_wedge(&self) -> bool {
        self.kind == ArcType::Wedge
    }
}

/// Whether an arc drawn with these parameters yields a convex path
/// (`SkPathPriv::DrawArcIsConvex`).
// Port of: src/core/SkPathPriv.cpp#L348-L362 (chrome/m156)
#[doc(alias = "DrawArcIsConvex")]
#[must_use]
pub fn draw_arc_is_convex(
    sweep_angle: scalar,
    arc_type: ArcType,
    is_fill_no_path_effect: bool,
) -> bool {
    if is_fill_no_path_effect && scalar_abs(sweep_angle) >= 360.0 {
        // This gets converted to an oval.
        return true;
    }
    if arc_type == ArcType::Wedge {
        // This is a pie wedge. It's convex if the angle is <= 180.
        return scalar_abs(sweep_angle) <= 180.0;
    }
    // When the angle exceeds 360 this wraps back on top of itself. Otherwise it is a circle
    // clipped to a secant, i.e. convex.
    scalar_abs(sweep_angle) <= 360.0
}

/// The path `drawArc` fills or strokes for `arc` (`SkPathPriv::CreateDrawArcPath`).
// Port of: src/core/SkPathPriv.cpp#L364-L419 (chrome/m156)
#[doc(alias = "CreateDrawArcPath")]
#[must_use]
#[allow(clippy::float_cmp)] // mirrors `SkASSERT(sweepAngle)`
pub fn create_draw_arc_path(arc: &Arc, is_fill_no_path_effect: bool) -> Path {
    let oval = arc.oval;
    let mut start_angle = arc.start_angle;
    let mut sweep_angle = arc.sweep_angle;
    debug_assert!(!oval.is_empty());
    debug_assert!(sweep_angle != 0.0);
    // We cap the number of total rotations. This keeps the resulting paths simpler. More
    // important, it prevents values so large that the loops below never terminate (once ULP >
    // 360).
    if scalar_abs(sweep_angle) > 3600.0 {
        sweep_angle = 3600.0_f32.copysign(sweep_angle) + (sweep_angle % 360.0);
    }

    let mut builder = PathBuilder::new_with_fill_type(PathFillType::Winding);
    builder.set_is_volatile(true);

    if is_fill_no_path_effect && scalar_abs(sweep_angle) >= 360.0 {
        builder.add_oval(oval, None, None);
        debug_assert!(draw_arc_is_convex(
            sweep_angle,
            ArcType::Arc,
            is_fill_no_path_effect
        ));
        return builder.detach();
    }

    if arc.is_wedge() {
        builder.move_to((oval.center_x(), oval.center_y()));
    }
    let first_dir = if sweep_angle > 0.0 {
        PathFirstDirection::CW
    } else {
        PathFirstDirection::CCW
    };
    let convex = draw_arc_is_convex(sweep_angle, arc.kind, is_fill_no_path_effect);
    // Arc to mods at 360 and drawArc is not supposed to.
    let mut force_move_to = !arc.is_wedge();
    while sweep_angle <= -360.0 {
        builder.arc_to(oval, start_angle, -180.0, force_move_to);
        start_angle -= 180.0;
        builder.arc_to(oval, start_angle, -180.0, false);
        start_angle -= 180.0;
        force_move_to = false;
        sweep_angle += 360.0;
    }
    while sweep_angle >= 360.0 {
        builder.arc_to(oval, start_angle, 180.0, force_move_to);
        start_angle += 180.0;
        builder.arc_to(oval, start_angle, 180.0, false);
        start_angle += 180.0;
        force_move_to = false;
        sweep_angle -= 360.0;
    }
    builder.arc_to(oval, start_angle, sweep_angle, force_move_to);
    if arc.is_wedge() {
        builder.close();
    }

    let path = builder.detach();
    let convexity = if convex {
        first_dir.to_convexity()
    } else {
        PathConvexity::Concave
    };
    path_priv::set_convexity(&path, convexity);
    path
}
