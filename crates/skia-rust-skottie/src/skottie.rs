// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/include/Skottie.h, modules/skottie/src/Skottie.cpp
// (chrome/m156)
//
// The public face of the player: [`Animation`] and its [`Builder`]. The builder is
// `skottie::Animation::Builder` in Skia and `skia_safe::skottie::Builder` in rust-skia; its
// setters take `&mut self` and return `&mut Self`, so a builder can be reused after `make`, which
// is what `getSlotManager` and `getLayerInfo` rely on.
//
// Not yet ported (see the module docs of `internal::effects` and `internal::text`): the text
// shaping factory (`setTextShapingFactory`) and text layers wait for the shaper (M5/M22), and the
// layer effects for M21.

use std::rc::Rc;

use skia_rust_core::canvas::{Canvas, SaveLayerRec};
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::data::Data;
use skia_rust_core::font_mgr::FontMgr;
use skia_rust_core::matrix::{Matrix, ScaleToFit};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::SCALAR_MAX;
use skia_rust_core::size::Size;
use skia_rust_core::stream::Stream;
use skia_rust_core::t_pin::t_pin;
use skia_rust_sksg::{InvalidationController, RenderNode};

use skia_rust_resources::ResourceProvider;

use crate::external_layer::PrecompInterceptor;
use crate::internal::animator::Animator;
use crate::internal::skottie_priv::AnimationBuilder;
use crate::json::DOM;
use crate::skottie_json::{ValueExt, parse_default};
use crate::skottie_property::PropertyObserver;
use crate::slot_manager::SlotManager;

/// The image assets are the ones of `skresources`.
// Port of: modules/skottie/include/Skottie.h#L47 (chrome/m156) (`skottie::ImageAsset`)
pub use skia_rust_resources::ImageAsset;

/// Information about a layer of the animation.
// Port of: modules/skottie/include/Skottie.h#L49-L54 (chrome/m156) (`LayerInfo`)
#[doc(alias = "skottie::LayerInfo")]
#[derive(Debug, Clone, PartialEq)]
pub struct LayerInfo {
    /// The name of the layer (`fName`).
    pub name: String,
    /// The size of the layer (`fSize`).
    pub size: Size,
    /// The in-point of the layer, in frames (`fInPoint`).
    pub in_point: f32,
    /// The out-point of the layer, in frames (`fOutPoint`).
    pub out_point: f32,
}

/// The severity of a log message (`Logger::Level`).
// Port of: modules/skottie/include/Skottie.h#L62-L65 (chrome/m156) (`Logger::Level`)
#[doc(alias = "skottie::Logger::Level")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoggerLevel {
    /// A recoverable problem (`kWarning`).
    Warning,
    /// A problem that fails (part of) the animation (`kError`).
    Error,
}

/// A `Logger` can be used to receive `Animation::Builder` parsing errors and warnings.
// Port of: modules/skottie/include/Skottie.h#L60-L69 (chrome/m156) (`class Logger`)
#[doc(alias = "skottie::Logger")]
pub trait Logger {
    /// Receives a message, and the JSON it refers to if there is any (`log`).
    fn log(&self, level: LoggerLevel, message: &str, json: Option<&str>);
}

/// Evaluates AE expressions.
// Port of: modules/skottie/include/Skottie.h#L71-L77 (chrome/m156) (`class ExpressionEvaluator`)
#[doc(alias = "skottie::ExpressionEvaluator")]
pub trait ExpressionEvaluator<T> {
    /// Evaluates the expression at the current time (`evaluate`).
    fn evaluate(&self, t: f32) -> T;
}

/// Creates `ExpressionEvaluator`s to evaluate AE expressions and return the results. Skia ships
/// no expression engine, only this interface.
// Port of: modules/skottie/include/Skottie.h#L79-L91 (chrome/m156) (`class ExpressionManager`)
#[doc(alias = "skottie::ExpressionManager")]
pub trait ExpressionManager {
    /// An evaluator of an expression that yields a number.
    #[doc(alias = "createNumberExpressionEvaluator")]
    fn create_number_expression_evaluator(
        &self,
        expression: &str,
    ) -> Option<Rc<dyn ExpressionEvaluator<f32>>>;

    /// An evaluator of an expression that yields a string.
    #[doc(alias = "createStringExpressionEvaluator")]
    fn create_string_expression_evaluator(
        &self,
        expression: &str,
    ) -> Option<Rc<dyn ExpressionEvaluator<String>>>;

    /// An evaluator of an expression that yields an array of numbers.
    #[doc(alias = "createArrayExpressionEvaluator")]
    fn create_array_expression_evaluator(
        &self,
        expression: &str,
    ) -> Option<Rc<dyn ExpressionEvaluator<Vec<f32>>>>;
}

/// Interface for receiving AE composition markers at `Animation` build time.
// Port of: modules/skottie/include/Skottie.h#L93-L100 (chrome/m156) (`class MarkerObserver`)
#[doc(alias = "skottie::MarkerObserver")]
pub trait MarkerObserver {
    /// Receives a marker; `t0` and `t1` are in the `Animation::seek()` domain (`onMarker`).
    #[doc(alias = "onMarker")]
    fn on_marker(&self, name: &str, t0: f32, t1: f32);
}

bitflags::bitflags! {
    /// Flags for configuring the animation builder (`Animation::Builder::Flags`).
    // Port of: modules/skottie/include/Skottie.h#L105-L111 (chrome/m156)
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct BuilderFlags: u32 {
        /// Normally, all static image frames are resolved at load time via
        /// `ImageAsset::get_frame(0)`. With this flag, frames are only resolved when needed, at
        /// seek time (`kDeferImageLoading`).
        const DEFER_IMAGE_LOADING = 0x01;
        /// Attempt to use the embedded fonts (glyph paths, normally used as fallback) over
        /// native Skia typefaces (`kPreferEmbeddedFonts`).
        const PREFER_EMBEDDED_FONTS = 0x02;
    }
}

bitflags::bitflags! {
    /// Flags for rendering control (`Animation::RenderFlag`).
    // Port of: modules/skottie/include/Skottie.h#L202-L211 (chrome/m156)
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct RenderFlags: u32 {
        /// When rendering into a known transparent buffer, clients can pass this flag to avoid
        /// some unnecessary compositing overhead for animations using layer blend modes
        /// (`kSkipTopLevelIsolation`).
        const SKIP_TOP_LEVEL_ISOLATION = 0x01;
        /// By default, content is clipped to the intrinsic animation bounds (as determined by its
        /// size). If this flag is set, then the animation can draw outside of the bounds
        /// (`kDisableTopLevelClipping`).
        const DISABLE_TOP_LEVEL_CLIPPING = 0x02;
    }
}

/// Various animation build stats (`Animation::Builder::Stats`).
// Port of: modules/skottie/include/Skottie.h#L119-L125 (chrome/m156)
#[doc(alias = "skottie::Animation::Builder::Stats")]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Stats {
    /// Total animation instantiation time (`fTotalLoadTimeMS`).
    pub total_load_time_ms: f32,
    /// Time spent building a JSON DOM (`fJsonParseTimeMS`).
    pub json_parse_time_ms: f32,
    /// Time spent constructing the animation scene graph (`fSceneParseTimeMS`).
    pub scene_parse_time_ms: f32,
    /// Input JSON size (`fJsonSize`).
    pub json_size: usize,
    /// Number of dynamically animated properties (`fAnimatorCount`).
    pub animator_count: usize,
}

/// Builds [`Animation`]s.
// Port of: modules/skottie/include/Skottie.h#L102-L190 (chrome/m156) (`class Animation::Builder`)
#[doc(alias = "skottie::Animation::Builder")]
#[derive(Default)]
pub struct Builder {
    flags: BuilderFlags,
    resource_provider: Option<Rc<dyn ResourceProvider>>,
    font_mgr: Option<FontMgr>,
    property_observer: Option<Rc<dyn PropertyObserver>>,
    logger: Option<Rc<dyn Logger>>,
    marker_observer: Option<Rc<dyn MarkerObserver>>,
    precomp_interceptor: Option<Rc<dyn PrecompInterceptor>>,
    expression_manager: Option<Rc<dyn ExpressionManager>>,
    slot_manager: Option<Rc<SlotManager>>,
    stats: Stats,
    layer_info: Vec<LayerInfo>,
}

impl std::fmt::Debug for Builder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Builder")
            .field("flags", &self.flags)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl Builder {
    /// A builder with no flags (`Builder(uint32_t flags = 0)`).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A builder with the given flags.
    #[must_use]
    pub fn with_flags(flags: BuilderFlags) -> Self {
        Self {
            flags,
            ..Self::default()
        }
    }

    /// Various animation build stats, of the last animation made (`getStats`).
    #[doc(alias = "getStats")]
    #[must_use]
    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    /// Specifies a loader for external resources (images, etc.).
    // Port of: modules/skottie/src/Skottie.cpp#L254-L257 (chrome/m156)
    #[doc(alias = "setResourceProvider")]
    pub fn set_resource_provider(&mut self, rp: Rc<dyn ResourceProvider>) -> &mut Self {
        self.resource_provider = Some(rp);
        self
    }

    /// Specifies a font manager for loading animation fonts.
    // Port of: modules/skottie/src/Skottie.cpp#L259-L262 (chrome/m156)
    #[doc(alias = "setFontManager")]
    pub fn set_font_manager(&mut self, fmgr: FontMgr) -> &mut Self {
        self.font_mgr = Some(fmgr);
        self
    }

    /// Specifies a `PropertyObserver` to receive callbacks during parsing.
    // Port of: modules/skottie/src/Skottie.cpp#L264-L267 (chrome/m156)
    #[doc(alias = "setPropertyObserver")]
    pub fn set_property_observer(&mut self, pobserver: Rc<dyn PropertyObserver>) -> &mut Self {
        self.property_observer = Some(pobserver);
        self
    }

    /// Registers a `Logger` with this builder.
    // Port of: modules/skottie/src/Skottie.cpp#L269-L272 (chrome/m156)
    #[doc(alias = "setLogger")]
    pub fn set_logger(&mut self, logger: Rc<dyn Logger>) -> &mut Self {
        self.logger = Some(logger);
        self
    }

    /// Registers a `MarkerObserver` with this builder.
    // Port of: modules/skottie/src/Skottie.cpp#L274-L277 (chrome/m156)
    #[doc(alias = "setMarkerObserver")]
    pub fn set_marker_observer(&mut self, mobserver: Rc<dyn MarkerObserver>) -> &mut Self {
        self.marker_observer = Some(mobserver);
        self
    }

    /// Registers a precomp layer interceptor. This allows substituting precomp layers with
    /// custom/externally managed content.
    // Port of: modules/skottie/src/Skottie.cpp#L279-L282 (chrome/m156)
    #[doc(alias = "setPrecompInterceptor")]
    pub fn set_precomp_interceptor(&mut self, pi: Rc<dyn PrecompInterceptor>) -> &mut Self {
        self.precomp_interceptor = Some(pi);
        self
    }

    /// Registers an `ExpressionManager` to evaluate AE expressions. If unspecified, expressions
    /// in the animation JSON will be ignored.
    // Port of: modules/skottie/src/Skottie.cpp#L284-L287 (chrome/m156)
    #[doc(alias = "setExpressionManager")]
    pub fn set_expression_manager(&mut self, em: Rc<dyn ExpressionManager>) -> &mut Self {
        self.expression_manager = Some(em);
        self
    }

    /// Builds an animation from a stream (`make(SkStream*)`).
    // Port of: modules/skottie/src/Skottie.cpp#L294-L311 (chrome/m156)
    pub fn make_from_stream(&mut self, stream: &mut dyn Stream) -> Option<Animation> {
        if !stream.has_length() {
            // TODO: handle explicit buffering?
            if let Some(logger) = &self.logger {
                logger.log(LoggerLevel::Error, "Cannot parse streaming content.\n", None);
            }
            return None;
        }

        let length = stream.get_length();
        let Some(data) = Data::from_stream(stream, length) else {
            if let Some(logger) = &self.logger {
                logger.log(LoggerLevel::Error, "Failed to read the input stream.\n", None);
            }
            return None;
        };

        self.make(data.as_bytes())
    }

    /// Builds an animation from JSON text (`make(const char*, size_t)`).
    ///
    /// Like Skia's, the builder gives its observers to the animation: they are not kept for the
    /// next `make`.
    // Port of: modules/skottie/src/Skottie.cpp#L313-L408 (chrome/m156)
    pub fn make(&mut self, data: impl AsRef<[u8]>) -> Option<Animation> {
        let data = data.as_ref();

        self.stats = Stats::default();

        self.stats.json_size = data.len();
        let t0 = std::time::Instant::now();

        let dom = DOM::new(data);
        let Some(json) = dom.root().as_object() else {
            // TODO: more error info.
            if let Some(logger) = &self.logger {
                logger.log(LoggerLevel::Error, "Failed to parse JSON input.\n", None);
            }
            return None;
        };

        let t1 = std::time::Instant::now();
        self.stats.json_parse_time_ms = (t1 - t0).as_secs_f32() * 1000.0;

        let version = parse_default::<String>(json.get("v"), String::new());
        let size = Size::new(
            parse_default::<f32>(json.get("w"), 0.0),
            parse_default::<f32>(json.get("h"), 0.0),
        );
        let fps = parse_default::<f32>(json.get("fr"), -1.0);
        let in_point = parse_default::<f32>(json.get("ip"), 0.0);
        let out_point = {
            // std::max(a, b) is `(a < b) ? b : a`.
            let op = parse_default::<f32>(json.get("op"), SCALAR_MAX);
            if op < in_point { in_point } else { op }
        };
        let duration = skia_rust_core::floating_point::ieee_float_divide(out_point - in_point, fps);

        if size.is_empty()
            || fps <= 0.0
            || !skia_rust_core::floating_point::is_finite_all(in_point, &[out_point, duration])
        {
            if let Some(logger) = &self.logger {
                let msg = format!(
                    "Invalid animation params (size: [{:.6} {:.6}], frame rate: {:.6}, \
                     in-point: {:.6}, out-point: {:.6})\n",
                    f64::from(size.width),
                    f64::from(size.height),
                    f64::from(fps),
                    f64::from(in_point),
                    f64::from(out_point)
                );
                logger.log(LoggerLevel::Error, &msg, None);
            }
            return None;
        }

        let builder = AnimationBuilder::new(
            self.resource_provider.take(),
            self.font_mgr.clone(),
            self.property_observer.take(),
            self.logger.take(),
            self.marker_observer.take(),
            self.precomp_interceptor.take(),
            self.expression_manager.take(),
            size,
            duration,
            fps,
            self.flags,
        );
        let ainfo = builder.parse(json);
        self.stats.animator_count = ainfo.animators.len();

        self.slot_manager = Some(ainfo.slot_manager);
        self.layer_info = ainfo.layer_info;

        let t2 = std::time::Instant::now();
        self.stats.scene_parse_time_ms = (t2 - t1).as_secs_f32() * 1000.0;
        self.stats.total_load_time_ms = (t2 - t0).as_secs_f32() * 1000.0;

        if ainfo.scene_root.is_none() {
            builder.log(LoggerLevel::Error, "Could not parse animation.\n");
        }

        let mut flags = AnimationFlags::empty();
        if builder.has_nontrivial_blending() {
            flags |= AnimationFlags::REQUIRES_TOP_LEVEL_ISOLATION;
        }

        Some(Animation {
            scene_root: ainfo.scene_root,
            animators: ainfo.animators,
            version,
            size,
            in_point,
            out_point,
            duration,
            fps,
            flags,
        })
    }

    /// Builds an animation from a file (`makeFromFile`).
    // Port of: modules/skottie/src/Skottie.cpp#L410-L415 (chrome/m156)
    #[doc(alias = "makeFromFile")]
    pub fn make_from_file(&mut self, path: impl AsRef<std::path::Path>) -> Option<Animation> {
        let data = Data::from_filename(path)?;

        self.make(data.as_bytes())
    }

    /// The handle for the `SlotManager`, after the animation is built (`getSlotManager`).
    #[doc(alias = "getSlotManager")]
    #[must_use]
    pub fn slot_manager(&self) -> Option<&Rc<SlotManager>> {
        self.slot_manager.as_ref()
    }

    /// Information about the layers of the animation, after it is built (`getLayerInfo`).
    #[doc(alias = "getLayerInfo")]
    #[must_use]
    pub fn layer_info(&self) -> &[LayerInfo] {
        &self.layer_info
    }
}

bitflags::bitflags! {
    /// The private flags of an animation (`Animation::Flags`).
    // Port of: modules/skottie/include/Skottie.h#L262-L264 (chrome/m156)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct AnimationFlags: u32 {
        /// Needs to draw into a layer due to layer blending.
        const REQUIRES_TOP_LEVEL_ISOLATION = 1 << 0;
    }
}

/// A Lottie animation.
// Port of: modules/skottie/include/Skottie.h#L102-L276 (chrome/m156) (`class Animation`)
#[doc(alias = "skottie::Animation")]
pub struct Animation {
    scene_root: Option<Rc<dyn RenderNode>>,
    animators: Vec<Rc<dyn Animator>>,
    version: String,
    size: Size,
    in_point: f32,
    out_point: f32,
    duration: f32,
    fps: f32,
    flags: AnimationFlags,
}

impl std::fmt::Debug for Animation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Animation")
            .field("version", &self.version)
            .field("duration", &self.duration)
            .field("fps", &self.fps)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

/// `std::nextafterf(x, toward)`.
fn next_after(x: f32, toward: f32) -> f32 {
    if x.is_nan() || toward.is_nan() {
        return f32::NAN;
    }
    #[allow(clippy::float_cmp)] // exact, as nextafterf
    if x == toward {
        return toward;
    }
    if x == 0.0 {
        return f32::from_bits(1).copysign(toward);
    }
    let bits = x.to_bits();
    if (toward > x) == (x > 0.0) {
        f32::from_bits(bits + 1)
    } else {
        f32::from_bits(bits - 1)
    }
}

impl Animation {
    /// Parses an animation from JSON text (`Animation::Make`).
    // Port of: modules/skottie/src/Skottie.cpp#L521-L523 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        Builder::new().make(data)
    }

    /// Parses an animation from JSON text (`Animation::Make`).
    #[allow(clippy::should_implement_trait)] // mirrors skia-safe
    #[doc(alias = "Make")]
    #[must_use]
    pub fn from_str(json: impl AsRef<str>) -> Option<Self> {
        Self::from_bytes(json.as_ref().as_bytes())
    }

    /// Parses an animation from a stream (`Animation::Make(SkStream*)`).
    // Port of: modules/skottie/src/Skottie.cpp#L525-L527 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn from_stream(stream: &mut dyn Stream) -> Option<Self> {
        Builder::new().make_from_stream(stream)
    }

    /// Loads an animation from a file (`Animation::MakeFromFile`).
    // Port of: modules/skottie/src/Skottie.cpp#L529-L531 (chrome/m156)
    #[doc(alias = "MakeFromFile")]
    #[must_use]
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Option<Self> {
        Builder::new().make_from_file(path)
    }

    /// Draws the current animation frame (`render`).
    ///
    /// It is undefined behavior to call `render` on a newly created `Animation` before
    /// specifying an initial frame via one of the seek variants.
    ///
    /// `dst` is the optional destination rect.
    // Port of: modules/skottie/src/Skottie.cpp#L463-L493 (chrome/m156)
    pub fn render(&self, canvas: &Canvas, dst: Option<&Rect>) {
        self.render_with_flags(canvas, dst, RenderFlags::empty());
    }

    /// Draws the current animation frame, with the given render flags (`render`).
    // Port of: modules/skottie/src/Skottie.cpp#L469-L493 (chrome/m156)
    pub fn render_with_flags(&self, canvas: &Canvas, dst: Option<&Rect>, render_flags: RenderFlags) {
        let Some(scene_root) = &self.scene_root else {
            return;
        };

        // SkAutoCanvasRestore restore(canvas, true);
        let restore_count = canvas.save_count();
        canvas.save();

        let src_r = Rect::from_wh(self.size.width, self.size.height);
        if let Some(dst_r) = dst {
            canvas.concat(&Matrix::rect_to_rect_or_identity(
                src_r,
                dst_r,
                ScaleToFit::Center,
            ));
        }

        if !render_flags.contains(RenderFlags::DISABLE_TOP_LEVEL_CLIPPING) {
            canvas.clip_rect(src_r, ClipOp::Intersect, false);
        }

        if self.flags.contains(AnimationFlags::REQUIRES_TOP_LEVEL_ISOLATION)
            && !render_flags.contains(RenderFlags::SKIP_TOP_LEVEL_ISOLATION)
        {
            // The animation uses non-trivial blending, and needs
            // to be rendered into a separate/transparent layer.
            canvas.save_layer(&SaveLayerRec::default().bounds(&src_r));
        }

        scene_root.render(canvas, None);

        canvas.restore_to_count(restore_count);
    }

    /// Updates the animation state for `t`, a normalized `[0..1]` frame selector (0 -> first
    /// frame, 1 -> final frame). Deprecated: use one of the other versions.
    // Port of: modules/skottie/include/Skottie.h#L225-L227 (chrome/m156)
    pub fn seek(&self, t: f32, ic: Option<&mut InvalidationController>) {
        self.seek_frame_time(f64::from(t) * f64::from(self.duration()), ic);
    }

    /// Updates the animation state to match `t`, specified as a frame index i.e. relative to
    /// `duration() * fps()`.
    ///
    /// Fractional values are allowed and meaningful - e.g.
    ///
    ///   0.0 -> first frame
    ///   1.0 -> second frame
    ///   0.5 -> halfway between first and second frame
    // Port of: modules/skottie/src/Skottie.cpp#L495-L511 (chrome/m156)
    #[doc(alias = "seekFrame")]
    pub fn seek_frame(&self, t: f64, ic: Option<&mut InvalidationController>) {
        let Some(scene_root) = &self.scene_root else {
            return;
        };

        // Per AE/Lottie semantics out_point is exclusive.
        let last_valid_frame = next_after(self.out_point, self.in_point);
        // SkTPin<float>(fInPoint + t, ...): the double sum converts to float.
        #[allow(clippy::cast_possible_truncation)] // mirrors the implicit double -> float
        let requested = (f64::from(self.in_point) + t) as f32;
        let comp_time = t_pin(requested, self.in_point, last_valid_frame);

        for anim in &self.animators {
            anim.seek(comp_time);
        }

        scene_root.revalidate(ic, &Matrix::new_identity());
    }

    /// Updates the animation state to match `t`, specified in frame time i.e. relative to
    /// `duration()`.
    // Port of: modules/skottie/src/Skottie.cpp#L513-L515 (chrome/m156)
    #[doc(alias = "seekFrameTime")]
    pub fn seek_frame_time(&self, t: f64, ic: Option<&mut InvalidationController>) {
        self.seek_frame(t * f64::from(self.fps), ic);
    }

    /// The animation duration in seconds.
    #[must_use]
    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// The animation frame rate (frames / second).
    #[must_use]
    pub fn fps(&self) -> f32 {
        self.fps
    }

    /// The animation in point, in frame index units (`inPoint`).
    #[doc(alias = "inPoint")]
    #[must_use]
    pub fn in_point(&self) -> f32 {
        self.in_point
    }

    /// The animation out point, in frame index units (`outPoint`).
    #[doc(alias = "outPoint")]
    #[must_use]
    pub fn out_point(&self) -> f32 {
        self.out_point
    }

    /// The version of the animation file.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The intrinsic animation size.
    #[must_use]
    pub fn size(&self) -> Size {
        self.size
    }
}
