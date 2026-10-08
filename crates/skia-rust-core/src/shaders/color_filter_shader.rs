// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkColorFilterShader.{h,cpp}

//! `SkColorFilterShader`: a shader whose colors are run through a color filter.
//!
//! D4 needs it because `SkPaintPriv::RemoveColorFilter` (used by `SkBlitter::Choose`) wraps a
//! paint's shader and color filter in one. Flattening is not ported (no `SkWriteBuffer`).

use core::cell::Cell;

use crate::color_filter::ColorFilter;
use crate::effect_priv::StageRec;
use crate::flattenable::FlattenableRegistry;
use crate::raster_pipeline::Stage;
use crate::read_buffer::ReadBuffer;
use crate::shader::Shader;
use crate::shaders::shader_base::{MatrixRec, ShaderBase, ShaderType};
use crate::write_buffer::BinaryWriteBuffer;

/// A shader that modulates another shader's colors by `alpha`, then applies a color filter
/// (`SkColorFilterShader`).
// Port of: src/shaders/SkColorFilterShader.h#L22-L45 (chrome/m156)
#[doc(alias = "SkColorFilterShader")]
#[derive(Clone, Debug)]
pub struct ColorFilterShader {
    shader: Shader,
    filter: ColorFilter,
    alpha: f32,
}

impl ColorFilterShader {
    /// `SkColorFilterShader::Make`: `shader` filtered by `filter` after being scaled by `alpha`;
    /// just `shader` if there is no filter.
    // Port of: src/shaders/SkColorFilterShader.cpp#L35-L45 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(shader: Shader, alpha: f32, filter: Option<ColorFilter>) -> Shader {
        match filter {
            None => shader,
            Some(filter) => Shader::from_base(ColorFilterShader {
                shader,
                filter,
                alpha,
            }),
        }
    }

    /// The wrapped shader.
    #[must_use]
    pub fn shader(&self) -> &Shader {
        &self.shader
    }

    /// The filter.
    #[must_use]
    pub fn filter(&self) -> &ColorFilter {
        &self.filter
    }

    /// The alpha the shader's colors are scaled by before filtering (`fAlpha`).
    #[must_use]
    pub fn alpha(&self) -> f32 {
        self.alpha
    }
}

impl ShaderBase for ColorFilterShader {
    // Port of: src/shaders/SkColorFilterShader.cpp#L62 (chrome/m156), SK_REGISTER_FLATTENABLE
    fn type_name(&self) -> &'static str {
        "SkColorFilterShader"
    }

    // Port of: src/shaders/SkColorFilterShader.cpp#L56-L60 (chrome/m156)
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        buffer.write_shader(Some(&self.shader));
        buffer.write_color_filter(Some(&self.filter));
    }

    // Port of: src/shaders/SkColorFilterShader.cpp#L54-L56 (chrome/m156)
    #[allow(clippy::float_cmp)] // Skia compares the alpha with 1 exactly
    fn is_opaque(&self) -> bool {
        self.shader.is_opaque() && self.alpha == 1.0 && self.filter.is_alpha_unchanged()
    }

    fn shader_type(&self) -> ShaderType {
        ShaderType::ColorFilter
    }

    // Port of: src/shaders/SkColorFilterShader.cpp#L65-L78 (chrome/m156)
    #[allow(clippy::float_cmp)] // Skia compares the alpha with 1 exactly
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool {
        if !self.shader.as_base().append_stages(rec, m_rec) {
            return false;
        }
        if self.alpha != 1.0 {
            rec.pipeline
                .append(Stage::Scale1Float(rec.alloc.make(Cell::new(self.alpha))));
        }
        if !self
            .filter
            .as_base()
            .append_stages(rec, self.alpha == 1.0 && self.shader.is_opaque())
        {
            return false;
        }
        true
    }
}

/// `SkColorFilterShader::CreateProc`: the shader, then its filter, made with `Make` (so a missing
/// filter gives the shader itself).
// Port of: src/shaders/SkColorFilterShader.cpp#L46-L50 (chrome/m156)
#[doc(alias = "CreateProc")]
#[must_use]
pub fn create_proc(buffer: &mut ReadBuffer<'_>, registry: &FlattenableRegistry) -> Option<Shader> {
    let shader = buffer.read_shader(registry)?;
    let filter = buffer.read_color_filter(registry);
    Some(ColorFilterShader::make(shader, 1.0, filter))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena_alloc::ArenaAlloc;
    use crate::color::{Color, colors};
    use crate::color_filter::tests::TestFilter;
    use crate::color_type::ColorType;
    use crate::matrix::Matrix;
    use crate::raster_pipeline::RasterPipeline;
    use crate::shaders;

    fn filter(clear: bool, alpha_unchanged: bool) -> ColorFilter {
        ColorFilter::from_base(TestFilter {
            clear,
            alpha_unchanged,
        })
    }

    fn stages(shader: &Shader) -> (bool, String) {
        let alloc = ArenaAlloc::new();
        let mut p = RasterPipeline::new();
        let mut rec = StageRec {
            pipeline: &mut p,
            alloc: &alloc,
            dst_color_type: ColorType::RGBA8888,
            dst_cs: None,
            paint_color: colors::BLACK,
            surface_props: crate::surface_props::SurfaceProps::default(),
            dst_bounds: crate::rect::Rect::new_empty(),
        };
        let ok = shader.as_base().append_root_stages(&mut rec, Matrix::i());
        (ok, p.to_string())
    }

    #[test]
    fn no_filter_is_the_shader() {
        let s = shaders::color(Color::new(0xFF11_2233));
        assert_eq!(ColorFilterShader::make(s.clone(), 0.5, None), s);
    }

    #[test]
    fn wraps_the_shader_and_filter() {
        let s = shaders::color(Color::new(0xFF11_2233));
        let f = filter(false, true);
        let cfs = s.with_color_filter(f.clone());
        assert_eq!(cfs.as_base().shader_type(), ShaderType::ColorFilter);
        let base: &dyn core::any::Any = cfs.as_base();
        let base = base.downcast_ref::<ColorFilterShader>().unwrap();
        assert_eq!(base.shader(), &s);
        assert_eq!(base.filter(), &f);
        #[allow(clippy::float_cmp)]
        {
            assert_eq!(base.alpha(), 1.0);
        }
    }

    #[test]
    fn is_opaque_needs_all_of_shader_alpha_and_filter() {
        let opaque = shaders::color(Color::new(0xFF11_2233));
        let translucent = shaders::color(Color::new(0x8011_2233));
        assert!(opaque.with_color_filter(filter(false, true)).is_opaque());
        assert!(!opaque.with_color_filter(filter(false, false)).is_opaque());
        assert!(
            !translucent
                .with_color_filter(filter(false, true))
                .is_opaque()
        );
        let scaled = ColorFilterShader::make(opaque, 0.5, Some(filter(false, true)));
        assert!(!scaled.is_opaque());
    }

    #[test]
    fn appends_the_shader_then_the_alpha_then_the_filter() {
        let s = shaders::color(Color::new(0xFF11_2233));
        let (ok, dump) = stages(&s.with_color_filter(filter(true, false)));
        assert!(ok);
        let names: Vec<&str> = dump
            .lines()
            .filter(|l| l.starts_with('\t'))
            .map(|l| l.split_whitespace().next().unwrap())
            .collect();
        assert_eq!(names.last(), Some(&"clear"));
        assert!(!names.contains(&"scale_1_float"));

        let scaled = ColorFilterShader::make(s, 0.25, Some(filter(true, false)));
        let (ok, dump) = stages(&scaled);
        assert!(ok);
        let names: Vec<&str> = dump
            .lines()
            .filter(|l| l.starts_with('\t'))
            .map(|l| l.split_whitespace().next().unwrap())
            .collect();
        let n = names.len();
        assert_eq!(&names[n - 2..], ["scale_1_float", "clear"]);

        // A shader that draws nothing makes the filter shader draw nothing.
        let empty = shaders::empty().with_color_filter(filter(true, false));
        assert!(!stages(&empty).0);
    }
}
