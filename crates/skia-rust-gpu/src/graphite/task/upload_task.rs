// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/task/UploadTask.h, src/gpu/graphite/task/UploadTask.cpp

//! `UploadTask` and the types that describe what it uploads: `UploadSource`, `UploadInstance`,
//! `UploadList` and `ConditionalUploadContext`.
//!
//! skia-rust: `MipLevel` and `UploadSource` borrow the client's pixels (Skia keeps raw
//! pointers), so they carry a lifetime. `UploadInstance`, `UploadList` and `UploadTask`, which
//! outlive the pixels, copy the data into a transfer buffer and own none of it.

use std::fmt::Debug;
use std::sync::Arc;

use skia_rust_core::align::align_to;
use skia_rust_core::color_space_xform_steps::ColorSpaceXformSteps;
use skia_rust_core::compressed_data_utils::compressed_data_size;
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::mipmap::Mipmap;
use skia_rust_core::rect::IRect;
use skia_rust_core::safe_math::SafeMath;
use skia_rust_core::size::ISize;
use skia_rust_core::texture_compression_type::TextureCompressionType;

use crate::gpu::data_utils::{
    compressed_dimensions, compressed_dimensions_in_blocks, compressed_row_bytes,
};
use crate::gpu::gpu_types::Mipmapped;
use crate::gpu::sk_log::{skia_log_e, skia_log_w};
use crate::graphite::buffer::Buffer;
use crate::graphite::caps::Caps;
use crate::graphite::command_buffer::{BufferTextureCopyData, CommandBuffer};
use crate::graphite::context_priv::ContextPriv;
use crate::graphite::resource::Resource;
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::scratch_resource_manager::ScratchResourceManager;
use crate::graphite::task::{ReplayTargetData, Status, Task, TaskRef};
use crate::graphite::texture_format::{
    texture_format_bytes_per_block, texture_format_compression_type,
};
use crate::graphite::texture_format_xfer_fn::TextureFormatXferFn;
use crate::graphite::texture_proxy::TextureProxy;
use crate::graphite::texture_proxy_view::TextureProxyView;
use crate::graphite::upload_buffer_manager::UploadBufferManager;

/// One mip level of CPU pixels: the pixels (`None` is a missing level) and their row stride.
// Port of: src/gpu/graphite/task/UploadTask.h#L46-L49 (chrome/m156)
#[doc(alias = "skgpu::graphite::MipLevel")]
#[derive(Clone, Copy, Debug, Default)]
pub struct MipLevel<'a> {
    /// `fPixels`.
    pub pixels: Option<&'a [u8]>,
    /// `fRowBytes`.
    pub row_bytes: usize,
}

/// The `ConditionalUploadContext`, if set, is used to determine whether an upload needs to
/// occur on Recording playback. Clients will need to create their own implementations to store
/// the necessary data and override the `needs_upload()` method to do this check.
// Port of: src/gpu/graphite/task/UploadTask.h#L51-L66 (chrome/m156)
#[doc(alias = "skgpu::graphite::ConditionalUploadContext")]
pub trait ConditionalUploadContext: Send + Debug {
    /// `needsUpload()`: returns true if the upload needs to occur; false if it should be skipped
    /// this time.
    #[doc(alias = "needsUpload")]
    fn needs_upload(&mut self, context: &mut dyn ContextPriv) -> bool;

    /// `uploadSubmitted()`: returns true if the upload should be kept in the task (and possibly
    /// re-executed on replay depending on `needs_upload()`'s return value), or false if it
    /// should be discarded and never attempt to be uploaded on any replay.
    #[doc(alias = "uploadSubmitted")]
    fn upload_submitted(&mut self) -> bool {
        true
    }
}

/// `ImageUploadContext` is an implementation of `ConditionalUploadContext` that returns true on
/// the first call to `needs_upload()` and then returns false on subsequent calls. This is used
/// to upload an image once and then avoid redundant uploads after that.
// Port of: src/gpu/graphite/task/UploadTask.h#L68-L79 (chrome/m156)
#[doc(alias = "skgpu::graphite::ImageUploadContext")]
#[derive(Clone, Copy, Debug, Default)]
pub struct ImageUploadContext;

impl ConditionalUploadContext for ImageUploadContext {
    /// Always upload, since it will be discarded right afterwards.
    fn needs_upload(&mut self, _context: &mut dyn ContextPriv) -> bool {
        true
    }

    /// Always return false so the upload instance is discarded after the first execution.
    fn upload_submitted(&mut self) -> bool {
        false
    }
}

// `SkIRect::contains(const SkIRect&)`.
fn irect_contains(outer: &IRect, inner: &IRect) -> bool {
    !inner.is_empty()
        && !outer.is_empty()
        && outer.left <= inner.left
        && outer.top <= inner.top
        && outer.right >= inner.right
        && outer.bottom >= inner.bottom
}

// Returns total buffer size to allocate, and required offset alignment of that allocation.
// Updates 'levelOffsetsAndRowBytes' with offsets relative to start of the allocation, as well as
// the aligned destination rowBytes for each level.
// Port of: src/gpu/graphite/task/UploadTask.cpp#L50-L108 (chrome/m156)
fn compute_combined_buffer_size(
    caps: &dyn Caps,
    format: crate::graphite::texture_format::TextureFormat,
    mip_level_count: usize,
    base_dimensions: ISize,
    level_offsets_and_row_bytes: &mut Vec<(usize, usize)>,
) -> (usize, usize) {
    debug_assert_eq!(level_offsets_and_row_bytes.len(), 0);
    debug_assert!(mip_level_count >= 1);

    let bytes_per_block =
        usize::try_from(texture_format_bytes_per_block(format)).expect("block sizes are positive");
    let compression_type = texture_format_compression_type(format);
    let mut compressed_block_dimensions =
        compressed_dimensions_in_blocks(compression_type, base_dimensions);

    let min_transfer_buffer_alignment =
        bytes_per_block.max(caps.required_transfer_buffer_alignment());

    let mut safe = SafeMath::new();
    let width = usize::try_from(compressed_block_dimensions.width).unwrap_or(0);
    let mut row_bytes = safe.mul(width, bytes_per_block);
    let mut aligned_bytes_per_row =
        caps.get_aligned_texture_data_row_bytes(row_bytes, bytes_per_block);
    level_offsets_and_row_bytes.push((0, aligned_bytes_per_row));
    let height = usize::try_from(base_dimensions.height).unwrap_or(0);
    let rows_size = safe.mul(aligned_bytes_per_row, height);
    let mut combined_buffer_size = safe.align_up_non_pow2(rows_size, min_transfer_buffer_alignment);

    let mut level_dimensions = base_dimensions;
    for _ in 1..mip_level_count {
        level_dimensions = ISize::new(
            1.max(level_dimensions.width / 2),
            1.max(level_dimensions.height / 2),
        );
        compressed_block_dimensions =
            compressed_dimensions_in_blocks(compression_type, level_dimensions);

        let width = usize::try_from(compressed_block_dimensions.width).unwrap_or(0);
        row_bytes = safe.mul(width, bytes_per_block);
        aligned_bytes_per_row = caps.get_aligned_texture_data_row_bytes(row_bytes, bytes_per_block);
        let rows = usize::try_from(compressed_block_dimensions.height).unwrap_or(0);
        let size = safe.mul(aligned_bytes_per_row, rows);
        let aligned_size = safe.align_up_non_pow2(size, min_transfer_buffer_alignment);

        level_offsets_and_row_bytes.push((combined_buffer_size, aligned_bytes_per_row));
        combined_buffer_size = safe.add(combined_buffer_size, aligned_size);
    }

    debug_assert_eq!(level_offsets_and_row_bytes.len(), mip_level_count);

    if !safe.ok() {
        level_offsets_and_row_bytes.clear();
        return (0, min_transfer_buffer_alignment);
    }

    debug_assert!(combined_buffer_size.is_multiple_of(min_transfer_buffer_alignment));
    (combined_buffer_size, min_transfer_buffer_alignment)
}

/// A set of `MipLevel`s, comprising the source data for an upload operation.
///
/// While preparing the upload source, this class additionally caches some needed information,
/// such as whether the upload can be done on the host.
// Port of: src/gpu/graphite/task/UploadTask.h#L81-L131 (chrome/m156)
#[doc(alias = "skgpu::graphite::UploadSource")]
#[derive(Debug)]
pub struct UploadSource<'a> {
    // Technically after we've created the TextureFormatXferFn, we don't need the view's swizzle
    // anymore, but hold on to the view for convenience since moving the original view/proxy into
    // the UploadSource for host uploads is encouraged.
    view: TextureProxyView,
    dst_rect: IRect,
    levels: Vec<MipLevel<'a>>,
    // All valid UploadSources will have a transfer function.
    xfer_fn: Option<TextureFormatXferFn>,
}

impl<'a> UploadSource<'a> {
    // Port of: src/gpu/graphite/task/UploadTask.h#L119-L121 (chrome/m156)
    fn invalid() -> Self {
        Self::with_view(TextureProxyView::default())
    }

    // Port of: src/gpu/graphite/task/UploadTask.cpp#L110 (chrome/m156)
    fn with_view(view: TextureProxyView) -> Self {
        Self {
            view,
            dst_rect: IRect::new_empty(),
            levels: Vec::new(),
            xfer_fn: None,
        }
    }

    /// `Make()`.
    ///
    /// # Panics
    /// If the destination view has no proxy.
    // Port of: src/gpu/graphite/task/UploadTask.cpp#L116-L172 (chrome/m156)
    #[must_use]
    pub fn make(
        _caps: &dyn Caps,
        dst_view: &TextureProxyView,
        src_color_info: &ColorInfo,
        dst_color_info: &ColorInfo,
        levels: &[MipLevel<'a>],
        dst_rect: IRect,
    ) -> UploadSource<'a> {
        // No data to upload
        if dst_rect.is_empty() {
            return Self::invalid();
        }

        let dst_proxy = dst_view.proxy().expect("the destination view has a proxy");

        // Ensure data would fit into the texture
        if !dst_proxy.is_fully_lazy()
            && !irect_contains(&IRect::from_size(dst_view.dimensions()), &dst_rect)
        {
            return Self::invalid();
        }

        let mip_level_count = levels.len();

        // The assumption is either that we have no mipmaps, or that our rect is the entire
        // texture
        if mip_level_count != 1 && dst_rect != IRect::from_size(dst_view.dimensions()) {
            return Self::invalid();
        }

        // We assume that if the texture has mips, we either upload to all the levels or just the
        // first.
        let num_expected_levels = if dst_view.mipmapped() == Mipmapped::Yes {
            usize::try_from(Mipmap::compute_level_count_size(dst_view.dimensions()) + 1)
                .expect("level counts are positive")
        } else {
            1
        };
        if num_expected_levels != mip_level_count {
            return Self::invalid();
        }

        let cs_steps = ColorSpaceXformSteps::new(
            src_color_info.color_space_ref(),
            src_color_info.alpha_type(),
            dst_color_info.color_space_ref(),
            dst_color_info.alpha_type(),
        );
        let Some(xfer_fn) = TextureFormatXferFn::make_cpu_to_gpu(
            src_color_info.color_type(),
            &cs_steps,
            dst_proxy.format(),
            dst_view.swizzle(),
        ) else {
            return Self::invalid();
        };

        let mut source = Self::with_view(dst_view.clone());
        for level in levels {
            // We do not allow any gaps in the mip data
            if level.pixels.is_none() {
                return Self::invalid();
            }
            source.levels.push(*level);
        }

        source.dst_rect = dst_rect;
        source.xfer_fn = Some(xfer_fn);
        source
    }

    /// `MakeCompressed()`: `data` is the whole compressed mip chain.
    // Port of: src/gpu/graphite/task/UploadTask.cpp#L174-L216 (chrome/m156)
    #[must_use]
    pub fn make_compressed(
        _caps: &dyn Caps,
        texture_proxy: Arc<TextureProxy>,
        data: Option<&'a [u8]>,
    ) -> UploadSource<'a> {
        let Some(data) = data else {
            return Self::invalid(); // no data to upload
        };

        let compression = texture_format_compression_type(texture_proxy.format());
        if compression == TextureCompressionType::None {
            return Self::invalid();
        }

        // Create a transfer buffer and fill with data.
        let dimensions = texture_proxy.dimensions();
        let mut src_mip_offsets = Vec::new();
        let computed_size = compressed_data_size(
            compression,
            dimensions,
            Some(&mut src_mip_offsets),
            texture_proxy.mipmapped() == Mipmapped::Yes,
        );
        if computed_size != data.len() {
            return Self::invalid();
        }

        let xfer_fn = TextureFormatXferFn::make_identity(texture_proxy.format());
        debug_assert!(xfer_fn.is_some());

        let mip_level_count = src_mip_offsets.len();
        let mut current_width = texture_proxy.dimensions().width;
        let mut source = Self::with_view(TextureProxyView::from_proxy(Some(texture_proxy)));
        for i in 0..mip_level_count {
            let end = src_mip_offsets.get(i + 1).copied().unwrap_or(data.len());
            source.levels.push(MipLevel {
                pixels: Some(&data[src_mip_offsets[i]..end]),
                // Assume the source data is tightly packed.
                row_bytes: compressed_row_bytes(compression, current_width),
            });
            current_width = 1.max(current_width / 2);
        }

        source.dst_rect = IRect::from_size(dimensions);
        source.xfer_fn = xfer_fn;
        source
    }

    /// `isValid()`.
    #[doc(alias = "isValid")]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.levels.is_empty()
    }

    /// `view()`.
    #[must_use]
    pub fn view(&self) -> &TextureProxyView {
        &self.view
    }

    /// `levels()`.
    #[must_use]
    pub fn levels(&self) -> &[MipLevel<'a>] {
        &self.levels
    }

    /// `dstRect()`.
    #[doc(alias = "dstRect")]
    #[must_use]
    pub fn dst_rect(&self) -> &IRect {
        &self.dst_rect
    }

    /// `formatXferFn()`.
    ///
    /// # Panics
    /// If the source is not valid.
    #[doc(alias = "formatXferFn")]
    #[must_use]
    pub fn format_xfer_fn(&self) -> &TextureFormatXferFn {
        self.xfer_fn
            .as_ref()
            .expect("a valid source has a transfer function")
    }

    /// `attemptUploadOnhost()`: this uploads the data to the texture directly without going
    /// through any command buffer. This is a) not always desired (e.g. for repeated uploads in a
    /// task graph), and b) not always possible (if it's in use by the GPU and the backend
    /// doesn't synchronize a host copy).
    ///
    /// This returns true if the upload succeeded, otherwise false, in which case a regular
    /// upload should be attempted with `UploadInstance`. When uploading on a host is desired,
    /// the target proxy should be moved into the `UploadSource` in order to track that the
    /// target is uniquely held by the current thread (required to ensure no simultaneous GPU use
    /// starts using the texture).
    ///
    /// # Panics
    /// If the source is not valid.
    // Port of: src/gpu/graphite/task/UploadTask.cpp#L218-L237 (chrome/m156)
    #[doc(alias = "attemptUploadOnhost")]
    #[must_use]
    pub fn attempt_upload_on_host(&self) -> bool {
        debug_assert!(self.is_valid());

        let proxy = self.view.proxy().expect("a valid source has a proxy");

        // Don't upload on the host if we need to perform conversions that could be done directly
        // into a mapped GPU buffer.
        if !self.format_xfer_fn().is_identity() || !proxy.is_instantiated() {
            return false;
        }
        let Some(texture) = proxy.ref_texture() else {
            return false;
        };
        if !texture.can_upload_on_host() {
            return false;
        }

        // Don't upload on the host if the UploadSource doesn't have a unique hold on the
        // TextureProxy (otherwise some other thread could trigger GPU work while this thread
        // was modifying the underlying Texture resource).
        if Arc::strong_count(proxy) != 1 {
            return false;
        }

        texture.upload_data_on_host(self)
    }
}

/// An `UploadInstance` represents a single set of uploads from a buffer to texture that can be
/// processed in a single command.
// Port of: src/gpu/graphite/task/UploadTask.h#L133-L168 (chrome/m156)
#[doc(alias = "skgpu::graphite::UploadInstance")]
#[derive(Debug, Default)]
pub struct UploadInstance {
    // The transfer buffer (owned by the UploadBufferManager, then the Recording).
    buffer: Option<Arc<Resource<Buffer>>>,
    texture_proxy: Option<Arc<TextureProxy>>,
    copy_data: Vec<BufferTextureCopyData>,
    conditional_context: Option<Box<dyn ConditionalUploadContext>>,
}

impl UploadInstance {
    /// `Invalid()`.
    #[must_use]
    pub fn invalid() -> Self {
        Self::default()
    }

    /// `isValid()`.
    #[doc(alias = "isValid")]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.buffer.is_some() && self.texture_proxy.is_some()
    }

    /// `Make()`: copies `source` into the transfer buffer the recorder's upload buffer manager
    /// hands out, and describes the copy.
    ///
    /// # Panics
    /// If the source is not valid.
    // Port of: src/gpu/graphite/task/UploadTask.cpp#L252-L337 (chrome/m156)
    #[must_use]
    pub fn make(
        caps: &dyn Caps,
        upload_buffer_manager: &mut UploadBufferManager,
        source: &UploadSource<'_>,
        cond_context: Option<Box<dyn ConditionalUploadContext>>,
    ) -> UploadInstance {
        let texture_proxy = source
            .view()
            .ref_proxy()
            .expect("a valid source has a proxy");
        let dst_rect = *source.dst_rect();
        let compression = texture_format_compression_type(texture_proxy.format());

        let levels = source.levels();
        let mip_level_count = levels.len();
        let mut level_offsets_and_row_bytes = Vec::with_capacity(mip_level_count);
        let (combined_buffer_size, min_alignment) = compute_combined_buffer_size(
            caps,
            texture_proxy.format(),
            mip_level_count,
            dst_rect.size(),
            &mut level_offsets_and_row_bytes,
        );
        if combined_buffer_size == 0 {
            return Self::invalid();
        }

        let Some((mut writer, buffer_info)) =
            upload_buffer_manager.get_texture_upload_writer(combined_buffer_size, min_alignment)
        else {
            skia_log_w!(
                "Failed to get write-mapped buffer for texture upload of size {}",
                combined_buffer_size
            );
            return Self::invalid();
        };

        // ATRACE_ANDROID_FRAMEWORK("Upload %s %sTexture [%dx%d]") is a tracing hook only.

        let mut upload = UploadInstance {
            buffer: buffer_info.buffer.clone(),
            texture_proxy: Some(texture_proxy),
            copy_data: Vec::with_capacity(1),
            conditional_context: cond_context,
        };

        // Fill in copy data
        let mut current_width = dst_rect.width();
        let mut current_height = dst_rect.height();
        for (current_mip_level, level) in levels.iter().enumerate() {
            // NOTE: When not compressed, this function automatically returns currentWidth and
            // height, e.g. uncompressed blocks are the same as texels.
            let block_dimensions = compressed_dimensions_in_blocks(
                compression,
                ISize::new(current_width, current_height),
            );

            let src_row_bytes = level.row_bytes;
            let (dst_mip_offset, dst_row_bytes) = level_offsets_and_row_bytes[current_mip_level];

            // copy data into the buffer, skipping any trailing bytes
            let src = level.pixels.expect("valid sources have all their levels");
            writer.convert(
                dst_mip_offset,
                usize::try_from(block_dimensions.width).expect("positive"),
                usize::try_from(block_dimensions.height).expect("positive"),
                src,
                src_row_bytes,
                source.format_xfer_fn(),
                dst_row_bytes,
            );

            let mut copy_width = current_width;
            let mut copy_height = current_height;
            if compression != TextureCompressionType::None
                && caps.full_compressed_upload_size_must_align_to_block_dims()
            {
                let one_block_dims = compressed_dimensions(compression, ISize::new(1, 1));
                copy_width = align_to(copy_width, one_block_dims.width);
                copy_height = align_to(copy_height, one_block_dims.height);
            }

            // For compressed and mipped data, the dstRect is always the full texture so we don't
            // need to worry about modifying the TL coord as it will always be 0,0,for all
            // levels.
            debug_assert!(
                (dst_rect.left() == 0 && dst_rect.top() == 0)
                    || (mip_level_count == 1 && compression == TextureCompressionType::None)
            );
            upload.copy_data.push(BufferTextureCopyData {
                buffer_offset: buffer_info.offset as usize + dst_mip_offset,
                buffer_row_bytes: dst_row_bytes,
                rect: IRect::from_xywh(dst_rect.left(), dst_rect.top(), copy_width, copy_height),
                mip_level: u32::try_from(current_mip_level).expect("few mip levels"),
            });

            current_width = 1.max(current_width / 2);
            current_height = 1.max(current_height / 2);
        }

        upload
    }

    /// `prepareResources()`.
    ///
    /// # Panics
    /// If the instance is not valid.
    // Port of: src/gpu/graphite/task/UploadTask.cpp#L339-L356 (chrome/m156)
    #[doc(alias = "prepareResources")]
    pub fn prepare_resources(&self, resource_provider: &mut ResourceProvider) -> bool {
        // While most uploads are to already instantiated proxies (e.g. for client-created
        // texture images) it is possible that writePixels() was issued as the first operation
        // on a scratch Device, or that this is the first upload to the raster or text atlas
        // proxies.
        // TODO: Determine how to instantatiate textues in this case; atlas proxies shouldn't
        // really be "scratch" because they aren't going to be reused for anything else in a
        // Recording. At the same time, it could still go through the ScratchResourceManager and
        // just never return them, which is no different from instantiating them directly with
        // the ResourceProvider.
        let proxy = self.texture_proxy.as_ref().expect("a valid instance");
        if !TextureProxy::instantiate_if_not_lazy(resource_provider, proxy) {
            skia_log_e!("Could not instantiate texture proxy for UploadTask!");
            return false;
        }
        true
    }

    /// `addCommand()`: adds the upload command to the given `CommandBuffer`; the status is
    /// `Discard` if the instance should be discarded.
    ///
    /// # Panics
    /// If the instance is not valid, or its proxy is not instantiated.
    // Port of: src/gpu/graphite/task/UploadTask.cpp#L358-L443 (chrome/m156)
    #[allow(clippy::if_not_else)] // keeps the C++ branch order
    #[doc(alias = "addCommand")]
    pub fn add_command(
        &mut self,
        context: &mut dyn ContextPriv,
        command_buffer: &mut dyn CommandBuffer,
        replay_data: &ReplayTargetData,
    ) -> Status {
        let texture_proxy = self.texture_proxy.clone().expect("a valid instance");
        debug_assert!(texture_proxy.is_instantiated());

        if let Some(cond) = &mut self.conditional_context
            && !cond.needs_upload(context)
        {
            // Assume that if a conditional context says to dynamically not upload that another
            // time through the tasks should try to upload again.
            return Status::Success;
        }

        let buffer = self.buffer.as_ref().expect("a valid instance");
        let texture = texture_proxy
            .ref_texture()
            .expect("the proxy is instantiated");
        let is_replay_target = replay_data.is_target(Some(texture.as_arc()));
        if !is_replay_target {
            // The CommandBuffer doesn't take ownership of the upload buffer here; it's owned by
            // UploadBufferManager, which will transfer ownership in transferToCommandBuffer.
            if !command_buffer.copy_buffer_to_texture(buffer, texture, &self.copy_data) {
                return Status::Fail;
            }
        } else {
            // Here we assume that multiple copies in a single UploadInstance are always used for
            // mipmaps of a single image, and that we won't ever upload to a replay target's
            // mipmaps directly.
            debug_assert_eq!(self.copy_data.len(), 1);
            let copy_data = &self.copy_data[0];
            let mut dst_rect = copy_data.rect;
            dst_rect.offset(replay_data.translation);
            let mut cropped_dst_rect = dst_rect;
            if !replay_data.clip.is_empty() {
                let mut dst_clip = replay_data.clip;
                dst_clip.offset(replay_data.translation);
                let Some(intersection) = IRect::intersect(&cropped_dst_rect, &dst_clip) else {
                    // The replay clip can change on each insert, so subsequent replays may
                    // actually intersect the copy rect.
                    return Status::Success;
                };
                cropped_dst_rect = intersection;
            }
            let Some(intersection) = IRect::intersect(
                &cropped_dst_rect,
                &IRect::from_size(texture_proxy.dimensions()),
            ) else {
                // The replay translation can change on each insert, so subsequent replays may
                // actually intersect the copy rect.
                return Status::Success;
            };
            cropped_dst_rect = intersection;

            let bpp = usize::try_from(texture_format_bytes_per_block(texture_proxy.format()))
                .expect("block sizes are positive");
            let mut transformed_copy_data = *copy_data;
            // The offsets are `int` products added to a `size_t` in C++, so they wrap the same
            // way for negative values.
            let row_delta = isize::try_from(cropped_dst_rect.y() - dst_rect.y())
                .expect("fits")
                .wrapping_mul(isize::try_from(copy_data.buffer_row_bytes).expect("fits"));
            let col_delta = isize::try_from(cropped_dst_rect.x() - dst_rect.x())
                .expect("fits")
                .wrapping_mul(isize::try_from(bpp).expect("fits"));
            transformed_copy_data.buffer_offset = transformed_copy_data
                .buffer_offset
                .wrapping_add_signed(row_delta.wrapping_add(col_delta));
            transformed_copy_data.rect = cropped_dst_rect;

            if !command_buffer.copy_buffer_to_texture(
                buffer,
                texture,
                std::slice::from_ref(&transformed_copy_data),
            ) {
                return Status::Fail;
            }
        }

        // The conditional context will return false if the upload should not happen anymore. If
        // there's no context assume that the upload should always be executed on replay.
        if let Some(cond) = &mut self.conditional_context
            && !cond.upload_submitted()
        {
            Status::Discard
        } else {
            Status::Success
        }
    }
}

/// An `UploadList` is a mutable collection of `UploadCommands`.
///
/// Currently commands are accumulated in order and processed in the same order. Dependency
/// management is expected to be handled by the `TaskGraph`.
///
/// When an upload is appended to the list its data will be copied to a Buffer in preparation for
/// a deferred upload.
// Port of: src/gpu/graphite/task/UploadTask.h#L170-L189 (chrome/m156)
#[doc(alias = "skgpu::graphite::UploadList")]
#[derive(Debug, Default)]
pub struct UploadList {
    instances: Vec<UploadInstance>,
}

impl UploadList {
    /// An empty list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `recordUpload()`.
    // Port of: src/gpu/graphite/task/UploadTask.cpp#L445-L457 (chrome/m156)
    #[doc(alias = "recordUpload")]
    pub fn record_upload(
        &mut self,
        caps: &dyn Caps,
        upload_buffer_manager: &mut UploadBufferManager,
        source: &UploadSource<'_>,
        cond_context: Option<Box<dyn ConditionalUploadContext>>,
    ) -> bool {
        let instance = UploadInstance::make(caps, upload_buffer_manager, source, cond_context);
        if !instance.is_valid() {
            return false;
        }
        self.instances.push(instance);
        true
    }

    /// `size()`.
    #[must_use]
    pub fn size(&self) -> usize {
        self.instances.len()
    }
}

/// An `UploadTask` is a immutable collection of `UploadCommands`.
///
/// When adding commands to the commandBuffer the texture proxies in those commands will be
/// instantiated and the copy command added.
// Port of: src/gpu/graphite/task/UploadTask.h#L191-L240 (chrome/m156)
#[doc(alias = "skgpu::graphite::UploadTask")]
#[derive(Debug)]
pub struct UploadTask {
    instances: Vec<UploadInstance>,
}

impl UploadTask {
    /// `Make(UploadList*)`: takes the list's instances; `None` if it has none.
    // Port of: src/gpu/graphite/task/UploadTask.cpp#L461-L470 (chrome/m156)
    #[must_use]
    pub fn make(upload_list: &mut UploadList) -> Option<TaskRef> {
        if upload_list.size() == 0 {
            return None;
        }
        Some(
            Task::Upload(UploadTask {
                instances: std::mem::take(&mut upload_list.instances),
            })
            .into_ref(),
        )
    }

    /// `Make(UploadInstance)`: `None` if the instance is not valid.
    // Port of: src/gpu/graphite/task/UploadTask.cpp#L472-L477 (chrome/m156)
    #[must_use]
    pub fn make_instance(instance: UploadInstance) -> Option<TaskRef> {
        if !instance.is_valid() {
            return None;
        }
        Some(
            Task::Upload(UploadTask {
                instances: vec![instance],
            })
            .into_ref(),
        )
    }

    /// `prepareResources()`.
    // Port of: src/gpu/graphite/task/UploadTask.cpp#L488-L498 (chrome/m156)
    pub fn prepare_resources(
        &mut self,
        resource_provider: &mut ResourceProvider,
        _scratch_manager: &mut ScratchResourceManager,
        _runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
    ) -> Status {
        for instance in &self.instances {
            // No upload should be invalidated before prepareResources() is called.
            debug_assert!(instance.is_valid());
            if !instance.prepare_resources(resource_provider) {
                return Status::Fail;
            }
        }
        Status::Success
    }

    /// `addCommands()`.
    // Port of: src/gpu/graphite/task/UploadTask.cpp#L500-L525 (chrome/m156)
    pub fn add_commands(
        &mut self,
        context: &mut dyn ContextPriv,
        command_buffer: &mut dyn CommandBuffer,
        replay_data: &ReplayTargetData,
    ) -> Status {
        let mut discard_count = 0;
        for instance in &mut self.instances {
            if !instance.is_valid() {
                discard_count += 1;
                continue;
            }

            let status = instance.add_command(context, command_buffer, replay_data);
            if status == Status::Fail {
                return Status::Fail;
            } else if status == Status::Discard {
                *instance = UploadInstance::invalid();
                discard_count += 1;
            }
        }
        if discard_count == self.instances.len() {
            Status::Discard
        } else {
            Status::Success
        }
    }

    /// `visitProxies()`: textures being uploaded to are never read from, so skip all visiting
    /// unless `reads_only` is false.
    // Port of: src/gpu/graphite/task/UploadTask.h#L213-L224 (chrome/m156)
    pub fn visit_proxies(
        &mut self,
        visitor: &mut dyn FnMut(&Arc<TextureProxy>) -> bool,
        reads_only: bool,
    ) -> bool {
        if !reads_only {
            for instance in &self.instances {
                if instance.is_valid()
                    && let Some(proxy) = &instance.texture_proxy
                    && !visitor(proxy)
                {
                    return false;
                }
            }
        }
        true
    }
}
