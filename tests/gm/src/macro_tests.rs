// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Exercises every registration macro end to end (registry + generated test + golden check).
//!
//! These are **stand-ins, not ports**: each borrows the name and size of a real GM whose goldens
//! are solid white on every tier and config (its real drawing ends up empty) and draws nothing,
//! so the generated tests check against real goldens. They exist only in this crate's unit-test
//! build, under `macro_tests::`, which no manifest id maps to; `gm-verify` never sees them.

use crate::prelude::*;
use crate::registry;

// `DEF_SIMPLE_GM_BG`, white background.
crate::def_simple_gm_bg!(path_effect_empty_result, canvas, 100, 100, Color::WHITE, {
    assert_eq!(canvas.base_layer_size(), ISize::new(100, 100));
    if canvas.save_count() == 2 {
        return; // the body is `void NAME_GM_inner(SkCanvas*)`: early returns are allowed
    }
    unreachable!("drawn inside GM::drawContent's SkAutoCanvasRestore");
});

// `DEF_SIMPLE_GM_CAN_FAIL`.
crate::def_simple_gm_can_fail!(bug591993, canvas, error_msg, 40, 140, {
    assert_eq!(error_msg, "");
    DrawResult::Ok
});

// `DEF_SIMPLE_GM_BG_NAME_CAN_FAIL`: the result is named after NAME_STR.
crate::def_simple_gm_bg_name_can_fail!(
    mesh_with_effects_standin,
    canvas,
    error_msg,
    320,
    320,
    Color::WHITE,
    "mesh_with_effects",
    { DrawResult::Ok }
);

// A skipping GM: neither we nor the oracle (which has no such GM) write a result.
crate::def_simple_gm_bg_can_fail!(skipping_standin, canvas, error_msg, 10, 10, Color::BLACK, {
    error_msg.push_str("not for raster");
    DrawResult::Skip
});

/// A class GM that sets its background in `onOnceBeforeDraw` (read after it, as in Skia).
struct MeshZeroInitStandin {
    bg: Color,
}

impl GM for MeshZeroInitStandin {
    fn name(&self) -> String {
        "mesh_zero_init".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(90, 30)
    }

    fn bg_color(&self) -> Color {
        self.bg
    }

    fn on_once_before_draw(&mut self) {
        self.bg = Color::WHITE;
    }

    fn on_draw(&mut self, _canvas: &Canvas) {}
}

// `DEF_GM(return new MeshZeroInitGM();)` with a manifest name that is not an identifier.
crate::def_gm!(
    MeshZeroInitStandin_ = "MeshZeroInitStandin()",
    MeshZeroInitStandin { bg: Color::RED }
);

#[test]
fn registrations_have_manifest_keys() {
    let keys: Vec<String> = registry::all().iter().map(|r| r.key()).collect();
    for key in [
        "gm::fiddle::fiddle",
        "macro_tests::path_effect_empty_result",
        "macro_tests::bug591993",
        "macro_tests::mesh_with_effects_standin",
        "macro_tests::skipping_standin",
        "macro_tests::MeshZeroInitStandin()",
    ] {
        assert!(keys.iter().any(|k| k == key), "{key} not in {keys:?}");
    }
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
}
