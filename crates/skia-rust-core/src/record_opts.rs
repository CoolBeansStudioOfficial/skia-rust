// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRecordOpts.h, src/core/SkRecordOpts.cpp

//! `SkRecordOpts`: peephole optimizations of a [`Record`].
//!
//! Most of the optimizations in this file are pattern-based. They are all functions with a
//! `Pattern` and an `on_match(record, pattern, begin, end)` that returns true if it made changes
//! and false if not.

use crate::blend_mode::BlendMode;
use crate::color::ALPHA_TRANSPARENT;
use crate::math::mul_div_255_round;
use crate::paint::Paint;
use crate::record::Record;
use crate::record_pattern::{Greedy, Is, IsDraw, IsSingleDraw, Not, Or, Pattern, PatternElements};
use crate::records::{ClipRect, NoOp, RecordKind, Restore, Save, SaveLayer};

// Run a pattern-based optimization once across the record, returning true if it made any changes.
// It looks for spans which match the pattern, and when found calls `on_match()` with that
// pattern, record, and [begin,end) span of the commands that matched.
// Port of: src/core/SkRecordOpts.cpp#L30-L42 (chrome/m156)
fn apply<Ms: PatternElements>(
    record: &mut Record,
    mut on_match: impl FnMut(&mut Record, &Pattern<Ms>, usize, usize) -> bool,
) -> bool {
    let mut pattern = Pattern::<Ms>::default();
    let mut changed = false;
    let mut begin = 0;
    let mut end = 0;

    while pattern.search(record, &mut begin, &mut end) {
        changed |= on_match(record, &pattern, begin, end);
    }
    changed
}

///////////////////////////////////////////////////////////////////////////////////////////////////

// Turns the logical NoOp Save and Restore in Save-Draw*-Restore patterns into actual NoOps.
// Port of: src/core/SkRecordOpts.cpp#L46-L57 (chrome/m156)
type SaveOnlyDrawsRestoreNooper = (Is<Save>, Greedy<Or<(Is<NoOp>, IsDraw)>>, Is<Restore>);

fn save_only_draws_restore_nooper(
    record: &mut Record,
    _pattern: &Pattern<SaveOnlyDrawsRestoreNooper>,
    begin: usize,
    end: usize,
) -> bool {
    record.replace(begin, NoOp); // Save
    record.replace(end - 1, NoOp); // Restore
    true
}

// Port of: src/core/SkRecordOpts.cpp#L59-L107 (chrome/m156)
fn fold_opacity_layer_color_to_paint(
    layer_paint: Option<&Paint>,
    is_save_layer: bool,
    paint: &mut Paint,
) -> bool {
    // We assume layerPaint is always from a saveLayer.  If isSaveLayer is
    // true, we assume paint is too.

    // The alpha folding can proceed if the filter layer paint does not have properties which cause
    // the resulting filter layer to be "blended" in complex ways to the parent layer.
    // TODO: most likely only some xfer modes are the hard constraints
    if !paint.is_src_over() {
        return false;
    }

    if !is_save_layer && paint.image_filter().is_some() {
        // For normal draws, the paint color is used as one input for the color for the draw. Image
        // filter will operate on the result, and thus we can not change the input.
        // For layer saves, the image filter is applied to the layer contents. The layer is then
        // modulated with the paint color, so it's fine to proceed with the fold for saveLayer
        // paints with image filters.
        return false;
    }

    if paint.color_filter().is_some() {
        // Filter input depends on the paint color.

        // Here we could filter the color if we knew the draw is going to be uniform color.  This
        // should be detectable as drawPath/drawRect/.. without a shader being uniform, while
        // drawBitmap/drawSprite or a shader being non-uniform. However, current matchers don't
        // give the type out easily, so just do not optimize that at the moment.
        return false;
    }

    if let Some(layer_paint) = layer_paint {
        let layer_color = layer_paint.color();
        // The layer paint color must have only alpha component.
        if crate::color::Color::TRANSPARENT != layer_color.with_a(ALPHA_TRANSPARENT) {
            return false;
        }

        // The layer paint can not have any effects.
        if layer_paint.path_effect().is_some()
            || layer_paint.shader().is_some()
            || !layer_paint.is_src_over()
            || layer_paint.mask_filter().is_some()
            || layer_paint.color_filter().is_some()
            || layer_paint.image_filter().is_some()
        {
            return false;
        }
        // The product is at most 255 (both factors are <= 255), as SkMulDiv255Round.
        #[allow(clippy::cast_possible_truncation)] // mirrors the U8CPU result of SkMulDiv255Round
        let alpha = mul_div_255_round(u32::from(paint.alpha()), u32::from(layer_color.a())) as u8;
        paint.set_alpha(alpha);
    }

    true
}

// Turns logical no-op Save-[non-drawing command]*-Restore patterns into actual no-ops.
//
// Greedy matches greedily, so we also have to exclude Save and Restore.
// Nested SaveLayers need to be excluded, or we'll match their Restore!
// Port of: src/core/SkRecordOpts.cpp#L109-L127 (chrome/m156)
type SaveNoDrawsRestoreNooper = (
    Is<Save>,
    Greedy<Not<Or<(Is<Save>, Is<SaveLayer>, Is<Restore>, IsDraw)>>>,
    Is<Restore>,
);

fn save_no_draws_restore_nooper(
    record: &mut Record,
    _pattern: &Pattern<SaveNoDrawsRestoreNooper>,
    begin: usize,
    end: usize,
) -> bool {
    // The entire span between Save and Restore (inclusively) does nothing.
    for i in begin..end {
        record.replace(i, NoOp);
    }
    true
}

/// Turns logical no-op Save-[non-drawing command]*-Restore patterns into actual no-ops.
// Port of: src/core/SkRecordOpts.cpp#L128-L135 (chrome/m156)
#[doc(alias = "SkRecordNoopSaveRestores")]
pub fn record_noop_save_restores(record: &mut Record) {
    // Run until they stop changing things.
    while apply(record, save_only_draws_restore_nooper)
        || apply(record, save_no_draws_restore_nooper)
    {}
}

// Port of: src/core/SkRecordOpts.cpp#L138-L145 (chrome/m156)
fn effectively_srcover(paint: Option<&Paint>) -> bool {
    let Some(paint) = paint else {
        return true;
    };
    if paint.is_src_over() {
        return true;
    }
    // src-mode with opaque and no effects (which might change opaqueness) is ok too.
    paint.shader().is_none()
        && paint.color_filter().is_none()
        && paint.image_filter().is_none()
        && 0xFF == paint.alpha()
        && paint.as_blend_mode() == Some(BlendMode::Src)
}

// For some SaveLayer-[drawing command]-Restore patterns, merge the SaveLayer's alpha into the
// draw, and no-op the SaveLayer and Restore.
//
// Note that we use IsSingleDraw here, to avoid matching drawAtlas, drawVertices, etc...
// Those operations (can) draw multiple, overlapping primitives that blend with each other.
// Applying this operation to them changes their behavior. (skbug.com/40045501)
// Port of: src/core/SkRecordOpts.cpp#L147-L187 (chrome/m156)
type SaveLayerDrawRestoreNooper = (Is<SaveLayer>, IsSingleDraw, Is<Restore>);

fn save_layer_draw_restore_nooper(
    record: &mut Record,
    pattern: &Pattern<SaveLayerDrawRestoreNooper>,
    begin: usize,
    _end: usize,
) -> bool {
    let Some(save_layer) = pattern.first::<SaveLayer>(record) else {
        return false;
    };
    if save_layer.backdrop.is_some() {
        // can't throw away the layer if we have a backdrop
        return false;
    }

    if !save_layer.filters.is_empty() {
        // Our optimizations don't handle the filter list correctly - don't bother trying
        return false;
    }

    // A SaveLayer's bounds field is just a hint, so we should be free to ignore it.
    // (Skia mutates the draw's paint while reading the layer's; a copy of the layer paint is the
    // same read.)
    let layer_paint = save_layer.paint.clone();
    let draw_index = pattern.second_index().expect("the pattern matched");

    let kill = record.mutate(draw_index, |draw| {
        let draw_paint = draw.paint_mut();

        if layer_paint.is_none() && effectively_srcover(draw_paint.as_deref()) {
            // There wasn't really any point to this SaveLayer at all.
            return true;
        }

        let Some(draw_paint) = draw_paint else {
            // We can just give the draw the SaveLayer's paint.
            // TODO(mtklein): figure out how to do this clearly
            return false;
        };

        fold_opacity_layer_color_to_paint(
            layer_paint.as_ref(),
            false, /*isSaveLayer*/
            draw_paint,
        )
    });
    if !kill {
        return false;
    }

    kill_save_layer_and_restore(record, begin)
}

// Port of: src/core/SkRecordOpts.cpp#L179-L186 (chrome/m156)
fn kill_save_layer_and_restore(record: &mut Record, save_layer_index: usize) -> bool {
    record.replace(save_layer_index, NoOp); // SaveLayer
    record.replace(save_layer_index + 2, NoOp); // Restore
    true
}

/// For some `SaveLayer`-[drawing command]-`Restore` patterns, merge the `SaveLayer`'s alpha into
/// the draw, and no-op the `SaveLayer` and `Restore`.
// Port of: src/core/SkRecordOpts.cpp#L188-L191 (chrome/m156)
#[doc(alias = "SkRecordNoopSaveLayerDrawRestores")]
pub fn record_noop_save_layer_draw_restores(record: &mut Record) {
    apply(record, save_layer_draw_restore_nooper);
}

// For SVG generated:
//   SaveLayer (non-opaque, typically for CSS opacity)
//     Save
//       ClipRect
//       SaveLayer (typically for SVG filter)
//       Restore
//     Restore
//   Restore
// Port of: src/core/SkRecordOpts.cpp#L194-L246 (chrome/m156)
type SvgOpacityAndFilterLayerMergePass = (
    Is<SaveLayer>,
    Is<Save>,
    Is<ClipRect>,
    Is<SaveLayer>,
    Is<Restore>,
    Is<Restore>,
    Is<Restore>,
);

fn svg_opacity_and_filter_layer_merge_pass(
    record: &mut Record,
    pattern: &Pattern<SvgOpacityAndFilterLayerMergePass>,
    begin: usize,
    _end: usize,
) -> bool {
    let (Some(first), Some(fourth)) = (
        pattern.first::<SaveLayer>(record),
        pattern.fourth::<SaveLayer>(record),
    ) else {
        return false;
    };
    if first.backdrop.is_some() {
        // can't throw away the layer if we have a backdrop
        return false;
    }

    if !first.filters.is_empty() || !fourth.filters.is_empty() {
        // Our optimizations don't handle the filter list correctly - don't bother trying
        return false;
    }

    let Some(opacity_paint) = first.paint.clone() else {
        // There wasn't really any point to this SaveLayer at all.
        return kill_svg_save_layer_and_restore(record, begin);
    };

    // This layer typically contains a filter, but this should work for layers with for other
    // purposes too.
    let fourth_index = pattern.fourth_index().expect("the pattern matched");
    let folded = record.mutate(fourth_index, |command| {
        let Some(filter_layer_paint) =
            SaveLayer::from_command_mut(command).and_then(|s| s.paint.as_mut())
        else {
            // We can just give the inner SaveLayer the paint of the outer SaveLayer.
            // TODO(mtklein): figure out how to do this clearly
            return false;
        };

        fold_opacity_layer_color_to_paint(
            Some(&opacity_paint),
            true, /*isSaveLayer*/
            filter_layer_paint,
        )
    });
    if !folded {
        return false;
    }

    kill_svg_save_layer_and_restore(record, begin)
}

// Port of: src/core/SkRecordOpts.cpp#L240-L244 (chrome/m156)
fn kill_svg_save_layer_and_restore(record: &mut Record, save_layer_index: usize) -> bool {
    record.replace(save_layer_index, NoOp); // SaveLayer
    record.replace(save_layer_index + 6, NoOp); // Restore
    true
}

/// For SVG generated `SaveLayer`-`Save`-`ClipRect`-`SaveLayer`-3x`Restore` patterns, merge the
/// alpha of the first `SaveLayer` to the second `SaveLayer`.
// Port of: src/core/SkRecordOpts.cpp#L248-L251 (chrome/m156)
#[doc(alias = "SkRecordMergeSvgOpacityAndFilterLayers")]
pub fn record_merge_svg_opacity_and_filter_layers(record: &mut Record) {
    apply(record, svg_opacity_and_filter_layer_merge_pass);
}

///////////////////////////////////////////////////////////////////////////////////////////////////

/// Runs all optimizations in recommended order.
// Port of: src/core/SkRecordOpts.cpp#L255-L274 (chrome/m156)
#[doc(alias = "SkRecordOptimize")]
pub fn record_optimize(record: &mut Record) {
    // This might be useful  as a first pass in the future if we want to weed
    // out junk for other optimization passes.  Right now, nothing needs it,
    // and the bounding box hierarchy will do the work of skipping no-op
    // Save-NoDraw-Restore sequences better than we can here.
    // As there is a known problem with this peephole and drawAnnotation, disable this.
    // If we want to enable this we must first fix this bug:
    //     https://bugs.chromium.org/p/skia/issues/detail?id=5548
    //    record_noop_save_restores(record);

    // (Skia turns the next optimization off for the Android framework build only.)
    record_noop_save_layer_draw_restores(record);
    record_merge_svg_opacity_and_filter_layers(record);

    record.defrag();
}
