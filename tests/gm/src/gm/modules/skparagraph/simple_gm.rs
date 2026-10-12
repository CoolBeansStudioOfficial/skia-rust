// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skparagraph/gm/simple_gm.cpp (chrome/m156)

use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::paint::{Cap, Paint};
use skia_rust_core::point::Point;
use skia_rust_core::scalar::scalar;
use skia_rust_paragraph::{
    FontCollection, Paragraph, ParagraphBuilderImpl, ParagraphStyle, TextDecoration,
    TextDecorationMode, TextStyle,
};
use skia_rust_tools::font_tool_utils::test_font_mgr;

use crate::prelude::*;

// Port of: modules/skparagraph/gm/simple_gm.cpp#L25 (chrome/m156)
const G_SPEECH: &str = "Five score years ago, a great American, in whose symbolic shadow we stand today, signed the Emancipation Proclamation. This momentous decree came as a great beacon light of hope to millions of Negro slaves who had been seared in the flames of withering injustice. It came as a joyous daybreak to end the long night of their captivity.";

// Port of: modules/skparagraph/gm/simple_gm.cpp#L40-L44 (chrome/m156), ParaFlags
const K_TIME_LAYOUT: u32 = 1 << 0;
const K_USE_UNDERLINE: u32 = 1 << 1;
const K_SHOW_VISITOR: u32 = 1 << 2;

// Port of: modules/skparagraph/gm/simple_gm.cpp#L46-L63 (chrome/m156)
fn get_unicode() -> Option<std::sync::Arc<dyn skia_rust_unicode::unicode::Unicode>> {
    // `SK_UNICODE_ICU_IMPLEMENTATION` is the Unicode of the oracle build; libgrapheme and ICU4X
    // are not part of it.
    skia_rust_unicode::unicode_icu::make()
}

// Port of: modules/skparagraph/gm/simple_gm.cpp#L66-L257 (chrome/m156), ParagraphGM
struct ParagraphGm {
    para: Option<Paragraph>,
    flags: u32,
}

impl ParagraphGm {
    fn new(flags: u32) -> Self {
        Self { para: None, flags }
    }

    // Port of: modules/skparagraph/gm/simple_gm.cpp#L73-L108 (chrome/m156)
    fn build_paragraph(&mut self) {
        let mut style = TextStyle::new();
        style.set_foreground_color(&Paint::default());
        style.set_font_families(&["sans-serif"]);
        style.set_font_size(30.0);

        if self.flags & K_USE_UNDERLINE != 0 {
            style.set_decoration_type(TextDecoration::UNDERLINE);
            style.set_decoration_mode(TextDecorationMode::Through);
            style.set_decoration_color(Color::BLACK);
            style.set_decoration_thickness_multiplier(2.0);
        }

        let mut para_style = ParagraphStyle::new();
        para_style.set_text_style(&style);

        let fontmgr = test_font_mgr();
        if fontmgr.count_families() == 0 {
            self.para = None;
            return;
        }
        let mut collection = FontCollection::new();
        collection.set_default_font_manager(fontmgr);

        let Some(unicode) = get_unicode() else {
            self.para = None;
            return;
        };
        let mut builder = ParagraphBuilderImpl::new(&para_style, collection, unicode);

        builder.add_text(G_SPEECH);

        let mut para = builder.build();
        para.layout(400.0);
        self.para = Some(para);
    }

    // Port of: modules/skparagraph/gm/simple_gm.cpp#L135-L192 (chrome/m156)
    fn draw_from_visitor(&self, canvas: &Canvas, para: &mut Paragraph) {
        const GAP: scalar = 2.0;
        let mut p = Paint::default();
        let mut p2 = Paint::default();
        p.set_color(Color::new(0xFF00_00FF));
        p2.set_color(Color::new(0xFFFF_0000));
        p2.set_stroke_width(4.0);
        p2.set_stroke_cap(Cap::Square);
        let mut underp = Paint::default();
        underp.set_stroke(true);
        underp.set_stroke_width(2.0);
        underp.set_anti_alias(true);
        underp.set_color(p.color());

        let flags = self.flags;
        para.visit(|_, info| {
            let Some(info) = info else {
                return;
            };
            canvas.draw_glyphs_at(
                &info.glyphs()[..info.count()],
                &info.positions()[..info.count()],
                info.origin(),
                info.font(),
                &p,
            );

            if flags & K_USE_UNDERLINE != 0 {
                // Need to modify positions to roll-in the orign
                let mut pos: Vec<Point> = Vec::new();
                for i in 0..info.count() {
                    pos.push(Point::new(
                        info.origin().x + info.positions()[i].x,
                        info.origin().y + info.positions()[i].y,
                    ));
                }

                let x0_start = pos[0].x;
                let x1_end = x0_start + info.advance_x();
                let y = pos[0].y;
                let sects = info.font().get_intercepts(
                    &info.glyphs()[..info.count()],
                    &pos,
                    (y + 1.0, y + 3.0),
                    None,
                );

                let mut x0 = x0_start;
                let mut i = 0;
                while i < sects.len() {
                    let x1 = sects[i] - GAP;
                    if x0 < x1 {
                        canvas.draw_line((x0, y + 2.0), (x1, y + 2.0), &underp);
                    }
                    x0 = sects[i + 1] + GAP;
                    i += 2;
                }
                canvas.draw_line((x0, y + 2.0), (x1_end, y + 2.0), &underp);
            }

            // `if ((false)) { ... }`: debug output and position points, never run.
            let _ = &p2;
        });
    }
}

impl GM for ParagraphGm {
    // Port of: modules/skparagraph/gm/simple_gm.cpp#L116-L125 (chrome/m156), getName
    fn name(&self) -> String {
        let mut name = format!(
            "paragraph{}_{}",
            if self.flags & K_TIME_LAYOUT != 0 {
                "_layout"
            } else {
                ""
            },
            if self.flags & K_USE_UNDERLINE != 0 {
                "_underline"
            } else {
                ""
            }
        );
        if self.flags & K_SHOW_VISITOR != 0 {
            name.push_str("_visitor");
        }
        name
    }

    // Port of: modules/skparagraph/gm/simple_gm.cpp#L127-L133 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        if self.flags & K_SHOW_VISITOR != 0 {
            return ISize::new(810, 420);
        }
        ISize::new(412, 420)
    }

    // Port of: modules/skparagraph/gm/simple_gm.cpp#L111-L114 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.build_paragraph();
    }

    // Port of: modules/skparagraph/gm/simple_gm.cpp#L194-L239 (chrome/m156), onDraw
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let flags = self.flags;
        if self.para.is_none() {
            "Font manager had no fonts or could not build paragraph.".clone_into(error_msg);
            return DrawResult::Skip;
        }

        if flags & K_SHOW_VISITOR != 0 {
            canvas.clear(Color::WHITE);
            // `fPara->layout(400); fPara->paint(canvas, 10, 10);`
            let mut para = self.para.take().expect("a paragraph");
            para.layout(400.0);
            para.paint(canvas, (10.0, 10.0));
            canvas.translate((400.0 + 10.0, 10.0));
            self.draw_from_visitor(canvas, &mut para);
            self.para = Some(para);
            return DrawResult::Ok;
        }

        // `(this->getMode() == kGM_Mode) ? 1 : 50`: the harness always runs as a GM.
        let loop_count = 1;

        let para = self.para.as_mut().expect("a paragraph");
        let mut parity = 0;
        for _ in 0..loop_count {
            let _acr = AutoCanvasRestore::guard(canvas, true);

            if flags & K_TIME_LAYOUT != 0 {
                // 400 or 401: exact in a `scalar`.
                #[allow(clippy::cast_precision_loss)]
                para.layout((400 + parity) as scalar);
                parity = (parity + 1) & 1;
            }
            para.paint(canvas, (10.0, 10.0));
        }
        // clean up if we've been looping
        if loop_count > 1 {
            canvas.clear(Color::WHITE);
            para.layout(400.0);
            para.paint(canvas, (10.0, 10.0));
        }

        // `(this->getMode() == kGM_Mode) && (fFlags & kTimeLayout)`
        if flags & K_TIME_LAYOUT != 0 {
            return DrawResult::Skip;
        }
        DrawResult::Ok
    }
}

// Port of: modules/skparagraph/gm/simple_gm.cpp#L253-L257 (chrome/m156)
crate::def_gm!(ParagraphGM_0 = "ParagraphGM(0)", ParagraphGm::new(0));
crate::def_gm!(
    ParagraphGM_kTimeLayout = "ParagraphGM(kTimeLayout)",
    ParagraphGm::new(K_TIME_LAYOUT)
);
crate::def_gm!(
    ParagraphGM_kUseUnderline = "ParagraphGM(kUseUnderline)",
    ParagraphGm::new(K_USE_UNDERLINE)
);
crate::def_gm!(
    ParagraphGM_kShowVisitor = "ParagraphGM(kShowVisitor)",
    ParagraphGm::new(K_SHOW_VISITOR)
);
crate::def_gm!(
    ParagraphGM_kShowVisitor_kUseUnderline = "ParagraphGM(kShowVisitor | kUseUnderline)",
    ParagraphGm::new(K_SHOW_VISITOR | K_USE_UNDERLINE)
);
