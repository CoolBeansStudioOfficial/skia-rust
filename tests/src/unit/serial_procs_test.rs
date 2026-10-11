// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SerialProcsTest.cpp (chrome/m156)
//
// Not ported yet (each is its own manifest entry and stays `todo`):
// - `serial_procs_image`, `serial_typeface`.

use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::data::Data;
use skia_rust_core::font::Font;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_core::serial_procs::{DeserialProcs, SerialProcs};
use skia_rust_core::stream::MemoryStream;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::typeface::Typeface;
use skia_rust_effects::flattenable::REGISTRY;
use skia_rust_raster::picture_shader::PictureShaderExt;
use skia_rust_tools::font_tool_utils::create_typeface_from_resource;

use crate::resources::get_resource_as_data;
use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/SerialProcsTest.cpp#L132-L136 (chrome/m156)
fn make_pic(drawer: impl FnOnce(&Canvas)) -> Picture {
    let mut rec = PictureRecorder::new();
    drawer(rec.begin_recording(Rect::from_wh(128.0, 128.0), false));
    rec.finish_recording_as_picture(None).expect("picture")
}

// packages the picture's point in the skdata, and records it in the ctx as an array
//
// skia-rust: the C++ ctx holds `SkPicture*` and the data holds the pointer's bytes; here the ctx
// holds the pictures themselves, and the data holds the unique ID of the picture, which names the
// same object.
// Port of: tests/SerialProcsTest.cpp#L151-L154 (chrome/m156)
#[derive(Default)]
struct Context {
    array: Vec<Picture>,
    skip_me: Option<u32>,
}

// Port of: tests/SerialProcsTest.cpp#L156-L163 (chrome/m156)
fn makes(ctx: &Arc<Mutex<Context>>) -> SerialProcs {
    let ctx = Arc::clone(ctx);
    SerialProcs {
        // array_serial_proc
        picture: Some(Arc::new(move |pic: &Picture| {
            let mut c = ctx.lock().unwrap_or_else(PoisonError::into_inner);
            if c.skip_me == Some(pic.unique_id()) {
                return None;
            }
            c.array.push(pic.clone());
            Some(Data::new_copy(&pic.unique_id().to_ne_bytes()))
        })),
        ..SerialProcs::default()
    }
}

// Port of: tests/SerialProcsTest.cpp#L165-L180 (chrome/m156)
fn maked(ctx: &Arc<Mutex<Context>>) -> DeserialProcs {
    let ctx = Arc::clone(ctx);
    DeserialProcs {
        // array_deserial_proc
        picture: Some(Arc::new(move |data: &[u8]| {
            let id = u32::from_ne_bytes(data.try_into().expect("sizeof(pic) == size"));

            let mut c = ctx.lock().unwrap_or_else(PoisonError::into_inner);
            let found = c.array.iter().position(|p| p.unique_id() == id);
            debug_assert!(found.is_some());
            found.map(|found| c.array.swap_remove(found)) // removeShuffle, then sk_ref_sp(pic)
        })),
        ..DeserialProcs::default()
    }
}

// Port of: tests/SerialProcsTest.cpp#L182-L199 (chrome/m156)
fn test_pictures(reporter: &mut Reporter, p0: &Picture, count: usize, skip_root: bool) {
    let ctx = Arc::new(Mutex::new(Context::default()));
    if skip_root {
        ctx.lock().unwrap().skip_me = Some(p0.unique_id());
    }

    let sprocs = makes(&ctx);
    let d0 = p0.serialize(Some(&sprocs)).expect("serialized");
    reporter_assert!(reporter, ctx.lock().unwrap().array.len() == count);
    let dprocs = maked(&ctx);
    let _p0 = Picture::from_data_with_registry(d0.as_bytes(), Some(&dprocs), &REGISTRY);
    reporter_assert!(reporter, ctx.lock().unwrap().array.is_empty());
}

// Port of: tests/SerialProcsTest.cpp#L201-L230 (chrome/m156)
def_test!(serial_procs_picture, |reporter| {
    let p1 = make_pic(|c| {
        // need to be large enough that drawPictures doesn't "unroll" us
        for _ in 0..20 {
            c.draw_color(Color::RED, None);
        }
    });

    // now use custom serialization
    let p0 = make_pic(|c| {
        c.draw_color(Color::BLUE, None);
    });
    test_pictures(reporter, &p0, 1, false);

    // test inside effect
    let p0 = make_pic(|c| {
        let mut paint = Paint::default();
        let tm = TileMode::Clamp;
        paint.set_shader(p1.to_shader((tm, tm), FilterMode::Nearest, None, None));
        c.draw_paint(&paint);
    });
    test_pictures(reporter, &p0, 1, true);

    // test nested picture
    let p0 = make_pic(|c| {
        c.draw_color(Color::RED, None);
        c.draw_picture(&p1, None, None);
        c.draw_color(Color::BLUE, None);
    });
    test_pictures(reporter, &p0, 1, true);
});

// Port of: tests/SerialProcsTest.cpp#L232-L242 (chrome/m156), make_picture
fn make_typeface_picture(tf0: &Typeface, tf1: &Typeface) -> Picture {
    let mut rec = PictureRecorder::new();
    let canvas = rec.begin_recording(Rect::from_wh(100.0, 100.0), false);
    let paint = Paint::default();
    let mut font = Font::default();
    font.set_typeface(Some(tf0.clone()));
    canvas.draw_str("hello", (0.0, 0.0), &font, &paint);
    font.set_typeface(Some(tf1.clone()));
    canvas.draw_str("hello", (0.0, 0.0), &font, &paint);
    font.set_typeface(Some(tf0.clone()));
    canvas.draw_str("hello", (0.0, 0.0), &font, &paint);
    font.set_typeface(Some(tf1.clone()));
    canvas.draw_str("hello", (0.0, 0.0), &font, &paint);
    rec.finish_recording_as_picture(None).expect("picture")
}

/// `ToolUtils::CreateTypefaceFromResource(path)` (`None` in the portable configuration).
fn create_typeface_from_resource_path(path: &str) -> Option<Typeface> {
    let data = get_resource_as_data(path)?;
    create_typeface_from_resource(Some(MemoryStream::make_copy(&data)), 0)
}

// Port of: tests/SerialProcsTest.cpp#L244-L265 (chrome/m156), serial_typeface
def_test!(serial_typeface, |reporter| {
    let tf0 = create_typeface_from_resource_path("fonts/hintgasp.ttf");
    let tf1 = create_typeface_from_resource_path("fonts/Roboto2-Regular_NoEmbed.ttf");
    let (Some(tf0), Some(tf1)) = (tf0, tf1) else {
        return; // need two different typefaces for this test to make sense.
    };
    if tf0.unique_id() == tf1.unique_id() {
        return; // need two different typefaces for this test to make sense.
    }

    let pic = make_typeface_picture(&tf0, &tf1);

    let counter = Arc::new(Mutex::new(0));
    let ctx = Arc::clone(&counter);
    let procs = SerialProcs {
        typeface: Some(Arc::new(move |_tf: &Typeface| {
            *ctx.lock().unwrap_or_else(PoisonError::into_inner) += 1;
            None
        })),
        ..SerialProcs::default()
    };
    let _data = pic.serialize(Some(&procs));

    // The picture has 2 references to each typeface, but we want the serialized picture to
    // only have written the data 1 time per typeface.
    reporter_assert!(
        reporter,
        *counter.lock().unwrap_or_else(PoisonError::into_inner) == 2
    );
});
