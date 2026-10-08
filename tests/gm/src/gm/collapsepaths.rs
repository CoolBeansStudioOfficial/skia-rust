// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/collapsepaths.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;

// SkDoubleToScalar: the C++ literals are doubles, narrowed to float.
#[allow(clippy::cast_possible_truncation)] // mirrors SkDoubleToScalar
fn d(x: f64) -> f32 {
    x as f32
}

// Port of: gm/collapsepaths.cpp#L19-L32 (chrome/m156)
#[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
fn test_collapse1(canvas: &Canvas, paint: &Paint) {
    let mut path = PathBuilder::new();
    canvas.translate((0.0, 0.0));
    path.move_to((652.830_078_125, 673.936_523_437_5));
    path.line_to((479.501_525_878_906_25, 213.412_628_173_828_125));
    path.line_to((511.840_545_654_296_875, 209.155_105_590_820_312_5));
    path.line_to((528.149_597_167_968_75, 208.621_215_820_312_5));
    path.move_to((370.506_530_761_718_75, 73.684_051_513_671_875));
    path.line_to((525.020_935_058_593_75, 208.641_372_680_664_062_5));
    path.line_to((478.403_564_453_125, 213.599_899_291_992_187_5));
    path.set_fill_type(PathFillType::EvenOdd);
    canvas.draw_path(&path.detach(), paint);
}

// Port of: gm/collapsepaths.cpp#L34-L44 (chrome/m156)
#[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
fn test_collapse2(canvas: &Canvas, paint: &Paint) {
    let mut path = PathBuilder::new();
    path.move_to((492.781_982_421_875, 508.713_989_257_812_5));
    path.line_to((361.946_746_826_171_875, 161.092_300_415_039_062_5));
    path.line_to((386.357_513_427_734_375, 157.878_555_297_851_562_5));
    path.line_to((398.668_212_890_625, 157.475_555_419_921_875));
    path.move_to((279.673_004_150_390_625, 55.619_640_350_341_796_875));
    path.line_to((396.306_579_589_843_75, 157.490_768_432_617_187_5));
    path.line_to((361.117_950_439_453_125, 161.233_657_836_914_062_5));
    canvas.draw_path(&path.detach(), paint);
}

// Port of: gm/collapsepaths.cpp#L46-L56 (chrome/m156)
#[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
fn test_collapse3(canvas: &Canvas, paint: &Paint) {
    let mut path = PathBuilder::new();
    path.move_to((31.973_098_754_882_812_5, 69.414_916_992_187_5));
    path.line_to((36.630_767_822_265_625, 67.661_903_381_347_656_25));
    path.line_to((51.149_887_084_960_937_5, 64.276_504_516_601_562_5));
    path.move_to((52.945_800_781_25, 64.055_603_027_343_75));
    path.line_to((38.999_435_424_804_687_5, 66.898_071_289_062_5));
    path.line_to((32.229_583_740_234_375, 69.316_963_195_800_781_25));
    path.line_to((12.998_107_910_156_25, 22.472_366_333_007_812_5));
    canvas.draw_path(&path.detach(), paint);
}

// Port of: gm/collapsepaths.cpp#L58-L68 (chrome/m156)
#[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
fn test_collapse4(canvas: &Canvas, paint: &Paint) {
    let mut path = PathBuilder::new();
    path.move_to((122.662_658_691_406_25, 77.814_888_000_488_281_25));
    path.line_to((161.983_642_578_125, 128.557_952_880_859_375));
    path.line_to((22.599_969_863_891_601_562, 76.618_598_937_988_281_25));
    path.line_to((18.031_547_546_386_718_75, 76.055_633_544_921_875));
    path.line_to((15.403_129_577_636_718_75, 75.764_724_731_445_312_5));
    path.line_to((18.572_841_644_287_109_375, 75.225_112_915_039_062_5));
    path.line_to((20.895_002_365_112_304_688, 73.793_777_465_820_312_5));
    canvas.draw_path(&path.detach(), paint);
}

// Port of: gm/collapsepaths.cpp#L70-L80 (chrome/m156)
#[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
fn test_collapse5(canvas: &Canvas, paint: &Paint) {
    let mut path = PathBuilder::new();
    path.move_to((52.659_847_259_521_484_375, 782.054_687_5));
    path.line_to((136.691_513_061_523_437_5, 690.180_114_746_093_75));
    path.line_to((392.147_796_630_859_375, 554.609_008_789_062_5));
    path.line_to((516.514_709_472_656_25, 534.441_345_214_843_75));
    path.move_to((154.618_270_874_023_437_5, 188.230_926_513_671_875));
    path.line_to((430.242_095_947_265_625, 546.766_052_246_093_75));
    path.line_to((373.100_585_937_5, 559.090_698_242_187_5));
    path.set_fill_type(PathFillType::EvenOdd);
    canvas.draw_path(&path.detach(), paint);
}

// Port of: gm/collapsepaths.cpp#L82-L92 (chrome/m156)
#[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
fn test_collapse6(canvas: &Canvas, paint: &Paint) {
    let mut path = PathBuilder::new();
    path.move_to((13.314_494_132_995_605_469, 197.734_390_258_789_062_5));
    path.line_to((34.561_027_526_855_468_75, 174.504_867_553_710_937_5));
    path.line_to((99.150_489_807_128_906_25, 140.227_111_816_406_25));
    path.line_to((130.595_367_431_640_625, 135.127_929_687_5));
    path.move_to((39.093_620_300_292_968_75, 47.592_231_750_488_281_25));
    path.line_to((108.782_241_821_289_062_5, 138.244_110_107_421_875));
    path.line_to((94.334_602_355_957_031_25, 141.360_260_009_765_625));
    path.set_fill_type(PathFillType::EvenOdd);
    canvas.draw_path(&path.detach(), paint);
}

// Port of: gm/collapsepaths.cpp#L94-L104 (chrome/m156)
#[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
fn test_collapse7(canvas: &Canvas, paint: &Paint) {
    let mut path = PathBuilder::new();
    path.move_to((13.737_141_609_191_894_531, 204.011_154_174_804_687_5));
    path.line_to((35.658_111_572_265_625, 180.044_250_488_281_25));
    path.line_to((102.297_866_821_289_062_5, 144.678_405_761_718_75));
    path.line_to((134.740_905_761_718_75, 139.417_358_398_437_5));
    path.move_to((40.334_587_097_167_968_75, 49.102_973_937_988_281_25));
    path.line_to((112.235_366_821_289_062_5, 142.632_446_289_062_5));
    path.line_to((97.329_109_191_894_531_25, 145.847_518_920_898_437_5));
    path.set_fill_type(PathFillType::EvenOdd);
    canvas.draw_path(&path.detach(), paint);
}

// Port of: gm/collapsepaths.cpp#L106-L116 (chrome/m156)
#[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
fn test_collapse8(canvas: &Canvas, paint: &Paint) {
    let mut path = PathBuilder::new();
    path.move_to((11.75, 174.50));
    path.line_to((30.50, 154.00));
    path.line_to((87.50, 123.75));
    path.line_to((115.25, 119.25));
    path.move_to((34.50, 42.00));
    path.line_to((96.00, 122.00));
    path.line_to((83.25, 124.75));
    path.set_fill_type(PathFillType::EvenOdd);
    canvas.draw_path(&path.detach(), paint);
}

// Port of: gm/collapsepaths.cpp#L118-L132 (chrome/m156)
#[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
fn test_collapse9(canvas: &Canvas, paint: &Paint) {
    let mut path = PathBuilder::new();
    path.move_to((d(13.25), d(197.75)));
    path.line_to((d(34.75), d(174.75)));
    path.line_to((d(99.0364), d(140.364)));
    path.line_to((d(99.25), d(140.25)));
    path.line_to((d(100.167), d(140.096)));
    path.line_to((d(130.50), d(135.00)));
    path.move_to((d(39.25), d(47.50)));
    path.line_to((d(100.167), d(140.096)));
    path.line_to((d(99.0364), d(140.364)));
    path.line_to((d(94.25), d(141.50)));
    path.set_fill_type(PathFillType::EvenOdd);
    canvas.draw_path(&path.detach(), paint);
}

// This one is a thin inverted 'v', but the edges should not disappear at any point.
// Port of: gm/collapsepaths.cpp#L134-L143 (chrome/m156)
#[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
fn test_collapse10(canvas: &Canvas, paint: &Paint) {
    let mut path = PathBuilder::new();
    path.move_to((5.5, 36.0));
    path.line_to((47.5, 5.0));
    path.line_to((90.0, 36.0));
    path.line_to((88.5, 36.0));
    path.line_to((47.5, 6.0));
    path.line_to((7.0, 36.0));
    canvas.draw_path(&path.detach(), paint);
}

// Port of: gm/collapsepaths.cpp#L147-L161 (chrome/m156)
crate::def_simple_gm!(collapsepaths, canvas, 500, 600, {
    let mut paint = Paint::default();

    paint.set_anti_alias(true);
    paint.set_style(Style::Fill);
    test_collapse1(canvas, &paint);
    test_collapse2(canvas, &paint);
    test_collapse3(canvas, &paint);
    test_collapse4(canvas, &paint);
    test_collapse5(canvas, &paint);
    test_collapse6(canvas, &paint);
    test_collapse7(canvas, &paint);
    test_collapse8(canvas, &paint);
    test_collapse9(canvas, &paint);
    test_collapse10(canvas, &paint);
});
