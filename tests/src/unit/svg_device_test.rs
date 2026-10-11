// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SVGDeviceTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_codec::encode::png_encoder;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{Canvas, PointMode};
use skia_rust_core::color::{Color, colors};
use skia_rust_core::color_filters;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::{Scalar, scalar};
use skia_rust_core::shaders;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::WStream;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::utils::parse;
use skia_rust_core::xml::{Dom, Node, NodeType, XmlParserWriter};
use skia_rust_effects::dash_path_effect;
use skia_rust_raster::surfaces;
use skia_rust_svg::canvas::{Flags, Options};
use skia_rust_svg::device::SvgDevice;
use skia_rust_tools::font_tool_utils::default_portable_font;

use crate::{Reporter, def_test, errorf, reporter_assert};

/// Port of `ABORT_TEST(r, cond, ...)`: reports a failure and returns from the test.
macro_rules! abort_test {
    ($r:expr, $cond:expr, $($fmt:tt)+) => {
        if $cond {
            $r.report_failed($crate::Failure {
                file: file!(),
                line: line!(),
                condition: stringify!($cond),
                message: format!($($fmt)+),
            });
            return;
        }
    };
}

/// C's `atoi`: the leading decimal digits of `s`, with an optional sign.
fn atoi(s: &str) -> i32 {
    let s = s.trim_start();
    let end = s
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || (i == 0 && (c == '-' || c == '+'))))
        .map_or(s.len(), |(i, _)| i);
    s[..end].parse().unwrap_or(0)
}

/// C's `atof`: the leading floating-point number of `s`.
fn atof(s: &str) -> f64 {
    let s = s.trim_start();
    let mut end = 0;
    for i in (1..=s.len()).rev() {
        if s.is_char_boundary(i) && s[..i].parse::<f64>().is_ok() {
            end = i;
            break;
        }
    }
    s[..end].parse().unwrap_or(0.0)
}

// Port of: tests/SVGDeviceTest.cpp#L58-L63 (chrome/m156)
fn encode_png(dst: &mut dyn WStream, src: &Pixmap<'_>) -> bool {
    match png_encoder::encode_pixmap(src, &png_encoder::Options::default()) {
        Some(data) => dst.write(data.as_bytes()),
        None => false,
    }
}

// Port of: tests/SVGDeviceTest.cpp#L57-L72 (chrome/m156)
fn make_dom_canvas(dom: &mut Dom, flags: u32) -> Canvas {
    let opts = Options {
        flags: Flags::from_bits_retain(flags),
        png_encoder: Some(encode_png),
    };
    let svg_device = SvgDevice::make(
        ISize::new(100, 100),
        Box::new(XmlParserWriter::new_owned(Box::new(
            dom.begin_parsing_shared(),
        ))),
        opts,
    );
    Canvas::from_device(svg_device)
}

// Port of: tests/SVGDeviceTest.cpp#L77-L142 (chrome/m156)
#[allow(clippy::float_cmp)] // the C++ compares the parsed positions exactly
fn check_text_node(
    reporter: &mut Reporter,
    dom: &Dom,
    root: Option<Node>,
    offset: Point,
    scalars_per_pos: u32,
    txt: &str,
    expected: &str,
) {
    let Some(root) = root else {
        errorf!(reporter, "root element not found.");
        return;
    };

    let Some(text_elem) = dom.get_first_child(root, Some("text")) else {
        errorf!(reporter, "<text> element not found.");
        return;
    };
    reporter_assert!(reporter, dom.get_type(text_elem) == NodeType::Element);

    let text_node = dom.get_first_child(text_elem, None);
    reporter_assert!(reporter, text_node.is_some());
    if let Some(text_node) = text_node {
        reporter_assert!(reporter, dom.get_type(text_node) == NodeType::Text);
        reporter_assert!(reporter, expected == dom.get_name(text_node));
    }

    let text_len = expected.len();

    let x = dom.find_attr(text_elem, "x");
    reporter_assert!(reporter, x.is_some());
    if let Some(x) = x {
        let xpos_count = text_len;
        reporter_assert!(
            reporter,
            usize::try_from(parse::count(x.as_bytes())).ok() == Some(xpos_count)
        );

        let mut xpos: Vec<scalar> = vec![0.0; xpos_count];
        let _ = parse::find_scalars(x.as_bytes(), 0, &mut xpos);
        if scalars_per_pos < 1 {
            // For default-positioned text, we cannot make any assumptions regarding
            // the first glyph position when the string has leading whitespace (to be stripped).
            if !txt.starts_with(' ') && !txt.starts_with('\t') {
                reporter_assert!(reporter, xpos[0] == offset.x);
            }
        } else {
            for i in 0..xpos_count {
                reporter_assert!(reporter, xpos[i] == f32::from(expected.as_bytes()[i]));
            }
        }
    }

    let y = dom.find_attr(text_elem, "y");
    reporter_assert!(reporter, y.is_some());
    if let Some(y) = y {
        let ypos_count = if scalars_per_pos < 2 { 1 } else { text_len };
        reporter_assert!(
            reporter,
            usize::try_from(parse::count(y.as_bytes())).ok() == Some(ypos_count)
        );

        let mut ypos: Vec<scalar> = vec![0.0; ypos_count];
        let _ = parse::find_scalars(y.as_bytes(), 0, &mut ypos);
        if scalars_per_pos < 2 {
            reporter_assert!(reporter, ypos[0] == offset.y);
        } else {
            for i in 0..ypos_count {
                reporter_assert!(
                    reporter,
                    ypos[i] == 150.0 - f32::from(expected.as_bytes()[i])
                );
            }
        }
    }
}

// Port of: tests/SVGDeviceTest.cpp#L144-L181 (chrome/m156)
fn test_whitespace_pos(reporter: &mut Reporter, txt: &str, expected: &str) {
    let len = txt.len();

    let mut dom = Dom::new();
    let paint = Paint::default();
    let font = default_portable_font();
    let offset = Point::new(10.0, 20.0);

    {
        make_dom_canvas(&mut dom, 0).draw_simple_text(
            txt.as_bytes(),
            TextEncoding::UTF8,
            (offset.x, offset.y),
            &font,
            &paint,
        );
    }
    let root = dom.finish_parsing();
    check_text_node(reporter, &dom, root, offset, 0, txt, expected);

    {
        let xpos: Vec<scalar> = (0..len).map(|i| f32::from(txt.as_bytes()[i])).collect();

        let blob = TextBlob::from_pos_text_h(
            txt.as_bytes(),
            TextEncoding::UTF8,
            &xpos,
            offset.y,
            &font,
        )
        .expect("a text blob");
        make_dom_canvas(&mut dom, 0).draw_text_blob(&blob, (0.0, 0.0), &paint);
    }
    let root = dom.finish_parsing();
    check_text_node(reporter, &dom, root, offset, 1, txt, expected);

    {
        let pos: Vec<Point> = (0..len)
            .map(|i| {
                let c = f32::from(txt.as_bytes()[i]);
                Point::new(c, 150.0 - c)
            })
            .collect();

        let blob = TextBlob::from_pos_text(txt.as_bytes(), TextEncoding::UTF8, &pos, &font)
            .expect("a text blob");
        make_dom_canvas(&mut dom, 0).draw_text_blob(&blob, (0.0, 0.0), &paint);
    }
    let root = dom.finish_parsing();
    check_text_node(reporter, &dom, root, offset, 2, txt, expected);
}

// Port of: tests/SVGDeviceTest.cpp#L185-L206 (chrome/m156)
def_test!(SVGDevice_whitespace_pos, |reporter| {
    struct Tests {
        tst_in: &'static str,
        tst_out: &'static str,
    }
    let tests = [
        Tests {
            tst_in: "abcd",
            tst_out: "abcd",
        },
        Tests {
            tst_in: "ab cd",
            tst_out: "ab cd",
        },
        Tests {
            tst_in: "ab \t\t cd",
            tst_out: "ab cd",
        },
        Tests {
            tst_in: " abcd",
            tst_out: "abcd",
        },
        Tests {
            tst_in: "  abcd",
            tst_out: "abcd",
        },
        Tests {
            tst_in: " \t\t abcd",
            tst_out: "abcd",
        },
        Tests {
            tst_in: "abcd ",
            tst_out: "abcd ",
        }, // we allow one trailing whitespace char
        Tests {
            tst_in: "abcd  ",
            tst_out: "abcd ",
        }, // because it makes no difference and
        Tests {
            tst_in: "abcd\t  ",
            tst_out: "abcd ",
        }, // simplifies the implementation
        Tests {
            tst_in: "\t\t  \t ab \t\t  \t cd \t\t   \t  ",
            tst_out: "ab cd ",
        },
    ];

    for t in &tests {
        test_whitespace_pos(reporter, t.tst_in, t.tst_out);
    }
});

// Port of: tests/SVGDeviceTest.cpp#L208-L212 (chrome/m156)
fn set_image_shader(
    paint: &mut Paint,
    image_width: i32,
    image_height: i32,
    x_tile: TileMode,
    y_tile: TileMode,
) {
    let mut surface = surfaces::raster(
        &ImageInfo::new_n32_premul((image_width, image_height), None),
        None,
        None,
    )
    .expect("a surface");
    paint.set_shader(
        surface
            .image_snapshot()
            .expect("an image")
            .to_shader((x_tile, y_tile), SamplingOptions::default(), None),
    );
}

// Attempt to find the three nodes on which we have expectations:
// the pattern node, the image within that pattern, and the rect which
// uses the pattern as a fill.
// returns false if not all nodes are found.
// Port of: tests/SVGDeviceTest.cpp#L214-L255 (chrome/m156)
fn find_image_shader_nodes(
    reporter: &mut Reporter,
    dom: Option<&Dom>,
    root: Option<Node>,
    pattern_out: &mut Option<Node>,
    image_out: &mut Option<Node>,
    rect_out: &mut Option<Node>,
) -> bool {
    let (Some(root), Some(dom)) = (root, dom) else {
        errorf!(reporter, "root element not found");
        return false;
    };

    let Some(rect) = dom.get_first_child(root, Some("rect")) else {
        errorf!(reporter, "rect not found");
        return false;
    };
    *rect_out = Some(rect);

    let Some(defs) = dom.get_first_child(root, Some("defs")) else {
        errorf!(reporter, "defs not found");
        return false;
    };

    let Some(pattern) = dom.get_first_child(defs, Some("pattern")) else {
        errorf!(reporter, "pattern not found");
        return false;
    };
    *pattern_out = Some(pattern);

    let Some(image) = dom.get_first_child(pattern, Some("image")) else {
        errorf!(reporter, "image not found");
        return false;
    };
    *image_out = Some(image);

    true
}

// Port of: tests/SVGDeviceTest.cpp#L257-L265 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++
fn image_shader_test_setup(
    dom: &mut Dom,
    paint: &mut Paint,
    image_width: i32,
    image_height: i32,
    rect_width: i32,
    rect_height: i32,
    x_tile: TileMode,
    y_tile: TileMode,
) {
    set_image_shader(paint, image_width, image_height, x_tile, y_tile);
    let svg_canvas = make_dom_canvas(dom, 0);

    let bounds = Rect::new(0.0, 0.0, rect_width as scalar, rect_height as scalar);
    svg_canvas.draw_rect(bounds, paint);
}

/// The attribute of a node that the test requires to be there (`dom.findAttr` of C++ would
/// return null, and `atoi` or `strcmp` of it would crash).
fn attr<'a>(dom: &'a Dom, node: Node, name: &str) -> &'a str {
    dom.find_attr(node, name).expect("an attribute")
}

// Port of: tests/SVGDeviceTest.cpp#L267-L290 (chrome/m156)
def_test!(SVGDevice_image_shader_norepeat, |reporter| {
    let mut dom = Dom::new();
    let mut paint = Paint::default();
    let image_width = 3;
    let image_height = 3;
    let rect_width = 10;
    let rect_height = 10;
    image_shader_test_setup(
        &mut dom,
        &mut paint,
        image_width,
        image_height,
        rect_width,
        rect_height,
        TileMode::Clamp,
        TileMode::Clamp,
    );

    let root = dom.finish_parsing();

    let (mut pattern_node, mut image_node, mut rect_node) = (None, None, None);
    let structure_appropriate = find_image_shader_nodes(
        reporter,
        Some(&dom),
        root,
        &mut pattern_node,
        &mut image_node,
        &mut rect_node,
    );
    reporter_assert!(reporter, structure_appropriate);
    let (pattern_node, image_node) = (pattern_node.expect("a pattern"), image_node.expect("an image"));

    // the image should always maintain its size.
    reporter_assert!(reporter, atoi(attr(&dom, image_node, "width")) == image_width);
    reporter_assert!(reporter, atoi(attr(&dom, image_node, "height")) == image_height);

    // making the pattern as large as the container prevents
    // it from repeating.
    reporter_assert!(reporter, attr(&dom, pattern_node, "width") == "100%");
    reporter_assert!(reporter, attr(&dom, pattern_node, "height") == "100%");
});

// Port of: tests/SVGDeviceTest.cpp#L292-L320 (chrome/m156)
def_test!(SVGDevice_image_shader_tilex, |reporter| {
    let mut dom = Dom::new();
    let mut paint = Paint::default();
    let image_width = 3;
    let image_height = 3;
    let rect_width = 10;
    let rect_height = 10;
    image_shader_test_setup(
        &mut dom,
        &mut paint,
        image_width,
        image_height,
        rect_width,
        rect_height,
        TileMode::Repeat,
        TileMode::Clamp,
    );

    let root = dom.finish_parsing();
    let inner_svg = root.and_then(|root| dom.get_first_child(root, Some("svg")));
    if inner_svg.is_none() {
        errorf!(reporter, "inner svg element not found");
        return;
    }

    let (mut pattern_node, mut image_node, mut rect_node) = (None, None, None);
    let structure_appropriate = find_image_shader_nodes(
        reporter,
        Some(&dom),
        inner_svg,
        &mut pattern_node,
        &mut image_node,
        &mut rect_node,
    );
    reporter_assert!(reporter, structure_appropriate);
    let (pattern_node, image_node) = (pattern_node.expect("a pattern"), image_node.expect("an image"));

    // the imageNode should always maintain its size.
    reporter_assert!(reporter, atoi(attr(&dom, image_node, "width")) == image_width);
    reporter_assert!(reporter, atoi(attr(&dom, image_node, "height")) == image_height);

    // if the patternNode width matches the imageNode width,
    // it will repeat in along the x axis.
    reporter_assert!(reporter, atoi(attr(&dom, pattern_node, "width")) == image_width);
    reporter_assert!(reporter, attr(&dom, pattern_node, "height") == "100%");
});

// Port of: tests/SVGDeviceTest.cpp#L322-L350 (chrome/m156)
def_test!(SVGDevice_image_shader_tiley, |reporter| {
    let mut dom = Dom::new();
    let mut paint = Paint::default();
    let image_node_width = 3;
    let image_node_height = 3;
    let rect_node_width = 10;
    let rect_node_height = 10;
    image_shader_test_setup(
        &mut dom,
        &mut paint,
        image_node_width,
        image_node_height,
        rect_node_width,
        rect_node_height,
        TileMode::Clamp,
        TileMode::Repeat,
    );

    let root = dom.finish_parsing();
    let inner_svg = root.and_then(|root| dom.get_first_child(root, Some("svg")));
    if inner_svg.is_none() {
        errorf!(reporter, "inner svg element not found");
        return;
    }

    let (mut pattern_node, mut image_node, mut rect_node) = (None, None, None);
    let structure_appropriate = find_image_shader_nodes(
        reporter,
        Some(&dom),
        inner_svg,
        &mut pattern_node,
        &mut image_node,
        &mut rect_node,
    );
    reporter_assert!(reporter, structure_appropriate);
    let (pattern_node, image_node) = (pattern_node.expect("a pattern"), image_node.expect("an image"));

    // the imageNode should always maintain its size.
    reporter_assert!(reporter, atoi(attr(&dom, image_node, "width")) == image_node_width);
    reporter_assert!(reporter, atoi(attr(&dom, image_node, "height")) == image_node_height);

    // making the patternNode as large as the container prevents
    // it from repeating.
    reporter_assert!(reporter, attr(&dom, pattern_node, "width") == "100%");
    reporter_assert!(reporter, atoi(attr(&dom, pattern_node, "height")) == image_node_height);
});

// Port of: tests/SVGDeviceTest.cpp#L352-L378 (chrome/m156)
def_test!(SVGDevice_image_shader_tileboth, |reporter| {
    let mut dom = Dom::new();
    let mut paint = Paint::default();
    let image_width = 3;
    let image_height = 3;
    let rect_width = 10;
    let rect_height = 10;
    image_shader_test_setup(
        &mut dom,
        &mut paint,
        image_width,
        image_height,
        rect_width,
        rect_height,
        TileMode::Repeat,
        TileMode::Repeat,
    );

    let root = dom.finish_parsing();

    let (mut pattern_node, mut image_node, mut rect_node) = (None, None, None);
    let inner_svg = root.and_then(|root| dom.get_first_child(root, Some("svg")));
    if inner_svg.is_none() {
        errorf!(reporter, "inner svg element not found");
        return;
    }
    let structure_appropriate = find_image_shader_nodes(
        reporter,
        Some(&dom),
        inner_svg,
        &mut pattern_node,
        &mut image_node,
        &mut rect_node,
    );
    reporter_assert!(reporter, structure_appropriate);
    let (pattern_node, image_node) = (pattern_node.expect("a pattern"), image_node.expect("an image"));

    // the imageNode should always maintain its size.
    reporter_assert!(reporter, atoi(attr(&dom, image_node, "width")) == image_width);
    reporter_assert!(reporter, atoi(attr(&dom, image_node, "height")) == image_height);

    reporter_assert!(reporter, atoi(attr(&dom, pattern_node, "width")) == image_width);
    reporter_assert!(reporter, atoi(attr(&dom, pattern_node, "height")) == image_height);
});

// Port of: tests/SVGDeviceTest.cpp#L380-L459 (chrome/m156)
def_test!(SVGDevice_ColorFilters, |reporter| {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum CompositeType {
        None,
        Composite,
        Blend,
    }
    use CompositeType::{Blend, Composite};

    let mut check_blendmode = |bm: BlendMode,
                               ct: CompositeType,
                               expected_operator: Option<&str>,
                               expected_in: Option<&str>,
                               expected_in2: Option<&str>| {
        // Using a non-opaque color to avoid Blend() no-op collapse/optimizations.
        const ALPHA: f32 = 0.5;
        let color = colors::RED.with_alpha(ALPHA);

        let mut paint = Paint::default();
        paint.set_color_filter(color_filters::blend_color(color.to_color(), bm));

        let mut dom = Dom::new();
        {
            let svg_canvas = make_dom_canvas(&mut dom, 0);
            let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
            svg_canvas.draw_rect(bounds, &paint);
        }
        let root_element = dom.finish_parsing();
        abort_test!(reporter, root_element.is_none(), "root element not found");
        let root_element = root_element.expect("a root element");

        let filter_element = dom.get_first_child(root_element, Some("filter"));
        abort_test!(reporter, filter_element.is_none(), "filter element not found");
        let filter_element = filter_element.expect("a filter element");
        reporter_assert!(reporter, attr(&dom, filter_element, "width") == "100%");
        reporter_assert!(reporter, attr(&dom, filter_element, "height") == "100%");

        let flood_element = dom.get_first_child(filter_element, Some("feFlood"));
        abort_test!(reporter, flood_element.is_none(), "feFlood element not found");
        let flood_element = flood_element.expect("a flood element");
        reporter_assert!(reporter, attr(&dom, flood_element, "flood-color") == "red");
        reporter_assert!(
            reporter,
            <scalar as Scalar>::nearly_equal(
                atof(attr(&dom, flood_element, "flood-opacity")) as f32,
                ALPHA,
                0.01
            )
        );

        if ct == CompositeType::None {
            return;
        }

        let expected_fe_element = if ct == Composite {
            "feComposite"
        } else {
            "feBlend"
        };
        let expected_operator_attr = if ct == Composite { "operator" } else { "mode" };

        let fe_element = dom.get_first_child(filter_element, Some(expected_fe_element));
        abort_test!(
            reporter,
            fe_element.is_none(),
            "{} element not found",
            expected_fe_element
        );
        let fe_element = fe_element.expect("an fe element");

        reporter_assert!(reporter, Some(attr(&dom, fe_element, "in")) == expected_in);
        reporter_assert!(reporter, Some(attr(&dom, fe_element, "in2")) == expected_in2);
        reporter_assert!(
            reporter,
            Some(attr(&dom, fe_element, expected_operator_attr)) == expected_operator
        );
    };

    check_blendmode(BlendMode::Src, CompositeType::None, None, None, None);

    check_blendmode(BlendMode::SrcOver, Composite, Some("over"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::SrcIn, Composite, Some("in"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::SrcOut, Composite, Some("out"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::SrcATop, Composite, Some("atop"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::DstOver, Composite, Some("over"), Some("SourceGraphic"), Some("flood"));
    check_blendmode(BlendMode::DstIn, Composite, Some("in"), Some("SourceGraphic"), Some("flood"));
    check_blendmode(BlendMode::DstOut, Composite, Some("out"), Some("SourceGraphic"), Some("flood"));
    check_blendmode(BlendMode::DstATop, Composite, Some("atop"), Some("SourceGraphic"), Some("flood"));
    check_blendmode(BlendMode::Xor, Composite, Some("xor"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::Plus, Composite, Some("arithmetic"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::Modulate, Composite, Some("arithmetic"), Some("flood"), Some("SourceGraphic"));

    check_blendmode(BlendMode::Multiply, Blend, Some("multiply"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::Screen, Blend, Some("screen"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::Darken, Blend, Some("darken"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::Lighten, Blend, Some("lighten"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::Overlay, Blend, Some("overlay"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::ColorDodge, Blend, Some("color-dodge"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::ColorBurn, Blend, Some("color-burn"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::HardLight, Blend, Some("hard-light"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::SoftLight, Blend, Some("soft-light"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::Difference, Blend, Some("difference"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::Exclusion, Blend, Some("exclusion"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::Hue, Blend, Some("hue"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::Saturation, Blend, Some("saturation"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::Color, Blend, Some("color"), Some("flood"), Some("SourceGraphic"));
    check_blendmode(BlendMode::Luminosity, Blend, Some("luminosity"), Some("flood"), Some("SourceGraphic"));
});

// Port of: tests/SVGDeviceTest.cpp#L461-L490 (chrome/m156)
def_test!(SVGDevice_textpath, |reporter| {
    let mut dom = Dom::new();
    let font = default_portable_font();
    let mut paint = Paint::default();

    let mut check_text = |reporter: &mut Reporter, paint: &Paint, flags: u32, expect_path: bool| {
        // By default, we emit <text> nodes.
        {
            let svg_canvas = make_dom_canvas(&mut dom, flags);
            svg_canvas.draw_str("foo", (100.0, 100.0), &font, paint);
        }
        let root_element = dom.finish_parsing();
        reporter_assert!(reporter, root_element.is_some(), "root element not found");
        let Some(root_element) = root_element else {
            return;
        };
        let text_element = dom.get_first_child(root_element, Some("text"));
        reporter_assert!(
            reporter,
            text_element.is_some() == !expect_path,
            "unexpected text element"
        );
        let path_element = dom.get_first_child(root_element, Some("path"));
        reporter_assert!(
            reporter,
            path_element.is_some() == expect_path,
            "unexpected path element"
        );
    };

    // By default, we emit <text> nodes.
    check_text(reporter, &paint, 0, false);

    // With kConvertTextToPaths_Flag, we emit <path> nodes.
    check_text(
        reporter,
        &paint,
        Flags::CONVERT_TEXT_TO_PATHS.bits(),
        true,
    );

    // We also use paths in the presence of path effects.
    let intervals: [scalar; 2] = [10.0, 5.0];
    paint.set_path_effect(dash_path_effect::new(&intervals, 0.0));
    check_text(reporter, &paint, 0, true);
});

// Port of: tests/SVGDeviceTest.cpp#L492-L530 (chrome/m156)
def_test!(SVGDevice_fill_stroke, |reporter| {
    struct Test {
        color: Color,
        style: Style,
        expected_fill: Option<&'static str>,
        expected_stroke: Option<&'static str>,
    }
    let g_tests = [
        Test {
            color: Color::BLACK,
            style: Style::Fill,
            expected_fill: None,
            expected_stroke: None,
        },
        Test {
            color: Color::BLACK,
            style: Style::Stroke,
            expected_fill: Some("none"),
            expected_stroke: Some("black"),
        },
        Test {
            color: Color::RED,
            style: Style::Fill,
            expected_fill: Some("red"),
            expected_stroke: None,
        },
        Test {
            color: Color::RED,
            style: Style::Stroke,
            expected_fill: Some("none"),
            expected_stroke: Some("red"),
        },
    ];

    for tst in &g_tests {
        let mut p = Paint::default();
        p.set_color(tst.color);
        p.set_style(tst.style);

        let mut dom = Dom::new();
        {
            make_dom_canvas(&mut dom, 0).draw_rect(Rect::from_wh(100.0, 100.0), &p);
        }

        let root = dom.finish_parsing();
        reporter_assert!(reporter, root.is_some(), "root element not found");
        let Some(root) = root else {
            continue;
        };
        let rect = dom.get_first_child(root, Some("rect"));
        reporter_assert!(reporter, rect.is_some(), "rect element not found");
        let Some(rect) = rect else {
            continue;
        };
        let fill = dom.find_attr(rect, "fill");
        reporter_assert!(reporter, fill.is_some() == tst.expected_fill.is_some());
        if let Some(fill) = fill {
            reporter_assert!(reporter, Some(fill) == tst.expected_fill);
        }
        let stroke = dom.find_attr(rect, "stroke");
        reporter_assert!(reporter, stroke.is_some() == tst.expected_stroke.is_some());
        if let Some(stroke) = stroke {
            reporter_assert!(reporter, Some(stroke) == tst.expected_stroke);
        }
    }
});

// Port of: tests/SVGDeviceTest.cpp#L532-L566 (chrome/m156)
def_test!(SVGDevice_fill_opacity_black_fill, |reporter| {
    struct Test {
        color: Color,
        expected_fill_opacity: Option<&'static str>,
    }
    let g_tests = [
        // Semi-transparent black
        Test {
            color: Color::from_argb(0x33, 0x00, 0x00, 0x00),
            expected_fill_opacity: Some("0.2"),
        },
        // Opaque black
        Test {
            color: Color::from_argb(0xFF, 0x00, 0x00, 0x00),
            expected_fill_opacity: None,
        },
    ];

    for tst in &g_tests {
        let mut p = Paint::default();
        p.set_color(tst.color);
        p.set_style(Style::Fill);

        let mut dom = Dom::new();
        {
            let svg_canvas = make_dom_canvas(&mut dom, 0);
            let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
            svg_canvas.draw_rect(bounds, &p);
        }

        let root_element = dom.finish_parsing();
        abort_test!(reporter, root_element.is_none(), "root element not found");
        let root_element = root_element.expect("a root element");

        let rect_element = dom.get_first_child(root_element, Some("rect"));
        abort_test!(reporter, rect_element.is_none(), "rect element not found");
        let rect_element = rect_element.expect("a rect element");
        let fill_opacity = dom.find_attr(rect_element, "fill-opacity");
        reporter_assert!(
            reporter,
            fill_opacity.is_some() == tst.expected_fill_opacity.is_some()
        );
        if let Some(fill_opacity) = fill_opacity {
            reporter_assert!(reporter, Some(fill_opacity) == tst.expected_fill_opacity);
        }
    }
});

// Port of: tests/SVGDeviceTest.cpp#L568-L583 (chrome/m156)
def_test!(SVGDevice_fill_rect_hex, |reporter| {
    let mut dom = Dom::new();
    let mut paint = Paint::default();
    paint.set_color(Color::BLUE);
    {
        let svg_canvas = make_dom_canvas(&mut dom, 0);
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        svg_canvas.draw_rect(bounds, &paint);
    }
    let root_element = dom.finish_parsing();
    abort_test!(reporter, root_element.is_none(), "root element not found");
    let root_element = root_element.expect("a root element");

    let rect_element = dom.get_first_child(root_element, Some("rect"));
    abort_test!(reporter, rect_element.is_none(), "rect element not found");
    let rect_element = rect_element.expect("a rect element");
    reporter_assert!(reporter, attr(&dom, rect_element, "fill") == "blue");
});

// Port of: tests/SVGDeviceTest.cpp#L585-L615 (chrome/m156)
def_test!(SVGDevice_fill_rect_custom_hex, |reporter| {
    let mut dom = Dom::new();
    {
        let mut paint = Paint::default();
        paint.set_color(Color::new(0xFFAA_BCDE));
        let svg_canvas = make_dom_canvas(&mut dom, 0);
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        svg_canvas.draw_rect(bounds, &paint);
        paint.set_color(Color::new(0xFFAA_BBCC));
        svg_canvas.draw_rect(bounds, &paint);
        paint.set_color(Color::new(0xFFAA_1123));
        svg_canvas.draw_rect(bounds, &paint);
    }
    let root_element = dom.finish_parsing();
    abort_test!(reporter, root_element.is_none(), "root element not found");
    let root_element = root_element.expect("a root element");

    // Test 0xAABCDE filled rect.
    let rect_element = dom.get_first_child(root_element, Some("rect"));
    abort_test!(reporter, rect_element.is_none(), "rect element not found");
    let rect_element = rect_element.expect("a rect element");
    reporter_assert!(reporter, attr(&dom, rect_element, "fill") == "#AABCDE");

    // Test 0xAABBCC filled rect.
    let rect_element = dom.get_next_sibling(rect_element, Some("rect"));
    abort_test!(reporter, rect_element.is_none(), "rect element not found");
    let rect_element = rect_element.expect("a rect element");
    reporter_assert!(reporter, attr(&dom, rect_element, "fill") == "#ABC");

    // Test 0xFFAA1123 filled rect. Make sure it does not turn into #A123.
    let rect_element = dom.get_next_sibling(rect_element, Some("rect"));
    abort_test!(reporter, rect_element.is_none(), "rect element not found");
    let rect_element = rect_element.expect("a rect element");
    reporter_assert!(reporter, attr(&dom, rect_element, "fill") == "#AA1123");
});

// Port of: tests/SVGDeviceTest.cpp#L617-L642 (chrome/m156)
def_test!(SVGDevice_fill_stroke_rect_hex, |reporter| {
    let mut dom = Dom::new();
    {
        let svg_canvas = make_dom_canvas(&mut dom, 0);
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);

        let mut paint = Paint::default();
        paint.set_color(Color::new(0xFF00_BBAC));
        svg_canvas.draw_rect(bounds, &paint);
        paint.set_style(Style::Stroke);
        paint.set_color(Color::new(0xFF12_3456));
        paint.set_stroke_width(1.0);
        svg_canvas.draw_rect(bounds, &paint);
    }
    let root_element = dom.finish_parsing();
    abort_test!(reporter, root_element.is_none(), "root element not found");
    let root_element = root_element.expect("a root element");

    let rect_node = dom.get_first_child(root_element, Some("rect"));
    abort_test!(reporter, rect_node.is_none(), "rect element not found");
    let rect_node = rect_node.expect("a rect element");
    reporter_assert!(reporter, attr(&dom, rect_node, "fill") == "#00BBAC");

    let rect_node = dom.get_next_sibling(rect_node, Some("rect"));
    abort_test!(reporter, rect_node.is_none(), "rect element not found");
    let rect_node = rect_node.expect("a rect element");
    reporter_assert!(reporter, attr(&dom, rect_node, "stroke") == "#123456");
    reporter_assert!(reporter, attr(&dom, rect_node, "stroke-width") == "1");
});

// Port of: tests/SVGDeviceTest.cpp#L644-L662 (chrome/m156)
def_test!(SVGDevice_rect_with_path_effect, |reporter| {
    let mut dom = Dom::new();

    let intervals: [scalar; 2] = [0.0, 20.0];
    let path_effect = dash_path_effect::new(&intervals, 0.0);

    let mut paint = Paint::default();
    paint.set_path_effect(path_effect);

    {
        let svg_canvas = make_dom_canvas(&mut dom, 0);
        svg_canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 100.0, 100.0), &paint);
    }

    let root_element = dom.finish_parsing();
    reporter_assert!(reporter, root_element.is_some(), "root element not found");
    let Some(root_element) = root_element else {
        return;
    };
    let path_element = dom.get_first_child(root_element, Some("path"));
    reporter_assert!(reporter, path_element.is_some(), "path element not found");
});

// Port of: tests/SVGDeviceTest.cpp#L664-L682 (chrome/m156)
def_test!(SVGDevice_rrect_with_path_effect, |reporter| {
    let mut dom = Dom::new();

    let intervals: [scalar; 2] = [0.0, 20.0];
    let path_effect = dash_path_effect::new(&intervals, 0.0);

    let mut paint = Paint::default();
    paint.set_path_effect(path_effect);

    {
        let svg_canvas = make_dom_canvas(&mut dom, 0);
        svg_canvas.draw_rrect(
            RRect::new_rect_xy(Rect::from_xywh(0.0, 0.0, 100.0, 100.0), 10.0, 10.0),
            &paint,
        );
    }

    let root_element = dom.finish_parsing();
    reporter_assert!(reporter, root_element.is_some(), "root element not found");
    let Some(root_element) = root_element else {
        return;
    };
    let path_element = dom.get_first_child(root_element, Some("path"));
    reporter_assert!(reporter, path_element.is_some(), "path element not found");
});

// Port of: tests/SVGDeviceTest.cpp#L684-L702 (chrome/m156)
def_test!(SVGDevice_oval_with_path_effect, |reporter| {
    let mut dom = Dom::new();

    let intervals: [scalar; 2] = [0.0, 20.0];
    let path_effect = dash_path_effect::new(&intervals, 0.0);

    let mut paint = Paint::default();
    paint.set_path_effect(path_effect);

    {
        let svg_canvas = make_dom_canvas(&mut dom, 0);
        svg_canvas.draw_oval(Rect::from_xywh(0.0, 0.0, 100.0, 100.0), &paint);
    }

    let root_element = dom.finish_parsing();
    reporter_assert!(reporter, root_element.is_some(), "root element not found");
    let Some(root_element) = root_element else {
        return;
    };
    let path_element = dom.get_first_child(root_element, Some("path"));
    reporter_assert!(reporter, path_element.is_some(), "path element not found");
});

// Port of: tests/SVGDeviceTest.cpp#L705-L741 (chrome/m156)
def_test!(SVGDevice_path_effect, |reporter| {
    let mut dom = Dom::new();

    let mut paint = Paint::default();
    paint.set_color(Color::RED);
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(10.0);
    paint.set_stroke_cap(Cap::Round);

    // Produces a line of three red dots.
    let intervals: [scalar; 2] = [0.0, 20.0];
    let path_effect = dash_path_effect::new(&intervals, 0.0);
    paint.set_path_effect(path_effect);
    let points = [
        Point::new(50.0, 15.0),
        Point::new(100.0, 15.0),
        Point::new(150.0, 15.0),
    ];
    {
        let svg_canvas = make_dom_canvas(&mut dom, 0);
        svg_canvas.draw_points(PointMode::Lines, &points, &paint);
    }
    let root_element = dom.finish_parsing();
    reporter_assert!(reporter, root_element.is_some(), "root element not found");
    let Some(root_element) = root_element else {
        return;
    };
    let path_element = dom.get_first_child(root_element, Some("path"));
    reporter_assert!(reporter, path_element.is_some(), "path element not found");
    let Some(path_element) = path_element else {
        return;
    };

    // The SVG path to draw the three dots is a complex list of instructions.
    // To avoid test brittleness, we don't attempt to match the entire path.
    // Instead, we simply confirm there are three (M)ove instructions, one per
    // dot.  If path effects were not being honored, we would expect only one
    // Move instruction, to the starting position, before drawing a continuous
    // straight line.
    let d = attr(&dom, path_element, "d");
    let mut m_count = 0;
    for pos in d.bytes() {
        m_count += i32::from(pos == b'M');
    }
    reporter_assert!(reporter, m_count == 3);
});

// Port of: tests/SVGDeviceTest.cpp#L743-L763 (chrome/m156)
def_test!(SVGDevice_relative_path_encoding, |reporter| {
    let mut dom = Dom::new();
    {
        let svg_canvas = make_dom_canvas(&mut dom, Flags::RELATIVE_PATH_ENCODING.bits());
        let path = PathBuilder::new()
            .move_to((100.0, 50.0))
            .line_to((200.0, 50.0))
            .line_to((200.0, 150.0))
            .close()
            .detach();

        svg_canvas.draw_path(&path, &Paint::default());
    }

    let root_element = dom.finish_parsing();
    reporter_assert!(reporter, root_element.is_some(), "root element not found");
    let Some(root_element) = root_element else {
        return;
    };
    let path_element = dom.get_first_child(root_element, Some("path"));
    reporter_assert!(reporter, path_element.is_some(), "path element not found");
    let Some(path_element) = path_element else {
        return;
    };
    let d = attr(&dom, path_element, "d");
    reporter_assert!(reporter, d == "m100 50l100 0l0 100l-100 -100Z");
});

// Port of: tests/SVGDeviceTest.cpp#L765-L783 (chrome/m156)
def_test!(SVGDevice_color_shader, |reporter| {
    let mut dom = Dom::new();
    {
        let svg_canvas = make_dom_canvas(&mut dom, 0);

        let mut paint = Paint::default();
        paint.set_shader(shaders::color(Color::new(0xffff_ff00)));

        svg_canvas.draw_circle((100.0, 100.0), 100.0, &paint);
    }

    let root_element = dom.finish_parsing();
    reporter_assert!(reporter, root_element.is_some(), "root element not found");
    let Some(root_element) = root_element else {
        return;
    };
    let ellipse_element = dom.get_first_child(root_element, Some("ellipse"));
    reporter_assert!(reporter, ellipse_element.is_some(), "ellipse element not found");
    let Some(ellipse_element) = ellipse_element else {
        return;
    };
    let fill = dom.find_attr(ellipse_element, "fill");
    reporter_assert!(reporter, fill.is_some(), "fill attribute not found");
    let Some(fill) = fill else {
        return;
    };
    reporter_assert!(reporter, fill == "yellow");
});

// Port of: tests/SVGDeviceTest.cpp#L785-L800 (chrome/m156)
def_test!(SVGDevice_parse_minmax, |reporter| {
    let mut check = |n: i64, expected: bool| {
        let s = n.to_string();

        let parsed = parse::find_s32(s.as_bytes(), 0);
        reporter_assert!(reporter, parsed.is_some() == expected);
        if expected {
            reporter_assert!(reporter, parsed.map(|(_, val)| i64::from(val)) == Some(n));
        }
    };

    check(i64::from(i32::MAX), true);
    check(i64::from(i32::MIN), true);
    check(i64::from(i32::MAX) + 1, false);
    check(i64::from(i32::MIN) - 1, false);
});
