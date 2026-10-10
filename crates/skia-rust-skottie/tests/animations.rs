// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
//
// Robustness and behavior tests of the animation builder that are not ports of Skia tests: small
// animations with known pixels, and every Lottie file of Skia's `resources/skottie` built, seeked
// and rendered at several times.

use std::path::PathBuf;
use std::rc::Rc;

use skia_rust_core::color::Color;
use skia_rust_core::rect::Rect;
use skia_rust_raster::surfaces;
use skia_rust_skottie::{Animation, Builder, Logger, LoggerLevel, RenderFlags};

/// Renders the animation at `frame` into a `w` x `h` surface, and returns the color at each of
/// `points`.
fn render_colors(anim: &Animation, frame: f64, size: i32, points: &[(i32, i32)]) -> Vec<Color> {
    let mut surface = surfaces::raster_n32_premul((size, size)).expect("a surface");
    surface.canvas().clear(Color::new(0xff00_0000));
    anim.seek_frame(frame);
    anim.render(surface.canvas(), None);
    let peeked = surface.peek_pixels().expect("pixels");
    let pixmap = peeked.pixmap();
    points
        .iter()
        .map(|&(x, y)| pixmap.get_color((x, y)))
        .collect()
}

#[test]
fn solid_layer_fills_its_rect() {
    let json = r##"{
        "v": "5.2.1", "w": 10, "h": 10, "fr": 10, "ip": 0, "op": 10,
        "layers": [{
            "ty": 1, "ind": 0, "ip": 0, "op": 10, "ks": {},
            "sw": 5, "sh": 10, "sc": "#ff0000"
        }]
    }"##;
    let anim = Animation::from_str(json).expect("an animation");
    let colors = render_colors(&anim, 0.0, 10, &[(2, 5), (8, 5)]);
    assert_eq!(colors[0], Color::new(0xffff_0000));
    assert_eq!(colors[1], Color::new(0xff00_0000));
}

#[test]
fn layer_is_only_visible_between_in_and_out_points() {
    let json = r##"{
        "v": "5.2.1", "w": 10, "h": 10, "fr": 10, "ip": 0, "op": 10,
        "layers": [{
            "ty": 1, "ind": 0, "ip": 2, "op": 4, "ks": {},
            "sw": 10, "sh": 10, "sc": "#00ff00"
        }]
    }"##;
    let anim = Animation::from_str(json).expect("an animation");
    assert_eq!(
        render_colors(&anim, 1.0, 10, &[(5, 5)])[0],
        Color::new(0xff00_0000)
    );
    assert_eq!(
        render_colors(&anim, 2.0, 10, &[(5, 5)])[0],
        Color::new(0xff00_ff00)
    );
    assert_eq!(
        render_colors(&anim, 3.5, 10, &[(5, 5)])[0],
        Color::new(0xff00_ff00)
    );
    assert_eq!(
        render_colors(&anim, 4.0, 10, &[(5, 5)])[0],
        Color::new(0xff00_0000)
    );
}

#[test]
fn animated_position_moves_the_layer() {
    let json = r##"{
        "v": "5.2.1", "w": 20, "h": 10, "fr": 10, "ip": 0, "op": 11,
        "layers": [{
            "ty": 1, "ind": 0, "ip": 0, "op": 11,
            "ks": { "p": { "a": 1, "k": [
                { "t": 0,  "s": [0, 0], "e": [10, 0] },
                { "t": 10, "s": [10, 0] }
            ] } },
            "sw": 10, "sh": 10, "sc": "#0000ff"
        }]
    }"##;
    let anim = Animation::from_str(json).expect("an animation");
    let at_start = render_colors(&anim, 0.0, 20, &[(5, 5), (15, 5)]);
    assert_eq!(at_start[0], Color::new(0xff00_00ff));
    assert_eq!(at_start[1], Color::new(0xff00_0000));
    let at_end = render_colors(&anim, 10.0, 20, &[(5, 5), (15, 5)]);
    assert_eq!(at_end[0], Color::new(0xff00_0000));
    assert_eq!(at_end[1], Color::new(0xff00_00ff));
}

#[test]
fn shape_layer_draws_a_filled_rectangle() {
    let json = r#"{
        "v": "5.2.1", "w": 10, "h": 10, "fr": 10, "ip": 0, "op": 10,
        "layers": [{
            "ty": 4, "ind": 0, "ip": 0, "op": 10, "ks": {},
            "shapes": [{ "ty": "gr", "it": [
                { "ty": "rc", "p": { "a": 0, "k": [5, 5] }, "s": { "a": 0, "k": [6, 6] },
                  "r": { "a": 0, "k": 0 } },
                { "ty": "fl", "c": { "a": 0, "k": [1, 0, 0, 1] }, "o": { "a": 0, "k": 100 } },
                { "ty": "tr", "o": { "a": 0, "k": 100 } }
            ] }]
        }]
    }"#;
    let anim = Animation::from_str(json).expect("an animation");
    let colors = render_colors(&anim, 0.0, 10, &[(5, 5), (0, 0)]);
    assert_eq!(colors[0], Color::new(0xffff_0000));
    assert_eq!(colors[1], Color::new(0xff00_0000));
}

#[test]
fn precomp_layer_with_time_remapping_and_masks_builds() {
    let json = r##"{
        "v": "5.2.1", "w": 10, "h": 10, "fr": 10, "ip": 0, "op": 10,
        "assets": [{ "id": "comp_0", "layers": [{
            "ty": 1, "ind": 0, "ip": 0, "op": 10, "ks": {},
            "sw": 10, "sh": 10, "sc": "#ffffff",
            "masksProperties": [{ "mode": "a", "inv": false,
                "pt": { "a": 0, "k": { "c": true,
                    "v": [[0,0],[5,0],[5,10],[0,10]],
                    "i": [[0,0],[0,0],[0,0],[0,0]], "o": [[0,0],[0,0],[0,0],[0,0]] } },
                "o": { "a": 0, "k": 100 } }]
        }] }],
        "layers": [{ "ty": 0, "ind": 0, "ip": 0, "op": 10, "ks": {}, "refId": "comp_0",
                     "w": 10, "h": 10, "st": 0, "sr": 1 }]
    }"##;
    let anim = Animation::from_str(json).expect("an animation");
    let colors = render_colors(&anim, 0.0, 10, &[(2, 5), (8, 5)]);
    assert_eq!(colors[0], Color::new(0xffff_ffff));
    assert_eq!(colors[1], Color::new(0xff00_0000));
}

/// Collects the log messages.
struct CollectingLogger {
    messages: std::cell::RefCell<Vec<(LoggerLevel, String)>>,
}

impl Logger for CollectingLogger {
    fn log(&self, level: LoggerLevel, message: &str, _json: Option<&str>) {
        self.messages
            .borrow_mut()
            .push((level, message.to_string()));
    }
}

#[test]
fn invalid_animations_are_logged() {
    let logger = Rc::new(CollectingLogger {
        messages: std::cell::RefCell::new(Vec::new()),
    });
    let anim = Builder::new()
        .set_logger(logger.clone())
        .make(r#"{ "v": "5.2.1", "w": 0, "h": 10, "fr": 10, "ip": 0, "op": 10, "layers": [] }"#);
    assert!(anim.is_none());
    assert_eq!(logger.messages.borrow().len(), 1);
    assert_eq!(logger.messages.borrow()[0].0, LoggerLevel::Error);

    assert!(
        Builder::new()
            .set_logger(logger.clone())
            .make("not json")
            .is_none()
    );
    assert_eq!(logger.messages.borrow().len(), 2);
}

#[test]
fn render_flags_and_destination_rects() {
    let json = r##"{
        "v": "5.2.1", "w": 10, "h": 10, "fr": 10, "ip": 0, "op": 10,
        "layers": [{ "ty": 1, "ind": 0, "ip": 0, "op": 10, "ks": {},
                     "sw": 10, "sh": 10, "sc": "#ffffff" }]
    }"##;
    let anim = Animation::from_str(json).expect("an animation");
    anim.seek_frame(0.0);
    let mut surface = surfaces::raster_n32_premul((20, 20)).expect("a surface");
    surface.canvas().clear(Color::new(0xff00_0000));
    anim.render_with_flags(
        surface.canvas(),
        Some(&Rect::from_xywh(10.0, 10.0, 10.0, 10.0)),
        RenderFlags::SKIP_TOP_LEVEL_ISOLATION,
    );
    let peeked = surface.peek_pixels().expect("pixels");
    let pixmap = peeked.pixmap();
    assert_eq!(pixmap.get_color((15, 15)), Color::new(0xffff_ffff));
    assert_eq!(pixmap.get_color((5, 5)), Color::new(0xff00_0000));
}

/// The `resources/skottie` directory of Skia, if it is checked out.
fn skottie_resources() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(dir) = std::env::var_os("SKIA_RESOURCES") {
        candidates.push(PathBuf::from(dir));
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("third_party")
            .join("skia")
            .join("resources"),
    );
    candidates
        .into_iter()
        .map(|dir| dir.join("skottie"))
        .find(|dir| dir.is_dir())
}

/// Builds, seeks and renders every Lottie file of Skia's resources: none may panic.
#[test]
fn every_lottie_resource_builds_and_renders() {
    let Some(dir) = skottie_resources() else {
        eprintln!("skipping: Skia's resources/skottie is not available");
        return;
    };

    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("a readable directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    assert_ne!(files.len(), 0);

    let mut built = 0;
    for path in files {
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let mut builder = Builder::new();
        let Some(anim) = builder.make(&data) else {
            continue;
        };
        built += 1;

        let mut surface = surfaces::raster_n32_premul((64, 64)).expect("a surface");
        let frames = f64::from(anim.out_point() - anim.in_point());
        for fraction in [0.0, 0.25, 0.5, 0.99] {
            anim.seek_frame(frames * fraction);
            anim.render(surface.canvas(), Some(&Rect::from_wh(64.0, 64.0)));
        }
    }
    assert!(built > 0);
}
