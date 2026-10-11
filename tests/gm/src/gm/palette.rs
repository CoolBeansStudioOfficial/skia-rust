// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/palette.cpp (chrome/m156)
//
// Skip-matching only (as `gm/colrv1.rs`): the COLR/CPAL test font is loaded through the test font
// manager, and the GM returns `DrawResult::Skip` when either typeface is missing, as it did when
// the goldens were made. The palette drawing code is not ported, because it can never run here;
// a non-null typeface panics.

use crate::prelude::*;
use skia_rust_core::typeface::Typeface;
use skia_rust_tools::font_tool_utils::create_typeface_from_resource;
use skia_rust_tools::resources::get_resource_as_stream;

// Port of: gm/palette.cpp#L41 (chrome/m156), kColrCpalTestFontPath
const COLR_CPAL_TEST_FONT_PATH: &str = "fonts/test_glyphs-glyf_colr_1.ttf";

// Port of: gm/palette.cpp#L131-L177 (chrome/m156), FontPaletteGM
struct FontPaletteGm {
    test_name: &'static str,
    typeface: Option<Typeface>,
}

impl FontPaletteGm {
    fn new(test_name: &'static str) -> Self {
        Self {
            test_name,
            typeface: None,
        }
    }
}

impl GM for FontPaletteGm {
    // Port of: gm/palette.cpp#L144-L147 (chrome/m156), getName
    fn name(&self) -> String {
        format!("font_palette_{}", self.test_name)
    }

    fn size(&mut self) -> ISize {
        ISize::new(1000, 400)
    }

    // Port of: gm/palette.cpp#L135-L142 (chrome/m156), onOnceBeforeDraw. The typeface is loaded
    // from the resource, and the GM skips when it is not recognised.
    fn on_once_before_draw(&mut self) {
        self.typeface =
            create_typeface_from_resource(get_resource_as_stream(COLR_CPAL_TEST_FONT_PATH), 0);
    }

    // Port of: gm/palette.cpp#L148-L162 (chrome/m156), onDraw: the skip path
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        canvas.draw_color(Color::WHITE, None);
        if self.typeface.is_none() {
            "Did not recognize COLR v1 test font format.".clone_into(error_msg);
            return DrawResult::Skip;
        }
        panic!("a COLR/CPAL typeface is not drawn by this port (palette drawing is not ported)");
    }
}

// Port of: gm/palette.cpp#L170 (chrome/m156)
crate::def_gm!(
    FontPaletteGM_default = "FontPaletteGM(\"default\", SkFontArguments::Palette())",
    FontPaletteGm::new("default")
);
// Port of: gm/palette.cpp#L167 (chrome/m156)
crate::def_gm!(
    FontPaletteGM_light = "FontPaletteGM(\"light\", kLightPaletteOverride)",
    FontPaletteGm::new("light")
);
// Port of: gm/palette.cpp#L168 (chrome/m156)
crate::def_gm!(
    FontPaletteGM_dark = "FontPaletteGM(\"dark\", kDarkPaletteOverride)",
    FontPaletteGm::new("dark")
);
// Port of: gm/palette.cpp#L169 (chrome/m156)
crate::def_gm!(
    FontPaletteGM_one = "FontPaletteGM(\"one\", kOnePaletteOverride)",
    FontPaletteGm::new("one")
);
// Port of: gm/palette.cpp#L170 (chrome/m156)
crate::def_gm!(
    FontPaletteGM_all = "FontPaletteGM(\"all\", kAllPaletteOverride)",
    FontPaletteGm::new("all")
);
