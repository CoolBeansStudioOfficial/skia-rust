// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: dm/DMSrcSink.{h,cpp} (SkottieSrc), dm/DM.cpp (gather_file_srcs)

//! DM's `--src lottie`: [`SkottieSrc`] draws a 5x5 film strip of an animation (`resources/skottie`
//! files) into a 1000x1000 raster canvas.
//!
//! The oracle ran `--src lottie --config 8888`, so a lottie file has one result,
//! `8888/lottie/<file>.json` ([`Src::id_dir`] is `lottie`, [`Src::name`] the file's basename), and
//! the other configs are not checked ([`CONFIGS`]).
//!
//! `FLAGS_useLottieGlyphPaths` is false (the default): the builder flags are empty.
//! `setTextShapingFactory(SkShapers::BestAvailable())` is not passed: the builder has no text
//! shaping factory yet (text layers wait for Skottie text, M22).

// The C++ converts between int and SkScalar as written (frame indices 0..=4, tile counts, the
// 1000 pixel target size): the values are small, so the conversions are exact.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::path::{Path, PathBuf};
use std::rc::Rc;

use skia_rust_core::color::Color;
use skia_rust_core::matrix::{Matrix, ScaleToFit};
use skia_rust_core::os_path;
use skia_rust_core::rect::Rect;
use skia_rust_core::size::ISize;
use skia_rust_resources::{
    DataURIResourceProviderProxy, FileResourceProvider, ImageDecodeStrategy, ResourceProvider,
};
use skia_rust_skottie::Builder;
use skia_rust_skottie::skottie::BuilderFlags;
use skia_rust_skottie::utils::ExternalAnimationPrecompInterceptor;
use skia_rust_tools::font_tool_utils::test_font_mgr;

use crate::canvas::Canvas;
use crate::sink::{Config, DmResult, Src, Status};

/// The configs the oracle rendered for `--src lottie` (`--config 8888`).
pub const CONFIGS: [Config; 1] = [Config::N32];

// Port of: dm/DMSrcSink.h#L324-L329 (chrome/m156)
/// `SkottieSrc::kTileCount`: the film strip is `kTileCount` x `kTileCount` frames.
const TILE_COUNT: i32 = 5;
/// `SkottieSrc::kTargetSize`.
const TARGET_SIZE: f32 = 1000.0;
/// `SkottieSrc::kTileSize`.
const TILE_SIZE: f32 = TARGET_SIZE / 5.0;

/// `kInterceptPrefix`.
const INTERCEPT_PREFIX: &str = "__";
/// `frameOrder`.
const FRAME_ORDER: [i32; 5] = [4, 0, 3, 1, 2];

/// `DM::SkottieSrc`.
// Port of: dm/DMSrcSink.h#L308-L333 (chrome/m156)
#[derive(Clone, Debug)]
pub struct SkottieSrc {
    path: PathBuf,
}

impl SkottieSrc {
    /// `SkottieSrc::SkottieSrc(Path)`.
    // Port of: dm/DMSrcSink.cpp#L1297 (chrome/m156)
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl Src for SkottieSrc {
    // Port of: dm/DMSrcSink.cpp#L1299-L1363 (chrome/m156)
    fn draw(&self, canvas: &Canvas) -> DmResult {
        let path = self.path.display().to_string();
        let predecode = ImageDecodeStrategy::PreDecode;
        // DM should have already registered the codecs necessary for DataURIResourceProviderProxy
        // to decode images.
        let file_provider = FileResourceProvider::make(&os_path::dirname(Some(&path)), predecode)
            .map(|p| p as Rc<dyn ResourceProvider>);
        let resource_provider: Rc<dyn ResourceProvider> =
            DataURIResourceProviderProxy::make(file_provider, predecode, Some(test_font_mgr()));

        let precomp_interceptor = ExternalAnimationPrecompInterceptor::new(
            Rc::clone(&resource_provider),
            INTERCEPT_PREFIX,
        );
        // FLAGS_useLottieGlyphPaths is false.
        let flags = BuilderFlags::empty();

        let mut builder = Builder::with_flags(flags);
        builder
            .set_font_manager(test_font_mgr())
            .set_resource_provider(resource_provider)
            .set_precomp_interceptor(precomp_interceptor);
        let Some(animation) = builder.make_from_file(&self.path) else {
            return DmResult::new(Status::Fatal, format!("Unable to parse file: {path}"));
        };

        canvas.draw_color(Color::WHITE, None);

        let t_rate = 1.0_f32 / (TILE_COUNT * TILE_COUNT - 1) as f32;

        // Draw the frames in a shuffled order to exercise non-linear
        // frame progression. The film strip will still be in order left-to-right,
        // top-down, just not drawn in that order.

        for i in 0..TILE_COUNT {
            let y = FRAME_ORDER[i as usize] as f32 * TILE_SIZE;

            for j in 0..TILE_COUNT {
                let x = FRAME_ORDER[j as usize] as f32 * TILE_SIZE;
                let dest = Rect::from_xywh(x, y, TILE_SIZE, TILE_SIZE);

                let t = t_rate
                    * (FRAME_ORDER[i as usize] * TILE_COUNT + FRAME_ORDER[j as usize]) as f32;
                {
                    // SkAutoCanvasRestore acr(canvas, true);
                    let restore_count = canvas.save_count();
                    canvas.save();
                    canvas.clip_rect(dest, None, true);
                    let size = animation.size();
                    canvas.concat(&Matrix::rect_to_rect_or_identity(
                        Rect::from_wh(size.width, size.height),
                        dest,
                        ScaleToFit::Center,
                    ));
                    animation.seek(t);
                    animation.render(canvas, None);
                    canvas.restore_to_count(restore_count);
                }
            }
        }

        DmResult::new(Status::Ok, String::new())
    }

    // Port of: dm/DMSrcSink.cpp#L1365-L1367 (chrome/m156)
    fn size(&self) -> ISize {
        ISize::new(TARGET_SIZE as i32, TARGET_SIZE as i32)
    }

    // Port of: dm/DMSrcSink.cpp#L1369 (chrome/m156)
    fn name(&self) -> String {
        os_path::basename(Some(&self.path.display().to_string()))
    }

    fn id_dir(&self) -> String {
        "lottie".to_owned()
    }
}

/// `gather_file_srcs<SkottieSrc>(FLAGS_lotties, "json", "lottie")` for a directory: a source for
/// each `*.json` file of `dir`, by name. A subdirectory is not searched.
// Port of: dm/DM.cpp#L898-L916 (chrome/m156)
#[must_use]
pub fn collect_lotties(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_ids_follow_dm() {
        let src = SkottieSrc::new("resources/skottie/skottie-3d-2planes.json");
        assert_eq!(src.name(), "skottie-3d-2planes.json");
        assert_eq!(src.id_dir(), "lottie");
        assert_eq!(src.size(), ISize::new(1000, 1000));
        assert_eq!(
            Config::N32.src_result_id(&src.id_dir(), &src.name()),
            "8888/lottie/skottie-3d-2planes.json"
        );
    }
}
