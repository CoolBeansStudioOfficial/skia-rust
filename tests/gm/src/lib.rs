// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: gm/gm.h, gm/gm.cpp, dm/DMSrcSink.{h,cpp} (GMSrc, RasterSink), dm/DM.cpp

//! 1:1 ports of Skia's GMs, plus the harness that renders them exactly like DM and checks the
//! bytes against the oracle goldens on every CPU tier (design `docs/design/raster-pipeline.md`
//! §4.4; usage in `docs/PORTING.md` "Porting GMs").
//!
//! # Writing a GM
//! A GM file `gm/<file>.cpp` is ported to `src/gm/<file_snake>.rs` (a leading digit gets a `_`:
//! `gm/3d.cpp` → `gm::_3d`). Each registration macro maps to a macro of this crate with the same
//! arguments:
//!
//! | Skia | skia-rust |
//! |---|---|
//! | `DEF_SIMPLE_GM(name, canvas, W, H) { … }` | [`def_simple_gm!`]`(name, canvas, W, H, { … });` |
//! | `DEF_SIMPLE_GM_BG(name, canvas, W, H, BG) { … }` | [`def_simple_gm_bg!`]`(name, canvas, W, H, BG, { … });` |
//! | `DEF_SIMPLE_GM_BG_NAME(name, canvas, W, H, BG, NAME_STR) { … }` | [`def_simple_gm_bg_name!`] |
//! | `DEF_SIMPLE_GM_CAN_FAIL(name, canvas, errorMsg, W, H) { … }` | [`def_simple_gm_can_fail!`] |
//! | `DEF_SIMPLE_GM_BG_CAN_FAIL(…)`, `DEF_SIMPLE_GM_BG_NAME_CAN_FAIL(…)` | [`def_simple_gm_bg_can_fail!`], [`def_simple_gm_bg_name_can_fail!`] |
//! | `DEF_GM(return new FooGM;)` | [`def_gm!`]`(FooGM, FooGM::new());` |
//! | `DEF_GM(return new FooGM(1, 2);)` | [`def_gm!`]`(FooGM_1_2 = "FooGM(1, 2)", FooGM::new(1, 2));` |
//!
//! A class GM (`class FooGM : public skiagm::GM`) implements the [`GM`] trait. Every
//! registration both adds the GM to the [registry](registry) (what `cargo xtask inventory verify`
//! enumerates) and defines a `#[test]` with the registration's name that renders the GM for
//! every config and tier and compares it against the goldens ([`check::run_gm_test`]).
//!
//! # Layout
//! - [`canvas`]: the `Canvas`/`Surface` seam (re-exports of the real raster types).
//! - [`sink`]: DM's `GMSrc` + `RasterSink`, the configs `8888`/`565`/`f16`, byte extraction.
//! - [`goldens`]: loading the oracle's golden hashes and objects.
//! - [`check`]: the per-tier loop, verdicts and reports.
//! - [`diff`]: PNG diff images for mismatches.
//! - [`gm`](mod@gm): the ported GMs.

pub mod canvas;
pub mod check;
pub mod diff;
pub mod gm;
pub mod goldens;
pub mod sink;
pub mod tool_utils;

use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::size::ISize;

use crate::canvas::{BlendMode, Canvas, SurfaceProps};

/// What GM ports import: `use crate::prelude::*;`.
pub mod prelude {
    pub use crate::canvas::{BlendMode, Canvas, PixelGeometry, Surface, SurfaceProps};
    pub use crate::{DrawResult, GM};
    pub use skia_rust_core::color::{Color, Color4f};
    pub use skia_rust_core::size::ISize;
}

/// Port of `skiagm::DrawResult`.
#[doc(alias = "skiagm::DrawResult")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DrawResult {
    /// Test drew successfully.
    Ok,
    /// Test failed to draw.
    Fail,
    /// Test is not applicable in this context and should be skipped.
    Skip,
}

/// `GM::kErrorMsg_DrawSkippedGpuOnly`.
// Port of: gm/gm.h#L124-L125 (chrome/m156)
pub const ERROR_MSG_DRAW_SKIPPED_GPU_ONLY: &str = "This test is for GPU configs only.";

// Port of: gm/gm.cpp#L210-L219 (chrome/m156)
fn mark(canvas: &Canvas, x: f32, y: f32, f: impl FnOnce()) {
    let mut alpha = skia_rust_core::paint::Paint::default();
    alpha.set_alpha(0x50);
    canvas.save_layer(&skia_rust_core::canvas::SaveLayerRec::default().paint(&alpha));
    canvas.translate((x, y));
    canvas.scale((2.0, 2.0));
    f();
    canvas.restore();
}

/// Draws a green check mark at `(x, y)`: the GM checked its own result and it is good
/// (`MarkGMGood`).
// Port of: gm/gm.cpp#L221-L236 (chrome/m156)
#[doc(alias = "MarkGMGood")]
pub fn mark_gm_good(canvas: &Canvas, x: f32, y: f32) {
    use skia_rust_core::paint::{Paint, Style};
    mark(canvas, x, y, || {
        // A green circle.
        canvas.draw_circle(
            (0.0, 0.0),
            12.0,
            &Paint::new(Color4f::from_color(Color::from_rgb(27, 158, 119)), None),
        );

        // Cut out a check mark.
        let mut paint = Paint::new(skia_rust_core::color::colors::TRANSPARENT, None);
        paint.set_blend_mode(BlendMode::Src);
        paint.set_stroke_width(2.0);
        paint.set_style(Style::Stroke);
        canvas.draw_line((-6.0, 0.0), (-1.0, 5.0), &paint);
        canvas.draw_line((-1.0, 5.0), (7.0, -5.0), &paint);
    });
}

/// Draws a red cross at `(x, y)`: the GM checked its own result and it is bad (`MarkGMBad`).
// Port of: gm/gm.cpp#L238-L253 (chrome/m156)
#[doc(alias = "MarkGMBad")]
pub fn mark_gm_bad(canvas: &Canvas, x: f32, y: f32) {
    use skia_rust_core::paint::{Paint, Style};
    mark(canvas, x, y, || {
        // A red circle.
        canvas.draw_circle(
            (0.0, 0.0),
            12.0,
            &Paint::new(Color4f::from_color(Color::from_rgb(231, 41, 138)), None),
        );

        // Cut out an 'X'.
        let mut paint = Paint::new(skia_rust_core::color::colors::TRANSPARENT, None);
        paint.set_blend_mode(BlendMode::Src);
        paint.set_stroke_width(2.0);
        paint.set_style(Style::Stroke);
        canvas.draw_line((-5.0, -5.0), (5.0, 5.0), &paint);
        canvas.draw_line((5.0, -5.0), (-5.0, 5.0), &paint);
    });
}

/// Port of the virtual interface of `skiagm::GM`: what a GM overrides.
///
/// The non-virtual driver (`draw`, `drawBackground`, `drawContent`, `onceBeforeDraw`) is
/// [`GmInstance`]. Hooks that only matter for GPU or interactive use (`modifyGrContextOptions`,
/// `onAnimate`, `onChar`, controls, `getGoldKeys`) are not part of the raster harness.
// Port of: gm/gm.h#L107-L215 (chrome/m156)
#[doc(alias = "skiagm::GM")]
pub trait GM {
    /// `getName()`: the result name DM uses (`<config>/gm/<name>`).
    #[doc(alias = "getName")]
    fn name(&self) -> String;

    /// `getISize()`.
    #[doc(alias = "getISize")]
    fn size(&mut self) -> ISize;

    /// `getBGColor()`: the background DM clears to with `kSrc` before drawing (default white).
    /// A GM that calls `setBGColor` returns the color it set; it is read after
    /// [`on_once_before_draw`](GM::on_once_before_draw), as in Skia.
    #[doc(alias = "getBGColor")]
    fn bg_color(&self) -> Color {
        Color::WHITE
    }

    /// `modifySurfaceProps()`: adjusts the surface properties DM creates the surface with.
    #[doc(alias = "modifySurfaceProps")]
    fn modify_surface_props(&self, _props: &mut SurfaceProps) {}

    /// `onGpuSetup()`. DM calls it for raster sinks too, with no GPU context; a GM may return
    /// [`DrawResult::Skip`] here.
    #[doc(alias = "onGpuSetup")]
    fn on_gpu_setup(&mut self, _canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        DrawResult::Ok
    }

    /// `onOnceBeforeDraw()`: one-time setup, before the first draw.
    #[doc(alias = "onOnceBeforeDraw")]
    fn on_once_before_draw(&mut self) {}

    /// `onDraw(SkCanvas*)`: the drawing, for GMs that cannot fail.
    ///
    /// # Panics
    /// The default panics (`SK_ABORT("Not implemented.")`): override this or
    /// [`on_draw_with_error`](GM::on_draw_with_error).
    #[doc(alias = "onDraw")]
    fn on_draw(&mut self, _canvas: &Canvas) {
        panic!("Not implemented.");
    }

    /// `onDraw(SkCanvas*, SkString* errorMsg)`: the drawing, for GMs that can fail or skip.
    /// The default calls [`on_draw`](GM::on_draw) and returns [`DrawResult::Ok`].
    // Port of: gm/gm.cpp#L139-L143 (chrome/m156)
    #[doc(alias = "onDraw")]
    fn on_draw_with_error(&mut self, canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        self.on_draw(canvas);
        DrawResult::Ok
    }
}

/// A GM instance plus the state of `skiagm::GM`'s non-virtual driver.
#[derive(Debug)]
pub struct GmInstance {
    gm: Box<dyn GM>,
    have_called_once_before_draw: bool,
}

impl std::fmt::Debug for dyn GM {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GM({})", self.name())
    }
}

impl GmInstance {
    #[must_use]
    pub fn new(gm: Box<dyn GM>) -> Self {
        Self {
            gm,
            have_called_once_before_draw: false,
        }
    }

    /// The GM.
    pub fn gm(&mut self) -> &mut dyn GM {
        self.gm.as_mut()
    }

    /// `GM::onceBeforeDraw()`.
    // Port of: gm/gm.h#L130-L135 (chrome/m156)
    pub fn once_before_draw(&mut self) {
        if !self.have_called_once_before_draw {
            self.have_called_once_before_draw = true;
            self.gm.on_once_before_draw();
        }
    }

    /// `GM::gpuSetup()` as DM's `GMSrc::draw` calls it for a raster sink (no GPU context).
    // Port of: gm/gm.cpp#L88-L105 (chrome/m156)
    pub fn gpu_setup(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let result = self.gm.on_gpu_setup(canvas, error_msg);
        if result != DrawResult::Ok {
            handle_gm_failure(canvas, result, error_msg);
        }
        result
    }

    /// `GM::draw(canvas, errorMsg)`: background, then content.
    // Port of: gm/gm.cpp#L116-L120 (chrome/m156)
    pub fn draw(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        self.draw_background(canvas);
        self.draw_content(canvas, error_msg)
    }

    /// `GM::drawBackground()`: `drawColor(bgColor, kSrc)`.
    // Port of: gm/gm.cpp#L133-L137 (chrome/m156)
    pub fn draw_background(&mut self, canvas: &Canvas) {
        self.once_before_draw();
        canvas.draw_color(self.gm.bg_color(), BlendMode::Src);
    }

    /// `GM::drawContent()`: `onDraw` inside an `SkAutoCanvasRestore`.
    // Port of: gm/gm.cpp#L122-L131 (chrome/m156)
    pub fn draw_content(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        self.once_before_draw();
        // SkAutoCanvasRestore acr(canvas, true);
        let save_count = canvas.save_count();
        canvas.save();
        let draw_result = self.gm.on_draw_with_error(canvas, error_msg);
        if draw_result != DrawResult::Ok {
            handle_gm_failure(canvas, draw_result, error_msg);
        }
        canvas.restore_to_count(save_count);
        draw_result
    }
}

/// `handle_gm_failure()` in `gm.cpp`.
///
/// DM discards the pixels of a skipped GM, so the skip drawings do not matter. A failed GM's
/// pixels are written (and are goldens), but they contain the failure message as text, which
/// needs fonts (Phase 5): the background is drawn, the text is not, and the harness reports any
/// `Fail` as a failure without comparing.
// Port of: gm/gm.cpp#L34-L79 (chrome/m156)
fn handle_gm_failure(canvas: &Canvas, result: DrawResult, _error_msg: &str) {
    if result == DrawResult::Fail {
        // draw_failure_message(canvas, "DRAW FAILED: %s", errorMsg)
        canvas.draw_color(Color::from_rgb(200, 0, 0), None);
        // skia-rust: the message text needs `SkFont` (Phase 5).
    }
    // Skips: `draw_gpu_only_message` / `draw_failure_message("DRAW SKIPPED…")` draw into pixels
    // that DM never writes out.
}

/// Port of `skiagm::SimpleGM`: what the `DEF_SIMPLE_GM*` macros register.
// Port of: gm/gm.h#L240-L258, gm/gm.cpp#L145-L149 (chrome/m156)
#[doc(alias = "skiagm::SimpleGM")]
#[derive(Debug)]
pub struct SimpleGM {
    bg_color: Color,
    name: &'static str,
    size: ISize,
    draw_proc: DrawProc,
}

/// `SimpleGM::DrawProc`.
pub type DrawProc = fn(&Canvas, &mut String) -> DrawResult;

impl SimpleGM {
    #[must_use]
    pub const fn new(
        bg_color: Color,
        name: &'static str,
        size: ISize,
        draw_proc: DrawProc,
    ) -> Self {
        Self {
            bg_color,
            name,
            size,
            draw_proc,
        }
    }
}

impl GM for SimpleGM {
    fn name(&self) -> String {
        self.name.to_owned()
    }

    fn size(&mut self) -> ISize {
        self.size
    }

    fn bg_color(&self) -> Color {
        self.bg_color
    }

    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        (self.draw_proc)(canvas, error_msg)
    }
}

/// The GM registry (`skiagm::GMRegistry`).
pub mod registry {
    use super::GM;

    /// One registered GM: what a `DEF_GM*` macro registers.
    #[derive(Debug)]
    pub struct GmRegistration {
        /// `module_path!()` of the registration's own (macro-generated) module:
        /// `skia_rust_gm::gm::<file>::<test name>`.
        pub module_path: &'static str,
        /// The manifest registration name: the first `DEF_*` argument as the manifest has it
        /// (`dash_line_zero_off_interval`, `DashingGM`, `Dashing5GM(true)`, …).
        pub name: &'static str,
        /// `GMFactory`: a fresh GM per call (DM creates one per size query, name query and draw).
        pub factory: fn() -> Box<dyn GM>,
    }

    inventory::collect!(GmRegistration);

    impl GmRegistration {
        /// The key `cargo xtask inventory verify` maps manifest ids to:
        /// `gm/dashing.cpp::Dashing5GM(true)` ↔ `gm::dashing::Dashing5GM(true)`.
        #[must_use]
        pub fn key(&self) -> String {
            let path = self
                .module_path
                .strip_prefix("skia_rust_gm::")
                .unwrap_or(self.module_path);
            let file = path.rsplit_once("::").map_or(path, |(file, _)| file);
            format!("{file}::{}", self.name)
        }
    }

    /// Every registered GM, sorted by key.
    #[must_use]
    pub fn all() -> Vec<&'static GmRegistration> {
        let mut all: Vec<_> = inventory::iter::<GmRegistration>.into_iter().collect();
        all.sort_by_key(|r| r.key());
        all
    }
}

/// Port of `DEF_GM(return new FooGM(...);)`.
///
/// `def_gm!(FooGM, FooGM::new())` registers the GM under the manifest name `FooGM` and defines
/// the test `FooGM`. When the manifest name is not an identifier, give the test a name and the
/// manifest name as a string: `def_gm!(Dashing5GM_true = "Dashing5GM(true)", Dashing5GM::new(true))`.
/// Attributes (e.g. `#[ignore = "see notes/…"]`) go first and apply to the test.
#[macro_export]
macro_rules! def_gm {
    ($(#[$attr:meta])* $test:ident = $name:expr, $make:expr $(,)?) => {
        #[allow(non_snake_case)]
        #[doc(hidden)]
        mod $test {
            #[allow(unused_imports)]
            use super::*;
            pub(super) const REGISTRATION: $crate::registry::GmRegistration =
                $crate::registry::GmRegistration {
                    module_path: module_path!(),
                    name: $name,
                    factory: {
                        fn factory() -> ::std::boxed::Box<dyn $crate::GM> {
                            ::std::boxed::Box::new($make)
                        }
                        factory
                    },
                };
        }
        $crate::__submit_gm!($test::REGISTRATION);
        #[test]
        $(#[$attr])*
        #[allow(non_snake_case)]
        fn $test() {
            $crate::check::run_gm_test(&$test::REGISTRATION);
        }
    };
    ($(#[$attr:meta])* $test:ident, $make:expr $(,)?) => {
        $crate::def_gm!($(#[$attr])* $test = stringify!($test), $make);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __submit_gm {
    ($reg:expr) => {
        $crate::__inventory::submit!($reg);
    };
}

#[doc(hidden)]
pub use inventory as __inventory;

/// Port of `DEF_SIMPLE_GM(NAME, CANVAS, W, H) { … }`: white background.
#[macro_export]
macro_rules! def_simple_gm {
    ($(#[$attr:meta])* $name:ident, $canvas:ident, $w:expr, $h:expr, $body:block $(,)?) => {
        $crate::def_simple_gm_bg_name!(
            $(#[$attr])* $name, $canvas, $w, $h,
            $crate::prelude::Color::WHITE, stringify!($name), $body
        );
    };
}

/// Port of `DEF_SIMPLE_GM_BG(NAME, CANVAS, W, H, BGCOLOR) { … }`.
#[macro_export]
macro_rules! def_simple_gm_bg {
    ($(#[$attr:meta])* $name:ident, $canvas:ident, $w:expr, $h:expr, $bg:expr, $body:block $(,)?) => {
        $crate::def_simple_gm_bg_name!(
            $(#[$attr])* $name, $canvas, $w, $h, $bg, stringify!($name), $body
        );
    };
}

/// Port of `DEF_SIMPLE_GM_BG_NAME(NAME, CANVAS, W, H, BGCOLOR, NAME_STR) { … }`: the body is the
/// `void NAME_GM_inner(SkCanvas*)` function (it may `return;` early) and the GM always returns
/// `DrawResult::kOk`.
#[macro_export]
macro_rules! def_simple_gm_bg_name {
    ($(#[$attr:meta])* $name:ident, $canvas:ident, $w:expr, $h:expr, $bg:expr, $name_str:expr,
     $body:block $(,)?) => {
        $crate::def_gm!(
            $(#[$attr])* $name = stringify!($name),
            {
                #[allow(unused_variables)]
                fn __gm_inner($canvas: &$crate::canvas::Canvas) $body
                fn __gm_draw(
                    canvas: &$crate::canvas::Canvas,
                    _error_msg: &mut ::std::string::String,
                ) -> $crate::DrawResult {
                    __gm_inner(canvas);
                    $crate::DrawResult::Ok
                }
                $crate::SimpleGM::new(
                    $bg,
                    $name_str,
                    $crate::prelude::ISize::new($w, $h),
                    __gm_draw,
                )
            }
        );
    };
}

/// Port of `DEF_SIMPLE_GM_CAN_FAIL(NAME, CANVAS, ERR_MSG, W, H) { … return DrawResult::kOk; }`:
/// the body returns a [`DrawResult`] and may write `ERR_MSG` (a `&mut String`).
#[macro_export]
macro_rules! def_simple_gm_can_fail {
    ($(#[$attr:meta])* $name:ident, $canvas:ident, $err:ident, $w:expr, $h:expr,
     $body:block $(,)?) => {
        $crate::def_simple_gm_bg_name_can_fail!(
            $(#[$attr])* $name, $canvas, $err, $w, $h,
            $crate::prelude::Color::WHITE, stringify!($name), $body
        );
    };
}

/// Port of `DEF_SIMPLE_GM_BG_CAN_FAIL(NAME, CANVAS, ERR_MSG, W, H, BGCOLOR) { … }`.
#[macro_export]
macro_rules! def_simple_gm_bg_can_fail {
    ($(#[$attr:meta])* $name:ident, $canvas:ident, $err:ident, $w:expr, $h:expr, $bg:expr,
     $body:block $(,)?) => {
        $crate::def_simple_gm_bg_name_can_fail!(
            $(#[$attr])* $name, $canvas, $err, $w, $h, $bg, stringify!($name), $body
        );
    };
}

/// Port of `DEF_SIMPLE_GM_BG_NAME_CAN_FAIL(NAME, CANVAS, ERR_MSG, W, H, BGCOLOR, NAME_STR) { … }`.
#[macro_export]
macro_rules! def_simple_gm_bg_name_can_fail {
    ($(#[$attr:meta])* $name:ident, $canvas:ident, $err:ident, $w:expr, $h:expr, $bg:expr,
     $name_str:expr, $body:block $(,)?) => {
        $crate::def_gm!(
            $(#[$attr])* $name = stringify!($name),
            {
                #[allow(unused_variables)]
                #[allow(clippy::ptr_arg)] // `SkString* errorMsg`: the body may assign a new string
                fn __gm_draw(
                    $canvas: &$crate::canvas::Canvas,
                    $err: &mut ::std::string::String,
                ) -> $crate::DrawResult $body
                $crate::SimpleGM::new(
                    $bg,
                    $name_str,
                    $crate::prelude::ISize::new($w, $h),
                    __gm_draw,
                )
            }
        );
    };
}

#[cfg(test)]
mod macro_tests;
