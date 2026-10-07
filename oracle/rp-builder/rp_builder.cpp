// Skia side of the raster pipeline builder tests (task A4, design §2.7): builds the pipelines of
// crates/skia-rust-core/src/raster_pipeline/tests.rs through SkRasterPipeline's appenders and
// prints, per case,
//   - `== <case>` followed by SkRasterPipeline::dump()'s output (SkDebugf), and
//   - the oracle's stage record for compile() (SKIA_ORACLE_RP_DUMP, see SkRasterPipeline.cpp):
//     `<case> <lowp|highp> <op> ...` and `# <index> <op> <ctx values>` lines.
// The Rust tests compare their builder against the two outputs, committed as
// crates/skia-rust-core/src/raster_pipeline/skia_dump.txt and skia_rp_dump.txt.
//
// With the argument `d2` it builds the D2 cases instead (see run_d2 below).
//
// Build and run (Windows): oracle/rp-builder/build.ps1 builds and runs it against
// out/oracle/x64-sse2 with SKIA_ORACLE_CPU_CAP=baseline (lowp decisions are those of every tier
// with a lowp pipeline), and writes the two files.

#include "include/core/SkBlendMode.h"
#include "include/core/SkBlender.h"
#include "include/core/SkColorSpace.h"
#include "include/core/SkColorType.h"
#include "include/core/SkImageInfo.h"
#include "include/core/SkMatrix.h"
#include "include/core/SkShader.h"
#include "include/core/SkSurfaceProps.h"
#include "include/core/SkTypes.h"
#include "modules/skcms/skcms.h"
#include "src/core/SkArenaAlloc.h"
#include "src/core/SkBlenderBase.h"
#include "src/core/SkEffectPriv.h"
#include "src/core/SkOpts.h"
#include "src/core/SkRasterPipeline.h"
#include "src/core/SkRasterPipelineOpContexts.h"
#include "src/core/SkRasterPipelineOpList.h"
#include "src/shaders/SkShaderBase.h"

#include <algorithm>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <functional>
#include <vector>

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

// ---- D2: blenders, color/empty shaders, MatrixRec (`rp_builder.exe d2`) ----------------------
//
// Per case: `== <case>`, `ok <0|1>` (the appender's result), dump()'s output, then
// `# <index> <op> <values>` lines for the uniform_color / unbounded_uniform_color / matrix_*
// contexts (all of them, unlike the oracle's record, which skips unbounded_uniform_color).
// Compared by crates/skia-rust-core/src/raster_pipeline/d2_tests.rs.

static void print_floats(const float* f, int n) {
    for (int i = 0; i < n; i++) {
        SkDebugf(" %.9g", f[i]);
    }
}

static void print_contexts(const SkRasterPipeline& p) {
    std::vector<const SkRasterPipeline::StageList*> stages;
    for (auto st = p.getStageList(); st; st = st->prev) {
        stages.push_back(st);
    }
    std::reverse(stages.begin(), stages.end());
    for (size_t i = 0; i < stages.size(); i++) {
        const auto* st = stages[i];
        switch (st->stage) {
            case Op::uniform_color: {
                auto* u = (const SkRasterPipelineContexts::UniformColorCtx*)st->ctx;
                SkDebugf("# %d %s", (int)i, SkRasterPipeline::GetOpName(st->stage));
                print_floats(&u->r, 4);
                SkDebugf(" | %u %u %u %u\n", u->rgba[0], u->rgba[1], u->rgba[2], u->rgba[3]);
                break;
            }
            case Op::unbounded_uniform_color: {
                // (rgba is left uninitialized by appendConstantColor for this op.)
                auto* u = (const SkRasterPipelineContexts::UniformColorCtx*)st->ctx;
                SkDebugf("# %d %s", (int)i, SkRasterPipeline::GetOpName(st->stage));
                print_floats(&u->r, 4);
                SkDebugf("\n");
                break;
            }
            case Op::matrix_translate:
                SkDebugf("# %d %s", (int)i, SkRasterPipeline::GetOpName(st->stage));
                print_floats((const float*)st->ctx, 2);
                SkDebugf("\n");
                break;
            case Op::matrix_scale_translate:
                SkDebugf("# %d %s", (int)i, SkRasterPipeline::GetOpName(st->stage));
                print_floats((const float*)st->ctx, 4);
                SkDebugf("\n");
                break;
            case Op::matrix_2x3:
                SkDebugf("# %d %s", (int)i, SkRasterPipeline::GetOpName(st->stage));
                print_floats((const float*)st->ctx, 6);
                SkDebugf("\n");
                break;
            case Op::matrix_perspective:
                SkDebugf("# %d %s", (int)i, SkRasterPipeline::GetOpName(st->stage));
                print_floats((const float*)st->ctx, 9);
                SkDebugf("\n");
                break;
            default:
                break;
        }
    }
}

using BuildRec = std::function<bool(const SkStageRec&)>;

static void run_d2_case(const char* name, SkColorSpace* dstCS, const BuildRec& build) {
    SkArenaAlloc alloc(1024);
    SkRasterPipeline p(&alloc);
    SkSurfaceProps props{};
    SkStageRec rec = {&p, &alloc, kRGBA_8888_SkColorType, dstCS,
                      SkColor4f{0, 0, 0, 1}, props, SkRect::MakeEmpty()};
    bool ok = build(rec);
    SkDebugf("== %s\nok %d\n", name, ok ? 1 : 0);
    p.dump();
    print_contexts(p);
    SkOracleSetResultId(name);
    (void)p.compile();
}

static int run_d2() {
    // Every blend mode through its blender, between a dst load and a store.
    for (int i = 0; i <= (int)SkBlendMode::kLastMode; i++) {
        SkBlendMode mode = (SkBlendMode)i;
        char name[64];
        snprintf(name, sizeof(name), "blender_%s", SkBlendMode_Name(mode));
        run_d2_case(name, nullptr, [&](const SkStageRec& rec) {
            rec.fPipeline->appendLoadDst(kRGBA_8888_SkColorType, &gMem[1]);
            bool ok = as_BB(SkBlender::Mode(mode))->appendStages(rec);
            rec.fPipeline->appendStore(kRGBA_8888_SkColorType, &gMem[1]);
            return ok;
        });
    }

    // Color shaders: colors x source color spaces x destination color spaces.
    struct ColorCase { const char* name; SkColor4f c; };
    const ColorCase kColors[] = {
        {"opaque",      {0.25f, 0.5f, 0.75f, 1}},
        {"translucent", {0.1f, 0.2f, 0.3f, 0.4f}},
        {"white",       {1, 1, 1, 1}},
        {"black",       {0, 0, 0, 1}},
        {"transparent", {0, 0, 0, 0}},
        {"wide",        {1.5f, -0.25f, 0.5f, 0.8f}},
        {"alpha_over",  {0.5f, 0.5f, 0.5f, 2.0f}},
    };
    sk_sp<SkColorSpace> p3 = SkColorSpace::MakeRGB(SkNamedTransferFn::kSRGB,
                                                   SkNamedGamut::kDisplayP3);
    sk_sp<SkColorSpace> rec2020 = SkColorSpace::MakeRGB(SkNamedTransferFn::kRec2020,
                                                        SkNamedGamut::kRec2020);
    struct CSCase { const char* name; sk_sp<SkColorSpace> cs; };
    const CSCase kSrcs[] = {
        {"null", nullptr},
        {"linear", SkColorSpace::MakeSRGBLinear()},
        {"p3", p3},
    };
    const CSCase kDsts[] = {
        {"null", nullptr},
        {"srgb", SkColorSpace::MakeSRGB()},
        {"linear", SkColorSpace::MakeSRGBLinear()},
        {"rec2020", rec2020},
    };
    for (const ColorCase& c : kColors) {
        for (const CSCase& src : kSrcs) {
            for (const CSCase& dst : kDsts) {
                char name[128];
                snprintf(name, sizeof(name), "color_shader_%s_%s_%s", c.name, src.name, dst.name);
                sk_sp<SkShader> shader = SkShaders::Color(c.c, src.cs);
                run_d2_case(name, dst.cs.get(), [&](const SkStageRec& rec) {
                    return as_SB(shader)->appendRootStages(rec, SkMatrix::Translate(3, 4));
                });
            }
        }
    }
    run_d2_case("color_shader_skcolor", nullptr, [](const SkStageRec& rec) {
        return as_SB(SkShaders::Color(0x80FF8040))->appendRootStages(rec, SkMatrix::I());
    });
    run_d2_case("color_shader_skcolor_linear", SkColorSpace::MakeSRGBLinear().get(),
                [](const SkStageRec& rec) {
        return as_SB(SkShaders::Color(0x80FF8040))->appendRootStages(rec, SkMatrix::I());
    });
    run_d2_case("empty_shader", nullptr, [](const SkStageRec& rec) {
        return as_SB(SkShaders::Empty())->appendRootStages(rec, SkMatrix::I());
    });

    // MatrixRec::apply: ctm x pending local matrix x post-inverse.
    struct MatrixCase { const char* name; SkMatrix ctm, lm, postInv; bool ctmApplied; };
    const MatrixCase kMatrices[] = {
        {"mrec_identity", SkMatrix::I(), SkMatrix::I(), SkMatrix::I(), false},
        {"mrec_ctm_translate", SkMatrix::Translate(3, -4.5f), SkMatrix::I(), SkMatrix::I(), false},
        {"mrec_scale_lm_translate", SkMatrix::Scale(2, 4), SkMatrix::Translate(1, 1),
         SkMatrix::I(), false},
        {"mrec_affine", SkMatrix::MakeAll(0.75f, -0.5f, 10, 0.5f, 0.75f, -20, 0, 0, 1),
         SkMatrix::Scale(3, 3), SkMatrix::I(), false},
        {"mrec_perspective", SkMatrix::MakeAll(1, 0.25f, 3, 0.5f, 2, 4, 0.001f, -0.002f, 1),
         SkMatrix::I(), SkMatrix::I(), false},
        {"mrec_post_inv", SkMatrix::Scale(2, 2), SkMatrix::I(), SkMatrix::Translate(5, 6),
         false},
        {"mrec_ctm_applied", SkMatrix::Scale(2, 2), SkMatrix::Translate(7, 8), SkMatrix::I(),
         true},
        {"mrec_singular", SkMatrix::Scale(0, 1), SkMatrix::I(), SkMatrix::I(), false},
    };
    for (const MatrixCase& c : kMatrices) {
        run_d2_case(c.name, nullptr, [&](const SkStageRec& rec) {
            SkShaders::MatrixRec m(c.ctm);
            if (c.ctmApplied) {
                m.markCTMApplied();
            }
            return m.concat(c.lm).apply(rec, c.postInv).has_value();
        });
    }
    return 0;
}

int main(int argc, char** argv) {
    SkOpts::Init();
    if (argc > 1 && strcmp(argv[1], "d2") == 0) {
        return run_d2();
    }

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
