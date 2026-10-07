// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Unit tests of `Canvas` over the raster device and `Surface`: the state machine (save counts,
//! deferred saves, matrices, clips, layers) and the draws it dispatches. The ported Skia tests are
//! in `skia-rust-tests`; these cover what they do not.

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{Canvas, ContentChangeMode, SaveLayerRec};
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};

use crate::raster_canvas::RasterCanvas;
use crate::surfaces;

fn paint(c: Color) -> Paint {
    let mut p = Paint::default();
    p.set_color(c);
    p
}

fn surface(w: i32, h: i32) -> crate::surface::Surface<'static> {
    surfaces::raster(&ImageInfo::new_n32_premul((w, h), None), None, None).expect("surface")
}

fn px(s: &mut crate::surface::Surface<'_>, x: i32, y: i32) -> u32 {
    let peek = s.peek_pixels().expect("pixels");
    peek.pixmap().addr32(x, y)
}

#[test]
fn save_restore_counts_and_deferred_saves() {
    let mut s = surface(10, 10);
    let c = s.canvas();
    assert_eq!(c.save_count(), 1);
    assert_eq!(c.save(), 1);
    assert_eq!(c.save(), 2);
    assert_eq!(c.save_count(), 3);
    c.translate((3.0, 4.0));
    assert_eq!(c.save_count(), 3);
    c.restore();
    assert_eq!(c.save_count(), 2);
    assert!(c.total_matrix().is_identity());
    c.restore_to_count(1);
    assert_eq!(c.save_count(), 1);
    c.restore(); // underflow is ignored
    assert_eq!(c.save_count(), 1);
}

#[test]
fn draws_through_matrix_and_clip() {
    let mut s = surface(10, 10);
    {
        let c = s.canvas();
        c.clear(Color4f::new(0.0, 0.0, 0.0, 0.0));
        c.save();
        c.translate((2.0, 2.0));
        c.clip_rect(Rect::new(0.0, 0.0, 4.0, 4.0), ClipOp::Intersect, false);
        assert_eq!(c.device_clip_bounds(), Some(IRect::new(2, 2, 6, 6)));
        assert_eq!(c.local_clip_bounds().map(|r| r.left), Some(-1.0 + 0.0));
        c.draw_rect(Rect::new(-10.0, -10.0, 10.0, 10.0), &paint(Color::RED));
        assert!(c.quick_reject_rect(Rect::new(20.0, 20.0, 30.0, 30.0)));
        c.restore();
    }
    assert_eq!(px(&mut s, 2, 2), 0xFFFF_0000);
    assert_eq!(px(&mut s, 5, 5), 0xFFFF_0000);
    assert_eq!(px(&mut s, 6, 6), 0);
    assert_eq!(px(&mut s, 1, 1), 0);
}

#[test]
fn save_layer_is_drawn_on_restore() {
    let mut s = surface(8, 8);
    {
        let c = s.canvas();
        c.clear(Color4f::new(1.0, 1.0, 1.0, 1.0));
        let bounds = Rect::new(2.0, 2.0, 6.0, 6.0);
        let mut layer_paint = Paint::default();
        layer_paint.set_alpha_f(0.5);
        let n = c.save_layer(&SaveLayerRec::default().bounds(&bounds).paint(&layer_paint));
        assert_eq!(n, 1);
        assert_eq!(c.save_count(), 2);
        // The layer covers the bounds only: the rect outside is clipped away.
        assert_eq!(c.device_clip_bounds(), Some(IRect::new(2, 2, 6, 6)));
        c.draw_rect(Rect::new(0.0, 0.0, 8.0, 8.0), &paint(Color::BLUE));
        c.restore();
    }
    // 50% blue over white.
    let mid = px(&mut s, 3, 3);
    let (r, g, b) = ((mid >> 16) & 0xFF, (mid >> 8) & 0xFF, mid & 0xFF);
    assert!((126..=129).contains(&r) && (126..=129).contains(&g) && b == 255);
    assert_eq!(px(&mut s, 1, 1), 0xFFFF_FFFF);
    assert_eq!(px(&mut s, 6, 6), 0xFFFF_FFFF);
}

#[test]
fn layers_translate_with_the_canvas() {
    let mut s = surface(8, 8);
    {
        let c = s.canvas();
        c.clear(Color4f::new(0.0, 0.0, 0.0, 0.0));
        c.translate((3.0, 3.0));
        let _ = c.save_layer(&SaveLayerRec::default());
        c.draw_rect(Rect::new(0.0, 0.0, 2.0, 2.0), &paint(Color::GREEN));
        c.restore();
    }
    assert_eq!(px(&mut s, 3, 3), 0xFF00_FF00);
    assert_eq!(px(&mut s, 4, 4), 0xFF00_FF00);
    assert_eq!(px(&mut s, 2, 2), 0);
    assert_eq!(px(&mut s, 5, 5), 0);
}

#[test]
fn surface_generation_id_changes_on_draw() {
    let mut s = surface(4, 4);
    let a = s.generation_id();
    assert_eq!(a, s.generation_id());
    s.canvas()
        .draw_rect(Rect::new(0.0, 0.0, 1.0, 1.0), &paint(Color::RED));
    let b = s.generation_id();
    assert_ne!(a, b);
    s.notify_content_will_change(ContentChangeMode::Retain);
    assert_ne!(b, s.generation_id());
}

#[test]
fn wrapping_a_bitmap_gives_the_pixels_back() {
    let mut bm = Bitmap::new();
    bm.alloc_pixels_flags(&ImageInfo::new_n32_premul((4, 4), None));
    {
        let c = Canvas::from_bitmap(&mut bm, None).expect("canvas");
        c.draw_rect(Rect::new(0.0, 0.0, 4.0, 4.0), &paint(Color::RED));
        let mut p = Paint::default();
        p.set_color(Color::BLUE);
        p.set_blend_mode(BlendMode::Src);
        c.draw_rect(Rect::new(0.0, 0.0, 1.0, 1.0), &p);
    }
    assert_eq!(bm.get_color((0, 0)), Color::BLUE);
    assert_eq!(bm.get_color((3, 3)), Color::RED);

    let mut bytes = vec![0_u8; 4 * 4 * 4];
    {
        let c = Canvas::from_raster_direct(
            &ImageInfo::new_n32_premul((4, 4), None),
            &mut bytes,
            None,
            None,
        )
        .expect("canvas");
        c.clear(Color4f::new(1.0, 0.0, 0.0, 1.0));
    }
    assert!(bytes.chunks(4).all(|p| p[2] == 255 || p[0] == 255));
}

#[test]
fn read_and_write_pixels() {
    let mut s = surface(4, 4);
    s.canvas().clear(Color4f::new(0.0, 1.0, 0.0, 1.0));
    let info = ImageInfo::new_n32_premul((2, 2), None);
    let mut out = vec![0_u8; 16];
    assert!(s.read_pixels(&info, &mut out, 8, (1, 1)));
    assert_eq!(
        u32::from_ne_bytes([out[0], out[1], out[2], out[3]]),
        0xFF00_FF00
    );
    let red = vec![0xFF_u8; 16];
    assert!(s.canvas().write_pixels(&info, &red, 8, (0, 0)));
    assert_eq!(px(&mut s, 0, 0), 0xFFFF_FFFF);
    assert_eq!(px(&mut s, 2, 2), 0xFF00_FF00);
}
