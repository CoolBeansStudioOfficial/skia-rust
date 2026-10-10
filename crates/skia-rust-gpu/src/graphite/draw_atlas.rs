// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/DrawAtlas.h, src/gpu/graphite/DrawAtlas.cpp (chrome/m156)

//! `skgpu::graphite::DrawAtlas`: a multi-page texture atlas, divided into plots that each pack
//! their own sub-rectangles, with least-recently-used eviction driven by flush tokens.

// The atlas packs its coordinates into `uint16_t` and bit-fields exactly as the C++ does (`SkToU16`,
// `uint8_t` page and plot indices, the 0x1FFF/0xE000 masks), so the integer casts mirror that
// arithmetic. The ranges are asserted where the C++ asserts them.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::{ColorInfo, ImageInfo};
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;

use crate::gpu::gpu_types::{Budgeted, Mipmapped, Renderable};
use crate::gpu::mask_format::MaskFormat;
use crate::gpu::rectanizer::Rectanizer;
use crate::gpu::rectanizer_skyline::RectanizerSkyline;
use crate::gpu::token::Token;
use crate::graphite::draw_context::DrawContext;
use crate::graphite::recorder::Recorder;
use crate::graphite::task::upload_task::{MipLevel, UploadSource};
use crate::graphite::texture_format::read_swizzle_for_color_type;
use crate::graphite::texture_proxy::TextureProxy;
use crate::graphite::texture_proxy_view::TextureProxyView;

/// `kMaxMultitexturePages`: the atlas never has more pages than this.
// Port of: src/gpu/graphite/DrawAtlas.h#L72 (chrome/m156)
pub const K_MAX_MULTITEXTURE_PAGES: u32 = 4;

/// `kMaxPlots`: the number of plots per page, limited by the bitfield in `BulkUsePlotUpdater`.
// Port of: src/gpu/graphite/DrawAtlas.h#L73 (chrome/m156)
pub const K_MAX_PLOTS: u32 = 32;

/// `kPlotRecentlyUsedCount`.
// Port of: src/gpu/graphite/DrawAtlas.cpp#L~445 (chrome/m156)
const K_PLOT_RECENTLY_USED_COUNT: i32 = 32;
/// `kPlotUsedCountBeforeEvict`.
const K_PLOT_USED_COUNT_BEFORE_EVICT: i32 = 8;
/// `kAtlasRecentlyUsedCount`.
const K_ATLAS_RECENTLY_USED_COUNT: i32 = 128;

/// `SK_InvalidGenID`: no generation or ID is ever zero.
const SK_INVALID_GEN_ID: u32 = 0;

/// `DrawAtlas::GenerationCounter`: hands out generation numbers for atlases and plots. Clones
/// share the counter, as the C++ atlases share the pointer to their client's counter.
// Port of: src/gpu/graphite/DrawAtlas.h#L92-L104 (chrome/m156)
#[doc(alias = "DrawAtlas::GenerationCounter")]
#[derive(Clone, Debug)]
pub struct GenerationCounter {
    generation: Arc<AtomicU64>,
}

impl Default for GenerationCounter {
    fn default() -> Self {
        GenerationCounter {
            generation: Arc::new(AtomicU64::new(1)),
        }
    }
}

impl GenerationCounter {
    /// `kInvalidGeneration`.
    pub const INVALID_GENERATION: u64 = 0;

    /// `next()`: returns the current generation and advances the counter.
    // Port of: src/gpu/graphite/DrawAtlas.h#L99 (chrome/m156)
    #[must_use]
    pub fn next(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::Relaxed)
    }
}

/// `DrawAtlas::PlotEvictionCallback`: told whenever the atlas evicts a plot.
// Port of: src/gpu/graphite/DrawAtlas.h#L166-L172 (chrome/m156)
#[doc(alias = "DrawAtlas::PlotEvictionCallback")]
pub trait PlotEvictionCallback {
    /// `evict(PlotLocator)`.
    fn evict(&mut self, plot_locator: PlotLocator);
}

/// `DrawAtlas::AllowMultitexturing`.
// Port of: src/gpu/graphite/DrawAtlas.h#L180 (chrome/m156)
#[doc(alias = "DrawAtlas::AllowMultitexturing")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllowMultitexturing {
    /// One page only.
    No,
    /// Up to [`K_MAX_MULTITEXTURE_PAGES`] pages.
    Yes,
}

/// `DrawAtlas::UseStorageTextures`.
// Port of: src/gpu/graphite/DrawAtlas.h#L183 (chrome/m156)
#[doc(alias = "DrawAtlas::UseStorageTextures")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseStorageTextures {
    /// Sampled textures.
    No,
    /// Storage textures.
    Yes,
}

/// `DrawAtlas::ErrorCode`.
// Port of: src/gpu/graphite/DrawAtlas.h#L199-L203 (chrome/m156)
#[doc(alias = "DrawAtlas::ErrorCode")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    /// An unrecoverable error: the draw being created should be discarded.
    Error,
    /// The subimage was added.
    Succeeded,
    /// The subimage cannot fit without overwriting texels that current draws read: end the draw,
    /// snap a `DrawPass` and try again.
    TryAgain,
}

/// `DrawAtlas::PlotID`: a unique ID for the content of a plot, 0 is invalid.
// Port of: src/gpu/graphite/DrawAtlas.h#L81 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PlotId(u32);

impl PlotId {
    const INVALID: PlotId = PlotId(0);
}

/// `DrawAtlas::EntryID`: identifies one rectangle within a plot.
// Port of: src/gpu/graphite/DrawAtlas.h#L82-L86 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct EntryId(i32);

impl EntryId {
    const INVALID: EntryId = EntryId(0);
}

/// `DrawAtlas::Rect16`: a rectangle stored as four `uint16_t`s.
// Port of: src/gpu/graphite/DrawAtlas.h#L124-L140 (chrome/m156)
#[derive(Clone, Copy, Debug)]
struct Rect16 {
    l: u16,
    t: u16,
    r: u16,
    b: u16,
}

impl Rect16 {
    // Port of: src/gpu/graphite/DrawAtlas.h#L125-L134 (chrome/m156)
    fn from_irect(r: IRect) -> Self {
        Rect16 {
            l: r.left() as u16,
            t: r.top() as u16,
            r: r.right() as u16,
            b: r.bottom() as u16,
        }
    }

    // Port of: src/gpu/graphite/DrawAtlas.h#L135 (chrome/m156)
    fn to_irect(self) -> IRect {
        IRect::from_ltrb(
            i32::from(self.l),
            i32::from(self.t),
            i32::from(self.r),
            i32::from(self.b),
        )
    }
}

/// `DrawAtlas::Record`: which plot and entry an atlas entry lives in.
// Port of: src/gpu/graphite/DrawAtlas.h#L104-L122 (chrome/m156)
#[doc(alias = "DrawAtlas::Record")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    plot_id: PlotId,
    entry_id: EntryId,
}

impl Default for Record {
    // Port of: src/gpu/graphite/DrawAtlas.h#L113 (chrome/m156)
    fn default() -> Self {
        Record {
            plot_id: PlotId::INVALID,
            entry_id: EntryId::INVALID,
        }
    }
}

/// `DrawAtlas::PlotLocator`: a plot's page, index and generation. A locator whose plot has been
/// evicted no longer matches the plot's generation.
// Port of: src/gpu/graphite/DrawAtlas.h#L345-L385 (chrome/m156)
#[doc(alias = "DrawAtlas::PlotLocator")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlotLocator {
    // A 48-bit generation, a 8-bit plot index and a 8-bit page index.
    gen_id: u64,
    plot_index: u8,
    page_index: u8,
}

impl Default for PlotLocator {
    // Port of: src/gpu/graphite/DrawAtlas.h#L355 (chrome/m156)
    fn default() -> Self {
        PlotLocator {
            gen_id: GenerationCounter::INVALID_GENERATION,
            plot_index: 0,
            page_index: 0,
        }
    }
}

impl PlotLocator {
    /// `PlotLocator(pageIdx, plotIdx, generation)`.
    // Port of: src/gpu/graphite/DrawAtlas.h#L347-L352 (chrome/m156)
    #[must_use]
    pub fn new(page_idx: u32, plot_idx: u32, generation: u64) -> Self {
        debug_assert!(page_idx < K_MAX_MULTITEXTURE_PAGES);
        debug_assert!(plot_idx < K_MAX_PLOTS);
        debug_assert!(generation < (1 << 48));
        PlotLocator {
            gen_id: generation,
            plot_index: plot_idx as u8,
            page_index: page_idx as u8,
        }
    }

    /// `isValid()`.
    // Port of: src/gpu/graphite/DrawAtlas.h#L357-L360 (chrome/m156)
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.gen_id != GenerationCounter::INVALID_GENERATION
            || self.plot_index != 0
            || self.page_index != 0
    }

    /// `makeInvalid()`.
    // Port of: src/gpu/graphite/DrawAtlas.h#L361-L365 (chrome/m156)
    pub fn make_invalid(&mut self) {
        self.gen_id = GenerationCounter::INVALID_GENERATION;
        self.plot_index = 0;
        self.page_index = 0;
    }

    /// `pageIndex()`.
    #[must_use]
    pub fn page_index(&self) -> u32 {
        u32::from(self.page_index)
    }

    /// `plotIndex()`.
    #[must_use]
    pub fn plot_index(&self) -> u32 {
        u32::from(self.plot_index)
    }

    /// `genID()`.
    #[must_use]
    pub fn gen_id(&self) -> u64 {
        self.gen_id
    }
}

/// `DrawAtlas::AtlasLocator`: where one entry is in the atlas. The UV array holds the padded
/// bounds in its low 13 bits and the page index in bits 13 and 14.
// Port of: src/gpu/graphite/DrawAtlas.h#L387-L455 (chrome/m156)
#[doc(alias = "DrawAtlas::AtlasLocator")]
#[derive(Clone, Copy, Debug)]
pub struct AtlasLocator {
    plot_locator: PlotLocator,
    record: Record,
    uvs: [u16; 4],
}

impl Default for AtlasLocator {
    // Port of: src/gpu/graphite/DrawAtlas.h#L390 (chrome/m156)
    fn default() -> Self {
        AtlasLocator {
            plot_locator: PlotLocator::new(0, 0, GenerationCounter::INVALID_GENERATION),
            record: Record::default(),
            uvs: [0; 4],
        }
    }
}

impl AtlasLocator {
    /// `getUVs()`.
    #[must_use]
    pub fn uvs(&self) -> [u16; 4] {
        self.uvs
    }

    /// `invalidatePlotLocator()`.
    pub fn invalidate_plot_locator(&mut self) {
        self.plot_locator.make_invalid();
    }

    /// `plotLocator()`.
    #[must_use]
    pub fn plot_locator(&self) -> PlotLocator {
        self.plot_locator
    }

    /// `pageIndex()`.
    #[must_use]
    pub fn page_index(&self) -> u32 {
        self.plot_locator.page_index()
    }

    /// `plotIndex()`.
    #[must_use]
    pub fn plot_index(&self) -> u32 {
        self.plot_locator.plot_index()
    }

    /// `genID()`.
    #[must_use]
    pub fn gen_id(&self) -> u64 {
        self.plot_locator.gen_id()
    }

    /// `topLeft()`.
    #[must_use]
    pub fn top_left(&self) -> IPoint {
        IPoint::new(i32::from(self.uvs[0] & 0x1FFF), i32::from(self.uvs[1]))
    }

    /// `dimensions()`.
    #[must_use]
    pub fn dimensions(&self) -> ISize {
        ISize::new(
            i32::from(self.uvs[2]) - i32::from(self.uvs[0]),
            i32::from(self.uvs[3]) - i32::from(self.uvs[1]),
        )
    }

    /// `width()`: the `uint16_t` result of the subtraction, as in C++.
    #[must_use]
    pub fn width(&self) -> u16 {
        self.uvs[2].wrapping_sub(self.uvs[0])
    }

    /// `height()`.
    #[must_use]
    pub fn height(&self) -> u16 {
        self.uvs[3].wrapping_sub(self.uvs[1])
    }

    /// `insetSrc(padding)`.
    // Port of: src/gpu/graphite/DrawAtlas.h#L403-L411 (chrome/m156)
    pub fn inset_src(&mut self, padding: i32) {
        debug_assert!(2 * padding <= i32::from(self.width()));
        debug_assert!(2 * padding <= i32::from(self.height()));
        let p = padding as u16;
        self.uvs[0] = self.uvs[0].wrapping_add(p);
        self.uvs[1] = self.uvs[1].wrapping_add(p);
        self.uvs[2] = self.uvs[2].wrapping_sub(p);
        self.uvs[3] = self.uvs[3].wrapping_sub(p);
    }

    /// `updatePlotLocator(p)`.
    // Port of: src/gpu/graphite/DrawAtlas.h#L413-L419 (chrome/m156)
    pub fn update_plot_locator(&mut self, p: PlotLocator) {
        self.plot_locator = p;
        debug_assert!(self.plot_locator.page_index() <= 3);
        let page = (self.plot_locator.page_index() as u16) << 13;
        self.uvs[0] = (self.uvs[0] & 0x1FFF) | page;
        self.uvs[2] = (self.uvs[2] & 0x1FFF) | page;
    }

    /// `updateRect(rect)`.
    // Port of: src/gpu/graphite/DrawAtlas.h#L421-L427 (chrome/m156)
    pub fn update_rect(&mut self, rect: IRect) {
        debug_assert!(rect.left() <= rect.right());
        debug_assert!(rect.right() <= 0x1FFF);
        self.uvs[0] = (self.uvs[0] & 0xE000) | (rect.left() as u16);
        self.uvs[1] = rect.top() as u16;
        self.uvs[2] = (self.uvs[2] & 0xE000) | (rect.right() as u16);
        self.uvs[3] = rect.bottom() as u16;
    }

    /// `updateRecord(r)`.
    pub fn update_record(&mut self, r: Record) {
        self.record = r;
    }

    /// `record()`.
    #[must_use]
    pub fn record(&self) -> Record {
        self.record
    }
}

/// `DrawAtlas::BulkUsePlotUpdater`: collects the distinct plots a draw uses, so their last-use
/// token can be set in one pass.
// Port of: src/gpu/graphite/DrawAtlas.h#L457-L511 (chrome/m156)
#[doc(alias = "DrawAtlas::BulkUsePlotUpdater")]
#[derive(Clone, Debug, Default)]
pub struct BulkUsePlotUpdater {
    plots_to_update: Vec<PlotData>,
    plot_already_updated: [u32; K_MAX_MULTITEXTURE_PAGES as usize],
}

/// `DrawAtlas::BulkUsePlotUpdater::PlotData`.
#[derive(Clone, Copy, Debug)]
pub struct PlotData {
    /// `fPageIndex`.
    pub page_index: u32,
    /// `fPlotIndex`.
    pub plot_index: u32,
}

impl BulkUsePlotUpdater {
    /// `add(atlasLocator)`: returns `false` if the plot was already added.
    // Port of: src/gpu/graphite/DrawAtlas.h#L461-L470 (chrome/m156)
    pub fn add(&mut self, atlas_locator: &AtlasLocator) -> bool {
        let plot_idx = atlas_locator.plot_index();
        let page_idx = atlas_locator.page_index();
        if self.find(page_idx, plot_idx) {
            return false;
        }
        self.set(page_idx, plot_idx);
        true
    }

    /// `reset()`.
    pub fn reset(&mut self) {
        self.plots_to_update.clear();
        self.plot_already_updated = [0; K_MAX_MULTITEXTURE_PAGES as usize];
    }

    /// `count()`.
    #[must_use]
    pub fn count(&self) -> usize {
        self.plots_to_update.len()
    }

    /// `plotData(index)`.
    #[must_use]
    pub fn plot_data(&self, index: usize) -> PlotData {
        self.plots_to_update[index]
    }

    // Port of: src/gpu/graphite/DrawAtlas.h#L486-L489 (chrome/m156)
    fn find(&self, page_idx: u32, index: u32) -> bool {
        debug_assert!(index < K_MAX_PLOTS);
        (self.plot_already_updated[page_idx as usize] >> index) & 1 != 0
    }

    // Port of: src/gpu/graphite/DrawAtlas.h#L490-L496 (chrome/m156)
    fn set(&mut self, page_idx: u32, index: u32) {
        debug_assert!(!self.find(page_idx, index));
        self.plot_already_updated[page_idx as usize] |= 1 << index;
        self.plots_to_update.push(PlotData {
            page_index: page_idx,
            plot_index: index,
        });
    }
}

/// `DrawAtlas::Plot::PlotCoord`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PlotCoord {
    page_idx: u32,
    x: i32,
    y: i32,
}

/// `DrawAtlas::Plot::AddResult`.
struct AddResult {
    entry_id: EntryId,
    position_in_atlas: IPoint,
}

/// `DrawAtlas::Plot`: one rectangular region of a page, with its own rectanizer and CPU backing
/// store. The plots of a page are all in one MRU list (see [`Page`]).
// Port of: src/gpu/graphite/DrawAtlas.h#L513-L740 (chrome/m156)
// The field names mirror the C++ members (`fPlotID`, `fPlotIndex`, ...).
#[allow(clippy::struct_field_names)]
struct Plot {
    data: Option<Vec<u8>>,
    entries: HashMap<EntryId, Rect16>,
    rectanizer: RectanizerSkyline,
    last_use: Token,
    flushes_since_last_use: i32,
    plot_id: PlotId,
    prev_entry_id: EntryId,
    plot_dimensions: ISize,
    plot_index: u32,
    plot_coord: PlotCoord,
    mask_format: MaskFormat,
    dirty_rect: IRect,
    is_full: bool,
}

/// `kPlotID` source: a process-wide counter (as `Plot::NextPlotID`) so that a recycled plot never
/// reuses the generation of its previous content.
static NEXT_PLOT_ID: AtomicU32 = AtomicU32::new(1);

/// `DrawAtlas::Plot::NextPlotID()`.
// Port of: src/gpu/graphite/DrawAtlas.cpp#L~560 (chrome/m156)
fn next_plot_id() -> PlotId {
    loop {
        let id = NEXT_PLOT_ID.fetch_add(1, Ordering::Relaxed);
        if id != PlotId::INVALID.0 {
            return PlotId(id);
        }
    }
}

/// `DrawAtlas::Plot::NextEntryID(entryID)`.
// Port of: src/gpu/graphite/DrawAtlas.cpp#L~574 (chrome/m156)
fn next_entry_id(entry_id: EntryId) -> EntryId {
    let value = entry_id.0;
    if value == i32::MAX {
        return EntryId(1);
    }
    let value = value + 1;
    debug_assert!(value != EntryId::INVALID.0);
    EntryId(value)
}

/// `copy_pixels()`: copies `size` pixels of `bytes_per_pixel` bytes. For 4-byte pixels on a
/// BGRA-native build the channels are swapped to RGBA, as `SkOpts::RGBA_to_BGRA` does.
// Port of: src/gpu/graphite/DrawAtlas.cpp#L37-L60 (chrome/m156)
fn copy_pixels(
    dst: &mut [u8],
    dst_row_bytes: usize,
    src: &[u8],
    src_row_bytes: usize,
    size: ISize,
    bytes_per_pixel: usize,
) {
    // `kBGRAIsNative`.
    const BGRA_IS_NATIVE: bool = matches!(ColorType::N32, ColorType::BGRA8888);
    let width = size.width as usize;
    let height = size.height as usize;
    if bytes_per_pixel == 4 && BGRA_IS_NATIVE {
        for row in 0..height {
            let d = &mut dst[row * dst_row_bytes..row * dst_row_bytes + width * 4];
            let s = &src[row * src_row_bytes..row * src_row_bytes + width * 4];
            let (dp_px, _) = d.as_chunks_mut::<4>();
            let (sp_px, _) = s.as_chunks::<4>();
            for (dp, sp) in dp_px.iter_mut().zip(sp_px) {
                dp[0] = sp[2];
                dp[1] = sp[1];
                dp[2] = sp[0];
                dp[3] = sp[3];
            }
        }
    } else {
        for row in 0..height {
            let d = &mut dst[row * dst_row_bytes..row * dst_row_bytes + src_row_bytes];
            d.copy_from_slice(&src[row * src_row_bytes..row * src_row_bytes + src_row_bytes]);
        }
    }
}

impl Plot {
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~537-L553 (chrome/m156)
    fn new(
        plot_coord: PlotCoord,
        plot_index: u32,
        plot_dimensions: ISize,
        mask_format: MaskFormat,
    ) -> Self {
        let plot = Plot {
            data: None,
            entries: HashMap::new(),
            rectanizer: RectanizerSkyline::new(plot_dimensions.width, plot_dimensions.height),
            last_use: Token::invalid_token(),
            flushes_since_last_use: 0,
            plot_id: next_plot_id(),
            prev_entry_id: EntryId::INVALID,
            plot_dimensions,
            plot_index,
            plot_coord,
            mask_format,
            dirty_rect: IRect::new_empty(),
            is_full: false,
        };
        debug_assert!((plot_dimensions.width as usize * plot.bpp()).is_multiple_of(4));
        debug_assert!(matches!(plot.bpp(), 1 | 2 | 4));
        plot
    }

    /// `bpp()`.
    fn bpp(&self) -> usize {
        self.mask_format.bytes_per_pixel()
    }

    /// `rowBytes()`.
    fn row_bytes(&self) -> usize {
        self.plot_dimensions.width as usize * self.bpp()
    }

    /// `plotLocator()`.
    fn plot_locator(&self) -> PlotLocator {
        PlotLocator::new(
            self.plot_coord.page_idx,
            self.plot_index,
            u64::from(self.plot_id.0),
        )
    }

    /// `topLeftInAtlas()`.
    fn top_left_in_atlas(&self) -> IPoint {
        IPoint::new(
            self.plot_coord.x * self.plot_dimensions.width,
            self.plot_coord.y * self.plot_dimensions.height,
        )
    }

    /// `isEmpty()`: no rectangle has been packed into the plot.
    fn is_empty(&self) -> bool {
        self.rectanizer.percent_full() == 0.0
    }

    /// `hasAllocation()`.
    fn has_allocation(&self) -> bool {
        self.data.is_some()
    }

    /// `needsUpload()`.
    fn needs_upload(&self) -> bool {
        !self.dirty_rect.is_empty()
    }

    /// `markFullIfUsed()`.
    fn mark_full_if_used(&mut self) {
        self.is_full = !self.dirty_rect.is_empty();
    }

    /// `makeEntry(size)`: packs a rectangle into the plot and records its entry.
    // Port of: src/gpu/graphite/DrawAtlas.h#L684-L694 (chrome/m156)
    fn make_entry(&mut self, width: i32, height: i32) -> Option<(EntryId, IPoint)> {
        let loc = self.rectanizer.add_rect(width, height)?;
        self.prev_entry_id = next_entry_id(self.prev_entry_id);
        debug_assert!(!self.entries.contains_key(&self.prev_entry_id));
        let rect = IRect::from_xywh(i32::from(loc.x), i32::from(loc.y), width, height);
        self.entries
            .insert(self.prev_entry_id, Rect16::from_irect(rect));
        Some((
            self.prev_entry_id,
            IPoint::new(i32::from(loc.x), i32::from(loc.y)),
        ))
    }

    /// `dataAt(localAtlasPoint)`: the byte offset into the backing store, allocating it (zeroed)
    /// on first use.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~650-L662 (chrome/m156)
    fn data_at(&mut self, local: IPoint) -> usize {
        let bpp = self.bpp();
        let len = bpp * (self.plot_dimensions.width * self.plot_dimensions.height) as usize;
        let width = self.plot_dimensions.width;
        let height = self.plot_dimensions.height;
        self.data.get_or_insert_with(|| vec![0; len]);
        debug_assert!(local.x >= 0 && local.x < width);
        debug_assert!(local.y >= 0 && local.y < height);
        bpp * (local.y as usize * width as usize + local.x as usize)
    }

    /// `addRect(size, image)`: packs a rectangle, copying `image` into it if one is given.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~587-L603 (chrome/m156)
    fn add_rect_image(&mut self, size: ISize, image: Option<&[u8]>) -> Option<AddResult> {
        let (entry_id, local_pos) = self.make_entry(size.width, size.height)?;
        let abs_pos = IPoint::new(
            local_pos.x + self.top_left_in_atlas().x,
            local_pos.y + self.top_left_in_atlas().y,
        );
        if let Some(image) = image {
            let row_bytes = self.row_bytes();
            let bpp = self.bpp();
            let off = self.data_at(local_pos);
            let data = self
                .data
                .as_mut()
                .expect("data_at allocated the backing store");
            copy_pixels(
                &mut data[off..],
                row_bytes,
                image,
                size.width as usize * bpp,
                size,
                bpp,
            );
        }
        let local_rect = IRect::from_xywh(local_pos.x, local_pos.y, size.width, size.height);
        self.dirty_rect = IRect::join(&self.dirty_rect, &local_rect);
        Some(AddResult {
            entry_id,
            position_in_atlas: abs_pos,
        })
    }

    /// `addRect(width, height, atlasLocator)`: packs a rectangle and points `atlas_locator` at it.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~605-L617 (chrome/m156)
    fn add_rect(&mut self, width: i32, height: i32, atlas_locator: &mut AtlasLocator) -> bool {
        let Some(res) = self.add_rect_image(ISize::new(width, height), None) else {
            return false;
        };
        let rect = IRect::from_xywh(
            res.position_in_atlas.x,
            res.position_in_atlas.y,
            width,
            height,
        );
        atlas_locator.update_rect(rect);
        atlas_locator.update_plot_locator(self.plot_locator());
        atlas_locator.update_record(Record {
            plot_id: self.plot_id,
            entry_id: res.entry_id,
        });
        true
    }

    /// `copySubImage(atlasLocator, image)`.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~739-L749 (chrome/m156)
    fn copy_sub_image(&mut self, al: &AtlasLocator, image: &[u8]) {
        let top_left = self.top_left_in_atlas();
        let local_pos = IPoint::new(al.top_left().x - top_left.x, al.top_left().y - top_left.y);
        let size = ISize::new(i32::from(al.width()), i32::from(al.height()));
        let row_bytes = self.row_bytes();
        let bpp = self.bpp();
        let off = self.data_at(local_pos);
        let data = self
            .data
            .as_mut()
            .expect("data_at allocated the backing store");
        copy_pixels(
            &mut data[off..],
            row_bytes,
            image,
            size.width as usize * bpp,
            size,
            bpp,
        );
        let local_rect = IRect::from_xywh(local_pos.x, local_pos.y, size.width, size.height);
        self.dirty_rect = IRect::join(&self.dirty_rect, &local_rect);
    }

    /// `prepareForUpload()`: the byte offset of the aligned dirty rect and the rect in the atlas
    /// texture, or `None` if the plot has no backing store.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~751-L765 (chrome/m156)
    fn prepare_for_upload(&mut self) -> (Option<usize>, IRect) {
        debug_assert!(!self.dirty_rect.is_empty());
        if self.data.is_none() {
            return (None, IRect::new_empty());
        }
        let aligned = self.aligned_dirty_rect();
        let bpp = self.bpp();
        let row_bytes = self.row_bytes();
        let offset = row_bytes * aligned.top() as usize + bpp * aligned.left() as usize;
        let top_left = self.top_left_in_atlas();
        let offset_rect = IRect::from_ltrb(
            aligned.left() + top_left.x,
            aligned.top() + top_left.y,
            aligned.right() + top_left.x,
            aligned.bottom() + top_left.y,
        );
        self.dirty_rect = IRect::new_empty();
        self.is_full = false;
        (Some(offset), offset_rect)
    }

    /// `alignedDirtyRect()`: the dirty rect widened to 4-byte boundaries horizontally.
    // Port of: src/gpu/graphite/DrawAtlas.h#L689-L694 (chrome/m156)
    fn aligned_dirty_rect(&self) -> IRect {
        let d = self.dirty_rect;
        IRect::from_ltrb(d.left() & !0x3, d.top(), (d.right() + 3) & !0x3, d.bottom())
    }

    /// `recycle(freeData)`: empties the plot, keeping its backing store unless `free_data`.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~800-L815 (chrome/m156)
    fn recycle(&mut self, free_data: bool) {
        self.entries.clear();
        self.rectanizer.reset();
        self.plot_id = next_plot_id();
        self.last_use = Token::invalid_token();
        self.flushes_since_last_use = 0;
        self.dirty_rect = IRect::new_empty();
        self.is_full = false;
        if free_data {
            self.data = None;
        } else {
            let n = self.row_bytes() * self.plot_dimensions.height as usize;
            if let Some(data) = self.data.as_mut() {
                data[..n].fill(0);
            }
        }
    }
}

/// `DrawAtlas::Page`: a page's plots, and their MRU list. The list holds plot indices with the
/// most recently used at the front (C++ `fPlotList` head) and the least recently used at the back
/// (the tail), so the eviction order matches the C++ `SkTInternalLList`.
// Port of: src/gpu/graphite/DrawAtlas.h#L735-L741 (chrome/m156)
#[derive(Default)]
struct Page {
    plots: Vec<Plot>,
    plot_list: Vec<usize>,
}

impl Page {
    /// `fPlotList.head()`.
    fn head(&self) -> usize {
        self.plot_list[0]
    }

    /// `fPlotList.tail()`.
    fn tail(&self) -> usize {
        *self.plot_list.last().expect("a page has plots")
    }

    /// `makeMRU`: moves `plot_idx` to the front of the list.
    // Port of: src/gpu/graphite/DrawAtlas.h#L168-L177 (chrome/m156)
    fn make_mru(&mut self, plot_idx: usize) {
        if self.plot_list[0] == plot_idx {
            return;
        }
        let pos = self
            .plot_list
            .iter()
            .position(|&p| p == plot_idx)
            .expect("the plot is on its page's list");
        self.plot_list.remove(pos);
        self.plot_list.insert(0, plot_idx);
    }
}

/// `DrawAtlas`: a texture atlas of up to [`K_MAX_MULTITEXTURE_PAGES`] pages, each a grid of
/// plots. Entries are packed into plots; plots are evicted least-recently-used once their last
/// use has flushed.
// Port of: src/gpu/graphite/DrawAtlas.h#L72-L455 (chrome/m156)
pub struct DrawAtlas {
    mask_format: MaskFormat,
    texture_width: i32,
    texture_height: i32,
    plot_width: i32,
    plot_height: i32,
    num_plots: u32,
    use_storage_textures: UseStorageTextures,
    label: String,
    atlas_id: u32,
    generation_counter: GenerationCounter,
    atlas_generation: u64,
    prev_flush_token: Token,
    flushes_since_last_use: i32,
    evictor: Option<Box<dyn PlotEvictionCallback + Send>>,
    proxies: [Option<Arc<TextureProxy>>; K_MAX_MULTITEXTURE_PAGES as usize],
    pages: [Page; K_MAX_MULTITEXTURE_PAGES as usize],
    max_pages: u32,
    num_active_pages: u32,
}

impl std::fmt::Debug for DrawAtlas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DrawAtlas")
            .field("label", &self.label)
            .field("mask_format", &self.mask_format)
            .field("atlas_id", &self.atlas_id)
            .field("num_active_pages", &self.num_active_pages)
            .field("max_pages", &self.max_pages)
            .finish_non_exhaustive()
    }
}

/// `DrawAtlas::next_id()`: a process-wide unique atlas ID, never `SK_InvalidGenID`.
static NEXT_ATLAS_ID: AtomicU32 = AtomicU32::new(1);

// Port of: src/gpu/graphite/DrawAtlas.cpp#L~548-L556 (chrome/m156)
fn next_atlas_id() -> u32 {
    loop {
        let id = NEXT_ATLAS_ID.fetch_add(1, Ordering::Relaxed);
        if id != SK_INVALID_GEN_ID {
            return id;
        }
    }
}

impl DrawAtlas {
    /// `DrawAtlas::Make()`: creates an atlas. `evictor`, if given, is told about each plot the
    /// atlas evicts. The dimensions must tile exactly into plots.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~86-L103 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors DrawAtlas::Make()
    #[must_use]
    #[doc(alias = "Make")]
    pub fn make(
        mask_format: MaskFormat,
        width: i32,
        height: i32,
        plot_width: i32,
        plot_height: i32,
        generation_counter: &GenerationCounter,
        allow_multitexturing: AllowMultitexturing,
        use_storage_textures: UseStorageTextures,
        evictor: Option<Box<dyn PlotEvictionCallback + Send>>,
        label: &str,
    ) -> Box<DrawAtlas> {
        let mut atlas = Box::new(DrawAtlas::new(
            mask_format,
            width,
            height,
            plot_width,
            plot_height,
            generation_counter,
            allow_multitexturing,
            use_storage_textures,
            label,
        ));
        atlas.evictor = evictor;
        atlas
    }

    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~105-L135 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the DrawAtlas constructor
    fn new(
        mask_format: MaskFormat,
        width: i32,
        height: i32,
        plot_width: i32,
        plot_height: i32,
        generation_counter: &GenerationCounter,
        allow_multitexturing: AllowMultitexturing,
        use_storage_textures: UseStorageTextures,
        label: &str,
    ) -> DrawAtlas {
        let num_plots_x = width / plot_width;
        let num_plots_y = height / plot_height;
        debug_assert!(num_plots_x * num_plots_y <= K_MAX_PLOTS.cast_signed());
        debug_assert!(
            plot_width * num_plots_x == width,
            "Invalid DrawAtlas: plot width"
        );
        debug_assert!(
            plot_height * num_plots_y == height,
            "Invalid DrawAtlas: plot height"
        );
        let mut atlas = DrawAtlas {
            mask_format,
            texture_width: width,
            texture_height: height,
            plot_width,
            plot_height,
            num_plots: (num_plots_x * num_plots_y) as u32,
            use_storage_textures,
            label: label.to_owned(),
            atlas_id: next_atlas_id(),
            generation_counter: generation_counter.clone(),
            atlas_generation: generation_counter.next(),
            prev_flush_token: Token::invalid_token(),
            flushes_since_last_use: 0,
            evictor: None,
            proxies: [None, None, None, None],
            pages: Default::default(),
            max_pages: if allow_multitexturing == AllowMultitexturing::Yes {
                K_MAX_MULTITEXTURE_PAGES
            } else {
                1
            },
            num_active_pages: 0,
        };
        atlas.create_pages();
        atlas
    }

    /// `createPages()`: every page starts with all its plots empty, the list head being the last
    /// plot index (as in the C++ `addToHead` loop).
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~443-L470 (chrome/m156)
    fn create_pages(&mut self) {
        let num_plots_x = self.texture_width / self.plot_width;
        let num_plots_y = self.texture_height / self.plot_height;
        let n = (num_plots_x * num_plots_y) as usize;
        for i in 0..self.max_pages {
            self.proxies[i as usize] = None;
            let mut plots = Vec::with_capacity(n);
            for r in 0..num_plots_y {
                let y = num_plots_y - 1 - r;
                for c in 0..num_plots_x {
                    let x = num_plots_x - 1 - c;
                    let plot_index = (r * num_plots_x + c) as u32;
                    plots.push(Plot::new(
                        PlotCoord { page_idx: i, x, y },
                        plot_index,
                        ISize::new(self.plot_width, self.plot_height),
                        self.mask_format,
                    ));
                }
            }
            let page = &mut self.pages[i as usize];
            page.plots = plots;
            page.plot_list = (0..n).rev().collect();
        }
    }

    /// `getListIndex(locator)`.
    // Port of: src/gpu/graphite/DrawAtlas.h#L531-L533 (chrome/m156)
    #[must_use]
    pub fn get_list_index(&self, locator: &PlotLocator) -> u32 {
        locator.page_index() * self.num_plots + locator.plot_index()
    }

    /// `hasID(plotLocator)`: whether the locator still names the plot's current content.
    // Port of: src/gpu/graphite/DrawAtlas.h#L535-L545 (chrome/m156)
    #[must_use]
    pub fn has_id(&self, plot_locator: &PlotLocator) -> bool {
        if !plot_locator.is_valid() {
            return false;
        }
        let plot = plot_locator.plot_index();
        let page = plot_locator.page_index();
        let plot_generation = u64::from(self.pages[page as usize].plots[plot as usize].plot_id.0);
        let locator_generation = plot_locator.gen_id();
        plot < self.num_plots
            && page < self.num_active_pages
            && plot_generation == locator_generation
    }

    /// `findPlot(atlasLocator)`.
    // Port of: src/gpu/graphite/DrawAtlas.h#L632-L637 (chrome/m156)
    fn find_plot_mut(&mut self, atlas_locator: &AtlasLocator) -> &mut Plot {
        debug_assert!(self.has_id(&atlas_locator.plot_locator()));
        let page_idx = atlas_locator.page_index() as usize;
        let plot_idx = atlas_locator.plot_index() as usize;
        &mut self.pages[page_idx].plots[plot_idx]
    }

    /// `makeMRU(plot, pageIdx)`, addressed by page and plot index.
    fn make_mru(&mut self, page_idx: u32, plot_idx: usize) {
        self.pages[page_idx as usize].make_mru(plot_idx);
    }

    /// `updatePlot(plot, atlasLocator)`.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~183-L188 (chrome/m156)
    fn update_plot(&mut self, page_idx: u32, plot_idx: usize, atlas_locator: &mut AtlasLocator) {
        self.make_mru(page_idx, plot_idx);
        let locator = self.pages[page_idx as usize].plots[plot_idx].plot_locator();
        atlas_locator.update_plot_locator(locator);
    }

    /// `addRectToPage(pageIdx, width, height, atlasLocator)`.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~190-L201 (chrome/m156)
    fn add_rect_to_page(
        &mut self,
        page_idx: u32,
        width: i32,
        height: i32,
        atlas_locator: &mut AtlasLocator,
    ) -> bool {
        debug_assert!(self.proxies[page_idx as usize].is_some());
        let len = self.pages[page_idx as usize].plot_list.len();
        for i in 0..len {
            let plot_idx = self.pages[page_idx as usize].plot_list[i];
            if self.pages[page_idx as usize].plots[plot_idx].add_rect(width, height, atlas_locator)
            {
                self.update_plot(page_idx, plot_idx, atlas_locator);
                return true;
            }
        }
        false
    }

    /// `processEvictionAndResetRects(plot, freeData)`.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~171-L181 (chrome/m156)
    fn process_eviction_and_reset_rects(
        &mut self,
        page_idx: u32,
        plot_idx: usize,
        free_data: bool,
    ) {
        let (locator, has_content) = {
            let plot = &self.pages[page_idx as usize].plots[plot_idx];
            (plot.plot_locator(), !plot.is_empty())
        };
        if has_content {
            if let Some(evictor) = self.evictor.as_mut() {
                evictor.evict(locator);
            }
            self.atlas_generation = self.generation_counter.next();
        }
        self.pages[page_idx as usize].plots[plot_idx].recycle(free_data);
    }

    /// `addRect(recorder, width, height, atlasLocator)`: packs a rectangle of `width` by `height`
    /// into the atlas, evicting an unused plot if every active page is full.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~214-L271 (chrome/m156)
    pub fn add_rect(
        &mut self,
        recorder: &Recorder,
        width: i32,
        height: i32,
        atlas_locator: &mut AtlasLocator,
    ) -> ErrorCode {
        if width > self.plot_width || height > self.plot_height || width < 0 || height < 0 {
            return ErrorCode::Error;
        }

        if width == 0 || height == 0 {
            if self.num_active_pages == 0 {
                self.activate_new_page(recorder);
            }
            atlas_locator.update_rect(IRect::new_empty());
            let head = self.pages[0].head();
            let locator = self.pages[0].plots[head].plot_locator();
            atlas_locator.update_plot_locator(locator);
            return ErrorCode::Succeeded;
        }

        for page_idx in 0..self.num_active_pages {
            if self.add_rect_to_page(page_idx, width, height, atlas_locator) {
                return ErrorCode::Succeeded;
            }
        }

        if self.num_active_pages == self.max_pages {
            let next_flush = recorder.priv_().token_tracker().borrow().next_flush_token();
            for page_idx in 0..self.num_active_pages {
                let plot_idx = self.pages[page_idx as usize].tail();
                let last_use = self.pages[page_idx as usize].plots[plot_idx].last_use;
                if last_use < next_flush {
                    self.process_eviction_and_reset_rects(page_idx, plot_idx, false);
                    let verify = self.pages[page_idx as usize].plots[plot_idx].add_rect(
                        width,
                        height,
                        atlas_locator,
                    );
                    debug_assert!(verify);
                    self.update_plot(page_idx, plot_idx, atlas_locator);
                    return ErrorCode::Succeeded;
                }
            }
        } else {
            if !self.activate_new_page(recorder) {
                return ErrorCode::Error;
            }
            if self.add_rect_to_page(self.num_active_pages - 1, width, height, atlas_locator) {
                return ErrorCode::Succeeded;
            }
            return ErrorCode::Error;
        }

        if self.num_active_pages == 0 {
            return ErrorCode::Error;
        }
        ErrorCode::TryAgain
    }

    /// `addToAtlas(recorder, width, height, image, atlasLocator)`: packs a `width` by `height`
    /// image, which is tightly packed in `image`, and copies it in.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~273-L283 (chrome/m156)
    pub fn add_to_atlas(
        &mut self,
        recorder: &Recorder,
        width: i32,
        height: i32,
        image: &[u8],
        atlas_locator: &mut AtlasLocator,
    ) -> ErrorCode {
        let ec = self.add_rect(recorder, width, height, atlas_locator);
        if ec == ErrorCode::Succeeded {
            let plot = self.find_plot_mut(atlas_locator);
            plot.copy_sub_image(atlas_locator, image);
        }
        ec
    }

    /// `prepForRender(locator, padding, initialColor)`: the writable region of the plot backing
    /// `locator`, inset by `padding` (and cleared to `initial_color` if given). `None` is the
    /// empty pixmap of the C++ code.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~285-L289 (chrome/m156)
    pub fn prep_for_render(
        &mut self,
        locator: &AtlasLocator,
        padding: i32,
        initial_color: Option<u32>,
    ) -> Option<Pixmap<'_>> {
        let plot = self.find_plot_mut(locator);
        plot.prep_for_render(locator, padding, initial_color)
    }

    /// `recordUploads(dc, recorder)`: records an upload of every plot changed since its last
    /// upload.
    ///
    /// # Panics
    /// If a plot with dirty pixels has no backing store, which `addRect` never leaves behind.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~203-L240 (chrome/m156)
    pub fn record_uploads(&mut self, dc: &mut DrawContext, recorder: &Recorder) -> bool {
        let mask_ct = self.mask_format.to_color_type();
        let color_info = ColorInfo::new(mask_ct, AlphaType::Unknown, None);
        let rp = recorder.priv_();
        let caps = rp.caps().clone();
        for page_idx in 0..self.num_active_pages {
            let page_idx_usize = page_idx as usize;
            let Some(proxy) = self.proxies[page_idx_usize].clone() else {
                continue;
            };
            let read_swizzle = read_swizzle_for_color_type(mask_ct, proxy.format());
            let view = TextureProxyView::new(Some(proxy), read_swizzle);
            let len = self.pages[page_idx_usize].plot_list.len();
            for i in 0..len {
                let plot_idx = self.pages[page_idx_usize].plot_list[i];
                let plot = &mut self.pages[page_idx_usize].plots[plot_idx];
                if !plot.needs_upload() {
                    continue;
                }
                let (offset, dst_rect) = plot.prepare_for_upload();
                if dst_rect.is_empty() {
                    continue;
                }
                let row_bytes = plot.row_bytes();
                let data = plot
                    .data
                    .as_ref()
                    .expect("a plot with dirty pixels has a backing store");
                let level = MipLevel {
                    pixels: offset.map(|o| &data[o..]),
                    row_bytes,
                };
                let source = UploadSource::make(
                    caps.as_ref(),
                    &view,
                    &color_info,
                    &color_info,
                    std::slice::from_ref(&level),
                    dst_rect,
                );
                if !dc.record_upload(&rp, &source, None) {
                    return false;
                }
            }
        }
        true
    }

    /// `getProxies()`: the texture of each active page (`None` for inactive pages).
    #[must_use]
    pub fn get_proxies(&self) -> &[Option<Arc<TextureProxy>>] {
        &self.proxies
    }

    /// `atlasID()`.
    #[must_use]
    pub fn atlas_id(&self) -> u32 {
        self.atlas_id
    }

    /// `atlasGeneration()`.
    #[must_use]
    pub fn atlas_generation(&self) -> u64 {
        self.atlas_generation
    }

    /// `numActivePages()`.
    #[must_use]
    pub fn num_active_pages(&self) -> u32 {
        self.num_active_pages
    }

    /// `numPlots()`.
    #[must_use]
    pub fn num_plots(&self) -> u32 {
        self.num_plots
    }

    /// `plotSize()`.
    #[must_use]
    pub fn plot_size(&self) -> ISize {
        ISize::new(self.plot_width, self.plot_height)
    }

    /// `maxPages()`.
    #[must_use]
    pub fn max_pages(&self) -> u32 {
        self.max_pages
    }

    /// `setLastUseToken(atlasLocator, token)`: keeps the entry's plot from being evicted until
    /// `token` has flushed.
    // Port of: src/gpu/graphite/DrawAtlas.h#L547-L550 (chrome/m156)
    pub fn set_last_use_token(&mut self, atlas_locator: &AtlasLocator, token: Token) {
        let plot_idx = atlas_locator.plot_index() as usize;
        self.internal_set_last_use_token(atlas_locator.page_index(), plot_idx, token);
    }

    /// `setLastUseTokenBulk(updater, token)`.
    // Port of: src/gpu/graphite/DrawAtlas.h#L551-L560 (chrome/m156)
    pub fn set_last_use_token_bulk(&mut self, updater: &BulkUsePlotUpdater, token: Token) {
        for i in 0..updater.count() {
            let pd = updater.plot_data(i);
            if pd.page_index < self.num_active_pages {
                self.internal_set_last_use_token(pd.page_index, pd.plot_index as usize, token);
            }
        }
    }

    // Port of: src/gpu/graphite/DrawAtlas.h#L623-L627 (chrome/m156)
    fn internal_set_last_use_token(&mut self, page_idx: u32, plot_idx: usize, token: Token) {
        self.make_mru(page_idx, plot_idx);
        self.pages[page_idx as usize].plots[plot_idx].last_use = token;
    }

    /// `activateNewPage(recorder)`: creates the texture of the next page.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~479-L500 (chrome/m156)
    fn activate_new_page(&mut self, recorder: &Recorder) -> bool {
        debug_assert!(self.num_active_pages < self.max_pages);
        debug_assert!(self.proxies[self.num_active_pages as usize].is_none());
        let ct = self.mask_format.to_color_type();
        let rp = recorder.priv_();
        let caps = rp.caps();
        let texture_info = if self.use_storage_textures == UseStorageTextures::Yes {
            caps.get_default_storage_texture_info(ct)
        } else {
            caps.get_default_sampled_texture_info(
                ct,
                Mipmapped::No,
                rp.is_protected(),
                Renderable::No,
            )
        };
        let proxy = {
            let mut resource_provider = rp
                .resource_provider()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            TextureProxy::make(
                caps.as_ref(),
                &mut resource_provider,
                ISize::new(self.texture_width, self.texture_height),
                &texture_info,
                Budgeted::Yes,
                &self.label,
            )
        };
        let Some(proxy) = proxy else {
            return false;
        };
        self.proxies[self.num_active_pages as usize] = Some(proxy);
        self.num_active_pages += 1;
        true
    }

    /// `deactivateLastPage()`: releases the texture of the last active page and recycles all its
    /// plots, in the same order as the C++ code.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~502-L525 (chrome/m156)
    fn deactivate_last_page(&mut self) {
        debug_assert!(self.num_active_pages > 0);
        let last_page = self.num_active_pages - 1;
        self.pages[last_page as usize].plot_list.clear();
        for plot_idx in 0..self.num_plots as usize {
            self.process_eviction_and_reset_rects(last_page, plot_idx, true);
            self.pages[last_page as usize].plots[plot_idx].flushes_since_last_use = 0;
            self.pages[last_page as usize].plot_list.insert(0, plot_idx);
        }
        self.proxies[last_page as usize] = None;
        self.num_active_pages -= 1;
    }

    /// `compact(startTokenForNextFlush)`: ages the plots by flush, evicts the plots that have not
    /// been used for too long, and releases pages that have no plots in use.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~318-L405 (chrome/m156)
    pub fn compact(&mut self, start_token_for_next_flush: Token) {
        let prev = self.prev_flush_token;
        let start = start_token_for_next_flush;
        if self.num_active_pages < 1 {
            self.prev_flush_token = start;
            return;
        }

        let mut atlas_used_this_flush = false;
        for page_idx in 0..self.num_active_pages as usize {
            let len = self.pages[page_idx].plot_list.len();
            for i in 0..len {
                let plot_idx = self.pages[page_idx].plot_list[i];
                let plot = &mut self.pages[page_idx].plots[plot_idx];
                if plot.last_use.in_interval(&prev, &start) {
                    plot.flushes_since_last_use = 0;
                    atlas_used_this_flush = true;
                }
            }
        }

        if atlas_used_this_flush {
            self.flushes_since_last_use = 0;
        } else {
            self.flushes_since_last_use += 1;
        }

        if atlas_used_this_flush || self.flushes_since_last_use > K_ATLAS_RECENTLY_USED_COUNT {
            // (page index, plot index) pairs, in the order the C++ `TArray<Plot*>` holds them.
            let mut available_plots: Vec<(u32, usize)> = Vec::new();
            let last_page_index = self.num_active_pages - 1;
            for page_idx in 0..last_page_index {
                let len = self.pages[page_idx as usize].plot_list.len();
                for i in 0..len {
                    let plot_idx = self.pages[page_idx as usize].plot_list[i];
                    let plot = &mut self.pages[page_idx as usize].plots[plot_idx];
                    if !plot.last_use.in_interval(&prev, &start) {
                        plot.flushes_since_last_use += 1;
                    }
                    if plot.flushes_since_last_use > K_PLOT_RECENTLY_USED_COUNT {
                        available_plots.push((page_idx, plot_idx));
                    }
                }
            }

            let mut used_plots: u32 = 0;
            let len = self.pages[last_page_index as usize].plot_list.len();
            for i in 0..len {
                let plot_idx = self.pages[last_page_index as usize].plot_list[i];
                let (flushes, last_use) = {
                    let plot = &mut self.pages[last_page_index as usize].plots[plot_idx];
                    if !plot.last_use.in_interval(&prev, &start) {
                        plot.flushes_since_last_use += 1;
                    }
                    (plot.flushes_since_last_use, plot.last_use)
                };
                if flushes <= K_PLOT_RECENTLY_USED_COUNT {
                    used_plots += 1;
                } else if last_use != Token::invalid_token() {
                    self.process_eviction_and_reset_rects(last_page_index, plot_idx, false);
                }
            }

            if !available_plots.is_empty() && used_plots != 0 && used_plots <= self.num_plots / 4 {
                let len = self.pages[last_page_index as usize].plot_list.len();
                for i in 0..len {
                    let plot_idx = self.pages[last_page_index as usize].plot_list[i];
                    let plot_flushes =
                        self.pages[last_page_index as usize].plots[plot_idx].flushes_since_last_use;
                    if (K_PLOT_USED_COUNT_BEFORE_EVICT..=K_PLOT_RECENTLY_USED_COUNT)
                        .contains(&plot_flushes)
                    {
                        if let Some((avail_page, avail_plot)) = available_plots.pop() {
                            self.process_eviction_and_reset_rects(last_page_index, plot_idx, true);
                            self.process_eviction_and_reset_rects(avail_page, avail_plot, false);
                            used_plots -= 1;
                        }
                        if used_plots == 0 || available_plots.is_empty() {
                            break;
                        }
                    }
                }
            }

            if used_plots == 0 {
                self.deactivate_last_page();
                self.flushes_since_last_use = 0;
            }
        }

        self.prev_flush_token = start;
    }

    /// `markUsedPlotsAsFull()`.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~407-L417 (chrome/m156)
    pub fn mark_used_plots_as_full(&mut self) {
        for page_idx in 0..self.num_active_pages as usize {
            for plot in &mut self.pages[page_idx].plots {
                plot.mark_full_if_used();
            }
        }
    }

    /// `freeGpuResources(token)`: releases the trailing pages that have no plot used since the
    /// previous flush up to `token`.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~572-L588 (chrome/m156)
    pub fn free_gpu_resources(&mut self, token: Token) {
        let prev = self.prev_flush_token;
        let mut page_index = i64::from(self.num_active_pages) - 1;
        while page_index >= 0 {
            let page_idx = page_index as usize;
            let len = self.pages[page_idx].plot_list.len();
            for i in 0..len {
                let plot_idx = self.pages[page_idx].plot_list[i];
                if self.pages[page_idx].plots[plot_idx]
                    .last_use
                    .in_interval(&prev, &token)
                {
                    return;
                }
            }
            self.deactivate_last_page();
            page_index -= 1;
        }
    }

    /// `evictAllPlots()`.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~590-L600 (chrome/m156)
    pub fn evict_all_plots(&mut self) {
        for page_idx in 0..self.num_active_pages {
            let len = self.pages[page_idx as usize].plot_list.len();
            for i in 0..len {
                let plot_idx = self.pages[page_idx as usize].plot_list[i];
                self.process_eviction_and_reset_rects(page_idx, plot_idx, true);
            }
        }
    }

    /// `numAllocatedPlots()`: plots with a backing store (`GPU_TEST_UTILS`).
    #[must_use]
    pub fn num_allocated_plots(&self) -> u32 {
        self.iterate_plots(Plot::has_allocation)
    }

    /// `numNonEmptyPlots()`: plots with at least one entry (`GPU_TEST_UTILS`).
    #[must_use]
    pub fn num_non_empty_plots(&self) -> u32 {
        self.iterate_plots(|plot| !plot.is_empty())
    }

    // Port of: src/gpu/graphite/DrawAtlas.h#L668-L684 (chrome/m156)
    fn iterate_plots(&self, pred: impl Fn(&Plot) -> bool) -> u32 {
        let mut count = 0;
        for page in &self.pages[..self.max_pages as usize] {
            for plot in &page.plots {
                if pred(plot) {
                    count += 1;
                }
            }
        }
        count
    }
}

impl Plot {
    /// `prepForRender(locator, padding, initialColor)`.
    // Port of: src/gpu/graphite/DrawAtlas.cpp#L~291-L310 (chrome/m156)
    fn prep_for_render(
        &mut self,
        al: &AtlasLocator,
        padding: i32,
        initial_color: Option<u32>,
    ) -> Option<Pixmap<'_>> {
        let r = al.record();
        if r.plot_id != PlotId::INVALID
            && r.entry_id != EntryId::INVALID
            && let Some(rect) = self.entries.get(&r.entry_id).copied().map(Rect16::to_irect)
        {
            // `entryPixmap()`: the entry's region is cleared, and used if non-empty.
            let off = self.data_at(rect.top_left());
            self.clear_outer(off, rect.size(), initial_color);
            if inner_is_non_empty(rect.size(), padding) {
                return self.inner_pixmap(off, rect.size(), padding);
            }
        }
        debug_assert!(padding >= 0);
        let local = IPoint::new(
            al.top_left().x - self.top_left_in_atlas().x,
            al.top_left().y - self.top_left_in_atlas().y,
        );
        let off = self.data_at(local);
        self.clear_outer(off, al.dimensions(), initial_color);
        self.inner_pixmap(off, al.dimensions(), padding)
    }

    /// `outerPM.erase(color)`: clears the whole `dims` rect at byte `offset`.
    fn clear_outer(&mut self, offset: usize, dims: ISize, clear_color: Option<u32>) {
        let Some(color) = clear_color else {
            return;
        };
        let ct = self.mask_format.to_color_type();
        let row_bytes = self.row_bytes();
        let outer_info = ImageInfo::new(dims, ct, AlphaType::Opaque, None);
        if let Some(data) = self.data.as_mut()
            && let Some(mut outer) = Pixmap::new(&outer_info, &mut data[offset..], row_bytes)
        {
            outer.erase(Color::new(color), None);
        }
    }

    /// `innerPM` of `outerPM.extractSubset(inset by padding)`, as a pixmap over the same pixels.
    /// `None` is the empty pixmap of the C++ code.
    fn inner_pixmap(&mut self, offset: usize, dims: ISize, padding: i32) -> Option<Pixmap<'_>> {
        if !inner_is_non_empty(dims, padding) {
            return None;
        }
        let ct = self.mask_format.to_color_type();
        let bpp = self.bpp();
        let row_bytes = self.row_bytes();
        let inner_info = ImageInfo::new(
            ISize::new(dims.width - 2 * padding, dims.height - 2 * padding),
            ct,
            AlphaType::Opaque,
            None,
        );
        let inner_off = offset + padding as usize * row_bytes + padding as usize * bpp;
        let data = self.data.as_mut()?;
        Pixmap::new(&inner_info, &mut data[inner_off..], row_bytes)
    }
}

/// Whether the region of `dims` inset by `padding` on every side is non-empty.
fn inner_is_non_empty(dims: ISize, padding: i32) -> bool {
    dims.width - 2 * padding > 0 && dims.height - 2 * padding > 0
}
