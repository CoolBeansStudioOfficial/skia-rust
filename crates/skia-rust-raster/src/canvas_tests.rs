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

// `tests/ReadPixelsTest.cpp::ReadPixels` with the source drawn by `Canvas::write_pixels` of the
// same bitmap instead of `drawImage(asImage(), SkBlendMode::kSrc)`: there is no `SkImage` yet, so
// the ported test stays todo. Same rects, configs and checks, no tolerance beyond Skia's own
// +-1 rule for unpremul round trips.
mod read_pixels_matches_the_surface {
    // A single long test over the same loops as the C++; short channel names as in Skia.
    #![allow(clippy::many_single_char_names, clippy::too_many_lines)]

    use super::*;
    use skia_rust_core::alpha_type::AlphaType;
    use skia_rust_core::color_priv::{
        get_packed_a32, get_packed_b32, get_packed_g32, get_packed_r32, pack_argb32,
        premultiply_argb_inline,
    };
    use skia_rust_core::color_type::ColorType;
    use skia_rust_core::image_info_priv::{color_type_is_alpha_only, image_info_valid_conversion};
    use skia_rust_core::math_priv::mul_div_255_ceiling;
    use skia_rust_core::point::IPoint;
    use skia_rust_core::rect::Contains;

    const W: i32 = 100;
    const H: i32 = 100;

    fn src_color(x: i32, y: i32) -> u32 {
        let a = match (x + y) % 5 {
            1 => 0x80,
            2 => 0xCC,
            4 => 0x01,
            3 => 0x00,
            _ => 0xff,
        };
        premultiply_argb_inline(a, x.cast_unsigned(), y.cast_unsigned(), 0xc)
    }

    fn dst_init_color(x: i32, y: i32, w: i32) -> u32 {
        let n = (y * w + x).cast_unsigned();
        pack_argb32(0xff, (n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff)
    }

    fn to_pm(ct: ColorType, at: AlphaType, px: u32) -> (u32, bool) {
        let c = px.to_ne_bytes().map(u32::from);
        let (mut r, mut g, mut b) = match ct {
            ColorType::BGRA8888 => (c[2], c[1], c[0]),
            _ => (c[0], c[1], c[2]),
        };
        let a = c[3];
        let unpremul = at == AlphaType::Unpremul;
        if unpremul {
            r = mul_div_255_ceiling(r, a);
            g = mul_div_255_ceiling(g, a);
            b = mul_div_255_ceiling(b, a);
        }
        (pack_argb32(a, r, g, b), unpremul)
    }

    fn close(a: u32, b: u32, premul_conversion: bool) -> bool {
        if !premul_conversion {
            return a == b;
        }
        let d = |f: fn(u32) -> u32| (f(a).cast_signed() - f(b).cast_signed()).abs() <= 1;
        get_packed_a32(a) == get_packed_a32(b)
            && d(get_packed_r32)
            && d(get_packed_g32)
            && d(get_packed_b32)
    }

    fn alpha_byte(c: u32) -> u8 {
        get_packed_a32(c).to_le_bytes()[0]
    }

    #[test]
    fn all_rects_and_configs() {
        let mut s = surface(W, H);
        {
            let mut src = Bitmap::new();
            src.alloc_n32_pixels((W, H), None);
            for y in 0..H {
                for x in 0..W {
                    src.set_addr32(x, y, src_color(x, y));
                }
            }
            assert!(s.canvas().write_pixels_from_bitmap(&src, (0, 0)));
        }
        let dev = IRect::new(0, 0, W, H);
        let surface_info = ImageInfo::new_n32_premul((W, H), None);
        let rects = [
            IRect::new(0, 0, W, H),
            IRect::new(-10, -10, W + 10, H + 10),
            IRect::new(W / 4, H / 4, 3 * W / 4, 3 * H / 4),
            IRect::new(-10, -10, -1, -1),
            IRect::new(-10, -10, 0, 0),
            IRect::new(-10, -10, W / 4, H / 4),
            IRect::new(-10, -10, W + 10, H / 4),
            IRect::new(-10, -10, W + 10, 0),
            IRect::new(3 * W / 4, -10, W + 10, H / 4),
            IRect::new(W / 4, -10, 3 * W / 4, H / 4),
            IRect::new(W + 1, -10, W + 10, -1),
            IRect::new(W, -10, W + 10, 0),
            IRect::new(-10, -10, W / 4, H + 10),
            IRect::new(-10, -10, 0, H + 10),
            IRect::new(-10, 3 * H / 4, W / 4, H + 10),
            IRect::new(-10, H / 4, W / 4, 3 * H / 4),
            IRect::new(-10, H + 1, -1, H + 10),
            IRect::new(-10, H, 0, H + 10),
            IRect::new(-10, 3 * H / 4, W + 10, H + 10),
            IRect::new(0, H, W, H + 10),
            IRect::new(3 * W / 4, 3 * H / 4, W + 10, H + 10),
            IRect::new(3 * W / 4, -10, W + 10, H + 10),
        ];
        let configs = [
            (ColorType::RGBA8888, AlphaType::Premul),
            (ColorType::RGBA8888, AlphaType::Unpremul),
            (ColorType::RGB888x, AlphaType::Opaque),
            (ColorType::BGRA8888, AlphaType::Premul),
            (ColorType::BGRA8888, AlphaType::Unpremul),
            (ColorType::Alpha8, AlphaType::Premul),
        ];
        for rect in &rects {
            for tight in [true, false] {
                for &(ct, at) in &configs {
                    let info = ImageInfo::new((rect.width(), rect.height()), ct, at, None);
                    let rb = if tight {
                        0
                    } else {
                        (info.width().cast_unsigned() as usize + 16) * info.bytes_per_pixel() / 4
                            * 4
                    };
                    let mut bmp = Bitmap::new();
                    bmp.alloc_pixels_info(&info, rb);
                    let (bw, bh) = (bmp.width(), bmp.height());
                    for y in 0..bh {
                        for x in 0..bw {
                            let init = dst_init_color(x, y, bw);
                            if ct == ColorType::Alpha8 {
                                bmp.set_addr8(x, y, alpha_byte(init));
                            } else {
                                bmp.set_addr32(x, y, init);
                            }
                        }
                    }
                    let before = s.generation_id();
                    let ok = s.read_pixels_to_bitmap(&mut bmp, (rect.left, rect.top));
                    assert_eq!(before, s.generation_id());
                    let expect = IRect::intersects(rect, &dev)
                        && image_info_valid_conversion(bmp.info(), &surface_info);
                    assert_eq!(expect, ok, "{rect:?} {ct:?} {at:?}");

                    let clipped = IRect::intersect(&dev, rect);
                    for by in 0..bh {
                        for bx in 0..bw {
                            let (dx, dy) = (bx + rect.left, by + rect.top);
                            let inside = clipped.is_some_and(|c| c.contains(IPoint::new(dx, dy)));
                            if ct == ColorType::Alpha8 {
                                let a = bmp.get_addr8(bx, by);
                                if inside && ok {
                                    assert_eq!(a, alpha_byte(src_color(dx, dy)));
                                } else if !inside {
                                    assert_eq!(a, alpha_byte(dst_init_color(bx, by, bw)));
                                }
                                continue;
                            }
                            let px = bmp.get_addr32(bx, by);
                            if inside {
                                if ok {
                                    let mut want = src_color(dx, dy);
                                    if color_type_is_alpha_only(surface_info.color_type()) {
                                        want &= 0xFF00_0000;
                                    }
                                    if at == AlphaType::Opaque {
                                        want |= 0xFF00_0000;
                                    }
                                    let (got, premul) = to_pm(ct, at, px);
                                    assert!(
                                        close(got, want, premul),
                                        "{dx},{dy} {got:08x} {want:08x}"
                                    );
                                }
                            } else {
                                assert_eq!(
                                    px,
                                    dst_init_color(bx, by, bw),
                                    "clipped-out area changed"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
