// Skia side of the raster pipeline builder tests (task A4, design §2.7): builds the pipelines of
// crates/skia-rust-core/src/raster_pipeline/tests.rs through SkRasterPipeline's appenders and
// prints, per case,
//   - `== <case>` followed by SkRasterPipeline::dump()'s output (SkDebugf), and
//   - the oracle's stage record for compile() (SKIA_ORACLE_RP_DUMP, see SkRasterPipeline.cpp):
//     `<case> <lowp|highp> <op> ...` and `# <index> <op> <ctx values>` lines.
// The Rust tests compare their builder against the two outputs, committed as
// crates/skia-rust-core/src/raster_pipeline/skia_dump.txt and skia_rp_dump.txt.
//
// Build and run (Windows): oracle/rp-builder/build.ps1 builds and runs it against
// out/oracle/x64-sse2 with SKIA_ORACLE_CPU_CAP=baseline (lowp decisions are those of every tier
// with a lowp pipeline), and writes the two files.

#include "include/core/SkColorType.h"
#include "include/core/SkImageInfo.h"
#include "include/core/SkMatrix.h"
#include "include/core/SkTypes.h"
#include "modules/skcms/skcms.h"
#include "src/core/SkArenaAlloc.h"
#include "src/core/SkOpts.h"
#include "src/core/SkRasterPipeline.h"
#include "src/core/SkRasterPipelineOpContexts.h"
#include "src/core/SkRasterPipelineOpList.h"

#include <cstdio>
#include <cstdlib>
#include <functional>

// Defined in src/core/SkRasterPipeline.cpp (no header declares it).
void SkOracleSetResultId(const char* id);

using Op = SkRasterPipelineOp;
using Build = std::function<void(SkRasterPipeline&, SkArenaAlloc*)>;

static SkRasterPipelineContexts::MemoryCtx gMem[4];

static void run_case(const char* name, const Build& build) {
    SkArenaAlloc alloc(1024);
    SkRasterPipeline p(&alloc);
    build(p, &alloc);
    SkDebugf("== %s\n", name);
    p.dump();
    SkOracleSetResultId(name);
    (void)p.compile();
}

int main() {
    SkOpts::Init();

    struct ColorCase { const char* name; float rgba[4]; };
    static const ColorCase kColors[] = {
        {"constant_black",        {0, 0, 0, 1}},
        {"constant_white",        {1, 1, 1, 1}},
        {"constant_neg_zero",     {-0.0f, -0.0f, -0.0f, 1}},
        {"constant_opaque",       {0.25f, 0.5f, 0.75f, 1}},
        {"constant_rounding",     {0.1f, 0.2f, 0.3f, 0.4f}},
        {"constant_half",         {0.5f / 255, 1.5f / 255, 254.5f / 255, 1}},
        {"constant_transparent",  {0, 0, 0, 0}},
        {"constant_unpremul",     {0.6f, 0, 0, 0.5f}},
        {"constant_out_of_range", {1.5f, -0.25f, 0, 1}},
    };
    for (const ColorCase& c : kColors) {
        run_case(c.name, [&](SkRasterPipeline& p, SkArenaAlloc* a) {
            p.appendConstantColor(a, c.rgba);
        });
    }
    run_case("constant_color4f", [](SkRasterPipeline& p, SkArenaAlloc* a) {
        p.appendConstantColor(a, SkColor4f{0.125f, 0.25f, 0.375f, 0.5f});
    });

    struct RGBCase { const char* name; float rgb[3]; };
    static const RGBCase kRGBs[] = {
        {"set_rgb_in_range", {0.5f, 0.25f, 1}},
        {"set_rgb_zero",     {0, -0.0f, 0}},
        {"set_rgb_negative", {-0.1f, 0, 0}},
        {"set_rgb_over",     {0, 0, 1.0001f}},
    };
    for (const RGBCase& c : kRGBs) {
        run_case(c.name, [&](SkRasterPipeline& p, SkArenaAlloc* a) {
            p.appendSetRGB(a, c.rgb);
        });
    }
    run_case("set_rgb_color4f", [](SkRasterPipeline& p, SkArenaAlloc* a) {
        p.appendSetRGB(a, SkColor4f{0.75f, 0.5f, 0.25f, 7});
    });

    struct MatrixCase { const char* name; SkMatrix m; };
    const MatrixCase kMatrices[] = {
        {"matrix_identity",        SkMatrix::I()},
        {"matrix_translate",       SkMatrix::Translate(3, -4.5f)},
        {"matrix_scale",           SkMatrix::Scale(2, 3)},
        {"matrix_scale_translate", SkMatrix::MakeAll(2, 0, 5, 0, -3, 7, 0, 0, 1)},
        {"matrix_skew",            SkMatrix::Skew(0.5f, 0)},
        {"matrix_affine",          SkMatrix::MakeAll(0.75f, -0.5f, 10, 0.5f, 0.75f, -20, 0, 0, 1)},
        {"matrix_perspective",     SkMatrix::MakeAll(1, 0.25f, 3, 0.5f, 2, 4, 0.001f, -0.002f, 1)},
        {"matrix_persp_scale",     SkMatrix::MakeAll(2, 0, 0, 0, 2, 0, 0, 0, 2)},
    };
    for (const MatrixCase& c : kMatrices) {
        run_case(c.name, [&](SkRasterPipeline& p, SkArenaAlloc* a) {
            p.appendMatrix(a, c.m);
            p.append(Op::seed_shader);
        });
    }

    // Every color type except kUnknown, through appendLoad, appendLoadDst, appendStore.
    for (int i = 1; i <= kLastEnum_SkColorType; i++) {
        SkColorType ct = (SkColorType)i;
        char name[64];
        snprintf(name, sizeof(name), "load_store_ct%d", i);
        run_case(name, [&](SkRasterPipeline& p, SkArenaAlloc*) {
            p.appendLoad(ct, &gMem[0]);
            p.appendLoadDst(ct, &gMem[1]);
            p.append(Op::srcover);
            p.appendStore(ct, &gMem[1]);
        });
    }

    struct TFCase { const char* name; skcms_TransferFunction tf; };
    const TFCase kTFs[] = {
        {"tf_srgb",      *skcms_sRGB_TransferFunction()},
        {"tf_srgb_inv",  *skcms_sRGB_Inverse_TransferFunction()},
        {"tf_gamma",     {2.2f, 1, 0, 0, 0, 0, 0}},
        {"tf_linear",    {1, 1, 0, 0, 0, 0, 0}},
        {"tf_pqish",     {-2.0f, -107 / 128.0f, 1.0f, 32 / 2523.0f, 2413 / 128.0f,
                          -2392 / 128.0f, 8192 / 1305.0f}},
        {"tf_hlgish",    {-3.0f, 2.0f, 2.0f, 1 / 0.17883277f, 0.28466892f, 0.55991073f, 0.0f}},
        {"tf_hlginvish", {-4.0f, 2.0f, 2.0f, 1 / 0.17883277f, 0.28466892f, 0.55991073f, 0.0f}},
    };
    for (const TFCase& c : kTFs) {
        run_case(c.name, [&](SkRasterPipeline& p, SkArenaAlloc*) {
            p.append(Op::load_8888, &gMem[0]);
            p.appendTransferFunction(c.tf);
            p.append(Op::store_8888, &gMem[0]);
        });
    }

    for (int i = 1; i <= kLastEnum_SkColorType; i++) {
        SkColorType ct = (SkColorType)i;
        char name[64];
        snprintf(name, sizeof(name), "clamp_if_normalized_ct%d", i);
        run_case(name, [&](SkRasterPipeline& p, SkArenaAlloc*) {
            p.append(Op::seed_shader);
            p.appendClampIfNormalized(SkImageInfo::Make(1, 1, ct, kPremul_SkAlphaType));
        });
    }

    run_case("stack_rewind", [](SkRasterPipeline& p, SkArenaAlloc*) {
        p.append(Op::load_8888, &gMem[0]);
        p.appendStackRewind();
        p.append(Op::swap_rb);
        p.appendStackRewind();
        p.append(Op::store_8888, &gMem[0]);
    });

    run_case("extend", [](SkRasterPipeline& p, SkArenaAlloc* a) {
        SkRasterPipeline q(a);
        q.append(Op::load_565, &gMem[1]);
        q.append(Op::srcover);
        q.append(Op::store_565, &gMem[1]);
        SkRasterPipeline empty(a);
        p.append(Op::load_8888, &gMem[0]);
        p.extend(empty);
        p.extend(q);
        p.extend(q);
    });

    run_case("extend_rewind", [](SkRasterPipeline& p, SkArenaAlloc* a) {
        SkRasterPipeline q(a);
        q.append(Op::seed_shader);
        q.appendStackRewind();
        q.append(Op::store_8888, &gMem[0]);
        p.append(Op::black_color);
        p.extend(q);
    });

    run_case("highp_only_op", [](SkRasterPipeline& p, SkArenaAlloc*) {
        p.append(Op::load_8888, &gMem[0]);
        p.append(Op::unpremul);
        p.append(Op::store_8888, &gMem[0]);
    });

    run_case("empty", [](SkRasterPipeline&, SkArenaAlloc*) {});
    return 0;
}
