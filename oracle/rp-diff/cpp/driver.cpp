// oracle/rp-diff driver: runs raster-pipeline cases through real Skia, one tier at a time
// (docs/design/raster-pipeline.md §4.2).
//
//   rp_diff <tier> <cases.txt> <out.txt>       tier: scalar | sse2 | sse41 | ml3 | ml4
//
// The cases are written by `cargo xtask oracle rp-diff` from the Rust case list
// (oracle/rp-diff/src/cases.rs); the format is documented in oracle/rp-diff/src/case.rs. Each
// case is built with Skia's own SkRasterPipeline (linked from an oracle build's static libraries)
// after pointing SkOpts' stage tables at the requested tier's instantiation of
// SkRasterPipeline_opts.h (tier.cpp). For every case the driver writes one line,
// `<name> <hex>`, the bytes of all of the case's buffers after its runs, in slot order.

#include "include/core/SkTypes.h"
#include "src/core/SkArenaAlloc.h"
#include "src/core/SkCpu.h"
#include "src/core/SkOpts.h"
#include "src/core/SkRasterPipeline.h"
#include "src/core/SkRasterPipelineOpContexts.h"
#include "src/core/SkRasterPipelineOpList.h"

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

// Defined in src/core/SkRasterPipeline.cpp (no header declares it).
extern bool gForceHighPrecisionRasterPipeline;

void rpdiff_install_scalar();
void rpdiff_install_sse2();
void rpdiff_install_sse41();
void rpdiff_install_ml3();
void rpdiff_install_ml4();

namespace ctx = SkRasterPipelineContexts;
using Op = SkRasterPipelineOp;

struct OpName {
    const char* name;
    Op op;
};
static const OpName kOps[] = {
#define M(st) {#st, Op::st},
        SK_RASTER_PIPELINE_OPS_ALL(M)
#undef M
};

[[noreturn]] static void fail(const std::string& msg) {
    fprintf(stderr, "rp_diff: %s\n", msg.c_str());
    exit(1);
}

static Op op_by_name(const std::string& name) {
    for (const OpName& o : kOps) {
        if (name == o.name) {
            return o.op;
        }
    }
    fail("unknown op " + name);
}

static uint32_t hex_u32(const std::string& s) { return (uint32_t)strtoul(s.c_str(), nullptr, 16); }

static float hex_f32(const std::string& s) {
    uint32_t bits = hex_u32(s);
    float f;
    memcpy(&f, &bits, 4);
    return f;
}

static std::vector<uint8_t> unhex(const std::string& s, size_t len) {
    std::vector<uint8_t> out(len, 0);
    if (s == "-") {
        return out;
    }
    if (s.size() != 2 * len) {
        fail("buffer hex has the wrong length");
    }
    for (size_t i = 0; i < len; i++) {
        out[i] = (uint8_t)strtoul(s.substr(2 * i, 2).c_str(), nullptr, 16);
    }
    return out;
}

struct Buffer {
    std::vector<uint8_t> bytes;
    long long stride;
    long long origin;
};

struct Case {
    std::string name;
    bool forceHighp = false;
    bool compiled = false;
    std::vector<Buffer> buffers;
    std::vector<std::vector<std::string>> stages;  // tokens after "stage"
    std::vector<size_t> runs;                      // x y w h, flattened
};

static uint8_t* slot_ptr(Case& c, const std::string& slot, const std::string& offset) {
    size_t i = std::stoul(slot);
    if (i >= c.buffers.size()) {
        fail(c.name + ": no buffer " + slot);
    }
    return c.buffers[i].bytes.data() + std::stoul(offset);
}

// Builds the context of one stage (`stage <op> <kind> <args>`; see oracle/rp-diff/src/case.rs).
static void* make_ctx(Case& c, Op op, const std::vector<std::string>& t, SkArenaAlloc* alloc) {
    const std::string& kind = t.at(1);
    if (kind == "-") {
        return nullptr;
    }
    if (kind == "ptr") {
        return slot_ptr(c, t.at(2), t.at(3));
    }
    if (kind == "mem") {
        Buffer& b = c.buffers.at(std::stoul(t.at(2)));
        auto* m = alloc->make<ctx::MemoryCtx>();
        // `pixels` may point outside the buffer (Skia's "fake base" pointers); only the pixels
        // the case accesses must lie inside it.
        m->pixels = (void*)((uintptr_t)b.bytes.data() + (uintptr_t)(intptr_t)b.origin);
        m->stride = (int)b.stride;
        return m;
    }
    if (kind == "f32") {
        size_t n = std::stoul(t.at(2));
        float* f = alloc->makeArray<float>(n);
        for (size_t i = 0; i < n; i++) {
            f[i] = hex_f32(t.at(3 + i));
        }
        return f;
    }
    if (kind == "u8x4") {
        // Contexts packed into the pointer value itself (e.g. `swizzle`).
        uintptr_t v = 0;
        uint8_t bytes[4];
        for (int i = 0; i < 4; i++) {
            bytes[i] = (uint8_t)std::stoul(t.at(2 + i));
        }
        memcpy(&v, bytes, 4);
        void* p;
        memcpy(&p, &v, sizeof(p));
        return p;
    }
    if (kind == "branch") {
        // BranchIfAllLanesActiveCtx extends BranchCtx; the builder fills in its tail pointer.
        auto* b = alloc->make<ctx::BranchIfAllLanesActiveCtx>();
        b->offset = std::stoi(t.at(2));
        return b;
    }
    if (kind == "branch_eq") {
        auto* b = alloc->make<ctx::BranchIfEqualCtx>();
        b->offset = std::stoi(t.at(2));
        b->value = std::stoi(t.at(3));
        b->ptr = (const int*)slot_ptr(c, t.at(4), t.at(5));
        return b;
    }
    if (kind == "emboss") {
        auto* e = alloc->make<ctx::EmbossCtx>();
        for (int i = 0; i < 2; i++) {
            Buffer& b = c.buffers.at(std::stoul(t.at(2 + i)));
            ctx::MemoryCtx& m = i == 0 ? e->mul : e->add;
            m.pixels = (void*)((uintptr_t)b.bytes.data() + (uintptr_t)(intptr_t)b.origin);
            m.stride = (int)b.stride;
        }
        return e;
    }
    if (kind == "tables") {
        std::vector<uint8_t> bytes = unhex(t.at(2), 1024);
        auto* tb = alloc->make<ctx::TablesCtx>();
        uint8_t* data = alloc->makeArray<uint8_t>(1024);
        memcpy(data, bytes.data(), 1024);
        tb->r = data;
        tb->g = data + 256;
        tb->b = data + 512;
        tb->a = data + 768;
        return tb;
    }
    if (kind == "uniform_color") {
        auto* u = alloc->make<ctx::UniformColorCtx>();
        u->r = hex_f32(t.at(2));
        u->g = hex_f32(t.at(3));
        u->b = hex_f32(t.at(4));
        u->a = hex_f32(t.at(5));
        for (int i = 0; i < 4; i++) {
            u->rgba[i] = (uint16_t)std::stoul(t.at(6 + i));
        }
        return u;
    }
    fail(c.name + ": unknown context kind " + kind + " for " +
         SkRasterPipeline::GetOpName(op));
}

static void run_case(Case& c, FILE* out) {
    SkArenaAlloc alloc(4096);
    SkRasterPipeline p(&alloc);
    for (const auto& t : c.stages) {
        Op op = op_by_name(t.at(0));
        if (op == Op::stack_rewind) {
            p.appendStackRewind();  // the only way to append it (it owns the rewind context)
        } else {
            p.append(op, make_ctx(c, op, t, &alloc));
        }
    }
    gForceHighPrecisionRasterPipeline = c.forceHighp;
    if (c.compiled) {
        auto fn = p.compile();
        for (size_t i = 0; i < c.runs.size(); i += 4) {
            fn(c.runs[i], c.runs[i + 1], c.runs[i + 2], c.runs[i + 3]);
        }
    } else {
        for (size_t i = 0; i < c.runs.size(); i += 4) {
            p.run(c.runs[i], c.runs[i + 1], c.runs[i + 2], c.runs[i + 3]);
        }
    }
    gForceHighPrecisionRasterPipeline = false;

    fprintf(out, "%s ", c.name.c_str());
    for (const Buffer& b : c.buffers) {
        for (uint8_t byte : b.bytes) {
            fprintf(out, "%02x", byte);
        }
    }
    fprintf(out, "\n");
}

int main(int argc, char** argv) {
    if (argc != 4) {
        fail("usage: rp_diff <scalar|sse2|sse41|ml3|ml4> <cases.txt> <out.txt>");
    }
    SkCpu::CacheRuntimeFeatures();
    std::string tier = argv[1];
    if (tier == "scalar") {
        rpdiff_install_scalar();
    } else if (tier == "sse2") {
        rpdiff_install_sse2();
    } else if (tier == "sse41") {
        if (!SkCpu::Supports(SkX64::SSE41)) fail("this CPU has no SSE4.1");
        rpdiff_install_sse41();
    } else if (tier == "ml3") {
        if (!SkCpu::Supports(SkX64::ML3)) fail("this CPU cannot run ml3 (x86-64-v3)");
        rpdiff_install_ml3();
    } else if (tier == "ml4") {
        if (!SkCpu::Supports(SkX64::ML4)) fail("this CPU cannot run ml4 (AVX-512)");
        rpdiff_install_ml4();
    } else {
        fail("unknown tier " + tier);
    }

    std::ifstream in(argv[2]);
    if (!in) fail(std::string("cannot read ") + argv[2]);
    FILE* out = fopen(argv[3], "wb");
    if (!out) fail(std::string("cannot write ") + argv[3]);

    std::string line;
    Case c;
    bool inCase = false;
    size_t count = 0;
    while (std::getline(in, line)) {
        if (!line.empty() && line.back() == '\r') line.pop_back();
        std::istringstream ss(line);
        std::vector<std::string> t;
        for (std::string w; ss >> w;) t.push_back(w);
        if (t.empty() || t[0][0] == '#') continue;
        const std::string& k = t[0];
        if (k == "case") {
            c = Case();
            c.name = t.at(1);
            c.forceHighp = t.at(2) == "highp";
            c.compiled = t.at(3) == "compile";
            inCase = true;
        } else if (!inCase) {
            fail("`" + k + "` outside a case");
        } else if (k == "buf") {
            size_t len = std::stoul(t.at(1));
            c.buffers.push_back({unhex(t.at(4), len), std::stoll(t.at(2)), std::stoll(t.at(3))});
        } else if (k == "stage") {
            c.stages.emplace_back(t.begin() + 1, t.end());
        } else if (k == "run") {
            for (int i = 1; i <= 4; i++) c.runs.push_back(std::stoull(t.at(i)));
        } else if (k == "end") {
            run_case(c, out);
            inCase = false;
            count++;
        } else {
            fail("unknown line `" + k + "`");
        }
    }
    fclose(out);
    fprintf(stderr, "rp_diff %s: %zu cases (highp stride %zu, lowp stride %zu)\n", tier.c_str(),
            count, SkOpts::raster_pipeline_highp_stride, SkOpts::raster_pipeline_lowp_stride);
    return 0;
}
