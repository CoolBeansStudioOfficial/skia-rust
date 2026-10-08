// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/VerticesTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::vertices::{Builder, BuilderFlags, VertexMode, Vertices, vertices_priv};
use skia_rust_core::write_buffer::BinaryWriteBuffer;

use skia_rust_raster::surfaces;

use crate::tools::tool_utils::PixelIter;
use crate::{Reporter, def_test, def_tier_test, reporter_assert};

// Port of: tests/VerticesTest.cpp#L25-L66 (chrome/m156)
fn equal(vert0: &Vertices, vert1: &Vertices) -> bool {
    if vertices_priv::mode(vert0) != vertices_priv::mode(vert1) {
        return false;
    }
    if vertices_priv::vertex_count(vert0) != vertices_priv::vertex_count(vert1) {
        return false;
    }
    if vertices_priv::index_count(vert0) != vertices_priv::index_count(vert1) {
        return false;
    }

    if vertices_priv::tex_coords(vert0).is_some() != vertices_priv::tex_coords(vert1).is_some() {
        return false;
    }
    if vertices_priv::colors(vert0).is_some() != vertices_priv::colors(vert1).is_some() {
        return false;
    }

    for i in 0..vertices_priv::vertex_count(vert0) {
        if vertices_priv::positions(vert0)[i] != vertices_priv::positions(vert1)[i] {
            return false;
        }
        if let Some(texs) = vertices_priv::tex_coords(vert0)
            && texs[i] != vertices_priv::tex_coords(vert1).expect("checked above")[i]
        {
            return false;
        }
        if let Some(colors) = vertices_priv::colors(vert0)
            && colors[i] != vertices_priv::colors(vert1).expect("checked above")[i]
        {
            return false;
        }
    }
    for i in 0..vertices_priv::index_count(vert0) {
        if vertices_priv::indices(vert0).expect("indexed")[i]
            != vertices_priv::indices(vert1).expect("indexed")[i]
        {
            return false;
        }
    }
    true
}

// Port of: tests/VerticesTest.cpp#L68-L84 (chrome/m156)
fn self_test(v0: &Vertices, reporter: &mut Reporter) {
    let mut writer = BinaryWriteBuffer::new();
    vertices_priv::encode(v0, &mut writer);

    let mut buf = vec![0u8; writer.bytes_written()];
    writer.write_to_memory(&mut buf);
    let mut reader = ReadBuffer::new(&buf);

    let v1 = vertices_priv::decode(&mut reader);

    reporter_assert!(reporter, v1.is_some());
    reporter_assert!(reporter, v0.unique_id() != 0);
    let Some(v1) = v1 else {
        return;
    };
    reporter_assert!(reporter, v1.unique_id() != 0);
    reporter_assert!(reporter, v0.unique_id() != v1.unique_id());
    reporter_assert!(reporter, equal(v0, &v1));
}

// Port of: tests/VerticesTest.cpp#L86-L147 (chrome/m156)
def_test!(Vertices, |reporter| {
    let v_count = 5;
    let i_count = 9; // odd value exercises padding logic in encode()

    // color-tex tests
    let tex_flags = [BuilderFlags::empty(), BuilderFlags::HAS_TEX_COORDS];
    let col_flags = [BuilderFlags::empty(), BuilderFlags::HAS_COLORS];
    for tex_f in tex_flags {
        for col_f in col_flags {
            let flags = tex_f | col_f;

            let mut builder = Builder::new(VertexMode::Triangles, v_count, i_count, flags);

            for i in 0..v_count {
                #[allow(clippy::cast_precision_loss)] // mirrors (float)i
                let x = i as f32;
                builder.positions()[i].set(x, 1.0);
                if let Some(texs) = builder.tex_coords() {
                    texs[i].set(x, 2.0);
                }
                if let Some(colors) = builder.colors() {
                    #[allow(clippy::cast_possible_truncation)]
                    // mirrors SkColorSetARGB(0xFF, i, ..)
                    let green = i as u8;
                    colors[i] = Color::from_argb(0xFF, green, 0x80, 0);
                }
            }
            for i in 0..i_count {
                #[allow(clippy::cast_possible_truncation)] // i % vCount < 5
                let index = (i % v_count) as u16;
                builder.indices().expect("indexed")[i] = index;
            }
            self_test(&builder.detach().expect("valid builder"), reporter);
        }
    }

    {
        // This has the maximum number of vertices to be rewritten as indexed triangles without
        // overflowing a 16bit index.
        let builder = Builder::new(
            VertexMode::TriangleFan,
            usize::from(u16::MAX) + 1,
            0,
            BuilderFlags::HAS_COLORS,
        );
        reporter_assert!(reporter, builder.is_valid());
    }
    {
        // This has too many to be rewritten.
        let builder = Builder::new(
            VertexMode::TriangleFan,
            usize::from(u16::MAX) + 2,
            0,
            BuilderFlags::HAS_COLORS,
        );
        reporter_assert!(reporter, !builder.is_valid());
    }
    {
        // Only two vertices - can't be rewritten.
        let builder = Builder::new(VertexMode::TriangleFan, 2, 0, BuilderFlags::HAS_COLORS);
        reporter_assert!(reporter, !builder.is_valid());
    }
    {
        // Minimum number of indices to be rewritten.
        let builder = Builder::new(VertexMode::TriangleFan, 10, 3, BuilderFlags::HAS_COLORS);
        reporter_assert!(reporter, builder.is_valid());
    }
    {
        // Too few indices to be rewritten.
        let builder = Builder::new(VertexMode::TriangleFan, 10, 2, BuilderFlags::HAS_COLORS);
        reporter_assert!(reporter, !builder.is_valid());
    }
});

// Port of: tests/VerticesTest.cpp#L149-L153 (chrome/m156)
fn fill_triangle(canvas: &Canvas, pts: &[Point; 3], c: Color) {
    let colors = [c, c, c];
    let verts = Vertices::new_copy(VertexMode::Triangles, pts, None, Some(&colors), None);
    canvas.draw_vertices(
        &verts.expect("three vertices"),
        BlendMode::Src,
        &Paint::default(),
    );
}

// Port of: tests/VerticesTest.cpp#L155-L174 (chrome/m156)
def_tier_test!(Vertices_clipping, |reporter| {
    // A very large triangle has to be geometrically clipped (since its "fast" clipping is
    // normally done in after building SkFixed coordinates). Check that we handle this.
    // (and don't assert).
    let mut surf = surfaces::raster_n32_premul((3, 3)).expect("a 3x3 surface");

    let pts = [
        Point::new(-10.0, 1.0),
        Point::new(-10.0, 2.0),
        Point::new(1e9, 1.5),
    ];
    fill_triangle(surf.canvas(), &pts, Color::BLACK);

    let peek = surf.peek_pixels().expect("raster pixels");
    let pm = peek.pixmap();
    let mut iter = PixelIter::new(&pm);
    while let Some(loc) = iter.next() {
        let c = pm.addr32(loc.x, loc.y);
        if loc.y == 1 {
            reporter_assert!(reporter, c == 0xFF00_0000);
        } else {
            reporter_assert!(reporter, c == 0);
        }
    }
});

// Port of: tests/VerticesTest.cpp#L176-L213 (chrome/m156)
def_tier_test!(Vertices_invalid, |reporter| {
    let mut surf = surfaces::raster_n32_premul((10, 10)).expect("a 10x10 surface");

    let verts = [
        Point::new(0.0, 0.0),
        Point::new(1.0, 0.0),
        Point::new(1.0, 1.0),
    ];
    let colors = [Color::RED, Color::GREEN, Color::BLUE];
    let indices: [u16; 3] = [0, 1, 20000];

    {
        let vertices = Vertices::new_copy(
            VertexMode::Triangles,
            &verts[..0],
            None,
            Some(&colors),
            None,
        );
        reporter_assert!(reporter, vertices.is_none());

        // should not crash
        // skia-rust: not expressible in Rust: `drawVertices(nullptr, ...)` (`draw_vertices`
        // takes a `&Vertices`, so there is no null to pass).
    }

    {
        let vertices = Vertices::new_copy(
            VertexMode::Triangles,
            &verts,
            None,
            Some(&colors),
            Some(&indices[..1]),
        );
        // reflects current behavior
        // TODO: should we reject invalid index counts?
        reporter_assert!(reporter, vertices.is_some());

        // should not crash
        surf.canvas().draw_vertices(
            &vertices.expect("checked above"),
            BlendMode::SrcOver,
            &Paint::default(),
        );
    }

    {
        let vertices = Vertices::new_copy(
            VertexMode::Triangles,
            &verts,
            None,
            Some(&colors),
            Some(&indices),
        );
        reporter_assert!(reporter, vertices.is_some());

        // should not crash
        surf.canvas().draw_vertices(
            &vertices.expect("checked above"),
            BlendMode::SrcOver,
            &Paint::default(),
        );
    }
});
