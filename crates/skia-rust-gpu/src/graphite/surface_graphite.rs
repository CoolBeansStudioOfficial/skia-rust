// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Surface_Graphite.{h,cpp}
//
// skia-rust deviations:
// - `SkSurface_Base` is `SurfaceBase` plus the canvas, as for the raster surface.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::canvas::{Canvas, SurfaceBase};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::device::Device as CoreDevice;
use skia_rust_core::image::Image as CoreImage;
use skia_rust_core::image_info::{ColorInfo, ImageInfo};
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_core::surface_props::SurfaceProps;

use crate::gpu::backing_fit::BackingFit;
use crate::gpu::gpu_types::{Budgeted, Mipmapped};
use crate::graphite::device::{Device, DeviceCore};
use crate::graphite::draw_context::DrawContext;
use crate::graphite::image_graphite::wrap_device;
use crate::graphite::recorder::Recorder;
use crate::graphite::resource_types::LoadOp;
use crate::graphite::texture_proxy_view::TextureProxyView;

/// `skgpu::graphite::Surface`: a canvas over a Graphite `Device`, with the image of the device's
/// target (`fImageView`).
// Port of: src/gpu/graphite/Surface_Graphite.h#L27-L80 (chrome/m156)
#[doc(alias = "skgpu::graphite::Surface")]
pub struct Surface {
    base: Rc<SurfaceBase>,
    canvas: Canvas,
    // `fDevice`: the canvas holds the device itself, and the surface keeps a handle to the same
    // device state for the calls the canvas does not make.
    device: Rc<RefCell<DeviceCore>>,
    info: ImageInfo,
    props: SurfaceProps,
    target: TextureProxyView,
    // `fDevice->isTexturable()`, which does not change for a device.
    texturable: bool,
    // `fImageView`: the image object `asImage()` returns, made when the surface is.
    image_view: CoreImage,
}

impl std::fmt::Debug for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Surface")
            .field("info", &self.info)
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

impl Surface {
    /// `Surface::Make(recorder, info, label, budgeted, mipmapped, backingFit, props, initialLoadOp,
    /// registerWithRecorder, allowUnpremul)`, the common factory.
    // Port of: src/gpu/graphite/Surface_Graphite.cpp#L124-L143 (chrome/m156)
    #[doc(alias = "Make")]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    fn make_impl(
        recorder: &Recorder,
        info: &ImageInfo,
        label: &str,
        budgeted: Budgeted,
        mipmapped: Mipmapped,
        backing_fit: BackingFit,
        props: Option<&SurfaceProps>,
        initial_load_op: LoadOp,
        register_with_recorder: bool,
        allow_unpremul: bool,
    ) -> Option<Surface> {
        let props = props.copied().unwrap_or_default();
        let device = Device::make_with_info(
            Some(recorder),
            info,
            budgeted,
            mipmapped,
            backing_fit,
            &props,
            initial_load_op,
            label,
            register_with_recorder,
            allow_unpremul,
        )?;
        Surface::new(device)
    }

    /// `Surface::Make(recorder, info, label, budgeted, mipmapped, backingFit, props)`: a surface
    /// whose device is cleared to transparent black and registered with the recorder.
    // Port of: src/gpu/graphite/Surface_Graphite.h#L37-L45 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(
        recorder: &Recorder,
        info: &ImageInfo,
        label: &str,
        budgeted: Budgeted,
        mipmapped: Mipmapped,
        backing_fit: BackingFit,
        props: Option<&SurfaceProps>,
    ) -> Option<Surface> {
        Surface::make_impl(
            recorder,
            info,
            label,
            budgeted,
            mipmapped,
            backing_fit,
            props,
            LoadOp::Clear,
            /*register_with_recorder=*/ true,
            /*allow_unpremul=*/ false,
        )
    }

    /// `Surface::MakeScratch(recorder, info, label, budgeted, mipmapped, backingFit, allowUnpremul)`:
    /// a short-lived surface that is not registered with the recorder.
    // Port of: src/gpu/graphite/Surface_Graphite.h#L47-L57 (chrome/m156)
    #[doc(alias = "MakeScratch")]
    #[must_use]
    pub fn make_scratch(
        recorder: &Recorder,
        info: &ImageInfo,
        label: &str,
        budgeted: Budgeted,
        mipmapped: Mipmapped,
        backing_fit: BackingFit,
        allow_unpremul: bool,
    ) -> Option<Surface> {
        Surface::make_impl(
            recorder,
            info,
            label,
            budgeted,
            mipmapped,
            backing_fit,
            None,
            LoadOp::Discard,
            /*register_with_recorder=*/ false,
            allow_unpremul,
        )
    }

    /// `SkSurfaces::RenderTarget(recorder, info, mipmapped, props, label)`: an unbudgeted, exact
    /// render target.
    // Port of: src/gpu/graphite/Surface_Graphite.cpp#L332-L344 (chrome/m156)
    #[doc(alias = "RenderTarget")]
    #[must_use]
    pub fn render_target(
        recorder: &Recorder,
        info: &ImageInfo,
        mipmapped: Mipmapped,
        props: Option<&SurfaceProps>,
        label: &str,
    ) -> Option<Surface> {
        let label = if label.is_empty() {
            "SkSurfaceRenderTarget"
        } else {
            label
        };
        Surface::make(
            recorder,
            info,
            label,
            Budgeted::No,
            mipmapped,
            BackingFit::Exact,
            props,
        )
    }

    /// `Surface(sk_sp<Device>)`: the canvas over `device`, and the image of its target.
    // Port of: src/gpu/graphite/Surface_Graphite.cpp#L24-L29 (chrome/m156)
    fn new(device: Device) -> Option<Surface> {
        let info = device.state().image_info().clone();
        let props = *device.state().surface_props();
        let target = device.target();
        let core = Rc::clone(device.core());
        // `Image::WrapDevice(fDevice)`: fails for a device whose target cannot be sampled.
        let texturable = device.is_texturable();
        let link = Arc::clone(device.core().borrow().link());
        let image_view = wrap_device(&target, texturable, &info, None, link)?;

        let canvas = Canvas::from_device(Box::new(device));
        let base = Rc::new(SurfaceBase::new());
        canvas.set_surface_base(Some(Rc::clone(&base)));
        Some(Surface {
            base,
            canvas,
            device: core,
            info,
            props,
            target,
            texturable,
            image_view: image_view.into_core(),
        })
    }

    /// `getCanvas()`.
    #[must_use]
    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    /// `imageInfo()`.
    #[must_use]
    pub fn image_info(&self) -> &ImageInfo {
        &self.info
    }

    /// `props()`.
    #[must_use]
    pub fn props(&self) -> &SurfaceProps {
        &self.props
    }

    /// `generationID()`: the base's generation ID.
    #[must_use]
    pub fn generation_id(&self) -> u32 {
        self.base.generation_id()
    }

    /// `target()`: the device's target texture view.
    // Port of: src/gpu/graphite/Surface_Graphite.cpp#L100-L102 (chrome/m156)
    #[must_use]
    pub fn target(&self) -> &TextureProxyView {
        &self.target
    }

    /// `asImage()`: the image of the surface's target, which is made with the surface. The image
    /// is linked to the surface's device: a draw of the image flushes the draws made to the
    /// surface before it (`Image_Base::notifyInUse`), including those made after this call.
    // Port of: src/gpu/graphite/Surface_Graphite.cpp#L70-L77 (chrome/m156)
    #[doc(alias = "asImage")]
    #[must_use]
    pub fn as_image(&self) -> CoreImage {
        self.image_view.clone()
    }

    /// `asImage(otherCT, otherAT)`: the same target read as another color type and alpha type.
    // Port of: src/gpu/graphite/Surface_Graphite.cpp#L79-L88 (chrome/m156)
    #[doc(alias = "asImage")]
    #[must_use]
    pub fn as_image_with_color(
        &self,
        other_ct: ColorType,
        other_alpha: AlphaType,
    ) -> Option<CoreImage> {
        if other_ct == self.image_view.color_type() && other_alpha == self.image_view.alpha_type() {
            return Some(self.image_view.clone());
        }
        let color_info = ColorInfo::new(other_ct, other_alpha, self.info.color_space());
        let link = Arc::clone(self.device.borrow().link());
        let image = wrap_device(
            &self.target,
            self.texturable,
            &self.info,
            Some(color_info),
            link,
        )?;
        Some(image.into_core())
    }

    /// `makeImageCopy(subset, mipmapped)`: an unbudgeted, exact copy of `subset` (the whole surface
    /// when `None`).
    // Port of: src/gpu/graphite/Surface_Graphite.cpp#L90-L97 (chrome/m156)
    #[doc(alias = "makeImageCopy")]
    #[must_use]
    pub fn make_image_copy(
        &self,
        subset: Option<IRect>,
        mipmapped: Mipmapped,
    ) -> Option<CoreImage> {
        let src_rect = subset.unwrap_or_else(|| IRect::from_size(self.info.dimensions()));
        self.device.borrow_mut().make_image_copy(
            src_rect,
            Budgeted::No,
            mipmapped,
            BackingFit::Exact,
        )
    }

    /// `flushToDrawContext(drawContext)`: flushes the device's pending work into `draw_context`, or
    /// into the recorder's root task list when `None`.
    // Port of: src/gpu/graphite/Surface_Graphite.cpp#L146-L148 (chrome/m156)
    #[doc(alias = "flushToDrawContext")]
    pub fn flush_to_draw_context(&self, draw_context: Option<&mut DrawContext>) {
        self.device.borrow_mut().flush_pending_work(draw_context);
    }

    /// `Flush(surface)` for a Graphite surface: flushes the pending work and resets the storage
    /// cache.
    // Port of: src/gpu/graphite/Surface_Graphite.cpp#L301-L311 (chrome/m156)
    #[doc(alias = "Flush")]
    pub fn flush(&self) {
        self.flush_pending_work(None);
        self.device.borrow().reset_storage_cache();
    }

    /// The recorder of the surface's device, or `None` once it has been abandoned or dropped.
    #[must_use]
    pub fn recorder(&self) -> Option<Recorder> {
        self.device.borrow().recorder()
    }

    /// `Surface::onNewSurface(ii)`: `Device::makeSurface(ii, props)`.
    // Port of: src/gpu/graphite/Surface_Graphite.cpp#L58-L60 (chrome/m156)
    #[doc(alias = "onNewSurface")]
    #[must_use]
    pub fn new_surface(&self, info: &ImageInfo) -> Option<Surface> {
        self.device.borrow().make_surface(info, &self.props)
    }

    /// The `Surface`'s dimensions as an `ISize` (`SkSurface::width()`, `height()`).
    #[must_use]
    pub fn dimensions(&self) -> ISize {
        self.info.dimensions()
    }

    fn flush_pending_work(&self, draw_context: Option<&mut DrawContext>) {
        self.device.borrow_mut().flush_pending_work(draw_context);
    }
}

impl Drop for Surface {
    // Port of: src/gpu/graphite/Surface_Graphite.cpp#L29-L31 (chrome/m156)
    fn drop(&mut self) {
        self.device.borrow_mut().set_immutable();
    }
}
