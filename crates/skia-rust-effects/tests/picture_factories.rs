// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Serialized pictures whose paints have effects: the flattenables are written by their factory
//! index, and the factory section names them.

use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_effects::dash_path_effect::new;
use skia_rust_effects::flattenable::REGISTRY;

/// A tag as a picture writes it: its four characters, read as a big-endian word, in native order.
fn push_tag(bytes: &mut Vec<u8>, tag: [u8; 4], size: u32) {
    bytes.extend_from_slice(&u32::from_be_bytes(tag).to_ne_bytes());
    bytes.extend_from_slice(&size.to_ne_bytes());
}

fn push_word(bytes: &mut Vec<u8>, word: u32) {
    bytes.extend_from_slice(&word.to_ne_bytes());
}

fn push_scalar(bytes: &mut Vec<u8>, value: f32) {
    bytes.extend_from_slice(&value.to_ne_bytes());
}

// A paint with a path effect and a mask filter writes both flattenables by factory index, and
// the factory section names them in the order of first use. The bytes follow
// `SkPictureData::serialize` (header, `read`, `fact`, `tpfc`, `aray`, `eof `) and
// `SkPaintPriv::Flatten` with `SkWriteBuffer::writeFlattenable` (index, size, body).
#[test]
fn paint_effects_are_written_by_factory_index() {
    let mut paint = Paint::default();
    paint.set_path_effect(new(&[2.0, 2.0], 0.0).unwrap());
    paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 3.0, true).unwrap());

    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(Rect::new(0.0, 0.0, 10.0, 10.0), false);
    canvas.draw_paint(&paint);
    let picture = recorder.finish_recording_as_picture(None).unwrap();
    let data = picture.serialize(None).unwrap();

    let mut expected = Vec::new();
    expected.extend_from_slice(b"skiapict");
    expected.extend_from_slice(&110u32.to_ne_bytes());
    for edge in [0.0f32, 0.0, 10.0, 10.0] {
        push_scalar(&mut expected, edge);
    }
    expected.push(1); // the picture data follows the header

    // The op stream: DRAW_PAINT (op 13, size 8), then the paint's index, 1.
    push_tag(&mut expected, *b"read", 8);
    push_word(&mut expected, (13 << 24) | 8);
    push_word(&mut expected, 1);

    // The factories in order of first use: the dash (`SkDashImpl`) and the blur
    // (`SkBlurMaskFilterImpl`), each as a packed length and its name.
    push_tag(&mut expected, *b"fact", 4 + 1 + 10 + 1 + 20);
    push_word(&mut expected, 2);
    expected.push(10);
    expected.extend_from_slice(b"SkDashImpl");
    expected.push(20);
    expected.extend_from_slice(b"SkBlurMaskFilterImpl");

    push_tag(&mut expected, *b"tpfc", 0); // no typefaces

    // The buffer of tables: one paint, then the empty slugs.
    let paint_bytes: usize = 4 + 4 + 16 + 4 // stroke width, miter, color, packed flags
        + (4 + 4 + 4 + 2 * 4 + 4) // path effect: index, size, phase, count, two intervals
        + 4 // shader
        + (4 + 4 + 12) // mask filter: index, size, sigma, style, ignore CTM
        + 3 * 4; // color filter, image filter, blender
    push_tag(
        &mut expected,
        *b"aray",
        u32::try_from(8 + paint_bytes + 8).expect("a small buffer"),
    );
    push_tag(&mut expected, *b"pnt ", 1);
    push_scalar(&mut expected, 0.0); // stroke width
    push_scalar(&mut expected, 4.0); // stroke miter
    for channel in [0.0f32, 0.0, 0.0, 1.0] {
        push_scalar(&mut expected, channel); // color, opaque black
    }
    // pack_v68: the effects flag in the top byte, and the SrcOver blend mode (3) in the next.
    push_word(&mut expected, (2 << 24) | (3 << 8));
    push_word(&mut expected, 1); // path effect index
    push_word(&mut expected, 16); // its size
    push_scalar(&mut expected, 0.0); // phase
    push_word(&mut expected, 2); // interval count
    push_scalar(&mut expected, 2.0);
    push_scalar(&mut expected, 2.0);
    push_word(&mut expected, 0); // shader
    push_word(&mut expected, 2); // mask filter index
    push_word(&mut expected, 12); // its size
    push_scalar(&mut expected, 3.0); // sigma
    push_word(&mut expected, 0); // blur style, normal
    push_word(&mut expected, 0); // do not ignore the CTM
    push_word(&mut expected, 0); // color filter
    push_word(&mut expected, 0); // image filter
    push_word(&mut expected, 0); // blender
    push_tag(&mut expected, *b"slug", 0);

    push_word(&mut expected, u32::from_be_bytes(*b"eof "));

    assert_eq!(data.as_bytes(), expected.as_slice());

    // The reader resolves the indices through the factory names, and writes the same bytes.
    let read_back = Picture::from_data_with_registry(data.as_bytes(), None, &REGISTRY).unwrap();
    let again = read_back.serialize(None).unwrap();
    assert_eq!(again.as_bytes(), expected.as_slice());
}
