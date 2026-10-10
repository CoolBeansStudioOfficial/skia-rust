// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Image_Graphite.{h,cpp}, and the parts of
// src/gpu/graphite/Image_Base_Graphite.{h,cpp} that a Graphite image needs (`copyImage`,
// `onMakeSubset`, `makeColorTypeAndColorSpace`, `textureSize`, the device links and
// `notifyInUse`).
//
// skia-rust deviations:
// - Linked devices (`fLinkedDevices`) are [`DeviceLink`]s, not `sk_sp<Device>`. `ImageBase` is
//   `Send + Sync` and a `Device` is `Rc`-based (`docs/design/gpu.md` §5.1, §5.6), so an image holds
//   the `Send + Sync` half of the device's state that `Device::notifyInUse` reads (its recorder's
//   ID, its target, whether it is immutable or gone, a scratch device's last task) and reaches the
//   device itself through its recorder's tracked devices, on the recorder's thread.
// - `makeNonBudgeted` is not ported.
// - Operations that take an `SkRecorder*` (`onMakeSubset`, `makeColorTypeAndColorSpace`) need the
//   recorder, which `ImageBase` cannot pass; they are `*_with_recorder` methods here, and the
//   `ImageBase` methods return `None`.

use std::any::Any;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::{Image as CoreImage, RequiredProperties};
use skia_rust_core::image_base::{ImageBase, ImageType};
use skia_rust_core::image_info::{ColorInfo, ImageInfo};
use skia_rust_core::pixel_ref::next_image_id;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;

use crate::gpu::backing_fit::{BackingFit, get_approx_size};
use crate::gpu::gpu_types::{Budgeted, Mipmapped, Protected};
use crate::gpu::sk_log::skia_log_w;
use crate::graphite::device::DeviceCore;
use crate::graphite::draw_context::DrawContext;
use crate::graphite::recorder::Recorder;
use crate::graphite::resource_types::Shareable;
use crate::graphite::task::TaskRef;
use crate::graphite::task::copy_task::CopyTextureToTextureTask;
use crate::graphite::texture_format::{
    are_color_type_and_format_compatible, read_swizzle_for_color_type,
};
use crate::graphite::texture_proxy::TextureProxy;
use crate::graphite::texture_proxy_view::TextureProxyView;
use crate::graphite::texture_utils::{copy_as_draw, generate_mipmaps};

/// `Image`: a Graphite-backed image, which is a view of a texture (`skgpu::graphite::Image`).
// Port of: src/gpu/graphite/Image_Graphite.h#L24-L69 (chrome/m156)
#[doc(alias = "skgpu::graphite::Image")]
#[derive(Debug)]
pub struct Image {
    info: ImageInfo,
    unique_id: u32,
    texture_proxy_view: TextureProxyView,
    // `Image_Base::fLinkedDevices` (with `fDeviceLinkLock`): devices are flushed in
    // `notify_in_use()`. If a linked device is gone or marked immutable, it is unlinked. If all
    // linked devices are removed, this array becomes empty.
    links: ImageLinks,
}

/// The `Send + Sync` half of a Graphite `Device` that the images of its target hold
/// (`docs/design/gpu.md` §5.6): what `Device::notifyInUse()` reads of the device, and the ID its
/// recorder finds the device by.
#[derive(Debug)]
pub struct DeviceLink {
    device_id: u32,
    recorder_id: u32,
    target: Arc<TextureProxy>,
    state: Mutex<LinkState>,
}

#[derive(Debug, Default)]
struct LinkState {
    // The device abandoned its recorder (`!fRecorder`): it is immutable.
    abandoned: bool,
    // The device was dropped; in C++ the image would hold the last reference (`unique()`).
    dropped: bool,
    // `Device::fLastTask` of a scratch device.
    last_task: Option<TaskRef>,
}

/// The next device ID (never 0).
fn next_device_id() -> u32 {
    static NEXT_ID: AtomicU32 = AtomicU32::new(1);
    loop {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        if id != 0 {
            return id;
        }
    }
}

impl DeviceLink {
    /// The link of a new device of the recorder `recorder_id` that draws into `target`.
    #[must_use]
    pub fn new(recorder_id: u32, target: Arc<TextureProxy>) -> Arc<DeviceLink> {
        Arc::new(DeviceLink {
            device_id: next_device_id(),
            recorder_id,
            target,
            state: Mutex::new(LinkState::default()),
        })
    }

    fn state(&self) -> MutexGuard<'_, LinkState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The ID of the linked device.
    #[must_use]
    pub fn device_id(&self) -> u32 {
        self.device_id
    }

    /// Records `Device::fLastTask`.
    pub fn set_last_task(&self, task: Option<TaskRef>) {
        self.state().last_task = task;
    }

    /// The device abandoned its recorder.
    pub fn mark_abandoned(&self) {
        self.state().abandoned = true;
    }

    /// The device was dropped.
    pub fn mark_dropped(&self) {
        self.state().dropped = true;
    }

    /// `device->recorder() && !device->unique()`: the device can still write to the texture.
    #[must_use]
    pub fn is_live(&self) -> bool {
        let state = self.state();
        !state.abandoned && !state.dropped
    }

    // `Device::isScratchDevice()`, read off the shared target.
    // Port of: src/gpu/graphite/Device.cpp#L2559-L2571 (chrome/m156)
    fn is_scratch_device(&self) -> bool {
        !self.target.is_instantiated() && !self.target.is_lazy()
    }

    /// `Device::notifyInUse(recorder, drawContext)` through the link: `current` is the device
    /// whose draw reads the image (C++'s `drawContext` is its `DrawContext`), or `None` for a
    /// copy. Returns true if the image does not need to track the device anymore.
    // Port of: src/gpu/graphite/Device.cpp#L593-L652 (chrome/m156)
    pub fn notify_in_use(&self, recorder: &Recorder, current: Option<&mut DeviceCore>) -> bool {
        if self.is_scratch_device() {
            let last_task = self.state().last_task.clone();
            if let Some(last_task) = last_task {
                // Increment the pending read count for the device's target
                recorder.priv_().add_pending_read(&self.target);
                if let Some(current) = current {
                    // Add a reference to the device's drawTask to `drawContext` if that's
                    // provided.
                    current.record_dependency(last_task);
                } else {
                    // If there's no `drawContext` this notify represents a copy, so for now
                    // append the task to the root task list since that is where the subsequent
                    // copy task will go as well.
                    recorder.priv_().add(last_task);
                }
            }
            // (Else there is no draw task yet: the device has no pending work, or it flushed it
            // to a drawContext's local task list. The correct action is to do nothing.)

            // Scratch devices are often already marked immutable, but they are also the way in
            // which Image finds the last snapped DrawTask so we don't unlink scratch devices.
            false
        } else {
            // Automatic flushing of image views only happens when mixing reads and writes on the
            // originating Recorder. Draws of the view on another Recorder will always see the
            // texture content dependent on how Recordings are inserted.
            let same_recorder = self.is_live() && self.recorder_id == recorder.priv_().unique_id();
            if same_recorder {
                let has_draw_context = current.is_some();
                match current {
                    Some(current) if current.device_id() == self.device_id => {
                        // The device draws its own image.
                        current.flush_pending_work(None);
                        current.set_must_flush_dependencies();
                    }
                    current => {
                        if let Some(device) = recorder.priv_().find_tracked_device(self.device_id)
                            && let Ok(mut device) = device.try_borrow_mut()
                            && let Some(device) = device.as_device_core()
                        {
                            // Non-scratch devices push their tasks to the root task list to
                            // maintain an order consistent with the client-triggering actions.
                            // Because of this, there's no need to add references to the
                            // `drawContext` that the device is being drawn into.
                            device.flush_pending_work_with_current(None, current);
                            if has_draw_context {
                                // But if we are being drawn into another context, remember that
                                // there is an outstanding dependency on the current state of
                                // this device, in which case it's next flush must also flush
                                // those other devices before its new tasks are added.
                                device.set_must_flush_dependencies();
                            }
                        }
                    }
                }
            }
            // Return true (to unlink with the image) if the non-scratch surface is immutable
            // since this Device cannot record any more commands that will modify its texture.
            !self.is_live()
        }
    }
}

/// The devices linked to a Graphite image (`Image_Base::fLinkedDevices`, with
/// `fDeviceLinkLock`). Shared by [`Image`] and the YUVA image, which are both `Image_Base`s.
#[derive(Debug, Default)]
pub struct ImageLinks {
    linked_devices: Mutex<Vec<Option<Arc<DeviceLink>>>>,
}

impl ImageLinks {
    fn links(&self) -> MutexGuard<'_, Vec<Option<Arc<DeviceLink>>>> {
        self.linked_devices
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// `linkDevice(device)`: links this image to the device that can write to its texture, so
    /// that when the image is sampled in a draw, any pending work from the device is
    /// automatically flushed. Only called before the image is returned from a factory.
    // Port of: src/gpu/graphite/Image_Base_Graphite.cpp#L40-L46 (chrome/m156)
    #[doc(alias = "linkDevice")]
    pub fn link_device(&self, link: Arc<DeviceLink>) {
        // Technically this lock isn't needed since this is only called before the Image is
        // returned to user code that could expose it to multiple threads.
        self.links().push(Some(link));
    }

    /// `linkDevices(other)`: copies `other`'s links to this image, which shares its texture.
    // Port of: src/gpu/graphite/Image_Base_Graphite.cpp#L31-L38 (chrome/m156)
    #[doc(alias = "linkDevices")]
    pub fn link_devices(&self, other: &ImageLinks) {
        let other_links: Vec<Option<Arc<DeviceLink>>> = other.links().clone();
        self.links().extend(other_links);
    }

    /// `notifyInUse(recorder, drawContext, unlinkDevices)`: notifies the linked devices that
    /// their pending contents will be read by `recorder` (by `current`'s draw, if any).
    // Port of: src/gpu/graphite/Image_Base_Graphite.cpp#L50-L81 (chrome/m156)
    fn notify_in_use_impl(
        &self,
        recorder: &Recorder,
        mut current: Option<&mut DeviceCore>,
        unlink_devices: bool,
    ) {
        // unlinkDevices can't be used with a DrawContext
        debug_assert!(current.is_none() || !unlink_devices);

        // skia-rust: the links are taken out of the lock while the devices are notified (C++
        // holds a spin lock); a device flush never reaches this image again.
        let mut links = std::mem::take(&mut *self.links());
        if !links.is_empty() {
            let mut empty_count = 0;
            for slot in &mut links {
                let unlink = match slot {
                    None => true,
                    Some(link) => {
                        link.notify_in_use(recorder, current.as_deref_mut()) || unlink_devices
                    }
                };
                if unlink {
                    // Already unlinked or notifyInUse() signals the device doesn't need to be
                    // linked anymore.
                    *slot = None;
                    empty_count += 1;
                }
            }
            if empty_count == links.len() {
                links.clear();
            }
        }
        let mut guard = self.links();
        links.append(&mut guard);
        *guard = links;
    }

    /// `notifyInUse(recorder, drawContext)`: `current` is the device whose draw samples this
    /// image (C++'s `drawContext` is its `DrawContext`), or `None` for a copy.
    // Port of: src/gpu/graphite/Image_Base_Graphite.h#L40-L42 (chrome/m156)
    #[doc(alias = "notifyInUse")]
    pub fn notify_in_use(&self, recorder: &Recorder, current: Option<&mut DeviceCore>) {
        self.notify_in_use_impl(recorder, current, /*unlink_devices=*/ false);
    }

    /// `unlinkDevices(recorder)`: notifies any linked devices as in-use without a draw context
    /// and then removes all links so the image is no longer dynamic.
    // Port of: src/gpu/graphite/Image_Base_Graphite.h#L130-L133 (chrome/m156)
    #[doc(alias = "unlinkDevices")]
    pub fn unlink_devices(&self, recorder: &Recorder) {
        self.notify_in_use_impl(recorder, None, /*unlink_devices=*/ true);
    }

    /// `isDynamic()`: unlinks the devices that can no longer write to the texture.
    ///
    /// As in C++, the result is true only when some (not all) of several linked devices were
    /// unlinked by this call.
    // Port of: src/gpu/graphite/Image_Base_Graphite.cpp#L83-L100 (chrome/m156)
    #[doc(alias = "isDynamic")]
    #[must_use]
    pub fn is_dynamic(&self) -> bool {
        let mut links = self.links();
        let mut empty_count = 0;
        if !links.is_empty() {
            for slot in links.iter_mut() {
                if slot.as_ref().is_none_or(|link| !link.is_live()) {
                    *slot = None;
                    empty_count += 1;
                }
            }
            if empty_count == links.len() {
                links.clear();
                empty_count = 0;
            }
        }
        empty_count > 0
    }

    /// Whether any device is linked (for tests: `fLinkedDevices` is not empty).
    #[must_use]
    pub fn has_linked_devices(&self) -> bool {
        self.links().iter().any(Option::is_some)
    }
}

impl Image {
    /// `Image(TextureProxyView, const SkColorInfo&)`. The image's dimensions are the proxy's.
    // Port of: src/gpu/graphite/Image_Graphite.cpp#L24-L30 (chrome/m156)
    #[doc(alias = "Image")]
    #[must_use]
    pub fn new(view: TextureProxyView, color_info: &ColorInfo) -> Image {
        // Graphite does not cache based on the image's unique ID so always request a new one
        // (`kNeedNewImageUniqueID`).
        let info = ImageInfo::from_color_info(view.dimensions(), color_info.clone());
        Image {
            info,
            unique_id: next_image_id(),
            texture_proxy_view: view,
            links: ImageLinks::default(),
        }
    }

    /// The links of this image (`fLinkedDevices`), for the images that share its texture.
    #[must_use]
    pub fn links(&self) -> &ImageLinks {
        &self.links
    }

    /// `linkDevice(device)`: see [`ImageLinks::link_device`].
    #[doc(alias = "linkDevice")]
    pub fn link_device(&self, link: Arc<DeviceLink>) {
        self.links.link_device(link);
    }

    /// `linkDevices(other)`: copies `other`'s links to this image, which shares its texture.
    // Port of: src/gpu/graphite/Image_Base_Graphite.cpp#L31-L38 (chrome/m156)
    #[doc(alias = "linkDevices")]
    pub fn link_devices(&self, other: &Image) {
        self.links.link_devices(&other.links);
    }

    /// `notifyInUse(recorder, drawContext)`: see [`ImageLinks::notify_in_use`].
    #[doc(alias = "notifyInUse")]
    pub fn notify_in_use(&self, recorder: &Recorder, current: Option<&mut DeviceCore>) {
        self.links.notify_in_use(recorder, current);
    }

    /// `unlinkDevices(recorder)`: see [`ImageLinks::unlink_devices`].
    #[doc(alias = "unlinkDevices")]
    pub fn unlink_devices(&self, recorder: &Recorder) {
        self.links.unlink_devices(recorder);
    }

    /// `isDynamic()`: see [`ImageLinks::is_dynamic`].
    #[doc(alias = "isDynamic")]
    #[must_use]
    pub fn is_dynamic(&self) -> bool {
        self.links.is_dynamic()
    }

    /// Whether any device is linked (for tests: `fLinkedDevices` is not empty).
    #[must_use]
    pub fn has_linked_devices(&self) -> bool {
        self.links.has_linked_devices()
    }

    /// Wraps this image as a core `SkImage` handle (`sk_sp<Image>`).
    #[must_use]
    pub fn into_core(self) -> CoreImage {
        CoreImage::from_base(self)
    }

    /// The Graphite image behind a core image, if it is one (`static_cast<const Image*>` after
    /// `isGraphiteBacked()`).
    // Port of: src/gpu/graphite/TextureUtils.cpp#L700-L712 (chrome/m156), the cast in `AsView`
    #[must_use]
    pub fn from_core(image: &CoreImage) -> Option<&Image> {
        image.as_base().as_any().downcast_ref::<Image>()
    }

    /// `textureProxyView()`.
    #[doc(alias = "textureProxyView")]
    #[must_use]
    pub fn texture_proxy_view(&self) -> &TextureProxyView {
        &self.texture_proxy_view
    }

    /// `Image::textureSize()`.
    // Port of: src/gpu/graphite/Image_Graphite.cpp#L120-L128 (chrome/m156)
    #[doc(alias = "textureSize")]
    #[must_use]
    pub fn texture_size(&self) -> usize {
        let Some(proxy) = self.texture_proxy_view.proxy() else {
            return 0;
        };
        proxy.with_texture(|texture| match texture {
            None => proxy.uninstantiated_gpu_memory_size(),
            Some(texture) => texture.base().gpu_memory_size(),
        })
    }

    /// `Image::Copy(recorder, drawContext, srcView, srcColorInfo, subset, budgeted, mipmapped,
    /// backingFit, label)`: copies `src_view`'s `subset` into a new texture, as a blit when the
    /// source is copyable and as a draw otherwise.
    ///
    /// `draw_context` is `None` for copies made outside a draw; otherwise the copy's tasks are
    /// recorded as dependencies of that draw context.
    // Port of: src/gpu/graphite/Image_Graphite.cpp#L44-L112 (chrome/m156)
    #[doc(alias = "Copy")]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    #[must_use]
    pub fn copy(
        recorder: &Recorder,
        mut draw_context: Option<&mut DrawContext>,
        src_view: &TextureProxyView,
        src_color_info: &ColorInfo,
        subset: IRect,
        budgeted: Budgeted,
        mipmapped: Mipmapped,
        backing_fit: BackingFit,
        label: &str,
    ) -> Option<CoreImage> {
        debug_assert!(draw_context.is_none() || budgeted == Budgeted::Yes);
        debug_assert!(!(mipmapped == Mipmapped::Yes && backing_fit == BackingFit::Approx));
        let src_proxy = src_view.proxy()?;

        let priv_ = recorder.priv_();
        let caps = Arc::clone(priv_.caps());
        if !caps.is_copyable_src(src_proxy.texture_info()) {
            if !caps.is_texturable(src_proxy.texture_info(), false) {
                // The texture is not blittable nor texturable so copying cannot be done.
                return None;
            }
            // Copy-as-draw
            let src_image = Image::new(src_view.clone(), src_color_info).into_core();
            return copy_as_draw(
                recorder,
                draw_context,
                &src_image,
                subset,
                src_color_info,
                budgeted,
                mipmapped,
                backing_fit,
                label,
            );
        }

        let texture_info =
            caps.get_texture_info_for_sampled_copy(src_proxy.texture_info(), mipmapped);
        let dst_size = if backing_fit == BackingFit::Approx {
            get_approx_size(subset.size())
        } else {
            subset.size()
        };
        let dst = {
            let mut provider = priv_
                .resource_provider()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            TextureProxy::make(
                &*caps,
                &mut provider,
                dst_size,
                &texture_info,
                budgeted,
                label,
            )?
        };

        let copy_task = CopyTextureToTextureTask::make(
            src_view.ref_proxy(),
            subset,
            Some(Arc::clone(&dst)),
            IPoint::new(0, 0),
            0,
        )?;
        if let Some(draw_context) = draw_context.as_deref_mut() {
            draw_context.record_dependency(copy_task);
        } else {
            priv_.add(copy_task);
        }

        if mipmapped == Mipmapped::Yes && !generate_mipmaps(recorder, draw_context, &dst) {
            skia_log_w!("Image::Copy failed to generate mipmaps");
            return None;
        }

        Some(
            Image::new(
                TextureProxyView::new(Some(dst), src_view.swizzle()),
                src_color_info,
            )
            .into_core(),
        )
    }

    /// `Image::copyImage(recorder, subset, budgeted, mipmapped, backingFit, label)`.
    // Port of: src/gpu/graphite/Image_Graphite.cpp#L130-L138 (chrome/m156)
    #[doc(alias = "copyImage")]
    #[must_use]
    pub fn copy_image(
        &self,
        recorder: &Recorder,
        subset: IRect,
        budgeted: Budgeted,
        mipmapped: Mipmapped,
        backing_fit: BackingFit,
        label: &str,
    ) -> Option<CoreImage> {
        self.notify_in_use(recorder, None);
        Image::copy(
            recorder,
            None,
            &self.texture_proxy_view,
            self.info.color_info(),
            subset,
            budgeted,
            mipmapped,
            backing_fit,
            label,
        )
    }

    /// `Image::onReinterpretColorSpace(newCS)`: the same texture under another color space.
    // Port of: src/gpu/graphite/Image_Graphite.cpp#L140-L146 (chrome/m156)
    #[doc(alias = "onReinterpretColorSpace")]
    #[must_use]
    pub fn reinterpret_color_space(&self, new_cs: Option<ColorSpace>) -> CoreImage {
        let image = Image::new(
            self.texture_proxy_view.clone(),
            &self.info.color_info().with_color_space(new_cs),
        );
        image.link_devices(self);
        image.into_core()
    }

    fn bounds_irect(&self) -> IRect {
        IRect::from_size(self.info.dimensions())
    }

    /// The bounds of the image (`bounds()`).
    #[must_use]
    pub fn bounds(&self) -> IRect {
        self.bounds_irect()
    }

    /// `hasMipmaps()`: whether the texture has its mip levels.
    #[must_use]
    pub fn has_mipmaps(&self) -> bool {
        self.texture_proxy_view
            .proxy()
            .is_some_and(|p| p.mipmapped() == Mipmapped::Yes)
    }
}

/// The label of a copy of an image's texture (`get_chained_label`): the texture's label with
/// `concat` added, or `empty` when the texture is unlabeled.
// Port of: src/gpu/graphite/Image_Base_Graphite.cpp#L63-L75 (chrome/m156)
fn get_chained_label(image: &Image, empty: &str, concat: &str) -> String {
    let label = image
        .texture_proxy_view()
        .proxy()
        .map_or("", |proxy| proxy.label());
    if label.is_empty() {
        empty.to_owned()
    } else {
        format!("{label}{concat}")
    }
}

/// `Image_Base::onMakeSubset(recorder, subset, requiredProps)`: `this` itself when it already is
/// `subset` with the mipmaps that are required, and a copy of `subset` otherwise.
///
// Port of: src/gpu/graphite/Image_Base_Graphite.cpp#L185-L201 (chrome/m156)
#[doc(alias = "onMakeSubset")]
#[must_use]
pub fn make_subset(
    recorder: &Recorder,
    this: &CoreImage,
    subset: IRect,
    required_props: RequiredProperties,
) -> Option<CoreImage> {
    let image = Image::from_core(this)?;
    // optimization : return self if the subset == our bounds and requirements met and the
    // image's texture is immutable
    if image.bounds() == subset
        && (!required_props.mipmapped || image.has_mipmaps())
        && !image.is_dynamic()
    {
        return Some(this.clone());
    }
    image.copy_image(
        recorder,
        subset,
        Budgeted::No,
        if required_props.mipmapped {
            Mipmapped::Yes
        } else {
            Mipmapped::No
        },
        BackingFit::Exact,
        &get_chained_label(image, "ImageSubsetTexture", "_Subset"),
    )
}

/// `Image_Base::makeNonBudgeted(recorder)`: `this` with a non-budgeted, non-shareable texture. An
/// instantiated budgeted texture is copied; an uninstantiated one is made non-budgeted and
/// instantiated in place, and the image is unlinked from its devices.
// Port of: src/gpu/graphite/Image_Base_Graphite.cpp#L123-L170 (chrome/m156)
#[doc(alias = "makeNonBudgeted")]
#[must_use]
pub fn make_non_budgeted(recorder: &Recorder, this: &CoreImage) -> Option<CoreImage> {
    let image = Image::from_core(this)?;
    let proxy = image.texture_proxy_view().proxy()?;
    // First iterate the proxies held by the image and see if they are instantiated or need to be
    // updated to Budgeted::kNo.
    let mut needs_instantiation = false;
    if proxy.is_instantiated() {
        // At this point, the properties of the TextureProxy are locked in.
        let compatible = proxy.with_texture(|texture| {
            texture.is_some_and(|texture| {
                texture.base().budgeted() == Budgeted::No
                    && texture.base().shareable() == Shareable::No
            })
        });
        if !compatible {
            // Not compatible but instantiated, so make a copy that is non-budgeted.
            return image.copy_image(
                recorder,
                image.bounds(),
                Budgeted::No,
                if image.has_mipmaps() {
                    Mipmapped::Yes
                } else {
                    Mipmapped::No
                },
                BackingFit::Exact,
                &get_chained_label(image, "NonBudgeted", "_NonBudgeted"),
            );
        }
        // else this proxy is already consistent with the contract.
    } else {
        needs_instantiation = true;
    }

    if needs_instantiation {
        // There are presumably tasks (either in the root task list already or on a linked
        // Device) that will initialize this image's proxy. Since the tasks reference the existing
        // TextureProxy, we can't create a new proxy that is non-budgeted. Instead we modify it
        // directly and then unlink this Image from its devices.
        proxy.set_budgeted(Budgeted::No);
        let instantiated = {
            let priv_ = recorder.priv_();
            let mut provider = priv_
                .resource_provider()
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            proxy.instantiate(&mut provider)
        };
        if !instantiated {
            return None;
        }

        image.unlink_devices(recorder);
    }

    // At this point we already were not dynamic, or we unlinked all our devices.
    debug_assert!(!image.is_dynamic());
    Some(this.clone())
}

/// `Image_Base::makeColorTypeAndColorSpace(recorder, targetCT, targetCS, requiredProps)`: `this`
/// itself when the color info already matches, and a copy drawn into the new color info otherwise.
// Port of: src/gpu/graphite/Image_Base_Graphite.cpp#L226-L247 (chrome/m156)
#[doc(alias = "makeColorTypeAndColorSpace")]
#[must_use]
pub fn make_color_type_and_color_space(
    recorder: &Recorder,
    this: &CoreImage,
    target_color_type: ColorType,
    target_color_space: Option<ColorSpace>,
    required_props: RequiredProperties,
) -> Option<CoreImage> {
    let image = Image::from_core(this)?;
    let dst_color_info = ColorInfo::new(target_color_type, this.alpha_type(), target_color_space);
    // optimization : return self if there's no color type/space change and the image's texture
    // is immutable
    if *this.image_info().color_info() == dst_color_info && !image.is_dynamic() {
        return Some(this.clone());
    }
    copy_as_draw(
        recorder,
        None,
        this,
        image.bounds(),
        &dst_color_info,
        Budgeted::No,
        if required_props.mipmapped {
            Mipmapped::Yes
        } else {
            Mipmapped::No
        },
        BackingFit::Exact,
        &get_chained_label(image, "ImageMakeCTandCSTexture", "_CTandCSConversion"),
    )
}

/// `Image::WrapDevice`'s view-level part: the image of a device's target, read with `override_info`
/// when one is given. `None` if the target cannot be sampled or the override does not fit the
/// texture's format or the device's alpha type.
///
/// The image is linked to the device through `link` (`linkDevice`).
// Port of: src/gpu/graphite/Image_Graphite.cpp#L49-L85 (chrome/m156)
#[doc(alias = "WrapDevice")]
#[must_use]
pub fn wrap_device(
    target: &TextureProxyView,
    texturable: bool,
    device_info: &ImageInfo,
    override_info: Option<ColorInfo>,
    link: Arc<DeviceLink>,
) -> Option<Image> {
    if !target.is_valid() || !texturable {
        return None;
    }
    let format = target.proxy()?.format();
    let (view, info) = match override_info {
        Some(override_info) => {
            // If an overrideInfo is provided, it needs to be compatible with the format still and
            // the changes to alpha type need to make sense.
            if !are_color_type_and_format_compatible(override_info.color_type(), format) {
                return None;
            }
            // For alpha type, it should match the device's alpha type or be changing from opaque
            // back to premul or unpremul.
            let override_at = override_info.alpha_type();
            let dev_at = device_info.alpha_type();
            if override_at != dev_at
                && (dev_at != AlphaType::Opaque
                    || (override_at != AlphaType::Premul && override_at != AlphaType::Unpremul))
            {
                return None;
            }
            // Update the swizzle on the texture view to match the change in color type.
            let read_swizzle = read_swizzle_for_color_type(override_info.color_type(), format);
            (target.replace_swizzle(read_swizzle), override_info)
        }
        // Leave the texture view alone since its swizzle should match the device already.
        None => (target.clone(), device_info.color_info().clone()),
    };
    // The image's dimensions are the target's, which can be larger than the device's when the
    // device was created with an approximate backing fit.
    let image = Image::new(view, &info);
    image.link_device(link);
    Some(image)
}

impl ImageBase for Image {
    // Port of: src/gpu/graphite/Image_Graphite.h#L38-L44 (chrome/m156)
    fn info(&self) -> &ImageInfo {
        &self.info
    }

    fn unique_id(&self) -> u32 {
        self.unique_id
    }

    fn image_type(&self) -> ImageType {
        ImageType::Graphite
    }

    // `Image_Base::onReadPixels` is a no-op for Graphite: reading needs a Context.
    fn on_read_pixels(
        &self,
        _dst_info: &ImageInfo,
        _dst_pixels: &mut [u8],
        _dst_row_bytes: usize,
        _src_x: i32,
        _src_y: i32,
    ) -> bool {
        false
    }

    // Port of: src/gpu/graphite/Image_Graphite.h#L48-L50 (chrome/m156)
    fn on_has_mipmaps(&self) -> bool {
        self.has_mipmaps()
    }

    // Port of: src/gpu/graphite/Image_Graphite.h#L51-L53 (chrome/m156)
    fn on_is_protected(&self) -> bool {
        self.texture_proxy_view.is_protected() == Protected::Yes
    }

    fn get_ro_pixels(&self) -> Option<Bitmap> {
        None
    }

    // The subset of a Graphite image is copied with a recorder: see `make_subset_with_recorder`.
    fn on_make_subset(
        &self,
        _subset: &IRect,
        _required_properties: RequiredProperties,
    ) -> Option<CoreImage> {
        None
    }

    // Port of: src/gpu/graphite/Image_Graphite.cpp#L148-L156 (chrome/m156)
    fn on_reinterpret_color_space(&self, new_cs: ColorSpace) -> Option<CoreImage> {
        Some(self.reinterpret_color_space(Some(new_cs)))
    }

    fn make_color_type_and_color_space(
        &self,
        _target_color_type: ColorType,
        _target_color_space: Option<ColorSpace>,
        _required_properties: RequiredProperties,
    ) -> Option<CoreImage> {
        None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
