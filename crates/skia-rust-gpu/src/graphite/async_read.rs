// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/AsyncReadTypes.h (TAsyncReadResult),
//                   src/gpu/graphite/Context.h (Context::PixelTransferResult),
//                   src/gpu/graphite/Context.cpp (AsyncParams)

//! The types of `Context`'s asynchronous readback: [`PixelTransferResult`] (a transfer buffer
//! with the copy of a texture region and how to convert it), [`AsyncReadResult`]
//! (`TAsyncReadResult` / `SkImage::AsyncReadResult`, the planes the client gets), and
//! [`AsyncReadParams`] (`Context::AsyncParams<TextureProxyView>`).
//!
//! # Mapped memory without raw pointers
//!
//! `TAsyncReadResult` hands out the mapped pointer of the transfer buffer. The port's
//! `Buffer::map()` hands out an owned copy of the mapped range (see `buffer.rs`), so a plane
//! keeps that copy for `data()`, and the mapped buffer for what mapped memory is for: the client
//! manager unmaps it once the result is dropped, and a buffer that is still mapped is not reused.

use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_core::image_info::{ColorInfo, ImageInfo};
use skia_rust_core::rect::{Contains, IRect};
use skia_rust_core::size::ISize;

use crate::gpu::async_read_types::BufferFinishedMessage;
use crate::graphite::buffer::Buffer;
use crate::graphite::client_mapped_buffer_manager::{
    ClientBufferFinishedSender, ClientMappedBufferManager, ContextId,
};
use crate::graphite::resource::ResourceRef;
use crate::graphite::texture_proxy_view::TextureProxyView;

/// `PixelTransferResult::fPixelConverter`: converts the transferred pixels (`src`) to the
/// client's (`dst`).
pub type PixelConverter = Arc<dyn Fn(&mut [u8], &[u8]) + Send + Sync>;

/// `Context::PixelTransferResult`: the transfer buffer a texture region is copied to.
// Port of: src/gpu/graphite/Context.h#L255-L267 (chrome/m156)
#[doc(alias = "Context::PixelTransferResult")]
#[derive(Default)]
pub struct PixelTransferResult {
    /// `fTransferBuffer`: `None` if the transfer could not be set up.
    pub transfer_buffer: Option<ResourceRef<Buffer>>,
    /// `fSize`.
    pub size: ISize,
    /// `fRowBytes`: of the data after `pixel_converter`, if there is one.
    pub row_bytes: usize,
    /// `fPixelConverter`: set if the transferred pixels need converting.
    pub pixel_converter: Option<PixelConverter>,
}

impl std::fmt::Debug for PixelTransferResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PixelTransferResult")
            .field("has_transfer_buffer", &self.transfer_buffer.is_some())
            .field("size", &self.size)
            .field("row_bytes", &self.row_bytes)
            .field("has_pixel_converter", &self.pixel_converter.is_some())
            .finish()
    }
}

/// One plane of an [`AsyncReadResult`].
struct Plane {
    data: Vec<u8>,
    row_bytes: usize,
    /// The mapped buffer the data was read from, which is unmapped when the result is dropped.
    mapped_buffer: Option<ResourceRef<Buffer>>,
}

/// `SkImage::AsyncReadResult` as `TAsyncReadResult` implements it: the pixels of an async read.
// Port of: src/gpu/AsyncReadTypes.h#L134-L227 (chrome/m156)
#[doc(alias = "TAsyncReadResult")]
pub struct AsyncReadResult {
    planes: Vec<Plane>,
    intended_recipient: ContextId,
    sender: ClientBufferFinishedSender,
}

impl std::fmt::Debug for AsyncReadResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncReadResult")
            .field("count", &self.planes.len())
            .finish_non_exhaustive()
    }
}

impl AsyncReadResult {
    /// `TAsyncReadResult(intendedRecipient)`: the buffers of the planes are released to the
    /// manager that `sender` posts to.
    #[must_use]
    pub fn new(intended_recipient: ContextId, sender: ClientBufferFinishedSender) -> Self {
        Self {
            planes: Vec::new(),
            intended_recipient,
            sender,
        }
    }

    /// `count()`: the number of planes.
    #[must_use]
    pub fn count(&self) -> usize {
        self.planes.len()
    }

    /// `data(i)`: the pixels of plane `i`.
    #[must_use]
    pub fn data(&self, i: usize) -> &[u8] {
        &self.planes[i].data
    }

    /// `rowBytes(i)`.
    #[doc(alias = "rowBytes")]
    #[must_use]
    pub fn row_bytes(&self, i: usize) -> usize {
        self.planes[i].row_bytes
    }

    /// `addTransferResult()`: adds the plane for a finished transfer, reading its buffer, which
    /// the manager keeps (and later unmaps) unless it was converted. False if the buffer could
    /// not be mapped.
    // Port of: src/gpu/AsyncReadTypes.h#L152-L169 (chrome/m156)
    #[doc(alias = "addTransferResult")]
    pub fn add_transfer_result(
        &mut self,
        result: &PixelTransferResult,
        dimensions: ISize,
        row_bytes: usize,
        manager: &mut ClientMappedBufferManager,
    ) -> bool {
        let Some(transfer_buffer) = &result.transfer_buffer else {
            return false;
        };
        let Some(mapped_data) = transfer_buffer.map() else {
            return false;
        };
        if let Some(pixel_converter) = &result.pixel_converter {
            let height = usize::try_from(dimensions.height).unwrap_or(0);
            let size = row_bytes * height;
            let mut data = vec![0u8; size];
            pixel_converter(&mut data, &mapped_data);
            debug_assert!(row_bytes > 0);
            self.planes.push(Plane {
                data,
                row_bytes,
                mapped_buffer: None,
            });
            transfer_buffer.unmap();
        } else {
            manager.insert(transfer_buffer.clone());
            debug_assert!(row_bytes > 0);
            debug_assert!(transfer_buffer.is_mapped());
            self.planes.push(Plane {
                data: mapped_data,
                row_bytes,
                mapped_buffer: Some(transfer_buffer.clone()),
            });
        }
        true
    }
}

impl Drop for AsyncReadResult {
    // Port of: src/gpu/AsyncReadTypes.h#L141-L145 (chrome/m156)
    fn drop(&mut self) {
        for plane in &mut self.planes {
            // `releaseMappedBuffer()`
            if let Some(buffer) = plane.mapped_buffer.take() {
                self.sender
                    .post(BufferFinishedMessage::new(buffer, self.intended_recipient));
            }
        }
    }
}

/// `SkImage::ReadPixelsCallback`: called with the pixels, or `None` if the read failed.
pub type ReadPixelsCallback = Box<dyn FnOnce(Option<AsyncReadResult>) + Send>;

/// `Context::AsyncParams<TextureProxyView>`.
// Port of: src/gpu/graphite/Context.cpp#L281-L324 (chrome/m156)
#[doc(alias = "AsyncParams")]
pub struct AsyncReadParams {
    /// `fSrcImage`.
    pub src: TextureProxyView,
    /// The color info of the source pixels (`fSrcImage->imageInfo().colorInfo()` of the image the
    /// view comes from).
    pub src_color_info: ColorInfo,
    /// `fSrcRect`.
    pub src_rect: IRect,
    /// `fDstImageInfo`.
    pub dst_image_info: ImageInfo,
    /// `fCallback` and `fCallbackContext`.
    pub callback: ReadPixelsCallback,
}

impl std::fmt::Debug for AsyncReadParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncReadParams")
            .field("src_rect", &self.src_rect)
            .field("dst_image_info", &self.dst_image_info)
            .finish_non_exhaustive()
    }
}

impl AsyncReadParams {
    /// `fail()`: calls the callback with no result.
    pub fn fail(self) {
        (self.callback)(None);
    }

    /// `validate()`.
    #[must_use]
    pub fn validate(&self) -> bool {
        if !self.src.is_valid() {
            return false;
        }
        if self.src.is_protected() == crate::gpu::gpu_types::Protected::Yes {
            return false;
        }
        if !IRect::from_size(self.src.dimensions()).contains(&self.src_rect) {
            return false;
        }
        if !skia_rust_core::image_info_priv::image_info_is_valid(&self.dst_image_info) {
            return false;
        }
        true
    }
}

/// The client mapped buffer manager, shared with the finished procs of the reads.
pub(crate) type SharedClientMappedBufferManager = Arc<Mutex<ClientMappedBufferManager>>;

/// Locks the manager.
pub(crate) fn lock_manager(
    manager: &SharedClientMappedBufferManager,
) -> std::sync::MutexGuard<'_, ClientMappedBufferManager> {
    manager.lock().unwrap_or_else(PoisonError::into_inner)
}
