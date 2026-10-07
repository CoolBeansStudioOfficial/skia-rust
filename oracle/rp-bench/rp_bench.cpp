// Skia side of the raster pipeline interpreter micro-benchmark (design §2.6, R3). Times the
// same pipelines as crates/skia-rust-simd/examples/rp_bench.rs with SkRPBench.cpp's harness
// shape (`p.run(0, 0, 128, 1)` in a loop; `compile()` for the "compiled" column is not used, as
// SkRPBench doesn't either), best of 7 trials of ~50 ms.
//
// Build (Windows, from a shell where the oracle's clang-cl/lld-link are on PATH; see
// oracle/rp-bench/build.ps1): compile with the oracle's flags and link against an oracle build's
// static libraries, e.g. out/oracle/x64-sse2 (Sse2 baseline) or out/oracle/x64-sse41.
// Run with SKIA_ORACLE_CPU_CAP=baseline so SkOpts::Init does not upgrade to ml3/ml4, or
// =ml3/ml4 to time those tiers.

#include "include/core/SkTypes.h"
#include "src/core/SkArenaAlloc.h"
#include "src/core/SkOpts.h"
#include "src/core/SkRasterPipeline.h"
#include "src/core/SkRasterPipelineOpContexts.h"
#include "src/core/SkRasterPipelineOpList.h"

#include <chrono>
#include <cstdio>
#include <functional>

// Defined in src/core/SkRasterPipeline.cpp (no header declares it).
extern bool gForceHighPrecisionRasterPipeline;

static constexpr int W = 128;

static double time_ns(const std::function<void()>& f) {
    using clock = std::chrono::steady_clock;
    long loops = 1;
    for (;;) {
        auto t = clock::now();
        for (long i = 0; i < loops; i++) f();
        if (clock::now() - t > std::chrono::milliseconds(50)) break;
        loops *= 2;
    }
    double best = 1e300;
    for (int trial = 0; trial < 7; trial++) {
        auto t = clock::now();
        for (long i = 0; i < loops; i++) f();
        double ns = std::chrono::duration<double, std::nano>(clock::now() - t).count() / loops;
        if (ns < best) best = ns;
    }
    return best;
}

int main() {
    SkOpts::Init();
    alignas(64) float src[4 * 16];
    alignas(64) float dst[4 * 16];
    for (float& f : src) f = 0.5f;
    for (float& f : dst) f = 0.25f;

    struct Case {
        const char* name;
        std::function<void(SkRasterPipeline&)> build;
    };
    Case cases[] = {
            {"srcover (1 stage)",
             [](SkRasterPipeline& p) { p.append(SkRasterPipelineOp::srcover); }},
            {"seed_shader, store_src",
             [&](SkRasterPipeline& p) {
                 p.append(SkRasterPipelineOp::seed_shader);
                 p.append(SkRasterPipelineOp::store_src, dst);
             }},
            {"load_src, load_dst, srcover, store_dst",
             [&](SkRasterPipeline& p) {
                 p.append(SkRasterPipelineOp::load_src, src);
                 p.append(SkRasterPipelineOp::load_dst, dst);
                 p.append(SkRasterPipelineOp::srcover);
                 p.append(SkRasterPipelineOp::store_dst, dst);
             }},
    };
    printf("ns per run(0, 0, %d, 1), best of 7 (highp stride %zu, lowp stride %zu)\n", W,
           SkOpts::raster_pipeline_highp_stride, SkOpts::raster_pipeline_lowp_stride);
    for (const Case& c : cases) {
        printf("%s\n", c.name);
        for (bool forceHighp : {false, true}) {
            gForceHighPrecisionRasterPipeline = forceHighp;
            SkArenaAlloc alloc(1024);
            SkRasterPipeline p(&alloc);
            c.build(p);
            double runNs = time_ns([&] { p.run(0, 0, W, 1); });
            auto compiled = p.compile();
            double compiledNs = time_ns([&] { compiled(0, 0, W, 1); });
            printf("  %s run %8.1f ns  compiled %8.1f ns  (%.3f ns/px compiled)\n",
                   forceHighp ? "highp" : "lowp*", runNs, compiledNs, compiledNs / W);
        }
    }
    return 0;
}
