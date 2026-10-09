// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/StorageContext.{h,cpp} (chrome/m156)

//! `StorageContext`: the per-recording CPU copy of gradient data and appended vertices, written
//! into one storage buffer (or, without storage-buffer support, a fallback RGBA32F texture).
//!
//! `finalize()` takes the texture path's `DrawContext::recordDependency` as a closure: the
//! `DrawContext` is G10a and does not exist yet. The closure receives the upload task, which the
//! caller records exactly where `recordDependency` would have put it.

// The size_t, int and uint32_t casts below mirror the C++ arithmetic of StorageContext.cpp, whose
// offsets and sizes are checked against the limits before each cast.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use std::collections::HashMap;
use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_effects::gradient_base_shader::GradientBaseShader;

use crate::gpu::gpu_types::Budgeted;
use crate::graphite::buffer::BindBufferInfo;
use crate::graphite::buffer_manager::buffer_aligner;
use crate::graphite::recorder::Recorder;
use crate::graphite::task::TaskRef;
use crate::graphite::task::upload_task::{MipLevel, UploadInstance, UploadSource, UploadTask};
use crate::graphite::texture_format::{TextureFormat, read_swizzle_for_color_type};
use crate::graphite::texture_proxy::TextureProxy;
use crate::graphite::texture_proxy_view::TextureProxyView;

/// `StorageContext::kTexelBytes`.
pub const TEXEL_BYTES: usize = 16;
/// `StorageContext::kColorType`.
pub const COLOR_TYPE: ColorType = ColorType::RGBAF32;
/// `StorageContext::kTextureFormat`.
pub const TEXTURE_FORMAT: TextureFormat = TextureFormat::RGBA32F;

/// `GradientCache::kMaxGradientStops`.
const MAX_GRADIENT_STOPS: i32 = 1024 * 1024;
/// `GradientCache::kMaxStorageFloats`.
const MAX_STORAGE_FLOATS: i32 = (u32::MAX as usize / std::mem::size_of::<f32>()) as i32;

/// `StorageContextResult`: a bound range of a storage buffer, or a fallback texture.
// Port of: src/gpu/graphite/StorageContext.h#L30-L30 (chrome/m156)
#[derive(Clone, Debug)]
pub enum StorageContextResult {
    /// `BindBufferInfo`.
    Buffer(BindBufferInfo),
    /// `sk_sp<TextureProxy>`.
    Texture(Arc<TextureProxy>),
}

/// `StorageContext::GradientCache`.
// Port of: src/gpu/graphite/StorageContext.h#L63-L75 (chrome/m156)
#[derive(Debug, Default)]
struct GradientCache {
    // `fLocalGradientOffsetCache`, keyed by the shader's address (see `allocate_gradient_data`).
    local_gradient_offset_cache: HashMap<usize, i32>,
    gradient_data: Vec<f32>,
    gradient_data_size: usize,
}

impl GradientCache {
    // Port of: src/gpu/graphite/StorageContext.cpp#L49-L56 (chrome/m156)
    fn reset(&mut self) {
        self.local_gradient_offset_cache.clear();
        self.gradient_data.clear();
        self.gradient_data_size = 0;
    }

    // Port of: src/gpu/graphite/StorageContext.h#L76-L76 (chrome/m156)
    fn is_empty(&self) -> bool {
        self.gradient_data.is_empty()
    }
}

/// Accumulates the gradient and vertex data of one recording until it is finalized.
// Port of: src/gpu/graphite/StorageContext.h#L30-L114 (chrome/m156)
#[doc(alias = "skgpu::graphite::StorageContext")]
#[derive(Debug)]
pub struct StorageContext {
    gradient_cache: GradientCache,
    vertex_data: Vec<u8>,
    running_lcm: u32,
    max_fallback_texture_size: i32,
    storage_buffer_support: bool,
    #[cfg(debug_assertions)]
    finalized: bool,
}

impl StorageContext {
    /// `StorageContext(maxFallbackTextureSize, storageBufferSupport)`.
    // Port of: src/gpu/graphite/StorageContext.cpp#L51-L57 (chrome/m156)
    #[must_use]
    pub fn new(max_fallback_texture_size: i32, storage_buffer_support: bool) -> Self {
        debug_assert!(max_fallback_texture_size > 0);
        Self {
            gradient_cache: GradientCache::default(),
            vertex_data: Vec::new(),
            running_lcm: 1,
            max_fallback_texture_size,
            storage_buffer_support,
            #[cfg(debug_assertions)]
            finalized: false,
        }
    }

    /// `resetCache()`: drops the cached gradient and vertex data.
    // Port of: src/gpu/graphite/StorageContext.cpp#L63-L69 (chrome/m156)
    pub fn reset_cache(&mut self) {
        self.gradient_cache.reset();
        self.vertex_data.clear();
        self.running_lcm = 1;
        #[cfg(debug_assertions)]
        {
            self.finalized = false;
        }
    }

    /// `reset()`: a stub that will call more than `reset_cache()` in the future.
    // Port of: src/gpu/graphite/StorageContext.h#L43-L45 (chrome/m156)
    pub fn reset(&mut self) {
        self.reset_cache();
    }

    /// `allocateGradientData(numStops, shader)`: the float slice a gradient's data is written
    /// into and its offset in bytes, or `None` with the existing offset if `shader` already has
    /// data in this recording, or `None` with `-1` if it does not fit.
    ///
    /// The C++ cache keys on the shader pointer and takes a reference to keep it alive. Here the
    /// key is the shader's address: the caller keeps `shader` alive until the cache is reset,
    /// which is what the reference was for.
    // Port of: src/gpu/graphite/StorageContext.cpp#L71-L100 (chrome/m156)
    #[doc(alias = "allocateGradientData")]
    pub fn allocate_gradient_data(
        &mut self,
        num_stops: i32,
        shader: &GradientBaseShader,
    ) -> (Option<&mut [f32]>, i32) {
        debug_assert!(!self.is_finalized());
        if num_stops > MAX_GRADIENT_STOPS {
            return (None, -1);
        }

        let key = std::ptr::from_ref(shader) as usize;
        if let Some(&existing_local_offset) =
            self.gradient_cache.local_gradient_offset_cache.get(&key)
        {
            return (None, existing_local_offset);
        }

        let float_offset = self.gradient_cache.gradient_data.len() as i32;
        let float_count = if self.storage_buffer_support {
            num_stops * 5
        } else {
            // SkAlign4(numStops) + numStops * 4
            ((num_stops + 3) & !3) + num_stops * 4
        };
        debug_assert!(self.storage_buffer_support || float_count % 4 == 0);

        if MAX_STORAGE_FLOATS - float_count < float_offset {
            return (None, -1);
        }

        let start = float_offset as usize;
        let end = start + float_count as usize;
        self.gradient_cache.gradient_data.resize(end, 0.0);
        self.gradient_cache
            .local_gradient_offset_cache
            .insert(key, float_offset);
        self.gradient_cache.gradient_data_size = self.gradient_cache.gradient_data.len() * 4;
        (
            Some(&mut self.gradient_cache.gradient_data[start..end]),
            float_offset,
        )
    }

    /// `recordAlignment(stride, align)`: folds a draw's stride and alignment into the running
    /// LCM. Writes no data.
    // Port of: src/gpu/graphite/StorageContext.cpp#L102-L113 (chrome/m156)
    pub fn record_alignment(&mut self, stride: usize, align: usize) {
        debug_assert!(stride > 0 && align > 0);
        debug_assert!(!self.is_finalized());
        let (mut stride, mut align) = (stride, align);
        if !self.storage_buffer_support {
            align = align.max(TEXEL_BYTES);
            stride = stride.div_ceil(TEXEL_BYTES) * TEXEL_BYTES;
        }
        let align32 = buffer_aligner::lcm_alignment(align as u32, stride as u32);
        self.running_lcm = buffer_aligner::lcm_alignment(self.running_lcm, align32);
    }

    /// `appendVertices(data, count, stride, align)`: appends `count * stride` bytes of vertex
    /// data, aligned to `stride` and `align`. Returns the byte offset within the storage buffer
    /// (0 if the data cannot be represented).
    ///
    /// `data` holds at least `count * stride` bytes.
    // Port of: src/gpu/graphite/StorageContext.cpp#L115-L158 (chrome/m156)
    #[doc(alias = "appendVertices")]
    pub fn append_vertices(
        &mut self,
        data: &[u8],
        count: usize,
        stride: usize,
        align: usize,
    ) -> u32 {
        debug_assert!(!data.is_empty() && count > 0 && stride > 0 && align > 0);
        let mut padded_stride = stride;
        let mut padded_align = align;
        if !self.storage_buffer_support {
            padded_stride = stride.div_ceil(TEXEL_BYTES) * TEXEL_BYTES;
            padded_align = align.max(TEXEL_BYTES);
        }

        let align32 = buffer_aligner::lcm_alignment(padded_align as u32, padded_stride as u32);
        debug_assert!(self.running_lcm.is_multiple_of(align32));

        let required_bytes =
            buffer_aligner::validate_count_and_stride(count, padded_stride, 0, align32);
        if required_bytes == 0 {
            return 0;
        }

        let size = self.vertex_data.len() as u32;
        let aligned_vert_offset = size.div_ceil(align32) * align32;
        if aligned_vert_offset > size {
            let pad_bytes = (aligned_vert_offset - size) as usize;
            self.vertex_data
                .resize(self.vertex_data.len() + pad_bytes, 0);
        }

        let required_bytes = required_bytes as usize;
        // Because the fallback texture format is kRGBA32F (16 bytes per texel), the unpack shader
        // addresses instances by whole texels (index * nTexels). Pad each instance's stride to a
        // 16-byte texel boundary so subsequent instances align with the shader's texel indexing.
        if stride == padded_stride {
            self.vertex_data.extend_from_slice(&data[..required_bytes]);
        } else {
            let diff = padded_stride - stride;
            for i in 0..count {
                let src = &data[i * stride..(i + 1) * stride];
                self.vertex_data.extend_from_slice(src);
                self.vertex_data.resize(self.vertex_data.len() + diff, 0);
            }
        }

        // SkTo<uint32_t>(fGradientCache.fGradientDataSize) + alignedVertOffset
        (self.gradient_cache.gradient_data_size as u32).wrapping_add(aligned_vert_offset)
    }

    /// `finalizePrecachedStorageData()`: pads the precached gradient data to the running LCM so
    /// appended data starts at a valid offset. Does no GPU work.
    // Port of: src/gpu/graphite/StorageContext.cpp#L160-L164 (chrome/m156)
    pub fn finalize_precached_storage_data(&mut self) {
        let lcm = self.running_lcm as usize;
        self.gradient_cache.gradient_data_size =
            self.gradient_cache.gradient_data_size.div_ceil(lcm) * lcm;
        #[cfg(debug_assertions)]
        {
            self.finalized = true;
        }
    }

    /// `finalize(recorder, drawContext)`: writes the gradient and vertex data into one storage
    /// buffer, or into a fallback texture (whose upload task goes to `record_dependency`).
    /// `None` if there is no data or the data cannot be written.
    // Port of: src/gpu/graphite/StorageContext.cpp#L166-L183 (chrome/m156)
    pub fn finalize(
        &mut self,
        recorder: &Recorder,
        record_dependency: &mut dyn FnMut(TaskRef),
    ) -> Option<StorageContextResult> {
        debug_assert!(self.is_finalized());
        #[cfg(debug_assertions)]
        {
            self.finalized = false;
        }

        if self.is_empty() {
            return None;
        }

        if self.storage_buffer_support {
            self.finalize_storage_buffer(recorder)
                .map(StorageContextResult::Buffer)
        } else {
            self.finalize_texture(recorder, record_dependency)
                .map(StorageContextResult::Texture)
        }
    }

    /// `isEmpty()`.
    // Port of: src/gpu/graphite/StorageContext.h#L47-L47 (chrome/m156)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.gradient_cache.is_empty() && self.vertex_data.is_empty()
    }

    /// `runningLCM()` (`GPU_TEST_UTILS`).
    #[must_use]
    pub fn running_lcm(&self) -> u32 {
        self.running_lcm
    }

    #[cfg(debug_assertions)]
    fn is_finalized(&self) -> bool {
        self.finalized
    }

    #[cfg(not(debug_assertions))]
    fn is_finalized(&self) -> bool {
        true
    }

    // The gradient data as the bytes a float array occupies in memory.
    fn gradient_bytes(&self) -> Vec<u8> {
        self.gradient_cache
            .gradient_data
            .iter()
            .flat_map(|f| f.to_ne_bytes())
            .collect()
    }

    /// `finalizeStorageBuffer(recorder)`.
    // Port of: src/gpu/graphite/StorageContext.cpp#L185-L211 (chrome/m156)
    fn finalize_storage_buffer(&self, recorder: &Recorder) -> Option<BindBufferInfo> {
        let priv_ = recorder.priv_();
        let buffer_mgr = priv_.draw_buffer_manager();

        let total_bytes = self.gradient_cache.gradient_data_size + self.vertex_data.len();
        if total_bytes == 0 {
            return None;
        }
        let mut mapped = buffer_mgr.get_mapped_storage_buffer(total_bytes, /*stride=*/ 1)?;
        let mut writer = mapped.writer();
        if !self.gradient_cache.is_empty() {
            let gradient_bytes = self.gradient_bytes();
            writer.write_bytes(&gradient_bytes);
            if self.gradient_cache.gradient_data_size > gradient_bytes.len() {
                writer.zero_bytes(self.gradient_cache.gradient_data_size - gradient_bytes.len());
            }
        }
        if !self.vertex_data.is_empty() {
            writer.write_bytes(&self.vertex_data);
        }
        Some(mapped.binding)
    }

    /// `finalizeTexture(recorder, drawContext)`.
    // Port of: src/gpu/graphite/StorageContext.cpp#L213-L285 (chrome/m156)
    fn finalize_texture(
        &self,
        recorder: &Recorder,
        record_dependency: &mut dyn FnMut(TaskRef),
    ) -> Option<Arc<TextureProxy>> {
        let grad_size = self.gradient_cache.gradient_data_size;
        let vert_size = self.vertex_data.len();
        let total_bytes = grad_size + vert_size;
        if total_bytes == 0 {
            return None;
        }

        let total_texels = total_bytes.div_ceil(TEXEL_BYTES) as i32;
        let width = total_texels.min(self.max_fallback_texture_size);
        let height =
            (total_texels + self.max_fallback_texture_size - 1) / self.max_fallback_texture_size;
        if height > self.max_fallback_texture_size {
            return None;
        }

        let atlas_row_bytes = width as usize * TEXEL_BYTES;
        let padded_bytes = height as usize * atlas_row_bytes;
        let mut upload_buffer = vec![0u8; padded_bytes];
        if grad_size > 0 {
            let gradient_bytes = self.gradient_bytes();
            upload_buffer[..gradient_bytes.len()].copy_from_slice(&gradient_bytes);
        }
        if !self.vertex_data.is_empty() {
            upload_buffer[grad_size..grad_size + vert_size].copy_from_slice(&self.vertex_data);
        }

        let priv_ = recorder.priv_();
        let caps = priv_.caps();
        let texture_info =
            caps.get_default_readable_texture_info(TEXTURE_FORMAT, priv_.is_protected());
        let proxy = TextureProxy::make(
            caps.as_ref(),
            &mut priv_
                .resource_provider()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            ISize::new(width, height),
            &texture_info,
            Budgeted::Yes,
            "StorageFallbackTexture",
        )?;

        let level = MipLevel {
            pixels: Some(&upload_buffer),
            row_bytes: atlas_row_bytes,
        };
        let dst_rect = IRect::from_wh(width, height);
        let read_swizzle = read_swizzle_for_color_type(COLOR_TYPE, proxy.format());
        let proxy_view = TextureProxyView::new(Some(proxy), read_swizzle);
        let color_info = ColorInfo::new(COLOR_TYPE, AlphaType::Premul, None);
        let source = UploadSource::make(
            caps.as_ref(),
            &proxy_view,
            &color_info,
            &color_info,
            std::slice::from_ref(&level),
            dst_rect,
        );
        let instance = UploadInstance::make(
            caps.as_ref(),
            &mut priv_.upload_buffer_manager().borrow_mut(),
            &source,
            None,
        );
        if !instance.is_valid() {
            return None;
        }
        let upload_task = UploadTask::make_instance(instance)?;
        record_dependency(upload_task);
        proxy_view.ref_proxy()
    }
}
