// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/colorfilters/SkWorkingFormatColorFilter.{h,cpp}
// (SkColorFilterPriv::WithWorkingFormat)

//! `SkWorkingFormatColorFilter`: runs a child color filter in a working color space, converting
//! from the destination color space and back.
//!

use core::fmt;

use skia_rust_skcms::{Matrix3x3, TransferFunction};

use crate::alpha_type::AlphaType;
use crate::blend_mode::BlendMode;
use crate::color::{Color, PMColor4f};
use crate::color_filter::{ColorFilter, ColorFilterBase, ColorFilterType};
use crate::color_space::ColorSpace;
use crate::color_space_xform_steps::ColorSpaceXformSteps;
use crate::effect_priv::StageRec;
use crate::flattenable::FlattenableRegistry;
use crate::read_buffer::ReadBuffer;
use crate::write_buffer::BinaryWriteBuffer;

/// `SkWorkingFormatCalculator`: the working format (transfer function, gamut and alpha type),
/// where each part either is fixed or follows the destination color space.
// Port of: src/effects/colorfilters/SkWorkingFormatColorFilter.h#L21-L38 (chrome/m156)
#[doc(alias = "SkWorkingFormatCalculator")]
#[derive(Clone, Debug)]
pub struct WorkingFormatCalculator {
    tf: TransferFunction,
    use_dst_tf: bool,
    gamut: Matrix3x3,
    use_dst_gamut: bool,
    at: AlphaType,
    use_dst_at: bool,
}

impl WorkingFormatCalculator {
    // Port of: src/effects/colorfilters/SkWorkingFormatColorFilter.cpp#L40-L55 (chrome/m156)
    #[must_use]
    pub fn new(
        tf: Option<&TransferFunction>,
        gamut: Option<&Matrix3x3>,
        at: Option<&AlphaType>,
    ) -> Self {
        let mut calc = WorkingFormatCalculator {
            tf: TransferFunction::default(),
            use_dst_tf: true,
            gamut: Matrix3x3::default(),
            use_dst_gamut: true,
            at: AlphaType::default(),
            use_dst_at: true,
        };
        if let Some(tf) = tf {
            calc.tf = *tf;
            calc.use_dst_tf = false;
        }
        if let Some(gamut) = gamut {
            calc.gamut = *gamut;
            calc.use_dst_gamut = false;
        }
        if let Some(at) = at {
            calc.at = *at;
            calc.use_dst_at = false;
        }
        calc
    }

    /// The working color space and alpha type for the destination `dst_cs`
    /// (`SkWorkingFormatCalculator::workingFormat`).
    ///
    /// # Panics
    /// If the destination has no numerical transfer function and the calculator takes its
    /// transfer function from it.
    // Port of: src/effects/colorfilters/SkWorkingFormatColorFilter.cpp#L57-L72 (chrome/m156)
    #[must_use]
    pub fn working_format(&self, dst_cs: &ColorSpace) -> (Option<ColorSpace>, AlphaType) {
        let tf = if self.use_dst_tf {
            dst_cs
                .is_numerical_transfer_fn()
                .expect("the destination has a numerical transfer function")
        } else {
            self.tf
        };
        let gamut = if self.use_dst_gamut {
            dst_cs.to_xyzd50()
        } else {
            self.gamut
        };
        let at = if self.use_dst_at {
            AlphaType::Premul
        } else {
            self.at
        };
        (ColorSpace::new_rgb(&tf, &gamut), at)
    }
}

/// A color filter that runs `child` in a working format (`SkWorkingFormatColorFilter`).
// Port of: src/effects/colorfilters/SkWorkingFormatColorFilter.h#L40-L81 (chrome/m156)
#[doc(alias = "SkWorkingFormatColorFilter")]
pub struct WorkingFormatColorFilter {
    child: ColorFilter,
    calc: WorkingFormatCalculator,
}

impl fmt::Debug for WorkingFormatColorFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkingFormatColorFilter")
            .field("child", &self.child)
            .field("calc", &self.calc)
            .finish()
    }
}

impl WorkingFormatColorFilter {
    /// A filter that runs `child` in the working format given by the optional parts; a `None`
    /// part follows the destination color space (`SkWorkingFormatColorFilter`).
    // Port of: src/effects/colorfilters/SkWorkingFormatColorFilter.cpp#L17-L23 (chrome/m156)
    #[must_use]
    pub fn new(
        child: ColorFilter,
        tf: Option<&TransferFunction>,
        gamut: Option<&Matrix3x3>,
        at: Option<&AlphaType>,
    ) -> Self {
        WorkingFormatColorFilter {
            child,
            calc: WorkingFormatCalculator::new(tf, gamut, at),
        }
    }

    /// The working color space and alpha type for `dst_cs` (`workingFormat`).
    // Port of: src/effects/colorfilters/SkWorkingFormatColorFilter.cpp#L25-L28 (chrome/m156)
    #[must_use]
    pub fn working_format(&self, dst_cs: &ColorSpace) -> (Option<ColorSpace>, AlphaType) {
        self.calc.working_format(dst_cs)
    }

    /// The filter that is run in the working format (`child`).
    #[must_use]
    pub fn child(&self) -> &ColorFilter {
        &self.child
    }
}

impl ColorFilterBase for WorkingFormatColorFilter {
    // Port of: src/effects/colorfilters/SkWorkingFormatColorFilter.cpp#L30-L62 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, shader_is_opaque: bool) -> bool {
        let dst_cs = rec.dst_cs.cloned().unwrap_or_else(ColorSpace::new_srgb);
        let (working_cs, working_at) = self.working_format(&dst_cs);
        let dst_to_working = ColorSpaceXformSteps::new(
            Some(&dst_cs),
            AlphaType::Premul,
            working_cs.as_ref(),
            working_at,
        );
        let working_to_dst = ColorSpaceXformSteps::new(
            working_cs.as_ref(),
            working_at,
            Some(&dst_cs),
            AlphaType::Premul,
        );

        // The paint color is in the destination color space, so *should* be converted to working
        // space. That's not necessary, though:
        //   - Tinting alpha-only image shaders is the only effect that uses paint-color
        //   - Alpha-only image shaders can't be reached from color-filters without SkSL
        //   - SkSL disables paint-color tinting of alpha-only image shaders
        dst_to_working.apply_to_pipeline(rec.pipeline, rec.alloc);
        {
            let mut working_rec = StageRec {
                pipeline: &mut *rec.pipeline,
                alloc: rec.alloc,
                dst_color_type: rec.dst_color_type,
                dst_cs: working_cs.as_ref(),
                paint_color: rec.paint_color,
                surface_props: rec.surface_props,
                dst_bounds: rec.dst_bounds,
            };
            if !self
                .child
                .as_base()
                .append_stages(&mut working_rec, shader_is_opaque)
            {
                return false;
            }
        }
        working_to_dst.apply_to_pipeline(rec.pipeline, rec.alloc);
        true
    }

    // Port of: src/effects/colorfilters/SkWorkingFormatColorFilter.cpp#L64-L80 (chrome/m156)
    fn on_filter_color4f(&self, orig_color: &PMColor4f, dst_cs: Option<&ColorSpace>) -> PMColor4f {
        let dst_cs = dst_cs.cloned().unwrap_or_else(ColorSpace::new_srgb);
        let (working_cs, working_at) = self.working_format(&dst_cs);

        let mut color = orig_color.as_array();
        ColorSpaceXformSteps::new(
            Some(&dst_cs),
            AlphaType::Premul,
            working_cs.as_ref(),
            working_at,
        )
        .apply(&mut color);
        let color = PMColor4f::new(color[0], color[1], color[2], color[3]);

        let color = self
            .child
            .as_base()
            .on_filter_color4f(&color, working_cs.as_ref());

        let mut out = color.as_array();
        ColorSpaceXformSteps::new(
            working_cs.as_ref(),
            working_at,
            Some(&dst_cs),
            AlphaType::Premul,
        )
        .apply(&mut out);
        PMColor4f::new(out[0], out[1], out[2], out[3])
    }

    // Port of: src/effects/colorfilters/SkWorkingFormatColorFilter.cpp#L82-L84 (chrome/m156)
    fn on_is_alpha_unchanged(&self) -> bool {
        self.child.is_alpha_unchanged()
    }

    // Port of: src/effects/colorfilters/SkWorkingFormatColorFilter.cpp#L86-L88 (chrome/m156)
    fn on_as_a_color_mode(&self) -> Option<(Color, BlendMode)> {
        self.child.to_a_color_mode()
    }

    // Port of: src/effects/colorfilters/SkWorkingFormatColorFilter.cpp#L90-L92 (chrome/m156)
    fn on_as_a_color_matrix(&self) -> Option<[f32; 20]> {
        self.child.to_a_color_matrix()
    }

    // Port of: src/effects/colorfilters/SkWorkingFormatColorFilter.h#L53-L55 (chrome/m156)
    fn color_filter_type(&self) -> ColorFilterType {
        ColorFilterType::WorkingFormat
    }

    // Port of: src/effects/colorfilters/SkWorkingFormatColorFilter.cpp (chrome/m156),
    // SK_REGISTER_FLATTENABLE
    fn type_name(&self) -> &'static str {
        "SkWorkingFormatColorFilter"
    }

    // Port of: src/effects/colorfilters/SkWorkingFormatColorFilter.cpp#L168-L173 (chrome/m156)
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        buffer.write_color_filter(Some(&self.child));
        // `SkWorkingFormatCalculator::flatten`: the three "use the destination" flags, then each
        // fixed part.
        let calc = &self.calc;
        buffer.write_bool(calc.use_dst_tf);
        buffer.write_bool(calc.use_dst_gamut);
        buffer.write_bool(calc.use_dst_at);
        if !calc.use_dst_tf {
            let tf = calc.tf;
            buffer.write_scalar_array(&[tf.g, tf.a, tf.b, tf.c, tf.d, tf.e, tf.f]);
        }
        if !calc.use_dst_gamut {
            let m = calc.gamut.vals;
            buffer.write_scalar_array(&[
                m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2], m[2][0], m[2][1], m[2][2],
            ]);
        }
        if !calc.use_dst_at {
            buffer.write_int(calc.at as i32);
        }
    }
}

/// `SkColorFilterPriv::WithWorkingFormat`: `child` run in the working format, or `None` for a
/// `None` child (the identity, since the conversions cancel out).
// Port of: src/effects/colorfilters/SkWorkingFormatColorFilter.cpp#L140-L153 (chrome/m156)
#[doc(alias = "WithWorkingFormat")]
#[must_use]
pub fn with_working_format(
    child: Option<ColorFilter>,
    tf: Option<&TransferFunction>,
    gamut: Option<&Matrix3x3>,
    at: Option<&AlphaType>,
) -> Option<ColorFilter> {
    let child = child?;
    Some(ColorFilter::from_base(WorkingFormatColorFilter::new(
        child, tf, gamut, at,
    )))
}

/// `SkWorkingFormatColorFilter::CreateProc`: the child filter, the three "use the destination"
/// flags, then whichever of the transfer function, gamut and alpha type are fixed.
// Port of: src/effects/colorfilters/SkWorkingFormatColorFilter.cpp#L175-L199 (chrome/m156)
#[doc(alias = "CreateProc")]
#[must_use]
pub fn create_proc(
    buffer: &mut ReadBuffer<'_>,
    registry: &FlattenableRegistry,
) -> Option<ColorFilter> {
    let child = buffer.read_color_filter(registry);
    let use_dst_tf = buffer.read_bool();
    let use_dst_gamut = buffer.read_bool();
    let use_dst_at = buffer.read_bool();

    let mut tf = TransferFunction::default();
    let mut gamut = Matrix3x3::default();
    let mut at = AlphaType::Unknown;

    if !use_dst_tf {
        let mut v = [0.0f32; 7];
        buffer.read_scalar_array(&mut v);
        tf = TransferFunction {
            g: v[0],
            a: v[1],
            b: v[2],
            c: v[3],
            d: v[4],
            e: v[5],
            f: v[6],
        };
    }
    if !use_dst_gamut {
        let mut v = [0.0f32; 9];
        buffer.read_scalar_array(&mut v);
        gamut.vals = [[v[0], v[1], v[2]], [v[3], v[4], v[5]], [v[6], v[7], v[8]]];
    }
    if !use_dst_at {
        at = alpha_type_from_u32(buffer.read32_le(AlphaType::LAST_ENUM as u32));
    }

    with_working_format(
        child,
        (!use_dst_tf).then_some(&tf),
        (!use_dst_gamut).then_some(&gamut),
        (!use_dst_at).then_some(&at),
    )
}

/// The alpha type with the value `value`, an enum value read from a buffer that was already
/// checked to be at most `kLastEnum_SkAlphaType`.
fn alpha_type_from_u32(value: u32) -> AlphaType {
    match value {
        1 => AlphaType::Opaque,
        2 => AlphaType::Premul,
        3 => AlphaType::Unpremul,
        _ => AlphaType::Unknown,
    }
}
