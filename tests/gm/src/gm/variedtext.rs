// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/variedtext.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::color_to_565;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_core::typeface::Typeface;
use skia_rust_tools::font_tool_utils::create_portable_typeface;

// Port of: gm/variedtext.cpp#L147 (chrome/m156), VariedTextGM::kCnt
const CNT: usize = 30;
// Port of: gm/variedtext.cpp#L148-L149 (chrome/m156), kMinLength and kMaxLength
const MIN_LENGTH: u32 = 15;
const MAX_LENGTH: u32 = 40;

// Port of: gm/variedtext.cpp#L34-L165 (chrome/m156), VariedTextGM
struct VariedTextGm {
    effective_clip: bool,
    lcd: bool,
    typefaces: Vec<Typeface>,
    paint: Paint,
    font: Font,
    // precomputed for each text draw
    strings: Vec<Vec<u8>>,
    colors: Vec<Color>,
    pt_sizes: Vec<scalar>,
    typeface_indices: Vec<usize>,
    offsets: Vec<Point>,
    clip_rects: Vec<Rect>,
}

impl VariedTextGm {
    // Port of: gm/variedtext.cpp#L35-L38 (chrome/m156), the constructor
    fn new(effective_clip: bool, lcd: bool) -> Self {
        Self {
            effective_clip,
            lcd,
            typefaces: Vec::new(),
            paint: Paint::default(),
            font: Font::from_typeface(None),
            strings: Vec::new(),
            colors: Vec::new(),
            pt_sizes: Vec::new(),
            typeface_indices: Vec::new(),
            offsets: Vec::new(),
            clip_rects: Vec::new(),
        }
    }
}

impl GM for VariedTextGm {
    fn name(&self) -> String {
        let mut name = "varied_text".to_owned();
        name.push_str(if self.effective_clip {
            "_clipped"
        } else {
            "_ignorable_clip"
        });
        name.push_str(if self.lcd { "_lcd" } else { "_no_lcd" });
        name
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    // Port of: gm/variedtext.cpp#L59-L116 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.paint.set_anti_alias(true);
        self.font.set_edging(if self.lcd {
            Edging::SubpixelAntiAlias
        } else {
            Edging::AntiAlias
        });
        let size = self.size();
        // The GM size is 640x480, exact in a scalar.
        #[allow(clippy::cast_precision_loss)]
        let gm_width = size.width as scalar;
        #[allow(clippy::cast_precision_loss)]
        let gm_height = size.height as scalar;

        self.typefaces = vec![
            create_portable_typeface(Some("sans-serif"), FontStyle::default()),
            create_portable_typeface(Some("sans-serif"), FontStyle::bold()),
            create_portable_typeface(Some("serif"), FontStyle::default()),
            create_portable_typeface(Some("serif"), FontStyle::bold()),
        ];

        let mut random = Random::default();
        self.strings = Vec::with_capacity(CNT);
        self.colors = Vec::with_capacity(CNT);
        self.pt_sizes = Vec::with_capacity(CNT);
        self.typeface_indices = Vec::with_capacity(CNT);
        self.offsets = vec![Point::default(); CNT];
        self.clip_rects = vec![Rect::default(); CNT];
        for i in 0..CNT {
            let length = random.next_range_u(MIN_LENGTH, MAX_LENGTH) as usize;
            let mut text = vec![0u8; length];
            for byte in &mut text {
                // The range is '!'..='z', which fits in a byte.
                #[allow(clippy::cast_possible_truncation)]
                let c = random.next_range_u(u32::from(b'!'), u32::from(b'z')) as u8;
                *byte = c;
            }
            self.strings.push(text);
            let mut color = random.next_u();
            color |= 0xFF00_0000;
            self.colors.push(color_to_565(Color::from(color)));

            // constexpr SkScalar kMinPtSize = 8.f; kMaxPtSize = 32.f;
            let pt_size = random.next_range_scalar(8.0, 32.0);
            self.pt_sizes.push(pt_size);
            // The typeface count is four, so it fits in a u32.
            #[allow(clippy::cast_possible_truncation)]
            let typeface_count = self.typefaces.len() as u32;
            let typeface_index = random.next_u_less_than(typeface_count) as usize;
            self.typeface_indices.push(typeface_index);

            self.paint.set_color(self.colors[i]);
            self.font
                .set_typeface(Some(self.typefaces[typeface_index].clone()));
            self.font.set_size(pt_size);
            let (_, bounds) = self
                .font
                .measure_text(&self.strings[i], TextEncoding::UTF8, None);

            // The set of x,y offsets which place the bounding box inside the GM's border.
            let mut safe_rect = Rect::from_ltrb(
                -bounds.left,
                -bounds.top,
                gm_width - bounds.right,
                gm_height - bounds.bottom,
            );
            if safe_rect.is_empty() {
                // If the bounds don't fit then allow any offset in the GM's border.
                safe_rect = Rect::from_xywh(0.0, 0.0, gm_width, gm_height);
            }
            let offset_x = random.next_range_scalar(safe_rect.left, safe_rect.right);
            let offset_y = random.next_range_scalar(safe_rect.top, safe_rect.bottom);
            self.offsets[i] = Point::new(offset_x, offset_y);

            let mut clip = bounds;
            clip.offset(self.offsets[i]);
            clip.outset((2.0, 2.0));
            if self.effective_clip {
                clip.right -= 0.25 * clip.width();
            }
            self.clip_rects[i] = clip;
        }
    }

    // Port of: gm/variedtext.cpp#L118-L145 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        for i in 0..CNT {
            self.paint.set_color(self.colors[i]);
            self.font.set_size(self.pt_sizes[i]);
            self.font
                .set_typeface(Some(self.typefaces[self.typeface_indices[i]].clone()));
            canvas.save();
            canvas.clip_rect(self.clip_rects[i], ClipOp::Intersect, false);
            canvas.translate(self.offsets[i]);
            canvas.draw_simple_text(
                &self.strings[i],
                TextEncoding::UTF8,
                (0.0, 0.0),
                &self.font,
                &self.paint,
            );
            canvas.restore();
        }

        // Visualize the clips, but not in bench mode.
        let mut wire_paint = Paint::default();
        wire_paint.set_anti_alias(true);
        wire_paint.set_stroke_width(0.0);
        wire_paint.set_style(Style::Stroke);
        for rect in &self.clip_rects {
            canvas.draw_rect(rect, &wire_paint);
        }
    }
}

// Port of: gm/variedtext.cpp#L168 (chrome/m156)
crate::def_gm!(
    VariedTextGM_ff = "VariedTextGM(false, false)",
    VariedTextGm::new(false, false)
);
// Port of: gm/variedtext.cpp#L169 (chrome/m156)
crate::def_gm!(
    VariedTextGM_tf = "VariedTextGM(true, false)",
    VariedTextGm::new(true, false)
);
// Port of: gm/variedtext.cpp#L170 (chrome/m156)
crate::def_gm!(
    VariedTextGM_ft = "VariedTextGM(false, true)",
    VariedTextGm::new(false, true)
);
// Port of: gm/variedtext.cpp#L171 (chrome/m156)
crate::def_gm!(
    VariedTextGM_tt = "VariedTextGM(true, true)",
    VariedTextGm::new(true, true)
);
