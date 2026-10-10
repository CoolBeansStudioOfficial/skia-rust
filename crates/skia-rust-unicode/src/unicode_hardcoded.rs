// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skunicode/src/SkUnicode_hardcoded.{h,cpp}

//! The hardcoded character properties of `SkUnicodeHardCodedCharProperties`. The
//! [`crate::unicode::Unicode`] trait's default methods call these.

use skia_rust_core::utf::Unichar;

// Port of: modules/skunicode/src/SkUnicode_hardcoded.cpp#L14-L18 (chrome/m156)
pub(crate) fn is_control(utf8: Unichar) -> bool {
    (utf8 < Unichar::from(b' '))
        || (0x7f..=0x9f).contains(&utf8)
        || (0x200D..=0x200F).contains(&utf8)
        || (0x202A..=0x202E).contains(&utf8)
}

// Port of: modules/skunicode/src/SkUnicode_hardcoded.cpp#L20-L48 (chrome/m156)
pub(crate) fn is_whitespace(unichar: Unichar) -> bool {
    const WHITESPACES: [Unichar; 21] = [
        0x0009, // character tabulation
        0x000A, // line feed
        0x000B, // line tabulation
        0x000C, // form feed
        0x000D, // carriage return
        0x0020, // space
        //0x0085, // next line
        //0x00A0, // no-break space
        0x1680, // ogham space mark
        0x2000, // en quad
        0x2001, // em quad
        0x2002, // en space
        0x2003, // em space
        0x2004, // three-per-em space
        0x2005, // four-per-em space
        0x2006, // six-per-em space
        //0x2007, // figure space
        0x2008, // punctuation space
        0x2009, // thin space
        0x200A, // hair space
        0x2028, // line separator
        0x2029, // paragraph separator
        //0x202F, // narrow no-break space
        0x205F, // medium mathematical space
        0x3000, // ideographic space
    ];
    WHITESPACES.contains(&unichar)
}

// Port of: modules/skunicode/src/SkUnicode_hardcoded.cpp#L50-L78 (chrome/m156)
pub(crate) fn is_space(unichar: Unichar) -> bool {
    const SPACES: [Unichar; 25] = [
        0x0009, // character tabulation
        0x000A, // line feed
        0x000B, // line tabulation
        0x000C, // form feed
        0x000D, // carriage return
        0x0020, // space
        0x0085, // next line
        0x00A0, // no-break space
        0x1680, // ogham space mark
        0x2000, // en quad
        0x2001, // em quad
        0x2002, // en space
        0x2003, // em space
        0x2004, // three-per-em space
        0x2005, // four-per-em space
        0x2006, // six-per-em space
        0x2007, // figure space
        0x2008, // punctuation space
        0x2009, // thin space
        0x200A, // hair space
        0x2028, // line separator
        0x2029, // paragraph separator
        0x202F, // narrow no-break space
        0x205F, // medium mathematical space
        0x3000, // ideographic space
    ];
    SPACES.contains(&unichar)
}

// Port of: modules/skunicode/src/SkUnicode_hardcoded.cpp#L80-L82 (chrome/m156)
pub(crate) fn is_tabulation(utf8: Unichar) -> bool {
    utf8 == Unichar::from(b'\t')
}

// Port of: modules/skunicode/src/SkUnicode_hardcoded.cpp#L84-L86 (chrome/m156)
pub(crate) fn is_hard_break(utf8: Unichar) -> bool {
    utf8 == Unichar::from(b'\n') || utf8 == 0x2028
}

// Port of: modules/skunicode/src/SkUnicode_hardcoded.cpp#L88-L91 (chrome/m156)
// The emoji and regional-indicator properties are not known to the hardcoded implementation.
// `SkDEBUGFAIL` is a debug assertion, so the debug build stops here, as Skia's does.
pub(crate) fn is_emoji(_unichar: Unichar) -> bool {
    debug_assert!(false, "isEmoji Not implemented");
    false
}

// Port of: modules/skunicode/src/SkUnicode_hardcoded.cpp#L93-L96 (chrome/m156)
pub(crate) fn is_emoji_component(_utf8: Unichar) -> bool {
    debug_assert!(false, "isEmojiComponent Not implemented");
    false
}

// Port of: modules/skunicode/src/SkUnicode_hardcoded.cpp#L98-L101 (chrome/m156)
pub(crate) fn is_emoji_modifier(_utf8: Unichar) -> bool {
    debug_assert!(false, "isEmojiModifier Not implemented");
    false
}

// Port of: modules/skunicode/src/SkUnicode_hardcoded.cpp#L103-L106 (chrome/m156)
pub(crate) fn is_emoji_modifier_base(_utf8: Unichar) -> bool {
    debug_assert!(false, "isEmojiModifierBase Not implemented");
    false
}

// Port of: modules/skunicode/src/SkUnicode_hardcoded.cpp#L108-L111 (chrome/m156)
pub(crate) fn is_regional_indicator(_unichar: Unichar) -> bool {
    debug_assert!(false, "isRegionalIndicator Not implemented");
    false
}

// Port of: modules/skunicode/src/SkUnicode_hardcoded.cpp#L113-L130 (chrome/m156)
// Skia's upper bounds are exclusive (`range.second > unichar`), so the ranges are half-open.
pub(crate) fn is_ideographic(unichar: Unichar) -> bool {
    const RANGES: [(Unichar, Unichar); 8] = [
        (4352, 4607),       // Hangul Jamo
        (11904, 42191),     // CJK_Radicals
        (43072, 43135),     // Phags_Pa
        (44032, 55215),     // Hangul_Syllables
        (63744, 64255),     // CJK_Compatibility_Ideographs
        (65072, 65103),     // CJK_Compatibility_Forms
        (65381, 65500),     // Katakana_Hangul_Halfwidth
        (131_072, 196_607), // Supplementary_Ideographic_Plane
    ];
    RANGES
        .iter()
        .any(|&(first, second)| first <= unichar && second > unichar)
}
