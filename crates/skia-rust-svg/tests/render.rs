// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! End to end tests of the SVG DOM core: parse a document, render it into a raster surface and
//! look at pixels.

use skia_rust_core::color::Color;
use skia_rust_core::size::Size;
use skia_rust_raster::surfaces;
use skia_rust_svg::{Dom, Tag};

fn render(svg: &str, w: i32, h: i32) -> Vec<Color> {
    let dom = Dom::from_str(svg).expect("a valid SVG document");
    let mut surface = surfaces::raster_n32_premul((w, h)).unwrap();
    dom.render(surface.canvas());
    let pixels = surface.peek_pixels().unwrap();
    let pixmap = pixels.pixmap();
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            out.push(pixmap.get_color((x, y)));
        }
    }
    out
}

fn at(pixels: &[Color], w: i32, x: i32, y: i32) -> Color {
    pixels[usize::try_from(y * w + x).unwrap()]
}

#[test]
fn rejects_what_is_not_an_svg_document() {
    assert!(Dom::from_str("").is_err());
    assert!(Dom::from_str("<html/>").is_err());
    assert!(Dom::from_str("<svg>").is_err());
    assert!(Dom::from_str("<svg/>").is_ok());
}

#[test]
fn container_size_is_the_intrinsic_size() {
    let dom = Dom::from_str(r#"<svg width="40" height="30"/>"#).unwrap();
    assert_eq!(*dom.container_size(), Size::new(40.0, 30.0));

    // Relative dimensions give no intrinsic size.
    let dom = Dom::from_str(r#"<svg width="50%" height="30"/>"#).unwrap();
    assert_eq!(*dom.container_size(), Size::new(0.0, 0.0));

    let mut dom = dom;
    dom.set_container_size(Size::new(10.0, 20.0));
    assert_eq!(*dom.container_size(), Size::new(10.0, 20.0));
}

#[test]
fn fills_a_rect() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20">
        <rect x="2" y="2" width="10" height="10" fill="#ff0000"/>
    </svg>"##;
    let px = render(svg, 20, 20);
    assert_eq!(at(&px, 20, 5, 5), Color::from_argb(0xff, 0xff, 0, 0));
    assert_eq!(at(&px, 20, 15, 15), Color::from_argb(0, 0, 0, 0));
    assert_eq!(at(&px, 20, 0, 0), Color::from_argb(0, 0, 0, 0));
}

#[test]
fn style_attribute_and_inheritance() {
    // fill comes from the group's style, the circle overrides stroke only.
    let svg = r#"<svg width="20" height="20">
        <g style="fill: #00ff00; stroke:none">
            <circle cx="10" cy="10" r="6"/>
        </g>
    </svg>"#;
    let px = render(svg, 20, 20);
    assert_eq!(at(&px, 20, 10, 10), Color::from_argb(0xff, 0, 0xff, 0));
    assert_eq!(at(&px, 20, 0, 0), Color::from_argb(0, 0, 0, 0));
}

#[test]
fn transform_and_view_box() {
    // The view box maps 10x10 user units to the 20x20 viewport.
    let svg = r#"<svg width="20" height="20" viewBox="0 0 10 10">
        <rect width="5" height="5" fill="blue" transform="translate(5,5)"/>
    </svg>"#;
    let px = render(svg, 20, 20);
    assert_eq!(at(&px, 20, 15, 15), Color::from_argb(0xff, 0, 0, 0xff));
    assert_eq!(at(&px, 20, 5, 5), Color::from_argb(0, 0, 0, 0));
}

#[test]
fn use_renders_the_referenced_node_and_defs_do_not_render() {
    let svg = r##"<svg width="20" height="20" xmlns:xlink="http://www.w3.org/1999/xlink">
        <defs><rect id="r" width="4" height="4" fill="red"/></defs>
        <use xlink:href="#r" x="10" y="10"/>
    </svg>"##;
    let px = render(svg, 20, 20);
    assert_eq!(at(&px, 20, 2, 2), Color::from_argb(0, 0, 0, 0));
    assert_eq!(at(&px, 20, 11, 11), Color::from_argb(0xff, 0xff, 0, 0));
}

#[test]
fn use_cycles_terminate() {
    let svg = r##"<svg width="20" height="20">
        <g id="a"><use xlink:href="#a"/><rect width="4" height="4"/></g>
    </svg>"##;
    let px = render(svg, 20, 20);
    assert_eq!(at(&px, 20, 1, 1), Color::from_argb(0xff, 0, 0, 0));
}

#[test]
fn finds_nodes_by_id() {
    let dom = Dom::from_str(r#"<svg><g id="x"><path id="p" d="M0 0L1 1"/></g></svg>"#).unwrap();
    assert_eq!(dom.find_node_by_id("x").unwrap().tag(), Tag::G);
    assert_eq!(dom.find_node_by_id("p").unwrap().tag(), Tag::Path);
    assert!(dom.find_node_by_id("nope").is_none());
}

#[test]
fn opacity_and_display() {
    let svg = r##"<svg width="20" height="20">
        <rect width="10" height="10" fill="#000000" opacity="0.5"/>
        <rect x="10" y="10" width="10" height="10" display="none"/>
        <rect x="10" width="10" height="10" visibility="hidden"/>
    </svg>"##;
    let px = render(svg, 20, 20);
    let half = at(&px, 20, 5, 5);
    assert_eq!(half.r(), 0);
    assert!(half.a() == 127 || half.a() == 128, "alpha {}", half.a());
    assert_eq!(at(&px, 20, 15, 15), Color::from_argb(0, 0, 0, 0));
    assert_eq!(at(&px, 20, 15, 5), Color::from_argb(0, 0, 0, 0));
}

#[test]
fn polygon_polyline_line_ellipse_and_path() {
    let svg = r##"<svg width="40" height="40">
        <polygon points="0,0 10,0 10,10 0,10" fill="#010203"/>
        <ellipse cx="25" cy="5" rx="4" ry="4" fill="#040506"/>
        <path d="M0 20 h10 v10 h-10 z" fill="#070809"/>
        <line x1="20" y1="25" x2="30" y2="25" stroke="#0a0b0c" stroke-width="4"/>
        <polyline points="0,35 10,35 10,39" fill="none" stroke="#0d0e0f"/>
    </svg>"##;
    let px = render(svg, 40, 40);
    assert_eq!(at(&px, 40, 5, 5), Color::from_argb(0xff, 1, 2, 3));
    assert_eq!(at(&px, 40, 25, 5), Color::from_argb(0xff, 4, 5, 6));
    assert_eq!(at(&px, 40, 5, 25), Color::from_argb(0xff, 7, 8, 9));
    assert_eq!(at(&px, 40, 25, 25), Color::from_argb(0xff, 10, 11, 12));
}

#[test]
fn renders_the_smile_glyph() {
    let svg = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../third_party/skia/resources/fonts/svg/smile.svg"
    ))
    .unwrap();
    let px = render(&svg, 800, 800);
    let painted = px.iter().filter(|c| c.a() != 0).count();
    assert!(painted > 100_000, "{painted}");
    // The face is yellow-ish.
    let yellow = px
        .iter()
        .filter(|c| c.r() > 200 && c.g() > 150 && c.b() < 100)
        .count();
    assert!(yellow > 50_000, "{yellow}");
}
