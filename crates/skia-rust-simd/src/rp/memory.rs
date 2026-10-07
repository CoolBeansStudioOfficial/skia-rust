// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRasterPipeline.cpp (uncheckedAppend's memory registration,
// addMemoryContext), src/opts/SkRasterPipeline_opts.h (patch/restore_memory_contexts,
// ptr_at_xy), src/core/SkRasterPipelineOpContexts.h (MemoryCtxPatch)

//! Writable memory of a raster pipeline run: [`MemoryBindings`], tail patching, and
//! [`Params`], the per-chunk state every stage receives.

use super::contexts::{MAX_SCRATCH_PER_PATCH, MemPtr, MemSlot, MemoryCtx, MemoryCtxInfo};
use super::ops::Stage;

/// The bytes of a binding.
#[derive(Debug)]
enum Bytes<'m> {
    Read(&'m [u8]),
    Write(&'m mut [u8]),
}

/// Memory bound to a [`MemSlot`] for one run: bytes plus, for [`MemoryCtx`] access, the
/// position of pixel `(0, 0)` and the row stride (Skia's `MemoryCtx { pixels, stride }`).
///
/// The pixel at `(x, y)` of a format with `bpp` bytes per pixel starts at byte
/// `origin + bpp * (y * stride + x)` (Skia's `ptr_at_xy`). `origin` may lie outside the bytes, like
/// the "fake base" pointers Skia's blitters build (`pixels - left*bpp - top*rowBytes`); every
/// access is bounds-checked and panics instead of reading or writing out of bounds.
/// [`MemPtr`]s address the bytes directly, ignoring origin and stride.
#[derive(Debug)]
pub struct MemView<'m> {
    bytes: Bytes<'m>,
    origin: isize,
    stride: isize,
}

impl<'m> MemView<'m> {
    /// Read-only memory (loads only).
    #[must_use]
    pub fn read(bytes: &'m [u8]) -> MemView<'m> {
        MemView {
            bytes: Bytes::Read(bytes),
            origin: 0,
            stride: 0,
        }
    }

    /// Writable memory.
    #[must_use]
    pub fn write(bytes: &'m mut [u8]) -> MemView<'m> {
        MemView {
            bytes: Bytes::Write(bytes),
            origin: 0,
            stride: 0,
        }
    }

    /// Sets the row stride in pixels (`MemoryCtx::stride`; default 0).
    #[must_use]
    pub fn with_stride(mut self, stride: isize) -> MemView<'m> {
        self.stride = stride;
        self
    }

    /// Sets the byte offset of pixel `(0, 0)` within the bytes (default 0; may be negative).
    #[must_use]
    pub fn with_origin(mut self, origin: isize) -> MemView<'m> {
        self.origin = origin;
        self
    }

    /// The row stride in pixels.
    #[must_use]
    pub fn stride(&self) -> isize {
        self.stride
    }

    /// The byte offset of pixel `(0, 0)`.
    #[must_use]
    pub fn origin(&self) -> isize {
        self.origin
    }

    /// The bound bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        match &self.bytes {
            Bytes::Read(b) => b,
            Bytes::Write(b) => b,
        }
    }

    /// The bound bytes, writable.
    ///
    /// # Panics
    /// If the view is read-only.
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        match &mut self.bytes {
            Bytes::Read(_) => panic!("raster pipeline: store to read-only memory"),
            Bytes::Write(b) => b,
        }
    }

    /// The byte offset of pixel `(x, y)` with `bpp` bytes per pixel (`ptr_at_xy`).
    fn pixel_offset(&self, x: usize, y: usize, bpp: usize) -> usize {
        // Skia: `(T*)ctx->pixels + dy*ctx->stride + dx`, in units of the pixel type.
        let index = isize::try_from(y)
            .ok()
            .and_then(|y| y.checked_mul(self.stride))
            .and_then(|i| i.checked_add(isize::try_from(x).ok()?));
        let offset = index
            .and_then(|i| i.checked_mul(isize::try_from(bpp).ok()?))
            .and_then(|o| o.checked_add(self.origin))
            .and_then(|o| usize::try_from(o).ok());
        offset.unwrap_or_else(|| {
            panic!("raster pipeline: pixel ({x}, {y}) lies before the bound memory")
        })
    }
}

/// The writable (and patched) memory of one run: a [`MemView`] per [`MemSlot`].
///
/// Programs never borrow writable memory; it is bound per run, so a compiled program can be
/// reused (Skia's blitters keep one per draw and point its `MemoryCtx`s at new memory).
#[derive(Debug, Default)]
pub struct MemoryBindings<'m> {
    pub(crate) views: Vec<Option<MemView<'m>>>,
}

impl<'m> MemoryBindings<'m> {
    /// No bindings.
    #[must_use]
    pub fn new() -> MemoryBindings<'m> {
        MemoryBindings { views: Vec::new() }
    }

    /// Binds `view` to `slot`, replacing any previous binding.
    pub fn bind(&mut self, slot: MemSlot, view: MemView<'m>) -> &mut Self {
        let i = usize::from(slot.0);
        if self.views.len() <= i {
            self.views.resize_with(i + 1, || None);
        }
        self.views[i] = Some(view);
        self
    }

    /// `self` with `view` bound to `slot`.
    #[must_use]
    pub fn with(mut self, slot: MemSlot, view: MemView<'m>) -> MemoryBindings<'m> {
        self.bind(slot, view);
        self
    }

    /// The view bound to `slot`.
    #[must_use]
    pub fn view(&self, slot: MemSlot) -> Option<&MemView<'m>> {
        self.views.get(usize::from(slot.0)).and_then(Option::as_ref)
    }

    /// Removes and returns the view bound to `slot`.
    pub fn unbind(&mut self, slot: MemSlot) -> Option<MemView<'m>> {
        self.views
            .get_mut(usize::from(slot.0))
            .and_then(Option::take)
    }
}

/// The view bound to `slot`.
fn view<'v, 'm>(views: &'v [Option<MemView<'m>>], slot: MemSlot) -> &'v MemView<'m> {
    views
        .get(usize::from(slot.0))
        .and_then(Option::as_ref)
        .unwrap_or_else(|| panic!("raster pipeline: no memory bound to {slot:?}"))
}

/// The view bound to `slot`, mutably.
fn view_mut<'v, 'm>(views: &'v mut [Option<MemView<'m>>], slot: MemSlot) -> &'v mut MemView<'m> {
    views
        .get_mut(usize::from(slot.0))
        .and_then(Option::as_mut)
        .unwrap_or_else(|| panic!("raster pipeline: no memory bound to {slot:?}"))
}

// Port of: src/core/SkRasterPipelineOpContexts.h#L64-L69 (chrome/m156)
/// `MemoryCtxPatch`: one [`MemoryCtx`]'s tail scratch buffer. (Skia's `backup` pointer is not
/// needed: the context is "patched" by the interpreter's tail flag, not by rewriting it.)
#[doc(alias = "SkRasterPipelineContexts::MemoryCtxPatch")]
#[derive(Clone, Debug)]
pub(crate) struct MemoryCtxPatch {
    pub(crate) scratch: [u8; MAX_SCRATCH_PER_PATCH],
    pub(crate) info: MemoryCtxInfo,
}

impl MemoryCtxPatch {
    /// A patch with zeroed scratch (`memset(patches[i].scratch, 0, …)` in `run`/`compile`).
    pub(crate) const fn new(info: MemoryCtxInfo) -> MemoryCtxPatch {
        MemoryCtxPatch {
            scratch: [0; MAX_SCRATCH_PER_PATCH],
            info,
        }
    }
}

/// The pixel format width of the [`MemoryCtx`] stages, by op (`SkColorTypeBytesPerPixel` of
/// the color type `uncheckedAppend` assigns), with whether the stage loads and/or stores.
#[allow(clippy::match_same_arms)] // one arm per Skia COLOR_TYPE_CASE
fn memory_access(stage: &Stage<'_>) -> Option<(MemoryCtx, usize, bool, bool)> {
    use Stage as S;
    // Port of: src/core/SkRasterPipeline.cpp#L72-L146 (chrome/m156) (COLOR_TYPE_CASE et al.)
    let (ctx, bpp, load, store) = match *stage {
        // a8: kAlpha_8
        S::LoadA8(c) | S::LoadA8Dst(c) => (c, 1, true, false),
        S::StoreA8(c) => (c, 1, false, true),
        // 565: kRGB_565
        S::Load565(c) | S::Load565Dst(c) => (c, 2, true, false),
        S::Store565(c) => (c, 2, false, true),
        // 4444: kARGB_4444
        S::Load4444(c) | S::Load4444Dst(c) => (c, 2, true, false),
        S::Store4444(c) => (c, 2, false, true),
        // 8888: kRGBA_8888
        S::Load8888(c) | S::Load8888Dst(c) => (c, 4, true, false),
        S::Store8888(c) => (c, 4, false, true),
        // rg88: kR8G8_unorm
        S::LoadRg88(c) | S::LoadRg88Dst(c) => (c, 2, true, false),
        S::StoreRg88(c) => (c, 2, false, true),
        // 16161616: kR16G16B16A16_unorm
        S::Load16161616(c) | S::Load16161616Dst(c) => (c, 8, true, false),
        S::Store16161616(c) => (c, 8, false, true),
        // a16: kA16_unorm
        S::LoadA16(c) | S::LoadA16Dst(c) => (c, 2, true, false),
        S::StoreA16(c) => (c, 2, false, true),
        // r16: kR16_unorm
        S::LoadR16(c) | S::LoadR16Dst(c) => (c, 2, true, false),
        S::StoreR16(c) => (c, 2, false, true),
        // rg1616: kR16G16_unorm
        S::LoadRg1616(c) | S::LoadRg1616Dst(c) => (c, 4, true, false),
        S::StoreRg1616(c) => (c, 4, false, true),
        // f16: kRGBA_F16
        S::LoadF16(c) | S::LoadF16Dst(c) => (c, 8, true, false),
        S::StoreF16(c) => (c, 8, false, true),
        // af16: kA16_float
        S::LoadAf16(c) | S::LoadAf16Dst(c) => (c, 2, true, false),
        S::StoreAf16(c) => (c, 2, false, true),
        // rf16: kR16_float
        S::LoadRf16(c) | S::LoadRf16Dst(c) => (c, 2, true, false),
        S::StoreRf16(c) => (c, 2, false, true),
        // rgf16: kR16G16_float
        S::LoadRgf16(c) | S::LoadRgf16Dst(c) => (c, 4, true, false),
        S::StoreRgf16(c) => (c, 4, false, true),
        // f32: kRGBA_F32
        S::LoadF32(c) | S::LoadF32Dst(c) => (c, 16, true, false),
        S::StoreF32(c) => (c, 16, false, true),
        // 1010102: kRGBA_1010102
        S::Load1010102(c) | S::Load1010102Dst(c) => (c, 4, true, false),
        S::Store1010102(c) => (c, 4, false, true),
        // 1010102_xr: kBGR_101010x_XR
        S::Load1010102Xr(c) | S::Load1010102XrDst(c) => (c, 4, true, false),
        S::Store1010102Xr(c) => (c, 4, false, true),
        // 10101010_xr: kBGRA_10101010_XR
        S::Load10101010Xr(c) | S::Load10101010XrDst(c) => (c, 8, true, false),
        S::Store10101010Xr(c) => (c, 8, false, true),
        // 10x6: kRGBA_10x6
        S::Load10x6(c) | S::Load10x6Dst(c) => (c, 8, true, false),
        S::Store10x6(c) => (c, 8, false, true),
        // debug_*: kRGBA_8888, store
        S::DebugR(c)
        | S::DebugG(c)
        | S::DebugB(c)
        | S::DebugA(c)
        | S::DebugR255(c)
        | S::DebugG255(c)
        | S::DebugB255(c)
        | S::DebugA255(c)
        | S::DebugX(c)
        | S::DebugY(c) => (c, 4, false, true),
        // Odd stage that doesn't have a load variant (appendLoad uses load_a8 + alpha_to_red)
        S::StoreR8(c) => (c, 1, false, true),
        S::SrcoverRgba8888(c) => (c, 4, true, true),
        S::ScaleU8(c) | S::LerpU8(c) => (c, 1, true, false),
        S::Scale565(c) | S::Lerp565(c) => (c, 2, true, false),
        _ => return None,
    };
    Some((ctx, bpp, load, store))
}

// Port of: src/core/SkRasterPipeline.cpp#L799-L815 (chrome/m156)
/// `addMemoryContext`: registers a use of `ctx`, merging the load/store flags of repeated uses.
pub fn add_memory_context(
    infos: &mut Vec<MemoryCtxInfo>,
    ctx: MemoryCtx,
    bytes_per_pixel: usize,
    load: bool,
    store: bool,
) {
    if let Some(info) = infos.iter_mut().find(|i| i.context == ctx) {
        debug_assert_eq!(bytes_per_pixel, info.bytes_per_pixel);
        info.load = info.load || load;
        info.store = info.store || store;
    } else {
        infos.push(MemoryCtxInfo {
            context: ctx,
            bytes_per_pixel,
            load,
            store,
        });
    }
}

/// Registers the [`MemoryCtx`]s `stage` uses (the part of `uncheckedAppend` that fills
/// `fMemoryCtxInfos`).
pub fn register_memory_ctxs(infos: &mut Vec<MemoryCtxInfo>, stage: &Stage<'_>) {
    // Port of: src/core/SkRasterPipeline.cpp#L147-L158 (chrome/m156)
    if let Stage::Emboss(e) = stage {
        // Special-case, this op uses a context that holds *two* MemoryCtxs
        add_memory_context(infos, e.add, 1, true, false);
        add_memory_context(infos, e.mul, 1, true, false);
    } else if let Some((ctx, bpp, load, store)) = memory_access(stage) {
        // Port of: src/core/SkRasterPipeline.cpp#L176-L182 (chrome/m156)
        add_memory_context(infos, ctx, bpp, load, store);
    }
}

/// The [`MemoryCtxInfo`]s a pipeline registers, in registration order (what `uncheckedAppend`
/// accumulates in `fMemoryCtxInfos` while the stages are appended).
#[must_use]
pub fn memory_ctx_infos(stages: &[Stage<'_>]) -> Vec<MemoryCtxInfo> {
    let mut infos = Vec::new();
    for stage in stages {
        register_memory_ctxs(&mut infos, stage);
    }
    infos
}

// Port of: src/opts/SkRasterPipeline_opts.h#L1697-L1716 (chrome/m156)
/// `patch_memory_contexts`: before the tail chunk, copies the `tail` pixels of every loaded
/// context into its scratch buffer. The stages then address the scratch buffer (see
/// [`Params::ptr_at_xy`]).
pub(crate) fn patch_memory_contexts(
    views: &[Option<MemView<'_>>],
    patches: &mut [MemoryCtxPatch],
    dx: usize,
    dy: usize,
    tail: usize,
) {
    for patch in patches {
        let bpp = patch.info.bytes_per_pixel;
        if patch.info.load {
            let v = view(views, patch.info.context.slot);
            let offset = v.pixel_offset(dx, dy, bpp);
            let n = bpp * tail;
            patch.scratch[..n].copy_from_slice(&v.bytes()[offset..offset + n]);
        }
    }
}

// Port of: src/opts/SkRasterPipeline_opts.h#L1718-L1735 (chrome/m156)
/// `restore_memory_contexts`: after the tail chunk, copies the `tail` pixels of every stored
/// context back from its scratch buffer. Bytes past the tail stay in the scratch buffer.
pub(crate) fn restore_memory_contexts(
    views: &mut [Option<MemView<'_>>],
    patches: &[MemoryCtxPatch],
    dx: usize,
    dy: usize,
    tail: usize,
) {
    for patch in patches {
        let bpp = patch.info.bytes_per_pixel;
        if patch.info.store {
            let v = view_mut(views, patch.info.context.slot);
            let offset = v.pixel_offset(dx, dy, bpp);
            let n = bpp * tail;
            v.bytes_mut()[offset..offset + n].copy_from_slice(&patch.scratch[..n]);
        }
    }
}

/// The value of the pipeline's tail byte outside the tail chunk (`*tailPointer = 0xFF`).
pub const NO_TAIL: u8 = 0xFF;

/// What a stage receives besides its context and registers: Skia's `dx`, `dy`, `base` stage
/// arguments, the pipeline's tail byte, and the run's memory.
///
/// Created per stage by the interpreter; LLVM keeps it in registers when the stage is inlined.
#[derive(Debug)]
pub struct Params<'r, 'm> {
    /// The x coordinate of the chunk's first pixel.
    pub dx: usize,
    /// The row.
    pub dy: usize,
    /// `*tailPointer`: the number of active pixels in the tail chunk, [`NO_TAIL`] (`0xFF`)
    /// otherwise.
    pub tail: u8,
    /// `SkSL`'s `base` pointer (`set_base_pointer`); `None` until set in this chunk.
    pub base: Option<MemPtr>,
    pub(crate) views: &'r mut [Option<MemView<'m>>],
    pub(crate) patches: &'r mut [MemoryCtxPatch],
}

impl Params<'_, '_> {
    /// The scratch buffer standing in for `ctx` during the tail chunk.
    fn scratch(&self, ctx: MemoryCtx) -> &[u8; MAX_SCRATCH_PER_PATCH] {
        let patch = self.patches.iter().find(|p| p.info.context == ctx);
        &patch
            .expect("raster pipeline: unregistered MemoryCtx")
            .scratch
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2079-L2082 (chrome/m156) (`ptr_at_xy`)
    /// `ptr_at_xy<T>(ctx, dx, dy)` for a format of `bpp` bytes per pixel: the bytes from the
    /// chunk's first pixel on. During the tail chunk this is the context's scratch buffer
    /// (`MemoryCtx::pixels` patched to `scratch - offset`).
    ///
    /// # Panics
    /// If no memory is bound to the context's slot or the pixel lies outside it.
    #[must_use]
    pub fn ptr_at_xy(&self, ctx: MemoryCtx, bpp: usize) -> &[u8] {
        if self.tail != NO_TAIL {
            return self.scratch(ctx);
        }
        let v = view(self.views, ctx.slot);
        &v.bytes()[v.pixel_offset(self.dx, self.dy, bpp)..]
    }

    /// `ptr_at_xy` for stores: the writable bytes from the chunk's first pixel on (the scratch
    /// buffer during the tail chunk).
    ///
    /// # Panics
    /// If no memory is bound to the context's slot, it is read-only, or the pixel lies outside
    /// it.
    pub fn ptr_at_xy_mut(&mut self, ctx: MemoryCtx, bpp: usize) -> &mut [u8] {
        if self.tail != NO_TAIL {
            let patch = self.patches.iter_mut().find(|p| p.info.context == ctx);
            return &mut patch
                .expect("raster pipeline: unregistered MemoryCtx")
                .scratch;
        }
        let v = view_mut(self.views, ctx.slot);
        let offset = v.pixel_offset(self.dx, self.dy, bpp);
        &mut v.bytes_mut()[offset..]
    }

    /// The bytes at `ptr` (to the end of the bound memory).
    ///
    /// # Panics
    /// If no memory is bound to the slot or the offset lies outside it.
    #[must_use]
    pub fn ptr(&self, ptr: MemPtr) -> &[u8] {
        &view(self.views, ptr.slot).bytes()[ptr.offset as usize..]
    }

    /// The writable bytes at `ptr` (to the end of the bound memory).
    ///
    /// # Panics
    /// If no memory is bound to the slot, it is read-only, or the offset lies outside it.
    pub fn ptr_mut(&mut self, ptr: MemPtr) -> &mut [u8] {
        &mut view_mut(self.views, ptr.slot).bytes_mut()[ptr.offset as usize..]
    }

    /// `base + offset` (`SkSL`'s `SkRPOffset` addressing).
    ///
    /// # Panics
    /// If `set_base_pointer` has not run in this chunk.
    #[must_use]
    pub fn base_ptr(&self, offset: u32) -> MemPtr {
        self.base
            .expect("raster pipeline: SkSL base pointer not set")
            .add(offset)
    }
}
