// One raster-pipeline code path of Skia for oracle/rp-diff (docs/design/raster-pipeline.md §4.2).
//
// Compiled once per tier by `cargo xtask oracle rp-diff`, with the oracle's flags plus, per tier:
//   scalar: -DSKRP_CPU_SCALAR       sse2: (baseline)      sse41: /clang:-msse4.1
//   ml3:    /arch:AVX2              ml4:  /arch:AVX512
// and -DSK_OPTS_NS=rpdiff_<tier> -DRPDIFF_INSTALL=rpdiff_install_<tier>. Each instantiation of
// SkRasterPipeline_opts.h lives in its own namespace, exactly like src/opts/SkOpts_ml3.cpp, and
// RPDIFF_INSTALL() points SkOpts' tables at it, the same way SkOpts::Init_ml3() does.

#include "src/core/SkOpts.h"
#include "src/opts/SkRasterPipeline_opts.h"

#if !defined(RPDIFF_INSTALL)
    #error "define RPDIFF_INSTALL (and SK_OPTS_NS)"
#endif

void RPDIFF_INSTALL();
void RPDIFF_INSTALL() {
    // Same as src/opts/SkOpts_ml3.cpp's SkOpts::Init_ml3().
    SkOpts::raster_pipeline_lowp_stride  = SK_OPTS_NS::raster_pipeline_lowp_stride();
    SkOpts::raster_pipeline_highp_stride = SK_OPTS_NS::raster_pipeline_highp_stride();

#define M(st) SkOpts::ops_highp[(int)SkRasterPipelineOp::st] = (SkOpts::StageFn)SK_OPTS_NS::st;
    SK_RASTER_PIPELINE_OPS_ALL(M)
    SkOpts::just_return_highp = (SkOpts::StageFn)SK_OPTS_NS::just_return;
    SkOpts::start_pipeline_highp = SK_OPTS_NS::start_pipeline;
#undef M

#define M(st) SkOpts::ops_lowp[(int)SkRasterPipelineOp::st] = (SkOpts::StageFn)SK_OPTS_NS::lowp::st;
    SK_RASTER_PIPELINE_OPS_LOWP(M)
    SkOpts::just_return_lowp = (SkOpts::StageFn)SK_OPTS_NS::lowp::just_return;
    SkOpts::start_pipeline_lowp = SK_OPTS_NS::lowp::start_pipeline;
#undef M
}
