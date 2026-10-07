// Skia side of the raster pipeline builder tests (task A4, design §2.7): builds the pipelines of
// crates/skia-rust-core/src/raster_pipeline/tests.rs through SkRasterPipeline's appenders and
// prints, per case,
//   - `== <case>` followed by SkRasterPipeline::dump()'s output (SkDebugf), and
//   - the oracle's stage record for compile() (SKIA_ORACLE_RP_DUMP, see SkRasterPipeline.cpp):
//     `<case> <lowp|highp> <op> ...` and `# <index> <op> <ctx values>` lines.
// The Rust tests compare their builder against the two outputs, committed as
// crates/skia-rust-core/src/raster_pipeline/skia_dump.txt and skia_rp_dump.txt.
//
// With the argument `d2` it builds the D2 cases instead (see run_d2 below); with `d3` the
// raster pipeline blitter cases (run_d3).
//
// Build and run (Windows): oracle/rp-builder/build.ps1 builds and runs it against
// out/oracle/x64-sse2 with SKIA_ORACLE_CPU_CAP=baseline (lowp decisions are those of every tier
// with a lowp pipeline), and writes the two files.

#include "include/core/SkBlendMode.h"
#include "include/core/SkBlender.h"
#include "include/core/SkColorFilter.h"
#include "include/core/SkGraphics.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPixmap.h"
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
#include "src/core/SkBlitter.h"
#include "src/core/SkConvertPixels.h"
#include "src/core/SkCoreBlitters.h"
#include "src/core/SkMask.h"
#include "src/effects/colorfilters/SkColorFilterBase.h"
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
#include <string>
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

// ---- D3: SkRasterPipelineBlitter (`rp_builder.exe d3`) ---------------------------------------
//
// Builds blitters through SkCreateRasterPipelineBlitter for a matrix of blend modes x destination
// formats x paints (x clip shaders, color filters, blenders, dither) and drives each through the
// same blit calls. Output (SkDebugf):
//   `<case> null`                 SkCreateRasterPipelineBlitter returned nullptr, or
//   `<case> <step> <hash>`        the FNV-1a hash of the destination bytes after each step
//                                 (`init` first), and
//   `direct <case> <none|value>`  what canDirectBlit() returned.
// The oracle's pipeline records (SKIA_ORACLE_RP_DUMP) are tagged `<case>/<step>`; the ones made
// while creating the blitter are tagged `<case>/create`.
// Compared by crates/skia-rust-raster/src/raster_pipeline_blitter_tests.rs, which builds the same
// cases (the loops below and there must stay in step).

namespace {

// A non-constant, position dependent shader: seed_shader, the inverse of the CTM, then opaque
// (r, g = the transformed coordinates, b = 1), optionally scaled to a translucent premul color.
class RampShader final : public SkShaderBase {
public:
    explicit RampShader(bool translucent) : fTranslucent(translucent) {}
    bool isOpaque() const override { return !fTranslucent; }
    ShaderType type() const override { return ShaderType::kRuntime; }
    SK_FLATTENABLE_HOOKS(RampShader)

private:
    bool appendStages(const SkStageRec& rec, const SkShaders::MatrixRec& mrec) const override {
        if (!mrec.apply(rec, SkMatrix::I())) {
            return false;
        }
        rec.fPipeline->append(SkRasterPipelineOp::force_opaque);
        if (fTranslucent) {
            rec.fPipeline->append(SkRasterPipelineOp::scale_1_float, rec.fAlloc->make<float>(0.625f));
        }
        return true;
    }
    bool fTranslucent;
};
sk_sp<SkFlattenable> RampShader::CreateProc(SkReadBuffer&) { return nullptr; }

// kind 0: swap_rb (alpha unchanged); 1: scale_1_float 0.5 (alpha changes); 2: fails;
// 3: premul only when the shader is not opaque (alpha unchanged).
class TestColorFilter final : public SkColorFilterBase {
public:
    explicit TestColorFilter(int kind) : fKind(kind) {}
    Type type() const override { return Type::kNoop; }
    bool onIsAlphaUnchanged() const override { return fKind == 0 || fKind == 3; }
    SK_FLATTENABLE_HOOKS(TestColorFilter)

private:
    bool appendStages(const SkStageRec& rec, bool shaderIsOpaque) const override {
        switch (fKind) {
            case 0: rec.fPipeline->append(SkRasterPipelineOp::swap_rb); return true;
            case 1:
                rec.fPipeline->append(SkRasterPipelineOp::scale_1_float,
                                      rec.fAlloc->make<float>(0.5f));
                return true;
            case 3:
                if (!shaderIsOpaque) {
                    rec.fPipeline->append(SkRasterPipelineOp::premul);
                }
                return true;
            default: return false;
        }
    }
    int fKind;
};
sk_sp<SkFlattenable> TestColorFilter::CreateProc(SkReadBuffer&) { return nullptr; }

// Not a blend mode: appends `multiply` (or fails).
class TestBlender final : public SkBlenderBase {
public:
    explicit TestBlender(bool fail) : fFail(fail) {}
    BlenderType type() const override { return BlenderType::kRuntime; }
    bool onAppendStages(const SkStageRec& rec) const override {
        if (fFail) {
            return false;
        }
        rec.fPipeline->append(SkRasterPipelineOp::multiply);
        return true;
    }
    SK_FLATTENABLE_HOOKS(TestBlender)

private:
    bool fFail;
};
sk_sp<SkFlattenable> TestBlender::CreateProc(SkReadBuffer&) { return nullptr; }

constexpr int kW = 40, kH = 6;

struct DstCfg {
    const char* name;
    SkColorType ct;
    SkAlphaType at;
    sk_sp<SkColorSpace> cs;
};

uint64_t fnv(const uint8_t* p, size_t n) {
    uint64_t h = 0xcbf29ce484222325ull;
    for (size_t i = 0; i < n; i++) {
        h = (h ^ p[i]) * 0x100000001b3ull;
    }
    return h;
}

// The destination: kW x kH with two pixels of row padding, filled from a deterministic F32 ramp.
struct Dst {
    std::vector<uint8_t> bytes;
    SkImageInfo info;
    size_t rowBytes;
    Dst(const DstCfg& cfg) {
        info = SkImageInfo::Make(kW, kH, cfg.ct, cfg.at, cfg.cs);
        rowBytes = (size_t)(kW + 2) * info.bytesPerPixel();
        bytes.assign(rowBytes * kH, 0);
        std::vector<float> src(kW * kH * 4);
        for (int y = 0; y < kH; y++) {
            for (int x = 0; x < kW; x++) {
                float* p = &src[(y * kW + x) * 4];
                p[0] = (float)((x * 7 + y * 3) % 16) / 15.0f;
                p[1] = (float)((x * 5 + y * 11) % 16) / 15.0f;
                p[2] = (float)((x * 3 + y * 13) % 16) / 15.0f;
                p[3] = cfg.at == kOpaque_SkAlphaType
                               ? 1.0f
                               : (float)((x * 13 + y * 5) % 255 + 1) / 255.0f;
            }
        }
        SkImageInfo srcInfo = SkImageInfo::Make(kW, kH, kRGBA_F32_SkColorType, kUnpremul_SkAlphaType,
                                                nullptr);
        SkOracleSetResultId("convert");
        (void)SkConvertPixels(info, bytes.data(), rowBytes, srcInfo, src.data(), kW * 16);
    }
    SkPixmap pixmap() { return SkPixmap(info, bytes.data(), rowBytes); }
};

// The blit calls every case is driven through.
void run_steps(const char* name, SkBlitter* b, const Dst& dst) {
    auto emit = [&](const char* step) {
        SkDebugf("%s %s %016llx\n", name, step,
                 (unsigned long long)fnv(dst.bytes.data(), dst.bytes.size()));
    };
    auto setid = [&](const char* step) {
        char id[256];
        snprintf(id, sizeof(id), "%s/%s", name, step);
        SkOracleSetResultId(id);
    };
    emit("init");

    setid("h");
    b->blitH(3, 1, 17);
    emit("h");

    setid("rect");
    b->blitRect(2, 2, 23, 3);
    emit("rect");

    {
        const struct { int n; SkAlpha a; } spans[] = {
                {3, 0x40}, {4, 0xff}, {2, 0x00}, {5, 0x80}, {1, 0x01}, {3, 0xfe}, {9, 0x30}};
        SkAlpha aa[28] = {};
        int16_t runs[28] = {};
        int i = 0;
        for (auto s : spans) {
            runs[i] = s.n;
            aa[i] = s.a;
            i += s.n;
        }
        runs[i] = 0;
        setid("antih");
        b->blitAntiH(1, 0, aa, runs);
        emit("antih");
    }
    {
        const struct { int n; SkAlpha a; } spans[] = {{1, 0x7f}, {16, 0x55}, {9, 0xff}};
        SkAlpha aa[27] = {};
        int16_t runs[27] = {};
        int i = 0;
        for (auto s : spans) {
            runs[i] = s.n;
            aa[i] = s.a;
            i += s.n;
        }
        runs[i] = 0;
        setid("antih2");
        b->blitAntiH(6, 5, aa, runs);
        emit("antih2");
    }

    setid("v");
    b->blitV(5, 0, 6, 0x90);
    emit("v");
    setid("v255");
    b->blitV(7, 1, 3, 0xff);
    emit("v255");

    setid("aa2");
    b->blitAntiH2(10, 4, 0x20, 0xe0);
    emit("aa2");
    setid("av2");
    b->blitAntiV2(12, 3, 0x77, 0x05);
    emit("av2");

    {
        uint8_t image[24 * 4];
        for (int i = 0; i < 24 * 4; i++) {
            image[i] = (uint8_t)(i * 37 + 11);
        }
        SkMask mask(image, SkIRect::MakeLTRB(4, 1, 27, 5), 24, SkMask::kA8_Format);
        setid("mask_a8");
        b->blitMask(mask, SkIRect::MakeLTRB(6, 2, 25, 5));
        emit("mask_a8");
    }
    {
        uint8_t image[40 * 3];
        for (int i = 0; i < 20 * 3; i++) {
            uint16_t v = (uint16_t)(i * 2579 + 101);
            image[2 * i] = (uint8_t)v;
            image[2 * i + 1] = (uint8_t)(v >> 8);
        }
        SkMask mask(image, SkIRect::MakeLTRB(2, 0, 20, 3), 40, SkMask::kLCD16_Format);
        setid("mask_lcd");
        b->blitMask(mask, SkIRect::MakeLTRB(3, 1, 19, 3));
        emit("mask_lcd");
    }
    {
        uint8_t image[16 * 3 * 3];
        for (int i = 0; i < 16 * 3 * 3; i++) {
            image[i] = (uint8_t)(i * 53 + 7);
        }
        SkMask mask(image, SkIRect::MakeLTRB(8, 2, 22, 5), 16, SkMask::k3D_Format);
        setid("mask_3d");
        b->blitMask(mask, SkIRect::MakeLTRB(9, 2, 21, 5));
        emit("mask_3d");
    }
    {
        uint8_t image[3 * 4];
        for (int i = 0; i < 12; i++) {
            image[i] = (uint8_t)(i * 91 + 0x5a);
        }
        SkMask mask(image, SkIRect::MakeLTRB(0, 0, 19, 4), 3, SkMask::kBW_Format);
        setid("mask_bw");
        b->blitMask(mask, SkIRect::MakeLTRB(1, 1, 17, 3));
        emit("mask_bw");
    }
}

SkPaint make_paint(int pk) {
    SkPaint paint;
    switch (pk) {
        case 0: paint.setColor4f({0.25f, 0.5f, 0.75f, 1.0f}, nullptr); break;
        case 1: paint.setColor4f({0.1f, 0.2f, 0.3f, 0.4f}, nullptr); break;
        case 2:
            paint.setColor4f({0, 0, 0, 1}, nullptr);
            paint.setShader(sk_make_sp<RampShader>(false));
            break;
        case 3:
            paint.setColor4f({0, 0, 0, 0.75f}, nullptr);
            paint.setShader(sk_make_sp<RampShader>(true));
            break;
    }
    return paint;
}
const char* const kPaintNames[] = {"const_opaque", "const_translucent", "ramp", "ramp_alpha"};

// CTM of the ramp shaders (its inverse is applied to the device coordinates).
SkMatrix ramp_ctm() { return SkMatrix::MakeAll(48.f, 6.f, -3.f, 0.f, 40.f, 2.f, 0, 0, 1); }

// Creates the blitter for `paint` over `dst` and runs the steps. `clipShader` may be null.
void run_case(const std::string& name, const DstCfg& cfg, const SkPaint& paint,
              sk_sp<SkShader> clipShader) {
    Dst dst(cfg);
    SkPixmap pm = dst.pixmap();
    SkArenaAlloc alloc(4096);
    SkSurfaceProps props{};
    SkOracleSetResultId((name + "/create").c_str());
    SkBlitter* b = SkCreateRasterPipelineBlitter(pm, paint, ramp_ctm(), &alloc, clipShader, props,
                                                 SkRect::MakeEmpty());
    if (!b) {
        SkDebugf("%s null\n", name.c_str());
        return;
    }
    run_steps(name.c_str(), b, dst);
}

void print_direct(const std::string& name, const DstCfg& cfg, const SkPaint& paint) {
    Dst dst(cfg);
    SkPixmap pm = dst.pixmap();
    SkArenaAlloc alloc(4096);
    SkSurfaceProps props{};
    SkBlitter* b = SkCreateRasterPipelineBlitter(pm, paint, ramp_ctm(), &alloc, nullptr, props,
                                                 SkRect::MakeEmpty());
    if (!b) {
        SkDebugf("direct %s null\n", name.c_str());
        return;
    }
    auto d = b->canDirectBlit();
    if (d.has_value()) {
        SkDebugf("direct %s %llx\n", name.c_str(), (unsigned long long)d->value);
    } else {
        SkDebugf("direct %s none\n", name.c_str());
    }
}

}  // namespace

static int run_d3() {
    sk_sp<SkColorSpace> srgb = SkColorSpace::MakeSRGB();
    sk_sp<SkColorSpace> linear = SkColorSpace::MakeSRGBLinear();

    // Matrix 1: every blend mode x three destinations x four paints.
    const DstCfg kDsts[] = {
            {"rgba8888", kRGBA_8888_SkColorType, kPremul_SkAlphaType, nullptr},
            {"bgra_srgb", kBGRA_8888_SkColorType, kPremul_SkAlphaType, srgb},
            {"f16_linear", kRGBA_F16_SkColorType, kPremul_SkAlphaType, linear},
    };
    for (int i = 0; i <= (int)SkBlendMode::kLastMode; i++) {
        SkBlendMode mode = (SkBlendMode)i;
        for (const DstCfg& dst : kDsts) {
            for (int pk = 0; pk < 4; pk++) {
                SkPaint paint = make_paint(pk);
                paint.setBlendMode(mode);
                run_case(std::string("m1_") + SkBlendMode_Name(mode) + "_" + dst.name + "_" +
                                 kPaintNames[pk],
                         dst, paint, nullptr);
            }
        }
    }

    // Matrix 2: every color type x alpha types x five paint/mode/dither configurations.
    struct P2 { const char* name; int pk; SkBlendMode mode; bool dither; };
    const P2 kP2[] = {
            {"opq_over", 0, SkBlendMode::kSrcOver, false},
            {"tr_over", 1, SkBlendMode::kSrcOver, false},
            {"tr_src", 1, SkBlendMode::kSrc, false},
            {"ramp_dither_over", 2, SkBlendMode::kSrcOver, true},
            {"rampad_dither_plus", 3, SkBlendMode::kPlus, true},
    };
    for (int ct = 1; ct <= kLastEnum_SkColorType; ct++) {
        std::vector<std::pair<const char*, SkAlphaType>> ats = {{"p", kPremul_SkAlphaType}};
        switch ((SkColorType)ct) {
            case kRGBA_8888_SkColorType:
            case kBGRA_8888_SkColorType:
            case kRGBA_F16_SkColorType:
            case kRGBA_F32_SkColorType:
            case kRGBA_1010102_SkColorType:
            case kBGRA_1010102_SkColorType:
            case kSRGBA_8888_SkColorType:
            case kARGB_4444_SkColorType:
            case kR16G16B16A16_unorm_SkColorType:
                ats.push_back({"u", kUnpremul_SkAlphaType});
                break;
            case kRGB_565_SkColorType:
            case kRGB_888x_SkColorType:
            case kGray_8_SkColorType:
            case kRGB_101010x_SkColorType:
                ats.push_back({"o", kOpaque_SkAlphaType});
                break;
            default: break;
        }
        for (auto [atName, at] : ats) {
            DstCfg dst = {"", (SkColorType)ct, at, nullptr};
            for (const P2& p : kP2) {
                SkPaint paint = make_paint(p.pk);
                paint.setBlendMode(p.mode);
                paint.setDither(p.dither);
                run_case("m2_ct" + std::to_string(ct) + "_" + atName + "_" + p.name, dst, paint,
                         nullptr);
            }
        }
    }

    // Matrix 3: clip shaders, color filters, blenders, failures, pre-baked pipelines.
    const DstCfg kSpecialDsts[] = {
            {"rgba8888", kRGBA_8888_SkColorType, kPremul_SkAlphaType, nullptr},
            {"bgra8888", kBGRA_8888_SkColorType, kPremul_SkAlphaType, nullptr},
            {"f16_linear", kRGBA_F16_SkColorType, kPremul_SkAlphaType, linear},
    };
    sk_sp<SkShader> clipConst = SkShaders::Color(SkColor4f{0, 0, 0, 0.5f}, nullptr);
    sk_sp<SkShader> clipRamp = sk_make_sp<RampShader>(true);
    for (const DstCfg& dst : kSpecialDsts) {
        auto sp = [&](const std::string& n) { return "sp_" + n + "_" + dst.name; };
        auto with_mode = [&](int pk, SkBlendMode mode) {
            SkPaint p = make_paint(pk);
            p.setBlendMode(mode);
            return p;
        };
        run_case(sp("clip_const_over"), dst, with_mode(1, SkBlendMode::kSrcOver), clipConst);
        run_case(sp("clip_ramp_over"), dst, with_mode(2, SkBlendMode::kSrcOver), clipRamp);
        run_case(sp("clip_ramp_opq_src"), dst, with_mode(0, SkBlendMode::kSrc), clipRamp);
        run_case(sp("clip_ramp_rampad_plus"), dst, with_mode(3, SkBlendMode::kPlus), clipRamp);
        run_case(sp("clip_const_multiply"), dst, with_mode(1, SkBlendMode::kMultiply), clipConst);
        run_case(sp("clip_ramp_clear"), dst, with_mode(0, SkBlendMode::kClear), clipRamp);
        run_case(sp("clip_ramp_dst"), dst, with_mode(2, SkBlendMode::kDst), clipRamp);
        run_case(sp("clip_empty"), dst, with_mode(1, SkBlendMode::kSrcOver), SkShaders::Empty());

        for (int kind = 0; kind < 4; kind++) {
            for (int pk : {0, 2, 3}) {
                SkPaint p = make_paint(pk);
                p.setColorFilter(sk_make_sp<TestColorFilter>(kind));
                run_case(sp("cf" + std::to_string(kind) + "_" + kPaintNames[pk]), dst, p, nullptr);
            }
        }

        for (int pk : {1, 2}) {
            SkPaint p = make_paint(pk);
            p.setBlender(sk_make_sp<TestBlender>(false));
            run_case(sp(std::string("blender_") + kPaintNames[pk]), dst, p, nullptr);
            p.setBlender(sk_make_sp<TestBlender>(true));
            run_case(sp(std::string("blender_fail_") + kPaintNames[pk]), dst, p, nullptr);
        }

        {
            SkPaint p = make_paint(0);
            p.setShader(SkShaders::Empty());
            run_case(sp("empty_shader"), dst, p, nullptr);
        }

        // Pre-baked shader pipelines (sprites, vertices, atlases).
        for (int variant = 0; variant < 4; variant++) {
            for (SkBlendMode mode : {SkBlendMode::kSrcOver, SkBlendMode::kSrc,
                                     SkBlendMode::kModulate}) {
                Dst d(dst);
                SkPixmap pm = d.pixmap();
                SkArenaAlloc alloc(4096);
                SkRasterPipeline shaderPipeline(&alloc);
                shaderPipeline.append(SkRasterPipelineOp::seed_shader);
                shaderPipeline.append(SkRasterPipelineOp::force_opaque);
                bool opaque = true;
                if (variant & 1) {
                    shaderPipeline.append(SkRasterPipelineOp::scale_1_float,
                                          alloc.make<float>(0.5f));
                    opaque = false;
                }
                SkPaint p = make_paint(1);
                p.setBlendMode(mode);
                p.setDither((variant & 2) != 0);
                std::string name =
                        sp("pipeline" + std::to_string(variant) + "_" + SkBlendMode_Name(mode));
                SkOracleSetResultId((name + "/create").c_str());
                SkBlitter* b = SkCreateRasterPipelineBlitter(
                        pm, p, shaderPipeline, opaque, &alloc, (variant & 2) ? clipConst : nullptr);
                if (b) {
                    run_steps(name.c_str(), b, d);
                } else {
                    SkDebugf("%s null\n", name.c_str());
                }
            }
        }
    }

    // canDirectBlit.
    const DstCfg kDirectDsts[] = {
            {"rgba8888", kRGBA_8888_SkColorType, kPremul_SkAlphaType, nullptr},
            {"bgra_unpremul", kBGRA_8888_SkColorType, kUnpremul_SkAlphaType, nullptr},
            {"rgb565", kRGB_565_SkColorType, kOpaque_SkAlphaType, nullptr},
            {"f16", kRGBA_F16_SkColorType, kPremul_SkAlphaType, nullptr},
            {"gray8", kGray_8_SkColorType, kOpaque_SkAlphaType, nullptr},
            {"f32", kRGBA_F32_SkColorType, kPremul_SkAlphaType, nullptr},
            {"alpha8", kAlpha_8_SkColorType, kPremul_SkAlphaType, nullptr},
            {"rgba1010102", kRGBA_1010102_SkColorType, kPremul_SkAlphaType, nullptr},
            {"rgba8888_srgb", kRGBA_8888_SkColorType, kPremul_SkAlphaType, srgb},
    };
    for (const DstCfg& dst : kDirectDsts) {
        auto direct = [&](const char* n, const SkPaint& p) {
            print_direct(std::string(dst.name) + "_" + n, dst, p);
        };
        direct("opq_over", make_paint(0));
        direct("tr_over", make_paint(1));
        {
            SkPaint p = make_paint(1);
            p.setBlendMode(SkBlendMode::kSrc);
            direct("tr_src", p);
        }
        {
            SkPaint p = make_paint(0);
            p.setBlendMode(SkBlendMode::kClear);
            direct("opq_clear", p);
        }
        {
            SkPaint p = make_paint(0);
            p.setBlendMode(SkBlendMode::kPlus);
            direct("opq_plus", p);
        }
        direct("ramp", make_paint(2));
        {
            SkPaint p = make_paint(0);
            p.setDither(true);
            direct("opq_dither", p);
        }
        {
            SkPaint p = make_paint(0);
            p.setColorFilter(sk_make_sp<TestColorFilter>(0));
            direct("opq_cf", p);
        }
        {
            SkPaint p = make_paint(0);
            p.setBlender(sk_make_sp<TestBlender>(false));
            direct("opq_blender", p);
        }
    }
    return 0;
}

int main(int argc, char** argv) {
    // Caches the runtime CPU features (honoring SKIA_ORACLE_CPU_CAP) before the SkOpts tables are
    // chosen; build.ps1 runs the pipeline dumps with the baseline cap.
    SkGraphics::Init();
    if (argc > 1 && strcmp(argv[1], "d2") == 0) {
        return run_d2();
    }
    if (argc > 1 && strcmp(argv[1], "d3") == 0) {
        return run_d3();
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
