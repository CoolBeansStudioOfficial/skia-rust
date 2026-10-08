// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsSimplifyTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::Rect;
use skia_rust_pathops::simplify;

use crate::unit::path_ops_extended_test::{test_simplify, test_simplify_fail};
use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/PathOpsSimplifyTest.cpp#L9491-L9965 (chrome/m156)
/// `static TestDesc tests[]`: the tests that `DEF_TEST(PathOpsSimplify)` runs, in order.
pub(crate) const TESTS: &[(&str, fn(&mut Reporter, &str))] = &[
    ("bug8290", bug8290),
    ("bug8249", bug8249),
    ("bug11958_a", bug11958_a),
    ("bug11958_b", bug11958_b),
    ("bug11958_c", bug11958_c),
    ("grshapearc", grshapearc),
    ("coincubics", coincubics),
    ("joel_16x", joel_16x),
    ("joel_16", joel_16),
    ("joel_15x", joel_15x),
    ("joel_15", joel_15),
    ("joel_14x", joel_14x),
    ("joel_14", joel_14),
    ("joel_13x", joel_13x),
    ("joel_13", joel_13),
    ("joel_12x", joel_12x),
    ("joel_12", joel_12),
    ("joel_11", joel_11),
    ("joel_10", joel_10),
    ("joel_9", joel_9),
    ("joel_8", joel_8),
    ("joel_7", joel_7),
    ("joel_6", joel_6),
    ("joel_5", joel_5),
    ("joel_4", joel_4),
    ("joel_3", joel_3),
    ("joel_2", joel_2),
    ("joel_1", joel_1),
    ("simplifyTest_1", simplify_test_1),
    ("carsvg_1", carsvg_1),
    ("tiger8_393", tiger8_393),
    ("bug5169", bug5169),
    ("testQuads73", test_quads73),
    ("testQuads72", test_quads72),
    ("testQuads71", test_quads71),
    ("testQuads70", test_quads70),
    ("testQuads69", test_quads69),
    ("testQuads68", test_quads68),
    ("testQuads67", test_quads67),
    ("testQuads66", test_quads66),
    ("dean4", dean4),
    ("fuzz763_4713_b", fuzz763_4713_b),
    ("fuzz_twister2", fuzz_twister2),
    ("fuzz_twister", fuzz_twister),
    ("fuzz994s_3414", fuzz994s_3414),
    ("fuzz994s_11", fuzz994s_11),
    ("cr514118", cr514118),
    ("fuzz864a", fuzz864a),
    ("testQuads65", test_quads65),
    ("testIssue3838_3", test_issue3838_3),
    ("testIssue3838", test_issue3838),
    ("testArc", test_arc),
    ("testTriangle2", test_triangle2),
    ("testTriangle1", test_triangle1),
    ("testQuads64", test_quads64),
    ("testQuads63", test_quads63),
    ("testQuads62", test_quads62),
    ("testRect4", test_rect4),
    ("testRect3", test_rect3),
    ("testQuadralateral10", test_quadralateral10),
    ("testQuads61", test_quads61),
    ("testQuads60", test_quads60),
    ("testQuads59", test_quads59),
    ("testQuads58", test_quads58),
    ("testQuads57", test_quads57),
    ("testQuads56", test_quads56),
    ("testQuads54", test_quads54),
    ("testQuads53", test_quads53),
    ("testQuads52", test_quads52),
    ("testQuads51", test_quads51),
    ("testQuads50", test_quads50),
    ("testQuads49", test_quads49),
    ("testQuads48", test_quads48),
    ("testQuads47", test_quads47),
    ("testQuads46x", test_quads46x),
    ("testQuads45", test_quads45),
    ("testQuads44", test_quads44),
    ("testQuads43", test_quads43),
    ("testQuads42", test_quads42),
    ("testQuads41", test_quads41),
    ("testQuads36", test_quads36),
    ("testQuads37", test_quads37),
    ("testQuads38", test_quads38),
    ("testQuads39", test_quads39),
    ("testQuads40", test_quads40),
    ("testQuads16", test_quads16),
    ("testQuads17", test_quads17),
    ("testQuads18", test_quads18),
    ("testQuads19", test_quads19),
    ("testQuads20", test_quads20),
    ("testQuads21", test_quads21),
    ("testQuads22", test_quads22),
    ("testQuads23", test_quads23),
    ("testQuads24", test_quads24),
    ("testQuads25", test_quads25),
    ("testQuads26", test_quads26),
    ("testQuads27", test_quads27),
    ("testQuads28", test_quads28),
    ("testQuads29", test_quads29),
    ("testQuads30", test_quads30),
    ("testQuads31", test_quads31),
    ("testQuads32", test_quads32),
    ("testQuads33", test_quads33),
    ("testQuads34", test_quads34),
    ("testQuads35", test_quads35),
    ("testDegenerates1", test_degenerates1),
    ("testQuad13", test_quad13),
    ("testQuad14", test_quad14),
    ("testQuad15", test_quad15),
    ("testQuadratic56", test_quadratic56),
    ("testQuadralateral4", test_quadralateral4),
    ("testQuadralateral3", test_quadralateral3),
    ("testDegenerate5", test_degenerate5),
    ("testQuad12", test_quad12),
    ("testQuadratic51", test_quadratic51),
    ("testQuad8", test_quad8),
    ("testQuad11", test_quad11),
    ("testQuad10", test_quad10),
    ("testQuad9", test_quad9),
    ("testTriangles4x", test_triangles4x),
    ("testTriangles3x", test_triangles3x),
    ("testRect2s", test_rect2s),
    ("testRect1s", test_rect1s),
    ("tooCloseTest", too_close_test),
    ("skphealth_com76s", skphealth_com76s),
    ("testQuadLineIntersect1", test_quad_line_intersect1),
    ("testQuadLineIntersect2", test_quad_line_intersect2),
    ("testQuadLineIntersect3", test_quad_line_intersect3),
    ("testQuad7", test_quad7),
    ("testQuad6", test_quad6),
    ("testQuad5", test_quad5),
    ("testQuad4", test_quad4),
    ("testQuad3", test_quad3),
    ("testQuad2", test_quad2),
    ("testAddTCoincident2", test_add_tcoincident2),
    ("testAddTCoincident1", test_add_tcoincident1),
    ("testTriangles2", test_triangles2),
    ("testTriangles1", test_triangles1),
    ("testQuadratic97", test_quadratic97),
    ("testQuadratic96", test_quadratic96),
    ("testQuadratic95", test_quadratic95),
    ("testQuadratic94", test_quadratic94),
    ("testQuadralateral2", test_quadralateral2),
    ("testQuad1", test_quad1),
    ("testCubic2", test_cubic2),
    ("testCubic1", test_cubic1),
    ("testQuadralateral1", test_quadralateral1),
    ("testLine85", test_line85),
    ("testLine84", test_line84),
    ("testLine84x", test_line84x),
    ("testLine83", test_line83),
    ("testLine82h", test_line82h),
    ("testLine82g", test_line82g),
    ("testLine82f", test_line82f),
    ("testLine82e", test_line82e),
    ("testLine82d", test_line82d),
    ("testLine82c", test_line82c),
    ("testLine82b", test_line82b),
    ("testLine82a", test_line82a),
    ("testLine82", test_line82),
    ("testQuadratic93", test_quadratic93),
    ("testQuadratic92x", test_quadratic92x),
    ("testQuadratic91", test_quadratic91),
    ("testQuadratic90x", test_quadratic90x),
    ("testQuadratic89x", test_quadratic89x),
    ("testQuadratic88", test_quadratic88),
    ("testQuadratic87", test_quadratic87),
    ("testQuadratic86", test_quadratic86),
    ("testQuadratic85", test_quadratic85),
    ("testQuadratic84", test_quadratic84),
    ("testQuadratic83", test_quadratic83),
    ("testQuadratic82", test_quadratic82),
    ("testQuadratic81", test_quadratic81),
    ("testQuadratic80", test_quadratic80),
    ("testEight1", test_eight1),
    ("testEight2", test_eight2),
    ("testEight3", test_eight3),
    ("testEight4", test_eight4),
    ("testEight5", test_eight5),
    ("testEight6", test_eight6),
    ("testEight7", test_eight7),
    ("testEight8", test_eight8),
    ("testEight9", test_eight9),
    ("testEight10", test_eight10),
    ("testQuadratic79", test_quadratic79),
    ("testQuadratic78", test_quadratic78),
    ("testQuadratic77", test_quadratic77),
    ("testQuadratic76", test_quadratic76),
    ("testQuadratic75", test_quadratic75),
    ("testQuadratic74", test_quadratic74),
    ("testQuadratic73", test_quadratic73),
    ("testQuadratic72", test_quadratic72),
    ("testQuadratic71", test_quadratic71),
    ("testQuadratic70x", test_quadratic70x),
    ("testQuadratic69", test_quadratic69),
    ("testQuadratic68", test_quadratic68),
    ("testQuadratic67x", test_quadratic67x),
    ("testQuadratic65", test_quadratic65),
    ("testQuadratic64", test_quadratic64),
    ("testQuadratic63", test_quadratic63),
    ("testLine1a", test_line1a),
    ("testQuadratic59", test_quadratic59),
    ("testQuadratic59x", test_quadratic59x),
    ("testQuadratic58", test_quadratic58),
    ("testQuadratic55", test_quadratic55),
    ("testQuadratic53", test_quadratic53),
    ("testQuadratic38", test_quadratic38),
    ("testQuadratic37", test_quadratic37),
    ("testQuadratic36", test_quadratic36),
    ("testQuadratic35", test_quadratic35),
    ("testQuadratic34", test_quadratic34),
    ("testQuadratic33", test_quadratic33),
    ("testQuadratic32", test_quadratic32),
    ("testQuadratic31", test_quadratic31),
    ("testQuadratic30", test_quadratic30),
    ("testQuadratic29", test_quadratic29),
    ("testQuadratic28", test_quadratic28),
    ("testQuadratic27", test_quadratic27),
    ("testQuadratic26", test_quadratic26),
    ("testQuadratic25", test_quadratic25),
    ("testQuadratic24", test_quadratic24),
    ("testQuadratic23", test_quadratic23),
    ("testQuadratic22", test_quadratic22),
    ("testQuadratic21", test_quadratic21),
    ("testQuadratic20", test_quadratic20),
    ("testQuadratic19", test_quadratic19),
    ("testQuadratic18", test_quadratic18),
    ("testQuadratic17x", test_quadratic17x),
    ("testQuadratic15", test_quadratic15),
    ("testQuadratic14", test_quadratic14),
    ("testQuadratic9", test_quadratic9),
    ("testQuadratic8", test_quadratic8),
    ("testQuadratic7", test_quadratic7),
    ("testQuadratic6", test_quadratic6),
    ("testQuadratic5", test_quadratic5),
    ("testQuadratic4x", test_quadratic4x),
    ("testQuadratic3x", test_quadratic3x),
    ("testQuadratic2x", test_quadratic2x),
    ("testQuadratic1x", test_quadratic1x),
    ("testQuadratic4", test_quadratic4),
    ("testQuadratic3", test_quadratic3),
    ("testQuadratic2", test_quadratic2),
    ("testQuadratic1", test_quadratic1),
    ("testLine4ax", test_line4ax),
    ("testLine3aax", test_line3aax),
    ("testLine2ax", test_line2ax),
    ("testLine1ax", test_line1ax),
    ("testQuadralateral9x", test_quadralateral9x),
    ("testQuadralateral8x", test_quadralateral8x),
    ("testQuadralateral7x", test_quadralateral7x),
    ("testQuadralateral6x", test_quadralateral6x),
    ("testQuadralateral6ax", test_quadralateral6ax),
    ("testQuadralateral9", test_quadralateral9),
    ("testQuadralateral8", test_quadralateral8),
    ("testQuadralateral7", test_quadralateral7),
    ("testQuadralateral6", test_quadralateral6),
    ("testQuadralateral6a", test_quadralateral6a),
    ("testFauxQuadralateral6dx", test_faux_quadralateral6dx),
    ("testFauxQuadralateral6cx", test_faux_quadralateral6cx),
    ("testFauxQuadralateral6bx", test_faux_quadralateral6bx),
    ("testFauxQuadralateral6ax", test_faux_quadralateral6ax),
    ("testFauxQuadralateral6x", test_faux_quadralateral6x),
    ("testFauxQuadralateral6d", test_faux_quadralateral6d),
    ("testFauxQuadralateral6c", test_faux_quadralateral6c),
    ("testFauxQuadralateral6b", test_faux_quadralateral6b),
    ("testFauxQuadralateral6a", test_faux_quadralateral6a),
    ("testFauxQuadralateral6", test_faux_quadralateral6),
    ("testQuadralateral5x", test_quadralateral5x),
    ("testQuadralateral5", test_quadralateral5),
    ("testNondegenerate4x", test_nondegenerate4x),
    ("testNondegenerate3x", test_nondegenerate3x),
    ("testNondegenerate2x", test_nondegenerate2x),
    ("testNondegenerate1x", test_nondegenerate1x),
    ("testNondegenerate4", test_nondegenerate4),
    ("testNondegenerate3", test_nondegenerate3),
    ("testNondegenerate2", test_nondegenerate2),
    ("testNondegenerate1", test_nondegenerate1),
    ("testDegenerate4x", test_degenerate4x),
    ("testDegenerate3x", test_degenerate3x),
    ("testDegenerate2x", test_degenerate2x),
    ("testDegenerate1x", test_degenerate1x),
    ("testDegenerate4", test_degenerate4),
    ("testDegenerate3", test_degenerate3),
    ("testDegenerate2", test_degenerate2),
    ("testDegenerate1", test_degenerate1),
    ("testLine79x", test_line79x),
    ("testLine78x", test_line78x),
    ("testLine77x", test_line77x),
    ("testLine76x", test_line76x),
    ("testLine75x", test_line75x),
    ("testLine74x", test_line74x),
    ("testLine73x", test_line73x),
    ("testLine72x", test_line72x),
    ("testLine71x", test_line71x),
    ("testLine70x", test_line70x),
    ("testLine69x", test_line69x),
    ("testLine68hx", test_line68hx),
    ("testLine68gx", test_line68gx),
    ("testLine68fx", test_line68fx),
    ("testLine68ex", test_line68ex),
    ("testLine68dx", test_line68dx),
    ("testLine68cx", test_line68cx),
    ("testLine68bx", test_line68bx),
    ("testLine68ax", test_line68ax),
    ("testLine67x", test_line67x),
    ("testLine66x", test_line66x),
    ("testLine65x", test_line65x),
    ("testLine64x", test_line64x),
    ("testLine63x", test_line63x),
    ("testLine62x", test_line62x),
    ("testLine61x", test_line61x),
    ("testLine60x", test_line60x),
    ("testLine59x", test_line59x),
    ("testLine58x", test_line58x),
    ("testLine57x", test_line57x),
    ("testLine56x", test_line56x),
    ("testLine55x", test_line55x),
    ("testLine54x", test_line54x),
    ("testLine53x", test_line53x),
    ("testLine52x", test_line52x),
    ("testLine51x", test_line51x),
    ("testLine50x", test_line50x),
    ("testLine49x", test_line49x),
    ("testLine48x", test_line48x),
    ("testLine47x", test_line47x),
    ("testLine46x", test_line46x),
    ("testLine45x", test_line45x),
    ("testLine44x", test_line44x),
    ("testLine43x", test_line43x),
    ("testLine42x", test_line42x),
    ("testLine41x", test_line41x),
    ("testLine40x", test_line40x),
    ("testLine38x", test_line38x),
    ("testLine37x", test_line37x),
    ("testLine36x", test_line36x),
    ("testLine35x", test_line35x),
    ("testLine34x", test_line34x),
    ("testLine33x", test_line33x),
    ("testLine32x", test_line32x),
    ("testLine31x", test_line31x),
    ("testLine30x", test_line30x),
    ("testLine29x", test_line29x),
    ("testLine28x", test_line28x),
    ("testLine27x", test_line27x),
    ("testLine26x", test_line26x),
    ("testLine25x", test_line25x),
    ("testLine24ax", test_line24ax),
    ("testLine24x", test_line24x),
    ("testLine23x", test_line23x),
    ("testLine22x", test_line22x),
    ("testLine21x", test_line21x),
    ("testLine20x", test_line20x),
    ("testLine19x", test_line19x),
    ("testLine18x", test_line18x),
    ("testLine17x", test_line17x),
    ("testLine16x", test_line16x),
    ("testLine15x", test_line15x),
    ("testLine14x", test_line14x),
    ("testLine13x", test_line13x),
    ("testLine12x", test_line12x),
    ("testLine11x", test_line11x),
    ("testLine10ax", test_line10ax),
    ("testLine10x", test_line10x),
    ("testLine9x", test_line9x),
    ("testLine8x", test_line8x),
    ("testLine7bx", test_line7bx),
    ("testLine7ax", test_line7ax),
    ("testLine7x", test_line7x),
    ("testLine6x", test_line6x),
    ("testLine5x", test_line5x),
    ("testLine4x", test_line4x),
    ("testLine3bx", test_line3bx),
    ("testLine3ax", test_line3ax),
    ("testLine3x", test_line3x),
    ("testLine2x", test_line2x),
    ("testLine1x", test_line1x),
    ("testLine81", test_line81),
    ("testLine80", test_line80),
    ("testLine79", test_line79),
    ("testLine78", test_line78),
    ("testLine77", test_line77),
    ("testLine76", test_line76),
    ("testLine75", test_line75),
    ("testLine74", test_line74),
    ("testLine73", test_line73),
    ("testLine72", test_line72),
    ("testLine71", test_line71),
    ("testLine70", test_line70),
    ("testLine69", test_line69),
    ("testLine68h", test_line68h),
    ("testLine68g", test_line68g),
    ("testLine68f", test_line68f),
    ("testLine68e", test_line68e),
    ("testLine68d", test_line68d),
    ("testLine68c", test_line68c),
    ("testLine68b", test_line68b),
    ("testLine68a", test_line68a),
    ("testLine67", test_line67),
    ("testLine66", test_line66),
    ("testLine65", test_line65),
    ("testLine64", test_line64),
    ("testLine63", test_line63),
    ("testLine62", test_line62),
    ("testLine61", test_line61),
    ("testLine60", test_line60),
    ("testLine59", test_line59),
    ("testLine58", test_line58),
    ("testLine57", test_line57),
    ("testLine56", test_line56),
    ("testLine55", test_line55),
    ("testLine54", test_line54),
    ("testLine53", test_line53),
    ("testLine52", test_line52),
    ("testLine51", test_line51),
    ("testLine50", test_line50),
    ("testLine49", test_line49),
    ("testLine48", test_line48),
    ("testLine47", test_line47),
    ("testLine46", test_line46),
    ("testLine45", test_line45),
    ("testLine44", test_line44),
    ("testLine43", test_line43),
    ("testLine42", test_line42),
    ("testLine41", test_line41),
    ("testLine40", test_line40),
    ("testLine38", test_line38),
    ("testLine37", test_line37),
    ("testLine36", test_line36),
    ("testLine35", test_line35),
    ("testLine34", test_line34),
    ("testLine33", test_line33),
    ("testLine32", test_line32),
    ("testLine31", test_line31),
    ("testLine30", test_line30),
    ("testLine29", test_line29),
    ("testLine28", test_line28),
    ("testLine27", test_line27),
    ("testLine26", test_line26),
    ("testLine25", test_line25),
    ("testLine24a", test_line24a),
    ("testLine24", test_line24),
    ("testLine23", test_line23),
    ("testLine22", test_line22),
    ("testLine21", test_line21),
    ("testLine20", test_line20),
    ("testLine19", test_line19),
    ("testLine18", test_line18),
    ("testLine17", test_line17),
    ("testLine16", test_line16),
    ("testLine15", test_line15),
    ("testLine14", test_line14),
    ("testLine13", test_line13),
    ("testLine12", test_line12),
    ("testLine11", test_line11),
    ("testLine10a", test_line10a),
    ("testLine10", test_line10),
    ("testLine9", test_line9),
    ("testLine8", test_line8),
    ("testLine7b", test_line7b),
    ("testLine7a", test_line7a),
    ("testLine7", test_line7),
    ("testLine6", test_line6),
    ("testLine5", test_line5),
    ("testLine4", test_line4),
    ("testLine3b", test_line3b),
    ("testLine3a", test_line3a),
    ("testLine3", test_line3),
    ("testLine2", test_line2),
    ("testLine1", test_line1),
    ("testDegenerates", test_degenerates),
];

/// `static TestDesc subTests[]`: run only when `runSubTests` is set (it is not).
// Port of: tests/PathOpsSimplifyTest.cpp#L9966-L9969 (chrome/m156)
pub(crate) const SUB_TESTS: &[(&str, fn(&mut Reporter, &str))] = &[
    ("fuzz994s_3414", fuzz994s_3414),
    ("fuzz994s_11", fuzz994s_11),
];

fn test_degenerates(reporter: &mut Reporter, _filename: &str) {
    let mut doubleback;
    {
        let mut _b = PathBuilder::new();
        _b.line_to((1.0, 0.0));
        doubleback = _b.detach();
    }
    let mut simple = simplify(&doubleback);
    reporter_assert!(reporter, simple.is_some());
    reporter_assert!(reporter, simple.as_ref().is_some_and(|p| p.is_empty()));
    {
        let mut _b = PathBuilder::new();
        _b.line_to((1.0, 0.0)).line_to((2.0, 0.0));
        doubleback = _b.detach();
    }
    simple = simplify(&doubleback);
    reporter_assert!(reporter, simple.is_some());
    reporter_assert!(reporter, simple.as_ref().is_some_and(|p| p.is_empty()));
    {
        let mut _b = PathBuilder::new();
        _b.line_to((-1.0, 0.0)).line_to((-1.0, 1.0)).line_to((-1.0, 0.0));
        doubleback = _b.detach();
    }
    simple = simplify(&doubleback);
    reporter_assert!(reporter, simple.is_some());
    reporter_assert!(reporter, simple.as_ref().is_some_and(|p| p.is_empty()));
    {
        let mut _b = PathBuilder::new();
        _b.line_to((1.0, 0.0)).line_to((1.0, 0.0)).line_to((1.0, 1.0)).line_to((1.0, 1.0)).line_to((1.0, 0.0));
        doubleback = _b.detach();
    }
    simple = simplify(&doubleback);
    reporter_assert!(reporter, simple.is_some());
    reporter_assert!(reporter, simple.as_ref().is_some_and(|p| p.is_empty()));
}

fn test_line1(reporter: &mut Reporter, filename: &str) {
    let mut _b = PathBuilder::new();
    _b.move_to((2.0,0.0)).line_to((1.0,1.0)).line_to((0.0,0.0)).close();
    let path = _b.detach();
    test_simplify(reporter, &path, filename);
}

fn test_line1x(reporter: &mut Reporter, filename: &str) {
    let mut _b = PathBuilder::new_with_fill_type(PathFillType::EvenOdd);
    _b.move_to((2.0,0.0)).line_to((1.0,1.0)).line_to((0.0,0.0)).close();
    let path = _b.detach();
    test_simplify(reporter, &path, filename);
}

fn add_inner_cwtriangle(path: &mut PathBuilder) {
    path.move_to((3.0,0.0));
    path.line_to((4.0,1.0));
    path.line_to((2.0,1.0));
    path.close();
}

fn add_inner_ccwtriangle(path: &mut PathBuilder) {
    path.move_to((3.0,0.0));
    path.line_to((2.0,1.0));
    path.line_to((4.0,1.0));
    path.close();
}

fn add_outer_cwtriangle(path: &mut PathBuilder) {
    path.move_to((3.0,0.0));
    path.line_to((6.0,2.0));
    path.line_to((0.0,2.0));
    path.close();
}

fn add_outer_ccwtriangle(path: &mut PathBuilder) {
    path.move_to((3.0,0.0));
    path.line_to((0.0,2.0));
    path.line_to((6.0,2.0));
    path.close();
}

fn test_line2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    add_inner_cwtriangle(&mut path);
    add_outer_cwtriangle(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line2x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    add_inner_cwtriangle(&mut path);
    add_outer_cwtriangle(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    add_inner_ccwtriangle(&mut path);
    add_outer_cwtriangle(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line3x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    add_inner_ccwtriangle(&mut path);
    add_outer_cwtriangle(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line3a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    add_inner_cwtriangle(&mut path);
    add_outer_ccwtriangle(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line3ax(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    add_inner_cwtriangle(&mut path);
    add_outer_ccwtriangle(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line3b(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    add_inner_ccwtriangle(&mut path);
    add_outer_ccwtriangle(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line3bx(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    add_inner_ccwtriangle(&mut path);
    add_outer_ccwtriangle(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    add_outer_ccwtriangle(&mut path);
    add_outer_cwtriangle(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line4x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    add_outer_ccwtriangle(&mut path);
    add_outer_cwtriangle(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line5(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    add_outer_cwtriangle(&mut path);
    add_outer_cwtriangle(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line5x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    add_outer_cwtriangle(&mut path);
    add_outer_cwtriangle(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line6(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0,0.0));
    path.line_to((4.0,0.0));
    path.line_to((2.0,2.0));
    path.close();
    path.move_to((2.0,0.0));
    path.line_to((6.0,0.0));
    path.line_to((4.0,2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line6x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0,0.0));
    path.line_to((4.0,0.0));
    path.line_to((2.0,2.0));
    path.close();
    path.move_to((2.0,0.0));
    path.line_to((6.0,0.0));
    path.line_to((4.0,2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line7(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0,0.0));
    path.line_to((4.0,0.0));
    path.line_to((2.0,2.0));
    path.close();
    path.move_to((6.0,0.0));
    path.line_to((2.0,0.0));
    path.line_to((4.0,2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line7x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0,0.0));
    path.line_to((4.0,0.0));
    path.line_to((2.0,2.0));
    path.close();
    path.move_to((6.0,0.0));
    path.line_to((2.0,0.0));
    path.line_to((4.0,2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line7a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0,0.0));
    path.line_to((4.0,0.0));
    path.line_to((2.0,2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line7ax(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0,0.0));
    path.line_to((4.0,0.0));
    path.line_to((2.0,2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line7b(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0,0.0));
    path.line_to((4.0,0.0));
    path.close();
    path.move_to((6.0,0.0));
    path.line_to((2.0,0.0));
    path.line_to((4.0,2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line7bx(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0,0.0));
    path.line_to((4.0,0.0));
    path.close();
    path.move_to((6.0,0.0));
    path.line_to((2.0,0.0));
    path.line_to((4.0,2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line8(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0,4.0));
    path.line_to((4.0,4.0));
    path.line_to((2.0,2.0));
    path.close();
    path.move_to((2.0,4.0));
    path.line_to((6.0,4.0));
    path.line_to((4.0,2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line8x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0,4.0));
    path.line_to((4.0,4.0));
    path.line_to((2.0,2.0));
    path.close();
    path.move_to((2.0,4.0));
    path.line_to((6.0,4.0));
    path.line_to((4.0,2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line9(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0,4.0));
    path.line_to((4.0,4.0));
    path.line_to((2.0,2.0));
    path.close();
    path.move_to((6.0,4.0));
    path.line_to((2.0,4.0));
    path.line_to((4.0,2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line9x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0,4.0));
    path.line_to((4.0,4.0));
    path.line_to((2.0,2.0));
    path.close();
    path.move_to((6.0,4.0));
    path.line_to((2.0,4.0));
    path.line_to((4.0,2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line10(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0,4.0));
    path.line_to((4.0,4.0));
    path.line_to((2.0,2.0));
    path.close();
    path.move_to((2.0,1.0));
    path.line_to((3.0,4.0));
    path.line_to((6.0,1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line10x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0,4.0));
    path.line_to((4.0,4.0));
    path.line_to((2.0,2.0));
    path.close();
    path.move_to((2.0,1.0));
    path.line_to((3.0,4.0));
    path.line_to((6.0,1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line10a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0,4.0));
    path.line_to((8.0,4.0));
    path.line_to((4.0,0.0));
    path.close();
    path.move_to((2.0,2.0));
    path.line_to((3.0,3.0));
    path.line_to((4.0,2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line10ax(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0,4.0));
    path.line_to((8.0,4.0));
    path.line_to((4.0,0.0));
    path.close();
    path.move_to((2.0,2.0));
    path.line_to((3.0,3.0));
    path.line_to((4.0,2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn add_cwcontainer(path: &mut PathBuilder) {
    path.move_to((6.0,4.0));
    path.line_to((0.0,4.0));
    path.line_to((3.0,1.0));
    path.close();
}

fn add_ccwcontainer(path: &mut PathBuilder) {
    path.move_to((0.0,4.0));
    path.line_to((6.0,4.0));
    path.line_to((3.0,1.0));
    path.close();
}

fn add_cwcontents(path: &mut PathBuilder) {
    path.move_to((2.0,3.0));
    path.line_to((3.0,2.0));
    path.line_to((4.0,3.0));
    path.close();
}

fn add_ccwcontents(path: &mut PathBuilder) {
    path.move_to((3.0,2.0));
    path.line_to((2.0,3.0));
    path.line_to((4.0,3.0));
    path.close();
}

fn test_line11(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    add_cwcontainer(&mut path);
    add_cwcontents(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line11x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    add_cwcontainer(&mut path);
    add_cwcontents(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line12(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    add_ccwcontainer(&mut path);
    add_cwcontents(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line12x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    add_ccwcontainer(&mut path);
    add_cwcontents(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line13(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    add_cwcontainer(&mut path);
    add_ccwcontents(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line13x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    add_cwcontainer(&mut path);
    add_ccwcontents(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line14(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    add_ccwcontainer(&mut path);
    add_ccwcontents(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line14x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    add_ccwcontainer(&mut path);
    add_ccwcontents(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line15(reporter: &mut Reporter, filename: &str) {
    let path = Path::rect(Rect::new(0.0, 0.0, 9.0, 9.0), PathDirection::CW).with_fill_type(PathFillType::Winding);
    test_simplify(reporter, &path, filename);
}

fn test_line15x(reporter: &mut Reporter, filename: &str) {
    let path = Path::rect(Rect::new(0.0, 0.0, 9.0, 9.0), PathDirection::CW).with_fill_type(PathFillType::EvenOdd);
    test_simplify(reporter, &path, filename);
}

fn test_line16(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 4.0, 9.0, 9.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line16x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 4.0, 9.0, 9.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line17(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line17x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line18(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 4.0, 21.0, 21.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line18x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 4.0, 21.0, 21.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line19(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 16.0, 21.0, 21.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line19x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 16.0, 21.0, 21.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line20(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 12.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 12.0, 9.0, 9.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line20x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 12.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 12.0, 9.0, 9.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line21(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 12.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 16.0, 9.0, 9.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line21x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 12.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 16.0, 9.0, 9.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line22(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 12.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line22x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 12.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line23(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 12.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 0.0, 21.0, 21.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line23x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 12.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 0.0, 21.0, 21.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line24a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0,0.0));
    path.line_to((4.0,4.0));
    path.line_to((0.0,4.0));
    path.close();
    path.move_to((2.0,0.0));
    path.line_to((1.0,2.0));
    path.line_to((2.0,2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line24ax(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((2.0,0.0));
    path.line_to((4.0,4.0));
    path.line_to((0.0,4.0));
    path.close();
    path.move_to((2.0,0.0));
    path.line_to((1.0,2.0));
    path.line_to((2.0,2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line24(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 18.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line24x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 18.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line25(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 6.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 0.0, 21.0, 21.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line25x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 6.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 0.0, 21.0, 21.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line26(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 18.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 12.0, 9.0, 9.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line26x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 18.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 12.0, 9.0, 9.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line27(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 18.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 8.0, 21.0, 21.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line27x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 18.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 8.0, 21.0, 21.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line28(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 6.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 9.0, 9.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line28x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 6.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 9.0, 9.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line29(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 18.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 12.0, 21.0, 21.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line29x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 18.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 12.0, 21.0, 21.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line30(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 4.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line30x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 4.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line31(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 4.0, 9.0, 9.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line31x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 4.0, 9.0, 9.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line32(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line32x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line33(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 16.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line33x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 16.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line34(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 6.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line34x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 6.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line35(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 0.0, 18.0, 18.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 16.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line35x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 0.0, 18.0, 18.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 16.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line36(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 10.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 12.0, 18.0, 18.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 16.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line36x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 10.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 12.0, 18.0, 18.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 16.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line37(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 20.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(18.0, 24.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 9.0, 9.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line37x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 20.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(18.0, 24.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 9.0, 9.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line38(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(10.0, 0.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 12.0, 18.0, 18.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 12.0, 21.0, 21.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line38x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(10.0, 0.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 12.0, 18.0, 18.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 12.0, 21.0, 21.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line40(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(10.0, 0.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 18.0, 24.0, 24.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 16.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line40x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(10.0, 0.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 18.0, 24.0, 24.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 16.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line41(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(18.0, 24.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 0.0, 21.0, 21.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line41x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(18.0, 24.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 0.0, 21.0, 21.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line42(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(8.0, 16.0, 17.0, 17.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line42x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(8.0, 16.0, 17.0, 17.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line43(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 24.0, 18.0, 18.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 32.0, 9.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line43x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 24.0, 18.0, 18.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 32.0, 9.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line44(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(10.0, 40.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(18.0, 0.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(18.0, 32.0, 27.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line44x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(10.0, 40.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(18.0, 0.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(18.0, 32.0, 27.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line45(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(10.0, 0.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(18.0, 0.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 32.0, 33.0, 36.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line45x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(10.0, 0.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(18.0, 0.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 32.0, 33.0, 36.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line46(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(10.0, 40.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 0.0, 36.0, 36.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 32.0, 33.0, 36.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line46x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(10.0, 40.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 0.0, 36.0, 36.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 32.0, 33.0, 36.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line47(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 9.0, 9.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line47x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 9.0, 9.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line48(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 6.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 9.0, 9.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line48x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 6.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 9.0, 9.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line49(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 9.0, 9.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line49x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 9.0, 9.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line50(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(10.0, 30.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 20.0, 36.0, 30.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line50x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(10.0, 30.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 20.0, 36.0, 30.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line51(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 12.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line51x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 12.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line52(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 30.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 20.0, 18.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(32.0, 0.0, 36.0, 41.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line52x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 30.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 20.0, 18.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(32.0, 0.0, 36.0, 41.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line53(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(10.0, 30.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 20.0, 24.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 32.0, 21.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line53x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(10.0, 30.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 20.0, 24.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 32.0, 21.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line54(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 0.0, 18.0, 18.0), PathDirection::CW, None);
    path.add_rect(Rect::new(8.0, 4.0, 17.0, 17.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line54x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 0.0, 18.0, 18.0), PathDirection::CW, None);
    path.add_rect(Rect::new(8.0, 4.0, 17.0, 17.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line55(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 6.0, 18.0, 18.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 4.0, 13.0, 13.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line55x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 6.0, 18.0, 18.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 4.0, 13.0, 13.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line56(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 20.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(18.0, 20.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 0.0, 21.0, 21.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line56x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 20.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(18.0, 20.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 0.0, 21.0, 21.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line57(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(20.0, 0.0, 40.0, 40.0), PathDirection::CW, None);
    path.add_rect(Rect::new(20.0, 0.0, 30.0, 40.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 0.0, 21.0, 21.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line57x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(20.0, 0.0, 40.0, 40.0), PathDirection::CW, None);
    path.add_rect(Rect::new(20.0, 0.0, 30.0, 40.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 0.0, 21.0, 21.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line58(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(0.0, 12.0, 9.0, 9.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line58x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(0.0, 12.0, 9.0, 9.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line59(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 6.0, 18.0, 18.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(4.0, 4.0, 13.0, 13.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line59x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 6.0, 18.0, 18.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(4.0, 4.0, 13.0, 13.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line60(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 12.0, 18.0, 18.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line60x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 12.0, 18.0, 18.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line61(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 0.0, 24.0, 24.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(12.0, 0.0, 21.0, 21.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line61x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 0.0, 24.0, 24.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(12.0, 0.0, 21.0, 21.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line62(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 12.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line62x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 12.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line63(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 10.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 6.0, 12.0, 12.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(0.0, 32.0, 9.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line63x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 10.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 6.0, 12.0, 12.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(0.0, 32.0, 9.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line64(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(10.0, 40.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(18.0, 6.0, 30.0, 30.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line64x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(10.0, 40.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(18.0, 6.0, 30.0, 30.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line65(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(10.0, 0.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 0.0, 36.0, 36.0), PathDirection::CW, None);
    path.add_rect(Rect::new(32.0, 6.0, 36.0, 41.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line65x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(10.0, 0.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 0.0, 36.0, 36.0), PathDirection::CW, None);
    path.add_rect(Rect::new(32.0, 6.0, 36.0, 41.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line66(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 30.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 20.0, 24.0, 30.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line66x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 30.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 20.0, 24.0, 30.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line67(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(10.0, 40.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 20.0, 36.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(32.0, 0.0, 36.0, 41.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line67x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(10.0, 40.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 20.0, 36.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(32.0, 0.0, 36.0, 41.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line68a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CW, None);
    path.add_rect(Rect::new(1.0, 2.0, 4.0, 2.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line68ax(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CW, None);
    path.add_rect(Rect::new(1.0, 2.0, 4.0, 2.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line68b(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(1.0, 2.0, 2.0, 2.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line68bx(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(1.0, 2.0, 2.0, 2.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line68c(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CW, None);
    path.add_rect(Rect::new(1.0, 2.0, 4.0, 2.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line68cx(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CW, None);
    path.add_rect(Rect::new(1.0, 2.0, 4.0, 2.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line68d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(1.0, 2.0, 4.0, 2.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line68dx(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(1.0, 2.0, 4.0, 2.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line68e(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(1.0, 2.0, 2.0, 2.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line68ex(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(1.0, 2.0, 2.0, 2.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line68f(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(1.0, 2.0, 2.0, 2.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line68fx(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(1.0, 2.0, 2.0, 2.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line68g(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(1.0, 2.0, 2.0, 2.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line68gx(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(1.0, 2.0, 2.0, 2.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line68h(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(1.0, 2.0, 2.0, 2.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line68hx(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 8.0, 8.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 2.0, 6.0, 6.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(1.0, 2.0, 2.0, 2.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line69(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 20.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 20.0, 12.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 32.0, 21.0, 36.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line69x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 20.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 20.0, 12.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 32.0, 21.0, 36.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line70(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 24.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 32.0, 21.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line70x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 24.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 32.0, 21.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line71(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 0.0, 24.0, 24.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 32.0, 21.0, 36.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line71x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 0.0, 24.0, 24.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 32.0, 21.0, 36.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line72(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(10.0, 40.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 20.0, 18.0, 30.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line72x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(10.0, 40.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(6.0, 20.0, 18.0, 30.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line73(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 40.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 20.0, 12.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 9.0, 9.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line73x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 40.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 20.0, 12.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 9.0, 9.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line74(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(20.0, 30.0, 40.0, 40.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 20.0, 36.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(32.0, 24.0, 36.0, 41.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line74x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(20.0, 30.0, 40.0, 40.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 20.0, 36.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(32.0, 24.0, 36.0, 41.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line75(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(10.0, 0.0, 30.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(18.0, 0.0, 30.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(12.0, 0.0, 21.0, 21.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line75x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(10.0, 0.0, 30.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(18.0, 0.0, 30.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(12.0, 0.0, 21.0, 21.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line76(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(36.0, 0.0, 66.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(10.0, 20.0, 40.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 20.0, 36.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(32.0, 6.0, 36.0, 41.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line76x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(36.0, 0.0, 66.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(10.0, 20.0, 40.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 20.0, 36.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(32.0, 6.0, 36.0, 41.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line77(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(20.0, 0.0, 40.0, 40.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 6.0, 36.0, 36.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(24.0, 32.0, 33.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line77x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(20.0, 0.0, 40.0, 40.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 6.0, 36.0, 36.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(24.0, 32.0, 33.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line78(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 30.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(10.0, 20.0, 30.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(18.0, 20.0, 30.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(32.0, 0.0, 36.0, 41.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line78x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 30.0, 60.0), PathDirection::CW, None);
    path.add_rect(Rect::new(10.0, 20.0, 30.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(18.0, 20.0, 30.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(32.0, 0.0, 36.0, 41.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line79(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 36.0, 60.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(10.0, 30.0, 40.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 20.0, 12.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(0.0, 32.0, 9.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line79x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 36.0, 60.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(10.0, 30.0, 40.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 20.0, 12.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(0.0, 32.0, 9.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line81(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(-1.0, -1.0, 3.0, 3.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 0.0, 1.0, 1.0), PathDirection::CW, None);
    path.add_rect(Rect::new(1.0, 1.0, 2.0, 2.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_degenerate1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((2.0, 0.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((2.0, 0.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_degenerate1x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((2.0, 0.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((2.0, 0.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_degenerate2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_degenerate2x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_degenerate3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 0.0));
    path.line_to((1.0, 0.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((3.0, 0.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_degenerate3x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 0.0));
    path.line_to((1.0, 0.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((3.0, 0.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_degenerate4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.line_to((1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_degenerate4x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.line_to((1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_nondegenerate1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((3.0, 0.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((1.0, 1.0));
    path.line_to((2.0, 1.0));
    path.line_to((1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_nondegenerate1x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((3.0, 0.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((1.0, 1.0));
    path.line_to((2.0, 1.0));
    path.line_to((1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_nondegenerate2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.line_to((0.0, 1.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((0.0, 2.0));
    path.line_to((0.0, 3.0));
    path.line_to((1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_nondegenerate2x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1.0, 0.0));
    path.line_to((0.0, 1.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((0.0, 2.0));
    path.line_to((0.0, 3.0));
    path.line_to((1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_nondegenerate3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((2.0, 1.0));
    path.close();
    path.move_to((0.0, 1.0));
    path.line_to((1.0, 1.0));
    path.line_to((0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_nondegenerate3x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((2.0, 1.0));
    path.close();
    path.move_to((0.0, 1.0));
    path.line_to((1.0, 1.0));
    path.line_to((0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_nondegenerate4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.line_to((0.0, 1.0));
    path.line_to((1.0, 2.0));
    path.close();
    path.move_to((0.0, 2.0));
    path.line_to((0.0, 3.0));
    path.line_to((1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_nondegenerate4x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1.0, 0.0));
    path.line_to((0.0, 1.0));
    path.line_to((1.0, 2.0));
    path.close();
    path.move_to((0.0, 2.0));
    path.line_to((0.0, 3.0));
    path.line_to((1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral5(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 2.0));
    path.line_to((3.0, 2.0));
    path.line_to((3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral5x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 2.0));
    path.line_to((3.0, 2.0));
    path.line_to((3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral6(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((2.0, 0.0));
    path.line_to((0.0, 2.0));
    path.line_to((2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral6x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((2.0, 0.0));
    path.line_to((0.0, 2.0));
    path.line_to((2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_faux_quadralateral6(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((2.0, 0.0));
    path.line_to((1.0 + 1.0/3.0, 2.0/3.0));
    path.close();
    path.move_to((1.0 + 1.0/3.0, 2.0/3.0));
    path.line_to((0.0, 2.0));
    path.line_to((2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_faux_quadralateral6x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((2.0, 0.0));
    path.line_to((1.0 + 1.0/3.0, 2.0/3.0));
    path.close();
    path.move_to((1.0 + 1.0/3.0, 2.0/3.0));
    path.line_to((0.0, 2.0));
    path.line_to((2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_faux_quadralateral6a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((3.0, 0.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((6.0, 0.0));
    path.line_to((4.0, 2.0));
    path.close();
    path.move_to((4.0, 2.0));
    path.line_to((0.0, 6.0));
    path.line_to((6.0, 6.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_faux_quadralateral6ax(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((3.0, 0.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((6.0, 0.0));
    path.line_to((4.0, 2.0));
    path.close();
    path.move_to((4.0, 2.0));
    path.line_to((0.0, 6.0));
    path.line_to((6.0, 6.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_faux_quadralateral6b(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((3.0, 0.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((6.0, 0.0));
    path.line_to((4.0, 2.0));
    path.close();
    path.move_to((4.0, 2.0));
    path.line_to((6.0, 6.0));
    path.line_to((0.0, 6.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_faux_quadralateral6bx(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((3.0, 0.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((6.0, 0.0));
    path.line_to((4.0, 2.0));
    path.close();
    path.move_to((4.0, 2.0));
    path.line_to((6.0, 6.0));
    path.line_to((0.0, 6.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_faux_quadralateral6c(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((3.0, 3.0));
    path.line_to((3.0, 0.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((6.0, 0.0));
    path.line_to((4.0, 2.0));
    path.close();
    path.move_to((4.0, 2.0));
    path.line_to((0.0, 6.0));
    path.line_to((6.0, 6.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_faux_quadralateral6cx(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((3.0, 3.0));
    path.line_to((3.0, 0.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((6.0, 0.0));
    path.line_to((4.0, 2.0));
    path.close();
    path.move_to((4.0, 2.0));
    path.line_to((0.0, 6.0));
    path.line_to((6.0, 6.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_faux_quadralateral6d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((3.0, 3.0));
    path.line_to((3.0, 0.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((6.0, 0.0));
    path.line_to((4.0, 2.0));
    path.close();
    path.move_to((4.0, 2.0));
    path.line_to((6.0, 6.0));
    path.line_to((0.0, 6.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_faux_quadralateral6dx(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((3.0, 3.0));
    path.line_to((3.0, 0.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((6.0, 0.0));
    path.line_to((4.0, 2.0));
    path.close();
    path.move_to((4.0, 2.0));
    path.line_to((6.0, 6.0));
    path.line_to((0.0, 6.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral6a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((3.0, 0.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((6.0, 0.0));
    path.line_to((0.0, 6.0));
    path.line_to((6.0, 6.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral6ax(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((3.0, 0.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((6.0, 0.0));
    path.line_to((0.0, 6.0));
    path.line_to((6.0, 6.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral7(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((2.0, 1.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.line_to((2.0, 2.0));
    path.line_to((1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral7x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((2.0, 1.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.line_to((2.0, 2.0));
    path.line_to((1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral8(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((3.0, 1.0));
    path.line_to((1.0, 3.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((2.0, 1.0));
    path.line_to((0.0, 2.0));
    path.line_to((3.0, 2.0));
    path.line_to((2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral8x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((3.0, 1.0));
    path.line_to((1.0, 3.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((2.0, 1.0));
    path.line_to((0.0, 2.0));
    path.line_to((3.0, 2.0));
    path.line_to((2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral9(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((1.0, 2.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((1.0, 1.0));
    path.line_to((2.0, 1.0));
    path.line_to((1.0, 3.0));
    path.line_to((2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral9x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((1.0, 2.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((1.0, 1.0));
    path.line_to((2.0, 1.0));
    path.line_to((1.0, 3.0));
    path.line_to((2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line1a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 0.0, 13.0, 13.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line1ax(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 0.0, 13.0, 13.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line2ax(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 20.0, 20.0, 20.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 20.0, 12.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(12.0, 0.0, 21.0, 21.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line3aax(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(10.0, 30.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(18.0, 20.0, 30.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(0.0, 32.0, 9.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line4ax(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(10.0, 30.0, 30.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(24.0, 20.0, 36.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(0.0, 32.0, 9.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 0.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic1x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 0.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 0.0));
    path.line_to((3.0, 0.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic2x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 0.0));
    path.line_to((3.0, 0.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 0.0));
    path.line_to((0.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic3x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 0.0));
    path.line_to((0.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 0.0));
    path.line_to((0.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic4x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 0.0));
    path.line_to((0.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic5(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 0.0));
    path.line_to((0.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((0.0, 1.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic6(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 0.0));
    path.line_to((2.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((2.0, 0.0), (0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic7(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 0.0));
    path.line_to((3.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((3.0, 0.0), (1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic8(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 0.0));
    path.line_to((0.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic9(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 0.0));
    path.line_to((3.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((1.0, 2.0), (3.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic14(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 0.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((3.0, 2.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic15(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 0.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((0.0, 1.0));
    path.quad_to((1.0, 1.0), (0.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic17x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (3.0, 1.0));
    path.line_to((0.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((3.0, 1.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic18(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.line_to((0.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (1.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic19(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.line_to((0.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((2.0, 0.0), (0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic20(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.line_to((0.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic21(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.line_to((0.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic22(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.line_to((0.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((0.0, 1.0), (2.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic23(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.line_to((0.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((0.0, 2.0), (1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic24(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.line_to((0.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((2.0, 0.0), (0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic25(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (1.0, 1.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((2.0, 1.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic26(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (1.0, 1.0));
    path.line_to((0.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic27(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (1.0, 1.0));
    path.line_to((2.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((2.0, 1.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic28(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.line_to((0.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 2.0));
    path.quad_to((1.0, 2.0), (0.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic29(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (2.0, 1.0));
    path.line_to((0.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic30(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (1.0, 2.0));
    path.line_to((1.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic31(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (1.0, 2.0));
    path.line_to((1.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic32(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (2.0, 3.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((3.0, 1.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic33(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((2.0, 0.0), (0.0, 1.0));
    path.line_to((0.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 1.0));
    path.quad_to((2.0, 1.0), (2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic34(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((2.0, 0.0), (0.0, 1.0));
    path.line_to((0.0, 1.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.quad_to((2.0, 1.0), (1.0, 2.0));
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic35(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 1.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((3.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic36(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((2.0, 1.0), (2.0, 3.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((3.0, 1.0));
    path.line_to((1.0, 2.0));
    path.quad_to((3.0, 2.0), (1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic37(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 2.0), (1.0, 2.0));
    path.line_to((1.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((3.0, 1.0));
    path.quad_to((0.0, 2.0), (1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic38(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 1.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 2.0));
    path.quad_to((2.0, 2.0), (1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic51(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((369.863983, 145.645813));
    path.quad_to((382.380371, 121.254936), (406.236359, 121.254936));
    path.line_to((369.863983, 145.645813));
    path.close();
    path.move_to((369.970581, 137.94342));
    path.quad_to((383.98465, 121.254936), (406.235992, 121.254936));
    path.line_to((369.970581, 137.94342));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic53(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((303.12088, 141.299606));
    path.line_to((330.463562, 217.659027));
    path.line_to((303.12088, 141.299606));
    path.close();
    path.move_to((371.919067, 205.854996));
    path.line_to((326.236786, 205.854996));
    path.quad_to((329.104431, 231.663818), (351.512085, 231.663818));
    path.line_to((371.919067, 205.854996));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic55(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((303.12088, 141.299606));
    path.line_to((330.463562, 217.659027));
    path.line_to((358.606506, 141.299606));
    path.line_to((303.12088, 141.299606));
    path.close();
    path.move_to((326.236786, 205.854996));
    path.quad_to((329.104431, 231.663818), (351.512085, 231.663818));
    path.line_to((326.236786, 205.854996));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic56(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((366.608826, 151.196014));
    path.quad_to((378.803101, 136.674606), (398.164948, 136.674606));
    path.line_to((354.009216, 208.816208));
    path.line_to((393.291473, 102.232819));
    path.line_to((359.978058, 136.581512));
    path.quad_to((378.315979, 136.581512), (388.322723, 149.613556));
    path.line_to((364.390686, 157.898193));
    path.quad_to((375.281769, 136.674606), (396.039917, 136.674606));
    path.line_to((350.0, 120.0));
    path.line_to((366.608826, 151.196014));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line80(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((4.0, 0.0));
    path.line_to((3.0, 7.0));
    path.line_to((7.0, 5.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((0.0, 6.0));
    path.line_to((6.0, 12.0));
    path.line_to((8.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic58(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((283.714233, 240.0));
    path.line_to((283.714233, 141.299606));
    path.line_to((303.12088, 141.299606));
    path.line_to((330.463562, 217.659027));
    path.line_to((358.606506, 141.299606));
    path.line_to((362.874634, 159.705902));
    path.line_to((335.665344, 233.397751));
    path.line_to((322.12738, 233.397751));
    path.line_to((295.718353, 159.505829));
    path.line_to((295.718353, 240.0));
    path.line_to((283.714233, 240.0));
    path.close();
    path.move_to((322.935669, 231.030273));
    path.quad_to((312.832214, 220.393295), (312.832214, 203.454178));
    path.quad_to((312.832214, 186.981888), (321.73526, 176.444946));
    path.quad_to((330.638306, 165.90802), (344.509705, 165.90802));
    path.quad_to((357.647522, 165.90802), (364.81665, 175.244537));
    path.line_to((371.919067, 205.854996));
    path.line_to((326.236786, 205.854996));
    path.quad_to((329.104431, 231.663818), (351.512085, 231.663818));
    path.line_to((322.935669, 231.030273));
    path.close();
    path.move_to((326.837006, 195.984955));
    path.line_to((358.78125, 195.984955));
    path.quad_to((358.78125, 175.778046), (343.709442, 175.778046));
    path.quad_to((328.570923, 175.778046), (326.837006, 195.984955));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic59x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 0.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 0.0));
    path.quad_to((3.0, 1.0), (1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic59(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 0.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 0.0));
    path.quad_to((3.0, 1.0), (1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic63(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 0.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((2.0, 1.0));
    path.quad_to((2.0, 1.0), (2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic64(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 0.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((1.0, 2.0));
    path.line_to((2.0, 2.0));
    path.quad_to((0.0, 3.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic65(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 0.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((2.0, 1.0));
    path.line_to((2.0, 2.0));
    path.quad_to((0.0, 3.0), (1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic67x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (2.0, 1.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 0.0));
    path.quad_to((1.0, 1.0), (3.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic68(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.line_to((1.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((0.0, 1.0), (2.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic69(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 1.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((1.0, 1.0));
    path.quad_to((3.0, 2.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic70x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.line_to((1.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((0.0, 1.0), (2.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic71(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (1.0, 1.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((1.0, 1.0), (3.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic72(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (1.0, 2.0));
    path.line_to((1.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((0.0, 1.0), (3.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic73(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 3.0));
    path.line_to((0.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic74(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (1.0, 3.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 1.0));
    path.quad_to((3.0, 2.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic75(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (1.0, 3.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 1.0));
    path.quad_to((3.0, 2.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic76(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 0.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 2.0));
    path.quad_to((1.0, 2.0), (2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic77(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (1.0, 1.0));
    path.line_to((3.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((0.0, 1.0), (3.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic78(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (1.0, 2.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((2.0, 1.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic79(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (1.0, 2.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((0.0, 1.0), (3.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_eight1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 2.0));
    path.line_to((0.0, 2.0));
    path.line_to((2.0, 0.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_eight2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 0.0));
    path.line_to((0.0, 2.0));
    path.line_to((2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_eight3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 2.0));
    path.line_to((2.0, 0.0));
    path.line_to((2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_eight4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 2.0));
    path.line_to((2.0, 0.0));
    path.line_to((0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_eight5(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 2.0));
    path.line_to((0.0, 2.0));
    path.line_to((2.0, 0.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_eight6(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.line_to((2.0, 0.0));
    path.line_to((0.0, 2.0));
    path.line_to((1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_eight7(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 1.0));
    path.line_to((2.0, 1.0));
    path.line_to((2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_eight8(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 2.0));
    path.line_to((2.0, 1.0));
    path.line_to((0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_eight9(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 2.0));
    path.line_to((2.0, 1.0));
    path.line_to((0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_eight10(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.line_to((0.0, 1.0));
    path.line_to((2.0, 1.0));
    path.line_to((1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic80(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (2.0, 3.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((3.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic81(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((2.0, 0.0), (1.0, 1.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((2.0, 1.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic82(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((2.0, 0.0), (1.0, 1.0));
    path.line_to((0.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((2.0, 1.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic83(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (2.0, 0.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((0.0, 1.0));
    path.line_to((0.0, 2.0));
    path.quad_to((2.0, 2.0), (1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic84(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((2.0, 0.0), (1.0, 1.0));
    path.line_to((2.0, 1.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((2.0, 0.0));
    path.quad_to((0.0, 1.0), (2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic85(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((3.0, 0.0), (1.0, 1.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((3.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic86(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 1.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((1.0, 1.0), (1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic87(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((2.0, 1.0), (0.0, 2.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 1.0));
    path.quad_to((0.0, 2.0), (3.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic88(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((2.0, 1.0), (0.0, 2.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.quad_to((0.0, 2.0), (2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic89x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.quad_to((3.0, 1.0), (2.0, 2.0));
    path.line_to((0.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 1.0));
    path.quad_to((3.0, 1.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic90x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.quad_to((3.0, 0.0), (2.0, 2.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 1.0));
    path.quad_to((3.0, 2.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic91(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((3.0, 2.0), (2.0, 3.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 1.0));
    path.quad_to((2.0, 1.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic92x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1.0, 0.0));
    path.quad_to((3.0, 0.0), (2.0, 2.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((0.0, 1.0));
    path.quad_to((3.0, 2.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line82(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(20.0, 0.0, 40.0, 40.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(24.0, 20.0, 36.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(24.0, 32.0, 33.0, 36.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line82a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 6.0, 10.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 4.0, 4.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 6.0, 4.0, 8.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line82b(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 6.0, 10.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 4.0, 4.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 6.0, 4.0, 8.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line82c(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 6.0, 10.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 4.0, 4.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 6.0, 4.0, 8.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line82d(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 6.0, 10.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 2.0, 4.0, 4.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 6.0, 4.0, 8.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line82e(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 6.0, 10.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 2.0, 4.0, 4.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 6.0, 4.0, 8.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line82f(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 6.0, 10.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 2.0, 4.0, 4.0), PathDirection::CW, None);
    path.add_rect(Rect::new(2.0, 6.0, 4.0, 8.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line82g(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 6.0, 10.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 2.0, 4.0, 4.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 6.0, 4.0, 8.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line82h(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 6.0, 10.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 2.0, 4.0, 4.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(2.0, 6.0, 4.0, 8.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line83(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(10.0, 30.0, 30.0, 40.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(0.0, 12.0, 12.0, 18.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(4.0, 13.0, 13.0, 16.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line84(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 12.0, 60.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(10.0, 20.0, 40.0, 30.0), PathDirection::CW, None);
    path.add_rect(Rect::new(0.0, 12.0, 12.0, 12.0), PathDirection::CW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line84x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 12.0, 60.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(10.0, 20.0, 40.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(0.0, 12.0, 12.0, 12.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(4.0, 12.0, 13.0, 13.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_line85(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(36.0, 0.0, 66.0, 60.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(20.0, 0.0, 40.0, 40.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(12.0, 0.0, 24.0, 24.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(32.0, 0.0, 36.0, 41.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 1.0));
    path.line_to((2.0, 2.0));
    path.line_to((2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_cubic1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.cubic_to((0.0, 1.0), (1.0, 1.0), (1.0, 0.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.cubic_to((0.0, 0.0), (0.0, 1.0), (1.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic93(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((3.0, 0.0));
    path.quad_to((0.0, 1.0), (3.0, 2.0));
    path.line_to((0.0, 3.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((2.0, 0.0));
    path.quad_to((1.0, 1.0), (2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_cubic2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0,2.0));
    path.cubic_to((0.0, 3.0), (2.0, 1.0), (4.0, 0.0));
    path.close();
    path.move_to((1.0,2.0));
    path.cubic_to((0.0, 4.0), (2.0, 0.0), (3.0, 0.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0,0.0));
    path.quad_to((0.0, 0.0), (0.0, 1.0));
    path.line_to((1.0,1.0));
    path.close();
    path.move_to((0.0,0.0));
    path.quad_to((1.0, 1.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 2.0));
    path.line_to((0.0, 3.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((3.0, 0.0));
    path.line_to((0.0, 1.0));
    path.line_to((1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic94(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((8.0, 8.0));
    path.quad_to((8.0, 4.0), (4.0, 4.0));
    path.quad_to((4.0, 0.0), (0.0, 0.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic95(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((8.0, 8.0));
    path.line_to((0.0, 0.0));
    path.quad_to((4.0, 0.0), (4.0, 4.0));
    path.quad_to((8.0, 4.0), (8.0, 8.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic96(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((8.0, 0.0));
    path.line_to((0.0, 8.0));
    path.quad_to((0.0, 4.0), (4.0, 4.0));
    path.quad_to((4.0, 0.0), (8.0, 0.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadratic97(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 8.0));
    path.line_to((8.0, 0.0));
    path.quad_to((4.0, 0.0), (4.0, 4.0));
    path.quad_to((0.0, 4.0), (0.0, 8.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_triangles1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 2.0));
    path.line_to((1.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_triangles2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((1.0, 1.0));
    path.line_to((2.0, 3.0));
    path.line_to((1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_add_tcoincident1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.line_to((2.0, 2.0));
    path.line_to((1.0, 1.0));
    path.line_to((2.0, 0.0));
    path.line_to((2.0, 2.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((2.0, 2.0));
    path.line_to((3.0, 1.0));
    path.line_to((2.0, 0.0));
    path.line_to((2.0, 2.0));
    path.line_to((3.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_add_tcoincident2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.line_to((2.0, 2.0));
    path.line_to((1.0, 1.0));
    path.line_to((2.0, 0.0));
    path.line_to((2.0, 2.0));
    path.line_to((1.0, 1.0));
    path.move_to((2.0, 0.0));
    path.line_to((2.0, 2.0));
    path.line_to((3.0, 1.0));
    path.line_to((2.0, 0.0));
    path.line_to((2.0, 2.0));
    path.line_to((3.0, 1.0));
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad2(_reporter: &mut Reporter, _filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.quad_to((0.0, 1.0), (3.0, 2.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 1.0));
    path.close();
}

fn test_quad3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.quad_to((0.0, 1.0), (3.0, 2.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 1.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 0.0));
    path.quad_to((0.0, 1.0), (2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad5(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.quad_to((0.0, 1.0), (2.0, 2.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad6(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.quad_to((0.0, 1.0), (2.0, 2.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((2.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad7(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((3.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 1.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((3.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad_line_intersect1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((3.0, 1.0), (0.0, 3.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((0.0, 1.0));
    path.quad_to((3.0, 1.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad_line_intersect2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((3.0, 1.0), (0.0, 3.0));
    path.line_to((0.0, 3.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((0.0, 1.0));
    path.quad_to((3.0, 1.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad_line_intersect3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((3.0, 1.0), (0.0, 3.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((0.0, 1.0));
    path.quad_to((3.0, 1.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn skphealth_com76s(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((708.099182, 7.09919119));
    path.line_to((708.099182, 7.09920025));
    path.quad_to((704.000000, 11.2010098), (704.000000, 17.0000000));
    path.line_to((704.000000, 33.0000000));
    path.line_to((705.000000, 33.0000000));
    path.line_to((705.000000, 17.0000000));
    path.cubic_to((705.000000, 13.4101496), (706.455078, 10.1601505), (708.807617, 7.80761385));
    path.line_to((708.099182, 7.09919119));
    path.close();
    path.move_to((704.000000, 3.00000000));
    path.line_to((704.000000, 33.0000000));
    path.line_to((705.000000, 33.0000000));
    path.line_to((719.500000, 3.00000000));
    test_simplify(reporter, &path.detach(), filename);
}

fn too_close_test(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 1.0));
    path.line_to((1.0,-1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0,-2.0));
    path.line_to((1.0, 2.0));
    path.line_to((2.0, 0.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_rect1s(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(30.0, 20.0, 50.0, 50.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(24.0, 20.0, 36.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(32.0, 24.0, 36.0, 41.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_rect2s(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 0.0));
    path.line_to((60.0, 0.0));
    path.line_to((60.0, 60.0));
    path.line_to((0.0, 60.0));
    path.close();
    path.move_to((30.0, 20.0));
    path.line_to((30.0, 50.0));
    path.line_to((50.0, 50.0));
    path.line_to((50.0, 20.0));
    path.close();
    path.move_to((24.0, 20.0));
    path.line_to((24.0, 30.0));
    path.line_to((36.0, 30.0));
    path.line_to((36.0, 20.0));
    path.close();
    path.move_to((32.0, 24.0));
    path.line_to((32.0, 41.0));
    path.line_to((36.0, 41.0));
    path.line_to((36.0, 24.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_triangles3x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1.0, 0.0));
    path.quad_to((0.0, 1.0), (3.0, 2.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 1.0));
    path.quad_to((2.0, 1.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad8(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((3.0, 0.0));
    path.quad_to((0.0, 1.0), (3.0, 2.0));
    path.line_to((0.0, 3.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((3.0, 0.0));
    path.quad_to((1.0, 1.0), (2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_triangles4x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.quad_to((2.0, 0.0), (0.0, 3.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 1.0));
    path.quad_to((3.0, 2.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad9(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1.0, 0.0));
    path.quad_to((0.0, 1.0), (3.0, 2.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.quad_to((2.0, 1.0), (1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad10(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.quad_to((0.0, 1.0), (3.0, 2.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((2.0, 0.0));
    path.quad_to((2.0, 3.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad11(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 2.0));
    path.line_to((1.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 1.0));
    path.quad_to((1.0, 3.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad12(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 0.0));
    path.line_to((0.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_degenerate5(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((0.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((3.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((0.0, 1.0));
    path.line_to((3.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_degenerates1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 1.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((3.0, 2.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad13(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 1.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((3.0, 2.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad14(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 1.0));
    path.line_to((1.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((3.0, 1.0), (1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quad15(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 1.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((2.0, 0.0), (1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads16(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 1.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((0.0, 1.0), (3.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads17(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 1.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((0.0, 2.0), (3.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads18(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 1.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((1.0, 2.0), (3.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads19(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (2.0, 1.0));
    path.line_to((1.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((2.0, 1.0), (1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads20(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (2.0, 1.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((2.0, 1.0), (1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads21(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 1.0));
    path.line_to((2.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((3.0, 0.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads22(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (2.0, 0.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((0.0, 1.0), (3.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads23(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((0.0, 1.0), (3.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads24(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 1.0));
    path.line_to((1.0, 2.0));
    path.close();
    path.move_to((0.0, 1.0));
    path.line_to((0.0, 1.0));
    path.quad_to((0.0, 2.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads25(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 1.0));
    path.line_to((2.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((3.0, 0.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads26(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (3.0, 0.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((0.0, 1.0), (3.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads27(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (2.0, 0.0));
    path.line_to((2.0, 1.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((2.0, 0.0));
    path.quad_to((3.0, 0.0), (1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads28(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (0.0, 1.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((3.0, 0.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads29(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (3.0, 0.0));
    path.line_to((2.0, 1.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((3.0, 0.0));
    path.quad_to((3.0, 1.0), (0.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads30(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (2.0, 0.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((2.0, 0.0));
    path.quad_to((3.0, 2.0), (1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads31(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (2.0, 1.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((0.0, 1.0));
    path.quad_to((2.0, 1.0), (1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads32(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 1.0));
    path.line_to((1.0, 2.0));
    path.close();
    path.move_to((1.0, 1.0));
    path.line_to((1.0, 1.0));
    path.quad_to((3.0, 1.0), (0.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads33(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 1.0));
    path.line_to((2.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((3.0, 0.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads34(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 0.0));
    path.line_to((2.0, 1.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((2.0, 0.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads35(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 0.0));
    path.line_to((1.0, 2.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((3.0, 1.0), (0.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads36(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.quad_to((2.0, 0.0), (1.0, 2.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((3.0, 0.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads37(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((1.0, 0.0));
    path.quad_to((2.0, 0.0), (1.0, 2.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((3.0, 0.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads38(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.quad_to((3.0, 0.0), (0.0, 2.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((2.0, 1.0), (3.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads39(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.quad_to((3.0, 0.0), (0.0, 3.0));
    path.line_to((0.0, 3.0));
    path.close();
    path.move_to((1.0, 1.0));
    path.line_to((0.0, 2.0));
    path.quad_to((1.0, 2.0), (0.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads40(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.quad_to((3.0, 0.0), (3.0, 3.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((2.0, 1.0));
    path.line_to((2.0, 2.0));
    path.quad_to((3.0, 2.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads41(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (1.0, 0.0));
    path.line_to((2.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads54(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.quad_to((2.0, 0.0), (1.0, 1.0));
    path.line_to((3.0, 1.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((1.0, 1.0));
    path.quad_to((1.0, 1.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads53(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.quad_to((2.0, 0.0), (1.0, 1.0));
    path.line_to((3.0, 1.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((1.0, 1.0));
    path.quad_to((2.0, 3.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads52(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.quad_to((2.0, 0.0), (1.0, 1.0));
    path.line_to((3.0, 1.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((1.0, 1.0));
    path.quad_to((2.0, 3.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads51(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.quad_to((3.0, 0.0), (2.0, 1.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((3.0, 1.0));
    path.quad_to((3.0, 1.0), (1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads50(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.quad_to((3.0, 0.0), (2.0, 1.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((3.0, 1.0));
    path.quad_to((1.0, 2.0), (1.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads49(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.quad_to((3.0, 0.0), (2.0, 1.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((2.0, 2.0));
    path.quad_to((2.0, 2.0), (0.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads48(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.quad_to((3.0, 0.0), (2.0, 1.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((2.0, 2.0));
    path.quad_to((3.0, 2.0), (0.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads47(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.quad_to((3.0, 0.0), (2.0, 1.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((2.0, 2.0));
    path.quad_to((0.0, 3.0), (0.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads46x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((2.0, 0.0));
    path.quad_to((0.0, 1.0), (3.0, 2.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 1.0));
    path.quad_to((3.0, 2.0), (1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads45(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.quad_to((3.0, 2.0), (3.0, 3.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 2.0));
    path.quad_to((3.0, 2.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads44(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.quad_to((3.0, 2.0), (3.0, 3.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((0.0, 2.0));
    path.quad_to((3.0, 2.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads43(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.quad_to((2.0, 3.0), (3.0, 3.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((0.0, 2.0));
    path.line_to((0.0, 2.0));
    path.quad_to((2.0, 3.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads42(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.quad_to((3.0, 2.0), (3.0, 3.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((0.0, 2.0));
    path.quad_to((3.0, 2.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads56(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.quad_to((3.0, 1.0), (0.0, 2.0));
    path.line_to((3.0, 2.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((2.0, 1.0));
    path.quad_to((2.0, 1.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads57(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.quad_to((3.0, 0.0), (3.0, 1.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((3.0, 1.0));
    path.quad_to((2.0, 2.0), (3.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads58(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.quad_to((3.0, 0.0), (3.0, 1.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((3.0, 1.0));
    path.quad_to((2.0, 2.0), (3.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads59(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((3.0, 0.0));
    path.quad_to((3.0, 1.0), (3.0, 1.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((3.0, 1.0));
    path.quad_to((2.0, 2.0), (3.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads60(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 1.0));
    path.quad_to((0.0, 2.0), (3.0, 2.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 0.0));
    path.quad_to((1.0, 1.0), (2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads61(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 0.0), (2.0, 0.0));
    path.line_to((1.0, 1.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((1.0, 0.0), (2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quadralateral10(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.line_to((2.0, 2.0));
    path.line_to((1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_rect3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 60.0, 60.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(10.0, 30.0, 40.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(24.0, 6.0, 36.0, 36.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(32.0, 6.0, 36.0, 41.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_rect4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.add_rect(Rect::new(0.0, 0.0, 30.0, 60.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(10.0, 0.0, 40.0, 30.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(20.0, 0.0, 30.0, 40.0), PathDirection::CCW, None);
    path.add_rect(Rect::new(32.0, 0.0, 36.0, 41.0), PathDirection::CCW, None);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads62(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((3.0, 2.0));
    path.quad_to((1.0, 3.0), (3.0, 3.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((2.0, 0.0));
    path.quad_to((1.0, 3.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads63(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((3.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 2.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 1.0));
    path.quad_to((0.0, 2.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads64(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((3.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 2.0));
    path.line_to((2.0, 2.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 1.0));
    path.quad_to((0.0, 2.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_triangle1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 2.0));
    path.line_to((1.0, 0.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_triangle2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.line_to((0.0, 1.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((0.0, 2.0));
    path.line_to((2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_arc(reporter: &mut Reporter, filename: &str) {
    let r = Rect::new(0.0, 0.0, 150.0, 100.0);
    let mut path = PathBuilder::new();
    path.arc_to(r, 0.0, 0.0025, false);
    test_simplify(reporter, &path.detach(), filename);
}

fn test_issue3838(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((220.0, 170.0));
    path.line_to((200.0, 170.0));
    path.line_to((200.0, 190.0));
    path.line_to((180.0, 190.0));
    path.line_to((180.0, 210.0));
    path.line_to((200.0, 210.0));
    path.line_to((200.0, 250.0));
    path.line_to((260.0, 250.0));
    path.line_to((260.0, 190.0));
    path.line_to((220.0, 190.0));
    path.line_to((220.0, 170.0));
    path.close();
    path.move_to((220.0, 210.0));
    path.line_to((220.0, 230.0));
    path.line_to((240.0, 230.0));
    path.line_to((240.0, 210.0));
    path.line_to((220.0, 210.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_issue3838_3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((40.0, 10.0));
    path.line_to((60.0, 10.0));
    path.line_to((60.0, 30.0));
    path.line_to((40.0, 30.0));
    path.line_to((40.0, 10.0));
    path.move_to((41.0, 11.0));
    path.line_to((41.0, 29.0));
    path.line_to((59.0, 29.0));
    path.line_to((59.0, 11.0));
    path.line_to((41.0, 11.0));
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads65(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 2.0));
    path.quad_to((3.0, 2.0), (0.0, 3.0));
    path.line_to((1.0, 3.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 2.0));
    path.quad_to((3.0, 2.0), (1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn fuzz864a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((10.0, 90.0));
    path.line_to((10.0, 90.0));
    path.line_to((10.0, 30.0));
    path.line_to((10.0, 30.0));
    path.line_to((10.0, 90.0));
    path.close();
    path.move_to((10.0, 90.0));
    path.line_to((10.0, 90.0));
    path.line_to((10.0, 30.0));
    path.line_to((10.0, 30.0));
    path.line_to((10.0, 90.0));
    path.close();
    path.move_to((10.0, 90.0));
    path.line_to((110.0, 90.0));
    path.line_to((110.0, 30.0));
    path.line_to((10.0, 30.0));
    path.line_to((10.0, 90.0));
    path.close();
    path.move_to((10.0, 30.0));
    path.line_to((32678.0, 30.0));
    path.line_to((32678.0, 30.0));
    path.line_to((10.0, 30.0));
    path.close();
    path.move_to((10.0, 3.35545e+07));
    path.line_to((110.0, 3.35545e+07));
    path.line_to((110.0, 30.0));
    path.line_to((10.0, 30.0));
    path.line_to((10.0, 3.35545e+07));
    path.close();
    path.move_to((10.0, 315.0));
    path.line_to((110.0, 315.0));
    path.line_to((110.0, 255.0));
    path.line_to((10.0, 255.0));
    path.line_to((10.0, 315.0));
    path.close();
    path.move_to((0.0, 60.0));
    path.line_to((100.0, 60.0));
    path.line_to((100.0, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((0.0, 60.0));
    path.close();
    path.move_to((10.0, 90.0));
    path.line_to((110.0, 90.0));
    path.line_to((110.0, 30.0));
    path.line_to((10.0, 30.0));
    path.line_to((10.0, 90.0));
    path.close();
    path.move_to((10.0, 3.35545e+07));
    path.line_to((110.0, 3.35545e+07));
    path.line_to((110.0, 30.0));
    path.line_to((10.0, 30.0));
    path.line_to((10.0, 3.35545e+07));
    path.close();
    path.move_to((10.0, 90.0));
    path.line_to((110.0, 90.0));
    path.line_to((110.0, 30.0));
    path.line_to((10.0, 30.0));
    path.line_to((10.0, 90.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn cr514118(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x42c80000), f32::from_bits(0x42480000)));
    // 100, 50
    path.conic_to((f32::from_bits(0x42c80000), f32::from_bits(0x00000000)), (f32::from_bits(0x42480000), f32::from_bits(0x00000000)), f32::from_bits(0x3f3504f3));
    // 100, 0, 50, 0, 0.707107f
    path.conic_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)), (f32::from_bits(0x00000000), f32::from_bits(0x42480000)), f32::from_bits(0x3f3504f3));
    // 0, 0, 0, 50, 0.707107f
    path.conic_to((f32::from_bits(0x00000000), f32::from_bits(0x42c80000)), (f32::from_bits(0x42480000), f32::from_bits(0x42c80000)), f32::from_bits(0x3f3504f3));
    // 0, 100, 50, 100, 0.707107f
    path.conic_to((f32::from_bits(0x42c80000), f32::from_bits(0x42c80000)), (f32::from_bits(0x42c80000), f32::from_bits(0x42480000)), f32::from_bits(0x3f3504f3));
    // 100, 100, 100, 50, 0.707107f
    path.close();
    path.move_to((f32::from_bits(0x42c80133), f32::from_bits(0x42480000)));
    // 100.002f, 50
    path.conic_to((f32::from_bits(0x42c80133), f32::from_bits(0x00000000)), (f32::from_bits(0x42480267), f32::from_bits(0x00000000)), f32::from_bits(0x3f3504f3));
    // 100.002f, 0, 50.0023f, 0, 0.707107f
    path.conic_to((f32::from_bits(0x3b19b530), f32::from_bits(0x00000000)), (f32::from_bits(0x3b19b530), f32::from_bits(0x42480000)), f32::from_bits(0x3f3504f3));
    // 0.00234539f, 0, 0.00234539f, 50, 0.707107f
    path.conic_to((f32::from_bits(0x3b19b530), f32::from_bits(0x42c80000)), (f32::from_bits(0x42480267), f32::from_bits(0x42c80000)), f32::from_bits(0x3f3504f3));
    // 0.00234539f, 100, 50.0023f, 100, 0.707107f
    path.conic_to((f32::from_bits(0x42c80133), f32::from_bits(0x42c80000)), (f32::from_bits(0x42c80133), f32::from_bits(0x42480000)), f32::from_bits(0x3f3504f3));
    // 100.002f, 100, 100.002f, 50, 0.707107f
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn fuzz994s_11(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x41200000), f32::from_bits(0x42b40000)));
    // 10, 90
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x42b40000)));
    // 10, 90
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x41f00000)));
    // 10, 30
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x41f00000)));
    // 10, 30
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x42b40000)));
    // 10, 90
    path.close();
    path.move_to((f32::from_bits(0x41200000), f32::from_bits(0x42b40000)));
    // 10, 90
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x42b40000)));
    // 10, 90
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x41f00000)));
    // 10, 30
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x41f00000)));
    // 10, 30
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x42b40000)));
    // 10, 90
    path.close();
    path.move_to((f32::from_bits(0x41200000), f32::from_bits(0x42b40000)));
    // 10, 90
    path.line_to((f32::from_bits(0x42dc0000), f32::from_bits(0x42b40000)));
    // 110, 90
    path.line_to((f32::from_bits(0x42dc0000), f32::from_bits(0x41f00000)));
    // 110, 30
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x41f00000)));
    // 10, 30
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x42b40000)));
    // 10, 90
    path.close();
    path.move_to((f32::from_bits(0x41200000), f32::from_bits(0x41f00000)));
    // 10, 30
    path.line_to((f32::from_bits(0x46ff4c00), f32::from_bits(0x41f00000)));
    // 32678, 30
    path.line_to((f32::from_bits(0x46ff4c00), f32::from_bits(0x41f00000)));
    // 32678, 30
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x41f00000)));
    // 10, 30
    path.close();
    path.move_to((f32::from_bits(0x41200000), f32::from_bits(0x4c000006)));
    // 10, 3.35545e+07f
    path.line_to((f32::from_bits(0x42dc0000), f32::from_bits(0x4c000006)));
    // 110, 3.35545e+07f
    path.line_to((f32::from_bits(0x42dc0000), f32::from_bits(0x41f00000)));
    // 110, 30
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x41f00000)));
    // 10, 30
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x4c000006)));
    // 10, 3.35545e+07f
    path.close();
    path.move_to((f32::from_bits(0x41200000), f32::from_bits(0x439d8000)));
    // 10, 315
    path.line_to((f32::from_bits(0x42dc0000), f32::from_bits(0x439d8000)));
    // 110, 315
    path.line_to((f32::from_bits(0x42dc0000), f32::from_bits(0x437f0000)));
    // 110, 255
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x437f0000)));
    // 10, 255
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x439d8000)));
    // 10, 315
    path.close();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x42700000)));
    // 0, 60
    path.line_to((f32::from_bits(0x42c80000), f32::from_bits(0x42700000)));
    // 100, 60
    path.line_to((f32::from_bits(0x42c80000), f32::from_bits(0x00000000)));
    // 100, 0
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    // 0, 0
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x42700000)));
    // 0, 60
    path.close();
    path.move_to((f32::from_bits(0x41200000), f32::from_bits(0x42b40000)));
    // 10, 90
    path.line_to((f32::from_bits(0x42dc0000), f32::from_bits(0x42b40000)));
    // 110, 90
    path.line_to((f32::from_bits(0x42dc0000), f32::from_bits(0x41f00000)));
    // 110, 30
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x41f00000)));
    // 10, 30
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x42b40000)));
    // 10, 90
    path.close();
    path.move_to((f32::from_bits(0x41200000), f32::from_bits(0x4c000006)));
    // 10, 3.35545e+07f
    path.line_to((f32::from_bits(0x42dc0000), f32::from_bits(0x4c000006)));
    // 110, 3.35545e+07f
    path.line_to((f32::from_bits(0x42dc0000), f32::from_bits(0x41f00000)));
    // 110, 30
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x41f00000)));
    // 10, 30
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x4c000006)));
    // 10, 3.35545e+07f
    path.close();
    path.move_to((f32::from_bits(0x41200000), f32::from_bits(0x42b40000)));
    // 10, 90
    path.line_to((f32::from_bits(0x42dc0000), f32::from_bits(0x42b40000)));
    // 110, 90
    path.line_to((f32::from_bits(0x42dc0000), f32::from_bits(0x41f00000)));
    // 110, 30
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x41f00000)));
    // 10, 30
    path.line_to((f32::from_bits(0x41200000), f32::from_bits(0x42b40000)));
    // 10, 90
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn fuzz994s_3414(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42c80000), f32::from_bits(0x42480000)));
    // 100, 50
    path.conic_to((f32::from_bits(0x42c80000), f32::from_bits(0x00000000)), (f32::from_bits(0x42480000), f32::from_bits(0x00000000)), f32::from_bits(0x3f3504f3));
    // 100, 0, 50, 0, 0.707107f
    path.conic_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)), (f32::from_bits(0x00000000), f32::from_bits(0x42480000)), f32::from_bits(0x3f3504f3));
    // 0, 0, 0, 50, 0.707107f
    path.conic_to((f32::from_bits(0x00000000), f32::from_bits(0x42c80000)), (f32::from_bits(0x42480000), f32::from_bits(0x42c80000)), f32::from_bits(0x3f3504f3));
    // 0, 100, 50, 100, 0.707107f
    path.conic_to((f32::from_bits(0x42c80000), f32::from_bits(0x42c80000)), (f32::from_bits(0x42c80000), f32::from_bits(0x42480000)), f32::from_bits(0x3f3504f3));
    // 100, 100, 100, 50, 0.707107f
    path.close();
    path.move_to((f32::from_bits(0x42c84964), f32::from_bits(0x42480000)));
    // 100.143f, 50
    path.conic_to((f32::from_bits(0x42c84964), f32::from_bits(0x00000000)), (f32::from_bits(0x424892c8), f32::from_bits(0x00000000)), f32::from_bits(0x3f3504f3));
    // 100.143f, 0, 50.1433f, 0, 0.707107f
    path.conic_to((f32::from_bits(0x3e12c788), f32::from_bits(0x00000000)), (f32::from_bits(0x3e12c788), f32::from_bits(0x42480000)), f32::from_bits(0x3f3504f3));
    // 0.143339f, 0, 0.143339f, 50, 0.707107f
    path.conic_to((f32::from_bits(0x3e12c788), f32::from_bits(0x42c80000)), (f32::from_bits(0x424892c8), f32::from_bits(0x42c80000)), f32::from_bits(0x3f3504f3));
    // 0.143339f, 100, 50.1433f, 100, 0.707107f
    path.conic_to((f32::from_bits(0x42c84964), f32::from_bits(0x42c80000)), (f32::from_bits(0x42c84964), f32::from_bits(0x42480000)), f32::from_bits(0x3f3504f3));
    // 100.143f, 100, 100.143f, 50, 0.707107f
    path.close();
    path.move_to((f32::from_bits(0x42c80000), f32::from_bits(0x42480000)));
    // 100, 50
    path.conic_to((f32::from_bits(0x42c80000), f32::from_bits(0x00000000)), (f32::from_bits(0x42480000), f32::from_bits(0x00000000)), f32::from_bits(0x3f3504f3));
    // 100, 0, 50, 0, 0.707107f
    path.conic_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)), (f32::from_bits(0x00000000), f32::from_bits(0x42480000)), f32::from_bits(0x3f3504f3));
    // 0, 0, 0, 50, 0.707107f
    path.conic_to((f32::from_bits(0x00000000), f32::from_bits(0x42c80000)), (f32::from_bits(0x42480000), f32::from_bits(0x42c80000)), f32::from_bits(0x3f3504f3));
    // 0, 100, 50, 100, 0.707107f
    path.conic_to((f32::from_bits(0x42c80000), f32::from_bits(0x42c80000)), (f32::from_bits(0x42c80000), f32::from_bits(0x42480000)), f32::from_bits(0x3f3504f3));
    // 100, 100, 100, 50, 0.707107f
    path.close();
    path.move_to((f32::from_bits(0x4c00006b), f32::from_bits(0x424c0000)));
    // 3.35549e+07f, 51
    path.conic_to((f32::from_bits(0x4c00006b), f32::from_bits(0xcbffffe5)), (f32::from_bits(0x43d6e720), f32::from_bits(0xcbffffe5)), f32::from_bits(0x3f3504f3));
    // 3.35549e+07f, -3.35544e+07f, 429.806f, -3.35544e+07f, 0.707107f
    path.conic_to((f32::from_bits(0xcbffff28), f32::from_bits(0xcbffffe5)), (f32::from_bits(0xcbffff28), f32::from_bits(0x424c0000)), f32::from_bits(0x3f3504f3));
    // -3.3554e+07f, -3.35544e+07f, -3.3554e+07f, 51, 0.707107f
    path.conic_to((f32::from_bits(0xcbffff28), f32::from_bits(0x4c00000c)), (f32::from_bits(0x43d6e720), f32::from_bits(0x4c00000c)), f32::from_bits(0x3f3504f3));
    // -3.3554e+07f, 3.35545e+07f, 429.806f, 3.35545e+07f, 0.707107f
    path.conic_to((f32::from_bits(0x4c00006b), f32::from_bits(0x4c00000c)), (f32::from_bits(0x4c00006b), f32::from_bits(0x424c0000)), f32::from_bits(0x3f3504f3));
    // 3.35549e+07f, 3.35545e+07f, 3.35549e+07f, 51, 0.707107f
    path.close();
    path.move_to((f32::from_bits(0x43ef6720), f32::from_bits(0x42480000)));
    // 478.806f, 50
    path.conic_to((f32::from_bits(0x43ef6720), f32::from_bits(0x00000000)), (f32::from_bits(0x43d66720), f32::from_bits(0x00000000)), f32::from_bits(0x3f3504f3));
    // 478.806f, 0, 428.806f, 0, 0.707107f
    path.conic_to((f32::from_bits(0x43bd6720), f32::from_bits(0x00000000)), (f32::from_bits(0x43bd6720), f32::from_bits(0x42480000)), f32::from_bits(0x3f3504f3));
    // 378.806f, 0, 378.806f, 50, 0.707107f
    path.conic_to((f32::from_bits(0x43bd6720), f32::from_bits(0x42c80000)), (f32::from_bits(0x43d66720), f32::from_bits(0x42c80000)), f32::from_bits(0x3f3504f3));
    // 378.806f, 100, 428.806f, 100, 0.707107f
    path.conic_to((f32::from_bits(0x43ef6720), f32::from_bits(0x42c80000)), (f32::from_bits(0x43ef6720), f32::from_bits(0x42480000)), f32::from_bits(0x3f3504f3));
    // 478.806f, 100, 478.806f, 50, 0.707107f
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn fuzz_twister(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((0.0, 600.0));
    path.line_to((3.35544e+07, 600.0));
    path.line_to((3.35544e+07, 0.0));
    path.line_to((0.0, 0.0));
    path.line_to((0.0, 600.0));
    path.close();
    path.move_to((63.0, 600.0));
    path.line_to((3.35545e+07, 600.0));
    path.line_to((3.35545e+07, 0.0));
    path.line_to((63.0, 0.0));
    path.line_to((63.0, 600.0));
    path.close();
    path.move_to((93.0, 600.0));
    path.line_to((3.35545e+07, 600.0));
    path.line_to((3.35545e+07, 0.0));
    path.line_to((93.0, 0.0));
    path.line_to((93.0, 600.0));
    path.close();
    path.move_to((123.0, 600.0));
    path.line_to((3.35546e+07, 600.0));
    path.line_to((3.35546e+07, 0.0));
    path.line_to((123.0, 0.0));
    path.line_to((123.0, 600.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn fuzz_twister2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x44160000)));
    // 0, 600
    path.line_to((f32::from_bits(0x4bfffffe), f32::from_bits(0x44160000)));
    // 3.35544e+07f, 600
    path.line_to((f32::from_bits(0x4bfffffe), f32::from_bits(0x00000000)));
    // 3.35544e+07f, 0
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x00000000)));
    // 0, 0
    path.line_to((f32::from_bits(0x00000000), f32::from_bits(0x44160000)));
    // 0, 600
    path.close();
    path.move_to((f32::from_bits(0x427c0000), f32::from_bits(0x00000000)));
    // 63, 0
    path.line_to((f32::from_bits(0x4c00000f), f32::from_bits(0x00000000)));
    // 3.35545e+07f, 0
    path.line_to((f32::from_bits(0x4c00000f), f32::from_bits(0x00000000)));
    // 3.35545e+07f, 0
    path.line_to((f32::from_bits(0x427c0000), f32::from_bits(0x00000000)));
    // 63, 0
    path.close();
    path.move_to((f32::from_bits(0x42ba0000), f32::from_bits(0x00000000)));
    // 93, 0
    path.line_to((f32::from_bits(0x4c000016), f32::from_bits(0x00000000)));
    // 3.35545e+07f, 0
    path.line_to((f32::from_bits(0x4c000016), f32::from_bits(0x00000000)));
    // 3.35545e+07f, 0
    path.line_to((f32::from_bits(0x42ba0000), f32::from_bits(0x00000000)));
    // 93, 0
    path.close();
    path.move_to((f32::from_bits(0x42f60000), f32::from_bits(0x00000000)));
    // 123, 0
    path.line_to((f32::from_bits(0x4c00001e), f32::from_bits(0x00000000)));
    // 3.35546e+07f, 0
    path.line_to((f32::from_bits(0x4c00001e), f32::from_bits(0x00000000)));
    // 3.35546e+07f, 0
    path.line_to((f32::from_bits(0x42f60000), f32::from_bits(0x00000000)));
    // 123, 0
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn fuzz763_4713_b(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42240000), f32::from_bits(0x42040000)));
    path.quad_to((f32::from_bits(0x42240000), f32::from_bits(0x4211413d)), (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)));
    path.quad_to((f32::from_bits(0x4211413d), f32::from_bits(0x42240000)), (f32::from_bits(0x42040000), f32::from_bits(0x42240000)));
    path.quad_to((f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)), (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)));
    path.quad_to((f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)), (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)));
    path.quad_to((f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)), (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)));
    path.quad_to((f32::from_bits(0x41ed7d86), f32::from_bits(0x41c80000)), (f32::from_bits(0x42040000), f32::from_bits(0x41c80000)));
    path.quad_to((f32::from_bits(0x4211413d), f32::from_bits(0x41c80000)), (f32::from_bits(0x421aa09e), f32::from_bits(0x41dabec3)));
    path.quad_to((f32::from_bits(0x42240000), f32::from_bits(0x41ed7d86)), (f32::from_bits(0x42240000), f32::from_bits(0x42040000)));
    path.close();
    path.move_to((f32::from_bits(0x4204f72e), f32::from_bits(0x41c56cd2)));
    path.quad_to((f32::from_bits(0x42123842), f32::from_bits(0x41c52adf)), (f32::from_bits(0x421baed7), f32::from_bits(0x41d7bac6)));
    path.quad_to((f32::from_bits(0x4225256d), f32::from_bits(0x41ea4aad)), (f32::from_bits(0x42254667), f32::from_bits(0x4202666b)));
    path.quad_to((f32::from_bits(0x42256760), f32::from_bits(0x420fa77f)), (f32::from_bits(0x421c1f6c), f32::from_bits(0x42191e14)));
    path.quad_to((f32::from_bits(0x421bff97), f32::from_bits(0x42193e89)), (f32::from_bits(0x421bdf6b), f32::from_bits(0x42195eb8)));
    path.quad_to((f32::from_bits(0x421bbff6), f32::from_bits(0x42197f32)), (f32::from_bits(0x421ba03b), f32::from_bits(0x42199f57)));
    path.quad_to((f32::from_bits(0x421b605e), f32::from_bits(0x4219e00a)), (f32::from_bits(0x421b1fa8), f32::from_bits(0x421a1f22)));
    path.quad_to((f32::from_bits(0x421ae0f1), f32::from_bits(0x421a604b)), (f32::from_bits(0x421aa09e), f32::from_bits(0x421aa09e)));
    path.quad_to((f32::from_bits(0x4211413d), f32::from_bits(0x42240000)), (f32::from_bits(0x42040000), f32::from_bits(0x42240000)));
    path.quad_to((f32::from_bits(0x41ed7d86), f32::from_bits(0x42240000)), (f32::from_bits(0x41dabec3), f32::from_bits(0x421aa09e)));
    path.quad_to((f32::from_bits(0x41c80000), f32::from_bits(0x4211413d)), (f32::from_bits(0x41c80000), f32::from_bits(0x42040000)));
    path.quad_to((f32::from_bits(0x41c80000), f32::from_bits(0x41ed7d86)), (f32::from_bits(0x41dabec3), f32::from_bits(0x41dabec3)));
    path.quad_to((f32::from_bits(0x41db19b1), f32::from_bits(0x41da63d5)), (f32::from_bits(0x41db755b), f32::from_bits(0x41da0a9b)));
    path.quad_to((f32::from_bits(0x41dbce01), f32::from_bits(0x41d9ae59)), (f32::from_bits(0x41dc285e), f32::from_bits(0x41d952ce)));
    path.quad_to((f32::from_bits(0x41dc55b6), f32::from_bits(0x41d924df)), (f32::from_bits(0x41dc82cd), f32::from_bits(0x41d8f7cd)));
    path.quad_to((f32::from_bits(0x41dcaf1e), f32::from_bits(0x41d8ca01)), (f32::from_bits(0x41dcdc4c), f32::from_bits(0x41d89bf0)));
    path.quad_to((f32::from_bits(0x41ef6c33), f32::from_bits(0x41c5aec5)), (f32::from_bits(0x4204f72e), f32::from_bits(0x41c56cd2)));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn dean4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    // start region
    // start loop, contour: 1
    // Segment 1145.3381097316742 2017.6783947944641 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1145.3381097316742 2017.0033947825432
    path.move_to(((1145.3381347656250 as f32), (2017.6783447265625 as f32)));
    path.line_to(((1145.3381347656250 as f32), (2017.0034179687500 as f32)));
    // Segment 1145.3381097316742 2017.0033947825432 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1143.6927231521568 2017.0033947825432
    path.line_to(((1143.6927490234375 as f32), (2017.0034179687500 as f32)));
    // Segment 1143.6927231521568 2017.0033947825432 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1144.8640675112890 2018.1589246992417
    path.line_to(((1144.8640136718750 as f32), (2018.1589355468750 as f32)));
    // Segment 1144.8640675112890 2018.1589246992417 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1145.3381097316742 2017.6783947944641
    path.line_to(((1145.3381347656250 as f32), (2017.6783447265625 as f32)));
    path.close();
    // start loop, contour: 2
    // Segment 1145.3381097316742 2016.3216052055359 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1144.8640675258462 2015.8410752863977
    path.move_to(((1145.3381347656250 as f32), (2016.3216552734375 as f32)));
    path.line_to(((1144.8640136718750 as f32), (2015.8410644531250 as f32)));
    // Segment 1144.8640675258462 2015.8410752863977 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1143.6927230811802 2016.9966052174568
    path.line_to(((1143.6927490234375 as f32), (2016.9965820312500 as f32)));
    // Segment 1143.6927230811802 2016.9966052174568 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1145.3381097316742 2016.9966052174568
    path.line_to(((1145.3381347656250 as f32), (2016.9965820312500 as f32)));
    // Segment 1145.3381097316742 2016.9966052174568 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1145.3381097316742 2016.3216052055359
    path.line_to(((1145.3381347656250 as f32), (2016.3216552734375 as f32)));
    path.close();
    // start loop, contour: 3
    // Segment 1147.3323798179626 2014.3542600870132 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1147.8064220239557 2014.8347900059885
    path.move_to(((1147.3323974609375 as f32), (2014.3542480468750 as f32)));
    path.line_to(((1147.8063964843750 as f32), (2014.8348388671875 as f32)));
    // Segment 1147.8064220239557 2014.8347900059885 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1147.8064220516883 2014.8347899786306
    path.line_to(((1147.8063964843750 as f32), (2014.8348388671875 as f32)));
    // Segment 1147.8064220516883 2014.8347899786306 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1147.3323798179626 2014.3542600870132
    path.line_to(((1147.3323974609375 as f32), (2014.3542480468750 as f32)));
    path.close();
    // start loop, contour: 4
    // Segment 1146.3696286678314 2013.4045072346926 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1146.8436708778083 2013.8850371497379
    path.move_to(((1146.3696289062500 as f32), (2013.4045410156250 as f32)));
    path.line_to(((1146.8436279296875 as f32), (2013.8850097656250 as f32)));
    // Segment 1146.8436708778083 2013.8850371497379 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1146.8436709015571 2013.8850371263100
    path.line_to(((1146.8436279296875 as f32), (2013.8850097656250 as f32)));
    // Segment 1146.8436709015571 2013.8850371263100 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1146.3696286678314 2013.4045072346926
    path.line_to(((1146.3696289062500 as f32), (2013.4045410156250 as f32)));
    path.close();
    // start loop, contour: 5
    // Segment 1143.2063037902117 2016.5251235961914 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1142.7322615802348 2016.0445936811461
    path.move_to(((1143.2062988281250 as f32), (2016.5251464843750 as f32)));
    path.line_to(((1142.7322998046875 as f32), (2016.0445556640625 as f32)));
    // Segment 1142.7322615802348 2016.0445936811461 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1142.7322615564860 2016.0445937045740
    path.line_to(((1142.7322998046875 as f32), (2016.0445556640625 as f32)));
    // Segment 1142.7322615564860 2016.0445937045740 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1143.2063037902117 2016.5251235961914
    path.line_to(((1143.2062988281250 as f32), (2016.5251464843750 as f32)));
    path.close();
    // start loop, contour: 6
    // Segment 1143.0687679275870 2016.7286419868469 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1143.5428101613127 2017.2091718784643
    path.move_to(((1143.0687255859375 as f32), (2016.7286376953125 as f32)));
    path.line_to(((1143.5428466796875 as f32), (2017.2092285156250 as f32)));
    // Segment 1143.5428101613127 2017.2091718784643 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1143.7437679395080 2017.0109272411960
    path.line_to(((1143.7437744140625 as f32), (2017.0109863281250 as f32)));
    // Segment 1143.7437679395080 2017.0109272411960 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1143.7437679395080 2016.7286419868469
    path.line_to(((1143.7437744140625 as f32), (2016.7286376953125 as f32)));
    // Segment 1143.7437679395080 2016.7286419868469 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1143.0687679275870 2016.7286419868469
    path.line_to(((1143.0687255859375 as f32), (2016.7286376953125 as f32)));
    path.close();
    // start loop, contour: 7
    // Segment 1143.2063037902117 2017.4748764038086 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1142.7322615603032 2017.9554062991915
    path.move_to(((1143.2062988281250 as f32), (2017.4748535156250 as f32)));
    path.line_to(((1142.7322998046875 as f32), (2017.9554443359375 as f32)));
    // Segment 1142.7322615603032 2017.9554062991915 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1142.7322615746241 2017.9554063133189
    path.line_to(((1142.7322998046875 as f32), (2017.9554443359375 as f32)));
    // Segment 1142.7322615746241 2017.9554063133189 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1143.2063037902117 2017.4748764038086
    path.line_to(((1143.2062988281250 as f32), (2017.4748535156250 as f32)));
    path.close();
    // start loop, contour: 8
    // Segment 1146.3696286678314 2020.5954928398132 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1146.8436708977399 2020.1149629444303
    path.move_to(((1146.3696289062500 as f32), (2020.5954589843750 as f32)));
    path.line_to(((1146.8436279296875 as f32), (2020.1149902343750 as f32)));
    // Segment 1146.8436708977399 2020.1149629444303 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1146.8436708834190 2020.1149629303029
    path.line_to(((1146.8436279296875 as f32), (2020.1149902343750 as f32)));
    // Segment 1146.8436708834190 2020.1149629303029 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1146.3696286678314 2020.5954928398132
    path.line_to(((1146.3696289062500 as f32), (2020.5954589843750 as f32)));
    path.close();
    // start loop, contour: 9
    // Segment 1147.3323798179626 2019.6457400321960 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1147.8064220484741 2019.1652101374082
    path.move_to(((1147.3323974609375 as f32), (2019.6457519531250 as f32)));
    path.line_to(((1147.8063964843750 as f32), (2019.1651611328125 as f32)));
    // Segment 1147.8064220484741 2019.1652101374082 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1147.8064220383478 2019.1652101274185
    path.line_to(((1147.8063964843750 as f32), (2019.1651611328125 as f32)));
    // Segment 1147.8064220383478 2019.1652101274185 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1147.3323798179626 2019.6457400321960
    path.line_to(((1147.3323974609375 as f32), (2019.6457519531250 as f32)));
    path.close();
    // start loop, contour: 10
    // Segment 1145.3381097316742 2018.3533948063850 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1156.6848182678223 2018.3533948063850
    path.move_to(((1145.3381347656250 as f32), (2018.3533935546875 as f32)));
    path.line_to(((1156.6848144531250 as f32), (2018.3533935546875 as f32)));
    // Segment 1156.6848182678223 2018.3533948063850 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1156.6848182678223 2017.0033947825432
    path.line_to(((1156.6848144531250 as f32), (2017.0034179687500 as f32)));
    // Segment 1156.6848182678223 2017.0033947825432 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1145.3381097316742 2017.0033947825432
    path.line_to(((1145.3381347656250 as f32), (2017.0034179687500 as f32)));
    // Segment 1145.3381097316742 2017.0033947825432 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1145.3381097316742 2018.3533948063850
    path.line_to(((1145.3381347656250 as f32), (2018.3533935546875 as f32)));
    path.close();
    // start loop, contour: 11
    // Segment 1156.6848182678223 2018.3533948063850 0.3569631313191 0.0000000000000 -0.2645167304388 0.2609454237780 1157.6574279406423 2017.9723661860094
    path.move_to(((1156.6848144531250 as f32), (2018.3533935546875 as f32)));
    path.cubic_to(((1157.0417480468750 as f32), (2018.3533935546875 as f32)), ((1157.3929443359375 as f32), (2018.2332763671875 as f32)), ((1157.6574707031250 as f32), (2017.9724121093750 as f32)));
    // Segment 1157.6574279406423 2017.9723661860094 0.2653344079822 -0.2617520616521 0.0000000000000 0.3596905289350 1158.0474975705147 2017.0000000000000
    path.cubic_to(((1157.9227294921875 as f32), (2017.7105712890625 as f32)), ((1158.0474853515625 as f32), (2017.3597412109375 as f32)), ((1158.0474853515625 as f32), (2017.0000000000000 as f32)));
    // Segment 1158.0474975705147 2017.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1156.6974975466728 2017.0000000000000
    path.line_to(((1156.6975097656250 as f32), (2017.0000000000000 as f32)));
    // Segment 1156.6974975466728 2017.0000000000000 0.0028009248351 0.0403311981485 0.0118595244351 -0.0220843520393 1156.6941780622435 2017.0325257649940
    path.cubic_to(((1156.7003173828125 as f32), (2017.0402832031250 as f32)), ((1156.7060546875000 as f32), (2017.0104980468750 as f32)), ((1156.6942138671875 as f32), (2017.0324707031250 as f32)));
    // Segment 1156.6941780622435 2017.0325257649940 -0.0032637855860 0.0184860248562 0.0120617528380 -0.0065934603083 1156.7093435710913 2017.0113063061967
    path.cubic_to(((1156.6909179687500 as f32), (2017.0510253906250 as f32)), ((1156.7214355468750 as f32), (2017.0047607421875 as f32)), ((1156.7093505859375 as f32), (2017.0113525390625 as f32)));
    // split at 0.4496445953846
    // path.cubicTo(1156.6927490234375, 2017.0407714843750, 1156.6981201171875, 2017.0360107421875, 1156.7033691406250, 2017.0289306640625);
    // path.cubicTo(1156.7097167968750, 2017.0201416015625, 1156.7159423828125, 2017.0076904296875, 1156.7093505859375, 2017.0113525390625);
    // Segment 1156.7093435710913 2017.0113063061967 -0.0070717276929 0.0122220954353 0.0203483811973 -0.0039136894418 1156.7268834554304 2016.9985353221975
    path.cubic_to(((1156.7022705078125 as f32), (2017.0235595703125 as f32)), ((1156.7471923828125 as f32), (2016.9946289062500 as f32)), ((1156.7269287109375 as f32), (2016.9985351562500 as f32)));
    // Segment 1156.7268834554304 2016.9985353221975 -0.0244396787691 0.0123649140586 0.0433322464027 0.0026558844666 1156.6848182678223 2017.0033947825432
    path.cubic_to(((1156.7023925781250 as f32), (2017.0108642578125 as f32)), ((1156.7281494140625 as f32), (2017.0061035156250 as f32)), ((1156.6848144531250 as f32), (2017.0034179687500 as f32)));
    // split at 0.4418420493603
    // path.cubicTo(1156.7160644531250, 2017.0040283203125, 1156.7150878906250, 2017.0061035156250, 1156.7136230468750, 2017.0065917968750);
    // path.cubicTo(1156.7116699218750, 2017.0070800781250, 1156.7089843750000, 2017.0048828125000, 1156.6848144531250, 2017.0034179687500);
    // Segment 1156.6848182678223 2017.0033947825432 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1156.6848182678223 2018.3533948063850
    path.line_to(((1156.6848144531250 as f32), (2018.3533935546875 as f32)));
    path.close();
    // start loop, contour: 12
    // Segment 1158.0474975705147 2017.0000000000000 0.0000000000000 -0.3596905289350 0.2653344079822 0.2617520616521 1157.6574279406423 2016.0276338139906
    path.move_to(((1158.0474853515625 as f32), (2017.0000000000000 as f32)));
    path.cubic_to(((1158.0474853515625 as f32), (2016.6402587890625 as f32)), ((1157.9227294921875 as f32), (2016.2894287109375 as f32)), ((1157.6574707031250 as f32), (2016.0275878906250 as f32)));
    // Segment 1157.6574279406423 2016.0276338139906 -0.2645167304388 -0.2609454237780 0.3569631313191 0.0000000000000 1156.6848182678223 2015.6466051936150
    path.cubic_to(((1157.3929443359375 as f32), (2015.7667236328125 as f32)), ((1157.0417480468750 as f32), (2015.6466064453125 as f32)), ((1156.6848144531250 as f32), (2015.6466064453125 as f32)));
    // split at 0.5481675863266
    // path.cubicTo(1157.5124511718750, 2015.8846435546875, 1157.3414306640625, 2015.7839355468750, 1157.1577148437500, 2015.7220458984375);
    // path.cubicTo(1157.0062255859375, 2015.6711425781250, 1156.8460693359375, 2015.6466064453125, 1156.6848144531250, 2015.6466064453125);
    // Segment 1156.6848182678223 2015.6466051936150 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1156.6848182678223 2016.9966052174568
    path.line_to(((1156.6848144531250 as f32), (2016.9965820312500 as f32)));
    // Segment 1156.6848182678223 2016.9966052174568 0.0433322464027 -0.0026558844666 -0.0244396787691 -0.0123649140586 1156.7268834554304 2017.0014646778025
    path.cubic_to(((1156.7281494140625 as f32), (2016.9938964843750 as f32)), ((1156.7023925781250 as f32), (2016.9891357421875 as f32)), ((1156.7269287109375 as f32), (2017.0014648437500 as f32)));
    // split at 0.5581579208374
    // path.cubicTo(1156.7089843750000, 2016.9951171875000, 1156.7116699218750, 2016.9929199218750, 1156.7136230468750, 2016.9934082031250);
    // path.cubicTo(1156.7150878906250, 2016.9938964843750, 1156.7160644531250, 2016.9959716796875, 1156.7269287109375, 2017.0014648437500);
    // Segment 1156.7268834554304 2017.0014646778025 0.0203483811973 0.0039136894418 -0.0070717276929 -0.0122220954353 1156.7093435710913 2016.9886936938033
    path.cubic_to(((1156.7471923828125 as f32), (2017.0053710937500 as f32)), ((1156.7022705078125 as f32), (2016.9764404296875 as f32)), ((1156.7093505859375 as f32), (2016.9886474609375 as f32)));
    // Segment 1156.7093435710913 2016.9886936938033 0.0120617528380 0.0065934603083 -0.0032637855860 -0.0184860248562 1156.6941780622435 2016.9674742350060
    path.cubic_to(((1156.7214355468750 as f32), (2016.9952392578125 as f32)), ((1156.6909179687500 as f32), (2016.9489746093750 as f32)), ((1156.6942138671875 as f32), (2016.9675292968750 as f32)));
    // Segment 1156.6941780622435 2016.9674742350060 0.0118595244351 0.0220843520393 0.0028009248351 -0.0403311981485 1156.6974975466728 2017.0000000000000
    path.cubic_to(((1156.7060546875000 as f32), (2016.9895019531250 as f32)), ((1156.7003173828125 as f32), (2016.9597167968750 as f32)), ((1156.6975097656250 as f32), (2017.0000000000000 as f32)));
    // split at 0.4572408795357
    // path.cubicTo(1156.6995849609375, 2016.9775390625000, 1156.7014160156250, 2016.9768066406250, 1156.7014160156250, 2016.9768066406250);
    // path.cubicTo(1156.7014160156250, 2016.9769287109375, 1156.6989746093750, 2016.9781494140625, 1156.6975097656250, 2017.0000000000000);
    // Segment 1156.6974975466728 2017.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1158.0474975705147 2017.0000000000000
    path.line_to(((1158.0474853515625 as f32), (2017.0000000000000 as f32)));
    path.close();
    // start loop, contour: 13
    // Segment 1156.6848182678223 2015.6466051936150 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1145.3381097316742 2015.6466051936150
    path.move_to(((1156.6848144531250 as f32), (2015.6466064453125 as f32)));
    path.line_to(((1145.3381347656250 as f32), (2015.6466064453125 as f32)));
    // Segment 1145.3381097316742 2015.6466051936150 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1145.3381097316742 2016.9966052174568
    path.line_to(((1145.3381347656250 as f32), (2016.9965820312500 as f32)));
    // Segment 1145.3381097316742 2016.9966052174568 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1156.6848182678223 2016.9966052174568
    path.line_to(((1156.6848144531250 as f32), (2016.9965820312500 as f32)));
    // Segment 1156.6848182678223 2016.9966052174568 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1156.6848182678223 2015.6466051936150
    path.line_to(((1156.6848144531250 as f32), (2015.6466064453125 as f32)));
    path.close();
    // start loop, contour: 14
    // Segment 1145.8121519375022 2016.8021351246741 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1147.8064220237907 2014.8347900061515
    path.move_to(((1145.8121337890625 as f32), (2016.8021240234375 as f32)));
    path.line_to(((1147.8063964843750 as f32), (2014.8348388671875 as f32)));
    // Segment 1147.8064220237907 2014.8347900061515 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1146.8583376121346 2013.8737301678750
    path.line_to(((1146.8583984375000 as f32), (2013.8737792968750 as f32)));
    // Segment 1146.8583376121346 2013.8737301678750 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1144.8640675258462 2015.8410752863977
    path.line_to(((1144.8640136718750 as f32), (2015.8410644531250 as f32)));
    // Segment 1144.8640675258462 2015.8410752863977 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1145.8121519375022 2016.8021351246741
    path.line_to(((1145.8121337890625 as f32), (2016.8021240234375 as f32)));
    path.close();
    // start loop, contour: 15
    // Segment 1147.8064220516883 2014.8347899786306 0.5430154146087 -0.5356841365729 0.5430154146087 0.5356841365729 1147.8064220516883 2012.9239773430752
    path.move_to(((1147.8063964843750 as f32), (2014.8348388671875 as f32)));
    path.cubic_to(((1148.3494873046875 as f32), (2014.2990722656250 as f32)), ((1148.3494873046875 as f32), (2013.4597167968750 as f32)), ((1147.8063964843750 as f32), (2012.9239501953125 as f32)));
    // Segment 1147.8064220516883 2012.9239773430752 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1146.8583375842370 2013.8850371263100
    path.line_to(((1146.8583984375000 as f32), (2013.8850097656250 as f32)));
    // Segment 1146.8583375842370 2013.8850371263100 0.0071280060876 0.0070317705240 0.0071280060876 -0.0070317705240 1146.8583375842370 2013.8737301953959
    path.cubic_to(((1146.8654785156250 as f32), (2013.8920898437500 as f32)), ((1146.8654785156250 as f32), (2013.8666992187500 as f32)), ((1146.8583984375000 as f32), (2013.8737792968750 as f32)));
    // Segment 1146.8583375842370 2013.8737301953959 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1147.8064220516883 2014.8347899786306
    path.line_to(((1147.8063964843750 as f32), (2014.8348388671875 as f32)));
    path.close();
    // start loop, contour: 16
    // Segment 1147.8064220516883 2012.9239773430752 -0.5379138488298 -0.5306514472866 0.5379138488298 -0.5306514472866 1145.8955864341058 2012.9239773430752
    path.move_to(((1147.8063964843750 as f32), (2012.9239501953125 as f32)));
    path.cubic_to(((1147.2685546875000 as f32), (2012.3933105468750 as f32)), ((1146.4334716796875 as f32), (2012.3933105468750 as f32)), ((1145.8956298828125 as f32), (2012.9239501953125 as f32)));
    // Segment 1145.8955864341058 2012.9239773430752 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1146.8436709015571 2013.8850371263100
    path.line_to(((1146.8436279296875 as f32), (2013.8850097656250 as f32)));
    // Segment 1146.8436709015571 2013.8850371263100 0.0122295718664 -0.0120644598103 -0.0122295718664 -0.0120644598103 1146.8583375842370 2013.8850371263100
    path.cubic_to(((1146.8559570312500 as f32), (2013.8729248046875 as f32)), ((1146.8460693359375 as f32), (2013.8729248046875 as f32)), ((1146.8583984375000 as f32), (2013.8850097656250 as f32)));
    // Segment 1146.8583375842370 2013.8850371263100 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1147.8064220516883 2012.9239773430752
    path.line_to(((1147.8063964843750 as f32), (2012.9239501953125 as f32)));
    path.close();
    // start loop, contour: 17
    // Segment 1145.8955864579798 2012.9239773195236 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1142.7322615803600 2016.0445936810224
    path.move_to(((1145.8956298828125 as f32), (2012.9239501953125 as f32)));
    path.line_to(((1142.7322998046875 as f32), (2016.0445556640625 as f32)));
    // Segment 1142.7322615803600 2016.0445936810224 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1143.6803460000633 2017.0056535113604
    path.line_to(((1143.6802978515625 as f32), (2017.0056152343750 as f32)));
    // Segment 1143.6803460000633 2017.0056535113604 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1146.8436708776831 2013.8850371498615
    path.line_to(((1146.8436279296875 as f32), (2013.8850097656250 as f32)));
    // Segment 1146.8436708776831 2013.8850371498615 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1145.8955864579798 2012.9239773195236
    path.line_to(((1145.8956298828125 as f32), (2012.9239501953125 as f32)));
    path.close();
    // start loop, contour: 18
    // Segment 1142.7322615564860 2016.0445937045740 -0.0343838913237 0.0339196727021 0.0561572931720 -0.0710493024751 1142.5744069596683 2016.2183613784646
    path.move_to(((1142.7322998046875 as f32), (2016.0445556640625 as f32)));
    path.cubic_to(((1142.6978759765625 as f32), (2016.0784912109375 as f32)), ((1142.6306152343750 as f32), (2016.1473388671875 as f32)), ((1142.5744628906250 as f32), (2016.2183837890625 as f32)));
    // Segment 1142.5744069596683 2016.2183613784646 -0.0547779032556 0.0720510806539 0.0000000000000 -0.2570904015602 1142.3937679156661 2016.7286419868469
    path.cubic_to(((1142.5196533203125 as f32), (2016.2904052734375 as f32)), ((1142.3937988281250 as f32), (2016.4715576171875 as f32)), ((1142.3937988281250 as f32), (2016.7286376953125 as f32)));
    // Segment 1142.3937679156661 2016.7286419868469 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1143.7437679395080 2016.7286419868469
    path.line_to(((1143.7437744140625 as f32), (2016.7286376953125 as f32)));
    // Segment 1143.7437679395080 2016.7286419868469 -0.0051909534315 0.0665915567290 0.0133980913650 -0.0361675066532 1143.6976291086639 2016.9514128270803
    path.cubic_to(((1143.7385253906250 as f32), (2016.7952880859375 as f32)), ((1143.7110595703125 as f32), (2016.9152832031250 as f32)), ((1143.6976318359375 as f32), (2016.9514160156250 as f32)));
    // Segment 1143.6976291086639 2016.9514128270803 -0.0142876819622 0.0277028472317 0.0040377216094 -0.0063254385208 1143.6490888124401 2017.0354042045738
    path.cubic_to(((1143.6833496093750 as f32), (2016.9791259765625 as f32)), ((1143.6530761718750 as f32), (2017.0290527343750 as f32)), ((1143.6490478515625 as f32), (2017.0354003906250 as f32)));
    // Segment 1143.6490888124401 2017.0354042045738 -0.0045813437564 0.0032098513409 -0.0343840362634 0.0339198156850 1143.6803460239373 2017.0056534878088
    path.cubic_to(((1143.6445312500000 as f32), (2017.0385742187500 as f32)), ((1143.6459960937500 as f32), (2017.0395507812500 as f32)), ((1143.6802978515625 as f32), (2017.0056152343750 as f32)));
    // Segment 1143.6803460239373 2017.0056534878088 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1142.7322615564860 2016.0445937045740
    path.line_to(((1142.7322998046875 as f32), (2016.0445556640625 as f32)));
    path.close();
    // start loop, contour: 19
    // Segment 1142.5947256938614 2016.2481120952295 -0.1857487117715 0.1832409092043 0.0167379373694 -0.0990717748979 1142.3430278987244 2016.7518748698508
    path.move_to(((1142.5947265625000 as f32), (2016.2481689453125 as f32)));
    path.cubic_to(((1142.4089355468750 as f32), (2016.4313964843750 as f32)), ((1142.3597412109375 as f32), (2016.6528320312500 as f32)), ((1142.3430175781250 as f32), (2016.7518310546875 as f32)));
    // Segment 1142.3430278987244 2016.7518748698508 -0.0156657977007 0.1069052535795 0.0000000000000 -0.0339197441936 1142.3249999880791 2017.0000000000000
    path.cubic_to(((1142.3273925781250 as f32), (2016.8587646484375 as f32)), ((1142.3249511718750 as f32), (2016.9660644531250 as f32)), ((1142.3249511718750 as f32), (2017.0000000000000 as f32)));
    // Segment 1142.3249999880791 2017.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1143.6750000119209 2017.0000000000000
    path.line_to(((1143.6750488281250 as f32), (2017.0000000000000 as f32)));
    // Segment 1143.6750000119209 2017.0000000000000 0.0000000000000 -0.0339197441936 -0.0015261841961 -0.0051459911965 1143.6741640831724 2016.9767671169961
    path.cubic_to(((1143.6750488281250 as f32), (2016.9660644531250 as f32)), ((1143.6726074218750 as f32), (2016.9716796875000 as f32)), ((1143.6741943359375 as f32), (2016.9768066406250 as f32)));
    // Segment 1143.6741640831724 2016.9767671169961 -0.0007886982052 0.0013596649622 0.0074114058388 -0.0224954551713 1143.6525251830094 2017.0486861571169
    path.cubic_to(((1143.6733398437500 as f32), (2016.9781494140625 as f32)), ((1143.6599121093750 as f32), (2017.0262451171875 as f32)), ((1143.6524658203125 as f32), (2017.0487060546875 as f32)));
    // split at 0.4203657805920
    // path.cubicTo(1143.6738281250000, 2016.9774169921875, 1143.6712646484375, 2016.9862060546875, 1143.6678466796875, 2016.9979248046875);
    // path.cubicTo(1143.6630859375000, 2017.0140380859375, 1143.6567382812500, 2017.0356445312500, 1143.6524658203125, 2017.0487060546875);
    // Segment 1143.6525251830094 2017.0486861571169 -0.0119644334077 0.0236755853369 0.0381324473830 -0.0447670202574 1143.5428101613127 2017.2091718784643
    path.cubic_to(((1143.6405029296875 as f32), (2017.0723876953125 as f32)), ((1143.5809326171875 as f32), (2017.1644287109375 as f32)), ((1143.5428466796875 as f32), (2017.2092285156250 as f32)));
    // Segment 1143.5428101613127 2017.2091718784643 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1142.5947256938614 2016.2481120952295
    path.line_to(((1142.5947265625000 as f32), (2016.2481689453125 as f32)));
    path.close();
    // start loop, contour: 20
    // Segment 1142.3249999880791 2017.0000000000000 0.0000000000000 0.0339197441936 -0.0156657977007 -0.1069052535795 1142.3430278987244 2017.2481251301492
    path.move_to(((1142.3249511718750 as f32), (2017.0000000000000 as f32)));
    path.cubic_to(((1142.3249511718750 as f32), (2017.0339355468750 as f32)), ((1142.3273925781250 as f32), (2017.1412353515625 as f32)), ((1142.3430175781250 as f32), (2017.2481689453125 as f32)));
    // Segment 1142.3430278987244 2017.2481251301492 0.0167379373694 0.0990717748979 -0.1857487117715 -0.1832409092043 1142.5947256938614 2017.7518879047705
    path.cubic_to(((1142.3597412109375 as f32), (2017.3471679687500 as f32)), ((1142.4089355468750 as f32), (2017.5686035156250 as f32)), ((1142.5947265625000 as f32), (2017.7518310546875 as f32)));
    // split at 0.4008532166481
    // path.cubicTo(1142.3497314453125, 2017.2878417968750, 1142.3616943359375, 2017.3471679687500, 1142.3854980468750, 2017.4158935546875);
    // path.cubicTo(1142.4211425781250, 2017.5185546875000, 1142.4833984375000, 2017.6420898437500, 1142.5947265625000, 2017.7518310546875);
    // Segment 1142.5947256938614 2017.7518879047705 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1143.5428101613127 2016.7908281215357
    path.line_to(((1143.5428466796875 as f32), (2016.7907714843750 as f32)));
    // Segment 1143.5428101613127 2016.7908281215357 0.0381324473830 0.0447670202574 -0.0119644334077 -0.0236755853369 1143.6525251830094 2016.9513138428831
    path.cubic_to(((1143.5809326171875 as f32), (2016.8355712890625 as f32)), ((1143.6405029296875 as f32), (2016.9276123046875 as f32)), ((1143.6524658203125 as f32), (2016.9512939453125 as f32)));
    // Segment 1143.6525251830094 2016.9513138428831 0.0074114058388 0.0224954551713 -0.0007886982052 -0.0013596649622 1143.6741640831724 2017.0232328830039
    path.cubic_to(((1143.6599121093750 as f32), (2016.9737548828125 as f32)), ((1143.6733398437500 as f32), (2017.0218505859375 as f32)), ((1143.6741943359375 as f32), (2017.0231933593750 as f32)));
    // Segment 1143.6741640831724 2017.0232328830039 -0.0015261841961 0.0051459911965 0.0000000000000 0.0339197441936 1143.6750000119209 2017.0000000000000
    path.cubic_to(((1143.6726074218750 as f32), (2017.0283203125000 as f32)), ((1143.6750488281250 as f32), (2017.0339355468750 as f32)), ((1143.6750488281250 as f32), (2017.0000000000000 as f32)));
    // Segment 1143.6750000119209 2017.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1142.3249999880791 2017.0000000000000
    path.line_to(((1142.3249511718750 as f32), (2017.0000000000000 as f32)));
    path.close();
    // start loop, contour: 21
    // Segment 1142.5947256938614 2017.7518879047705 -0.0799271403989 -0.1522613934208 -0.2174629955730 -0.2879403701950 1142.7322615564860 2017.9554062954260
    path.move_to(((1142.5947265625000 as f32), (2017.7518310546875 as f32)));
    path.cubic_to(((1142.5147705078125 as f32), (2017.5996093750000 as f32)), ((1142.5147705078125 as f32), (2017.6674804687500 as f32)), ((1142.7322998046875 as f32), (2017.9554443359375 as f32)));
    // Segment 1142.7322615564860 2017.9554062954260 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1143.6803460239373 2016.9943465121912
    path.line_to(((1143.6802978515625 as f32), (2016.9943847656250 as f32)));
    // Segment 1143.6803460239373 2016.9943465121912 0.0799271403989 0.1522613934208 0.2174629955730 0.2879403701950 1143.5428101613127 2016.7908281215357
    path.cubic_to(((1143.7602539062500 as f32), (2017.1466064453125 as f32)), ((1143.7602539062500 as f32), (2017.0787353515625 as f32)), ((1143.5428466796875 as f32), (2016.7907714843750 as f32)));
    // Segment 1143.5428101613127 2016.7908281215357 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1142.5947256938614 2017.7518879047705
    path.line_to(((1142.5947265625000 as f32), (2017.7518310546875 as f32)));
    path.close();
    // start loop, contour: 22
    // Segment 1142.7322615746241 2017.9554063133189 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1145.8955864522438 2021.0760227493236
    path.move_to(((1142.7322998046875 as f32), (2017.9554443359375 as f32)));
    path.line_to(((1145.8956298828125 as f32), (2021.0760498046875 as f32)));
    // Segment 1145.8955864522438 2021.0760227493236 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1146.8436708834190 2020.1149629303029
    path.line_to(((1146.8436279296875 as f32), (2020.1149902343750 as f32)));
    // Segment 1146.8436708834190 2020.1149629303029 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1143.6803460057993 2016.9943464942983
    path.line_to(((1143.6802978515625 as f32), (2016.9943847656250 as f32)));
    // Segment 1143.6803460057993 2016.9943464942983 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1142.7322615746241 2017.9554063133189
    path.line_to(((1142.7322998046875 as f32), (2017.9554443359375 as f32)));
    path.close();
    // start loop, contour: 23
    // Segment 1145.8955864341058 2021.0760227314306 0.2730164534637 0.2693304447891 -0.3016608168437 0.0000000000000 1146.8510041236877 2021.4740112423897
    path.move_to(((1145.8956298828125 as f32), (2021.0760498046875 as f32)));
    path.cubic_to(((1146.1685791015625 as f32), (2021.3453369140625 as f32)), ((1146.5493164062500 as f32), (2021.4739990234375 as f32)), ((1146.8509521484375 as f32), (2021.4739990234375 as f32)));
    // Segment 1146.8510041236877 2021.4740112423897 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1146.8510041236877 2020.1240112185478
    path.line_to(((1146.8509521484375 as f32), (2020.1240234375000 as f32)));
    // Segment 1146.8510041236877 2020.1240112185478 -0.0031276099109 0.0031991747760 0.0281856144058 0.0140930868099 1146.8580791488898 2020.1202473991566
    path.cubic_to(((1146.8479003906250 as f32), (2020.1271972656250 as f32)), ((1146.8862304687500 as f32), (2020.1343994140625 as f32)), ((1146.8580322265625 as f32), (2020.1202392578125 as f32)));
    // split at 0.3845077157021
    // path.cubicTo(1146.8497314453125, 2020.1252441406250, 1146.8547363281250, 2020.1270751953125, 1146.8596191406250, 2020.1280517578125);
    // path.cubicTo(1146.8675537109375, 2020.1296386718750, 1146.8753662109375, 2020.1289062500000, 1146.8580322265625, 2020.1202392578125);
    // Segment 1146.8580791488898 2020.1202473991566 -0.0369995545027 -0.0123195805663 0.0067223483810 0.0136883790721 1146.8436709015571 2020.1149629481959
    path.cubic_to(((1146.8210449218750 as f32), (2020.1079101562500 as f32)), ((1146.8503417968750 as f32), (2020.1286621093750 as f32)), ((1146.8436279296875 as f32), (2020.1149902343750 as f32)));
    // Segment 1146.8436709015571 2020.1149629481959 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1145.8955864341058 2021.0760227314306
    path.line_to(((1145.8956298828125 as f32), (2021.0760498046875 as f32)));
    path.close();
    // start loop, contour: 24
    // Segment 1146.8510041236877 2021.4740112423897 0.3016605789999 0.0000000000000 -0.2730166120260 0.2693306012106 1147.8064220516883 2021.0760227314306
    path.move_to(((1146.8509521484375 as f32), (2021.4739990234375 as f32)));
    path.cubic_to(((1147.1527099609375 as f32), (2021.4739990234375 as f32)), ((1147.5334472656250 as f32), (2021.3453369140625 as f32)), ((1147.8063964843750 as f32), (2021.0760498046875 as f32)));
    // Segment 1147.8064220516883 2021.0760227314306 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1146.8583375842370 2020.1149629481959
    path.line_to(((1146.8583984375000 as f32), (2020.1149902343750 as f32)));
    // Segment 1146.8583375842370 2020.1149629481959 -0.0067222671256 0.0136883164611 0.0369996293611 -0.0123196021258 1146.8439293663473 2020.1202473404985
    path.cubic_to(((1146.8515625000000 as f32), (2020.1286621093750 as f32)), ((1146.8809814453125 as f32), (2020.1079101562500 as f32)), ((1146.8438720703125 as f32), (2020.1202392578125 as f32)));
    // Segment 1146.8439293663473 2020.1202473404985 -0.0281857033438 0.0140931104690 0.0031276541428 0.0031991704542 1146.8510041236877 2020.1240112185478
    path.cubic_to(((1146.8157958984375 as f32), (2020.1343994140625 as f32)), ((1146.8541259765625 as f32), (2020.1271972656250 as f32)), ((1146.8509521484375 as f32), (2020.1240234375000 as f32)));
    // Segment 1146.8510041236877 2020.1240112185478 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1146.8510041236877 2021.4740112423897
    path.line_to(((1146.8509521484375 as f32), (2021.4739990234375 as f32)));
    path.close();
    // start loop, contour: 25
    // Segment 1147.8064220516883 2021.0760227314306 0.5430154146087 -0.5356841365729 0.5430154146087 0.5356841365729 1147.8064220516883 2019.1652101405787
    path.move_to(((1147.8063964843750 as f32), (2021.0760498046875 as f32)));
    path.cubic_to(((1148.3494873046875 as f32), (2020.5402832031250 as f32)), ((1148.3494873046875 as f32), (2019.7009277343750 as f32)), ((1147.8063964843750 as f32), (2019.1651611328125 as f32)));
    // Segment 1147.8064220516883 2019.1652101405787 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1146.8583375842370 2020.1262699238134
    path.line_to(((1146.8583984375000 as f32), (2020.1262207031250 as f32)));
    // Segment 1146.8583375842370 2020.1262699238134 0.0071280060876 0.0070317705240 0.0071280060876 -0.0070317705240 1146.8583375842370 2020.1149629481959
    path.cubic_to(((1146.8654785156250 as f32), (2020.1333007812500 as f32)), ((1146.8654785156250 as f32), (2020.1079101562500 as f32)), ((1146.8583984375000 as f32), (2020.1149902343750 as f32)));
    // Segment 1146.8583375842370 2020.1149629481959 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1147.8064220516883 2021.0760227314306
    path.line_to(((1147.8063964843750 as f32), (2021.0760498046875 as f32)));
    path.close();
    // start loop, contour: 26
    // Segment 1147.8064220383478 2019.1652101274185 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1145.8121519520594 2017.1978648896866
    path.move_to(((1147.8063964843750 as f32), (2019.1651611328125 as f32)));
    path.line_to(((1145.8121337890625 as f32), (2017.1978759765625 as f32)));
    // Segment 1145.8121519520594 2017.1978648896866 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1144.8640675112890 2018.1589246992417
    path.line_to(((1144.8640136718750 as f32), (2018.1589355468750 as f32)));
    // Segment 1144.8640675112890 2018.1589246992417 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1146.8583375975775 2020.1262699369736
    path.line_to(((1146.8583984375000 as f32), (2020.1262207031250 as f32)));
    // Segment 1146.8583375975775 2020.1262699369736 0.0000000000000 0.0000000000000 0.0000000000000 0.0000000000000 1147.8064220383478 2019.1652101274185
    path.line_to(((1147.8063964843750 as f32), (2019.1651611328125 as f32)));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads66(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((2.0, 0.0));
    path.quad_to((3.0, 1.0), (2.0, 2.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((2.0, 1.0));
    path.line_to((2.0, 1.0));
    path.quad_to((1.0, 2.0), (2.0, 2.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads67(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((3.0, 2.0));
    path.quad_to((1.0, 3.0), (3.0, 3.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((2.0, 3.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads68(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 2.0));
    path.quad_to((0.0, 3.0), (2.0, 3.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((0.0, 1.0));
    path.quad_to((1.0, 3.0), (2.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads69(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 0.0));
    path.quad_to((2.0, 2.0), (2.0, 3.0));
    path.line_to((2.0, 3.0));
    path.close();
    path.move_to((1.0, 0.0));
    path.line_to((1.0, 0.0));
    path.quad_to((3.0, 0.0), (1.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads70(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 1.0));
    path.quad_to((2.0, 3.0), (3.0, 3.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((2.0, 0.0));
    path.line_to((2.0, 2.0));
    path.quad_to((1.0, 3.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads71(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 1.0));
    path.quad_to((2.0, 3.0), (3.0, 3.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((3.0, 0.0));
    path.line_to((2.0, 2.0));
    path.quad_to((1.0, 3.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads72(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((1.0, 1.0));
    path.quad_to((2.0, 3.0), (3.0, 3.0));
    path.line_to((3.0, 3.0));
    path.close();
    path.move_to((0.0, 1.0));
    path.line_to((2.0, 2.0));
    path.quad_to((1.0, 3.0), (3.0, 3.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn test_quads73(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((0.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 2.0));
    path.line_to((0.0, 3.0));
    path.close();
    path.move_to((0.0, 0.0));
    path.line_to((0.0, 0.0));
    path.quad_to((0.0, 1.0), (1.0, 1.0));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn bug5169(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x00000000), f32::from_bits(0x4281c71c)));
    // 0, 64.8889f
    path.cubic_to((f32::from_bits(0x434e0000), f32::from_bits(0x4281c71c)), (f32::from_bits(0x00000000), f32::from_bits(0xc2a238e4)), (f32::from_bits(0x00000000), f32::from_bits(0x4281c71c)));
    // 206, 64.8889f, 0, -81.1111f, 0, 64.8889f
    path.move_to((f32::from_bits(0x43300000), f32::from_bits(0x41971c72)));
    // 176, 18.8889f
    path.cubic_to((f32::from_bits(0xc29e0000), f32::from_bits(0xc25c71c7)), (f32::from_bits(0x42b20000), f32::from_bits(0x42fbc71c)), (f32::from_bits(0x43300000), f32::from_bits(0x41971c72)));
    // -79, -55.1111f, 89, 125.889f, 176, 18.8889f
    test_simplify(reporter, &path.detach(), filename);
}

fn tiger8_393(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42b93333), f32::from_bits(0x43d5a666)));
    // 92.6f, 427.3f
    path.cubic_to((f32::from_bits(0x42b93333), f32::from_bits(0x43d5a666)), (f32::from_bits(0x42b5cccd), f32::from_bits(0x43da1999)), (f32::from_bits(0x42b80000), f32::from_bits(0x43ddf333)));
    // 92.6f, 427.3f, 90.9f, 436.2f, 92, 443.9f
    path.cubic_to((f32::from_bits(0x42b80000), f32::from_bits(0x43ddf333)), (f32::from_bits(0x42b30000), f32::from_bits(0x43e17333)), (f32::from_bits(0x42cf999a), f32::from_bits(0x43e1b333)));
    // 92, 443.9f, 89.5f, 450.9f, 103.8f, 451.4f
    path.cubic_to((f32::from_bits(0x42ec3334), f32::from_bits(0x43e14ccd)), (f32::from_bits(0x42e73334), f32::from_bits(0x43ddf333)), (f32::from_bits(0x42e73334), f32::from_bits(0x43ddf333)));
    // 118.1f, 450.6f, 115.6f, 443.9f, 115.6f, 443.9f
    path.cubic_to((f32::from_bits(0x42e7999a), f32::from_bits(0x43de8000)), (f32::from_bits(0x42ea6667), f32::from_bits(0x43db4000)), (f32::from_bits(0x42e60001), f32::from_bits(0x43d5a666)));
    // 115.8f, 445, 117.2f, 438.5f, 115, 427.3f
    test_simplify(reporter, &path.detach(), filename);
}

fn carsvg_1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4393d61e), f32::from_bits(0x43e768f9)));
    // 295.673f, 462.82f
    path.cubic_to((f32::from_bits(0x4396b50e), f32::from_bits(0x43e63c20)), (f32::from_bits(0x43998931), f32::from_bits(0x43e6c43e)), (f32::from_bits(0x439cb6a8), f32::from_bits(0x43e70ef9)));
    // 301.414f, 460.47f, 307.072f, 461.533f, 313.427f, 462.117f
    path.cubic_to((f32::from_bits(0x439dfc1e), f32::from_bits(0x43e72ce0)), (f32::from_bits(0x439a285c), f32::from_bits(0x43e717fb)), (f32::from_bits(0x4398e23c), f32::from_bits(0x43e7027c)));
    // 315.97f, 462.351f, 308.315f, 462.187f, 305.767f, 462.019f
    path.cubic_to((f32::from_bits(0x4398136f), f32::from_bits(0x43e6f4db)), (f32::from_bits(0x439a7e14), f32::from_bits(0x43e6d390)), (f32::from_bits(0x439b4ba9), f32::from_bits(0x43e6b956)));
    // 304.152f, 461.913f, 308.985f, 461.653f, 310.591f, 461.448f
    path.cubic_to((f32::from_bits(0x439c2b19), f32::from_bits(0x43e68603)), (f32::from_bits(0x43abf4df), f32::from_bits(0x43e9ca9e)), (f32::from_bits(0x43a1daea), f32::from_bits(0x43e912a5)));
    // 312.337f, 461.047f, 343.913f, 467.583f, 323.71f, 466.146f
    path.cubic_to((f32::from_bits(0x43a4f45a), f32::from_bits(0x43e78baf)), (f32::from_bits(0x43a2a391), f32::from_bits(0x43e86a82)), (f32::from_bits(0x43a946bd), f32::from_bits(0x43e90c56)));
    // 329.909f, 463.091f, 325.278f, 464.832f, 338.553f, 466.096f
    path.line_to((f32::from_bits(0x43a4250b), f32::from_bits(0x43e998dc)));
    // 328.289f, 467.194f
    path.cubic_to((f32::from_bits(0x43a8a9c8), f32::from_bits(0x43e8f06c)), (f32::from_bits(0x43a95cb5), f32::from_bits(0x43e84ea6)), (f32::from_bits(0x43a6f7c1), f32::from_bits(0x43e9bdb5)));
    // 337.326f, 465.878f, 338.724f, 464.614f, 333.936f, 467.482f
    path.cubic_to((f32::from_bits(0x43a59ed0), f32::from_bits(0x43e9d2ca)), (f32::from_bits(0x4395ea4d), f32::from_bits(0x43e92afe)), (f32::from_bits(0x43a06569), f32::from_bits(0x43e7773d)));
    // 331.241f, 467.647f, 299.83f, 466.336f, 320.792f, 462.932f
    path.cubic_to((f32::from_bits(0x438bf0ff), f32::from_bits(0x43ea0fef)), (f32::from_bits(0x43a0e17a), f32::from_bits(0x43e5f41b)), (f32::from_bits(0x4398f3fb), f32::from_bits(0x43e804c8)));
    // 279.883f, 468.124f, 321.762f, 459.907f, 305.906f, 464.037f
    path.line_to((f32::from_bits(0x4393d61e), f32::from_bits(0x43e768f9)));
    // 295.673f, 462.82f
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn simplify_test_1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42bfefd4), f32::from_bits(0x42ef80ef)));
    // 95.9684f, 119.752f
    path.quad_to((f32::from_bits(0x42c26810), f32::from_bits(0x42e214b8)), (f32::from_bits(0x42cdcad5), f32::from_bits(0x42d82aa2)));
    // 97.2032f, 113.04f, 102.896f, 108.083f
    path.line_to((f32::from_bits(0x42cdcb21), f32::from_bits(0x42d82a61)));
    // 102.897f, 108.083f
    path.quad_to((f32::from_bits(0x42d5e3c8), f32::from_bits(0x42d12140)), (f32::from_bits(0x42e20ee8), f32::from_bits(0x42cdc937)));
    // 106.945f, 104.565f, 113.029f, 102.893f
    path.line_to((f32::from_bits(0x42e256e3), f32::from_bits(0x42cdbc92)));
    // 113.17f, 102.868f
    path.line_to((f32::from_bits(0x42f5eadb), f32::from_bits(0x42cc2cb3)));
    // 122.959f, 102.087f
    path.line_to((f32::from_bits(0x42f746a6), f32::from_bits(0x42cccf85)));
    // 123.638f, 102.405f
    path.quad_to((f32::from_bits(0x42fa586c), f32::from_bits(0x42d126c4)), (f32::from_bits(0x42f6c657), f32::from_bits(0x42d5d315)));
    // 125.173f, 104.576f, 123.387f, 106.912f
    path.line_to((f32::from_bits(0x42f591eb), f32::from_bits(0x42d4e76d)));
    // 122.785f, 106.452f
    path.line_to((f32::from_bits(0x42f6c6e0), f32::from_bits(0x42d5d261)));
    // 123.388f, 106.911f
    path.quad_to((f32::from_bits(0x42f6bb33), f32::from_bits(0x42d5e1bb)), (f32::from_bits(0x42f6a3d8), f32::from_bits(0x42d6007c)));
    // 123.366f, 106.941f, 123.32f, 107.001f
    path.quad_to((f32::from_bits(0x42ea3850), f32::from_bits(0x42e65af0)), (f32::from_bits(0x42d97a6e), f32::from_bits(0x42ed841c)));
    // 117.11f, 115.178f, 108.739f, 118.758f
    path.line_to((f32::from_bits(0x42d91d92), f32::from_bits(0x42ed9ec0)));
    // 108.558f, 118.81f
    path.line_to((f32::from_bits(0x42c1a959), f32::from_bits(0x42f146b0)));
    // 96.8308f, 120.638f
    path.line_to((f32::from_bits(0x42bfefd4), f32::from_bits(0x42ef80f0)));
    // 95.9684f, 119.752f
    path.line_to((f32::from_bits(0x42bfefd4), f32::from_bits(0x42ef80ef)));
    // 95.9684f, 119.752f
    path.close();
    path.move_to((f32::from_bits(0x42c2eb4e), f32::from_bits(0x42f00d68)));
    // 97.4596f, 120.026f
    path.line_to((f32::from_bits(0x42c16d91), f32::from_bits(0x42efc72c)));
    // 96.714f, 119.889f
    path.line_to((f32::from_bits(0x42c131c9), f32::from_bits(0x42ee47a8)));
    // 96.5972f, 119.14f
    path.line_to((f32::from_bits(0x42d8a602), f32::from_bits(0x42ea9fb8)));
    // 108.324f, 117.312f
    path.line_to((f32::from_bits(0x42d8e1ca), f32::from_bits(0x42ec1f3c)));
    // 108.441f, 118.061f
    path.line_to((f32::from_bits(0x42d84926), f32::from_bits(0x42eaba5c)));
    // 108.143f, 117.364f
    path.quad_to((f32::from_bits(0x42e84a40), f32::from_bits(0x42e3e1f0)), (f32::from_bits(0x42f439a2), f32::from_bits(0x42d42af8)));
    // 116.145f, 113.941f, 122.113f, 106.084f
    path.quad_to((f32::from_bits(0x42f45121), f32::from_bits(0x42d40c08)), (f32::from_bits(0x42f45cf6), f32::from_bits(0x42d3fc79)));
    // 122.158f, 106.023f, 122.182f, 105.993f
    path.line_to((f32::from_bits(0x42f45d7f), f32::from_bits(0x42d3fbc5)));
    // 122.183f, 105.992f
    path.quad_to((f32::from_bits(0x42f69510), f32::from_bits(0x42d114f4)), (f32::from_bits(0x42f4ccce), f32::from_bits(0x42ce8fb7)));
    // 123.291f, 104.541f, 122.4f, 103.281f
    path.line_to((f32::from_bits(0x42f609ba), f32::from_bits(0x42cdaf9e)));
    // 123.019f, 102.843f
    path.line_to((f32::from_bits(0x42f62899), f32::from_bits(0x42cf3289)));
    // 123.079f, 103.599f
    path.line_to((f32::from_bits(0x42e294a1), f32::from_bits(0x42d0c268)));
    // 113.29f, 104.38f
    path.line_to((f32::from_bits(0x42e275c2), f32::from_bits(0x42cf3f7d)));
    // 113.23f, 103.624f
    path.line_to((f32::from_bits(0x42e2dc9c), f32::from_bits(0x42d0b5c3)));
    // 113.431f, 104.355f
    path.quad_to((f32::from_bits(0x42d75bb8), f32::from_bits(0x42d3df08)), (f32::from_bits(0x42cfc853), f32::from_bits(0x42da7457)));
    // 107.679f, 105.936f, 103.891f, 109.227f
    path.line_to((f32::from_bits(0x42cec9ba), f32::from_bits(0x42d94f5c)));
    // 103.394f, 108.655f
    path.line_to((f32::from_bits(0x42cfc89f), f32::from_bits(0x42da7416)));
    // 103.892f, 109.227f
    path.quad_to((f32::from_bits(0x42c53268), f32::from_bits(0x42e3ac00)), (f32::from_bits(0x42c2eb4e), f32::from_bits(0x42f00d67)));
    // 98.5984f, 113.836f, 97.4596f, 120.026f
    path.line_to((f32::from_bits(0x42c2eb4e), f32::from_bits(0x42f00d68)));
    // 97.4596f, 120.026f
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn joel_1(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((144.859, 285.172));
    path.line_to((144.859, 285.172));
    path.line_to((144.859, 285.172));
    path.line_to((143.132, 284.617));
    path.line_to((144.859, 285.172));
    path.close();
    path.move_to((135.922, 286.844));
    path.line_to((135.922, 286.844));
    path.line_to((135.922, 286.844));
    path.line_to((135.367, 288.571));
    path.line_to((135.922, 286.844));
    path.close();
    path.move_to((135.922, 286.844));
    path.cubic_to((137.07, 287.219), (138.242, 287.086), (139.242, 286.578));
    path.cubic_to((140.234, 286.078), (141.031, 285.203), (141.406, 284.055));
    path.line_to((144.859, 285.172));
    path.cubic_to((143.492, 289.375), (138.992, 291.656), (134.797, 290.297));
    path.line_to((135.922, 286.844));
    path.close();
    path.move_to((129.68, 280.242));
    path.line_to((129.68, 280.242));
    path.line_to((129.68, 280.242));
    path.line_to((131.407, 280.804));
    path.line_to((129.68, 280.242));
    path.close();
    path.move_to((133.133, 281.367));
    path.cubic_to((132.758, 282.508), (132.883, 283.687), (133.391, 284.679));
    path.cubic_to((133.907, 285.679), (134.774, 286.468), (135.922, 286.843));
    path.line_to((134.797, 290.296));
    path.cubic_to((130.602, 288.929), (128.313, 284.437), (129.68, 280.241));
    path.line_to((133.133, 281.367));
    path.close();
    path.move_to((139.742, 275.117));
    path.line_to((139.742, 275.117));
    path.line_to((139.18, 276.844));
    path.line_to((139.742, 275.117));
    path.close();
    path.move_to((138.609, 278.57));
    path.cubic_to((137.461, 278.203), (136.297, 278.328), (135.297, 278.836));
    path.cubic_to((134.297, 279.344), (133.508, 280.219), (133.133, 281.367));
    path.line_to((129.68, 280.242));
    path.cubic_to((131.047, 276.039), (135.539, 273.758), (139.742, 275.117));
    path.line_to((138.609, 278.57));
    path.close();
    path.move_to((141.406, 284.055));
    path.cubic_to((141.773, 282.907), (141.648, 281.735), (141.148, 280.735));
    path.cubic_to((140.625, 279.735), (139.757, 278.946), (138.609, 278.571));
    path.line_to((139.742, 275.118));
    path.cubic_to((143.937, 276.493), (146.219, 280.977), (144.859, 285.173));
    path.line_to((141.406, 284.055));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn joel_2(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((403.283, 497.197));
    path.cubic_to((403.424, 497.244), (391.111, 495.556), (391.111, 495.556));
    path.line_to((392.291, 493.165));
    path.cubic_to((392.291, 493.165), (388.994, 492.056), (386.65, 491.821));
    path.cubic_to((384.244, 491.454), (381.603, 490.774), (381.603, 490.774));
    path.line_to((383.392, 488.383));
    path.cubic_to((383.392, 488.383), (379.119, 487.453), (378.939, 485.695));
    path.cubic_to((378.791, 483.57), (383.064, 485.25), (384.877, 485.843));
    path.line_to((387.697, 484.351));
    path.cubic_to((382.752, 483.835), (376.595, 482.124), (374.478, 480.312));
    path.line_to((356.22, 496.304));
    path.line_to((368.095, 510.499));
    path.line_to((373.884, 510.202));
    path.line_to((374.478, 509.007));
    path.line_to((370.916, 506.913));
    path.line_to((371.807, 506.022));
    path.cubic_to((371.807, 506.022), (374.807, 507.28), (377.752, 507.514));
    path.cubic_to((380.752, 507.881), (387.4, 508.108), (387.4, 508.108));
    path.line_to((388.884, 506.764));
    path.cubic_to((388.884, 506.764), (378.345, 504.998), (378.345, 504.819));
    path.line_to((378.04, 503.03));
    path.cubic_to((378.04, 503.03), (391.415, 505.796), (391.399, 505.866));
    path.line_to((386.063, 502.132));
    path.line_to((387.547, 500.335));
    path.line_to((398.375, 501.976));
    path.line_to((403.283, 497.197));
    path.line_to((403.283, 497.197));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn joel_3(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((391.097, 334.453));
    path.line_to((390.761, 334.617));
    path.line_to((390.425, 333.937));
    path.line_to((390.761, 333.765));
    path.line_to((391.097, 334.453));
    path.close();
    path.move_to((391.128, 334.438));
    path.line_to((390.808, 334.633));
    path.line_to((390.402, 333.992));
    path.line_to((390.73, 333.781));
    path.line_to((391.128, 334.438));
    path.line_to((391.128, 334.438));
    path.close();
    path.move_to((455.073, 302.219));
    path.line_to((455.018, 302.375));
    path.line_to((454.87, 302.453));
    path.line_to((454.706, 302.109));
    path.line_to((455.073, 302.219));
    path.close();
    path.move_to((454.87, 302.453));
    path.line_to((391.097, 334.453));
    path.line_to((390.761, 333.765));
    path.line_to((454.534, 301.765));
    path.line_to((454.87, 302.453));
    path.close();
    path.move_to((456.245, 296.867));
    path.line_to((456.659, 296.953));
    path.line_to((456.526, 297.351));
    path.line_to((456.174, 297.242));
    path.line_to((456.245, 296.867));
    path.line_to((456.245, 296.867));
    path.close();
    path.move_to((456.526, 297.352));
    path.line_to((455.073, 302.219));
    path.line_to((454.339, 302.0));
    path.line_to((455.808, 297.133));
    path.line_to((456.526, 297.352));
    path.line_to((456.526, 297.352));
    path.close();
    path.move_to((450.979, 295.891));
    path.line_to((451.112, 295.813));
    path.line_to((451.26, 295.836));
    path.line_to((451.19, 296.211));
    path.line_to((450.979, 295.891));
    path.close();
    path.move_to((451.261, 295.836));
    path.line_to((456.245, 296.867));
    path.line_to((456.089, 297.617));
    path.line_to((451.105, 296.586));
    path.line_to((451.261, 295.836));
    path.close();
    path.move_to((390.729, 333.781));
    path.line_to((450.979, 295.89));
    path.line_to((451.385, 296.531));
    path.line_to((391.127, 334.437));
    path.line_to((390.729, 333.781));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn joel_4(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x4199d4fe), f32::from_bits(0x4265ac08)));
    // 19.229f, 57.418f
    path.cubic_to((f32::from_bits(0x419be979), f32::from_bits(0x426574bc)), (f32::from_bits(0x419c2b02), f32::from_bits(0x42653c6a)), (f32::from_bits(0x419af5c3), f32::from_bits(0x42645f3b)));
    // 19.489f, 57.364f, 19.521f, 57.309f, 19.37f, 57.093f
    path.cubic_to((f32::from_bits(0x419a1894), f32::from_bits(0x4263a3d7)), (f32::from_bits(0x4198cccd), f32::from_bits(0x4262f2b0)), (f32::from_bits(0x4197c290), f32::from_bits(0x4262374b)));
    // 19.262f, 56.91f, 19.1f, 56.737f, 18.97f, 56.554f
    path.cubic_to((f32::from_bits(0x41960832), f32::from_bits(0x42610c49)), (f32::from_bits(0x41944dd4), f32::from_bits(0x425fd709)), (f32::from_bits(0x41927cee), f32::from_bits(0x425ea0c4)));
    // 18.754f, 56.262f, 18.538f, 55.96f, 18.311f, 55.657f
    path.cubic_to((f32::from_bits(0x4191b646), f32::from_bits(0x425e1cab)), (f32::from_bits(0x418edd30), f32::from_bits(0x425ca4dd)), (f32::from_bits(0x418f4bc7), f32::from_bits(0x425bdd2e)));
    // 18.214f, 55.528f, 17.858f, 55.161f, 17.912f, 54.966f
    path.line_to((f32::from_bits(0x41903f7d), f32::from_bits(0x425b6e96)));
    // 18.031f, 54.858f
    path.cubic_to((f32::from_bits(0x41921062), f32::from_bits(0x425aa6e8)), (f32::from_bits(0x4193872b), f32::from_bits(0x425bd1ea)), (f32::from_bits(0x41947ae1), f32::from_bits(0x425c77cd)));
    // 18.258f, 54.663f, 18.441f, 54.955f, 18.56f, 55.117f
    path.cubic_to((f32::from_bits(0x4195dd2f), f32::from_bits(0x425d6b83)), (f32::from_bits(0x4197ae14), f32::from_bits(0x425e1caa)), (f32::from_bits(0x419924dd), f32::from_bits(0x425ef9d9)));
    // 18.733f, 55.355f, 18.96f, 55.528f, 19.143f, 55.744f
    path.cubic_to((f32::from_bits(0x419a1893), f32::from_bits(0x425f9479)), (f32::from_bits(0x419adf3b), f32::from_bits(0x42601997)), (f32::from_bits(0x419bd2f1), f32::from_bits(0x42609db0)));
    // 19.262f, 55.895f, 19.359f, 56.025f, 19.478f, 56.154f
    path.cubic_to((f32::from_bits(0x419c147a), f32::from_bits(0x4260c9b8)), (f32::from_bits(0x419c8312), f32::from_bits(0x4260e03f)), (f32::from_bits(0x419cb020), f32::from_bits(0x42610104)));
    // 19.51f, 56.197f, 19.564f, 56.219f, 19.586f, 56.251f
    path.cubic_to((f32::from_bits(0x419d0830), f32::from_bits(0x42613850)), (f32::from_bits(0x419da3d6), f32::from_bits(0x4261bd6e)), (f32::from_bits(0x419e126e), f32::from_bits(0x4261d2f0)));
    // 19.629f, 56.305f, 19.705f, 56.435f, 19.759f, 56.456f
    path.line_to((f32::from_bits(0x419e28f5), f32::from_bits(0x4261d2f0)));
    // 19.77f, 56.456f
    path.line_to((f32::from_bits(0x419e28f5), f32::from_bits(0x4261f4bb)));
    // 19.77f, 56.489f
    path.cubic_to((f32::from_bits(0x419e3d70), f32::from_bits(0x4261fef8)), (f32::from_bits(0x419e53f7), f32::from_bits(0x4261f4bb)), (f32::from_bits(0x419e8105), f32::from_bits(0x4261fef8)));
    // 19.78f, 56.499f, 19.791f, 56.489f, 19.813f, 56.499f
    path.cubic_to((f32::from_bits(0x419eac07), f32::from_bits(0x426220c3)), (f32::from_bits(0x419eac07), f32::from_bits(0x42624187)), (f32::from_bits(0x419eef9d), f32::from_bits(0x4262580f)));
    // 19.834f, 56.532f, 19.834f, 56.564f, 19.867f, 56.586f
    path.cubic_to((f32::from_bits(0x419fe353), f32::from_bits(0x4262f2af)), (f32::from_bits(0x41a0eb84), f32::from_bits(0x426377cd)), (f32::from_bits(0x41a1b22c), f32::from_bits(0x4263fbe6)));
    // 19.986f, 56.737f, 20.115f, 56.867f, 20.212f, 56.996f
    path.cubic_to((f32::from_bits(0x41a20a3c), f32::from_bits(0x42641db1)), (f32::from_bits(0x41a2e76b), f32::from_bits(0x4264a1c9)), (f32::from_bits(0x41a34188), f32::from_bits(0x4264ad0d)));
    // 20.255f, 57.029f, 20.363f, 57.158f, 20.407f, 57.169f
    path.cubic_to((f32::from_bits(0x41a36c8a), f32::from_bits(0x4264ad0d)), (f32::from_bits(0x41a3c6a7), f32::from_bits(0x4264a1c9)), (f32::from_bits(0x41a3f1a9), f32::from_bits(0x4264ad0d)));
    // 20.428f, 57.169f, 20.472f, 57.158f, 20.493f, 57.169f
    path.cubic_to((f32::from_bits(0x41a3f1a9), f32::from_bits(0x42648c48)), (f32::from_bits(0x41a41eb7), f32::from_bits(0x42648105)), (f32::from_bits(0x41a449b9), f32::from_bits(0x426475c1)));
    // 20.493f, 57.137f, 20.515f, 57.126f, 20.536f, 57.115f
    path.cubic_to((f32::from_bits(0x41a48d4f), f32::from_bits(0x4263f1a8)), (f32::from_bits(0x41a46040), f32::from_bits(0x42634082)), (f32::from_bits(0x41a48d4f), f32::from_bits(0x4262bb63)));
    // 20.569f, 56.986f, 20.547f, 56.813f, 20.569f, 56.683f
    path.cubic_to((f32::from_bits(0x41a51061), f32::from_bits(0x426122d0)), (f32::from_bits(0x41a63126), f32::from_bits(0x425f51ea)), (f32::from_bits(0x41a82d0d), f32::from_bits(0x425e0624)));
    // 20.633f, 56.284f, 20.774f, 55.83f, 21.022f, 55.506f
    path.cubic_to((f32::from_bits(0x41a90a3c), f32::from_bits(0x425d820b)), (f32::from_bits(0x41aab01f), f32::from_bits(0x425cba5d)), (f32::from_bits(0x41ab0830), f32::from_bits(0x425c147a)));
    // 21.13f, 55.377f, 21.336f, 55.182f, 21.379f, 55.02f
    path.cubic_to((f32::from_bits(0x41aa147a), f32::from_bits(0x425bf3b5)), (f32::from_bits(0x41a8df3a), f32::from_bits(0x425c0936)), (f32::from_bits(0x41a7d4fd), f32::from_bits(0x425c147a)));
    // 21.26f, 54.988f, 21.109f, 55.009f, 20.979f, 55.02f
    path.cubic_to((f32::from_bits(0x41a74fde), f32::from_bits(0x425c147a)), (f32::from_bits(0x41a65e34), f32::from_bits(0x425c4082)), (f32::from_bits(0x41a5c28e), f32::from_bits(0x425c4082)));
    // 20.914f, 55.02f, 20.796f, 55.063f, 20.72f, 55.063f
    path.cubic_to((f32::from_bits(0x41a56a7e), f32::from_bits(0x425c353e)), (f32::from_bits(0x41a4fbe6), f32::from_bits(0x425c147a)), (f32::from_bits(0x41a4ced8), f32::from_bits(0x425c0936)));
    // 20.677f, 55.052f, 20.623f, 55.02f, 20.601f, 55.009f
    path.cubic_to((f32::from_bits(0x41a53d70), f32::from_bits(0x425af4bb)), (f32::from_bits(0x41a5ed90), f32::from_bits(0x425abd6f)), (f32::from_bits(0x41a85a1c), f32::from_bits(0x425aa6e8)));
    // 20.655f, 54.739f, 20.741f, 54.685f, 21.044f, 54.663f
    path.cubic_to((f32::from_bits(0x41a920c4), f32::from_bits(0x425a9cab)), (f32::from_bits(0x41a9d0e5), f32::from_bits(0x425aa6e8)), (f32::from_bits(0x41aa5603), f32::from_bits(0x425a9167)));
    // 21.141f, 54.653f, 21.227f, 54.663f, 21.292f, 54.642f
    path.cubic_to((f32::from_bits(0x41aa8311), f32::from_bits(0x425a8623)), (f32::from_bits(0x41aa9999), f32::from_bits(0x425a655f)), (f32::from_bits(0x41aab020), f32::from_bits(0x425a655f)));
    // 21.314f, 54.631f, 21.325f, 54.599f, 21.336f, 54.599f
    path.cubic_to((f32::from_bits(0x41aa3f7c), f32::from_bits(0x42599eb7)), (f32::from_bits(0x41a9a5e3), f32::from_bits(0x42591998)), (f32::from_bits(0x41a9374b), f32::from_bits(0x42586871)));
    // 21.281f, 54.405f, 21.206f, 54.275f, 21.152f, 54.102f
    path.cubic_to((f32::from_bits(0x41a8c8b3), f32::from_bits(0x4257e458)), (f32::from_bits(0x41a8b22c), f32::from_bits(0x42575f3a)), (f32::from_bits(0x41a85a1c), f32::from_bits(0x4256c49a)));
    // 21.098f, 53.973f, 21.087f, 53.843f, 21.044f, 53.692f
    path.cubic_to((f32::from_bits(0x41a76666), f32::from_bits(0x42551479)), (f32::from_bits(0x41a68937), f32::from_bits(0x4252cabf)), (f32::from_bits(0x41a74fdf), f32::from_bits(0x4250a1c9)));
    // 20.925f, 53.27f, 20.817f, 52.698f, 20.914f, 52.158f
    path.cubic_to((f32::from_bits(0x41a77ced), f32::from_bits(0x42500729)), (f32::from_bits(0x41a870a4), f32::from_bits(0x424e8417)), (f32::from_bits(0x41a8b22d), f32::from_bits(0x424e4ccb)));
    // 20.936f, 52.007f, 21.055f, 51.629f, 21.087f, 51.575f
    path.cubic_to((f32::from_bits(0x41a8b22d), f32::from_bits(0x424e4187)), (f32::from_bits(0x41aa147b), f32::from_bits(0x424cc9b9)), (f32::from_bits(0x41aab021), f32::from_bits(0x424c2f19)));
    // 21.087f, 51.564f, 21.26f, 51.197f, 21.336f, 51.046f
    path.cubic_to((f32::from_bits(0x41aac49c), f32::from_bits(0x424c1892)), (f32::from_bits(0x41ab49bb), f32::from_bits(0x424b9eb7)), (f32::from_bits(0x41ab8b44), f32::from_bits(0x424b676b)));
    // 21.346f, 51.024f, 21.411f, 50.905f, 21.443f, 50.851f
    path.cubic_to((f32::from_bits(0x41ac3d71), f32::from_bits(0x424ab644)), (f32::from_bits(0x41ad45a2), f32::from_bits(0x424a26e8)), (f32::from_bits(0x41ae22d1), f32::from_bits(0x42498105)));
    // 21.53f, 50.678f, 21.659f, 50.538f, 21.767f, 50.376f
    path.cubic_to((f32::from_bits(0x41ae6667), f32::from_bits(0x42496b84)), (f32::from_bits(0x41aeeb85), f32::from_bits(0x42491db1)), (f32::from_bits(0x41af0000), f32::from_bits(0x4248fbe6)));
    // 21.8f, 50.355f, 21.865f, 50.279f, 21.875f, 50.246f
    path.cubic_to((f32::from_bits(0x41b0624e), f32::from_bits(0x4248353e)), (f32::from_bits(0x41b1db23), f32::from_bits(0x424779da)), (f32::from_bits(0x41b353f8), f32::from_bits(0x4246bd6f)));
    // 22.048f, 50.052f, 22.232f, 49.869f, 22.416f, 49.685f
    path.cubic_to((f32::from_bits(0x41b3c083), f32::from_bits(0x42468623)), (f32::from_bits(0x41b445a2), f32::from_bits(0x42464ed7)), (f32::from_bits(0x41b4cac1), f32::from_bits(0x4246178c)));
    // 22.469f, 49.631f, 22.534f, 49.577f, 22.599f, 49.523f
    path.cubic_to((f32::from_bits(0x41b56667), f32::from_bits(0x4245c9b9)), (f32::from_bits(0x41b62d0f), f32::from_bits(0x4245872a)), (f32::from_bits(0x41b6c8b5), f32::from_bits(0x4245449a)));
    // 22.675f, 49.447f, 22.772f, 49.382f, 22.848f, 49.317f
    path.cubic_to((f32::from_bits(0x41b7624f), f32::from_bits(0x42450311)), (f32::from_bits(0x41b7e76d), f32::from_bits(0x4244a9fa)), (f32::from_bits(0x41b88313), f32::from_bits(0x42445d2d)));
    // 22.923f, 49.253f, 22.988f, 49.166f, 23.064f, 49.091f
    path.cubic_to((f32::from_bits(0x41b949bb), f32::from_bits(0x4243ee95)), (f32::from_bits(0x41ba1063), f32::from_bits(0x424374ba)), (f32::from_bits(0x41baed92), f32::from_bits(0x42431166)));
    // 23.161f, 48.983f, 23.258f, 48.864f, 23.366f, 48.767f
    path.cubic_to((f32::from_bits(0x41bb45a2), f32::from_bits(0x4242c393)), (f32::from_bits(0x41bbb43a), f32::from_bits(0x424276c6)), (f32::from_bits(0x41bc0e57), f32::from_bits(0x424228f3)));
    // 23.409f, 48.691f, 23.463f, 48.616f, 23.507f, 48.54f
    path.cubic_to((f32::from_bits(0x41bc6667), f32::from_bits(0x4241e664)), (f32::from_bits(0x41bc7ae2), f32::from_bits(0x4241a4da)), (f32::from_bits(0x41bcd2f3), f32::from_bits(0x4241624b)));
    // 23.55f, 48.475f, 23.56f, 48.411f, 23.603f, 48.346f
    path.cubic_to((f32::from_bits(0x41bd0001), f32::from_bits(0x42411478)), (f32::from_bits(0x41bd0001), f32::from_bits(0x4240c6a5)), (f32::from_bits(0x41bd1689), f32::from_bits(0x4240851c)));
    // 23.625f, 48.27f, 23.625f, 48.194f, 23.636f, 48.13f
    path.cubic_to((f32::from_bits(0x41bd2d10), f32::from_bits(0x42404cca)), (f32::from_bits(0x41bdb023), f32::from_bits(0x423fd3f5)), (f32::from_bits(0x41bd8521), f32::from_bits(0x423f7adf)));
    // 23.647f, 48.075f, 23.711f, 47.957f, 23.69f, 47.87f
    path.line_to((f32::from_bits(0x41bd6e9a), f32::from_bits(0x423f7adf)));
    // 23.679f, 47.87f
    path.cubic_to((f32::from_bits(0x41bd6e9a), f32::from_bits(0x423f7adf)), (f32::from_bits(0x41bd5813), f32::from_bits(0x423f4ed7)), (f32::from_bits(0x41bd168a), f32::from_bits(0x423f4499)));
    // 23.679f, 47.87f, 23.668f, 47.827f, 23.636f, 47.817f
    path.cubic_to((f32::from_bits(0x41bc916b), f32::from_bits(0x423f22ce)), (f32::from_bits(0x41bc22d4), f32::from_bits(0x423f3955)), (f32::from_bits(0x41bb893a), f32::from_bits(0x423f178b)));
    // 23.571f, 47.784f, 23.517f, 47.806f, 23.442f, 47.773f
    path.cubic_to((f32::from_bits(0x41bb2f1d), f32::from_bits(0x423f0c47)), (f32::from_bits(0x41bb041b), f32::from_bits(0x423ee03f)), (f32::from_bits(0x41baac0b), f32::from_bits(0x423ec9b8)));
    // 23.398f, 47.762f, 23.377f, 47.719f, 23.334f, 47.697f
    path.cubic_to((f32::from_bits(0x41baac0b), f32::from_bits(0x423ebf7b)), (f32::from_bits(0x41bac086), f32::from_bits(0x423ea8f3)), (f32::from_bits(0x41bac086), f32::from_bits(0x423e926c)));
    // 23.334f, 47.687f, 23.344f, 47.665f, 23.344f, 47.643f
    path.cubic_to((f32::from_bits(0x41bb2f1e), f32::from_bits(0x423e882f)), (f32::from_bits(0x41bc0e59), f32::from_bits(0x423e6664)), (f32::from_bits(0x41bc916b), f32::from_bits(0x423e5c26)));
    // 23.398f, 47.633f, 23.507f, 47.6f, 23.571f, 47.59f
    path.cubic_to((f32::from_bits(0x41be4bc9), f32::from_bits(0x423e50e2)), (f32::from_bits(0x41c53542), f32::from_bits(0x423e926c)), (f32::from_bits(0x41c5ba61), f32::from_bits(0x423e24da)));
    // 23.787f, 47.579f, 24.651f, 47.643f, 24.716f, 47.536f
    path.cubic_to((f32::from_bits(0x41c61271), f32::from_bits(0x423de24b)), (f32::from_bits(0x41c61271), f32::from_bits(0x423d1a9d)), (f32::from_bits(0x41c63f80), f32::from_bits(0x423ca1c8)));
    // 24.759f, 47.471f, 24.759f, 47.276f, 24.781f, 47.158f
    path.cubic_to((f32::from_bits(0x41c68109), f32::from_bits(0x423bda1a)), (f32::from_bits(0x41c6ae18), f32::from_bits(0x423afceb)), (f32::from_bits(0x41c70628), f32::from_bits(0x423a2aff)));
    // 24.813f, 46.963f, 24.835f, 46.747f, 24.878f, 46.542f
    path.cubic_to((f32::from_bits(0x41c71caf), f32::from_bits(0x42399ba3)), (f32::from_bits(0x41c81065), f32::from_bits(0x42379eb5)), (f32::from_bits(0x41c79fc2), f32::from_bits(0x4237459f)));
    // 24.889f, 46.402f, 25.008f, 45.905f, 24.953f, 45.818f
    path.cubic_to((f32::from_bits(0x41c70628), f32::from_bits(0x4236e24b)), (f32::from_bits(0x41c4dd33), f32::from_bits(0x4237459f)), (f32::from_bits(0x41c45814), f32::from_bits(0x423750e3)));
    // 24.878f, 45.721f, 24.608f, 45.818f, 24.543f, 45.829f
    path.cubic_to((f32::from_bits(0x41c245a5), f32::from_bits(0x42379eb6)), (f32::from_bits(0x41bea5e7), f32::from_bits(0x42380d4d)), (f32::from_bits(0x41bbf5c6), f32::from_bits(0x4237ec89)));
    // 24.284f, 45.905f, 23.831f, 46.013f, 23.495f, 45.981f
    path.cubic_to((f32::from_bits(0x41b9f9df), f32::from_bits(0x4237e145)), (f32::from_bits(0x41b7e770), f32::from_bits(0x4237a9fa)), (f32::from_bits(0x41b62d12), f32::from_bits(0x4237676a)));
    // 23.247f, 45.97f, 22.988f, 45.916f, 22.772f, 45.851f
    path.cubic_to((f32::from_bits(0x41b4312b), f32::from_bits(0x423724db)), (f32::from_bits(0x41b1f1ae), f32::from_bits(0x42369fbc)), (f32::from_bits(0x41af9baa), f32::from_bits(0x423673b4)));
    // 22.524f, 45.786f, 22.243f, 45.656f, 21.951f, 45.613f
    path.cubic_to((f32::from_bits(0x41ae7ae5), f32::from_bits(0x42366977)), (f32::from_bits(0x41aced96), f32::from_bits(0x42365d2d)), (f32::from_bits(0x41ab8b48), f32::from_bits(0x42366977)));
    // 21.81f, 45.603f, 21.616f, 45.591f, 21.443f, 45.603f
    path.cubic_to((f32::from_bits(0x41a9e771), f32::from_bits(0x42368a3c)), (f32::from_bits(0x41a82d13), f32::from_bits(0x4236d708)), (f32::from_bits(0x41a65e3a), f32::from_bits(0x4236b644)));
    // 21.238f, 45.635f, 21.022f, 45.71f, 20.796f, 45.678f
    path.cubic_to((f32::from_bits(0x41a65e3a), f32::from_bits(0x4236ab00)), (f32::from_bits(0x41a647b3), f32::from_bits(0x42369fbd)), (f32::from_bits(0x41a65e3a), f32::from_bits(0x42369479)));
    // 20.796f, 45.667f, 20.785f, 45.656f, 20.796f, 45.645f
    path.cubic_to((f32::from_bits(0x41a672b5), f32::from_bits(0x42366977)), (f32::from_bits(0x41a7a7f4), f32::from_bits(0x42363125)), (f32::from_bits(0x41a81898), f32::from_bits(0x42361ba4)));
    // 20.806f, 45.603f, 20.957f, 45.548f, 21.012f, 45.527f
    path.cubic_to((f32::from_bits(0x41a85a21), f32::from_bits(0x42361060)), (f32::from_bits(0x41a8df40), f32::from_bits(0x4235d915)), (f32::from_bits(0x41a94dd7), f32::from_bits(0x4235cdd1)));
    // 21.044f, 45.516f, 21.109f, 45.462f, 21.163f, 45.451f
    path.cubic_to((f32::from_bits(0x41ab8b48), f32::from_bits(0x42356a7d)), (f32::from_bits(0x41af8523), f32::from_bits(0x423575c1)), (f32::from_bits(0x41b249be), f32::from_bits(0x42359685)));
    // 21.443f, 45.354f, 21.94f, 45.365f, 22.286f, 45.397f
    path.cubic_to((f32::from_bits(0x41b3d70e), f32::from_bits(0x4235a1c9)), (f32::from_bits(0x41b6168b), f32::from_bits(0x4235cdd1)), (f32::from_bits(0x41b7e770), f32::from_bits(0x4235ad0c)));
    // 22.48f, 45.408f, 22.761f, 45.451f, 22.988f, 45.419f
    path.cubic_to((f32::from_bits(0x41bac087), f32::from_bits(0x42359685)), (f32::from_bits(0x41bd6e9b), f32::from_bits(0x4234fbe5)), (f32::from_bits(0x41c03337), f32::from_bits(0x4234af18)));
    // 23.344f, 45.397f, 23.679f, 45.246f, 24.025f, 45.171f
    path.cubic_to((f32::from_bits(0x41c2cac4), f32::from_bits(0x42346145)), (f32::from_bits(0x41c56252), f32::from_bits(0x4234820a)), (f32::from_bits(0x41c81066), f32::from_bits(0x42346145)));
    // 24.349f, 45.095f, 24.673f, 45.127f, 25.008f, 45.095f
    path.cubic_to((f32::from_bits(0x41c824e1), f32::from_bits(0x42340935)), (f32::from_bits(0x41c89378), f32::from_bits(0x42330a3b)), (f32::from_bits(0x41c7b649), f32::from_bits(0x4232fef7)));
    // 25.018f, 45.009f, 25.072f, 44.76f, 24.964f, 44.749f
    path.cubic_to((f32::from_bits(0x41c6d91a), f32::from_bits(0x4232e976)), (f32::from_bits(0x41c5a3da), f32::from_bits(0x42338416)), (f32::from_bits(0x41c51ebc), f32::from_bits(0x4233a4da)));
    // 24.856f, 44.728f, 24.705f, 44.879f, 24.64f, 44.911f
    path.cubic_to((f32::from_bits(0x41c42b06), f32::from_bits(0x4233bb61)), (f32::from_bits(0x41c2cac4), f32::from_bits(0x4233d0e2)), (f32::from_bits(0x41c1d70e), f32::from_bits(0x4233e769)));
    // 24.521f, 44.933f, 24.349f, 44.954f, 24.23f, 44.976f
    path.cubic_to((f32::from_bits(0x41c08b47), f32::from_bits(0x4233f2ad)), (f32::from_bits(0x41bf1272), f32::from_bits(0x4233c6a4)), (f32::from_bits(0x41bdf3ba), f32::from_bits(0x4233bb61)));
    // 24.068f, 44.987f, 23.884f, 44.944f, 23.744f, 44.933f
    path.cubic_to((f32::from_bits(0x41bcd2f5), f32::from_bits(0x4233b01d)), (f32::from_bits(0x41bbf5c6), f32::from_bits(0x4233b01d)), (f32::from_bits(0x41baed95), f32::from_bits(0x4233a4da)));
    // 23.603f, 44.922f, 23.495f, 44.922f, 23.366f, 44.911f
    path.cubic_to((f32::from_bits(0x41ba26ed), f32::from_bits(0x42338f59)), (f32::from_bits(0x41b91cb0), f32::from_bits(0x4233580d)), (f32::from_bits(0x41b83f81), f32::from_bits(0x4233580d)));
    // 23.269f, 44.89f, 23.139f, 44.836f, 23.031f, 44.836f
    path.cubic_to((f32::from_bits(0x41b4b43d), f32::from_bits(0x42333642)), (f32::from_bits(0x41b19791), f32::from_bits(0x4233a4da)), (f32::from_bits(0x41aea7f4), f32::from_bits(0x4233d0e2)));
    // 22.588f, 44.803f, 22.199f, 44.911f, 21.832f, 44.954f
    path.cubic_to((f32::from_bits(0x41aba1cf), f32::from_bits(0x42340934)), (f32::from_bits(0x41a7666b), f32::from_bits(0x4233e769)), (f32::from_bits(0x41a4b856), f32::from_bits(0x42338415)));
    // 21.454f, 45.009f, 20.925f, 44.976f, 20.59f, 44.879f
    path.cubic_to((f32::from_bits(0x41a46046), f32::from_bits(0x423378d1)), (f32::from_bits(0x41a3f1ae), f32::from_bits(0x4233580d)), (f32::from_bits(0x41a3c6ac), f32::from_bits(0x42334cc9)));
    // 20.547f, 44.868f, 20.493f, 44.836f, 20.472f, 44.825f
    path.cubic_to((f32::from_bits(0x41a28f60), f32::from_bits(0x4233157d)), (f32::from_bits(0x41a19db6), f32::from_bits(0x42330a3a)), (f32::from_bits(0x41a0c087), f32::from_bits(0x4232c7aa)));
    // 20.32f, 44.771f, 20.202f, 44.76f, 20.094f, 44.695f
    path.cubic_to((f32::from_bits(0x41a0eb89), f32::from_bits(0x4232bc66)), (f32::from_bits(0x41a0eb89), f32::from_bits(0x4232905e)), (f32::from_bits(0x41a10210), f32::from_bits(0x4232905e)));
    // 20.115f, 44.684f, 20.115f, 44.641f, 20.126f, 44.641f
    path.cubic_to((f32::from_bits(0x41a19db6), f32::from_bits(0x42325912)), (f32::from_bits(0x41a2645e), f32::from_bits(0x42326f99)), (f32::from_bits(0x41a35608), f32::from_bits(0x42326f99)));
    // 20.202f, 44.587f, 20.299f, 44.609f, 20.417f, 44.609f
    path.cubic_to((f32::from_bits(0x41a476cd), f32::from_bits(0x42324ed4)), (f32::from_bits(0x41a5ed95), f32::from_bits(0x4232384d)), (f32::from_bits(0x41a724e1), f32::from_bits(0x42320c45)));
    // 20.558f, 44.577f, 20.741f, 44.555f, 20.893f, 44.512f
    path.cubic_to((f32::from_bits(0x41a8c8b8), f32::from_bits(0x4231c9b6)), (f32::from_bits(0x41aa999d), f32::from_bits(0x42316662)), (f32::from_bits(0x41ac26ed), f32::from_bits(0x4231188f)));
    // 21.098f, 44.447f, 21.325f, 44.35f, 21.519f, 44.274f
    path.cubic_to((f32::from_bits(0x41af168b), f32::from_bits(0x423072ac)), (f32::from_bits(0x41b249be), f32::from_bits(0x42300f58)), (f32::from_bits(0x41b57ae5), f32::from_bits(0x422fe249)));
    // 21.886f, 44.112f, 22.286f, 44.015f, 22.685f, 43.971f
    path.cubic_to((f32::from_bits(0x41b66e9b), f32::from_bits(0x422fd80c)), (f32::from_bits(0x41b7d0e9), f32::from_bits(0x422fee93)), (f32::from_bits(0x41b89791), f32::from_bits(0x422fee93)));
    // 22.804f, 43.961f, 22.977f, 43.983f, 23.074f, 43.983f
    path.cubic_to((f32::from_bits(0x41bb1897), f32::from_bits(0x42300f58)), (f32::from_bits(0x41bd2d12), f32::from_bits(0x423024d9)), (f32::from_bits(0x41bfc49f), f32::from_bits(0x4230301c)));
    // 23.387f, 44.015f, 23.647f, 44.036f, 23.971f, 44.047f
    path.cubic_to((f32::from_bits(0x41c0e357), f32::from_bits(0x423046a3)), (f32::from_bits(0x41c245a5), f32::from_bits(0x42305c24)), (f32::from_bits(0x41c3a7f3), f32::from_bits(0x423051e7)));
    // 24.111f, 44.069f, 24.284f, 44.09f, 24.457f, 44.08f
    path.cubic_to((f32::from_bits(0x41c50835), f32::from_bits(0x423046a3)), (f32::from_bits(0x41c69791), f32::from_bits(0x42300f58)), (f32::from_bits(0x41c79fc2), f32::from_bits(0x422fb641)));
    // 24.629f, 44.069f, 24.824f, 44.015f, 24.953f, 43.928f
    path.cubic_to((f32::from_bits(0x41c7f9df), f32::from_bits(0x422fa0c0)), (f32::from_bits(0x41c86876), f32::from_bits(0x422f5e31)), (f32::from_bits(0x41c8eb89), f32::from_bits(0x422f52ed)));
    // 24.997f, 43.907f, 25.051f, 43.842f, 25.115f, 43.831f
    path.cubic_to((f32::from_bits(0x41c9b43d), f32::from_bits(0x422f3c66)), (f32::from_bits(0x41c9df3f), f32::from_bits(0x422fb641)), (f32::from_bits(0x41c9f5c6), f32::from_bits(0x42300f57)));
    // 25.213f, 43.809f, 25.234f, 43.928f, 25.245f, 44.015f
    path.cubic_to((f32::from_bits(0x41ca0c4d), f32::from_bits(0x4230e143)), (f32::from_bits(0x41c9df3f), f32::from_bits(0x42319ca7)), (f32::from_bits(0x41c9f5c6), f32::from_bits(0x4232384d)));
    // 25.256f, 44.22f, 25.234f, 44.403f, 25.245f, 44.555f
    path.cubic_to((f32::from_bits(0x41ca395c), f32::from_bits(0x4234fbe2)), (f32::from_bits(0x41ca22d4), f32::from_bits(0x4237cabc)), (f32::from_bits(0x41ca7ae5), f32::from_bits(0x423a6d8c)));
    // 25.278f, 45.246f, 25.267f, 45.948f, 25.31f, 46.607f
    path.cubic_to((f32::from_bits(0x41ca916c), f32::from_bits(0x423b3f78)), (f32::from_bits(0x41ca645e), f32::from_bits(0x423ca1c5)), (f32::from_bits(0x41ca916c), f32::from_bits(0x423d9475)));
    // 25.321f, 46.812f, 25.299f, 47.158f, 25.321f, 47.395f
    path.cubic_to((f32::from_bits(0x41ca916c), f32::from_bits(0x423daafc)), (f32::from_bits(0x41ca7ae5), f32::from_bits(0x423dd704)), (f32::from_bits(0x41ca916c), f32::from_bits(0x423dec85)));
    // 25.321f, 47.417f, 25.31f, 47.46f, 25.321f, 47.481f
    path.cubic_to((f32::from_bits(0x41caa5e7), f32::from_bits(0x423e0e50)), (f32::from_bits(0x41cb0004), f32::from_bits(0x423e459c)), (f32::from_bits(0x41cb2b06), f32::from_bits(0x423e50df)));
    // 25.331f, 47.514f, 25.375f, 47.568f, 25.396f, 47.579f
    path.cubic_to((f32::from_bits(0x41cb6e9c), f32::from_bits(0x423e5c23)), (f32::from_bits(0x41ce47b2), f32::from_bits(0x423e7ce7)), (f32::from_bits(0x41ce8b48), f32::from_bits(0x423e6660)));
    // 25.429f, 47.59f, 25.785f, 47.622f, 25.818f, 47.6f
    path.line_to((f32::from_bits(0x41ceb64a), f32::from_bits(0x423e5c23)));
    // 25.839f, 47.59f
    path.cubic_to((f32::from_bits(0x41d1395c), f32::from_bits(0x423e5c23)), (f32::from_bits(0x41d41273), f32::from_bits(0x423e50df)), (f32::from_bits(0x41d6666b), f32::from_bits(0x423e6660)));
    // 26.153f, 47.59f, 26.509f, 47.579f, 26.8f, 47.6f
    path.cubic_to((f32::from_bits(0x41d71898), f32::from_bits(0x423e7ce7)), (f32::from_bits(0x41d80a42), f32::from_bits(0x423e5c23)), (f32::from_bits(0x41d8a5e8), f32::from_bits(0x423e7ce7)));
    // 26.887f, 47.622f, 27.005f, 47.59f, 27.081f, 47.622f
    path.cubic_to((f32::from_bits(0x41d8d2f6), f32::from_bits(0x423e882b)), (f32::from_bits(0x41d8d2f6), f32::from_bits(0x423e9268)), (f32::from_bits(0x41d8fdf8), f32::from_bits(0x423e9eb2)));
    // 27.103f, 47.633f, 27.103f, 47.643f, 27.124f, 47.655f
    path.cubic_to((f32::from_bits(0x41d8e771), f32::from_bits(0x423ebf77)), (f32::from_bits(0x41d8fdf8), f32::from_bits(0x423ed4f8)), (f32::from_bits(0x41d8e771), f32::from_bits(0x423eeb7f)));
    // 27.113f, 47.687f, 27.124f, 47.708f, 27.113f, 47.73f
    path.cubic_to((f32::from_bits(0x41d88f61), f32::from_bits(0x423f4496)), (f32::from_bits(0x41d71898), f32::from_bits(0x423f4496)), (f32::from_bits(0x41d6aa00), f32::from_bits(0x423f9162)));
    // 27.07f, 47.817f, 26.887f, 47.817f, 26.833f, 47.892f
    path.cubic_to((f32::from_bits(0x41d547b2), f32::from_bits(0x42406e91)), (f32::from_bits(0x41d43d75), f32::from_bits(0x4241ba58)), (f32::from_bits(0x41d38d54), f32::from_bits(0x4242b952)));
    // 26.66f, 48.108f, 26.53f, 48.432f, 26.444f, 48.681f
    path.cubic_to((f32::from_bits(0x41d1395c), f32::from_bits(0x4245a8f0)), (f32::from_bits(0x41d0b231), f32::from_bits(0x42491dac)), (f32::from_bits(0x41d2147f), f32::from_bits(0x424c2f15)));
    // 26.153f, 49.415f, 26.087f, 50.279f, 26.26f, 51.046f
    path.cubic_to((f32::from_bits(0x41d2418d), f32::from_bits(0x424c7be2)), (f32::from_bits(0x41d2999e), f32::from_bits(0x424cc9b5)), (f32::from_bits(0x41d2b025), f32::from_bits(0x424d0c44)));
    // 26.282f, 51.121f, 26.325f, 51.197f, 26.336f, 51.262f
    path.cubic_to((f32::from_bits(0x41d33544), f32::from_bits(0x424dc7a8)), (f32::from_bits(0x41d3a3db), f32::from_bits(0x424e8413)), (f32::from_bits(0x41d453fc), f32::from_bits(0x424f136f)));
    // 26.401f, 51.445f, 26.455f, 51.629f, 26.541f, 51.769f
    path.cubic_to((f32::from_bits(0x41d453fc), f32::from_bits(0x424f136f)), (f32::from_bits(0x41d59fc3), f32::from_bits(0x42506a79)), (f32::from_bits(0x41d6c087), f32::from_bits(0x4250e454)));
    // 26.541f, 51.769f, 26.703f, 52.104f, 26.844f, 52.223f
    path.cubic_to((f32::from_bits(0x41d6c087), f32::from_bits(0x4250ef98)), (f32::from_bits(0x41d6eb89), f32::from_bits(0x4251105c)), (f32::from_bits(0x41d70210), f32::from_bits(0x4251105c)));
    // 26.844f, 52.234f, 26.865f, 52.266f, 26.876f, 52.266f
    path.cubic_to((f32::from_bits(0x41d71897), f32::from_bits(0x42511ba0)), (f32::from_bits(0x41d75a20), f32::from_bits(0x4251105c)), (f32::from_bits(0x41d7872f), f32::from_bits(0x4251105c)));
    // 26.887f, 52.277f, 26.919f, 52.266f, 26.941f, 52.266f
    path.cubic_to((f32::from_bits(0x41d87ae5), f32::from_bits(0x42501ca6)), (f32::from_bits(0x41d9147f), f32::from_bits(0x424f136e)), (f32::from_bits(0x41da0835), f32::from_bits(0x424e157b)));
    // 27.06f, 52.028f, 27.135f, 51.769f, 27.254f, 51.521f
    path.cubic_to((f32::from_bits(0x41da1ebc), f32::from_bits(0x424df4b6)), (f32::from_bits(0x41db1066), f32::from_bits(0x424d0c44)), (f32::from_bits(0x41db1066), f32::from_bits(0x424d0100)));
    // 27.265f, 51.489f, 27.383f, 51.262f, 27.383f, 51.251f
    path.cubic_to((f32::from_bits(0x41db3d74), f32::from_bits(0x424cc9b4)), (f32::from_bits(0x41db9585), f32::from_bits(0x424c8725)), (f32::from_bits(0x41dbd91a), f32::from_bits(0x424c5b1d)));
    // 27.405f, 51.197f, 27.448f, 51.132f, 27.481f, 51.089f
    path.cubic_to((f32::from_bits(0x41dc5e39), f32::from_bits(0x424bcbc1)), (f32::from_bits(0x41dcf7d2), f32::from_bits(0x424b301b)), (f32::from_bits(0x41dd7cf1), f32::from_bits(0x424aac02)));
    // 27.546f, 50.949f, 27.621f, 50.797f, 27.686f, 50.668f
    path.cubic_to((f32::from_bits(0x41ddd501), f32::from_bits(0x424a5e2f)), (f32::from_bits(0x41ddeb89), f32::from_bits(0x424a105c)), (f32::from_bits(0x41de4399), f32::from_bits(0x4249b84c)));
    // 27.729f, 50.592f, 27.74f, 50.516f, 27.783f, 50.43f
    path.cubic_to((f32::from_bits(0x41de70a7), f32::from_bits(0x4249a1c5)), (f32::from_bits(0x41def5c6), f32::from_bits(0x42490725)), (f32::from_bits(0x41df20c8), f32::from_bits(0x4248e660)));
    // 27.805f, 50.408f, 27.87f, 50.257f, 27.891f, 50.225f
    path.cubic_to((f32::from_bits(0x41df8f60), f32::from_bits(0x42488206)), (f32::from_bits(0x41e0c49f), f32::from_bits(0x42474cc6)), (f32::from_bits(0x41e10835), f32::from_bits(0x42472c02)));
    // 27.945f, 50.127f, 28.096f, 49.825f, 28.129f, 49.793f
    path.cubic_to((f32::from_bits(0x41e11ebc), f32::from_bits(0x42472c02)), (f32::from_bits(0x41e13337), f32::from_bits(0x4246fef4)), (f32::from_bits(0x41e13337), f32::from_bits(0x4246f4b6)));
    // 28.14f, 49.793f, 28.15f, 49.749f, 28.15f, 49.739f
    path.cubic_to((f32::from_bits(0x41e149be), f32::from_bits(0x4246c7a8)), (f32::from_bits(0x41e226ed), f32::from_bits(0x42461787)), (f32::from_bits(0x41e253fc), f32::from_bits(0x4245df35)));
    // 28.161f, 49.695f, 28.269f, 49.523f, 28.291f, 49.468f
    path.cubic_to((f32::from_bits(0x41e27efe), f32::from_bits(0x4245d3f1)), (f32::from_bits(0x41e2ac0c), f32::from_bits(0x42459ca6)), (f32::from_bits(0x41e2ac0c), f32::from_bits(0x42459162)));
    // 28.312f, 49.457f, 28.334f, 49.403f, 28.334f, 49.392f
    path.cubic_to((f32::from_bits(0x41e372b4), f32::from_bits(0x4244e141)), (f32::from_bits(0x41e4666a), f32::from_bits(0x42445c23)), (f32::from_bits(0x41e4eb89), f32::from_bits(0x42437ef3)));
    // 28.431f, 49.22f, 28.55f, 49.09f, 28.615f, 48.874f
    path.cubic_to((f32::from_bits(0x41e4a7f3), f32::from_bits(0x424373af)), (f32::from_bits(0x41e47ae5), f32::from_bits(0x42435e2e)), (f32::from_bits(0x41e4666a), f32::from_bits(0x42435e2e)));
    // 28.582f, 48.863f, 28.56f, 48.842f, 28.55f, 48.842f
    path.cubic_to((f32::from_bits(0x41e3893b), f32::from_bits(0x42433c63)), (f32::from_bits(0x41e1fbeb), f32::from_bits(0x4243686b)), (f32::from_bits(0x41e18b47), f32::from_bits(0x42431b9f)));
    // 28.442f, 48.809f, 28.248f, 48.852f, 28.193f, 48.777f
    path.cubic_to((f32::from_bits(0x41e16045), f32::from_bits(0x4242f9d4)), (f32::from_bits(0x41e18b47), f32::from_bits(0x4242ee91)), (f32::from_bits(0x41e16045), f32::from_bits(0x4242d910)));
    // 28.172f, 48.744f, 28.193f, 48.733f, 28.172f, 48.712f
    path.cubic_to((f32::from_bits(0x41e1a1ce), f32::from_bits(0x4242b84b)), (f32::from_bits(0x41e1fbeb), f32::from_bits(0x42429681)), (f32::from_bits(0x41e226ed), f32::from_bits(0x42429681)));
    // 28.204f, 48.68f, 28.248f, 48.647f, 28.269f, 48.647f
    path.cubic_to((f32::from_bits(0x41e3cac4), f32::from_bits(0x42425f35)), (f32::from_bits(0x41e9c087), f32::from_bits(0x4242b84c)), (f32::from_bits(0x41ea5c2c), f32::from_bits(0x424248ae)));
    // 28.474f, 48.593f, 29.219f, 48.68f, 29.295f, 48.571f
    path.cubic_to((f32::from_bits(0x41eacac4), f32::from_bits(0x4241fbe1)), (f32::from_bits(0x41eacac4), f32::from_bits(0x42414aba)), (f32::from_bits(0x41eaf7d2), f32::from_bits(0x4240d0df)));
    // 29.349f, 48.496f, 29.349f, 48.323f, 29.371f, 48.204f
    path.cubic_to((f32::from_bits(0x41eb395b), f32::from_bits(0x4240580a)), (f32::from_bits(0x41eba7f3), f32::from_bits(0x423fb121)), (f32::from_bits(0x41ebd501), f32::from_bits(0x423f21c4)));
    // 29.403f, 48.086f, 29.457f, 47.923f, 29.479f, 47.783f
    path.cubic_to((f32::from_bits(0x41ec2d11), f32::from_bits(0x423e4fd8)), (f32::from_bits(0x41ec5813), f32::from_bits(0x423d936e)), (f32::from_bits(0x41ecb230), f32::from_bits(0x423cb63f)));
    // 29.522f, 47.578f, 29.543f, 47.394f, 29.587f, 47.178f
    path.cubic_to((f32::from_bits(0x41ecc8b7), f32::from_bits(0x423c5e2f)), (f32::from_bits(0x41edba61), f32::from_bits(0x423b332d)), (f32::from_bits(0x41ed8f5f), f32::from_bits(0x423ac495)));
    // 29.598f, 47.092f, 29.716f, 46.8f, 29.695f, 46.692f
    path.cubic_to((f32::from_bits(0x41ed6251), f32::from_bits(0x423a8d49)), (f32::from_bits(0x41ec9ba9), f32::from_bits(0x423a407c)), (f32::from_bits(0x41ec2d11), f32::from_bits(0x423a3539)));
    // 29.673f, 46.638f, 29.576f, 46.563f, 29.522f, 46.552f
    path.cubic_to((f32::from_bits(0x41ec0003), f32::from_bits(0x423a29f5)), (f32::from_bits(0x41ebeb88), f32::from_bits(0x423a3539)), (f32::from_bits(0x41ebd501), f32::from_bits(0x423a3539)));
    // 29.5f, 46.541f, 29.49f, 46.552f, 29.479f, 46.552f
    path.cubic_to((f32::from_bits(0x41eb6669), f32::from_bits(0x423a29f5)), (f32::from_bits(0x41ea72b3), f32::from_bits(0x4239f2aa)), (f32::from_bits(0x41e9c086), f32::from_bits(0x423a0931)));
    // 29.425f, 46.541f, 29.306f, 46.487f, 29.219f, 46.509f
    path.cubic_to((f32::from_bits(0x41e99584), f32::from_bits(0x423a0931)), (f32::from_bits(0x41e96876), f32::from_bits(0x423a29f6)), (f32::from_bits(0x41e953fb), f32::from_bits(0x423a3539)));
    // 29.198f, 46.509f, 29.176f, 46.541f, 29.166f, 46.552f
    path.cubic_to((f32::from_bits(0x41e96876), f32::from_bits(0x423a8d49)), (f32::from_bits(0x41e9c086), f32::from_bits(0x423acfd9)), (f32::from_bits(0x41e9d70d), f32::from_bits(0x423b28ef)));
    // 29.176f, 46.638f, 29.219f, 46.703f, 29.23f, 46.79f
    path.cubic_to((f32::from_bits(0x41ea041b), f32::from_bits(0x423bd910)), (f32::from_bits(0x41e8fbea), f32::from_bits(0x423c73b0)), (f32::from_bits(0x41e849bd), f32::from_bits(0x423cac01)));
    // 29.252f, 46.962f, 29.123f, 47.113f, 29.036f, 47.168f
    path.cubic_to((f32::from_bits(0x41e75607), f32::from_bits(0x423cf8ce)), (f32::from_bits(0x41e5f3b9), f32::from_bits(0x423ced8a)), (f32::from_bits(0x41e4eb88), f32::from_bits(0x423cd809)));
    // 28.917f, 47.243f, 28.744f, 47.232f, 28.615f, 47.211f
    path.cubic_to((f32::from_bits(0x41e372b3), f32::from_bits(0x423cb63e)), (f32::from_bits(0x41e2ac0b), f32::from_bits(0x423c0517)), (f32::from_bits(0x41e10834), f32::from_bits(0x423c52ea)));
    // 28.431f, 47.178f, 28.334f, 47.005f, 28.129f, 47.081f
    path.cubic_to((f32::from_bits(0x41e0db26), f32::from_bits(0x423cd809)), (f32::from_bits(0x41e0999c), f32::from_bits(0x423d46a0)), (f32::from_bits(0x41dfd0e8), f32::from_bits(0x423d72a8)));
    // 28.107f, 47.211f, 28.075f, 47.319f, 27.977f, 47.362f
    path.cubic_to((f32::from_bits(0x41deb230), f32::from_bits(0x423dcab8)), (f32::from_bits(0x41dd3b67), f32::from_bits(0x423d8829)), (f32::from_bits(0x41dc312a), f32::from_bits(0x423d46a0)));
    // 27.837f, 47.448f, 27.654f, 47.383f, 27.524f, 47.319f
    path.cubic_to((f32::from_bits(0x41dae563), f32::from_bits(0x423cf8cd)), (f32::from_bits(0x41d98316), f32::from_bits(0x423cccc5)), (f32::from_bits(0x41d8645d), f32::from_bits(0x423c6971)));
    // 27.362f, 47.243f, 27.189f, 47.2f, 27.049f, 47.103f
    path.cubic_to((f32::from_bits(0x41d7df3e), f32::from_bits(0x423c52ea)), (f32::from_bits(0x41d72d11), f32::from_bits(0x423c311f)), (f32::from_bits(0x41d6a9ff), f32::from_bits(0x423c0517)));
    // 26.984f, 47.081f, 26.897f, 47.048f, 26.833f, 47.005f
    path.cubic_to((f32::from_bits(0x41d67cf1), f32::from_bits(0x423bfada)), (f32::from_bits(0x41d572b3), f32::from_bits(0x423b967f)), (f32::from_bits(0x41d5893a), f32::from_bits(0x423b967f)));
    // 26.811f, 46.995f, 26.681f, 46.897f, 26.692f, 46.897f
    path.cubic_to((f32::from_bits(0x41d5893a), f32::from_bits(0x423b967f)), (f32::from_bits(0x41d5b648), f32::from_bits(0x423b6a77)), (f32::from_bits(0x41d5ccd0), f32::from_bits(0x423b6a77)));
    // 26.692f, 46.897f, 26.714f, 46.854f, 26.725f, 46.854f
    path.cubic_to((f32::from_bits(0x41d6eb88), f32::from_bits(0x423b3e6f)), (f32::from_bits(0x41d8374f), f32::from_bits(0x423b967f)), (f32::from_bits(0x41d8fdf7), f32::from_bits(0x423bad06)));
    // 26.865f, 46.811f, 27.027f, 46.897f, 27.124f, 46.919f
    path.cubic_to((f32::from_bits(0x41d9c6ab), f32::from_bits(0x423bb84a)), (f32::from_bits(0x41da49be), f32::from_bits(0x423bb84a)), (f32::from_bits(0x41db1066), f32::from_bits(0x423bd90e)));
    // 27.222f, 46.93f, 27.286f, 46.93f, 27.383f, 46.962f
    path.cubic_to((f32::from_bits(0x41db810a), f32::from_bits(0x423bd90e)), (f32::from_bits(0x41dc5e39), f32::from_bits(0x423bfad9)), (f32::from_bits(0x41dcf7d3), f32::from_bits(0x423bef95)));
    // 27.438f, 46.962f, 27.546f, 46.995f, 27.621f, 46.984f
    path.cubic_to((f32::from_bits(0x41ddd502), f32::from_bits(0x423bc38d)), (f32::from_bits(0x41dd4fe3), f32::from_bits(0x423b332b)), (f32::from_bits(0x41dd7cf2), f32::from_bits(0x423ab94f)));
    // 27.729f, 46.941f, 27.664f, 46.8f, 27.686f, 46.681f
    path.cubic_to((f32::from_bits(0x41dda7f4), f32::from_bits(0x423a77c6)), (f32::from_bits(0x41de2d13), f32::from_bits(0x423a29f3)), (f32::from_bits(0x41de70a8), f32::from_bits(0x423a136c)));
    // 27.707f, 46.617f, 27.772f, 46.541f, 27.805f, 46.519f
    path.cubic_to((f32::from_bits(0x41dfba62), f32::from_bits(0x4239c69f)), (f32::from_bits(0x41e253fc), f32::from_bits(0x423a092f)), (f32::from_bits(0x41e372b4), f32::from_bits(0x423a4bbe)));
    // 27.966f, 46.444f, 28.291f, 46.509f, 28.431f, 46.574f
    path.cubic_to((f32::from_bits(0x41e40e5a), f32::from_bits(0x423a6c83)), (f32::from_bits(0x41e49379), f32::from_bits(0x423a8d47)), (f32::from_bits(0x41e55a21), f32::from_bits(0x423ab94f)));
    // 28.507f, 46.606f, 28.572f, 46.638f, 28.669f, 46.681f
    path.line_to((f32::from_bits(0x41e58523), f32::from_bits(0x423acfd6)));
    // 28.69f, 46.703f
    path.cubic_to((f32::from_bits(0x41e5b231), f32::from_bits(0x423acfd6)), (f32::from_bits(0x41e60a42), f32::from_bits(0x423ac492)), (f32::from_bits(0x41e66252), f32::from_bits(0x423acfd6)));
    // 28.712f, 46.703f, 28.755f, 46.692f, 28.798f, 46.703f
    path.cubic_to((f32::from_bits(0x41e66252), f32::from_bits(0x423ab94f)), (f32::from_bits(0x41e68f60), f32::from_bits(0x423ab94f)), (f32::from_bits(0x41e6a5e8), f32::from_bits(0x423aae0b)));
    // 28.798f, 46.681f, 28.82f, 46.681f, 28.831f, 46.67f
    path.cubic_to((f32::from_bits(0x41e6fdf8), f32::from_bits(0x423a136b)), (f32::from_bits(0x41e5dd34), f32::from_bits(0x423978cc)), (f32::from_bits(0x41e68f61), f32::from_bits(0x4238fef0)));
    // 28.874f, 46.519f, 28.733f, 46.368f, 28.82f, 46.249f
    path.cubic_to((f32::from_bits(0x41e72b07), f32::from_bits(0x42389058)), (f32::from_bits(0x41eaf7d4), f32::from_bits(0x42391577)), (f32::from_bits(0x41ec5815), f32::from_bits(0x4238f3ac)));
    // 28.896f, 46.141f, 29.371f, 46.271f, 29.543f, 46.238f
    path.cubic_to((f32::from_bits(0x41ef1cb1), f32::from_bits(0x4238bd66)), (f32::from_bits(0x41ed6252), f32::from_bits(0x4237d4f4)), (f32::from_bits(0x41ede771), f32::from_bits(0x42369eae)));
    // 29.889f, 46.185f, 29.673f, 45.958f, 29.738f, 45.655f
    path.cubic_to((f32::from_bits(0x41ee28fa), f32::from_bits(0x423651e1)), (f32::from_bits(0x41ee8317), f32::from_bits(0x42366868)), (f32::from_bits(0x41eedb27), f32::from_bits(0x42365c1f)));
    // 29.77f, 45.58f, 29.814f, 45.602f, 29.857f, 45.59f
    path.cubic_to((f32::from_bits(0x41ef0629), f32::from_bits(0x4236a9f2)), (f32::from_bits(0x41ef3337), f32::from_bits(0x42371889)), (f32::from_bits(0x41ef3337), f32::from_bits(0x42375b19)));
    // 29.878f, 45.666f, 29.9f, 45.774f, 29.9f, 45.839f
    path.cubic_to((f32::from_bits(0x41ef49be), f32::from_bits(0x4237e038)), (f32::from_bits(0x41ef3337), f32::from_bits(0x42386450)), (f32::from_bits(0x41ef49be), f32::from_bits(0x4238d2e8)));
    // 29.911f, 45.969f, 29.9f, 46.098f, 29.911f, 46.206f
    path.cubic_to((f32::from_bits(0x41ef8b47), f32::from_bits(0x42394cc3)), (f32::from_bits(0x41eff9df), f32::from_bits(0x4239e763)), (f32::from_bits(0x41f026ed), f32::from_bits(0x423a613e)));
    // 29.943f, 46.325f, 29.997f, 46.476f, 30.019f, 46.595f
    path.cubic_to((f32::from_bits(0x41f0ac0c), f32::from_bits(0x423b967d)), (f32::from_bits(0x41f11897), f32::from_bits(0x423ca0bb)), (f32::from_bits(0x41f1893b), f32::from_bits(0x423dd5fa)));
    // 30.084f, 46.897f, 30.137f, 47.157f, 30.192f, 47.459f
    path.cubic_to((f32::from_bits(0x41f19db6), f32::from_bits(0x423e1889)), (f32::from_bits(0x41f1f5c6), f32::from_bits(0x423e7bdd)), (f32::from_bits(0x41f20e5a), f32::from_bits(0x423ebe6d)));
    // 30.202f, 47.524f, 30.245f, 47.621f, 30.257f, 47.686f
    path.cubic_to((f32::from_bits(0x41f27ae5), f32::from_bits(0x423f9059)), (f32::from_bits(0x41f2be7b), f32::from_bits(0x42406d88)), (f32::from_bits(0x41f3168b), f32::from_bits(0x424128ec)));
    // 30.31f, 47.891f, 30.343f, 48.107f, 30.386f, 48.29f
    path.cubic_to((f32::from_bits(0x41f35814), f32::from_bits(0x42418203)), (f32::from_bits(0x41f35814), f32::from_bits(0x4241e556)), (f32::from_bits(0x41f38523), f32::from_bits(0x42423329)));
    // 30.418f, 48.377f, 30.418f, 48.474f, 30.44f, 48.55f
    path.cubic_to((f32::from_bits(0x41f3b025), f32::from_bits(0x424248aa)), (f32::from_bits(0x41f420c9), f32::from_bits(0x424275b8)), (f32::from_bits(0x41f46252), f32::from_bits(0x424280fc)));
    // 30.461f, 48.571f, 30.516f, 48.615f, 30.548f, 48.626f
    path.cubic_to((f32::from_bits(0x41f4fdf8), f32::from_bits(0x4242967d)), (f32::from_bits(0x41f5db27), f32::from_bits(0x424275b8)), (f32::from_bits(0x41f674c1), f32::from_bits(0x424280fc)));
    // 30.624f, 48.647f, 30.732f, 48.615f, 30.807f, 48.626f
    path.cubic_to((f32::from_bits(0x41f8f5c7), f32::from_bits(0x4242967d)), (f32::from_bits(0x41fc5609), f32::from_bits(0x424280fc)), (f32::from_bits(0x41feeb8a), f32::from_bits(0x4242a1c1)));
    // 31.12f, 48.647f, 31.542f, 48.626f, 31.865f, 48.658f
    path.cubic_to((f32::from_bits(0x41ff45a7), f32::from_bits(0x4242a1c1)), (f32::from_bits(0x41ffdf40), f32::from_bits(0x424280fc)), (f32::from_bits(0x4200322f), f32::from_bits(0x4242a1c1)));
    // 31.909f, 48.658f, 31.984f, 48.626f, 32.049f, 48.658f
    path.cubic_to((f32::from_bits(0x420048b6), f32::from_bits(0x4242a1c1)), (f32::from_bits(0x42005e37), f32::from_bits(0x4242c286)), (f32::from_bits(0x420074be), f32::from_bits(0x4242d90d)));
    // 32.071f, 48.658f, 32.092f, 48.69f, 32.114f, 48.712f
    path.cubic_to((f32::from_bits(0x420074be), f32::from_bits(0x4242ee8e)), (f32::from_bits(0x42008002), f32::from_bits(0x42431b9c)), (f32::from_bits(0x420074be), f32::from_bits(0x4243311d)));
    // 32.114f, 48.733f, 32.125f, 48.777f, 32.114f, 48.798f
    path.line_to((f32::from_bits(0x420052f3), f32::from_bits(0x42433c61)));
    // 32.081f, 48.809f
    path.cubic_to((f32::from_bits(0x42001cad), f32::from_bits(0x42439fb5)), (f32::from_bits(0x41ff2f1d), f32::from_bits(0x42436869)), (f32::from_bits(0x41fe7cf0), f32::from_bits(0x4243aaf9)));
    // 32.028f, 48.906f, 31.898f, 48.852f, 31.811f, 48.917f
    path.cubic_to((f32::from_bits(0x41fe24e0), f32::from_bits(0x4243cbbe)), (f32::from_bits(0x41fd3336), f32::from_bits(0x4244cab7)), (f32::from_bits(0x41fd0627), f32::from_bits(0x42450203)));
    // 31.768f, 48.949f, 31.65f, 49.198f, 31.628f, 49.252f
    path.cubic_to((f32::from_bits(0x41fcc291), f32::from_bits(0x4245438c)), (f32::from_bits(0x41fcc291), f32::from_bits(0x42457bde)), (f32::from_bits(0x41fcae17), f32::from_bits(0x4245be6d)));
    // 31.595f, 49.316f, 31.595f, 49.371f, 31.585f, 49.436f
    path.cubic_to((f32::from_bits(0x41fc9790), f32::from_bits(0x4245fff6)), (f32::from_bits(0x41fc28f8), f32::from_bits(0x4246634a)), (f32::from_bits(0x41fc1271), f32::from_bits(0x4246b11d)));
    // 31.574f, 49.5f, 31.52f, 49.597f, 31.509f, 49.673f
    path.cubic_to((f32::from_bits(0x41fbba61), f32::from_bits(0x42478e4c)), (f32::from_bits(0x41fba3d9), f32::from_bits(0x424880fc)), (f32::from_bits(0x41fbba61), f32::from_bits(0x424974b2)));
    // 31.466f, 49.889f, 31.455f, 50.126f, 31.466f, 50.364f
    path.cubic_to((f32::from_bits(0x41fbd0e8), f32::from_bits(0x424a7de9)), (f32::from_bits(0x41fc8109), f32::from_bits(0x424b5b18)), (f32::from_bits(0x41fd47b1), f32::from_bits(0x424c4ecf)));
    // 31.477f, 50.623f, 31.563f, 50.839f, 31.66f, 51.077f
    path.cubic_to((f32::from_bits(0x41fd8b47), f32::from_bits(0x424c915e)), (f32::from_bits(0x41fdccd0), f32::from_bits(0x424cde2b)), (f32::from_bits(0x41fe3b67), f32::from_bits(0x424d167d)));
    // 31.693f, 51.142f, 31.725f, 51.217f, 31.779f, 51.272f
    path.cubic_to((f32::from_bits(0x41fe9377), f32::from_bits(0x424d4dc9)), (f32::from_bits(0x41fec086), f32::from_bits(0x424d8f52)), (f32::from_bits(0x41ff2f1d), f32::from_bits(0x424dc69e)));
    // 31.822f, 51.326f, 31.844f, 51.39f, 31.898f, 51.444f
    path.cubic_to((f32::from_bits(0x41ff70a6), f32::from_bits(0x424df3ac)), (f32::from_bits(0x41ffdf3e), f32::from_bits(0x424e092d)), (f32::from_bits(0x42000626), f32::from_bits(0x424e3536)));
    // 31.93f, 51.488f, 31.984f, 51.509f, 32.006f, 51.552f
    path.cubic_to((f32::from_bits(0x42003d72), f32::from_bits(0x424e6c82)), (f32::from_bits(0x4200c18a), f32::from_bits(0x424f3e6d)), (f32::from_bits(0x4201041a), f32::from_bits(0x424f49b1)));
    // 32.06f, 51.606f, 32.189f, 51.811f, 32.254f, 51.822f
    path.cubic_to((f32::from_bits(0x420172b2), f32::from_bits(0x424f6b7c)), (f32::from_bits(0x4201ec8d), f32::from_bits(0x424e8309)), (f32::from_bits(0x42020d51), f32::from_bits(0x424e4bbd)));
    // 32.362f, 51.855f, 32.481f, 51.628f, 32.513f, 51.574f
    path.cubic_to((f32::from_bits(0x4202be78), f32::from_bits(0x424d5807)), (f32::from_bits(0x42037ae2), f32::from_bits(0x424c6557)), (f32::from_bits(0x42044cce), f32::from_bits(0x424b9265)));
    // 32.686f, 51.336f, 32.87f, 51.099f, 33.075f, 50.893f
    path.cubic_to((f32::from_bits(0x42049aa1), f32::from_bits(0x424b4598)), (f32::from_bits(0x4204e874), f32::from_bits(0x424ae13e)), (f32::from_bits(0x42054084), f32::from_bits(0x424a9471)));
    // 33.151f, 50.818f, 33.227f, 50.72f, 33.313f, 50.645f
    path.cubic_to((f32::from_bits(0x42058d51), f32::from_bits(0x424a51e2)), (f32::from_bits(0x4206ef9f), f32::from_bits(0x4248fad7)), (f32::from_bits(0x42071063), f32::from_bits(0x4248cecf)));
    // 33.388f, 50.58f, 33.734f, 50.245f, 33.766f, 50.202f
    path.cubic_to((f32::from_bits(0x42075e36), f32::from_bits(0x424876bf)), (f32::from_bits(0x4207cccd), f32::from_bits(0x4248342f)), (f32::from_bits(0x42083021), f32::from_bits(0x4247e65c)));
    // 33.842f, 50.116f, 33.95f, 50.051f, 34.047f, 49.975f
    path.cubic_to((f32::from_bits(0x42088831), f32::from_bits(0x42478308)), (f32::from_bits(0x4208f6c9), f32::from_bits(0x4247363b)), (f32::from_bits(0x420970a4), f32::from_bits(0x4246f3ac)));
    // 34.133f, 49.878f, 34.241f, 49.803f, 34.36f, 49.738f
    path.cubic_to((f32::from_bits(0x4209f5c3), f32::from_bits(0x42469a95)), (f32::from_bits(0x420a645a), f32::from_bits(0x42464285)), (f32::from_bits(0x420add2f), f32::from_bits(0x4245f4b2)));
    // 34.49f, 49.651f, 34.598f, 49.565f, 34.716f, 49.489f
    path.cubic_to((f32::from_bits(0x420b2b02), f32::from_bits(0x4245be6c)), (f32::from_bits(0x420bc5a2), f32::from_bits(0x42455a12)), (f32::from_bits(0x420b8418), f32::from_bits(0x4244eb7b)));
    // 34.792f, 49.436f, 34.943f, 49.338f, 34.879f, 49.23f
    path.cubic_to((f32::from_bits(0x420b624d), f32::from_bits(0x4244cab6)), (f32::from_bits(0x420b1fbe), f32::from_bits(0x42449eae)), (f32::from_bits(0x420b0a3d), f32::from_bits(0x42448827)));
    // 34.846f, 49.198f, 34.781f, 49.155f, 34.76f, 49.133f
    path.cubic_to((f32::from_bits(0x420abd70), f32::from_bits(0x424450db)), (f32::from_bits(0x420a9ba5), f32::from_bits(0x42440e4c)), (f32::from_bits(0x420a5916), f32::from_bits(0x4243d700)));
    // 34.685f, 49.079f, 34.652f, 49.014f, 34.587f, 48.96f
    path.cubic_to((f32::from_bits(0x420a3851), f32::from_bits(0x4243b63b)), (f32::from_bits(0x420a21ca), f32::from_bits(0x4243b63b)), (f32::from_bits(0x4209f5c2), f32::from_bits(0x42439fb4)));
    // 34.555f, 48.928f, 34.533f, 48.928f, 34.49f, 48.906f
    path.cubic_to((f32::from_bits(0x4209ea7e), f32::from_bits(0x42439470)), (f32::from_bits(0x4209ea7e), f32::from_bits(0x424373ac)), (f32::from_bits(0x4209d3f7), f32::from_bits(0x42436868)));
    // 34.479f, 48.895f, 34.479f, 48.863f, 34.457f, 48.852f
    path.cubic_to((f32::from_bits(0x4209b332), f32::from_bits(0x424352e7)), (f32::from_bits(0x42099db1), f32::from_bits(0x42435e2b)), (f32::from_bits(0x42097be7), f32::from_bits(0x424352e7)));
    // 34.425f, 48.831f, 34.404f, 48.842f, 34.371f, 48.831f
    path.cubic_to((f32::from_bits(0x420970a3), f32::from_bits(0x42433c60)), (f32::from_bits(0x42096560), f32::from_bits(0x42431b9b)), (f32::from_bits(0x4209449b), f32::from_bits(0x42431b9b)));
    // 34.36f, 48.809f, 34.349f, 48.777f, 34.317f, 48.777f
    path.cubic_to((f32::from_bits(0x4208f6c8), f32::from_bits(0x4242e349)), (f32::from_bits(0x42089eb8), f32::from_bits(0x4242c284)), (f32::from_bits(0x42083020), f32::from_bits(0x4242a1c0)));
    // 34.241f, 48.722f, 34.155f, 48.69f, 34.047f, 48.658f
    path.cubic_to((f32::from_bits(0x42080f5b), f32::from_bits(0x4242967c)), (f32::from_bits(0x4207d810), f32::from_bits(0x42425f31)), (f32::from_bits(0x4207c188), f32::from_bits(0x42425f31)));
    // 34.015f, 48.647f, 33.961f, 48.593f, 33.939f, 48.593f
    path.cubic_to((f32::from_bits(0x420748b3), f32::from_bits(0x424227e5)), (f32::from_bits(0x42066040), f32::from_bits(0x4241fbdd)), (f32::from_bits(0x4205b957), f32::from_bits(0x42421ca2)));
    // 33.821f, 48.539f, 33.594f, 48.496f, 33.431f, 48.528f
    path.cubic_to((f32::from_bits(0x4204c6a7), f32::from_bits(0x42423329)), (f32::from_bits(0x42041580), f32::from_bits(0x4242ad04)), (f32::from_bits(0x42032d0d), f32::from_bits(0x4242c285)));
    // 33.194f, 48.55f, 33.021f, 48.669f, 32.794f, 48.69f
    path.cubic_to((f32::from_bits(0x42032d0d), f32::from_bits(0x4242b848)), (f32::from_bits(0x420322d0), f32::from_bits(0x4242a1c0)), (f32::from_bits(0x42032d0d), f32::from_bits(0x4242a1c0)));
    // 32.794f, 48.68f, 32.784f, 48.658f, 32.794f, 48.658f
    path.cubic_to((f32::from_bits(0x42032d0d), f32::from_bits(0x424280fb)), (f32::from_bits(0x42036459), f32::from_bits(0x424275b8)), (f32::from_bits(0x42036f9c), f32::from_bits(0x42426a74)));
    // 32.794f, 48.626f, 32.848f, 48.615f, 32.859f, 48.604f
    path.cubic_to((f32::from_bits(0x4203e977), f32::from_bits(0x4241cfd4)), (f32::from_bits(0x4204580f), f32::from_bits(0x42418201)), (f32::from_bits(0x420529fa), f32::from_bits(0x42413f72)));
    // 32.978f, 48.453f, 33.086f, 48.377f, 33.291f, 48.312f
    path.line_to((f32::from_bits(0x42054abf), f32::from_bits(0x424128eb)));
    // 33.323f, 48.29f
    path.cubic_to((f32::from_bits(0x4205cfde), f32::from_bits(0x4240fde9)), (f32::from_bits(0x420649b9), f32::from_bits(0x4240fde9)), (f32::from_bits(0x4206b850), f32::from_bits(0x4240b016)));
    // 33.453f, 48.248f, 33.572f, 48.248f, 33.68f, 48.172f
    path.cubic_to((f32::from_bits(0x4206a1c9), f32::from_bits(0x4240998f)), (f32::from_bits(0x4206b850), f32::from_bits(0x42408e4b)), (f32::from_bits(0x4206a1c9), f32::from_bits(0x424078ca)));
    // 33.658f, 48.15f, 33.68f, 48.139f, 33.658f, 48.118f
    path.cubic_to((f32::from_bits(0x42068104), f32::from_bits(0x4240363b)), (f32::from_bits(0x42054081), f32::from_bits(0x423fb11c)), (f32::from_bits(0x4204d1ea), f32::from_bits(0x423f9057)));
    // 33.626f, 48.053f, 33.313f, 47.923f, 33.205f, 47.891f
    path.cubic_to((f32::from_bits(0x42044ccb), f32::from_bits(0x423f79d0)), (f32::from_bits(0x42035915), f32::from_bits(0x423f644f)), (f32::from_bits(0x4202be75), f32::from_bits(0x423f8513)));
    // 33.075f, 47.869f, 32.837f, 47.848f, 32.686f, 47.88f
    path.cubic_to((f32::from_bits(0x42022f19), f32::from_bits(0x423f9b9a)), (f32::from_bits(0x4201c081), f32::from_bits(0x423fde2a)), (f32::from_bits(0x420125e2), f32::from_bits(0x423ff3ab)));
    // 32.546f, 47.902f, 32.438f, 47.967f, 32.287f, 47.988f
    path.line_to((f32::from_bits(0x42010f5b), f32::from_bits(0x423fc7a3)));
    // 32.265f, 47.945f
    path.cubic_to((f32::from_bits(0x4201a9fb), f32::from_bits(0x423f167c)), (f32::from_bits(0x42036459), f32::from_bits(0x423d5c1e)), (f32::from_bits(0x4204580f), f32::from_bits(0x423d198f)));
    // 32.416f, 47.772f, 32.848f, 47.34f, 33.086f, 47.275f
    path.cubic_to((f32::from_bits(0x4205b957), f32::from_bits(0x423cabfe)), (f32::from_bits(0x4207c188), f32::from_bits(0x423cd806)), (f32::from_bits(0x42090d4e), f32::from_bits(0x423d24d3)));
    // 33.431f, 47.168f, 33.939f, 47.211f, 34.263f, 47.286f
    path.cubic_to((f32::from_bits(0x420ae871), f32::from_bits(0x423d936b)), (f32::from_bits(0x420c9892), f32::from_bits(0x423e7bdd)), (f32::from_bits(0x420e6871), f32::from_bits(0x423ed3ee)));
    // 34.727f, 47.394f, 35.149f, 47.621f, 35.602f, 47.707f
    path.cubic_to((f32::from_bits(0x42103956), f32::from_bits(0x423f438c)), (f32::from_bits(0x42121479), f32::from_bits(0x423f0b3a)), (f32::from_bits(0x4213c49a), f32::from_bits(0x423e2e0b)));
    // 36.056f, 47.816f, 36.52f, 47.761f, 36.942f, 47.545f
    path.cubic_to((f32::from_bits(0x4214cdd1), f32::from_bits(0x423db536)), (f32::from_bits(0x4215c081), f32::from_bits(0x423d24d4)), (f32::from_bits(0x42169db1), f32::from_bits(0x423c696f)));
    // 37.201f, 47.427f, 37.438f, 47.286f, 37.654f, 47.103f
    path.cubic_to((f32::from_bits(0x4216eb84), f32::from_bits(0x423c26e0)), (f32::from_bits(0x4217df3a), f32::from_bits(0x423afbde)), (f32::from_bits(0x4218580f), f32::from_bits(0x423b75b9)));
    // 37.73f, 47.038f, 37.968f, 46.746f, 38.086f, 46.865f
    path.cubic_to((f32::from_bits(0x42189a9e), f32::from_bits(0x423bad05)), (f32::from_bits(0x421820c3), f32::from_bits(0x423c1b9c)), (f32::from_bits(0x4217ffff), f32::from_bits(0x423c311d)));
    // 38.151f, 46.919f, 38.032f, 47.027f, 38, 47.048f
    path.cubic_to((f32::from_bits(0x4217a6e8), f32::from_bits(0x423c9577)), (f32::from_bits(0x42173851), f32::from_bits(0x423ced87)), (f32::from_bits(0x4216cac0), f32::from_bits(0x423d5c1f)));
    // 37.913f, 47.146f, 37.805f, 47.232f, 37.698f, 47.34f
    path.cubic_to((f32::from_bits(0x42168831), f32::from_bits(0x423d9eae)), (f32::from_bits(0x421650e5), f32::from_bits(0x423deb7b)), (f32::from_bits(0x4215f7ce), f32::from_bits(0x423e23cd)));
    // 37.633f, 47.405f, 37.579f, 47.48f, 37.492f, 47.535f
    path.line_to((f32::from_bits(0x4215f7ce), f32::from_bits(0x423e4492)));
    // 37.492f, 47.567f
    path.cubic_to((f32::from_bits(0x4215ed91), f32::from_bits(0x423e4fd6)), (f32::from_bits(0x4215d709), f32::from_bits(0x423e4492)), (f32::from_bits(0x4215cbc6), f32::from_bits(0x423e4fd6)));
    // 37.482f, 47.578f, 37.46f, 47.567f, 37.449f, 47.578f
    path.cubic_to((f32::from_bits(0x42158937), f32::from_bits(0x423e8722)), (f32::from_bits(0x42153126), f32::from_bits(0x423f00fd)), (f32::from_bits(0x4214ee97), f32::from_bits(0x423f3849)));
    // 37.384f, 47.632f, 37.298f, 47.751f, 37.233f, 47.805f
    path.cubic_to((f32::from_bits(0x4214d810), f32::from_bits(0x423f438d)), (f32::from_bits(0x4214cdd2), f32::from_bits(0x423f590e)), (f32::from_bits(0x4214cdd2), f32::from_bits(0x423f590e)));
    // 37.211f, 47.816f, 37.201f, 47.837f, 37.201f, 47.837f
    path.line_to((f32::from_bits(0x4214b74b), f32::from_bits(0x423f590e)));
    // 37.179f, 47.837f
    path.line_to((f32::from_bits(0x4214b74b), f32::from_bits(0x423f79d3)));
    // 37.179f, 47.869f
    path.cubic_to((f32::from_bits(0x42147fff), f32::from_bits(0x423f905a)), (f32::from_bits(0x421474bc), f32::from_bits(0x423fb11f)), (f32::from_bits(0x421448b3), f32::from_bits(0x423fc7a6)));
    // 37.125f, 47.891f, 37.114f, 47.923f, 37.071f, 47.945f
    path.line_to((f32::from_bits(0x421448b3), f32::from_bits(0x423fdd27)));
    // 37.071f, 47.966f
    path.line_to((f32::from_bits(0x42143332), f32::from_bits(0x423fdd27)));
    // 37.05f, 47.966f
    path.line_to((f32::from_bits(0x4213b957), f32::from_bits(0x424077c7)));
    // 36.931f, 48.117f
    path.cubic_to((f32::from_bits(0x4213a2d0), f32::from_bits(0x4240830b)), (f32::from_bits(0x4213b957), f32::from_bits(0x4240988c)), (f32::from_bits(0x4213b957), f32::from_bits(0x4240988c)));
    // 36.909f, 48.128f, 36.931f, 48.149f, 36.931f, 48.149f
    path.cubic_to((f32::from_bits(0x4213c49b), f32::from_bits(0x4240988c)), (f32::from_bits(0x4213b957), f32::from_bits(0x4240ba57)), (f32::from_bits(0x4213da1c), f32::from_bits(0x4240af13)));
    // 36.942f, 48.149f, 36.931f, 48.182f, 36.963f, 48.171f
    path.cubic_to((f32::from_bits(0x42141cab), f32::from_bits(0x4240af13)), (f32::from_bits(0x4214a1ca), f32::from_bits(0x42405703)), (f32::from_bits(0x4214ee97), f32::from_bits(0x42403538)));
    // 37.028f, 48.171f, 37.158f, 48.085f, 37.233f, 48.052f
    path.cubic_to((f32::from_bits(0x42153126), f32::from_bits(0x42401473)), (f32::from_bits(0x42157ef9), f32::from_bits(0x423ffdec)), (f32::from_bits(0x4215cbc6), f32::from_bits(0x423fd1e4)));
    // 37.298f, 48.02f, 37.374f, 47.998f, 37.449f, 47.955f
    path.cubic_to((f32::from_bits(0x421650e5), f32::from_bits(0x423f8f55)), (f32::from_bits(0x4216cac0), f32::from_bits(0x423f4288)), (f32::from_bits(0x42178624), f32::from_bits(0x423f20bd)));
    // 37.579f, 47.89f, 37.698f, 47.815f, 37.881f, 47.782f
    path.cubic_to((f32::from_bits(0x42177ae0), f32::from_bits(0x423f8f55)), (f32::from_bits(0x421770a3), f32::from_bits(0x423fc6a0)), (f32::from_bits(0x42174395), f32::from_bits(0x423ffdec)));
    // 37.87f, 47.89f, 37.86f, 47.944f, 37.816f, 47.998f
    path.cubic_to((f32::from_bits(0x4216bf7c), f32::from_bits(0x4240ba56)), (f32::from_bits(0x4215ab02), f32::from_bits(0x4241332b)), (f32::from_bits(0x4214f9db), f32::from_bits(0x4241c38e)));
    // 37.687f, 48.182f, 37.417f, 48.3f, 37.244f, 48.441f
    path.cubic_to((f32::from_bits(0x42143333), f32::from_bits(0x424274b5)), (f32::from_bits(0x42136b85), f32::from_bits(0x42433019)), (f32::from_bits(0x4212c5a2), f32::from_bits(0x4243f7c7)));
    // 37.05f, 48.614f, 36.855f, 48.797f, 36.693f, 48.992f
    path.cubic_to((f32::from_bits(0x42115917), f32::from_bits(0x42459b9e)), (f32::from_bits(0x421022d1), f32::from_bits(0x42476c83)), (f32::from_bits(0x420f0313), f32::from_bits(0x4249311f)));
    // 36.337f, 49.402f, 36.034f, 49.856f, 35.753f, 50.298f
    path.cubic_to((f32::from_bits(0x420e1ba6), f32::from_bits(0x424a936d)), (f32::from_bits(0x420d75c3), f32::from_bits(0x424c21c3)), (f32::from_bits(0x420cdb23), f32::from_bits(0x424dba56)));
    // 35.527f, 50.644f, 35.365f, 51.033f, 35.214f, 51.432f
    path.cubic_to((f32::from_bits(0x420c3f7d), f32::from_bits(0x424f6a77)), (f32::from_bits(0x420b8419), f32::from_bits(0x42510e4e)), (f32::from_bits(0x420b1fbf), f32::from_bits(0x4252d3f0)));
    // 35.062f, 51.854f, 34.879f, 52.264f, 34.781f, 52.707f
    path.cubic_to((f32::from_bits(0x420ad2f2), f32::from_bits(0x42548e4e)), (f32::from_bits(0x420ab127), f32::from_bits(0x42565e2d)), (f32::from_bits(0x420a9063), f32::from_bits(0x4258188c)));
    // 34.706f, 53.139f, 34.673f, 53.592f, 34.641f, 54.024f
    path.cubic_to((f32::from_bits(0x420a7ae2), f32::from_bits(0x4258882a)), (f32::from_bits(0x420a9ba7), f32::from_bits(0x4258e03a)), (f32::from_bits(0x420a9ba7), f32::from_bits(0x42594ed2)));
    // 34.62f, 54.133f, 34.652f, 54.219f, 34.652f, 54.327f
    path.cubic_to((f32::from_bits(0x420aa6eb), f32::from_bits(0x425e301a)), (f32::from_bits(0x420c820d), f32::from_bits(0x4262c495)), (f32::from_bits(0x420ecbc8), f32::from_bits(0x4266fff9)));
    // 34.663f, 55.547f, 35.127f, 56.692f, 35.699f, 57.75f
    path.cubic_to((f32::from_bits(0x420eed93), f32::from_bits(0x426721c4)), (f32::from_bits(0x420f0e57), f32::from_bits(0x42674dcc)), (f32::from_bits(0x420f3022), f32::from_bits(0x42676e91)));
    // 35.732f, 57.783f, 35.764f, 57.826f, 35.797f, 57.858f
    path.cubic_to((f32::from_bits(0x420f7df5), f32::from_bits(0x42680a37)), (f32::from_bits(0x420fbf7e), f32::from_bits(0x42689993)), (f32::from_bits(0x42100d51), f32::from_bits(0x42693433)));
    // 35.873f, 58.01f, 35.937f, 58.15f, 36.013f, 58.301f
    path.cubic_to((f32::from_bits(0x42102e16), f32::from_bits(0x426955fe)), (f32::from_bits(0x42105a1e), f32::from_bits(0x426976c2)), (f32::from_bits(0x42106561), f32::from_bits(0x42698d4a)));
    // 36.045f, 58.334f, 36.088f, 58.366f, 36.099f, 58.388f
    path.cubic_to((f32::from_bits(0x4210872c), f32::from_bits(0x4269e55a)), (f32::from_bits(0x4210a7f0), f32::from_bits(0x426a3d6b)), (f32::from_bits(0x4210ea80), f32::from_bits(0x426a6a79)));
    // 36.132f, 58.474f, 36.164f, 58.56f, 36.229f, 58.604f
    path.cubic_to((f32::from_bits(0x42119aa1), f32::from_bits(0x426acdcd)), (f32::from_bits(0x42131376), f32::from_bits(0x426a48ae)), (f32::from_bits(0x4213e561), f32::from_bits(0x426a6a79)));
    // 36.401f, 58.701f, 36.769f, 58.571f, 36.974f, 58.604f
    path.cubic_to((f32::from_bits(0x4213fae2), f32::from_bits(0x426a75bd)), (f32::from_bits(0x42141cad), f32::from_bits(0x426a8b3e)), (f32::from_bits(0x42143d71), f32::from_bits(0x426a8b3e)));
    // 36.995f, 58.615f, 37.028f, 58.636f, 37.06f, 58.636f
    path.cubic_to((f32::from_bits(0x42141cac), f32::from_bits(0x426acdcd)), (f32::from_bits(0x42143334), f32::from_bits(0x426aee92)), (f32::from_bits(0x42141cac), f32::from_bits(0x426b25de)));
    // 37.028f, 58.701f, 37.05f, 58.733f, 37.028f, 58.787f
    path.cubic_to((f32::from_bits(0x4213e560), f32::from_bits(0x426b9fb9)), (f32::from_bits(0x4212dc29), f32::from_bits(0x426d0d4b)), (f32::from_bits(0x4212f1aa), f32::from_bits(0x426da7ea)));
    // 36.974f, 58.906f, 36.715f, 59.263f, 36.736f, 59.414f
    path.cubic_to((f32::from_bits(0x4212f1aa), f32::from_bits(0x426dfffa)), (f32::from_bits(0x4213b958), f32::from_bits(0x426ed1e6)), (f32::from_bits(0x4213c49c), f32::from_bits(0x426edd29)));
    // 36.736f, 59.5f, 36.931f, 59.705f, 36.942f, 59.716f
    path.cubic_to((f32::from_bits(0x4213e561), f32::from_bits(0x426f1fb8)), (f32::from_bits(0x42143d71), f32::from_bits(0x426f9993)), (f32::from_bits(0x421448b5), f32::from_bits(0x426ffce7)));
    // 36.974f, 59.781f, 37.06f, 59.9f, 37.071f, 59.997f
    path.cubic_to((f32::from_bits(0x421448b5), f32::from_bits(0x427076c2)), (f32::from_bits(0x4214072c), f32::from_bits(0x4270ef97)), (f32::from_bits(0x4213fae2), f32::from_bits(0x427148ae)));
    // 37.071f, 60.116f, 37.007f, 60.234f, 36.995f, 60.321f
    path.cubic_to((f32::from_bits(0x4213e561), f32::from_bits(0x42717ffa)), (f32::from_bits(0x4213fae2), f32::from_bits(0x42718b3d)), (f32::from_bits(0x4213e561), f32::from_bits(0x4271b746)));
    // 36.974f, 60.375f, 36.995f, 60.386f, 36.974f, 60.429f
    path.cubic_to((f32::from_bits(0x4213da1d), f32::from_bits(0x4271ccc7)), (f32::from_bits(0x4213b959), f32::from_bits(0x42721a9a)), (f32::from_bits(0x4213a2d2), f32::from_bits(0x42721a9a)));
    // 36.963f, 60.45f, 36.931f, 60.526f, 36.909f, 60.526f
    path.cubic_to((f32::from_bits(0x42134ac2), f32::from_bits(0x42723c65)), (f32::from_bits(0x4212d0e6), f32::from_bits(0x427225de)), (f32::from_bits(0x42126d93), f32::from_bits(0x427225de)));
    // 36.823f, 60.559f, 36.704f, 60.537f, 36.607f, 60.537f
    path.cubic_to((f32::from_bits(0x42124bc8), f32::from_bits(0x427225de)), (f32::from_bits(0x4211bc6c), f32::from_bits(0x42723c65)), (f32::from_bits(0x42119064), f32::from_bits(0x42723c65)));
    // 36.574f, 60.537f, 36.434f, 60.559f, 36.391f, 60.559f
    path.cubic_to((f32::from_bits(0x4210d3fa), f32::from_bits(0x427246a2)), (f32::from_bits(0x420ff6ca), f32::from_bits(0x4272301b)), (f32::from_bits(0x420f676e), f32::from_bits(0x4272686d)));
    // 36.207f, 60.569f, 35.991f, 60.547f, 35.851f, 60.602f
    path.cubic_to((f32::from_bits(0x420eb647), f32::from_bits(0x4272b53a)), (f32::from_bits(0x420e52f3), f32::from_bits(0x42737ce8)), (f32::from_bits(0x420dc291), f32::from_bits(0x4273f5bd)));
    // 35.678f, 60.677f, 35.581f, 60.872f, 35.44f, 60.99f
    path.cubic_to((f32::from_bits(0x420d116a), f32::from_bits(0x4274861f)), (f32::from_bits(0x420c5606), f32::from_bits(0x4274e973)), (f32::from_bits(0x420b999b), f32::from_bits(0x4275580b)));
    // 35.267f, 61.131f, 35.084f, 61.228f, 34.9f, 61.336f
    path.cubic_to((f32::from_bits(0x420a9ba7), f32::from_bits(0x4275fdee)), (f32::from_bits(0x4209b335), f32::from_bits(0x42768d4a)), (f32::from_bits(0x42089eba), f32::from_bits(0x4276f1a5)));
    // 34.652f, 61.498f, 34.425f, 61.638f, 34.155f, 61.736f
    path.cubic_to((f32::from_bits(0x4207ab04), f32::from_bits(0x42773e72)), (f32::from_bits(0x4206a1cc), f32::from_bits(0x42778101)), (f32::from_bits(0x4205b95a), f32::from_bits(0x4277c391)));
    // 33.917f, 61.811f, 33.658f, 61.876f, 33.431f, 61.941f
    path.cubic_to((f32::from_bits(0x4203bd73), f32::from_bits(0x42786974)), (f32::from_bits(0x4201cbc9), f32::from_bits(0x42793b60)), (f32::from_bits(0x4200ac0a), f32::from_bits(0x427af5be)));
    // 32.935f, 62.103f, 32.449f, 62.308f, 32.168f, 62.74f
    path.cubic_to((f32::from_bits(0x420074be), f32::from_bits(0x427b428b)), (f32::from_bits(0x41ffb43d), f32::from_bits(0x427c4cc8)), (f32::from_bits(0x41ff872f), f32::from_bits(0x427ca4d9)));
    // 32.114f, 62.815f, 31.963f, 63.075f, 31.941f, 63.161f
    path.cubic_to((f32::from_bits(0x41ff872f), f32::from_bits(0x427cbb60)), (f32::from_bits(0x41ff9db6), f32::from_bits(0x427cd0e1)), (f32::from_bits(0x41ff872f), f32::from_bits(0x427ce768)));
    // 31.941f, 63.183f, 31.952f, 63.204f, 31.941f, 63.226f
    path.cubic_to((f32::from_bits(0x41ffb43d), f32::from_bits(0x427cfce9)), (f32::from_bits(0x41ffb43d), f32::from_bits(0x427cfce9)), (f32::from_bits(0x41ffdf3f), f32::from_bits(0x427d1370)));
    // 31.963f, 63.247f, 31.963f, 63.247f, 31.984f, 63.269f
    path.cubic_to((f32::from_bits(0x4200ac0a), f32::from_bits(0x427cfce9)), (f32::from_bits(0x42010f5e), f32::from_bits(0x427cd0e1)), (f32::from_bits(0x4201a9fe), f32::from_bits(0x427ca4d8)));
    // 32.168f, 63.247f, 32.265f, 63.204f, 32.416f, 63.161f
    path.cubic_to((f32::from_bits(0x4201c085), f32::from_bits(0x427c9994)), (f32::from_bits(0x4201f7d1), f32::from_bits(0x427c78d0)), (f32::from_bits(0x42020315), f32::from_bits(0x427c78d0)));
    // 32.438f, 63.15f, 32.492f, 63.118f, 32.503f, 63.118f
    path.cubic_to((f32::from_bits(0x420223da), f32::from_bits(0x427c6249)), (f32::from_bits(0x42022f1d), f32::from_bits(0x427c78d0)), (f32::from_bits(0x42023a61), f32::from_bits(0x427c78d0)));
    // 32.535f, 63.096f, 32.546f, 63.118f, 32.557f, 63.118f
    path.cubic_to((f32::from_bits(0x42025b26), f32::from_bits(0x427c6249)), (f32::from_bits(0x42028834), f32::from_bits(0x427c4184)), (f32::from_bits(0x4202a8f9), f32::from_bits(0x427c4184)));
    // 32.589f, 63.096f, 32.633f, 63.064f, 32.665f, 63.064f
    path.cubic_to((f32::from_bits(0x4203e97c), f32::from_bits(0x427bc7a9)), (f32::from_bits(0x42061db5), f32::from_bits(0x427ba6e4)), (f32::from_bits(0x4207b649), f32::from_bits(0x427bfef5)));
    // 32.978f, 62.945f, 33.529f, 62.913f, 33.928f, 62.999f
    path.cubic_to((f32::from_bits(0x42089ebc), f32::from_bits(0x427c20c0)), (f32::from_bits(0x420970a7), f32::from_bits(0x427c78d0)), (f32::from_bits(0x420a21ce), f32::from_bits(0x427cc59d)));
    // 34.155f, 63.032f, 34.36f, 63.118f, 34.533f, 63.193f
    path.cubic_to((f32::from_bits(0x420a6fa1), f32::from_bits(0x427cdc24)), (f32::from_bits(0x420ab12a), f32::from_bits(0x427ce768)), (f32::from_bits(0x420af3ba), f32::from_bits(0x427d1370)));
    // 34.609f, 63.215f, 34.673f, 63.226f, 34.738f, 63.269f
    path.cubic_to((f32::from_bits(0x420b0a41), f32::from_bits(0x427d1370)), (f32::from_bits(0x420af3ba), f32::from_bits(0x427d353b)), (f32::from_bits(0x420b1585), f32::from_bits(0x427d407e)));
    // 34.76f, 63.269f, 34.738f, 63.302f, 34.771f, 63.313f
    path.cubic_to((f32::from_bits(0x420b0a41), f32::from_bits(0x427d6143)), (f32::from_bits(0x420b0a41), f32::from_bits(0x427d8207)), (f32::from_bits(0x420af3ba), f32::from_bits(0x427dae0f)));
    // 34.76f, 63.345f, 34.76f, 63.377f, 34.738f, 63.42f
    path.cubic_to((f32::from_bits(0x420ad2f5), f32::from_bits(0x427df09e)), (f32::from_bits(0x420a2d12), f32::from_bits(0x427e54f8)), (f32::from_bits(0x420a4293), f32::from_bits(0x427ee455)));
    // 34.706f, 63.485f, 34.544f, 63.583f, 34.565f, 63.723f
    path.cubic_to((f32::from_bits(0x420a591a), f32::from_bits(0x427f051a)), (f32::from_bits(0x420ad2f5), f32::from_bits(0x427f3122)), (f32::from_bits(0x420af3ba), f32::from_bits(0x427f47a9)));
    // 34.587f, 63.755f, 34.706f, 63.798f, 34.738f, 63.82f
    path.cubic_to((f32::from_bits(0x420af3ba), f32::from_bits(0x427f5d2a)), (f32::from_bits(0x420af3ba), f32::from_bits(0x427f73b1)), (f32::from_bits(0x420b0a41), f32::from_bits(0x427f7ef5)));
    // 34.738f, 63.841f, 34.738f, 63.863f, 34.76f, 63.874f
    path.cubic_to((f32::from_bits(0x420add33), f32::from_bits(0x427fccc8)), (f32::from_bits(0x420a21ce), f32::from_bits(0x42803e74)), (f32::from_bits(0x420a2d12), f32::from_bits(0x4280701e)));
    // 34.716f, 63.95f, 34.533f, 64.122f, 34.544f, 64.219f
    path.cubic_to((f32::from_bits(0x420a3856), f32::from_bits(0x42808bc4)), (f32::from_bits(0x420ad2f5), f32::from_bits(0x4280a7ed)), (f32::from_bits(0x420ae876), f32::from_bits(0x4280b2ad)));
    // 34.555f, 64.273f, 34.706f, 64.328f, 34.727f, 64.349f
    path.cubic_to((f32::from_bits(0x420af3ba), f32::from_bits(0x4280bdf1)), (f32::from_bits(0x420add32), f32::from_bits(0x4280c8b1)), (f32::from_bits(0x420af3ba), f32::from_bits(0x4280d3f5)));
    // 34.738f, 64.371f, 34.716f, 64.392f, 34.738f, 64.414f
    path.cubic_to((f32::from_bits(0x420abd74), f32::from_bits(0x4280f53d)), (f32::from_bits(0x4209f5c6), f32::from_bits(0x4281428d)), (f32::from_bits(0x420a21ce), f32::from_bits(0x42816e95)));
    // 34.685f, 64.479f, 34.49f, 64.63f, 34.533f, 64.716f
    path.cubic_to((f32::from_bits(0x420a4293), f32::from_bits(0x4281957e)), (f32::from_bits(0x420ad2f5), f32::from_bits(0x4281a664)), (f32::from_bits(0x420ae876), f32::from_bits(0x4281c187)));
    // 34.565f, 64.792f, 34.706f, 64.825f, 34.727f, 64.878f
    path.cubic_to((f32::from_bits(0x420ae876), f32::from_bits(0x4281c729)), (f32::from_bits(0x420add32), f32::from_bits(0x4281d26c)), (f32::from_bits(0x420ae876), f32::from_bits(0x4281d26c)));
    // 34.727f, 64.889f, 34.716f, 64.911f, 34.727f, 64.911f
    path.cubic_to((f32::from_bits(0x420aa6ed), f32::from_bits(0x4281fe74)), (f32::from_bits(0x420a591a), f32::from_bits(0x42821a1a)), (f32::from_bits(0x4209f5c6), f32::from_bits(0x42823b62)));
    // 34.663f, 64.997f, 34.587f, 65.051f, 34.49f, 65.116f
    path.cubic_to((f32::from_bits(0x420a168b), f32::from_bits(0x42825caa)), (f32::from_bits(0x420a010a), f32::from_bits(0x4282624b)), (f32::from_bits(0x420a2d12), f32::from_bits(0x42827850)));
    // 34.522f, 65.181f, 34.501f, 65.192f, 34.544f, 65.235f
    path.cubic_to((f32::from_bits(0x420a645e), f32::from_bits(0x428288b2)), (f32::from_bits(0x420a9baa), f32::from_bits(0x428293f6)), (f32::from_bits(0x420ad2f5), f32::from_bits(0x4282a458)));
    // 34.598f, 65.267f, 34.652f, 65.289f, 34.706f, 65.321f
    path.cubic_to((f32::from_bits(0x420ad2f5), f32::from_bits(0x4282a458)), (f32::from_bits(0x420add32), f32::from_bits(0x4282d685)), (f32::from_bits(0x420abd74), f32::from_bits(0x4282c5a0)));
    // 34.706f, 65.321f, 34.716f, 65.419f, 34.685f, 65.386f
    path.cubic_to((f32::from_bits(0x420aa6ed), f32::from_bits(0x4282cb42)), (f32::from_bits(0x420a9066), f32::from_bits(0x4282e146)), (f32::from_bits(0x420a6fa1), f32::from_bits(0x4282e6e8)));
    // 34.663f, 65.397f, 34.641f, 65.44f, 34.609f, 65.451f
    path.cubic_to((f32::from_bits(0x4209df3f), f32::from_bits(0x42830830)), (f32::from_bits(0x4208f6cc), f32::from_bits(0x4282bae0)), (f32::from_bits(0x42088834), f32::from_bits(0x4282a459)));
    // 34.468f, 65.516f, 34.241f, 65.365f, 34.133f, 65.321f
    path.cubic_to((f32::from_bits(0x420846ab), f32::from_bits(0x42829915)), (f32::from_bits(0x42080f5f), f32::from_bits(0x42829915)), (f32::from_bits(0x4207c18c), f32::from_bits(0x428293f7)));
    // 34.069f, 65.299f, 34.015f, 65.299f, 33.939f, 65.289f
    path.cubic_to((f32::from_bits(0x42079584), f32::from_bits(0x428288b3)), (f32::from_bits(0x420748b7), f32::from_bits(0x42826d0e)), (f32::from_bits(0x42071ba9), f32::from_bits(0x42826d0e)));
    // 33.896f, 65.267f, 33.821f, 65.213f, 33.777f, 65.213f
    path.cubic_to((f32::from_bits(0x4206cedc), f32::from_bits(0x4282624d)), (f32::from_bits(0x42068109), f32::from_bits(0x4282624d)), (f32::from_bits(0x42061272), f32::from_bits(0x4282624d)));
    // 33.702f, 65.192f, 33.626f, 65.192f, 33.518f, 65.192f
    path.cubic_to((f32::from_bits(0x4205cfe3), f32::from_bits(0x42825cab)), (f32::from_bits(0x4205614b), f32::from_bits(0x42824bc6)), (f32::from_bits(0x42051ebc), f32::from_bits(0x42824bc6)));
    // 33.453f, 65.181f, 33.345f, 65.148f, 33.28f, 65.148f
    path.cubic_to((f32::from_bits(0x42037ae5), f32::from_bits(0x428246a7)), (f32::from_bits(0x4201cbca), f32::from_bits(0x42829eb8)), (f32::from_bits(0x4200ac0c), f32::from_bits(0x4282e147)));
    // 32.87f, 65.138f, 32.449f, 65.31f, 32.168f, 65.44f
    path.cubic_to((f32::from_bits(0x42008b47), f32::from_bits(0x4282e6e9)), (f32::from_bits(0x42005e39), f32::from_bits(0x4282fced)), (f32::from_bits(0x42003d74), f32::from_bits(0x4283028f)));
    // 32.136f, 65.451f, 32.092f, 65.494f, 32.06f, 65.505f
    path.cubic_to((f32::from_bits(0x41fdf9e2), f32::from_bits(0x42833f7d)), (f32::from_bits(0x41fa4190), f32::from_bits(0x42836041)), (f32::from_bits(0x41f674c3), f32::from_bits(0x42834fdf)));
    // 31.747f, 65.624f, 31.282f, 65.688f, 30.807f, 65.656f
    path.cubic_to((f32::from_bits(0x41f59794), f32::from_bits(0x4283451e)), (f32::from_bits(0x41f48d56), f32::from_bits(0x4283451e)), (f32::from_bits(0x41f3b027), f32::from_bits(0x428339db)));
    // 30.699f, 65.635f, 30.569f, 65.635f, 30.461f, 65.613f
    path.cubic_to((f32::from_bits(0x41f32d15), f32::from_bits(0x42832e97)), (f32::from_bits(0x41f2666d), f32::from_bits(0x428312f2)), (f32::from_bits(0x41f1b440), f32::from_bits(0x42830831)));
    // 30.397f, 65.591f, 30.3f, 65.537f, 30.213f, 65.516f
    path.cubic_to((f32::from_bits(0x41f1041f), f32::from_bits(0x4282fced)), (f32::from_bits(0x41f07f01), f32::from_bits(0x4282f74c)), (f32::from_bits(0x41efb859), f32::from_bits(0x4282e6e9)));
    // 30.127f, 65.494f, 30.062f, 65.483f, 29.965f, 65.451f
    path.cubic_to((f32::from_bits(0x41efa1d2), f32::from_bits(0x4282e147)), (f32::from_bits(0x41ef6049), f32::from_bits(0x4282d687)), (f32::from_bits(0x41ef49c1), f32::from_bits(0x4282d687)));
    // 29.954f, 65.44f, 29.922f, 65.419f, 29.911f, 65.419f
    path.cubic_to((f32::from_bits(0x41ef062b), f32::from_bits(0x4282cb43)), (f32::from_bits(0x41eec4a2), f32::from_bits(0x4282cb43)), (f32::from_bits(0x41ee560b), f32::from_bits(0x4282c5a2)));
    // 29.878f, 65.397f, 29.846f, 65.397f, 29.792f, 65.386f
    path.cubic_to((f32::from_bits(0x41ee1275), f32::from_bits(0x4282c000)), (f32::from_bits(0x41ed8f63), f32::from_bits(0x4282a45a)), (f32::from_bits(0x41ed3546), f32::from_bits(0x42829eb9)));
    // 29.759f, 65.375f, 29.695f, 65.321f, 29.651f, 65.31f
    path.cubic_to((f32::from_bits(0x41ebbe7d), f32::from_bits(0x42827d71)), (f32::from_bits(0x41ea72b7), f32::from_bits(0x42825cad)), (f32::from_bits(0x41e91069), f32::from_bits(0x42823b65)));
    // 29.468f, 65.245f, 29.306f, 65.181f, 29.133f, 65.116f
    path.cubic_to((f32::from_bits(0x41e6fdfa), f32::from_bits(0x42820419)), (f32::from_bits(0x41e4a7f6), f32::from_bits(0x4281ab86)), (f32::from_bits(0x41e18b4a), f32::from_bits(0x4281ab86)));
    // 28.874f, 65.008f, 28.582f, 64.835f, 28.193f, 64.835f
    path.cubic_to((f32::from_bits(0x41de9bac), f32::from_bits(0x4281b128)), (f32::from_bits(0x41dcf7d5), f32::from_bits(0x4281fe78)), (f32::from_bits(0x41db3d77), f32::from_bits(0x428246a9)));
    // 27.826f, 64.846f, 27.621f, 64.997f, 27.405f, 65.138f
    path.cubic_to((f32::from_bits(0x41dacedf), f32::from_bits(0x4282570b)), (f32::from_bits(0x41da76cf), f32::from_bits(0x4282570b)), (f32::from_bits(0x41da0838), f32::from_bits(0x4282676e)));
    // 27.351f, 65.17f, 27.308f, 65.17f, 27.254f, 65.202f
    path.cubic_to((f32::from_bits(0x41d9f1b1), f32::from_bits(0x4282676e)), (f32::from_bits(0x41d9f1b1), f32::from_bits(0x42827853)), (f32::from_bits(0x41d9db2a), f32::from_bits(0x42827d72)));
    // 27.243f, 65.202f, 27.243f, 65.235f, 27.232f, 65.245f
    path.cubic_to((f32::from_bits(0x41d96c92), f32::from_bits(0x428288b6)), (f32::from_bits(0x41d91482), f32::from_bits(0x428288b6)), (f32::from_bits(0x41d8a5eb), f32::from_bits(0x42829eba)));
    // 27.178f, 65.267f, 27.135f, 65.267f, 27.081f, 65.31f
    path.line_to((f32::from_bits(0x41d88f64), f32::from_bits(0x4282a9fe)));
    // 27.07f, 65.332f
    path.cubic_to((f32::from_bits(0x41d6eb8d), f32::from_bits(0x4282e14a)), (f32::from_bits(0x41d4ac10), f32::from_bits(0x42830291)), (f32::from_bits(0x41d25818), f32::from_bits(0x428312f4)));
    // 26.865f, 65.44f, 26.584f, 65.505f, 26.293f, 65.537f
    path.cubic_to((f32::from_bits(0x41d0b235), f32::from_bits(0x42831896)), (f32::from_bits(0x41ce74c4), f32::from_bits(0x428312f4)), (f32::from_bits(0x41cce568), f32::from_bits(0x42830292)));
    // 26.087f, 65.548f, 25.807f, 65.537f, 25.612f, 65.505f
    path.cubic_to((f32::from_bits(0x41cca3df), f32::from_bits(0x4282fcf0)), (f32::from_bits(0x41cc1ec0), f32::from_bits(0x4282f1ad)), (f32::from_bits(0x41cbf3be), f32::from_bits(0x4282f1ad)));
    // 25.58f, 65.494f, 25.515f, 65.472f, 25.494f, 65.472f
    path.cubic_to((f32::from_bits(0x41ca9170), f32::from_bits(0x4282dba9)), (f32::from_bits(0x41c99dba), f32::from_bits(0x4282e14b)), (f32::from_bits(0x41c8687a), f32::from_bits(0x4282cb47)));
    // 25.321f, 65.429f, 25.202f, 65.44f, 25.051f, 65.397f
    path.cubic_to((f32::from_bits(0x41c7b64d), f32::from_bits(0x4282c003)), (f32::from_bits(0x41c71cb3), f32::from_bits(0x4282bae5)), (f32::from_bits(0x41c6560b), f32::from_bits(0x4282a9ff)));
    // 24.964f, 65.375f, 24.889f, 65.365f, 24.792f, 65.332f
    path.line_to((f32::from_bits(0x41c628fd), f32::from_bits(0x42829ebb)));
    // 24.77f, 65.31f
    path.cubic_to((f32::from_bits(0x41c58d57), f32::from_bits(0x428293fa)), (f32::from_bits(0x41c53547), f32::from_bits(0x42829919)), (f32::from_bits(0x41c4b028), f32::from_bits(0x428293fa)));
    // 24.694f, 65.289f, 24.651f, 65.299f, 24.586f, 65.289f
    path.line_to((f32::from_bits(0x41c46e9f), f32::from_bits(0x42828315)));
    // 24.554f, 65.256f
    path.cubic_to((f32::from_bits(0x41c1d712), f32::from_bits(0x4282570d)), (f32::from_bits(0x41be20cc), f32::from_bits(0x428209bd)), (f32::from_bits(0x41bb0420), f32::from_bits(0x42820f5f)));
    // 24.23f, 65.17f, 23.766f, 65.019f, 23.377f, 65.03f
    path.cubic_to((f32::from_bits(0x41b9a1d2), f32::from_bits(0x42820f5f)), (f32::from_bits(0x41b7e774), f32::from_bits(0x42823024)), (f32::from_bits(0x41b6dd37), f32::from_bits(0x428246ab)));
    // 23.204f, 65.03f, 22.988f, 65.094f, 22.858f, 65.138f
    path.cubic_to((f32::from_bits(0x41b5eb8d), f32::from_bits(0x4282570d)), (f32::from_bits(0x41b54fe7), f32::from_bits(0x4282570d)), (f32::from_bits(0x41b45c31), f32::from_bits(0x42826770)));
    // 22.74f, 65.17f, 22.664f, 65.17f, 22.545f, 65.202f
    path.cubic_to((f32::from_bits(0x41b3ed99), f32::from_bits(0x42826d12)), (f32::from_bits(0x41b35400), f32::from_bits(0x428288b8)), (f32::from_bits(0x41b2fbef), f32::from_bits(0x428293fb)));
    // 22.491f, 65.213f, 22.416f, 65.267f, 22.373f, 65.289f
    path.cubic_to((f32::from_bits(0x41b274c4), f32::from_bits(0x4282991a)), (f32::from_bits(0x41b249c2), f32::from_bits(0x428293fb)), (f32::from_bits(0x41b1c4a3), f32::from_bits(0x42829ebc)));
    // 22.307f, 65.299f, 22.286f, 65.289f, 22.221f, 65.31f
    path.cubic_to((f32::from_bits(0x41b1560b), f32::from_bits(0x4282a45e)), (f32::from_bits(0x41b08f64), f32::from_bits(0x4282c004)), (f32::from_bits(0x41aff3be), f32::from_bits(0x4282cb47)));
    // 22.167f, 65.321f, 22.07f, 65.375f, 21.994f, 65.397f
    path.cubic_to((f32::from_bits(0x41aea7f7), f32::from_bits(0x4282e14b)), (f32::from_bits(0x41ad893f), f32::from_bits(0x4282f1ad)), (f32::from_bits(0x41ac3d78), f32::from_bits(0x42830835)));
    // 21.832f, 65.44f, 21.692f, 65.472f, 21.53f, 65.516f
    path.cubic_to((f32::from_bits(0x41ac106a), f32::from_bits(0x428312f6)), (f32::from_bits(0x41aba1d2), f32::from_bits(0x42831e39)), (f32::from_bits(0x41ab76d0), f32::from_bits(0x428323db)));
    // 21.508f, 65.537f, 21.454f, 65.559f, 21.433f, 65.57f
    path.cubic_to((f32::from_bits(0x41aac4a3), f32::from_bits(0x4283343d)), (f32::from_bits(0x41aa560b), f32::from_bits(0x4283343d)), (f32::from_bits(0x41a9ba66), f32::from_bits(0x42833f81)));
    // 21.346f, 65.602f, 21.292f, 65.602f, 21.216f, 65.624f
    path.line_to((f32::from_bits(0x41a98f64), f32::from_bits(0x42834fe3)));
    // 21.195f, 65.656f
    path.cubic_to((f32::from_bits(0x41a96256), f32::from_bits(0x42834fe3)), (f32::from_bits(0x41a93754), f32::from_bits(0x42834522)), (f32::from_bits(0x41a920cc), f32::from_bits(0x42834fe3)));
    // 21.173f, 65.656f, 21.152f, 65.635f, 21.141f, 65.656f
    path.cubic_to((f32::from_bits(0x41a90a45), f32::from_bits(0x42834fe3)), (f32::from_bits(0x41a8b234), f32::from_bits(0x42836045)), (f32::from_bits(0x41a89bad), f32::from_bits(0x42836b89)));
    // 21.13f, 65.656f, 21.087f, 65.688f, 21.076f, 65.71f
    path.cubic_to((f32::from_bits(0x41a7d505), f32::from_bits(0x42837beb)), (f32::from_bits(0x41a7666e), f32::from_bits(0x4283818d)), (f32::from_bits(0x41a6cac8), f32::from_bits(0x42839d33)));
    // 20.979f, 65.742f, 20.925f, 65.753f, 20.849f, 65.807f
    path.cubic_to((f32::from_bits(0x41a6b64d), f32::from_bits(0x4283a2d5)), (f32::from_bits(0x41a672b8), f32::from_bits(0x4283b3ba)), (f32::from_bits(0x41a65e3d), f32::from_bits(0x4283b8d9)));
    // 20.839f, 65.818f, 20.806f, 65.851f, 20.796f, 65.861f
    path.cubic_to((f32::from_bits(0x41a6312f), f32::from_bits(0x4283be7b)), (f32::from_bits(0x41a60420), f32::from_bits(0x4283b8d9)), (f32::from_bits(0x41a5ed99), f32::from_bits(0x4283be7b)));
    // 20.774f, 65.872f, 20.752f, 65.861f, 20.741f, 65.872f
    path.cubic_to((f32::from_bits(0x41a5810e), f32::from_bits(0x4283cedd)), (f32::from_bits(0x41a4e568), f32::from_bits(0x428406ac)), (f32::from_bits(0x41a48d57), f32::from_bits(0x42840bcb)));
    // 20.688f, 65.904f, 20.612f, 66.013f, 20.569f, 66.023f
    path.line_to((f32::from_bits(0x41a41ebf), f32::from_bits(0x42840bcb)));
    // 20.515f, 66.023f
    path.cubic_to((f32::from_bits(0x41a40838), f32::from_bits(0x4283fb69)), (f32::from_bits(0x41a3f1b1), f32::from_bits(0x428406ac)), (f32::from_bits(0x41a3f1b1), f32::from_bits(0x4283fb69)));
    // 20.504f, 65.991f, 20.493f, 66.013f, 20.493f, 65.991f
    path.cubic_to((f32::from_bits(0x41a38319), f32::from_bits(0x4283b8da)), (f32::from_bits(0x41a4b859), f32::from_bits(0x4282f750)), (f32::from_bits(0x41a4e567), f32::from_bits(0x4282cb48)));
    // 20.439f, 65.861f, 20.59f, 65.483f, 20.612f, 65.397f
    path.cubic_to((f32::from_bits(0x41a5ed98), f32::from_bits(0x4281d273)), (f32::from_bits(0x41a74fe6), f32::from_bits(0x4280ea00)), (f32::from_bits(0x41a96255), f32::from_bits(0x42802e19)));
    // 20.741f, 64.911f, 20.914f, 64.457f, 21.173f, 64.09f
    path.cubic_to((f32::from_bits(0x41aa2b09), f32::from_bits(0x427fccd6)), (f32::from_bits(0x41ab1ebf), f32::from_bits(0x427f6982)), (f32::from_bits(0x41abfbef), f32::from_bits(0x427eefa7)));
    // 21.271f, 63.95f, 21.39f, 63.853f, 21.498f, 63.734f
    path.cubic_to((f32::from_bits(0x41ac7f01), f32::from_bits(0x427e9690)), (f32::from_bits(0x41aced99), f32::from_bits(0x427e49c4)), (f32::from_bits(0x41ad893f), f32::from_bits(0x427e0734)));
    // 21.562f, 63.647f, 21.616f, 63.572f, 21.692f, 63.507f
    path.cubic_to((f32::from_bits(0x41aed506), f32::from_bits(0x427d8215)), (f32::from_bits(0x41b020cc), f32::from_bits(0x427d137e)), (f32::from_bits(0x41b1831a), f32::from_bits(0x427cbb6d)));
    // 21.854f, 63.377f, 22.016f, 63.269f, 22.189f, 63.183f
    path.cubic_to((f32::from_bits(0x41b1f1b2), f32::from_bits(0x427c99a2)), (f32::from_bits(0x41b26049), f32::from_bits(0x427c6256)), (f32::from_bits(0x41b2cee1), f32::from_bits(0x427c4cd5)));
    // 22.243f, 63.15f, 22.297f, 63.096f, 22.351f, 63.075f
    path.cubic_to((f32::from_bits(0x41b3106a), f32::from_bits(0x427c2b0a)), (f32::from_bits(0x41b445aa), f32::from_bits(0x427bff02)), (f32::from_bits(0x41b49dba), f32::from_bits(0x427bde3d)));
    // 22.383f, 63.042f, 22.534f, 62.999f, 22.577f, 62.967f
    path.cubic_to((f32::from_bits(0x41b49dba), f32::from_bits(0x427bd2f9)), (f32::from_bits(0x41b4cac8), f32::from_bits(0x427ba6f1)), (f32::from_bits(0x41b4cac8), f32::from_bits(0x427ba6f1)));
    // 22.577f, 62.956f, 22.599f, 62.913f, 22.599f, 62.913f
    path.cubic_to((f32::from_bits(0x41b4cac8), f32::from_bits(0x427b6462)), (f32::from_bits(0x41b33b6c), f32::from_bits(0x427a4fe7)), (f32::from_bits(0x41b2fbef), f32::from_bits(0x427a189b)));
    // 22.599f, 62.848f, 22.404f, 62.578f, 22.373f, 62.524f
    path.cubic_to((f32::from_bits(0x41b2cee1), f32::from_bits(0x4279f7d6)), (f32::from_bits(0x41b19795), f32::from_bits(0x42796774)), (f32::from_bits(0x41b1560c), f32::from_bits(0x42795c31)));
    // 22.351f, 62.492f, 22.199f, 62.351f, 22.167f, 62.34f
    path.cubic_to((f32::from_bits(0x41b0e774), f32::from_bits(0x42793b6c)), (f32::from_bits(0x41aff3be), f32::from_bits(0x42795c31)), (f32::from_bits(0x41af70ac), f32::from_bits(0x42795c31)));
    // 22.113f, 62.308f, 21.994f, 62.34f, 21.93f, 62.34f
    path.cubic_to((f32::from_bits(0x41ae0e5e), f32::from_bits(0x42796775)), (f32::from_bits(0x41ac9589), f32::from_bits(0x427946b0)), (f32::from_bits(0x41ab76d1), f32::from_bits(0x42793b6c)));
    // 21.757f, 62.351f, 21.573f, 62.319f, 21.433f, 62.308f
    path.cubic_to((f32::from_bits(0x41aa3f85), f32::from_bits(0x42793028)), (f32::from_bits(0x41a94ddb), f32::from_bits(0x42793b6c)), (f32::from_bits(0x41a82d17), f32::from_bits(0x42793028)));
    // 21.281f, 62.297f, 21.163f, 62.308f, 21.022f, 62.297f
    path.cubic_to((f32::from_bits(0x41a5c298), f32::from_bits(0x42791aa7)), (f32::from_bits(0x41a2e775), f32::from_bits(0x4278ed99)), (f32::from_bits(0x41a07cf6), f32::from_bits(0x4278c190)));
    // 20.72f, 62.276f, 20.363f, 62.232f, 20.061f, 62.189f
    path.cubic_to((f32::from_bits(0x419f47b7), f32::from_bits(0x4278b753)), (f32::from_bits(0x419e810f), f32::from_bits(0x4278b753)), (f32::from_bits(0x419d4bcf), f32::from_bits(0x4278a0cb)));
    // 19.91f, 62.179f, 19.813f, 62.179f, 19.662f, 62.157f
    path.cubic_to((f32::from_bits(0x419c831b), f32::from_bits(0x42788a44)), (f32::from_bits(0x419b20cd), f32::from_bits(0x42785e3c)), (f32::from_bits(0x419a45aa), f32::from_bits(0x427847b4)));
    // 19.564f, 62.135f, 19.391f, 62.092f, 19.284f, 62.07f
    path.cubic_to((f32::from_bits(0x41949171), f32::from_bits(0x4277e460)), (f32::from_bits(0x418e5819), f32::from_bits(0x42778c50)), (f32::from_bits(0x41896a87), f32::from_bits(0x4275dd35)));
    // 18.571f, 61.973f, 17.793f, 61.887f, 17.177f, 61.466f
    path.cubic_to((f32::from_bits(0x4182efa6), f32::from_bits(0x4273a8fc)), (f32::from_bits(0x417fd71a), f32::from_bits(0x42703f83)), (f32::from_bits(0x4180dd37), f32::from_bits(0x426c5b29)));
    // 16.367f, 60.915f, 15.99f, 60.062f, 16.108f, 59.089f
    path.cubic_to((f32::from_bits(0x41813547), f32::from_bits(0x426b5d35)), (f32::from_bits(0x41821276), f32::from_bits(0x426a8006)), (f32::from_bits(0x4182560c), f32::from_bits(0x426976cf)));
    // 16.151f, 58.841f, 16.259f, 58.625f, 16.292f, 58.366f
    path.cubic_to((f32::from_bits(0x418228fe), f32::from_bits(0x426976cf)), (f32::from_bits(0x41823f85), f32::from_bits(0x42696b8b)), (f32::from_bits(0x418228fe), f32::from_bits(0x42694ac7)));
    // 16.27f, 58.366f, 16.281f, 58.355f, 16.27f, 58.323f
    path.cubic_to((f32::from_bits(0x4181a5ec), f32::from_bits(0x42696b8c)), (f32::from_bits(0x41813548), f32::from_bits(0x42696b8c)), (f32::from_bits(0x41809bae), f32::from_bits(0x4269560b)));
    // 16.206f, 58.355f, 16.151f, 58.355f, 16.076f, 58.334f
    path.cubic_to((f32::from_bits(0x4180b235), f32::from_bits(0x4269560b)), (f32::from_bits(0x4180439e), f32::from_bits(0x426976d0)), (f32::from_bits(0x4180168f), f32::from_bits(0x42696b8c)));
    // 16.087f, 58.334f, 16.033f, 58.366f, 16.011f, 58.355f
    path.cubic_to((f32::from_bits(0x417eccdc), f32::from_bits(0x4269560b)), (f32::from_bits(0x417e9fce), f32::from_bits(0x4268d0ec)), (f32::from_bits(0x417f4fef), f32::from_bits(0x42688319)));
    // 15.925f, 58.334f, 15.914f, 58.204f, 15.957f, 58.128f
    path.cubic_to((f32::from_bits(0x4180168f), f32::from_bits(0x4268364c)), (f32::from_bits(0x41849589), f32::from_bits(0x4267a5ea)), (f32::from_bits(0x4185b441), f32::from_bits(0x42679069)));
    // 16.011f, 58.053f, 16.573f, 57.912f, 16.713f, 57.891f
    path.cubic_to((f32::from_bits(0x41891276), f32::from_bits(0x42674296)), (f32::from_bits(0x418c9dba), f32::from_bits(0x4266df42)), (f32::from_bits(0x418fd0ed), f32::from_bits(0x4266916f)));
    // 17.134f, 57.815f, 17.577f, 57.718f, 17.977f, 57.642f
    path.cubic_to((f32::from_bits(0x4190ae1c), f32::from_bits(0x42668732)), (f32::from_bits(0x4191333b), f32::from_bits(0x42668732)), (f32::from_bits(0x4192106a), f32::from_bits(0x426670aa)));
    // 18.085f, 57.632f, 18.15f, 57.632f, 18.258f, 57.61f
    path.cubic_to((f32::from_bits(0x4193189b), f32::from_bits(0x42665a23)), (f32::from_bits(0x4194a5eb), f32::from_bits(0x426622d7)), (f32::from_bits(0x4195dd37), f32::from_bits(0x42660d56)));
    // 18.387f, 57.588f, 18.581f, 57.534f, 18.733f, 57.513f
    path.cubic_to((f32::from_bits(0x41975400), f32::from_bits(0x4265e254)), (f32::from_bits(0x41988b4b), f32::from_bits(0x4265c18f)), (f32::from_bits(0x4199d506), f32::from_bits(0x4265ac0e)));
    // 18.916f, 57.471f, 19.068f, 57.439f, 19.229f, 57.418f
    path.move_to((f32::from_bits(0x41a4e568), f32::from_bits(0x4277d0eb)));
    // 20.612f, 61.954f
    path.cubic_to((f32::from_bits(0x41a4cee1), f32::from_bits(0x4277d0eb)), (f32::from_bits(0x41a48d58), f32::from_bits(0x4277f1b0)), (f32::from_bits(0x41a48d58), f32::from_bits(0x4277f1b0)));
    // 20.601f, 61.954f, 20.569f, 61.986f, 20.569f, 61.986f
    path.cubic_to((f32::from_bits(0x41a3831b), f32::from_bits(0x42781275)), (f32::from_bits(0x41a0c08b), f32::from_bits(0x4277c5a8)), (f32::from_bits(0x419fe35c), f32::from_bits(0x4277af21)));
    // 20.439f, 62.018f, 20.094f, 61.943f, 19.986f, 61.921f
    path.cubic_to((f32::from_bits(0x419dd0ed), f32::from_bits(0x42778319)), (f32::from_bits(0x419bbc73), f32::from_bits(0x42775711)), (f32::from_bits(0x4199c08b), f32::from_bits(0x42771481)));
    // 19.727f, 61.878f, 19.467f, 61.835f, 19.219f, 61.77f
    path.cubic_to((f32::from_bits(0x4199687b), f32::from_bits(0x4277093d)), (f32::from_bits(0x4198f7d7), f32::from_bits(0x4276f3bc)), (f32::from_bits(0x4198b64e), f32::from_bits(0x4276dd35)));
    // 19.176f, 61.759f, 19.121f, 61.738f, 19.089f, 61.716f
    path.cubic_to((f32::from_bits(0x419847b6), f32::from_bits(0x4276d1f1)), (f32::from_bits(0x4198062d), f32::from_bits(0x4276dd35)), (f32::from_bits(0x4197ae1d), f32::from_bits(0x4276d1f1)));
    // 19.035f, 61.705f, 19.003f, 61.716f, 18.96f, 61.705f
    path.cubic_to((f32::from_bits(0x4196fbf0), f32::from_bits(0x4276c6ad)), (f32::from_bits(0x4196083a), f32::from_bits(0x42768f62)), (f32::from_bits(0x4195831b), f32::from_bits(0x427679e1)));
    // 18.873f, 61.694f, 18.754f, 61.64f, 18.689f, 61.619f
    path.cubic_to((f32::from_bits(0x41951690), f32::from_bits(0x4276635a)), (f32::from_bits(0x41950009), f32::from_bits(0x427679e1)), (f32::from_bits(0x4194a5ec), f32::from_bits(0x4276635a)));
    // 18.636f, 61.597f, 18.625f, 61.619f, 18.581f, 61.597f
    path.cubic_to((f32::from_bits(0x41940c52), f32::from_bits(0x42764dd9)), (f32::from_bits(0x41935a25), f32::from_bits(0x4276168d)), (f32::from_bits(0x4192c08c), f32::from_bits(0x42760006)));
    // 18.506f, 61.576f, 18.419f, 61.522f, 18.344f, 61.5f
    path.cubic_to((f32::from_bits(0x4190c298), f32::from_bits(0x42759cb2)), (f32::from_bits(0x418f6257), f32::from_bits(0x427544a2)), (f32::from_bits(0x418e2b0b), f32::from_bits(0x42748837)));
    // 18.095f, 61.403f, 17.923f, 61.317f, 17.771f, 61.133f
    path.cubic_to((f32::from_bits(0x418e1690), f32::from_bits(0x4274666c)), (f32::from_bits(0x418dd2fb), f32::from_bits(0x4274666c)), (f32::from_bits(0x418dbe80), f32::from_bits(0x42745c2f)));
    // 17.761f, 61.1f, 17.728f, 61.1f, 17.718f, 61.09f
    path.cubic_to((f32::from_bits(0x418da7f9), f32::from_bits(0x42742f21)), (f32::from_bits(0x418da7f9), f32::from_bits(0x42740e5c)), (f32::from_bits(0x418d6670), f32::from_bits(0x4273ed97)));
    // 17.707f, 61.046f, 17.707f, 61.014f, 17.675f, 60.982f
    path.cubic_to((f32::from_bits(0x418d22da), f32::from_bits(0x42739fc4)), (f32::from_bits(0x418ccaca), f32::from_bits(0x427373bc)), (f32::from_bits(0x418c9dbc), f32::from_bits(0x42731aa5)));
    // 17.642f, 60.906f, 17.599f, 60.863f, 17.577f, 60.776f
    path.cubic_to((f32::from_bits(0x418bd714), f32::from_bits(0x4271b95d)), (f32::from_bits(0x418d22db), f32::from_bits(0x4270999f)), (f32::from_bits(0x418fd0ef), f32::from_bits(0x4270418e)));
    // 17.48f, 60.431f, 17.642f, 60.15f, 17.977f, 60.064f
    path.cubic_to((f32::from_bits(0x41919fc8), f32::from_bits(0x426ffeff)), (f32::from_bits(0x4193df45), f32::from_bits(0x42701fc3)), (f32::from_bits(0x4195f3c0), f32::from_bits(0x4270841d)));
    // 18.203f, 59.999f, 18.484f, 60.031f, 18.744f, 60.129f
    path.cubic_to((f32::from_bits(0x419847b8), f32::from_bits(0x4270e771)), (f32::from_bits(0x419a5a26), f32::from_bits(0x42718211)), (f32::from_bits(0x419bd2fb), f32::from_bits(0x42723231)));
    // 19.035f, 60.226f, 19.294f, 60.377f, 19.478f, 60.549f
    path.cubic_to((f32::from_bits(0x419be982), f32::from_bits(0x42723e7b)), (f32::from_bits(0x419be982), f32::from_bits(0x42726a83)), (f32::from_bits(0x419c1484), f32::from_bits(0x42726a83)));
    // 19.489f, 60.561f, 19.489f, 60.604f, 19.51f, 60.604f
    path.cubic_to((f32::from_bits(0x419c4192), f32::from_bits(0x42728004)), (f32::from_bits(0x419c831c), f32::from_bits(0x42728004)), (f32::from_bits(0x419c99a3), f32::from_bits(0x4272968b)));
    // 19.532f, 60.625f, 19.564f, 60.625f, 19.575f, 60.647f
    path.cubic_to((f32::from_bits(0x419cdb2c), f32::from_bits(0x4272b750)), (f32::from_bits(0x419d083b), f32::from_bits(0x4272ee9b)), (f32::from_bits(0x419d3549), f32::from_bits(0x427325e7)));
    // 19.607f, 60.679f, 19.629f, 60.733f, 19.651f, 60.787f
    path.cubic_to((f32::from_bits(0x419e28ff), f32::from_bits(0x4273cbca)), (f32::from_bits(0x419f062e), f32::from_bits(0x4274666a)), (f32::from_bits(0x419ff7d8), f32::from_bits(0x42750c4d)));
    // 19.77f, 60.949f, 19.878f, 61.1f, 19.996f, 61.262f
    path.cubic_to((f32::from_bits(0x41a0c08c), f32::from_bits(0x42758628)), (f32::from_bits(0x41a1f5cc), f32::from_bits(0x4275df3f)), (f32::from_bits(0x41a2d2fb), f32::from_bits(0x42766357)));
    // 20.094f, 61.381f, 20.245f, 61.468f, 20.353f, 61.597f
    path.cubic_to((f32::from_bits(0x41a31484), f32::from_bits(0x42769aa3)), (f32::from_bits(0x41a36c95), f32::from_bits(0x4276f3b9)), (f32::from_bits(0x41a3db2c), f32::from_bits(0x42771fc1)));
    // 20.385f, 61.651f, 20.428f, 61.738f, 20.482f, 61.781f
    path.cubic_to((f32::from_bits(0x41a4083a), f32::from_bits(0x42774bc9)), (f32::from_bits(0x41a4b85b), f32::from_bits(0x42778315)), (f32::from_bits(0x41a4e569), f32::from_bits(0x4277af1d)));
    // 20.504f, 61.824f, 20.59f, 61.878f, 20.612f, 61.921f
    path.cubic_to((f32::from_bits(0x41a4e569), f32::from_bits(0x4277ba61)), (f32::from_bits(0x41a4cee2), f32::from_bits(0x4277c5a4)), (f32::from_bits(0x41a4e569), f32::from_bits(0x4277d0e8)));
    // 20.612f, 61.932f, 20.601f, 61.943f, 20.612f, 61.954f
    path.move_to((f32::from_bits(0x41ad72b9), f32::from_bits(0x42786044)));
    // 21.681f, 62.094f
    path.cubic_to((f32::from_bits(0x41ac106b), f32::from_bits(0x42788c4c)), (f32::from_bits(0x41a9d0ee), f32::from_bits(0x4277d0e8)), (f32::from_bits(0x41a8b236), f32::from_bits(0x42778e58)));
    // 21.508f, 62.137f, 21.227f, 61.954f, 21.087f, 61.889f
    path.cubic_to((f32::from_bits(0x41a2fdfd), f32::from_bits(0x42761689)), (f32::from_bits(0x41a10215), f32::from_bits(0x42733c6c)), (f32::from_bits(0x419fb64f), f32::from_bits(0x42704ccf)));
    // 20.374f, 61.522f, 20.126f, 60.809f, 19.964f, 60.075f
    path.cubic_to((f32::from_bits(0x419f9fc8), f32::from_bits(0x42700a40)), (f32::from_bits(0x419f47b7), f32::from_bits(0x426f9ba8)), (f32::from_bits(0x419f3130), f32::from_bits(0x426f5919)));
    // 19.953f, 60.01f, 19.91f, 59.902f, 19.899f, 59.837f
    path.cubic_to((f32::from_bits(0x419f3130), f32::from_bits(0x426f0b46)), (f32::from_bits(0x419f47b7), f32::from_bits(0x426ec9bd)), (f32::from_bits(0x419f3130), f32::from_bits(0x426e70a6)));
    // 19.899f, 59.761f, 19.91f, 59.697f, 19.899f, 59.61f
    path.cubic_to((f32::from_bits(0x419f1aa9), f32::from_bits(0x426de14a)), (f32::from_bits(0x419f062e), f32::from_bits(0x426ced94)), (f32::from_bits(0x419f3130), f32::from_bits(0x426c5d31)));
    // 19.888f, 59.47f, 19.878f, 59.232f, 19.899f, 59.091f
    path.cubic_to((f32::from_bits(0x419f72b9), f32::from_bits(0x426befa0)), (f32::from_bits(0x419fe35d), f32::from_bits(0x426b8108)), (f32::from_bits(0x41a00e5f), f32::from_bits(0x426b3335)));
    // 19.931f, 58.984f, 19.986f, 58.876f, 20.007f, 58.8f
    path.cubic_to((f32::from_bits(0x41a0666f), f32::from_bits(0x426acfe1)), (f32::from_bits(0x41a10215), f32::from_bits(0x4269c6aa)), (f32::from_bits(0x41a19dbb), f32::from_bits(0x4269bb66)));
    // 20.05f, 58.703f, 20.126f, 58.444f, 20.202f, 58.433f
    path.cubic_to((f32::from_bits(0x41a220cd), f32::from_bits(0x4269bb66)), (f32::from_bits(0x41a2a5ec), f32::from_bits(0x4269f2b2)), (f32::from_bits(0x41a31484), f32::from_bits(0x426a3f7f)));
    // 20.266f, 58.433f, 20.331f, 58.487f, 20.385f, 58.562f
    path.cubic_to((f32::from_bits(0x41a3c6b1), f32::from_bits(0x426aa3d9)), (f32::from_bits(0x41a449c3), f32::from_bits(0x426b1cae)), (f32::from_bits(0x41a476d2), f32::from_bits(0x426b3e79)));
    // 20.472f, 58.66f, 20.536f, 58.778f, 20.558f, 58.811f
    path.cubic_to((f32::from_bits(0x41a5ac11), f32::from_bits(0x426c0521)), (f32::from_bits(0x41a6caca), f32::from_bits(0x426ce250)), (f32::from_bits(0x41a8189d), f32::from_bits(0x426da9fe)));
    // 20.709f, 59.005f, 20.849f, 59.221f, 21.012f, 59.416f
    path.cubic_to((f32::from_bits(0x41aa3f86), f32::from_bits(0x426f1689)), (f32::from_bits(0x41ac5401), f32::from_bits(0x4270841b)), (f32::from_bits(0x41ae7aeb), f32::from_bits(0x4271f0a6)));
    // 21.281f, 59.772f, 21.541f, 60.129f, 21.81f, 60.485f
    path.cubic_to((f32::from_bits(0x41af000a), f32::from_bits(0x427248b6)), (f32::from_bits(0x41afb237), f32::from_bits(0x4272a1cd)), (f32::from_bits(0x41b020ce), f32::from_bits(0x4272ee9a)));
    // 21.875f, 60.571f, 21.962f, 60.658f, 22.016f, 60.733f
    path.cubic_to((f32::from_bits(0x41b06257), f32::from_bits(0x42731aa2)), (f32::from_bits(0x41b19797), f32::from_bits(0x4273f7d1)), (f32::from_bits(0x41b19797), f32::from_bits(0x4274199c)));
    // 22.048f, 60.776f, 22.199f, 60.992f, 22.199f, 61.025f
    path.cubic_to((f32::from_bits(0x41b1c4a5), f32::from_bits(0x427424e0)), (f32::from_bits(0x41b1831c), f32::from_bits(0x42746669)), (f32::from_bits(0x41b1831c), f32::from_bits(0x42746669)));
    // 22.221f, 61.036f, 22.189f, 61.1f, 22.189f, 61.1f
    path.cubic_to((f32::from_bits(0x41ac3d7a), f32::from_bits(0x42742f1d)), (f32::from_bits(0x41a96257), f32::from_bits(0x4271ae17)), (f32::from_bits(0x41a7a7f9), f32::from_bits(0x426fb12a)));
    // 21.53f, 61.046f, 21.173f, 60.42f, 20.957f, 59.923f
    path.cubic_to((f32::from_bits(0x41a77cf7), f32::from_bits(0x426f9ba9)), (f32::from_bits(0x41a73b6e), f32::from_bits(0x426f79de)), (f32::from_bits(0x41a73b6e), f32::from_bits(0x426f591a)));
    // 20.936f, 59.902f, 20.904f, 59.869f, 20.904f, 59.837f
    path.cubic_to((f32::from_bits(0x41a6e151), f32::from_bits(0x426eea82)), (f32::from_bits(0x41a68941), f32::from_bits(0x426e6564)), (f32::from_bits(0x41a672ba), f32::from_bits(0x426dec8f)));
    // 20.86f, 59.729f, 20.817f, 59.599f, 20.806f, 59.481f
    path.cubic_to((f32::from_bits(0x41a65e3f), f32::from_bits(0x426daa00)), (f32::from_bits(0x41a68941), f32::from_bits(0x426d71ae)), (f32::from_bits(0x41a65e3f), f32::from_bits(0x426d50e9)));
    // 20.796f, 59.416f, 20.817f, 59.361f, 20.796f, 59.329f
    path.cubic_to((f32::from_bits(0x41a63131), f32::from_bits(0x426d24e1)), (f32::from_bits(0x41a56a89), f32::from_bits(0x426cf8d9)), (f32::from_bits(0x41a4fbf1), f32::from_bits(0x426cf8d9)));
    // 20.774f, 59.286f, 20.677f, 59.243f, 20.623f, 59.243f
    path.cubic_to((f32::from_bits(0x41a449c4), f32::from_bits(0x426ced95)), (f32::from_bits(0x41a36c95), f32::from_bits(0x426cf8d9)), (f32::from_bits(0x41a31484), f32::from_bits(0x426d24e1)));
    // 20.536f, 59.232f, 20.428f, 59.243f, 20.385f, 59.286f
    path.cubic_to((f32::from_bits(0x41a20a47), f32::from_bits(0x426d71ae)), (f32::from_bits(0x41a1f5cc), f32::from_bits(0x426f645e)), (f32::from_bits(0x41a220ce), f32::from_bits(0x42701fc2)));
    // 20.255f, 59.361f, 20.245f, 59.848f, 20.266f, 60.031f
    path.cubic_to((f32::from_bits(0x41a28f66), f32::from_bits(0x4272e45e)), (f32::from_bits(0x41a4b85b), f32::from_bits(0x4274c9be)), (f32::from_bits(0x41a7eb8e), f32::from_bits(0x427621ce)));
    // 20.32f, 60.723f, 20.59f, 61.197f, 20.99f, 61.533f
    path.cubic_to((f32::from_bits(0x41a82d17), f32::from_bits(0x42764293)), (f32::from_bits(0x41a870ad), f32::from_bits(0x42764293)), (f32::from_bits(0x41a8b236), f32::from_bits(0x4276591a)));
    // 21.022f, 61.565f, 21.055f, 61.565f, 21.087f, 61.587f
    path.cubic_to((f32::from_bits(0x41a90a46), f32::from_bits(0x427679df)), (f32::from_bits(0x41a93755), f32::from_bits(0x4276b12a)), (f32::from_bits(0x41a98f65), f32::from_bits(0x4276c6ab)));
    // 21.13f, 61.619f, 21.152f, 61.673f, 21.195f, 61.694f
    path.cubic_to((f32::from_bits(0x41aadb2c), f32::from_bits(0x42774086)), (f32::from_bits(0x41ac958a), f32::from_bits(0x42778e59)), (f32::from_bits(0x41adb64e), f32::from_bits(0x427828f9)));
    // 21.357f, 61.813f, 21.573f, 61.889f, 21.714f, 62.04f
    path.cubic_to((f32::from_bits(0x41adb64e), f32::from_bits(0x427828f9)), (f32::from_bits(0x41ad8940), f32::from_bits(0x42786045)), (f32::from_bits(0x41ad72b8), f32::from_bits(0x42786045)));
    // 21.714f, 62.04f, 21.692f, 62.094f, 21.681f, 62.094f
    path.move_to((f32::from_bits(0x41bd168f), f32::from_bits(0x4267be7a)));
    // 23.636f, 57.936f
    path.cubic_to((f32::from_bits(0x41bd168f), f32::from_bits(0x42679caf)), (f32::from_bits(0x41bd2d16), f32::from_bits(0x4267666a)), (f32::from_bits(0x41bd168f), f32::from_bits(0x42674fe2)));
    // 23.636f, 57.903f, 23.647f, 57.85f, 23.636f, 57.828f
    path.cubic_to((f32::from_bits(0x41bd168f), f32::from_bits(0x4267449e)), (f32::from_bits(0x41bd0008), f32::from_bits(0x42674fe2)), (f32::from_bits(0x41bce981), f32::from_bits(0x42672f1d)));
    // 23.636f, 57.817f, 23.625f, 57.828f, 23.614f, 57.796f
    path.cubic_to((f32::from_bits(0x41bcd2fa), f32::from_bits(0x42672f1d)), (f32::from_bits(0x41bc9171), f32::from_bits(0x4267449e)), (f32::from_bits(0x41bc7ae9), f32::from_bits(0x42672f1d)));
    // 23.603f, 57.796f, 23.571f, 57.817f, 23.56f, 57.796f
    path.cubic_to((f32::from_bits(0x41bb9dba), f32::from_bits(0x4267d500)), (f32::from_bits(0x41bbb441), f32::from_bits(0x42693648)), (f32::from_bits(0x41bb72b8), f32::from_bits(0x426a1377)));
    // 23.452f, 57.958f, 23.463f, 58.303f, 23.431f, 58.519f
    path.cubic_to((f32::from_bits(0x41bb45aa), f32::from_bits(0x426a6c8e)), (f32::from_bits(0x41bb2f22), f32::from_bits(0x426acfe1)), (f32::from_bits(0x41bb189b), f32::from_bits(0x426b3335)));
    // 23.409f, 58.606f, 23.398f, 58.703f, 23.387f, 58.8f
    path.line_to((f32::from_bits(0x41baed99), f32::from_bits(0x426b5f3d)));
    // 23.366f, 58.843f
    path.cubic_to((f32::from_bits(0x41baac10), f32::from_bits(0x426bd918)), (f32::from_bits(0x41bac08b), f32::from_bits(0x426c3129)), (f32::from_bits(0x41baac10), f32::from_bits(0x426cab04)));
    // 23.334f, 58.962f, 23.344f, 59.048f, 23.334f, 59.167f
    path.cubic_to((f32::from_bits(0x41ba7f02), f32::from_bits(0x426d50e7)), (f32::from_bits(0x41ba3b6c), f32::from_bits(0x426e0d52)), (f32::from_bits(0x41ba106a), f32::from_bits(0x426ec9bc)));
    // 23.312f, 59.329f, 23.279f, 59.513f, 23.258f, 59.697f
    path.cubic_to((f32::from_bits(0x41b9ccd4), f32::from_bits(0x426f645c)), (f32::from_bits(0x41b974c4), f32::from_bits(0x42701fc0)), (f32::from_bits(0x41b949c2), f32::from_bits(0x4270c5a3)));
    // 23.225f, 59.848f, 23.182f, 60.031f, 23.161f, 60.193f
    path.cubic_to((f32::from_bits(0x41b9333b), f32::from_bits(0x42713f7e)), (f32::from_bits(0x41b98b4b), f32::from_bits(0x4271820d)), (f32::from_bits(0x41b9f9e3), f32::from_bits(0x4271ae16)));
    // 23.15f, 60.312f, 23.193f, 60.377f, 23.247f, 60.42f
    path.cubic_to((f32::from_bits(0x41ba3b6c), f32::from_bits(0x42718d51)), (f32::from_bits(0x41ba7f02), f32::from_bits(0x4271b95a)), (f32::from_bits(0x41ba9589), f32::from_bits(0x42716b87)));
    // 23.279f, 60.388f, 23.312f, 60.431f, 23.323f, 60.355f
    path.cubic_to((f32::from_bits(0x41baac10), f32::from_bits(0x4271343b)), (f32::from_bits(0x41ba9589), f32::from_bits(0x4270e76e)), (f32::from_bits(0x41ba9589), f32::from_bits(0x4270999b)));
    // 23.334f, 60.301f, 23.323f, 60.226f, 23.323f, 60.15f
    path.cubic_to((f32::from_bits(0x41ba9589), f32::from_bits(0x4270418b)), (f32::from_bits(0x41bac08b), f32::from_bits(0x426fd1ed)), (f32::from_bits(0x41baed99), f32::from_bits(0x426f645c)));
    // 23.323f, 60.064f, 23.344f, 59.955f, 23.366f, 59.848f
    path.cubic_to((f32::from_bits(0x41bb2f22), f32::from_bits(0x426e6562)), (f32::from_bits(0x41bb9dba), f32::from_bits(0x426d3b66)), (f32::from_bits(0x41bbf5ca), f32::from_bits(0x426c3c6c)));
    // 23.398f, 59.599f, 23.452f, 59.308f, 23.495f, 59.059f
    path.cubic_to((f32::from_bits(0x41bc0e5d), f32::from_bits(0x426bb853)), (f32::from_bits(0x41bc0e5d), f32::from_bits(0x426b5f3d)), (f32::from_bits(0x41bc22d8), f32::from_bits(0x426ae562)));
    // 23.507f, 58.93f, 23.507f, 58.843f, 23.517f, 58.724f
    path.cubic_to((f32::from_bits(0x41bc395f), f32::from_bits(0x426a820e)), (f32::from_bits(0x41bc9170), f32::from_bits(0x4269f2b2)), (f32::from_bits(0x41bca7f7), f32::from_bits(0x42698f5e)));
    // 23.528f, 58.627f, 23.571f, 58.487f, 23.582f, 58.39f
    path.cubic_to((f32::from_bits(0x41bcd2f9), f32::from_bits(0x426920c6)), (f32::from_bits(0x41bca7f7), f32::from_bits(0x4268d2f4)), (f32::from_bits(0x41bcd2f9), f32::from_bits(0x4268645c)));
    // 23.603f, 58.282f, 23.582f, 58.206f, 23.603f, 58.098f
    path.cubic_to((f32::from_bits(0x41bcd2f9), f32::from_bits(0x42684291)), (f32::from_bits(0x41bd168f), f32::from_bits(0x4267df3d)), (f32::from_bits(0x41bd168f), f32::from_bits(0x4267be79)));
    // 23.603f, 58.065f, 23.636f, 57.968f, 23.636f, 57.936f
    path.move_to((f32::from_bits(0x41bd6e9f), f32::from_bits(0x426e916b)));
    // 23.679f, 59.642f
    path.cubic_to((f32::from_bits(0x41bdb028), f32::from_bits(0x426d199c)), (f32::from_bits(0x41bdf3be), f32::from_bits(0x426bb854)), (f32::from_bits(0x41be6255), f32::from_bits(0x426a343c)));
    // 23.711f, 59.275f, 23.744f, 58.93f, 23.798f, 58.551f
    path.cubic_to((f32::from_bits(0x41be78dc), f32::from_bits(0x4269f2b3)), (f32::from_bits(0x41bed0ed), f32::from_bits(0x4269841b)), (f32::from_bits(0x41bed0ed), f32::from_bits(0x4269418c)));
    // 23.809f, 58.487f, 23.852f, 58.379f, 23.852f, 58.314f
    path.cubic_to((f32::from_bits(0x41bee774), f32::from_bits(0x4268bc6d)), (f32::from_bits(0x41bee774), f32::from_bits(0x42684edc)), (f32::from_bits(0x41bf1276), f32::from_bits(0x4267df3e)));
    // 23.863f, 58.184f, 23.863f, 58.077f, 23.884f, 57.968f
    path.cubic_to((f32::from_bits(0x41bf3f84), f32::from_bits(0x4267a7f2)), (f32::from_bits(0x41bf3f84), f32::from_bits(0x4267872e)), (f32::from_bits(0x41bf9795), f32::from_bits(0x426770a6)));
    // 23.906f, 57.914f, 23.906f, 57.882f, 23.949f, 57.86f
    path.cubic_to((f32::from_bits(0x41c0ccd4), f32::from_bits(0x42675b25)), (f32::from_bits(0x41c6810e), f32::from_bits(0x4268d2f4)), (f32::from_bits(0x41c6d91e), f32::from_bits(0x426920c7)));
    // 24.1f, 57.839f, 24.813f, 58.206f, 24.856f, 58.282f
    path.cubic_to((f32::from_bits(0x41c7333b), f32::from_bits(0x42696d94)), (f32::from_bits(0x41c7062c), f32::from_bits(0x4270e76f)), (f32::from_bits(0x41c6ae1c), f32::from_bits(0x42713f7f)));
    // 24.9f, 58.357f, 24.878f, 60.226f, 24.835f, 60.312f
    path.cubic_to((f32::from_bits(0x41c63f84), f32::from_bits(0x4271a2d3)), (f32::from_bits(0x41c3a7f7), f32::from_bits(0x42716b87)), (f32::from_bits(0x41c2cac8), f32::from_bits(0x427176cb)));
    // 24.781f, 60.409f, 24.457f, 60.355f, 24.349f, 60.366f
    path.cubic_to((f32::from_bits(0x41c2b441), f32::from_bits(0x427176cb)), (f32::from_bits(0x41c270ab), f32::from_bits(0x4271a2d3)), (f32::from_bits(0x41c245a9), f32::from_bits(0x4271a2d3)));
    // 24.338f, 60.366f, 24.305f, 60.409f, 24.284f, 60.409f
    path.cubic_to((f32::from_bits(0x41c1aa03), f32::from_bits(0x4271b95a)), (f32::from_bits(0x41c1106a), f32::from_bits(0x4271ae17)), (f32::from_bits(0x41c05e3c), f32::from_bits(0x4271b95a)));
    // 24.208f, 60.431f, 24.133f, 60.42f, 24.046f, 60.431f
    path.cubic_to((f32::from_bits(0x41bf1275), f32::from_bits(0x4271e562)), (f32::from_bits(0x41be4bcd), f32::from_bits(0x427227f2)), (f32::from_bits(0x41bcd2f8), f32::from_bits(0x4272322f)));
    // 23.884f, 60.474f, 23.787f, 60.539f, 23.603f, 60.549f
    path.cubic_to((f32::from_bits(0x41bc395e), f32::from_bits(0x427128f8)), (f32::from_bits(0x41bd2d15), f32::from_bits(0x426f8f5e)), (f32::from_bits(0x41bd6e9e), f32::from_bits(0x426e916a)));
    // 23.528f, 60.29f, 23.647f, 59.89f, 23.679f, 59.642f
    path.move_to((f32::from_bits(0x41d21481), f32::from_bits(0x42700a3f)));
    // 26.26f, 60.01f
    path.cubic_to((f32::from_bits(0x41d22b08), f32::from_bits(0x42704cce)), (f32::from_bits(0x41d299a0), f32::from_bits(0x4270f1ac)), (f32::from_bits(0x41d2418f), f32::from_bits(0x42713f7e)));
    // 26.271f, 60.075f, 26.325f, 60.236f, 26.282f, 60.312f
    path.cubic_to((f32::from_bits(0x41d2418f), f32::from_bits(0x42714ac2)), (f32::from_bits(0x41d22b08), f32::from_bits(0x42713f7e)), (f32::from_bits(0x41d21481), f32::from_bits(0x42715605)));
    // 26.282f, 60.323f, 26.271f, 60.312f, 26.26f, 60.334f
    path.cubic_to((f32::from_bits(0x41d1bc71), f32::from_bits(0x42715605)), (f32::from_bits(0x41d1916f), f32::from_bits(0x42715605)), (f32::from_bits(0x41d1395e), f32::from_bits(0x42714ac1)));
    // 26.217f, 60.334f, 26.196f, 60.334f, 26.153f, 60.323f
    path.cubic_to((f32::from_bits(0x41d0b233), f32::from_bits(0x42708419)), (f32::from_bits(0x41d0c8ba), f32::from_bits(0x426f645b)), (f32::from_bits(0x41d09db8), f32::from_bits(0x426e5a1d)));
    // 26.087f, 60.129f, 26.098f, 59.848f, 26.077f, 59.588f
    path.cubic_to((f32::from_bits(0x41d09db8), f32::from_bits(0x426e23d7)), (f32::from_bits(0x41d05a22), f32::from_bits(0x426d9375)), (f32::from_bits(0x41d070aa), f32::from_bits(0x426d50e6)));
    // 26.077f, 59.535f, 26.044f, 59.394f, 26.055f, 59.329f
    path.cubic_to((f32::from_bits(0x41d09db8), f32::from_bits(0x426d3b65)), (f32::from_bits(0x41d0b233), f32::from_bits(0x426d50e6)), (f32::from_bits(0x41d0b233), f32::from_bits(0x426d2f1b)));
    // 26.077f, 59.308f, 26.087f, 59.329f, 26.087f, 59.296f
    path.cubic_to((f32::from_bits(0x41d1395e), f32::from_bits(0x426d3b65)), (f32::from_bits(0x41d14dd9), f32::from_bits(0x426d2f1b)), (f32::from_bits(0x41d1916e), f32::from_bits(0x426d50e6)));
    // 26.153f, 59.308f, 26.163f, 59.296f, 26.196f, 59.329f
    path.cubic_to((f32::from_bits(0x41d1a5e9), f32::from_bits(0x426d50e6)), (f32::from_bits(0x41d1e97e), f32::from_bits(0x426de148)), (f32::from_bits(0x41d1e97e), f32::from_bits(0x426dec8c)));
    // 26.206f, 59.329f, 26.239f, 59.47f, 26.239f, 59.481f
    path.cubic_to((f32::from_bits(0x41d22b07), f32::from_bits(0x426e9cad)), (f32::from_bits(0x41d1e97e), f32::from_bits(0x426f4dd4)), (f32::from_bits(0x41d21480), f32::from_bits(0x42700a3e)));
    // 26.271f, 59.653f, 26.239f, 59.826f, 26.26f, 60.01f
    path.move_to((f32::from_bits(0x41ee1274), f32::from_bits(0x42564ac1)));
    // 29.759f, 53.573f
    path.cubic_to((f32::from_bits(0x41ee1274), f32::from_bits(0x42566b86)), (f32::from_bits(0x41ee3f82), f32::from_bits(0x4256c49c)), (f32::from_bits(0x41ee28fb), f32::from_bits(0x4256fbe8)));
    // 29.759f, 53.605f, 29.781f, 53.692f, 29.77f, 53.746f
    path.cubic_to((f32::from_bits(0x41ee28fb), f32::from_bits(0x42571cad)), (f32::from_bits(0x41ede772), f32::from_bits(0x425748b5)), (f32::from_bits(0x41ede772), f32::from_bits(0x42576a80)));
    // 29.77f, 53.778f, 29.738f, 53.821f, 29.738f, 53.854f
    path.cubic_to((f32::from_bits(0x41ed8f62), f32::from_bits(0x425774bd)), (f32::from_bits(0x41ed20ca), f32::from_bits(0x42579688)), (f32::from_bits(0x41ec6e9d), f32::from_bits(0x42579688)));
    // 29.695f, 53.864f, 29.641f, 53.897f, 29.554f, 53.897f
    path.cubic_to((f32::from_bits(0x41ebeb8b), f32::from_bits(0x42579688)), (f32::from_bits(0x41eb666c), f32::from_bits(0x425774bd)), (f32::from_bits(0x41eaf7d4), f32::from_bits(0x42576a80)));
    // 29.49f, 53.897f, 29.425f, 53.864f, 29.371f, 53.854f
    path.cubic_to((f32::from_bits(0x41eacac6), f32::from_bits(0x425676ca)), (f32::from_bits(0x41eb666c), f32::from_bits(0x42556d92)), (f32::from_bits(0x41ebbe7c), f32::from_bits(0x42549063)));
    // 29.349f, 53.616f, 29.425f, 53.357f, 29.468f, 53.141f
    path.cubic_to((f32::from_bits(0x41ebd503), f32::from_bits(0x425421cb)), (f32::from_bits(0x41ebd503), f32::from_bits(0x4253d3f9)), (f32::from_bits(0x41ec0005), f32::from_bits(0x42537be8)));
    // 29.479f, 53.033f, 29.479f, 52.957f, 29.5f, 52.871f
    path.cubic_to((f32::from_bits(0x41ec2d13), f32::from_bits(0x42535a1d)), (f32::from_bits(0x41ec6e9d), f32::from_bits(0x42531894)), (f32::from_bits(0x41ecb232), f32::from_bits(0x42531894)));
    // 29.522f, 52.838f, 29.554f, 52.774f, 29.587f, 52.774f
    path.cubic_to((f32::from_bits(0x41ed3544), f32::from_bits(0x4253020d)), (f32::from_bits(0x41edd0ea), f32::from_bits(0x42531894)), (f32::from_bits(0x41ede771), f32::from_bits(0x4253449c)));
    // 29.651f, 52.752f, 29.727f, 52.774f, 29.738f, 52.817f
    path.cubic_to((f32::from_bits(0x41ee1273), f32::from_bits(0x42534fe0)), (f32::from_bits(0x41ede771), f32::from_bits(0x42536561)), (f32::from_bits(0x41ede771), f32::from_bits(0x42537be8)));
    // 29.759f, 52.828f, 29.738f, 52.849f, 29.738f, 52.871f
    path.cubic_to((f32::from_bits(0x41ee3f81), f32::from_bits(0x42544290)), (f32::from_bits(0x41ede771), f32::from_bits(0x42554ccd)), (f32::from_bits(0x41ee1273), f32::from_bits(0x42564ac1)));
    // 29.781f, 53.065f, 29.738f, 53.325f, 29.759f, 53.573f
    path.move_to((f32::from_bits(0x41f51273), f32::from_bits(0x4258cbc7)));
    // 30.634f, 54.199f
    path.cubic_to((f32::from_bits(0x41f4e771), f32::from_bits(0x4259199a)), (f32::from_bits(0x41f3b025), f32::from_bits(0x4259bf7d)), (f32::from_bits(0x41f35815), f32::from_bits(0x4259eb85)));
    // 30.613f, 54.275f, 30.461f, 54.437f, 30.418f, 54.48f
    path.cubic_to((f32::from_bits(0x41f2395d), f32::from_bits(0x425aa6e9)), (f32::from_bits(0x41f2395d), f32::from_bits(0x425a449c)), (f32::from_bits(0x41f222d6), f32::from_bits(0x42596666)));
    // 30.278f, 54.663f, 30.278f, 54.567f, 30.267f, 54.35f
    path.cubic_to((f32::from_bits(0x41f222d6), f32::from_bits(0x425945a1)), (f32::from_bits(0x41f1f5c8), f32::from_bits(0x4258e24d)), (f32::from_bits(0x41f222d6), f32::from_bits(0x4258ab02)));
    // 30.267f, 54.318f, 30.245f, 54.221f, 30.267f, 54.167f
    path.cubic_to((f32::from_bits(0x41f2395d), f32::from_bits(0x42589fbe)), (f32::from_bits(0x41f2e97e), f32::from_bits(0x42588a3d)), (f32::from_bits(0x41f30005), f32::from_bits(0x425873b6)));
    // 30.278f, 54.156f, 30.364f, 54.135f, 30.375f, 54.113f
    path.cubic_to((f32::from_bits(0x41f3b026), f32::from_bits(0x42586872)), (f32::from_bits(0x41f48d55), f32::from_bits(0x42588937)), (f32::from_bits(0x41f51274), f32::from_bits(0x4258947b)));
    // 30.461f, 54.102f, 30.569f, 54.134f, 30.634f, 54.145f
    path.cubic_to((f32::from_bits(0x41f4fdf9), f32::from_bits(0x42589fbf)), (f32::from_bits(0x41f51274), f32::from_bits(0x4258b646)), (f32::from_bits(0x41f51274), f32::from_bits(0x4258cbc7)));
    // 30.624f, 54.156f, 30.634f, 54.178f, 30.634f, 54.199f
    path.move_to((f32::from_bits(0x41f20e5b), f32::from_bits(0x425727f0)));
    // 30.257f, 53.789f
    path.cubic_to((f32::from_bits(0x41f1cac5), f32::from_bits(0x4256da1d)), (f32::from_bits(0x41f222d6), f32::from_bits(0x42561375)), (f32::from_bits(0x41f222d6), f32::from_bits(0x4255d0e6)));
    // 30.224f, 53.713f, 30.267f, 53.519f, 30.267f, 53.454f
    path.cubic_to((f32::from_bits(0x41f222d6), f32::from_bits(0x42553646)), (f32::from_bits(0x41f1b43e), f32::from_bits(0x4254374c)), (f32::from_bits(0x41f20e5b), f32::from_bits(0x42539169)));
    // 30.267f, 53.303f, 30.213f, 53.054f, 30.257f, 52.892f
    path.cubic_to((f32::from_bits(0x41f222d6), f32::from_bits(0x42536561)), (f32::from_bits(0x41f2916d), f32::from_bits(0x4253449c)), (f32::from_bits(0x41f2be7c), f32::from_bits(0x4253449c)));
    // 30.267f, 52.849f, 30.321f, 52.817f, 30.343f, 52.817f
    path.cubic_to((f32::from_bits(0x41f3b026), f32::from_bits(0x42532e15)), (f32::from_bits(0x41f845a7), f32::from_bits(0x42539cac)), (f32::from_bits(0x41f88730), f32::from_bits(0x4253d3f8)));
    // 30.461f, 52.795f, 31.034f, 52.903f, 31.066f, 52.957f
    path.cubic_to((f32::from_bits(0x41f8cac6), f32::from_bits(0x42540000)), (f32::from_bits(0x41f8cac6), f32::from_bits(0x42544290)), (f32::from_bits(0x41f8e14d), f32::from_bits(0x4254851f)));
    // 31.099f, 53, 31.099f, 53.065f, 31.11f, 53.13f
    path.cubic_to((f32::from_bits(0x41f8f5c8), f32::from_bits(0x4254d1ec)), (f32::from_bits(0x41f97ae7), f32::from_bits(0x425578d5)), (f32::from_bits(0x41f9666c), f32::from_bits(0x4255e76d)));
    // 31.12f, 53.205f, 31.185f, 53.368f, 31.175f, 53.476f
    path.cubic_to((f32::from_bits(0x41f94dd9), f32::from_bits(0x42561375)), (f32::from_bits(0x41f88731), f32::from_bits(0x4256a2d1)), (f32::from_bits(0x41f85c2f), f32::from_bits(0x4256c49c)));
    // 31.163f, 53.519f, 31.066f, 53.659f, 31.045f, 53.692f
    path.cubic_to((f32::from_bits(0x41f845a8), f32::from_bits(0x4256da1d)), (f32::from_bits(0x41f7d710), f32::from_bits(0x4256f0a4)), (f32::from_bits(0x41f7d710), f32::from_bits(0x4256fbe8)));
    // 31.034f, 53.713f, 30.98f, 53.735f, 30.98f, 53.746f
    path.line_to((f32::from_bits(0x41f7d710), f32::from_bits(0x42571cad)));
    // 30.98f, 53.778f
    path.cubic_to((f32::from_bits(0x41f79587), f32::from_bits(0x4257322e)), (f32::from_bits(0x41f73b6a), f32::from_bits(0x425748b5)), (f32::from_bits(0x41f6f9e1), f32::from_bits(0x42575f3c)));
    // 30.948f, 53.799f, 30.904f, 53.821f, 30.872f, 53.843f
    path.cubic_to((f32::from_bits(0x41f6062b), f32::from_bits(0x425774bd)), (f32::from_bits(0x41f2395e), f32::from_bits(0x425774bd)), (f32::from_bits(0x41f20e5c), f32::from_bits(0x425727f0)));
    // 30.753f, 53.864f, 30.278f, 53.864f, 30.257f, 53.789f
    path.move_to((f32::from_bits(0x42048f5f), f32::from_bits(0x426b072b)));
    // 33.14f, 58.757f
    path.cubic_to((f32::from_bits(0x42046d94), f32::from_bits(0x426acfdf)), (f32::from_bits(0x42048f5f), f32::from_bits(0x426ab958)), (f32::from_bits(0x420478d8), f32::from_bits(0x426a77cf)));
    // 33.107f, 58.703f, 33.14f, 58.681f, 33.118f, 58.617f
    path.cubic_to((f32::from_bits(0x42045813), f32::from_bits(0x4269d0e6)), (f32::from_bits(0x42042c0b), f32::from_bits(0x42693646)), (f32::from_bits(0x42041584), f32::from_bits(0x4268851f)));
    // 33.086f, 58.454f, 33.043f, 58.303f, 33.021f, 58.13f
    path.cubic_to((f32::from_bits(0x4203e97c), f32::from_bits(0x4267c9bb)), (f32::from_bits(0x42039caf), f32::from_bits(0x42670d50)), (f32::from_bits(0x4203a6ec), f32::from_bits(0x426624dd)));
    // 32.978f, 57.947f, 32.903f, 57.763f, 32.913f, 57.536f
    path.cubic_to((f32::from_bits(0x4203a6ec), f32::from_bits(0x426624dd)), (f32::from_bits(0x4203de38), f32::from_bits(0x4265f8d5)), (f32::from_bits(0x4203e97b), f32::from_bits(0x4265f8d5)));
    // 32.913f, 57.536f, 32.967f, 57.493f, 32.978f, 57.493f
    path.cubic_to((f32::from_bits(0x42042c0a), f32::from_bits(0x4265ee98)), (f32::from_bits(0x4204c6aa), f32::from_bits(0x4266199a)), (f32::from_bits(0x4204e875), f32::from_bits(0x42663b64)));
    // 33.043f, 57.483f, 33.194f, 57.525f, 33.227f, 57.558f
    path.cubic_to((f32::from_bits(0x42051ebb), f32::from_bits(0x42668937)), (f32::from_bits(0x42051ebb), f32::from_bits(0x42671893)), (f32::from_bits(0x42054085), f32::from_bits(0x426770a3)));
    // 33.28f, 57.634f, 33.28f, 57.774f, 33.313f, 57.86f
    path.cubic_to((f32::from_bits(0x42058314), f32::from_bits(0x4268a6e9)), (f32::from_bits(0x4206072d), f32::from_bits(0x4269d0e5)), (f32::from_bits(0x42061271), f32::from_bits(0x426b3e76)));
    // 33.378f, 58.163f, 33.507f, 58.454f, 33.518f, 58.811f
    path.cubic_to((f32::from_bits(0x4205e669), f32::from_bits(0x426b3e76)), (f32::from_bits(0x4205e669), f32::from_bits(0x426b49ba)), (f32::from_bits(0x4205b95a), f32::from_bits(0x426b5f3b)));
    // 33.475f, 58.811f, 33.475f, 58.822f, 33.431f, 58.843f
    path.cubic_to((f32::from_bits(0x42056c8d), f32::from_bits(0x426b5f3b)), (f32::from_bits(0x4204e875), f32::from_bits(0x426b75c2)), (f32::from_bits(0x4204b023), f32::from_bits(0x426b49ba)));
    // 33.356f, 58.843f, 33.227f, 58.865f, 33.172f, 58.822f
    path.line_to((f32::from_bits(0x4204b023), f32::from_bits(0x426b3333)));
    // 33.172f, 58.8f
    path.cubic_to((f32::from_bits(0x4204b023), f32::from_bits(0x426b27ef)), (f32::from_bits(0x42048f5e), f32::from_bits(0x426b072b)), (f32::from_bits(0x42048f5e), f32::from_bits(0x426b072b)));
    // 33.172f, 58.789f, 33.14f, 58.757f, 33.14f, 58.757f
    path.move_to((f32::from_bits(0x42035918), f32::from_bits(0x426b6a7f)));
    // 32.837f, 58.854f
    path.cubic_to((f32::from_bits(0x42032d10), f32::from_bits(0x426b6a7f)), (f32::from_bits(0x42030108), f32::from_bits(0x426b75c3)), (f32::from_bits(0x4202d4ff), f32::from_bits(0x426b75c3)));
    // 32.794f, 58.854f, 32.751f, 58.865f, 32.708f, 58.865f
    path.cubic_to((f32::from_bits(0x42026667), f32::from_bits(0x426b75c3)), (f32::from_bits(0x42020d51), f32::from_bits(0x426b5f3c)), (f32::from_bits(0x4201ec8c), f32::from_bits(0x426b27f0)));
    // 32.6f, 58.865f, 32.513f, 58.843f, 32.481f, 58.789f
    path.cubic_to((f32::from_bits(0x4201cbc7), f32::from_bits(0x426ae561)), (f32::from_bits(0x4201cbc7), f32::from_bits(0x426a6c8c)), (f32::from_bits(0x4201b540), f32::from_bits(0x426a0832)));
    // 32.449f, 58.724f, 32.449f, 58.606f, 32.427f, 58.508f
    path.cubic_to((f32::from_bits(0x42018938), f32::from_bits(0x426920c5)), (f32::from_bits(0x42016873), f32::from_bits(0x42683853)), (f32::from_bits(0x42013021), f32::from_bits(0x42672f1b)));
    // 32.384f, 58.282f, 32.352f, 58.055f, 32.297f, 57.796f
    path.cubic_to((f32::from_bits(0x42013021), f32::from_bits(0x4267020d)), (f32::from_bits(0x4200f9db), f32::from_bits(0x42669375)), (f32::from_bits(0x4200f9db), f32::from_bits(0x426651ec)));
    // 32.297f, 57.752f, 32.244f, 57.644f, 32.244f, 57.58f
    path.cubic_to((f32::from_bits(0x42010418), f32::from_bits(0x4266199a)), (f32::from_bits(0x420151eb), f32::from_bits(0x4265ee98)), (f32::from_bits(0x42018937), f32::from_bits(0x4265ee98)));
    // 32.254f, 57.525f, 32.33f, 57.483f, 32.384f, 57.483f
    path.cubic_to((f32::from_bits(0x4201e147), f32::from_bits(0x4265e24e)), (f32::from_bits(0x42022f1a), f32::from_bits(0x4265ee98)), (f32::from_bits(0x42023a5e), f32::from_bits(0x4266199a)));
    // 32.47f, 57.471f, 32.546f, 57.483f, 32.557f, 57.525f
    path.cubic_to((f32::from_bits(0x420271aa), f32::from_bits(0x42665c29)), (f32::from_bits(0x42027be7), f32::from_bits(0x42670d50)), (f32::from_bits(0x42029db2), f32::from_bits(0x426770a4)));
    // 32.611f, 57.59f, 32.621f, 57.763f, 32.654f, 57.86f
    path.cubic_to((f32::from_bits(0x42029db2), f32::from_bits(0x4267be77)), (f32::from_bits(0x4202d4fe), f32::from_bits(0x4268178d)), (f32::from_bits(0x4202e041), f32::from_bits(0x42684ed9)));
    // 32.654f, 57.936f, 32.708f, 58.023f, 32.719f, 58.077f
    path.cubic_to((f32::from_bits(0x4202ea7e), f32::from_bits(0x4268bc6a)), (f32::from_bits(0x4202ea7e), f32::from_bits(0x4268fefa)), (f32::from_bits(0x42030106), f32::from_bits(0x42695810)));
    // 32.729f, 58.184f, 32.729f, 58.249f, 32.751f, 58.336f
    path.cubic_to((f32::from_bits(0x420322d1), f32::from_bits(0x4269fced)), (f32::from_bits(0x4203645a), f32::from_bits(0x426a820c)), (f32::from_bits(0x4203645a), f32::from_bits(0x426b49ba)));
    // 32.784f, 58.497f, 32.848f, 58.627f, 32.848f, 58.822f
    path.cubic_to((f32::from_bits(0x42034395), f32::from_bits(0x426b49ba)), (f32::from_bits(0x42035916), f32::from_bits(0x426b49ba)), (f32::from_bits(0x42035916), f32::from_bits(0x426b6a7f)));
    // 32.816f, 58.822f, 32.837f, 58.822f, 32.837f, 58.854f
    path.move_to((f32::from_bits(0x42009580), f32::from_bits(0x426b6a7f)));
    // 32.146f, 58.854f
    path.line_to((f32::from_bits(0x42008b43), f32::from_bits(0x426b8106)));
    // 32.136f, 58.876f
    path.cubic_to((f32::from_bits(0x42007fff), f32::from_bits(0x426b8106)), (f32::from_bits(0x42005e35), f32::from_bits(0x426b75c2)), (f32::from_bits(0x420048b4), f32::from_bits(0x426b8106)));
    // 32.125f, 58.876f, 32.092f, 58.865f, 32.071f, 58.876f
    path.cubic_to((f32::from_bits(0x41fdcccc), f32::from_bits(0x426bad0e)), (f32::from_bits(0x41f94dd2), f32::from_bits(0x426b8c4a)), (f32::from_bits(0x41f6cccc), f32::from_bits(0x426b8c4a)));
    // 31.725f, 58.919f, 31.163f, 58.887f, 30.85f, 58.887f
    path.cubic_to((f32::from_bits(0x41f65e34), f32::from_bits(0x426b8106)), (f32::from_bits(0x41f39ba5), f32::from_bits(0x426b8106)), (f32::from_bits(0x41f35810), f32::from_bits(0x426b49bb)));
    // 30.796f, 58.876f, 30.451f, 58.876f, 30.418f, 58.822f
    path.cubic_to((f32::from_bits(0x41f35810), f32::from_bits(0x426b3334)), (f32::from_bits(0x41f2e978), f32::from_bits(0x4267926f)), (f32::from_bits(0x41f31687), f32::from_bits(0x426723d8)));
    // 30.418f, 58.8f, 30.364f, 57.893f, 30.386f, 57.785f
    path.line_to((f32::from_bits(0x41f36e97), f32::from_bits(0x4266ec8c)));
    // 30.429f, 57.731f
    path.cubic_to((f32::from_bits(0x41f3f3b6), f32::from_bits(0x4266b540)), (f32::from_bits(0x41f4d0e5), f32::from_bits(0x4266b540)), (f32::from_bits(0x41f58106), f32::from_bits(0x42669eb9)));
    // 30.494f, 57.677f, 30.602f, 57.677f, 30.688f, 57.655f
    path.cubic_to((f32::from_bits(0x41f7ed91), f32::from_bits(0x42663b65)), (f32::from_bits(0x41fac6a8), f32::from_bits(0x4265ee98)), (f32::from_bits(0x41fdb646), f32::from_bits(0x4265d811)));
    // 30.991f, 57.558f, 31.347f, 57.483f, 31.714f, 57.461f
    path.cubic_to((f32::from_bits(0x41fe51ec), f32::from_bits(0x4265c18a)), (f32::from_bits(0x41ff2f1b), f32::from_bits(0x4265d811)), (f32::from_bits(0x41ff872b), f32::from_bits(0x4265f8d6)));
    // 31.79f, 57.439f, 31.898f, 57.461f, 31.941f, 57.493f
    path.cubic_to((f32::from_bits(0x41ffb439), f32::from_bits(0x4266199b)), (f32::from_bits(0x41ffb439), f32::from_bits(0x42669eb9)), (f32::from_bits(0x41ffdf3b), f32::from_bits(0x4266d605)));
    // 31.963f, 57.525f, 31.963f, 57.655f, 31.984f, 57.709f
    path.cubic_to((f32::from_bits(0x41fff5c2), f32::from_bits(0x42670d51)), (f32::from_bits(0x42001cac), f32::from_bits(0x42675b24)), (f32::from_bits(0x42001cac), f32::from_bits(0x4267926f)));
    // 31.995f, 57.763f, 32.028f, 57.839f, 32.028f, 57.893f
    path.cubic_to((f32::from_bits(0x42003d71), f32::from_bits(0x42684290)), (f32::from_bits(0x420048b4), f32::from_bits(0x4268c7ae)), (f32::from_bits(0x42005e35), f32::from_bits(0x42696d92)));
    // 32.06f, 58.065f, 32.071f, 58.195f, 32.092f, 58.357f
    path.cubic_to((f32::from_bits(0x42008000), f32::from_bits(0x4269d0e6)), (f32::from_bits(0x4200ac08), f32::from_bits(0x426a5605)), (f32::from_bits(0x4200b74c), f32::from_bits(0x426acfe0)));
    // 32.125f, 58.454f, 32.168f, 58.584f, 32.179f, 58.703f
    path.cubic_to((f32::from_bits(0x4200c189), f32::from_bits(0x426b072c)), (f32::from_bits(0x4200b74c), f32::from_bits(0x426b49bb)), (f32::from_bits(0x42009581), f32::from_bits(0x426b6a80)));
    // 32.189f, 58.757f, 32.179f, 58.822f, 32.146f, 58.854f
    path.move_to((f32::from_bits(0x41eeae14), f32::from_bits(0x426bef9f)));
    // 29.835f, 58.984f
    path.cubic_to((f32::from_bits(0x41ee8312), f32::from_bits(0x426c26eb)), (f32::from_bits(0x41ed353f), f32::from_bits(0x426c52f3)), (f32::from_bits(0x41ecc8b4), f32::from_bits(0x426c73b8)));
    // 29.814f, 59.038f, 29.651f, 59.081f, 29.598f, 59.113f
    path.cubic_to((f32::from_bits(0x41eb7ae1), f32::from_bits(0x426cd70c)), (f32::from_bits(0x41ea3127), f32::from_bits(0x426d9376)), (f32::from_bits(0x41e96872), f32::from_bits(0x426e2e16)));
    // 29.435f, 59.21f, 29.274f, 59.394f, 29.176f, 59.545f
    path.cubic_to((f32::from_bits(0x41e88b43), f32::from_bits(0x426ed3f9)), (f32::from_bits(0x41e7c49b), f32::from_bits(0x426fdd31)), (f32::from_bits(0x41e6a5e3), f32::from_bits(0x4270570c)));
    // 29.068f, 59.707f, 28.971f, 59.966f, 28.831f, 60.085f
    path.cubic_to((f32::from_bits(0x41e678d5), f32::from_bits(0x427078d7)), (f32::from_bits(0x41e6624d), f32::from_bits(0x42706d93)), (f32::from_bits(0x41e620c4), f32::from_bits(0x427078d7)));
    // 28.809f, 60.118f, 28.798f, 60.107f, 28.766f, 60.118f
    path.cubic_to((f32::from_bits(0x41e60a3d), f32::from_bits(0x4270841b)), (f32::from_bits(0x41e5f3b6), f32::from_bits(0x4270999c)), (f32::from_bits(0x41e5f3b6), f32::from_bits(0x4270999c)));
    // 28.755f, 60.129f, 28.744f, 60.15f, 28.744f, 60.15f
    path.cubic_to((f32::from_bits(0x41e52d0e), f32::from_bits(0x4270d0e8)), (f32::from_bits(0x41e49374), f32::from_bits(0x4270e76f)), (f32::from_bits(0x41e39fbe), f32::from_bits(0x4270fcf0)));
    // 28.647f, 60.204f, 28.572f, 60.226f, 28.453f, 60.247f
    path.cubic_to((f32::from_bits(0x41e2c28f), f32::from_bits(0x42711377)), (f32::from_bits(0x41e1a1ca), f32::from_bits(0x42714ac3)), (f32::from_bits(0x41e03f7c), f32::from_bits(0x4271343c)));
    // 28.345f, 60.269f, 28.204f, 60.323f, 28.031f, 60.301f
    path.cubic_to((f32::from_bits(0x41de2d0d), f32::from_bits(0x42711377)), (f32::from_bits(0x41e0c49b), f32::from_bits(0x426e9caf)), (f32::from_bits(0x41e149b9), f32::from_bits(0x426e23da)));
    // 27.772f, 60.269f, 28.096f, 59.653f, 28.161f, 59.535f
    path.cubic_to((f32::from_bits(0x41e23d6f), f32::from_bits(0x426d2f1e)), (f32::from_bits(0x41e38936), f32::from_bits(0x426c52f5)), (f32::from_bits(0x41e4eb84), f32::from_bits(0x426b8109)));
    // 28.28f, 59.296f, 28.442f, 59.081f, 28.615f, 58.876f
    path.cubic_to((f32::from_bits(0x41e55a1c), f32::from_bits(0x426b49bd)), (f32::from_bits(0x41e5dd2e), f32::from_bits(0x426b1caf)), (f32::from_bits(0x41e6624d), f32::from_bits(0x426ae563)));
    // 28.669f, 58.822f, 28.733f, 58.778f, 28.798f, 58.724f
    path.cubic_to((f32::from_bits(0x41e78312), f32::from_bits(0x426a77d2)), (f32::from_bits(0x41e88b43), f32::from_bits(0x4269fcf0)), (f32::from_bits(0x41e99580), f32::from_bits(0x42698f5f)));
    // 28.939f, 58.617f, 29.068f, 58.497f, 29.198f, 58.39f
    path.cubic_to((f32::from_bits(0x41ea3126), f32::from_bits(0x42695813)), (f32::from_bits(0x41edd0e4), f32::from_bits(0x4267a7f2)), (f32::from_bits(0x41eeae13), f32::from_bits(0x42684292)));
    // 29.274f, 58.336f, 29.727f, 57.914f, 29.835f, 58.065f
    path.cubic_to((f32::from_bits(0x41eeae13), f32::from_bits(0x42684292)), (f32::from_bits(0x41eec49a), f32::from_bits(0x42684edc)), (f32::from_bits(0x41eec49a), f32::from_bits(0x42685919)));
    // 29.835f, 58.065f, 29.846f, 58.077f, 29.846f, 58.087f
    path.cubic_to((f32::from_bits(0x41ef0623), f32::from_bits(0x4268a6ec)), (f32::from_bits(0x41eedb21), f32::from_bits(0x426bb854)), (f32::from_bits(0x41eeae13), f32::from_bits(0x426befa0)));
    // 29.878f, 58.163f, 29.857f, 58.93f, 29.835f, 58.984f
    path.move_to((f32::from_bits(0x41eaf7cd), f32::from_bits(0x4258947d)));
    // 29.371f, 54.145f
    path.cubic_to((f32::from_bits(0x41ebd4fc), f32::from_bits(0x425873b8)), (f32::from_bits(0x41ed353e), f32::from_bits(0x42589fc1)), (f32::from_bits(0x41edba5c), f32::from_bits(0x4258ab04)));
    // 29.479f, 54.113f, 29.651f, 54.156f, 29.716f, 54.167f
    path.cubic_to((f32::from_bits(0x41ede76a), f32::from_bits(0x4259c9bc)), (f32::from_bits(0x41ee3f7b), f32::from_bits(0x425b6e9a)), (f32::from_bits(0x41ee126c), f32::from_bits(0x425c8314)));
    // 29.738f, 54.447f, 29.781f, 54.858f, 29.759f, 55.128f
    path.cubic_to((f32::from_bits(0x41ede76a), f32::from_bits(0x425d343b)), (f32::from_bits(0x41ee5602), f32::from_bits(0x425dda1e)), (f32::from_bits(0x41edd0e3), f32::from_bits(0x425e74be)));
    // 29.738f, 55.301f, 29.792f, 55.463f, 29.727f, 55.614f
    path.cubic_to((f32::from_bits(0x41ed624b), f32::from_bits(0x425f1aa1)), (f32::from_bits(0x41ec6e95), f32::from_bits(0x425f947c)), (f32::from_bits(0x41ebd4fc), f32::from_bits(0x426023d9)));
    // 29.673f, 55.776f, 29.554f, 55.895f, 29.479f, 56.035f
    path.cubic_to((f32::from_bits(0x41eb22cf), f32::from_bits(0x4260c9bc)), (f32::from_bits(0x41ea5c27), f32::from_bits(0x4261645c)), (f32::from_bits(0x41e9957f), f32::from_bits(0x42621583)));
    // 29.392f, 56.197f, 29.295f, 56.348f, 29.198f, 56.521f
    path.cubic_to((f32::from_bits(0x41e8e55e), f32::from_bits(0x4262c6aa)), (f32::from_bits(0x41e849b8), f32::from_bits(0x42638314)), (f32::from_bits(0x41e78310), f32::from_bits(0x426427f2)));
    // 29.112f, 56.694f, 29.036f, 56.878f, 28.939f, 57.039f
    path.cubic_to((f32::from_bits(0x41e72b00), f32::from_bits(0x42646b88)), (f32::from_bits(0x41e6e76a), f32::from_bits(0x4264b854)), (f32::from_bits(0x41e68f5a), f32::from_bits(0x4264efa0)));
    // 28.896f, 57.105f, 28.863f, 57.18f, 28.82f, 57.234f
    path.cubic_to((f32::from_bits(0x41e6624c), f32::from_bits(0x42651ba8)), (f32::from_bits(0x41e60a3b), f32::from_bits(0x4265322f)), (f32::from_bits(0x41e5dd2d), f32::from_bits(0x426552f4)));
    // 28.798f, 57.277f, 28.755f, 57.299f, 28.733f, 57.331f
    path.cubic_to((f32::from_bits(0x41e570a2), f32::from_bits(0x4264ad11)), (f32::from_bits(0x41e620c3), f32::from_bits(0x4263c49e)), (f32::from_bits(0x41e6624c), f32::from_bits(0x426329fe)));
    // 28.68f, 57.169f, 28.766f, 56.942f, 28.798f, 56.791f
    path.cubic_to((f32::from_bits(0x41e6a5e2), f32::from_bits(0x4262418b)), (f32::from_bits(0x41e6e76b), f32::from_bits(0x42617ae3)), (f32::from_bits(0x41e72b00), f32::from_bits(0x42609271)));
    // 28.831f, 56.564f, 28.863f, 56.37f, 28.896f, 56.143f
    path.cubic_to((f32::from_bits(0x41e75602), f32::from_bits(0x42604fe2)), (f32::from_bits(0x41e7978b), f32::from_bits(0x425fe250)), (f32::from_bits(0x41e7c49a), f32::from_bits(0x425f9fc1)));
    // 28.917f, 56.078f, 28.949f, 55.971f, 28.971f, 55.906f
    path.cubic_to((f32::from_bits(0x41e7db21), f32::from_bits(0x425f25e6)), (f32::from_bits(0x41e7db21), f32::from_bits(0x425ec18c)), (f32::from_bits(0x41e80623), f32::from_bits(0x425e53fa)));
    // 28.982f, 55.787f, 28.982f, 55.689f, 29.003f, 55.582f
    path.line_to((f32::from_bits(0x41e849b9), f32::from_bits(0x425e26ec)));
    // 29.036f, 55.538f
    path.cubic_to((f32::from_bits(0x41e874bb), f32::from_bits(0x425da2d3)), (f32::from_bits(0x41e8b851), f32::from_bits(0x425d28f8)), (f32::from_bits(0x41e8e55f), f32::from_bits(0x425caf1d)));
    // 29.057f, 55.409f, 29.09f, 55.29f, 29.112f, 55.171f
    path.cubic_to((f32::from_bits(0x41e93b63), f32::from_bits(0x425b8f5f)), (f32::from_bits(0x41e97ef9), f32::from_bits(0x425a7ae4)), (f32::from_bits(0x41ea0417), f32::from_bits(0x42596669)));
    // 29.154f, 54.89f, 29.187f, 54.62f, 29.252f, 54.35f
    path.cubic_to((f32::from_bits(0x41ea3125), f32::from_bits(0x4259199c)), (f32::from_bits(0x41ea5c27), f32::from_bits(0x4258ab05)), (f32::from_bits(0x41eaf7cd), f32::from_bits(0x4258947d)));
    // 29.274f, 54.275f, 29.295f, 54.167f, 29.371f, 54.145f
    path.move_to((f32::from_bits(0x41e96871), f32::from_bits(0x4256a2d3)));
    // 29.176f, 53.659f
    path.cubic_to((f32::from_bits(0x41e953f6), f32::from_bits(0x4256e562)), (f32::from_bits(0x41e96871), f32::from_bits(0x425727f2)), (f32::from_bits(0x41e93b63), f32::from_bits(0x42575f3d)));
    // 29.166f, 53.724f, 29.176f, 53.789f, 29.154f, 53.843f
    path.cubic_to((f32::from_bits(0x41e8fbe6), f32::from_bits(0x42578002)), (f32::from_bits(0x41e88b42), f32::from_bits(0x42578002)), (f32::from_bits(0x41e81cab), f32::from_bits(0x42578002)));
    // 29.123f, 53.875f, 29.068f, 53.875f, 29.014f, 53.875f
    path.cubic_to((f32::from_bits(0x41e7db22), f32::from_bits(0x42578002)), (f32::from_bits(0x41e78311), f32::from_bits(0x42576a81)), (f32::from_bits(0x41e75603), f32::from_bits(0x42575f3d)));
    // 28.982f, 53.875f, 28.939f, 53.854f, 28.917f, 53.843f
    path.cubic_to((f32::from_bits(0x41e72b01), f32::from_bits(0x4257322f)), (f32::from_bits(0x41e72b01), f32::from_bits(0x4257322f)), (f32::from_bits(0x41e72b01), f32::from_bits(0x4256fbe9)));
    // 28.896f, 53.799f, 28.896f, 53.799f, 28.896f, 53.746f
    path.cubic_to((f32::from_bits(0x41e72b01), f32::from_bits(0x4256b95a)), (f32::from_bits(0x41e78311), f32::from_bits(0x42564ac2)), (f32::from_bits(0x41e7978c), f32::from_bits(0x42561376)));
    // 28.896f, 53.681f, 28.939f, 53.573f, 28.949f, 53.519f
    path.cubic_to((f32::from_bits(0x41e7db22), f32::from_bits(0x4255570c)), (f32::from_bits(0x41e80624), f32::from_bits(0x4254b128)), (f32::from_bits(0x41e86040), f32::from_bits(0x42540b45)));
    // 28.982f, 53.335f, 29.003f, 53.173f, 29.047f, 53.011f
    path.cubic_to((f32::from_bits(0x41e874bb), f32::from_bits(0x4253cac2)), (f32::from_bits(0x41e86040), f32::from_bits(0x4253916a)), (f32::from_bits(0x41e8b850), f32::from_bits(0x42536562)));
    // 29.057f, 52.948f, 29.047f, 52.892f, 29.09f, 52.849f
    path.cubic_to((f32::from_bits(0x41e8ced7), f32::from_bits(0x42534fe1)), (f32::from_bits(0x41e953f6), f32::from_bits(0x42532e16)), (f32::from_bits(0x41e97ef8), f32::from_bits(0x42532e16)));
    // 29.101f, 52.828f, 29.166f, 52.795f, 29.187f, 52.795f
    path.cubic_to((f32::from_bits(0x41ea0417), f32::from_bits(0x425323d9)), (f32::from_bits(0x41ea3125), f32::from_bits(0x42534fe1)), (f32::from_bits(0x41ea72ae), f32::from_bits(0x42535a1e)));
    // 29.252f, 52.785f, 29.274f, 52.828f, 29.306f, 52.838f
    path.cubic_to((f32::from_bits(0x41ea72ae), f32::from_bits(0x42548520)), (f32::from_bits(0x41e9d708), f32::from_bits(0x4255a4df)), (f32::from_bits(0x41e96871), f32::from_bits(0x4256a2d2)));
    // 29.306f, 53.13f, 29.23f, 53.411f, 29.176f, 53.659f
    path.move_to((f32::from_bits(0x41e874bb), f32::from_bits(0x4258b647)));
    // 29.057f, 54.178f
    path.cubic_to((f32::from_bits(0x41e86040), f32::from_bits(0x42595c2a)), (f32::from_bits(0x41e849b9), f32::from_bits(0x4259bf7e)), (f32::from_bits(0x41e80623), f32::from_bits(0x425a4eda)));
    // 29.047f, 54.34f, 29.036f, 54.437f, 29.003f, 54.577f
    path.cubic_to((f32::from_bits(0x41e7db21), f32::from_bits(0x425ad3f9)), (f32::from_bits(0x41e76c89), f32::from_bits(0x425b8520)), (f32::from_bits(0x41e72b00), f32::from_bits(0x425c147c)));
    // 28.982f, 54.707f, 28.928f, 54.88f, 28.896f, 55.02f
    path.cubic_to((f32::from_bits(0x41e71479), f32::from_bits(0x425c570b)), (f32::from_bits(0x41e72b00), f32::from_bits(0x425c77d0)), (f32::from_bits(0x41e71479), f32::from_bits(0x425cba5f)));
    // 28.885f, 55.085f, 28.896f, 55.117f, 28.885f, 55.182f
    path.cubic_to((f32::from_bits(0x41e68f5a), f32::from_bits(0x425dfae2)), (f32::from_bits(0x41e5dd2d), f32::from_bits(0x425f676d)), (f32::from_bits(0x41e570a2), f32::from_bits(0x4260a8f7)));
    // 28.82f, 55.495f, 28.733f, 55.851f, 28.68f, 56.165f
    path.cubic_to((f32::from_bits(0x41e52d0c), f32::from_bits(0x42610c4b)), (f32::from_bits(0x41e55a1b), f32::from_bits(0x42614eda)), (f32::from_bits(0x41e52d0c), f32::from_bits(0x42619ba7)));
    // 28.647f, 56.262f, 28.669f, 56.327f, 28.647f, 56.402f
    path.cubic_to((f32::from_bits(0x41e51685), f32::from_bits(0x4261f4be)), (f32::from_bits(0x41e4be74), f32::from_bits(0x42624cce)), (f32::from_bits(0x41e4a7ed), f32::from_bits(0x42628f5d)));
    // 28.636f, 56.489f, 28.593f, 56.575f, 28.582f, 56.64f
    path.cubic_to((f32::from_bits(0x41e46664), f32::from_bits(0x42634bc7)), (f32::from_bits(0x41e43b62), f32::from_bits(0x4263e667)), (f32::from_bits(0x41e3f7cc), f32::from_bits(0x4264a1cc)));
    // 28.55f, 56.824f, 28.529f, 56.975f, 28.496f, 57.158f
    path.cubic_to((f32::from_bits(0x41e39fbc), f32::from_bits(0x42657efb)), (f32::from_bits(0x41e31a9d), f32::from_bits(0x42669376)), (f32::from_bits(0x41e2ac05), f32::from_bits(0x426770a5)));
    // 28.453f, 57.374f, 28.388f, 57.644f, 28.334f, 57.86f
    path.cubic_to((f32::from_bits(0x41e27ef7), f32::from_bits(0x426821cc)), (f32::from_bits(0x41e253f5), f32::from_bits(0x4268bc6c)), (f32::from_bits(0x41e2105f), f32::from_bits(0x42695812)));
    // 28.312f, 58.033f, 28.291f, 58.184f, 28.258f, 58.336f
    path.cubic_to((f32::from_bits(0x41e1ced6), f32::from_bits(0x4269f2b2)), (f32::from_bits(0x41e1082e), f32::from_bits(0x426aa3d9)), (f32::from_bits(0x41e09996), f32::from_bits(0x426b3335)));
    // 28.226f, 58.487f, 28.129f, 58.66f, 28.075f, 58.8f
    path.line_to((f32::from_bits(0x41e05600), f32::from_bits(0x426b3e79)));
    // 28.042f, 58.811f
    path.cubic_to((f32::from_bits(0x41dfe768), f32::from_bits(0x426bb854)), (f32::from_bits(0x41dfba5a), f32::from_bits(0x426c3129)), (f32::from_bits(0x41df4dcf), f32::from_bits(0x426ccccf)));
    // 27.988f, 58.93f, 27.966f, 59.048f, 27.913f, 59.2f
    path.cubic_to((f32::from_bits(0x41def5bf), f32::from_bits(0x426d50e8)), (f32::from_bits(0x41de5a19), f32::from_bits(0x426de14a)), (f32::from_bits(0x41ddeb81), f32::from_bits(0x426e70a6)));
    // 27.87f, 59.329f, 27.794f, 59.47f, 27.74f, 59.61f
    path.cubic_to((f32::from_bits(0x41dd3b60), f32::from_bits(0x426f79dd)), (f32::from_bits(0x41dd4fdb), f32::from_bits(0x426e1896)), (f32::from_bits(0x41dd6662), f32::from_bits(0x426db43c)));
    // 27.654f, 59.869f, 27.664f, 59.524f, 27.675f, 59.426f
    path.cubic_to((f32::from_bits(0x41de9ba1), f32::from_bits(0x426aa3da)), (f32::from_bits(0x41e01476), f32::from_bits(0x42679271)), (f32::from_bits(0x41e1332f), f32::from_bits(0x42648109)));
    // 27.826f, 58.66f, 28.01f, 57.893f, 28.15f, 57.126f
    path.cubic_to((f32::from_bits(0x41e149b6), f32::from_bits(0x42645f3e)), (f32::from_bits(0x41e1a1c7), f32::from_bits(0x4264072e)), (f32::from_bits(0x41e1a1c7), f32::from_bits(0x4263f1ad)));
    // 28.161f, 57.093f, 28.204f, 57.007f, 28.204f, 56.986f
    path.cubic_to((f32::from_bits(0x41e253f4), f32::from_bits(0x42626e9b)), (f32::from_bits(0x41e2c28c), f32::from_bits(0x42610109)), (f32::from_bits(0x41e3459e), f32::from_bits(0x425f72b3)));
    // 28.291f, 56.608f, 28.345f, 56.251f, 28.409f, 55.862f
    path.cubic_to((f32::from_bits(0x41e372ac), f32::from_bits(0x425f51ee)), (f32::from_bits(0x41e3b642), f32::from_bits(0x425ef9de)), (f32::from_bits(0x41e3b642), f32::from_bits(0x425ed813)));
    // 28.431f, 55.83f, 28.464f, 55.744f, 28.464f, 55.711f
    path.cubic_to((f32::from_bits(0x41e46663), f32::from_bits(0x425d76cb)), (f32::from_bits(0x41e4be73), f32::from_bits(0x425c3542)), (f32::from_bits(0x41e570a0), f32::from_bits(0x425ad3fa)));
    // 28.55f, 55.366f, 28.593f, 55.052f, 28.68f, 54.707f
    path.cubic_to((f32::from_bits(0x41e570a0), f32::from_bits(0x425a916b)), (f32::from_bits(0x41e5dd2b), f32::from_bits(0x425a22d3)), (f32::from_bits(0x41e5f3b2), f32::from_bits(0x4259e044)));
    // 28.68f, 54.642f, 28.733f, 54.534f, 28.744f, 54.469f
    path.cubic_to((f32::from_bits(0x41e620c0), f32::from_bits(0x42595c2b)), (f32::from_bits(0x41e60a39), f32::from_bits(0x4258ab05)), (f32::from_bits(0x41e72afe), f32::from_bits(0x4258947d)));
    // 28.766f, 54.34f, 28.755f, 54.167f, 28.896f, 54.145f
    path.cubic_to((f32::from_bits(0x41e79789), f32::from_bits(0x4258947d)), (f32::from_bits(0x41e80621), f32::from_bits(0x4258ab04)), (f32::from_bits(0x41e874b8), f32::from_bits(0x4258b648)));
    // 28.949f, 54.145f, 29.003f, 54.167f, 29.057f, 54.178f
    path.move_to((f32::from_bits(0x41e5b229), f32::from_bits(0x4256a2d3)));
    // 28.712f, 53.659f
    path.cubic_to((f32::from_bits(0x41e5851b), f32::from_bits(0x4256e562)), (f32::from_bits(0x41e59ba2), f32::from_bits(0x425727f2)), (f32::from_bits(0x41e570a0), f32::from_bits(0x42575f3d)));
    // 28.69f, 53.724f, 28.701f, 53.789f, 28.68f, 53.843f
    path.cubic_to((f32::from_bits(0x41e52d0a), f32::from_bits(0x42578002)), (f32::from_bits(0x41e4a7ec), f32::from_bits(0x42579689)), (f32::from_bits(0x41e43b61), f32::from_bits(0x42578002)));
    // 28.647f, 53.875f, 28.582f, 53.897f, 28.529f, 53.875f
    path.cubic_to((f32::from_bits(0x41e3f7cb), f32::from_bits(0x42578002)), (f32::from_bits(0x41e39fbb), f32::from_bits(0x425748b6)), (f32::from_bits(0x41e3459e), f32::from_bits(0x42573e79)));
    // 28.496f, 53.875f, 28.453f, 53.821f, 28.409f, 53.811f
    path.cubic_to((f32::from_bits(0x41e39fbb), f32::from_bits(0x42566044)), (f32::from_bits(0x41e40e52), f32::from_bits(0x42558e58)), (f32::from_bits(0x41e47add), f32::from_bits(0x4254c7b0)));
    // 28.453f, 53.594f, 28.507f, 53.389f, 28.56f, 53.195f
    path.cubic_to((f32::from_bits(0x41e49370), f32::from_bits(0x425479dd)), (f32::from_bits(0x41e49370), f32::from_bits(0x42541689)), (f32::from_bits(0x41e4eb81), f32::from_bits(0x4253df3d)));
    // 28.572f, 53.119f, 28.572f, 53.022f, 28.615f, 52.968f
    path.cubic_to((f32::from_bits(0x41e4fffc), f32::from_bits(0x4253c9bc)), (f32::from_bits(0x41e5b229), f32::from_bits(0x4253916a)), (f32::from_bits(0x41e60a39), f32::from_bits(0x4253916a)));
    // 28.625f, 52.947f, 28.712f, 52.892f, 28.755f, 52.892f
    path.cubic_to((f32::from_bits(0x41e68f58), f32::from_bits(0x4253872d)), (f32::from_bits(0x41e68f58), f32::from_bits(0x4253a7f1)), (f32::from_bits(0x41e6e768), f32::from_bits(0x4253be78)));
    // 28.82f, 52.882f, 28.82f, 52.914f, 28.863f, 52.936f
    path.cubic_to((f32::from_bits(0x41e68f58), f32::from_bits(0x4254c7af)), (f32::from_bits(0x41e60a39), f32::from_bits(0x4255af1c)), (f32::from_bits(0x41e5b229), f32::from_bits(0x4256a2d2)));
    // 28.82f, 53.195f, 28.755f, 53.421f, 28.712f, 53.659f
    path.move_to((f32::from_bits(0x41e372ac), f32::from_bits(0x42589fc0)));
    // 28.431f, 54.156f
    path.cubic_to((f32::from_bits(0x41e55a19), f32::from_bits(0x42586874)), (f32::from_bits(0x41e40e52), f32::from_bits(0x425a178f)), (f32::from_bits(0x41e3cabc), f32::from_bits(0x425a7ae3)));
    // 28.669f, 54.102f, 28.507f, 54.523f, 28.474f, 54.62f
    path.cubic_to((f32::from_bits(0x41e1fbe3), f32::from_bits(0x425f3b66)), (f32::from_bits(0x41dfd0e1), f32::from_bits(0x4263f1ac)), (f32::from_bits(0x41ddeb81), f32::from_bits(0x4268c7b0)));
    // 28.248f, 55.808f, 27.977f, 56.986f, 27.74f, 58.195f
    path.cubic_to((f32::from_bits(0x41ddd4fa), f32::from_bits(0x42690a3f)), (f32::from_bits(0x41dd7ce9), f32::from_bits(0x42696d93)), (f32::from_bits(0x41dd6662), f32::from_bits(0x4269999c)));
    // 27.729f, 58.26f, 27.686f, 58.357f, 27.675f, 58.4f
    path.cubic_to((f32::from_bits(0x41dd3b60), f32::from_bits(0x426a29fe)), (f32::from_bits(0x41dd3b60), f32::from_bits(0x426a8d52)), (f32::from_bits(0x41dcf7ca), f32::from_bits(0x426b1cae)));
    // 27.654f, 58.541f, 27.654f, 58.638f, 27.621f, 58.778f
    path.cubic_to((f32::from_bits(0x41dcb641), f32::from_bits(0x426bf9dd)), (f32::from_bits(0x41dc0414), f32::from_bits(0x426cf8d7)), (f32::from_bits(0x41db957c), f32::from_bits(0x426dec8d)));
    // 27.589f, 58.994f, 27.502f, 59.243f, 27.448f, 59.481f
    path.cubic_to((f32::from_bits(0x41db53f3), f32::from_bits(0x426e916a)), (f32::from_bits(0x41db3d6c), f32::from_bits(0x426eea81)), (f32::from_bits(0x41daa3d2), f32::from_bits(0x426f5918)));
    // 27.416f, 59.642f, 27.405f, 59.729f, 27.33f, 59.837f
    path.cubic_to((f32::from_bits(0x41da76c4), f32::from_bits(0x426f4dd4)), (f32::from_bits(0x41da49b5), f32::from_bits(0x426f4291)), (f32::from_bits(0x41da082c), f32::from_bits(0x426f21cc)));
    // 27.308f, 59.826f, 27.286f, 59.815f, 27.254f, 59.783f
    path.cubic_to((f32::from_bits(0x41d9db1e), f32::from_bits(0x426f0b45)), (f32::from_bits(0x41d9f1a5), f32::from_bits(0x426f0b45)), (f32::from_bits(0x41d9c6a3), f32::from_bits(0x426eea80)));
    // 27.232f, 59.761f, 27.243f, 59.761f, 27.222f, 59.729f
    path.line_to((f32::from_bits(0x41d99995), f32::from_bits(0x426edf3c)));
    // 27.2f, 59.718f
    path.cubic_to((f32::from_bits(0x41d91476), f32::from_bits(0x426ea7f0)), (f32::from_bits(0x41d8e768), f32::from_bits(0x426e6561)), (f32::from_bits(0x41d8a5df), f32::from_bits(0x426e020d)));
    // 27.135f, 59.664f, 27.113f, 59.599f, 27.081f, 59.502f
    path.cubic_to((f32::from_bits(0x41d8a5df), f32::from_bits(0x426e020d)), (f32::from_bits(0x41d86456), f32::from_bits(0x426dd605)), (f32::from_bits(0x41d8a5df), f32::from_bits(0x426dd605)));
    // 27.081f, 59.502f, 27.049f, 59.459f, 27.081f, 59.459f
    path.cubic_to((f32::from_bits(0x41d8e768), f32::from_bits(0x426d5c2a)), (f32::from_bits(0x41d8fdef), f32::from_bits(0x426cf8d6)), (f32::from_bits(0x41d92afe), f32::from_bits(0x426c7efb)));
    // 27.113f, 59.34f, 27.124f, 59.243f, 27.146f, 59.124f
    path.cubic_to((f32::from_bits(0x41d9830e), f32::from_bits(0x426bb853)), (f32::from_bits(0x41da1eb4), f32::from_bits(0x426ae561)), (f32::from_bits(0x41da8d4c), f32::from_bits(0x426a29fd)));
    // 27.189f, 58.93f, 27.265f, 58.724f, 27.319f, 58.541f
    path.cubic_to((f32::from_bits(0x41dccabd), f32::from_bits(0x4265d811)), (f32::from_bits(0x41e02afe), f32::from_bits(0x42617ae2)), (f32::from_bits(0x41e1332f), f32::from_bits(0x425cfcef)));
    // 27.599f, 57.461f, 28.021f, 56.37f, 28.15f, 55.247f
    path.cubic_to((f32::from_bits(0x41e149b6), f32::from_bits(0x425c4085)), (f32::from_bits(0x41e1a1c7), f32::from_bits(0x425b8f5e)), (f32::from_bits(0x41e1fbe3), f32::from_bits(0x425adf3d)));
    // 28.161f, 55.063f, 28.204f, 54.89f, 28.248f, 54.718f
    path.cubic_to((f32::from_bits(0x41e226e5), f32::from_bits(0x425a4edb)), (f32::from_bits(0x41e226e5), f32::from_bits(0x42598833)), (f32::from_bits(0x41e2ac04), f32::from_bits(0x4258f7d0)));
    // 28.269f, 54.577f, 28.269f, 54.383f, 28.334f, 54.242f
    path.cubic_to((f32::from_bits(0x41e2c28b), f32::from_bits(0x4258ec8c)), (f32::from_bits(0x41e372ac), f32::from_bits(0x42589fc0)), (f32::from_bits(0x41e372ac), f32::from_bits(0x42589fc0)));
    // 28.345f, 54.231f, 28.431f, 54.156f, 28.431f, 54.156f
    path.move_to((f32::from_bits(0x41d9830e), f32::from_bits(0x427128f7)));
    // 27.189f, 60.29f
    path.cubic_to((f32::from_bits(0x41d95600), f32::from_bits(0x42714ac2)), (f32::from_bits(0x41d92afe), f32::from_bits(0x427176ca)), (f32::from_bits(0x41d8e768), f32::from_bits(0x427176ca)));
    // 27.167f, 60.323f, 27.146f, 60.366f, 27.113f, 60.366f
    path.cubic_to((f32::from_bits(0x41d86456), f32::from_bits(0x42718d51)), (f32::from_bits(0x41d67ce9), f32::from_bits(0x4271820e)), (f32::from_bits(0x41d60e51), f32::from_bits(0x42716b86)));
    // 27.049f, 60.388f, 26.811f, 60.377f, 26.757f, 60.355f
    path.cubic_to((f32::from_bits(0x41d5f7ca), f32::from_bits(0x42716b86)), (f32::from_bits(0x41d5ccc8), f32::from_bits(0x42714ac1)), (f32::from_bits(0x41d5b641), f32::from_bits(0x42713f7e)));
    // 26.746f, 60.355f, 26.725f, 60.323f, 26.714f, 60.312f
    path.cubic_to((f32::from_bits(0x41d5b641), f32::from_bits(0x42708e57)), (f32::from_bits(0x41d5f7ca), f32::from_bits(0x426ffefb)), (f32::from_bits(0x41d69370), f32::from_bits(0x426f8f5d)));
    // 26.714f, 60.139f, 26.746f, 59.999f, 26.822f, 59.89f
    path.cubic_to((f32::from_bits(0x41d6eb80), f32::from_bits(0x426f9ba7)), (f32::from_bits(0x41d7188f), f32::from_bits(0x426f8f5d)), (f32::from_bits(0x41d7709f), f32::from_bits(0x426f9ba7)));
    // 26.865f, 59.902f, 26.887f, 59.89f, 26.93f, 59.902f
    path.cubic_to((f32::from_bits(0x41d7b228), f32::from_bits(0x426fb128)), (f32::from_bits(0x41d99995), f32::from_bits(0x42706d93)), (f32::from_bits(0x41d9c6a3), f32::from_bits(0x42708e57)));
    // 26.962f, 59.923f, 27.2f, 60.107f, 27.222f, 60.139f
    path.cubic_to((f32::from_bits(0x41d9db1e), f32::from_bits(0x4270d0e6)), (f32::from_bits(0x41d99995), f32::from_bits(0x42710832)), (f32::from_bits(0x41d9830d), f32::from_bits(0x427128f7)));
    // 27.232f, 60.204f, 27.2f, 60.258f, 27.189f, 60.29f
    path.move_to((f32::from_bits(0x41e1603c), f32::from_bits(0x4255f1ab)));
    // 28.172f, 53.486f
    path.cubic_to((f32::from_bits(0x41e149b5), f32::from_bits(0x42563f7e)), (f32::from_bits(0x41e1603c), f32::from_bits(0x425676ca)), (f32::from_bits(0x41e1332e), f32::from_bits(0x4256c49d)));
    // 28.161f, 53.562f, 28.172f, 53.616f, 28.15f, 53.692f
    path.cubic_to((f32::from_bits(0x41e11eb3), f32::from_bits(0x4256f0a5)), (f32::from_bits(0x41e0db1e), f32::from_bits(0x425727f1)), (f32::from_bits(0x41e0b01c), f32::from_bits(0x425748b6)));
    // 28.14f, 53.735f, 28.107f, 53.789f, 28.086f, 53.821f
    path.line_to((f32::from_bits(0x41e055ff), f32::from_bits(0x425748b6)));
    // 28.042f, 53.821f
    path.cubic_to((f32::from_bits(0x41e055ff), f32::from_bits(0x425748b6)), (f32::from_bits(0x41df4dce), f32::from_bits(0x4256e562)), (f32::from_bits(0x41df3747), f32::from_bits(0x4256da1e)));
    // 28.042f, 53.821f, 27.913f, 53.724f, 27.902f, 53.713f
    path.cubic_to((f32::from_bits(0x41deb228), f32::from_bits(0x4256820e)), (f32::from_bits(0x41de4391), f32::from_bits(0x42561376)), (f32::from_bits(0x41ddbe72), f32::from_bits(0x4255ba60)));
    // 27.837f, 53.627f, 27.783f, 53.519f, 27.718f, 53.432f
    path.line_to((f32::from_bits(0x41dd7ce9), f32::from_bits(0x4255af1c)));
    // 27.686f, 53.421f
    path.cubic_to((f32::from_bits(0x41dd7ce9), f32::from_bits(0x4255a4df)), (f32::from_bits(0x41dda7eb), f32::from_bits(0x425578d6)), (f32::from_bits(0x41dd7ce9), f32::from_bits(0x42556d93)));
    // 27.686f, 53.411f, 27.707f, 53.368f, 27.686f, 53.357f
    path.cubic_to((f32::from_bits(0x41de9ba1), f32::from_bits(0x4255147c)), (f32::from_bits(0x41df8f58), f32::from_bits(0x4254c7b0)), (f32::from_bits(0x41e0b01c), f32::from_bits(0x42549064)));
    // 27.826f, 53.27f, 27.945f, 53.195f, 28.086f, 53.141f
    path.cubic_to((f32::from_bits(0x41e0c497), f32::from_bits(0x42548520)), (f32::from_bits(0x41e11eb4), f32::from_bits(0x4254645c)), (f32::from_bits(0x41e1332e), f32::from_bits(0x4254645c)));
    // 28.096f, 53.13f, 28.14f, 53.098f, 28.15f, 53.098f
    path.cubic_to((f32::from_bits(0x41e18b3e), f32::from_bits(0x42545918)), (f32::from_bits(0x41e1ced4), f32::from_bits(0x425479dd)), (f32::from_bits(0x41e1fbe2), f32::from_bits(0x425479dd)));
    // 28.193f, 53.087f, 28.226f, 53.119f, 28.248f, 53.119f
    path.cubic_to((f32::from_bits(0x41e1fbe2), f32::from_bits(0x4255147d)), (f32::from_bits(0x41e1a1c5), f32::from_bits(0x4255841a)), (f32::from_bits(0x41e1603c), f32::from_bits(0x4255f1ac)));
    // 28.248f, 53.27f, 28.204f, 53.379f, 28.172f, 53.486f
    path.move_to((f32::from_bits(0x41df6248), f32::from_bits(0x425b4ccf)));
    // 27.923f, 54.825f
    path.cubic_to((f32::from_bits(0x41dfe767), f32::from_bits(0x425b9aa2)), (f32::from_bits(0x41df4dcd), f32::from_bits(0x425c6c8d)), (f32::from_bits(0x41df20bf), f32::from_bits(0x425cd0e8)));
    // 27.988f, 54.901f, 27.913f, 55.106f, 27.891f, 55.204f
    path.cubic_to((f32::from_bits(0x41ddeb80), f32::from_bits(0x425f893a)), (f32::from_bits(0x41dc8932), f32::from_bits(0x4262374e)), (f32::from_bits(0x41db105d), f32::from_bits(0x4264e45d)));
    // 27.74f, 55.884f, 27.567f, 56.554f, 27.383f, 57.223f
    path.cubic_to((f32::from_bits(0x41daced4), f32::from_bits(0x42657efd)), (f32::from_bits(0x41d78726), f32::from_bits(0x426c52f5)), (f32::from_bits(0x41d6c07e), f32::from_bits(0x426c3c6d)));
    // 27.351f, 57.374f, 26.941f, 59.081f, 26.844f, 59.059f
    path.cubic_to((f32::from_bits(0x41d58932), f32::from_bits(0x426c3129)), (f32::from_bits(0x41d50620), f32::from_bits(0x426b1caf)), (f32::from_bits(0x41d48101), f32::from_bits(0x426aa3da)));
    // 26.692f, 59.048f, 26.628f, 58.778f, 26.563f, 58.66f
    path.cubic_to((f32::from_bits(0x41d3d0e0), f32::from_bits(0x426a0834)), (f32::from_bits(0x41d34bc2), f32::from_bits(0x42696d94)), (f32::from_bits(0x41d2db1e), f32::from_bits(0x4268bc6d)));
    // 26.477f, 58.508f, 26.412f, 58.357f, 26.357f, 58.184f
    path.cubic_to((f32::from_bits(0x41d21476), f32::from_bits(0x42674fe2)), (f32::from_bits(0x41d19164), f32::from_bits(0x4265c18c)), (f32::from_bits(0x41d19164), f32::from_bits(0x426449bd)));
    // 26.26f, 57.828f, 26.196f, 57.439f, 26.196f, 57.072f
    path.cubic_to((f32::from_bits(0x41d1a5df), f32::from_bits(0x4261bd73)), (f32::from_bits(0x41d3d0e1), f32::from_bits(0x425f51ee)), (f32::from_bits(0x41d79dae), f32::from_bits(0x425d820f)));
    // 26.206f, 56.435f, 26.477f, 55.83f, 26.952f, 55.377f
    path.cubic_to((f32::from_bits(0x41d8a5df), f32::from_bits(0x425cfcf0)), (f32::from_bits(0x41d9db1f), f32::from_bits(0x425c8e59)), (f32::from_bits(0x41db105e), f32::from_bits(0x425c3542)));
    // 27.081f, 55.247f, 27.232f, 55.139f, 27.383f, 55.052f
    path.cubic_to((f32::from_bits(0x41dbed8d), f32::from_bits(0x425bf4bf)), (f32::from_bits(0x41ddbe72), f32::from_bits(0x425b21cd)), (f32::from_bits(0x41dec8b0), f32::from_bits(0x425b21cd)));
    // 27.491f, 54.989f, 27.718f, 54.783f, 27.848f, 54.783f
    path.cubic_to((f32::from_bits(0x41df20c0), f32::from_bits(0x425b374e)), (f32::from_bits(0x41df4dcf), f32::from_bits(0x425b4292)), (f32::from_bits(0x41df624a), f32::from_bits(0x425b4ccf)));
    // 27.891f, 54.804f, 27.913f, 54.815f, 27.923f, 54.825f
    path.move_to((f32::from_bits(0x41d453f4), f32::from_bits(0x426fbc6d)));
    // 26.541f, 59.934f
    path.cubic_to((f32::from_bits(0x41d48102), f32::from_bits(0x426f8521)), (f32::from_bits(0x41d51a9c), f32::from_bits(0x426ea7f2)), (f32::from_bits(0x41d4957d), f32::from_bits(0x426e872e)));
    // 26.563f, 59.88f, 26.638f, 59.664f, 26.573f, 59.632f
    path.cubic_to((f32::from_bits(0x41d453f4), f32::from_bits(0x426e70a7)), (f32::from_bits(0x41d428f2), f32::from_bits(0x426e872e)), (f32::from_bits(0x41d3fbe3), f32::from_bits(0x426e70a7)));
    // 26.541f, 59.61f, 26.52f, 59.632f, 26.498f, 59.61f
    path.cubic_to((f32::from_bits(0x41d3d0e1), f32::from_bits(0x426e916c)), (f32::from_bits(0x41d3b84d), f32::from_bits(0x426e872e)), (f32::from_bits(0x41d3a3d3), f32::from_bits(0x426e916c)));
    // 26.477f, 59.642f, 26.465f, 59.632f, 26.455f, 59.642f
    path.cubic_to((f32::from_bits(0x41d3603d), f32::from_bits(0x426f010a)), (f32::from_bits(0x41d3d0e1), f32::from_bits(0x426f9ba9)), (f32::from_bits(0x41d4126b), f32::from_bits(0x426fdd33)));
    // 26.422f, 59.751f, 26.477f, 59.902f, 26.509f, 59.966f
    path.line_to((f32::from_bits(0x41d43d6d), f32::from_bits(0x426fdd33)));
    // 26.53f, 59.966f
    path.cubic_to((f32::from_bits(0x41d43d6d), f32::from_bits(0x426fd1ef)), (f32::from_bits(0x41d43d6d), f32::from_bits(0x426fbc6e)), (f32::from_bits(0x41d453f4), f32::from_bits(0x426fbc6e)));
    // 26.53f, 59.955f, 26.53f, 59.934f, 26.541f, 59.934f
    path.move_to((f32::from_bits(0x42071ba4), f32::from_bits(0x42670210)));
    // 33.777f, 57.752f
    path.cubic_to((f32::from_bits(0x42075e33), f32::from_bits(0x42670d54)), (f32::from_bits(0x4207957f), f32::from_bits(0x42671897)), (f32::from_bits(0x4207cccb), f32::from_bits(0x42672f1e)));
    // 33.842f, 57.763f, 33.896f, 57.774f, 33.95f, 57.796f
    path.cubic_to((f32::from_bits(0x4208a9fa), f32::from_bits(0x4267872e)), (f32::from_bits(0x42097be6), f32::from_bits(0x42681791)), (f32::from_bits(0x420a3850), f32::from_bits(0x42688522)));
    // 34.166f, 57.882f, 34.371f, 58.023f, 34.555f, 58.13f
    path.cubic_to((f32::from_bits(0x420b0a3c), f32::from_bits(0x4268fefd)), (f32::from_bits(0x420d1167), f32::from_bits(0x4269e770)), (f32::from_bits(0x420d27ee), f32::from_bits(0x426ae564)));
    // 34.76f, 58.249f, 35.267f, 58.476f, 35.289f, 58.724f
    path.cubic_to((f32::from_bits(0x420d0729), f32::from_bits(0x426af0a8)), (f32::from_bits(0x420cdb21), f32::from_bits(0x426afbeb)), (f32::from_bits(0x420cb956), f32::from_bits(0x426b072f)));
    // 35.257f, 58.735f, 35.214f, 58.746f, 35.181f, 58.757f
    path.cubic_to((f32::from_bits(0x420b9998), f32::from_bits(0x426b27f4)), (f32::from_bits(0x420a6f9c), f32::from_bits(0x426b27f4)), (f32::from_bits(0x42095b21), f32::from_bits(0x426b3337)));
    // 34.9f, 58.789f, 34.609f, 58.789f, 34.339f, 58.8f
    path.cubic_to((f32::from_bits(0x42090d4e), f32::from_bits(0x426b3337)), (f32::from_bits(0x4207b644), f32::from_bits(0x426b49be)), (f32::from_bits(0x420773b4), f32::from_bits(0x426b3337)));
    // 34.263f, 58.8f, 33.928f, 58.822f, 33.863f, 58.8f
    path.cubic_to((f32::from_bits(0x4207322b), f32::from_bits(0x426b072f)), (f32::from_bits(0x4206ef9b), f32::from_bits(0x4269999d)), (f32::from_bits(0x4206ced7), f32::from_bits(0x426920c8)));
    // 33.799f, 58.757f, 33.734f, 58.4f, 33.702f, 58.282f
    path.cubic_to((f32::from_bits(0x42069685), f32::from_bits(0x4268645e)), (f32::from_bits(0x4205c49a), f32::from_bits(0x4266b543)), (f32::from_bits(0x42071ba4), f32::from_bits(0x42670210)));
    // 33.647f, 58.098f, 33.442f, 57.677f, 33.777f, 57.752f
    path.move_to((f32::from_bits(0x41f026e6), f32::from_bits(0x423f0c4e)));
    // 30.019f, 47.762f
    path.cubic_to((f32::from_bits(0x41effbe4), f32::from_bits(0x42400004)), (f32::from_bits(0x41f0105f), f32::from_bits(0x4240e877)), (f32::from_bits(0x41f03b61), f32::from_bits(0x4241d0ea)));
    // 29.998f, 48, 30.008f, 48.227f, 30.029f, 48.454f
    path.line_to((f32::from_bits(0x41f03b61), f32::from_bits(0x424228fa)));
    // 30.029f, 48.54f
    path.cubic_to((f32::from_bits(0x41f051e8), f32::from_bits(0x42423f81)), (f32::from_bits(0x41f0c080), f32::from_bits(0x424276cd)), (f32::from_bits(0x41f11890), f32::from_bits(0x424276cd)));
    // 30.04f, 48.562f, 30.094f, 48.616f, 30.137f, 48.616f
    path.cubic_to((f32::from_bits(0x41f11890), f32::from_bits(0x424276cd)), (f32::from_bits(0x41f18934), f32::from_bits(0x42426b89)), (f32::from_bits(0x41f12f17), f32::from_bits(0x42426b89)));
    // 30.137f, 48.616f, 30.192f, 48.605f, 30.148f, 48.605f
    path.cubic_to((f32::from_bits(0x41f1459e), f32::from_bits(0x42426045)), (f32::from_bits(0x41f18934), f32::from_bits(0x42426b89)), (f32::from_bits(0x41f18934), f32::from_bits(0x42426b89)));
    // 30.159f, 48.594f, 30.192f, 48.605f, 30.192f, 48.605f
    path.cubic_to((f32::from_bits(0x41f19daf), f32::from_bits(0x424249be)), (f32::from_bits(0x41f19daf), f32::from_bits(0x42423f81)), (f32::from_bits(0x41f1b436), f32::from_bits(0x42423f81)));
    // 30.202f, 48.572f, 30.202f, 48.562f, 30.213f, 48.562f
    path.cubic_to((f32::from_bits(0x41f18934), f32::from_bits(0x42414087)), (f32::from_bits(0x41f11890), f32::from_bits(0x424079df)), (f32::from_bits(0x41f0ac05), f32::from_bits(0x423f9cb0)));
    // 30.192f, 48.313f, 30.137f, 48.119f, 30.084f, 47.903f
    path.cubic_to((f32::from_bits(0x41f0957e), f32::from_bits(0x423f7ae5)), (f32::from_bits(0x41f0c080), f32::from_bits(0x423f5a21)), (f32::from_bits(0x41f0957e), f32::from_bits(0x423f395c)));
    // 30.073f, 47.87f, 30.094f, 47.838f, 30.073f, 47.806f
    path.line_to((f32::from_bits(0x41f026e6), f32::from_bits(0x423f0c4e)));
    // 30.019f, 47.762f
    path.move_to((f32::from_bits(0x41ed4dcf), f32::from_bits(0x423fd3fc)));
    // 29.663f, 47.957f
    path.cubic_to((f32::from_bits(0x41ecc8b0), f32::from_bits(0x42408523)), (f32::from_bits(0x41ec580c), f32::from_bits(0x42414bcb)), (f32::from_bits(0x41ec580c), f32::from_bits(0x42423f81)));
    // 29.598f, 48.13f, 29.543f, 48.324f, 29.543f, 48.562f
    path.cubic_to((f32::from_bits(0x41ec6e93), f32::from_bits(0x42423f81)), (f32::from_bits(0x41ec9ba2), f32::from_bits(0x42426046)), (f32::from_bits(0x41ecb229), f32::from_bits(0x42426b89)));
    // 29.554f, 48.562f, 29.576f, 48.594f, 29.587f, 48.605f
    path.cubic_to((f32::from_bits(0x41ecc8b0), f32::from_bits(0x42426b89)), (f32::from_bits(0x41ecdd2b), f32::from_bits(0x42426045)), (f32::from_bits(0x41ecf3b2), f32::from_bits(0x42426b89)));
    // 29.598f, 48.605f, 29.608f, 48.594f, 29.619f, 48.605f
    path.cubic_to((f32::from_bits(0x41ecf3b2), f32::from_bits(0x42426b89)), (f32::from_bits(0x41eda5df), f32::from_bits(0x42426045)), (f32::from_bits(0x41edba5a), f32::from_bits(0x42423f81)));
    // 29.619f, 48.605f, 29.706f, 48.594f, 29.716f, 48.562f
    path.cubic_to((f32::from_bits(0x41ee126a), f32::from_bits(0x4241e66a)), (f32::from_bits(0x41edd0e1), f32::from_bits(0x42403750)), (f32::from_bits(0x41eda5df), f32::from_bits(0x423fdf3f)));
    // 29.759f, 48.475f, 29.727f, 48.054f, 29.706f, 47.968f
    path.line_to((f32::from_bits(0x41ed4dcf), f32::from_bits(0x423fd3fb)));
    // 29.663f, 47.957f
    path.move_to((f32::from_bits(0x41d05a19), f32::from_bits(0x4258ab05)));
    // 26.044f, 54.167f
    path.cubic_to((f32::from_bits(0x41d05a19), f32::from_bits(0x42589fc1)), (f32::from_bits(0x41d070a0), f32::from_bits(0x42588a40)), (f32::from_bits(0x41d05a19), f32::from_bits(0x42586876)));
    // 26.044f, 54.156f, 26.055f, 54.135f, 26.044f, 54.102f
    path.cubic_to((f32::from_bits(0x41d05a19), f32::from_bits(0x42583c6e)), (f32::from_bits(0x41d02f17), f32::from_bits(0x4257ee9b)), (f32::from_bits(0x41d00209), f32::from_bits(0x4257c293)));
    // 26.044f, 54.059f, 26.023f, 53.983f, 26.001f, 53.94f
    path.cubic_to((f32::from_bits(0x41cfeb82), f32::from_bits(0x42571cb0)), (f32::from_bits(0x41d00209), f32::from_bits(0x42568210)), (f32::from_bits(0x41cfeb82), f32::from_bits(0x4255c5a5)));
    // 25.99f, 53.778f, 26.001f, 53.627f, 25.99f, 53.443f
    path.cubic_to((f32::from_bits(0x41cfeb82), f32::from_bits(0x4255a4e0)), (f32::from_bits(0x41cfc080), f32::from_bits(0x42552b05)), (f32::from_bits(0x41cfd4fb), f32::from_bits(0x4254dd32)));
    // 25.99f, 53.411f, 25.969f, 53.292f, 25.979f, 53.216f
    path.cubic_to((f32::from_bits(0x41cfeb82), f32::from_bits(0x4254b12a)), (f32::from_bits(0x41d05a1a), f32::from_bits(0x4254b12a)), (f32::from_bits(0x41d0df38), f32::from_bits(0x4254c7b1)));
    // 25.99f, 53.173f, 26.044f, 53.173f, 26.109f, 53.195f
    path.cubic_to((f32::from_bits(0x41d24186), f32::from_bits(0x42552b05)), (f32::from_bits(0x41d4ac05), f32::from_bits(0x42563f80)), (f32::from_bits(0x41d50621), f32::from_bits(0x42566044)));
    // 26.282f, 53.292f, 26.584f, 53.562f, 26.628f, 53.594f
    path.cubic_to((f32::from_bits(0x41d60e52), f32::from_bits(0x4256da1f)), (f32::from_bits(0x41d70208), f32::from_bits(0x425748b7)), (f32::from_bits(0x41d80a3a), f32::from_bits(0x4257c292)));
    // 26.757f, 53.713f, 26.876f, 53.821f, 27.005f, 53.94f
    path.cubic_to((f32::from_bits(0x41d8a5e0), f32::from_bits(0x4257f9de)), (f32::from_bits(0x41da1eb5), f32::from_bits(0x4258947e)), (f32::from_bits(0x41d8a5e0), f32::from_bits(0x4258ab05)));
    // 27.081f, 53.994f, 27.265f, 54.145f, 27.081f, 54.167f
    path.cubic_to((f32::from_bits(0x41d7df38), f32::from_bits(0x4258cbca)), (f32::from_bits(0x41d72d0b), f32::from_bits(0x4258b649)), (f32::from_bits(0x41d66663), f32::from_bits(0x4258b649)));
    // 26.984f, 54.199f, 26.897f, 54.178f, 26.8f, 54.178f
    path.cubic_to((f32::from_bits(0x41d547ab), f32::from_bits(0x4258cbca)), (f32::from_bits(0x41d1bc67), f32::from_bits(0x42592f1e)), (f32::from_bits(0x41d0b22a), f32::from_bits(0x4258e251)));
    // 26.66f, 54.199f, 26.217f, 54.296f, 26.087f, 54.221f
    path.line_to((f32::from_bits(0x41d0b22a), f32::from_bits(0x4258d70d)));
    // 26.087f, 54.21f
    path.cubic_to((f32::from_bits(0x41d09daf), f32::from_bits(0x4258d70d)), (f32::from_bits(0x41d070a1), f32::from_bits(0x4258b648)), (f32::from_bits(0x41d05a1a), f32::from_bits(0x4258ab05)));
    // 26.077f, 54.21f, 26.055f, 54.178f, 26.044f, 54.167f
    path.move_to((f32::from_bits(0x41ce8b41), f32::from_bits(0x42588a40)));
    // 25.818f, 54.135f
    path.cubic_to((f32::from_bits(0x41ceb643), f32::from_bits(0x4258ab05)), (f32::from_bits(0x41ce74ba), f32::from_bits(0x4258ab05)), (f32::from_bits(0x41ceccca), f32::from_bits(0x4258ab05)));
    // 25.839f, 54.167f, 25.807f, 54.167f, 25.85f, 54.167f
    path.cubic_to((f32::from_bits(0x41cef7cc), f32::from_bits(0x4258ab05)), (f32::from_bits(0x41cf0e53), f32::from_bits(0x4258b336)), (f32::from_bits(0x41cf0e53), f32::from_bits(0x42589db5)));
    // 25.871f, 54.167f, 25.882f, 54.175f, 25.882f, 54.154f
    path.cubic_to((f32::from_bits(0x41cf0e53), f32::from_bits(0x4258395b)), (f32::from_bits(0x41cf0a3a), f32::from_bits(0x42579790)), (f32::from_bits(0x41cedd2c), f32::from_bits(0x4257343c)));
    // 25.882f, 54.056f, 25.88f, 53.898f, 25.858f, 53.801f
    path.cubic_to((f32::from_bits(0x41cec8b1), f32::from_bits(0x42564086)), (f32::from_bits(0x41ceccca), f32::from_bits(0x4254f3b9)), (f32::from_bits(0x41ce5e32), f32::from_bits(0x425421cd)));
    // 25.848f, 53.563f, 25.85f, 53.238f, 25.796f, 53.033f
    path.line_to((f32::from_bits(0x41cdef9a), f32::from_bits(0x425421cd)));
    // 25.742f, 53.033f
    path.cubic_to((f32::from_bits(0x41cdd913), f32::from_bits(0x4254dd31)), (f32::from_bits(0x41ce126b), f32::from_bits(0x425626ec)), (f32::from_bits(0x41ce28f2), f32::from_bits(0x4256e250)));
    // 25.731f, 53.216f, 25.759f, 53.538f, 25.77f, 53.721f
    path.cubic_to((f32::from_bits(0x41ce3f79), f32::from_bits(0x42579377)), (f32::from_bits(0x41ce47aa), f32::from_bits(0x42580f5e)), (f32::from_bits(0x41ce8b40), f32::from_bits(0x42588a40)));
    // 25.781f, 53.894f, 25.785f, 54.015f, 25.818f, 54.135f
    path.move_to((f32::from_bits(0x41c58d4c), f32::from_bits(0x425271ad)));
    // 24.694f, 52.611f
    path.cubic_to((f32::from_bits(0x41c58d4c), f32::from_bits(0x42525c2c)), (f32::from_bits(0x41c5ba5a), f32::from_bits(0x42523024)), (f32::from_bits(0x41c5fbe4), f32::from_bits(0x425224e0)));
    // 24.694f, 52.59f, 24.716f, 52.547f, 24.748f, 52.536f
    path.line_to((f32::from_bits(0x41c6126b), f32::from_bits(0x4252199c)));
    // 24.759f, 52.525f
    path.cubic_to((f32::from_bits(0x41c6978a), f32::from_bits(0x42520f5f)), (f32::from_bits(0x41c774b9), f32::from_bits(0x42523023)), (f32::from_bits(0x41c79fbb), f32::from_bits(0x42525c2b)));
    // 24.824f, 52.515f, 24.932f, 52.547f, 24.953f, 52.59f
    path.cubic_to((f32::from_bits(0x41c7f9d8), f32::from_bits(0x4252a9fe)), (f32::from_bits(0x41c79fbb), f32::from_bits(0x4258e250)), (f32::from_bits(0x41c78b40), f32::from_bits(0x4259199c)));
    // 24.997f, 52.666f, 24.953f, 54.221f, 24.943f, 54.275f
    path.line_to((f32::from_bits(0x41c78b40), f32::from_bits(0x42592f1d)));
    // 24.943f, 54.296f
    path.cubic_to((f32::from_bits(0x41c747aa), f32::from_bits(0x42595c2b)), (f32::from_bits(0x41c68103), f32::from_bits(0x42596669)), (f32::from_bits(0x41c5fbe4), f32::from_bits(0x42596669)));
    // 24.91f, 54.34f, 24.813f, 54.35f, 24.748f, 54.35f
    path.cubic_to((f32::from_bits(0x41c5353c), f32::from_bits(0x425971ad)), (f32::from_bits(0x41c41684), f32::from_bits(0x425971ad)), (f32::from_bits(0x41c3e975), f32::from_bits(0x42592f1d)));
    // 24.651f, 54.361f, 24.511f, 54.361f, 24.489f, 54.296f
    path.cubic_to((f32::from_bits(0x41c3a7ec), f32::from_bits(0x4258cbc9)), (f32::from_bits(0x41c42afe), f32::from_bits(0x4257d919)), (f32::from_bits(0x41c44185), f32::from_bits(0x42578002)));
    // 24.457f, 54.199f, 24.521f, 53.962f, 24.532f, 53.875f
    path.cubic_to((f32::from_bits(0x41c46e93), f32::from_bits(0x42563f7f)), (f32::from_bits(0x41c4c6a4), f32::from_bits(0x42550a3f)), (f32::from_bits(0x41c5353b), f32::from_bits(0x4253df3d)));
    // 24.554f, 53.562f, 24.597f, 53.26f, 24.651f, 52.968f
    path.cubic_to((f32::from_bits(0x41c54bc2), f32::from_bits(0x42537be9)), (f32::from_bits(0x41c56249), f32::from_bits(0x42530d51)), (f32::from_bits(0x41c58d4b), f32::from_bits(0x4252cac2)));
    // 24.662f, 52.871f, 24.673f, 52.763f, 24.694f, 52.698f
    path.cubic_to((f32::from_bits(0x41c58d4b), f32::from_bits(0x4252a9fd)), (f32::from_bits(0x41c56249), f32::from_bits(0x42528833)), (f32::from_bits(0x41c58d4b), f32::from_bits(0x425271ab)));
    // 24.694f, 52.666f, 24.673f, 52.633f, 24.694f, 52.611f
    path.move_to((f32::from_bits(0x41c36662), f32::from_bits(0x42534fe0)));
    // 24.425f, 52.828f
    path.cubic_to((f32::from_bits(0x41c33954), f32::from_bits(0x4253c9bb)), (f32::from_bits(0x41c34dcf), f32::from_bits(0x42541688)), (f32::from_bits(0x41c322cc), f32::from_bits(0x42549063)));
    // 24.403f, 52.947f, 24.413f, 53.022f, 24.392f, 53.141f
    path.cubic_to((f32::from_bits(0x41c2f5be), f32::from_bits(0x4254fefb)), (f32::from_bits(0x41c2b434), f32::from_bits(0x42558e57)), (f32::from_bits(0x41c29dad), f32::from_bits(0x42560832)));
    // 24.37f, 53.249f, 24.338f, 53.389f, 24.327f, 53.508f
    path.cubic_to((f32::from_bits(0x41c2709f), f32::from_bits(0x4256e561)), (f32::from_bits(0x41c2459d), f32::from_bits(0x4257ad0f)), (f32::from_bits(0x41c1ed8c), f32::from_bits(0x42586874)));
    // 24.305f, 53.724f, 24.284f, 53.919f, 24.241f, 54.102f
    path.cubic_to((f32::from_bits(0x41c1d705), f32::from_bits(0x4258cbc8)), (f32::from_bits(0x41c20207), f32::from_bits(0x42590e57)), (f32::from_bits(0x41c1c07e), f32::from_bits(0x425950e7)));
    // 24.23f, 54.199f, 24.251f, 54.264f, 24.219f, 54.329f
    path.cubic_to((f32::from_bits(0x41c1c07e), f32::from_bits(0x42596668)), (f32::from_bits(0x41c1686e), f32::from_bits(0x42599270)), (f32::from_bits(0x41c13b5f), f32::from_bits(0x42599270)));
    // 24.219f, 54.35f, 24.176f, 54.393f, 24.154f, 54.393f
    path.cubic_to((f32::from_bits(0x41c0ccc7), f32::from_bits(0x4259a8f7)), (f32::from_bits(0x41c074b7), f32::from_bits(0x42599270)), (f32::from_bits(0x41c00620), f32::from_bits(0x425971ab)));
    // 24.1f, 54.415f, 24.057f, 54.393f, 24.003f, 54.361f
    path.cubic_to((f32::from_bits(0x41c00620), f32::from_bits(0x425825e4)), (f32::from_bits(0x41c08b3f), f32::from_bits(0x4256da1e)), (f32::from_bits(0x41c0f9d6), f32::from_bits(0x42558e57)));
    // 24.003f, 54.037f, 24.068f, 53.713f, 24.122f, 53.389f
    path.cubic_to((f32::from_bits(0x41c151e6), f32::from_bits(0x425479dc)), (f32::from_bits(0x41c151e6), f32::from_bits(0x42534fe0)), (f32::from_bits(0x41c1ed8c), f32::from_bits(0x425245a3)));
    // 24.165f, 53.119f, 24.165f, 52.828f, 24.241f, 52.568f
    path.cubic_to((f32::from_bits(0x41c22f15), f32::from_bits(0x42520f5d)), (f32::from_bits(0x41c22f15), f32::from_bits(0x4251d70b)), (f32::from_bits(0x41c25c24), f32::from_bits(0x4251ccce)));
    // 24.273f, 52.515f, 24.273f, 52.46f, 24.295f, 52.45f
    path.cubic_to((f32::from_bits(0x41c2e143), f32::from_bits(0x4251b647)), (f32::from_bits(0x41c34dce), f32::from_bits(0x4251e24f)), (f32::from_bits(0x41c3a7eb), f32::from_bits(0x4251e24f)));
    // 24.36f, 52.428f, 24.413f, 52.471f, 24.457f, 52.471f
    path.cubic_to((f32::from_bits(0x41c3be72), f32::from_bits(0x42525c2a)), (f32::from_bits(0x41c37add), f32::from_bits(0x4252e149)), (f32::from_bits(0x41c36662), f32::from_bits(0x42534fe0)));
    // 24.468f, 52.59f, 24.435f, 52.72f, 24.425f, 52.828f
    path.move_to((f32::from_bits(0x41b3105e), f32::from_bits(0x426e020d)));
    // 22.383f, 59.502f
    path.cubic_to((f32::from_bits(0x41b2ced5), f32::from_bits(0x426dcac1)), (f32::from_bits(0x41b28b3f), f32::from_bits(0x426d9375)), (f32::from_bits(0x41b21ca8), f32::from_bits(0x426d676d)));
    // 22.351f, 59.448f, 22.318f, 59.394f, 22.264f, 59.351f
    path.line_to((f32::from_bits(0x41b1f1a6), f32::from_bits(0x426d676d)));
    // 22.243f, 59.351f
    path.line_to((f32::from_bits(0x41b1f1a6), f32::from_bits(0x426d50e6)));
    // 22.243f, 59.329f
    path.cubic_to((f32::from_bits(0x41b1f1a6), f32::from_bits(0x426d2f1b)), (f32::from_bits(0x41b1830e), f32::from_bits(0x426d199a)), (f32::from_bits(0x41b15600), f32::from_bits(0x426d0f5d)));
    // 22.243f, 59.296f, 22.189f, 59.275f, 22.167f, 59.265f
    path.cubic_to((f32::from_bits(0x41b0e768), f32::from_bits(0x426cccce)), (f32::from_bits(0x41af1683), f32::from_bits(0x426bd917)), (f32::from_bits(0x41aefffc), f32::from_bits(0x426b8107)));
    // 22.113f, 59.2f, 21.886f, 58.962f, 21.875f, 58.876f
    path.cubic_to((f32::from_bits(0x41aeeb81), f32::from_bits(0x426b3334)), (f32::from_bits(0x41af5a19), f32::from_bits(0x426acfe0)), (f32::from_bits(0x41af70a0), f32::from_bits(0x426a8d51)));
    // 21.865f, 58.8f, 21.919f, 58.703f, 21.93f, 58.638f
    path.cubic_to((f32::from_bits(0x41b04dcf), f32::from_bits(0x42693647)), (f32::from_bits(0x41b1db1f), f32::from_bits(0x4268645b)), (f32::from_bits(0x41b43123), f32::from_bits(0x4267c9bc)));
    // 22.038f, 58.303f, 22.232f, 58.098f, 22.524f, 57.947f
    path.cubic_to((f32::from_bits(0x41b472ac), f32::from_bits(0x4267a7f1)), (f32::from_bits(0x41b4f7cb), f32::from_bits(0x426770a5)), (f32::from_bits(0x41b56662), f32::from_bits(0x42676668)));
    // 22.556f, 57.914f, 22.621f, 57.86f, 22.675f, 57.85f
    path.cubic_to((f32::from_bits(0x41b5a7eb), f32::from_bits(0x42675b24)), (f32::from_bits(0x41b5d4fa), f32::from_bits(0x42676668)), (f32::from_bits(0x41b62d0a), f32::from_bits(0x42675b24)));
    // 22.707f, 57.839f, 22.729f, 57.85f, 22.772f, 57.839f
    path.cubic_to((f32::from_bits(0x41b69ba2), f32::from_bits(0x42674fe0)), (f32::from_bits(0x41b78f58), f32::from_bits(0x42671895)), (f32::from_bits(0x41b828f1), f32::from_bits(0x42671895)));
    // 22.826f, 57.828f, 22.945f, 57.774f, 23.02f, 57.774f
    path.cubic_to((f32::from_bits(0x41b8ae10), f32::from_bits(0x42671895)), (f32::from_bits(0x41b8c497), f32::from_bits(0x42672f1c)), (f32::from_bits(0x41b91ca7), f32::from_bits(0x4267449d)));
    // 23.085f, 57.774f, 23.096f, 57.796f, 23.139f, 57.817f
    path.line_to((f32::from_bits(0x41b91ca7), f32::from_bits(0x42675b24)));
    // 23.139f, 57.839f
    path.cubic_to((f32::from_bits(0x41b91ca7), f32::from_bits(0x42674fe0)), (f32::from_bits(0x41b9332e), f32::from_bits(0x426770a5)), (f32::from_bits(0x41b9332e), f32::from_bits(0x4267872c)));
    // 23.139f, 57.828f, 23.15f, 57.86f, 23.15f, 57.882f
    path.cubic_to((f32::from_bits(0x41b91ca7), f32::from_bits(0x4267df3c)), (f32::from_bits(0x41b90620), f32::from_bits(0x42685918)), (f32::from_bits(0x41b8db1e), f32::from_bits(0x4268bc6b)));
    // 23.139f, 57.968f, 23.128f, 58.087f, 23.107f, 58.184f
    path.cubic_to((f32::from_bits(0x41b855ff), f32::from_bits(0x426a29fc)), (f32::from_bits(0x41b7d0e1), f32::from_bits(0x426bc290)), (f32::from_bits(0x41b76249), f32::from_bits(0x426d2f1b)));
    // 23.042f, 58.541f, 22.977f, 58.94f, 22.923f, 59.296f
    path.cubic_to((f32::from_bits(0x41b720c0), f32::from_bits(0x426e0d50)), (f32::from_bits(0x41b720c0), f32::from_bits(0x426ed3f8)), (f32::from_bits(0x41b69ba1), f32::from_bits(0x426f79dc)));
    // 22.891f, 59.513f, 22.891f, 59.707f, 22.826f, 59.869f
    path.cubic_to((f32::from_bits(0x41b64391), f32::from_bits(0x426f645b)), (f32::from_bits(0x41b62d09), f32::from_bits(0x426f79dc)), (f32::from_bits(0x41b5eb80), f32::from_bits(0x426f645b)));
    // 22.783f, 59.848f, 22.772f, 59.869f, 22.74f, 59.848f
    path.cubic_to((f32::from_bits(0x41b5a7ea), f32::from_bits(0x426f5917)), (f32::from_bits(0x41b57adc), f32::from_bits(0x426f374d)), (f32::from_bits(0x41b53953), f32::from_bits(0x426f1688)));
    // 22.707f, 59.837f, 22.685f, 59.804f, 22.653f, 59.772f
    path.line_to((f32::from_bits(0x41b53953), f32::from_bits(0x426f0107)));
    // 22.653f, 59.751f
    path.cubic_to((f32::from_bits(0x41b472ab), f32::from_bits(0x426ea7f0)), (f32::from_bits(0x41b3ac03), f32::from_bits(0x426e5a1e)), (f32::from_bits(0x41b3105d), f32::from_bits(0x426e020d)));
    // 22.556f, 59.664f, 22.459f, 59.588f, 22.383f, 59.502f
    test_simplify(reporter, &path.detach(), filename);
}

fn joel_5(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x43c5145a), f32::from_bits(0x43dc82f2)));
    // 394.159f, 441.023f
    path.line_to((f32::from_bits(0x43c5145a), f32::from_bits(0x43dc82f2)));
    // 394.159f, 441.023f
    path.close();
    path.move_to((f32::from_bits(0x43af4e56), f32::from_bits(0x43dbc604)));
    // 350.612f, 439.547f
    path.line_to((f32::from_bits(0x43af4e56), f32::from_bits(0x43dbc604)));
    // 350.612f, 439.547f
    path.close();
    path.move_to((f32::from_bits(0x43af4e56), f32::from_bits(0x43dbc604)));
    // 350.612f, 439.547f
    path.cubic_to((f32::from_bits(0x43b64a5e), f32::from_bits(0x43dc9604)), (f32::from_bits(0x43be0958), f32::from_bits(0x43dbb604)), (f32::from_bits(0x43c5145a), f32::from_bits(0x43dc8312)));
    // 364.581f, 441.172f, 380.073f, 439.422f, 394.159f, 441.024f
    path.cubic_to((f32::from_bits(0x43be0958), f32::from_bits(0x43dbb604)), (f32::from_bits(0x43b64a5e), f32::from_bits(0x43dc9604)), (f32::from_bits(0x43af4e56), f32::from_bits(0x43dbc604)));
    // 380.073f, 439.422f, 364.581f, 441.172f, 350.612f, 439.547f
    path.close();
    path.move_to((f32::from_bits(0x43a9126f), f32::from_bits(0x43e11604)));
    // 338.144f, 450.172f
    path.line_to((f32::from_bits(0x43a9126f), f32::from_bits(0x43e11604)));
    // 338.144f, 450.172f
    path.close();
    path.move_to((f32::from_bits(0x43a9126f), f32::from_bits(0x43e11604)));
    // 338.144f, 450.172f
    path.cubic_to((f32::from_bits(0x43ab3c6b), f32::from_bits(0x43debc08)), (f32::from_bits(0x43ad1b65), f32::from_bits(0x43de18f6)), (f32::from_bits(0x43af4e77), f32::from_bits(0x43dbc604)));
    // 342.472f, 445.469f, 346.214f, 444.195f, 350.613f, 439.547f
    path.cubic_to((f32::from_bits(0x43ad1b65), f32::from_bits(0x43de18f6)), (f32::from_bits(0x43ab3c6b), f32::from_bits(0x43debc08)), (f32::from_bits(0x43a9126f), f32::from_bits(0x43e11604)));
    // 346.214f, 444.195f, 342.472f, 445.469f, 338.144f, 450.172f
    path.close();
    path.move_to((f32::from_bits(0x43aa9d50), f32::from_bits(0x43e173f8)));
    // 341.229f, 450.906f
    path.line_to((f32::from_bits(0x43aa9d50), f32::from_bits(0x43e173f8)));
    // 341.229f, 450.906f
    path.close();
    path.move_to((f32::from_bits(0x43aa9d50), f32::from_bits(0x43e173f8)));
    // 341.229f, 450.906f
    path.cubic_to((f32::from_bits(0x43aa0852), f32::from_bits(0x43e183f8)), (f32::from_bits(0x43a9be56), f32::from_bits(0x43e0d2f2)), (f32::from_bits(0x43a9124e), f32::from_bits(0x43e11604)));
    // 340.065f, 451.031f, 339.487f, 449.648f, 338.143f, 450.172f
    path.cubic_to((f32::from_bits(0x43a9be56), f32::from_bits(0x43e0d2f2)), (f32::from_bits(0x43aa0852), f32::from_bits(0x43e183f8)), (f32::from_bits(0x43aa9d50), f32::from_bits(0x43e173f8)));
    // 339.487f, 449.648f, 340.065f, 451.031f, 341.229f, 450.906f
    path.close();
    path.move_to((f32::from_bits(0x43b13667), f32::from_bits(0x43dce106)));
    // 354.425f, 441.758f
    path.line_to((f32::from_bits(0x43b13667), f32::from_bits(0x43dce106)));
    // 354.425f, 441.758f
    path.close();
    path.move_to((f32::from_bits(0x43b13667), f32::from_bits(0x43dce106)));
    // 354.425f, 441.758f
    path.cubic_to((f32::from_bits(0x43aead71), f32::from_bits(0x43dd9d0e)), (f32::from_bits(0x43acd375), f32::from_bits(0x43dff20c)), (f32::from_bits(0x43aa9d71), f32::from_bits(0x43e173f8)));
    // 349.355f, 443.227f, 345.652f, 447.891f, 341.23f, 450.906f
    path.cubic_to((f32::from_bits(0x43acd354), f32::from_bits(0x43dff20c)), (f32::from_bits(0x43aead50), f32::from_bits(0x43dd9d0f)), (f32::from_bits(0x43b13667), f32::from_bits(0x43dce106)));
    // 345.651f, 447.891f, 349.354f, 443.227f, 354.425f, 441.758f
    path.close();
    path.move_to((f32::from_bits(0x43ac8561), f32::from_bits(0x43e30106)));
    // 345.042f, 454.008f
    path.line_to((f32::from_bits(0x43ac8561), f32::from_bits(0x43e30106)));
    // 345.042f, 454.008f
    path.close();
    path.move_to((f32::from_bits(0x43ac8561), f32::from_bits(0x43e30106)));
    // 345.042f, 454.008f
    path.cubic_to((f32::from_bits(0x43adc76d), f32::from_bits(0x43e0f4fe)), (f32::from_bits(0x43b21a5f), f32::from_bits(0x43df7efa)), (f32::from_bits(0x43b13667), f32::from_bits(0x43dce106)));
    // 347.558f, 449.914f, 356.206f, 446.992f, 354.425f, 441.758f
    path.cubic_to((f32::from_bits(0x43b21a5f), f32::from_bits(0x43df7efa)), (f32::from_bits(0x43adc76d), f32::from_bits(0x43e0f4fe)), (f32::from_bits(0x43ac8561), f32::from_bits(0x43e30106)));
    // 356.206f, 446.992f, 347.558f, 449.914f, 345.042f, 454.008f
    path.close();
    path.move_to((f32::from_bits(0x43b33169), f32::from_bits(0x43dc82f2)));
    // 358.386f, 441.023f
    path.line_to((f32::from_bits(0x43b33169), f32::from_bits(0x43dc82f2)));
    // 358.386f, 441.023f
    path.close();
    path.move_to((f32::from_bits(0x43b33169), f32::from_bits(0x43dc82f2)));
    // 358.386f, 441.023f
    path.cubic_to((f32::from_bits(0x43b16169), f32::from_bits(0x43ded7f0)), (f32::from_bits(0x43aef375), f32::from_bits(0x43e13be8)), (f32::from_bits(0x43ac8561), f32::from_bits(0x43e300e6)));
    // 354.761f, 445.687f, 349.902f, 450.468f, 345.042f, 454.007f
    path.cubic_to((f32::from_bits(0x43aef355), f32::from_bits(0x43e13c09)), (f32::from_bits(0x43b16169), f32::from_bits(0x43ded811)), (f32::from_bits(0x43b33169), f32::from_bits(0x43dc82f2)));
    // 349.901f, 450.469f, 354.761f, 445.688f, 358.386f, 441.023f
    path.close();
    path.move_to((f32::from_bits(0x43b4bb65), f32::from_bits(0x43dd4000)));
    // 361.464f, 442.5f
    path.line_to((f32::from_bits(0x43b4bb65), f32::from_bits(0x43dd4000)));
    // 361.464f, 442.5f
    path.close();
    path.move_to((f32::from_bits(0x43b4bb65), f32::from_bits(0x43dd4000)));
    // 361.464f, 442.5f
    path.cubic_to((f32::from_bits(0x43b44959), f32::from_bits(0x43dcddf4)), (f32::from_bits(0x43b3e76d), f32::from_bits(0x43dc48f6)), (f32::from_bits(0x43b33169), f32::from_bits(0x43dc82f2)));
    // 360.573f, 441.734f, 359.808f, 440.57f, 358.386f, 441.023f
    path.cubic_to((f32::from_bits(0x43b3e76d), f32::from_bits(0x43dc48f6)), (f32::from_bits(0x43b44959), f32::from_bits(0x43dcddf4)), (f32::from_bits(0x43b4bb65), f32::from_bits(0x43dd4000)));
    // 359.808f, 440.57f, 360.573f, 441.734f, 361.464f, 442.5f
    path.close();
    path.move_to((f32::from_bits(0x43ae7f5d), f32::from_bits(0x43e5a70a)));
    // 348.995f, 459.305f
    path.line_to((f32::from_bits(0x43ae7f5d), f32::from_bits(0x43e5a70a)));
    // 348.995f, 459.305f
    path.close();
    path.move_to((f32::from_bits(0x43ae7f5d), f32::from_bits(0x43e5a70a)));
    // 348.995f, 459.305f
    path.cubic_to((f32::from_bits(0x43af945b), f32::from_bits(0x43e21d0e)), (f32::from_bits(0x43b3a74d), f32::from_bits(0x43e0ce14)), (f32::from_bits(0x43b4bb65), f32::from_bits(0x43dd4000)));
    // 351.159f, 452.227f, 359.307f, 449.61f, 361.464f, 442.5f
    path.cubic_to((f32::from_bits(0x43b3a76d), f32::from_bits(0x43e0cdf4)), (f32::from_bits(0x43af945b), f32::from_bits(0x43e21d0e)), (f32::from_bits(0x43ae7f5d), f32::from_bits(0x43e5a70a)));
    // 359.308f, 449.609f, 351.159f, 452.227f, 348.995f, 459.305f
    path.close();
    path.move_to((f32::from_bits(0x43b58a5f), f32::from_bits(0x43dce106)));
    // 363.081f, 441.758f
    path.line_to((f32::from_bits(0x43b58a5f), f32::from_bits(0x43dce106)));
    // 363.081f, 441.758f
    path.close();
    path.move_to((f32::from_bits(0x43b58a5f), f32::from_bits(0x43dce106)));
    // 363.081f, 441.758f
    path.cubic_to((f32::from_bits(0x43b2c063), f32::from_bits(0x43dfa604)), (f32::from_bits(0x43b1d561), f32::from_bits(0x43e374fe)), (f32::from_bits(0x43ae7f5d), f32::from_bits(0x43e5a70a)));
    // 357.503f, 447.297f, 355.667f, 454.914f, 348.995f, 459.305f
    path.cubic_to((f32::from_bits(0x43b1d561), f32::from_bits(0x43e374fe)), (f32::from_bits(0x43b2c063), f32::from_bits(0x43dfa604)), (f32::from_bits(0x43b58a5f), f32::from_bits(0x43dce106)));
    // 355.667f, 454.914f, 357.503f, 447.297f, 363.081f, 441.758f
    path.close();
    path.move_to((f32::from_bits(0x43b6b561), f32::from_bits(0x43dd4000)));
    // 365.417f, 442.5f
    path.line_to((f32::from_bits(0x43b6b561), f32::from_bits(0x43dd4000)));
    // 365.417f, 442.5f
    path.close();
    path.move_to((f32::from_bits(0x43b6b561), f32::from_bits(0x43dd4000)));
    // 365.417f, 442.5f
    path.line_to((f32::from_bits(0x43b58a5f), f32::from_bits(0x43dce106)));
    // 363.081f, 441.758f
    path.line_to((f32::from_bits(0x43b6b561), f32::from_bits(0x43dd4000)));
    // 365.417f, 442.5f
    path.close();
    path.move_to((f32::from_bits(0x43b07a5f), f32::from_bits(0x43e7220c)));
    // 352.956f, 462.266f
    path.line_to((f32::from_bits(0x43b07a5f), f32::from_bits(0x43e7220c)));
    // 352.956f, 462.266f
    path.close();
    path.move_to((f32::from_bits(0x43b07a5f), f32::from_bits(0x43e7220c)));
    // 352.956f, 462.266f
    path.cubic_to((f32::from_bits(0x43b29f5d), f32::from_bits(0x43e3e810)), (f32::from_bits(0x43b59667), f32::from_bits(0x43e0f916)), (f32::from_bits(0x43b6b561), f32::from_bits(0x43dd4000)));
    // 357.245f, 455.813f, 363.175f, 449.946f, 365.417f, 442.5f
    path.cubic_to((f32::from_bits(0x43b59667), f32::from_bits(0x43e0f8f6)), (f32::from_bits(0x43b29f5d), f32::from_bits(0x43e3e7f0)), (f32::from_bits(0x43b07a5f), f32::from_bits(0x43e7220c)));
    // 363.175f, 449.945f, 357.245f, 455.812f, 352.956f, 462.266f
    path.close();
    path.move_to((f32::from_bits(0x43b0d853), f32::from_bits(0x43e84efa)));
    // 353.69f, 464.617f
    path.line_to((f32::from_bits(0x43b0d853), f32::from_bits(0x43e84efa)));
    // 353.69f, 464.617f
    path.close();
    path.move_to((f32::from_bits(0x43b0d853), f32::from_bits(0x43e84efa)));
    // 353.69f, 464.617f
    path.cubic_to((f32::from_bits(0x43b03a5f), f32::from_bits(0x43e934fe)), (f32::from_bits(0x43b1345b), f32::from_bits(0x43e7870a)), (f32::from_bits(0x43b07a5f), f32::from_bits(0x43e721ec)));
    // 352.456f, 466.414f, 354.409f, 463.055f, 352.956f, 462.265f
    path.cubic_to((f32::from_bits(0x43b1345b), f32::from_bits(0x43e7870b)), (f32::from_bits(0x43b03a5f), f32::from_bits(0x43e934fe)), (f32::from_bits(0x43b0d853), f32::from_bits(0x43e84efa)));
    // 354.409f, 463.055f, 352.456f, 466.414f, 353.69f, 464.617f
    path.close();
    path.move_to((f32::from_bits(0x43b84063), f32::from_bits(0x43ddb106)));
    // 368.503f, 443.383f
    path.line_to((f32::from_bits(0x43b84063), f32::from_bits(0x43ddb106)));
    // 368.503f, 443.383f
    path.close();
    path.move_to((f32::from_bits(0x43b84063), f32::from_bits(0x43ddb106)));
    // 368.503f, 443.383f
    path.cubic_to((f32::from_bits(0x43b42667), f32::from_bits(0x43e039fc)), (f32::from_bits(0x43b39d71), f32::from_bits(0x43e4e000)), (f32::from_bits(0x43b0d873), f32::from_bits(0x43e84efa)));
    // 360.3f, 448.453f, 359.23f, 457.75f, 353.691f, 464.617f
    path.cubic_to((f32::from_bits(0x43b39d50), f32::from_bits(0x43e4e000)), (f32::from_bits(0x43b42667), f32::from_bits(0x43e039fc)), (f32::from_bits(0x43b84063), f32::from_bits(0x43ddb106)));
    // 359.229f, 457.75f, 360.3f, 448.453f, 368.503f, 443.383f
    path.close();
    path.move_to((f32::from_bits(0x43b89d51), f32::from_bits(0x43de0efa)));
    // 369.229f, 444.117f
    path.line_to((f32::from_bits(0x43b89d51), f32::from_bits(0x43de0efa)));
    // 369.229f, 444.117f
    path.close();
    path.move_to((f32::from_bits(0x43b89d51), f32::from_bits(0x43de0efa)));
    // 369.229f, 444.117f
    path.line_to((f32::from_bits(0x43b84043), f32::from_bits(0x43ddb106)));
    // 368.502f, 443.383f
    path.line_to((f32::from_bits(0x43b89d51), f32::from_bits(0x43de0efa)));
    // 369.229f, 444.117f
    path.close();
    path.move_to((f32::from_bits(0x43b26270), f32::from_bits(0x43e90c08)));
    // 356.769f, 466.094f
    path.line_to((f32::from_bits(0x43b26270), f32::from_bits(0x43e90c08)));
    // 356.769f, 466.094f
    path.close();
    path.move_to((f32::from_bits(0x43b26270), f32::from_bits(0x43e90c08)));
    // 356.769f, 466.094f
    path.cubic_to((f32::from_bits(0x43b48d72), f32::from_bits(0x43e569fc)), (f32::from_bits(0x43b7897a), f32::from_bits(0x43e21d0e)), (f32::from_bits(0x43b89d72), f32::from_bits(0x43de0efa)));
    // 361.105f, 458.828f, 367.074f, 452.227f, 369.23f, 444.117f
    path.cubic_to((f32::from_bits(0x43b78959), f32::from_bits(0x43e21d0e)), (f32::from_bits(0x43b48d51), f32::from_bits(0x43e569fc)), (f32::from_bits(0x43b26270), f32::from_bits(0x43e90c08)));
    // 367.073f, 452.227f, 361.104f, 458.828f, 356.769f, 466.094f
    path.close();
    path.move_to((f32::from_bits(0x43b3316a), f32::from_bits(0x43e90c08)));
    // 358.386f, 466.094f
    path.line_to((f32::from_bits(0x43b3316a), f32::from_bits(0x43e90c08)));
    // 358.386f, 466.094f
    path.close();
    path.move_to((f32::from_bits(0x43b3316a), f32::from_bits(0x43e90c08)));
    // 358.386f, 466.094f
    path.line_to((f32::from_bits(0x43b26270), f32::from_bits(0x43e90c08)));
    // 356.769f, 466.094f
    path.line_to((f32::from_bits(0x43b3316a), f32::from_bits(0x43e90c08)));
    // 358.386f, 466.094f
    path.close();
    path.move_to((f32::from_bits(0x43ba2853), f32::from_bits(0x43ddb106)));
    // 372.315f, 443.383f
    path.line_to((f32::from_bits(0x43ba2853), f32::from_bits(0x43ddb106)));
    // 372.315f, 443.383f
    path.close();
    path.move_to((f32::from_bits(0x43ba2853), f32::from_bits(0x43ddb106)));
    // 372.315f, 443.383f
    path.cubic_to((f32::from_bits(0x43b7d74d), f32::from_bits(0x43e17604)), (f32::from_bits(0x43b5824f), f32::from_bits(0x43e59604)), (f32::from_bits(0x43b33149), f32::from_bits(0x43e90c08)));
    // 367.682f, 450.922f, 363.018f, 459.172f, 358.385f, 466.094f
    path.cubic_to((f32::from_bits(0x43b58270), f32::from_bits(0x43e59604)), (f32::from_bits(0x43b7d76e), f32::from_bits(0x43e17604)), (f32::from_bits(0x43ba2853), f32::from_bits(0x43ddb106)));
    // 363.019f, 459.172f, 367.683f, 450.922f, 372.315f, 443.383f
    path.close();
    path.move_to((f32::from_bits(0x43bb5355), f32::from_bits(0x43de0efa)));
    // 374.651f, 444.117f
    path.line_to((f32::from_bits(0x43bb5355), f32::from_bits(0x43de0efa)));
    // 374.651f, 444.117f
    path.close();
    path.move_to((f32::from_bits(0x43bb5355), f32::from_bits(0x43de0efa)));
    // 374.651f, 444.117f
    path.cubic_to((f32::from_bits(0x43bb1853), f32::from_bits(0x43dd92f2)), (f32::from_bits(0x43ba9e57), f32::from_bits(0x43ddab02)), (f32::from_bits(0x43ba2853), f32::from_bits(0x43ddb106)));
    // 374.19f, 443.148f, 373.237f, 443.336f, 372.315f, 443.383f
    path.cubic_to((f32::from_bits(0x43ba9e57), f32::from_bits(0x43ddab02)), (f32::from_bits(0x43bb1853), f32::from_bits(0x43dd92f2)), (f32::from_bits(0x43bb5355), f32::from_bits(0x43de0efa)));
    // 373.237f, 443.336f, 374.19f, 443.148f, 374.651f, 444.117f
    path.close();
    path.move_to((f32::from_bits(0x43b58a5f), f32::from_bits(0x43e90c08)));
    // 363.081f, 466.094f
    path.line_to((f32::from_bits(0x43b58a5f), f32::from_bits(0x43e90c08)));
    // 363.081f, 466.094f
    path.close();
    path.move_to((f32::from_bits(0x43b58a5f), f32::from_bits(0x43e90c08)));
    // 363.081f, 466.094f
    path.cubic_to((f32::from_bits(0x43b76c6b), f32::from_bits(0x43e55d0e)), (f32::from_bits(0x43ba4a5f), f32::from_bits(0x43e21312)), (f32::from_bits(0x43bb5355), f32::from_bits(0x43de0efa)));
    // 366.847f, 458.727f, 372.581f, 452.149f, 374.651f, 444.117f
    path.cubic_to((f32::from_bits(0x43ba4a5f), f32::from_bits(0x43e212f2)), (f32::from_bits(0x43b76c6c), f32::from_bits(0x43e55d0e)), (f32::from_bits(0x43b58a5f), f32::from_bits(0x43e90c08)));
    // 372.581f, 452.148f, 366.847f, 458.727f, 363.081f, 466.094f
    path.close();
    path.move_to((f32::from_bits(0x43b6b561), f32::from_bits(0x43e90c08)));
    // 365.417f, 466.094f
    path.line_to((f32::from_bits(0x43b6b561), f32::from_bits(0x43e90c08)));
    // 365.417f, 466.094f
    path.close();
    path.move_to((f32::from_bits(0x43b6b561), f32::from_bits(0x43e90c08)));
    // 365.417f, 466.094f
    path.line_to((f32::from_bits(0x43b58a5f), f32::from_bits(0x43e90c08)));
    // 363.081f, 466.094f
    path.line_to((f32::from_bits(0x43b6b561), f32::from_bits(0x43e90c08)));
    // 365.417f, 466.094f
    path.close();
    path.move_to((f32::from_bits(0x43bc8063), f32::from_bits(0x43e058f6)));
    // 377.003f, 448.695f
    path.line_to((f32::from_bits(0x43bc8063), f32::from_bits(0x43e058f6)));
    // 377.003f, 448.695f
    path.close();
    path.move_to((f32::from_bits(0x43bc8063), f32::from_bits(0x43e058f6)));
    // 377.003f, 448.695f
    path.cubic_to((f32::from_bits(0x43b9de57), f32::from_bits(0x43e29df4)), (f32::from_bits(0x43b84355), f32::from_bits(0x43e5fefa)), (f32::from_bits(0x43b6b561), f32::from_bits(0x43e90be8)));
    // 371.737f, 453.234f, 368.526f, 459.992f, 365.417f, 466.093f
    path.cubic_to((f32::from_bits(0x43b84355), f32::from_bits(0x43e5fefa)), (f32::from_bits(0x43b9de57), f32::from_bits(0x43e29df4)), (f32::from_bits(0x43bc8063), f32::from_bits(0x43e058f6)));
    // 368.526f, 459.992f, 371.737f, 453.234f, 377.003f, 448.695f
    path.close();
    path.move_to((f32::from_bits(0x43b89d51), f32::from_bits(0x43e969fc)));
    // 369.229f, 466.828f
    path.line_to((f32::from_bits(0x43b89d51), f32::from_bits(0x43e969fc)));
    // 369.229f, 466.828f
    path.close();
    path.move_to((f32::from_bits(0x43b89d51), f32::from_bits(0x43e969fc)));
    // 369.229f, 466.828f
    path.cubic_to((f32::from_bits(0x43b98149), f32::from_bits(0x43e637f0)), (f32::from_bits(0x43bd3355), f32::from_bits(0x43e3adf4)), (f32::from_bits(0x43bc8043), f32::from_bits(0x43e058f6)));
    // 371.01f, 460.437f, 378.401f, 455.359f, 377.002f, 448.695f
    path.cubic_to((f32::from_bits(0x43bd3355), f32::from_bits(0x43e3adf4)), (f32::from_bits(0x43b9816a), f32::from_bits(0x43e638f6)), (f32::from_bits(0x43b89d51), f32::from_bits(0x43e969fc)));
    // 378.401f, 455.359f, 371.011f, 460.445f, 369.229f, 466.828f
    path.close();
    path.move_to((f32::from_bits(0x43ba8668), f32::from_bits(0x43e9c7f0)));
    // 373.05f, 467.562f
    path.line_to((f32::from_bits(0x43ba8668), f32::from_bits(0x43e9c7f0)));
    // 373.05f, 467.562f
    path.close();
    path.move_to((f32::from_bits(0x43ba8668), f32::from_bits(0x43e9c7f0)));
    // 373.05f, 467.562f
    path.cubic_to((f32::from_bits(0x43ba1376), f32::from_bits(0x43e90000)), (f32::from_bits(0x43b94270), f32::from_bits(0x43e8f1ec)), (f32::from_bits(0x43b89d72), f32::from_bits(0x43e969fc)));
    // 372.152f, 466, 370.519f, 465.89f, 369.23f, 466.828f
    path.cubic_to((f32::from_bits(0x43b94270), f32::from_bits(0x43e8f20c)), (f32::from_bits(0x43ba1355), f32::from_bits(0x43e90000)), (f32::from_bits(0x43ba8668), f32::from_bits(0x43e9c7f0)));
    // 370.519f, 465.891f, 372.151f, 466, 373.05f, 467.562f
    path.close();
    path.move_to((f32::from_bits(0x43c0c064), f32::from_bits(0x43dc82f2)));
    // 385.503f, 441.023f
    path.line_to((f32::from_bits(0x43c0c064), f32::from_bits(0x43dc82f2)));
    // 385.503f, 441.023f
    path.close();
    path.move_to((f32::from_bits(0x43c0c064), f32::from_bits(0x43dc82f2)));
    // 385.503f, 441.023f
    path.cubic_to((f32::from_bits(0x43be095a), f32::from_bits(0x43e0acee)), (f32::from_bits(0x43bd8a60), f32::from_bits(0x43e5c0e6)), (f32::from_bits(0x43ba8668), f32::from_bits(0x43e9c7f0)));
    // 380.073f, 449.351f, 379.081f, 459.507f, 373.05f, 467.562f
    path.cubic_to((f32::from_bits(0x43bd8a60), f32::from_bits(0x43e5c107)), (f32::from_bits(0x43be095a), f32::from_bits(0x43e0ad0f)), (f32::from_bits(0x43c0c064), f32::from_bits(0x43dc82f2)));
    // 379.081f, 459.508f, 380.073f, 449.352f, 385.503f, 441.023f
    path.close();
    path.move_to((f32::from_bits(0x43c00562), f32::from_bits(0x43e23000)));
    // 384.042f, 452.375f
    path.line_to((f32::from_bits(0x43c00562), f32::from_bits(0x43e23000)));
    // 384.042f, 452.375f
    path.close();
    path.move_to((f32::from_bits(0x43c00562), f32::from_bits(0x43e23000)));
    // 384.042f, 452.375f
    path.cubic_to((f32::from_bits(0x43bfaf5e), f32::from_bits(0x43e013f8)), (f32::from_bits(0x43c40668), f32::from_bits(0x43ddc2f2)), (f32::from_bits(0x43c0c064), f32::from_bits(0x43dc82f2)));
    // 383.37f, 448.156f, 392.05f, 443.523f, 385.503f, 441.023f
    path.cubic_to((f32::from_bits(0x43c40668), f32::from_bits(0x43ddc2f2)), (f32::from_bits(0x43bfaf5e), f32::from_bits(0x43e013f8)), (f32::from_bits(0x43c00562), f32::from_bits(0x43e23000)));
    // 392.05f, 443.523f, 383.37f, 448.156f, 384.042f, 452.375f
    path.close();
    path.move_to((f32::from_bits(0x43bed854), f32::from_bits(0x43e5370a)));
    // 381.69f, 458.43f
    path.line_to((f32::from_bits(0x43bed854), f32::from_bits(0x43e5370a)));
    // 381.69f, 458.43f
    path.close();
    path.move_to((f32::from_bits(0x43bed854), f32::from_bits(0x43e5370a)));
    // 381.69f, 458.43f
    path.cubic_to((f32::from_bits(0x43c06562), f32::from_bits(0x43e4b4fe)), (f32::from_bits(0x43bf095a), f32::from_bits(0x43e2fd0e)), (f32::from_bits(0x43c00562), f32::from_bits(0x43e23000)));
    // 384.792f, 457.414f, 382.073f, 453.977f, 384.042f, 452.375f
    path.cubic_to((f32::from_bits(0x43bf095a), f32::from_bits(0x43e2fdf4)), (f32::from_bits(0x43c06562), f32::from_bits(0x43e4b4fe)), (f32::from_bits(0x43bed854), f32::from_bits(0x43e5370a)));
    // 382.073f, 453.984f, 384.792f, 457.414f, 381.69f, 458.43f
    path.close();
    path.move_to((f32::from_bits(0x43bf3668), f32::from_bits(0x43e5a70a)));
    // 382.425f, 459.305f
    path.line_to((f32::from_bits(0x43bf3668), f32::from_bits(0x43e5a70a)));
    // 382.425f, 459.305f
    path.close();
    path.move_to((f32::from_bits(0x43bf3668), f32::from_bits(0x43e5a70a)));
    // 382.425f, 459.305f
    path.line_to((f32::from_bits(0x43bed874), f32::from_bits(0x43e5370a)));
    // 381.691f, 458.43f
    path.line_to((f32::from_bits(0x43bf3668), f32::from_bits(0x43e5a70a)));
    // 382.425f, 459.305f
    path.close();
    path.move_to((f32::from_bits(0x43bcde58), f32::from_bits(0x43e9c7ef)));
    // 377.737f, 467.562f
    path.line_to((f32::from_bits(0x43bcde58), f32::from_bits(0x43e9c7ef)));
    // 377.737f, 467.562f
    path.close();
    path.move_to((f32::from_bits(0x43bcde58), f32::from_bits(0x43e9c7ef)));
    // 377.737f, 467.562f
    path.cubic_to((f32::from_bits(0x43bdfb66), f32::from_bits(0x43e888f5)), (f32::from_bits(0x43bd6854), f32::from_bits(0x43e69ced)), (f32::from_bits(0x43bf3668), f32::from_bits(0x43e5a6e9)));
    // 379.964f, 465.07f, 378.815f, 461.226f, 382.425f, 459.304f
    path.cubic_to((f32::from_bits(0x43bd6854), f32::from_bits(0x43e69d0e)), (f32::from_bits(0x43bdfb66), f32::from_bits(0x43e888f5)), (f32::from_bits(0x43bcde58), f32::from_bits(0x43e9c7ef)));
    // 378.815f, 461.227f, 379.964f, 465.07f, 377.737f, 467.562f
    path.close();
    path.move_to((f32::from_bits(0x43bf3668), f32::from_bits(0x43ea9810)));
    // 382.425f, 469.188f
    path.line_to((f32::from_bits(0x43bf3668), f32::from_bits(0x43ea9810)));
    // 382.425f, 469.188f
    path.close();
    path.move_to((f32::from_bits(0x43bf3668), f32::from_bits(0x43ea9810)));
    // 382.425f, 469.188f
    path.cubic_to((f32::from_bits(0x43bebf5e), f32::from_bits(0x43e99e14)), (f32::from_bits(0x43bdc562), f32::from_bits(0x43e9d70a)), (f32::from_bits(0x43bcde58), f32::from_bits(0x43e9c810)));
    // 381.495f, 467.235f, 379.542f, 467.68f, 377.737f, 467.563f
    path.cubic_to((f32::from_bits(0x43bdc562), f32::from_bits(0x43e9d70a)), (f32::from_bits(0x43bebf5e), f32::from_bits(0x43e99df3)), (f32::from_bits(0x43bf3668), f32::from_bits(0x43ea9810)));
    // 379.542f, 467.68f, 381.495f, 467.234f, 382.425f, 469.188f
    path.close();
    path.move_to((f32::from_bits(0x43c0c064), f32::from_bits(0x43e78000)));
    // 385.503f, 463
    path.line_to((f32::from_bits(0x43c0c064), f32::from_bits(0x43e78000)));
    // 385.503f, 463
    path.close();
    path.move_to((f32::from_bits(0x43c0c064), f32::from_bits(0x43e78000)));
    // 385.503f, 463
    path.cubic_to((f32::from_bits(0x43bfaf5e), f32::from_bits(0x43e7f9fc)), (f32::from_bits(0x43bfbe58), f32::from_bits(0x43e98b02)), (f32::from_bits(0x43bf3668), f32::from_bits(0x43ea9810)));
    // 383.37f, 463.953f, 383.487f, 467.086f, 382.425f, 469.188f
    path.cubic_to((f32::from_bits(0x43bfbe58), f32::from_bits(0x43e98b02)), (f32::from_bits(0x43bfaf5e), f32::from_bits(0x43e7f9fc)), (f32::from_bits(0x43c0c064), f32::from_bits(0x43e78000)));
    // 383.487f, 467.086f, 383.37f, 463.953f, 385.503f, 463
    path.close();
    path.move_to((f32::from_bits(0x43c1316a), f32::from_bits(0x43e35efa)));
    // 386.386f, 454.742f
    path.line_to((f32::from_bits(0x43c1316a), f32::from_bits(0x43e35efa)));
    // 386.386f, 454.742f
    path.close();
    path.move_to((f32::from_bits(0x43c1316a), f32::from_bits(0x43e35efa)));
    // 386.386f, 454.742f
    path.cubic_to((f32::from_bits(0x43c35270), f32::from_bits(0x43e586ea)), (f32::from_bits(0x43beb064), f32::from_bits(0x43e561ec)), (f32::from_bits(0x43c0c064), f32::from_bits(0x43e78000)));
    // 390.644f, 459.054f, 381.378f, 458.765f, 385.503f, 463
    path.cubic_to((f32::from_bits(0x43beb064), f32::from_bits(0x43e5620c)), (f32::from_bits(0x43c35270), f32::from_bits(0x43e5870a)), (f32::from_bits(0x43c1316a), f32::from_bits(0x43e35efa)));
    // 381.378f, 458.766f, 390.644f, 459.055f, 386.386f, 454.742f
    path.close();
    path.move_to((f32::from_bits(0x43c3e76e), f32::from_bits(0x43df2b02)));
    // 391.808f, 446.336f
    path.line_to((f32::from_bits(0x43c3e76e), f32::from_bits(0x43df2b02)));
    // 391.808f, 446.336f
    path.close();
    path.move_to((f32::from_bits(0x43c3e76e), f32::from_bits(0x43df2b02)));
    // 391.808f, 446.336f
    path.cubic_to((f32::from_bits(0x43c2ba60), f32::from_bits(0x43e07810)), (f32::from_bits(0x43c32a60), f32::from_bits(0x43e31106)), (f32::from_bits(0x43c1316a), f32::from_bits(0x43e35efa)));
    // 389.456f, 448.938f, 390.331f, 454.133f, 386.386f, 454.742f
    path.cubic_to((f32::from_bits(0x43c32a60), f32::from_bits(0x43e31106)), (f32::from_bits(0x43c2ba60), f32::from_bits(0x43e07811)), (f32::from_bits(0x43c3e76e), f32::from_bits(0x43df2b02)));
    // 390.331f, 454.133f, 389.456f, 448.938f, 391.808f, 446.336f
    path.close();
    path.move_to((f32::from_bits(0x43c3e76e), f32::from_bits(0x43dd4000)));
    // 391.808f, 442.5f
    path.line_to((f32::from_bits(0x43c3e76e), f32::from_bits(0x43dd4000)));
    // 391.808f, 442.5f
    path.close();
    path.move_to((f32::from_bits(0x43c3e76e), f32::from_bits(0x43dd4000)));
    // 391.808f, 442.5f
    path.cubic_to((f32::from_bits(0x43c2a668), f32::from_bits(0x43ddbefa)), (f32::from_bits(0x43c35f7e), f32::from_bits(0x43def4fe)), (f32::from_bits(0x43c3e76e), f32::from_bits(0x43df2b02)));
    // 389.3f, 443.492f, 390.746f, 445.914f, 391.808f, 446.336f
    path.cubic_to((f32::from_bits(0x43c35f5e), f32::from_bits(0x43def4fe)), (f32::from_bits(0x43c2a668), f32::from_bits(0x43ddbefa)), (f32::from_bits(0x43c3e76e), f32::from_bits(0x43dd4000)));
    // 390.745f, 445.914f, 389.3f, 443.492f, 391.808f, 442.5f
    path.close();
    path.move_to((f32::from_bits(0x43c44562), f32::from_bits(0x43ddb106)));
    // 392.542f, 443.383f
    path.line_to((f32::from_bits(0x43c44562), f32::from_bits(0x43ddb106)));
    // 392.542f, 443.383f
    path.close();
    path.move_to((f32::from_bits(0x43c44562), f32::from_bits(0x43ddb106)));
    // 392.542f, 443.383f
    path.line_to((f32::from_bits(0x43c3e76e), f32::from_bits(0x43dd4000)));
    // 391.808f, 442.5f
    path.line_to((f32::from_bits(0x43c44562), f32::from_bits(0x43ddb106)));
    // 392.542f, 443.383f
    path.close();
    path.move_to((f32::from_bits(0x43c5145c), f32::from_bits(0x43dc82f2)));
    // 394.159f, 441.023f
    path.line_to((f32::from_bits(0x43c44562), f32::from_bits(0x43ddb0e6)));
    // 392.542f, 443.382f
    path.line_to((f32::from_bits(0x43c5145c), f32::from_bits(0x43dc82f2)));
    // 394.159f, 441.023f
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn joel_6(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x43c38c6a), f32::from_bits(0x43a739fc)));
    // 391.097f, 334.453f
    path.line_to((f32::from_bits(0x43c36168), f32::from_bits(0x43a74efa)));
    // 390.761f, 334.617f
    path.line_to((f32::from_bits(0x43c33666), f32::from_bits(0x43a6f7f0)));
    // 390.425f, 333.937f
    path.line_to((f32::from_bits(0x43c36168), f32::from_bits(0x43a6e1ec)));
    // 390.761f, 333.765f
    path.line_to((f32::from_bits(0x43c38c6a), f32::from_bits(0x43a739fc)));
    // 391.097f, 334.453f
    path.close();
    path.move_to((f32::from_bits(0x43c39062), f32::from_bits(0x43a73810)));
    // 391.128f, 334.438f
    path.line_to((f32::from_bits(0x43c3676c), f32::from_bits(0x43a75106)));
    // 390.808f, 334.633f
    path.line_to((f32::from_bits(0x43c33374), f32::from_bits(0x43a6fefa)));
    // 390.402f, 333.992f
    path.line_to((f32::from_bits(0x43c35d70), f32::from_bits(0x43a6e3f8)));
    // 390.73f, 333.781f
    path.line_to((f32::from_bits(0x43c39062), f32::from_bits(0x43a73811)));
    // 391.128f, 334.438f
    path.line_to((f32::from_bits(0x43c39062), f32::from_bits(0x43a73810)));
    // 391.128f, 334.438f
    path.close();
    path.move_to((f32::from_bits(0x43e38958), f32::from_bits(0x43971c08)));
    // 455.073f, 302.219f
    path.line_to((f32::from_bits(0x43e3824e), f32::from_bits(0x43973000)));
    // 455.018f, 302.375f
    path.line_to((f32::from_bits(0x43e36f5c), f32::from_bits(0x439739fc)));
    // 454.87f, 302.453f
    path.line_to((f32::from_bits(0x43e35a5e), f32::from_bits(0x43970df4)));
    // 454.706f, 302.109f
    path.line_to((f32::from_bits(0x43e38958), f32::from_bits(0x43971c08)));
    // 455.073f, 302.219f
    path.close();
    path.move_to((f32::from_bits(0x43e36f5c), f32::from_bits(0x439739fc)));
    // 454.87f, 302.453f
    path.line_to((f32::from_bits(0x43c38c6a), f32::from_bits(0x43a739fc)));
    // 391.097f, 334.453f
    path.line_to((f32::from_bits(0x43c36168), f32::from_bits(0x43a6e1ec)));
    // 390.761f, 333.765f
    path.line_to((f32::from_bits(0x43e3445a), f32::from_bits(0x4396e1ec)));
    // 454.534f, 301.765f
    path.line_to((f32::from_bits(0x43e36f5c), f32::from_bits(0x439739fc)));
    // 454.87f, 302.453f
    path.close();
    path.move_to((f32::from_bits(0x43e41f5c), f32::from_bits(0x43946efa)));
    // 456.245f, 296.867f
    path.line_to((f32::from_bits(0x43e4545a), f32::from_bits(0x439479fc)));
    // 456.659f, 296.953f
    path.line_to((f32::from_bits(0x43e44354), f32::from_bits(0x4394acee)));
    // 456.526f, 297.351f
    path.line_to((f32::from_bits(0x43e41646), f32::from_bits(0x43949efa)));
    // 456.174f, 297.242f
    path.line_to((f32::from_bits(0x43e41f5d), f32::from_bits(0x43946efa)));
    // 456.245f, 296.867f
    path.line_to((f32::from_bits(0x43e41f5c), f32::from_bits(0x43946efa)));
    // 456.245f, 296.867f
    path.close();
    path.move_to((f32::from_bits(0x43e44354), f32::from_bits(0x4394ad0e)));
    // 456.526f, 297.352f
    path.line_to((f32::from_bits(0x43e38958), f32::from_bits(0x43971c08)));
    // 455.073f, 302.219f
    path.line_to((f32::from_bits(0x43e32b64), f32::from_bits(0x43970000)));
    // 454.339f, 302
    path.line_to((f32::from_bits(0x43e3e76c), f32::from_bits(0x43949106)));
    // 455.808f, 297.133f
    path.line_to((f32::from_bits(0x43e44353), f32::from_bits(0x4394ad0e)));
    // 456.526f, 297.352f
    path.line_to((f32::from_bits(0x43e44354), f32::from_bits(0x4394ad0e)));
    // 456.526f, 297.352f
    path.close();
    path.move_to((f32::from_bits(0x43e17d50), f32::from_bits(0x4393f20c)));
    // 450.979f, 295.891f
    path.line_to((f32::from_bits(0x43e18e56), f32::from_bits(0x4393e810)));
    // 451.112f, 295.813f
    path.line_to((f32::from_bits(0x43e1a148), f32::from_bits(0x4393eb02)));
    // 451.26f, 295.836f
    path.line_to((f32::from_bits(0x43e19852), f32::from_bits(0x43941b02)));
    // 451.19f, 296.211f
    path.line_to((f32::from_bits(0x43e17d50), f32::from_bits(0x4393f20c)));
    // 450.979f, 295.891f
    path.close();
    path.move_to((f32::from_bits(0x43e1a169), f32::from_bits(0x4393eb02)));
    // 451.261f, 295.836f
    path.line_to((f32::from_bits(0x43e41f5d), f32::from_bits(0x43946efa)));
    // 456.245f, 296.867f
    path.line_to((f32::from_bits(0x43e40b65), f32::from_bits(0x4394cefa)));
    // 456.089f, 297.617f
    path.line_to((f32::from_bits(0x43e18d71), f32::from_bits(0x43944b02)));
    // 451.105f, 296.586f
    path.line_to((f32::from_bits(0x43e1a169), f32::from_bits(0x4393eb02)));
    // 451.261f, 295.836f
    path.close();
    path.move_to((f32::from_bits(0x43c35d50), f32::from_bits(0x43a6e3f8)));
    // 390.729f, 333.781f
    path.line_to((f32::from_bits(0x43e17d50), f32::from_bits(0x4393f1ec)));
    // 450.979f, 295.89f
    path.line_to((f32::from_bits(0x43e1b148), f32::from_bits(0x439443f8)));
    // 451.385f, 296.531f
    path.line_to((f32::from_bits(0x43c39042), f32::from_bits(0x43a737f0)));
    // 391.127f, 334.437f
    path.line_to((f32::from_bits(0x43c35d50), f32::from_bits(0x43a6e3f8)));
    // 390.729f, 333.781f
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn joel_7(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x4321220c), f32::from_bits(0x43eac70a)));
    // 161.133f, 469.555f
    path.line_to((f32::from_bits(0x4321220c), f32::from_bits(0x43eac70a)));
    // 161.133f, 469.555f
    path.line_to((f32::from_bits(0x431f8e14), f32::from_bits(0x43eb3b02)));
    // 159.555f, 470.461f
    path.line_to((f32::from_bits(0x4321220c), f32::from_bits(0x43eac70a)));
    // 161.133f, 469.555f
    path.close();
    path.move_to((f32::from_bits(0x431e33f8), f32::from_bits(0x43f03b02)));
    // 158.203f, 480.461f
    path.line_to((f32::from_bits(0x431e33f8), f32::from_bits(0x43f03b02)));
    // 158.203f, 480.461f
    path.line_to((f32::from_bits(0x431e33f8), f32::from_bits(0x43f03b02)));
    // 158.203f, 480.461f
    path.line_to((f32::from_bits(0x431d4c08), f32::from_bits(0x43ef720c)));
    // 157.297f, 478.891f
    path.line_to((f32::from_bits(0x431e33f8), f32::from_bits(0x43f03b02)));
    // 158.203f, 480.461f
    path.close();
    path.move_to((f32::from_bits(0x431c6419), f32::from_bits(0x43eea7f0)));
    // 156.391f, 477.312f
    path.cubic_to((f32::from_bits(0x431d6e15), f32::from_bits(0x43ee5ae2)), (f32::from_bits(0x431e2000), f32::from_bits(0x43ede000)), (f32::from_bits(0x431e69fc), f32::from_bits(0x43ed55e4)));
    // 157.43f, 476.71f, 158.125f, 475.75f, 158.414f, 474.671f
    path.cubic_to((f32::from_bits(0x431eb3f8), f32::from_bits(0x43eccbc8)), (f32::from_bits(0x431e93f8), f32::from_bits(0x43ec35e4)), (f32::from_bits(0x431df9db), f32::from_bits(0x43ebafe0)));
    // 158.703f, 473.592f, 158.578f, 472.421f, 157.976f, 471.374f
    path.line_to((f32::from_bits(0x432121cb), f32::from_bits(0x43eac6ea)));
    // 161.132f, 469.554f
    path.cubic_to((f32::from_bits(0x432355c3), f32::from_bits(0x43ecb0e6)), (f32::from_bits(0x432207ae), f32::from_bits(0x43ef1fe0)), (f32::from_bits(0x431e33b7), f32::from_bits(0x43f03ae2)));
    // 163.335f, 473.382f, 162.03f, 478.249f, 158.202f, 480.46f
    path.line_to((f32::from_bits(0x431c6419), f32::from_bits(0x43eea7f0)));
    // 156.391f, 477.312f
    path.close();
    path.move_to((f32::from_bits(0x43134c08), f32::from_bits(0x43eec4fe)));
    // 147.297f, 477.539f
    path.line_to((f32::from_bits(0x43134c08), f32::from_bits(0x43eec4fe)));
    // 147.297f, 477.539f
    path.line_to((f32::from_bits(0x43134c08), f32::from_bits(0x43eec4fe)));
    // 147.297f, 477.539f
    path.line_to((f32::from_bits(0x4314e20c), f32::from_bits(0x43ee5106)));
    // 148.883f, 476.633f
    path.line_to((f32::from_bits(0x43134c08), f32::from_bits(0x43eec4fe)));
    // 147.297f, 477.539f
    path.close();
    path.move_to((f32::from_bits(0x431673f8), f32::from_bits(0x43eddc08)));
    // 150.453f, 475.719f
    path.cubic_to((f32::from_bits(0x43170e15), f32::from_bits(0x43ee620c)), (f32::from_bits(0x43180000), f32::from_bits(0x43eebb02)), (f32::from_bits(0x43191604), f32::from_bits(0x43eee000)));
    // 151.055f, 476.766f, 152, 477.461f, 153.086f, 477.75f
    path.cubic_to((f32::from_bits(0x431a2c08), f32::from_bits(0x43ef04fe)), (f32::from_bits(0x431b5810), f32::from_bits(0x43eef4fe)), (f32::from_bits(0x431c6418), f32::from_bits(0x43eea7f0)));
    // 154.172f, 478.039f, 155.344f, 477.914f, 156.391f, 477.312f
    path.line_to((f32::from_bits(0x431e33f7), f32::from_bits(0x43f03ae2)));
    // 158.203f, 480.46f
    path.cubic_to((f32::from_bits(0x431a620b), f32::from_bits(0x43f154de)), (f32::from_bits(0x4315820c), f32::from_bits(0x43f0add4)), (f32::from_bits(0x43134c07), f32::from_bits(0x43eec4de)));
    // 154.383f, 482.663f, 149.508f, 481.358f, 147.297f, 477.538f
    path.line_to((f32::from_bits(0x431673f8), f32::from_bits(0x43eddc08)));
    // 150.453f, 475.719f
    path.close();
    path.move_to((f32::from_bits(0x43163a1d), f32::from_bits(0x43e95106)));
    // 150.227f, 466.633f
    path.line_to((f32::from_bits(0x43163a1d), f32::from_bits(0x43e95106)));
    // 150.227f, 466.633f
    path.line_to((f32::from_bits(0x4317220d), f32::from_bits(0x43ea19fc)));
    // 151.133f, 468.203f
    path.line_to((f32::from_bits(0x43163a1d), f32::from_bits(0x43e95106)));
    // 150.227f, 466.633f
    path.close();
    path.move_to((f32::from_bits(0x43180c08), f32::from_bits(0x43eae3f8)));
    // 152.047f, 469.781f
    path.cubic_to((f32::from_bits(0x43170000), f32::from_bits(0x43eb31ec)), (f32::from_bits(0x43164e14), f32::from_bits(0x43ebabe8)), (f32::from_bits(0x43160418), f32::from_bits(0x43ec3604)));
    // 151, 470.39f, 150.305f, 471.343f, 150.016f, 472.422f
    path.cubic_to((f32::from_bits(0x4315ba1c), f32::from_bits(0x43ecc106)), (f32::from_bits(0x4315d810), f32::from_bits(0x43ed570a)), (f32::from_bits(0x43167439), f32::from_bits(0x43eddc08)));
    // 149.727f, 473.508f, 149.844f, 474.68f, 150.454f, 475.719f
    path.line_to((f32::from_bits(0x43134c49), f32::from_bits(0x43eec4fe)));
    // 147.298f, 477.539f
    path.cubic_to((f32::from_bits(0x43111851), f32::from_bits(0x43ecdb02)), (f32::from_bits(0x43126830), f32::from_bits(0x43ea6c08)), (f32::from_bits(0x43163a5d), f32::from_bits(0x43e95106)));
    // 145.095f, 473.711f, 146.407f, 468.844f, 150.228f, 466.633f
    path.line_to((f32::from_bits(0x43180c08), f32::from_bits(0x43eae3f8)));
    // 152.047f, 469.781f
    path.close();
    path.move_to((f32::from_bits(0x431dfa1d), f32::from_bits(0x43ebb000)));
    // 157.977f, 471.375f
    path.cubic_to((f32::from_bits(0x431d620d), f32::from_bits(0x43eb29fc)), (f32::from_bits(0x431c6e15), f32::from_bits(0x43ead20c)), (f32::from_bits(0x431b5811), f32::from_bits(0x43eaad0e)));
    // 157.383f, 470.328f, 156.43f, 469.641f, 155.344f, 469.352f
    path.cubic_to((f32::from_bits(0x431a420d), f32::from_bits(0x43ea8810)), (f32::from_bits(0x43191605), f32::from_bits(0x43ea970a)), (f32::from_bits(0x43180c09), f32::from_bits(0x43eae418)));
    // 154.258f, 469.063f, 153.086f, 469.18f, 152.047f, 469.782f
    path.line_to((f32::from_bits(0x43163a1d), f32::from_bits(0x43e95126)));
    // 150.227f, 466.634f
    path.cubic_to((f32::from_bits(0x431a0c09), f32::from_bits(0x43e8372a)), (f32::from_bits(0x431eec08), f32::from_bits(0x43e8de34)), (f32::from_bits(0x4321220d), f32::from_bits(0x43eac72a)));
    // 154.047f, 464.431f, 158.922f, 465.736f, 161.133f, 469.556f
    path.line_to((f32::from_bits(0x431dfa1d), f32::from_bits(0x43ebb000)));
    // 157.977f, 471.375f
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn joel_8(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((f32::from_bits(0x42d97520), f32::from_bits(0x410ac429)));
    // 108.729f, 8.67289f
    path.cubic_to((f32::from_bits(0x42d97520), f32::from_bits(0x410ac429)), (f32::from_bits(0x42e9a9ce), f32::from_bits(0x41834e87)), (f32::from_bits(0x42e99c8c), f32::from_bits(0x41c5c960)));
    // 108.729f, 8.67289f, 116.832f, 16.4133f, 116.806f, 24.7233f
    path.cubic_to((f32::from_bits(0x42e98f49), f32::from_bits(0x4204221c)), (f32::from_bits(0x42d97520), f32::from_bits(0x4223825f)), (f32::from_bits(0x42d97520), f32::from_bits(0x4223825f)));
    // 116.78f, 33.0333f, 108.729f, 40.8773f, 108.729f, 40.8773f
    path.cubic_to((f32::from_bits(0x42d97520), f32::from_bits(0x4223825f)), (f32::from_bits(0x42dbbc54), f32::from_bits(0x42099f18)), (f32::from_bits(0x42d1cb74), f32::from_bits(0x41f77dc0)));
    // 108.729f, 40.8773f, 109.868f, 34.4054f, 104.897f, 30.9364f
    path.cubic_to((f32::from_bits(0x42c7da94), f32::from_bits(0x41dbbd4f)), (f32::from_bits(0x42b1b1a1), f32::from_bits(0x41d802fb)), (f32::from_bits(0x42b1b1a1), f32::from_bits(0x41d802fb)));
    // 99.9269f, 27.4674f, 88.8469f, 27.0015f, 88.8469f, 27.0015f
    path.cubic_to((f32::from_bits(0x42a75637), f32::from_bits(0x41d6909f)), (f32::from_bits(0x4296c543), f32::from_bits(0x41f1b139)), (f32::from_bits(0x4296c543), f32::from_bits(0x41f1b139)));
    // 83.6684f, 26.8206f, 75.3853f, 30.2115f, 75.3853f, 30.2115f
    path.line_to((f32::from_bits(0x42824475), f32::from_bits(0x41c69d70)));
    // 65.1337f, 24.8269f
    path.line_to((f32::from_bits(0x4296c543), f32::from_bits(0x419b89a8)));
    // 75.3853f, 19.4422f
    path.cubic_to((f32::from_bits(0x4296c543), f32::from_bits(0x419b89a8)), (f32::from_bits(0x42a6b798), f32::from_bits(0x41b89815)), (f32::from_bits(0x42b1b1a1), f32::from_bits(0x41b95c48)));
    // 75.3853f, 19.4422f, 83.3586f, 23.0743f, 88.8469f, 23.1701f
    path.cubic_to((f32::from_bits(0x42b1b1a1), f32::from_bits(0x41b95c48)), (f32::from_bits(0x42c80258), f32::from_bits(0x41b03f7a)), (f32::from_bits(0x42d1cb74), f32::from_bits(0x419340ee)));
    // 88.8469f, 23.1701f, 100.005f, 22.031f, 104.897f, 18.4067f
    path.cubic_to((f32::from_bits(0x42db9490), f32::from_bits(0x416c84c2)), (f32::from_bits(0x42d97520), f32::from_bits(0x410ac42a)), (f32::from_bits(0x42d97520), f32::from_bits(0x410ac42a)));
    // 109.79f, 14.7824f, 108.729f, 8.67289f, 108.729f, 8.67289f
    path.line_to((f32::from_bits(0x42d97520), f32::from_bits(0x410ac429)));
    // 108.729f, 8.67289f
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn joel_9(_reporter: &mut Reporter, _filename: &str) {
}

fn joel_10(_reporter: &mut Reporter, _filename: &str) {
}

fn joel_11(_reporter: &mut Reporter, _filename: &str) {
}

fn make_joel_12(path: &mut PathBuilder) {
    path.move_to((f32::from_bits(0x4324e9fc), f32::from_bits(0x437211ec)));
    // 164.914f, 242.07f
    path.line_to((f32::from_bits(0x4324e9fc), f32::from_bits(0x437211ec)));
    // 164.914f, 242.07f
    path.line_to((f32::from_bits(0x4324e9fc), f32::from_bits(0x437211ec)));
    // 164.914f, 242.07f
    path.line_to((f32::from_bits(0x43235810), f32::from_bits(0x437129fc)));
    // 163.344f, 241.164f
    path.line_to((f32::from_bits(0x4324e9fc), f32::from_bits(0x437211ec)));
    // 164.914f, 242.07f
    path.close();
    path.move_to((f32::from_bits(0x431a020c), f32::from_bits(0x4374fdf4)));
    // 154.008f, 244.992f
    path.line_to((f32::from_bits(0x431a020c), f32::from_bits(0x4374fdf4)));
    // 154.008f, 244.992f
    path.line_to((f32::from_bits(0x431a020c), f32::from_bits(0x4374fdf4)));
    // 154.008f, 244.992f
    path.line_to((f32::from_bits(0x431aec08), f32::from_bits(0x437369fc)));
    // 154.922f, 243.414f
    path.line_to((f32::from_bits(0x431a020c), f32::from_bits(0x4374fdf4)));
    // 154.008f, 244.992f
    path.close();
    path.move_to((f32::from_bits(0x431bd3f8), f32::from_bits(0x4371d810)));
    // 155.828f, 241.844f
    path.cubic_to((f32::from_bits(0x431ce000), f32::from_bits(0x4372722d)), (f32::from_bits(0x431e0e15), f32::from_bits(0x43729020)), (f32::from_bits(0x431f2000), f32::from_bits(0x43724831)));
    // 156.875f, 242.446f, 158.055f, 242.563f, 159.125f, 242.282f
    path.cubic_to((f32::from_bits(0x43203604), f32::from_bits(0x4371fe35)), (f32::from_bits(0x43212c08), f32::from_bits(0x43714a3d)), (f32::from_bits(0x4321c5e3), f32::from_bits(0x43704041)));
    // 160.211f, 241.993f, 161.172f, 241.29f, 161.773f, 240.251f
    path.line_to((f32::from_bits(0x4324e9fc), f32::from_bits(0x4372122d)));
    // 164.914f, 242.071f
    path.cubic_to((f32::from_bits(0x4322b3f8), f32::from_bits(0x4375e419)), (f32::from_bits(0x431dd810), f32::from_bits(0x4377322d)), (f32::from_bits(0x431a020c), f32::from_bits(0x4374fe35)));
    // 162.703f, 245.891f, 157.844f, 247.196f, 154.008f, 244.993f
    path.line_to((f32::from_bits(0x431bd3f8), f32::from_bits(0x4371d810)));
    // 155.828f, 241.844f
    path.close();
    path.move_to((f32::from_bits(0x43171810), f32::from_bits(0x436a1604)));
    // 151.094f, 234.086f
    path.line_to((f32::from_bits(0x43171810), f32::from_bits(0x436a1604)));
    // 151.094f, 234.086f
    path.line_to((f32::from_bits(0x43171810), f32::from_bits(0x436a1604)));
    // 151.094f, 234.086f
    path.line_to((f32::from_bits(0x4318a9fc), f32::from_bits(0x436afdf4)));
    // 152.664f, 234.992f
    path.line_to((f32::from_bits(0x43171810), f32::from_bits(0x436a1604)));
    // 151.094f, 234.086f
    path.close();
    path.move_to((f32::from_bits(0x431a4000), f32::from_bits(0x436be7f0)));
    // 154.25f, 235.906f
    path.cubic_to((f32::from_bits(0x4319a20c), f32::from_bits(0x436cf3f8)), (f32::from_bits(0x431985e3), f32::from_bits(0x436e1df4)), (f32::from_bits(0x4319ce14), f32::from_bits(0x436f33f8)));
    // 153.633f, 236.953f, 153.523f, 238.117f, 153.805f, 239.203f
    path.cubic_to((f32::from_bits(0x431a1a1c), f32::from_bits(0x437047f0)), (f32::from_bits(0x431ac831), f32::from_bits(0x43713df4)), (f32::from_bits(0x431bd3f7), f32::from_bits(0x4371d811)));
    // 154.102f, 240.281f, 154.782f, 241.242f, 155.828f, 241.844f
    path.line_to((f32::from_bits(0x431a020b), f32::from_bits(0x4374fdf4)));
    // 154.008f, 244.992f
    path.cubic_to((f32::from_bits(0x4316322c), f32::from_bits(0x4372c5e4)), (f32::from_bits(0x4314e417), f32::from_bits(0x436de9fc)), (f32::from_bits(0x4317180f), f32::from_bits(0x436a1604)));
    // 150.196f, 242.773f, 148.891f, 237.914f, 151.094f, 234.086f
    path.line_to((f32::from_bits(0x431a4000), f32::from_bits(0x436be7f0)));
    // 154.25f, 235.906f
    path.close();
    path.move_to((f32::from_bits(0x43220000), f32::from_bits(0x436729fc)));
    // 162, 231.164f
    path.line_to((f32::from_bits(0x43220000), f32::from_bits(0x436729fc)));
    // 162, 231.164f
    path.line_to((f32::from_bits(0x43220000), f32::from_bits(0x436729fc)));
    // 162, 231.164f
    path.line_to((f32::from_bits(0x43211810), f32::from_bits(0x4368bbe8)));
    // 161.094f, 232.734f
    path.line_to((f32::from_bits(0x43220000), f32::from_bits(0x436729fc)));
    // 162, 231.164f
    path.close();
    path.move_to((f32::from_bits(0x43202e14), f32::from_bits(0x436a4fdf)));
    // 160.18f, 234.312f
    path.cubic_to((f32::from_bits(0x431f2418), f32::from_bits(0x4369b5c2)), (f32::from_bits(0x431df810), f32::from_bits(0x436995c2)), (f32::from_bits(0x431ce20c), f32::from_bits(0x4369dfbe)));
    // 159.141f, 233.71f, 157.969f, 233.585f, 156.883f, 233.874f
    path.cubic_to((f32::from_bits(0x431bcc08), f32::from_bits(0x436a2bc6)), (f32::from_bits(0x431ad810), f32::from_bits(0x436adba5)), (f32::from_bits(0x431a4000), f32::from_bits(0x436be7ae)));
    // 155.797f, 234.171f, 154.844f, 234.858f, 154.25f, 235.905f
    path.line_to((f32::from_bits(0x43171810), f32::from_bits(0x436a15c2)));
    // 151.094f, 234.085f
    path.cubic_to((f32::from_bits(0x43194e14), f32::from_bits(0x436643d6)), (f32::from_bits(0x431e2c08), f32::from_bits(0x4364f3b6)), (f32::from_bits(0x43220000), f32::from_bits(0x436729ba)));
    // 153.305f, 230.265f, 158.172f, 228.952f, 162, 231.163f
    path.line_to((f32::from_bits(0x43202e14), f32::from_bits(0x436a4fdf)));
    // 160.18f, 234.312f
    path.close();
    path.move_to((f32::from_bits(0x4321c5e3), f32::from_bits(0x43704000)));
    // 161.773f, 240.25f
    path.cubic_to((f32::from_bits(0x43226000), f32::from_bits(0x436f3604)), (f32::from_bits(0x43228000), f32::from_bits(0x436e09fc)), (f32::from_bits(0x43223604), f32::from_bits(0x436cf3f8)));
    // 162.375f, 239.211f, 162.5f, 238.039f, 162.211f, 236.953f
    path.cubic_to((f32::from_bits(0x4321ec08), f32::from_bits(0x436be000)), (f32::from_bits(0x43213a1d), f32::from_bits(0x436ae9fc)), (f32::from_bits(0x43202e14), f32::from_bits(0x436a4fdf)));
    // 161.922f, 235.875f, 161.227f, 234.914f, 160.18f, 234.312f
    path.line_to((f32::from_bits(0x43220000), f32::from_bits(0x436729fc)));
    // 162, 231.164f
    path.cubic_to((f32::from_bits(0x4325d1ec), f32::from_bits(0x43696000)), (f32::from_bits(0x4327220c), f32::from_bits(0x436e4000)), (f32::from_bits(0x4324e9fc), f32::from_bits(0x437211ec)));
    // 165.82f, 233.375f, 167.133f, 238.25f, 164.914f, 242.07f
    path.line_to((f32::from_bits(0x4321c5e3), f32::from_bits(0x43704000)));
    // 161.773f, 240.25f
    path.close();
}

fn joel_12(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    make_joel_12(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn joel_12x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    make_joel_12(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn make_joel_13(path: &mut PathBuilder) {
    path.move_to((f32::from_bits(0x43b4126f), f32::from_bits(0x43c058f6)));
    // 360.144f, 384.695f
    path.cubic_to((f32::from_bits(0x43bd7c6b), f32::from_bits(0x43c05b02)), (f32::from_bits(0x43c51d71), f32::from_bits(0x43b8e8f6)), (f32::from_bits(0x43c5276d), f32::from_bits(0x43afc1ec)));
    // 378.972f, 384.711f, 394.23f, 369.82f, 394.308f, 351.515f
    path.cubic_to((f32::from_bits(0x43c51d71), f32::from_bits(0x43a688f6)), (f32::from_bits(0x43bd7c6b), f32::from_bits(0x439f16ea)), (f32::from_bits(0x43b4126f), f32::from_bits(0x439f16ea)));
    // 394.23f, 333.07f, 378.972f, 318.179f, 360.144f, 318.179f
    path.cubic_to((f32::from_bits(0x43aaa979), f32::from_bits(0x439f16ea)), (f32::from_bits(0x43a3076d), f32::from_bits(0x43a688f6)), (f32::from_bits(0x43a31063), f32::from_bits(0x43afc1ec)));
    // 341.324f, 318.179f, 326.058f, 333.07f, 326.128f, 351.515f
    path.cubic_to((f32::from_bits(0x43a3076d), f32::from_bits(0x43b8e8f6)), (f32::from_bits(0x43aaa959), f32::from_bits(0x43c05b02)), (f32::from_bits(0x43b4126f), f32::from_bits(0x43c058f6)));
    // 326.058f, 369.82f, 341.323f, 384.711f, 360.144f, 384.695f
    path.close();
}

fn joel_13(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    make_joel_13(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn joel_13x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    make_joel_13(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn make_joel_14(path: &mut PathBuilder) {
    path.move_to((f32::from_bits(0x43f3b354), f32::from_bits(0x43d6770a)));
    // 487.401f, 428.93f
    path.line_to((f32::from_bits(0x43f3b354), f32::from_bits(0x43d6770a)));
    // 487.401f, 428.93f
    path.close();
    path.move_to((f32::from_bits(0x43f0fd50), f32::from_bits(0x43d6770a)));
    // 481.979f, 428.93f
    path.line_to((f32::from_bits(0x43f0fd50), f32::from_bits(0x43d6770a)));
    // 481.979f, 428.93f
    path.close();
    path.move_to((f32::from_bits(0x43f0fd50), f32::from_bits(0x43d6770a)));
    // 481.979f, 428.93f
    path.line_to((f32::from_bits(0x43f3b354), f32::from_bits(0x43d6770a)));
    // 487.401f, 428.93f
    path.line_to((f32::from_bits(0x43f0fd50), f32::from_bits(0x43d6770a)));
    // 481.979f, 428.93f
    path.close();
    path.move_to((f32::from_bits(0x43dfe76d), f32::from_bits(0x43d792f1)));
    // 447.808f, 431.148f
    path.line_to((f32::from_bits(0x43dfe76d), f32::from_bits(0x43d792f1)));
    // 447.808f, 431.148f
    path.close();
    path.move_to((f32::from_bits(0x43dfe76d), f32::from_bits(0x43d792f1)));
    // 447.808f, 431.148f
    path.cubic_to((f32::from_bits(0x43e51979), f32::from_bits(0x43d611eb)), (f32::from_bits(0x43eb8667), f32::from_bits(0x43d765e3)), (f32::from_bits(0x43f0fd71), f32::from_bits(0x43d676e9)));
    // 458.199f, 428.14f, 471.05f, 430.796f, 481.98f, 428.929f
    path.cubic_to((f32::from_bits(0x43eb8667), f32::from_bits(0x43d76604)), (f32::from_bits(0x43e51958), f32::from_bits(0x43d6120c)), (f32::from_bits(0x43dfe76d), f32::from_bits(0x43d792f1)));
    // 471.05f, 430.797f, 458.198f, 428.141f, 447.808f, 431.148f
    path.close();
    path.move_to((f32::from_bits(0x43df776d), f32::from_bits(0x43d6d603)));
    // 446.933f, 429.672f
    path.line_to((f32::from_bits(0x43df776d), f32::from_bits(0x43d6d603)));
    // 446.933f, 429.672f
    path.close();
    path.move_to((f32::from_bits(0x43df776d), f32::from_bits(0x43d6d603)));
    // 446.933f, 429.672f
    path.line_to((f32::from_bits(0x43dfe76d), f32::from_bits(0x43d79311)));
    // 447.808f, 431.149f
    path.line_to((f32::from_bits(0x43df776d), f32::from_bits(0x43d6d603)));
    // 446.933f, 429.672f
    path.close();
    path.move_to((f32::from_bits(0x43dd3169), f32::from_bits(0x43d792f1)));
    // 442.386f, 431.148f
    path.line_to((f32::from_bits(0x43dd3169), f32::from_bits(0x43d792f1)));
    // 442.386f, 431.148f
    path.close();
    path.move_to((f32::from_bits(0x43dd3169), f32::from_bits(0x43d792f1)));
    // 442.386f, 431.148f
    path.cubic_to((f32::from_bits(0x43de376d), f32::from_bits(0x43d743f7)), (f32::from_bits(0x43de2873), f32::from_bits(0x43d68df3)), (f32::from_bits(0x43df776d), f32::from_bits(0x43d6d5e3)));
    // 444.433f, 430.531f, 444.316f, 429.109f, 446.933f, 429.671f
    path.cubic_to((f32::from_bits(0x43de2852), f32::from_bits(0x43d68df3)), (f32::from_bits(0x43de376d), f32::from_bits(0x43d743f7)), (f32::from_bits(0x43dd3169), f32::from_bits(0x43d792f1)));
    // 444.315f, 429.109f, 444.433f, 430.531f, 442.386f, 431.148f
    path.close();
    path.move_to((f32::from_bits(0x43dcc169), f32::from_bits(0x43d6d603)));
    // 441.511f, 429.672f
    path.line_to((f32::from_bits(0x43dcc169), f32::from_bits(0x43d6d603)));
    // 441.511f, 429.672f
    path.close();
    path.move_to((f32::from_bits(0x43dcc169), f32::from_bits(0x43d6d603)));
    // 441.511f, 429.672f
    path.line_to((f32::from_bits(0x43dd3169), f32::from_bits(0x43d79311)));
    // 442.386f, 431.149f
    path.line_to((f32::from_bits(0x43dcc169), f32::from_bits(0x43d6d603)));
    // 441.511f, 429.672f
    path.close();
    path.move_to((f32::from_bits(0x43dad959), f32::from_bits(0x43d6d603)));
    // 437.698f, 429.672f
    path.line_to((f32::from_bits(0x43dad959), f32::from_bits(0x43d6d603)));
    // 437.698f, 429.672f
    path.close();
    path.move_to((f32::from_bits(0x43dad959), f32::from_bits(0x43d6d603)));
    // 437.698f, 429.672f
    path.line_to((f32::from_bits(0x43dcc149), f32::from_bits(0x43d6d603)));
    // 441.51f, 429.672f
    path.line_to((f32::from_bits(0x43dad959), f32::from_bits(0x43d6d603)));
    // 437.698f, 429.672f
    path.close();
    path.move_to((f32::from_bits(0x43e3cb65), f32::from_bits(0x43e3bd0d)));
    // 455.589f, 455.477f
    path.line_to((f32::from_bits(0x43e3cb65), f32::from_bits(0x43e3bd0d)));
    // 455.589f, 455.477f
    path.close();
    path.move_to((f32::from_bits(0x43e3cb65), f32::from_bits(0x43e3bd0d)));
    // 455.589f, 455.477f
    path.line_to((f32::from_bits(0x43dad959), f32::from_bits(0x43d6d603)));
    // 437.698f, 429.672f
    path.line_to((f32::from_bits(0x43e3cb65), f32::from_bits(0x43e3bd0d)));
    // 455.589f, 455.477f
    path.close();
    path.move_to((f32::from_bits(0x43e22d51), f32::from_bits(0x43e41b01)));
    // 452.354f, 456.211f
    path.line_to((f32::from_bits(0x43e22d51), f32::from_bits(0x43e41b01)));
    // 452.354f, 456.211f
    path.close();
    path.move_to((f32::from_bits(0x43e22d51), f32::from_bits(0x43e41b01)));
    // 452.354f, 456.211f
    path.cubic_to((f32::from_bits(0x43e2ba5f), f32::from_bits(0x43e3f9fb)), (f32::from_bits(0x43e37e57), f32::from_bits(0x43e46df3)), (f32::from_bits(0x43e3cb45), f32::from_bits(0x43e3bd0d)));
    // 453.456f, 455.953f, 454.987f, 456.859f, 455.588f, 455.477f
    path.cubic_to((f32::from_bits(0x43e37e57), f32::from_bits(0x43e46df2)), (f32::from_bits(0x43e2ba60), f32::from_bits(0x43e3f9fb)), (f32::from_bits(0x43e22d51), f32::from_bits(0x43e41b01)));
    // 454.987f, 456.859f, 453.456f, 455.953f, 452.354f, 456.211f
    path.close();
    path.move_to((f32::from_bits(0x43e22d51), f32::from_bits(0x43e479fb)));
    // 452.354f, 456.953f
    path.line_to((f32::from_bits(0x43e22d51), f32::from_bits(0x43e479fb)));
    // 452.354f, 456.953f
    path.close();
    path.move_to((f32::from_bits(0x43e22d51), f32::from_bits(0x43e479fb)));
    // 452.354f, 456.953f
    path.line_to((f32::from_bits(0x43e22d51), f32::from_bits(0x43e41b01)));
    // 452.354f, 456.211f
    path.line_to((f32::from_bits(0x43e22d51), f32::from_bits(0x43e479fb)));
    // 452.354f, 456.953f
    path.close();
    path.move_to((f32::from_bits(0x43e35a5f), f32::from_bits(0x43e479fb)));
    // 454.706f, 456.953f
    path.line_to((f32::from_bits(0x43e35a5f), f32::from_bits(0x43e479fb)));
    // 454.706f, 456.953f
    path.close();
    path.move_to((f32::from_bits(0x43e35a5f), f32::from_bits(0x43e479fb)));
    // 454.706f, 456.953f
    path.line_to((f32::from_bits(0x43e22d51), f32::from_bits(0x43e479fb)));
    // 452.354f, 456.953f
    path.line_to((f32::from_bits(0x43e35a5f), f32::from_bits(0x43e479fb)));
    // 454.706f, 456.953f
    path.close();
    path.move_to((f32::from_bits(0x43e35a5f), f32::from_bits(0x43e41b01)));
    // 454.706f, 456.211f
    path.line_to((f32::from_bits(0x43e35a5f), f32::from_bits(0x43e41b01)));
    // 454.706f, 456.211f
    path.close();
    path.move_to((f32::from_bits(0x43e35a5f), f32::from_bits(0x43e41b01)));
    // 454.706f, 456.211f
    path.line_to((f32::from_bits(0x43e35a5f), f32::from_bits(0x43e479fb)));
    // 454.706f, 456.953f
    path.line_to((f32::from_bits(0x43e35a5f), f32::from_bits(0x43e41b01)));
    // 454.706f, 456.211f
    path.close();
    path.move_to((f32::from_bits(0x43e1726f), f32::from_bits(0x43e90c07)));
    // 450.894f, 466.094f
    path.line_to((f32::from_bits(0x43e1726f), f32::from_bits(0x43e90c07)));
    // 450.894f, 466.094f
    path.close();
    path.move_to((f32::from_bits(0x43e1726f), f32::from_bits(0x43e90c07)));
    // 450.894f, 466.094f
    path.cubic_to((f32::from_bits(0x43e2226f), f32::from_bits(0x43e769fb)), (f32::from_bits(0x43e50a7f), f32::from_bits(0x43e63915)), (f32::from_bits(0x43e35a5f), f32::from_bits(0x43e41b01)));
    // 452.269f, 462.828f, 458.082f, 460.446f, 454.706f, 456.211f
    path.cubic_to((f32::from_bits(0x43e50a5f), f32::from_bits(0x43e638f5)), (f32::from_bits(0x43e2226f), f32::from_bits(0x43e769fb)), (f32::from_bits(0x43e1726f), f32::from_bits(0x43e90c07)));
    // 458.081f, 460.445f, 452.269f, 462.828f, 450.894f, 466.094f
    path.close();
    path.move_to((f32::from_bits(0x43f09f5d), f32::from_bits(0x43ea2709)));
    // 481.245f, 468.305f
    path.line_to((f32::from_bits(0x43f09f5d), f32::from_bits(0x43ea2709)));
    // 481.245f, 468.305f
    path.close();
    path.move_to((f32::from_bits(0x43f09f5d), f32::from_bits(0x43ea2709)));
    // 481.245f, 468.305f
    path.cubic_to((f32::from_bits(0x43ebbc6b), f32::from_bits(0x43ea4105)), (f32::from_bits(0x43e56c6b), f32::from_bits(0x43ec9fff)), (f32::from_bits(0x43e1724f), f32::from_bits(0x43e90c07)));
    // 471.472f, 468.508f, 458.847f, 473.25f, 450.893f, 466.094f
    path.cubic_to((f32::from_bits(0x43e56c6c), f32::from_bits(0x43ec9fff)), (f32::from_bits(0x43ebbc6c), f32::from_bits(0x43ea4105)), (f32::from_bits(0x43f09f5d), f32::from_bits(0x43ea2709)));
    // 458.847f, 473.25f, 471.472f, 468.508f, 481.245f, 468.305f
    path.close();
    path.move_to((f32::from_bits(0x43eea45b), f32::from_bits(0x43e9c7ee)));
    // 477.284f, 467.562f
    path.line_to((f32::from_bits(0x43eea45b), f32::from_bits(0x43e9c7ee)));
    // 477.284f, 467.562f
    path.close();
    path.move_to((f32::from_bits(0x43eea45b), f32::from_bits(0x43e9c7ee)));
    // 477.284f, 467.562f
    path.cubic_to((f32::from_bits(0x43ef0c4b), f32::from_bits(0x43ea7ef8)), (f32::from_bits(0x43eff355), f32::from_bits(0x43ea10e4)), (f32::from_bits(0x43f09f5d), f32::from_bits(0x43ea26e8)));
    // 478.096f, 468.992f, 479.901f, 468.132f, 481.245f, 468.304f
    path.cubic_to((f32::from_bits(0x43eff355), f32::from_bits(0x43ea1105)), (f32::from_bits(0x43ef0c6b), f32::from_bits(0x43ea7ef8)), (f32::from_bits(0x43eea45b), f32::from_bits(0x43e9c7ee)));
    // 479.901f, 468.133f, 478.097f, 468.992f, 477.284f, 467.562f
    path.close();
    path.move_to((f32::from_bits(0x43ee4667), f32::from_bits(0x43ea2709)));
    // 476.55f, 468.305f
    path.line_to((f32::from_bits(0x43ee4667), f32::from_bits(0x43ea2709)));
    // 476.55f, 468.305f
    path.close();
    path.move_to((f32::from_bits(0x43ee4667), f32::from_bits(0x43ea2709)));
    // 476.55f, 468.305f
    path.line_to((f32::from_bits(0x43eea45b), f32::from_bits(0x43e9c80f)));
    // 477.284f, 467.563f
    path.line_to((f32::from_bits(0x43ee4667), f32::from_bits(0x43ea2709)));
    // 476.55f, 468.305f
    path.close();
    path.move_to((f32::from_bits(0x43e9f26f), f32::from_bits(0x43e6c2f0)));
    // 467.894f, 461.523f
    path.line_to((f32::from_bits(0x43e9f26f), f32::from_bits(0x43e6c2f0)));
    // 467.894f, 461.523f
    path.close();
    path.move_to((f32::from_bits(0x43e9f26f), f32::from_bits(0x43e6c2f0)));
    // 467.894f, 461.523f
    path.cubic_to((f32::from_bits(0x43eb8873), f32::from_bits(0x43e7dcec)), (f32::from_bits(0x43eb747b), f32::from_bits(0x43ea9b00)), (f32::from_bits(0x43ee4667), f32::from_bits(0x43ea26e8)));
    // 471.066f, 463.726f, 470.91f, 469.211f, 476.55f, 468.304f
    path.cubic_to((f32::from_bits(0x43eb745b), f32::from_bits(0x43ea9b01)), (f32::from_bits(0x43eb8853), f32::from_bits(0x43e7dd0d)), (f32::from_bits(0x43e9f26f), f32::from_bits(0x43e6c2f0)));
    // 470.909f, 469.211f, 471.065f, 463.727f, 467.894f, 461.523f
    path.close();
    path.move_to((f32::from_bits(0x43ebee56), f32::from_bits(0x43decc07)));
    // 471.862f, 445.594f
    path.line_to((f32::from_bits(0x43ebee56), f32::from_bits(0x43decc07)));
    // 471.862f, 445.594f
    path.close();
    path.move_to((f32::from_bits(0x43ebee56), f32::from_bits(0x43decc07)));
    // 471.862f, 445.594f
    path.cubic_to((f32::from_bits(0x43e85f5c), f32::from_bits(0x43e04915)), (f32::from_bits(0x43eaa148), f32::from_bits(0x43e41c07)), (f32::from_bits(0x43e9f24e), f32::from_bits(0x43e6c311)));
    // 464.745f, 448.571f, 469.26f, 456.219f, 467.893f, 461.524f
    path.cubic_to((f32::from_bits(0x43eaa169), f32::from_bits(0x43e41c07)), (f32::from_bits(0x43e85f5c), f32::from_bits(0x43e048f4)), (f32::from_bits(0x43ebee56), f32::from_bits(0x43decc07)));
    // 469.261f, 456.219f, 464.745f, 448.57f, 471.862f, 445.594f
    path.close();
    path.move_to((f32::from_bits(0x43eac168), f32::from_bits(0x43dd3fff)));
    // 469.511f, 442.5f
    path.line_to((f32::from_bits(0x43eac168), f32::from_bits(0x43dd3fff)));
    // 469.511f, 442.5f
    path.close();
    path.move_to((f32::from_bits(0x43eac168), f32::from_bits(0x43dd3fff)));
    // 469.511f, 442.5f
    path.cubic_to((f32::from_bits(0x43eb245a), f32::from_bits(0x43ddc7ef)), (f32::from_bits(0x43eaf45a), f32::from_bits(0x43dedd0d)), (f32::from_bits(0x43ebee76), f32::from_bits(0x43decc07)));
    // 470.284f, 443.562f, 469.909f, 445.727f, 471.863f, 445.594f
    path.cubic_to((f32::from_bits(0x43eaf459), f32::from_bits(0x43dedd0d)), (f32::from_bits(0x43eb2459), f32::from_bits(0x43ddc7ee)), (f32::from_bits(0x43eac168), f32::from_bits(0x43dd3fff)));
    // 469.909f, 445.727f, 470.284f, 443.562f, 469.511f, 442.5f
    path.close();
    path.move_to((f32::from_bits(0x43ec4c6a), f32::from_bits(0x43dce105)));
    // 472.597f, 441.758f
    path.line_to((f32::from_bits(0x43ec4c6a), f32::from_bits(0x43dce105)));
    // 472.597f, 441.758f
    path.close();
    path.move_to((f32::from_bits(0x43ec4c6a), f32::from_bits(0x43dce105)));
    // 472.597f, 441.758f
    path.cubic_to((f32::from_bits(0x43ebcb64), f32::from_bits(0x43dd08f5)), (f32::from_bits(0x43eb0c6a), f32::from_bits(0x43dc9603)), (f32::from_bits(0x43eac168), f32::from_bits(0x43dd3fff)));
    // 471.589f, 442.07f, 470.097f, 441.172f, 469.511f, 442.5f
    path.cubic_to((f32::from_bits(0x43eb0c6a), f32::from_bits(0x43dc9603)), (f32::from_bits(0x43ebcb64), f32::from_bits(0x43dd08f5)), (f32::from_bits(0x43ec4c6a), f32::from_bits(0x43dce105)));
    // 470.097f, 441.172f, 471.589f, 442.07f, 472.597f, 441.758f
    path.close();
    path.move_to((f32::from_bits(0x43ecbb64), f32::from_bits(0x43ddb105)));
    // 473.464f, 443.383f
    path.line_to((f32::from_bits(0x43ecbb64), f32::from_bits(0x43ddb105)));
    // 473.464f, 443.383f
    path.close();
    path.move_to((f32::from_bits(0x43ecbb64), f32::from_bits(0x43ddb105)));
    // 473.464f, 443.383f
    path.line_to((f32::from_bits(0x43ec4c6a), f32::from_bits(0x43dce105)));
    // 472.597f, 441.758f
    path.line_to((f32::from_bits(0x43ecbb64), f32::from_bits(0x43ddb105)));
    // 473.464f, 443.383f
    path.close();
    path.move_to((f32::from_bits(0x43eea45a), f32::from_bits(0x43dc24fd)));
    // 477.284f, 440.289f
    path.line_to((f32::from_bits(0x43eea45a), f32::from_bits(0x43dc24fd)));
    // 477.284f, 440.289f
    path.close();
    path.move_to((f32::from_bits(0x43eea45a), f32::from_bits(0x43dc24fd)));
    // 477.284f, 440.289f
    path.cubic_to((f32::from_bits(0x43eef354), f32::from_bits(0x43dd4c07)), (f32::from_bits(0x43ed4a5e), f32::from_bits(0x43dcfef9)), (f32::from_bits(0x43ecbb64), f32::from_bits(0x43ddb105)));
    // 477.901f, 442.594f, 474.581f, 441.992f, 473.464f, 443.383f
    path.cubic_to((f32::from_bits(0x43ed4a5e), f32::from_bits(0x43dcfef9)), (f32::from_bits(0x43eef354), f32::from_bits(0x43dd4c07)), (f32::from_bits(0x43eea45a), f32::from_bits(0x43dc24fd)));
    // 474.581f, 441.992f, 477.901f, 442.594f, 477.284f, 440.289f
    path.close();
    path.move_to((f32::from_bits(0x43f09f5c), f32::from_bits(0x43dc24fd)));
    // 481.245f, 440.289f
    path.line_to((f32::from_bits(0x43f09f5c), f32::from_bits(0x43dc24fd)));
    // 481.245f, 440.289f
    path.close();
    path.move_to((f32::from_bits(0x43f09f5c), f32::from_bits(0x43dc24fd)));
    // 481.245f, 440.289f
    path.cubic_to((f32::from_bits(0x43effc6a), f32::from_bits(0x43daeced)), (f32::from_bits(0x43ef6a5e), f32::from_bits(0x43dbe4fd)), (f32::from_bits(0x43eea45a), f32::from_bits(0x43dc24fd)));
    // 479.972f, 437.851f, 478.831f, 439.789f, 477.284f, 440.289f
    path.cubic_to((f32::from_bits(0x43ef6a5e), f32::from_bits(0x43dbe4fd)), (f32::from_bits(0x43effc6a), f32::from_bits(0x43daed0d)), (f32::from_bits(0x43f09f5c), f32::from_bits(0x43dc24fd)));
    // 478.831f, 439.789f, 479.972f, 437.852f, 481.245f, 440.289f
    path.close();
    path.move_to((f32::from_bits(0x43f2f76c), f32::from_bits(0x43dbc603)));
    // 485.933f, 439.547f
    path.line_to((f32::from_bits(0x43f2f76c), f32::from_bits(0x43dbc603)));
    // 485.933f, 439.547f
    path.close();
    path.move_to((f32::from_bits(0x43f2f76c), f32::from_bits(0x43dbc603)));
    // 485.933f, 439.547f
    path.cubic_to((f32::from_bits(0x43f24c6a), f32::from_bits(0x43dc3b01)), (f32::from_bits(0x43f16b64), f32::from_bits(0x43dc2311)), (f32::from_bits(0x43f09f5c), f32::from_bits(0x43dc24fd)));
    // 484.597f, 440.461f, 482.839f, 440.274f, 481.245f, 440.289f
    path.cubic_to((f32::from_bits(0x43f16b64), f32::from_bits(0x43dc23f7)), (f32::from_bits(0x43f24c6a), f32::from_bits(0x43dc3b01)), (f32::from_bits(0x43f2f76c), f32::from_bits(0x43dbc603)));
    // 482.839f, 440.281f, 484.597f, 440.461f, 485.933f, 439.547f
    path.close();
    path.move_to((f32::from_bits(0x43f4de55), f32::from_bits(0x43d97d0d)));
    // 489.737f, 434.977f
    path.line_to((f32::from_bits(0x43f4de55), f32::from_bits(0x43d97d0d)));
    // 489.737f, 434.977f
    path.close();
    path.move_to((f32::from_bits(0x43f4de55), f32::from_bits(0x43d97d0d)));
    // 489.737f, 434.977f
    path.cubic_to((f32::from_bits(0x43f47665), f32::from_bits(0x43da020b)), (f32::from_bits(0x43f42851), f32::from_bits(0x43db9417)), (f32::from_bits(0x43f2f74b), f32::from_bits(0x43dbc603)));
    // 488.925f, 436.016f, 488.315f, 439.157f, 485.932f, 439.547f
    path.cubic_to((f32::from_bits(0x43f42851), f32::from_bits(0x43db93f7)), (f32::from_bits(0x43f47666), f32::from_bits(0x43da020b)), (f32::from_bits(0x43f4de55), f32::from_bits(0x43d97d0d)));
    // 488.315f, 439.156f, 488.925f, 436.016f, 489.737f, 434.977f
    path.close();
    path.move_to((f32::from_bits(0x43f48061), f32::from_bits(0x43d97d0d)));
    // 489.003f, 434.977f
    path.line_to((f32::from_bits(0x43f48061), f32::from_bits(0x43d97d0d)));
    // 489.003f, 434.977f
    path.close();
    path.move_to((f32::from_bits(0x43f48061), f32::from_bits(0x43d97d0d)));
    // 489.003f, 434.977f
    path.line_to((f32::from_bits(0x43f4de55), f32::from_bits(0x43d97d0d)));
    // 489.737f, 434.977f
    path.line_to((f32::from_bits(0x43f48061), f32::from_bits(0x43d97d0d)));
    // 489.003f, 434.977f
    path.close();
    path.move_to((f32::from_bits(0x43f3b353), f32::from_bits(0x43d67709)));
    // 487.401f, 428.93f
    path.cubic_to((f32::from_bits(0x43f39957), f32::from_bits(0x43d79ef9)), (f32::from_bits(0x43f3ca5d), f32::from_bits(0x43d8a603)), (f32::from_bits(0x43f48061), f32::from_bits(0x43d97d0d)));
    // 487.198f, 431.242f, 487.581f, 433.297f, 489.003f, 434.977f
    path.cubic_to((f32::from_bits(0x43f3ca5d), f32::from_bits(0x43d8a603)), (f32::from_bits(0x43f39957), f32::from_bits(0x43d79ef9)), (f32::from_bits(0x43f3b353), f32::from_bits(0x43d67709)));
    // 487.581f, 433.297f, 487.198f, 431.242f, 487.401f, 428.93f
    path.close();
}

fn joel_14(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    make_joel_14(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn joel_14x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    make_joel_14(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn make_joel_15(path: &mut PathBuilder) {
    path.move_to((f32::from_bits(0x439e276d), f32::from_bits(0x43dad106)));
    // 316.308f, 437.633f
    path.line_to((f32::from_bits(0x439e276d), f32::from_bits(0x43dad106)));
    // 316.308f, 437.633f
    path.close();
    path.move_to((f32::from_bits(0x439c1959), f32::from_bits(0x43d78000)));
    // 312.198f, 431
    path.line_to((f32::from_bits(0x439c1959), f32::from_bits(0x43d78000)));
    // 312.198f, 431
    path.close();
    path.move_to((f32::from_bits(0x439c1959), f32::from_bits(0x43d78000)));
    // 312.198f, 431
    path.cubic_to((f32::from_bits(0x439ea45b), f32::from_bits(0x43d6d000)), (f32::from_bits(0x439cce57), f32::from_bits(0x43d9f3f8)), (f32::from_bits(0x439e274d), f32::from_bits(0x43dad106)));
    // 317.284f, 429.625f, 313.612f, 435.906f, 316.307f, 437.633f
    path.cubic_to((f32::from_bits(0x439cce57), f32::from_bits(0x43d9f3f8)), (f32::from_bits(0x439ea45b), f32::from_bits(0x43d6d000)), (f32::from_bits(0x439c1959), f32::from_bits(0x43d78000)));
    // 313.612f, 435.906f, 317.284f, 429.625f, 312.198f, 431
    path.close();
    path.move_to((f32::from_bits(0x439c1959), f32::from_bits(0x43d8f8f6)));
    // 312.198f, 433.945f
    path.line_to((f32::from_bits(0x439c1959), f32::from_bits(0x43d8f8f6)));
    // 312.198f, 433.945f
    path.close();
    path.move_to((f32::from_bits(0x439c1959), f32::from_bits(0x43d8f8f6)));
    // 312.198f, 433.945f
    path.line_to((f32::from_bits(0x439c1959), f32::from_bits(0x43d78000)));
    // 312.198f, 431
    path.line_to((f32::from_bits(0x439c1959), f32::from_bits(0x43d8f8f6)));
    // 312.198f, 433.945f
    path.close();
    path.move_to((f32::from_bits(0x439f7853), f32::from_bits(0x43e5820c)));
    // 318.94f, 459.016f
    path.line_to((f32::from_bits(0x439f7853), f32::from_bits(0x43e5820c)));
    // 318.94f, 459.016f
    path.close();
    path.move_to((f32::from_bits(0x439f7853), f32::from_bits(0x43e5820c)));
    // 318.94f, 459.016f
    path.cubic_to((f32::from_bits(0x439e1647), f32::from_bits(0x43e17106)), (f32::from_bits(0x439d945b), f32::from_bits(0x43dd020c)), (f32::from_bits(0x439c1959), f32::from_bits(0x43d8f916)));
    // 316.174f, 450.883f, 315.159f, 442.016f, 312.198f, 433.946f
    path.cubic_to((f32::from_bits(0x439d945b), f32::from_bits(0x43dd020c)), (f32::from_bits(0x439e1667), f32::from_bits(0x43e17106)), (f32::from_bits(0x439f7853), f32::from_bits(0x43e5820c)));
    // 315.159f, 442.016f, 316.175f, 450.883f, 318.94f, 459.016f
    path.close();
    path.move_to((f32::from_bits(0x439ffc6c), f32::from_bits(0x43e7f106)));
    // 319.972f, 463.883f
    path.line_to((f32::from_bits(0x439ffc6c), f32::from_bits(0x43e7f106)));
    // 319.972f, 463.883f
    path.close();
    path.move_to((f32::from_bits(0x439ffc6c), f32::from_bits(0x43e7f106)));
    // 319.972f, 463.883f
    path.cubic_to((f32::from_bits(0x439f5668), f32::from_bits(0x43e758f6)), (f32::from_bits(0x439fec6c), f32::from_bits(0x43e63604)), (f32::from_bits(0x439f7874), f32::from_bits(0x43e5820c)));
    // 318.675f, 462.695f, 319.847f, 460.422f, 318.941f, 459.016f
    path.cubic_to((f32::from_bits(0x439fec6c), f32::from_bits(0x43e63604)), (f32::from_bits(0x439f5668), f32::from_bits(0x43e758f5)), (f32::from_bits(0x439ffc6c), f32::from_bits(0x43e7f106)));
    // 319.847f, 460.422f, 318.675f, 462.695f, 319.972f, 463.883f
    path.close();
    path.move_to((f32::from_bits(0x43a12853), f32::from_bits(0x43ede9fc)));
    // 322.315f, 475.828f
    path.line_to((f32::from_bits(0x43a12853), f32::from_bits(0x43ede9fc)));
    // 322.315f, 475.828f
    path.close();
    path.move_to((f32::from_bits(0x43a12853), f32::from_bits(0x43ede9fc)));
    // 322.315f, 475.828f
    path.cubic_to((f32::from_bits(0x43a18c4b), f32::from_bits(0x43eb7604)), (f32::from_bits(0x439fe45b), f32::from_bits(0x43ea4b02)), (f32::from_bits(0x439ffc4b), f32::from_bits(0x43e7f106)));
    // 323.096f, 470.922f, 319.784f, 468.586f, 319.971f, 463.883f
    path.cubic_to((f32::from_bits(0x439fe45b), f32::from_bits(0x43ea4b02)), (f32::from_bits(0x43a18c6c), f32::from_bits(0x43eb7604)), (f32::from_bits(0x43a12853), f32::from_bits(0x43ede9fc)));
    // 319.784f, 468.586f, 323.097f, 470.922f, 322.315f, 475.828f
    path.close();
    path.move_to((f32::from_bits(0x43a1e45b), f32::from_bits(0x43ef63f8)));
    // 323.784f, 478.781f
    path.line_to((f32::from_bits(0x43a1e45b), f32::from_bits(0x43ef63f8)));
    // 323.784f, 478.781f
    path.close();
    path.move_to((f32::from_bits(0x43a1e45b), f32::from_bits(0x43ef63f8)));
    // 323.784f, 478.781f
    path.cubic_to((f32::from_bits(0x43a20561), f32::from_bits(0x43eeb9fc)), (f32::from_bits(0x43a1ae57), f32::from_bits(0x43ee4be8)), (f32::from_bits(0x43a12853), f32::from_bits(0x43ede9fc)));
    // 324.042f, 477.453f, 323.362f, 476.593f, 322.315f, 475.828f
    path.cubic_to((f32::from_bits(0x43a1ae57), f32::from_bits(0x43ee4c08)), (f32::from_bits(0x43a20561), f32::from_bits(0x43eeb9fc)), (f32::from_bits(0x43a1e45b), f32::from_bits(0x43ef63f8)));
    // 323.362f, 476.594f, 324.042f, 477.453f, 323.784f, 478.781f
    path.close();
    path.move_to((f32::from_bits(0x439fb169), f32::from_bits(0x43f032f2)));
    // 319.386f, 480.398f
    path.line_to((f32::from_bits(0x439fb169), f32::from_bits(0x43f032f2)));
    // 319.386f, 480.398f
    path.close();
    path.move_to((f32::from_bits(0x439fb169), f32::from_bits(0x43f032f2)));
    // 319.386f, 480.398f
    path.cubic_to((f32::from_bits(0x43a08063), f32::from_bits(0x43f022f2)), (f32::from_bits(0x43a1ec6b), f32::from_bits(0x43f078f6)), (f32::from_bits(0x43a1e45b), f32::from_bits(0x43ef63f8)));
    // 321.003f, 480.273f, 323.847f, 480.945f, 323.784f, 478.781f
    path.cubic_to((f32::from_bits(0x43a1ec6b), f32::from_bits(0x43f078f6)), (f32::from_bits(0x43a08063), f32::from_bits(0x43f022f2)), (f32::from_bits(0x439fb169), f32::from_bits(0x43f032f2)));
    // 323.847f, 480.945f, 321.003f, 480.273f, 319.386f, 480.398f
    path.close();
    path.move_to((f32::from_bits(0x439e4d50), f32::from_bits(0x43f16106)));
    // 316.604f, 482.758f
    path.line_to((f32::from_bits(0x439e4d50), f32::from_bits(0x43f16106)));
    // 316.604f, 482.758f
    path.close();
    path.move_to((f32::from_bits(0x439e4d50), f32::from_bits(0x43f16106)));
    // 316.604f, 482.758f
    path.cubic_to((f32::from_bits(0x439de45a), f32::from_bits(0x43f05000)), (f32::from_bits(0x439f445a), f32::from_bits(0x43f0b20c)), (f32::from_bits(0x439fb148), f32::from_bits(0x43f03312)));
    // 315.784f, 480.625f, 318.534f, 481.391f, 319.385f, 480.399f
    path.cubic_to((f32::from_bits(0x439f445a), f32::from_bits(0x43f0b20c)), (f32::from_bits(0x439de45a), f32::from_bits(0x43f05000)), (f32::from_bits(0x439e4d50), f32::from_bits(0x43f16106)));
    // 318.534f, 481.391f, 315.784f, 480.625f, 316.604f, 482.758f
    path.close();
    path.move_to((f32::from_bits(0x43a0de56), f32::from_bits(0x43f7470a)));
    // 321.737f, 494.555f
    path.line_to((f32::from_bits(0x43a0de56), f32::from_bits(0x43f7470a)));
    // 321.737f, 494.555f
    path.close();
    path.move_to((f32::from_bits(0x43a0de56), f32::from_bits(0x43f7470a)));
    // 321.737f, 494.555f
    path.cubic_to((f32::from_bits(0x439f4062), f32::from_bits(0x43f5a106)), (f32::from_bits(0x439f2b64), f32::from_bits(0x43f33106)), (f32::from_bits(0x439e4d50), f32::from_bits(0x43f16106)));
    // 318.503f, 491.258f, 318.339f, 486.383f, 316.604f, 482.758f
    path.cubic_to((f32::from_bits(0x439f2b64), f32::from_bits(0x43f33106)), (f32::from_bits(0x439f4062), f32::from_bits(0x43f5a106)), (f32::from_bits(0x43a0de56), f32::from_bits(0x43f7470a)));
    // 318.339f, 486.383f, 318.503f, 491.258f, 321.737f, 494.555f
    path.close();
    path.move_to((f32::from_bits(0x43a3945a), f32::from_bits(0x43fa13f8)));
    // 327.159f, 500.156f
    path.line_to((f32::from_bits(0x43a3945a), f32::from_bits(0x43fa13f8)));
    // 327.159f, 500.156f
    path.close();
    path.move_to((f32::from_bits(0x43a3945a), f32::from_bits(0x43fa13f8)));
    // 327.159f, 500.156f
    path.cubic_to((f32::from_bits(0x43a2dc4a), f32::from_bits(0x43f8ab02)), (f32::from_bits(0x43a0d74c), f32::from_bits(0x43f8f4fe)), (f32::from_bits(0x43a0de56), f32::from_bits(0x43f746ea)));
    // 325.721f, 497.336f, 321.682f, 497.914f, 321.737f, 494.554f
    path.cubic_to((f32::from_bits(0x43a0d76d), f32::from_bits(0x43f8f4fe)), (f32::from_bits(0x43a2dc6a), f32::from_bits(0x43f8ab03)), (f32::from_bits(0x43a3945a), f32::from_bits(0x43fa13f8)));
    // 321.683f, 497.914f, 325.722f, 497.336f, 327.159f, 500.156f
    path.close();
    path.move_to((f32::from_bits(0x43a58e56), f32::from_bits(0x43fa98f6)));
    // 331.112f, 501.195f
    path.line_to((f32::from_bits(0x43a58e56), f32::from_bits(0x43fa98f6)));
    // 331.112f, 501.195f
    path.close();
    path.move_to((f32::from_bits(0x43a58e56), f32::from_bits(0x43fa98f6)));
    // 331.112f, 501.195f
    path.cubic_to((f32::from_bits(0x43a50148), f32::from_bits(0x43fa2be8)), (f32::from_bits(0x43a45646), f32::from_bits(0x43fa02f2)), (f32::from_bits(0x43a3945a), f32::from_bits(0x43fa13f8)));
    // 330.01f, 500.343f, 328.674f, 500.023f, 327.159f, 500.156f
    path.cubic_to((f32::from_bits(0x43a45666), f32::from_bits(0x43fa02f2)), (f32::from_bits(0x43a50168), f32::from_bits(0x43fa2c08)), (f32::from_bits(0x43a58e56), f32::from_bits(0x43fa98f6)));
    // 328.675f, 500.023f, 330.011f, 500.344f, 331.112f, 501.195f
    path.close();
    path.move_to((f32::from_bits(0x43a64958), f32::from_bits(0x43f8c000)));
    // 332.573f, 497.5f
    path.line_to((f32::from_bits(0x43a64958), f32::from_bits(0x43f8c000)));
    // 332.573f, 497.5f
    path.close();
    path.move_to((f32::from_bits(0x43a64958), f32::from_bits(0x43f8c000)));
    // 332.573f, 497.5f
    path.line_to((f32::from_bits(0x43a58e56), f32::from_bits(0x43fa98f6)));
    // 331.112f, 501.195f
    path.line_to((f32::from_bits(0x43a64958), f32::from_bits(0x43f8c000)));
    // 332.573f, 497.5f
    path.close();
    path.move_to((f32::from_bits(0x43a73e56), f32::from_bits(0x43f5820c)));
    // 334.487f, 491.016f
    path.line_to((f32::from_bits(0x43a73e56), f32::from_bits(0x43f5820c)));
    // 334.487f, 491.016f
    path.close();
    path.move_to((f32::from_bits(0x43a73e56), f32::from_bits(0x43f5820c)));
    // 334.487f, 491.016f
    path.cubic_to((f32::from_bits(0x43a64d50), f32::from_bits(0x43f654fe)), (f32::from_bits(0x43a7174c), f32::from_bits(0x43f7de14)), (f32::from_bits(0x43a64958), f32::from_bits(0x43f8c000)));
    // 332.604f, 492.664f, 334.182f, 495.735f, 332.573f, 497.5f
    path.cubic_to((f32::from_bits(0x43a7176c), f32::from_bits(0x43f7ddf4)), (f32::from_bits(0x43a64d50), f32::from_bits(0x43f654fe)), (f32::from_bits(0x43a73e56), f32::from_bits(0x43f5820c)));
    // 334.183f, 495.734f, 332.604f, 492.664f, 334.487f, 491.016f
    path.close();
    path.move_to((f32::from_bits(0x43a6f26f), f32::from_bits(0x43f20b02)));
    // 333.894f, 484.086f
    path.line_to((f32::from_bits(0x43a6f26f), f32::from_bits(0x43f20b02)));
    // 333.894f, 484.086f
    path.close();
    path.move_to((f32::from_bits(0x43a6f26f), f32::from_bits(0x43f20b02)));
    // 333.894f, 484.086f
    path.cubic_to((f32::from_bits(0x43a78d71), f32::from_bits(0x43f2f810)), (f32::from_bits(0x43a72873), f32::from_bits(0x43f453f8)), (f32::from_bits(0x43a73e77), f32::from_bits(0x43f5820c)));
    // 335.105f, 485.938f, 334.316f, 488.656f, 334.488f, 491.016f
    path.cubic_to((f32::from_bits(0x43a72852), f32::from_bits(0x43f453f8)), (f32::from_bits(0x43a78d50), f32::from_bits(0x43f2f810)), (f32::from_bits(0x43a6f26f), f32::from_bits(0x43f20b02)));
    // 334.315f, 488.656f, 335.104f, 485.938f, 333.894f, 484.086f
    path.close();
    path.move_to((f32::from_bits(0x43a6ba5f), f32::from_bits(0x43ef3d0e)));
    // 333.456f, 478.477f
    path.line_to((f32::from_bits(0x43a6ba5f), f32::from_bits(0x43ef3d0e)));
    // 333.456f, 478.477f
    path.close();
    path.move_to((f32::from_bits(0x43a6ba5f), f32::from_bits(0x43ef3d0e)));
    // 333.456f, 478.477f
    path.cubic_to((f32::from_bits(0x43a60e57), f32::from_bits(0x43f04000)), (f32::from_bits(0x43a82355), f32::from_bits(0x43f0fc08)), (f32::from_bits(0x43a6f26f), f32::from_bits(0x43f20b02)));
    // 332.112f, 480.5f, 336.276f, 481.969f, 333.894f, 484.086f
    path.cubic_to((f32::from_bits(0x43a82354), f32::from_bits(0x43f0fc08)), (f32::from_bits(0x43a60e56), f32::from_bits(0x43f04000)), (f32::from_bits(0x43a6ba5f), f32::from_bits(0x43ef3d0e)));
    // 336.276f, 481.969f, 332.112f, 480.5f, 333.456f, 478.477f
    path.close();
    path.move_to((f32::from_bits(0x43a35c6b), f32::from_bits(0x43ef88f5)));
    // 326.722f, 479.07f
    path.line_to((f32::from_bits(0x43a35c6b), f32::from_bits(0x43ef88f5)));
    // 326.722f, 479.07f
    path.close();
    path.move_to((f32::from_bits(0x43a35c6b), f32::from_bits(0x43ef88f5)));
    // 326.722f, 479.07f
    path.cubic_to((f32::from_bits(0x43a4b26f), f32::from_bits(0x43efe105)), (f32::from_bits(0x43a5b76d), f32::from_bits(0x43ee2ef9)), (f32::from_bits(0x43a6ba5f), f32::from_bits(0x43ef3ced)));
    // 329.394f, 479.758f, 331.433f, 476.367f, 333.456f, 478.476f
    path.cubic_to((f32::from_bits(0x43a5b76d), f32::from_bits(0x43ee2ef9)), (f32::from_bits(0x43a4b26f), f32::from_bits(0x43efe106)), (f32::from_bits(0x43a35c6b), f32::from_bits(0x43ef88f5)));
    // 331.433f, 476.367f, 329.394f, 479.758f, 326.722f, 479.07f
    path.close();
    path.move_to((f32::from_bits(0x43a08063), f32::from_bits(0x43e5a70a)));
    // 321.003f, 459.305f
    path.line_to((f32::from_bits(0x43a08063), f32::from_bits(0x43e5a70a)));
    // 321.003f, 459.305f
    path.close();
    path.move_to((f32::from_bits(0x43a08063), f32::from_bits(0x43e5a70a)));
    // 321.003f, 459.305f
    path.cubic_to((f32::from_bits(0x43a15169), f32::from_bits(0x43e90312)), (f32::from_bits(0x43a2626f), f32::from_bits(0x43ec4312)), (f32::from_bits(0x43a35c6b), f32::from_bits(0x43ef8916)));
    // 322.636f, 466.024f, 324.769f, 472.524f, 326.722f, 479.071f
    path.cubic_to((f32::from_bits(0x43a2626f), f32::from_bits(0x43ec42f1)), (f32::from_bits(0x43a15169), f32::from_bits(0x43e902f1)), (f32::from_bits(0x43a08063), f32::from_bits(0x43e5a70a)));
    // 324.769f, 472.523f, 322.636f, 466.023f, 321.003f, 459.305f
    path.close();
    path.move_to((f32::from_bits(0x43a05a5f), f32::from_bits(0x43e407ef)));
    // 320.706f, 456.062f
    path.line_to((f32::from_bits(0x43a05a5f), f32::from_bits(0x43e407ef)));
    // 320.706f, 456.062f
    path.close();
    path.move_to((f32::from_bits(0x43a05a5f), f32::from_bits(0x43e407ef)));
    // 320.706f, 456.062f
    path.line_to((f32::from_bits(0x43a08063), f32::from_bits(0x43e5a6e9)));
    // 321.003f, 459.304f
    path.line_to((f32::from_bits(0x43a05a5f), f32::from_bits(0x43e407ef)));
    // 320.706f, 456.062f
    path.close();
    path.move_to((f32::from_bits(0x439ecf5d), f32::from_bits(0x43dd3fff)));
    // 317.62f, 442.5f
    path.line_to((f32::from_bits(0x439ecf5d), f32::from_bits(0x43dd3fff)));
    // 317.62f, 442.5f
    path.close();
    path.move_to((f32::from_bits(0x439ecf5d), f32::from_bits(0x43dd3fff)));
    // 317.62f, 442.5f
    path.cubic_to((f32::from_bits(0x439e9c6b), f32::from_bits(0x43dfcb01)), (f32::from_bits(0x439fbe57), f32::from_bits(0x43e1cc07)), (f32::from_bits(0x43a05a5f), f32::from_bits(0x43e407ef)));
    // 317.222f, 447.586f, 319.487f, 451.594f, 320.706f, 456.062f
    path.cubic_to((f32::from_bits(0x439fbe57), f32::from_bits(0x43e1cc08)), (f32::from_bits(0x439e9c6b), f32::from_bits(0x43dfcb01)), (f32::from_bits(0x439ecf5d), f32::from_bits(0x43dd3fff)));
    // 319.487f, 451.594f, 317.222f, 447.586f, 317.62f, 442.5f
    path.close();
    path.move_to((f32::from_bits(0x439e276d), f32::from_bits(0x43dad105)));
    // 316.308f, 437.633f
    path.cubic_to((f32::from_bits(0x439e4979), f32::from_bits(0x43dba4fd)), (f32::from_bits(0x439dc375), f32::from_bits(0x43dce915)), (f32::from_bits(0x439ecf5d), f32::from_bits(0x43dd3fff)));
    // 316.574f, 439.289f, 315.527f, 441.821f, 317.62f, 442.5f
    path.cubic_to((f32::from_bits(0x439dc355), f32::from_bits(0x43dce8f5)), (f32::from_bits(0x439e4959), f32::from_bits(0x43dba4fd)), (f32::from_bits(0x439e276d), f32::from_bits(0x43dad105)));
    // 315.526f, 441.82f, 316.573f, 439.289f, 316.308f, 437.633f
    path.close();
}

fn joel_15(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    make_joel_15(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn joel_15x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    make_joel_15(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn make_joel_16(path: &mut PathBuilder) {
    path.move_to((f32::from_bits(0x420e6c8b), f32::from_bits(0x426bdf3b)));
    // 35.606f, 58.968f
    path.line_to((f32::from_bits(0x420fcccd), f32::from_bits(0x426c7ef9)));
    // 35.95f, 59.124f
    path.cubic_to((f32::from_bits(0x420fcccd), f32::from_bits(0x426c7ef9)), (f32::from_bits(0x42093d71), f32::from_bits(0x426c6e97)), (f32::from_bits(0x42036c8b), f32::from_bits(0x426cbf7c)));
    // 35.95f, 59.124f, 34.31f, 59.108f, 32.856f, 59.187f
    path.cubic_to((f32::from_bits(0x41fb3958), f32::from_bits(0x426d0f5b)), (f32::from_bits(0x41f076c8), f32::from_bits(0x426d48b3)), (f32::from_bits(0x41ef47ae), f32::from_bits(0x426d947a)));
    // 31.403f, 59.265f, 30.058f, 59.321f, 29.91f, 59.395f
    path.cubic_to((f32::from_bits(0x41ee1aa0), f32::from_bits(0x426ddf3b)), (f32::from_bits(0x41ec6041), f32::from_bits(0x426edb22)), (f32::from_bits(0x41eb1aa0), f32::from_bits(0x426fee97)));
    // 29.763f, 59.468f, 29.547f, 59.714f, 29.388f, 59.983f
    path.cubic_to((f32::from_bits(0x41eb1eb9), f32::from_bits(0x426feb85)), (f32::from_bits(0x41e9ba5e), f32::from_bits(0x42711eb8)), (f32::from_bits(0x41e9ba5e), f32::from_bits(0x42711eb8)));
    // 29.39f, 59.98f, 29.216f, 60.28f, 29.216f, 60.28f
    path.line_to((f32::from_bits(0x41e99999), f32::from_bits(0x42718f5c)));
    // 29.2f, 60.39f
    path.cubic_to((f32::from_bits(0x41ea76c8), f32::from_bits(0x4271a5e3)), (f32::from_bits(0x4212dd2f), f32::from_bits(0x42707efa)), (f32::from_bits(0x4212dd2f), f32::from_bits(0x42707efa)));
    // 29.308f, 60.412f, 36.716f, 60.124f, 36.716f, 60.124f
    path.cubic_to((f32::from_bits(0x4212dd2f), f32::from_bits(0x42707efa)), (f32::from_bits(0x42124395), f32::from_bits(0x42707be8)), (f32::from_bits(0x42131ba6), f32::from_bits(0x4270b646)));
    // 36.716f, 60.124f, 36.566f, 60.121f, 36.777f, 60.178f
    path.cubic_to((f32::from_bits(0x42131581), f32::from_bits(0x42710000)), (f32::from_bits(0x42130831), f32::from_bits(0x42711688)), (f32::from_bits(0x4213072b), f32::from_bits(0x42711688)));
    // 36.771f, 60.25f, 36.758f, 60.272f, 36.757f, 60.272f
    path.cubic_to((f32::from_bits(0x4212fae1), f32::from_bits(0x42711aa1)), (f32::from_bits(0x42127cee), f32::from_bits(0x42714eda)), (f32::from_bits(0x42127cee), f32::from_bits(0x42714eda)));
    // 36.745f, 60.276f, 36.622f, 60.327f, 36.622f, 60.327f
    path.cubic_to((f32::from_bits(0x42127ae2), f32::from_bits(0x42714eda)), (f32::from_bits(0x41c67ae2), f32::from_bits(0x42730f5d)), (f32::from_bits(0x41c345a2), f32::from_bits(0x427329fd)));
    // 36.62f, 60.327f, 24.81f, 60.765f, 24.409f, 60.791f
    path.cubic_to((f32::from_bits(0x41c247ae), f32::from_bits(0x42733e78)), (f32::from_bits(0x41c04396), f32::from_bits(0x42738e57)), (f32::from_bits(0x41bf4bc7), f32::from_bits(0x4273e45b)));
    // 24.285f, 60.811f, 24.033f, 60.889f, 23.912f, 60.973f
    path.cubic_to((f32::from_bits(0x41bf5c29), f32::from_bits(0x4273e042)), (f32::from_bits(0x41be9db3), f32::from_bits(0x4274322e)), (f32::from_bits(0x41be9db3), f32::from_bits(0x4274322e)));
    // 23.92f, 60.969f, 23.827f, 61.049f, 23.827f, 61.049f
    path.line_to((f32::from_bits(0x41be26ea), f32::from_bits(0x42746c8c)));
    // 23.769f, 61.106f
    path.cubic_to((f32::from_bits(0x41be1eb9), f32::from_bits(0x427470a5)), (f32::from_bits(0x41bde354), f32::from_bits(0x42748313)), (f32::from_bits(0x41bde354), f32::from_bits(0x42748313)));
    // 23.765f, 61.11f, 23.736f, 61.128f, 23.736f, 61.128f
    path.line_to((f32::from_bits(0x41bcc083), f32::from_bits(0x42751582)));
    // 23.594f, 61.271f
    path.line_to((f32::from_bits(0x41bcf3b6), f32::from_bits(0x427526ea)));
    // 23.619f, 61.288f
    path.line_to((f32::from_bits(0x41bd0e56), f32::from_bits(0x42756979)));
    // 23.632f, 61.353f
    path.line_to((f32::from_bits(0x41bd7cee), f32::from_bits(0x42758313)));
    // 23.686f, 61.378f
    path.cubic_to((f32::from_bits(0x41be8107), f32::from_bits(0x427572b1)), (f32::from_bits(0x41bf2d0f), f32::from_bits(0x42754290)), (f32::from_bits(0x41bfd2f2), f32::from_bits(0x4275147b)));
    // 23.813f, 61.362f, 23.897f, 61.315f, 23.978f, 61.27f
    path.line_to((f32::from_bits(0x41c0ba5f), f32::from_bits(0x4274da1d)));
    // 24.091f, 61.213f
    path.line_to((f32::from_bits(0x41c0ef9e), f32::from_bits(0x4274de36)));
    // 24.117f, 61.217f
    path.line_to((f32::from_bits(0x41c13f7d), f32::from_bits(0x4274d3f9)));
    // 24.156f, 61.207f
    path.cubic_to((f32::from_bits(0x41c13f7d), f32::from_bits(0x4274d3f9)), (f32::from_bits(0x41c174bc), f32::from_bits(0x4274c18a)), (f32::from_bits(0x41c17cee), f32::from_bits(0x4274be78)));
    // 24.156f, 61.207f, 24.182f, 61.189f, 24.186f, 61.186f
    path.cubic_to((f32::from_bits(0x41c18107), f32::from_bits(0x4274bf7e)), (f32::from_bits(0x41c1e561), f32::from_bits(0x4274b022)), (f32::from_bits(0x41c1e561), f32::from_bits(0x4274b022)));
    // 24.188f, 61.187f, 24.237f, 61.172f, 24.237f, 61.172f
    path.line_to((f32::from_bits(0x41c45e36), f32::from_bits(0x42746e99)));
    // 24.546f, 61.108f
    path.cubic_to((f32::from_bits(0x41c4624f), f32::from_bits(0x42746e99)), (f32::from_bits(0x41cf999a), f32::from_bits(0x42743853)), (f32::from_bits(0x41cf999a), f32::from_bits(0x42743853)));
    // 24.548f, 61.108f, 25.95f, 61.055f, 25.95f, 61.055f
    path.line_to((f32::from_bits(0x420d126f), f32::from_bits(0x4272b43a)));
    // 35.268f, 60.676f
    path.cubic_to((f32::from_bits(0x420d0938), f32::from_bits(0x4272c084)), (f32::from_bits(0x420cfcee), f32::from_bits(0x4272c49c)), (f32::from_bits(0x420cfcee), f32::from_bits(0x4272d917)));
    // 35.259f, 60.688f, 35.247f, 60.692f, 35.247f, 60.712f
    path.line_to((f32::from_bits(0x420d0938), f32::from_bits(0x4272b43a)));
    // 35.259f, 60.676f
    path.cubic_to((f32::from_bits(0x420c7be8), f32::from_bits(0x42737efb)), (f32::from_bits(0x420b3128), f32::from_bits(0x42743128)), (f32::from_bits(0x420a27f0), f32::from_bits(0x4274c18a)));
    // 35.121f, 60.874f, 34.798f, 61.048f, 34.539f, 61.189f
    path.line_to((f32::from_bits(0x42099eb9), f32::from_bits(0x42750c4b)));
    // 34.405f, 61.262f
    path.cubic_to((f32::from_bits(0x420872b1), f32::from_bits(0x4275b022)), (f32::from_bits(0x4206fbe8), f32::from_bits(0x42764397)), (f32::from_bits(0x42054396), f32::from_bits(0x4276c084)));
    // 34.112f, 61.422f, 33.746f, 61.566f, 33.316f, 61.688f
    path.cubic_to((f32::from_bits(0x42028313), f32::from_bits(0x42776b86)), (f32::from_bits(0x42007be8), f32::from_bits(0x4278de36)), (f32::from_bits(0x41fe7ae2), f32::from_bits(0x427b0f5d)));
    // 32.628f, 61.855f, 32.121f, 62.217f, 31.81f, 62.765f
    path.cubic_to((f32::from_bits(0x41fe4fe0), f32::from_bits(0x427b21cc)), (f32::from_bits(0x41fdbe78), f32::from_bits(0x427b8419)), (f32::from_bits(0x41fdbe78), f32::from_bits(0x427b8419)));
    // 31.789f, 62.783f, 31.718f, 62.879f, 31.718f, 62.879f
    path.cubic_to((f32::from_bits(0x41fdccce), f32::from_bits(0x427b71aa)), (f32::from_bits(0x41fd1cad), f32::from_bits(0x427c27f0)), (f32::from_bits(0x41fd1cad), f32::from_bits(0x427c27f0)));
    // 31.725f, 62.861f, 31.639f, 63.039f, 31.639f, 63.039f
    path.line_to((f32::from_bits(0x41fc1eb9), f32::from_bits(0x427d178e)));
    // 31.515f, 63.273f
    path.line_to((f32::from_bits(0x41fc7efb), f32::from_bits(0x427d020d)));
    // 31.562f, 63.252f
    path.line_to((f32::from_bits(0x41fbb647), f32::from_bits(0x427d3646)));
    // 31.464f, 63.303f
    path.line_to((f32::from_bits(0x41fbe76e), f32::from_bits(0x427d25e4)));
    // 31.488f, 63.287f
    path.line_to((f32::from_bits(0x41fae149), f32::from_bits(0x427d1fbf)));
    // 31.36f, 63.281f
    path.line_to((f32::from_bits(0x41fa5812), f32::from_bits(0x427d178e)));
    // 31.293f, 63.273f
    path.cubic_to((f32::from_bits(0x41f88108), f32::from_bits(0x427cf9dc)), (f32::from_bits(0x41f73541), f32::from_bits(0x427cb646)), (f32::from_bits(0x41f5d70c), f32::from_bits(0x427c6d92)));
    // 31.063f, 63.244f, 30.901f, 63.178f, 30.73f, 63.107f
    path.line_to((f32::from_bits(0x41f5999b), f32::from_bits(0x427c6148)));
    // 30.7f, 63.095f
    path.cubic_to((f32::from_bits(0x41f5999b), f32::from_bits(0x427c6148)), (f32::from_bits(0x41f2d0e7), f32::from_bits(0x427bdc29)), (f32::from_bits(0x41f2a9fd), f32::from_bits(0x427bd4fe)));
    // 30.7f, 63.095f, 30.352f, 62.965f, 30.333f, 62.958f
    path.cubic_to((f32::from_bits(0x41f28d51), f32::from_bits(0x427bc49c)), (f32::from_bits(0x41f26667), f32::from_bits(0x427bb021)), (f32::from_bits(0x41f26667), f32::from_bits(0x427bb021)));
    // 30.319f, 62.942f, 30.3f, 62.922f, 30.3f, 62.922f
    path.line_to((f32::from_bits(0x41efed92), f32::from_bits(0x427b1db2)));
    // 29.991f, 62.779f
    path.line_to((f32::from_bits(0x41ec9582), f32::from_bits(0x427a624e)));
    // 29.573f, 62.596f
    path.cubic_to((f32::from_bits(0x41eca1cc), f32::from_bits(0x427a645a)), (f32::from_bits(0x41eaf9dc), f32::from_bits(0x427a3021)), (f32::from_bits(0x41eaf9dc), f32::from_bits(0x427a3021)));
    // 29.579f, 62.598f, 29.372f, 62.547f, 29.372f, 62.547f
    path.cubic_to((f32::from_bits(0x41eaf9dc), f32::from_bits(0x427a3021)), (f32::from_bits(0x41ea126f), f32::from_bits(0x427a1894)), (f32::from_bits(0x41e9f3b7), f32::from_bits(0x427a1687)));
    // 29.372f, 62.547f, 29.259f, 62.524f, 29.244f, 62.522f
    path.cubic_to((f32::from_bits(0x41e9ccce), f32::from_bits(0x427a072b)), (f32::from_bits(0x41e99375), f32::from_bits(0x4279f1aa)), (f32::from_bits(0x41e99375), f32::from_bits(0x4279f1aa)));
    // 29.225f, 62.507f, 29.197f, 62.486f, 29.197f, 62.486f
    path.line_to((f32::from_bits(0x41e86e98), f32::from_bits(0x4279d604)));
    // 29.054f, 62.459f
    path.line_to((f32::from_bits(0x41e6147b), f32::from_bits(0x4279a3d7)));
    // 28.76f, 62.41f
    path.cubic_to((f32::from_bits(0x41e00625), f32::from_bits(0x42796b85)), (f32::from_bits(0x41db49ba), f32::from_bits(0x427a7ae1)), (f32::from_bits(0x41d62b02), f32::from_bits(0x427bc8b4)));
    // 28.003f, 62.355f, 27.411f, 62.62f, 26.771f, 62.946f
    path.cubic_to((f32::from_bits(0x41d24fdf), f32::from_bits(0x427cba5e)), (f32::from_bits(0x41cecccd), f32::from_bits(0x427ce872)), (f32::from_bits(0x41ca0e56), f32::from_bits(0x427c6872)));
    // 26.289f, 63.182f, 25.85f, 63.227f, 25.257f, 63.102f
    path.cubic_to((f32::from_bits(0x41ca0a3d), f32::from_bits(0x427c676c)), (f32::from_bits(0x41c9353f), f32::from_bits(0x427c570a)), (f32::from_bits(0x41c9353f), f32::from_bits(0x427c570a)));
    // 25.255f, 63.101f, 25.151f, 63.085f, 25.151f, 63.085f
    path.line_to((f32::from_bits(0x41c73b64), f32::from_bits(0x427c26e9)));
    // 24.904f, 63.038f
    path.line_to((f32::from_bits(0x41c774bc), f32::from_bits(0x427c374b)));
    // 24.932f, 63.054f
    path.line_to((f32::from_bits(0x41c67ef9), f32::from_bits(0x427c0312)));
    // 24.812f, 63.003f
    path.cubic_to((f32::from_bits(0x41c4df3b), f32::from_bits(0x427bc5a1)), (f32::from_bits(0x41c2a3d6), f32::from_bits(0x427b8d4f)), (f32::from_bits(0x41c0851e), f32::from_bits(0x427b6978)));
    // 24.609f, 62.943f, 24.33f, 62.888f, 24.065f, 62.853f
    path.cubic_to((f32::from_bits(0x41bf1893), f32::from_bits(0x427b52f1)), (f32::from_bits(0x41bd2d0e), f32::from_bits(0x427b52f1)), (f32::from_bits(0x41bc020c), f32::from_bits(0x427b5e34)));
    // 23.887f, 62.831f, 23.647f, 62.831f, 23.501f, 62.842f
    path.line_to((f32::from_bits(0x41bac6a8), f32::from_bits(0x427b6871)));
    // 23.347f, 62.852f
    path.cubic_to((f32::from_bits(0x41b9db23), f32::from_bits(0x427b72ae)), (f32::from_bits(0x41b87cee), f32::from_bits(0x427b820b)), (f32::from_bits(0x41b7fbe7), f32::from_bits(0x427b655f)));
    // 23.232f, 62.862f, 23.061f, 62.877f, 22.998f, 62.849f
    path.cubic_to((f32::from_bits(0x41b7fbe7), f32::from_bits(0x427b5f3a)), (f32::from_bits(0x41b7dd2f), f32::from_bits(0x427b48b3)), (f32::from_bits(0x41b7dd2f), f32::from_bits(0x427b48b3)));
    // 22.998f, 62.843f, 22.983f, 62.821f, 22.983f, 62.821f
    path.line_to((f32::from_bits(0x41b7a5e3), f32::from_bits(0x427b22d0)));
    // 22.956f, 62.784f
    path.cubic_to((f32::from_bits(0x41b7be76), f32::from_bits(0x427b3332)), (f32::from_bits(0x41b74395), f32::from_bits(0x427aed91)), (f32::from_bits(0x41b74395), f32::from_bits(0x427aed91)));
    // 22.968f, 62.8f, 22.908f, 62.732f, 22.908f, 62.732f
    path.line_to((f32::from_bits(0x41b70c49), f32::from_bits(0x427acfdf)));
    // 22.881f, 62.703f
    path.cubic_to((f32::from_bits(0x41b70418), f32::from_bits(0x427ad916)), (f32::from_bits(0x41b6d70a), f32::from_bits(0x427a9168)), (f32::from_bits(0x41b6d70a), f32::from_bits(0x427a9168)));
    // 22.877f, 62.712f, 22.855f, 62.642f, 22.855f, 62.642f
    path.line_to((f32::from_bits(0x41b6bc6a), f32::from_bits(0x427a645a)));
    // 22.842f, 62.598f
    path.line_to((f32::from_bits(0x41b66e97), f32::from_bits(0x427a75c2)));
    // 22.804f, 62.615f
    path.cubic_to((f32::from_bits(0x41b6872a), f32::from_bits(0x427a71a9)), (f32::from_bits(0x41b5a9fb), f32::from_bits(0x4279c6a7)), (f32::from_bits(0x41b5a9fb), f32::from_bits(0x4279c6a7)));
    // 22.816f, 62.611f, 22.708f, 62.444f, 22.708f, 62.444f
    path.line_to((f32::from_bits(0x41b59580), f32::from_bits(0x4279b645)));
    // 22.698f, 62.428f
    path.line_to((f32::from_bits(0x41b549b9), f32::from_bits(0x42799fbe)));
    // 22.661f, 62.406f
    path.line_to((f32::from_bits(0x41b53957), f32::from_bits(0x42799ba5)));
    // 22.653f, 62.402f
    path.cubic_to((f32::from_bits(0x41b52b01), f32::from_bits(0x42798d4f)), (f32::from_bits(0x41b4a3d6), f32::from_bits(0x427920c4)), (f32::from_bits(0x41b4a3d6), f32::from_bits(0x427920c4)));
    // 22.646f, 62.388f, 22.58f, 62.282f, 22.58f, 62.282f
    path.line_to((f32::from_bits(0x41b43126), f32::from_bits(0x4278be76)));
    // 22.524f, 62.186f
    path.line_to((f32::from_bits(0x41b3ed90), f32::from_bits(0x4278ab01)));
    // 22.491f, 62.167f
    path.line_to((f32::from_bits(0x41b3be75), f32::from_bits(0x42789ba5)));
    // 22.468f, 62.152f
    path.line_to((f32::from_bits(0x41b3d0e4), f32::from_bits(0x4278b957)));
    // 22.477f, 62.181f
    path.line_to((f32::from_bits(0x41b351ea), f32::from_bits(0x42786353)));
    // 22.415f, 62.097f
    path.line_to((f32::from_bits(0x41b33957), f32::from_bits(0x42786353)));
    // 22.403f, 62.097f
    path.cubic_to((f32::from_bits(0x41b326e8), f32::from_bits(0x42785a1c)), (f32::from_bits(0x41b2fbe6), f32::from_bits(0x427846a7)), (f32::from_bits(0x41b2fbe6), f32::from_bits(0x427846a7)));
    // 22.394f, 62.088f, 22.373f, 62.069f, 22.373f, 62.069f
    path.line_to((f32::from_bits(0x41b2353e), f32::from_bits(0x4277f8d4)));
    // 22.276f, 61.993f
    path.cubic_to((f32::from_bits(0x41b26040), f32::from_bits(0x42780624)), (f32::from_bits(0x41b16e96), f32::from_bits(0x4277d0e4)), (f32::from_bits(0x41b16e96), f32::from_bits(0x4277d0e4)));
    // 22.297f, 62.006f, 22.179f, 61.954f, 22.179f, 61.954f
    path.cubic_to((f32::from_bits(0x41b16e96), f32::from_bits(0x4277d0e4)), (f32::from_bits(0x41b10417), f32::from_bits(0x4277c188)), (f32::from_bits(0x41b0fffe), f32::from_bits(0x4277c188)));
    // 22.179f, 61.954f, 22.127f, 61.939f, 22.125f, 61.939f
    path.cubic_to((f32::from_bits(0x41b0fffe), f32::from_bits(0x4277bf7c)), (f32::from_bits(0x41b03f7b), f32::from_bits(0x427778d4)), (f32::from_bits(0x41b03f7b), f32::from_bits(0x427778d4)));
    // 22.125f, 61.937f, 22.031f, 61.868f, 22.031f, 61.868f
    path.line_to((f32::from_bits(0x41ae8729), f32::from_bits(0x4276f7ce)));
    // 21.816f, 61.742f
    path.cubic_to((f32::from_bits(0x41adb644), f32::from_bits(0x4276d0e5)), (f32::from_bits(0x41ad22cf), f32::from_bits(0x42768e55)), (f32::from_bits(0x41ac8729), f32::from_bits(0x427648b3)));
    // 21.714f, 61.704f, 21.642f, 61.639f, 21.566f, 61.571f
    path.line_to((f32::from_bits(0x41ab957f), f32::from_bits(0x4275e24d)));
    // 21.448f, 61.471f
    path.cubic_to((f32::from_bits(0x41aa8f5a), f32::from_bits(0x42757df3)), (f32::from_bits(0x41a9b644), f32::from_bits(0x42751fbe)), (f32::from_bits(0x41a8a3d5), f32::from_bits(0x42747fff)));
    // 21.32f, 61.373f, 21.214f, 61.281f, 21.08f, 61.125f
    path.cubic_to((f32::from_bits(0x41a6d708), f32::from_bits(0x4273a3d6)), (f32::from_bits(0x41a645a0), f32::from_bits(0x4272dd2e)), (f32::from_bits(0x41a58935), f32::from_bits(0x4271b126)));
    // 20.855f, 60.91f, 20.784f, 60.716f, 20.692f, 60.423f
    path.line_to((f32::from_bits(0x41a5851c), f32::from_bits(0x4271a7ef)));
    // 20.69f, 60.414f
    path.line_to((f32::from_bits(0x41a56a7c), f32::from_bits(0x42719687)));
    // 20.677f, 60.397f
    path.line_to((f32::from_bits(0x41a54dd0), f32::from_bits(0x4271820c)));
    // 20.663f, 60.377f
    path.cubic_to((f32::from_bits(0x41a50209), f32::from_bits(0x42711062)), (f32::from_bits(0x41a4ced6), f32::from_bits(0x42707efa)), (f32::from_bits(0x41a4be74), f32::from_bits(0x426ff4bc)));
    // 20.626f, 60.266f, 20.601f, 60.124f, 20.593f, 59.989f
    path.cubic_to((f32::from_bits(0x41a51478), f32::from_bits(0x427073b6)), (f32::from_bits(0x41a576c6), f32::from_bits(0x42710b43)), (f32::from_bits(0x41a576c6), f32::from_bits(0x42710b43)));
    // 20.635f, 60.113f, 20.683f, 60.261f, 20.683f, 60.261f
    path.cubic_to((f32::from_bits(0x41a71478), f32::from_bits(0x42730418)), (f32::from_bits(0x41a9df39), f32::from_bits(0x42746666)), (f32::from_bits(0x41adc6a5), f32::from_bits(0x427526e9)));
    // 20.885f, 60.754f, 21.234f, 61.1f, 21.722f, 61.288f
    path.cubic_to((f32::from_bits(0x41adc499), f32::from_bits(0x427525e3)), (f32::from_bits(0x41ae47ab), f32::from_bits(0x42754395)), (f32::from_bits(0x41ae47ab), f32::from_bits(0x42754395)));
    // 21.721f, 61.287f, 21.785f, 61.316f, 21.785f, 61.316f
    path.line_to((f32::from_bits(0x41afe55d), f32::from_bits(0x4275978d)));
    // 21.987f, 61.398f
    path.cubic_to((f32::from_bits(0x41b27cea), f32::from_bits(0x4275e147)), (f32::from_bits(0x41b54dd0), f32::from_bits(0x4275d916)), (f32::from_bits(0x41b772ad), f32::from_bits(0x42758106)));
    // 22.311f, 61.47f, 22.663f, 61.462f, 22.931f, 61.376f
    path.cubic_to((f32::from_bits(0x41b8df38), f32::from_bits(0x42753d70)), (f32::from_bits(0x41ba1684), f32::from_bits(0x4274d1eb)), (f32::from_bits(0x41bb4186), f32::from_bits(0x42746979)));
    // 23.109f, 61.31f, 23.261f, 61.205f, 23.407f, 61.103f
    path.line_to((f32::from_bits(0x41bdbc67), f32::from_bits(0x4273a1cb)));
    // 23.717f, 60.908f
    path.cubic_to((f32::from_bits(0x41c0f1a6), f32::from_bits(0x4272cccd)), (f32::from_bits(0x41c3cabd), f32::from_bits(0x4272b958)), (f32::from_bits(0x41c71684), f32::from_bits(0x4272a3d7)));
    // 24.118f, 60.7f, 24.474f, 60.681f, 24.886f, 60.66f
    path.line_to((f32::from_bits(0x41ca4392), f32::from_bits(0x42728831)));
    // 25.283f, 60.633f
    path.line_to((f32::from_bits(0x41def9d8), f32::from_bits(0x42723f7d)));
    // 27.872f, 60.562f
    path.cubic_to((f32::from_bits(0x41e15a1a), f32::from_bits(0x42722d0e)), (f32::from_bits(0x41e4105f), f32::from_bits(0x42723333)), (f32::from_bits(0x41e60e53), f32::from_bits(0x4271c7ae)));
    // 28.169f, 60.544f, 28.508f, 60.55f, 28.757f, 60.445f
    path.cubic_to((f32::from_bits(0x41e87ceb), f32::from_bits(0x42715810)), (f32::from_bits(0x41e97ef7), f32::from_bits(0x427077cf)), (f32::from_bits(0x41ea9165), f32::from_bits(0x426f8a3d)));
    // 29.061f, 60.336f, 29.187f, 60.117f, 29.321f, 59.885f
    path.line_to((f32::from_bits(0x41ebccc9), f32::from_bits(0x426e8a3d)));
    // 29.475f, 59.635f
    path.cubic_to((f32::from_bits(0x41ebced5), f32::from_bits(0x426e8937)), (f32::from_bits(0x41ec2d0b), f32::from_bits(0x426e4ccc)), (f32::from_bits(0x41ec2d0b), f32::from_bits(0x426e4ccc)));
    // 29.476f, 59.634f, 29.522f, 59.575f, 29.522f, 59.575f
    path.line_to((f32::from_bits(0x41ecae11), f32::from_bits(0x426dde34)));
    // 29.585f, 59.467f
    path.line_to((f32::from_bits(0x41ecdf38), f32::from_bits(0x426dde34)));
    // 29.609f, 59.467f
    path.line_to((f32::from_bits(0x41ed26e6), f32::from_bits(0x426dc082)));
    // 29.644f, 59.438f
    path.cubic_to((f32::from_bits(0x41ee1ca9), f32::from_bits(0x426d5a1c)), (f32::from_bits(0x41eeccc9), f32::from_bits(0x426d1061)), (f32::from_bits(0x41f01684), f32::from_bits(0x426ce978)));
    // 29.764f, 59.338f, 29.85f, 59.266f, 30.011f, 59.228f
    path.cubic_to((f32::from_bits(0x41f29fbb), f32::from_bits(0x426c8e55)), (f32::from_bits(0x420cced8), f32::from_bits(0x426bd4fd)), (f32::from_bits(0x420e6c8a), f32::from_bits(0x426bdf3b)));
    // 30.328f, 59.139f, 35.202f, 58.958f, 35.606f, 58.968f
    path.move_to((f32::from_bits(0x41b60622), f32::from_bits(0x427adb22)));
    // 22.753f, 62.714f
    path.line_to((f32::from_bits(0x41b60416), f32::from_bits(0x427ad709)));
    // 22.752f, 62.71f
    path.cubic_to((f32::from_bits(0x41b60416), f32::from_bits(0x427ad603)), (f32::from_bits(0x41b60416), f32::from_bits(0x427ad915)), (f32::from_bits(0x41b60622), f32::from_bits(0x427adb22)));
    // 22.752f, 62.709f, 22.752f, 62.712f, 22.753f, 62.714f
    path.move_to((f32::from_bits(0x41bed2ef), f32::from_bits(0x4274cbc6)));
    // 23.853f, 61.199f
    path.close();
    path.move_to((f32::from_bits(0x41c04fdd), f32::from_bits(0x42746560)));
    // 24.039f, 61.099f
    path.close();
}

fn joel_16(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    make_joel_16(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn joel_16x(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    make_joel_16(&mut path);
    test_simplify(reporter, &path.detach(), filename);
}

fn coincubics(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.move_to((((0.00000000000000000 as f32) as f32), ((0.00000000000000000 as f32) as f32)));
    path.cubic_to((((0.00022939755581319332 as f32) as f32), ((0.00022927834652364254 as f32) as f32)), (((0.00022930106206331402 as f32) as f32), ((0.00022929999977350235 as f32) as f32)), (((0.00022930069826543331 as f32) as f32), ((0.00022913678549230099 as f32) as f32)));
    path.line_to((((0.00022930069826543331 as f32) as f32), ((0.00022930069826543331 as f32) as f32)));
    path.cubic_to((((0.00011465034913271666 as f32) as f32), ((0.00011465034913271666 as f32) as f32)), (((0.00011465061106719077 as f32) as f32), ((0.00011460937093943357 as f32) as f32)), (((0.00014331332931760699 as f32) as f32), ((0.00014325146912597120 as f32) as f32)));
    test_simplify(reporter, &path.detach(), filename);
}

fn grshapearc(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((25.0098, 23.1973));
    path.line_to((25.5689, 22.3682));
    path.conic_to((26.1281, 21.5392), (26.9572, 22.0984), 0.707107);
    path.conic_to((27.7862, 22.6576), (27.227, 23.4866), 0.707107);
    path.line_to((26.6678, 24.3156));
    path.conic_to((26.1086, 25.1447), (25.2796, 24.5855), 0.707107);
    path.conic_to((24.4506, 24.0263), (25.0098, 23.1973), 0.707107);
    path.close();
    path.move_to((26.6873, 20.7101));
    path.line_to((27.2465, 19.8811));
    path.conic_to((27.8057, 19.0521), (28.6348, 19.6113), 0.707107);
    path.conic_to((29.4638, 20.1704), (28.9046, 20.9995), 0.707107);
    path.line_to((28.3454, 21.8285));
    path.conic_to((27.7862, 22.6576), (26.9572, 22.0984), 0.707107);
    path.conic_to((26.1281, 21.5392), (26.6873, 20.7101), 0.707107);
    path.close();
    path.move_to((28.3649, 18.223));
    path.line_to((28.9241, 17.394));
    path.conic_to((29.4833, 16.565), (30.3123, 17.1241), 0.707107);
    path.conic_to((31.1414, 17.6833), (30.5822, 18.5124), 0.707107);
    path.line_to((30.023, 19.3414));
    path.conic_to((29.4638, 20.1704), (28.6348, 19.6113), 0.707107);
    path.conic_to((27.8057, 19.0521), (28.3649, 18.223), 0.707107);
    path.close();
    path.move_to((30.0425, 15.7359));
    path.line_to((30.6017, 14.9069));
    path.conic_to((31.1609, 14.0778), (31.9899, 14.637), 0.707107);
    path.conic_to((32.8189, 15.1962), (32.2598, 16.0253), 0.707107);
    path.line_to((31.7006, 16.8543));
    path.conic_to((31.1414, 17.6833), (30.3123, 17.1241), 0.707107);
    path.conic_to((29.4833, 16.565), (30.0425, 15.7359), 0.707107);
    path.close();
    path.move_to((31.7201, 13.2488));
    path.line_to((32.2793, 12.4198));
    path.conic_to((32.8385, 11.5907), (33.6675, 12.1499), 0.707107);
    path.conic_to((34.4965, 12.7091), (33.9373, 13.5381), 0.707107);
    path.line_to((33.3781, 14.3672));
    path.conic_to((32.8189, 15.1962), (31.9899, 14.637), 0.707107);
    path.conic_to((31.1609, 14.0778), (31.7201, 13.2488), 0.707107);
    path.close();
    path.move_to((33.3976, 10.7617));
    path.line_to((33.9568, 9.93265));
    path.conic_to((34.516, 9.10361), (35.3451, 9.6628), 0.707107);
    path.conic_to((36.1741, 10.222), (35.6149, 11.051), 0.707107);
    path.line_to((35.0557, 11.8801));
    path.conic_to((34.4965, 12.7091), (33.6675, 12.1499), 0.707107);
    path.conic_to((32.8385, 11.5907), (33.3976, 10.7617), 0.707107);
    path.close();
    path.move_to((35.0752, 8.27457));
    path.line_to((35.6344, 7.44554));
    path.conic_to((36.1936, 6.6165), (37.0226, 7.17569), 0.707107);
    path.conic_to((37.8517, 7.73488), (37.2925, 8.56392), 0.707107);
    path.line_to((36.7333, 9.39296));
    path.conic_to((36.1741, 10.222), (35.3451, 9.6628), 0.707107);
    path.conic_to((34.516, 9.10361), (35.0752, 8.27457), 0.707107);
    path.close();
    path.move_to((36.7528, 5.78746));
    path.line_to((37.312, 4.95842));
    path.conic_to((37.8712, 4.12939), (38.7002, 4.68858), 0.707107);
    path.conic_to((39.5293, 5.24777), (38.9701, 6.07681), 0.707107);
    path.line_to((38.4109, 6.90585));
    path.conic_to((37.8517, 7.73488), (37.0226, 7.17569), 0.707107);
    path.conic_to((36.1936, 6.6165), (36.7528, 5.78746), 0.707107);
    path.close();
    path.move_to((39.9447, 3.72429));
    path.quad_to((40.3524, 4.01069), (40.7489, 4.31248));
    path.conic_to((41.5445, 4.9182), (40.9388, 5.71387), 0.707107);
    path.conic_to((40.3331, 6.50955), (39.5374, 5.90383), 0.707107);
    path.quad_to((39.1714, 5.62521), (38.7951, 5.36088));
    path.conic_to((37.9768, 4.78608), (38.5516, 3.96779), 0.707107);
    path.conic_to((39.1264, 3.14949), (39.9447, 3.72429), 0.707107);
    path.close();
    path.move_to((42.3194, 5.60826));
    path.quad_to((42.707, 5.95446), (43.0804, 6.31583));
    path.conic_to((43.7991, 7.01122), (43.1037, 7.72985), 0.707107);
    path.conic_to((42.4083, 8.44848), (41.6896, 7.75308), 0.707107);
    path.quad_to((41.3448, 7.41944), (40.9871, 7.09992));
    path.conic_to((40.2413, 6.43379), (40.9074, 5.68796), 0.707107);
    path.conic_to((41.5735, 4.94212), (42.3194, 5.60826), 0.707107);
    path.close();
    path.move_to((44.5406, 7.84871));
    path.quad_to((44.8959, 8.25352), (45.2341, 8.67266));
    path.conic_to((45.862, 9.4509), (45.0838, 10.0789), 0.707107);
    path.conic_to((44.3056, 10.7068), (43.6776, 9.9286), 0.707107);
    path.quad_to((43.3654, 9.54174), (43.0374, 9.16805));
    path.conic_to((42.3778, 8.41649), (43.1293, 7.75682), 0.707107);
    path.conic_to((43.8809, 7.09715), (44.5406, 7.84871), 0.707107);
    path.close();
    path.move_to((46.528, 10.4211));
    path.quad_to((46.815, 10.8449), (47.0851, 11.2796));
    path.conic_to((47.6128, 12.129), (46.7633, 12.6567), 0.707107);
    path.conic_to((45.9139, 13.1844), (45.3862, 12.335), 0.707107);
    path.quad_to((45.1369, 11.9337), (44.872, 11.5426));
    path.conic_to((44.3113, 10.7146), (45.1393, 10.1538), 0.707107);
    path.conic_to((45.9673, 9.5931), (46.528, 10.4211), 0.707107);
    path.close();
    path.move_to((48.1056, 13.0782));
    path.quad_to((48.3449, 13.542), (48.5654, 14.015));
    path.conic_to((48.9879, 14.9213), (48.0816, 15.3438), 0.707107);
    path.conic_to((47.1752, 15.7663), (46.7527, 14.86), 0.707107);
    path.quad_to((46.5492, 14.4234), (46.3283, 13.9953));
    path.conic_to((45.8698, 13.1066), (46.7584, 12.6481), 0.707107);
    path.conic_to((47.6471, 12.1895), (48.1056, 13.0782), 0.707107);
    path.close();
    path.move_to((49.3755, 15.9538));
    path.quad_to((49.5594, 16.4493), (49.7229, 16.9516));
    path.conic_to((50.0325, 17.9025), (49.0816, 18.2121), 0.707107);
    path.conic_to((48.1307, 18.5216), (47.8212, 17.5707), 0.707107);
    path.quad_to((47.6702, 17.1069), (47.5005, 16.6497));
    path.conic_to((47.1526, 15.7122), (48.0901, 15.3642), 0.707107);
    path.conic_to((49.0276, 15.0163), (49.3755, 15.9538), 0.707107);
    path.close();
    path.move_to((50.2964, 18.9923));
    path.quad_to((50.4191, 19.5089), (50.5206, 20.0302));
    path.conic_to((50.7117, 21.0117), (49.7302, 21.2029), 0.707107);
    path.conic_to((48.7486, 21.394), (48.5575, 20.4125), 0.707107);
    path.quad_to((48.4638, 19.9313), (48.3505, 19.4544));
    path.conic_to((48.1194, 18.4815), (49.0924, 18.2504), 0.707107);
    path.conic_to((50.0653, 18.0193), (50.2964, 18.9923), 0.707107);
    path.close();
    path.move_to((50.8373, 22.0956));
    path.quad_to((50.8955, 22.6138), (50.933, 23.1341));
    path.conic_to((51.0047, 24.1315), (50.0073, 24.2033), 0.707107);
    path.conic_to((49.0099, 24.275), (48.9381, 23.2776), 0.707107);
    path.quad_to((48.9036, 22.7975), (48.8498, 22.3191));
    path.conic_to((48.7381, 21.3253), (49.7318, 21.2136), 0.707107);
    path.conic_to((50.7255, 21.1019), (50.8373, 22.0956), 0.707107);
    path.close();
    path.move_to((50.9992, 25.2099));
    path.quad_to((50.9949, 25.7358), (50.9694, 26.2608));
    path.conic_to((50.9209, 27.2596), (49.9221, 27.2111), 0.707107);
    path.conic_to((48.9233, 27.1626), (48.9718, 26.1638), 0.707107);
    path.quad_to((48.9953, 25.679), (48.9992, 25.1938));
    path.conic_to((49.0073, 24.1938), (50.0073, 24.2019), 0.707107);
    path.conic_to((51.0072, 24.21), (50.9992, 25.2099), 0.707107);
    path.close();
    path.move_to((50.7839, 28.3454));
    path.quad_to((50.7172, 28.8596), (50.63, 29.3708));
    path.conic_to((50.4619, 30.3565), (49.4761, 30.1884), 0.707107);
    path.conic_to((48.4903, 30.0203), (48.6584, 29.0346), 0.707107);
    path.quad_to((48.7389, 28.5627), (48.8005, 28.088));
    path.conic_to((48.9292, 27.0963), (49.9209, 27.225), 0.707107);
    path.conic_to((50.9126, 27.3537), (50.7839, 28.3454), 0.707107);
    path.close();
    path.move_to((50.1906, 31.437));
    path.quad_to((50.0558, 31.9646), (49.899, 32.4861));
    path.conic_to((49.611, 33.4438), (48.6534, 33.1558), 0.707107);
    path.conic_to((47.6957, 32.8679), (47.9837, 31.9103), 0.707107);
    path.quad_to((48.1284, 31.4289), (48.2528, 30.9418));
    path.conic_to((48.5004, 29.9729), (49.4693, 30.2205), 0.707107);
    path.conic_to((50.4382, 30.4681), (50.1906, 31.437), 0.707107);
    path.close();
    path.move_to((49.1978, 34.5114));
    path.quad_to((49.0051, 35.0016), (48.7927, 35.4837));
    path.conic_to((48.3895, 36.3988), (47.4744, 35.9956), 0.707107);
    path.conic_to((46.5593, 35.5923), (46.9625, 34.6772), 0.707107);
    path.quad_to((47.1586, 34.2323), (47.3364, 33.7797));
    path.conic_to((47.7023, 32.849), (48.6329, 33.2149), 0.707107);
    path.conic_to((49.5636, 33.5807), (49.1978, 34.5114), 0.707107);
    path.close();
    path.move_to((47.8852, 37.3397));
    path.quad_to((47.6449, 37.7853), (47.3876, 38.2211));
    path.conic_to((46.879, 39.0821), (46.018, 38.5736), 0.707107);
    path.conic_to((45.1569, 38.0651), (45.6655, 37.204), 0.707107);
    path.quad_to((45.903, 36.8018), (46.1248, 36.3906));
    path.conic_to((46.5993, 35.5103), (47.4796, 35.9849), 0.707107);
    path.conic_to((48.3598, 36.4595), (47.8852, 37.3397), 0.707107);
    path.close();
    path.move_to((46.3154, 39.8881));
    path.quad_to((46.0303, 40.2962), (45.7299, 40.693));
    path.conic_to((45.1264, 41.4903), (44.3291, 40.8867), 0.707107);
    path.conic_to((43.5318, 40.2831), (44.1353, 39.4858), 0.707107);
    path.quad_to((44.4126, 39.1195), (44.6757, 38.7428));
    path.conic_to((45.2483, 37.923), (46.0682, 38.4956), 0.707107);
    path.conic_to((46.888, 39.0682), (46.3154, 39.8881), 0.707107);
    path.close();
    path.move_to((44.4398, 42.2654));
    path.quad_to((44.095, 42.6536), (43.7349, 43.0278));
    path.conic_to((43.0415, 43.7484), (42.321, 43.055), 0.707107);
    path.conic_to((41.6004, 42.3616), (42.2938, 41.641), 0.707107);
    path.quad_to((42.6261, 41.2957), (42.9444, 40.9374));
    path.conic_to((43.6084, 40.1897), (44.3561, 40.8537), 0.707107);
    path.conic_to((45.1038, 41.5177), (44.4398, 42.2654), 0.707107);
    path.close();
    path.move_to((42.2075, 44.4911));
    path.quad_to((41.804, 44.8473), (41.3862, 45.1865));
    path.conic_to((40.6098, 45.8167), (39.9795, 45.0403), 0.707107);
    path.conic_to((39.3493, 44.2639), (40.1257, 43.6336), 0.707107);
    path.quad_to((40.5114, 43.3205), (40.8838, 42.9918));
    path.conic_to((41.6335, 42.3299), (42.2953, 43.0796), 0.707107);
    path.conic_to((42.9572, 43.8292), (42.2075, 44.4911), 0.707107);
    path.close();
    path.move_to((39.6379, 46.488));
    path.quad_to((39.2151, 46.776), (38.7814, 47.0471));
    path.conic_to((37.9334, 47.5771), (37.4034, 46.7292), 0.707107);
    path.conic_to((36.8733, 45.8812), (37.7213, 45.3511), 0.707107);
    path.quad_to((38.1217, 45.1009), (38.5119, 44.835));
    path.conic_to((39.3383, 44.2721), (39.9013, 45.0985), 0.707107);
    path.conic_to((40.4643, 45.925), (39.6379, 46.488), 0.707107);
    path.close();
    path.move_to((36.9864, 48.0722));
    path.quad_to((36.5234, 48.3127), (36.0513, 48.5344));
    path.conic_to((35.1461, 48.9595), (34.7211, 48.0543), 0.707107);
    path.conic_to((34.296, 47.1491), (35.2012, 46.7241), 0.707107);
    path.quad_to((35.6371, 46.5194), (36.0644, 46.2974));
    path.conic_to((36.9518, 45.8364), (37.4128, 46.7238), 0.707107);
    path.conic_to((37.8738, 47.6112), (36.9864, 48.0722), 0.707107);
    path.close();
    path.move_to((34.1153, 49.3498));
    path.quad_to((33.6206, 49.535), (33.1187, 49.6999));
    path.conic_to((32.1687, 50.0122), (31.8565, 49.0622), 0.707107);
    path.conic_to((31.5442, 48.1122), (32.4942, 47.7999), 0.707107);
    path.quad_to((32.9575, 47.6477), (33.4141, 47.4767));
    path.conic_to((34.3507, 47.1261), (34.7012, 48.0627), 0.707107);
    path.conic_to((35.0518, 48.9992), (34.1153, 49.3498), 0.707107);
    path.close();
    path.move_to((31.08, 50.2791));
    path.quad_to((30.5637, 50.4033), (30.0427, 50.5063));
    path.conic_to((29.0617, 50.7002), (28.8678, 49.7192), 0.707107);
    path.conic_to((28.6738, 48.7382), (29.6548, 48.5443), 0.707107);
    path.quad_to((30.1357, 48.4492), (30.6122, 48.3346));
    path.conic_to((31.5845, 48.1007), (31.8184, 49.073), 0.707107);
    path.conic_to((32.0522, 50.0453), (31.08, 50.2791), 0.707107);
    path.close();
    path.move_to((27.9769, 50.829));
    path.quad_to((27.4588, 50.8887), (26.9386, 50.9276));
    path.conic_to((25.9414, 51.0022), (25.8668, 50.005), 0.707107);
    path.conic_to((25.7923, 49.0078), (26.7895, 48.9332), 0.707107);
    path.quad_to((27.2696, 48.8973), (27.7479, 48.8422));
    path.conic_to((28.7413, 48.7277), (28.8558, 49.7211), 0.707107);
    path.conic_to((28.9703, 50.7145), (27.9769, 50.829), 0.707107);
    path.close();
    path.move_to((24.8625, 50.9996));
    path.quad_to((24.3373, 50.9969), (23.8128, 50.9729));
    path.conic_to((22.8138, 50.9272), (22.8595, 49.9283), 0.707107);
    path.conic_to((22.9051, 48.9293), (23.9041, 48.975), 0.707107);
    path.quad_to((24.3884, 48.9971), (24.8731, 48.9997));
    path.conic_to((25.8731, 49.005), (25.8678, 50.005), 0.707107);
    path.conic_to((25.8624, 51.0049), (24.8625, 50.9996), 0.707107);
    path.close();
    path.move_to((21.7268, 50.7931));
    path.quad_to((21.2121, 50.7278), (20.7005, 50.642));
    path.conic_to((19.7143, 50.4767), (19.8796, 49.4905), 0.707107);
    path.conic_to((20.045, 48.5042), (21.0312, 48.6696), 0.707107);
    path.quad_to((21.5036, 48.7488), (21.9786, 48.8091));
    path.conic_to((22.9707, 48.9349), (22.8448, 49.927), 0.707107);
    path.conic_to((22.7189, 50.919), (21.7268, 50.7931), 0.707107);
    path.close();
    path.move_to((18.6372, 50.2094));
    path.quad_to((18.1089, 50.0761), (17.5865, 49.9207));
    path.conic_to((16.628, 49.6356), (16.9132, 48.6771), 0.707107);
    path.conic_to((17.1983, 47.7186), (18.1568, 48.0037), 0.707107);
    path.quad_to((18.639, 48.1472), (19.1267, 48.2702));
    path.conic_to((20.0963, 48.515), (19.8516, 49.4846), 0.707107);
    path.conic_to((19.6068, 50.4542), (18.6372, 50.2094), 0.707107);
    path.close();
    path.move_to((15.5577, 49.2248));
    path.quad_to((15.0665, 49.0334), (14.5834, 48.8222));
    path.conic_to((13.6672, 48.4215), (14.0678, 47.5053), 0.707107);
    path.conic_to((14.4684, 46.589), (15.3847, 46.9897), 0.707107);
    path.quad_to((15.8306, 47.1846), (16.284, 47.3614));
    path.conic_to((17.2158, 47.7246), (16.8526, 48.6563), 0.707107);
    path.conic_to((16.4894, 49.588), (15.5577, 49.2248), 0.707107);
    path.close();
    path.move_to((12.7231, 47.9189));
    path.quad_to((12.2765, 47.6797), (11.8395, 47.4233));
    path.conic_to((10.9771, 46.9171), (11.4833, 46.0547), 0.707107);
    path.conic_to((11.9894, 45.1922), (12.8519, 45.6984), 0.707107);
    path.quad_to((13.2552, 45.9351), (13.6675, 46.156));
    path.conic_to((14.549, 46.6282), (14.0768, 47.5096), 0.707107);
    path.conic_to((13.6046, 48.3911), (12.7231, 47.9189), 0.707107);
    path.close();
    path.move_to((10.1686, 46.3548));
    path.quad_to((9.76024, 46.0712), (9.363, 45.7722));
    path.conic_to((8.56406, 45.1708), (9.16549, 44.3718), 0.707107);
    path.conic_to((9.76691, 43.5729), (10.5658, 44.1743), 0.707107);
    path.quad_to((10.9325, 44.4504), (11.3095, 44.7122));
    path.conic_to((12.1308, 45.2826), (11.5604, 46.1039), 0.707107);
    path.conic_to((10.9899, 46.9253), (10.1686, 46.3548), 0.707107);
    path.close();
    path.move_to((7.78853, 44.4876));
    path.quad_to((7.39972, 44.1442), (7.02492, 43.7855));
    path.conic_to((6.3024, 43.0942), (6.99374, 42.3717), 0.707107);
    path.conic_to((7.68509, 41.6492), (8.40761, 42.3405), 0.707107);
    path.quad_to((8.7536, 42.6715), (9.11249, 42.9885));
    path.conic_to((9.86201, 43.6505), (9.20003, 44.4), 0.707107);
    path.conic_to((8.53805, 45.1496), (7.78853, 44.4876), 0.707107);
    path.close();
    path.move_to((5.55855, 42.2635));
    path.quad_to((5.20148, 41.8614), (4.86131, 41.4449));
    path.conic_to((4.22883, 40.6703), (5.0034, 40.0378), 0.707107);
    path.conic_to((5.77797, 39.4053), (6.41046, 40.1799), 0.707107);
    path.quad_to((6.72443, 40.5644), (7.05403, 40.9356));
    path.conic_to((7.71802, 41.6833), (6.97028, 42.3473), 0.707107);
    path.conic_to((6.22254, 43.0113), (5.55855, 42.2635), 0.707107);
    path.close();
    path.move_to((3.55261, 39.6973));
    path.quad_to((3.26341, 39.2752), (2.99107, 38.8422));
    path.conic_to((2.45867, 37.9957), (3.30517, 37.4633), 0.707107);
    path.conic_to((4.15167, 36.9309), (4.68406, 37.7774), 0.707107);
    path.quad_to((4.93548, 38.1772), (5.20241, 38.5667));
    path.conic_to((5.76769, 39.3916), (4.94279, 39.9569), 0.707107);
    path.conic_to((4.11789, 40.5222), (3.55261, 39.6973), 0.707107);
    path.close();
    path.move_to((1.96145, 37.0509));
    path.quad_to((1.71975, 36.5889), (1.49677, 36.1175));
    path.conic_to((1.06917, 35.2135), (1.97315, 34.7859), 0.707107);
    path.conic_to((2.87712, 34.3583), (3.30471, 35.2623), 0.707107);
    path.quad_to((3.51053, 35.6974), (3.73364, 36.1239));
    path.conic_to((4.19714, 37.01), (3.31105, 37.4735), 0.707107);
    path.conic_to((2.42495, 37.937), (1.96145, 37.0509), 0.707107);
    path.close();
    path.move_to((0.676191, 34.1844));
    path.quad_to((0.489621, 33.6902), (0.323275, 33.189));
    path.conic_to((0.00831527, 32.2399), (0.95742, 31.9249), 0.707107);
    path.conic_to((1.90653, 31.6099), (2.22149, 32.559), 0.707107);
    path.quad_to((2.37504, 33.0218), (2.54726, 33.4779));
    path.conic_to((2.9005, 34.4134), (1.96497, 34.7666), 0.707107);
    path.conic_to((1.02943, 35.1199), (0.676191, 34.1844), 0.707107);
    path.close();
    path.move_to((-0.261658, 31.1521));
    path.quad_to((-0.387304, 30.6362), (-0.491779, 30.1156));
    path.conic_to((-0.68853, 29.1351), (0.291923, 28.9384), 0.707107);
    path.conic_to((1.27238, 28.7416), (1.46913, 29.7221), 0.707107);
    path.quad_to((1.56557, 30.2026), (1.68155, 30.6789));
    path.conic_to((1.91817, 31.6505), (0.946565, 31.8871), 0.707107);
    path.conic_to((-0.0250367, 32.1237), (-0.261658, 31.1521), 0.707107);
    path.close();
    path.move_to((-0.820549, 28.0495));
    path.quad_to((-0.881733, 27.5314), (-0.922089, 27.0113));
    path.conic_to((-0.999449, 26.0143), (-0.00244591, 25.9369), 0.707107);
    path.conic_to((0.994557, 25.8596), (1.07192, 26.8566), 0.707107);
    path.quad_to((1.10917, 27.3367), (1.16565, 27.8149));
    path.conic_to((1.28293, 28.808), (0.289834, 28.9253), 0.707107);
    path.conic_to((-0.703265, 29.0426), (-0.820549, 28.0495), 0.707107);
    path.close();
    path.move_to((-0.999918, 24.9349));
    path.quad_to((-0.998605, 24.4104), (-0.976138, 23.8863));
    path.conic_to((-0.933305, 22.8873), (0.0657772, 22.9301), 0.707107);
    path.conic_to((1.06486, 22.9729), (1.02203, 23.972), 0.707107);
    path.quad_to((1.00129, 24.4557), (1.00008, 24.9399));
    path.conic_to((0.997572, 25.9399), (-0.0024244, 25.9374), 0.707107);
    path.conic_to((-1.00242, 25.9349), (-0.999918, 24.9349), 0.707107);
    path.close();
    path.move_to((-0.802212, 21.7991));
    path.quad_to((-0.738311, 21.284), (-0.653903, 20.7719));
    path.conic_to((-0.491283, 19.7852), (0.495406, 19.9478), 0.707107);
    path.conic_to((1.48209, 20.1104), (1.31948, 21.0971), 0.707107);
    path.quad_to((1.24156, 21.5698), (1.18257, 22.0453));
    path.conic_to((1.05946, 23.0377), (0.0670681, 22.9146), 0.707107);
    path.conic_to((-0.925325, 22.7915), (-0.802212, 21.7991), 0.707107);
    path.close();
    path.move_to((-0.228066, 18.7115));
    path.quad_to((-0.096172, 18.1824), (0.0577899, 17.6593));
    path.conic_to((0.340124, 16.7), (1.29944, 16.9823), 0.707107);
    path.conic_to((2.25876, 17.2646), (1.97642, 18.2239), 0.707107);
    path.quad_to((1.8343, 18.7068), (1.71255, 19.1953));
    path.conic_to((1.47069, 20.1656), (0.50038, 19.9237), 0.707107);
    path.conic_to((-0.46993, 19.6819), (-0.228066, 18.7115), 0.707107);
    path.close();
    path.move_to((0.74831, 15.6269));
    path.quad_to((0.938539, 15.1347), (1.14857, 14.6506));
    path.conic_to((1.54662, 13.7333), (2.46398, 14.1313), 0.707107);
    path.conic_to((3.38135, 14.5294), (2.9833, 15.4467), 0.707107);
    path.quad_to((2.78942, 15.8936), (2.61382, 16.3479));
    path.conic_to((2.25331, 17.2806), (1.32056, 16.9201), 0.707107);
    path.conic_to((0.387801, 16.5596), (0.74831, 15.6269), 0.707107);
    path.close();
    path.move_to((2.04744, 12.7861));
    path.quad_to((2.28569, 12.3384), (2.5412, 11.9003));
    path.conic_to((3.04504, 11.0365), (3.90884, 11.5403), 0.707107);
    path.conic_to((4.77264, 12.0442), (4.26881, 12.908), 0.707107);
    path.quad_to((4.03293, 13.3123), (3.81302, 13.7256));
    path.conic_to((3.34325, 14.6084), (2.46046, 14.1386), 0.707107);
    path.conic_to((1.57767, 13.6689), (2.04744, 12.7861), 0.707107);
    path.close();
    path.move_to((3.60589, 10.2253));
    path.quad_to((3.88812, 9.81661), (4.18576, 9.419));
    path.conic_to((4.78503, 8.61845), (5.58558, 9.21772), 0.707107);
    path.conic_to((6.38613, 9.81699), (5.78686, 10.6175), 0.707107);
    path.quad_to((5.51211, 10.9846), (5.25159, 11.3618));
    path.conic_to((4.68333, 12.1847), (3.86048, 11.6164), 0.707107);
    path.conic_to((3.03763, 11.0481), (3.60589, 10.2253), 0.707107);
    path.close();
    path.move_to((5.46482, 7.84259));
    path.quad_to((5.80682, 7.4532), (6.16407, 7.07773));
    path.conic_to((6.85339, 6.35327), (7.57785, 7.04259), 0.707107);
    path.conic_to((8.30231, 7.73191), (7.61299, 8.45636), 0.707107);
    path.quad_to((7.28322, 8.80295), (6.96752, 9.16239));
    path.conic_to((6.30762, 9.91375), (5.55627, 9.25385), 0.707107);
    path.conic_to((4.80492, 8.59395), (5.46482, 7.84259), 0.707107);
    path.close();
    path.move_to((7.68062, 5.60827));
    path.quad_to((8.08142, 5.25031), (8.49666, 4.90921));
    path.conic_to((9.26938, 4.27447), (9.90412, 5.04719), 0.707107);
    path.conic_to((10.5389, 5.81992), (9.76614, 6.45466), 0.707107);
    path.quad_to((9.38285, 6.76951), (9.01289, 7.09994));
    path.conic_to((8.26705, 7.76607), (7.60092, 7.02024), 0.707107);
    path.conic_to((6.93479, 6.2744), (7.68062, 5.60827), 0.707107);
    path.close();
    path.move_to((10.2392, 3.59627));
    path.quad_to((10.6626, 3.30433), (11.0971, 3.02935));
    path.conic_to((11.9421, 2.49463), (12.4768, 3.33965), 0.707107);
    path.conic_to((13.0116, 4.18467), (12.1666, 4.7194), 0.707107);
    path.quad_to((11.7654, 4.97322), (11.3747, 5.24271));
    path.conic_to((10.5515, 5.81043), (9.98373, 4.98721), 0.707107);
    path.conic_to((9.41601, 4.16399), (10.2392, 3.59627), 0.707107);
    path.close();
    path.move_to((12.8847, 1.99524));
    path.quad_to((13.3459, 1.75234), (13.8165, 1.52812));
    path.conic_to((14.7193, 1.09799), (15.1494, 2.00075), 0.707107);
    path.conic_to((15.5795, 2.90352), (14.6768, 3.33365), 0.707107);
    path.quad_to((14.2424, 3.54063), (13.8166, 3.76484));
    path.conic_to((12.9318, 4.23081), (12.4658, 3.34601), 0.707107);
    path.conic_to((11.9999, 2.46122), (12.8847, 1.99524), 0.707107);
    path.close();
    path.move_to((15.7467, 0.702339));
    path.quad_to((16.2402, 0.514409), (16.7409, 0.346672));
    path.conic_to((17.6891, 0.029011), (18.0067, 0.977215), 0.707107);
    path.conic_to((18.3244, 1.92542), (17.3762, 2.24308), 0.707107);
    path.quad_to((16.914, 2.39792), (16.4585, 2.57139));
    path.conic_to((15.524, 2.92729), (15.1681, 1.99276), 0.707107);
    path.conic_to((14.8122, 1.05824), (15.7467, 0.702339), 0.707107);
    path.close();
    path.move_to((18.7758, -0.24399));
    path.quad_to((19.2913, -0.371107), (19.8116, -0.477061));
    path.conic_to((20.7915, -0.676608), (20.9911, 0.303281), 0.707107);
    path.conic_to((21.1906, 1.28317), (20.2107, 1.48272), 0.707107);
    path.quad_to((19.7304, 1.58052), (19.2546, 1.69785));
    path.conic_to((18.2836, 1.93725), (18.0443, 0.966329), 0.707107);
    path.conic_to((17.8049, -0.00459272), (18.7758, -0.24399), 0.707107);
    path.close();
    path.move_to((21.878, -0.811882));
    path.quad_to((22.396, -0.874528), (22.916, -0.916348));
    path.conic_to((23.9128, -0.996504), (23.993, 0.000278629), 0.707107);
    path.conic_to((24.0731, 0.997061), (23.0764, 1.07722), 0.707107);
    path.quad_to((22.5963, 1.11582), (22.1182, 1.17365));
    path.conic_to((21.1254, 1.29372), (21.0053, 0.300958), 0.707107);
    path.conic_to((20.8853, -0.691807), (21.878, -0.811882), 0.707107);
    path.close();
    path.move_to((24.9926, -0.999999));
    path.quad_to((25.5166, -1.00015), (26.0401, -0.979188));
    path.conic_to((27.0393, -0.939179), (26.9992, 0.0600199), 0.707107);
    path.conic_to((26.9592, 1.05922), (25.96, 1.01921), 0.707107);
    path.quad_to((25.4768, 0.999863), (24.9932, 1.0));
    path.conic_to((23.9932, 1.00029), (23.9929, 0.000287339), 0.707107);
    path.conic_to((23.9926, -0.999713), (24.9926, -0.999999), 0.707107);
    path.close();
    path.move_to((28.1286, -0.811081));
    path.quad_to((28.6441, -0.748593), (29.1567, -0.665572));
    path.conic_to((30.1439, -0.505698), (29.984, 0.48144), 0.707107);
    path.conic_to((29.8241, 1.46858), (28.837, 1.3087), 0.707107);
    path.quad_to((28.3638, 1.23207), (27.8879, 1.17439));
    path.conic_to((26.8952, 1.05406), (27.0155, 0.0613233), 0.707107);
    path.conic_to((27.1359, -0.931411), (28.1286, -0.811081), 0.707107);
    path.close();
    path.move_to((31.214, -0.246499));
    path.quad_to((31.7439, -0.116076), (32.2679, 0.0364622));
    path.conic_to((33.228, 0.315996), (32.9485, 1.27613), 0.707107);
    path.conic_to((32.6689, 2.23627), (31.7088, 1.95673), 0.707107);
    path.quad_to((31.2252, 1.81593), (30.736, 1.69554));
    path.conic_to((29.765, 1.45654), (30.004, 0.48552), 0.707107);
    path.conic_to((30.243, -0.485499), (31.214, -0.246499), 0.707107);
    path.close();
    path.move_to((34.3038, 0.721629));
    path.quad_to((34.797, 0.910612), (35.282, 1.11946));
    path.conic_to((36.2005, 1.51493), (35.805, 2.43341), 0.707107);
    path.conic_to((35.4096, 3.35189), (34.4911, 2.95642), 0.707107);
    path.quad_to((34.0434, 2.76365), (33.5881, 2.5892));
    path.conic_to((32.6543, 2.23137), (33.0122, 1.29758), 0.707107);
    path.conic_to((33.37, 0.363796), (34.3038, 0.721629), 0.707107);
    path.close();
    path.move_to((37.1508, 2.01396));
    path.quad_to((37.5996, 2.2512), (38.0388, 2.50578));
    path.conic_to((38.904, 3.00727), (38.4025, 3.87244), 0.707107);
    path.conic_to((37.901, 4.7376), (37.0358, 4.23612), 0.707107);
    path.quad_to((36.6304, 4.00111), (36.2161, 3.78211));
    path.conic_to((35.332, 3.31476), (35.7994, 2.43069), 0.707107);
    path.conic_to((36.2667, 1.54661), (37.1508, 2.01396), 0.707107);
    path.close();
    path.move_to((39.718, 3.56681));
    path.quad_to((40.1269, 3.84765), (40.5249, 4.14392));
    path.conic_to((41.3271, 4.74104), (40.73, 5.54319), 0.707107);
    path.conic_to((40.1329, 6.34535), (39.3307, 5.74823), 0.707107);
    path.quad_to((38.9634, 5.47478), (38.5858, 5.21552));
    path.conic_to((37.7615, 4.64945), (38.3275, 3.82509), 0.707107);
    path.conic_to((38.8936, 3.00074), (39.718, 3.56681), 0.707107);
    path.close();
    path.move_to((42.1033, 5.41741));
    path.quad_to((42.4933, 5.75802), (42.8694, 6.11388));
    path.conic_to((43.5958, 6.80115), (42.9085, 7.52755), 0.707107);
    path.conic_to((42.2212, 8.25394), (41.4948, 7.56667), 0.707107);
    path.quad_to((41.1476, 7.23817), (40.7876, 6.92375));
    path.conic_to((40.0345, 6.26593), (40.6923, 5.51275), 0.707107);
    path.conic_to((41.3501, 4.75958), (42.1033, 5.41741), 0.707107);
    path.close();
    path.move_to((44.3419, 7.62498));
    path.quad_to((44.7007, 8.02444), (45.0428, 8.43835));
    path.conic_to((45.6797, 9.20922), (44.9089, 9.84622), 0.707107);
    path.conic_to((44.138, 10.4832), (43.501, 9.71234), 0.707107);
    path.quad_to((43.1852, 9.3302), (42.854, 8.96151));
    path.conic_to((42.1858, 8.21759), (42.9297, 7.54932), 0.707107);
    path.conic_to((43.6736, 6.88106), (44.3419, 7.62498), 0.707107);
    path.close();
    path.move_to((46.3599, 10.1759));
    path.quad_to((46.6546, 10.6005), (46.9322, 11.0366));
    path.conic_to((47.4693, 11.8801), (46.6257, 12.4172), 0.707107);
    path.conic_to((45.7822, 12.9542), (45.2451, 12.1107), 0.707107);
    path.quad_to((44.9889, 11.7082), (44.7168, 11.3162));
    path.conic_to((44.1467, 10.4947), (44.9682, 9.92452), 0.707107);
    path.conic_to((45.7897, 9.35435), (46.3599, 10.1759), 0.707107);
    path.close();
    path.move_to((47.9708, 12.8204));
    path.quad_to((48.2149, 13.2808), (48.4403, 13.7506));
    path.conic_to((48.873, 14.6521), (47.9715, 15.0848), 0.707107);
    path.conic_to((47.0699, 15.5174), (46.6372, 14.6159), 0.707107);
    path.quad_to((46.4291, 14.1822), (46.2038, 13.7573));
    path.conic_to((45.7354, 12.8738), (46.6188, 12.4054), 0.707107);
    path.conic_to((47.5023, 11.9369), (47.9708, 12.8204), 0.707107);
    path.close();
    path.move_to((49.2713, 15.6778));
    path.quad_to((49.4606, 16.1706), (49.6297, 16.6708));
    path.conic_to((49.9501, 17.6181), (49.0028, 17.9384), 0.707107);
    path.conic_to((48.0555, 18.2588), (47.7351, 17.3115), 0.707107);
    path.quad_to((47.5791, 16.8499), (47.4043, 16.3949));
    path.conic_to((47.0458, 15.4614), (47.9793, 15.1029), 0.707107);
    path.conic_to((48.9128, 14.7443), (49.2713, 15.6778), 0.707107);
    path.close();
    path.move_to((50.2261, 18.7037));
    path.quad_to((50.3547, 19.2188), (50.4621, 19.7388));
    path.conic_to((50.6645, 20.7182), (49.6852, 20.9205), 0.707107);
    path.conic_to((48.7059, 21.1229), (48.5035, 20.1436), 0.707107);
    path.quad_to((48.4043, 19.6636), (48.2856, 19.1881));
    path.conic_to((48.0435, 18.2178), (49.0137, 17.9757), 0.707107);
    path.conic_to((49.984, 17.7335), (50.2261, 18.7037), 0.707107);
    path.close();
    path.move_to((50.803, 21.8055));
    path.quad_to((50.8671, 22.3234), (50.9104, 22.8434));
    path.conic_to((50.9934, 23.8399), (49.9968, 23.9229), 0.707107);
    path.conic_to((49.0002, 24.0058), (48.9173, 23.0093), 0.707107);
    path.quad_to((48.8773, 22.5293), (48.8182, 22.0513));
    path.conic_to((48.6953, 21.0588), (49.6877, 20.936), 0.707107);
    path.conic_to((50.6801, 20.8131), (50.803, 21.8055), 0.707107);
    path.close();
    path.move_to((50.9999, 24.9202));
    path.quad_to((51.0015, 25.4434), (50.982, 25.9664));
    path.conic_to((50.9449, 26.9657), (49.9456, 26.9286), 0.707107);
    path.conic_to((48.9463, 26.8914), (48.9834, 25.8921), 0.707107);
    path.quad_to((49.0014, 25.4094), (48.9999, 24.9263));
    path.conic_to((48.9968, 23.9263), (49.9968, 23.9232), 0.707107);
    path.conic_to((50.9968, 23.9202), (50.9999, 24.9202), 0.707107);
    path.close();
    path.move_to((50.8198, 28.0562));
    path.quad_to((50.7587, 28.5721), (50.677, 29.0852));
    path.conic_to((50.5199, 30.0728), (49.5323, 29.9157), 0.707107);
    path.conic_to((48.5448, 29.7586), (48.7019, 28.771), 0.707107);
    path.quad_to((48.7772, 28.2974), (48.8336, 27.8211));
    path.conic_to((48.9512, 26.8281), (49.9442, 26.9456), 0.707107);
    path.conic_to((50.9373, 27.0632), (50.8198, 28.0562), 0.707107);
    path.close();
    path.move_to((50.2647, 31.1395));
    path.quad_to((50.1358, 31.6701), (49.9847, 32.1949));
    path.conic_to((49.7079, 33.1558), (48.747, 32.8791), 0.707107);
    path.conic_to((47.786, 32.6024), (48.0628, 31.6414), 0.707107);
    path.quad_to((48.2022, 31.1571), (48.3213, 30.6672));
    path.conic_to((48.5574, 29.6955), (49.5291, 29.9317), 0.707107);
    path.conic_to((50.5009, 30.1678), (50.2647, 31.1395), 0.707107);
    path.close();
    path.move_to((49.3049, 34.2343));
    path.quad_to((49.1171, 34.7285), (48.9095, 35.2145));
    path.conic_to((48.5166, 36.1341), (47.597, 35.7412), 0.707107);
    path.conic_to((46.6774, 35.3483), (47.0703, 34.4288), 0.707107);
    path.quad_to((47.262, 33.9801), (47.4353, 33.524));
    path.conic_to((47.7904, 32.5892), (48.7252, 32.9444), 0.707107);
    path.conic_to((49.66, 33.2995), (49.3049, 34.2343), 0.707107);
    path.close();
    path.move_to((48.0194, 37.0875));
    path.quad_to((47.7831, 37.5374), (47.5295, 37.9777));
    path.conic_to((47.0304, 38.8443), (46.1638, 38.3451), 0.707107);
    path.conic_to((45.2973, 37.846), (45.7965, 36.9795), 0.707107);
    path.quad_to((46.0306, 36.5729), (46.2487, 36.1577));
    path.conic_to((46.7136, 35.2723), (47.5989, 35.7372), 0.707107);
    path.conic_to((48.4843, 36.2021), (48.0194, 37.0875), 0.707107);
    path.close();
    path.move_to((46.4721, 39.6612));
    path.quad_to((46.1926, 40.0705), (45.8977, 40.4688));
    path.conic_to((45.3028, 41.2726), (44.499, 40.6776), 0.707107);
    path.conic_to((43.6953, 40.0827), (44.2902, 39.2789), 0.707107);
    path.quad_to((44.5624, 38.9112), (44.8204, 38.5334));
    path.conic_to((45.3843, 37.7075), (46.2101, 38.2714), 0.707107);
    path.conic_to((47.036, 38.8353), (46.4721, 39.6612), 0.707107);
    path.close();
    path.move_to((44.6298, 42.0491));
    path.quad_to((44.2906, 42.4396), (43.9361, 42.8164));
    path.conic_to((43.2509, 43.5447), (42.5226, 42.8595), 0.707107);
    path.conic_to((41.7942, 42.1742), (42.4795, 41.4459), 0.707107);
    path.quad_to((42.8067, 41.0981), (43.1198, 40.7376));
    path.conic_to((43.7756, 39.9826), (44.5306, 40.6383), 0.707107);
    path.conic_to((45.2856, 41.2941), (44.6298, 42.0491), 0.707107);
    path.close();
    path.move_to((42.4305, 44.2919));
    path.quad_to((42.0324, 44.6516), (41.6198, 44.9946));
    path.conic_to((40.8507, 45.6338), (40.2115, 44.8648), 0.707107);
    path.conic_to((39.5723, 44.0958), (40.3413, 43.4566), 0.707107);
    path.quad_to((40.7222, 43.1399), (41.0897, 42.8079));
    path.conic_to((41.8317, 42.1375), (42.5021, 42.8795), 0.707107);
    path.conic_to((43.1725, 43.6215), (42.4305, 44.2919), 0.707107);
    path.close();
    path.move_to((39.8873, 46.3159));
    path.quad_to((39.4613, 46.6134), (39.0238, 46.8936));
    path.conic_to((38.1818, 47.433), (37.6424, 46.5909), 0.707107);
    path.conic_to((37.103, 45.7489), (37.9451, 45.2095), 0.707107);
    path.quad_to((38.3489, 44.9508), (38.7421, 44.6763));
    path.conic_to((39.5619, 44.1037), (40.1345, 44.9235), 0.707107);
    path.conic_to((40.7071, 45.7434), (39.8873, 46.3159), 0.707107);
    path.close();
    path.move_to((37.2437, 47.9367));
    path.quad_to((36.7842, 48.182), (36.3153, 48.4086));
    path.conic_to((35.415, 48.8439), (34.9797, 47.9435), 0.707107);
    path.conic_to((34.5445, 47.0432), (35.4449, 46.608), 0.707107);
    path.quad_to((35.8778, 46.3987), (36.3019, 46.1723));
    path.conic_to((37.1841, 45.7014), (37.655, 46.5836), 0.707107);
    path.conic_to((38.1259, 47.4658), (37.2437, 47.9367), 0.707107);
    path.close();
    path.move_to((34.3909, 49.2448));
    path.quad_to((33.8988, 49.4354), (33.3992, 49.606));
    path.conic_to((32.4528, 49.929), (32.1298, 48.9826), 0.707107);
    path.conic_to((31.8068, 48.0362), (32.7532, 47.7132), 0.707107);
    path.quad_to((33.2142, 47.5558), (33.6685, 47.3798));
    path.conic_to((34.601, 47.0186), (34.9622, 47.9511), 0.707107);
    path.conic_to((35.3234, 48.8836), (34.3909, 49.2448), 0.707107);
    path.close();
    path.move_to((31.3682, 50.208));
    path.quad_to((30.8535, 50.3381), (30.3338, 50.447));
    path.conic_to((29.3551, 50.6521), (29.15, 49.6734), 0.707107);
    path.conic_to((28.9448, 48.6947), (29.9236, 48.4895), 0.707107);
    path.quad_to((30.4033, 48.389), (30.8784, 48.269));
    path.conic_to((31.8479, 48.024), (32.0929, 48.9936), 0.707107);
    path.conic_to((32.3378, 49.9631), (31.3682, 50.208), 0.707107);
    path.close();
    path.move_to((28.2669, 50.7939));
    path.quad_to((27.7491, 50.8595), (27.2292, 50.9043));
    path.conic_to((26.2329, 50.99), (26.1472, 49.9937), 0.707107);
    path.conic_to((26.0615, 48.9973), (27.0578, 48.9116), 0.707107);
    path.quad_to((27.5378, 48.8703), (28.0156, 48.8098));
    path.conic_to((29.0077, 48.6841), (29.1334, 49.6762), 0.707107);
    path.conic_to((29.259, 50.6683), (28.2669, 50.7939), 0.707107);
    path.close();
    path.move_to((25.1523, 50.9996));
    path.quad_to((24.6297, 51.0026), (24.1072, 50.9847));
    path.conic_to((23.1078, 50.9503), (23.1422, 49.9509), 0.707107);
    path.conic_to((23.1765, 48.9515), (24.1759, 48.9858), 0.707107);
    path.quad_to((24.658, 49.0024), (25.1406, 48.9996));
    path.conic_to((26.1406, 48.9937), (26.1464, 49.9937), 0.707107);
    path.conic_to((26.1523, 50.9937), (25.1523, 50.9996), 0.707107);
    path.close();
    path.move_to((22.0162, 50.8282));
    path.quad_to((21.4999, 50.7686), (20.9863, 50.6883));
    path.conic_to((19.9983, 50.5339), (20.1527, 49.5459), 0.707107);
    path.conic_to((20.307, 48.5579), (21.295, 48.7123), 0.707107);
    path.quad_to((21.7691, 48.7864), (22.2457, 48.8414));
    path.conic_to((23.2391, 48.9562), (23.1243, 49.9496), 0.707107);
    path.conic_to((23.0096, 50.943), (22.0162, 50.8282), 0.707107);
    path.close();
    path.move_to((18.9351, 50.2827));
    path.quad_to((18.4037, 50.1553), (17.8782, 50.0056));
    path.conic_to((16.9164, 49.7317), (17.1904, 48.7699), 0.707107);
    path.conic_to((17.4643, 47.8082), (18.426, 48.0821), 0.707107);
    path.quad_to((18.9112, 48.2203), (19.4016, 48.3379));
    path.conic_to((20.374, 48.5712), (20.1408, 49.5436), 0.707107);
    path.conic_to((19.9075, 50.516), (18.9351, 50.2827), 0.707107);
    path.close();
    path.move_to((15.8352, 49.3312));
    path.quad_to((15.3403, 49.1448), (14.8531, 48.9383));
    path.conic_to((13.9324, 48.548), (14.3227, 47.6273), 0.707107);
    path.conic_to((14.713, 46.7066), (15.6337, 47.0969), 0.707107);
    path.quad_to((16.0832, 47.2874), (16.5402, 47.4596));
    path.conic_to((17.476, 47.812), (17.1235, 48.7479), 0.707107);
    path.conic_to((16.771, 49.6837), (15.8352, 49.3312), 0.707107);
    path.close();
    path.move_to((12.9759, 48.0526));
    path.quad_to((12.5249, 47.8173), (12.0835, 47.5647));
    path.conic_to((11.2156, 47.0679), (11.7124, 46.2), 0.707107);
    path.conic_to((12.2092, 45.3321), (13.0771, 45.8289), 0.707107);
    path.quad_to((13.4846, 46.0622), (13.9009, 46.2793));
    path.conic_to((14.7875, 46.7418), (14.325, 47.6284), 0.707107);
    path.conic_to((13.8626, 48.5151), (12.9759, 48.0526), 0.707107);
    path.close();
    path.move_to((10.3957, 46.5108));
    path.quad_to((9.9861, 46.2327), (9.58733, 45.9392));
    path.conic_to((8.78198, 45.3464), (9.37478, 44.541), 0.707107);
    path.conic_to((9.96757, 43.7357), (10.7729, 44.3285), 0.707107);
    path.quad_to((11.141, 44.5994), (11.5191, 44.8561));
    path.conic_to((12.3464, 45.4178), (11.7847, 46.2451), 0.707107);
    path.conic_to((11.223, 47.0725), (10.3957, 46.5108), 0.707107);
    path.close();
    path.move_to((8.00525, 44.6769));
    path.quad_to((7.6141, 44.339), (7.23672, 43.9859));
    path.conic_to((6.50649, 43.3027), (7.18969, 42.5725), 0.707107);
    path.conic_to((7.87289, 41.8423), (8.60312, 42.5255), 0.707107);
    path.quad_to((8.95149, 42.8514), (9.31254, 43.1632));
    path.conic_to((10.0693, 43.8169), (9.4157, 44.5737), 0.707107);
    path.conic_to((8.76206, 45.3305), (8.00525, 44.6769), 0.707107);
    path.close();
    path.move_to((5.75818, 42.4858));
    path.quad_to((5.39763, 42.089), (5.05371, 41.6777));
    path.conic_to((4.41226, 40.9105), (5.17942, 40.2691), 0.707107);
    path.conic_to((5.94658, 39.6276), (6.58804, 40.3948), 0.707107);
    path.quad_to((6.90548, 40.7744), (7.23832, 41.1407));
    path.conic_to((7.91085, 41.8808), (7.17078, 42.5533), 0.707107);
    path.conic_to((6.43071, 43.2258), (5.75818, 42.4858), 0.707107);
    path.close();
    path.move_to((3.72821, 39.9503));
    path.quad_to((3.42794, 39.523), (3.1451, 39.0842));
    path.conic_to((2.6034, 38.2436), (3.44397, 37.7019), 0.707107);
    path.conic_to((4.28454, 37.1602), (4.82624, 38.0008), 0.707107);
    path.quad_to((5.08734, 38.4059), (5.3645, 38.8003));
    path.conic_to((5.93951, 39.6184), (5.12137, 40.1934), 0.707107);
    path.conic_to((4.30322, 40.7684), (3.72821, 39.9503), 0.707107);
    path.close();
    path.move_to((2.09762, 37.3078));
    path.quad_to((1.85114, 36.8491), (1.62324, 36.381));
    path.conic_to((1.18551, 35.4819), (2.08461, 35.0442), 0.707107);
    path.conic_to((2.98372, 34.6064), (3.42145, 35.5055), 0.707107);
    path.quad_to((3.63184, 35.9377), (3.85934, 36.361));
    path.conic_to((4.33272, 37.2419), (3.45185, 37.7153), 0.707107);
    path.conic_to((2.57099, 38.1886), (2.09762, 37.3078), 0.707107);
    path.close();
    path.move_to((0.781912, 34.4596));
    path.quad_to((0.589924, 33.9681), (0.418029, 33.4692));
    path.conic_to((0.0922952, 32.5237), (1.03776, 32.198), 0.707107);
    path.conic_to((1.98322, 31.8722), (2.30895, 32.8177), 0.707107);
    path.quad_to((2.46761, 33.2782), (2.64484, 33.7319));
    path.conic_to((3.00867, 34.6634), (2.07721, 35.0272), 0.707107);
    path.conic_to((1.14575, 35.3911), (0.781912, 34.4596), 0.707107);
    path.close();
    path.move_to((-0.189761, 31.4402));
    path.quad_to((-0.321263, 30.9258), (-0.431662, 30.4065));
    path.conic_to((-0.639608, 29.4284), (0.338532, 29.2205), 0.707107);
    path.conic_to((1.31667, 29.0125), (1.52462, 29.9906), 0.707107);
    path.quad_to((1.62653, 30.47), (1.74791, 30.9448));
    path.conic_to((1.99561, 31.9136), (1.02677, 32.1613), 0.707107);
    path.conic_to((0.0579369, 32.409), (-0.189761, 31.4402), 0.707107);
    path.close();
    path.move_to((-0.784658, 28.3394));
    path.quad_to((-0.851693, 27.8218), (-0.897902, 27.3019));
    path.conic_to((-0.986437, 26.3058), (0.00963629, 26.2173), 0.707107);
    path.conic_to((1.00571, 26.1288), (1.09424, 27.1248), 0.707107);
    path.quad_to((1.1369, 27.6047), (1.19878, 28.0825));
    path.conic_to((1.32721, 29.0742), (0.335496, 29.2027), 0.707107);
    path.conic_to((-0.656222, 29.3311), (-0.784658, 28.3394), 0.707107);
    path.close();
    path.move_to((-0.999031, 25.2248));
    path.quad_to((-1.00354, 24.7027), (-0.987098, 24.1809));
    path.conic_to((-0.955596, 23.1814), (0.0439078, 23.2129), 0.707107);
    path.conic_to((1.04341, 23.2444), (1.01191, 24.2439), 0.707107);
    path.quad_to((0.996728, 24.7256), (1.00089, 25.2075));
    path.conic_to((1.00954, 26.2075), (0.00957754, 26.2161), 0.707107);
    path.conic_to((-0.990385, 26.2248), (-0.999031, 25.2248), 0.707107);
    path.close();
    path.move_to((-0.836492, 22.0887));
    path.quad_to((-0.778263, 21.5719), (-0.699419, 21.0579));
    path.conic_to((-0.5478, 20.0695), (0.440639, 20.2211), 0.707107);
    path.conic_to((1.42908, 20.3727), (1.27746, 21.3612), 0.707107);
    path.quad_to((1.20468, 21.8356), (1.15093, 22.3126));
    path.conic_to((1.03896, 23.3063), (0.0452449, 23.1944), 0.707107);
    path.conic_to((-0.948466, 23.0824), (-0.836492, 22.0887), 0.707107);
    path.close();
    path.move_to((-0.300548, 19.0098));
    path.quad_to((-0.174573, 18.4777), (-0.0263361, 17.9514));
    path.conic_to((0.244762, 16.9889), (1.20731, 17.26), 0.707107);
    path.conic_to((2.16987, 17.5311), (1.89877, 18.4936), 0.707107);
    path.quad_to((1.76193, 18.9794), (1.64565, 19.4706));
    path.conic_to((1.41526, 20.4437), (0.442159, 20.2133), 0.707107);
    path.conic_to((-0.530939, 19.9829), (-0.300548, 19.0098), 0.707107);
    path.close();
    path.move_to((0.642658, 15.9049));
    path.quad_to((0.827861, 15.409), (1.0331, 14.9209));
    path.conic_to((1.42076, 13.9991), (2.34256, 14.3868), 0.707107);
    path.conic_to((3.26437, 14.7744), (2.87671, 15.6962), 0.707107);
    path.quad_to((2.68726, 16.1467), (2.5163, 16.6046));
    path.conic_to((2.16648, 17.5414), (1.22967, 17.1916), 0.707107);
    path.conic_to((0.292846, 16.8418), (0.642658, 15.9049), 0.707107);
    path.close();
    path.move_to((1.91434, 13.0395));
    path.quad_to((2.14856, 12.5875), (2.40031, 12.1449));
    path.conic_to((2.89473, 11.2757), (3.76395, 11.7701), 0.707107);
    path.conic_to((4.63317, 12.2645), (4.13875, 13.1337), 0.707107);
    path.quad_to((3.90637, 13.5423), (3.69016, 13.9596));
    path.conic_to((3.23014, 14.8475), (2.34223, 14.3875), 0.707107);
    path.conic_to((1.45432, 13.9275), (1.91434, 13.0395), 0.707107);
    path.close();
    path.move_to((3.45073, 10.4525));
    path.quad_to((3.72744, 10.0426), (4.01954, 9.64356));
    path.conic_to((4.61017, 8.83661), (5.41711, 9.42725), 0.707107);
    path.conic_to((6.22405, 10.0179), (5.63342, 10.8248), 0.707107);
    path.quad_to((5.36379, 11.1932), (5.10836, 11.5716));
    path.conic_to((4.54884, 12.4004), (3.72003, 11.8409), 0.707107);
    path.conic_to((2.89121, 11.2813), (3.45073, 10.4525), 0.707107);
    path.close();
    path.move_to((5.2763, 8.05964));
    path.quad_to((5.61273, 7.66793), (5.96445, 7.2899));
    path.conic_to((6.6456, 6.55776), (7.37774, 7.23892), 0.707107);
    path.conic_to((8.10988, 7.92008), (7.42872, 8.65221), 0.707107);
    path.quad_to((7.10407, 9.00116), (6.79351, 9.36274));
    path.conic_to((6.14196, 10.1213), (5.38336, 9.46979), 0.707107);
    path.conic_to((4.62475, 8.81824), (5.2763, 8.05964), 0.707107);
    path.close();
    path.move_to((7.45913, 5.80839));
    path.quad_to((7.85457, 5.44696), (8.26455, 5.10214));
    path.conic_to((9.02985, 4.45847), (9.67352, 5.22377), 0.707107);
    path.conic_to((10.3172, 5.98907), (9.5519, 6.63274), 0.707107);
    path.quad_to((9.17345, 6.95105), (8.80843, 7.28467));
    path.conic_to((8.07029, 7.95931), (7.39564, 7.22117), 0.707107);
    path.conic_to((6.72099, 6.48303), (7.45913, 5.80839), 0.707107);
    path.close();
    path.move_to((9.98688, 3.77251));
    path.quad_to((10.4153, 3.46948), (10.8557, 3.18397));
    path.conic_to((11.6948, 2.63996), (12.2388, 3.47904), 0.707107);
    path.conic_to((12.7828, 4.31812), (11.9437, 4.86213), 0.707107);
    path.quad_to((11.5373, 5.12566), (11.1417, 5.40539));
    path.conic_to((10.3253, 5.98282), (9.74787, 5.16638), 0.707107);
    path.conic_to((9.17044, 4.34994), (9.98688, 3.77251), 0.707107);
    path.close();
    path.move_to((12.6283, 2.13208));
    path.quad_to((13.0861, 1.88442), (13.5534, 1.65529));
    path.conic_to((14.4513, 1.21504), (14.8915, 2.11291), 0.707107);
    path.conic_to((15.3318, 3.01078), (14.4339, 3.45104), 0.707107);
    path.quad_to((14.0025, 3.66255), (13.58, 3.89115));
    path.conic_to((12.7005, 4.36698), (12.2246, 3.48744), 0.707107);
    path.conic_to((11.7488, 2.60791), (12.6283, 2.13208), 0.707107);
    path.close();
    path.move_to((15.4718, 0.808815));
    path.quad_to((15.9627, 0.615476), (16.461, 0.442208));
    path.conic_to((17.4055, 0.113784), (17.7339, 1.05831), 0.707107);
    path.conic_to((18.0624, 2.00284), (17.1178, 2.33127), 0.707107);
    path.quad_to((16.6578, 2.49121), (16.2047, 2.66968));
    path.conic_to((15.2743, 3.03614), (14.9078, 2.10571), 0.707107);
    path.conic_to((14.5414, 1.17528), (15.4718, 0.808815), 0.707107);
    path.close();
    path.move_to((18.4879, -0.171272));
    path.quad_to((19.0019, -0.304236), (19.5208, -0.416111));
    path.conic_to((20.4984, -0.62685), (20.7091, 0.350692), 0.707107);
    path.conic_to((20.9198, 1.32823), (19.9423, 1.53897), 0.707107);
    path.quad_to((19.4633, 1.64224), (18.9889, 1.76498));
    path.conic_to((18.0207, 2.01544), (17.7703, 1.04732), 0.707107);
    path.conic_to((17.5198, 0.0791926), (18.4879, -0.171272), 0.707107);
    path.close();
    path.move_to((21.5882, -0.77517));
    path.quad_to((22.1056, -0.843665), (22.6254, -0.891339));
    path.conic_to((23.6212, -0.982672), (23.7126, 0.0131486), 0.707107);
    path.conic_to((23.8039, 1.00897), (22.8081, 1.1003), 0.707107);
    path.quad_to((22.3283, 1.14431), (21.8506, 1.20754));
    path.conic_to((20.8592, 1.33876), (20.728, 0.347405), 0.707107);
    path.conic_to((20.5968, -0.643948), (21.5882, -0.77517), 0.707107);
    path.close();
    path.move_to((24.7026, -0.998301));
    path.quad_to((25.2241, -1.00426), (25.7453, -0.989316));
    path.conic_to((26.7449, -0.960651), (26.7162, 0.0389383), 0.707107);
    path.conic_to((26.6876, 1.03853), (25.688, 1.00986), 0.707107);
    path.quad_to((25.2068, 0.996064), (24.7255, 1.00157));
    path.conic_to((23.7256, 1.013), (23.7141, 0.0130688), 0.707107);
    path.conic_to((23.7027, -0.986866), (24.7026, -0.998301), 0.707107);
    path.close();
    path.move_to((27.8388, -0.844563));
    path.quad_to((28.3559, -0.787759), (28.8704, -0.710314));
    path.conic_to((29.8592, -0.561454), (29.7104, 0.427404), 0.707107);
    path.conic_to((29.5615, 1.41626), (28.5726, 1.2674), 0.707107);
    path.quad_to((28.0978, 1.19591), (27.6204, 1.14348));
    path.conic_to((26.6264, 1.0343), (26.7356, 0.0402742), 0.707107);
    path.conic_to((26.8447, -0.953747), (27.8388, -0.844563), 0.707107);
    path.close();
    path.move_to((30.9153, -0.318153));
    path.quad_to((31.4481, -0.193671), (31.9752, -0.046875));
    path.conic_to((32.9386, 0.221405), (32.6703, 1.18475), 0.707107);
    path.conic_to((32.402, 2.14809), (31.4387, 1.87981), 0.707107);
    path.quad_to((30.9521, 1.74431), (30.4603, 1.6294));
    path.conic_to((29.4865, 1.40189), (29.714, 0.428111), 0.707107);
    path.conic_to((29.9416, -0.545664), (30.9153, -0.318153), 0.707107);
    path.close();
    path.move_to((34.0252, 0.616677));
    path.quad_to((34.5221, 0.800609), (35.0111, 1.00465));
    path.conic_to((35.934, 1.3897), (35.549, 2.31259), 0.707107);
    path.conic_to((35.1639, 3.23549), (34.241, 2.85044), 0.707107);
    path.quad_to((33.7896, 2.66211), (33.3309, 2.49232));
    path.conic_to((32.3931, 2.1452), (32.7402, 1.20738), 0.707107);
    path.conic_to((33.0873, 0.269559), (34.0252, 0.616677), 0.707107);
    path.close();
    path.move_to((36.8967, 1.88141));
    path.quad_to((37.3499, 2.11462), (37.7936, 2.3654));
    path.conic_to((38.6641, 2.85746), (38.1721, 3.72802), 0.707107);
    path.conic_to((37.68, 4.59858), (36.8094, 4.10652), 0.707107);
    path.quad_to((36.3999, 3.87504), (35.9815, 3.65976));
    path.conic_to((35.0924, 3.2022), (35.5499, 2.31302), 0.707107);
    path.conic_to((36.0075, 1.42384), (36.8967, 1.88141), 0.707107);
    path.close();
    path.move_to((39.4914, 3.413));
    path.line_to((39.5381, 3.44439));
    path.quad_to((39.9244, 3.70494), (40.3002, 3.97845));
    path.conic_to((41.1087, 4.56692), (40.5202, 5.37544), 0.707107);
    path.conic_to((39.9317, 6.18396), (39.1232, 5.59549), 0.707107);
    path.quad_to((38.7763, 5.34298), (38.4215, 5.10371));
    path.line_to((38.3749, 5.07232));
    path.conic_to((37.5452, 4.51406), (38.1035, 3.68439), 0.707107);
    path.conic_to((38.6618, 2.85473), (39.4914, 3.413), 0.707107);
    path.close();
    path.move_to((41.8859, 5.22965));
    path.quad_to((42.2782, 5.56471), (42.6568, 5.91499));
    path.conic_to((43.3908, 6.5941), (42.7117, 7.32814), 0.707107);
    path.conic_to((42.0326, 8.06218), (41.2986, 7.38308), 0.707107);
    path.quad_to((40.949, 7.05968), (40.587, 6.75043));
    path.conic_to((39.8266, 6.10097), (40.476, 5.34058), 0.707107);
    path.conic_to((41.1255, 4.58018), (41.8859, 5.22965), 0.707107);
    path.close();
    path.move_to((44.1413, 7.40421));
    path.quad_to((44.5035, 7.79829), (44.8493, 8.20695));
    path.conic_to((45.4952, 8.97038), (44.7317, 9.61627), 0.707107);
    path.conic_to((43.9683, 10.2622), (43.3224, 9.49874), 0.707107);
    path.quad_to((43.0033, 9.1215), (42.6689, 8.75773));
    path.conic_to((41.9921, 8.02152), (42.7283, 7.34476), 0.707107);
    path.conic_to((43.4645, 6.668), (44.1413, 7.40421), 0.707107);
    path.close();
    path.move_to((46.183, 9.9242));
    path.quad_to((46.4888, 10.3539), (46.777, 10.7957));
    path.conic_to((47.3233, 11.6332), (46.4857, 12.1796), 0.707107);
    path.conic_to((45.6482, 12.7259), (45.1018, 11.8883), 0.707107);
    path.quad_to((44.8358, 11.4805), (44.5535, 11.0839));
    path.conic_to((43.9737, 10.2691), (44.7884, 9.6893), 0.707107);
    path.conic_to((45.6032, 9.10947), (46.183, 9.9242), 0.707107);
    path.close();
    path.move_to((47.8333, 12.5645));
    path.quad_to((48.0821, 13.0214), (48.3125, 13.4879));
    path.conic_to((48.7552, 14.3845), (47.8586, 14.8273), 0.707107);
    path.conic_to((46.962, 15.2701), (46.5192, 14.3734), 0.707107);
    path.quad_to((46.3065, 13.9428), (46.0769, 13.5211));
    path.conic_to((45.5986, 12.6429), (46.4768, 12.1646), 0.707107);
    path.conic_to((47.355, 11.6863), (47.8333, 12.5645), 0.707107);
    path.close();
    path.move_to((49.1641, 15.4033));
    path.quad_to((49.3588, 15.8935), (49.5334, 16.3912));
    path.conic_to((49.8645, 17.3348), (48.9209, 17.6659), 0.707107);
    path.conic_to((47.9773, 17.997), (47.6462, 17.0534), 0.707107);
    path.quad_to((47.485, 16.5939), (47.3053, 16.1415));
    path.conic_to((46.9362, 15.2121), (47.8656, 14.843), 0.707107);
    path.conic_to((48.795, 14.4739), (49.1641, 15.4033), 0.707107);
    path.close();
    path.move_to((50.1526, 18.4161));
    path.quad_to((50.287, 18.9296), (50.4003, 19.4482));
    path.conic_to((50.6139, 20.4252), (49.6369, 20.6387), 0.707107);
    path.conic_to((48.66, 20.8522), (48.4465, 19.8753), 0.707107);
    path.quad_to((48.3419, 19.3966), (48.2178, 18.9225));
    path.conic_to((47.9645, 17.9551), (48.9319, 17.7019), 0.707107);
    path.conic_to((49.8993, 17.4487), (50.1526, 18.4161), 0.707107);
    path.close();
    path.move_to((50.7655, 21.5157));
    path.quad_to((50.8354, 22.033), (50.8846, 22.5528));
    path.conic_to((50.9787, 23.5483), (49.9831, 23.6425), 0.707107);
    path.conic_to((48.9876, 23.7366), (48.8935, 22.741), 0.707107);
    path.quad_to((48.8481, 22.2613), (48.7835, 21.7837));
    path.conic_to((48.6495, 20.7928), (49.6405, 20.6587), 0.707107);
    path.conic_to((50.6315, 20.5247), (50.7655, 21.5157), 0.707107);
    path.close();
    path.move_to((50.9974, 24.6301));
    path.quad_to((51.0048, 25.1509), (50.9913, 25.6715));
    path.conic_to((50.9655, 26.6712), (49.9658, 26.6454), 0.707107);
    path.conic_to((48.9662, 26.6196), (48.992, 25.6199), 0.707107);
    path.quad_to((49.0044, 25.1393), (48.9976, 24.6585));
    path.conic_to((48.9834, 23.6586), (49.9833, 23.6444), 0.707107);
    path.conic_to((50.9832, 23.6302), (50.9974, 24.6301), 0.707107);
    path.close();
    path.move_to((50.8524, 27.7662));
    path.quad_to((50.7971, 28.2837), (50.721, 28.7986));
    path.conic_to((50.5749, 29.7879), (49.5856, 29.6418), 0.707107);
    path.conic_to((48.5963, 29.4957), (48.7425, 28.5064), 0.707107);
    path.quad_to((48.8127, 28.0311), (48.8638, 27.5534));
    path.conic_to((48.9702, 26.5591), (49.9645, 26.6655), 0.707107);
    path.conic_to((50.9588, 26.7718), (50.8524, 27.7662), 0.707107);
    path.close();
    path.move_to((50.3355, 30.8404));
    path.quad_to((50.2125, 31.3739), (50.0672, 31.9018));
    path.conic_to((49.8018, 32.8659), (48.8376, 32.6005), 0.707107);
    path.conic_to((47.8735, 32.335), (48.139, 31.3709), 0.707107);
    path.quad_to((48.2731, 30.8836), (48.3867, 30.3912));
    path.conic_to((48.6113, 29.4167), (49.5857, 29.6413), 0.707107);
    path.conic_to((50.5602, 29.866), (50.3355, 30.8404), 0.707107);
    path.close();
    path.move_to((49.4091, 33.9552));
    path.quad_to((49.2264, 34.4531), (49.0236, 34.9431));
    path.conic_to((48.6412, 35.8671), (47.7172, 35.4846), 0.707107);
    path.conic_to((46.7932, 35.1022), (47.1757, 34.1782), 0.707107);
    path.quad_to((47.3629, 33.7259), (47.5315, 33.2663));
    path.conic_to((47.8759, 32.3275), (48.8147, 32.672), 0.707107);
    path.conic_to((49.7535, 33.0164), (49.4091, 33.9552), 0.707107);
    path.close();
    path.move_to((48.1514, 36.8328));
    path.quad_to((47.9191, 37.2871), (47.6694, 37.7318));
    path.conic_to((47.1797, 38.6038), (46.3078, 38.1141), 0.707107);
    path.conic_to((45.4359, 37.6244), (45.9256, 36.7525), 0.707107);
    path.quad_to((46.1562, 36.3418), (46.3705, 35.9226));
    path.conic_to((46.8256, 35.0321), (47.716, 35.4872), 0.707107);
    path.conic_to((48.6065, 35.9423), (48.1514, 36.8328), 0.707107);
    path.close();
    path.move_to((46.6245, 39.4354));
    path.line_to((46.5563, 39.537));
    path.quad_to((46.3146, 39.8955), (46.0624, 40.2438));
    path.conic_to((45.4761, 41.0539), (44.666, 40.4676), 0.707107);
    path.conic_to((43.8559, 39.8813), (44.4422, 39.0712), 0.707107);
    path.quad_to((44.6749, 38.7498), (44.8955, 38.4226));
    path.line_to((44.9637, 38.3211));
    path.conic_to((45.5209, 37.4907), (46.3513, 38.0479), 0.707107);
    path.conic_to((47.1817, 38.605), (46.6245, 39.4354), 0.707107);
    path.close();
    path.move_to((44.8168, 41.8314));
    path.quad_to((44.4832, 42.2241), (44.1342, 42.6034));
    path.conic_to((43.4572, 43.3394), (42.7212, 42.6623), 0.707107);
    path.conic_to((41.9853, 41.9853), (42.6623, 41.2494), 0.707107);
    path.quad_to((42.9845, 40.8992), (43.2924, 40.5366));
    path.conic_to((43.9398, 39.7745), (44.702, 40.4218), 0.707107);
    path.conic_to((45.4642, 41.0692), (44.8168, 41.8314), 0.707107);
    path.close();
    path.move_to((42.6505, 44.0908));
    path.quad_to((42.2577, 44.454), (41.8504, 44.8006));
    path.conic_to((41.0888, 45.4487), (40.4408, 44.6871), 0.707107);
    path.conic_to((39.7927, 43.9256), (40.5542, 43.2775), 0.707107);
    path.quad_to((40.9302, 42.9575), (41.2928, 42.6223));
    path.conic_to((42.027, 41.9434), (42.7059, 42.6777), 0.707107);
    path.conic_to((43.3848, 43.412), (42.6505, 44.0908), 0.707107);
    path.close();
    path.move_to((40.1383, 46.1384));
    path.quad_to((39.7073, 46.4471), (39.2641, 46.7378));
    path.conic_to((38.4281, 47.2865), (37.8795, 46.4504), 0.707107);
    path.conic_to((37.3308, 45.6143), (38.1669, 45.0657), 0.707107);
    path.quad_to((38.576, 44.7972), (38.9738, 44.5124));
    path.conic_to((39.7868, 43.9301), (40.369, 44.7432), 0.707107);
    path.conic_to((40.9513, 45.5562), (40.1383, 46.1384), 0.707107);
    path.close();
    path.move_to((37.4991, 47.7985));
    path.quad_to((37.0431, 48.0485), (36.5775, 48.2801));
    path.conic_to((35.6821, 48.7254), (35.2368, 47.83), 0.707107);
    path.conic_to((34.7915, 46.9346), (35.6869, 46.4893), 0.707107);
    path.quad_to((36.1167, 46.2755), (36.5376, 46.0448));
    path.conic_to((37.4145, 45.5641), (37.8952, 46.4409), 0.707107);
    path.conic_to((38.376, 47.3178), (37.4991, 47.7985), 0.707107);
    path.close();
    path.move_to((34.6651, 49.1368));
    path.quad_to((34.1756, 49.3328), (33.6785, 49.5089));
    path.conic_to((32.7358, 49.8427), (32.402, 48.9), 0.707107);
    path.conic_to((32.0682, 47.9574), (33.0109, 47.6236), 0.707107);
    path.quad_to((33.4697, 47.4611), (33.9216, 47.2801));
    path.conic_to((34.85, 46.9084), (35.2217, 47.8368), 0.707107);
    path.conic_to((35.5934, 48.7651), (34.6651, 49.1368), 0.707107);
    path.close();
    path.move_to((31.6557, 50.1337));
    path.quad_to((31.1425, 50.2696), (30.6243, 50.3844));
    path.conic_to((29.648, 50.6007), (29.4317, 49.6244), 0.707107);
    path.conic_to((29.2153, 48.6481), (30.1917, 48.4317), 0.707107);
    path.quad_to((30.6701, 48.3257), (31.1437, 48.2003));
    path.conic_to((32.1104, 47.9443), (32.3664, 48.911), 0.707107);
    path.conic_to((32.6223, 49.8777), (31.6557, 50.1337), 0.707107);
    path.close();
    path.move_to((28.5567, 50.7556));
    path.quad_to((28.0395, 50.827), (27.5198, 50.8776));
    path.conic_to((26.5245, 50.9745), (26.4276, 49.9792), 0.707107);
    path.conic_to((26.3307, 48.9839), (27.326, 48.887), 0.707107);
    path.quad_to((27.8056, 48.8403), (28.2831, 48.7744));
    path.conic_to((29.2737, 48.6376), (29.4105, 49.6282), 0.707107);
    path.conic_to((29.5473, 50.6188), (28.5567, 50.7556), 0.707107);
    path.close();
    path.move_to((25.4424, 50.9962));
    path.quad_to((24.9222, 51.0051), (24.4022, 50.9931));
    path.conic_to((23.4025, 50.9701), (23.4255, 49.9704), 0.707107);
    path.conic_to((23.4485, 48.9707), (24.4482, 48.9937), 0.707107);
    path.quad_to((24.9283, 49.0047), (25.4084, 48.9965));
    path.conic_to((26.4083, 48.9795), (26.4253, 49.9794), 0.707107);
    path.conic_to((26.4423, 50.9792), (25.4424, 50.9962), 0.707107);
    path.close();
    path.move_to((22.3065, 50.8601));
    path.quad_to((21.7885, 50.8062), (21.2732, 50.7315));
    path.conic_to((20.2835, 50.5882), (20.4268, 49.5985), 0.707107);
    path.conic_to((20.5702, 48.6088), (21.5599, 48.7522), 0.707107);
    path.quad_to((22.0355, 48.8211), (22.5136, 48.8709));
    path.conic_to((23.5083, 48.9745), (23.4047, 49.9691), 0.707107);
    path.conic_to((23.3011, 50.9637), (22.3065, 50.8601), 0.707107);
    path.close();
    path.move_to((19.2346, 50.3527));
    path.quad_to((18.7003, 50.2312), (18.1717, 50.0873));
    path.conic_to((17.2068, 49.8247), (17.4694, 48.8598), 0.707107);
    path.conic_to((17.732, 47.8949), (18.6969, 48.1575), 0.707107);
    path.quad_to((19.185, 48.2904), (19.6781, 48.4025));
    path.conic_to((20.6532, 48.6243), (20.4314, 49.5994), 0.707107);
    path.conic_to((20.2097, 50.5745), (19.2346, 50.3527), 0.707107);
    path.close();
    path.move_to((16.1149, 49.4347));
    path.quad_to((15.6161, 49.2533), (15.1251, 49.0517));
    path.conic_to((14.2, 48.6719), (14.5798, 47.7469), 0.707107);
    path.conic_to((14.9596, 46.8218), (15.8847, 47.2016), 0.707107);
    path.quad_to((16.3379, 47.3877), (16.7984, 47.5551));
    path.conic_to((17.7382, 47.8969), (17.3964, 48.8366), 0.707107);
    path.conic_to((17.0547, 49.7764), (16.1149, 49.4347), 0.707107);
    path.close();
    path.move_to((13.2313, 48.184));
    path.quad_to((12.776, 47.9529), (12.33, 47.704));
    path.conic_to((11.4568, 47.2167), (11.9441, 46.3434), 0.707107);
    path.conic_to((12.4314, 45.4702), (13.3046, 45.9575), 0.707107);
    path.quad_to((13.7162, 46.1872), (14.1365, 46.4006));
    path.conic_to((15.0282, 46.8532), (14.5756, 47.7449), 0.707107);
    path.conic_to((14.123, 48.6366), (13.2313, 48.184), 0.707107);
    path.close();
    path.move_to((10.6208, 46.6619));
    path.line_to((10.4641, 46.5571));
    path.quad_to((10.1333, 46.334), (9.81253, 46.1031));
    path.conic_to((9.00087, 45.519), (9.585, 44.7073), 0.707107);
    path.conic_to((10.1691, 43.8957), (10.9808, 44.4798), 0.707107);
    path.quad_to((11.2769, 44.6929), (11.5763, 44.8948));
    path.line_to((11.7329, 44.9996));
    path.conic_to((12.564, 45.5557), (12.008, 46.3868), 0.707107);
    path.conic_to((11.4519, 47.2179), (10.6208, 46.6619), 0.707107);
    path.close();
    path.move_to((8.22326, 44.8631));
    path.quad_to((7.82986, 44.5308), (7.44999, 44.1833));
    path.conic_to((6.71217, 43.5082), (7.38718, 42.7704), 0.707107);
    path.conic_to((8.06219, 42.0326), (8.8, 42.7076), 0.707107);
    path.quad_to((9.15066, 43.0284), (9.51375, 43.3351));
    path.conic_to((10.2777, 43.9804), (9.63248, 44.7443), 0.707107);
    path.conic_to((8.98724, 45.5083), (8.22326, 44.8631), 0.707107);
    path.close();
    path.move_to((5.95972, 42.705));
    path.quad_to((5.59577, 42.3136), (5.24823, 41.9076));
    path.conic_to((4.59793, 41.148), (5.3576, 40.4977), 0.707107);
    path.conic_to((6.11728, 39.8473), (6.76758, 40.607), 0.707107);
    path.quad_to((7.08843, 40.9818), (7.42436, 41.3431));
    path.conic_to((8.10532, 42.0754), (7.373, 42.7564), 0.707107);
    path.conic_to((6.64068, 43.4373), (5.95972, 42.705), 0.707107);
    path.close();
    path.move_to((3.90635, 40.2006));
    path.quad_to((3.59492, 39.7684), (3.30147, 39.3239));
    path.conic_to((2.75055, 38.4893), (3.58511, 37.9384), 0.707107);
    path.conic_to((4.41967, 37.3875), (4.97059, 38.222), 0.707107);
    path.quad_to((5.24148, 38.6324), (5.52894, 39.0313));
    path.conic_to((6.11358, 39.8426), (5.30228, 40.4272), 0.707107);
    path.conic_to((4.49099, 41.0119), (3.90635, 40.2006), 0.707107);
    path.close();
    path.move_to((2.23643, 37.5626));
    path.quad_to((1.98525, 37.1075), (1.75248, 36.6427));
    path.conic_to((1.30469, 35.7486), (2.19883, 35.3008), 0.707107);
    path.conic_to((3.09296, 34.853), (3.54076, 35.7471), 0.707107);
    path.quad_to((3.75563, 36.1762), (3.98747, 36.5963));
    path.conic_to((4.47065, 37.4718), (3.59513, 37.955), 0.707107);
    path.conic_to((2.71961, 38.4382), (2.23643, 37.5626), 0.707107);
    path.close();
    path.move_to((0.890647, 34.7334));
    path.quad_to((0.69328, 34.2445), (0.515902, 33.7481));
    path.conic_to((0.179435, 32.8064), (1.12113, 32.4699), 0.707107);
    path.conic_to((2.06282, 32.1335), (2.39929, 33.0752), 0.707107);
    path.quad_to((2.56303, 33.5334), (2.74521, 33.9847));
    path.conic_to((3.11957, 34.912), (2.19229, 35.2863), 0.707107);
    path.conic_to((1.26501, 35.6607), (0.890647, 34.7334), 0.707107);
    path.close();
    path.move_to((-0.114587, 31.7274));
    path.quad_to((-0.251922, 31.2147), (-0.368218, 30.6968));
    path.conic_to((-0.587327, 29.7211), (0.388373, 29.502), 0.707107);
    path.conic_to((1.36407, 29.2829), (1.58318, 30.2586), 0.707107);
    path.quad_to((1.69053, 30.7366), (1.8173, 31.2099));
    path.conic_to((2.07605, 32.1758), (1.1101, 32.4346), 0.707107);
    path.conic_to((0.144159, 32.6933), (-0.114587, 31.7274), 0.707107);
    path.close();
    path.move_to((-0.745485, 28.6291));
    path.quad_to((-0.818367, 28.112), (-0.870432, 27.5925));
    path.conic_to((-0.970142, 26.5974), (0.0248742, 26.4977), 0.707107);
    path.conic_to((1.01989, 26.398), (1.1196, 27.393), 0.707107);
    path.quad_to((1.16766, 27.8726), (1.23494, 28.3499));
    path.conic_to((1.37452, 29.3401), (0.384305, 29.4797), 0.707107);
    path.conic_to((-0.605905, 29.6193), (-0.745485, 28.6291), 0.707107);
    path.close();
    path.move_to((-0.994901, 25.515));
    path.quad_to((-1.00519, 24.9955), (-0.994722, 24.4761));
    path.conic_to((-0.97457, 23.4763), (0.0252273, 23.4964), 0.707107);
    path.conic_to((1.02502, 23.5166), (1.00487, 24.5164), 0.707107);
    path.quad_to((0.995207, 24.9959), (1.00471, 25.4754));
    path.conic_to((1.02451, 26.4752), (0.0247103, 26.495), 0.707107);
    path.conic_to((-0.975093, 26.5148), (-0.994901, 25.515), 0.707107);
    path.close();
    path.move_to((-0.867571, 22.3792));
    path.quad_to((-0.81506, 21.8609), (-0.741825, 21.3451));
    path.conic_to((-0.60125, 20.355), (0.38882, 20.4956), 0.707107);
    path.conic_to((1.37889, 20.6361), (1.23831, 21.6262), 0.707107);
    path.quad_to((1.17071, 22.1023), (1.12224, 22.5807));
    path.conic_to((1.02144, 23.5757), (0.026537, 23.4749), 0.707107);
    path.conic_to((-0.96837, 23.3741), (-0.867571, 22.3792), 0.707107);
    path.close();
    path.move_to((-0.369678, 19.3097));
    path.quad_to((-0.249693, 18.7748), (-0.107265, 18.2453));
    path.conic_to((0.152529, 17.2797), (1.11819, 17.5395), 0.707107);
    path.conic_to((2.08386, 17.7993), (1.82406, 18.7649), 0.707107);
    path.quad_to((1.69259, 19.2536), (1.58184, 19.7474));
    path.conic_to((1.36298, 20.7232), (0.387221, 20.5043), 0.707107);
    path.conic_to((-0.588536, 20.2855), (-0.369678, 19.3097), 0.707107);
    path.close();
    path.move_to((0.539863, 16.1851));
    path.quad_to((0.719962, 15.6854), (0.920307, 15.1934));
    path.conic_to((1.29748, 14.2673), (2.22362, 14.6445), 0.707107);
    path.conic_to((3.14976, 15.0216), (2.7726, 15.9478), 0.707107);
    path.quad_to((2.58765, 16.4019), (2.42141, 16.8632));
    path.conic_to((2.08237, 17.804), (1.1416, 17.4649), 0.707107);
    path.conic_to((0.200823, 17.1259), (0.539863, 16.1851), 0.707107);
    path.close();
    path.move_to((1.78353, 13.2955));
    path.quad_to((2.01364, 12.8391), (2.26151, 12.392));
    path.conic_to((2.74643, 11.5175), (3.62099, 12.0024), 0.707107);
    path.conic_to((4.49555, 12.4873), (4.01063, 13.3618), 0.707107);
    path.quad_to((3.78183, 13.7745), (3.56941, 14.1958));
    path.conic_to((3.11923, 15.0888), (2.22629, 14.6386), 0.707107);
    path.conic_to((1.33336, 14.1884), (1.78353, 13.2955), 0.707107);
    path.close();
    path.move_to((3.30083, 10.6771));
    path.line_to((3.44218, 10.4652));
    path.quad_to((3.6466, 10.1621), (3.85641, 9.86895));
    path.conic_to((4.43837, 9.05574), (5.25159, 9.6377), 0.707107);
    path.conic_to((6.0648, 10.2197), (5.48284, 11.0329), 0.707107);
    path.quad_to((5.28917, 11.3035), (5.10592, 11.5752));
    path.line_to((4.96457, 11.787));
    path.conic_to((4.4096, 12.6189), (3.57773, 12.0639), 0.707107);
    path.conic_to((2.74586, 11.509), (3.30083, 10.6771), 0.707107);
    path.close();
    path.move_to((5.0909, 8.27793));
    path.quad_to((5.42174, 7.88403), (5.76791, 7.50353));
    path.conic_to((6.44085, 6.76383), (7.18054, 7.43678), 0.707107);
    path.conic_to((7.92024, 8.10972), (7.24729, 8.84942), 0.707107);
    path.quad_to((6.92775, 9.20065), (6.62237, 9.56424));
    path.conic_to((5.97921, 10.33), (5.21348, 9.68682), 0.707107);
    path.conic_to((4.44774, 9.04367), (5.0909, 8.27793), 0.707107);
    path.close();
    path.move_to((7.24064, 6.0104));
    path.quad_to((7.63069, 5.64561), (8.03537, 5.29717));
    path.conic_to((8.79318, 4.64469), (9.44566, 5.40249), 0.707107);
    path.conic_to((10.0981, 6.16029), (9.34034, 6.81278), 0.707107);
    path.quad_to((8.96678, 7.13442), (8.60675, 7.47113));
    path.conic_to((7.87638, 8.15419), (7.19332, 7.42382), 0.707107);
    path.conic_to((6.51027, 6.69345), (7.24064, 6.0104), 0.707107);
    path.close();
    path.move_to((9.73726, 3.95128));
    path.quad_to((10.1706, 3.63704), (10.6165, 3.34092));
    path.conic_to((11.4496, 2.78771), (12.0028, 3.62075), 0.707107);
    path.conic_to((12.556, 4.4538), (11.7229, 5.007), 0.707107);
    path.quad_to((11.3113, 5.28035), (10.9113, 5.57041));
    path.conic_to((10.1018, 6.15744), (9.51472, 5.34787), 0.707107);
    path.conic_to((8.92769, 4.53831), (9.73726, 3.95128), 0.707107);
    path.close();
    path.move_to((12.374, 2.27153));
    path.quad_to((12.8282, 2.01921), (13.2921, 1.78522));
    path.conic_to((14.185, 1.33492), (14.6353, 2.22779), 0.707107);
    path.conic_to((15.0856, 3.12067), (14.1927, 3.57097), 0.707107);
    path.quad_to((13.7645, 3.78696), (13.3452, 4.01988));
    path.conic_to((12.471, 4.5055), (11.9854, 3.63132), 0.707107);
    path.conic_to((11.4998, 2.75715), (12.374, 2.27153), 0.707107);
    path.close();
    path.move_to((15.1984, 0.918296));
    path.quad_to((15.6866, 0.719602), (16.1824, 0.540851));
    path.conic_to((17.1231, 0.20171), (17.4623, 1.14245), 0.707107);
    path.conic_to((17.8014, 2.08318), (16.8607, 2.42232), 0.707107);
    path.quad_to((16.403, 2.58733), (15.9524, 2.77074));
    path.conic_to((15.0261, 3.14772), (14.6492, 2.2215), 0.707107);
    path.conic_to((14.2722, 1.29528), (15.1984, 0.918296), 0.707107);
    path.close();
    path.move_to((18.201, -0.0952874));
    path.quad_to((18.7132, -0.234075), (19.2308, -0.351842));
    path.conic_to((20.2058, -0.573734), (20.4277, 0.401338), 0.707107);
    path.conic_to((20.6496, 1.37641), (19.6745, 1.5983), 0.707107);
    path.quad_to((19.1968, 1.70701), (18.724, 1.83512));
    path.conic_to((17.7588, 2.09662), (17.4973, 1.13142), 0.707107);
    path.conic_to((17.2358, 0.166216), (18.201, -0.0952874), 0.707107);
    path.close();
    path.move_to((21.2986, -0.73518));
    path.quad_to((21.8155, -0.809526), (22.3349, -0.863052));
    path.conic_to((23.3297, -0.965552), (23.4322, 0.029181), 0.707107);
    path.conic_to((23.5347, 1.02391), (22.5399, 1.12641), 0.707107);
    path.quad_to((22.0604, 1.17582), (21.5833, 1.24445));
    path.conic_to((20.5935, 1.38681), (20.4511, 0.397), 0.707107);
    path.conic_to((20.3088, -0.592814), (21.2986, -0.73518), 0.707107);
    path.close();
    path.move_to((24.4124, -0.993361));
    path.quad_to((24.9312, -1.00509), (25.4501, -0.996107));
    path.conic_to((26.4499, -0.978799), (26.4326, 0.0210512), 0.707107);
    path.conic_to((26.4153, 1.0209), (25.4155, 1.00359), 0.707107);
    path.quad_to((24.9365, 0.995302), (24.4576, 1.00613));
    path.conic_to((23.4578, 1.02873), (23.4352, 0.0289853), 0.707107);
    path.conic_to((23.4126, -0.970759), (24.4124, -0.993361), 0.707107);
    path.close();
    path.move_to((27.5481, -0.87484));
    path.quad_to((28.0668, -0.823762), (28.583, -0.75194));
    path.conic_to((29.5734, -0.614138), (29.4356, 0.376322), 0.707107);
    path.conic_to((29.2978, 1.36678), (28.3074, 1.22898), 0.707107);
    path.quad_to((27.8309, 1.16268), (27.3521, 1.11553));
    path.conic_to((26.3569, 1.01753), (26.4549, 0.0223428), 0.707107);
    path.conic_to((26.5529, -0.972843), (27.5481, -0.87484), 0.707107);
    path.close();
    path.move_to((30.6151, -0.386432));
    path.quad_to((31.1507, -0.267954), (31.6809, -0.126991));
    path.conic_to((32.6473, 0.129965), (32.3904, 1.09639), 0.707107);
    path.conic_to((32.1334, 2.06281), (31.167, 1.80585), 0.707107);
    path.quad_to((30.6776, 1.67574), (30.1832, 1.56637));
    path.conic_to((29.2068, 1.35041), (29.4227, 0.374005), 0.707107);
    path.conic_to((29.6387, -0.602396), (30.6151, -0.386432), 0.707107);
    path.close();
    path.move_to((33.7445, 0.514616));
    path.quad_to((34.2452, 0.693421), (34.7381, 0.892536));
    path.conic_to((35.6653, 1.26708), (35.2908, 2.19429), 0.707107);
    path.conic_to((34.9162, 3.1215), (33.989, 2.74696), 0.707107);
    path.quad_to((33.534, 2.56316), (33.0718, 2.3981));
    path.conic_to((32.1301, 2.06177), (32.4664, 1.12003), 0.707107);
    path.conic_to((32.8027, 0.178285), (33.7445, 0.514616), 0.707107);
    path.close();
    path.move_to((36.6402, 1.7512));
    path.quad_to((37.0977, 1.98026), (37.5458, 2.22715));
    path.conic_to((38.4217, 2.70968), (37.9392, 3.58556), 0.707107);
    path.conic_to((37.4566, 4.46144), (36.5808, 3.97891), 0.707107);
    path.quad_to((36.1671, 3.75102), (35.7448, 3.53956));
    path.conic_to((34.8506, 3.09185), (35.2983, 2.19767), 0.707107);
    path.conic_to((35.746, 1.30349), (36.6402, 1.7512), 0.707107);
    path.close();
    path.move_to((39.2611, 3.26012));
    path.quad_to((39.4005, 3.35159), (39.539, 3.44501));
    path.quad_to((39.8091, 3.62717), (40.0746, 3.81611));
    path.conic_to((40.8893, 4.3959), (40.3096, 5.21067), 0.707107);
    path.conic_to((39.7298, 6.02543), (38.915, 5.44564), 0.707107);
    path.quad_to((38.67, 5.2713), (38.4206, 5.10309));
    path.quad_to((38.293, 5.017), (38.164, 4.9324));
    path.conic_to((37.3279, 4.38388), (37.8764, 3.54775), 0.707107);
    path.conic_to((38.4249, 2.71161), (39.2611, 3.26012), 0.707107);
    path.close();
    path.move_to((41.6673, 5.04503));
    path.quad_to((42.0618, 5.37449), (42.4428, 5.71927));
    path.conic_to((43.1844, 6.39015), (42.5135, 7.13171), 0.707107);
    path.conic_to((41.8426, 7.87327), (41.1011, 7.20239), 0.707107);
    path.quad_to((40.7493, 6.88414), (40.3852, 6.58004));
    path.conic_to((39.6177, 5.93899), (40.2588, 5.17149), 0.707107);
    path.conic_to((40.8998, 4.40399), (41.6673, 5.04503), 0.707107);
    path.close();
    path.move_to((43.9388, 7.1865));
    path.quad_to((44.3044, 7.57519), (44.6538, 7.97856));
    path.conic_to((45.3084, 8.73448), (44.5525, 9.38914), 0.707107);
    path.conic_to((43.7966, 10.0438), (43.1419, 9.28789), 0.707107);
    path.quad_to((42.8195, 8.91555), (42.482, 8.55677));
    path.conic_to((41.7969, 7.82836), (42.5253, 7.14322), 0.707107);
    path.conic_to((43.2537, 6.45808), (43.9388, 7.1865), 0.707107);
    path.close();
    path.move_to((46.0036, 9.6753));
    path.quad_to((46.3207, 10.1098), (46.6195, 10.5571));
    path.conic_to((47.175, 11.3886), (46.3435, 11.9441), 0.707107);
    path.conic_to((45.5119, 12.4996), (44.9564, 11.6681), 0.707107);
    path.quad_to((44.6806, 11.2552), (44.388, 10.8541));
    path.conic_to((43.7986, 10.0463), (44.6064, 9.45688), 0.707107);
    path.conic_to((45.4142, 8.86747), (46.0036, 9.6753), 0.707107);
    path.close();
    path.move_to((47.6932, 12.3107));
    path.quad_to((47.9467, 12.764), (48.1819, 13.2271));
    path.conic_to((48.6347, 14.1187), (47.7431, 14.5715), 0.707107);
    path.conic_to((46.8514, 15.0243), (46.3986, 14.1327), 0.707107);
    path.quad_to((46.1816, 13.7053), (45.9476, 13.2868));
    path.conic_to((45.4595, 12.414), (46.3323, 11.9259), 0.707107);
    path.conic_to((47.2051, 11.4379), (47.6932, 12.3107), 0.707107);
    path.close();
    path.move_to((49.0539, 15.1303));
    path.quad_to((49.2539, 15.6178), (49.434, 16.113));
    path.conic_to((49.7758, 17.0527), (48.836, 17.3946), 0.707107);
    path.conic_to((47.8963, 17.7364), (47.5545, 16.7966), 0.707107);
    path.quad_to((47.3882, 16.3395), (47.2036, 15.8895));
    path.conic_to((46.824, 14.9643), (47.7491, 14.5847), 0.707107);
    path.conic_to((48.6743, 14.2051), (49.0539, 15.1303), 0.707107);
    path.close();
    path.move_to((50.0758, 18.1294));
    path.quad_to((50.216, 18.6412), (50.3352, 19.1584));
    path.conic_to((50.5599, 20.1328), (49.5855, 20.3575), 0.707107);
    path.conic_to((48.6111, 20.5821), (48.3864, 19.6077), 0.707107);
    path.quad_to((48.2763, 19.1304), (48.1469, 18.6579));
    path.conic_to((47.8826, 17.6935), (48.8471, 17.4292), 0.707107);
    path.conic_to((49.8115, 17.165), (50.0758, 18.1294), 0.707107);
    path.close();
    path.move_to((50.7247, 21.2262));
    path.quad_to((50.8005, 21.743), (50.8555, 22.2623));
    path.conic_to((50.9607, 23.2568), (49.9663, 23.3621), 0.707107);
    path.conic_to((48.9719, 23.4673), (48.8666, 22.4729), 0.707107);
    path.quad_to((48.8158, 21.9935), (48.7458, 21.5165));
    path.conic_to((48.6007, 20.5271), (49.5901, 20.382), 0.707107);
    path.conic_to((50.5795, 20.2368), (50.7247, 21.2262), 0.707107);
    path.close();
    path.move_to((50.9916, 24.3398));
    path.quad_to((51.0048, 24.858), (50.9973, 25.3762));
    path.conic_to((50.9828, 26.3761), (49.9829, 26.3616), 0.707107);
    path.conic_to((48.983, 26.3472), (48.9975, 25.3473), 0.707107);
    path.quad_to((49.0044, 24.8687), (48.9923, 24.3906));
    path.conic_to((48.9669, 23.3909), (49.9665, 23.3655), 0.707107);
    path.conic_to((50.9662, 23.3401), (50.9916, 24.3398), 0.707107);
    path.close();
    path.move_to((50.8819, 27.4753));
    path.quad_to((50.8323, 27.9943), (50.7618, 28.511));
    path.conic_to((50.6268, 29.5018), (49.636, 29.3668), 0.707107);
    path.conic_to((48.6451, 29.2317), (48.7802, 28.2409), 0.707107);
    path.quad_to((48.8452, 27.7641), (48.891, 27.2849));
    path.conic_to((48.9862, 26.2894), (49.9816, 26.3846), 0.707107);
    path.conic_to((50.9771, 26.4798), (50.8819, 27.4753), 0.707107);
    path.close();
    path.move_to((50.4023, 30.5429));
    path.quad_to((50.2856, 31.0775), (50.1465, 31.607));
    path.conic_to((49.8924, 32.5742), (48.9252, 32.3201), 0.707107);
    path.conic_to((47.9581, 32.066), (48.2122, 31.0988), 0.707107);
    path.quad_to((48.3405, 30.6102), (48.4483, 30.1165));
    path.conic_to((48.6614, 29.1395), (49.6385, 29.3527), 0.707107);
    path.conic_to((50.6155, 29.5659), (50.4023, 30.5429), 0.707107);
    path.close();
    path.move_to((49.5104, 33.674));
    path.quad_to((49.3329, 34.1756), (49.1351, 34.6695));
    path.conic_to((48.7632, 35.5977), (47.8349, 35.2258), 0.707107);
    path.conic_to((46.9066, 34.854), (47.2785, 33.9257), 0.707107);
    path.quad_to((47.4612, 33.4697), (47.625, 33.0067));
    path.conic_to((47.9587, 32.064), (48.9014, 32.3977), 0.707107);
    path.conic_to((49.8441, 32.7313), (49.5104, 33.674), 0.707107);
    path.close();
    path.move_to((48.281, 36.5756));
    path.quad_to((48.053, 37.0342), (47.8071, 37.4835));
    path.conic_to((47.3269, 38.3607), (46.4497, 37.8805), 0.707107);
    path.conic_to((45.5725, 37.4004), (46.0527, 36.5232), 0.707107);
    path.quad_to((46.2797, 36.1085), (46.4901, 35.6852));
    path.conic_to((46.9353, 34.7898), (47.8307, 35.235), 0.707107);
    path.conic_to((48.7262, 35.6802), (48.281, 36.5756), 0.707107);
    path.close();
    path.move_to((46.7777, 39.2033));
    path.quad_to((46.6677, 39.3719), (46.555, 39.539));
    path.quad_to((46.3865, 39.7888), (46.2121, 40.0349));
    path.conic_to((45.6338, 40.8507), (44.818, 40.2724), 0.707107);
    path.conic_to((44.0021, 39.6942), (44.5804, 38.8783), 0.707107);
    path.quad_to((44.7413, 38.6513), (44.8969, 38.4206));
    path.quad_to((45.0008, 38.2665), (45.1025, 38.1107));
    path.conic_to((45.6488, 37.2731), (46.4864, 37.8194), 0.707107);
    path.conic_to((47.324, 38.3657), (46.7777, 39.2033), 0.707107);
    path.close();
    path.move_to((44.9527, 41.6701));
    path.quad_to((44.6177, 42.0709), (44.267, 42.458));
    path.conic_to((43.5955, 43.1991), (42.8545, 42.5276), 0.707107);
    path.conic_to((42.1135, 41.8561), (42.7849, 41.1151), 0.707107);
    path.quad_to((43.1087, 40.7578), (43.4178, 40.3878));
    path.conic_to((44.059, 39.6203), (44.8264, 40.2615), 0.707107);
    path.conic_to((45.5938, 40.9027), (44.9527, 41.6701), 0.707107);
    path.close();
    path.move_to((42.7884, 43.9624));
    path.quad_to((42.4083, 44.319), (42.014, 44.6602));
    path.conic_to((41.2578, 45.3146), (40.6034, 44.5585), 0.707107);
    path.conic_to((39.949, 43.8023), (40.7052, 43.1479), 0.707107);
    path.quad_to((41.0691, 42.833), (41.4201, 42.5037));
    path.conic_to((42.1494, 41.8196), (42.8336, 42.5489), 0.707107);
    path.conic_to((43.5178, 43.2782), (42.7884, 43.9624), 0.707107);
    path.close();
    path.move_to((40.3892, 45.9564));
    path.quad_to((39.9683, 46.2655), (39.5354, 46.5574));
    path.conic_to((38.7062, 47.1165), (38.1472, 46.2873), 0.707107);
    path.conic_to((37.5881, 45.4582), (38.4173, 44.8992), 0.707107);
    path.quad_to((38.8169, 44.6297), (39.2054, 44.3444));
    path.conic_to((40.0114, 43.7525), (40.6033, 44.5585), 0.707107);
    path.conic_to((41.1952, 45.3645), (40.3892, 45.9564), 0.707107);
    path.close();
    path.move_to((37.7543, 47.6568));
    path.quad_to((37.2977, 47.9138), (36.8312, 48.1522));
    path.conic_to((35.9407, 48.6072), (35.4857, 47.7167), 0.707107);
    path.conic_to((35.0306, 46.8263), (35.9211, 46.3712), 0.707107);
    path.quad_to((36.3518, 46.1511), (36.7732, 45.9139));
    path.conic_to((37.6446, 45.4234), (38.1351, 46.2948), 0.707107);
    path.conic_to((38.6257, 47.1662), (37.7543, 47.6568), 0.707107);
    path.close();
    path.move_to((34.9311, 49.0286));
    path.quad_to((34.4488, 49.2279), (33.9589, 49.4077));
    path.conic_to((33.0202, 49.7523), (32.6756, 48.8136), 0.707107);
    path.conic_to((32.331, 47.8748), (33.2698, 47.5302), 0.707107);
    path.quad_to((33.722, 47.3642), (34.1672, 47.1802));
    path.conic_to((35.0914, 46.7983), (35.4733, 47.7224), 0.707107);
    path.conic_to((35.8553, 48.6466), (34.9311, 49.0286), 0.707107);
    path.close();
    path.move_to((31.9824, 50.0449));
    path.quad_to((31.4774, 50.1857), (30.9668, 50.3061));
    path.conic_to((29.9935, 50.5355), (29.764, 49.5622), 0.707107);
    path.conic_to((29.5346, 48.5889), (30.5079, 48.3594), 0.707107);
    path.quad_to((30.9789, 48.2484), (31.4453, 48.1184));
    path.conic_to((32.4086, 47.8498), (32.6771, 48.8131), 0.707107);
    path.conic_to((32.9457, 49.7763), (31.9824, 50.0449), 0.707107);
    path.close();
    path.move_to((28.899, 50.706));
    path.quad_to((28.3834, 50.7842), (27.8652, 50.8416));
    path.conic_to((26.8713, 50.9518), (26.7611, 49.9579), 0.707107);
    path.conic_to((26.6509, 48.964), (27.6448, 48.8538), 0.707107);
    path.quad_to((28.1231, 48.8008), (28.599, 48.7286));
    path.conic_to((29.5877, 48.5786), (29.7377, 49.5673), 0.707107);
    path.conic_to((29.8877, 50.556), (28.899, 50.706), 0.707107);
    path.close();
    path.move_to((25.8106, 50.9874));
    path.quad_to((25.6321, 50.9929), (25.4537, 50.996));
    path.conic_to((24.4539, 51.0135), (24.4365, 50.0136), 0.707115);
    path.line_to((24.4251, 49.3638));
    path.conic_to((24.4077, 48.364), (25.4075, 48.3465), 0.707107);
    path.conic_to((26.4073, 48.3291), (26.4248, 49.3289), 0.707107);
    path.line_to((26.4361, 49.9787));
    path.line_to((25.4363, 49.9962));
    path.line_to((25.4189, 48.9963));
    path.quad_to((25.5836, 48.9935), (25.7482, 48.9883));
    path.conic_to((26.7477, 48.9571), (26.7789, 49.9567), 0.707107);
    path.conic_to((26.8101, 50.9562), (25.8106, 50.9874), 0.707107);
    path.close();
    path.move_to((24.3902, 47.3641));
    path.line_to((24.3728, 46.3643));
    path.conic_to((24.3553, 45.3645), (25.3551, 45.347), 0.707107);
    path.conic_to((26.355, 45.3295), (26.3724, 46.3294), 0.707107);
    path.line_to((26.3899, 47.3292));
    path.conic_to((26.4074, 48.3291), (25.4075, 48.3465), 0.707107);
    path.conic_to((24.4077, 48.364), (24.3902, 47.3641), 0.707107);
    path.close();
    path.move_to((24.3378, 44.3646));
    path.line_to((24.3204, 43.3648));
    path.conic_to((24.3029, 42.3649), (25.3028, 42.3475), 0.707107);
    path.conic_to((26.3026, 42.33), (26.3201, 43.3298), 0.707107);
    path.line_to((26.3375, 44.3297));
    path.conic_to((26.355, 45.3295), (25.3551, 45.347), 0.707107);
    path.conic_to((24.3553, 45.3645), (24.3378, 44.3646), 0.707107);
    path.close();
    path.move_to((24.2855, 41.3651));
    path.line_to((24.268, 40.3652));
    path.conic_to((24.2506, 39.3654), (25.2504, 39.3479), 0.707107);
    path.conic_to((26.2503, 39.3305), (26.2677, 40.3303), 0.707107);
    path.line_to((26.2852, 41.3302));
    path.conic_to((26.3026, 42.33), (25.3028, 42.3475), 0.707107);
    path.conic_to((24.3029, 42.3649), (24.2855, 41.3651), 0.707107);
    path.close();
    path.move_to((24.2331, 38.3655));
    path.line_to((24.2157, 37.3657));
    path.conic_to((24.1982, 36.3658), (25.1981, 36.3484), 0.707107);
    path.conic_to((26.1979, 36.3309), (26.2154, 37.3308), 0.707107);
    path.line_to((26.2328, 38.3306));
    path.conic_to((26.2503, 39.3305), (25.2504, 39.3479), 0.707107);
    path.conic_to((24.2506, 39.3654), (24.2331, 38.3655), 0.707107);
    path.close();
    path.move_to((24.1808, 35.366));
    path.line_to((24.1633, 34.3661));
    path.conic_to((24.1459, 33.3663), (25.1457, 33.3488), 0.707107);
    path.conic_to((26.1456, 33.3314), (26.163, 34.3312), 0.707107);
    path.line_to((26.1805, 35.3311));
    path.conic_to((26.1979, 36.3309), (25.1981, 36.3484), 0.707107);
    path.conic_to((24.1982, 36.3658), (24.1808, 35.366), 0.707107);
    path.close();
    path.move_to((24.1284, 32.3664));
    path.line_to((24.111, 31.3666));
    path.conic_to((24.0935, 30.3667), (25.0934, 30.3493), 0.707107);
    path.conic_to((26.0932, 30.3318), (26.1107, 31.3317), 0.707107);
    path.line_to((26.1281, 32.3315));
    path.conic_to((26.1456, 33.3314), (25.1457, 33.3488), 0.707107);
    path.conic_to((24.1459, 33.3663), (24.1284, 32.3664), 0.707107);
    path.close();
    path.move_to((24.0761, 29.3669));
    path.line_to((24.0586, 28.367));
    path.conic_to((24.0412, 27.3672), (25.041, 27.3497), 0.707107);
    path.conic_to((26.0409, 27.3323), (26.0583, 28.3321), 0.707107);
    path.line_to((26.0758, 29.332));
    path.conic_to((26.0932, 30.3318), (25.0934, 30.3493), 0.707107);
    path.conic_to((24.0935, 30.3667), (24.0761, 29.3669), 0.707107);
    path.close();
    path.move_to((24.0237, 26.3673));
    path.line_to((24.0063, 25.3675));
    path.conic_to((23.9888, 24.3676), (24.9887, 24.3502), 0.707107);
    path.conic_to((25.9885, 24.3327), (26.006, 25.3326), 0.707107);
    path.line_to((26.0234, 26.3324));
    path.conic_to((26.0409, 27.3323), (25.041, 27.3497), 0.707107);
    path.conic_to((24.0412, 27.3672), (24.0237, 26.3673), 0.707107);
    path.close();
    test_simplify_fail(reporter, &path.detach(), filename);
}

fn bug8249(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x43310000), f32::from_bits(0x43810000)));
    // 177, 258
    path.line_to((f32::from_bits(0x43480000), f32::from_bits(0x43868000)));
    // 200, 269
    path.cubic_to((f32::from_bits(0x43480000), f32::from_bits(0x43b20000)), (f32::from_bits(0x437a0000), f32::from_bits(0x43cd0000)), (f32::from_bits(0x43c80000), f32::from_bits(0x43cd0000)));
    // 200, 356, 250, 410, 400, 410
    path.cubic_to((f32::from_bits(0x44098000), f32::from_bits(0x43cd0000)), (f32::from_bits(0x44160000), f32::from_bits(0x43b20000)), (f32::from_bits(0x44160000), f32::from_bits(0x43868000)));
    // 550, 410, 600, 356, 600, 269
    path.line_to((f32::from_bits(0x44160000), f32::from_bits(0x43808000)));
    // 600, 257
    path.cubic_to((f32::from_bits(0x44160000), f32::from_bits(0x43330000)), (f32::from_bits(0x44110000), f32::from_bits(0x429c0000)), (f32::from_bits(0x43cd0000), f32::from_bits(0x429c0000)));
    // 600, 179, 580, 78, 410, 78
    path.cubic_to((f32::from_bits(0x43700000), f32::from_bits(0x429c0000)), (f32::from_bits(0x43480000), f32::from_bits(0x431f0000)), (f32::from_bits(0x43480000), f32::from_bits(0x438a8000)));
    // 240, 78, 200, 159, 200, 277
    path.line_to((f32::from_bits(0x43480000), f32::from_bits(0x4401c000)));
    // 200, 519
    path.cubic_to((f32::from_bits(0x43480000), f32::from_bits(0x441f0000)), (f32::from_bits(0x43660000), f32::from_bits(0x44340000)), (f32::from_bits(0x43c80000), f32::from_bits(0x44340000)));
    // 200, 636, 230, 720, 400, 720
    path.cubic_to((f32::from_bits(0x4404c000), f32::from_bits(0x44340000)), (f32::from_bits(0x440d0000), f32::from_bits(0x442b8000)), (f32::from_bits(0x44118000), f32::from_bits(0x4416c000)));
    // 531, 720, 564, 686, 582, 603
    path.line_to((f32::from_bits(0x442cc000), f32::from_bits(0x441c8000)));
    // 691, 626
    path.cubic_to((f32::from_bits(0x44260000), f32::from_bits(0x443d4000)), (f32::from_bits(0x44114000), f32::from_bits(0x444a8000)), (f32::from_bits(0x43c88000), f32::from_bits(0x444a8000)));
    // 664, 757, 581, 810, 401, 810
    path.cubic_to((f32::from_bits(0x43350000), f32::from_bits(0x444a8000)), (f32::from_bits(0x42c80000), f32::from_bits(0x442e0000)), (f32::from_bits(0x42c80000), f32::from_bits(0x4401c000)));
    // 181, 810, 100, 696, 100, 519
    path.line_to((f32::from_bits(0x42c80000), f32::from_bits(0x438a8000)));
    // 100, 277
    path.cubic_to((f32::from_bits(0x42c80000), f32::from_bits(0x42cc0000)), (f32::from_bits(0x433e0000), f32::from_bits(0xc1200000)), (f32::from_bits(0x43cd0000), f32::from_bits(0xc1200000)));
    // 100, 102, 190, -10, 410, -10
    path.cubic_to((f32::from_bits(0x441d8000), f32::from_bits(0xc1200000)), (f32::from_bits(0x442f0000), f32::from_bits(0x42e60000)), (f32::from_bits(0x442f0000), f32::from_bits(0x437a0000)));
    // 630, -10, 700, 115, 700, 250
    path.line_to((f32::from_bits(0x442f0000), f32::from_bits(0x43880000)));
    // 700, 272
    path.cubic_to((f32::from_bits(0x442f0000), f32::from_bits(0x43d18000)), (f32::from_bits(0x44164000), f32::from_bits(0x43fa0000)), (f32::from_bits(0x43c88000), f32::from_bits(0x43fa0000)));
    // 700, 419, 601, 500, 401, 500
    path.cubic_to((f32::from_bits(0x43490000), f32::from_bits(0x43fa0000)), (f32::from_bits(0x43160000), f32::from_bits(0x43d00000)), (f32::from_bits(0x43160000), f32::from_bits(0x43868000)));
    // 201, 500, 150, 416, 150, 269
    path.line_to((f32::from_bits(0x43310000), f32::from_bits(0x43810000)));
    // 177, 258
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn bug8290(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((-(1e+09 as f32), -(1e+09 as f32)));
    path.line_to(((1e+09 as f32), -(1e+09 as f32)));
    path.line_to(((1e+09 as f32), (1e+09 as f32)));
    path.line_to((-(1e+09 as f32), (1e+09 as f32)));
    path.line_to((-(1e+09 as f32), -(1e+09 as f32)));
    path.close();
    path.move_to((0.0, 45.0));
    path.line_to((270.0, 45.0));
    path.line_to((270.0, 45.381));
    path.line_to((0.0, 45.381));
    path.line_to((0.0, 45.0));
    path.close();
    path.move_to((0.0, 90.381));
    path.line_to((270.0, 90.381));
    path.line_to((270.0, 90.7619));
    path.line_to((0.0, 90.7619));
    path.line_to((0.0, 90.381));
    path.close();
    path.move_to((0.0, 135.762));
    path.line_to((270.0, 135.762));
    path.line_to((270.0, 136.143));
    path.line_to((0.0, 136.143));
    path.line_to((0.0, 135.762));
    path.close();
    path.move_to((0.0, 181.143));
    path.line_to((270.0, 181.143));
    path.line_to((270.0, 181.524));
    path.line_to((0.0, 181.524));
    path.line_to((0.0, 181.143));
    path.close();
    path.move_to((0.0, 226.524));
    path.line_to((270.0, 226.524));
    path.line_to((270.0, 226.905));
    path.line_to((0.0, 226.905));
    path.line_to((0.0, 226.524));
    path.close();
    path.move_to((0.0, 271.905));
    path.line_to((270.0, 271.905));
    path.line_to((270.0, 272.286));
    path.line_to((0.0, 272.286));
    path.line_to((0.0, 271.905));
    path.close();
    path.move_to((0.0, 317.286));
    path.line_to((270.0, 317.286));
    path.line_to((270.0, 317.667));
    path.line_to((0.0, 317.667));
    path.line_to((0.0, 317.286));
    path.close();
    let matrix = Matrix::new_all(2.625, 0.0, 186.0, 0.0, 2.625, 620.0, 0.0, 0.0, 1.0);
    path.transform(&matrix);
    test_simplify(reporter, &path.detach(), filename);
}

fn bug11958_a(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x44099d81), f32::from_bits(0x00000000)));
    // 550.461f, 0
    path.line_to((f32::from_bits(0x44099d81), f32::from_bits(0x43b7276d)));
    // 550.461f, 366.308f
    path.line_to((f32::from_bits(0x44324ea8), f32::from_bits(0x43b7276d)));
    // 713.229f, 366.308f
    path.line_to((f32::from_bits(0x44324ea8), f32::from_bits(0x00000000)));
    // 713.229f, 0
    path.line_to((f32::from_bits(0x44099d81), f32::from_bits(0x00000000)));
    // 550.461f, 0
    path.close();
    path.move_to((f32::from_bits(0x440f9d71), f32::from_bits(0x00000000)));
    // 574.46f, 0
    path.line_to((f32::from_bits(0x440f9d71), f32::from_bits(0x438a1d91)));
    // 574.46f, 276.231f
    path.line_to((f32::from_bits(0x44387127), f32::from_bits(0x438a1d91)));
    // 737.768f, 276.231f
    path.quad_to((f32::from_bits(0x444d04cd), f32::from_bits(0x438a1d91)), (f32::from_bits(0x4456f396), f32::from_bits(0x4372a76c)));
    // 820.075f, 276.231f, 859.806f, 242.654f
    path.quad_to((f32::from_bits(0x4460e25e), f32::from_bits(0x435113b6)), (f32::from_bits(0x4460e25e), f32::from_bits(0x4310276d)));
    // 899.537f, 209.077f, 899.537f, 144.154f
    path.quad_to((f32::from_bits(0x4460e25e), f32::from_bits(0x429e0000)), (f32::from_bits(0x44555d70), f32::from_bits(0x421e0000)));
    // 899.537f, 79, 853.46f, 39.5f
    path.quad_to((f32::from_bits(0x4449d883), f32::from_bits(0x00000000)), (f32::from_bits(0x44321883), f32::from_bits(0x00000000)));
    // 807.383f, 0, 712.383f, 0
    path.line_to((f32::from_bits(0x440f9d71), f32::from_bits(0x00000000)));
    // 574.46f, 0
    path.close();
    // TODO(skbug.com/40043047) - This should not fail to simplify
    test_simplify_fail(reporter, &path.detach(), filename);
}

fn bug11958_b(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42240000), f32::from_bits(0x43420000)));
    // 41, 194
    path.quad_to((f32::from_bits(0x42240000), f32::from_bits(0x43928000)), (f32::from_bits(0x42930000), f32::from_bits(0x43b38000)));
    // 41, 293, 73.5f, 359
    path.quad_to((f32::from_bits(0x42d40000), f32::from_bits(0x43d48000)), (f32::from_bits(0x43240000), f32::from_bits(0x43e58000)));
    // 106, 425, 164, 459
    path.quad_to((f32::from_bits(0x435e0000), f32::from_bits(0x43f68000)), (f32::from_bits(0x43958000), f32::from_bits(0x43f68000)));
    // 222, 493, 299, 493
    path.quad_to((f32::from_bits(0x43ab0000), f32::from_bits(0x43f68000)), (f32::from_bits(0x43bd0000), f32::from_bits(0x43f2c000)));
    // 342, 493, 378, 485.5f
    path.quad_to((f32::from_bits(0x43cf0000), f32::from_bits(0x43ef0000)), (f32::from_bits(0x43df8000), f32::from_bits(0x43e80000)));
    // 414, 478, 447, 464
    path.quad_to((f32::from_bits(0x43f00000), f32::from_bits(0x43e10000)), (f32::from_bits(0x43ff8000), f32::from_bits(0x43d70000)));
    // 480, 450, 511, 430
    path.line_to((f32::from_bits(0x43f78000), f32::from_bits(0x43cc0000)));
    // 495, 408
    path.quad_to((f32::from_bits(0x43e90000), f32::from_bits(0x43d58000)), (f32::from_bits(0x43d9c000), f32::from_bits(0x43dc4000)));
    // 466, 427, 435.5f, 440.5f
    path.quad_to((f32::from_bits(0x43ca8000), f32::from_bits(0x43e30000)), (f32::from_bits(0x43b9c000), f32::from_bits(0x43e68000)));
    // 405, 454, 371.5f, 461
    path.quad_to((f32::from_bits(0x43a90000), f32::from_bits(0x43ea0000)), (f32::from_bits(0x43958000), f32::from_bits(0x43ea0000)));
    // 338, 468, 299, 468
    path.quad_to((f32::from_bits(0x43650000), f32::from_bits(0x43ea0000)), (f32::from_bits(0x43308000), f32::from_bits(0x43da4000)));
    // 229, 468, 176.5f, 436.5f
    path.quad_to((f32::from_bits(0x42f80000), f32::from_bits(0x43ca8000)), (f32::from_bits(0x42c00000), f32::from_bits(0x43ac0000)));
    // 124, 405, 96, 344
    path.quad_to((f32::from_bits(0x42880000), f32::from_bits(0x438d8000)), (f32::from_bits(0x42880000), f32::from_bits(0x43420000)));
    // 68, 283, 68, 194
    path.line_to((f32::from_bits(0x42240000), f32::from_bits(0x43420000)));
    // 41, 194
    path.close();
    path.move_to((f32::from_bits(0x43ddd958), f32::from_bits(0x440e8000)));
    // 443.698f, 570
    path.quad_to((f32::from_bits(0x43ddd958), f32::from_bits(0x44094000)), (f32::from_bits(0x43da5958), f32::from_bits(0x4404c000)));
    // 443.698f, 549, 436.698f, 531
    path.quad_to((f32::from_bits(0x43d6d958), f32::from_bits(0x44004000)), (f32::from_bits(0x43cfd958), f32::from_bits(0x43f98000)));
    // 429.698f, 513, 415.698f, 499
    path.quad_to((f32::from_bits(0x43c75958), f32::from_bits(0x43f18000)), (f32::from_bits(0x43ba9958), f32::from_bits(0x43ee0000)));
    // 398.698f, 483, 373.198f, 476
    path.quad_to((f32::from_bits(0x43add958), f32::from_bits(0x43ea8000)), (f32::from_bits(0x4396d958), f32::from_bits(0x43ea8000)));
    // 347.698f, 469, 301.698f, 469
    path.line_to((f32::from_bits(0x436cb2b0), f32::from_bits(0x43ea8000)));
    // 236.698f, 469
    path.line_to((f32::from_bits(0x436cb2b0), f32::from_bits(0x43f68000)));
    // 236.698f, 493
    path.line_to((f32::from_bits(0x43955958), f32::from_bits(0x43f68000)));
    // 298.698f, 493
    path.quad_to((f32::from_bits(0x43a8d958), f32::from_bits(0x43f68000)), (f32::from_bits(0x43b3d958), f32::from_bits(0x43f90000)));
    // 337.698f, 493, 359.698f, 498
    path.quad_to((f32::from_bits(0x43bed958), f32::from_bits(0x43fb8000)), (f32::from_bits(0x43c55958), f32::from_bits(0x4400c000)));
    // 381.698f, 503, 394.698f, 515
    path.quad_to((f32::from_bits(0x43cb5958), f32::from_bits(0x44030000)), (f32::from_bits(0x43cdd958), f32::from_bits(0x4406a000)));
    // 406.698f, 524, 411.698f, 538.5f
    path.quad_to((f32::from_bits(0x43d05958), f32::from_bits(0x440a4000)), (f32::from_bits(0x43d05958), f32::from_bits(0x440e8000)));
    // 416.698f, 553, 416.698f, 570
    path.line_to((f32::from_bits(0x43ddd958), f32::from_bits(0x440e8000)));
    // 443.698f, 570
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

fn bug11958_c(reporter: &mut Reporter, filename: &str) {
    let mut path = PathBuilder::new();
    path.set_fill_type(PathFillType::Winding);
    path.move_to((200., 200.));
    path.line_to((164., 459.));
    path.quad_to((222., 493.), (299., 493.));
    path.quad_to((342., 493.), (378., 485.));
    path.close();
    path.move_to((415.698, 499.));
    path.line_to((236.698, 469.));
    path.line_to((236.698, 493.));
    path.line_to((298.698, 493.));
    path.quad_to((337.698, 493.), (359.698, 498.));
    path.close();
    test_simplify(reporter, &path.detach(), filename);
}

// Port of: tests/PathOpsSimplifyTest.cpp#L9971-L9981 (chrome/m156)
def_test!(PathOpsSimplify, |reporter| {
    // RunTestSet(reporter, tests, testCount, nullptr, nullptr, nullptr, false), with
    // runSubTests false.
    for &(name, test) in TESTS {
        test(reporter, name);
    }
});

// Port of: tests/PathOpsSimplifyTest.cpp#L9989-L10015 (chrome/m156)
def_test!(bug_513001309, |_reporter| {
    let mut path = PathBuilder::new();
    for i in 0..5 {
    let off: f32 = i as f32 * 0.0001;
    path.set_fill_type(PathFillType::EvenOdd);
    path.move_to((f32::from_bits(0x43b40000) + off, f32::from_bits(0xcf000000)));
    path.cubic_to((f32::from_bits(0x4e0d628f), f32::from_bits(0xceffffff)), (f32::from_bits(0x4e800003), f32::from_bits(0xcec6b143)), (f32::from_bits(0x4e800002), f32::from_bits(0xce7ffffc)));
    path.cubic_to((f32::from_bits(0x4e800002), f32::from_bits(0xcde53aee)), (f32::from_bits(0x4e0d6292), f32::from_bits(0xc307820e)), (f32::from_bits(0x44627d00), f32::from_bits(0x437ffff2)));
    path.line_to((f32::from_bits(0x444bf3bc), f32::from_bits(0x4460537e)));
    path.line_to((f32::from_bits(0x43553abd), f32::from_bits(0x440f3cbd)));
    path.line_to((f32::from_bits(0x42000000), f32::from_bits(0x41800000)));
    path.line_to((f32::from_bits(0x42c80000), f32::from_bits(0x44000000)));
    path.line_to((f32::from_bits(0x43553abd), f32::from_bits(0x440f3cbd)));
    path.line_to((f32::from_bits(0x43b40000), f32::from_bits(0x44800000)));
    path.line_to((f32::from_bits(0x43b40000), f32::from_bits(0x45816000)));
    path.set_fill_type(PathFillType::Winding);
    path.move_to((f32::from_bits(0x42fe0000) + off, f32::from_bits(0x43a08000)));
    path.line_to((f32::from_bits(0x45d5c000), f32::from_bits(0x43870000)));
    path.line_to((f32::from_bits(0xd0a00000), f32::from_bits(0x4cbebc20)));
    path.line_to((f32::from_bits(0x451f7000), f32::from_bits(0x42800000)));
    path.line_to((f32::from_bits(0x42fe0000), f32::from_bits(0x43a08000)));
    path.close();
    }
    // This caused a corruption/assert w/o the fix
    let _ = simplify(&path.detach());
});
