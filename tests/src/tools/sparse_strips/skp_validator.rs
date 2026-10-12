// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/sparse_strips/SkpValidator.h, .cpp (chrome/m156),
//          tools/ToolUtils.cpp#L748-L775 (ExtractPathsFromSKP)

//! [`SkpValidator`]: validates the sparse strips of every path of an SKP against the oracle.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::too_many_arguments
)]

use std::cell::RefCell;
use std::rc::Rc;

use skia_rust_core::canvas::{Canvas, CanvasHooks};
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_utils::fill_path_with_paint_to_path;
use skia_rust_core::picture::Picture;
use skia_rust_core::stream::{FileStream, Stream};
use skia_rust_gpu::graphite::geom::end_caps::EndCaps;
use skia_rust_gpu::graphite::geom::wide_tiles::WideTiles;
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::sparse_strips::alpha_atlas_manager::AlphaAtlasManager;
use skia_rust_gpu::graphite::sparse_strips::flatten::Flatten;
use skia_rust_gpu::graphite::sparse_strips::make_strips::MakeStrips;
use skia_rust_gpu::graphite::sparse_strips::polyline::Polyline;
use skia_rust_gpu::graphite::sparse_strips::tiler::Tiles;
use skia_rust_gpu::graphite::sparse_strips::types::FlattenMode;
use skia_rust_simd::vx::Int8;

use super::oracle_validator::{OracleValidator, Strictness};
use crate::{Reporter, infof};

/// `SkpValidator::ExtractedPath`.
// Port of: tests/graphite/sparse_strips/SkpValidator.h#L31-L36 (chrome/m156)
#[derive(Clone, Debug)]
pub struct ExtractedPath {
    /// `path`.
    pub path: Path,
    /// `paint`.
    pub paint: Paint,
    /// `matrix`.
    pub matrix: Matrix,
    /// `devicePath`.
    pub device_path: Path,
}

/// `ToolUtils::PathSniffCallback`.
// Port of: tools/ToolUtils.h (chrome/m156), PathSniffCallback
type PathSniffCallback = dyn FnMut(&Matrix, &Path, &Paint);

/// The `PathSniffer` canvas: reports every path that is drawn with the matrix it is drawn with.
///
/// The C++ subclass of `SkCanvas` overrides `onDrawPath` and asks the canvas for
/// `getTotalMatrix()`. Here the canvas's hooks (`CanvasHooks`) track the matrix through the
/// notifications of the matrix changes and of the saves and restores instead, as a hook cannot
/// ask the canvas for its matrix while the canvas is drawing.
// Port of: tools/ToolUtils.cpp#L755-L765 (chrome/m156)
struct PathSniffer {
    callback: Box<PathSniffCallback>,
    total_matrix: M44,
    saved_matrices: Vec<M44>,
}

impl PathSniffer {
    /// `PathSniffer(callback)`: `SkCanvas(4096, 4096, nullptr)` with the sniffer's hooks.
    fn make(callback: Box<PathSniffCallback>) -> Canvas {
        let canvas = Canvas::new_no_pixels((4096, 4096), None).expect("a 4096x4096 canvas");
        canvas.set_hooks(Some(Box::new(PathSniffer {
            callback,
            total_matrix: M44::new_identity(),
            saved_matrices: Vec::new(),
        })));
        canvas
    }
}

impl CanvasHooks for PathSniffer {
    fn will_save(&mut self) {
        self.saved_matrices.push(self.total_matrix);
    }

    fn did_restore(&mut self, total_matrix: &Matrix) {
        self.saved_matrices.pop();
        self.total_matrix = M44::from(total_matrix);
    }

    fn did_concat44(&mut self, m: &M44) {
        self.total_matrix.pre_concat(m);
    }

    fn did_set_m44(&mut self, m: &M44) {
        self.total_matrix = *m;
    }

    fn did_scale(&mut self, sx: f32, sy: f32) {
        self.total_matrix.pre_scale(sx, sy);
    }

    fn did_translate(&mut self, dx: f32, dy: f32) {
        self.total_matrix.pre_translate(dx, dy, None);
    }

    // Port of: tools/ToolUtils.cpp#L761-L763 (chrome/m156)
    fn on_draw_path(&mut self, path: &Path, paint: &Paint) -> bool {
        (self.callback)(&self.total_matrix.to_m33(), path, paint);
        true
    }
}

/// `ToolUtils::ExtractPathsFromSKP(filepath, callback)`.
// Port of: tools/ToolUtils.cpp#L748-L775 (chrome/m156)
#[doc(alias = "ExtractPathsFromSKP")]
fn extract_paths_from_skp(filepath: &str, callback: Box<PathSniffCallback>) {
    let mut stream = FileStream::new(filepath);
    if !stream.is_valid() {
        eprintln!("ExtractPaths: invalid input file at \"{filepath}\"");
        return;
    }

    // We don't need to decode images etc, so we can pass nullptr for the deserial procs.
    let Some(skp) = Picture::from_stream(&mut stream, None) else {
        eprintln!("ExtractPaths: couldn't load skp at \"{filepath}\"");
        return;
    };
    let path_sniffer = PathSniffer::make(callback);
    skp.playback(&path_sniffer);
}

/// A callback that records every sniffed path as an [`ExtractedPath`] into `paths`.
fn collecting_callback(paths: &Rc<RefCell<Vec<ExtractedPath>>>) -> Box<PathSniffCallback> {
    let paths = Rc::clone(paths);
    Box::new(move |ctm: &Matrix, path: &Path, paint: &Paint| {
        let (fill_path, _is_fill) = fill_path_with_paint_to_path(path, paint);
        let dev_path = fill_path.make_transform(ctm);
        paths.borrow_mut().push(ExtractedPath {
            path: path.clone(),
            paint: paint.clone(),
            matrix: ctm.clone(),
            device_path: dev_path,
        });
    })
}

/// `SkpValidator`.
// Port of: tests/graphite/sparse_strips/SkpValidator.h#L28-L56 (chrome/m156)
#[derive(Clone, Copy, Debug)]
pub struct SkpValidator;

impl SkpValidator {
    /// `ExtractPaths(filepath)`.
    // Port of: tests/graphite/sparse_strips/SkpValidator.cpp#L41-L50 (chrome/m156)
    #[doc(alias = "ExtractPaths")]
    #[must_use]
    pub fn extract_paths(filepath: &str) -> Vec<ExtractedPath> {
        let paths = Rc::new(RefCell::new(Vec::new()));
        extract_paths_from_skp(filepath, collecting_callback(&paths));
        Rc::try_unwrap(paths)
            .map(RefCell::into_inner)
            .unwrap_or_default()
    }

    /// `ExtractPaths(stream)`.
    // Port of: tests/graphite/sparse_strips/SkpValidator.cpp#L47-L56 (chrome/m156)
    #[doc(alias = "ExtractPaths")]
    #[must_use]
    pub fn extract_paths_from_stream(stream: Option<&mut dyn Stream>) -> Vec<ExtractedPath> {
        let Some(stream) = stream else {
            return Vec::new();
        };
        let Some(picture) = Picture::from_stream(stream, None) else {
            return Vec::new();
        };
        Self::extract_paths_from_picture(Some(&picture))
    }

    /// `ExtractPaths(picture)`.
    // Port of: tests/graphite/sparse_strips/SkpValidator.cpp#L58-L87 (chrome/m156)
    #[doc(alias = "ExtractPaths")]
    #[must_use]
    pub fn extract_paths_from_picture(picture: Option<&Picture>) -> Vec<ExtractedPath> {
        let Some(picture) = picture else {
            return Vec::new();
        };

        let paths = Rc::new(RefCell::new(Vec::new()));
        let sniffer = PathSniffer::make(collecting_callback(&paths));
        picture.playback(&sniffer);
        drop(sniffer);
        Rc::try_unwrap(paths)
            .map(RefCell::into_inner)
            .unwrap_or_default()
    }

    /// `ValidatePath<kTileWidth, kTileHeight>(reporter, recorder, path, testName, maskLut,
    /// minorErrorCount)`.
    // Port of: tests/graphite/sparse_strips/SkpValidator.cpp#L90-L175 (chrome/m156)
    #[doc(alias = "ValidatePath")]
    pub fn validate_path<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16>(
        reporter: &mut Reporter,
        recorder: &Recorder,
        path: &Path,
        test_name: &str,
        mask_lut: &[u8],
        minor_error_count: Option<&mut [u32; 3]>,
    ) -> bool {
        if path.is_empty() || !path.is_finite() {
            return true;
        }

        let bounds = *path.bounds();
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            return true;
        }

        let margin = 4.0f32;
        let mut trans = Matrix::new_identity();
        trans.set_translate((margin - bounds.left, margin - bounds.top));
        let local_path = path.make_transform(&trans);
        let local_bounds = *local_path.bounds();

        let raw_width = (local_bounds.right + margin).ceil() as u16;
        let raw_height = (local_bounds.bottom + margin).ceil() as u16;

        let vp_width =
            (u32::from(raw_width) + u32::from(K_TILE_WIDTH) - 1) & !(u32::from(K_TILE_WIDTH) - 1);
        let vp_height = (u32::from(raw_height) + u32::from(K_TILE_HEIGHT) - 1)
            & !(u32::from(K_TILE_HEIGHT) - 1);
        let vp_width = vp_width as u16;
        let vp_height = vp_height as u16;

        let mut flattener = Flatten::new();
        let mut polyline = Polyline::new();
        flattener.process_paths(
            FlattenMode::Simd,
            &local_path,
            &Matrix::new_identity(),
            f32::from(vp_width),
            f32::from(vp_height),
            &mut polyline,
        );

        if polyline.count() == 0 {
            return true;
        }

        let mut tiler = Tiles::<K_TILE_WIDTH, K_TILE_HEIGHT>::new();
        tiler.make_tiles_msaa(&polyline, vp_width, vp_height);
        tiler.sort_tiles();

        if tiler.get_tiles().is_empty() {
            return true;
        }

        let mut exact_windings: Vec<Int8> = Vec::new();

        let mut wides = WideTiles::new();
        let mut ends = EndCaps::new();
        let mut atlas_manager = AlphaAtlasManager::new();
        {
            let mut observer = |_exact_mask: u8, winding: Int8| {
                exact_windings.push(winding);
            };
            MakeStrips::msaa_simd::<K_TILE_WIDTH, K_TILE_HEIGHT>(
                &tiler,
                &mut wides,
                &mut ends,
                &mut atlas_manager,
                recorder,
                local_path.fill_type(),
                &polyline,
                mask_lut,
                vp_width,
                vp_height,
                Some(&mut observer),
            );
        }

        let validator = OracleValidator::<K_TILE_WIDTH, K_TILE_HEIGHT>::new(
            &local_path,
            &wides,
            &ends,
            &exact_windings,
            Some(&polyline),
            Some(&tiler),
            test_name,
            Strictness::NonStrict,
        );

        let mut max_sample_diff: u32 = 0;
        let pass = validator.validate(reporter, vp_width, vp_height, Some(&mut max_sample_diff));
        if let Some(minor_error_count) = minor_error_count
            && max_sample_diff > 0
            && max_sample_diff <= 3
        {
            minor_error_count[max_sample_diff as usize - 1] += 1;
        }
        pass
    }

    /// `ValidateSkp<kTileWidth, kTileHeight>(reporter, recorder, filepath, maskLut)`.
    // Port of: tests/graphite/sparse_strips/SkpValidator.cpp#L177-L215 (chrome/m156)
    #[doc(alias = "ValidateSkp")]
    pub fn validate_skp<const K_TILE_WIDTH: u16, const K_TILE_HEIGHT: u16>(
        reporter: &mut Reporter,
        recorder: &Recorder,
        filepath: &str,
        mask_lut: &[u8],
    ) -> bool {
        let paths = Self::extract_paths(filepath);
        if paths.is_empty() {
            infof!(
                reporter,
                "[SkpValidator] SKP file not found or empty: {}",
                filepath
            );
            return true;
        }

        let count = paths.len();
        let mut minor_errors = [0u32; 3];
        let mut passed = 0;

        for (i, extracted) in paths.iter().enumerate() {
            let name = format!("{filepath}_path_{i}");
            if Self::validate_path::<K_TILE_WIDTH, K_TILE_HEIGHT>(
                reporter,
                recorder,
                &extracted.device_path,
                &name,
                mask_lut,
                Some(&mut minor_errors),
            ) {
                passed += 1;
            }
        }

        infof!(
            reporter,
            "[SIMD ({}x{})] Tested {} / {} paths from '{}'. Passed: {}. Minor Errors: [1s:{}, \
             2s:{}, 3s:{}]\n",
            K_TILE_WIDTH,
            K_TILE_HEIGHT,
            count,
            paths.len(),
            filepath,
            passed,
            minor_errors[0],
            minor_errors[1],
            minor_errors[2]
        );

        passed == count
    }
}
