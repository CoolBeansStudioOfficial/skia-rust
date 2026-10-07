// Differential harness for skcms: prints a deterministic text report of transfer functions,
// matrices, ICC parsing, curve approximation and skcms_Transform over many pixel formats and
// profiles. The Rust twin (rust/src/main.rs, the portable port in crates/skia-rust-skcms)
// prints the same report; the two outputs must be byte-identical.
//
// Usage (clang, from this directory; SKIA is third_party/skia):
//   clang++ -std=c++17 -O2 -ffp-contract=off -DSKCMS_PORTABLE -DSKCMS_DISABLE_HSW //       -DSKCMS_DISABLE_SKX -DSKCMS_FORCE_BASELINE -I$SKIA/modules/skcms harness.cpp //       $SKIA/modules/skcms/skcms.cc $SKIA/modules/skcms/src/skcms_TransformBaseline.cc //       -o harness
//   ./harness $SKIA/resources > cpp.txt
//   (cd rust && cargo run --release -- $SKIA/resources > ../rust.txt)
//   diff cpp.txt rust.txt        # set FIRST=1 for both to also dump the first pixels
// (Without -DSKCMS_PORTABLE the C++ baseline runs 4 pixels at a time; the report is the same.)
#include "skcms.h"
#include "src/skcms_internals.h"

#include <algorithm>
#include <cstdint>
#include <cstdlib>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <vector>

namespace fs = std::filesystem;

static uint32_t rng_state = 1;
static uint32_t rnd() {
    uint32_t x = rng_state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    rng_state = x;
    return x;
}

static uint64_t fnv(const uint8_t* p, size_t n) {
    uint64_t h = 0xcbf29ce484222325ULL;
    for (size_t i = 0; i < n; i++) {
        h ^= p[i];
        h *= 0x100000001b3ULL;
    }
    return h;
}

static uint32_t fbits(float f) {
    uint32_t b;
    memcpy(&b, &f, 4);
    return b;
}

static size_t bpp(int fmt) {
    switch (fmt >> 1) {
        case 0: return 1;   // A_8
        case 1: return 1;   // G_8
        case 2: return 2;   // GA_88
        case 3: return 2;   // 565
        case 4: return 2;   // 4444
        case 5: return 3;   // 888
        case 6: return 4;   // 8888
        case 7: return 4;   // 8888_sRGB
        case 8: return 4;   // 1010102
        case 9: return 6;   // 161616LE
        case 10: return 8;  // 16161616LE
        case 11: return 6;  // 161616BE
        case 12: return 8;  // 16161616BE
        case 13: return 6;  // hhh_Norm
        case 14: return 8;  // hhhh_Norm
        case 15: return 6;  // hhh
        case 16: return 8;  // hhhh
        case 17: return 12; // fff
        case 18: return 16; // ffff
        case 19: return 4;  // 101010x_XR
        case 20: return 8;  // 10101010_XR
    }
    return 0;
}

static float rnd_float() {
    return (float)(rnd() % 1400) / 1000.0f - 0.2f;
}

static uint16_t half_from_float(float f) {
    uint32_t sem = fbits(f);
    uint32_t s = sem & 0x80000000u;
    uint32_t em = sem ^ s;
    if (em < 0x38800000u) {
        return (uint16_t)(s >> 16);
    }
    return (uint16_t)((s >> 16) + (em >> 13) - ((127 - 15) << 10));
}

static std::vector<uint8_t> make_pixels(int fmt, int n) {
    size_t b = bpp(fmt);
    std::vector<uint8_t> v(n * b + 8, 0);
    int kind = fmt >> 1;
    for (int i = 0; i < n; i++) {
        uint8_t* p = v.data() + i * b;
        if (kind == 17 || kind == 18) {
            float* f = (float*)p;
            for (size_t c = 0; c < b / 4; c++) {
                f[c] = rnd_float();
            }
        } else if (kind == 13 || kind == 14 || kind == 15 || kind == 16) {
            uint16_t* h = (uint16_t*)p;
            for (size_t c = 0; c < b / 2; c++) {
                h[c] = half_from_float(rnd_float());
            }
        } else {
            for (size_t c = 0; c < b; c++) {
                p[c] = (uint8_t)rnd();
            }
        }
    }
    return v;
}

static void dump_tf(const char* name, const skcms_TransferFunction& tf) {
    printf("%s %08x %08x %08x %08x %08x %08x %08x\n", name, fbits(tf.g), fbits(tf.a), fbits(tf.b),
           fbits(tf.c), fbits(tf.d), fbits(tf.e), fbits(tf.f));
}

static void dump_mat(const char* name, const skcms_Matrix3x3& m) {
    printf("%s", name);
    for (int r = 0; r < 3; r++)
        for (int c = 0; c < 3; c++) printf(" %08x", fbits(m.vals[r][c]));
    printf("\n");
}

static void dump_curve(const char* name, const skcms_Curve& c) {
    if (c.table_entries == 0) {
        dump_tf(name, c.parametric);
    } else {
        int w = c.table_8 ? 1 : 2;
        const uint8_t* t = c.table_8 ? c.table_8 : c.table_16;
        size_t nbytes = std::min<size_t>(8, (size_t)c.table_entries * w);
        printf("%s T%d %u", name, w * 8, c.table_entries);
        for (size_t i = 0; i < nbytes; i++) printf(" %02x", t[i]);
        printf("\n");
    }
}

static void dump_profile(const std::string& name, const skcms_ICCProfile& p) {
    printf("PROFILE %s size=%u dcs=%08x pcs=%08x tags=%u trc=%d xyz=%d a2b=%d b2a=%d cicp=%d hagc=%d\n",
           name.c_str(), p.size, p.data_color_space, p.pcs, p.tag_count, p.has_trc, p.has_toXYZD50,
           p.has_A2B, p.has_B2A, p.has_CICP, p.has_HAGC);
    if (p.has_trc) {
        for (int i = 0; i < 3; i++) dump_curve("  trc", p.trc[i]);
    }
    if (p.has_toXYZD50) dump_mat("  xyz", p.toXYZD50);
    if (p.has_A2B) {
        const skcms_A2B& a = p.A2B;
        printf("  a2b in=%u grid=%u,%u,%u,%u mc=%u out=%u g8=%d g16=%d\n", a.input_channels,
               a.grid_points[0], a.grid_points[1], a.grid_points[2], a.grid_points[3],
               a.matrix_channels, a.output_channels, a.grid_8 != nullptr, a.grid_16 != nullptr);
        for (uint32_t i = 0; i < a.input_channels; i++) dump_curve("  a2b.in", a.input_curves[i]);
        for (uint32_t i = 0; i < a.matrix_channels; i++) dump_curve("  a2b.m", a.matrix_curves[i]);
        for (uint32_t i = 0; i < a.output_channels; i++) dump_curve("  a2b.out", a.output_curves[i]);
        printf("  a2b.matrix");
        for (int r = 0; r < 3; r++)
            for (int c = 0; c < 4; c++) printf(" %08x", fbits(a.matrix.vals[r][c]));
        printf("\n");
        const uint8_t* g = a.grid_8 ? a.grid_8 : a.grid_16;
        if (g) {
            printf("  a2b.grid %02x %02x %02x %02x\n", g[0], g[1], g[2], g[3]);
        }
    }
    if (p.has_B2A) {
        const skcms_B2A& b = p.B2A;
        printf("  b2a in=%u mc=%u out=%u grid=%u,%u,%u,%u g8=%d g16=%d\n", b.input_channels,
               b.matrix_channels, b.output_channels, b.grid_points[0], b.grid_points[1],
               b.grid_points[2], b.grid_points[3], b.grid_8 != nullptr, b.grid_16 != nullptr);
        for (uint32_t i = 0; i < b.input_channels; i++) dump_curve("  b2a.in", b.input_curves[i]);
        for (uint32_t i = 0; i < b.matrix_channels; i++) dump_curve("  b2a.m", b.matrix_curves[i]);
        for (uint32_t i = 0; i < b.output_channels; i++) dump_curve("  b2a.out", b.output_curves[i]);
        printf("  b2a.matrix");
        for (int r = 0; r < 3; r++)
            for (int c = 0; c < 4; c++) printf(" %08x", fbits(b.matrix.vals[r][c]));
        printf("\n");
        const uint8_t* g = b.grid_8 ? b.grid_8 : b.grid_16;
        if (g) {
            printf("  b2a.grid %02x %02x %02x %02x\n", g[0], g[1], g[2], g[3]);
        }
    }
    if (p.has_CICP) {
        printf("  cicp %u %u %u %u\n", p.CICP.color_primaries, p.CICP.transfer_characteristics,
               p.CICP.matrix_coefficients, p.CICP.video_full_range_flag);
    }
    if (p.has_HAGC) printf("  hagc %u\n", p.HAGC.size);
    printf("  channels=%d\n", skcms_GetInputChannelCount(&p));
    skcms_Matrix3x3 chad;
    if (skcms_GetCHAD(&p, &chad)) dump_mat("  chad", chad);
    float wtpt[3];
    if (skcms_GetWTPT(&p, wtpt)) printf("  wtpt %08x %08x %08x\n", fbits(wtpt[0]), fbits(wtpt[1]), fbits(wtpt[2]));
}

static int g_case = 0;

static void run_transform(const char* tag, const skcms_ICCProfile* src, int srcFmt, int srcAlpha,
                          const skcms_ICCProfile* dst, int dstFmt, int dstAlpha, int n) {
    rng_state = 0x9e3779b9u ^ (uint32_t)(++g_case * 2654435761u);
    if (rng_state == 0) rng_state = 1;
    std::vector<uint8_t> in = make_pixels(srcFmt, n);
    std::vector<uint8_t> out(n * bpp(dstFmt) + 8, 0xAA);
    bool ok = skcms_Transform(in.data(), (skcms_PixelFormat)srcFmt, (skcms_AlphaFormat)srcAlpha,
                              src, out.data(), (skcms_PixelFormat)dstFmt,
                              (skcms_AlphaFormat)dstAlpha, dst, n);
    printf("T %s %d/%d -> %d/%d ok=%d hash=%016llx", tag, srcFmt, srcAlpha, dstFmt, dstAlpha, ok,
           (unsigned long long)(ok ? fnv(out.data(), n * bpp(dstFmt)) : 0));
    if (getenv("FIRST")) {
        printf(" in=");
        for (size_t k = 0; k < std::min<size_t>(bpp(srcFmt) * 2, in.size()); k++) printf("%02x", in[k]);
        printf(" out=");
        for (size_t k = 0; k < std::min<size_t>(bpp(dstFmt) * 3, out.size()); k++) printf("%02x", out[k]);
    }
    printf("\n");
}

static skcms_ICCProfile make_profile(const skcms_TransferFunction& tf, const skcms_Matrix3x3& m) {
    skcms_ICCProfile p;
    skcms_Init(&p);
    skcms_SetTransferFunction(&p, &tf);
    skcms_SetXYZD50(&p, &m);
    return p;
}

int main(int argc, char** argv) {
    std::string root = argv[1];
    // ---- transfer functions
    std::vector<std::pair<std::string, skcms_TransferFunction>> tfs;
    tfs.push_back({"srgb", *skcms_sRGB_TransferFunction()});
    tfs.push_back({"srgb_inv", *skcms_sRGB_Inverse_TransferFunction()});
    tfs.push_back({"identity", *skcms_Identity_TransferFunction()});
    tfs.push_back({"gamma22", {2.2f, 1, 0, 0, 0, 0, 0}});
    tfs.push_back({"gamma18", {1.8f, 1, 0, 0, 0, 0, 0}});
    tfs.push_back({"rec709ish", {2.22222f, 0.909672f, 0.0903276f, 0.222222f, 0.0812429f, 0, 0}});
    tfs.push_back({"smpte240", {2.222222222222f, 0.899626676224f, 0.100373323776f, 0.25f, 0.091286342118f, 0, 0}});
    tfs.push_back({"bad_neg_a", {2.0f, -1, 0, 0, 0, 0, 0}});
    {
        skcms_TransferFunction t;
        skcms_TransferFunction_makePQ(&t, 203.f);
        tfs.push_back({"pq203", t});
        skcms_TransferFunction_makePQ(&t, 100.f);
        tfs.push_back({"pq100", t});
        skcms_TransferFunction_makeHLG(&t, 203.f, 1000.f, 1.2f);
        tfs.push_back({"hlg203", t});
        skcms_TransferFunction_makeHLG(&t, 1.f, 12.f, 1.f);
        tfs.push_back({"hlg12", t});
        skcms_TransferFunction_makePQish(&t, -107 / 128.0f, 1.0f, 32 / 2523.0f, 2413 / 128.0f,
                                         -2392 / 128.0f, 8192 / 1305.0f);
        tfs.push_back({"pqish", t});
        skcms_TransferFunction_makeHLGish(&t, 2.0f, 2.0f, 1 / 0.17883277f, 0.28466892f, 0.55991073f);
        tfs.push_back({"hlgish", t});
        skcms_TransferFunction_makeScaledHLGish(&t, 1.0f / 12.0f, 2.0f, 2.0f, 1 / 0.17883277f,
                                                0.28466892f, 0.55991073f);
        tfs.push_back({"hlgish_k", t});
    }
    const float xs[] = {-0.5f, -0.0f, 0.0f, 0.001f, 0.04045f, 0.2f, 0.5f, 0.75f, 0.9999f, 1.0f, 1.5f, 10.0f};
    for (auto& [name, tf] : tfs) {
        dump_tf(("TF " + name).c_str(), tf);
        printf("  type=%d\n", (int)skcms_TransferFunction_getType(&tf));
        printf("  eval");
        for (float x : xs) printf(" %08x", fbits(skcms_TransferFunction_eval(&tf, x)));
        printf("\n");
        skcms_TransferFunction inv;
        if (skcms_TransferFunction_invert(&tf, &inv)) {
            dump_tf("  inv", inv);
            printf("  inveval");
            for (float x : xs) printf(" %08x", fbits(skcms_TransferFunction_eval(&inv, x)));
            printf("\n");
        } else {
            printf("  inv fail\n");
        }
    }
    printf("powf");
    for (float x : {0.0f, 0.1f, 0.5f, 1.0f, 2.0f, 7.5f, 1000.f})
        for (float y : {0.0f, 0.4f, 1.0f, 2.2f, 2.4f, -1.0f})
            printf(" %08x", fbits(powf_(x, y)));
    printf("\n");

    // ---- matrices
    const skcms_Matrix3x3 ms[] = {
        {{{1, 0, 0}, {0, 1, 0}, {0, 0, 1}}},
        {{{0.436065674f, 0.385147095f, 0.143066406f}, {0.222488403f, 0.716873169f, 0.060607910f}, {0.013916016f, 0.097076416f, 0.714096069f}}},
        {{{0.515102f, 0.291965f, 0.157153f}, {0.241182f, 0.692236f, 0.0665819f}, {-0.00104941f, 0.0418818f, 0.784378f}}},
        {{{1, 2, 3}, {4, 5, 6}, {7, 8, 9}}},
        {{{0, 0, 0}, {0, 0, 0}, {0, 0, 0}}},
        {{{1e-30f, 0, 0}, {0, 1e-30f, 0}, {0, 0, 1e-30f}}},
    };
    for (int i = 0; i < 6; i++) {
        skcms_Matrix3x3 inv;
        bool ok = skcms_Matrix3x3_invert(&ms[i], &inv);
        printf("MAT %d inv=%d\n", i, ok);
        if (ok) dump_mat("  inv", inv);
        for (int j = 0; j < 6; j++) {
            skcms_Matrix3x3 c = skcms_Matrix3x3_concat(&ms[i], &ms[j]);
            dump_mat("  concat", c);
        }
    }
    const float prim[][8] = {
        {0.64f, 0.33f, 0.30f, 0.60f, 0.15f, 0.06f, 0.3127f, 0.3290f},
        {0.7347f, 0.2653f, 0.1596f, 0.8404f, 0.0366f, 0.0001f, 0.34567f, 0.35850f},
        {0.67f, 0.33f, 0.21f, 0.71f, 0.14f, 0.08f, 0.31006f, 0.31616f},
        {0.680f, 0.320f, 0.265f, 0.690f, 0.150f, 0.060f, 0.3127f, 0.3290f},
        {0.708f, 0.292f, 0.170f, 0.797f, 0.131f, 0.046f, 0.3127f, 0.3290f},
        {1.f, 0.f, 0.f, 1.f, 0.f, 0.f, 1 / 3.f, 1 / 3.f},
        {1.5f, 0.f, 0.f, 1.f, 0.f, 0.f, 1 / 3.f, 1 / 3.f},
    };
    for (auto& p : prim) {
        skcms_Matrix3x3 m;
        bool ok = skcms_PrimariesToXYZD50(p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7], &m);
        printf("PRIM ok=%d\n", ok);
        if (ok) dump_mat("  m", m);
    }

    // ---- profiles
    std::vector<std::string> files;
    for (auto& e : fs::recursive_directory_iterator(root + "/icc_profiles")) {
        if (e.is_regular_file()) {
            std::string ext = e.path().extension().string();
            if (ext == ".icc" || ext == ".icm") files.push_back(e.path().generic_string().substr(root.size() + 1));
        }
    }
    std::sort(files.begin(), files.end());
    struct Loaded {
        std::string name;
        std::vector<uint8_t> data;
        skcms_ICCProfile p;
    };
    std::vector<Loaded> loaded;
    for (auto& f : files) {
        std::ifstream in(root + "/" + f, std::ios::binary);
        std::vector<uint8_t> data((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
        // skcms needs a little padding after the profile; give it a real zero tail like Skia's SkData callers.
        // (Both sides parse the exact same bytes.)
        Loaded L;
        L.name = f;
        L.data = data;
        memset(&L.p, 0, sizeof(L.p));
        bool ok = skcms_Parse(L.data.data(), L.data.size(), &L.p);
        printf("FILE %s len=%zu parse=%d\n", f.c_str(), data.size(), ok);
        if (ok) dump_profile(f, L.p);
        if (ok) loaded.push_back(std::move(L));
    }
    // vectors may have moved data: re-parse so buffer pointers are valid
    for (auto& L : loaded) {
        skcms_Parse(L.data.data(), L.data.size(), &L.p);
    }

    // ---- profile comparisons, approximations
    for (size_t i = 0; i < loaded.size(); i++) {
        auto& L = loaded[i];
        auto& M = loaded[(i + 1) % loaded.size()];
        printf("EQ %s srgb=%d xyz=%d next=%d self=%d\n", L.name.c_str(),
               skcms_ApproximatelyEqualProfiles(&L.p, skcms_sRGB_profile()),
               skcms_ApproximatelyEqualProfiles(&L.p, skcms_XYZD50_profile()),
               skcms_ApproximatelyEqualProfiles(&L.p, &M.p),
               skcms_ApproximatelyEqualProfiles(&L.p, &L.p));
        printf("TRCS srgbinv=%d\n", skcms_TRCs_AreApproximateInverse(&L.p, skcms_sRGB_Inverse_TransferFunction()));
        if (L.p.has_trc) {
            for (int c = 0; c < 3; c++) {
                if (L.p.trc[c].table_entries) {
                    skcms_TransferFunction a;
                    float err;
                    bool ok = skcms_ApproximateCurve(&L.p.trc[c], &a, &err);
                    printf("APPROX %s trc%d ok=%d", L.name.c_str(), c, ok);
                    if (ok) {
                        printf(" err=%08x", fbits(err));
                        dump_tf(" tf", a);
                    } else {
                        printf("\n");
                    }
                    printf("  maxerr=%08x\n", fbits(skcms_MaxRoundtripError(&L.p.trc[c], skcms_sRGB_Inverse_TransferFunction())));
                }
            }
        }
        skcms_ICCProfile u = L.p;
        bool ok = skcms_MakeUsableAsDestination(&u);
        printf("USABLE %s ok=%d\n", L.name.c_str(), ok);
        if (ok) dump_profile(L.name + "#usable", u);
        skcms_ICCProfile u1 = L.p;
        ok = skcms_MakeUsableAsDestinationWithSingleCurve(&u1);
        printf("USABLE1 %s ok=%d\n", L.name.c_str(), ok);
        if (ok && u1.has_trc) dump_tf("  tf0", u1.trc[0].parametric);
    }

    // ---- transforms
    skcms_ICCProfile srgb_p = *skcms_sRGB_profile();
    skcms_ICCProfile xyz_p = *skcms_XYZD50_profile();
    skcms_Matrix3x3 p3 = ms[2];
    skcms_ICCProfile p3_p = make_profile({2.2f, 1, 0, 0, 0, 0, 0}, p3);
    skcms_ICCProfile p3srgb_p = make_profile(*skcms_sRGB_TransferFunction(), p3);
    skcms_TransferFunction pq203, hlg203;
    skcms_TransferFunction_makePQ(&pq203, 203.f);
    skcms_TransferFunction_makeHLG(&hlg203, 203.f, 1000.f, 1.2f);
    skcms_ICCProfile pq_p = make_profile(pq203, p3);
    pq_p.has_CICP = true;
    pq_p.CICP = {9, 16, 0, 1};
    skcms_ICCProfile hlg_p = make_profile(hlg203, p3);
    hlg_p.has_CICP = true;
    hlg_p.CICP = {9, 18, 0, 1};
    std::vector<std::pair<std::string, const skcms_ICCProfile*>> dsts = {
        {"srgb", &srgb_p}, {"xyz", &xyz_p}, {"p3g22", &p3_p}, {"p3srgb", &p3srgb_p}, {"pq", &pq_p}, {"hlg", &hlg_p},
        {"null", nullptr},
    };
    // format sweep: srgb -> each dst profile, all dst formats
    for (auto& [dname, dp] : dsts) {
        for (int fmt = 0; fmt < 42; fmt++) {
            run_transform(("sweepdst:" + dname).c_str(), &srgb_p, 33, 1, dp, fmt, (fmt + 1) % 3, 37);
        }
    }
    // format sweep: each src format -> srgb / xyz / p3
    for (int fmt = 0; fmt < 42; fmt++) {
        for (auto& [dname, dp] : dsts) {
            run_transform(("sweepsrc:" + dname).c_str(), &p3srgb_p, fmt, fmt % 3, dp, 27 - (fmt & 1), (fmt + 2) % 3, 41);
        }
    }
    // HDR sources
    for (auto& [sname, sp] : dsts) {
        for (auto& [dname, dp] : dsts) {
            run_transform(("hdr:" + sname + ">" + dname).c_str(), sp, 35, 0, dp, 35, 0, 29);
            run_transform(("hdr8:" + sname + ">" + dname).c_str(), sp, 13, 1, dp, 13, 2, 29);
        }
    }
    // parsed profiles: as sources to several destinations, in a couple of formats
    for (size_t i = 0; i < loaded.size(); i++) {
        auto& L = loaded[i];
        int channels = skcms_GetInputChannelCount(&L.p);
        for (auto& [dname, dp] : dsts) {
            int fmts[][2] = {{12, 12}, {10, 34}, {35, 35}, {25, 30}, {18, 13}};
            int k = 0;
            for (auto& f : fmts) {
                run_transform(("src:" + L.name + ">" + dname).c_str(), &L.p, f[0], (int)((i + k) % 3), dp, f[1], (int)((i + k + 1) % 3), 53);
                k++;
            }
        }
        (void)channels;
        // as destination (if usable): from sRGB, in a few formats
        skcms_ICCProfile u = L.p;
        bool usable = skcms_MakeUsableAsDestination(&u);
        if (usable) {
            int fmts[][2] = {{12, 12}, {35, 35}, {34, 14}, {19, 12}, {35, 10}};
            int k = 0;
            for (auto& f : fmts) {
                run_transform(("dst:" + L.name).c_str(), &srgb_p, f[0], (int)((i + k) % 3), &u, f[1], (int)((i + k + 2) % 3), 53);
                k++;
            }
            run_transform(("dstraw:" + L.name).c_str(), &srgb_p, 35, 0, &L.p, 35, 0, 31);
            run_transform(("self:" + L.name).c_str(), &L.p, 35, 0, &L.p, 35, 0, 31);
            // gray destination formats exercise the gray_dst_profile path
            run_transform(("gray:" + L.name).c_str(), &srgb_p, 12, 1, &u, 4, 2, 31);
            run_transform(("gray8:" + L.name).c_str(), &srgb_p, 12, 1, &u, 2, 1, 31);
        }
    }
    printf("DONE\n");
    return 0;
}
