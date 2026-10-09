# gm/rsxtext.cpp::RSXShaderGM (rsx_blob_shader): failing

Attempt 1: ported rsxtext.cpp 1:1 (RSX text blob via alloc_run_rsxform, four draw_one
cases with local matrices, make_shader with Repeat/Linear image shader + with_local_matrix).
Result: every 8888/565/f16 tier mismatches.

Diff image (cpu-x64-scalar-rgba, 8888): the yellow/green grids line up with the golden
(shader and local-matrix path look correct). The magenta difference is confined to the
"TEST" glyph pixels, so the divergence is in RSX glyph rendering (or the Sans ExtraBlack
typeface selection), not in the shader.

Next step: rp-dump of the RSX text glyph run (first divergent stage), and check which
typeface the portable manager returns for FontStyle(EXTRA_BLACK, NORMAL, Upright).
