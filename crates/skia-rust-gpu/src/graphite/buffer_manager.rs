// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/BufferManager.h, src/gpu/graphite/BufferManager.cpp

//! `BufferSubAllocator`, `DrawBufferManager` and `StaticBufferManager`: suballocation of GPU
//! buffer ranges for dynamic (per-`Recording`) and static (per-context) data.
//!
//! # Differences from the C++ ownership
//!
//! - Mapped memory is the buffer's CPU staging block, owned by the allocator that holds the
//!   buffer (see [`crate::graphite::buffer`]); the block is committed to the GPU buffer with
//!   [`Buffer::unmap_with`](crate::graphite::buffer::Buffer::unmap_with) when the buffer is
//!   transferred to a `Recording`. Writers therefore borrow the allocator they come from, so
//!   `getMapped*Buffer()` returns a [`MappedAllocationInfo`] whose
//!   [`writer`](MappedAllocationInfo::writer) writes the first range, instead of a
//!   `std::tuple<BufferWriter, BindBufferInfo, BufferSubAllocator>`.
//! - When the draw buffers cannot be mapped (`!caps.drawBufferCanBeMapped()`), the CPU data of a
//!   draw buffer goes through a transfer buffer range owned by the `UploadBufferManager`. The
//!   allocator stages the bytes itself and `transferToRecording()` copies them into the transfer
//!   buffer's staging block, where C++ wrote through a pointer into the transfer buffer.
//! - A `BufferSubAllocator` returns its buffer to its manager when it is reset or dropped. The
//!   manager's state lives in an `Rc<RefCell<..>>` that allocators reach through a `Weak`, so no
//!   lifetime appears on the public types. An allocator that outlives its manager just drops its
//!   buffer.
//! - `StaticBufferManager` gets the late-assigned `BindBufferInfo*` as a shared
//!   [`StaticBufferBinding`], and its `finalize()` reaches the `Context`, `QueueManager` and
//!   `GlobalCache` through [`StaticBufferHost`] (those are ported with G9b).

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};

use skia_rust_core::align::{align_non_pow2, align_to};

use crate::gpu::buffer_writer::{BufferWriter, VertexWriter};
use crate::gpu::gpu_types::Protected;
use crate::gpu::sk_log::skia_log_e;
use crate::graphite::buffer::{BindBufferInfo, Buffer, MappedData};
use crate::graphite::caps::Caps;
use crate::graphite::context_priv::SharedResourceProvider;
use crate::graphite::recording::Recording;
use crate::graphite::resource::{Resource, ResourceRef};
use crate::graphite::resource_cache::ScratchResourceSet;
use crate::graphite::resource_types::{AccessPattern, BufferType, ClearBuffer, Shareable};
use crate::graphite::task::TaskRef;
use crate::graphite::task::clear_buffers_task::ClearBuffersTask;
use crate::graphite::task::copy_task::CopyBufferToBufferTask;
use crate::graphite::upload_buffer_manager::UploadBufferManager;

// The limit for all data created by the StaticBufferManager. This data remains alive for
// the entire SharedContext so we want to keep it small and give a concrete upper bound to
// clients for our steady-state memory usage.
// FIXME The current usage is 6308 bytes across static vertex and index buffers, but that includes
// multiple copies of tessellation data, and an unoptimized AnalyticRRect mesh. Once those issues
// are addressed, we can tighten this and decide on the transfer buffer sizing as well.
// Port of: src/gpu/graphite/BufferManager.cpp#L40 (chrome/m156)
const MAX_STATIC_DATA_SIZE: u32 = 7 << 10;

fn to_u32(value: usize) -> u32 {
    u32::try_from(value).expect("SkTo<uint32_t>: value fits in 32 bits")
}

// Helpers for creating a BufferState based on type, options, and caps

// Port of: src/gpu/graphite/BufferManager.cpp#L43-L56 (chrome/m156)
fn get_gpu_access_pattern(
    is_gpu_only_access: bool,
    opts: &DrawBufferManagerOptions,
) -> AccessPattern {
    if is_gpu_only_access {
        if opts.allow_copying_gpu_only {
            return AccessPattern::GpuOnlyCopySrc;
        }
        AccessPattern::GpuOnly
    } else {
        AccessPattern::HostVisible
    }
}

// This returns the minimum required alignment depending on the type of buffer. This is guaranteed
// to be a power of two.
// Port of: src/gpu/graphite/BufferManager.cpp#L58-L75 (chrome/m156)
fn minimum_alignment(ty: BufferType, use_transfer_buffers: bool, caps: &dyn Caps) -> u32 {
    let mut alignment = 4;
    if ty == BufferType::Uniform {
        alignment = to_u32(caps.required_uniform_buffer_alignment());
    } else if ty == BufferType::Storage
        || ty == BufferType::VertexStorage
        || ty == BufferType::IndexStorage
        || ty == BufferType::Indirect
    {
        alignment = to_u32(caps.required_storage_buffer_alignment());
    }

    if use_transfer_buffers {
        // Both alignment and the requiredTransferBufferAlignment must be powers of two, so max
        // provides the correct alignment semantics
        alignment = alignment.max(to_u32(caps.required_transfer_buffer_alignment()));
    }

    alignment
}

// Port of: src/gpu/graphite/BufferManager.cpp#L77-L95 (chrome/m156)
fn min_block_size(ty: BufferType, min_alignment: u32, opts: &DrawBufferManagerOptions) -> u32 {
    let size = if ty == BufferType::Index || ty == BufferType::IndexStorage {
        opts.index_buffer_size
    } else if ty == BufferType::Vertex || ty == BufferType::VertexStorage {
        opts.vertex_buffer_min_size
    } else {
        opts.storage_buffer_min_size
    };

    if opts.use_exact_buff_sizes {
        return size; // No extra alignment
    }
    align_to(size, min_alignment)
}

// Port of: src/gpu/graphite/BufferManager.cpp#L97-L121 (chrome/m156)
fn max_block_size(ty: BufferType, min_alignment: u32, opts: &DrawBufferManagerOptions) -> u32 {
    if opts.use_exact_buff_sizes {
        // Clamp to the minimum size
        return min_block_size(ty, min_alignment, opts);
    }

    let size = if ty == BufferType::Index || ty == BufferType::IndexStorage {
        opts.index_buffer_size
    } else if ty == BufferType::Vertex || ty == BufferType::VertexStorage {
        opts.vertex_buffer_max_size
    } else {
        opts.storage_buffer_max_size
    };
    align_to(size, min_alignment)
}

/// `BufferAligner`: helper functions for buffer sub-allocation and alignment math.
// Port of: src/gpu/graphite/BufferManager.h#L336-L388 (chrome/m156)
pub mod buffer_aligner {
    /// `BufferAligner::ValidateCountAndStride`: the byte count of `count` blocks of `stride`
    /// (at least `headroom`), or 0 if it, or its alignment, would not fit in 32 bits.
    #[doc(alias = "ValidateCountAndStride")]
    #[must_use]
    pub fn validate_count_and_stride(
        count: usize,
        stride: usize,
        headroom: usize,
        alignment: u32,
    ) -> u32 {
        // size_t may just be uint32_t, so this ensures we have enough bits to
        // compute the required byte product.
        let count64 = count as u64;
        let stride64 = stride as u64;
        let bytes64 = count64.wrapping_mul(stride64);
        let headroom64 = headroom as u64;
        let bytes_with_headroom64 = headroom64.max(bytes64);
        let max32 = u64::from(u32::MAX);
        if count64 > max32
            || stride64 > max32
            || bytes64 > max32
            || headroom64 > max32
            || bytes_with_headroom64 > max32 - (u64::from(alignment) + 1)
        {
            // Return 0 to skip further allocation attempts.
            return 0;
        }

        // Since count64 and stride64 fit into 32-bits, their product won't overflow a 64-bit
        // multiply, and we've confirmed product fits into 32-bits with head room to be aligned
        // w/o overflow.
        u32::try_from(bytes_with_headroom64).unwrap_or(0)
    }

    fn is_pow2(value: u32) -> bool {
        value.is_power_of_two()
    }

    fn gcd(mut a: u32, mut b: u32) -> u32 {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    }

    /// `BufferAligner::LcmAlignment`.
    ///
    /// # Panics
    /// In debug builds if either alignment is 0.
    #[doc(alias = "LcmAlignment")]
    #[must_use]
    pub fn lcm_alignment(align_maybe_pow2: u32, align_prob_non_pow2: u32) -> u32 {
        debug_assert!(align_maybe_pow2 != 0 && align_prob_non_pow2 != 0);
        if align_maybe_pow2 == 1
            || align_maybe_pow2 == align_prob_non_pow2
            || (is_pow2(align_maybe_pow2)
                && align_prob_non_pow2 > align_maybe_pow2
                && (align_prob_non_pow2 & (align_maybe_pow2 - 1)) == 0)
        {
            // Trivial LCM since alignProbNonPow2 is the same or a larger multiple of
            // alignMaybePow2
            align_prob_non_pow2
        } else {
            // std::lcm: |m| / gcd(m, n) * |n| in uint32_t arithmetic.
            (align_maybe_pow2 / gcd(align_maybe_pow2, align_prob_non_pow2))
                .wrapping_mul(align_prob_non_pow2)
        }
    }
}

/// `DrawBufferManager::Options`.
// Port of: src/gpu/graphite/BufferManager.h#L186-L198 (chrome/m156)
#[doc(alias = "DrawBufferManager::Options")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawBufferManagerOptions {
    /// `fVertexBufferMinSize`: 16 KB.
    pub vertex_buffer_min_size: u32,
    /// `fVertexBufferMaxSize`: 1 MB.
    pub vertex_buffer_max_size: u32,
    /// `fIndexBufferSize`: 2 KB.
    pub index_buffer_size: u32,
    /// `fStorageBufferMinSize`: 2 KB.
    pub storage_buffer_min_size: u32,
    /// `fStorageBufferMaxSize`: 1 MB.
    pub storage_buffer_max_size: u32,
    /// `fUseExactBuffSizes` (`GPU_TEST_UTILS`): disables automatic buffer growth.
    pub use_exact_buff_sizes: bool,
    /// `fAllowCopyingGpuOnly` (`GPU_TEST_UTILS`): adds `kCopySrc` to GPU-only buffer usage.
    pub allow_copying_gpu_only: bool,
}

impl Default for DrawBufferManagerOptions {
    fn default() -> Self {
        Self {
            vertex_buffer_min_size: 16 << 10,
            vertex_buffer_max_size: 1 << 20,
            index_buffer_size: 2 << 10,
            storage_buffer_min_size: 2 << 10,
            storage_buffer_max_size: 1 << 20,
            use_exact_buff_sizes: false,
            allow_copying_gpu_only: false,
        }
    }
}

// The indices of DrawBufferManager::fCurrentBuffers.
// Port of: src/gpu/graphite/BufferManager.h#L283-L290 (chrome/m156)
const VERTEX_BUFFER_INDEX: usize = 0;
const INDEX_BUFFER_INDEX: usize = 1;
const UNIFORM_BUFFER_INDEX: usize = 2;
const STORAGE_BUFFER_INDEX: usize = 3;
const GPU_ONLY_STORAGE_BUFFER_INDEX: usize = 4;
const VERTEX_STORAGE_BUFFER_INDEX: usize = 5;
const INDEX_STORAGE_BUFFER_INDEX: usize = 6;
const INDIRECT_STORAGE_BUFFER_INDEX: usize = 7;

/// A buffer the `DrawBufferManager` has finished allocating from and that goes to the
/// `Recording`: the buffer, its transfer buffer range (if the CPU data goes through one) and
/// the CPU bytes written.
#[derive(Debug)]
struct UsedBuffer {
    buffer: ResourceRef<Buffer>,
    transfer_buffer: BindBufferInfo,
    mapped: Option<MappedData>,
}

/// `DrawBufferManager::BufferState`.
// Port of: src/gpu/graphite/BufferManager.h#L249-L270 (chrome/m156)
#[derive(Debug)]
struct BufferState {
    ty: BufferType,
    access_pattern: AccessPattern,
    use_transfer_buffer: bool,
    label: &'static str,
    // guaranteed power of two, required for binding
    min_alignment: u32,
    min_block_size: u32,
    max_block_size: u32,

    available_buffer: BufferSubAllocator,

    // Buffers held in this set are owned by still-alive BufferSubAllocators that were created
    // with Shareable::kScratch. This is compatible with ResourceCache::ScratchResourceSet.
    unavailable_scratch_buffers: ScratchResourceSet,

    // The size of the last allocated Buffer, pinned to min/max block size, for amortizing the
    // number of buffer allocations for large Recordings.
    last_buffer_size: u32,
}

impl BufferState {
    // Port of: src/gpu/graphite/BufferManager.cpp#L218-L238 (chrome/m156)
    fn new(
        ty: BufferType,
        label: &'static str,
        is_gpu_only: bool,
        opts: &DrawBufferManagerOptions,
        caps: &dyn Caps,
    ) -> Self {
        // The buffer can be GPU-only if
        //     a) the caller does not intend to ever upload CPU data to the buffer; or
        //     b) CPU data will get uploaded to fBuffer only via a transfer buffer
        let access_pattern =
            get_gpu_access_pattern(is_gpu_only || !caps.draw_buffer_can_be_mapped(), opts);
        let use_transfer_buffer = !is_gpu_only && !caps.draw_buffer_can_be_mapped();
        let min_alignment = minimum_alignment(ty, use_transfer_buffer, caps);
        let min_block_size = min_block_size(ty, min_alignment, opts);
        let max_block_size = max_block_size(ty, min_alignment, opts);
        debug_assert!(min_alignment.is_power_of_two());
        debug_assert!(min_block_size <= max_block_size);
        Self {
            ty,
            access_pattern,
            use_transfer_buffer,
            label,
            min_alignment,
            min_block_size,
            max_block_size,
            available_buffer: BufferSubAllocator::default(),
            unavailable_scratch_buffers: ScratchResourceSet::new(),
            last_buffer_size: 0,
        }
    }

    // Port of: src/gpu/graphite/BufferManager.cpp#L240-L254 (chrome/m156)
    fn find_or_create_buffer(
        &mut self,
        provider: &SharedResourceProvider,
        shareable: Shareable,
        byte_count: u32,
    ) -> Option<ResourceRef<Buffer>> {
        let mut provider = provider
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if shareable == Shareable::Scratch {
            let scratch_buffer = provider.find_or_create_scratch_buffer(
                byte_count as usize,
                self.ty,
                self.access_pattern,
                self.label,
                &self.unavailable_scratch_buffers,
            );
            if let Some(scratch_buffer) = &scratch_buffer {
                self.unavailable_scratch_buffers
                    .insert(scratch_buffer.base().unique_id());
            }
            scratch_buffer
        } else {
            provider.find_or_create_non_shareable_buffer(
                byte_count as usize,
                self.ty,
                self.access_pattern,
                self.label,
            )
        }
    }
}

/// The state of a `DrawBufferManager` that its allocators reach back into.
#[derive(Debug)]
struct DrawBufferManagerState {
    current_buffers: [BufferState; 8],

    // Vector of buffers, their transfer buffer ranges and CPU bytes.
    used_buffers: Vec<UsedBuffer>,

    // List of buffer regions that were requested to be cleared at the time of allocation.
    clear_list: Vec<BindBufferInfo>,

    // If mapping failed on Buffers created/managed by this DrawBufferManager or by the mapped
    // transfer buffers from the UploadManager, remember so that the next Recording will fail.
    mapping_failed: bool,
}

/// `BufferSubAllocator` provides an entire GPU buffer to the caller so that the caller can sub
/// allocate intervals within the buffer. Each buffer type has a minimum required alignment for
/// binding. This alignment is automatically used for the *first* suballocation from an allocator
/// instance. Scoping the lifetime of an allocator to when the contents are bound allows these
/// binding requirements to automatically be met and use a tighter alignment for additional
/// suballocations that can be accessed without requiring a new binding.
///
/// When a `BufferSubAllocator` goes out of scope, its underlying `Buffer` is returned to the
/// manager. By default, any remaining space can be returned by subsequent allocation requests
/// but written bytes will not be able to be overwritten by later `BufferSubAllocator`s. The
/// exception is with the `BufferSubAllocator` instances returned by
/// `DrawBufferManager::get_scratch_storage()`, whose Buffers will be `Shareable::Scratch`
/// resources, and can be fully reused by other Recorders or once the `BufferSubAllocator` goes
/// out of scope.
///
/// Buffers created by the `DrawBufferManager` for an allocator are automatically transferred to
/// the `Recording` and `CommandBuffer`s when snapped or inserted.
// Port of: src/gpu/graphite/BufferManager.h#L47-L180 (chrome/m156)
#[doc(alias = "skgpu::graphite::BufferSubAllocator")]
#[derive(Debug)]
pub struct BufferSubAllocator {
    // Non-null when valid and not already returned to the pool
    owner: Weak<RefCell<DrawBufferManagerState>>,
    state_index: usize,
    // The minimum alignment of the first suballocation after resetForNewBinding(): the owner's
    // `fCurrentBuffers[fStateIndex].fMinAlignment`.
    min_binding_alignment: u32,
    buffer: Option<ResourceRef<Buffer>>,
    transfer_buffer: BindBufferInfo,
    // If mapped for writing, this is the CPU staging block for `buffer` (the C++ address of
    // offset 0 of the buffer or its transfer buffer). When a mapped buffer is returned to the
    // DrawBufferManager, only the bytes after fOffset can be reused. If there is no mapped
    // buffer, it's assumed the GPU buffer is reusable for another BufferSubAllocator instance
    // (this default reuse policy can be revisited if needed).
    mapped: Option<MappedData>,
    offset: u32,    // Next suballocation can start at fOffset at the earliest
    stride: u32,    // The byte count of blocks for the optimized append() function
    remaining: u32, // The number of `fStride` contiguous blocks fitting after fOffset
}

impl Default for BufferSubAllocator {
    // Port of: src/gpu/graphite/BufferManager.h#L66 (chrome/m156)
    fn default() -> Self {
        Self {
            owner: Weak::new(),
            state_index: 0,
            min_binding_alignment: 1,
            buffer: None,
            transfer_buffer: BindBufferInfo::default(),
            mapped: None,
            offset: 0,
            stride: 1,
            remaining: 0,
        }
    }
}

impl BufferSubAllocator {
    // Port of: src/gpu/graphite/BufferManager.cpp#L126-L148 (chrome/m156)
    fn new(
        owner: Weak<RefCell<DrawBufferManagerState>>,
        state_index: usize,
        min_binding_alignment: u32,
        buffer: ResourceRef<Buffer>,
        transfer_buffer: BindBufferInfo,
        mapped: Option<MappedData>,
        stride: usize,
    ) -> Self {
        let stride = to_u32(stride);
        // A starting offset of 0 means we don't need to explicitly lookup the minimum binding
        // alignment since the first sub-allocation is automatically aligned.
        // This puts the BufferSubAllocator in the same state as prepForStride(stride, *)
        let remaining = if stride > 0 && stride as usize <= buffer.size() {
            to_u32(buffer.size()) / stride
        } else {
            0
        };
        debug_assert!(stride != 0);
        Self {
            owner,
            state_index,
            min_binding_alignment,
            buffer: Some(buffer),
            transfer_buffer,
            mapped,
            offset: 0,
            stride,
            remaining,
        }
    }

    /// `isValid()`: false if the underlying buffer has been returned to the reuse pool or moved.
    #[doc(alias = "isValid")]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.buffer.is_some()
    }

    /// Returns the underlying buffer object back to the pool and invalidates this allocator.
    /// Depending on the GPU buffer's `Shareable` value, either:
    ///  - `No`: The remaining space that hasn't been written to can be used by another
    ///    allocator, but it will assume that use will involve a new buffer binding command.
    ///  - `Scratch`: The entire buffer can be overwritten by another allocator.
    ///
    /// # Panics
    /// If the owning manager is in the middle of one of its own operations (an allocator must
    /// not be reset from inside the manager).
    // Port of: src/gpu/graphite/BufferManager.cpp#L205-L255 (chrome/m156)
    pub fn reset(&mut self) {
        if self.buffer.is_none() {
            return; // nothing to reset
        }
        let Some(owner) = self.owner.upgrade() else {
            // The manager is gone, so there is nowhere to return the buffer to.
            self.clear();
            return;
        };
        let mut state = owner.borrow_mut();
        self.return_to_state(&mut state, false);
    }

    // Empties this allocator without returning anything to the manager.
    fn clear(&mut self) {
        self.buffer = None;
        self.mapped = None;
        self.remaining = 0;
        self.transfer_buffer = BindBufferInfo::default();
    }

    // The body of reset() given the owner's state. `is_available_slot` is true if `self` was
    // taken out of `state.fCurrentBuffers[fStateIndex].fAvailableBuffer`, standing in for the
    // "can't stash itself" comparison of the C++ (where `this` is that member).
    // Port of: src/gpu/graphite/BufferManager.cpp#L205-L255 (chrome/m156)
    fn return_to_state(&mut self, owner: &mut DrawBufferManagerState, is_available_slot: bool) {
        let Some(buffer) = &self.buffer else {
            return; // nothing to reset
        };

        if owner.mapping_failed {
            self.clear();
            return;
        }

        let state_index = self.state_index;
        let remaining_bytes = self.remaining_bytes();
        let state = &mut owner.current_buffers[state_index];
        if buffer.base().shareable() == Shareable::Scratch {
            // TODO: Merge this reuse of scratch resources with the ScratchResourceManager, but
            // currently this is resolved outside of Task::prepareResources().
            // The scratch buffer's availability for reuse (scoped to the owning
            // DrawBufferManager) was tied to this BufferSubAllocator, so when that is reset, we
            // just remove the buffer from the set of unavailable buffers.
            let removed = state
                .unavailable_scratch_buffers
                .remove(&buffer.base().unique_id());
            debug_assert!(removed);
            // Scratch buffers shouldn't be using transfer buffers
            debug_assert!(!self.transfer_buffer.is_valid());
            let buffer = self.buffer.take().expect("checked above");
            owner.used_buffers.push(UsedBuffer {
                buffer,
                transfer_buffer: BindBufferInfo::default(),
                mapped: self.mapped.take(),
            });
        } else if is_available_slot // can't stash itself
            || state
                .available_buffer
                .buffer
                .as_ref()
                .is_some_and(|available| ResourceRef::ptr_eq(available, buffer))
            || remaining_bytes < state.available_buffer.remaining_bytes() // too small
            || remaining_bytes < state.min_alignment
        // basically empty
        {
            // Transfer ownership of the buffer (and any transfer buffer) back to the manager,
            // using the current offset as a more restricted limit for copying.
            if self.transfer_buffer.is_valid() {
                // This alignment ensures we are copying a subset that still respects xfer
                // alignment
                self.transfer_buffer.size = align_to(self.offset, state.min_alignment);
            }
            let buffer = self.buffer.take().expect("checked above");
            owner.used_buffers.push(UsedBuffer {
                buffer,
                transfer_buffer: std::mem::take(&mut self.transfer_buffer),
                mapped: self.mapped.take(),
            });
        } else {
            // Save this buffer for later, which leaves this instance empty and resets the prior
            // value of fAvailableBuffer (which then goes through the true branch of this if).
            let mut previous = std::mem::take(&mut state.available_buffer);
            previous.return_to_state(owner, true);
            owner.current_buffers[state_index].available_buffer = std::mem::take(self);
        }

        self.remaining = 0;
        debug_assert!(self.buffer.is_none());
    }

    /// `resetForNewBinding()`: causes the next call to `get[Mapped]Subrange()` to also align
    /// with the minimum binding requirement for the buffer's type. It resets any remaining
    /// strided blocks following the last suballocation or appended range.
    #[doc(alias = "resetForNewBinding")]
    pub fn reset_for_new_binding(&mut self) {
        self.remaining = 0;
        self.stride = 0; // signals prepForStride() to factor in minimum binding alignment next time.
    }

    /// `remainingBytes()`: the number of remaining bytes in the GPU buffer, assuming an
    /// alignment of 1.
    #[doc(alias = "remainingBytes")]
    #[must_use]
    pub fn remaining_bytes(&self) -> u32 {
        self.buffer
            .as_ref()
            .map_or(0, |buffer| to_u32(buffer.size()) - self.offset)
    }

    /// `availableWithStride()`: the number of `stride` blocks left in the buffer, where
    /// `stride` was the last value passed into `get[Mapped]Subrange()`.
    #[doc(alias = "availableWithStride")]
    #[must_use]
    pub fn available_with_stride(&self) -> u32 {
        self.remaining
    }

    // If minCount*stride bytes fit when aligned to the LCM of stride, align, and possibly the
    // binding alignment (when fStride == 0), then fOffset is updated and fRemaining is set to the
    // number of stride units that fit in the rest of the buffer after fOffset. If not, fRemaining
    // is set to 0 and fOffset is unmodified.
    // Port of: src/gpu/graphite/BufferManager.cpp#L165-L203 (chrome/m156)
    fn prep_for_stride(&mut self, stride: usize, align: usize, min_count: usize, headroom: usize) {
        debug_assert!(stride > 0 && align > 0); // Expect valid inputs

        if let Some(buffer) = &self.buffer {
            if self.stride as usize == stride && (align == 1 || align == stride) {
                // Shortcut if we're already aligned with the last call to prepForStride().
                // Leave fRemaining alone, it's either enough for minCount or not, but reserve()
                // will do the right thing regardless.
                debug_assert!((self.offset as usize).is_multiple_of(align));
                debug_assert!((self.offset as usize).is_multiple_of(stride));
                return;
            }

            // On re-aligning to a new stride, the offset needs to be aligned to the LCM of
            // `align` and `stride` so that repeated suballocations of `stride` can be performed
            // by simply adding to fOffset without additional instructions. If `fStride == 0`,
            // it's a signal that the first offset also needs to be aligned to the minimum
            // binding requirement.
            let mut align32 = buffer_aligner::lcm_alignment(to_u32(align), to_u32(stride));
            if self.stride == 0 {
                align32 = buffer_aligner::lcm_alignment(self.min_binding_alignment, align32);
            }

            let stride32 = to_u32(stride);
            let headroom32 = to_u32(headroom);
            let reserve_for_headroom = headroom32.saturating_sub(stride32);
            let buffer_size = to_u32(buffer.size());

            // Ensures we won't overflow fOffset past buffer size once we align it
            if self.remaining_bytes() >= align32 - 1 + reserve_for_headroom {
                let offset = align_non_pow2(self.offset, align32);
                debug_assert!(offset + reserve_for_headroom <= buffer_size);
                self.stride = stride32;
                self.remaining = (buffer_size - offset - reserve_for_headroom) / self.stride;
                if self.remaining > 0 && self.remaining as usize >= min_count {
                    // Successful prep, so preserve the aligned offset
                    self.offset = offset;
                    return;
                }
            }
        }

        // If we've reached here, there wasn't a buffer or enough room to align, or enough room
        // to satisfy minCount, so set fRemaining=0 to fail subsequent reservations.
        self.remaining = 0;
    }

    // Returns the (offset, size) of a `count`-stride range, or None if it does not fit.
    // Port of: src/gpu/graphite/BufferManager.h#L205-L220 (chrome/m156)
    fn reserve(&mut self, count: usize) -> Option<(u32, u32)> {
        if self.buffer.is_none() || count > self.remaining as usize {
            return None;
        }

        debug_assert!(self.stride > 0); // fRemaining should be set to 0 if fStride is 0

        // fRemaining and fStride were already validated in prepForStride() so we can cast
        // `count`
        let required_bytes32 = to_u32(count) * self.stride;
        let offset = self.offset;
        self.offset += required_bytes32;
        self.remaining -= to_u32(count);
        Some((offset, required_bytes32))
    }

    // Port of: src/gpu/graphite/BufferManager.h#L222-L226 (chrome/m156)
    fn binding(&self, offset: u32, size: u32) -> BindBufferInfo {
        let buffer = self.buffer.as_ref().expect("allocator has a buffer");
        debug_assert!(self.offset >= size && self.offset - size >= offset);
        BindBufferInfo::new(buffer, offset, size)
    }

    // Port of: src/gpu/graphite/BufferManager.h#L227-L232 (chrome/m156)
    fn writer(&mut self, offset: u32, size: u32) -> BufferWriter<'_> {
        debug_assert!(self.buffer.is_some());
        debug_assert!(self.offset >= size && self.offset - size >= offset);
        let mapped = self.mapped.as_mut().expect("allocator is mapped");
        BufferWriter::new(&mut mapped[offset as usize..(offset + size) as usize])
    }

    /// Suballocate `count*stride` bytes and a writer for the mapped range and the
    /// `BindBufferInfo` defining that range in a GPU-backed `Buffer`. The returned subrange
    /// will be aligned according to the following rules:
    ///  - The first suballocation, or the first after `reset_for_new_binding()`, will be aligned
    ///    to the lowest common multiple of `stride`, the binding's required alignment, and any
    ///    extra base alignment passed in as `align`.
    ///  - Subsequent suballocations will be aligned to just `stride` and any extra base
    ///    alignment.
    ///
    /// It is assumed the caller will write all `count*stride` bytes to the returned writer.
    ///
    /// It is acceptable to pass `count=0` to this function, which will still succeed if there is
    /// space to align at least one more aligned `stride` block. In this case, the writer will be
    /// empty but the `BindBufferInfo` will have a valid Buffer and byte offset (just a zero
    /// size). This then represents the start of the binding range for future calls to
    /// `append_mapped_with_stride()`.
    ///
    /// `None` is returned if the buffer does not have enough room remaining to fulfill the
    /// suballocation in this buffer.
    // Port of: src/gpu/graphite/BufferManager.h#L112-L122 (chrome/m156)
    #[doc(alias = "getMappedSubrange")]
    pub fn get_mapped_subrange(
        &mut self,
        count: usize,
        stride: usize,
        align: usize,
    ) -> Option<(BufferWriter<'_>, BindBufferInfo)> {
        // Writing should have checked validity of allocator first
        debug_assert!(self.mapped.is_some() || self.buffer.is_none());
        self.prep_for_stride(stride, align, count, 0);
        let (offset, size) = self.reserve(count)?;
        let binding = self.binding(offset, size);
        Some((self.writer(offset, size), binding))
    }

    /// Similar to `get_mapped_subrange` above but count and align = 1.
    ///
    /// Additionally it adds another parameter `headroom`. Headroom is used to make sure there
    /// are at least headroom bytes from the beginning of the returned subrange to the end of the
    /// buffer. This is useful for when you want the bind buffer size to be larger than the
    /// actual data size of stride. This call will still return a `BindBufferInfo` that has a
    /// size of stride, so the caller will need to manually adjust the binding size if they want
    /// a larger value.
    // Port of: src/gpu/graphite/BufferManager.h#L124-L132 (chrome/m156)
    #[doc(alias = "getMappedSubrangeWithHeadroom")]
    pub fn get_mapped_subrange_with_headroom(
        &mut self,
        stride: usize,
        headroom: usize,
    ) -> Option<(BufferWriter<'_>, BindBufferInfo)> {
        // Writing should have checked validity of allocator first
        debug_assert!(self.mapped.is_some() || self.buffer.is_none());
        self.prep_for_stride(stride, 1, 1, headroom);
        let (offset, size) = self.reserve(1)?;
        let binding = self.binding(offset, size);
        Some((self.writer(offset, size), binding))
    }

    /// Sub-allocate a slice within the scratch buffer object. This variation should be used when
    /// the returned range will be written to by the GPU as part of executing a command buffer.
    ///
    /// Other than returning just a buffer slice to be written to later by a GPU task, the
    /// suballocation behaves identically to `get_mapped_subrange()`. The binding is empty if
    /// the buffer does not have enough room.
    // Port of: src/gpu/graphite/BufferManager.h#L134-L142 (chrome/m156)
    #[doc(alias = "getSubrange")]
    pub fn get_subrange(&mut self, count: usize, stride: usize, align: usize) -> BindBufferInfo {
        debug_assert!(self.mapped.is_none()); // Should not be used when data is intended to be written by CPU
        self.prep_for_stride(stride, align, count, 0);
        match self.reserve(count) {
            Some((offset, size)) => self.binding(offset, size),
            None => BindBufferInfo::default(),
        }
    }

    /// Returns a writer for `count*stride` bytes, where `stride` was the last value passed to
    /// `get_mapped_subrange`.
    ///
    /// If `count <= self.available_with_stride()`, then the appended range will be contiguous
    /// with the `BindBufferInfo` returned from the last call to `get_mapped_subrange` (and/or
    /// other calls to `append_mapped_with_stride`). The binding's size should be increased by
    /// count*stride, which is the caller's responsibility.
    ///
    /// Otherwise, `None` will be returned and no change is made to the allocator.
    // Port of: src/gpu/graphite/BufferManager.h#L152-L156 (chrome/m156)
    #[doc(alias = "appendMappedWithStride")]
    pub fn append_mapped_with_stride(&mut self, count: usize) -> Option<BufferWriter<'_>> {
        debug_assert!(count > 0 && (self.mapped.is_some() || self.remaining == 0));
        let (offset, size) = self.reserve(count)?;
        Some(self.writer(offset, size))
    }
}

impl Drop for BufferSubAllocator {
    // Port of: src/gpu/graphite/BufferManager.h#L69 (chrome/m156)
    fn drop(&mut self) {
        self.reset();
    }
}

/// What `DrawBufferManager::getMapped*Buffer()` returns: the binding of the first mapped
/// subrange, the allocator for the whole GPU buffer (to get more mapped suballocations, which
/// when successful are guaranteed to be in the same buffer), and access to the writer for that
/// first subrange.
// Port of: src/gpu/graphite/BufferManager.h#L215-L218 (chrome/m156)
#[doc(alias = "DrawBufferManager::MappedAllocationInfo")]
#[derive(Debug)]
pub struct MappedAllocationInfo {
    /// The `BindBufferInfo` of the first subrange.
    pub binding: BindBufferInfo,
    /// The allocator for the whole GPU buffer.
    pub allocator: BufferSubAllocator,
    first_range: (u32, u32),
}

impl MappedAllocationInfo {
    /// The `BufferWriter` for the first subrange (the one `binding` names).
    pub fn writer(&mut self) -> BufferWriter<'_> {
        let (offset, size) = self.first_range;
        self.allocator.writer(offset, size)
    }
}

/// `DrawBufferManager` controls writing to buffer data ranges within larger, cacheable Buffers
/// and automatically handles either mapping or copying via transfer buffer depending on what the
/// GPU hardware supports for the requested buffer type and use case. It is intended for
/// repeatedly uploading dynamic data to the GPU.
// Port of: src/gpu/graphite/BufferManager.h#L182-L300 (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawBufferManager")]
pub struct DrawBufferManager {
    resource_provider: SharedResourceProvider,
    caps: Arc<dyn Caps>,
    upload_manager: Rc<RefCell<UploadBufferManager>>,
    state: Rc<RefCell<DrawBufferManagerState>>,
}

impl std::fmt::Debug for DrawBufferManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DrawBufferManager")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl DrawBufferManager {
    /// `DrawBufferManager(resourceProvider, caps, uploadManager, dbmOpts)`.
    // Port of: src/gpu/graphite/BufferManager.cpp#L257-L284 (chrome/m156)
    #[must_use]
    pub fn new(
        resource_provider: SharedResourceProvider,
        caps: Arc<dyn Caps>,
        upload_manager: Rc<RefCell<UploadBufferManager>>,
        dbm_opts: &DrawBufferManagerOptions,
    ) -> Self {
        let c = &*caps;
        let current_buffers = [
            // Mappable buffers
            BufferState::new(BufferType::Vertex, "VertexBuffer", false, dbm_opts, c),
            BufferState::new(BufferType::Index, "IndexBuffer", false, dbm_opts, c),
            BufferState::new(BufferType::Uniform, "UniformBuffer", false, dbm_opts, c),
            BufferState::new(BufferType::Storage, "StorageBuffer", false, dbm_opts, c),
            // GPU-only buffers
            BufferState::new(
                BufferType::Storage,
                "GPUOnlyStorageBuffer",
                true,
                dbm_opts,
                c,
            ),
            BufferState::new(
                BufferType::VertexStorage,
                "VertexStorageBuffer",
                true,
                dbm_opts,
                c,
            ),
            BufferState::new(
                BufferType::IndexStorage,
                "IndexStorageBuffer",
                true,
                dbm_opts,
                c,
            ),
            BufferState::new(
                BufferType::Indirect,
                "IndirectStorageBuffer",
                true,
                dbm_opts,
                c,
            ),
        ];
        Self {
            resource_provider,
            caps,
            upload_manager,
            state: Rc::new(RefCell::new(DrawBufferManagerState {
                current_buffers,
                used_buffers: Vec::new(),
                clear_list: Vec::new(),
                mapping_failed: false,
            })),
        }
    }

    /// `hasMappingFailed()`: lets possible users check if the manager is already in a bad
    /// mapping state and skip any extra work that will be wasted because the next Recording
    /// snap will fail.
    #[doc(alias = "hasMappingFailed")]
    #[must_use]
    pub fn has_mapping_failed(&self) -> bool {
        self.state.borrow().mapping_failed
    }

    /// `getMappedVertexBuffer()`: a writer for the `count*stride` bytes of the GPU buffer
    /// subrange represented by the returned binding. The returned `BufferSubAllocator`
    /// represents the entire GPU buffer that the mapped subrange belongs to; it can be used to
    /// get additional mapped suballocations, which when successful are guaranteed to be in the
    /// same buffer. This allows callers to more easily manage when buffers must be bound.
    ///
    /// The returned writer and binding are effectively an automatic call to
    /// `BufferSubAllocator::get_mapped_subrange(count, stride)`. The offset of this first
    /// allocation will be aligned to the LCM of `stride` and the minimum required alignment for
    /// the buffer type. For function variants that take an extra `alignment`, the initial
    /// suballocation will also be aligned to that. Subsequent suballocations from the returned
    /// allocator will only be aligned to their requested stride unless
    /// `reset_for_new_binding()` was called.
    ///
    /// When the returned `BufferSubAllocator` goes out of scope, any remaining bytes that were
    /// never returned from either this function or later calls to `get_mapped_subrange()` can
    /// be used to satisfy a future call to `get_mapped_*_buffer`.
    // Port of: src/gpu/graphite/BufferManager.h#L220-L224 (chrome/m156)
    #[doc(alias = "getMappedVertexBuffer")]
    #[must_use]
    pub fn get_mapped_vertex_buffer(
        &self,
        count: usize,
        stride: usize,
        reserved_count: usize,
        alignment: usize,
    ) -> Option<MappedAllocationInfo> {
        self.get_mapped_buffer(
            VERTEX_BUFFER_INDEX,
            count,
            stride,
            reserved_count,
            alignment,
        )
    }

    /// `getMappedIndexBuffer()`.
    // Port of: src/gpu/graphite/BufferManager.h#L225-L227 (chrome/m156)
    #[doc(alias = "getMappedIndexBuffer")]
    #[must_use]
    pub fn get_mapped_index_buffer(&self, count: usize) -> Option<MappedAllocationInfo> {
        self.get_mapped_buffer(INDEX_BUFFER_INDEX, count, std::mem::size_of::<u16>(), 0, 1)
    }

    /// `getMappedUniformBuffer()`.
    // Port of: src/gpu/graphite/BufferManager.h#L228-L240 (chrome/m156)
    #[doc(alias = "getMappedUniformBuffer")]
    #[must_use]
    pub fn get_mapped_uniform_buffer(
        &self,
        stride: usize,
        headroom: usize,
    ) -> Option<MappedAllocationInfo> {
        let mut buffer = self.get_buffer(
            UNIFORM_BUFFER_INDEX,
            /*count=*/ 1,
            stride,
            /*xtra_alignment=*/ 1,
            headroom,
            ClearBuffer::No,
            Shareable::No,
        );
        let (binding, first_range) = {
            let (_, binding) = buffer.get_mapped_subrange_with_headroom(stride, headroom)?;
            (binding.clone(), (binding.offset, binding.size))
        };
        Some(MappedAllocationInfo {
            binding,
            allocator: buffer,
            first_range,
        })
    }

    /// `getMappedStorageBuffer()`.
    // Port of: src/gpu/graphite/BufferManager.h#L241-L243 (chrome/m156)
    #[doc(alias = "getMappedStorageBuffer")]
    #[must_use]
    pub fn get_mapped_storage_buffer(
        &self,
        count: usize,
        stride: usize,
    ) -> Option<MappedAllocationInfo> {
        self.get_mapped_buffer(STORAGE_BUFFER_INDEX, count, stride, 0, 1)
    }

    // The remaining writers and buffer allocator functions assume that byte counts are safely
    // calculated by the caller (e.g. Vello).

    // Utilities that return an unmapped buffer suballocation for a particular usage. These
    // buffers are intended to be only accessed by the GPU and are not intended for CPU data
    // uploads.

    /// `getStorage()`.
    // Port of: src/gpu/graphite/BufferManager.h#L249-L251 (chrome/m156)
    #[doc(alias = "getStorage")]
    #[must_use]
    pub fn get_storage(&self, required_bytes: usize, cleared: ClearBuffer) -> BindBufferInfo {
        self.get_binding(GPU_ONLY_STORAGE_BUFFER_INDEX, required_bytes, cleared)
    }

    /// `getVertexStorage()`.
    // Port of: src/gpu/graphite/BufferManager.h#L252-L254 (chrome/m156)
    #[doc(alias = "getVertexStorage")]
    #[must_use]
    pub fn get_vertex_storage(&self, required_bytes: usize) -> BindBufferInfo {
        self.get_binding(VERTEX_STORAGE_BUFFER_INDEX, required_bytes, ClearBuffer::No)
    }

    /// `getIndexStorage()`.
    // Port of: src/gpu/graphite/BufferManager.h#L255-L257 (chrome/m156)
    #[doc(alias = "getIndexStorage")]
    #[must_use]
    pub fn get_index_storage(&self, required_bytes: usize) -> BindBufferInfo {
        self.get_binding(INDEX_STORAGE_BUFFER_INDEX, required_bytes, ClearBuffer::No)
    }

    /// `getIndirectStorage()`.
    // Port of: src/gpu/graphite/BufferManager.h#L258-L260 (chrome/m156)
    #[doc(alias = "getIndirectStorage")]
    #[must_use]
    pub fn get_indirect_storage(
        &self,
        required_bytes: usize,
        cleared: ClearBuffer,
    ) -> BindBufferInfo {
        self.get_binding(INDIRECT_STORAGE_BUFFER_INDEX, required_bytes, cleared)
    }

    /// `getScratchStorage()`: an entire storage buffer object that is large enough to fit
    /// `required_bytes`. The returned `BufferSubAllocator` can be used to sub-allocate one or
    /// more storage buffer bindings that reference the same buffer object.
    ///
    /// When the `BufferSubAllocator` goes out of scope, the buffer object gets added to an
    /// internal pool and is available for immediate reuse. `get_scratch_storage()` returns
    /// buffers from this pool if possible. A `BufferSubAllocator` can be explicitly returned to
    /// the pool by calling `reset()`.
    ///
    /// Returning a `BufferSubAllocator` back to the buffer too early can result in validation
    /// failures and/or data races. It is the callers responsibility to manage reuse within a
    /// Recording and guarantee synchronized access to buffer bindings.
    ///
    /// This type of usage is currently limited to GPU-only storage buffers.
    // Port of: src/gpu/graphite/BufferManager.h#L276-L280 (chrome/m156)
    #[doc(alias = "getScratchStorage")]
    #[must_use]
    pub fn get_scratch_storage(&self, required_bytes: usize) -> BufferSubAllocator {
        self.get_buffer(
            GPU_ONLY_STORAGE_BUFFER_INDEX,
            required_bytes,
            /*stride=*/ 1,
            /*xtra_alignment=*/ 1,
            /*headroom=*/ 0,
            ClearBuffer::No,
            Shareable::Scratch,
        )
    }

    /// `transferToRecording()`: finalizes all buffers and transfers ownership of them to a
    /// `Recording`. Returns true on success and false if a mapping had previously failed.
    ///
    /// Regardless of success or failure, the `DrawBufferManager` is reset to a valid initial
    /// state for recording buffer data for the next `Recording`.
    ///
    /// # Panics
    /// If a transfer buffer range is no longer held by the upload manager.
    // Port of: src/gpu/graphite/BufferManager.cpp#L309-L383 (chrome/m156)
    #[doc(alias = "transferToRecording")]
    #[must_use]
    pub fn transfer_to_recording(&self, recording: &mut Recording) -> bool {
        let mut state = self.state.borrow_mut();
        if state.mapping_failed {
            // All state should have been reset by onFailedBuffer() except for this error flag.
            debug_assert!(state.used_buffers.is_empty() && state.clear_list.is_empty());
            #[cfg(debug_assertions)]
            for buffer_state in &state.current_buffers {
                debug_assert!(!buffer_state.available_buffer.is_valid());
                debug_assert!(buffer_state.unavailable_scratch_buffers.is_empty());
            }
            state.mapping_failed = false;
            return false;
        }

        for index in 0..state.current_buffers.len() {
            // Reset all available buffer sub allocators since they won't be allocatable
            // anymore. This pushes the underlying resource and transfer range to fUsedBuffers
            let mut available = std::mem::take(&mut state.current_buffers[index].available_buffer);
            available.return_to_state(&mut state, true);
            let buffer_state = &mut state.current_buffers[index];

            // BufferSubAllocators should have gone out of scope well before Recorder::snap() is
            // called.
            debug_assert!(buffer_state.unavailable_scratch_buffers.is_empty());

            // We reset the last buffer size back to 0 to keep the buffer growth behavior the same
            // across calls to snap(). If we knew every snap() would be approximately the same
            // workload, we could choose to keep the last alloc size as-is so that subsequent
            // frames create fewer buffer allocations. We choose *not* to do this because:
            //  - Chrome often snaps Recordings with disparate workloads within a frame (e.g.
            //    tile vs canvas2d) and we don't want to overallocate on a small recording.
            //  - It obfuscates the performance cost of the first frame if we reach a steady
            //    state that requires no additional buffer allocations.
            // We could choose to reduce fLastBufferSize (e.g. halve it) to get a head start and
            // reduce the potential for over-allocation, but in performance measurements on
            // buffer-heavy scenes this did not lead to measurable improvements. Thus, we reset
            // so every frame is the same.
            buffer_state.last_buffer_size = 0;
        }

        if !state.clear_list.is_empty() {
            let clear_list = std::mem::take(&mut state.clear_list);
            recording
                .priv_()
                .task_list()
                .add(ClearBuffersTask::make(clear_list));
        }

        for used in std::mem::take(&mut state.used_buffers) {
            let UsedBuffer {
                buffer,
                transfer_buffer,
                mapped,
            } = used;
            if transfer_buffer.is_valid() {
                // Since the transfer buffer is managed by the UploadManager, we don't manually
                // unmap it here or need to pass a ref into CopyBufferToBufferTask.
                debug_assert!(!self.caps.draw_buffer_can_be_mapped());
                if let Some(data) = &mapped {
                    // The bytes written to the staging block go to the transfer buffer range
                    // the copy reads from.
                    self.upload_manager
                        .borrow_mut()
                        .write_range(&transfer_buffer, data);
                }
                let copy_size = buffer.size();
                recording
                    .priv_()
                    .task_list()
                    .add(CopyBufferToBufferTask::make(
                        transfer_buffer
                            .buffer
                            .as_ref()
                            .expect("transfer buffer is valid"),
                        transfer_buffer.offset as usize,
                        buffer,
                        /*dst_offset=*/ 0,
                        copy_size,
                    ));
            } else {
                if buffer.is_mapped() {
                    match &mapped {
                        Some(data) => buffer.unmap_with(data),
                        None => buffer.unmap(),
                    }
                }
                recording.priv_().add_resource_ref(buffer.into_any());
            }
        }
        true
    }

    /// `testingOnly_onFailedBuffer()`.
    #[doc(alias = "testingOnly_onFailedBuffer")]
    pub fn testing_only_on_failed_buffer(&self) {
        Self::on_failed_buffer(&mut self.state.borrow_mut());
    }

    // The returned sub allocator will have an offset that is aligned to `stride`,
    // `xtraAlignment` and the minimum alignment for `stateIndex`. Its `availableWithStride()`
    // will be >= `count`.
    // Port of: src/gpu/graphite/BufferManager.cpp#L385-L488 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    fn get_buffer(
        &self,
        state_index: usize,
        count: usize,
        stride: usize,
        xtra_alignment: usize,
        headroom: usize,
        cleared: ClearBuffer,
        shareable: Shareable,
    ) -> BufferSubAllocator {
        let mut guard = self.state.borrow_mut();
        let manager_state = &mut *guard;

        // The size for a buffer is aligned to the minimum block size for better resource reuse,
        // which is more conservative than fMinAlignment.
        let min_block_size = manager_state.current_buffers[state_index].min_block_size;
        let required_bytes32 =
            buffer_aligner::validate_count_and_stride(count, stride, headroom, min_block_size);
        if manager_state.mapping_failed || required_bytes32 == 0 {
            return BufferSubAllocator::default();
        }

        let state = &manager_state.current_buffers[state_index];
        let support_cpu_upload =
            state.access_pattern == AccessPattern::HostVisible || state.use_transfer_buffer;

        // Shareable buffers must be GPU-only to actually share effectively.
        debug_assert!(shareable == Shareable::No || !support_cpu_upload);

        // For non-shareable buffers, we keep the largest relinquished non-shareable buffer in
        // case it has room leftover to be used by future allocations. Scratch buffer ownership
        // is entirely managed by the caller, so always create a new BufferSubAllocator.
        if shareable == Shareable::No {
            let state = &mut manager_state.current_buffers[state_index];
            state.available_buffer.reset_for_new_binding(); // ensure we include min binding alignment
            state
                .available_buffer
                .prep_for_stride(stride, xtra_alignment, count, headroom);
            if state.available_buffer.available_with_stride() as usize >= count {
                debug_assert!(state.available_buffer.buffer.is_some());
                debug_assert!(
                    state
                        .available_buffer
                        .buffer
                        .as_ref()
                        .is_some_and(|buffer| buffer.base().shareable() == shareable)
                );
                debug_assert_eq!(state.available_buffer.mapped.is_some(), support_cpu_upload);
                return std::mem::take(&mut state.available_buffer);
            }

            // Not enough room in the available buffer so release it and create a new buffer.
            let mut available = std::mem::take(&mut state.available_buffer);
            available.return_to_state(manager_state, true);
        }

        let state = &mut manager_state.current_buffers[state_index];

        // Create the next buffer by doubling the size of the previous buffer and clamping to be
        // within the min and max block sizes if `requiredBytes` is less than the max. Otherwise,
        // create a buffer large enough to satisfy `requiredBytes` but align it to minBlockSize.
        let mut buffer_size = align_to(required_bytes32, state.min_block_size);
        if buffer_size < state.max_block_size {
            // fMaxBlockSize should be sufficiently small that there's no risk of overflowing
            // here.
            debug_assert!(u32::MAX / 2 > state.last_buffer_size);
            buffer_size = buffer_size.max((state.last_buffer_size * 2).min(state.max_block_size));
            state.last_buffer_size = buffer_size;
            debug_assert!(buffer_size <= state.max_block_size);
        } else {
            // Jump to the max block size for subsequent amortized allocations if we get a really
            // big buffer request.
            state.last_buffer_size = state.max_block_size;
        }
        debug_assert!(buffer_size >= required_bytes32 && buffer_size >= state.min_block_size);

        let Some(buffer) =
            state.find_or_create_buffer(&self.resource_provider, shareable, buffer_size)
        else {
            Self::on_failed_buffer(manager_state);
            return BufferSubAllocator::default();
        };

        let mut transfer_buffer = BindBufferInfo::default();
        let mut mapped = None;
        if support_cpu_upload {
            if state.use_transfer_buffer {
                let alignment = self.caps.required_transfer_buffer_alignment();
                let mut upload_manager = self.upload_manager.borrow_mut();
                if let Some((_, bind_info)) =
                    upload_manager.make_bind_info(buffer.size(), alignment, "TransferForDataBuffer")
                {
                    transfer_buffer = bind_info;
                    // The bytes are staged here and copied into the transfer buffer range when
                    // the buffer is transferred to the Recording.
                    mapped = Some(vec![0u8; buffer.size()]);
                }
            } else {
                mapped = buffer.map();
            }
            if mapped.is_none() {
                Self::on_failed_buffer(manager_state); // Either transfer buffer failed or direct mapping failed
                return BufferSubAllocator::default();
            }
        }

        if cleared == ClearBuffer::Yes {
            manager_state
                .clear_list
                .push(BindBufferInfo::new(&buffer, 0, buffer_size));
        }

        // The returned buffer is not set to fAvailableBuffer because it is going to be passed up
        // to the caller for their use first. Since a new BufferSubAllocator starts at offset 0,
        // there's no need to account for `xtraAlignment`.
        let min_binding_alignment = manager_state.current_buffers[state_index].min_alignment;
        BufferSubAllocator::new(
            Rc::downgrade(&self.state),
            state_index,
            min_binding_alignment,
            buffer,
            transfer_buffer,
            mapped,
            stride,
        )
    }

    // Port of: src/gpu/graphite/BufferManager.h#L304-L317 (chrome/m156)
    fn get_mapped_buffer(
        &self,
        state_index: usize,
        count: usize,
        stride: usize,
        reserved_count: usize,
        xtra_alignment: usize,
    ) -> Option<MappedAllocationInfo> {
        let mut buffer = self.get_buffer(
            state_index,
            count.max(reserved_count),
            stride,
            xtra_alignment,
            /*headroom=*/ 0,
            ClearBuffer::No,
            Shareable::No,
        );
        let (binding, first_range) = {
            let (_, binding) = buffer.get_mapped_subrange(count, stride, 1)?;
            (binding.clone(), (binding.offset, binding.size))
        };
        Some(MappedAllocationInfo {
            binding,
            allocator: buffer,
            first_range,
        })
    }

    // Helper method for the public GPU-only BufferBindInfo methods
    // Port of: src/gpu/graphite/BufferManager.h#L319-L330 (chrome/m156)
    fn get_binding(
        &self,
        state_index: usize,
        required_bytes: usize,
        cleared: ClearBuffer,
    ) -> BindBufferInfo {
        let mut alloc = self.get_buffer(
            state_index,
            required_bytes,
            /*stride=*/ 1,
            /*xtra_alignment=*/ 1,
            /*headroom=*/ 0,
            cleared,
            Shareable::No,
        );
        // `alloc` goes out of scope when this returns, but that is okay because it is only used
        // for GPU-only, non-shareable buffers. The returned BindBufferInfo will be unique still.
        alloc.get_subrange(required_bytes, /*stride=*/ 1, 1)
    }

    // Marks manager in a failed state, unmaps any previously collected buffers.
    // Port of: src/gpu/graphite/BufferManager.cpp#L287-L307 (chrome/m156)
    fn on_failed_buffer(state: &mut DrawBufferManagerState) {
        state.mapping_failed = true;

        // Clean up and unmap everything now
        state.clear_list.clear();
        for index in 0..state.current_buffers.len() {
            let mut available = std::mem::take(&mut state.current_buffers[index].available_buffer);
            available.return_to_state(state, true);

            let buffer_state = &mut state.current_buffers[index];
            // We aren't allocating anything anymore so don't maintain this list. Their
            // outstanding BufferSubAllocators will have a no-op when they get reset.
            buffer_state.unavailable_scratch_buffers.clear();
            buffer_state.last_buffer_size = 0;
        }

        for used in &state.used_buffers {
            if used.buffer.is_mapped() {
                used.buffer.unmap();
            }
        }
        state.used_buffers.clear();
    }
}

impl Drop for DrawBufferManager {
    // Port of: src/gpu/graphite/BufferManager.cpp#L286-L290 (chrome/m156)
    fn drop(&mut self) {
        // Must reset these *before* we are deleted
        let mut state = self.state.borrow_mut();
        for index in 0..state.current_buffers.len() {
            let mut available = std::mem::take(&mut state.current_buffers[index].available_buffer);
            available.return_to_state(&mut state, true);
        }
    }
}

/// A `BindBufferInfo` that `StaticBufferManager::finalize()` rewrites with the final binding of
/// the GPU-private static buffer (Skia passes a `BindBufferInfo*` that must outlive `finalize()`).
#[derive(Clone, Debug, Default)]
pub struct StaticBufferBinding(Arc<Mutex<BindBufferInfo>>);

impl StaticBufferBinding {
    /// A binding that points at no buffer yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The current value (a copy of the `BindBufferInfo`).
    #[must_use]
    pub fn get(&self) -> BindBufferInfo {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn set(&self, info: BindBufferInfo) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = info;
    }
}

/// `GlobalCache::StaticVertexCopyRanges` (`GPU_TEST_UTILS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticVertexCopyRanges {
    /// `fOffset`.
    pub offset: u32,
    /// `fUnalignedSize`.
    pub unaligned_size: u32,
    /// `fSize`.
    pub size: u32,
    /// `fRequiredAlignment`.
    pub required_alignment: u32,
}

/// What `StaticBufferManager::finalize()` needs from the `Context`, its `QueueManager` and the
/// `GlobalCache` (ported with G9b, which implements this trait).
pub trait StaticBufferHost {
    /// `queueManager->addUploadBufferManagerRefs(&fUploadManager, context->priv().resourceProvider())`.
    fn add_upload_buffer_manager_refs(&mut self, upload_manager: &mut UploadBufferManager);

    /// `queueManager->addTask(task, context, Protected::kNo)`.
    fn add_task(&mut self, task: &TaskRef, is_protected: Protected) -> bool;

    /// `globalCache->addStaticResource(buffer)`.
    fn add_static_resource(&mut self, buffer: ResourceRef<Buffer>);

    /// `globalCache->testingOnly_SetStaticVertexInfo(ranges, buffer)` (`GPU_TEST_UTILS`).
    fn testing_only_set_static_vertex_info(
        &mut self,
        _ranges: Vec<StaticVertexCopyRanges>,
        _buffer: Option<Arc<Resource<Buffer>>>,
    ) {
    }
}

/// `StaticBufferManager::FinishResult`.
// Port of: src/gpu/graphite/BufferManager.h#L433-L437 (chrome/m156)
#[doc(alias = "StaticBufferManager::FinishResult")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticFinishResult {
    /// Unable to create or copy static buffers.
    #[doc(alias = "kFailure")]
    Failure,
    /// Successfully created static buffers and added GPU tasks to the queue.
    #[doc(alias = "kSuccess")]
    Success,
    /// No static buffers required, no GPU tasks add to the queue.
    #[doc(alias = "kNoWork")]
    NoWork,
}

/// `StaticBufferManager::CopyRange`.
// Port of: src/gpu/graphite/BufferManager.h#L449-L457 (chrome/m156)
#[derive(Debug)]
struct CopyRange {
    // The CPU-to-GPU buffer and offset for the source of the copy
    source: BindBufferInfo,
    // The late-assigned destination of the copy
    target: StaticBufferBinding,
    // The requested stride of the data.
    required_alignment: u32,
    // The requested size without count-4 alignment
    unaligned_size: u32,
}

/// `StaticBufferManager::BufferState`.
// Port of: src/gpu/graphite/BufferManager.h#L459-L476 (chrome/m156)
#[derive(Debug)]
struct StaticBufferState {
    buffer_type: BufferType,
    // This is the lcm of the alignment requirement of the buffer type and the transfer buffer
    // alignment requirement.
    minimum_alignment: u32,
    data: Vec<CopyRange>,
    total_required_bytes: u32,
}

impl StaticBufferState {
    // Port of: src/gpu/graphite/BufferManager.cpp#L535-L539 (chrome/m156)
    fn new(ty: BufferType, caps: &dyn Caps) -> Self {
        Self {
            buffer_type: ty,
            minimum_alignment: minimum_alignment(ty, /*use_transfer_buffers=*/ true, caps),
            data: Vec::new(),
            total_required_bytes: 0,
        }
    }

    fn reset(&mut self) {
        self.data.clear();
        self.total_required_bytes = 0;
    }

    // Port of: src/gpu/graphite/BufferManager.cpp#L612-L683 (chrome/m156)
    fn create_and_update_bindings(
        &self,
        resource_provider: &SharedResourceProvider,
        host: &mut dyn StaticBufferHost,
        label: &str,
    ) -> bool {
        if self.total_required_bytes == 0 {
            return true; // No buffer needed
        }

        // The static buffer is always copyable when testing.
        let gpu_access_pattern = AccessPattern::GpuOnlyCopySrc;

        let static_buffer = resource_provider
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .find_or_create_non_shareable_buffer(
                self.total_required_bytes as usize,
                self.buffer_type,
                gpu_access_pattern,
                label,
            );
        let Some(static_buffer) = static_buffer else {
            skia_log_e!(
                "Failed to create static buffer for type {} of size {} bytes.",
                self.buffer_type as i32,
                self.total_required_bytes
            );
            return false;
        };

        let mut offset = 0u32;
        for data in &self.data {
            // Each copy range's size should be aligned to the lcm of the required alignment and
            // minimum alignment so we can increment the offset in the static buffer.
            let alignment =
                buffer_aligner::lcm_alignment(self.minimum_alignment, data.required_alignment);
            offset = align_non_pow2(offset, alignment);
            debug_assert!(
                offset.is_multiple_of(self.minimum_alignment)
                    && offset.is_multiple_of(data.required_alignment)
            );

            let size = data.source.size;
            data.target
                .set(BindBufferInfo::new(&static_buffer, offset, size));

            let copy_task = CopyBufferToBufferTask::make(
                data.source.buffer.as_ref().expect("source has a buffer"),
                data.source.offset as usize,
                static_buffer.clone(),
                offset as usize,
                size as usize,
            );

            // For static buffers, we want them all to be optimized as GPU only buffers. If we
            // are in a protected context, this means the buffers must be non-protected since
            // they will be read in the vertex shader which doesn't allow protected memory
            // access. Thus all the uploads to these buffers must be done as non-protected
            // commands.
            if !host.add_task(&copy_task, Protected::No) {
                skia_log_e!("Failed to copy data to static buffer.");
                return false;
            }

            offset += size;
        }

        debug_assert_eq!(offset, self.total_required_bytes);
        host.add_static_resource(static_buffer);
        true
    }
}

/// The `StaticBufferManager` is the one-time-only analog to `DrawBufferManager` and provides
/// "static" Buffers to `RenderSteps` and other Context-lifetime-tied objects, where the Buffers'
/// contents will not change and can benefit from prioritizing GPU reads. The assumed use case is
/// that they remain read-only on the GPU as well, so a single static buffer can be shared by all
/// Recorders.
///
/// Unlike `DrawBufferManager`'s `getXWriter()` functions that return both a Writer and a
/// `BindBufferInfo`, `StaticBufferManager` returns only a Writer and accepts a
/// [`StaticBufferBinding`] as an argument. This will be re-written with the final binding info
/// for the GPU-private data once that can be determined after *all* static buffers have been
/// requested.
// Port of: src/gpu/graphite/BufferManager.h#L420-L494 (chrome/m156)
#[doc(alias = "skgpu::graphite::StaticBufferManager")]
pub struct StaticBufferManager {
    resource_provider: SharedResourceProvider,
    upload_manager: UploadBufferManager,
    required_transfer_alignment: u32,

    // The source data that's copied into a final GPU-private buffer
    vertex_buffer_state: StaticBufferState,
    index_buffer_state: StaticBufferState,

    // If mapping failed on Buffers created/managed by this StaticBufferManager or by the mapped
    // transfer buffers from the UploadManager, remember so that finalize() will fail.
    mapping_failed: bool,
}

impl std::fmt::Debug for StaticBufferManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StaticBufferManager")
            .field("vertex_buffer_state", &self.vertex_buffer_state)
            .field("index_buffer_state", &self.index_buffer_state)
            .field("mapping_failed", &self.mapping_failed)
            .finish_non_exhaustive()
    }
}

impl StaticBufferManager {
    /// `StaticBufferManager(resourceProvider, caps)`.
    // Port of: src/gpu/graphite/BufferManager.cpp#L524-L533 (chrome/m156)
    #[must_use]
    pub fn new(resource_provider: SharedResourceProvider, caps: &dyn Caps) -> Self {
        Self {
            upload_manager: UploadBufferManager::new(resource_provider.clone(), caps),
            resource_provider,
            required_transfer_alignment: to_u32(caps.required_transfer_buffer_alignment()),
            vertex_buffer_state: StaticBufferState::new(BufferType::Vertex, caps),
            index_buffer_state: StaticBufferState::new(BufferType::Index, caps),
            mapping_failed: false,
        }
    }

    /// `getVertexWriter()`: the `binding` is updated when `finalize()` is later called, to
    /// point to the packed, GPU-private buffer at the appropriate offset. The data written to
    /// the returned writer is copied to the private buffer at that offset. For the vertex
    /// writer, the count and stride of the buffer is passed to allow alignment of future
    /// vertices.
    ///
    /// ARM hardware b/399631317 also means that static vertex data must be padded and zeroed
    /// out. So we always request a count 4 aligned offset, count 4 aligned amount of space, and
    /// zero it.
    // Port of: src/gpu/graphite/BufferManager.cpp#L543-L561 (chrome/m156)
    #[doc(alias = "getVertexWriter")]
    pub fn get_vertex_writer(
        &mut self,
        count: usize,
        stride: usize,
        binding: &StaticBufferBinding,
    ) -> Option<VertexWriter<'_>> {
        let size = count * stride;
        let aligned_count = align_to(count, 4);
        let data = Self::prepare_static_data(
            &mut self.upload_manager,
            &mut self.vertex_buffer_state,
            &mut self.mapping_failed,
            self.required_transfer_alignment,
            size,
            stride * 4,
            binding,
        )?;
        if aligned_count > count {
            let byte_diff = (aligned_count - count) * stride;
            data[count * stride..count * stride + byte_diff].fill(0);
        }
        Some(VertexWriter::new(&mut data[..size]))
    }

    /// `getIndexWriter()`.
    ///
    /// TODO: Update the tessellation index buffer generation functions to use an `IndexWriter`
    /// so this can return an `IndexWriter` vs. a `VertexWriter` that happens to just write
    /// uint16s...
    // Port of: src/gpu/graphite/BufferManager.cpp#L563-L574 (chrome/m156)
    #[doc(alias = "getIndexWriter")]
    pub fn get_index_writer(
        &mut self,
        size: usize,
        binding: &StaticBufferBinding,
    ) -> Option<VertexWriter<'_>> {
        // The index writer does not have the same alignment requirements as a vertex, so we
        // simply pass in the minimum alignment as the required alignment
        let required_alignment = self.index_buffer_state.minimum_alignment as usize;
        let data = Self::prepare_static_data(
            &mut self.upload_manager,
            &mut self.index_buffer_state,
            &mut self.mapping_failed,
            self.required_transfer_alignment,
            size,
            required_alignment,
            binding,
        )?;
        Some(VertexWriter::new(&mut data[..size]))
    }

    // Port of: src/gpu/graphite/BufferManager.cpp#L576-L610 (chrome/m156)
    fn prepare_static_data<'a>(
        upload_manager: &'a mut UploadBufferManager,
        state: &mut StaticBufferState,
        mapping_failed: &mut bool,
        required_transfer_alignment: u32,
        required_bytes: usize,
        required_alignment: usize,
        target: &StaticBufferBinding,
    ) -> Option<&'a mut [u8]> {
        // Zero-out the target binding in the event of any failure in actually transfering data
        // later. Unlike in BufferSubAllocator::reserve(), we do use SkTo<uint32_t> to check
        // `requiredAlignment`. This is not dynamic data and is fully controlled by Graphite, so
        // if it asserts, then there is a bug in the static data for a Renderer that must be
        // fixed.
        let align32 =
            buffer_aligner::lcm_alignment(state.minimum_alignment, to_u32(required_alignment));
        target.set(BindBufferInfo::default());
        let mut size32 = buffer_aligner::validate_count_and_stride(
            required_bytes,
            /*stride=*/ 1,
            /*headroom=*/ 0,
            align32,
        );
        if size32 == 0 || *mapping_failed {
            return None;
        }

        // Copy data must be aligned to the transfer alignment, so align the reserved size to the
        // LCM of the minimum alignment (already net buffer and transfer alignment) and the
        // required alignment stride.
        size32 = align_non_pow2(size32, align32);

        let Some((transfer_map, transfer_bind_info)) = upload_manager.make_bind_info(
            size32 as usize,
            required_transfer_alignment as usize,
            "TransferForStaticBuffer",
        ) else {
            skia_log_e!(
                "Failed to create or map transfer buffer that initializes static GPU data."
            );
            *mapping_failed = true;
            return None;
        };

        state.data.push(CopyRange {
            source: transfer_bind_info,
            target: target.clone(),
            required_alignment: to_u32(required_alignment),
            unaligned_size: to_u32(required_bytes),
        });
        state.total_required_bytes = align_non_pow2(state.total_required_bytes, align32) + size32;

        Some(transfer_map)
    }

    /// `finalize()`: finalizes all buffers and records a copy task to compact and privatize
    /// static data. The final static buffers will become owned by the Context's `GlobalCache`.
    // Port of: src/gpu/graphite/BufferManager.cpp#L685-L725 (chrome/m156)
    pub fn finalize(&mut self, host: &mut dyn StaticBufferHost) -> StaticFinishResult {
        if self.mapping_failed {
            return StaticFinishResult::Failure;
        }

        let total_required_bytes = self.vertex_buffer_state.total_required_bytes
            + self.index_buffer_state.total_required_bytes;
        debug_assert!(total_required_bytes <= MAX_STATIC_DATA_SIZE);
        if total_required_bytes == 0 {
            return StaticFinishResult::NoWork;
        }

        host.add_upload_buffer_manager_refs(&mut self.upload_manager);

        if !self.vertex_buffer_state.create_and_update_bindings(
            &self.resource_provider,
            host,
            "StaticVertexBuffer",
        ) {
            return StaticFinishResult::Failure;
        }

        let stat_vert_copy: Vec<StaticVertexCopyRanges> = self
            .vertex_buffer_state
            .data
            .iter()
            .map(|data| {
                let target = data.target.get();
                StaticVertexCopyRanges {
                    offset: target.offset,
                    unaligned_size: data.unaligned_size,
                    size: target.size,
                    required_alignment: data.required_alignment,
                }
            })
            .collect();
        host.testing_only_set_static_vertex_info(
            stat_vert_copy,
            self.vertex_buffer_state
                .data
                .first()
                .and_then(|data| data.target.get().buffer),
        );

        if !self.index_buffer_state.create_and_update_bindings(
            &self.resource_provider,
            host,
            "StaticIndexBuffer",
        ) {
            return StaticFinishResult::Failure;
        }

        // Reset the static buffer manager since the Recording's copy tasks now manage ownership
        // of the transfer buffers and the GlobalCache owns the final static buffers.
        self.vertex_buffer_state.reset();
        self.index_buffer_state.reset();
        StaticFinishResult::Success
    }
}

#[cfg(test)]
mod tests {
    use super::buffer_aligner::*;

    #[test]
    fn validate_count_and_stride_rejects_overflow() {
        assert_eq!(validate_count_and_stride(4, 8, 0, 16), 32);
        assert_eq!(validate_count_and_stride(4, 8, 100, 16), 100);
        assert_eq!(validate_count_and_stride(usize::MAX, 8, 0, 16), 0);
        assert_eq!(validate_count_and_stride(1 << 31, 4, 0, 16), 0);
        assert_eq!(validate_count_and_stride(1, 1, 0, 16), 1);
        assert_eq!(
            validate_count_and_stride(1, u32::MAX as usize - 16, 0, 16),
            0
        );
    }

    #[test]
    fn lcm_alignment_cases() {
        assert_eq!(lcm_alignment(1, 12), 12);
        assert_eq!(lcm_alignment(16, 16), 16);
        assert_eq!(lcm_alignment(4, 12), 12);
        assert_eq!(lcm_alignment(16, 12), 48);
        assert_eq!(lcm_alignment(6, 4), 12);
    }
}
