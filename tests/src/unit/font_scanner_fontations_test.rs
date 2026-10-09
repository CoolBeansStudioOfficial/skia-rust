// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/FontScanner_FontationsTest.cpp (chrome/m156), with the shared scanner checks of
// tests/FontScanner.cpp that it calls (ported below, as the same functions).

#![cfg(test)]

use std::collections::HashSet;

use skia_rust_core::font_parameters::variation::Axis;
use skia_rust_core::font_style::{FontStyle, Weight};
use skia_rust_core::font_types::set_four_byte_tag;
use skia_rust_core::stream::{MemoryStream, StreamAsset};
use skia_rust_text::font_scanner::{FontScanner, InstanceInfo};
use skia_rust_text::ports::fontations::font_scanner::new_fontations;

use crate::resources::get_resource_as_data;
use crate::{Reporter, def_test, errorf, reporter_assert, skip_missing_resource};

/// `GetResourcePath`/`SkStream::MakeFromFile` and `GetResourceAsStream`: the resource as a
/// stream, or `None` if it is missing.
fn resource_stream(path: &str) -> Option<Box<dyn StreamAsset>> {
    let data = get_resource_as_data(path)?;
    Some(MemoryStream::make_copy(&data))
}

/// The style of an instance as C++ prints it, `(weight width slant)`, for the failure messages.
fn describe(name: &str, style: FontStyle) -> String {
    format!(
        "{name} ({:?} {:?} {:?})",
        style.weight(),
        style.width(),
        style.slant()
    )
}

/// `FontScanner_VariableFont` (tests/FontScanner.cpp#L16-L68).
// Port of: tests/FontScanner.cpp#L16-L68 (chrome/m156)
fn font_scanner_variable_font(reporter: &mut Reporter, scanner: &dyn FontScanner) {
    let mut stream =
        skip_missing_resource!(resource_stream("fonts/Variable.ttf"), "fonts/Variable.ttf");
    let num_faces = scanner.scan_file(stream.as_mut());
    if num_faces.is_none() {
        errorf!(reporter, "Cannot scanFile");
    }
    let num_faces = num_faces.unwrap_or(0);
    reporter_assert!(reporter, num_faces == 1);
    let mut unique_styles: HashSet<FontStyle> = HashSet::new();
    for face_index in 0..num_faces {
        let Some(num_instances) = scanner.scan_face(stream.as_mut(), face_index) else {
            errorf!(reporter, "Cannot scanFace");
            continue;
        };
        reporter_assert!(reporter, num_instances == 5);
        // Not including the default instance.
        for instance_index in 1..=num_instances {
            let Some(info) = scanner.scan_instance(stream.as_mut(), face_index, instance_index)
            else {
                errorf!(reporter, "Cannot scanInstance Variable.ttf {face_index}");
                continue;
            };
            if !unique_styles.insert(info.style) {
                errorf!(reporter, "Font: {}", describe(&info.name, info.style));
            }
        }
        reporter_assert!(
            reporter,
            usize::try_from(num_instances).ok() == Some(unique_styles.len())
        );
    }
}

/// `FontScanner_NamedInstances1` (tests/FontScanner.cpp#L70-L135).
// Port of: tests/FontScanner.cpp#L70-L135 (chrome/m156)
fn font_scanner_named_instances1(reporter: &mut Reporter, scanner: &dyn FontScanner) {
    let mut stream =
        skip_missing_resource!(resource_stream("fonts/Variable.ttf"), "fonts/Variable.ttf");
    let num_faces = scanner.scan_file(stream.as_mut());
    if num_faces.is_none() {
        errorf!(reporter, "Cannot scanFile");
    }
    let num_faces = num_faces.unwrap_or(0);
    reporter_assert!(reporter, num_faces == 1);
    let mut unique_styles: HashSet<FontStyle> = HashSet::new();
    for face_index in 0..num_faces {
        let Some(num_instances) = scanner.scan_face(stream.as_mut(), face_index) else {
            errorf!(reporter, "Cannot scanFace");
            continue;
        };
        reporter_assert!(reporter, num_instances == 5);
        // Not including the default instance (most time it will be listed anyway).
        for instance_index in 1..=num_instances {
            let Some(info) = scanner.scan_instance(stream.as_mut(), face_index, instance_index)
            else {
                errorf!(reporter, "Cannot scanInstance Variable.ttf {face_index}");
                continue;
            };
            if unique_styles.insert(info.style) {
                reporter_assert!(reporter, info.axes.len() == 2);
                if instance_index == 5 {
                    check_weight_and_width_axes(reporter, &info);
                }
            } else {
                errorf!(
                    reporter,
                    "Font #{instance_index}: {}",
                    describe(&info.name, info.style)
                );
            }
        }
    }
}

/// The checks of `FontScanner_NamedInstances1` on instance 5 of `Variable.ttf`: `wght` then
/// `wdth`, with their defaults and ranges.
// Port of: tests/FontScanner.cpp#L100-L113 (chrome/m156)
#[allow(clippy::float_cmp)] // the axis values are copied from the font, as in C++
fn check_weight_and_width_axes(reporter: &mut Reporter, info: &InstanceInfo) {
    let weight = set_four_byte_tag(b'w', b'g', b'h', b't');
    let width = set_four_byte_tag(b'w', b'd', b't', b'h');
    let axes: &[Axis] = &info.axes;
    reporter_assert!(reporter, axes[0].tag == weight);
    reporter_assert!(reporter, axes[0].def == 400.0);
    reporter_assert!(reporter, axes[0].min == 100.0);
    reporter_assert!(reporter, axes[0].max == 900.0);
    reporter_assert!(reporter, axes[1].tag == width);
    reporter_assert!(reporter, axes[1].def == 100.0);
    reporter_assert!(reporter, axes[1].min == 50.0);
    reporter_assert!(reporter, axes[1].max == 200.0);
}

/// `FontScanner_NamedInstances2` (tests/FontScanner.cpp#L137-L176).
// Port of: tests/FontScanner.cpp#L137-L176 (chrome/m156)
#[allow(clippy::float_cmp)] // the axis values are copied from the font, as in C++
fn font_scanner_named_instances2(reporter: &mut Reporter, scanner: &dyn FontScanner) {
    let mut stream = skip_missing_resource!(
        resource_stream("fonts/VaryAlongQuads.ttf"),
        "fonts/VaryAlongQuads.ttf"
    );
    let num_faces = scanner.scan_file(stream.as_mut());
    if num_faces.is_none() {
        errorf!(reporter, "Cannot scanFile");
    }
    let num_faces = num_faces.unwrap_or(0);
    reporter_assert!(reporter, num_faces == 1);
    for face_index in 0..num_faces {
        let Some(num_instances) = scanner.scan_face(stream.as_mut(), face_index) else {
            errorf!(reporter, "Cannot scanFace");
            continue;
        };
        reporter_assert!(reporter, num_instances == 3);
        // Not including the default instance (most time it will be listed anyway).
        for instance_index in 1..=num_instances {
            let Some(info) = scanner.scan_instance(stream.as_mut(), face_index, instance_index)
            else {
                errorf!(
                    reporter,
                    "Cannot scanInstance VaryAlongQuads.ttf {face_index}"
                );
                continue;
            };
            reporter_assert!(reporter, info.axes.len() == 2);
            let weight = set_four_byte_tag(b'w', b'g', b'h', b't');
            for axis in &info.axes {
                reporter_assert!(
                    reporter,
                    instance_index != 1 || info.style.weight() == Weight::from(100)
                );
                reporter_assert!(
                    reporter,
                    instance_index != 2 || info.style.weight() == Weight::from(400)
                );
                reporter_assert!(
                    reporter,
                    instance_index != 3 || info.style.weight() == Weight::from(900)
                );
                reporter_assert!(reporter, axis.tag == weight);
                reporter_assert!(reporter, axis.def == 400.0);
                reporter_assert!(reporter, axis.min == 100.0);
                reporter_assert!(reporter, axis.max == 900.0);
            }
        }
    }
}

/// `FontScanner_FontCollection` (tests/FontScanner.cpp#L178-L212).
// Port of: tests/FontScanner.cpp#L178-L212 (chrome/m156)
fn font_scanner_font_collection(reporter: &mut Reporter, scanner: &dyn FontScanner) {
    let mut stream = skip_missing_resource!(resource_stream("fonts/test.ttc"), "fonts/test.ttc");
    let num_faces = scanner.scan_file(stream.as_mut());
    if num_faces.is_none() {
        errorf!(reporter, "Cannot scanFile");
    }
    let num_faces = num_faces.unwrap_or(0);
    reporter_assert!(reporter, num_faces == 2);
    for face_index in 0..num_faces {
        let Some(num_instances) = scanner.scan_face(stream.as_mut(), face_index) else {
            errorf!(reporter, "Cannot scanFace");
            continue;
        };
        reporter_assert!(reporter, num_instances == 0);
        let default_instance = 0;
        let Some(info) = scanner.scan_instance(stream.as_mut(), face_index, default_instance)
        else {
            errorf!(reporter, "Cannot scanInstance test.ttc {face_index}");
            continue;
        };
        reporter_assert!(reporter, info.axes.is_empty());
        reporter_assert!(
            reporter,
            face_index != 0 || info.style.weight() == Weight::from(400)
        );
        reporter_assert!(
            reporter,
            face_index != 1 || info.style.weight() == Weight::from(700)
        );
    }
}

// Port of: tests/FontScanner_FontationsTest.cpp#L10-L13 (chrome/m156)
def_test!(FontScanner_Fontations_VariableFont, |reporter| {
    font_scanner_variable_font(reporter, new_fontations().as_ref());
});

// Port of: tests/FontScanner_FontationsTest.cpp#L15-L18 (chrome/m156)
def_test!(FontScanner_Fontations_NamedInstances1, |reporter| {
    font_scanner_named_instances1(reporter, new_fontations().as_ref());
});

// Port of: tests/FontScanner_FontationsTest.cpp#L20-L23 (chrome/m156)
def_test!(FontScanner_Fontations_NamedInstances2, |reporter| {
    font_scanner_named_instances2(reporter, new_fontations().as_ref());
});

// Port of: tests/FontScanner_FontationsTest.cpp#L25-L28 (chrome/m156)
def_test!(FontScanner_Fontations_FontCollection, |reporter| {
    font_scanner_font_collection(reporter, new_fontations().as_ref());
});
