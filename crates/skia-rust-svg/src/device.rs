// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/svg/SkSVGDevice.h, src/svg/SkSVGDevice.cpp

//! `SkSVGDevice`: a device that writes its draw calls as SVG elements to an `SkXMLWriter`.
//!
//! skia-rust: Skia's `SkSVGDevice::AutoElement` is an RAII guard over the writer, and it is
//! kept as one (`AutoElement` here ends its element when dropped). The writer, the resource
//! counters and the PNG encoder are shared by the guards through an `Rc` (`Ctx`), because a Rust
//! guard cannot point back into the device that makes it. The device is split into the part
//! that Skia's `SkClipStackDevice` base class has (`clip`, `state`) and the rest (`Inner`), so
//! that a guard can be made while the clip stack is borrowed.
//!
//! Numbers are formatted as the C library does: `%g` (six significant digits) for transforms
//! and `%.8g` for positions and for `SkXMLWriter::addScalarAttribute`. Both go through
//! [`format_g`], which is correctly rounded as glibc and the UCRT are.

#![allow(clippy::cast_precision_loss)] // SkIntToScalar-style casts of pixel sizes mirror the C++

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use bitflags::bitflags;
use skia_rust_core::annotation::AnnotationKeys;
use skia_rust_core::base64;
use skia_rust_core::blend_mode::{BlendMode, BlendModeCoeff};
use skia_rust_core::canvas::{PointMode, SrcRectConstraint};
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::clip_stack::{AutoRestore, B2TIter, ClipStack, DeviceSpaceType};
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::data::Data;
use skia_rust_core::device::{Device, DeviceState};
use skia_rust_core::font::Font;
use skia_rust_core::font_priv;
use skia_rust_core::font_style::Slant;
use skia_rust_core::glyph_run::{GlyphRun, GlyphRunList};
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::{Matrix, TypeMask};
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::path_utils::fill_path_with_paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::Region;
use skia_rust_core::rrect::RRect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::{Scalar, scalar};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders::shader_base::{GradientInfo, GradientType, ShaderType};
use skia_rust_core::shaders::ColorShader;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::{DynamicMemoryWStream, WStream};
use skia_rust_core::string_utils::format_g;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::utils::parse_path::{PathEncoding, to_svg_with_encoding};
use skia_rust_core::utf::Unichar;
use skia_rust_core::vertices::Vertices;
use skia_rust_core::xml::XmlWriter;
use skia_rust_raster::clip_stack_device::ClipStackDevice;

bitflags! {
    /// `SkSVGCanvas::Flags`.
    // Port of: include/svg/SkSVGCanvas.h#L21-L25 (chrome/m156)
    #[doc(alias = "SkSVGCanvas::Flags")]
    #[derive(Default, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Flags: u32 {
        /// Emit text as `<path>`s (`kConvertTextToPaths_Flag`).
        const CONVERT_TEXT_TO_PATHS = 0x01;
        /// Suppress newlines and tabs in output (`kNoPrettyXML_Flag`).
        const NO_PRETTY_XML = 0x02;
        /// Use relative commands for path encoding (`kRelativePathEncoding_Flag`).
        const RELATIVE_PATH_ENCODING = 0x04;
    }
}

/// How the device encodes an image as PNG (`SkSVGCanvas::EncodePngCallback`).
// Port of: include/svg/SkSVGCanvas.h#L27 (chrome/m156)
#[doc(alias = "SkSVGCanvas::EncodePngCallback")]
pub type EncodePngCallback = fn(&mut dyn WStream, &Pixmap<'_>) -> bool;

/// `SkSVGCanvas::Options`.
// Port of: include/svg/SkSVGCanvas.h#L29-L40 (chrome/m156)
#[doc(alias = "SkSVGCanvas::Options")]
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// `flags`.
    pub flags: Flags,
    /// Clients can provide a way to encode png (`pngEncoder`). `None` is
    /// [`default_png_encoder`], which is what the legacy factories of Skia fill in.
    pub png_encoder: Option<EncodePngCallback>,
}

/// The PNG encoder that `SkSVGCanvas::Make` fills in when `pngEncoder` is null:
/// `SkPngEncoder::Encode(dst, src, {})`.
// Port of: src/svg/SkSVGCanvas.cpp#L27-L34 (chrome/m156)
#[must_use]
pub fn default_png_encoder(dst: &mut dyn WStream, src: &Pixmap<'_>) -> bool {
    match skia_rust_codec::encode::png_encoder::encode_pixmap(
        src,
        &skia_rust_codec::encode::png_encoder::Options::default(),
    ) {
        Some(data) => dst.write(data.as_bytes()),
        None => false,
    }
}

/// `printf("%g", v)`.
fn g(v: scalar) -> String {
    format_g(f64::from(v), 6)
}

/// `printf("%.8g", v)`.
fn g8(v: scalar) -> String {
    format_g(f64::from(v), 8)
}

// Port of: src/svg/SkSVGDevice.cpp#L72-L128 (chrome/m156)
fn svg_color(color: Color) -> String {
    // https://www.w3.org/TR/css-color-3/#html4
    let named_color = |c: Color| -> Option<&'static str> {
        match u32::from(c) & 0x00ff_ffff {
            0x00_0000 => Some("black"),
            0x00_0080 => Some("navy"),
            0x00_00ff => Some("blue"),
            0x00_8000 => Some("green"),
            0x00_8080 => Some("teal"),
            0x00_ff00 => Some("lime"),
            0x00_ffff => Some("aqua"),
            0x80_0000 => Some("maroon"),
            0x80_0080 => Some("purple"),
            0x80_8000 => Some("olive"),
            0x80_8080 => Some("gray"),
            0xc0_c0c0 => Some("silver"),
            0xff_0000 => Some("red"),
            0xff_00ff => Some("fuchsia"),
            0xff_ff00 => Some("yellow"),
            0xff_ffff => Some("white"),
            _ => None,
        }
    };

    if let Some(nc) = named_color(color) {
        return nc.to_owned();
    }

    let r = color.r();
    let g = color.g();
    let b = color.b();

    // Some users care about every byte here, so we'll use hex colors with single-digit channels
    // when possible.
    let rh = r >> 4;
    let rl = r & 0xf;
    let gh = g >> 4;
    let gl = g & 0xf;
    let bh = b >> 4;
    let bl = b & 0xf;
    if rh == rl && gh == gl && bh == bl {
        return format!("#{rh:1X}{gh:1X}{bh:1X}");
    }

    format!("#{r:02X}{g:02X}{b:02X}")
}

// Port of: src/svg/SkSVGDevice.cpp#L130-L132 (chrome/m156)
fn svg_opacity(color: Color) -> scalar {
    f32::from(color.a()) / 255.0
}

// Port of: src/svg/SkSVGDevice.cpp#L134-L147 (chrome/m156)
fn svg_cap(cap: Cap) -> Option<&'static str> {
    match cap {
        Cap::Butt => None, // default
        Cap::Round => Some("round"),
        Cap::Square => Some("square"),
    }
}

// Port of: src/svg/SkSVGDevice.cpp#L149-L162 (chrome/m156)
fn svg_join(join: Join) -> Option<&'static str> {
    match join {
        Join::Miter => None, // default
        Join::Round => Some("round"),
        Join::Bevel => Some("bevel"),
    }
}

// Port of: src/svg/SkSVGDevice.cpp#L164-L192 (chrome/m156)
fn svg_transform(t: &Matrix) -> String {
    debug_assert!(!t.is_identity());

    let ty = t.get_type();
    if ty == TypeMask::PERSPECTIVE {
        // TODO: handle perspective matrices?
        String::new()
    } else if ty == TypeMask::TRANSLATE {
        format!("translate({} {})", g(t.translate_x()), g(t.translate_y()))
    } else if ty == TypeMask::SCALE {
        format!("scale({} {})", g(t.scale_x()), g(t.scale_y()))
    } else {
        // http://www.w3.org/TR/SVG/coords.html#TransformMatrixDefined
        //    | a c e |
        //    | b d f |
        //    | 0 0 1 |
        format!(
            "matrix({} {} {} {} {} {})",
            g(t.scale_x()),
            g(t.skew_y()),
            g(t.skew_x()),
            g(t.scale_y()),
            g(t.translate_x()),
            g(t.translate_y())
        )
    }
}

/// `Resources`.
// Port of: src/svg/SkSVGDevice.cpp#L194-L200 (chrome/m156)
struct Resources {
    paint_server: String,
    color_filter: String,
}

impl Resources {
    fn new(paint: &Paint) -> Self {
        Self {
            paint_server: svg_color(paint.color()),
            color_filter: String::new(),
        }
    }
}

/// Determine if the paint requires us to reset the viewport. Currently, we do this whenever the
/// paint shader calls for a repeating image.
// Port of: src/svg/SkSVGDevice.cpp#L202-L219 (chrome/m156)
fn requires_viewport_reset(paint: &Paint) -> bool {
    let Some(shader) = paint.shader_ref() else {
        return false;
    };

    let Some((_, _, xy)) = shader.is_a_image() else {
        return false;
    };

    xy.0 == TileMode::Repeat || xy.1 == TileMode::Repeat
}

// Port of: src/svg/SkSVGDevice.cpp#L221-L242 (chrome/m156)
fn add_path(glyph_run: &GlyphRun, offset: Point, builder: &mut PathBuilder) {
    let positions = glyph_run.positions();
    let mut index = 0;
    glyph_run
        .font()
        .get_paths(glyph_run.glyph_ids(), |path, mx| {
            if let Some(path) = path {
                let pos = positions[index];
                let mut total = mx.clone();
                total.post_translate((pos.x + offset.x, pos.y + offset.y));
                builder.add_path_with_transform(path, &total, None);
            } else {
                // TODO: this is going to drop color emojis.
            }
            index += 1; // move to the next glyph's position
        });
}

/// For now all this does is serve unique serial IDs, but it will eventually evolve to track and
/// deduplicate resources.
// Port of: src/svg/SkSVGDevice.cpp#L246-L282 (chrome/m156)
#[derive(Debug, Default)]
#[allow(clippy::struct_field_names)] // the counters are named after the C++ members
struct ResourceBucket {
    gradient_count: Cell<u32>,
    image_count: Cell<u32>,
    pattern_count: Cell<u32>,
    color_filter_count: Cell<u32>,
}

impl ResourceBucket {
    fn next(counter: &Cell<u32>) -> u32 {
        let n = counter.get();
        counter.set(n.wrapping_add(1));
        n
    }

    fn add_gradient(&self) -> String {
        format!("gradient_{}", Self::next(&self.gradient_count))
    }

    fn add_image(&self) -> String {
        format!("img_{}", Self::next(&self.image_count))
    }

    fn add_color_filter(&self) -> String {
        format!("cfilter_{}", Self::next(&self.color_filter_count))
    }

    fn add_pattern(&self) -> String {
        format!("pattern_{}", Self::next(&self.pattern_count))
    }
}

type WriterHandle = Rc<RefCell<Box<dyn XmlWriter>>>;

/// What an `AutoElement` shares with the device that made it: the writer, the PNG encoder and
/// the resource bucket.
#[derive(Clone)]
struct Ctx {
    writer: WriterHandle,
    png_encoder: EncodePngCallback,
    resource_bucket: Rc<ResourceBucket>,
}

/// `SkSVGDevice::AutoElement`: starts an element, and ends it when dropped.
// Port of: src/svg/SkSVGDevice.cpp#L284-L400 (chrome/m156)
struct AutoElement {
    ctx: Ctx,
}

impl Drop for AutoElement {
    // Port of: src/svg/SkSVGDevice.cpp#L324-L326 (chrome/m156)
    fn drop(&mut self) {
        self.ctx.writer.borrow_mut().end_element();
    }
}

impl AutoElement {
    // Port of: src/svg/SkSVGDevice.cpp#L286-L303 (chrome/m156)
    fn new(ctx: &Ctx, name: &str) -> Self {
        ctx.writer.borrow_mut().start_element(name);
        Self { ctx: ctx.clone() }
    }

    // Port of: src/svg/SkSVGDevice.cpp#L305-L322 (chrome/m156)
    fn new_with_paint(
        ctx: &Ctx,
        name: &str,
        matrix: &Matrix,
        paint: &Paint,
        sync_clip_stack: impl FnOnce(),
    ) -> Self {
        sync_clip_stack();
        let res = ctx.add_resources(paint);

        ctx.writer.borrow_mut().start_element(name);

        let this = Self { ctx: ctx.clone() };
        this.add_paint(paint, &res);

        if !matrix.is_identity() {
            this.add_attribute("transform", &svg_transform(matrix));
        }
        this
    }

    // Port of: src/svg/SkSVGDevice.cpp#L328-L330 (chrome/m156)
    fn add_attribute(&self, name: &str, val: &str) {
        self.ctx.writer.borrow_mut().add_attribute(name, val);
    }

    // Port of: src/svg/SkSVGDevice.cpp#L336-L338 (chrome/m156)
    fn add_attribute_i32(&self, name: &str, val: i32) {
        self.ctx.writer.borrow_mut().add_s32_attribute(name, val);
    }

    // Port of: src/svg/SkSVGDevice.cpp#L340-L342 (chrome/m156)
    fn add_attribute_scalar(&self, name: &str, val: scalar) {
        self.ctx.writer.borrow_mut().add_scalar_attribute(name, val);
    }

    // Port of: src/svg/SkSVGDevice.cpp#L344-L346 (chrome/m156)
    fn add_text(&self, text: &str) {
        self.ctx.writer.borrow_mut().add_text(text);
    }

    // Port of: src/svg/SkSVGDevice.cpp#L789-L799 (chrome/m156)
    fn add_rect_attributes(&self, rect: &Rect) {
        // x, y default to 0
        if rect.left != 0.0 {
            self.add_attribute_scalar("x", rect.left);
        }
        if rect.top != 0.0 {
            self.add_attribute_scalar("y", rect.top);
        }

        self.add_attribute_scalar("width", rect.width());
        self.add_attribute_scalar("height", rect.height());
    }

    // Port of: src/svg/SkSVGDevice.cpp#L801-L804 (chrome/m156)
    fn add_path_attributes(&self, path: &Path, encoding: PathEncoding) {
        self.add_attribute("d", &to_svg_with_encoding(path, encoding));
    }

    // Port of: src/svg/SkSVGDevice.cpp#L806-L871 (chrome/m156)
    fn add_text_attributes(&self, font: &Font) {
        self.add_attribute_scalar("font-size", font.size());

        let mut family_name = String::new();
        let mut family_set: HashSet<String> = HashSet::new();
        let tface = font.typeface();

        let style = tface.font_style();
        if style.slant() == Slant::Italic {
            self.add_attribute("font-style", "italic");
        } else if style.slant() == Slant::Oblique {
            self.add_attribute("font-style", "oblique");
        }
        let weight_index = ((*style.weight()).clamp(100, 900) - 50) / 100;
        if weight_index != 3 {
            const WEIGHTS: [&str; 10] = [
                "100", "200", "300", "normal", "400", "500", "600", "bold", "800", "900",
            ];
            self.add_attribute(
                "font-weight",
                WEIGHTS[usize::try_from(weight_index).expect("a weight index")],
            );
        }
        let stretch_index = *style.width() - 1;
        if stretch_index != 4 {
            const STRETCHES: [&str; 9] = [
                "ultra-condensed",
                "extra-condensed",
                "condensed",
                "semi-condensed",
                "normal",
                "semi-expanded",
                "expanded",
                "extra-expanded",
                "ultra-expanded",
            ];
            self.add_attribute(
                "font-stretch",
                STRETCHES[usize::try_from(stretch_index).expect("a stretch index")],
            );
        }

        for family_string in tface.new_family_name_iterator() {
            if family_set.contains(&family_string.string) {
                continue;
            }
            family_set.insert(family_string.string.clone());
            if !family_name.is_empty() {
                family_name.push_str(", ");
            }
            family_name.push_str(&family_string.string);
        }
        if !family_name.is_empty() {
            self.add_attribute("font-family", &family_name);
        }
    }

    // Port of: src/svg/SkSVGDevice.cpp#L402-L458 (chrome/m156)
    fn add_paint(&self, paint: &Paint, resources: &Resources) {
        // Path effects are applied to all vector graphics (rects, rrects, ovals,
        // paths etc).  This should only happen when a path effect is attached to
        // non-vector graphics (text, image) or a new vector graphics primitive is
        //added that is not handled by base drawPath() routine.
        if paint.path_effect().is_some() {
            // SkDebugf("Unsupported path effect in addPaint.");
        }
        let style = paint.style();
        if style == Style::Fill || style == Style::StrokeAndFill {
            const DEFAULT_FILL: &str = "black";
            if resources.paint_server != DEFAULT_FILL {
                self.add_attribute("fill", &resources.paint_server);
            }
            if 255 != paint.color().a() {
                self.add_attribute_scalar("fill-opacity", svg_opacity(paint.color()));
            }
        } else {
            debug_assert_eq!(style, Style::Stroke);
            self.add_attribute("fill", "none");
        }

        if !resources.color_filter.is_empty() {
            self.add_attribute("filter", &resources.color_filter);
        }

        if style == Style::Stroke || style == Style::StrokeAndFill {
            self.add_attribute("stroke", &resources.paint_server);

            let mut stroke_width = paint.stroke_width();
            if stroke_width == 0.0 {
                // Hairline stroke
                stroke_width = 1.0;
                self.add_attribute("vector-effect", "non-scaling-stroke");
            }
            self.add_attribute_scalar("stroke-width", stroke_width);

            if let Some(cap) = svg_cap(paint.stroke_cap()) {
                self.add_attribute("stroke-linecap", cap);
            }

            if let Some(join) = svg_join(paint.stroke_join()) {
                self.add_attribute("stroke-linejoin", join);
            }

            if paint.stroke_join() == Join::Miter {
                self.add_attribute_scalar("stroke-miterlimit", paint.stroke_miter());
            }

            if 255 != paint.color().a() {
                self.add_attribute_scalar("stroke-opacity", svg_opacity(paint.color()));
            }
        } else {
            debug_assert_eq!(style, Style::Fill);
            // SVG default stroke value is "none".
        }
    }
}

/// How the last stage of a color filter combines the flood with the source.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CompositeMode {
    None,
    Composite,
    Blend,
}

/// `BlendModeInfo`.
// Port of: src/svg/SkSVGDevice.cpp#L492-L498 (chrome/m156)
struct BlendModeInfo {
    mode: CompositeMode,
    /// The `operator` (feComposite) or `mode` (feBlend) attribute.
    op: Option<&'static str>,
    /// Whether the flood/sourcegraphic inputs are swapped.
    swap_inputs: bool,
    /// Arithmetic coefficients.
    coeffs: Option<[f32; 4]>,
}

/// `gBlendModeInfoMap`; `None` for a blend mode that is not in the map.
// Port of: src/svg/SkSVGDevice.cpp#L499-L536 (chrome/m156)
fn blend_mode_info(bm: BlendMode) -> Option<BlendModeInfo> {
    use CompositeMode::{Blend, Composite, None as NoStage};
    let info = |mode, op, swap_inputs, coeffs| {
        Some(BlendModeInfo {
            mode,
            op,
            swap_inputs,
            coeffs,
        })
    };
    let comp = |op| info(Composite, Some(op), false, None);
    let comp_swapped = |op| info(Composite, Some(op), true, None);
    let blend = |op| info(Blend, Some(op), false, None);
    match bm {
        // kSrc does not have a feComposite/feBlend operator mapping, but we can achieve the same
        // result by skipping the final stage.
        BlendMode::Src => info(NoStage, None, false, None),

        // Standard feComposite modes
        BlendMode::SrcOver => comp("over"),
        BlendMode::SrcIn => comp("in"),
        BlendMode::SrcOut => comp("out"),
        BlendMode::SrcATop => comp("atop"),
        BlendMode::DstOver => comp_swapped("over"),
        BlendMode::DstIn => comp_swapped("in"),
        BlendMode::DstOut => comp_swapped("out"),
        BlendMode::DstATop => comp_swapped("atop"),

        BlendMode::Xor => comp("xor"),

        // Arithmetic feComposite modes
        // kPlus = s + d  => K2 = 1 (in/flood), K3 = 1 (in2/SourceGraphic)
        BlendMode::Plus => info(
            Composite,
            Some("arithmetic"),
            false,
            Some([0.0, 1.0, 1.0, 0.0]),
        ),
        // kModulate = s * d => K1 = 1 (in * in2)
        BlendMode::Modulate => info(
            Composite,
            Some("arithmetic"),
            false,
            Some([1.0, 0.0, 0.0, 0.0]),
        ),

        // Advanced feBlend modes (Correctly mapping kLighten and others!)
        BlendMode::Multiply => blend("multiply"),
        BlendMode::Screen => blend("screen"),
        BlendMode::Darken => blend("darken"),
        BlendMode::Lighten => blend("lighten"),
        BlendMode::Overlay => blend("overlay"),
        BlendMode::ColorDodge => blend("color-dodge"),
        BlendMode::ColorBurn => blend("color-burn"),
        BlendMode::HardLight => blend("hard-light"),
        BlendMode::SoftLight => blend("soft-light"),
        BlendMode::Difference => blend("difference"),
        BlendMode::Exclusion => blend("exclusion"),
        BlendMode::Hue => blend("hue"),
        BlendMode::Saturation => blend("saturation"),
        BlendMode::Color => blend("color"),
        BlendMode::Luminosity => blend("luminosity"),
        _ => None,
    }
}

// Port of: src/svg/SkSVGDevice.cpp#L596-L598 (chrome/m156)
fn is_png(bytes: &[u8]) -> bool {
    const PNG_SIG: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    bytes.starts_with(&PNG_SIG)
}

// Port of: src/svg/SkSVGDevice.cpp#L600-L603 (chrome/m156)
fn is_jpeg(bytes: &[u8]) -> bool {
    const JPEG_SIG: [u8; 3] = [0xFF, 0xD8, 0xFF];
    bytes.starts_with(&JPEG_SIG)
}

// Port of: src/svg/SkSVGDevice.cpp#L605-L612 (chrome/m156)
fn encode_png_pixmap(pixmap: &Pixmap<'_>, png_encoder: EncodePngCallback) -> Option<Vec<u8>> {
    let mut stream = DynamicMemoryWStream::new();
    if !png_encoder(&mut stream, pixmap) {
        return None;
    }

    Some(stream.detach_as_vector())
}

// Port of: src/svg/SkSVGDevice.cpp#L614-L627 (chrome/m156)
fn encode_png_image(image: &Image, png_encoder: EncodePngCallback) -> Option<Vec<u8>> {
    // GrDirectContext is nullptr because we shouldn't have any texture-based images passed in.
    let raster_image = image.to_raster_image()?;

    let pixmap = raster_image.peek_pixels()?;

    encode_png_pixmap(&pixmap, png_encoder)
}

/// Returns data uri from bytes. It will use any cached data if available, otherwise will encode
/// as png.
// Port of: src/svg/SkSVGDevice.cpp#L629-L669 (chrome/m156)
fn as_data_uri(image: &Image, png_encoder: EncodePngCallback) -> Option<String> {
    const JPG_DATA_PREFIX: &str = "data:image/jpeg;base64,";
    const PNG_DATA_PREFIX: &str = "data:image/png;base64,";

    debug_assert!(!image.is_texture_backed());

    let mut selected_prefix = PNG_DATA_PREFIX;

    let image_data: Vec<u8> = if let Some(image_data) = image.ref_encoded_data() {
        // Already encoded as something
        if is_jpeg(image_data.as_bytes()) {
            selected_prefix = JPG_DATA_PREFIX;
            image_data.as_bytes().to_vec()
        } else if !is_png(image_data.as_bytes()) {
            // re-encode the image as a PNG.
            encode_png_image(image, png_encoder)?
        } else {
            // else, it's already encoded as a PNG - we don't need to do anything.
            image_data.as_bytes().to_vec()
        }
    } else {
        // It was not encoded as something, so we need to encode it as a PNG.
        encode_png_image(image, png_encoder)?
    };

    let mut data_uri = String::from(selected_prefix);
    data_uri.push_str(&base64_encode(&image_data));
    Some(data_uri)
}

/// `SkBase64::Encode` into a string.
fn base64_encode(src: &[u8]) -> String {
    let mut dst = vec![0u8; base64::encoded_size(src.len())];
    let written = base64::encode(src, Some(&mut dst), None);
    dst.truncate(written);
    String::from_utf8(dst).expect("base64 is ASCII")
}

impl Ctx {
    // Port of: src/svg/SkSVGDevice.cpp#L460-L474 (chrome/m156)
    fn add_resources(&self, paint: &Paint) -> Resources {
        let mut resources = Resources::new(paint);

        if let Some(shader) = paint.shader_ref() {
            let _defs = AutoElement::new(self, "defs");

            self.add_shader_resources(shader, &mut resources);
        }

        if let Some(cf) = paint.color_filter() {
            self.add_color_filter_resources(&cf, &mut resources);
        }

        resources
    }

    // Port of: src/svg/SkSVGDevice.cpp#L476-L498 (chrome/m156)
    fn add_gradient_shader_resources(&self, shader: &Shader, resources: &mut Resources) {
        let base = shader.as_base();
        let mut gr_info = GradientInfo::default();
        let gradient_type = base.as_gradient(Some(&mut gr_info), None);

        if gradient_type != GradientType::Linear
            && gradient_type != GradientType::Radial
            && gradient_type != GradientType::Conical
        {
            return;
        }

        let count = gr_info.color_count;
        let mut gr_colors = vec![Color4f::new(0.0, 0.0, 0.0, 0.0); count];
        let mut gr_offsets = vec![0.0; count];

        // One more call to get the actual colors/offsets and local matrix.
        let mut local_matrix = Matrix::new_identity();
        let (point, radius, color_count) = {
            let mut gr_info = GradientInfo {
                color_count: count,
                colors: Some(&mut gr_colors),
                color_offsets: Some(&mut gr_offsets),
                ..GradientInfo::default()
            };
            base.as_gradient(Some(&mut gr_info), Some(&mut local_matrix));
            (gr_info.point, gr_info.radius, gr_info.color_count)
        };
        debug_assert!(color_count <= gr_colors.len());
        debug_assert!(color_count <= gr_offsets.len());

        debug_assert!(count > 0);
        resources.paint_server = format!(
            "url(#{})",
            self.add_gradient_def(
                gradient_type,
                point,
                radius,
                &gr_colors[..color_count],
                &gr_offsets[..color_count],
                &local_matrix
            )
        );
    }

    // Port of: src/svg/SkSVGDevice.cpp#L500-L594 (chrome/m156)
    fn add_color_filter_resources(
        &self,
        cf: &skia_rust_core::color_filter::ColorFilter,
        resources: &mut Resources,
    ) {
        let Some((filter_color, bm)) = cf.to_a_color_mode() else {
            // Only SkColorFilters::Blend is supported currently.
            return;
        };

        let Some(bm_info) = blend_mode_info(bm) else {
            // Unsupported blend mode.
            return;
        };

        let colorfilter_id = self.resource_bucket.add_color_filter();
        {
            let filter_element = AutoElement::new(self, "filter");
            filter_element.add_attribute("id", &colorfilter_id);
            filter_element.add_attribute("x", "0%");
            filter_element.add_attribute("y", "0%");
            filter_element.add_attribute("width", "100%");
            filter_element.add_attribute("height", "100%");

            // 1. flood to generate the filter color.
            {
                let flood_element = AutoElement::new(self, "feFlood");
                flood_element.add_attribute("flood-color", &svg_color(filter_color));
                flood_element.add_attribute_scalar("flood-opacity", svg_opacity(filter_color));
                flood_element.add_attribute("result", "flood");
            }

            // 2. optional compositing stage (feComposite or feBlend depending on blend mode).
            {
                let composite_element = match bm_info.mode {
                    CompositeMode::Composite => {
                        let e = AutoElement::new(self, "feComposite");
                        e.add_attribute("operator", bm_info.op.expect("an operator"));
                        if let Some(coeffs) = &bm_info.coeffs {
                            e.add_attribute_scalar("k1", coeffs[0]);
                            e.add_attribute_scalar("k2", coeffs[1]);
                            e.add_attribute_scalar("k3", coeffs[2]);
                            e.add_attribute_scalar("k4", coeffs[3]);
                        }
                        Some(e)
                    }
                    CompositeMode::Blend => {
                        let e = AutoElement::new(self, "feBlend");
                        e.add_attribute("mode", bm_info.op.expect("a mode"));
                        Some(e)
                    }
                    CompositeMode::None => None,
                };

                if let Some(composite_element) = &composite_element {
                    // Normally we composite the flood result against source, but we swap the order
                    // for inverted (kDst*) modes.
                    composite_element.add_attribute(
                        "in",
                        if bm_info.swap_inputs {
                            "SourceGraphic"
                        } else {
                            "flood"
                        },
                    );
                    composite_element.add_attribute(
                        "in2",
                        if bm_info.swap_inputs {
                            "flood"
                        } else {
                            "SourceGraphic"
                        },
                    );
                }
            }

            // 3. optional alpha mask stage to avoid flooding the whole filter area.
            let src_coeff = bm.as_coeff().map(|(src, _)| src);
            let needs_alpha_mask =
                // All advanced/non-Porter-Duff modes can leak when dst is transparent.
                src_coeff.is_none() ||
                // For Porter-Duff, dst stays transparent if the source term is
                // multiplied by destination alpha (kDA) or zero (kZero).
                (src_coeff != Some(BlendModeCoeff::DA) && src_coeff != Some(BlendModeCoeff::Zero));
            if needs_alpha_mask {
                let mask_element = AutoElement::new(self, "feComposite");
                // "in" defaults to the preceding stage, either feFlood or feComposite/feBlend.
                mask_element.add_attribute("in2", "SourceAlpha");
                mask_element.add_attribute("operator", "in");
            }
        }
        resources.color_filter = format!("url(#{colorfilter_id})");
    }

    // Port of: src/svg/SkSVGDevice.cpp#L671-L723 (chrome/m156)
    fn add_image_shader_resources(&self, shader: &Shader, resources: &mut Resources) {
        let (image, _out_matrix, xy) = shader.is_a_image().expect("an image shader");

        let mut pattern_dims = [String::new(), String::new()]; // width, height

        let Some(data_uri) = as_data_uri(&image, self.png_encoder) else {
            return;
        };
        let image_size = image.bounds();
        for (i, dim) in pattern_dims.iter_mut().enumerate() {
            let image_dimension = if i == 0 {
                image_size.width()
            } else {
                image_size.height()
            };
            let tile = if i == 0 { xy.0 } else { xy.1 };
            match tile {
                TileMode::Repeat => {
                    skia_rust_core::string::str_append_scalar(dim, image_dimension as scalar);
                }
                _ => {
                    // TODO: other tile modes?
                    "100%".clone_into(dim);
                }
            }
        }

        let pattern_id = self.resource_bucket.add_pattern();
        {
            let pattern = AutoElement::new(self, "pattern");
            pattern.add_attribute("id", &pattern_id);
            pattern.add_attribute("patternUnits", "userSpaceOnUse");
            pattern.add_attribute("patternContentUnits", "userSpaceOnUse");
            pattern.add_attribute("width", &pattern_dims[0]);
            pattern.add_attribute("height", &pattern_dims[1]);
            pattern.add_attribute_i32("x", 0);
            pattern.add_attribute_i32("y", 0);

            {
                let image_id = self.resource_bucket.add_image();
                let image_tag = AutoElement::new(self, "image");
                image_tag.add_attribute("id", &image_id);
                image_tag.add_attribute_i32("x", 0);
                image_tag.add_attribute_i32("y", 0);
                image_tag.add_attribute_i32("width", image.width());
                image_tag.add_attribute_i32("height", image.height());
                image_tag.add_attribute("xlink:href", &data_uri);
            }
        }
        resources.paint_server = format!("url(#{pattern_id})");
    }

    // Port of: src/svg/SkSVGDevice.cpp#L725-L743 (chrome/m156)
    fn add_shader_resources(&self, shader: &Shader, resources: &mut Resources) {
        if shader.as_base().shader_type() == ShaderType::Color {
            let base: &dyn std::any::Any = shader.as_base();
            let color_shader = base.downcast_ref::<ColorShader>().expect("a color shader");
            resources.paint_server = svg_color(color_shader.color().to_color());
        } else
        // Checking the shader type is too restrictive, as it rejects local matrix wrappers.
        // Wrappers do forward asGradient() though, so we can reach nested gradients this way.
        if shader.as_base().as_gradient(None, None) != GradientType::None {
            self.add_gradient_shader_resources(shader, resources);
        } else if shader.is_a_image().is_some() {
            self.add_image_shader_resources(shader, resources);
        }
        // TODO: other shader types?
    }

    // Port of: src/svg/SkSVGDevice.cpp#L745-L787 (chrome/m156)
    fn add_gradient_def(
        &self,
        gradient_type: GradientType,
        point: [Point; 2],
        radius: [scalar; 2],
        colors: &[Color4f],
        offsets: &[scalar],
        local_matrix: &Matrix,
    ) -> String {
        let id = self.resource_bucket.add_gradient();

        debug_assert!(matches!(
            gradient_type,
            GradientType::Linear | GradientType::Radial | GradientType::Conical
        ));

        let elem_name = if gradient_type == GradientType::Linear {
            "linearGradient"
        } else {
            "radialGradient"
        };
        {
            let gradient = AutoElement::new(self, elem_name);

            gradient.add_attribute("id", &id);
            gradient.add_attribute("gradientUnits", "userSpaceOnUse");

            match gradient_type {
                GradientType::Linear => {
                    gradient.add_attribute_scalar("x1", point[0].x);
                    gradient.add_attribute_scalar("y1", point[0].y);
                    gradient.add_attribute_scalar("x2", point[1].x);
                    gradient.add_attribute_scalar("y2", point[1].y);
                }
                GradientType::Conical | GradientType::Radial => {
                    if gradient_type == GradientType::Conical {
                        gradient.add_attribute_scalar("fx", point[1].x);
                        gradient.add_attribute_scalar("fy", point[1].y);
                        gradient.add_attribute_scalar("fr", radius[1]);
                    }
                    gradient.add_attribute_scalar("cx", point[0].x);
                    gradient.add_attribute_scalar("cy", point[0].y);
                    gradient.add_attribute_scalar("r", radius[0]);
                }
                _ => unreachable!(),
            }

            if !local_matrix.is_identity() {
                gradient.add_attribute("gradientTransform", &svg_transform(local_matrix));
            }

            debug_assert!(colors.len() >= 2);
            for (c, offset) in colors.iter().zip(offsets) {
                let color = c.to_color();
                let color_str = svg_color(color);

                {
                    let stop = AutoElement::new(self, "stop");
                    stop.add_attribute_scalar("offset", *offset);
                    stop.add_attribute("stop-color", &color_str);

                    if 255 != color.a() {
                        stop.add_attribute_scalar("stop-opacity", svg_opacity(color));
                    }
                }
            }
        }

        id
    }
}

/// `SkSVGDevice::ClipRec`.
// Port of: src/svg/SkSVGDevice.h#L83-L91 (chrome/m156)
struct ClipRec {
    /// Held for its drop, which ends the group.
    _clip_path_elem: AutoElement,
    gen_id: u32,
}

/// The part of the device that `SkClipStackDevice` does not have.
struct Inner {
    // Fields drop in this order, as the C++ members do in reverse: the clip groups, then the root
    // element, then the writer (the last owner of which flushes it).
    clip_stack: Vec<ClipRec>,
    /// Held for its drop, which ends the root element.
    _root_element: Option<AutoElement>,
    opts: Options,
    ctx: Ctx,
}

/// `SVGTextBuilder`.
// Port of: src/svg/SkSVGDevice.cpp#L1129-L1236 (chrome/m156)
struct SvgTextBuilder {
    origin: Point,
    text: String,
    pos_x_str: String,
    pos_y_str: String,
    const_y_str: String,
    const_y: scalar,
    last_char_was_whitespace: bool,
    has_const_y: bool,
}

impl SvgTextBuilder {
    // Port of: src/svg/SkSVGDevice.cpp#L1132-L1143 (chrome/m156)
    fn new(origin: Point, glyph_run: &GlyphRun) -> Self {
        let mut this = Self {
            origin,
            text: String::new(),
            pos_x_str: String::new(),
            pos_y_str: String::new(),
            const_y_str: String::new(),
            const_y: 0.0,
            // start off in whitespace mode to strip leading space
            last_char_was_whitespace: true,
            has_const_y: true,
        };
        let run_size = glyph_run.run_size();
        let mut unichars: Vec<Unichar> = vec![0; run_size];
        font_priv::glyphs_to_unichars(glyph_run.font(), glyph_run.glyph_ids(), &mut unichars);
        let positions = glyph_run.positions();
        for i in 0..run_size {
            this.append_unichar(unichars[i], positions[i]);
        }
        this
    }

    fn text(&self) -> &str {
        &self.text
    }

    fn pos_x(&self) -> &str {
        &self.pos_x_str
    }

    fn pos_y(&self) -> &str {
        if self.has_const_y {
            &self.const_y_str
        } else {
            &self.pos_y_str
        }
    }

    // Port of: src/svg/SkSVGDevice.cpp#L1153-L1226 (chrome/m156)
    fn append_unichar(&mut self, c: Unichar, position: Point) {
        let mut discard_pos = false;
        let mut is_whitespace = false;

        let append = |text: &mut String, c: Unichar| {
            text.push(
                u32::try_from(c)
                    .ok()
                    .and_then(char::from_u32)
                    .unwrap_or('\u{FFFD}'),
            );
        };

        match c {
            0x20 | 0x09 => {
                // ' ' and '\t': consolidate whitespace to match SVG's xml:space=default munging
                // (http://www.w3.org/TR/SVG/text.html#WhiteSpace)
                if self.last_char_was_whitespace {
                    discard_pos = true;
                } else {
                    append(&mut self.text, c);
                }
                is_whitespace = true;
            }
            0 => {
                // SkPaint::glyphsToUnichars() returns \0 for inconvertible glyphs, but these
                // are not legal XML characters (http://www.w3.org/TR/REC-xml/#charsets)
                discard_pos = true;
                is_whitespace = self.last_char_was_whitespace; // preserve whitespace consolidation
            }
            0x26 => self.text.push_str("&amp;"),
            0x22 => self.text.push_str("&quot;"),
            0x27 => self.text.push_str("&apos;"),
            0x3c => self.text.push_str("&lt;"),
            0x3e => self.text.push_str("&gt;"),
            _ => append(&mut self.text, c),
        }

        self.last_char_was_whitespace = is_whitespace;

        if discard_pos {
            return;
        }

        let position = Point::new(position.x + self.origin.x, position.y + self.origin.y);
        self.pos_x_str.push_str(&g8(position.x));
        self.pos_x_str.push_str(", ");
        self.pos_y_str.push_str(&g8(position.y));
        self.pos_y_str.push_str(", ");

        if self.const_y_str.is_empty() {
            self.const_y_str.clone_from(&self.pos_y_str);
            self.const_y = position.y;
        } else {
            self.has_const_y &= <scalar as Scalar>::nearly_equal(self.const_y, position.y, None);
        }
    }
}

impl Inner {
    fn path_encoding(&self) -> PathEncoding {
        if self.opts.flags.contains(Flags::RELATIVE_PATH_ENCODING) {
            PathEncoding::Relative
        } else {
            PathEncoding::Absolute
        }
    }

    /// `AutoElement(name, this)`.
    fn element(&self, name: &str) -> AutoElement {
        AutoElement::new(&self.ctx, name)
    }

    /// `AutoElement(name, this, bucket, MxCp(matrix, cs), paint)`.
    fn element_with_paint(
        &mut self,
        name: &str,
        matrix: &Matrix,
        cs: &ClipStack,
        paint: &Paint,
    ) -> AutoElement {
        let ctx = self.ctx.clone();
        AutoElement::new_with_paint(&ctx, name, matrix, paint, || self.sync_clip_stack(cs))
    }

    // Port of: src/svg/SkSVGDevice.cpp#L938-L1019 (chrome/m156)
    fn sync_clip_stack(&mut self, cs: &ClipStack) {
        let mut iter = B2TIter::new(cs);

        let mut rec_idx = 0;

        // First, find/preserve the common bottom.
        let mut elem;
        loop {
            elem = iter.next();
            let Some(e) = elem else {
                break;
            };
            if rec_idx >= self.clip_stack.len() {
                break;
            }
            if self.clip_stack[rec_idx].gen_id != e.gen_id() {
                break;
            }
            rec_idx += 1;
        }

        // Discard out-of-date stack top.
        while self.clip_stack.len() > rec_idx {
            self.clip_stack.pop();
        }

        // Rebuild the top.
        while let Some(e) = elem {
            let cid = self.define_clip(e);

            let clip_grp = self.element("g");
            clip_grp.add_attribute("clip-path", &format!("url(#{cid})"));

            self.clip_stack.push(ClipRec {
                _clip_path_elem: clip_grp,
                gen_id: e.gen_id(),
            });

            elem = iter.next();
        }
    }

    // Port of: src/svg/SkSVGDevice.cpp#L956-L1002 (chrome/m156)
    fn define_clip(&self, e: &skia_rust_core::clip_stack::Element) -> String {
        let cid = format!("cl_{:x}", e.gen_id());

        let clip_path = self.element("clipPath");
        clip_path.add_attribute("id", &cid);

        // TODO: handle non-intersect clips.

        match e.device_space_type() {
            DeviceSpaceType::Empty => {
                // TODO: can we skip this?
                let _rect = self.element("rect");
            }
            DeviceSpaceType::Rect => {
                let rect = self.element("rect");
                rect.add_rect_attributes(e.device_space_rect());
            }
            DeviceSpaceType::RRect => {
                // TODO: complex rrect handling?
                let rr = e.device_space_rrect();
                let radii = rr.simple_radii();

                let rrect = self.element("rect");
                rrect.add_rect_attributes(rr.rect());
                rrect.add_attribute_scalar("rx", radii.x);
                rrect.add_attribute_scalar("ry", radii.y);
            }
            DeviceSpaceType::Path => {
                let p = e.device_space_path();
                let path = self.element("path");
                path.add_path_attributes(p, self.path_encoding());
                if p.fill_type() == PathFillType::EvenOdd {
                    path.add_attribute("clip-rule", "evenodd");
                }
            }
            DeviceSpaceType::Shader => {
                // TODO: handle shader clipping, perhaps rasterize and apply as a mask image?
            }
        }

        drop(clip_path);
        cid
    }

    // Port of: src/svg/SkSVGDevice.cpp#L1021-L1027 (chrome/m156)
    fn draw_paint(&mut self, state: &DeviceState, cs: &ClipStack, paint: &Paint) {
        let rect = self.element_with_paint("rect", state.local_to_device(), cs, paint);
        rect.add_rect_attributes(&Rect::from_wh(
            state.width() as scalar,
            state.height() as scalar,
        ));
    }

    // Port of: src/svg/SkSVGDevice.cpp#L1029-L1055 (chrome/m156)
    fn draw_annotation(
        &mut self,
        state: &DeviceState,
        cs: &mut ClipStack,
        rect: &Rect,
        key: &str,
        value: Option<&Data>,
    ) {
        let Some(value) = value else {
            return;
        };

        if AnnotationKeys::url_key() == key || AnnotationKeys::link_named_dest_key() == key {
            cs.save();
            cs.clip_rect(rect, state.local_to_device(), ClipOp::Intersect, true);
            let origin = state.origin();
            let global_bounds = IRect::from_xywh(origin.x, origin.y, state.width(), state.height());
            let transformed_rect = cs.bounds(&global_bounds);
            cs.restore();
            if transformed_rect.is_empty() {
                return;
            }

            // `SkString url(data, size - 1)`: the data has a terminating zero.
            let bytes = value.as_bytes();
            let url = String::from_utf8_lossy(&bytes[..bytes.len().saturating_sub(1)]);
            let a = self.element("a");
            a.add_attribute("xlink:href", &url);
            {
                let r = self.element("rect");
                r.add_attribute("fill-opacity", "0.0");
                r.add_rect_attributes(&transformed_rect);
            }
            drop(a);
        }
    }

    // Port of: src/svg/SkSVGDevice.cpp#L1057-L1088 (chrome/m156)
    fn draw_points(
        &mut self,
        state: &DeviceState,
        cs: &ClipStack,
        mode: PointMode,
        pts: &[Point],
        paint: &Paint,
    ) {
        let mut path = PathBuilder::new();

        let count = pts.len();
        match mode {
            // todo
            PointMode::Points => {
                for &pt in pts {
                    path.move_to(pt);
                    path.line_to(pt);
                }
            }
            PointMode::Lines => {
                let mut i = 1;
                while i < count {
                    path.move_to(pts[i - 1]);
                    path.line_to(pts[i]);
                    i += 2;
                }
            }
            PointMode::Polygon => {
                if count > 1 {
                    path.add_polygon(pts, false);
                }
            }
        }

        self.draw_path(state, cs, &path.detach(), paint);
    }

    // Port of: src/svg/SkSVGDevice.cpp#L1090-L1112 (chrome/m156)
    fn draw_rect(&mut self, state: &DeviceState, cs: &ClipStack, r: &Rect, paint: &Paint) {
        if paint.path_effect().is_some() {
            self.draw_path(state, cs, &Path::rect(r, None), paint);
            return;
        }

        let m = state.local_to_device();
        let svg = if requires_viewport_reset(paint) {
            let svg = self.element_with_paint("svg", m, cs, paint);
            svg.add_rect_attributes(r);
            Some(svg)
        } else {
            None
        };

        let rect = self.element_with_paint("rect", m, cs, paint);

        if svg.is_some() {
            rect.add_attribute_i32("x", 0);
            rect.add_attribute_i32("y", 0);
            rect.add_attribute("width", "100%");
            rect.add_attribute("height", "100%");
        } else {
            rect.add_rect_attributes(r);
        }
    }

    // Port of: src/svg/SkSVGDevice.cpp#L1114-L1125 (chrome/m156)
    fn draw_oval(&mut self, state: &DeviceState, cs: &ClipStack, oval: &Rect, paint: &Paint) {
        if paint.path_effect().is_some() {
            self.draw_path(state, cs, &Path::oval(oval, None), paint);
            return;
        }

        let ellipse = self.element_with_paint("ellipse", state.local_to_device(), cs, paint);
        ellipse.add_attribute_scalar("cx", oval.center_x());
        ellipse.add_attribute_scalar("cy", oval.center_y());
        ellipse.add_attribute_scalar("rx", oval.width() / 2.0);
        ellipse.add_attribute_scalar("ry", oval.height() / 2.0);
    }

    // Port of: src/svg/SkSVGDevice.cpp#L1127-L1136 (chrome/m156)
    fn draw_rrect(&mut self, state: &DeviceState, cs: &ClipStack, rr: &RRect, paint: &Paint) {
        if paint.path_effect().is_some() {
            self.draw_path(state, cs, &Path::rrect(rr, None), paint);
            return;
        }

        let elem = self.element_with_paint("path", state.local_to_device(), cs, paint);
        elem.add_path_attributes(&Path::rrect(rr, None), self.path_encoding());
    }

    // Port of: src/svg/SkSVGDevice.cpp#L1138-L1182 (chrome/m156)
    fn draw_path(&mut self, state: &DeviceState, cs: &ClipStack, path: &Path, paint: &Paint) {
        if path.is_inverse_fill_type() {
            // SkDebugf("Inverse path fill type not yet implemented.");
            return;
        }

        let path_storage;
        let mut path_ptr = path;
        let paint_storage;
        let mut path_paint = paint;

        // Apply path effect from paint to path.
        if path_paint.path_effect().is_some() {
            let mut builder = PathBuilder::new();
            let fill = fill_path_with_paint(path, path_paint, &mut builder, None::<&Rect>, None);
            path_storage = builder.detach();
            path_ptr = &path_storage;
            let mut pp = path_paint.clone();
            if fill {
                // Path should be filled.
                pp.set_style(Style::Fill);
            } else {
                // Path should be drawn with a hairline (width == 0).
                pp.set_style(Style::Stroke);
                pp.set_stroke_width(0.0);
            }

            pp.set_path_effect(None); // path effect processed
            paint_storage = pp;
            path_paint = &paint_storage;
        }

        // Create path element.
        let elem = self.element_with_paint("path", state.local_to_device(), cs, path_paint);
        elem.add_path_attributes(path_ptr, self.path_encoding());

        // TODO: inverse fill types?
        if path_ptr.fill_type() == PathFillType::EvenOdd {
            elem.add_attribute("fill-rule", "evenodd");
        }
    }

    // Port of: src/svg/SkSVGDevice.cpp#L1184-L1215 (chrome/m156)
    fn draw_bitmap_common(
        &mut self,
        matrix: &Matrix,
        cs: &ClipStack,
        bm: &skia_rust_core::bitmap::Bitmap,
        paint: &Paint,
    ) {
        let Some(pixmap) = bm.peek_pixels() else {
            return;
        };
        let Some(png_data) = encode_png_pixmap(&pixmap, self.ctx.png_encoder) else {
            return;
        };

        let mut svg_image_data = String::from("data:image/png;base64,");
        svg_image_data.push_str(&base64_encode(&png_data));

        let image_id = self.ctx.resource_bucket.add_image();
        {
            let _defs = self.element("defs");
            {
                let image = self.element("image");
                image.add_attribute("id", &image_id);
                image.add_attribute_i32("width", bm.width());
                image.add_attribute_i32("height", bm.height());
                image.add_attribute("xlink:href", &svg_image_data);
            }
        }

        {
            let image_use = self.element_with_paint("use", matrix, cs, paint);
            image_use.add_attribute("xlink:href", &format!("#{image_id}"));
        }
    }
}

/// `SkSVGDevice`: a device that writes SVG.
// Port of: src/svg/SkSVGDevice.h#L38-L100 (chrome/m156)
#[doc(alias = "SkSVGDevice")]
pub struct SvgDevice {
    state: DeviceState,
    clip: ClipStackDevice,
    inner: Inner,
}

impl std::fmt::Debug for SvgDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SvgDevice")
            .field("state", &self.state)
            .field("opts", &self.inner.opts)
            .finish_non_exhaustive()
    }
}

impl SvgDevice {
    /// `SkSVGDevice::Make`.
    // Port of: src/svg/SkSVGDevice.cpp#L896-L902 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(size: ISize, writer: Box<dyn XmlWriter>, opts: Options) -> Box<dyn Device> {
        Box::new(Self::new(size, writer, opts))
    }

    // Port of: src/svg/SkSVGDevice.cpp#L904-L926 (chrome/m156)
    fn new(size: ISize, writer: Box<dyn XmlWriter>, opts: Options) -> Self {
        let ctx = Ctx {
            writer: Rc::new(RefCell::new(writer)),
            png_encoder: opts.png_encoder.unwrap_or(default_png_encoder),
            resource_bucket: Rc::new(ResourceBucket::default()),
        };

        ctx.writer.borrow_mut().write_header();

        // The root <svg> tag gets closed by the destructor.
        let root_element = AutoElement::new(&ctx, "svg");

        root_element.add_attribute("xmlns", "http://www.w3.org/2000/svg");
        root_element.add_attribute("xmlns:xlink", "http://www.w3.org/1999/xlink");
        root_element.add_attribute_i32("width", size.width);
        root_element.add_attribute_i32("height", size.height);

        Self {
            state: DeviceState::new(
                ImageInfo::new_unknown(Some(size)),
                SurfaceProps::default(),
            ),
            clip: ClipStackDevice::new(),
            inner: Inner {
                clip_stack: Vec::new(),
                _root_element: Some(root_element),
                opts,
                ctx,
            },
        }
    }
}

impl Drop for SvgDevice {
    // Port of: src/svg/SkSVGDevice.cpp#L928-L932 (chrome/m156)
    fn drop(&mut self) {
        // Pop order is important.
        while self.inner.clip_stack.pop().is_some() {}
    }
}

impl Device for SvgDevice {
    fn state(&self) -> &DeviceState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut DeviceState {
        &mut self.state
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

    fn draw_paint(&mut self, paint: &Paint) {
        self.inner.draw_paint(&self.state, self.clip.cs(), paint);
    }

    fn draw_annotation(&mut self, rect: &Rect, key: &str, value: Option<&Data>) {
        self.inner
            .draw_annotation(&self.state, self.clip.cs_mut(), rect, key, value);
    }

    fn draw_points(&mut self, mode: PointMode, points: &[Point], paint: &Paint) {
        self.inner
            .draw_points(&self.state, self.clip.cs(), mode, points, paint);
    }

    // Port of: src/svg/SkSVGDevice.cpp#L1217-L1245 (chrome/m156)
    fn draw_image_rect(
        &mut self,
        image: &Image,
        src: Option<&Rect>,
        dst: &Rect,
        _sampling: &SamplingOptions,
        paint: &Paint,
        _constraint: SrcRectConstraint,
    ) {
        // TODO: support gpu images
        let Some(bm) = image.get_ro_pixels() else {
            return;
        };

        let local_to_device = self.state.local_to_device();
        let bm_bounds = Rect::from_irect(bm.bounds());
        let mut cs = AutoRestore::new(self.clip.cs_mut(), false);
        if src.is_some_and(|src| *src != bm_bounds) {
            cs.save();
            cs.clip_rect(dst, local_to_device, ClipOp::Intersect, paint.is_anti_alias());
        }

        let adjusted_matrix = Matrix::concat(
            local_to_device,
            &Matrix::rect_to_rect_or_identity(src.copied().unwrap_or(bm_bounds), dst, None),
        );

        self.inner
            .draw_bitmap_common(&adjusted_matrix, &cs, &bm, paint);
    }

    fn draw_rect(&mut self, r: &Rect, paint: &Paint) {
        self.inner.draw_rect(&self.state, self.clip.cs(), r, paint);
    }

    fn draw_oval(&mut self, oval: &Rect, paint: &Paint) {
        self.inner
            .draw_oval(&self.state, self.clip.cs(), oval, paint);
    }

    fn draw_rrect(&mut self, rr: &RRect, paint: &Paint) {
        self.inner
            .draw_rrect(&self.state, self.clip.cs(), rr, paint);
    }

    fn draw_path(&mut self, path: &Path, paint: &Paint) {
        self.inner
            .draw_path(&self.state, self.clip.cs(), path, paint);
    }

    // Port of: src/svg/SkSVGDevice.cpp#L1301-L1331 (chrome/m156)
    fn on_draw_glyph_run_list(&mut self, glyph_run_list: &GlyphRunList<'_>, paint: &Paint) {
        debug_assert!(!glyph_run_list.has_rsxform());
        let draw_as_path = self
            .inner
            .opts
            .flags
            .contains(Flags::CONVERT_TEXT_TO_PATHS)
            || paint.path_effect().is_some();

        if draw_as_path {
            // Emit a single <path> element.
            let mut builder = PathBuilder::new();
            for glyph_run in glyph_run_list.runs() {
                add_path(glyph_run, glyph_run_list.origin(), &mut builder);
            }

            self.inner
                .draw_path(&self.state, self.clip.cs(), &builder.detach(), paint);

            return;
        }

        // Emit one <text> element for each run.
        for glyph_run in glyph_run_list.runs() {
            let elem = self.inner.element_with_paint(
                "text",
                self.state.local_to_device(),
                self.clip.cs(),
                paint,
            );
            elem.add_text_attributes(glyph_run.font());

            let builder = SvgTextBuilder::new(glyph_run_list.origin(), glyph_run);
            elem.add_attribute("x", builder.pos_x());
            elem.add_attribute("y", builder.pos_y());
            elem.add_text(builder.text());
        }
    }

    // Port of: src/svg/SkSVGDevice.cpp#L1333-L1335 (chrome/m156)
    fn draw_vertices(
        &mut self,
        _vertices: &Vertices,
        _blender: skia_rust_core::blender::Blender,
        _paint: &Paint,
        _skip_color_xform: bool,
    ) {
        // todo
    }

    // Port of: src/svg/SkSVGDevice.cpp#L1337-L1339 (chrome/m156)
    fn draw_mesh(
        &mut self,
        _mesh: &skia_rust_core::mesh::Mesh,
        _blender: skia_rust_core::blender::Blender,
        _paint: &Paint,
    ) {
        // todo
    }
}
