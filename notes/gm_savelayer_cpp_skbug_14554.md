# gm/savelayer.cpp::skbug_14554

Status: failing (ported 1:1 in tests/gm/src/gm/savelayer.rs, `#[ignore]`d).

What was tried: one full port of `skbug_14554` (draw_atlas, draw_vertices, draw_points,
draw_image_set, each recorded with `saveLayerAlphaf(nullptr, 0.6f)` into an `SkPictureRecorder`
picture, played back with `drawPicture`, with and without the injected `translate(1, 0)`).

Mismatch: all 45 checks (8888 and 565, every tier) differ from the golden. The diff image
(target/gm-diffs/cpu-x64-scalar-rgba/8888/skbug_14554.png) is magenta over the atlas, vertices
and image-set cells, and dimmed over the points cells.

Hypothesis (not verified): the picture-playback alpha push-down of saveLayer(alpha) around a
single draw (SkRecordOpts `SaveLayerDrawRestoreNooper`, ported in crates/skia-rust-core/src/
record_opts.rs) gives a different result for the drawAtlas / drawVertices / drawPoints /
experimental_DrawEdgeAAImageSet ops than Skia m156. The GM exists to pin that behaviour, so the
first divergence is most likely the non-injected column of the first row (drawAtlas), comparing
the recorded op list against Skia's `SkRecordOpts.cpp` checks (`effectively_srcover`, the
per-op "touches each pixel once" rule) for each op type.

Next step: dump the record ops for the drawAtlas cell and compare with SkRecordOpts.cpp
before changing any code.
