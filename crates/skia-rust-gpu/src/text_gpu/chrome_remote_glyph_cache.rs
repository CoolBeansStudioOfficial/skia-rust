// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/private/chromium/SkChromeRemoteGlyphCache.h,
// src/text/gpu/SkChromeRemoteGlyphCache.cpp

//! The remote glyph cache: a [`StrikeServer`] that finds out which glyphs a text draw needs and
//! serializes them, and a [`StrikeClient`] in another process that merges them into its strike
//! cache so that the draw can be rendered there.
//!
//! The server draws on an analysis canvas ([`StrikeServer::make_analysis_canvas`]). The canvas
//! has no pixels. It runs the strike calculations of the GPU text code, which ask the server for
//! the strikes (`RemoteStrike`s) and note the glyphs they use. [`StrikeServer::write_strike_data`]
//! writes the typefaces, strikes and glyph data the client does not have yet. The client reads
//! them with [`StrikeClient::read_strike_data`], which makes [`TypefaceProxy`] typefaces for the
//! server's typefaces and strikes (pinned by the client's [`StrikeClientDiscardableHandleManager`])
//! for the server's strikes. A glyph that the client needs and does not have is a cache miss.
//!
//! The server and the client are not thread safe: Skia says so, and it is the caller's to serialize.
//!
//! Not ported: the trace events (`TRACE_EVENT1`) and the `SkDebugf` of the read errors, which C++
//! writes to its log with the file and line.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::descriptor::{AutoDescriptor, Descriptor, REC_TAG};
use skia_rust_core::device::{CreateInfo, Device, DeviceState, NoPixelsDevice};
use skia_rust_core::font_metrics::FontMetrics;
use skia_rust_core::font_metrics_priv;
use skia_rust_core::glyph::{
    ActionType, Glyph, GlyphAction, GlyphDigest, GlyphPositionRoundingSpec,
};
use skia_rust_core::glyph_run::GlyphRunList;
use skia_rust_core::packed_glyph_id::PackedGlyphId;
use skia_rust_core::paint::Paint;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::scaler_context::{SCALER_CONTEXT_REC_SIZE, ScalerContext, ScalerContextRec};
use skia_rust_core::slug::Slug;
use skia_rust_core::strike::{
    Strike, StrikeForGpu, StrikeForGpuCacheInterface, StrikeForGpuLock, StrikePinner,
    StrikePromise, TypefaceIdTranslator,
};
use skia_rust_core::strike_cache::StrikeCache;
use skia_rust_core::strike_spec::StrikeSpec;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::typeface::{Typeface, TypefaceId};
use skia_rust_core::write_buffer::BinaryWriteBuffer;

use crate::text_gpu::slug_impl::{self, SlugImpl};
use crate::text_gpu::sub_run_container::{
    StrikeDeviceInfo, SubRunContainer, SubRunCreationBehavior,
};
use crate::text_gpu::sub_run_control::SubRunControl;
use crate::text_gpu::typeface_remote::{TypefaceProxy, TypefaceProxyPrototype};

/// `SkDiscardableHandleId`: identifies a strike's handle on the remote client.
// Port of: include/private/chromium/SkChromeRemoteGlyphCache.h#L29 (chrome/m156)
#[doc(alias = "SkDiscardableHandleId")]
pub type DiscardableHandleId = u32;

/// `SkStrikeServer::DiscardableHandleManager`: an interface used by the server to create handles
/// for pinning strikes on the remote client.
// Port of: include/private/chromium/SkChromeRemoteGlyphCache.h#L35-L54 (chrome/m156)
#[doc(alias = "SkStrikeServer::DiscardableHandleManager")]
pub trait StrikeServerDiscardableHandleManager: Send + Sync {
    /// `createHandle`: creates a new *locked* handle and returns a unique id that can be used to
    /// identify it on the remote client.
    fn create_handle(&self) -> DiscardableHandleId;

    /// `lockHandle`: returns true if the handle could be successfully locked. The server can
    /// assume it will remain locked until the next set of serialized entries is pulled from the
    /// [`StrikeServer`]. If it returns false, the cache entry mapped to the handle has been
    /// deleted on the client. Any subsequent attempts to lock the same handle are not allowed.
    fn lock_handle(&self, id: DiscardableHandleId) -> bool;

    /// `isHandleDeleted`: returns true if a handle has been deleted on the remote client. It is
    /// invalid to use a handle id again with this manager once this returns true.
    fn is_handle_deleted(&self, id: DiscardableHandleId) -> bool;
}

/// `SkStrikeClient::CacheMissType`. This enum is used in histogram reporting in Chromium. Please
/// don't re-order the list of entries, and consider it to be append-only.
// Port of: include/private/chromium/SkChromeRemoteGlyphCache.h#L86-L100 (chrome/m156)
#[doc(alias = "SkStrikeClient::CacheMissType")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum CacheMissType {
    /// `kFontMetrics`: hard failures where no fallback could be found.
    FontMetrics = 0,
    /// `kGlyphMetrics`.
    GlyphMetrics = 1,
    /// `kGlyphImage`.
    GlyphImage = 2,
    /// `kGlyphPath`.
    GlyphPath = 3,
    /// `kGlyphMetricsFallback` (deprecated): the original glyph could not be found and a
    /// fallback was used.
    GlyphMetricsFallback = 4,
    /// `kGlyphPathFallback` (deprecated).
    GlyphPathFallback = 5,
    /// `kGlyphDrawable`.
    GlyphDrawable = 6,
}

impl CacheMissType {
    /// `kLast`.
    pub const LAST: CacheMissType = CacheMissType::GlyphDrawable;
}

/// `SkStrikeClient::DiscardableHandleManager::ReadFailureData`.
// Port of: include/private/chromium/SkChromeRemoteGlyphCache.h#L114-L121 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadFailureData {
    /// `memorySize`.
    pub memory_size: usize,
    /// `bytesRead`.
    pub bytes_read: usize,
    /// `typefaceSize`.
    pub typeface_size: u64,
    /// `strikeCount`.
    pub strike_count: u64,
    /// `glyphImagesCount`.
    pub glyph_images_count: u64,
    /// `glyphPathsCount`.
    pub glyph_paths_count: u64,
}

/// `SkStrikeClient::DiscardableHandleManager`: an interface to delete handles that may be pinned
/// by the remote server.
// Port of: include/private/chromium/SkChromeRemoteGlyphCache.h#L102-L123 (chrome/m156)
#[doc(alias = "SkStrikeClient::DiscardableHandleManager")]
pub trait StrikeClientDiscardableHandleManager: Send + Sync {
    /// `deleteHandle`: returns true if the handle was unlocked and can be safely deleted. Once
    /// successful, subsequent attempts to delete the same handle are invalid.
    fn delete_handle(&self, id: DiscardableHandleId) -> bool;

    /// `assertHandleValid`.
    fn assert_handle_valid(&self, _id: DiscardableHandleId) {}

    /// `notifyCacheMiss`.
    fn notify_cache_miss(&self, miss_type: CacheMissType, font_size: i32);

    /// `notifyReadFailure`.
    fn notify_read_failure(&self, _data: &ReadFailureData) {}
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

// -- RemoteStrike -----------------------------------------------------------------------------

/// The state of a [`RemoteStrike`] that its lock guards.
// Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L75-L154 (chrome/m156)
struct RemoteStrikeInner {
    /// `fContext`: the context built using the descriptor. `None` until `ensureScalerContext`.
    context: Option<ScalerContext>,
    /// `fStrikeSpec`: set every time `getOrCreateCache` is called. This allows the code to
    /// maintain the `fContext` as lazy as possible.
    ///
    /// skia-rust: C++ keeps a pointer to the caller's spec. Here it is a copy.
    strike_spec: Option<StrikeSpec>,
    /// `fHaveSentFontMetrics`: have the metrics been sent for this strike. Only send them once.
    have_sent_font_metrics: bool,
    /// `fSentGlyphs`: the masks and paths that currently reside in the GPU process.
    sent_glyphs: HashMap<PackedGlyphId, GlyphDigest>,
    /// `fMasksToSend`: the masks, SDF masks and paths that need to be sent to the GPU task for
    /// the processed text blobs. Cleared after diffs are serialized.
    masks_to_send: Vec<Glyph>,
    /// `fPathsToSend`.
    paths_to_send: Vec<Glyph>,
    /// `fDrawablesToSend`.
    drawables_to_send: Vec<Glyph>,
}

impl RemoteStrikeInner {
    /// `RemoteStrike::ensureScalerContext`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L213-L217 (chrome/m156)
    fn ensure_scaler_context(&mut self) {
        if self.context.is_none() {
            self.context = Some(
                self.strike_spec
                    .as_ref()
                    .expect("a remote strike has its strike spec while it has pending glyphs")
                    .create_scaler_context(),
            );
        }
    }

    /// `RemoteStrike::prepareForImage`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L86-L90 (chrome/m156)
    fn prepare_for_image(context: &mut ScalerContext, glyph: &mut Glyph) -> bool {
        if !glyph.set_image_has_been_called() {
            context.get_image(glyph);
        }
        glyph.image().is_some()
    }

    /// `RemoteStrike::prepareForPath`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L91-L95 (chrome/m156)
    fn prepare_for_path(context: &mut ScalerContext, glyph: &mut Glyph) -> bool {
        if !glyph.set_path_has_been_called() {
            context.get_path(glyph);
        }
        glyph.path().is_some()
    }

    /// `RemoteStrike::prepareForDrawable`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L96-L100 (chrome/m156)
    fn prepare_for_drawable(context: &mut ScalerContext, glyph: &mut Glyph) -> bool {
        if !glyph.set_drawable_has_been_called() {
            let drawable = context.get_drawable(glyph);
            glyph.set_drawable(drawable);
        }
        glyph.drawable().is_some()
    }

    /// `RemoteStrike::digestFor`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L228-L261 (chrome/m156)
    fn digest_for(
        &mut self,
        action_type: ActionType,
        packed_glyph_id: PackedGlyphId,
    ) -> GlyphDigest {
        if let Some(digest) = self.sent_glyphs.get(&packed_glyph_id)
            && digest.action_for(action_type) != GlyphAction::Unset
        {
            return *digest;
        }

        self.ensure_scaler_context();
        let context = self
            .context
            .as_mut()
            .expect("the scaler context was ensured above");
        let new_glyph = context.make_glyph(packed_glyph_id);
        let glyphs = match action_type {
            ActionType::Path => &mut self.paths_to_send,
            ActionType::Drawable => &mut self.drawables_to_send,
            _ => &mut self.masks_to_send,
        };
        glyphs.push(new_glyph);
        let glyph = glyphs.last_mut().expect("a glyph was just pushed");

        let mut digest = self
            .sent_glyphs
            .get(&packed_glyph_id)
            .copied()
            .unwrap_or_else(|| GlyphDigest::new(0, glyph));

        digest.set_action_for(action_type, &mut |prepare_type| match prepare_type {
            ActionType::Path => Self::prepare_for_path(context, glyph),
            ActionType::Drawable => Self::prepare_for_drawable(context, glyph),
            _ => Self::prepare_for_image(context, glyph),
        });
        self.sent_glyphs.insert(packed_glyph_id, digest);

        digest
    }
}

/// The strike server's record of the glyphs a strike has used, and which of them the client
/// has not been sent (`RemoteStrike`).
// Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L75-L264 (chrome/m156)
struct RemoteStrike {
    /// `fDescriptor`.
    descriptor: AutoDescriptor,
    /// `fDiscardableHandleId`.
    discardable_handle_id: DiscardableHandleId,
    /// `fRoundingSpec`.
    rounding_spec: GlyphPositionRoundingSpec,
    /// `fStrikeSpecTypefaceId`.
    strike_spec_typeface_id: TypefaceId,
    inner: Mutex<RemoteStrikeInner>,
}

impl RemoteStrike {
    /// `RemoteStrike(strikeSpec, context, discardableHandleId)`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L122-L135 (chrome/m156)
    fn new(
        strike_spec: &StrikeSpec,
        context: ScalerContext,
        discardable_handle_id: DiscardableHandleId,
    ) -> Self {
        let rounding_spec = GlyphPositionRoundingSpec::new(
            context.is_subpixel(),
            context.compute_axis_alignment_for_h_text(),
        );
        Self {
            descriptor: AutoDescriptor::from_descriptor(strike_spec.descriptor()),
            discardable_handle_id,
            rounding_spec,
            strike_spec_typeface_id: strike_spec.typeface().unique_id(),
            inner: Mutex::new(RemoteStrikeInner {
                context: Some(context),
                strike_spec: None,
                have_sent_font_metrics: false,
                sent_glyphs: HashMap::new(),
                masks_to_send: Vec::new(),
                paths_to_send: Vec::new(),
                drawables_to_send: Vec::new(),
            }),
        }
    }

    /// `discardableHandleId`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L104 (chrome/m156)
    fn discardable_handle_id(&self) -> DiscardableHandleId {
        self.discardable_handle_id
    }

    /// `setStrikeSpec`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L224-L226 (chrome/m156)
    fn set_strike_spec(&self, strike_spec: &StrikeSpec) {
        lock_unpoisoned(&self.inner).strike_spec = Some(strike_spec.clone());
    }

    /// `hasPendingGlyphs`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L118-L120 (chrome/m156)
    fn has_pending_glyphs(&self) -> bool {
        let inner = lock_unpoisoned(&self.inner);
        !inner.masks_to_send.is_empty()
            || !inner.paths_to_send.is_empty()
            || !inner.drawables_to_send.is_empty()
    }

    /// `resetScalerContext`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L219-L222 (chrome/m156)
    fn reset_scaler_context(&self) {
        let mut inner = lock_unpoisoned(&self.inner);
        inner.context = None;
        inner.strike_spec = None;
    }

    /// `writePendingGlyphs`: writes the strike's identity, its font metrics (the first time) and
    /// the glyphs the client has not got yet.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L170-L211 (chrome/m156)
    fn write_pending_glyphs(&self, buffer: &mut BinaryWriteBuffer) {
        debug_assert!(self.has_pending_glyphs());
        let mut guard = lock_unpoisoned(&self.inner);
        let inner = &mut *guard;

        // ScalerContext should not hold to the typeface, so we should not use its ID.
        // We should use StrikeSpec typeface and its ID instead.
        buffer.write_uint(self.strike_spec_typeface_id);
        buffer.write_uint(self.discardable_handle_id);
        self.descriptor.get_desc().flatten(buffer);

        buffer.write_bool(inner.have_sent_font_metrics);
        if !inner.have_sent_font_metrics {
            // Write FontMetrics if not sent before.
            let font_metrics = inner
                .context
                .as_mut()
                .expect("a strike with pending glyphs has its scaler context")
                .get_font_metrics();
            font_metrics_priv::flatten(buffer, &font_metrics);
            inner.have_sent_font_metrics = true;
        }

        inner.ensure_scaler_context();
        let context = inner
            .context
            .as_mut()
            .expect("the scaler context was ensured above");

        // Make sure to install all the mask data into the glyphs before sending.
        for glyph in &mut inner.masks_to_send {
            RemoteStrikeInner::prepare_for_image(context, glyph);
        }

        // Make sure to install all the path data into the glyphs before sending.
        for glyph in &mut inner.paths_to_send {
            RemoteStrikeInner::prepare_for_path(context, glyph);
        }

        // Make sure to install all the drawable data into the glyphs before sending.
        for glyph in &mut inner.drawables_to_send {
            RemoteStrikeInner::prepare_for_drawable(context, glyph);
        }

        // Send all the pending glyph information.
        Strike::flatten_glyphs_by_type(
            buffer,
            &inner.masks_to_send,
            &inner.paths_to_send,
            &inner.drawables_to_send,
        );

        // Reset all the sending data.
        inner.masks_to_send.clear();
        inner.paths_to_send.clear();
        inner.drawables_to_send.clear();
    }
}

/// The locked remote strike.
struct RemoteStrikeLock<'a> {
    inner: MutexGuard<'a, RemoteStrikeInner>,
}

impl StrikeForGpuLock for RemoteStrikeLock<'_> {
    fn digest_for(&mut self, action: ActionType, packed_id: PackedGlyphId) -> GlyphDigest {
        self.inner.digest_for(action, packed_id)
    }
}

impl StrikeForGpu for RemoteStrike {
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L81-L82 (chrome/m156)
    fn lock_for_gpu(&self) -> Box<dyn StrikeForGpuLock + '_> {
        Box::new(RemoteStrikeLock {
            inner: lock_unpoisoned(&self.inner),
        })
    }

    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L106-L108 (chrome/m156)
    fn descriptor_for_gpu(&self) -> &Descriptor {
        self.descriptor.get_desc()
    }

    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L112-L114 (chrome/m156)
    fn rounding_spec_for_gpu(&self) -> &GlyphPositionRoundingSpec {
        &self.rounding_spec
    }

    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L263-L265 (chrome/m156)
    fn strike_promise(self: Arc<Self>) -> StrikePromise {
        let inner = lock_unpoisoned(&self.inner);
        StrikePromise::from_spec(
            inner
                .strike_spec
                .as_ref()
                .expect("a remote strike has its strike spec while it is in use"),
        )
    }
}

// -- StrikeServerImpl -------------------------------------------------------------------------

/// `SkStrikeServerImpl`.
// Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L269-L310 (chrome/m156)
struct StrikeServerImpl {
    /// `fDescToRemoteStrike`.
    desc_to_remote_strike: HashMap<Descriptor, Arc<RemoteStrike>>,
    /// `fDiscardableHandleManager`.
    discardable_handle_manager: Arc<dyn StrikeServerDiscardableHandleManager>,
    /// `fCachedTypefaces`.
    cached_typefaces: HashSet<TypefaceId>,
    /// `fMaxEntriesInDescriptorMap`.
    max_entries_in_descriptor_map: usize,
    /// `fRemoteStrikesToSend`: state cached until the next serialization, in the order the
    /// strikes were first used. `remote_strikes_to_send_set` holds the same strikes by address.
    remote_strikes_to_send: Vec<Arc<RemoteStrike>>,
    remote_strikes_to_send_set: HashSet<usize>,
    /// `fTypefacesToSend`.
    typefaces_to_send: Vec<TypefaceProxyPrototype>,
}

/// `kMaxEntriesInDescriptorMap`.
// Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L284 (chrome/m156)
const MAX_ENTRIES_IN_DESCRIPTOR_MAP: usize = 2000;

impl StrikeServerImpl {
    /// `SkStrikeServerImpl(discardableHandleManager)`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L291-L294 (chrome/m156)
    fn new(discardable_handle_manager: Arc<dyn StrikeServerDiscardableHandleManager>) -> Self {
        Self {
            desc_to_remote_strike: HashMap::new(),
            discardable_handle_manager,
            cached_typefaces: HashSet::new(),
            max_entries_in_descriptor_map: MAX_ENTRIES_IN_DESCRIPTOR_MAP,
            remote_strikes_to_send: Vec::new(),
            remote_strikes_to_send_set: HashSet::new(),
            typefaces_to_send: Vec::new(),
        }
    }

    fn strike_key(strike: &Arc<RemoteStrike>) -> usize {
        Arc::as_ptr(strike) as usize
    }

    fn will_send(&self, strike: &Arc<RemoteStrike>) -> bool {
        self.remote_strikes_to_send_set
            .contains(&Self::strike_key(strike))
    }

    fn add_to_send(&mut self, strike: &Arc<RemoteStrike>) {
        if self
            .remote_strikes_to_send_set
            .insert(Self::strike_key(strike))
        {
            self.remote_strikes_to_send.push(Arc::clone(strike));
        }
    }

    fn reset_to_send(&mut self) {
        self.remote_strikes_to_send.clear();
        self.remote_strikes_to_send_set.clear();
    }

    /// `writeStrikeData`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L324-L368 (chrome/m156)
    fn write_strike_data(&mut self, memory: &mut Vec<u8>) {
        // We can use the default SkSerialProcs because we do not currently need to encode any
        // SkImages.
        let mut buffer = BinaryWriteBuffer::new();

        // Gather statistics about what needs to be sent.
        let mut strikes_to_send: i32 = 0;
        for strike in &self.remote_strikes_to_send {
            if strike.has_pending_glyphs() {
                strikes_to_send += 1;
            } else {
                // This strike has nothing to send, so drop its scaler context to reduce memory.
                strike.reset_scaler_context();
            }
        }

        // If there are no strikes or typefaces to send, then cleanup and return.
        if strikes_to_send == 0 && self.typefaces_to_send.is_empty() {
            self.reset_to_send();
            return;
        }

        // Send newly seen typefaces.
        buffer.write_int(
            i32::try_from(self.typefaces_to_send.len())
                .expect("SkASSERT_RELEASE(SkTFitsIn<int>(fTypefacesToSend.size()))"),
        );
        for proto in &self.typefaces_to_send {
            proto.flatten(&mut buffer);
        }
        self.typefaces_to_send.clear();

        buffer.write_int(strikes_to_send);
        for strike in &self.remote_strikes_to_send {
            if strike.has_pending_glyphs() {
                strike.write_pending_glyphs(&mut buffer);
                strike.reset_scaler_context();
            }
        }
        self.reset_to_send();

        // Copy data into the vector.
        let data = buffer.snapshot_as_data();
        memory.clear();
        memory.extend_from_slice(data.as_bytes());
    }

    /// `checkForDeletedEntries`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L375-L390 (chrome/m156)
    fn check_for_deleted_entries(&mut self) {
        let keys: Vec<Descriptor> = self.desc_to_remote_strike.keys().cloned().collect();
        for key in keys {
            if self.desc_to_remote_strike.len() <= self.max_entries_in_descriptor_map {
                break;
            }
            let strike = Arc::clone(&self.desc_to_remote_strike[&key]);
            if self
                .discardable_handle_manager
                .is_handle_deleted(strike.discardable_handle_id())
            {
                // If we are trying to send the strike, then do not erase it.
                if !self.will_send(&strike) {
                    self.desc_to_remote_strike.remove(&key);
                }
            }
        }
    }

    /// `getOrCreateCache`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L392-L448 (chrome/m156)
    fn get_or_create_cache(&mut self, strike_spec: &StrikeSpec) -> Arc<RemoteStrike> {
        if let Some(strike) = self
            .desc_to_remote_strike
            .get(strike_spec.descriptor())
            .cloned()
        {
            // We have processed the RemoteStrike before. Reuse it.
            strike.set_strike_spec(strike_spec);
            if self.will_send(&strike) {
                // Already tracking
                return strike;
            }

            // Strike is in unknown state on GPU. Start tracking strike on GPU by locking it.
            let locked = self
                .discardable_handle_manager
                .lock_handle(strike.discardable_handle_id());
            if locked {
                self.add_to_send(&strike);
                return strike;
            }

            // If it wasn't locked, then forget this strike, and build it anew below.
            self.desc_to_remote_strike.remove(strike_spec.descriptor());
        }

        let typeface = strike_spec.typeface();
        // Create a new RemoteStrike. Start by processing the typeface.
        let typeface_id = typeface.unique_id();
        if self.cached_typefaces.insert(typeface_id) {
            self.typefaces_to_send
                .push(TypefaceProxyPrototype::new(typeface));
        }

        let context = strike_spec.create_scaler_context();
        let new_handle = self.discardable_handle_manager.create_handle(); // Locked on creation
        let remote_strike = Arc::new(RemoteStrike::new(strike_spec, context, new_handle));
        remote_strike.set_strike_spec(strike_spec);
        self.add_to_send(&remote_strike);
        self.desc_to_remote_strike.insert(
            remote_strike.descriptor.get_desc().clone(),
            Arc::clone(&remote_strike),
        );

        self.check_for_deleted_entries();

        remote_strike
    }
}

/// The strike server as the GPU text code's strike cache (`SkStrikeServerImpl`'s
/// `StrikeForGPUCacheInterface`).
#[derive(Clone)]
struct ServerStrikeCache(Arc<Mutex<StrikeServerImpl>>);

impl StrikeForGpuCacheInterface for ServerStrikeCache {
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L370-L373 (chrome/m156)
    fn find_or_create_scoped_strike(&self, strike_spec: &StrikeSpec) -> Arc<dyn StrikeForGpu> {
        lock_unpoisoned(&self.0).get_or_create_cache(strike_spec)
    }
}

// -- GlyphTrackingDevice ----------------------------------------------------------------------

/// `GlyphTrackingDevice`: a device with no pixels that notes the strikes and glyphs that the
/// text drawn on it needs.
// Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L451-L516 (chrome/m156)
struct GlyphTrackingDevice {
    inner: NoPixelsDevice,
    /// `fStrikeServerImpl`.
    strike_server: ServerStrikeCache,
    /// `fSubRunControl`.
    sub_run_control: SubRunControl,
}

impl GlyphTrackingDevice {
    /// `GlyphTrackingDevice(dimensions, props, server, colorSpace, subRunControl)`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L434-L442 (chrome/m156)
    fn new(
        width: i32,
        height: i32,
        props: SurfaceProps,
        server: ServerStrikeCache,
        color_space: Option<ColorSpace>,
        sub_run_control: SubRunControl,
    ) -> Self {
        Self {
            inner: NoPixelsDevice::new_with_color_space(
                &IRect::from_wh(width, height),
                props,
                color_space,
            ),
            strike_server: server,
            sub_run_control,
        }
    }

    /// `strikeDeviceInfo`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L473-L475 (chrome/m156)
    fn strike_device_info(&self) -> StrikeDeviceInfo {
        StrikeDeviceInfo {
            surface_props: *self.inner.state().surface_props(),
            scaler_context_flags: self.scaler_context_flags(),
            sub_run_control: self.sub_run_control,
        }
    }
}

impl Device for GlyphTrackingDevice {
    fn state(&self) -> &DeviceState {
        self.inner.state()
    }

    fn state_mut(&mut self) -> &mut DeviceState {
        self.inner.state_mut()
    }

    fn dev_clip_bounds(&self) -> IRect {
        self.inner.dev_clip_bounds()
    }

    fn push_clip_stack(&mut self) {
        self.inner.push_clip_stack();
    }

    fn pop_clip_stack(&mut self) {
        self.inner.pop_clip_stack();
    }

    fn clip_rect(&mut self, rect: &Rect, op: skia_rust_core::clip_op::ClipOp, aa: bool) {
        self.inner.clip_rect(rect, op, aa);
    }

    fn clip_rrect(
        &mut self,
        rrect: &skia_rust_core::rrect::RRect,
        op: skia_rust_core::clip_op::ClipOp,
        aa: bool,
    ) {
        self.inner.clip_rrect(rrect, op, aa);
    }

    fn clip_path(
        &mut self,
        path: &skia_rust_core::path::Path,
        op: skia_rust_core::clip_op::ClipOp,
        aa: bool,
    ) {
        self.inner.clip_path(path, op, aa);
    }

    fn clip_region(
        &mut self,
        region: &skia_rust_core::region::Region,
        op: skia_rust_core::clip_op::ClipOp,
    ) {
        self.inner.clip_region(region, op);
    }

    fn on_clip_shader(&mut self, shader: skia_rust_core::shader::Shader) {
        self.inner.on_clip_shader(shader);
    }

    fn replace_clip(&mut self, rect: &IRect) {
        self.inner.replace_clip(rect);
    }

    fn is_clip_anti_aliased(&self) -> bool {
        self.inner.is_clip_anti_aliased()
    }

    fn is_clip_empty(&self) -> bool {
        self.inner.is_clip_empty()
    }

    fn is_clip_rect(&self) -> bool {
        self.inner.is_clip_rect()
    }

    fn is_clip_wide_open(&self) -> bool {
        self.inner.is_clip_wide_open()
    }

    fn android_utils_clip_as_rgn(&self, rgn: &mut skia_rust_core::region::Region) {
        self.inner.android_utils_clip_as_rgn(rgn);
    }

    fn is_no_pixels_device(&self) -> bool {
        self.inner.is_no_pixels_device()
    }

    fn reset_for_next_picture(&mut self, bounds: &IRect) -> bool {
        self.inner.reset_for_next_picture(bounds)
    }

    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L462-L471 (chrome/m156)
    fn create_device(
        &mut self,
        info: &CreateInfo,
        _layer_paint: Option<&Paint>,
    ) -> Option<Box<dyn Device>> {
        let surface_props = self
            .inner
            .state()
            .surface_props()
            .clone_with_pixel_geometry(info.pixel_geometry);

        let size = info.info.dimensions();
        Some(Box::new(GlyphTrackingDevice::new(
            size.width,
            size.height,
            surface_props,
            self.strike_server.clone(),
            info.info.color_space(),
            self.sub_run_control,
        )))
    }

    fn draw_paint(&mut self, paint: &Paint) {
        self.inner.draw_paint(paint);
    }

    fn draw_points(
        &mut self,
        mode: skia_rust_core::canvas::PointMode,
        points: &[skia_rust_core::point::Point],
        paint: &Paint,
    ) {
        self.inner.draw_points(mode, points, paint);
    }

    fn draw_rect(&mut self, r: &Rect, paint: &Paint) {
        self.inner.draw_rect(r, paint);
    }

    fn draw_oval(&mut self, oval: &Rect, paint: &Paint) {
        self.inner.draw_oval(oval, paint);
    }

    fn draw_rrect(&mut self, rr: &skia_rust_core::rrect::RRect, paint: &Paint) {
        self.inner.draw_rrect(rr, paint);
    }

    fn draw_path(&mut self, path: &skia_rust_core::path::Path, paint: &Paint) {
        self.inner.draw_path(path, paint);
    }

    fn draw_vertices(
        &mut self,
        vertices: &skia_rust_core::vertices::Vertices,
        blender: skia_rust_core::blender::Blender,
        paint: &Paint,
        skip_color: bool,
    ) {
        self.inner
            .draw_vertices(vertices, blender, paint, skip_color);
    }

    fn draw_mesh(
        &mut self,
        mesh: &skia_rust_core::mesh::Mesh,
        blender: skia_rust_core::blender::Blender,
        paint: &Paint,
    ) {
        self.inner.draw_mesh(mesh, blender, paint);
    }

    fn draw_image_rect(
        &mut self,
        image: &skia_rust_core::image::Image,
        src: Option<&Rect>,
        dst: &Rect,
        sampling: &skia_rust_core::sampling_options::SamplingOptions,
        paint: &Paint,
        constraint: skia_rust_core::canvas::SrcRectConstraint,
    ) {
        self.inner
            .draw_image_rect(image, src, dst, sampling, paint, constraint);
    }

    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L478-L496 (chrome/m156)
    fn on_draw_glyph_run_list(&mut self, glyph_run_list: &GlyphRunList<'_>, paint: &Paint) {
        let mut draw_matrix = self.state().local_to_device().clone();
        draw_matrix.pre_translate(glyph_run_list.origin());

        // Just ignore the resulting SubRunContainer. Since we're not adding sub runs, no SubRuns
        // will be produced.
        let container = SubRunContainer::make(
            glyph_run_list,
            &draw_matrix,
            paint,
            &self.strike_device_info(),
            &self.strike_server,
            SubRunCreationBehavior::StrikeCalculationsOnly,
        );
        // Calculations only. No SubRuns.
        debug_assert!(container.is_empty());
    }

    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L499-L514 (chrome/m156)
    fn convert_glyph_run_list_to_slug(
        &mut self,
        glyph_run_list: &GlyphRunList<'_>,
        paint: &Paint,
    ) -> Option<Slug> {
        // Use the SkStrikeServer's strike cache to generate the Slug.
        SlugImpl::make(
            self.state().local_to_device(),
            glyph_run_list,
            paint,
            &self.strike_device_info(),
            &self.strike_server,
        )
        .map(Slug::from_base)
    }
}

// -- StrikeServer -----------------------------------------------------------------------------

/// `SkStrikeServer`: finds out which strikes and glyphs a text draw needs and serializes them
/// for a [`StrikeClient`] in another process. This class is not thread-safe.
// Port of: include/private/chromium/SkChromeRemoteGlyphCache.h#L31-L80 (chrome/m156)
#[doc(alias = "SkStrikeServer")]
pub struct StrikeServer {
    /// `fImpl`.
    imp: Arc<Mutex<StrikeServerImpl>>,
}

impl fmt::Debug for StrikeServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StrikeServer").finish_non_exhaustive()
    }
}

impl StrikeServer {
    /// `SkStrikeServer(discardableHandleManager)`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L505-L506 (chrome/m156)
    #[must_use]
    pub fn new(discardable_handle_manager: Arc<dyn StrikeServerDiscardableHandleManager>) -> Self {
        Self {
            imp: Arc::new(Mutex::new(StrikeServerImpl::new(
                discardable_handle_manager,
            ))),
        }
    }

    /// `makeAnalysisCanvas`: creates an analysis canvas used to populate the strike server with
    /// ops which will be serialized and rendered using the [`StrikeClient`].
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L524-L559 (chrome/m156)
    #[doc(alias = "makeAnalysisCanvas")]
    #[must_use]
    pub fn make_analysis_canvas(
        &self,
        width: i32,
        height: i32,
        props: &SurfaceProps,
        color_space: Option<ColorSpace>,
        dft_support: bool,
        dft_persp_support: bool,
    ) -> Canvas {
        // These are copied from the defaults in GrContextOptions for historical reasons.
        // TODO(herb, jvanverth) pipe in parameters that can be used for both Ganesh and Graphite
        // backends instead of just using the defaults.
        const MIN_DISTANCE_FIELD_FONT_SIZE: f32 = 18.0;

        #[cfg(target_os = "android")]
        const GLYPHS_AS_PATHS_FONT_SIZE: f32 = 384.0;
        #[cfg(target_os = "macos")]
        const GLYPHS_AS_PATHS_FONT_SIZE: f32 = 256.0;
        #[cfg(not(any(target_os = "android", target_os = "macos")))]
        const GLYPHS_AS_PATHS_FONT_SIZE: f32 = 324.0;

        // There is no need to set forcePathAA for the remote glyph cache as that control impacts
        // *how* the glyphs are rendered as paths, not *when* they are rendered as paths.
        let control = SubRunControl::new(
            dft_support,
            props.is_use_device_independent_fonts(),
            dft_persp_support,
            MIN_DISTANCE_FIELD_FONT_SIZE,
            GLYPHS_AS_PATHS_FONT_SIZE,
            false,
            false,
        );

        let tracking_device = GlyphTrackingDevice::new(
            width,
            height,
            *props,
            ServerStrikeCache(Arc::clone(&self.imp)),
            color_space,
            control,
        );
        Canvas::from_device(Box::new(tracking_device))
    }

    /// `writeStrikeData`: serializes the strike data captured using a canvas returned by
    /// [`make_analysis_canvas`](Self::make_analysis_canvas). Any handles locked using the
    /// [`StrikeServerDiscardableHandleManager`] will be assumed to be unlocked after this call.
    /// `memory` is left alone if there is nothing to send.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L561-L563 (chrome/m156)
    #[doc(alias = "writeStrikeData")]
    pub fn write_strike_data(&self, memory: &mut Vec<u8>) {
        lock_unpoisoned(&self.imp).write_strike_data(memory);
    }

    /// `setMaxEntriesInDescriptorMapForTesting`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L567-L569 (chrome/m156)
    #[doc(hidden)]
    pub fn set_max_entries_in_descriptor_map_for_testing(&self, count: usize) {
        lock_unpoisoned(&self.imp).max_entries_in_descriptor_map = count;
    }

    /// `remoteStrikeMapSizeForTesting`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L570-L572 (chrome/m156)
    #[doc(hidden)]
    #[must_use]
    pub fn remote_strike_map_size_for_testing(&self) -> usize {
        lock_unpoisoned(&self.imp).desc_to_remote_strike.len()
    }
}

// -- StrikeClient -----------------------------------------------------------------------------

/// `DiscardableStrikePinner`: pins a client strike until the server lets its handle go.
// Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L575-L588 (chrome/m156)
struct DiscardableStrikePinner {
    /// `fDiscardableHandleId`.
    discardable_handle_id: DiscardableHandleId,
    /// `fManager`.
    manager: Arc<dyn StrikeClientDiscardableHandleManager>,
}

impl StrikePinner for DiscardableStrikePinner {
    fn can_delete(&mut self) -> bool {
        self.manager.delete_handle(self.discardable_handle_id)
    }

    fn assert_valid(&self) {
        self.manager.assert_handle_valid(self.discardable_handle_id);
    }
}

/// `SkStrikeClient`: deserializes the strike data from a [`StrikeServer`], so that the ops the
/// server analyzed can be rasterized.
// Port of: include/private/chromium/SkChromeRemoteGlyphCache.h#L82-L146 (chrome/m156)
#[doc(alias = "SkStrikeClient")]
pub struct StrikeClient<'a> {
    /// `fServerTypefaceIdToTypeface`.
    server_typeface_id_to_typeface: HashMap<TypefaceId, Typeface>,
    /// `fDiscardableHandleManager`.
    discardable_handle_manager: Arc<dyn StrikeClientDiscardableHandleManager>,
    /// `fStrikeCache`.
    strike_cache: &'a StrikeCache,
    /// `fIsLogging`.
    is_logging: bool,
}

impl fmt::Debug for StrikeClient<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StrikeClient")
            .field("typefaces", &self.server_typeface_id_to_typeface.len())
            .field("is_logging", &self.is_logging)
            .finish_non_exhaustive()
    }
}

impl<'a> StrikeClient<'a> {
    /// `SkStrikeClient(discardableManager, isLogging, strikeCache)`. The strikes go to the global
    /// strike cache if `strike_cache` is `None`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L599-L606 (chrome/m156)
    #[must_use]
    pub fn new(
        discardable_manager: Arc<dyn StrikeClientDiscardableHandleManager>,
        is_logging: bool,
        strike_cache: Option<&'a StrikeCache>,
    ) -> Self {
        Self {
            server_typeface_id_to_typeface: HashMap::new(),
            discardable_handle_manager: discardable_manager,
            strike_cache: strike_cache.unwrap_or_else(|| StrikeCache::global()),
            is_logging,
        }
    }

    /// `readStrikeData`: deserializes the strike data from a [`StrikeServer`]. All messages
    /// generated from a server when serializing the ops must be deserialized before the op is
    /// rasterized. Returns false if the data is invalid.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L631-L746 (chrome/m156)
    #[doc(alias = "readStrikeData")]
    pub fn read_strike_data(&mut self, memory: &[u8]) -> bool {
        debug_assert_ne!(memory.len(), 0);

        // We do not need to set any SkDeserialProcs here because SkStrikeServerImpl::
        // writeStrikeData did not encode any SkImages.
        let mut buffer = ReadBuffer::new(memory);
        // Limit the kinds of effects that appear in a glyph's drawable (crbug.com/1442140):
        buffer.set_allow_sksl(false);

        let mut cur_typeface: i32 = 0;
        let mut cur_strike: i32 = 0;

        let manager = Arc::clone(&self.discardable_handle_manager);
        // C++ also logs "Read Error Posted <file> : <line>".
        let post_error = |buffer: &ReadBuffer<'_>, cur_typeface: i32, cur_strike: i32| {
            manager.notify_read_failure(&ReadFailureData {
                memory_size: memory.len(),
                bytes_read: buffer.offset(),
                typeface_size: u64::try_from(cur_typeface).unwrap_or(0),
                strike_count: u64::try_from(cur_strike).unwrap_or(0),
                glyph_images_count: 0,
                glyph_paths_count: 0,
            });
        };

        // Read the number of typefaces sent.
        let typeface_count = buffer.read_int();
        while cur_typeface < typeface_count {
            let proto = TypefaceProxyPrototype::make_from_buffer(&mut buffer);
            if let Some(proto) = proto {
                self.add_typeface(&proto);
            } else {
                post_error(&buffer, cur_typeface, cur_strike);
                return false;
            }
            cur_typeface += 1;
        }

        // Read the number of strikes sent.
        let strike_count = buffer.read_int();
        while cur_strike < strike_count {
            let server_typeface_id = buffer.read_uint();
            if server_typeface_id == 0 && !buffer.is_valid() {
                post_error(&buffer, cur_typeface, cur_strike);
                return false;
            }

            let discardable_handle_id = buffer.read_uint();
            if discardable_handle_id == 0 && !buffer.is_valid() {
                post_error(&buffer, cur_typeface, cur_strike);
                return false;
            }

            let server_descriptor = AutoDescriptor::make_from_buffer(&mut buffer);
            if !buffer.validate(server_descriptor.is_some()) {
                post_error(&buffer, cur_typeface, cur_strike);
                return false;
            }
            let Some(mut server_descriptor) = server_descriptor else {
                return false;
            };

            let font_metrics_initialized = buffer.read_bool();
            if !font_metrics_initialized && !buffer.is_valid() {
                post_error(&buffer, cur_typeface, cur_strike);
                return false;
            }

            let mut font_metrics: Option<FontMetrics> = None;
            if !font_metrics_initialized {
                font_metrics = font_metrics_priv::make_from_buffer(&mut buffer);
                if font_metrics.is_none() || !buffer.is_valid() {
                    post_error(&buffer, cur_typeface, cur_strike);
                    return false;
                }
            }

            let Some(client_typeface) = self
                .server_typeface_id_to_typeface
                .get(&server_typeface_id)
                .cloned()
            else {
                post_error(&buffer, cur_typeface, cur_strike);
                return false;
            };

            if !self.translate_typeface_id(&mut server_descriptor) {
                post_error(&buffer, cur_typeface, cur_strike);
                return false;
            }

            let client_descriptor = server_descriptor.get_desc();
            let mut strike = self.strike_cache.find_strike(client_descriptor);

            if strike.is_none() {
                // Metrics are only sent the first time. If creating a new strike, then the
                // metrics are not initialized.
                if font_metrics_initialized {
                    post_error(&buffer, cur_typeface, cur_strike);
                    return false;
                }
                let strike_spec = StrikeSpec::from_descriptor(client_descriptor, client_typeface);
                strike = Some(self.strike_cache.create_strike(
                    &strike_spec,
                    font_metrics,
                    Some(Box::new(DiscardableStrikePinner {
                        discardable_handle_id,
                        manager: Arc::clone(&self.discardable_handle_manager),
                    })),
                ));
            }
            let Some(strike) = strike else { return false };

            // Make sure this strike is pinned on the GPU side.
            strike.verify_pinned_strike();

            if !strike.merge_from_buffer(&mut buffer) {
                post_error(&buffer, cur_typeface, cur_strike);
                return false;
            }
            cur_strike += 1;
        }

        true
    }

    /// `retrieveTypefaceUsingServerIDForTest`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L805-L808 (chrome/m156)
    #[doc(hidden)]
    #[must_use]
    pub fn retrieve_typeface_using_server_id_for_test(
        &self,
        typeface_id: TypefaceId,
    ) -> Option<Typeface> {
        self.server_typeface_id_to_typeface
            .get(&typeface_id)
            .cloned()
    }

    /// `deserializeSlugForTest`: given the bytes of a slug, unflatten into a slug making sure to
    /// do the typeface id translation from renderer to GPU. Returns `None` if there was a
    /// problem.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L814-L817 (chrome/m156)
    #[doc(hidden)]
    #[must_use]
    pub fn deserialize_slug_for_test(&self, data: &[u8]) -> Option<Slug> {
        slug_impl::deserialize(data, Some(self))
    }

    /// `SkStrikeClientImpl::addTypeface`.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L778-L792 (chrome/m156)
    fn add_typeface(&mut self, typeface_proto: &TypefaceProxyPrototype) -> Typeface {
        // We already have the typeface.
        if let Some(typeface) = self
            .server_typeface_id_to_typeface
            .get(&typeface_proto.server_typeface_id())
        {
            return typeface.clone();
        }

        let new_typeface = Typeface::new(Arc::new(TypefaceProxy::from_prototype(
            typeface_proto,
            Arc::clone(&self.discardable_handle_manager),
            self.is_logging,
        )));
        self.server_typeface_id_to_typeface
            .insert(typeface_proto.server_typeface_id(), new_typeface.clone());
        new_typeface
    }
}

impl TypefaceIdTranslator for StrikeClient<'_> {
    /// `translateTypefaceID`: given a descriptor re-write the rec mapping the typeface id from the
    /// renderer to the corresponding typeface id on the GPU.
    // Port of: src/text/gpu/SkChromeRemoteGlyphCache.cpp#L748-L771 (chrome/m156)
    fn translate_typeface_id(&self, to_change: &mut AutoDescriptor) -> bool {
        let descriptor = to_change.get_desc_mut();

        // Rewrite the typefaceID in the rec.
        {
            let Some(entry) = descriptor.find_entry_mut(REC_TAG) else {
                return false;
            };
            let Ok(bytes) = <&mut [u8; SCALER_CONTEXT_REC_SIZE]>::try_from(&mut *entry) else {
                return false;
            };
            let mut rec = ScalerContextRec::from_bytes(bytes);
            // Get the local typeface from remote typefaceID.
            // Received a strike for a typeface which doesn't exist.
            let Some(typeface) = self.server_typeface_id_to_typeface.get(&rec.typeface_id) else {
                return false;
            };
            // Update the typeface id to work with the client side.
            rec.typeface_id = typeface.unique_id();
            bytes.copy_from_slice(&rec.to_bytes());
        }

        descriptor.compute_checksum();

        true
    }
}
