// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFDevice.cpp#L770-L1115 (chrome/m156)

//! The text of `SkPDFDevice`: `onDrawGlyphRunList`, `internalDrawGlyphRun`, `drawGlyphRunAsPath`
//! and `GlyphPositioner`.
//!
//! A glyph run is written as a `BT ... ET` text object that selects a [`PdfFont`] of the glyph's
//! strike, positions the glyphs and shows them as a hex string. The fonts themselves are written
//! when the document closes (`docs/design/modules.md` M26).

use skia_rust_core::advanced_typeface_metrics::FontType;
use skia_rust_core::clip_stack::ClipStack;
use skia_rust_core::color::Color;
use skia_rust_core::font_types::GlyphId;
use skia_rust_core::glyph::Glyph;
use skia_rust_core::glyph_run::{GlyphRun, GlyphRunList};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::stream::{DynamicMemoryWStream, WStream};
use skia_rust_core::strike_spec::BulkGlyphMetricsAndPaths;
use skia_rust_core::utf::next_utf8;

use super::{Content, DeviceCtx, StreamSelector, add_resource, clean_paint};
use crate::clusterator::Clusterator;
use crate::font::{
    PdfFont, PdfStrike, font_type, get_metrics, get_unicode_map, get_unicode_map_ex,
};
use crate::resource_dict::{ResourceType, write_resource_name};
use crate::types::write_text_string_to;
use crate::utils::{append_scalar, write_uint8, write_uint16_be};

/// `GlyphPositioner`: writes the positions and codes of the glyphs of a text object.
// Port of: src/pdf/SkPDFDevice.cpp#L775-L851 (chrome/m156)
struct GlyphPositioner {
    pdf_font: Option<PdfFont>,
    current_matrix_origin: Point,
    x_advance: f32,
    viewers_agree_on_advances_in_font: bool,
    viewers_agree_on_x_advance: bool,
    text_skew_x: f32,
    in_text: bool,
    initialized: bool,
}

impl GlyphPositioner {
    // Port of: src/pdf/SkPDFDevice.cpp#L778-L784 (chrome/m156)
    fn new(text_skew_x: f32, origin: Point) -> Self {
        Self {
            pdf_font: None,
            current_matrix_origin: origin,
            x_advance: 0.0,
            viewers_agree_on_advances_in_font: true,
            viewers_agree_on_x_advance: true,
            text_skew_x,
            in_text: false,
            initialized: false,
        }
    }

    // Port of: src/pdf/SkPDFDevice.cpp#L786-L791 (chrome/m156)
    fn flush(&mut self, content: &mut DynamicMemoryWStream) {
        if self.in_text {
            content.write_text("> Tj\n");
            self.in_text = false;
        }
    }

    // Port of: src/pdf/SkPDFDevice.cpp#L792-L799 (chrome/m156)
    fn set_font(&mut self, content: &mut DynamicMemoryWStream, pdf_font: PdfFont) {
        self.flush(content);
        // Reader 2020.013.20064 incorrectly advances some Type3 fonts https://crbug.com/1226960
        let converted_to_type3 = pdf_font.font_type() == FontType::Other;
        #[allow(clippy::float_cmp)] // exact comparison, as in Skia
        let thousand_em = pdf_font.strike().path().units_per_em == 1000.0;
        self.viewers_agree_on_advances_in_font = thousand_em || !converted_to_type3;
        self.pdf_font = Some(pdf_font);
    }

    // Port of: src/pdf/SkPDFDevice.cpp#L800-L843 (chrome/m156)
    fn write_glyph(
        &mut self,
        content: &mut DynamicMemoryWStream,
        glyph: GlyphId,
        advance_width: f32,
        xy: Point,
    ) {
        let multi_byte_glyphs = self
            .pdf_font
            .as_ref()
            .expect("a font was set")
            .multi_byte_glyphs();
        if !self.initialized {
            // Flip the text about the x-axis to account for origin swap and include
            // the passed parameters.
            content.write_text("1 0 ");
            append_scalar(-self.text_skew_x, content);
            content.write_text(" -1 ");
            append_scalar(self.current_matrix_origin.x, content);
            content.write_text(" ");
            append_scalar(self.current_matrix_origin.y, content);
            content.write_text(" Tm\n");
            self.current_matrix_origin = Point::new(0.0, 0.0);
            self.initialized = true;
        }
        let position = Point::new(
            xy.x - self.current_matrix_origin.x,
            xy.y - self.current_matrix_origin.y,
        );
        #[allow(clippy::float_cmp)] // exact comparison, as in Skia
        if !self.viewers_agree_on_x_advance || position != Point::new(self.x_advance, 0.0) {
            self.flush(content);
            append_scalar(position.x - position.y * self.text_skew_x, content);
            content.write_text(" ");
            append_scalar(-position.y, content);
            content.write_text(" Td ");
            self.current_matrix_origin = xy;
            self.x_advance = 0.0;
            self.viewers_agree_on_x_advance = true;
        }
        self.x_advance += advance_width;
        if !self.viewers_agree_on_advances_in_font {
            self.viewers_agree_on_x_advance = false;
        }
        if !self.in_text {
            content.write_text("<");
            self.in_text = true;
        }
        if multi_byte_glyphs {
            write_uint16_be(content, glyph);
        } else {
            debug_assert_eq!(0, glyph >> 8);
            write_uint8(content, (glyph & 0xFF) as u8);
        }
    }
}

// Port of: src/pdf/SkPDFDevice.cpp#L862-L869 (get_glyph_bounds_device_space, chrome/m156)
fn get_glyph_bounds_device_space(
    glyph: &Glyph,
    x_scale: f32,
    y_scale: f32,
    xy: Point,
    ctm: &Matrix,
) -> Rect {
    let mut glyph_bounds = Matrix::scale((x_scale, y_scale)).map_rect(glyph.rect()).0;
    glyph_bounds.offset(xy);
    ctm.map_rect(glyph_bounds).0 // now in dev space.
}

// Port of: src/pdf/SkPDFDevice.cpp#L871-L874 (contains, chrome/m156)
fn contains(r: &Rect, p: Point) -> bool {
    r.left <= p.x && p.x <= r.right && r.top <= p.y && p.y <= r.bottom
}

// Port of: src/pdf/SkPDFDevice.cpp#L915-L930 (needs_new_font, chrome/m156)
fn needs_new_font(font: Option<&PdfFont>, glyph: &Glyph, initial_font_type: FontType) -> bool {
    let Some(font) = font else {
        return true;
    };
    if !font.has_glyph(glyph.glyph_id()) {
        return true;
    }
    if initial_font_type == FontType::Other {
        return false;
    }
    if glyph.is_empty() {
        return false;
    }

    let has_unmodified_path = glyph.path().is_some() && !glyph.path_is_modified();
    let converted_to_type3 = font.font_type() == FontType::Other;
    converted_to_type3 == has_unmodified_path
}

impl Content {
    /// `onDrawGlyphRunList`.
    // Port of: src/pdf/SkPDFDevice.cpp#L1108-L1115 (chrome/m156)
    pub(super) fn draw_glyph_run_list(
        &mut self,
        ctx: &DeviceCtx,
        clip_stack: &ClipStack,
        glyph_run_list: &GlyphRunList<'_>,
        paint: &Paint,
    ) {
        debug_assert!(!glyph_run_list.has_rsxform());
        for glyph_run in glyph_run_list.runs() {
            self.internal_draw_glyph_run(
                ctx,
                clip_stack,
                glyph_run,
                glyph_run_list.origin(),
                paint,
            );
        }
    }

    /// `drawGlyphRunAsPath`.
    // Port of: src/pdf/SkPDFDevice.cpp#L876-L913 (chrome/m156)
    fn draw_glyph_run_as_path(
        &mut self,
        ctx: &DeviceCtx,
        clip_stack: &ClipStack,
        glyph_run: &GlyphRun,
        offset: Point,
        run_paint: &Paint,
    ) {
        let font = glyph_run.font();

        let mut builder = PathBuilder::new();
        let positions = glyph_run.positions();
        let mut pos_index = 0;
        font.get_paths(glyph_run.glyph_ids(), |path, mx| {
            if let Some(path) = path {
                let mut total = mx.clone();
                total.post_translate((
                    positions[pos_index].x + offset.x,
                    positions[pos_index].y + offset.y,
                ));
                builder.add_path_with_transform(path, &total, None);
            }
            pos_index += 1; // move to the next glyph's position
        });
        self.internal_draw_path(
            ctx,
            clip_stack,
            &ctx.local_to_device,
            &builder.detach(),
            run_paint,
        );

        let mut transparent_font = glyph_run.font().clone();
        transparent_font.set_embolden(false); // Stop Recursion
        let tmp_glyph_run = glyph_run.with_font(transparent_font);

        let mut transparent = Paint::default();
        transparent.set_color(Color::TRANSPARENT);

        if ctx.local_to_device.has_perspective() {
            // SkAutoDeviceTransformRestore adr(this, SkM44());
            let identity_ctx = DeviceCtx {
                bounds: ctx.bounds,
                device_to_global: ctx.device_to_global.clone(),
                local_to_device: Matrix::new_identity(),
            };
            self.internal_draw_glyph_run(
                &identity_ctx,
                clip_stack,
                &tmp_glyph_run,
                offset,
                &transparent,
            );
        } else {
            self.internal_draw_glyph_run(ctx, clip_stack, &tmp_glyph_run, offset, &transparent);
        }
    }

    /// `internalDrawGlyphRun`.
    // Port of: src/pdf/SkPDFDevice.cpp#L932-L1106 (chrome/m156)
    #[allow(clippy::too_many_lines)] // one function in the C++, kept as is
    fn internal_draw_glyph_run(
        &mut self,
        ctx: &DeviceCtx,
        clip_stack: &ClipStack,
        glyph_run: &GlyphRun,
        offset: Point,
        run_paint: &Paint,
    ) {
        let glyph_ids = glyph_run.glyph_ids();
        let glyph_count = glyph_ids.len();
        let glyph_run_font = glyph_run.font();

        if glyph_count == 0 || glyph_run_font.size() <= 0.0 || Self::has_empty_clip(ctx, clip_stack)
        {
            return;
        }

        // TODO: SkPDFFont has code to handle paints with mask filters, but the viewers do not.
        // See https://crbug.com/362796158 for Pdfium and b/325266484 for Preview
        if ctx.local_to_device.has_perspective() || run_paint.mask_filter().is_some() {
            self.draw_glyph_run_as_path(ctx, clip_stack, glyph_run, offset, run_paint);
            return;
        }

        let doc = self.doc.clone();
        let Some(pdf_strike) = PdfStrike::make(&doc, glyph_run_font, run_paint) else {
            return;
        };
        let typeface = pdf_strike.path().strike_spec.typeface().clone();

        let Some(metrics) = get_metrics(&typeface, &doc) else {
            return;
        };

        let glyph_to_unicode = get_unicode_map(&typeface, &doc);
        let glyph_to_unicode_ex = get_unicode_map_ex(&typeface, &doc);

        // TODO: FontType should probably be on SkPDFStrike?
        let initial_font_type = font_type(&pdf_strike, &metrics);

        let mut clusterator = Clusterator::new(glyph_run);

        // The size, skewX, and scaleX are applied here.
        let text_size = glyph_run_font.size();
        let advance_scale = text_size * glyph_run_font.scale_x() / pdf_strike.path().units_per_em;

        // textScaleX and textScaleY are used to get a conservative bounding box for glyphs.
        let text_scale_y = text_size / pdf_strike.path().units_per_em;
        let text_scale_x = advance_scale + glyph_run_font.skew_x() * text_scale_y;

        let clip_stack_bounds = clip_stack.bounds(&ctx.bounds);

        // Clear everything from the runPaint that will be applied by the strike.
        let mut fill_paint = run_paint.clone();
        if fill_paint.stroke_width() > 0.0 {
            fill_paint.set_stroke(false);
        }
        fill_paint.set_path_effect(None);
        fill_paint.set_mask_filter(None);
        let paint = clean_paint(&fill_paint);
        let content = self.begin_entry(
            ctx,
            Some(clip_stack),
            &ctx.local_to_device,
            &paint,
            glyph_run_font.scale_x(),
        );
        if !content.is_active() {
            return;
        }

        // Destinations are in absolute coordinates.
        // The glyphs bounds go through the localToDevice separately for clipping.
        let page_xform = self.page_xform(ctx);

        self.begin_mark();
        if !glyph_run.text().is_empty() {
            let elem_id = self.mark_manager.elem_id();
            doc.with(|d| {
                d.struct_tree
                    .add_struct_elem_title(elem_id, glyph_run.text())
            });
        }

        // `out` is the stream of the active stack state: a field borrow, so the other members
        // stay usable while the text is written.
        macro_rules! out {
            () => {
                match self.active_stack_state.stream {
                    Some(StreamSelector::ContentBuffer) => &mut self.content_buffer,
                    Some(StreamSelector::Content) | None => &mut self.content,
                }
            };
        }

        out!().write_text("BT\n");

        let num_glyphs = typeface.count_glyphs();

        if clusterator.reversed_chars() {
            out!().write_text("/ReversedChars BMC\n");
        }
        let mut glyph_positioner = GlyphPositioner::new(glyph_run_font.skew_x(), offset);
        let mut font: Option<PdfFont> = None;

        let paths = BulkGlyphMetricsAndPaths::new(&pdf_strike.path().strike_spec);
        let glyphs = paths.glyphs(glyph_ids);

        loop {
            let c = clusterator.next();
            if !c.is_valid() {
                break;
            }
            let mut glyph_index = c.glyph_index as usize;
            let glyph_limit = glyph_index + c.glyph_count as usize;

            let mut actual_text = false;
            if let Some(utf8_text) = c.utf8_text {
                let mut to_unicode = false;
                let mut text_ptr: &[u8] = utf8_text;
                let cluster_unichar = next_utf8(&mut text_ptr);
                // ToUnicode can only handle one glyph in a cluster.
                if cluster_unichar >= 0 && c.glyph_count == 1 {
                    let gid = glyph_ids[glyph_index];
                    let font_unichar = glyph_to_unicode.get(usize::from(gid)).copied().unwrap_or(0);

                    // The regular cmap can handle this if there is one glyph in the cluster,
                    // one code point in the cluster, and the glyph maps to the code point.
                    to_unicode = text_ptr.is_empty() && cluster_unichar == font_unichar;

                    // The extended cmap can handle this if there is one glyph in the cluster,
                    // the font has no code point for the glyph,
                    // there are less than 512 bytes in the UTF-16,
                    // and the mapping matches or can be added.
                    // UTF-16 uses at most 2x space of UTF-8; 64 code points seems enough.
                    if !to_unicode && font_unichar <= 0 && c.text_byte_length < 256 {
                        let mut ex = glyph_to_unicode_ex.borrow_mut();
                        match ex.find(&gid) {
                            None => {
                                ex.set(gid, utf8_text.to_vec());
                                to_unicode = true;
                            }
                            Some(unicodes) => {
                                if unicodes.as_slice() == utf8_text {
                                    to_unicode = true;
                                }
                            }
                        }
                    }
                }
                if !to_unicode {
                    let out = out!();
                    glyph_positioner.flush(out);
                    // Begin marked-content sequence with associated property list.
                    out.write_text("/Span<</ActualText ");
                    write_text_string_to(out, utf8_text);
                    out.write_text(" >> BDC\n");
                    actual_text = true;
                }
            }
            while glyph_index < glyph_limit {
                let this_index = glyph_index;
                glyph_index += 1;
                let gid = glyph_ids[this_index];
                if num_glyphs <= i32::from(gid) {
                    continue;
                }
                let xy = glyph_run.positions()[this_index];
                // Do a glyph-by-glyph bounds-reject if positions are absolute.
                let glyph_bounds = get_glyph_bounds_device_space(
                    &glyphs[this_index],
                    text_scale_x,
                    text_scale_y,
                    Point::new(xy.x + offset.x, xy.y + offset.y),
                    &ctx.local_to_device,
                );
                if glyph_bounds.is_empty() {
                    if !contains(
                        &clip_stack_bounds,
                        Point::new(glyph_bounds.left, glyph_bounds.top),
                    ) {
                        continue;
                    }
                } else if !clip_stack_bounds.intersects(glyph_bounds) {
                    continue; // reject glyphs as out of bounds
                }
                if needs_new_font(font.as_ref(), &glyphs[this_index], initial_font_type) {
                    // Not yet specified font or need to switch font.
                    let new_font = pdf_strike.get_font_resource(&doc, &glyphs[this_index]);
                    let out = out!();
                    glyph_positioner.set_font(out, new_font.clone());
                    let index =
                        add_resource(&mut self.font_resources, new_font.indirect_reference());
                    write_resource_name(out, ResourceType::Font, index);
                    out.write_text(" ");
                    append_scalar(text_size, out);
                    out.write_text(" Tf\n");
                    font = Some(new_font);
                }
                let font = font.as_ref().expect("a font was selected");
                font.note_glyph_usage(gid);
                let encoded_glyph = font.glyph_to_pdf_font_encoding(gid);
                let advance = advance_scale * glyphs[this_index].advance_x();
                if self.mark_manager.has_active_mark() {
                    let page_glyph_bounds = page_xform.map_rect(glyph_bounds).0;
                    self.mark_manager
                        .accumulate(Point::new(page_glyph_bounds.left, page_glyph_bounds.bottom)); // y-up
                }
                glyph_positioner.write_glyph(out!(), encoded_glyph, advance, xy);
            }

            // SK_AT_SCOPE_EXIT of the cluster
            if actual_text {
                let out = out!();
                glyph_positioner.flush(out);
                out.write_text("EMC\n");
            }
        }

        // The scope exits, in reverse order of declaration: the positioner, `EMC`, `ET`.
        glyph_positioner.flush(out!());
        if clusterator.reversed_chars() {
            out!().write_text("EMC\n");
        }
        out!().write_text("ET\n");

        self.end_entry(ctx, Some(clip_stack), content);
    }
}
