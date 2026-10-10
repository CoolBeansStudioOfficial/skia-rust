// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ParseColorTest.cpp (chrome/m156)

use skia_rust_core::color::Color;
use skia_rust_core::utils::parse_color::find_named_color;

use crate::{def_test, reporter_assert};

// Port of: tests/ParseColorTest.cpp#L14-L159 (chrome/m156)
const G_NAMED_COLORS: [(&str, u32); 140] = [
    ("aliceblue", 0xfff0_f8ff),
    ("antiquewhite", 0xfffa_ebd7),
    ("aqua", 0xff00_ffff),
    ("aquamarine", 0xff7f_ffd4),
    ("azure", 0xfff0_ffff),
    ("beige", 0xfff5_f5dc),
    ("bisque", 0xffff_e4c4),
    ("black", 0xff00_0000),
    ("blanchedalmond", 0xffff_ebcd),
    ("blue", 0xff00_00ff),
    ("blueviolet", 0xff8a_2be2),
    ("brown", 0xffa5_2a2a),
    ("burlywood", 0xffde_b887),
    ("cadetblue", 0xff5f_9ea0),
    ("chartreuse", 0xff7f_ff00),
    ("chocolate", 0xffd2_691e),
    ("coral", 0xffff_7f50),
    ("cornflowerblue", 0xff64_95ed),
    ("cornsilk", 0xffff_f8dc),
    ("crimson", 0xffdc_143c),
    ("cyan", 0xff00_ffff),
    ("darkblue", 0xff00_008b),
    ("darkcyan", 0xff00_8b8b),
    ("darkgoldenrod", 0xffb8_860b),
    ("darkgray", 0xffa9_a9a9),
    ("darkgreen", 0xff00_6400),
    ("darkkhaki", 0xffbd_b76b),
    ("darkmagenta", 0xff8b_008b),
    ("darkolivegreen", 0xff55_6b2f),
    ("darkorange", 0xffff_8c00),
    ("darkorchid", 0xff99_32cc),
    ("darkred", 0xff8b_0000),
    ("darksalmon", 0xffe9_967a),
    ("darkseagreen", 0xff8f_bc8f),
    ("darkslateblue", 0xff48_3d8b),
    ("darkslategray", 0xff2f_4f4f),
    ("darkturquoise", 0xff00_ced1),
    ("darkviolet", 0xff94_00d3),
    ("deeppink", 0xffff_1493),
    ("deepskyblue", 0xff00_bfff),
    ("dimgray", 0xff69_6969),
    ("dodgerblue", 0xff1e_90ff),
    ("firebrick", 0xffb2_2222),
    ("floralwhite", 0xffff_faf0),
    ("forestgreen", 0xff22_8b22),
    ("fuchsia", 0xffff_00ff),
    ("gainsboro", 0xffdc_dcdc),
    ("ghostwhite", 0xfff8_f8ff),
    ("gold", 0xffff_d700),
    ("goldenrod", 0xffda_a520),
    ("gray", 0xff80_8080),
    ("green", 0xff00_8000),
    ("greenyellow", 0xffad_ff2f),
    ("honeydew", 0xfff0_fff0),
    ("hotpink", 0xffff_69b4),
    ("indianred", 0xffcd_5c5c),
    ("indigo", 0xff4b_0082),
    ("ivory", 0xffff_fff0),
    ("khaki", 0xfff0_e68c),
    ("lavender", 0xffe6_e6fa),
    ("lavenderblush", 0xffff_f0f5),
    ("lawngreen", 0xff7c_fc00),
    ("lemonchiffon", 0xffff_facd),
    ("lightblue", 0xffad_d8e6),
    ("lightcoral", 0xfff0_8080),
    ("lightcyan", 0xffe0_ffff),
    ("lightgoldenrodyellow", 0xfffa_fad2),
    ("lightgreen", 0xff90_ee90),
    ("lightgrey", 0xffd3_d3d3),
    ("lightpink", 0xffff_b6c1),
    ("lightsalmon", 0xffff_a07a),
    ("lightseagreen", 0xff20_b2aa),
    ("lightskyblue", 0xff87_cefa),
    ("lightslategray", 0xff77_8899),
    ("lightsteelblue", 0xffb0_c4de),
    ("lightyellow", 0xffff_ffe0),
    ("lime", 0xff00_ff00),
    ("limegreen", 0xff32_cd32),
    ("linen", 0xfffa_f0e6),
    ("magenta", 0xffff_00ff),
    ("maroon", 0xff80_0000),
    ("mediumaquamarine", 0xff66_cdaa),
    ("mediumblue", 0xff00_00cd),
    ("mediumorchid", 0xffba_55d3),
    ("mediumpurple", 0xff93_70db),
    ("mediumseagreen", 0xff3c_b371),
    ("mediumslateblue", 0xff7b_68ee),
    ("mediumspringgreen", 0xff00_fa9a),
    ("mediumturquoise", 0xff48_d1cc),
    ("mediumvioletred", 0xffc7_1585),
    ("midnightblue", 0xff19_1970),
    ("mintcream", 0xfff5_fffa),
    ("mistyrose", 0xffff_e4e1),
    ("moccasin", 0xffff_e4b5),
    ("navajowhite", 0xffff_dead),
    ("navy", 0xff00_0080),
    ("oldlace", 0xfffd_f5e6),
    ("olive", 0xff80_8000),
    ("olivedrab", 0xff6b_8e23),
    ("orange", 0xffff_a500),
    ("orangered", 0xffff_4500),
    ("orchid", 0xffda_70d6),
    ("palegoldenrod", 0xffee_e8aa),
    ("palegreen", 0xff98_fb98),
    ("paleturquoise", 0xffaf_eeee),
    ("palevioletred", 0xffdb_7093),
    ("papayawhip", 0xffff_efd5),
    ("peachpuff", 0xffff_dab9),
    ("peru", 0xffcd_853f),
    ("pink", 0xffff_c0cb),
    ("plum", 0xffdd_a0dd),
    ("powderblue", 0xffb0_e0e6),
    ("purple", 0xff80_0080),
    ("red", 0xffff_0000),
    ("rosybrown", 0xffbc_8f8f),
    ("royalblue", 0xff41_69e1),
    ("saddlebrown", 0xff8b_4513),
    ("salmon", 0xfffa_8072),
    ("sandybrown", 0xfff4_a460),
    ("seagreen", 0xff2e_8b57),
    ("seashell", 0xffff_f5ee),
    ("sienna", 0xffa0_522d),
    ("silver", 0xffc0_c0c0),
    ("skyblue", 0xff87_ceeb),
    ("slateblue", 0xff6a_5acd),
    ("slategray", 0xff70_8090),
    ("snow", 0xffff_fafa),
    ("springgreen", 0xff00_ff7f),
    ("steelblue", 0xff46_82b4),
    ("tan", 0xffd2_b48c),
    ("teal", 0xff00_8080),
    ("thistle", 0xffd8_bfd8),
    ("tomato", 0xffff_6347),
    ("turquoise", 0xff40_e0d0),
    ("violet", 0xffee_82ee),
    ("wheat", 0xfff5_deb3),
    ("white", 0xffff_ffff),
    ("whitesmoke", 0xfff5_f5f5),
    ("yellow", 0xffff_ff00),
    ("yellowgreen", 0xff9a_cd32),
];

// Port of: tests/ParseColorTest.cpp#L161-L166 (chrome/m156)
fn is_valid_name(name: &str) -> bool {
    G_NAMED_COLORS.iter().any(|(n, _)| name == *n)
}

// Port of: tests/ParseColorTest.cpp#L168-L198 (chrome/m156)
def_test!(ParseNamedColor, |r| {
    for &(name, expected) in &G_NAMED_COLORS {
        // check the real name
        let found = find_named_color(name);
        reporter_assert!(r, found.map(|(len, _)| len) == Some(name.len()));
        reporter_assert!(
            r,
            found.map(|(_, color)| color) == Some(Color::new(expected))
        );

        // check partial prefixes
        for l in (0..name.len()).rev() {
            let s = &name[..l];
            // some substrings are valid color names
            if is_valid_name(s) {
                continue;
            }

            reporter_assert!(r, find_named_color(s).is_none());
        }

        // check suffixes
        let s = format!("{name}A");
        reporter_assert!(r, find_named_color(&s).is_none());
    }

    // some oddballs
    reporter_assert!(r, find_named_color("").is_none());
    reporter_assert!(r, find_named_color("aaa").is_none());
    reporter_assert!(r, find_named_color("zzz").is_none());
    reporter_assert!(r, find_named_color("aaaaaaaaaaaa").is_none());
    reporter_assert!(r, find_named_color("zzzzzzzzzzzz").is_none());
});
