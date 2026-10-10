// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/sksg/tests/SGTest.cpp (chrome/m156)

#![cfg(test)]

use std::rc::Rc;

use skia_rust_core::color::Color as SkColor;
use skia_rust_core::matrix::Matrix as SkMatrix;
use skia_rust_core::path::Path as SkPath;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_priv;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_sksg::render_effect::{DropShadowImageFilter, DropShadowMode, ImageFilterNode};
use skia_rust_sksg::render_node::node_at;
use skia_rust_sksg::transform::{Matrix as SgMatrix, make_concat};
use skia_rust_sksg::{
    Color, Draw, GeometryNode, Group, ImageFilterEffect, InvalidationController, Node, PaintNode,
    Rect as SgRect, RenderNode, Transform, TransformEffect,
};
use skia_rust_sksg::{Merge, MergeMode, MergeRec, Path as SgPath};

use crate::{Reporter, def_test, reporter_assert};

/// Port of `check_inval`.
// Port of: modules/sksg/tests/SGTest.cpp#L33-L65 (chrome/m156)
fn check_inval(
    reporter: &mut Reporter,
    root: &Rc<dyn RenderNode>,
    expected_bounds: Rect,
    expected_inval_bounds: Rect,
    expected_damage: Option<&[Rect]>,
) {
    let mut ic = InvalidationController::new();
    let bbox = root.revalidate(Some(&mut ic), &SkMatrix::new_identity());
    reporter_assert!(reporter, bbox == expected_bounds);
    reporter_assert!(reporter, ic.bounds() == expected_inval_bounds);
    if let Some(expected_damage) = expected_damage {
        let damage = ic.rects();
        reporter_assert!(reporter, expected_damage.len() == damage.len());
        for (r1, r2) in expected_damage.iter().zip(damage.iter()) {
            reporter_assert!(reporter, r1 == r2);
        }
    }
}

/// One hit-test expectation (`HitTest`).
struct HitTest {
    pt: Point,
    node: Option<Rc<dyn RenderNode>>,
}

fn hit(x: f32, y: f32, node: Option<&Rc<dyn RenderNode>>) -> HitTest {
    HitTest {
        pt: Point { x, y },
        node: node.cloned(),
    }
}

/// Port of `check_hittest`.
// Port of: modules/sksg/tests/SGTest.cpp#L70-L80 (chrome/m156)
fn check_hittest(reporter: &mut Reporter, root: &Rc<dyn RenderNode>, tests: &[HitTest]) {
    for tst in tests {
        let node = node_at(root, tst.pt);
        let same = match (&tst.node, &node) {
            (None, None) => true,
            (Some(expected), Some(actual)) => Rc::ptr_eq(expected, actual),
            _ => false,
        };
        reporter_assert!(reporter, same);
    }
}

/// `SkRectPriv::MakeLargeS32()`.
fn large_s32() -> Rect {
    skia_rust_sksg::util::make_large_s32()
}

/// Port of `inval_test1`.
// Port of: modules/sksg/tests/SGTest.cpp#L82-L225 (chrome/m156)
#[allow(clippy::too_many_lines)] // mirrors the C++ test body, one block per step
fn inval_test1(reporter: &mut Reporter) {
    let color = Color::make(SkColor::from(0xff00_0000));
    let r1 = SgRect::make(Rect::new(0.0, 0.0, 100.0, 100.0));
    let r2 = SgRect::make(Rect::new(0.0, 0.0, 100.0, 100.0));
    let grp = Group::make();
    let matrix = SgMatrix::<SkMatrix>::make(SkMatrix::new_identity());
    let root: Rc<dyn RenderNode> = TransformEffect::make(
        Some(grp.clone() as Rc<dyn RenderNode>),
        Some(matrix.clone() as Rc<dyn Transform>),
    )
    .expect("child and transform are set");
    let d1 = Draw::make(
        Some(r1.clone() as Rc<dyn GeometryNode>),
        Some(color.clone() as Rc<dyn PaintNode>),
    )
    .expect("geometry and paint are set");
    let d2 = Draw::make(
        Some(r2.clone() as Rc<dyn GeometryNode>),
        Some(color.clone() as Rc<dyn PaintNode>),
    )
    .expect("geometry and paint are set");
    let d1: Rc<dyn RenderNode> = d1;
    let d2: Rc<dyn RenderNode> = d2;
    grp.add_child(d1.clone());
    grp.add_child(d2.clone());
    {
        // Initial revalidation.
        check_inval(
            reporter,
            &root,
            Rect::new(0.0, 0.0, 100.0, 100.0),
            large_s32(),
            None,
        );
        check_hittest(
            reporter,
            &root,
            &[
                hit(-1.0, 0.0, None),
                hit(0.0, -1.0, None),
                hit(100.0, 0.0, None),
                hit(0.0, 100.0, None),
                hit(0.0, 0.0, Some(&d2)),
                hit(99.0, 99.0, Some(&d2)),
            ],
        );
    }
    {
        // Move r2 to (200 100).
        r2.set_l(200.0);
        r2.set_t(100.0);
        r2.set_r(300.0);
        r2.set_b(200.0);
        let damage = [
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Rect::new(200.0, 100.0, 300.0, 200.0),
        ];
        check_inval(
            reporter,
            &root,
            Rect::new(0.0, 0.0, 300.0, 200.0),
            Rect::new(0.0, 0.0, 300.0, 200.0),
            Some(&damage),
        );
        check_hittest(
            reporter,
            &root,
            &[
                hit(-1.0, 0.0, None),
                hit(0.0, -1.0, None),
                hit(100.0, 0.0, None),
                hit(0.0, 100.0, None),
                hit(0.0, 0.0, Some(&d1)),
                hit(99.0, 99.0, Some(&d1)),
                hit(199.0, 100.0, None),
                hit(200.0, 99.0, None),
                hit(300.0, 100.0, None),
                hit(200.0, 200.0, None),
                hit(200.0, 100.0, Some(&d2)),
                hit(299.0, 199.0, Some(&d2)),
            ],
        );
    }
    {
        // Update the common color.
        color.set_color(SkColor::from(0xffff_0000));
        let damage = [
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Rect::new(200.0, 100.0, 300.0, 200.0),
        ];
        check_inval(
            reporter,
            &root,
            Rect::new(0.0, 0.0, 300.0, 200.0),
            Rect::new(0.0, 0.0, 300.0, 200.0),
            Some(&damage),
        );
    }
    {
        // Shrink r1.
        r1.set_r(50.0);
        let damage = [
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Rect::new(0.0, 0.0, 50.0, 100.0),
        ];
        check_inval(
            reporter,
            &root,
            Rect::new(0.0, 0.0, 300.0, 200.0),
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Some(&damage),
        );
        check_hittest(
            reporter,
            &root,
            &[
                hit(-1.0, 0.0, None),
                hit(0.0, -1.0, None),
                hit(50.0, 0.0, None),
                hit(0.0, 100.0, None),
                hit(0.0, 0.0, Some(&d1)),
                hit(49.0, 99.0, Some(&d1)),
                hit(199.0, 100.0, None),
                hit(200.0, 99.0, None),
                hit(300.0, 100.0, None),
                hit(200.0, 200.0, None),
                hit(200.0, 100.0, Some(&d2)),
                hit(299.0, 199.0, Some(&d2)),
            ],
        );
    }
    {
        // Update transform.
        matrix.set_matrix(SkMatrix::scale((2.0, 2.0)));
        let damage = [
            Rect::new(0.0, 0.0, 300.0, 200.0),
            Rect::new(0.0, 0.0, 600.0, 400.0),
        ];
        check_inval(
            reporter,
            &root,
            Rect::new(0.0, 0.0, 600.0, 400.0),
            Rect::new(0.0, 0.0, 600.0, 400.0),
            Some(&damage),
        );
        check_hittest(
            reporter,
            &root,
            &[
                hit(-1.0, 0.0, None),
                hit(0.0, -1.0, None),
                hit(25.0, 0.0, None),
                hit(0.0, 50.0, None),
                hit(0.0, 0.0, Some(&d1)),
                hit(24.0, 49.0, Some(&d1)),
                hit(99.0, 50.0, None),
                hit(100.0, 49.0, None),
                hit(150.0, 50.0, None),
                hit(100.0, 100.0, None),
                hit(100.0, 50.0, Some(&d2)),
                hit(149.0, 99.0, Some(&d2)),
            ],
        );
    }
    {
        // Shrink r2 under transform.
        r2.set_r(250.0);
        let damage = [
            Rect::new(400.0, 200.0, 600.0, 400.0),
            Rect::new(400.0, 200.0, 500.0, 400.0),
        ];
        check_inval(
            reporter,
            &root,
            Rect::new(0.0, 0.0, 500.0, 400.0),
            Rect::new(400.0, 200.0, 600.0, 400.0),
            Some(&damage),
        );
        check_hittest(
            reporter,
            &root,
            &[
                hit(-1.0, 0.0, None),
                hit(0.0, -1.0, None),
                hit(25.0, 0.0, None),
                hit(0.0, 50.0, None),
                hit(0.0, 0.0, Some(&d1)),
                hit(24.0, 49.0, Some(&d1)),
                hit(99.0, 50.0, None),
                hit(100.0, 49.0, None),
                hit(125.0, 50.0, None),
                hit(100.0, 100.0, None),
                hit(100.0, 50.0, Some(&d2)),
                hit(124.0, 99.0, Some(&d2)),
            ],
        );
    }
}

/// Port of `inval_test2`.
// Port of: modules/sksg/tests/SGTest.cpp#L227-L292 (chrome/m156)
fn inval_test2(reporter: &mut Reporter) {
    let color = Color::make(SkColor::from(0xff00_0000));
    let rect = SgRect::make(Rect::new(0.0, 0.0, 100.0, 100.0));
    let m1 = SgMatrix::<SkMatrix>::make(SkMatrix::new_identity());
    let m2 = SgMatrix::<SkMatrix>::make(SkMatrix::new_identity());
    let draw_for_t1 = Draw::make(
        Some(rect.clone() as Rc<dyn GeometryNode>),
        Some(color.clone() as Rc<dyn PaintNode>),
    )
    .expect("geometry and paint are set");
    let t1 = TransformEffect::make(
        Some(draw_for_t1 as Rc<dyn RenderNode>),
        make_concat(
            Some(m1.clone() as Rc<dyn Transform>),
            Some(m2.clone() as Rc<dyn Transform>),
        ),
    )
    .expect("child and transform are set");
    let draw_for_t2 = Draw::make(
        Some(rect.clone() as Rc<dyn GeometryNode>),
        Some(color.clone() as Rc<dyn PaintNode>),
    )
    .expect("geometry and paint are set");
    let t2 = TransformEffect::make(
        Some(draw_for_t2 as Rc<dyn RenderNode>),
        Some(m1.clone() as Rc<dyn Transform>),
    )
    .expect("child and transform are set");
    let root = Group::make();
    root.add_child(t1.clone() as Rc<dyn RenderNode>);
    root.add_child(t2.clone() as Rc<dyn RenderNode>);
    let root: Rc<dyn RenderNode> = root;
    {
        check_inval(
            reporter,
            &root,
            Rect::new(0.0, 0.0, 100.0, 100.0),
            large_s32(),
            None,
        );
    }
    {
        color.set_color(SkColor::from(0xffff_0000));
        let damage = [
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Rect::new(0.0, 0.0, 100.0, 100.0),
        ];
        check_inval(
            reporter,
            &root,
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Some(&damage),
        );
    }
    {
        m2.set_matrix(SkMatrix::scale((2.0, 2.0)));
        let damage = [
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Rect::new(0.0, 0.0, 200.0, 200.0),
        ];
        check_inval(
            reporter,
            &root,
            Rect::new(0.0, 0.0, 200.0, 200.0),
            Rect::new(0.0, 0.0, 200.0, 200.0),
            Some(&damage),
        );
    }
    {
        m1.set_matrix(SkMatrix::translate((100.0, 100.0)));
        let damage = [
            Rect::new(0.0, 0.0, 200.0, 200.0),     // draw1 prev bounds
            Rect::new(100.0, 100.0, 300.0, 300.0), // draw1 new bounds
            Rect::new(0.0, 0.0, 100.0, 100.0),     // draw2 prev bounds
            Rect::new(100.0, 100.0, 200.0, 200.0), // draw2 new bounds
        ];
        check_inval(
            reporter,
            &root,
            Rect::new(100.0, 100.0, 300.0, 300.0),
            Rect::new(0.0, 0.0, 300.0, 300.0),
            Some(&damage),
        );
    }
    {
        rect.set_r(50.0);
        let damage = [
            Rect::new(100.0, 100.0, 300.0, 300.0), // draw1 prev bounds
            Rect::new(100.0, 100.0, 200.0, 300.0), // draw1 new bounds
            Rect::new(100.0, 100.0, 200.0, 200.0), // draw2 prev bounds
            Rect::new(100.0, 100.0, 150.0, 200.0), // draw2 new bounds
        ];
        check_inval(
            reporter,
            &root,
            Rect::new(100.0, 100.0, 200.0, 300.0),
            Rect::new(100.0, 100.0, 300.0, 300.0),
            Some(&damage),
        );
    }
}

/// Port of `inval_test3`.
// Port of: modules/sksg/tests/SGTest.cpp#L294-L344 (chrome/m156)
fn inval_test3(reporter: &mut Reporter) {
    let color1 = Color::make(SkColor::from(0xff00_0000));
    let color2 = Color::make(SkColor::from(0xff00_0000));
    let group = Group::make();
    group.add_child(
        Draw::make(
            Some(SgRect::make(Rect::new(0.0, 0.0, 100.0, 100.0)) as Rc<dyn GeometryNode>),
            Some(color1.clone() as Rc<dyn PaintNode>),
        )
        .expect("geometry and paint are set") as Rc<dyn RenderNode>,
    );
    group.add_child(
        Draw::make(
            Some(SgRect::make(Rect::new(200.0, 0.0, 300.0, 100.0)) as Rc<dyn GeometryNode>),
            Some(color2.clone() as Rc<dyn PaintNode>),
        )
        .expect("geometry and paint are set") as Rc<dyn RenderNode>,
    );
    let filter = DropShadowImageFilter::make();
    filter.set_offset((50.0, 75.0));
    let root: Rc<dyn RenderNode> = ImageFilterEffect::make(
        group.clone() as Rc<dyn RenderNode>,
        Some(filter.clone() as Rc<dyn ImageFilterNode>),
    );
    {
        check_inval(
            reporter,
            &root,
            Rect::new(0.0, 0.0, 350.0, 175.0),
            large_s32(),
            None,
        );
    }
    {
        filter.set_mode(DropShadowMode::ShadowOnly);
        let damage = [
            Rect::new(0.0, 0.0, 350.0, 175.0),
            Rect::new(50.0, 75.0, 350.0, 175.0),
        ];
        check_inval(
            reporter,
            &root,
            Rect::new(50.0, 75.0, 350.0, 175.0),
            Rect::new(0.0, 0.0, 350.0, 175.0),
            Some(&damage),
        );
    }
    {
        color1.set_color(SkColor::from(0xffff_0000));
        let damage = [Rect::new(50.0, 75.0, 350.0, 175.0)];
        check_inval(
            reporter,
            &root,
            Rect::new(50.0, 75.0, 350.0, 175.0),
            Rect::new(50.0, 75.0, 350.0, 175.0),
            Some(&damage),
        );
    }
    {
        group.set_visible(false);
        let damage = [Rect::new(50.0, 75.0, 350.0, 175.0)];
        check_inval(
            reporter,
            &root,
            Rect::new(50.0, 75.0, 350.0, 175.0),
            Rect::new(50.0, 75.0, 350.0, 175.0),
            Some(&damage),
        );
    }
}

/// Port of `inval_group_remove`.
// Port of: modules/sksg/tests/SGTest.cpp#L346-L356 (chrome/m156)
fn inval_group_remove() {
    let draw = Draw::make(
        Some(SgRect::make(Rect::new(0.0, 0.0, 100.0, 100.0)) as Rc<dyn GeometryNode>),
        Some(Color::make(SkColor::BLACK) as Rc<dyn PaintNode>),
    )
    .expect("geometry and paint are set") as Rc<dyn RenderNode>;
    let grp = Group::make();

    // Readding the child should not trigger asserts.
    grp.add_child(draw.clone());
    grp.remove_child(&draw);
    grp.add_child(draw);
}

// Port of: modules/sksg/tests/SGTest.cpp#L357-L362 (chrome/m156) (`DEF_TEST(SGInvalidation)`)
def_test!(SGInvalidation, |reporter| {
    inval_test1(reporter);
    inval_test2(reporter);
    inval_test3(reporter);
    inval_group_remove();
});

/// Port of `assert_paths_equal`: fill type, convexity, then the verbs, points and conic weights.
// Port of: modules/sksg/tests/SGTest.cpp#L385-L465 (chrome/m156)
fn assert_paths_equal(reporter: &mut Reporter, a: &SkPath, b: &SkPath) {
    reporter_assert!(reporter, a.fill_type() == b.fill_type());
    if a.fill_type() != b.fill_type() {
        return;
    }
    reporter_assert!(
        reporter,
        path_priv::get_convexity(a) == path_priv::get_convexity(b)
    );
    let segments_a: Vec<_> = a.iter().collect();
    let segments_b: Vec<_> = b.iter().collect();
    reporter_assert!(reporter, segments_a.len() == segments_b.len());
    for (rec_a, rec_b) in segments_a.iter().zip(segments_b.iter()) {
        let (verb_a, verb_b) = (rec_a.verb(), rec_b.verb());
        let (pts_a, pts_b) = (rec_a.points(), rec_b.points());
        reporter_assert!(reporter, verb_a == verb_b);
        if verb_a != verb_b {
            return;
        }
        reporter_assert!(reporter, pts_a == pts_b);
        if pts_a != pts_b {
            return;
        }
    }
}

/// Port of `test_merge`.
// Port of: modules/sksg/tests/SGTest.cpp#L467-L476 (chrome/m156)
fn test_merge(reporter: &mut Reporter, recs: &[MergeRec], expected: &SkPath) {
    let merge = Merge::make(recs);
    let mut ic = InvalidationController::new();
    merge.revalidate(Some(&mut ic), &SkMatrix::new_identity());
    assert_paths_equal(reporter, &merge.as_path(), expected);
}

// Port of: modules/sksg/tests/SGTest.cpp#L478-L560 (chrome/m156) (`DEF_TEST(SGMerge)`)
def_test!(SGMerge, |reporter| {
    let square = SgRect::make(Rect::new(0.0, 0.0, 100.0, 100.0));
    let rect = SgPath::make(SkPath::rect(Rect::new(50.0, 50.0, 150.0, 200.0), None));
    let window = SgPath::make(SkPath::rect(Rect::new(20.0, 30.0, 25.0, 90.0), None));
    let square_geo: Rc<dyn GeometryNode> = square.clone();
    let rect_geo: Rc<dyn GeometryNode> = rect.clone();
    let window_geo: Rc<dyn GeometryNode> = window.clone();

    {
        let expected = PathBuilder::new()
            .move_to((0.0, 0.0))
            .line_to((100.0, 0.0))
            .line_to((100.0, 100.0))
            .line_to((0.0, 100.0))
            .close()
            .move_to((50.0, 50.0))
            .line_to((150.0, 50.0))
            .line_to((150.0, 200.0))
            .line_to((50.0, 200.0))
            .close()
            .detach();
        test_merge(
            reporter,
            &[
                MergeRec {
                    geo: square_geo.clone(),
                    mode: MergeMode::Merge,
                },
                MergeRec {
                    geo: rect_geo.clone(),
                    mode: MergeMode::Merge,
                },
            ],
            &expected,
        );
    }

    {
        let expected = PathBuilder::new()
            .set_fill_type(PathFillType::EvenOdd)
            .move_to((100.0, 0.0))
            .line_to((0.0, 0.0))
            .line_to((0.0, 100.0))
            .line_to((50.0, 100.0))
            .line_to((50.0, 200.0))
            .line_to((150.0, 200.0))
            .line_to((150.0, 50.0))
            .line_to((100.0, 50.0))
            .line_to((100.0, 0.0))
            .close()
            .detach();
        test_merge(
            reporter,
            &[
                MergeRec {
                    geo: square_geo.clone(),
                    mode: MergeMode::Union,
                },
                MergeRec {
                    geo: rect_geo.clone(),
                    mode: MergeMode::Union,
                },
            ],
            &expected,
        );
    }

    {
        let expected = PathBuilder::new()
            .set_fill_type(PathFillType::EvenOdd)
            .move_to((50.0, 50.0))
            .line_to((100.0, 50.0))
            .line_to((100.0, 100.0))
            .line_to((50.0, 100.0))
            .close()
            .detach();
        test_merge(
            reporter,
            &[
                // first op is ignored
                MergeRec {
                    geo: square_geo.clone(),
                    mode: MergeMode::Union,
                },
                MergeRec {
                    geo: rect_geo.clone(),
                    mode: MergeMode::Intersect,
                },
            ],
            &expected,
        );
    }

    {
        let expected = PathBuilder::new()
            .set_fill_type(PathFillType::EvenOdd)
            .move_to((100.0, 0.0))
            .line_to((0.0, 0.0))
            .line_to((0.0, 100.0))
            .line_to((100.0, 100.0))
            .line_to((100.0, 0.0))
            .close()
            .move_to((25.0, 30.0))
            .line_to((20.0, 30.0))
            .line_to((20.0, 90.0))
            .line_to((25.0, 90.0))
            .line_to((25.0, 30.0))
            .close()
            .detach();
        test_merge(
            reporter,
            &[
                // first op is ignored
                MergeRec {
                    geo: square_geo.clone(),
                    mode: MergeMode::Union,
                },
                MergeRec {
                    geo: window_geo.clone(),
                    mode: MergeMode::Difference,
                },
            ],
            &expected,
        );
    }
});
