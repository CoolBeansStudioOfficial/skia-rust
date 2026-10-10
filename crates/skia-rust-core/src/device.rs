// Copyright 2010 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkDevice.h, src/core/SkDevice.cpp

//! `SkDevice`: the drawing target behind a canvas (or a layer of one).
//!
//! A device owns the device-space transform (`localToDevice`, `deviceToGlobal`,
//! `globalToDevice`: [`DeviceState`]), a clip stack and the drawing entry points of [`Device`].
//! The CPU device is `skia_rust_raster::bitmap_device::BitmapDevice`;
//! [`NoPixelsDevice`] only tracks clip bounds (pictures and queries use it).
//!
//! skia-rust: `SkDevice` is a trait ([`Device`]) over a [`DeviceState`] each implementation
//! embeds (`state`/`state_mut`). `SkDevice` is internal to Skia (`skia-safe` does not expose it),
//! so the names are mechanical. Everything that needs types ported later stays out of the trait
//! until its task adds it, each with a default body like Skia's: images,
//! meshes, text, drawables, shadows, special images, layers' `makeSurface` and `drawDevice`
//! (tasks D6/D7 and Phase 3, listed in `docs/design/raster-pipeline.md` "As implemented in
//! D5"). `SkRefCnt` is not modelled: a canvas owns its devices.

use crate::alpha_type::AlphaType;
use crate::arc::{Arc, create_draw_arc_path};
use crate::bitmap::Bitmap;
use crate::blend_mode::BlendMode;
use crate::blender::Blender;
use crate::canvas::{ImageSetEntry, PointMode, QuadAAFlags, SrcRectConstraint};
use crate::clip_op::ClipOp;
use crate::color::{Color, Color4f};
use crate::color_priv::{alpha_255_to_256, alpha_mul};
use crate::color_type::ColorType;
use crate::drawable::Drawable;
use crate::floating_point::float_round2int;
use crate::glyph_run::{GlyphRun, GlyphRunBuilder, GlyphRunList};
use crate::image::Image;
use crate::image_filter_types::Backend;
use crate::image_info::ImageInfo;
use crate::lattice_iter::{Lattice, LatticeIter};
use crate::m44::M44;
use crate::matrix::{Matrix, TypeMask};
use crate::matrix_priv::{is_scale_translate_as_m33, map_rect};
use crate::mesh::Mesh;
use crate::paint::{Paint, Style};
use crate::path::Path;
use crate::path_builder::PathBuilder;
use crate::path_types::{PathDirection, PathFillType};
use crate::pixmap::Pixmap;
use crate::point::{IPoint, Point};
use crate::rect::{IRect, Rect, RoundOut, rect_priv};
use crate::region::{Iterator as RegionIterator, Region};
use crate::rrect::RRect;
use crate::rsxform::RSXform;
use crate::sampling_options::{FilterMode, SamplingOptions};
use crate::scalar::scalar;
use crate::scaler_context::ScalerContextBuildFlags;
use crate::shader::Shader;
use crate::size::ISize;
use crate::slug::Slug;
use crate::special_image::SpecialImage;
use crate::surface_props::{PixelGeometry, SurfaceProps};
use crate::utils::patch_utils;
use crate::vertices::{Builder, BuilderFlags, VertexMode, Vertices};

/// The two triangles of a quad, as 6 points (`quad_to_tris`).
// Port of: src/core/SkDevice.cpp#L193-L205 (chrome/m156)
fn quad_to_tris(tris: &mut [Point], quad: &[Point; 4]) {
    tris[0] = quad[0];
    tris[1] = quad[1];
    tris[2] = quad[2];

    tris[3] = quad[0];
    tris[4] = quad[2];
    tris[5] = quad[3];
}

/// What [`Device::create_device`] is asked to make (`SkDevice::CreateInfo`).
///
/// skia-rust: `fAllocator` (`SkRasterHandleAllocator`) is not ported.
// Port of: src/core/SkDevice.h#L298-L310 (chrome/m156)
#[doc(alias = "SkDevice::CreateInfo")]
#[derive(Clone, Debug)]
pub struct CreateInfo {
    /// The image info of the new device (`fInfo`).
    pub info: ImageInfo,
    /// The pixel geometry of the new device's surface props (`fPixelGeometry`).
    pub pixel_geometry: PixelGeometry,
}

impl CreateInfo {
    /// `CreateInfo(info, geo, allocator)` without the allocator.
    #[must_use]
    pub fn new(info: ImageInfo, pixel_geometry: PixelGeometry) -> CreateInfo {
        CreateInfo {
            info,
            pixel_geometry,
        }
    }
}

/// The state every device has (the data members of `SkDevice`): image info, surface properties
/// and the three matrices.
// Port of: src/core/SkDevice.h#L107-L330 (chrome/m156)
#[doc(alias = "SkDevice")]
#[derive(Clone, Debug)]
pub struct DeviceState {
    info: ImageInfo,
    surface_props: SurfaceProps,
    local_to_device: M44,
    device_to_global: M44,
    global_to_device: M44,
    // Cached `fLocalToDevice.asM33()`.
    local_to_device33: Matrix,
    local_to_device_dirty: bool,
}

impl DeviceState {
    /// `SkDevice::SkDevice(info, surfaceProps)`: identity transforms.
    // Port of: src/core/SkDevice.cpp#L53-L58 (chrome/m156)
    #[must_use]
    pub fn new(info: ImageInfo, surface_props: SurfaceProps) -> DeviceState {
        let identity = M44::new_identity();
        DeviceState {
            info,
            surface_props,
            local_to_device: identity,
            device_to_global: identity,
            global_to_device: identity,
            local_to_device33: Matrix::new_identity(),
            local_to_device_dirty: true,
        }
    }

    /// The image info (`imageInfo`).
    #[doc(alias = "imageInfo")]
    #[must_use]
    pub fn image_info(&self) -> &ImageInfo {
        &self.info
    }

    /// The width in pixels.
    #[must_use]
    pub fn width(&self) -> i32 {
        self.info.width()
    }

    /// The height in pixels.
    #[must_use]
    pub fn height(&self) -> i32 {
        self.info.height()
    }

    /// Whether the device is opaque (`isOpaque`).
    #[doc(alias = "isOpaque")]
    #[must_use]
    pub fn is_opaque(&self) -> bool {
        self.info.is_opaque()
    }

    /// `(0, 0, width, height)`.
    #[must_use]
    pub fn bounds(&self) -> IRect {
        IRect::from_wh(self.width(), self.height())
    }

    /// The dimensions.
    #[must_use]
    pub fn size(&self) -> ISize {
        self.info.dimensions()
    }

    /// The surface properties (`surfaceProps`).
    #[doc(alias = "surfaceProps")]
    #[must_use]
    pub fn surface_props(&self) -> &SurfaceProps {
        &self.surface_props
    }

    /// The 4x4 local-to-device transform (`localToDevice44`).
    #[doc(alias = "localToDevice44")]
    #[must_use]
    pub fn local_to_device44(&self) -> &M44 {
        &self.local_to_device
    }

    /// The 3x3 local-to-device transform (`localToDevice`).
    #[doc(alias = "localToDevice")]
    #[must_use]
    pub fn local_to_device(&self) -> &Matrix {
        &self.local_to_device33
    }

    /// The device-to-global transform (`deviceToGlobal`).
    #[doc(alias = "deviceToGlobal")]
    #[must_use]
    pub fn device_to_global(&self) -> &M44 {
        &self.device_to_global
    }

    /// The global-to-device transform (`globalToDevice`).
    #[doc(alias = "globalToDevice")]
    #[must_use]
    pub fn global_to_device(&self) -> &M44 {
        &self.global_to_device
    }

    /// Sets the local-to-device transform (`setLocalToDevice`).
    // Port of: src/core/SkDevice.h#L170-L174 (chrome/m156)
    #[doc(alias = "setLocalToDevice")]
    pub fn set_local_to_device(&mut self, local_to_device: &M44) {
        self.local_to_device = *local_to_device;
        self.local_to_device33 = self.local_to_device.to_m33();
        self.local_to_device_dirty = true;
    }

    /// Sets the three transforms and the origin of the device's pixel buffer in the global
    /// coordinate system (`setDeviceCoordinateSystem`).
    // Port of: src/core/SkDevice.cpp#L60-L80 (chrome/m156)
    #[doc(alias = "setDeviceCoordinateSystem")]
    pub fn set_device_coordinate_system(
        &mut self,
        device_to_global: &M44,
        global_to_device: &M44,
        local_to_device: &M44,
        buffer_origin_x: i32,
        buffer_origin_y: i32,
    ) {
        self.device_to_global = *device_to_global;
        self.device_to_global.normalize_perspective();
        self.global_to_device = *global_to_device;
        self.global_to_device.normalize_perspective();

        self.local_to_device = *local_to_device;
        self.local_to_device.normalize_perspective();
        if (buffer_origin_x | buffer_origin_y) != 0 {
            #[allow(clippy::cast_precision_loss)] // mirrors the implicit int -> float conversion
            {
                self.device_to_global.pre_translate(
                    buffer_origin_x as scalar,
                    buffer_origin_y as scalar,
                    None,
                );
                self.global_to_device.post_translate(
                    -buffer_origin_x as scalar,
                    -buffer_origin_y as scalar,
                    None,
                );
                self.local_to_device.post_translate(
                    -buffer_origin_x as scalar,
                    -buffer_origin_y as scalar,
                    None,
                );
            }
        }
        self.local_to_device33 = self.local_to_device.to_m33();
        self.local_to_device_dirty = true;
    }

    /// `setOrigin`: the coordinate system of a device whose pixel buffer starts at `(x, y)` in a
    /// canvas whose current transform is `global_ctm`.
    // Port of: src/core/SkDevice.h#L253-L255 (chrome/m156)
    #[doc(alias = "setOrigin")]
    pub fn set_origin(&mut self, global_ctm: &M44, x: i32, y: i32) {
        self.set_device_coordinate_system(
            &M44::new_identity(),
            &M44::new_identity(),
            global_ctm,
            x,
            y,
        );
    }

    /// Sets the local-to-device transform from the canvas's global CTM (`setGlobalCTM`).
    // Port of: src/core/SkDevice.cpp#L82-L89 (chrome/m156)
    #[doc(alias = "setGlobalCTM")]
    pub fn set_global_ctm(&mut self, ctm: &M44) {
        self.local_to_device = *ctm;
        self.local_to_device.normalize_perspective();
        // Map from the global CTM state to this device's coordinate system.
        let global_to_device = self.global_to_device;
        self.local_to_device.post_concat(&global_to_device);
        self.local_to_device33 = self.local_to_device.to_m33();
        self.local_to_device_dirty = true;
    }

    /// True if the device-to-global transform is the identity plus an integer translation
    /// (`isPixelAlignedToGlobal`).
    // Port of: src/core/SkDevice.cpp#L91-L98 (chrome/m156)
    #[doc(alias = "isPixelAlignedToGlobal")]
    #[must_use]
    pub fn is_pixel_aligned_to_global(&self) -> bool {
        // pixelAligned is set to the identity + integer translation of the device-to-global
        // matrix. If they are equal then the device is by definition pixel aligned.
        let mut pixel_aligned = M44::new_identity();
        pixel_aligned.set_rc(0, 3, self.device_to_global.rc(0, 3).floor());
        pixel_aligned.set_rc(1, 3, self.device_to_global.rc(1, 3).floor());
        pixel_aligned == self.device_to_global
    }

    /// The origin of the device in the global space (`getOrigin`); the device must be pixel
    /// aligned.
    // Port of: src/core/SkDevice.cpp#L100-L109 (chrome/m156)
    #[doc(alias = "getOrigin")]
    #[must_use]
    pub fn origin(&self) -> IPoint {
        debug_assert!(self.is_pixel_aligned_to_global());
        IPoint::new(
            crate::floating_point::float_floor2int(self.device_to_global.rc(0, 3)),
            crate::floating_point::float_floor2int(self.device_to_global.rc(1, 3)),
        )
    }

    /// The transform from this device's space to `dst_device`'s (`getRelativeTransform`).
    // Port of: src/core/SkDevice.cpp#L111-L115 (chrome/m156)
    #[doc(alias = "getRelativeTransform")]
    #[must_use]
    pub fn relative_transform(&self, dst_device: &DeviceState) -> M44 {
        // To get the transform from this space to the other device's, transform from our space to
        // global and then from global to the other device.
        &dst_device.global_to_device * &self.device_to_global
    }

    /// Returns whether the local-to-device transform changed since the last call and clears the
    /// flag (`checkLocalToDeviceDirty`).
    // Port of: src/core/SkDevice.h#L257-L261 (chrome/m156)
    #[doc(alias = "checkLocalToDeviceDirty")]
    pub fn check_local_to_device_dirty(&mut self) -> bool {
        std::mem::take(&mut self.local_to_device_dirty)
    }
}

// Port of: src/core/SkDevice.cpp#L117-L119 (chrome/m156)
fn is_int(x: scalar) -> bool {
    #[allow(clippy::float_cmp, clippy::cast_precision_loss)] // mirrors `x == (float) round2int(x)`
    {
        x == float_round2int(x) as scalar
    }
}

/// The virtual interface of `SkDevice`, over the [`DeviceState`] each device embeds.
// Port of: src/core/SkDevice.h#L107-L560 (chrome/m156)
#[doc(alias = "SkDevice")]
pub trait Device {
    /// The state shared by all devices.
    fn state(&self) -> &DeviceState;
    /// The mutable state shared by all devices.
    fn state_mut(&mut self) -> &mut DeviceState;

    /// The bounds of the clip in device coordinates (`devClipBounds`).
    #[doc(alias = "devClipBounds")]
    fn dev_clip_bounds(&self) -> IRect;

    /// Saves the clip (`pushClipStack`).
    #[doc(alias = "pushClipStack")]
    fn push_clip_stack(&mut self);
    /// Restores the clip (`popClipStack`).
    #[doc(alias = "popClipStack")]
    fn pop_clip_stack(&mut self);

    /// Clips to `rect`, transformed by the local-to-device matrix (`clipRect`).
    #[doc(alias = "clipRect")]
    fn clip_rect(&mut self, rect: &Rect, op: ClipOp, aa: bool);
    /// Clips to `rrect` (`clipRRect`).
    #[doc(alias = "clipRRect")]
    fn clip_rrect(&mut self, rrect: &RRect, op: ClipOp, aa: bool);
    /// Clips to `path` (`clipPath`).
    #[doc(alias = "clipPath")]
    fn clip_path(&mut self, path: &Path, op: ClipOp, aa: bool);
    /// Clips to `region`, in global coordinates (`clipRegion`).
    #[doc(alias = "clipRegion")]
    fn clip_region(&mut self, region: &Region, op: ClipOp);
    /// Adds a clip shader (the virtual `onClipShader`; the shader already includes the CTM and,
    /// for a difference clip, an inverted alpha).
    #[doc(alias = "onClipShader")]
    fn on_clip_shader(&mut self, shader: Shader);
    /// Replaces the clip with `rect`, in global coordinates (`replaceClip`).
    #[doc(alias = "replaceClip")]
    fn replace_clip(&mut self, rect: &IRect);

    /// Whether the clip is anti-aliased (`isClipAntiAliased`).
    #[doc(alias = "isClipAntiAliased")]
    fn is_clip_anti_aliased(&self) -> bool;
    /// Whether the clip is empty (`isClipEmpty`).
    #[doc(alias = "isClipEmpty")]
    fn is_clip_empty(&self) -> bool;
    /// Whether the clip is a non-empty rectangle (`isClipRect`).
    #[doc(alias = "isClipRect")]
    fn is_clip_rect(&self) -> bool;
    /// Whether the clip is the whole device (`isClipWideOpen`).
    #[doc(alias = "isClipWideOpen")]
    fn is_clip_wide_open(&self) -> bool;
    /// Sets `rgn` to the clip, with anti-aliasing dropped (`android_utils_clipAsRgn`).
    fn android_utils_clip_as_rgn(&self, rgn: &mut Region);

    /// Whether this device has no pixels (`isNoPixelsDevice`).
    #[doc(alias = "isNoPixelsDevice")]
    fn is_no_pixels_device(&self) -> bool {
        false
    }

    /// Resets a no-pixels device for a new picture with `bounds` of the same size; false if this
    /// is not a no-pixels device or the size differs (`SkNoPixelsDevice::resetForNextPicture`).
    /// skia-rust: a trait method because `SkCanvas::resetForNextPicture` casts its root device.
    #[doc(alias = "resetForNextPicture")]
    fn reset_for_next_picture(&mut self, _bounds: &IRect) -> bool {
        false
    }

    /// Marks the device's pixels immutable (`setImmutable`).
    #[doc(alias = "setImmutable")]
    fn set_immutable(&mut self) {}

    /// Makes a device for a layer (`createDevice`); `None` if this device cannot.
    #[doc(alias = "createDevice")]
    fn create_device(
        &mut self,
        _info: &CreateInfo,
        _layer_paint: Option<&Paint>,
    ) -> Option<Box<dyn Device>> {
        None
    }

    /// Fills the clip with `paint` (`drawPaint`).
    #[doc(alias = "drawPaint")]
    fn draw_paint(&mut self, paint: &Paint);
    /// Draws points, lines or a polyline (`drawPoints`).
    #[doc(alias = "drawPoints")]
    fn draw_points(&mut self, mode: PointMode, points: &[crate::point::Point], paint: &Paint);
    /// Draws a rectangle (`drawRect`).
    #[doc(alias = "drawRect")]
    fn draw_rect(&mut self, r: &Rect, paint: &Paint);
    /// Draws an oval (`drawOval`).
    #[doc(alias = "drawOval")]
    fn draw_oval(&mut self, oval: &Rect, paint: &Paint);
    /// Draws a round rect (`drawRRect`).
    #[doc(alias = "drawRRect")]
    fn draw_rrect(&mut self, rr: &RRect, paint: &Paint);
    /// Draws a blurred round rect analytically, if the device can (`drawBlurredRRect`). Returns
    /// false when it did not draw, and the canvas then draws the shape another way. Only devices
    /// that use coverage masks for mask filters are asked (`useDrawCoverageMaskForMaskFilters`).
    // Port of: src/core/SkDevice.h#L468-L470 (chrome/m156)
    #[doc(alias = "drawBlurredRRect")]
    fn draw_blurred_rrect(
        &mut self,
        _rrect: &RRect,
        _paint: &Paint,
        _local_sigma: crate::m44::V2,
        _device_sigma: crate::scalar::scalar,
    ) -> bool {
        false
    }
    /// Draws a path (`drawPath`).
    #[doc(alias = "drawPath")]
    fn draw_path(&mut self, path: &Path, paint: &Paint);

    /// `SkDevice::onDrawGlyphRunList` (pure virtual in C++): draws the runs of `list`, none of
    /// which has `RSXform`s, with the device's glyph painter.
    // Port of: src/core/SkDevice.h#L535-L537 (chrome/m156)
    #[doc(alias = "onDrawGlyphRunList")]
    fn on_draw_glyph_run_list(&mut self, list: &GlyphRunList<'_>, paint: &Paint);

    /// Draws `list` up to its next glyph drawable and returns that drawable instead of drawing
    /// it; the caller draws the drawable and calls again with the same `cursor` to continue
    /// with the glyphs after it. `None` means the list is drawn completely.
    ///
    /// skia-rust: Skia's glyph painter draws a glyph drawable with the *canvas*
    /// (`canvas->saveLayer(); drawable->draw(canvas)`) in the middle of the run, between the
    /// path glyphs and the mask glyphs. A Rust device cannot reach the canvas that is drawing
    /// it, so the painter yields at each drawable and the canvas draws it, which keeps Skia's
    /// draw order. The default draws the whole list with
    /// [`on_draw_glyph_run_list`](Self::on_draw_glyph_run_list).
    fn on_draw_glyph_run_list_step(
        &mut self,
        list: &GlyphRunList<'_>,
        paint: &Paint,
        cursor: &mut GlyphRunDrawCursor,
    ) -> Option<PendingGlyphDrawable> {
        let _ = cursor;
        self.on_draw_glyph_run_list(list, paint);
        None
    }

    /// `SkDevice::convertGlyphRunListToSlug`: a slug of the glyphs of `list`, drawn with
    /// `paint`. Only GPU devices make slugs; every other device returns `None`.
    // Port of: src/core/SkDevice.cpp#L481-L484 (chrome/m156)
    #[doc(alias = "convertGlyphRunListToSlug")]
    fn convert_glyph_run_list_to_slug(
        &mut self,
        _list: &GlyphRunList<'_>,
        _paint: &Paint,
    ) -> Option<Slug> {
        None
    }

    /// `SkDevice::drawSlug`: draws a slug the device made. Skia aborts ("Slug drawing not
    /// supported.") on a device that never makes slugs; here nothing is drawn.
    // Port of: src/core/SkDevice.cpp#L486-L488 (chrome/m156)
    #[doc(alias = "drawSlug")]
    fn draw_slug(&mut self, _slug: &Slug, _paint: &Paint) {}

    /// `SkDevice::scalerContextFlags`: the flags of the glyph masks drawn on this device. A
    /// linear color space drops the gamma hacks; otherwise they stay on, and the contrast boost
    /// always applies.
    // Port of: src/core/SkDevice.cpp#L496-L506 (chrome/m156)
    #[doc(alias = "scalerContextFlags")]
    fn scaler_context_flags(&self) -> ScalerContextBuildFlags {
        match self.state().image_info().color_space() {
            Some(cs) if cs.gamma_is_linear() => ScalerContextBuildFlags::BOOST_CONTRAST,
            _ => ScalerContextBuildFlags::FAKE_GAMMA_AND_BOOST_CONTRAST,
        }
    }

    /// Draws a region (`drawRegion`).
    // Port of: src/core/SkDevice.cpp#L121-L141 (chrome/m156)
    #[doc(alias = "drawRegion")]
    fn draw_region(&mut self, region: &Region, paint: &Paint) {
        let local_to_device = self.state().local_to_device().clone();
        let is_non_translate = local_to_device
            .get_type()
            .intersects(TypeMask::all() - TypeMask::TRANSLATE);
        let complex_paint = paint.style() != Style::Fill
            || paint.mask_filter().is_some()
            || paint.path_effect().is_some();
        let anti_alias = paint.is_anti_alias()
            && (!is_int(local_to_device.translate_x()) || !is_int(local_to_device.translate_y()));
        if is_non_translate || complex_paint || anti_alias {
            let mut builder = PathBuilder::new();
            region.add_boundary_path(&mut builder);
            builder.set_is_volatile(true);
            let path = builder.detach();
            return self.draw_path(&path, paint);
        }

        let mut it = RegionIterator::new(region);
        while !it.is_done() {
            let r = Rect::from_irect(it.rect());
            self.draw_rect(&r, paint);
            it.next();
        }
    }

    /// Draws the area between two round rects (`drawDRRect`).
    // Port of: src/core/SkDevice.cpp#L143-L155 (chrome/m156)
    #[doc(alias = "drawDRRect")]
    fn draw_drrect(&mut self, outer: &RRect, inner: &RRect, paint: &Paint) {
        let mut builder = PathBuilder::new();
        builder.add_rrect(outer, None, None);
        builder.add_rrect(inner, None, None);
        builder.set_fill_type(PathFillType::EvenOdd);
        builder.set_is_volatile(true);
        let path = builder.detach();

        self.draw_path(&path, paint);
    }

    /// Draws an arc or wedge (`drawArc`).
    // Port of: src/core/SkDevice.cpp#L137-L141 (chrome/m156)
    #[doc(alias = "drawArc")]
    fn draw_arc(&mut self, arc: &Arc, paint: &Paint) {
        let is_fill_no_path_effect = Style::Fill == paint.style() && paint.path_effect().is_none();
        let path = create_draw_arc_path(arc, is_fill_no_path_effect);
        self.draw_path(&path, paint);
    }

    /// Draws triangles from `vertices` with `blender` combining the vertex colors with the
    /// paint's shader (or its opaque color); `blender` is ignored if there are no vertex colors
    /// (`drawVertices`). If `skip_color_xform` is true, then the implementation should assume
    /// that the provided vertex colors are already in the destination color space.
    #[doc(alias = "drawVertices")]
    fn draw_vertices(
        &mut self,
        vertices: &Vertices,
        blender: Blender,
        paint: &Paint,
        skip_color_xform: bool,
    );

    /// Draws a custom mesh (`drawMesh`). The blender combines the mesh's output with the
    /// destination; the canvas makes it [`Blender::mode`]`(kModulate)` when none is given.
    // Port of: src/core/SkDevice.h#L387 (chrome/m156)
    #[doc(alias = "drawMesh")]
    fn draw_mesh(&mut self, mesh: &Mesh, blender: Blender, paint: &Paint);

    /// Draws a Coons patch (`drawPatch`). The default makes vertices and calls
    /// [`draw_vertices`](Self::draw_vertices).
    // Port of: src/core/SkDevice.cpp#L154-L163 (chrome/m156)
    #[doc(alias = "drawPatch")]
    fn draw_patch(
        &mut self,
        cubics: &[Point; patch_utils::NUM_CTRL_PTS],
        colors: Option<&[Color; patch_utils::NUM_CORNERS]>,
        tex_coords: Option<&[Point; patch_utils::NUM_CORNERS]>,
        blender: Blender,
        paint: &Paint,
    ) {
        let lod = patch_utils::get_level_of_detail(cubics, self.state().local_to_device());
        let color_space = self.state().image_info().color_space();
        let vertices = patch_utils::make_vertices(
            cubics,
            colors,
            tex_coords,
            lod.width,
            lod.height,
            color_space.as_ref(),
        );
        if let Some(vertices) = vertices {
            self.draw_vertices(&vertices, blender, paint, false);
        }
    }

    /// Draws the `tex` rectangles of the atlas shader of `paint`, each transformed by the
    /// matching `xform` and (if `colors` is not empty) blended with its color
    /// (`drawAtlas`). The default makes vertices and calls
    /// [`draw_vertices`](Self::draw_vertices).
    // Port of: src/core/SkDevice.cpp#L207-L237 (chrome/m156)
    #[doc(alias = "drawAtlas")]
    fn draw_atlas(
        &mut self,
        xform: &[RSXform],
        tex: &[Rect],
        colors: &[Color],
        blender: Blender,
        paint: &Paint,
    ) {
        let quad_count = xform.len();
        let tri_count = quad_count << 1;
        let vertex_count = tri_count * 3;
        let mut flags = BuilderFlags::HAS_TEX_COORDS;
        if !colors.is_empty() {
            flags |= BuilderFlags::HAS_COLORS;
        }
        let mut builder = Builder::new(VertexMode::Triangles, vertex_count, 0, flags);

        for (i, (xform, tex)) in xform.iter().zip(tex).enumerate() {
            let tmp = xform.to_quad((tex.width(), tex.height()));
            quad_to_tris(&mut builder.positions()[6 * i..6 * i + 6], &tmp);

            let tex_quad = tex.to_quad(PathDirection::CW);
            if let Some(v_tex) = builder.tex_coords() {
                quad_to_tris(&mut v_tex[6 * i..6 * i + 6], &tex_quad);
            }

            if !colors.is_empty()
                && let Some(v_col) = builder.colors()
            {
                v_col[6 * i..6 * i + 6].fill(colors[i]);
            }
        }
        if let Some(vertices) = builder.detach() {
            self.draw_vertices(&vertices, blender, paint, false);
        }
    }

    /// The backend that image filters evaluated over this device use (`createImageFilteringBackend`).
    ///
    /// Skia's default is its raster backend (`skif::MakeRasterBackend`), which lives in
    /// `skia-rust-raster` next to `BitmapDevice`. This default is `None`: the filter is then not
    /// evaluated, which is observably the same for the devices that do not override it (they
    /// draw nothing). Pixel devices override it.
    // Port of: src/core/SkDevice.cpp#L322-L325 (chrome/m156)
    #[doc(alias = "createImageFilteringBackend")]
    fn create_image_filtering_backend(
        &self,
        _surface_props: &SurfaceProps,
        _color_type: ColorType,
    ) -> Option<std::sync::Arc<dyn Backend>> {
        None
    }

    /// Whether `SkCanvas` should simulate mask filters with a layer and `drawCoverageMask`
    /// (`useDrawCoverageMaskForMaskFilters`; false for the raster device).
    #[doc(alias = "useDrawCoverageMaskForMaskFilters")]
    fn use_draw_coverage_mask_for_mask_filters(&self) -> bool {
        false
    }

    /// Draws the `src` rect of `image` (all of it if `None`) into the `dst` rect with
    /// `sampling` (`drawImageRect`). `dst` is finite and sorted.
    // Port of: src/core/SkDevice.h#L362-L364 (chrome/m156)
    #[doc(alias = "drawImageRect")]
    fn draw_image_rect(
        &mut self,
        image: &Image,
        src: Option<&Rect>,
        dst: &Rect,
        sampling: &SamplingOptions,
        paint: &Paint,
        constraint: SrcRectConstraint,
    );

    /// Draws a solid-color quad, `rect` clipped by `clip` if it is `Some`, with each edge
    /// anti-aliased per `aa_flags` (`drawEdgeAAQuad`). The default draws `rect` (or the clip
    /// polygon) with a solid paint, anti-aliased only when all four edges are.
    // Port of: src/core/SkDevice.cpp#L239-L252 (chrome/m156)
    #[doc(alias = "drawEdgeAAQuad")]
    fn draw_edge_aa_quad(
        &mut self,
        rect: &Rect,
        clip: Option<&[Point; 4]>,
        aa_flags: QuadAAFlags,
        color: Color4f,
        mode: BlendMode,
    ) {
        let mut paint = Paint::default();
        paint.set_color4f(color, None);
        paint.set_blend_mode(mode);
        paint.set_anti_alias(aa_flags == QuadAAFlags::ALL);

        if let Some(clip) = clip {
            // Draw the clip directly as a quad since it's a filled color with no local coords
            self.draw_path(&Path::polygon(clip, true, None, None), &paint);
        } else {
            self.draw_rect(rect, &paint);
        }
    }

    /// Draws the entries of an image set (`drawEdgeAAImageSet`). The default draws each entry
    /// with `draw_image_rect`, anti-aliased only when all of its edges are, and applies its clip
    /// with `clip_path`. `dst_clips` and `preview_matrices` are empty for C++'s null arrays.
    // Port of: src/core/SkDevice.cpp#L254-L294 (chrome/m156)
    #[doc(alias = "drawEdgeAAImageSet")]
    fn draw_edge_aa_image_set(
        &mut self,
        images: &[ImageSetEntry],
        dst_clips: &[Point],
        preview_matrices: &[Matrix],
        sampling: &SamplingOptions,
        paint: &Paint,
        constraint: SrcRectConstraint,
    ) {
        debug_assert_eq!(paint.style(), Style::Fill);
        debug_assert!(paint.path_effect().is_none());

        let mut entry_paint = paint.clone();
        let base_local_to_device = *self.state().local_to_device44();
        let mut clip_index = 0;
        for image in images {
            // TODO: Handle per-edge AA. Right now this mirrors the SkiaRenderer component of
            // Chrome which turns off antialiasing unless all four edges should be antialiased.
            // This avoids seaming in tiled composited layers.
            entry_paint.set_anti_alias(image.aa_flags == QuadAAFlags::ALL);
            entry_paint.set_alpha_f(paint.alpha_f() * image.alpha);

            if let Some(i) = image.matrix_index {
                let m = M44::concat(&base_local_to_device, &M44::from(&preview_matrices[i]));
                self.state_mut().set_local_to_device(&m);
            }

            if image.has_clip {
                // Since drawImageRect requires a srcRect, the dst clip is implemented as a true
                // clip
                self.push_clip_stack();
                let clip_path =
                    Path::polygon(&dst_clips[clip_index..clip_index + 4], true, None, None);
                self.clip_path(&clip_path, ClipOp::Intersect, entry_paint.is_anti_alias());
                clip_index += 4;
            }
            self.draw_image_rect(
                &image.image,
                Some(&image.src_rect),
                &image.dst_rect,
                sampling,
                &entry_paint,
                constraint,
            );
            if image.has_clip {
                self.pop_clip_stack();
            }
            if image.matrix_index.is_some() {
                self.state_mut().set_local_to_device(&base_local_to_device);
            }
        }
    }

    /// Draws `image` divided by `lattice` into patches, stretched to fit `dst`
    /// (`drawImageLattice`).
    // Port of: src/core/SkDevice.cpp#L165-L205 (chrome/m156)
    #[doc(alias = "drawImageLattice")]
    fn draw_image_lattice(
        &mut self,
        image: &Image,
        lattice: &Lattice<'_>,
        dst: &Rect,
        filter: FilterMode,
        paint: &Paint,
    ) {
        let mut iter = LatticeIter::new(lattice, dst);

        let info = ImageInfo::new((1, 1), ColorType::BGRA8888, AlphaType::Unpremul, None);

        while let Some(patch) = iter.next_patch() {
            let src_r = Rect::from_irect(patch.src);
            let dst_r = patch.dst;
            let mut color = patch.fixed_color;
            let mut fast = color.is_some();
            // TODO: support this fast-path for GPU images
            if !fast && src_r.width() <= 1.0 && src_r.height() <= 1.0 {
                let mut pixel = [0u8; 4];
                if image.read_pixels(&info, &mut pixel, 4, (patch.src.left, patch.src.top)) {
                    color = Some(Color::new(u32::from_ne_bytes(pixel)));
                    fast = true;
                }
            }
            if fast {
                let c = color.unwrap_or_default();
                // Fast draw with drawRect, if this is a patch containing a single color
                // or if this is a patch containing a single pixel.
                if c != Color::new(0) || !paint.is_src_over() {
                    let mut paint_copy = paint.clone();
                    let alpha = alpha_mul(
                        i32::from(c.a()),
                        i32::try_from(alpha_255_to_256(u32::from(paint.alpha()))).unwrap_or(256),
                    );
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    // SkColorSetA takes a U8CPU and masks it to 8 bits
                    paint_copy.set_color(c.with_a(alpha as u8));
                    self.draw_rect(&dst_r, &paint_copy);
                }
            } else {
                self.draw_image_rect(
                    image,
                    Some(&src_r),
                    &dst_r,
                    &SamplingOptions::from(filter),
                    paint,
                    SrcRectConstraint::Strict,
                );
            }
        }
    }

    /// Draws `src` with `local_to_device` as the matrix (`drawSpecial`). The default draws
    /// nothing.
    #[doc(alias = "drawSpecial")]
    fn draw_special(
        &mut self,
        _src: &SpecialImage,
        _local_to_device: &Matrix,
        _sampling: &SamplingOptions,
        _paint: &Paint,
        _constraint: SrcRectConstraint,
    ) {
    }

    /// A special image of `subset` scaled to `dst_dims` (`snapSpecialScaled`); `None` by default.
    // Port of: src/core/SkDevice.cpp#L314-L317 (chrome/m156)
    #[doc(alias = "snapSpecialScaled")]
    fn snap_special_scaled(&mut self, _subset: &IRect, _dst_dims: ISize) -> Option<SpecialImage> {
        None
    }

    /// A special image of the `bounds` of the device's pixels, copied if `force_copy`
    /// (`snapSpecial`); `None` if the device cannot.
    #[doc(alias = "snapSpecial")]
    fn snap_special(&mut self, _bounds: &IRect, _force_copy: bool) -> Option<SpecialImage> {
        None
    }

    /// A special image of the whole device (`snapSpecial()`).
    // Port of: src/core/SkDevice.cpp#L318-L320 (chrome/m156)
    #[doc(alias = "snapSpecial")]
    fn snap_special_all(&mut self) -> Option<SpecialImage> {
        let bounds = IRect::from_wh(self.state().width(), self.state().height());
        self.snap_special(&bounds, false)
    }

    /// Draws the pixels of another device (a layer being restored) into this one
    /// (`drawDevice`).
    // Port of: src/core/SkDevice.cpp#L327-L343 (chrome/m156)
    #[doc(alias = "drawDevice")]
    fn draw_device(&mut self, device: &mut dyn Device, sampling: &SamplingOptions, paint: &Paint) {
        let Some(device_image) = device.snap_special_all() else {
            return;
        };
        // SkCanvas only calls drawDevice() when there are no filters (so the transform is pixel
        // aligned). As such it can be drawn without clamping.
        let relative_transform = device.state().relative_transform(self.state()).to_m33();
        let strict = sampling.filter != FilterMode::Nearest
            || sampling.use_cubic
            || sampling.mipmap != crate::sampling_options::MipmapMode::None
            || sampling.is_aniso()
            || !relative_transform.is_translate()
            || !is_int(relative_transform.translate_x())
            || !is_int(relative_transform.translate_y());
        self.draw_special(
            &device_image,
            &relative_transform,
            sampling,
            paint,
            if strict {
                SrcRectConstraint::Strict
            } else {
                SrcRectConstraint::Fast
            },
        );
    }

    /// The bitmap a raster device draws into, if it is one (the hook `Canvas` uses to hand a
    /// wrapped bitmap back to its owner).
    ///
    /// skia-rust: Skia shares the pixel ref between the caller's bitmap and the device; here a
    /// canvas that wraps caller pixels owns them for its lifetime (`docs/design/pixels.md`).
    fn bitmap_mut(&mut self) -> Option<&mut Bitmap> {
        None
    }

    /// Copies pixels out of the device (the virtual `onReadPixels`); `false` if unsupported.
    #[doc(alias = "onReadPixels")]
    fn on_read_pixels(&mut self, _dst: &mut Pixmap<'_>, _x: i32, _y: i32) -> bool {
        false
    }
    /// Copies pixels into the device (the virtual `onWritePixels`); `false` if unsupported.
    #[doc(alias = "onWritePixels")]
    fn on_write_pixels(&mut self, _src: &Pixmap<'_>, _x: i32, _y: i32) -> bool {
        false
    }
    /// The device's pixels, for writing, with the pixel generation bumped (the virtual
    /// `onAccessPixels`).
    #[doc(alias = "onAccessPixels")]
    fn on_access_pixels(&mut self) -> Option<Pixmap<'_>> {
        None
    }
    /// The device's pixels, for reading (the virtual `onPeekPixels`).
    #[doc(alias = "onPeekPixels")]
    fn on_peek_pixels(&self) -> Option<Pixmap<'_>> {
        None
    }

    /// Writes `src` into the device at `(x, y)` (`writePixels`).
    // Port of: src/core/SkDevice.h#L130 (chrome/m156)
    #[doc(alias = "writePixels")]
    fn write_pixels(&mut self, src: &Pixmap<'_>, x: i32, y: i32) -> bool {
        self.on_write_pixels(src, x, y)
    }
    /// Reads the device at `(x, y)` into `dst` (`readPixels`).
    // Port of: src/core/SkDevice.h#L137 (chrome/m156)
    #[doc(alias = "readPixels")]
    fn read_pixels(&mut self, dst: &mut Pixmap<'_>, x: i32, y: i32) -> bool {
        self.on_read_pixels(dst, x, y)
    }
    /// The device's pixels for writing (`accessPixels`).
    // Port of: src/core/SkDevice.cpp#L379-L386 (chrome/m156)
    #[doc(alias = "accessPixels")]
    fn access_pixels(&mut self) -> Option<Pixmap<'_>> {
        self.on_access_pixels()
    }
    /// The device's pixels for reading (`peekPixels`).
    // Port of: src/core/SkDevice.cpp#L388-L395 (chrome/m156)
    #[doc(alias = "peekPixels")]
    fn peek_pixels(&self) -> Option<Pixmap<'_>> {
        self.on_peek_pixels()
    }
}

/// Clips `device` to `sh` (`SkDevice::clipShader`): the shader keeps the device's current
/// local-to-device matrix, and for a difference clip its alpha is inverted.
// Port of: src/core/SkDevice.h#L251-L257 (chrome/m156)
#[doc(alias = "clipShader")]
pub fn clip_shader(device: &mut dyn Device, sh: &Shader, op: ClipOp) {
    let mut sh = sh.make_with_ctm(device.state().local_to_device());
    if op == ClipOp::Difference {
        sh = sh.make_invert_alpha();
    }
    device.on_clip_shader(sh);
}

/// A glyph drawable a device collected while drawing a glyph run list: the drawable, the matrix
/// that places it relative to the canvas, and the paint of the text
/// (`Device::on_draw_glyph_run_list_step`).
#[derive(Clone, Debug)]
pub struct PendingGlyphDrawable {
    /// The drawable of the glyph (`glyph->drawable()`).
    pub drawable: Drawable,
    /// The matrix from the drawable's space to the canvas's (`m` in `drawForBitmapDevice`).
    pub matrix: Matrix,
    /// The paint the text is drawn with.
    pub paint: Paint,
}

/// `SkDevice::drawGlyphRunList` without the canvas: draws `list` on `device`, through
/// [`Device::on_draw_glyph_run_list`], or through [`simplify_glyph_run_rsxform_and_redraw`] when a
/// run has `RSXform`s. Nothing is drawn when the local-to-device matrix is not finite.
// Port of: src/core/SkDevice.cpp#L424-L435 (chrome/m156)
#[doc(alias = "drawGlyphRunList")]
pub fn draw_glyph_run_list(device: &mut dyn Device, list: &GlyphRunList<'_>, paint: &Paint) {
    if !device.state().local_to_device().is_finite() {
        return;
    }
    if list.has_rsxform() {
        simplify_glyph_run_rsxform_and_redraw(device, list, paint);
    } else {
        device.on_draw_glyph_run_list(list, paint);
    }
}

/// Where a glyph run list draw stopped at a glyph drawable, so that
/// [`Device::on_draw_glyph_run_list_step`] continues right after it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GlyphRunDrawCursor {
    /// The tile of the device draw that was being drawn (`DrawTiler`); 0 without tiling.
    pub tile: usize,
    /// The run that was being drawn.
    pub run: usize,
    /// How many drawables of that run were returned already; `None` when the run has not
    /// started.
    pub drawables_done: Option<usize>,
    /// The progress through a list with `RSXform`s (`simplifyGlyphRunRSXFormAndRedraw`); `None`
    /// before such a list starts, and always `None` for a list without `RSXform`s.
    pub rsxform: Option<Box<RSXformDrawCursor>>,
}

/// Where a glyph run list with `RSXform`s stopped at a glyph drawable (see
/// [`GlyphRunDrawCursor::rsxform`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RSXformDrawCursor {
    /// The run that is being drawn.
    pub run: usize,
    /// The glyph of `run` that is being drawn, when the run has rotations.
    pub glyph: usize,
    /// The cursor of the sub-list that is being drawn.
    pub sub: GlyphRunDrawCursor,
}

/// [`draw_glyph_run_list`] up to the next glyph drawable (see
/// [`Device::on_draw_glyph_run_list_step`]). Start with a default `cursor` and call again with
/// the same one until it returns `None`.
// Port of: src/core/SkDevice.cpp#L424-L435 (chrome/m156)
pub fn draw_glyph_run_list_step(
    device: &mut dyn Device,
    list: &GlyphRunList<'_>,
    paint: &Paint,
    cursor: &mut GlyphRunDrawCursor,
) -> Option<PendingGlyphDrawable> {
    if !device.state().local_to_device().is_finite() {
        return None;
    }
    if list.has_rsxform() {
        simplify_glyph_run_rsxform_step(device, list, paint, cursor)
    } else {
        device.on_draw_glyph_run_list_step(list, paint, cursor)
    }
}

/// `SkDevice::simplifyGlyphRunRSXFormAndRedraw` without the canvas: draws each `RSXform` glyph
/// as its own run, with the device transform set to the glyph's rotation-scale and translation.
/// Glyph drawables are skipped, since only a canvas can draw them (see
/// [`simplify_glyph_run_rsxform_step`]).
// Port of: src/core/SkDevice.cpp#L438-L479 (chrome/m156)
#[doc(alias = "simplifyGlyphRunRSXFormAndRedraw")]
pub fn simplify_glyph_run_rsxform_and_redraw(
    device: &mut dyn Device,
    list: &GlyphRunList<'_>,
    paint: &Paint,
) {
    let mut cursor = GlyphRunDrawCursor::default();
    while simplify_glyph_run_rsxform_step(device, list, paint, &mut cursor).is_some() {}
}

/// `SkDevice::simplifyGlyphRunRSXFormAndRedraw` up to the next glyph drawable, which it returns
/// instead of drawing (see [`Device::on_draw_glyph_run_list_step`]). Start with a default
/// `cursor` and call again with the same one until it returns `None`.
///
/// The canvas-level `concat` of C++ becomes a device transform change while a glyph is drawn,
/// which is the same for the raster device (its matrix is the canvas matrix). A glyph drawable
/// is drawn by the canvas after that transform is restored, with the glyph's matrix
/// concatenated in front of the drawable's matrix: the canvas draws `CTM * glyphToLocal * m`
/// in C++, and here `CTM` with `glyphToLocal * m`. A shader in `paint` needs the local-matrix
/// shader that `make_post_inverse_lm` builds.
// Port of: src/core/SkDevice.cpp#L438-L479 (chrome/m156)
#[doc(alias = "simplifyGlyphRunRSXFormAndRedraw")]
pub fn simplify_glyph_run_rsxform_step(
    device: &mut dyn Device,
    list: &GlyphRunList<'_>,
    paint: &Paint,
    cursor: &mut GlyphRunDrawCursor,
) -> Option<PendingGlyphDrawable> {
    let builder = GlyphRunBuilder::new();
    let state = cursor.rsxform.get_or_insert_with(Default::default);
    let runs = list.runs();
    while state.run < runs.len() {
        let run = &runs[state.run];
        if run.scaled_rotations().is_empty() {
            let sub_list = builder.make_glyph_run_list(run.clone(), paint, Point::default());
            if let Some(pending) =
                draw_glyph_run_list_step(device, &sub_list, paint, &mut state.sub)
            {
                return Some(pending);
            }
            state.sub = GlyphRunDrawCursor::default();
            state.run += 1;
            continue;
        }

        let origin = list.origin();
        let Some((glyph_id, pos)) = run.source().nth(state.glyph) else {
            state.glyph = 0;
            state.run += 1;
            continue;
        };
        let scale_rotate = run.scaled_rotations()[state.glyph];
        let rsxform = RSXform::new(scale_rotate.x, scale_rotate.y, (pos.x, pos.y));
        let mut glyph_to_local = Matrix::default();
        glyph_to_local
            .set_rsxform(&rsxform)
            .post_translate(Point::new(origin.x, origin.y));

        // We want to rotate each glyph by the rsxform, but we don't want to rotate "space"
        // (i.e. the shader that cares about the ctm) so we have to undo our little ctm
        // trick with a localmatrixshader so that the shader draws as if there was no
        // change to the ctm.
        let mut inverting_paint = paint.clone();
        inverting_paint.set_shader(make_post_inverse_lm(paint.shader_ref(), &glyph_to_local));
        let sub_list = builder.make_glyph_run_list(
            GlyphRun::new(
                run.font().clone(),
                vec![Point::default()],
                vec![glyph_id],
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
            paint,
            Point::default(),
        );
        let local_to_device = M44::concat(
            device.state().local_to_device44(),
            &M44::from(glyph_to_local.clone()),
        );
        let pending = {
            let mut restore = DeviceTransformRestore::new(&mut *device, &local_to_device);
            draw_glyph_run_list_step(
                restore.device(),
                &sub_list,
                &inverting_paint,
                &mut state.sub,
            )
        };
        if let Some(pending) = pending {
            return Some(PendingGlyphDrawable {
                drawable: pending.drawable,
                matrix: Matrix::concat(&glyph_to_local, &pending.matrix),
                paint: pending.paint,
            });
        }
        state.sub = GlyphRunDrawCursor::default();
        state.glyph += 1;
    }
    None
}

/// `make_post_inverse_lm`: the shader drawn with the inverse of `lm` as its local matrix, or
/// `None` when there is no shader or `lm` is not invertible.
// Port of: src/core/SkDevice.cpp#L399-L422 (chrome/m156)
fn make_post_inverse_lm(shader: Option<&Shader>, lm: &Matrix) -> Option<Shader> {
    let inverse_lm = lm.invert();
    let (Some(shader), Some(inverse_lm)) = (shader, inverse_lm) else {
        return None;
    };
    Some(shader.with_local_matrix(&inverse_lm))
}

/// A device with no pixels, which only tracks the clip bounds (`SkNoPixelsDevice`).
// Port of: src/core/SkDevice.h#L564-L637 (chrome/m156)
#[doc(alias = "SkNoPixelsDevice")]
#[derive(Clone, Debug)]
pub struct NoPixelsDevice {
    state: DeviceState,
    clip_stack: Vec<ClipState>,
}

#[derive(Clone, Debug)]
struct ClipState {
    clip_bounds: IRect,
    deferred_save_count: i32,
    is_aa: bool,
    is_rect: bool,
}

impl ClipState {
    fn new(bounds: IRect, is_aa: bool, is_rect: bool) -> ClipState {
        ClipState {
            clip_bounds: bounds,
            deferred_save_count: 0,
            is_aa,
            is_rect,
        }
    }

    // Port of: src/core/SkDevice.cpp#L627-L643 (chrome/m156)
    fn op(&mut self, op: ClipOp, transform: &M44, bounds: &Rect, is_aa: bool, fills_bounds: bool) {
        let is_rect = fills_bounds && is_scale_translate_as_m33(transform);
        self.is_aa |= is_aa;

        let dev_bounds = if bounds.is_empty() {
            Rect::new_empty()
        } else {
            map_rect(transform, bounds)
        };
        if op == ClipOp::Intersect {
            let r = if is_aa {
                dev_bounds.round_out()
            } else {
                dev_bounds.round()
            };
            if let Some(clip_bounds) = IRect::intersect(&self.clip_bounds, &r) {
                self.clip_bounds = clip_bounds;
            } else {
                self.clip_bounds.set_empty();
            }
            // A rectangular clip remains rectangular if the intersection is a rect
            self.is_rect &= is_rect;
        } else if is_rect {
            // Conservatively, we can leave the clip bounds unchanged and respect the difference
            // op. But, if we're subtracting out an axis-aligned rectangle that fully spans our
            // existing clip on an axis, we can shrink the clip bounds.
            debug_assert_eq!(op, ClipOp::Difference);
            let mut difference = IRect::default();
            let sub = if is_aa {
                dev_bounds.round_in()
            } else {
                dev_bounds.round()
            };
            if rect_priv::subtract_irect(&self.clip_bounds, &sub, &mut difference) {
                self.clip_bounds = difference;
            } else {
                // The difference couldn't be represented as a rect
                self.is_rect = false;
            }
        } else {
            // A non-rect shape was applied
            self.is_rect = false;
        }
    }
}

impl NoPixelsDevice {
    /// A device covering `bounds` (in global coordinates) with the given surface properties and
    /// no color space.
    // Port of: src/core/SkDevice.cpp#L568-L585 (chrome/m156)
    #[must_use]
    pub fn new(bounds: &IRect, props: SurfaceProps) -> NoPixelsDevice {
        Self::new_with_color_space(bounds, props, None)
    }

    /// Like [`NoPixelsDevice::new`] with a color space.
    // Port of: src/core/SkDevice.cpp#L568-L585 (chrome/m156)
    #[must_use]
    pub fn new_with_color_space(
        bounds: &IRect,
        props: SurfaceProps,
        color_space: Option<crate::color_space::ColorSpace>,
    ) -> NoPixelsDevice {
        use crate::alpha_type::AlphaType;
        use crate::color_type::ColorType;
        let info = ImageInfo::new(
            bounds.size(),
            ColorType::Unknown,
            AlphaType::Unknown,
            color_space,
        );
        let mut dev = NoPixelsDevice {
            state: DeviceState::new(info, props),
            clip_stack: Vec::new(),
        };
        dev.state
            .set_origin(&M44::new_identity(), bounds.left, bounds.top);
        let b = dev.state.bounds();
        dev.clip_stack.push(ClipState::new(b, false, true));
        dev
    }

    /// Resets the device for a new picture with `bounds` of the same size; false if the size
    /// differs (`resetForNextPicture`).
    // Port of: src/core/SkDevice.cpp#L587-L606 (chrome/m156)
    #[doc(alias = "resetForNextPicture")]
    pub fn reset_for_next_picture(&mut self, bounds: &IRect) -> bool {
        // Resetting should only happen on the root SkNoPixelsDevice, so its device-to-global
        // transform should be pixel aligned.
        debug_assert!(self.state.is_pixel_aligned_to_global());
        // We can only reset the device as long as its dimensions are not changing.
        if bounds.width() != self.state.width() || bounds.height() != self.state.height() {
            return false;
        }

        // And the canvas should have restored back to the original save count.
        debug_assert!(self.clip_stack.len() == 1 && self.clip_stack[0].deferred_save_count == 0);
        // But in the event that the clip was modified w/o a save(), reset the tracking state
        let b = self.state.bounds();
        self.clip_stack[0].clip_bounds = b;
        self.clip_stack[0].is_aa = false;
        self.clip_stack[0].is_rect = true;

        self.state
            .set_origin(&M44::new_identity(), bounds.left, bounds.top);
        true
    }

    fn clip(&self) -> &ClipState {
        self.clip_stack
            .last()
            .expect("the clip stack is never empty")
    }

    // Port of: src/core/SkDevice.cpp#L545-L560 (chrome/m156)
    fn writable_clip(&mut self) -> &mut ClipState {
        let current = self
            .clip_stack
            .last_mut()
            .expect("the clip stack is never empty");
        if current.deferred_save_count > 0 {
            current.deferred_save_count -= 1;
            // Stash current state in case 'current' moves during a resize
            let bounds = current.clip_bounds;
            let aa = current.is_aa;
            let rect = current.is_rect;
            self.clip_stack.push(ClipState::new(bounds, aa, rect));
        }
        self.clip_stack
            .last_mut()
            .expect("the clip stack is never empty")
    }
}

impl Device for NoPixelsDevice {
    fn state(&self) -> &DeviceState {
        &self.state
    }

    // Port of: src/core/SkDevice.h#L586-L588 (chrome/m156)
    fn draw_image_rect(
        &mut self,
        _image: &Image,
        _src: Option<&Rect>,
        _dst: &Rect,
        _sampling: &SamplingOptions,
        _paint: &Paint,
        _constraint: SrcRectConstraint,
    ) {
    }

    fn state_mut(&mut self) -> &mut DeviceState {
        &mut self.state
    }

    // Port of: src/core/SkDevice.h#L618 (chrome/m156)
    fn dev_clip_bounds(&self) -> IRect {
        self.clip().clip_bounds
    }

    // Port of: src/core/SkDevice.cpp#L526-L530 (chrome/m156)
    fn push_clip_stack(&mut self) {
        debug_assert!(!self.clip_stack.is_empty());
        self.clip_stack
            .last_mut()
            .expect("the clip stack is never empty")
            .deferred_save_count += 1;
    }

    // Port of: src/core/SkDevice.cpp#L532-L543 (chrome/m156)
    fn pop_clip_stack(&mut self) {
        debug_assert!(!self.clip_stack.is_empty());
        let top = self
            .clip_stack
            .last_mut()
            .expect("the clip stack is never empty");
        if top.deferred_save_count > 0 {
            top.deferred_save_count -= 1;
        } else {
            self.clip_stack.pop();
            debug_assert!(!self.clip_stack.is_empty());
        }
    }

    // Port of: src/core/SkDevice.cpp#L562-L566 (chrome/m156)
    fn clip_rect(&mut self, rect: &Rect, op: ClipOp, aa: bool) {
        let m = *self.state.local_to_device44();
        self.writable_clip().op(op, &m, rect, aa, true);
    }

    // Port of: src/core/SkDevice.cpp#L568-L572 (chrome/m156)
    fn clip_rrect(&mut self, rrect: &RRect, op: ClipOp, aa: bool) {
        let m = *self.state.local_to_device44();
        self.writable_clip()
            .op(op, &m, rrect.bounds(), aa, rrect.is_rect());
    }

    // Port of: src/core/SkDevice.cpp#L574-L582 (chrome/m156)
    fn clip_path(&mut self, path: &Path, mut op: ClipOp, aa: bool) {
        // Toggle op if the path is inverse filled
        if path.is_inverse_fill_type() {
            op = if op == ClipOp::Difference {
                ClipOp::Intersect
            } else {
                ClipOp::Difference
            };
        }
        let m = *self.state.local_to_device44();
        self.writable_clip().op(op, &m, path.bounds(), aa, false);
    }

    // Port of: src/core/SkDevice.cpp#L584-L588 (chrome/m156)
    fn clip_region(&mut self, global_rgn: &Region, op: ClipOp) {
        let m = *self.state.global_to_device();
        let b = Rect::from_irect(global_rgn.bounds());
        self.writable_clip()
            .op(op, &m, &b, false, global_rgn.is_rect());
    }

    // Port of: src/core/SkDevice.cpp#L590-L592 (chrome/m156)
    fn on_clip_shader(&mut self, _shader: Shader) {
        self.writable_clip().is_rect = false;
    }

    // Port of: src/core/SkDevice.cpp#L594-L603 (chrome/m156)
    fn replace_clip(&mut self, rect: &IRect) {
        let mut device_rect =
            map_rect(self.state.global_to_device(), &Rect::from_irect(rect)).round();
        let bounds = self.state.bounds();
        if let Some(r) = IRect::intersect(&device_rect, &bounds) {
            device_rect = r;
        } else {
            device_rect.set_empty();
        }
        let clip = self.writable_clip();
        clip.clip_bounds = device_rect;
        clip.is_rect = true;
        clip.is_aa = false;
    }

    fn is_clip_anti_aliased(&self) -> bool {
        self.clip().is_aa
    }

    fn is_clip_empty(&self) -> bool {
        self.dev_clip_bounds().is_empty()
    }

    fn is_clip_rect(&self) -> bool {
        self.clip().is_rect && !self.is_clip_empty()
    }

    fn is_clip_wide_open(&self) -> bool {
        self.clip().is_rect && self.dev_clip_bounds() == self.state.bounds()
    }

    fn android_utils_clip_as_rgn(&self, rgn: &mut Region) {
        rgn.set_rect(self.dev_clip_bounds());
    }

    fn is_no_pixels_device(&self) -> bool {
        true
    }

    fn reset_for_next_picture(&mut self, bounds: &IRect) -> bool {
        NoPixelsDevice::reset_for_next_picture(self, bounds)
    }

    // The draw calls do nothing.
    fn draw_paint(&mut self, _paint: &Paint) {}
    fn draw_points(&mut self, _mode: PointMode, _points: &[crate::point::Point], _paint: &Paint) {}
    fn draw_rect(&mut self, _r: &Rect, _paint: &Paint) {}
    fn draw_oval(&mut self, _oval: &Rect, _paint: &Paint) {}
    fn draw_rrect(&mut self, _rr: &RRect, _paint: &Paint) {}
    fn draw_path(&mut self, _path: &Path, _paint: &Paint) {}
    fn draw_vertices(&mut self, _: &Vertices, _: Blender, _: &Paint, _: bool) {}
    fn draw_mesh(&mut self, _: &Mesh, _: Blender, _: &Paint) {}

    fn on_draw_glyph_run_list(&mut self, _list: &GlyphRunList<'_>, _paint: &Paint) {}
}

/// Sets a device's local-to-device transform for the lifetime of the guard
/// (`SkAutoDeviceTransformRestore`); use [`DeviceTransformRestore::device`] to draw.
// Port of: src/core/SkDevice.h#L640-L655 (chrome/m156)
#[doc(alias = "SkAutoDeviceTransformRestore")]
pub struct DeviceTransformRestore<'a> {
    device: &'a mut dyn Device,
    prev_local_to_device: M44,
}

impl std::fmt::Debug for DeviceTransformRestore<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceTransformRestore")
            .field("prev_local_to_device", &self.prev_local_to_device)
            .finish_non_exhaustive()
    }
}

impl<'a> DeviceTransformRestore<'a> {
    /// Sets `local_to_device` on `device`.
    #[must_use]
    pub fn new(device: &'a mut dyn Device, local_to_device: &M44) -> Self {
        let prev_local_to_device = *device.state().local_to_device44();
        device.state_mut().set_local_to_device(local_to_device);
        DeviceTransformRestore {
            device,
            prev_local_to_device,
        }
    }

    /// The device, with the new transform.
    pub fn device(&mut self) -> &mut dyn Device {
        &mut *self.device
    }
}

impl Drop for DeviceTransformRestore<'_> {
    fn drop(&mut self) {
        let prev = self.prev_local_to_device;
        self.device.state_mut().set_local_to_device(&prev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_pixels_device_tracks_clip_bounds() {
        let mut dev = NoPixelsDevice::new(&IRect::new(10, 20, 110, 70), SurfaceProps::default());
        assert!(dev.is_no_pixels_device());
        assert_eq!(dev.dev_clip_bounds(), IRect::new(0, 0, 100, 50));
        assert!(dev.is_clip_wide_open() && dev.is_clip_rect() && !dev.is_clip_anti_aliased());
        assert_eq!(dev.state().origin(), IPoint::new(10, 20));

        dev.push_clip_stack();
        dev.clip_rect(&Rect::new(10.0, 10.0, 60.5, 40.0), ClipOp::Intersect, true);
        assert_eq!(dev.dev_clip_bounds(), IRect::new(0, 0, 51, 20));
        assert!(dev.is_clip_anti_aliased() && !dev.is_clip_wide_open());
        dev.pop_clip_stack();
        assert_eq!(dev.dev_clip_bounds(), IRect::new(0, 0, 100, 50));
        assert!(!dev.is_clip_anti_aliased());

        // A difference that spans the whole height shrinks the bounds.
        dev.clip_rect(
            &Rect::new(60.0, -5.0, 120.0, 80.0),
            ClipOp::Difference,
            false,
        );
        assert_eq!(dev.dev_clip_bounds(), IRect::new(0, 0, 50, 50));
        assert!(dev.is_clip_rect());
        dev.clip_path(
            &Path::circle((5.0, 5.0), 3.0, None),
            ClipOp::Intersect,
            false,
        );
        assert!(!dev.is_clip_rect());
        let mut rgn = Region::new();
        dev.android_utils_clip_as_rgn(&mut rgn);
        assert_eq!(*rgn.bounds(), dev.dev_clip_bounds());
    }

    #[test]
    #[allow(clippy::float_cmp)] // exact small integers
    fn coordinate_systems() {
        let mut state = DeviceState::new(
            ImageInfo::new_n32_premul((20, 10), None),
            SurfaceProps::default(),
        );
        assert!(state.is_pixel_aligned_to_global());
        state.set_origin(&M44::new_identity(), 4, 6);
        assert_eq!(state.origin(), IPoint::new(4, 6));
        assert_eq!(state.local_to_device().translate_x(), -4.0);
        assert!(state.check_local_to_device_dirty());
        assert!(!state.check_local_to_device_dirty());
        let mut other = DeviceState::new(state.image_info().clone(), SurfaceProps::default());
        other.set_origin(&M44::new_identity(), 1, 1);
        let rel = state.relative_transform(&other);
        assert_eq!((rel.rc(0, 3), rel.rc(1, 3)), (3.0, 5.0));
    }
}
