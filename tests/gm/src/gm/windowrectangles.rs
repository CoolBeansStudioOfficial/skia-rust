// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/windowrectangles.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::draw_checkerboard;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::clip_stack::{ClipStack, DeviceSpaceType, Iter, IterStart};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Vector;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::rrect::RRect;

const K_DEVICE_RECT: IRect = IRect::from_ltrb(0, 0, 600, 600);

// Port of: gm/windowrectangles.cpp#L31-L38 (chrome/m156), WindowRectanglesGM
struct WindowRectanglesGm;

impl WindowRectanglesGm {
    // Port of: gm/windowrectangles.cpp#L40-L67 (chrome/m156), coverClipStack
    fn cover_clip_stack(stack: &ClipStack, canvas: &Canvas) -> DrawResult {
        let mut paint = Paint::default();
        paint.set_color(Color::from(0xff00_aa80));

        // Set up the canvas's clip to match our SkClipStack.
        let mut iter = Iter::new(stack, IterStart::Bottom);
        while let Some(element) = iter.next() {
            debug_assert!(!element.is_replace_op());
            let op = element.op();
            let is_aa = element.is_aa();
            match element.device_space_type() {
                DeviceSpaceType::Shader => {
                    if let Some(shader) = element.shader() {
                        canvas.clip_shader(shader.clone(), op);
                    }
                }
                DeviceSpaceType::Path => {
                    canvas.clip_path(element.device_space_path(), op, is_aa);
                }
                DeviceSpaceType::RRect => {
                    canvas.clip_rrect(element.device_space_rrect(), op, is_aa);
                }
                DeviceSpaceType::Rect => {
                    canvas.clip_rect(element.device_space_rect(), op, is_aa);
                }
                DeviceSpaceType::Empty => {
                    canvas.clip_rect(
                        Rect::from_ltrb(0.0, 0.0, 0.0, 0.0),
                        ClipOp::Intersect,
                        false,
                    );
                }
            }
        }
        canvas.draw_rect(Rect::from_ltrb(50.0, 50.0, 550.0, 550.0), &paint);
        DrawResult::Ok
    }
}

impl GM for WindowRectanglesGm {
    fn name(&self) -> String {
        "windowrectangles".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(K_DEVICE_RECT.width(), K_DEVICE_RECT.height())
    }

    // Port of: gm/windowrectangles.cpp#L70-L86 (chrome/m156), onDraw
    fn on_draw_with_error(&mut self, canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        draw_checkerboard(
            canvas,
            Color::from(0xffff_ffff),
            Color::from(0xffc6_c3c6),
            25,
        );

        let mut stack = ClipStack::new();
        let identity = Matrix::new_identity();
        stack.clip_rect(
            &Rect::from_xywh(370.75, 80.25, 149.0, 100.0),
            &identity,
            ClipOp::Difference,
            false,
        );
        stack.clip_rect(
            &Rect::from_xywh(80.25, 420.75, 150.0, 100.0),
            &identity,
            ClipOp::Difference,
            true,
        );
        stack.clip_rrect(
            &RRect::new_rect_xy(Rect::from_xywh(200.0, 200.0, 200.0, 200.0), 60.0, 45.0),
            &identity,
            ClipOp::Difference,
            true,
        );

        let mut nine = RRect::default();
        nine.set_nine_patch(
            Rect::from_xywh(550.0 - 30.25 - 100.0, 370.75, 100.0, 150.0),
            12.0,
            35.0,
            23.0,
            20.0,
        );
        stack.clip_rrect(&nine, &identity, ClipOp::Difference, true);

        let mut complx = RRect::default();
        let complx_radii = [
            Vector::new(6.0, 4.0),
            Vector::new(8.0, 12.0),
            Vector::new(16.0, 24.0),
            Vector::new(48.0, 32.0),
        ];
        complx.set_rect_radii(Rect::from_xywh(80.25, 80.75, 100.0, 149.0), &complx_radii);
        stack.clip_rrect(&complx, &identity, ClipOp::Difference, false);

        Self::cover_clip_stack(&stack, canvas)
    }
}

// Port of: gm/windowrectangles.cpp#L104 (chrome/m156)
crate::def_gm!(
    WindowRectanglesGM_ = "WindowRectanglesGM()",
    WindowRectanglesGm
);
