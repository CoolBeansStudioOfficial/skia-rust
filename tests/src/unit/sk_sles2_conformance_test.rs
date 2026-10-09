// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkSLES2ConformanceTest.cpp (chrome/m156)

//! `SkSLES2ConformanceTest`: the GLSL ES2 conformance shaders, run as runtime shaders.
//!
//! This test relies on GLSL ES2 conformance test files, which are not included in Skia (see
//! `resources/sksl/es2_conformance/import_conformance_tests.py`). Without them the directories
//! are empty, and both tests pass vacuously, as in Skia. The Ganesh variant is a GPU test.

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{Options, RuntimeEffect, RuntimeShaderBuilder};
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

use crate::resources::{get_resource_as_data, resource_dir};
use crate::{Reporter, def_test, errorf, reporter_assert};

/// `SkOSPath::Basename`.
fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

// Port of: tests/SkSLES2ConformanceTest.cpp#L35-L50 (chrome/m156)
fn test_expect_fail(r: &mut Reporter, test_file: &str) {
    let options = Options::default();
    let Some(shader_data) = get_resource_as_data(test_file) else {
        errorf!(r, "{}: Unable to load file", basename(test_file));
        return;
    };

    let shader_string = String::from_utf8_lossy(&shader_data).into_owned();
    let result = RuntimeEffect::make_for_shader(&shader_string, Some(&options));
    if result.is_ok() {
        errorf!(
            r,
            "{}: Expected failure, but compiled successfully",
            basename(test_file)
        );
    }
}

// Port of: tests/SkSLES2ConformanceTest.cpp#L52-L90 (chrome/m156)
fn test_expect_pass(r: &mut Reporter, surface: &mut Surface<'_>, test_file: &str) {
    let options = Options::default();
    let Some(shader_data) = get_resource_as_data(test_file) else {
        errorf!(r, "{}: Unable to load file", test_file);
        return;
    };

    let shader_string = String::from_utf8_lossy(&shader_data).into_owned();
    let effect = match RuntimeEffect::make_for_shader(&shader_string, Some(&options)) {
        Ok(effect) => effect,
        Err(error_text) => {
            errorf!(r, "{}: {}", test_file, error_text);
            return;
        }
    };

    let builder = RuntimeShaderBuilder::new(effect);
    let Some(shader) = builder.make_shader(None) else {
        errorf!(r, "{}: Unable to build shader", test_file);
        return;
    };

    let mut paint_shader = Paint::default();
    paint_shader.set_shader(shader);
    surface
        .canvas()
        .draw_rect(Rect::from_wh(1.0, 1.0), &paint_shader);

    let mut bitmap = Bitmap::new();
    reporter_assert!(r, bitmap.try_alloc_pixels_info(&surface.image_info(), None));
    reporter_assert!(r, surface.read_pixels_to_bitmap(&mut bitmap, (0, 0)));

    let color = bitmap.get_color((0, 0));
    if color != Color::from_argb(0xFF, 0x00, 0xFF, 0x00) {
        errorf!(
            r,
            "{}: Expected solid green. Actual:\nRRGGBBAA\n{:02X}{:02X}{:02X}{:02X}",
            test_file,
            color.r(),
            color.g(),
            color.b(),
            color.a()
        );
    }
}

// Port of: tests/SkSLES2ConformanceTest.cpp#L92-L101 (chrome/m156)
fn iterate_dir(directory: &str, run: &mut dyn FnMut(&str)) {
    let Some(resources) = resource_dir() else {
        return;
    };
    let mut resource_directory = resources;
    for component in directory.split('/').filter(|c| !c.is_empty()) {
        resource_directory.push(component);
    }
    let Ok(entries) = std::fs::read_dir(&resource_directory) else {
        return;
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|t| !t.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| {
            std::path::Path::new(name)
                .extension()
                .is_some_and(|e| e == "rts")
        })
        .collect();
    names.sort();
    for name in names {
        let path = format!("{directory}{name}");
        run(&path);
    }
}

// Port of: tests/SkSLES2ConformanceTest.cpp#L103-L111 (chrome/m156)
def_test!(SkSL_ES2Conformance_Pass_CPU, |r| {
    let mut surface = surfaces::raster_n32_premul((1, 1)).expect("a raster surface");

    iterate_dir("sksl/es2_conformance/pass/", &mut |path| {
        test_expect_pass(r, &mut surface, path);
    });
});

// Port of: tests/SkSLES2ConformanceTest.cpp#L127-L131 (chrome/m156)
def_test!(SkSL_ES2Conformance_Fail, |r| {
    iterate_dir("sksl/es2_conformance/fail/", &mut |path| {
        test_expect_fail(r, path);
    });
});
