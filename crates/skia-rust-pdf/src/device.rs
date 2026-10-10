// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFDevice.{h,cpp} (chrome/m156)

//! `SkPDFDevice`: the drawing context for a page or layer of PDF content.
//!
//! The device writes a content stream. A paint becomes a graphic state, a color or a pattern
//! resource, and the geometry becomes path operators; blend modes PDF does not have are done with
//! form `XObjects` and soft masks. Layers, patterns and masks are devices of their own that are
//! written as form `XObjects`.
//!
//! skia-rust: `SkPDFDevice` is split in two. [`PdfDevice`] is the `SkDevice`: the device state,
//! the clip stack (`SkClipStackDevice`) and a handle to the [`Content`], which has the rest of the
//! members (the content streams, the resources, the graphic stack, the marked-content manager).
//! The document keeps the handle of the page device's content to read the page when it ends, and
//! a device drawn into a canvas is gone from the caller's hands, so the content is shared.
//! `ScopedContentEntry` is a [`ScopedEntry`] that the draw begins and ends explicitly.
//!
//! Not ported yet, and waiting for the fonts (`modules.md` M26): drawing glyphs. A glyph run
//! draws nothing (`PdfDevice::on_draw_glyph_run_list`), where Skia's `internalDrawGlyphRun`,
//! `drawGlyphRunAsPath` and `GlyphPositioner` write the text, so the font resources stay empty.

#![allow(clippy::cast_precision_loss)] // SkIntToScalar-style casts of pixel sizes mirror the C++

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blend_mode_priv::{BlendFastPath, check_fast_path};
use skia_rust_core::blender::Blender;
use skia_rust_core::canvas::{Canvas, PointMode, SrcRectConstraint};
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::clip_stack::{ClipStack, WIDE_OPEN_GEN_ID};
use skia_rust_core::color::Color4f;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_space_priv::srgb_singleton;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::device::{CreateInfo, Device, DeviceState, draw_device_default};
use skia_rust_core::glyph_run::GlyphRunList;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask::{CreateMode, MaskBuilder};
use skia_rust_core::matrix::{Matrix, TypeMask};
use skia_rust_core::mesh::Mesh;
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::paint_priv::remove_color_filter;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_enums::ResolveConvexity;
use skia_rust_core::path_priv::raw_builder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::{IPoint, Point};
use skia_rust_core::rect::{IRect, Rect, RoundOut};
use skia_rust_core::region::Region;
use skia_rust_core::rrect::RRect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::{ColorShader, ShaderType};
use skia_rust_core::size::{ISize, Size};
use skia_rust_core::special_image::SpecialImage;
use skia_rust_core::stream::{DynamicMemoryWStream, WStream};
use skia_rust_core::stroke_rec::InitStyle;
use skia_rust_core::surface_props::{PixelGeometry, SurfaceProps, SurfacePropsFlags};
use skia_rust_core::vertices::Vertices;
use skia_rust_pathops::path_op::PathOp;
use skia_rust_raster::bitmap_device::BitmapDevice;
use skia_rust_raster::draw::{Draw, draw_to_mask};
use skia_rust_raster::raster_clip::RasterClip;
use skia_rust_raster::surfaces::raster_n32_premul;

use crate::bitmap::{serialize_image_size, serialize_image_xobject};
use crate::clip_stack_device::{ClipStackDevice, clip_stack_as_path};
use crate::clusterator::Clusterator;
use crate::document::{DocHandle, LinkType, PdfLink, PdfNamedDestination, elem_id_key};
use crate::form_xobject::make_form_x_object;
use crate::graphic_stack_state::{Entry, GraphicStackState, StreamSelector};
use crate::graphic_state::{SMaskMode, get_graphic_state_for_paint, get_smask_graphic_state};
use crate::keyed_image::KeyedImage;
use crate::resource_dict::{ResourceType, make_resource_dict, write_resource_name};
use crate::shader::make_shader;
use crate::tag::{Mark, node_id};
use crate::types::{PdfDict, PdfIndirectReference, PdfParentTreeKey, PdfUnion};
use crate::utils::{
    EmptyArea, EmptyPath, EmptyVerb, append_line, append_rectangle, append_transform,
    blend_mode_name, close_path, emit_path, make_int_array, move_to, paint_path, stroke_path,
};
use skia_rust_core::annotation::AnnotationKeys;

/// `SK_PDF_MASK_QUALITY`: the JPEG quality of the masks.
// Port of: src/pdf/SkPDFTypes.h#L28-L31 (chrome/m156)
const PDF_MASK_QUALITY: i32 = 50;

/// The handle to the content of a device, shared with the document.
pub(crate) type ContentHandle = Rc<RefCell<Content>>;

/// What a drawing method reads from the device that it does not own: `bounds()`,
/// `deviceToGlobal().asM33()` and `localToDevice()`.
#[derive(Debug, Clone)]
pub(crate) struct DeviceCtx {
    pub bounds: IRect,
    pub device_to_global: Matrix,
    pub local_to_device: Matrix,
}

impl DeviceCtx {
    /// The context of a device that was not drawn into (a shape device): identity transforms.
    fn identity(size: ISize) -> Self {
        Self {
            bounds: IRect::from_wh(size.width, size.height),
            device_to_global: Matrix::new_identity(),
            local_to_device: Matrix::new_identity(),
        }
    }
}

/// `MarkedContentManager`: tracks the marked-content sequence of the structure element that the
/// draws belong to.
// Port of: src/pdf/SkPDFDevice.h#L125-L170, src/pdf/SkPDFDevice.cpp#L90-L180 (chrome/m156)
#[derive(Debug)]
struct MarkedContentManager {
    doc: DocHandle,
    current_active_mark: Mark,
    current_marks_elem_id: i32,
    next_marks_elem_id: i32,
    struct_parents_key: PdfParentTreeKey,
}

impl MarkedContentManager {
    fn new(doc: DocHandle) -> Self {
        Self {
            doc,
            current_active_mark: Mark::default(),
            current_marks_elem_id: 0,
            next_marks_elem_id: 0,
            struct_parents_key: PdfParentTreeKey::default(),
        }
    }

    /// `setNextMarksElemId`: sets the current element identifier. Associate future draws with the
    /// structure element with the given element identifier. Element identifier 0 is reserved to
    /// mean no structure element.
    fn set_next_marks_elem_id(&mut self, next_marks_elem_id: i32) {
        self.next_marks_elem_id = next_marks_elem_id;
    }

    /// `elemId`: the current element identifier.
    fn elem_id(&self) -> i32 {
        self.next_marks_elem_id
    }

    /// `beginMark`: starts a marked-content sequence for a content item for the structure element
    /// with the current element identifier. If there is an active marked-content sequence
    /// associated with a different element identifier the active marked-content sequence will
    /// first be closed. If there is no structure element with the current element identifier then
    /// the marked-content sequence will not be started.
    fn begin_mark(&mut self, out: &mut DynamicMemoryWStream) {
        if self.next_marks_elem_id == self.current_marks_elem_id {
            return;
        }
        if self.current_marks_elem_id != 0 {
            // End this mark
            out.write_text("EMC\n");
            self.current_active_mark = Mark::default();
            self.current_marks_elem_id = 0;
        }
        if self.next_marks_elem_id != 0 {
            let next = self.next_marks_elem_id;
            let mark = self
                .doc
                .with(|d| d.create_mark_for_elem_id(next, &mut self.struct_parents_key));
            self.current_active_mark = mark;
            if self.current_active_mark.is_valid() {
                // Begin this mark
                let (struct_type, mcid, elem_id) = self.doc.with(|d| {
                    (
                        mark.struct_type(&d.struct_tree),
                        mark.mcid(&d.struct_tree),
                        mark.elem_id(&d.struct_tree),
                    )
                });
                PdfUnion::name_escaped(struct_type).emit_object(out);
                out.write_text(" <</MCID ");
                out.write_dec_as_text(mcid);
                out.write_text(" >>BDC\n");
                self.current_marks_elem_id = elem_id;
            } else if (node_id::BACKGROUND_ARTIFACT..=node_id::OTHER_ARTIFACT)
                .contains(&self.next_marks_elem_id)
                && self.doc.has_current_page()
            {
                out.write_text("/Artifact");
                let next = self.next_marks_elem_id;
                if next == node_id::OTHER_ARTIFACT {
                    out.write_text(" BMC\n");
                } else if next == node_id::PAGINATION_ARTIFACT
                    || next == node_id::PAGINATION_HEADER_ARTIFACT
                    || next == node_id::PAGINATION_FOOTER_ARTIFACT
                    || next == node_id::PAGINATION_WATERMARK_ARTIFACT
                {
                    out.write_text(" <</Type /Pagination");
                    if next == node_id::PAGINATION_HEADER_ARTIFACT {
                        out.write_text(" /Subtype /Header");
                    } else if next == node_id::PAGINATION_FOOTER_ARTIFACT {
                        out.write_text(" /Subtype /Footer");
                    } else if next == node_id::PAGINATION_WATERMARK_ARTIFACT {
                        out.write_text(" /Subtype /Watermark");
                    }
                    out.write_text(" >>BDC\n");
                } else if next == node_id::LAYOUT_ARTIFACT {
                    out.write_text(" <</Type /Layout >>BDC\n");
                } else if next == node_id::PAGE_ARTIFACT {
                    out.write_text(" <</Type /Page >>BDC\n");
                } else if next == node_id::BACKGROUND_ARTIFACT {
                    out.write_text(" <</Type /Background >>BDC\n");
                }
                self.current_marks_elem_id = next;
            }
        }
    }

    /// `hasActiveMark`: whether there is an active marked-content sequence.
    fn has_active_mark(&self) -> bool {
        self.current_active_mark.is_valid()
    }

    /// `accumulate`: accumulates an upper left location for the active mark. The point is in PDF
    /// page space and so is y-up. Only use if `has_active_mark`.
    fn accumulate(&self, p: Point) {
        debug_assert!(self.current_active_mark.is_valid());
        let mark = self.current_active_mark;
        self.doc.with(|d| mark.accumulate(&mut d.struct_tree, p));
    }

    /// `structParentsKey`: the key (index) into the `ParentsTree`. Valid if marks were made.
    fn struct_parents_key(&self) -> PdfParentTreeKey {
        self.struct_parents_key
    }

    /// `reset`.
    fn reset(&mut self) {
        // fDoc remains the same
        // fOut remains the same (device's fContent may be reset but remains valid)
        debug_assert!(!self.has_active_mark()); // fCurrentlyActiveMark and fCurrentMarksElemId unset
        // fNextMarksElemId unchanged, it is still this device's active structure element id.
        self.struct_parents_key = PdfParentTreeKey::default();
    }
}

/// This function destroys the mask and either frees or takes the pixels.
// Port of: src/pdf/SkPDFDevice.cpp#L182-L215 (mask_to_greyscale_image, chrome/m156)
#[allow(clippy::needless_pass_by_value)] // the C++ destroys the mask
fn mask_to_greyscale_image(mask: MaskBuilder, doc: &DocHandle) -> Option<Image> {
    let info = ImageInfo::new(
        (mask.bounds.width(), mask.bounds.height()),
        ColorType::Gray8,
        AlphaType::Opaque,
        None,
    );
    let pm = Pixmap::new_readonly(&info, &mask.image, mask.row_bytes as usize)?;
    let mut img = None;
    // `constexpr int imgQuality = SK_PDF_MASK_QUALITY; if constexpr (imgQuality <= 100 && >= 0)`
    let metadata = doc.metadata();
    if let (Some(encode_jpeg), Some(decode_jpeg)) = (metadata.jpeg_encoder, metadata.jpeg_decoder) {
        let mut buffer = DynamicMemoryWStream::new();
        // By encoding this into jpeg, it be embedded efficiently during drawImage.
        if encode_jpeg(&mut buffer, &pm, PDF_MASK_QUALITY) {
            let codec = decode_jpeg(buffer.detach_as_data());
            debug_assert!(codec.is_some());
            img = skia_rust_codec::codecs::deferred_image(codec, None);
            debug_assert!(img.is_some());
        }
    }
    if img.is_none() {
        img = skia_rust_core::images::raster_from_pixmap_copy(&pm);
    }
    img
}

// Port of: src/pdf/SkPDFDevice.cpp#L217-L228 (alpha_image_to_greyscale_image, chrome/m156)
fn alpha_image_to_greyscale_image(mask: &Image) -> Option<Image> {
    let (w, h) = (mask.width(), mask.height());
    let mut grey_bitmap = Bitmap::new();
    grey_bitmap.alloc_pixels_info(
        &ImageInfo::new((w, h), ColorType::Gray8, AlphaType::Opaque, None),
        None,
    );
    // TODO: support gpu images in pdf
    {
        let mut pixmap = grey_bitmap.peek_pixels_mut()?;
        let row_bytes = pixmap.row_bytes();
        let pixels = pixmap.writable_addr()?;
        if !mask.read_pixels(&ImageInfo::new_a8((w, h)), pixels, row_bytes, (0, 0)) {
            return None;
        }
    }
    grey_bitmap.set_immutable();
    grey_bitmap.as_image()
}

// Port of: src/pdf/SkPDFDevice.cpp#L230-L233 (add_resource, chrome/m156)
fn add_resource(
    resources: &mut BTreeSet<PdfIndirectReference>,
    reference: PdfIndirectReference,
) -> i32 {
    resources.insert(reference);
    reference.value
}

// Port of: src/pdf/SkPDFDevice.cpp#L257-L276 (transform_shader, chrome/m156)
fn transform_shader(paint: &mut Paint, ctm: &Matrix) {
    debug_assert!(!ctm.is_identity());
    if let Some(shader) = paint.shader() {
        paint.set_shader(shader.with_local_matrix(ctm));
    }
}

// Port of: src/pdf/SkPDFDevice.cpp#L279-L293 (clean_paint, chrome/m156)
fn clean_paint(src_paint: &Paint) -> Paint {
    let mut paint = src_paint.clone();
    // If the paint will definitely draw opaquely, replace kSrc with
    // kSrcOver.  http://crbug.com/473572
    if !paint.is_src_over() && BlendFastPath::SrcOver == check_fast_path(&paint, false) {
        paint.set_blend_mode(BlendMode::SrcOver);
    }
    if paint.color_filter().is_some() {
        // We assume here that PDFs all draw in sRGB.
        remove_color_filter(&mut paint, Some(srgb_singleton()));
    }
    debug_assert!(paint.color_filter().is_none());
    paint
}

// Port of: src/pdf/SkPDFDevice.cpp#L295-L299 (set_style, chrome/m156)
fn set_style(paint: &mut Paint, style: Style) {
    if paint.style() != style {
        paint.set_style(style);
    }
}

/// Calculate an inverted path's equivalent non-inverted path, given the canvas bounds.
// Port of: src/pdf/SkPDFDevice.cpp#L301-L312 (calculate_inverse_path, chrome/m156)
fn calculate_inverse_path(bounds: &Rect, inv_path: &Path) -> Option<Path> {
    debug_assert!(inv_path.is_inverse_fill_type());
    skia_rust_pathops::op(&Path::rect(bounds, None), inv_path, PathOp::Intersect)
}

// Port of: src/pdf/SkPDFDevice.cpp#L530-L535 (treat_as_regular_pdf_blend_mode, chrome/m156)
fn treat_as_regular_pdf_blend_mode(blend_mode: BlendMode) -> bool {
    blend_mode_name(blend_mode).is_some()
}

// Port of: src/pdf/SkPDFDevice.cpp#L760-L762 (is_integer, chrome/m156)
#[allow(clippy::float_cmp)] // exact comparison, as in Skia
fn is_integer(x: f32) -> bool {
    x == x.trunc()
}

// Port of: src/pdf/SkPDFDevice.cpp#L764-L769 (is_integral, chrome/m156)
fn is_integral(r: &Rect) -> bool {
    is_integer(r.left) && is_integer(r.top) && is_integer(r.right) && is_integer(r.bottom)
}

/// A draw in progress (`ScopedContentEntry`): the blend mode of the paint, the form `XObject` of
/// what was already drawn when the blend mode needs the destination, and the shape of the draw.
// Port of: src/pdf/SkPDFDevice.cpp#L317-L406 (ScopedContentEntry, chrome/m156)
#[derive(Debug)]
pub(crate) struct ScopedEntry {
    /// `fContentStream != nullptr`.
    active: bool,
    blend_mode: BlendMode,
    dst_form_x_object: PdfIndirectReference,
    shape: Path,
}

impl ScopedEntry {
    /// `explicit operator bool`.
    fn is_active(&self) -> bool {
        self.active
    }

    /// `needShape`: true when we explicitly need the shape of the drawing.
    fn need_shape(&self) -> bool {
        matches!(
            self.blend_mode,
            BlendMode::Clear
                | BlendMode::Src
                | BlendMode::SrcIn
                | BlendMode::SrcOut
                | BlendMode::DstIn
                | BlendMode::DstOut
                | BlendMode::SrcATop
                | BlendMode::DstATop
                | BlendMode::Modulate
        )
    }

    /// `needSource`: true unless we only need the shape of the drawing.
    fn need_source(&self) -> bool {
        self.blend_mode != BlendMode::Clear
    }

    /// `setShape`: if the shape is different than the alpha component of the content, then
    /// `set_shape` should be called with the shape. In particular, images and devices have
    /// rectangular shape.
    fn set_shape(&mut self, shape: Path) {
        self.shape = shape;
    }
}

/// The members of `SkPDFDevice` other than the device state and the clip: the content, the
/// resources it uses, the graphic stack and the marked content.
// Port of: src/pdf/SkPDFDevice.h#L110-L195 (chrome/m156)
#[derive(Debug)]
#[allow(clippy::struct_field_names)] // mirrors SkPDFDevice::fContent and fContentBuffer
pub(crate) struct Content {
    doc: DocHandle,
    size: ISize,
    initial_transform: Matrix,
    graphic_state_resources: BTreeSet<PdfIndirectReference>,
    x_object_resources: BTreeSet<PdfIndirectReference>,
    shader_resources: BTreeSet<PdfIndirectReference>,
    font_resources: BTreeSet<PdfIndirectReference>,
    mark_manager: MarkedContentManager,
    content: DynamicMemoryWStream,
    content_buffer: DynamicMemoryWStream,
    needs_extra_save: bool,
    active_stack_state: GraphicStackState,
}

impl Content {
    fn new(size: ISize, doc: &DocHandle, initial_transform: &Matrix) -> Self {
        debug_assert!(size.width > 0 && size.height > 0);
        Self {
            doc: doc.clone(),
            size,
            initial_transform: initial_transform.clone(),
            graphic_state_resources: BTreeSet::new(),
            x_object_resources: BTreeSet::new(),
            shader_resources: BTreeSet::new(),
            font_resources: BTreeSet::new(),
            mark_manager: MarkedContentManager::new(doc.clone()),
            content: DynamicMemoryWStream::new(),
            content_buffer: DynamicMemoryWStream::new(),
            needs_extra_save: false,
            active_stack_state: GraphicStackState::default(),
        }
    }

    /// The size of the device (`size()`).
    pub(crate) fn size(&self) -> ISize {
        self.size
    }

    /// `structParentsKey`.
    pub(crate) fn struct_parents_key(&self) -> PdfParentTreeKey {
        self.mark_manager.struct_parents_key()
    }

    /// The stream the graphic stack writes to (`fActiveStackState.fContentStream`).
    fn active_stream(&mut self) -> &mut DynamicMemoryWStream {
        match self.active_stack_state.stream {
            Some(StreamSelector::ContentBuffer) => &mut self.content_buffer,
            Some(StreamSelector::Content) | None => &mut self.content,
        }
    }

    /// `fActiveStackState.drainStack()`.
    fn drain_stack(&mut self) {
        let out: &mut DynamicMemoryWStream = match self.active_stack_state.stream {
            Some(StreamSelector::ContentBuffer) => &mut self.content_buffer,
            _ => &mut self.content,
        };
        self.active_stack_state.drain_stack(out);
    }

    /// `fMarkManager.beginMark()`: the sequences are written to `fContent`.
    fn begin_mark(&mut self) {
        self.mark_manager.begin_mark(&mut self.content);
    }

    /// `reset`.
    fn reset(&mut self) {
        self.graphic_state_resources.clear();
        self.x_object_resources.clear();
        self.shader_resources.clear();
        self.font_resources.clear();
        self.mark_manager.reset();
        self.content.reset();
        self.active_stack_state = GraphicStackState::default();
    }

    /// `isContentEmpty`.
    fn is_content_empty(&self) -> bool {
        self.content.bytes_written() == 0 && self.content_buffer.bytes_written() == 0
    }

    /// `makeResourceDict`: creates the resource dictionary for this device.
    // Port of: src/pdf/SkPDFDevice.cpp#L707-L721 (chrome/m156)
    pub(crate) fn make_resource_dict(&self) -> PdfDict {
        let sorted = |set: &BTreeSet<PdfIndirectReference>| -> Vec<PdfIndirectReference> {
            // Sorted by object number.
            set.iter().copied().collect()
        };
        make_resource_dict(
            &sorted(&self.graphic_state_resources),
            &sorted(&self.shader_resources),
            &sorted(&self.x_object_resources),
            &sorted(&self.font_resources),
        )
    }

    /// `content`: the page contents.
    // Port of: src/pdf/SkPDFDevice.cpp#L723-L756 (chrome/m156)
    pub(crate) fn content(&mut self) -> Vec<u8> {
        if self.active_stack_state.stream.is_some() {
            self.drain_stack();
            self.active_stack_state = GraphicStackState::default();
        }
        if self.content.bytes_written() == 0 {
            return Vec::new();
        }

        // Implicitly close any still active marked-content sequence.
        // Must do this before fContent is written to buffer.
        let elem_id = self.mark_manager.elem_id();
        self.mark_manager.set_next_marks_elem_id(0);
        self.begin_mark();

        let mut buffer = DynamicMemoryWStream::new();
        if self.initial_transform.get_type() != TypeMask::empty() {
            append_transform(&self.initial_transform, &mut buffer);
        }
        if self.needs_extra_save {
            buffer.write_text("q\n");
        }
        self.content.write_to_and_reset_dynamic(&mut buffer);
        if self.needs_extra_save {
            buffer.write_text("Q\n");
        }
        self.needs_extra_save = false;

        // Subsequent use of this SkPDFDevice is still associated with the current structure
        // element.
        self.mark_manager.set_next_marks_elem_id(elem_id);

        buffer.detach_as_vector()
    }

    /// `makeCongruentDevice`.
    // Port of: src/pdf/SkPDFDevice.cpp#L420-L422 (chrome/m156)
    fn make_congruent_device(&self) -> PdfDevice {
        PdfDevice::new(self.size, &self.doc, &Matrix::new_identity())
    }

    /// `setGraphicState`.
    // Port of: src/pdf/SkPDFDevice.cpp#L601-L603 (chrome/m156)
    fn set_graphic_state(&mut self, gs: PdfIndirectReference) {
        let index = add_resource(&mut self.graphic_state_resources, gs);
        crate::utils::apply_graphic_state(index, self.active_stream());
    }

    /// `clearMaskOnGraphicState`.
    // Port of: src/pdf/SkPDFDevice.cpp#L605-L614 (chrome/m156)
    fn clear_mask_on_graphic_state(&mut self) {
        // The no-softmask graphic state is used to "turn off" the mask for later draw calls.
        let mut no_s_mask_gs = self.doc.with(|d| d.no_smask_graphic_state);
        if !no_s_mask_gs.is_valid() {
            let mut tmp = PdfDict::new(Some("ExtGState"));
            tmp.insert_name("SMask", "None");
            no_s_mask_gs = self.doc.emit_new(&tmp);
            self.doc.with(|d| d.no_smask_graphic_state = no_s_mask_gs);
        }
        self.set_graphic_state(no_s_mask_gs);
    }

    /// The matrix `fDocument->currentPageTransform()` after `deviceToGlobal().asM33()`
    /// (`pageXform` of the marked-content accumulation).
    fn page_xform(&self, ctx: &DeviceCtx) -> Matrix {
        // Destinations are in absolute coordinates.
        let mut page_xform = ctx.device_to_global.clone();
        page_xform.post_concat(&self.doc.with(|d| d.current_page_transform()));
        page_xform
    }

    ////////////////////////////////////////////////////////////////////////////

    /// `ScopedContentEntry(device, clipStack, matrix, paint, textScale)`: begins a draw. The
    /// returned entry is not active if the paint or clip is such that nothing should be drawn.
    // Port of: src/pdf/SkPDFDevice.cpp#L317-L336 (chrome/m156)
    fn begin_entry(
        &mut self,
        ctx: &DeviceCtx,
        clip_stack: Option<&ClipStack>,
        matrix: &Matrix,
        paint: &Paint,
        text_scale: f32,
    ) -> ScopedEntry {
        let mut entry = ScopedEntry {
            active: false,
            blend_mode: BlendMode::SrcOver,
            dst_form_x_object: PdfIndirectReference::default(),
            shape: Path::new(),
        };
        if matrix.has_perspective() {
            // NOT_IMPLEMENTED(!matrix.hasPerspective(), false);
            return entry;
        }
        entry.blend_mode = paint.blend_mode_or(BlendMode::SrcOver);
        entry.active = self.set_up_content_entry(
            ctx,
            clip_stack,
            matrix,
            paint,
            text_scale,
            &mut entry.dst_form_x_object,
        );
        entry
    }

    /// `~ScopedContentEntry`: ends a draw.
    // Port of: src/pdf/SkPDFDevice.cpp#L338-L346 (chrome/m156)
    fn end_entry(&mut self, ctx: &DeviceCtx, clip_stack: Option<&ClipStack>, entry: ScopedEntry) {
        if entry.active {
            let shape = if entry.shape.is_empty() {
                None
            } else {
                Some(entry.shape)
            };
            self.finish_content_entry(
                ctx,
                clip_stack,
                entry.blend_mode,
                entry.dst_form_x_object,
                shape.as_ref(),
            );
        }
    }

    /// `populate_graphic_state_entry_from_paint`.
    // Port of: src/pdf/SkPDFDevice.cpp#L537-L618 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++
    fn populate_graphic_state_entry_from_paint(
        doc: &DocHandle,
        matrix: &Matrix,
        clip_stack: Option<&ClipStack>,
        device_bounds: &IRect,
        paint: &Paint,
        initial_transform: &Matrix,
        text_scale: f32,
        entry: &mut Entry,
        shader_resources: &mut BTreeSet<PdfIndirectReference>,
        graphic_state_resources: &mut BTreeSet<PdfIndirectReference>,
    ) {
        // NOT_IMPLEMENTED(paint.getPathEffect() != nullptr, false);
        // NOT_IMPLEMENTED(paint.getMaskFilter() != nullptr, false);
        // NOT_IMPLEMENTED(paint.getColorFilter() != nullptr, false);

        entry.matrix = matrix.clone();
        entry.clip_stack_gen_id = clip_stack.map_or(WIDE_OPEN_GEN_ID, ClipStack::topmost_gen_id);
        let mut color = paint.color4f();
        entry.color = Color4f::new(color.r, color.g, color.b, 1.0);
        entry.shader_index = -1;

        // PDF treats a shader as a color, so we only set one or the other.
        if let Some(shader) = paint.shader_ref() {
            if shader.as_base().shader_type() == ShaderType::Color {
                let base: &dyn std::any::Any = shader.as_base();
                let color_shader = base.downcast_ref::<ColorShader>().expect("a color shader");
                // We don't have to set a shader just for a color.
                color = color_shader.color();
                color.a *= paint.alpha_f();
                entry.color = color_shader.color().to_opaque();
            } else {
                // PDF positions patterns relative to the initial transform, so
                // we need to apply the current transform to the shader parameters.
                let mut transform = matrix.clone();
                transform.post_concat(initial_transform);

                // PDF doesn't support kClamp_TileMode, so we simulate it by making
                // a pattern the size of the current clip.
                let clip_stack_bounds = clip_stack.map_or_else(
                    || Rect::from_irect(device_bounds),
                    |cs| cs.bounds(device_bounds),
                );

                // We need to apply the initial transform to bounds in order to get
                // bounds in a consistent coordinate system.
                let clip_stack_bounds = initial_transform.map_rect(clip_stack_bounds).0;
                let bounds: IRect = clip_stack_bounds.round_out();

                // Use alpha 1 for the shader, the paint alpha is applied with newGraphicsState
                // (below)
                let c = paint.color4f();
                let pdf_shader = make_shader(
                    doc,
                    shader,
                    &transform,
                    &bounds,
                    &Color4f::new(c.r, c.g, c.b, 1.0),
                );

                if pdf_shader.is_valid() {
                    // pdfShader has been canonicalized so we can directly compare pointers.
                    entry.shader_index = add_resource(shader_resources, pdf_shader);
                }
            }
        }

        let new_graphic_state = if color == paint.color4f() {
            get_graphic_state_for_paint(doc, paint)
        } else {
            let mut new_paint = paint.clone();
            new_paint.set_color4f(color, None);
            get_graphic_state_for_paint(doc, &new_paint)
        };
        entry.graphic_state_index = add_resource(graphic_state_resources, new_graphic_state);
        entry.text_scale_x = text_scale;
    }

    /// `setUpContentEntry`: returns true if the stream is there to draw into.
    // Port of: src/pdf/SkPDFDevice.cpp#L620-L690 (chrome/m156)
    fn set_up_content_entry(
        &mut self,
        ctx: &DeviceCtx,
        clip_stack: Option<&ClipStack>,
        matrix: &Matrix,
        paint: &Paint,
        text_scale: f32,
        dst: &mut PdfIndirectReference,
    ) -> bool {
        debug_assert!(!dst.is_valid());
        let blend_mode = paint.blend_mode_or(BlendMode::SrcOver);

        // Dst xfer mode doesn't draw source at all.
        if blend_mode == BlendMode::Dst {
            return false;
        }

        // For the following modes, we want to handle source and destination
        // separately, so make an object of what's already there.
        if !treat_as_regular_pdf_blend_mode(blend_mode) && blend_mode != BlendMode::DstOver {
            if !self.is_content_empty() {
                *dst = self.make_form_x_object_from_device(false);
                debug_assert!(self.is_content_empty());
            } else if blend_mode != BlendMode::Src && blend_mode != BlendMode::SrcOut {
                // Except for Src and SrcOut, if there isn't anything already there,
                // then we're done.
                return false;
            }
        }
        // TODO(vandebo): Figure out how/if we can handle the following modes:
        // Xor, Plus.  For now, we treat them as SrcOver/Normal.

        if treat_as_regular_pdf_blend_mode(blend_mode) {
            if self.active_stack_state.stream.is_none() {
                if self.content.bytes_written() != 0 {
                    self.content.write_text("Q\nq\n");
                    self.needs_extra_save = true;
                }
                self.active_stack_state = GraphicStackState::new(Some(StreamSelector::Content));
            } else {
                debug_assert_eq!(
                    self.active_stack_state.stream,
                    Some(StreamSelector::Content)
                );
            }
        } else {
            self.drain_stack();
            self.active_stack_state = GraphicStackState::new(Some(StreamSelector::ContentBuffer));
        }
        debug_assert!(self.active_stack_state.stream.is_some());
        let mut entry = Entry::default();
        Self::populate_graphic_state_entry_from_paint(
            &self.doc,
            matrix,
            clip_stack,
            &ctx.bounds,
            paint,
            &self.initial_transform,
            text_scale,
            &mut entry,
            &mut self.shader_resources,
            &mut self.graphic_state_resources,
        );
        let out: &mut DynamicMemoryWStream = match self.active_stack_state.stream {
            Some(StreamSelector::ContentBuffer) => &mut self.content_buffer,
            _ => &mut self.content,
        };
        self.active_stack_state
            .update_clip(out, clip_stack, &ctx.bounds);
        self.active_stack_state.update_matrix(out, &entry.matrix);
        self.active_stack_state.update_drawing_state(out, &entry);

        true
    }

    /// `finishContentEntry`.
    // Port of: src/pdf/SkPDFDevice.cpp#L692-L830 (chrome/m156)
    #[allow(clippy::too_many_lines)] // one function in Skia
    fn finish_content_entry(
        &mut self,
        ctx: &DeviceCtx,
        clip_stack: Option<&ClipStack>,
        mut blend_mode: BlendMode,
        dst: PdfIndirectReference,
        shape: Option<&Path>,
    ) {
        debug_assert_ne!(blend_mode, BlendMode::Dst);
        if treat_as_regular_pdf_blend_mode(blend_mode) {
            debug_assert!(!dst.is_valid());
            return;
        }

        debug_assert!(self.active_stack_state.stream.is_some());

        self.drain_stack();
        self.active_stack_state = GraphicStackState::default();

        if blend_mode == BlendMode::DstOver {
            debug_assert!(!dst.is_valid());
            if self.content_buffer.bytes_written() != 0 {
                if self.content.bytes_written() != 0 {
                    self.content_buffer.write_text("Q\nq\n");
                    self.needs_extra_save = true;
                }
                self.content_buffer.prepend_to_and_reset(&mut self.content);
                debug_assert_eq!(self.content_buffer.bytes_written(), 0);
            }
            return;
        }
        if self.content_buffer.bytes_written() != 0 {
            if self.content.bytes_written() != 0 {
                self.content.write_text("Q\nq\n");
                self.needs_extra_save = true;
            }
            self.content_buffer
                .write_to_and_reset_dynamic(&mut self.content);
            debug_assert_eq!(self.content_buffer.bytes_written(), 0);
        }

        if !dst.is_valid() {
            debug_assert!(blend_mode == BlendMode::Src || blend_mode == BlendMode::SrcOut);
            return;
        }

        debug_assert!(dst.is_valid());
        // Changing the current content into a form-xobject will destroy the clip
        // objects which is fine since the xobject will already be clipped. However
        // if source has shape, we need to clip it too, so a copy of the clip is
        // saved.

        let stock_paint = Paint::default();

        let mut src_form_x_object = PdfIndirectReference::default();
        if self.is_content_empty() {
            // If nothing was drawn and there's no shape, then the draw was a
            // no-op, but dst needs to be restored for that to be true.
            // If there is shape, then an empty source with Src, SrcIn, SrcOut,
            // DstIn, DstAtop or Modulate reduces to Clear and DstOut or SrcAtop
            // reduces to Dst.
            if shape.is_none()
                || blend_mode == BlendMode::DstOut
                || blend_mode == BlendMode::SrcATop
            {
                let content =
                    self.begin_entry(ctx, None, &Matrix::new_identity(), &stock_paint, 0.0);
                self.draw_form_x_object(ctx, dst, None);
                self.end_entry(ctx, None, content);
                return;
            }
            blend_mode = BlendMode::Clear;
        } else {
            src_form_x_object = self.make_form_x_object_from_device(false);
        }

        // TODO(vandebo) srcFormXObject may contain alpha, but here we want it
        // without alpha.
        if blend_mode == BlendMode::SrcATop {
            // TODO(vandebo): In order to properly support SrcATop we have to track
            // the shape of what's been drawn at all times. It's the intersection of
            // the non-transparent parts of the device and the outlines (shape) of
            // all images and devices drawn.
            self.draw_form_x_object_with_mask(
                ctx,
                src_form_x_object,
                dst,
                BlendMode::SrcOver,
                true,
            );
        } else if let Some(shape) = shape {
            // Draw shape into a form-xobject.
            let mut filled_paint = Paint::default();
            filled_paint.set_color(skia_rust_core::color::Color::BLACK);
            filled_paint.set_style(Style::Fill);
            let empty = ClipStack::new();
            let mut shape_dev = Content::new(self.size, &self.doc, &self.initial_transform);
            let shape_ctx = DeviceCtx::identity(self.size);
            shape_dev.internal_draw_path(
                &shape_ctx,
                clip_stack.unwrap_or(&empty),
                &Matrix::new_identity(),
                shape,
                &filled_paint,
            );
            let s_mask = shape_dev.make_form_x_object_from_device(false);
            self.draw_form_x_object_with_mask(ctx, dst, s_mask, BlendMode::SrcOver, true);
        } else {
            self.draw_form_x_object_with_mask(
                ctx,
                dst,
                src_form_x_object,
                BlendMode::SrcOver,
                true,
            );
        }

        if blend_mode == BlendMode::Clear {
            return;
        } else if blend_mode == BlendMode::Src || blend_mode == BlendMode::DstATop {
            let content = self.begin_entry(ctx, None, &Matrix::new_identity(), &stock_paint, 0.0);
            if content.is_active() {
                self.draw_form_x_object(ctx, src_form_x_object, None);
            }
            self.end_entry(ctx, None, content);
            if blend_mode == BlendMode::Src {
                return;
            }
        } else if blend_mode == BlendMode::SrcATop {
            let content = self.begin_entry(ctx, None, &Matrix::new_identity(), &stock_paint, 0.0);
            if content.is_active() {
                self.draw_form_x_object(ctx, dst, None);
            }
            self.end_entry(ctx, None, content);
        }

        debug_assert!(matches!(
            blend_mode,
            BlendMode::SrcIn
                | BlendMode::DstIn
                | BlendMode::SrcOut
                | BlendMode::DstOut
                | BlendMode::SrcATop
                | BlendMode::DstATop
                | BlendMode::Modulate
        ));

        if blend_mode == BlendMode::SrcIn
            || blend_mode == BlendMode::SrcOut
            || blend_mode == BlendMode::SrcATop
        {
            self.draw_form_x_object_with_mask(
                ctx,
                src_form_x_object,
                dst,
                BlendMode::SrcOver,
                blend_mode == BlendMode::SrcOut,
            );
        } else {
            let mut mode = BlendMode::SrcOver;
            if blend_mode == BlendMode::Modulate {
                self.draw_form_x_object_with_mask(
                    ctx,
                    src_form_x_object,
                    dst,
                    BlendMode::SrcOver,
                    false,
                );
                mode = BlendMode::Multiply;
            }
            self.draw_form_x_object_with_mask(
                ctx,
                dst,
                src_form_x_object,
                mode,
                blend_mode == BlendMode::DstOut,
            );
        }
    }

    /// `makeFormXObjectFromDevice(bbox, alpha)`: the content of the device as a form `XObject`; the
    /// device is empty afterwards.
    // Port of: src/pdf/SkPDFDevice.cpp#L1263-L1288 (chrome/m156)
    pub(crate) fn make_form_x_object_from_device_bounds(
        &mut self,
        bounds: &IRect,
        alpha: bool,
    ) -> PdfIndirectReference {
        let inverse_transform = if let Some(inv) = self.initial_transform.invert() {
            inv
        } else {
            debug_assert!(false, "Layer initial transform should be invertible.");
            Matrix::new_identity()
        };
        let color_space = if alpha { Some("DeviceGray") } else { None };

        let content = self.content();
        let struct_parents_key = self.mark_manager.struct_parents_key();
        let resource_dict = self.make_resource_dict();
        let xobject = make_form_x_object(
            &self.doc,
            &content,
            struct_parents_key,
            make_int_array(&[bounds.left, bounds.top, bounds.right, bounds.bottom]),
            resource_dict,
            &inverse_transform,
            color_space,
        );
        // We always draw the form xobjects that we create back into the device, so
        // we simply preserve the font usage instead of pulling it out and merging
        // it back in later.
        self.reset();
        xobject
    }

    /// `makeFormXObjectFromDevice(alpha)`: the whole device.
    // Port of: src/pdf/SkPDFDevice.cpp#L1290-L1292 (chrome/m156)
    pub(crate) fn make_form_x_object_from_device(&mut self, alpha: bool) -> PdfIndirectReference {
        let bounds = IRect::from_wh(self.size.width, self.size.height);
        self.make_form_x_object_from_device_bounds(&bounds, alpha)
    }

    /// `drawFormXObjectWithMask`.
    // Port of: src/pdf/SkPDFDevice.cpp#L1294-L1311 (chrome/m156)
    fn draw_form_x_object_with_mask(
        &mut self,
        ctx: &DeviceCtx,
        x_object: PdfIndirectReference,
        s_mask: PdfIndirectReference,
        mode: BlendMode,
        invert_clip: bool,
    ) {
        debug_assert!(s_mask.is_valid());
        let mut paint = Paint::default();
        paint.set_blend_mode(mode);
        let content = self.begin_entry(ctx, None, &Matrix::new_identity(), &paint, 0.0);
        if !content.is_active() {
            self.end_entry(ctx, None, content);
            return;
        }
        let gs = get_smask_graphic_state(s_mask, invert_clip, SMaskMode::Alpha, &self.doc);
        self.set_graphic_state(gs);
        self.draw_form_x_object(ctx, x_object, None);
        self.clear_mask_on_graphic_state();
        self.end_entry(ctx, None, content);
    }

    /// `drawFormXObject`.
    // Port of: src/pdf/SkPDFDevice.cpp#L1103-L1124 (chrome/m156)
    fn draw_form_x_object(
        &mut self,
        ctx: &DeviceCtx,
        x_object: PdfIndirectReference,
        shape: Option<&Path>,
    ) {
        self.begin_mark();
        if self.mark_manager.has_active_mark()
            && let Some(shape) = shape
        {
            // Destinations are in absolute coordinates.
            let page_xform = self.page_xform(ctx);
            // The shape already has localToDevice applied.

            let shape_bounds = shape.compute_tight_bounds();
            let shape_bounds = page_xform.map_rect(shape_bounds).0;
            self.mark_manager
                .accumulate(Point::new(shape_bounds.left, shape_bounds.bottom)); // y-up
        }

        debug_assert!(x_object.is_valid());
        let index = add_resource(&mut self.x_object_resources, x_object);
        let out = self.active_stream();
        write_resource_name(out, ResourceType::XObject, index);
        out.write_text(" Do\n");
    }

    ////////////////////////////////////////////////////////////////////////////

    /// `hasEmptyClip`.
    fn has_empty_clip(ctx: &DeviceCtx, clip_stack: &ClipStack) -> bool {
        clip_stack.is_empty(&ctx.bounds)
    }

    /// `drawPaint`.
    // Port of: src/pdf/SkPDFDevice.cpp#L444-L460 (chrome/m156)
    pub(crate) fn draw_paint(
        &mut self,
        ctx: &DeviceCtx,
        clip_stack: &ClipStack,
        src_paint: &Paint,
    ) {
        if Self::has_empty_clip(ctx, clip_stack) {
            return;
        }
        // Clip is in device space. Transform shader into device space.
        let bbox: Rect = clip_stack.bounds(&ctx.bounds).round_out();
        let mut new_paint = src_paint.clone();
        new_paint.set_style(Style::Fill);
        if let Some(shader) = new_paint.shader() {
            new_paint.set_shader(shader.with_local_matrix(&ctx.local_to_device));
        }
        self.internal_draw_path(
            ctx,
            clip_stack,
            &Matrix::new_identity(),
            &Path::rect(bbox, None),
            &new_paint,
        );
    }

    /// `drawPoints`, without the case that goes through `drawPath` (`paint` is cleaned, and has
    /// no path effect, and the matrix has no perspective).
    // Port of: src/pdf/SkPDFDevice.cpp#L462-L555 (chrome/m156)
    pub(crate) fn draw_points_direct(
        &mut self,
        ctx: &DeviceCtx,
        clip_stack: &ClipStack,
        mode: PointMode,
        points: &[Point],
        paint: &Paint,
    ) {
        let mut paint = paint.clone();
        if mode == PointMode::Points && paint.stroke_cap() != Cap::Round {
            if paint.stroke_width() != 0.0 {
                // PDF won't draw a single point with square/butt caps because the
                // orientation is ambiguous.  Draw a rectangle instead.
                set_style(&mut paint, Style::Fill);
                let stroke_width = paint.stroke_width();
                let half_stroke = stroke_width / 2.0;
                for pt in points {
                    let mut r = Rect::from_xywh(pt.x, pt.y, 0.0, 0.0);
                    r.inset((-half_stroke, -half_stroke));
                    self.draw_rect(ctx, clip_stack, &r, &paint);
                }
                return;
            }
            if paint.stroke_cap() != Cap::Round {
                paint.set_stroke_cap(Cap::Round);
            }
        }

        let entry = self.begin_entry(ctx, Some(clip_stack), &ctx.local_to_device, &paint, 0.0);
        if !entry.is_active() {
            return;
        }
        self.begin_mark();
        if self.mark_manager.has_active_mark() {
            // Destinations are in absolute coordinates.
            let mut page_xform = self.page_xform(ctx);
            // The points do not already have localToDevice applied.
            page_xform.pre_concat(&ctx.local_to_device);

            for user_point in points {
                self.mark_manager
                    .accumulate(page_xform.map_point(*user_point));
            }
        }
        let count = points.len();
        let content_stream = self.active_stream();
        match mode {
            PointMode::Polygon => {
                move_to(points[0].x, points[0].y, content_stream);
                for point in points.iter().take(count).skip(1) {
                    append_line(point.x, point.y, content_stream);
                }
                stroke_path(content_stream);
            }
            PointMode::Lines => {
                for i in 0..count / 2 {
                    move_to(points[i * 2].x, points[i * 2].y, content_stream);
                    append_line(points[i * 2 + 1].x, points[i * 2 + 1].y, content_stream);
                    stroke_path(content_stream);
                }
            }
            PointMode::Points => {
                debug_assert_eq!(paint.stroke_cap(), Cap::Round);
                for point in points.iter().take(count) {
                    move_to(point.x, point.y, content_stream);
                    close_path(content_stream);
                    stroke_path(content_stream);
                }
            }
        }
        self.end_entry(ctx, Some(clip_stack), entry);
    }

    /// `drawRect`.
    // Port of: src/pdf/SkPDFDevice.cpp#L557-L562 (chrome/m156)
    pub(crate) fn draw_rect(
        &mut self,
        ctx: &DeviceCtx,
        clip_stack: &ClipStack,
        rect: &Rect,
        paint: &Paint,
    ) {
        let r = rect.sorted();
        self.internal_draw_path(
            ctx,
            clip_stack,
            &ctx.local_to_device,
            &Path::rect(r, None),
            paint,
        );
    }

    /// `internalDrawPathWithFilter`.
    // Port of: src/pdf/SkPDFDevice.cpp#L578-L645 (chrome/m156)
    fn internal_draw_path_with_filter(
        &mut self,
        ctx: &DeviceCtx,
        clip_stack: &ClipStack,
        ctm: &Matrix,
        orig_path: &Path,
        orig_paint: &Paint,
    ) {
        let Some(mask_filter) = orig_paint.mask_filter() else {
            debug_assert!(false);
            return;
        };
        let mut builder = PathBuilder::new();
        let mut paint = orig_paint.clone();

        let init_style = if skia_rust_core::path_utils::fill_path_with_paint(
            orig_path,
            &paint,
            &mut builder,
            None,
            None,
        ) {
            InitStyle::Fill
        } else {
            InitStyle::Hairline
        };
        builder.transform(ctm);
        let Some(path_raw) = raw_builder(&builder, ResolveConvexity::Yes) else {
            return;
        };

        let bounds: IRect = clip_stack.bounds(&ctx.bounds).round_out();
        let mut source_mask = MaskBuilder::default();
        if !draw_to_mask(
            &path_raw,
            &bounds,
            &mask_filter,
            &Matrix::new_identity(),
            &mut source_mask,
            CreateMode::ComputeBoundsAndRenderImage,
            init_style,
        ) {
            return;
        }
        let mut dst_mask = MaskBuilder::default();
        let mut margin = IPoint::new(0, 0);
        if !mask_filter.as_base().filter_mask(
            &mut dst_mask,
            &source_mask.as_mask(),
            ctm,
            Some(&mut margin),
        ) {
            return;
        }
        let dst_mask_bounds = dst_mask.bounds;
        let Some(mask) = mask_to_greyscale_image(dst_mask, &self.doc) else {
            return;
        };
        // PDF doesn't seem to allow masking vector graphics with an Image XObject.
        // Must mask with a Form XObject.
        let mask_device = self.make_congruent_device();
        let mask_content = mask_device.content_handle();
        {
            let canvas = Canvas::from_device(Box::new(mask_device));
            canvas.draw_image(
                &mask,
                (dst_mask_bounds.x() as f32, dst_mask_bounds.y() as f32),
                None,
            );
        }
        if !ctm.is_identity() && paint.shader().is_some() {
            transform_shader(&mut paint, ctm); // Since we are using identity matrix.
        }
        let content = self.begin_entry(ctx, Some(clip_stack), &Matrix::new_identity(), &paint, 0.0);
        if !content.is_active() {
            return;
        }
        let s_mask = mask_content
            .borrow_mut()
            .make_form_x_object_from_device_bounds(&dst_mask_bounds, true);
        let gs = get_smask_graphic_state(s_mask, false, SMaskMode::Luminosity, &self.doc);
        self.set_graphic_state(gs);
        let out = self.active_stream();
        append_rectangle(&Rect::from_irect(dst_mask_bounds), out);
        paint_path(Style::Fill, path_raw.fill_type(), out);
        self.clear_mask_on_graphic_state();
        self.end_entry(ctx, Some(clip_stack), content);
    }

    /// `internalDrawPath`.
    // Port of: src/pdf/SkPDFDevice.cpp#L647-L742 (chrome/m156)
    pub(crate) fn internal_draw_path(
        &mut self,
        ctx: &DeviceCtx,
        clip_stack: &ClipStack,
        ctm: &Matrix,
        orig_path: &Path,
        src_paint: &Paint,
    ) {
        if clip_stack.is_empty(&ctx.bounds) {
            return;
        }
        let mut paint = clean_paint(src_paint);
        let mut modified_path = orig_path.clone();

        if paint.mask_filter().is_some() {
            self.internal_draw_path_with_filter(ctx, clip_stack, ctm, orig_path, &paint);
            return;
        }

        let mut matrix = ctm.clone();

        if paint.path_effect().is_some() {
            if clip_stack.is_empty(&ctx.bounds) {
                return;
            }
            let (path, is_fill) =
                skia_rust_core::path_utils::fill_path_with_paint_to_path(&modified_path, &paint);
            if is_fill {
                set_style(&mut paint, Style::Fill);
            } else {
                set_style(&mut paint, Style::Stroke);
                if paint.stroke_width() != 0.0 {
                    paint.set_stroke_width(0.0);
                }
            }
            modified_path = path;
            paint.set_path_effect(None);
        }

        if self.handle_inverse_path(ctx, clip_stack, &modified_path, &paint) {
            return;
        }
        if matrix.get_type().contains(TypeMask::PERSPECTIVE) {
            modified_path = modified_path.make_transform(&matrix);
            if paint.shader().is_some() {
                transform_shader(&mut paint, &matrix);
            }
            matrix = Matrix::new_identity();
        }

        let content = self.begin_entry(ctx, Some(clip_stack), &matrix, &paint, 0.0);
        if !content.is_active() {
            return;
        }
        self.begin_mark();
        if self.mark_manager.has_active_mark() {
            // Destinations are in absolute coordinates.
            let mut page_xform = self.page_xform(ctx);
            // The path does not already have localToDevice / ctm / matrix applied.
            page_xform.pre_concat(&matrix);

            let path_bounds = modified_path.compute_tight_bounds();
            let path_bounds = page_xform.map_rect(path_bounds).0;
            self.mark_manager
                .accumulate(Point::new(path_bounds.left, path_bounds.bottom)); // y-up
        }
        #[allow(clippy::items_after_statements)] // mirrors the C++ function-local constant
        const TOLERANCE_SCALE: f32 = 0.0625; // smaller = better conics (circles).
        let matrix_scale = matrix.map_radius(1.0);
        let tolerance = if matrix_scale > 0.0 {
            TOLERANCE_SCALE / matrix_scale
        } else {
            TOLERANCE_SCALE
        };
        let discard_empty_verbs = paint.style() == Style::Fill
            || (paint.stroke_cap() != Cap::Round && paint.stroke_cap() != Cap::Square);
        let discard_empty_area = paint.style() == Style::Fill;
        let out = self.active_stream();
        if emit_path(
            &modified_path,
            EmptyPath::Discard,
            if discard_empty_verbs {
                EmptyVerb::Discard
            } else {
                EmptyVerb::Preserve
            },
            if discard_empty_area {
                EmptyArea::Discard
            } else {
                EmptyArea::Preserve
            },
            out,
            tolerance,
        ) {
            paint_path(paint.style(), modified_path.fill_type(), out);
        }
        self.end_entry(ctx, Some(clip_stack), content);
    }

    /// Draws an inverse filled path by using Path Ops to compute the positive inverse using the
    /// current clip as the inverse bounds. Returns true if this was an inverse path and was
    /// properly handled, otherwise returns false and the normal drawing routine should continue,
    /// either as a (incorrect) fallback or because the path was not inverse in the first place.
    // Port of: src/pdf/SkPDFDevice.cpp#L1181-L1261 (handleInversePath, chrome/m156)
    fn handle_inverse_path(
        &mut self,
        ctx: &DeviceCtx,
        clip_stack: &ClipStack,
        orig_path: &Path,
        src_paint: &Paint,
    ) -> bool {
        // Assume the caller has already applied the path effect.
        debug_assert!(src_paint.path_effect().is_none());

        if !orig_path.is_inverse_fill_type() {
            return false;
        }

        if Self::has_empty_clip(ctx, clip_stack) {
            return false;
        }

        let mut paint = src_paint.clone();
        let mut modified_path = orig_path.clone();

        // Merge stroking operations into final path.
        if Style::Stroke == paint.style() || Style::StrokeAndFill == paint.style() {
            let (path, do_fill_path) =
                skia_rust_core::path_utils::fill_path_with_paint_to_path(orig_path, &paint);
            modified_path = path;

            if do_fill_path {
                paint.set_style(Style::Fill);
                paint.set_stroke_width(0.0);
            } else {
                // Hairline strokes are rendered non-inverted.
                modified_path = modified_path.with_toggle_inverse_fill_type();
                self.internal_draw_path(
                    ctx,
                    clip_stack,
                    &ctx.local_to_device,
                    &modified_path,
                    &paint,
                );
                return true;
            }
        }

        // Clip is in device space. Transform path and shader into device space.
        let bounds = clip_stack.bounds(&ctx.bounds);
        modified_path = modified_path.make_transform(&ctx.local_to_device);
        let Some(inverse_path) = calculate_inverse_path(&bounds, &modified_path) else {
            return false;
        };
        modified_path = inverse_path;
        if let Some(shader) = paint.shader() {
            paint.set_shader(shader.with_local_matrix(&ctx.local_to_device));
        }
        self.internal_draw_path(
            ctx,
            clip_stack,
            &Matrix::new_identity(),
            &modified_path,
            &paint,
        );
        true
    }

    /// `internalDrawImageRect`.
    // Port of: src/pdf/SkPDFDevice.cpp#L1325-L1633 (chrome/m156)
    #[allow(clippy::too_many_lines, clippy::too_many_arguments)] // one function in Skia
    pub(crate) fn internal_draw_image_rect(
        &mut self,
        ctx: &DeviceCtx,
        clip_stack: &mut ClipStack,
        mut image_subset: KeyedImage,
        src: Option<&Rect>,
        dst: &Rect,
        sampling: &SamplingOptions,
        src_paint: &Paint,
        ctm: &Matrix,
    ) {
        if Self::has_empty_clip(ctx, clip_stack) {
            return;
        }
        let Some(subset_image) = image_subset.image() else {
            return;
        };

        let original_image = image_subset.clone();
        let mut can_use_original = true;
        let mut did_subset = false;

        // First, figure out the src->dst transform and subset the image if needed.
        let mut bounds = subset_image.bounds();
        let mut src_rect = src.copied().unwrap_or_else(|| Rect::from_irect(bounds));
        let mut transform = Matrix::rect_to_rect_or_identity(src_rect, dst, None);
        let mut original_transform = transform.clone();
        if let Some(src) = src
            && *src != Rect::from_irect(bounds)
        {
            if !src_rect.intersect(Rect::from_irect(bounds)) {
                return;
            }
            bounds = src_rect.round_out();
            transform.pre_translate((bounds.x() as f32, bounds.y() as f32));
            if bounds != image_subset.image().expect("an image").bounds() {
                image_subset = image_subset.subset(&bounds);
                did_subset = true;
            }
            if !image_subset.is_valid() {
                return;
            }
        }

        // If the image is opaque and the paint's alpha is too, replace
        // kSrc blendmode with kSrcOver.  http://crbug.com/473572
        let mut paint = src_paint.clone();
        if !paint.is_src_over()
            && image_subset.image().expect("an image").is_opaque()
            && BlendFastPath::SrcOver == check_fast_path(&paint, false)
        {
            paint.set_blend_mode(BlendMode::SrcOver);
        }

        // Alpha-only images need to get their color from the shader, before
        // applying the colorfilter.
        if image_subset.image().expect("an image").is_alpha_only() && paint.color_filter().is_some()
        {
            // must blend alpha image and shader before applying colorfilter.
            let image = image_subset.image().expect("an image").clone();
            let Some(mut surface) = raster_n32_premul(image.dimensions()) else {
                return;
            };
            {
                let canvas = surface.canvas();
                let mut tmp_paint = Paint::default();
                // In the case of alpha images with shaders, the shader's coordinate
                // system is the image's coordiantes.
                tmp_paint.set_shader(paint.shader());
                tmp_paint.set_color4f(paint.color4f(), None);
                canvas.clear(Color4f::new(0.0, 0.0, 0.0, 0.0));
                canvas.draw_image_with_sampling_options(
                    &image,
                    (0.0, 0.0),
                    *sampling,
                    Some(&tmp_paint),
                );
            }
            if paint.shader().is_some() {
                paint.set_shader(None);
            }
            image_subset = KeyedImage::new(surface.image_snapshot());
            can_use_original = false;
            debug_assert!(
                !image_subset
                    .image()
                    .is_some_and(skia_rust_core::image::Image::is_alpha_only)
            );
        }

        if image_subset.image().is_some_and(Image::is_alpha_only) {
            // The ColorFilter applies to the paint color/shader, not the alpha layer.
            debug_assert!(paint.color_filter().is_none());

            let Some(mask) =
                alpha_image_to_greyscale_image(image_subset.image().expect("an image"))
            else {
                return;
            };
            // PDF doesn't seem to allow masking vector graphics with an Image XObject.
            // Must mask with a Form XObject.
            let mask_device = self.make_congruent_device();
            let mask_content = mask_device.content_handle();
            let mask_device_bounds;
            {
                let canvas = Canvas::from_device(Box::new(mask_device));
                // This clip prevents the mask image shader from covering
                // entire device if unnecessary.
                canvas.clip_rect(clip_stack.bounds(&ctx.bounds), None, None);
                canvas.concat(ctm);
                if let Some(mask_filter) = paint.mask_filter() {
                    let mut tmp_paint = Paint::default();
                    tmp_paint.set_shader(mask.to_shader(
                        None,
                        SamplingOptions::default(),
                        Some(&transform),
                    ));
                    tmp_paint.set_mask_filter(mask_filter);
                    canvas.draw_rect(dst, &tmp_paint);
                } else {
                    if src.is_some_and(|s| !is_integral(s)) {
                        canvas.clip_rect(dst, None, None);
                    }
                    canvas.concat(&transform);
                    canvas.draw_image(&mask, (0.0, 0.0), None);
                }
                mask_device_bounds = canvas.with_top_device(|d| d.dev_clip_bounds());
            }
            if !ctm.is_identity() && paint.shader().is_some() {
                transform_shader(&mut paint, ctm); // Since we are using identity matrix.
            }
            let content =
                self.begin_entry(ctx, Some(clip_stack), &Matrix::new_identity(), &paint, 0.0);
            if !content.is_active() {
                return;
            }
            let s_mask = mask_content
                .borrow_mut()
                .make_form_x_object_from_device_bounds(&mask_device_bounds, true);
            let gs = get_smask_graphic_state(s_mask, false, SMaskMode::Luminosity, &self.doc);
            self.set_graphic_state(gs);
            let whole = Rect::from_wh(self.size.width as f32, self.size.height as f32);
            let out = self.active_stream();
            append_rectangle(&whole, out);
            paint_path(Style::Fill, PathFillType::Winding, out);
            self.clear_mask_on_graphic_state();
            self.end_entry(ctx, Some(clip_stack), content);
            return;
        }
        if paint.mask_filter().is_some() {
            paint.set_shader(image_subset.image().expect("an image").to_shader(
                None,
                SamplingOptions::default(),
                Some(&transform),
            ));
            let path = Path::rect(dst, None); // handles non-integral clipping.
            self.internal_draw_path(ctx, clip_stack, &ctx.local_to_device, &path, &paint);
            return;
        }
        transform.post_concat(ctm);
        original_transform.post_concat(ctm);

        let mut matrix = transform.clone();

        // Rasterize the bitmap using perspective in a new bitmap.
        if transform.has_perspective() {
            // Transform the bitmap in the new space, without taking into
            // account the initial transform.
            let image = image_subset.image().expect("an image").clone();
            let image_bounds = Rect::from_irect(image.bounds());
            let perspective_outline = Path::rect(image_bounds, None).make_transform(&transform);

            // Retrieve the bounds of the new shape.
            let mut outline_bounds = *perspective_outline.bounds();
            let dev_clip_bounds: IRect = clip_stack.bounds(&ctx.bounds).round_out();
            if !outline_bounds.intersect(Rect::from_irect(dev_clip_bounds)) {
                return;
            }

            // Transform the bitmap in the new space to the final space, to account for DPI
            let physical_bounds = self.initial_transform.map_rect(outline_bounds).0;
            let scale_x = physical_bounds.width() / outline_bounds.width();
            let scale_y = physical_bounds.height() / outline_bounds.height();

            // TODO(edisonn): A better approach would be to use a bitmap shader
            // (in clamp mode) and draw a rect over the entire bounding box. Then
            // intersect perspectiveOutline to the clip. That will avoid introducing
            // alpha to the image while still giving good behavior at the edge of
            // the image.  Avoiding alpha will reduce the pdf size and generation
            // CPU time some.

            let wh = Size::new(physical_bounds.width(), physical_bounds.height()).to_ceil();

            let Some(mut surface) = raster_n32_premul(wh) else {
                return;
            };
            let delta_x = outline_bounds.left;
            let delta_y = outline_bounds.top;
            {
                let canvas = surface.canvas();
                canvas.clear(Color4f::new(0.0, 0.0, 0.0, 0.0));

                let mut offset_matrix = transform;
                offset_matrix.post_translate((-delta_x, -delta_y));
                offset_matrix.post_scale((scale_x, scale_y), None);

                // Translate the draw in the new canvas, so we perfectly fit the
                // shape in the bitmap.
                canvas.set_matrix(&skia_rust_core::m44::M44::from(&offset_matrix));
                canvas.draw_image(&image, (0.0, 0.0), None);
            }

            // In the new space, we use the identity matrix translated
            // and scaled to reflect DPI.
            matrix.set_scale((1.0 / scale_x, 1.0 / scale_y), None);
            matrix.post_translate((delta_x, delta_y));

            image_subset = KeyedImage::new(surface.image_snapshot());
            can_use_original = false;
            if !image_subset.is_valid() {
                return;
            }
        }

        if let Some(color_filter) = paint.color_filter() {
            // `color_filter(image, colorFilter)`
            let image = image_subset.image().expect("an image").clone();
            let Some(mut surface) = raster_n32_premul(image.dimensions()) else {
                debug_assert!(false);
                return;
            };
            {
                let canvas = surface.canvas();
                canvas.clear(Color4f::new(0.0, 0.0, 0.0, 0.0));
                let mut cf_paint = Paint::default();
                cf_paint.set_color_filter(color_filter);
                canvas.draw_image_with_sampling_options(
                    &image,
                    (0.0, 0.0),
                    SamplingOptions::default(),
                    Some(&cf_paint),
                );
            }
            image_subset = KeyedImage::new(surface.image_snapshot());
            can_use_original = false;
            if !image_subset.is_valid() {
                return;
            }
            // TODO(halcanary): de-dupe this by caching filtered images.
            // (maybe in the resource cache?)
        }

        let mut use_cropped_original = false;
        if did_subset && can_use_original {
            let encoding_quality = self.doc.metadata().encoding_quality;
            let original_size = serialize_image_size(
                original_image.image().expect("an image"),
                &self.doc,
                encoding_quality,
            );
            let subset_size = serialize_image_size(
                image_subset.image().expect("an image"),
                &self.doc,
                encoding_quality,
            );
            if original_size <= subset_size {
                matrix = original_transform;
                image_subset = original_image;
                use_cropped_original = true;
            }
        }

        // Need sub-pixel clipping to fix skbug.com/40035524
        let saved = (src.is_some_and(|s| !is_integral(s))) || use_cropped_original;
        if saved {
            clip_stack.save();
            clip_stack.clip_rect(dst, ctm, ClipOp::Intersect, true);
        }

        // Adjust for origin flip.
        let mut scaled = Matrix::new_identity();
        scaled.set_scale((1.0, -1.0), None);
        scaled.post_translate((0.0, 1.0));
        // Scale the image up from 1x1 to WxH.
        let subset = image_subset.image().expect("an image").bounds();
        scaled.post_scale((subset.width() as f32, subset.height() as f32), None);
        scaled.post_concat(&matrix);
        let mut content = self.begin_entry(ctx, Some(clip_stack), &scaled, &paint, 0.0);
        if content.is_active() {
            let shape = Path::rect(Rect::from_irect(subset), None).make_transform(&matrix);
            if content.need_shape() {
                content.set_shape(shape.clone());
            }
            if content.need_source() {
                let key = *image_subset.key();
                let mut pdfimage = self.doc.with(|d| d.pdf_bitmap_map.get(&key).copied());
                if pdfimage.is_none() {
                    let encoding_quality = self.doc.metadata().encoding_quality;
                    let made = serialize_image_xobject(
                        image_subset.image().expect("an image"),
                        &self.doc,
                        encoding_quality,
                    );
                    debug_assert!(key != crate::keyed_image::BitmapKey::EMPTY);
                    self.doc.with(|d| d.pdf_bitmap_map.insert(key, made));
                    pdfimage = Some(made);
                }
                let pdfimage = pdfimage.expect("an image");
                debug_assert_ne!(pdfimage, PdfIndirectReference::default());
                self.draw_form_x_object(ctx, pdfimage, Some(&shape));
            }
        }
        self.end_entry(ctx, Some(clip_stack), content);
        if saved {
            clip_stack.restore();
        }
    }
}

/// `SkPDFDevice`: the drawing context for a page or layer of PDF content.
// Port of: src/pdf/SkPDFDevice.h#L58-L195 (chrome/m156)
#[doc(alias = "SkPDFDevice")]
#[derive(Debug)]
pub struct PdfDevice {
    state: DeviceState,
    clip: ClipStackDevice,
    content: ContentHandle,
    doc: DocHandle,
}

impl PdfDevice {
    /// `SkPDFDevice(pageSize, document, initialTransform)`. `page_size` is in point units (1 point
    /// == 127/360 mm == 1/72 inch); the document is responsible for de-duplicating across pages
    /// and for early serializing of large immutable objects, such as images; `initial_transform`
    /// is the transform to be applied to the entire page.
    // Port of: src/pdf/SkPDFDevice.cpp#L408-L418 (chrome/m156)
    #[must_use]
    pub(crate) fn new(page_size: ISize, doc: &DocHandle, initial_transform: &Matrix) -> Self {
        debug_assert!(!page_size.is_empty());
        Self {
            state: DeviceState::new(
                ImageInfo::new_unknown(Some(page_size)),
                SurfaceProps::new(
                    SurfacePropsFlags::PRESERVES_TRANSPARENT_DRAWS,
                    PixelGeometry::Unknown,
                ),
            ),
            clip: ClipStackDevice::new(),
            content: Rc::new(RefCell::new(Content::new(
                page_size,
                doc,
                initial_transform,
            ))),
            doc: doc.clone(),
        }
    }

    /// The content of the device, shared with whoever draws the device into a canvas.
    pub(crate) fn content_handle(&self) -> ContentHandle {
        Rc::clone(&self.content)
    }

    fn ctx(&self) -> DeviceCtx {
        DeviceCtx {
            bounds: self.state.bounds(),
            device_to_global: self.state.device_to_global().to_m33(),
            local_to_device: self.state.local_to_device().clone(),
        }
    }

    /// `hasEmptyClip`.
    fn has_empty_clip(&self) -> bool {
        self.clip.cs().is_empty(&self.state.bounds())
    }
}

impl Device for PdfDevice {
    fn state(&self) -> &DeviceState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut DeviceState {
        &mut self.state
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn dev_clip_bounds(&self) -> IRect {
        self.clip.dev_clip_bounds(&self.state)
    }

    fn push_clip_stack(&mut self) {
        self.clip.push_clip_stack();
    }

    fn pop_clip_stack(&mut self) {
        self.clip.pop_clip_stack();
    }

    fn clip_rect(&mut self, rect: &Rect, op: ClipOp, aa: bool) {
        self.clip.clip_rect(&self.state, rect, op, aa);
    }

    fn clip_rrect(&mut self, rrect: &RRect, op: ClipOp, aa: bool) {
        self.clip.clip_rrect(&self.state, rrect, op, aa);
    }

    fn clip_path(&mut self, path: &Path, op: ClipOp, aa: bool) {
        self.clip.clip_path(&self.state, path, op, aa);
    }

    fn clip_region(&mut self, region: &Region, op: ClipOp) {
        self.clip.clip_region(&self.state, region, op);
    }

    fn on_clip_shader(&mut self, shader: Shader) {
        self.clip.on_clip_shader(shader);
    }

    fn replace_clip(&mut self, rect: &IRect) {
        self.clip.replace_clip(&self.state, rect);
    }

    fn is_clip_anti_aliased(&self) -> bool {
        self.clip.is_clip_anti_aliased()
    }

    fn is_clip_empty(&self) -> bool {
        self.clip.is_clip_empty(&self.state)
    }

    fn is_clip_rect(&self) -> bool {
        self.clip.is_clip_rect(&self.state)
    }

    fn is_clip_wide_open(&self) -> bool {
        self.clip.is_clip_wide_open(&self.state)
    }

    fn android_utils_clip_as_rgn(&self, rgn: &mut Region) {
        self.clip.android_utils_clip_as_rgn(&self.state, rgn);
    }

    /// `createDevice`: PDF does not support image filters or color filters, nor color spaces
    /// other than sRGB, so the layers that use them are raster devices that are drawn into the
    /// page as images.
    // Port of: src/pdf/SkPDFDevice.cpp#L314-L331 (chrome/m156)
    fn create_device(
        &mut self,
        cinfo: &CreateInfo,
        layer_paint: Option<&Paint>,
    ) -> Option<Box<dyn Device>> {
        // PDF does not support image filters, so render them on CPU.
        // Note that this rendering is done at "screen" resolution (100dpi), not
        // printer resolution.

        // TODO: It may be possible to express some filters natively using PDF
        // to improve quality and file size (skbug.com/40034150)
        if layer_paint.is_some_and(|p| p.image_filter().is_some() || p.color_filter().is_some())
            || cinfo
                .info
                .color_space()
                .is_some_and(|cs| !ColorSpace::is_srgb(&cs))
        {
            // need to return a raster device, which we will detect in drawDevice()
            return BitmapDevice::create(&cinfo.info, SurfaceProps::default())
                .map(|d| Box::new(d) as Box<dyn Device>);
        }
        Some(Box::new(PdfDevice::new(
            cinfo.info.dimensions(),
            &self.doc,
            &Matrix::new_identity(),
        )))
    }

    /// `createImageFilteringBackend`: `SkDevice`'s default, the raster backend (the PDF device does
    /// not override it, so image filters run on the CPU, see `create_device`).
    // Port of: src/core/SkDevice.cpp#L322-L325 (chrome/m156)
    fn create_image_filtering_backend(
        &self,
        surface_props: &SurfaceProps,
        color_type: ColorType,
    ) -> Option<std::sync::Arc<dyn skia_rust_core::image_filter_types::Backend>> {
        Some(skia_rust_raster::image_filter_backend::make_raster_backend(
            *surface_props,
            color_type,
        ))
    }

    /// `drawAnnotation`.
    // Port of: src/pdf/SkPDFDevice.cpp#L430-L492 (chrome/m156)
    fn draw_annotation(&mut self, rect: &Rect, key: &str, value: Option<&Data>) {
        let Some(value) = value else {
            return;
        };
        if !self.doc.has_current_page() {
            return;
        }
        // Annotations are specified in absolute coordinates, so the page xform maps from device
        // space to the global space, and applies the document transform.
        let mut page_xform = self.state.device_to_global().to_m33();
        page_xform.post_concat(&self.doc.with(|d| d.current_page_transform()));
        if rect.is_empty() {
            if key == elem_id_key() {
                let bytes = value.as_bytes();
                let Ok(elem_id) = <[u8; 4]>::try_from(bytes) else {
                    return;
                };
                let elem_id = i32::from_ne_bytes(elem_id);
                self.content
                    .borrow_mut()
                    .mark_manager
                    .set_next_marks_elem_id(elem_id);
                return;
            }
            if AnnotationKeys::define_named_dest_key() == key {
                let mut p = self
                    .state
                    .local_to_device()
                    .map_point(Point::new(rect.x(), rect.y()));
                p = page_xform.map_point(p);
                let pg = self.doc.with(|d| d.current_page());
                self.doc.with(|d| {
                    d.named_destinations.push(PdfNamedDestination {
                        name: value.clone(),
                        point: p,
                        page: pg,
                    });
                });
            }
            return;
        }
        // Convert to path to handle non-90-degree rotations.
        let mut path = Path::rect(rect, None).make_transform(self.state.local_to_device());
        let clip = clip_stack_as_path(self.clip.cs());
        if let Some(result) = skia_rust_pathops::op(&clip, &path, PathOp::Intersect) {
            path = result;
        }
        // PDF wants a rectangle only.
        let transformed_rect = page_xform.map_rect(*path.bounds()).0;
        if transformed_rect.is_empty() {
            return;
        }

        let mut link_type = LinkType::None;
        if AnnotationKeys::url_key() == key {
            link_type = LinkType::Url;
        } else if AnnotationKeys::link_named_dest_key() == key {
            link_type = LinkType::NamedDestination;
        }

        if link_type != LinkType::None {
            let elem_id = self.content.borrow().mark_manager.elem_id();
            let link = PdfLink {
                link_type,
                data: value.clone(),
                rect: transformed_rect,
                elem_id,
            };
            self.doc.with(|d| d.current_page_links.push(link));
        }
    }

    fn draw_paint(&mut self, paint: &Paint) {
        let ctx = self.ctx();
        self.content
            .borrow_mut()
            .draw_paint(&ctx, self.clip.cs(), paint);
    }

    /// `drawPoints`.
    // Port of: src/pdf/SkPDFDevice.cpp#L462-L555 (chrome/m156)
    fn draw_points(&mut self, mode: PointMode, points: &[Point], src_paint: &Paint) {
        if self.has_empty_clip() {
            return;
        }
        if points.is_empty() {
            return;
        }
        let mut paint = clean_paint(src_paint);

        if PointMode::Points != mode {
            set_style(&mut paint, Style::Stroke);
        }

        // skcpu::Draw::drawPoints converts to multiple calls to fDevice->drawPath.
        // We only use this when there's a path effect or perspective because of the overhead
        // of multiple calls to setUpContentEntry it causes.
        if paint.path_effect().is_some() || self.state.local_to_device().has_perspective() {
            // `draw_points(mode, points, *paint, this->devClipBounds(), this)`
            let bounds = self.dev_clip_bounds();
            let rc = RasterClip::from_rect(&bounds);
            let ctm = self.state.local_to_device().clone();
            let mut draw = Draw::new(Pixmap::default(), &ctm, &rc);
            draw.draw_points(mode, points, &paint, Some(self));
            return;
        }

        let ctx = self.ctx();
        self.content
            .borrow_mut()
            .draw_points_direct(&ctx, self.clip.cs(), mode, points, &paint);
    }

    fn draw_rect(&mut self, rect: &Rect, paint: &Paint) {
        let ctx = self.ctx();
        self.content
            .borrow_mut()
            .draw_rect(&ctx, self.clip.cs(), rect, paint);
    }

    fn draw_rrect(&mut self, rrect: &RRect, paint: &Paint) {
        let ctx = self.ctx();
        self.content.borrow_mut().internal_draw_path(
            &ctx,
            self.clip.cs(),
            &ctx.local_to_device,
            &Path::rrect(rrect, None),
            paint,
        );
    }

    fn draw_oval(&mut self, oval: &Rect, paint: &Paint) {
        let ctx = self.ctx();
        self.content.borrow_mut().internal_draw_path(
            &ctx,
            self.clip.cs(),
            &ctx.local_to_device,
            &Path::oval(oval, None),
            paint,
        );
    }

    fn draw_path(&mut self, path: &Path, paint: &Paint) {
        let ctx = self.ctx();
        self.content.borrow_mut().internal_draw_path(
            &ctx,
            self.clip.cs(),
            &ctx.local_to_device,
            path,
            paint,
        );
    }

    /// `drawImageRect`.
    // Port of: src/pdf/SkPDFDevice.cpp#L1318-L1331 (chrome/m156)
    fn draw_image_rect(
        &mut self,
        image: &Image,
        src: Option<&Rect>,
        dst: &Rect,
        sampling: &SamplingOptions,
        paint: &Paint,
        _constraint: SrcRectConstraint,
    ) {
        let ctx = self.ctx();
        self.content.borrow_mut().internal_draw_image_rect(
            &ctx,
            self.clip.cs_mut(),
            KeyedImage::new(Some(image.clone())),
            src,
            dst,
            sampling,
            paint,
            &ctx.local_to_device,
        );
    }

    /// `drawVertices`: not implemented in Skia either.
    // Port of: src/pdf/SkPDFDevice.cpp#L1077-L1083 (chrome/m156)
    fn draw_vertices(&mut self, _: &Vertices, _: Blender, _: &Paint, _: bool) {
        if self.has_empty_clip() {}
        // TODO: implement drawVertices
    }

    /// `drawMesh`: not implemented in Skia either.
    // Port of: src/pdf/SkPDFDevice.cpp#L1085-L1090 (chrome/m156)
    fn draw_mesh(&mut self, _: &Mesh, _: Blender, _: &Paint) {
        if self.has_empty_clip() {}
        // TODO: implement drawMesh
    }

    /// `onDrawGlyphRunList`.
    ///
    /// TODO(M26): the glyph runs are drawn with `SkPDFFont`s (`internalDrawGlyphRun` of
    /// `SkPDFDevice.cpp#L849-L1075`, `drawGlyphRunAsPath`, `GlyphPositioner`). Until the fonts are
    /// ported, a glyph run draws nothing.
    // Port of: src/pdf/SkPDFDevice.cpp#L1068-L1075 (chrome/m156)
    fn on_draw_glyph_run_list(&mut self, list: &GlyphRunList<'_>, _paint: &Paint) {
        debug_assert!(!list.has_rsxform());
        // The clusters are walked as Skia does, so that the title of a structure element that is
        // drawn with text can be recorded; the fonts are the missing part.
        let ctx = self.ctx();
        let _ = ctx;
        for glyph_run in list.runs() {
            let _clusterator = Clusterator::new(glyph_run);
        }
    }

    /// `drawDevice`.
    // Port of: src/pdf/SkPDFDevice.cpp#L1635-L1685 (chrome/m156)
    fn draw_device(&mut self, device: &mut dyn Device, sampling: &SamplingOptions, paint: &Paint) {
        debug_assert!(paint.image_filter().is_none());
        debug_assert!(paint.mask_filter().is_none());

        // Check if SkPDFDevice::createDevice returned an SkBitmapDevice.
        // SkPDFDevice::createDevice creates SkBitmapDevice for color filters.
        // Image filters generally go through makeSpecial and drawSpecial.
        if device.peek_pixels().is_some() {
            draw_device_default(self, device, sampling, paint);
            return;
        }

        // Otherwise SkPDFDevice::createDevice() creates SkPDFDevice subclasses.
        let Some(pdf_device) = device
            .as_any()
            .and_then(|any| any.downcast_ref::<PdfDevice>())
        else {
            debug_assert!(false);
            return;
        };

        if pdf_device.content.borrow().is_content_empty() {
            return;
        }

        let matrix = device.state().relative_transform(&self.state).to_m33();
        let ctx = self.ctx();
        let mut content = self.content.borrow_mut();
        let entry = content.begin_entry(&ctx, Some(self.clip.cs()), &matrix, paint, 0.0);
        if !entry.is_active() {
            return;
        }
        let mut entry = entry;
        let dims = device.state().image_info().dimensions();
        let shape = Path::rect(Rect::from_wh(dims.width as f32, dims.height as f32), None)
            .make_transform(&matrix);
        if entry.need_shape() {
            entry.set_shape(shape.clone());
        }
        if !entry.need_source() {
            content.end_entry(&ctx, Some(self.clip.cs()), entry);
            return;
        }
        // This XObject may contain its own marks, which are hidden if emitted inside an outer
        // mark. If it does have its own marks any current mark is paused and then re-set after.
        // If it does not have its own marks it will be part of the content of the current mark.
        let current_struct_elem_id = content.mark_manager.elem_id();
        let other_content = pdf_device.content_handle();
        if other_content
            .borrow()
            .mark_manager
            .struct_parents_key()
            .is_valid()
        {
            content.mark_manager.set_next_marks_elem_id(0);
            content.begin_mark();
        }
        let x_object = other_content
            .borrow_mut()
            .make_form_x_object_from_device(false);
        content.draw_form_x_object(&ctx, x_object, Some(&shape));
        content
            .mark_manager
            .set_next_marks_elem_id(current_struct_elem_id);
        content.end_entry(&ctx, Some(self.clip.cs()), entry);
    }

    /// `drawSpecial`.
    // Port of: src/pdf/SkPDFDevice.cpp#L1687-L1702 (chrome/m156)
    fn draw_special(
        &mut self,
        src_img: &SpecialImage,
        local_to_device: &Matrix,
        sampling: &SamplingOptions,
        paint: &Paint,
        _constraint: SrcRectConstraint,
    ) {
        if self.has_empty_clip() {
            return;
        }
        debug_assert!(!src_img.is_ganesh_backed());
        debug_assert!(paint.mask_filter().is_none() && paint.image_filter().is_none());

        if let Some(result_bm) = src_img.as_bitmap() {
            let r = Rect::from_wh(result_bm.width() as f32, result_bm.height() as f32);
            let ctx = self.ctx();
            self.content.borrow_mut().internal_draw_image_rect(
                &ctx,
                self.clip.cs_mut(),
                KeyedImage::from_bitmap(&result_bm),
                None,
                &r,
                sampling,
                paint,
                local_to_device,
            );
        }
    }
}
