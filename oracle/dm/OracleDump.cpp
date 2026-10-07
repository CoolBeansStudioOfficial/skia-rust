/*
 * skia-rust oracle: raw output dump for DM. Test tooling only; never shipped.
 *
 * Copied into third_party/skia/dm/ by `cargo xtask oracle patch`. DM renders exactly as it does
 * for Skia's own bots; this file only writes what it rendered, byte for byte, so skia-rust can
 * compare its output against it. SHA-256 hashes are computed by `cargo xtask oracle hash`.
 */

#include "dm/OracleDump.h"

#include "include/core/SkBitmap.h"
#include "include/core/SkColorSpace.h"
#include "include/core/SkData.h"
#include "include/core/SkStream.h"
#include "include/core/SkString.h"
#include "include/private/SkFeatures.h"
#include "src/core/SkCpu.h"
#include "src/core/SkOSFile.h"
#include "src/utils/SkOSPath.h"
#include "tools/ToolUtils.h"
#include "tools/flags/CommandLineFlags.h"

#include <cstdlib>

static DEFINE_string(oracleRawPath, "",
                     "skia-rust oracle: if set, write raw pixels (.raw) or encoded bytes (.bin) "
                     "plus .json metadata for every result here.");

namespace {

SkString json_escape(const char* s) {
    SkString out;
    for (; *s; ++s) {
        const unsigned char c = static_cast<unsigned char>(*s);
        if (c == '"' || c == '\\') {
            out.append("\\");
            out.append(s, 1);
        } else if (c < 0x20) {
            out.appendf("\\u%04x", c);
        } else {
            out.append(s, 1);
        }
    }
    return out;
}

SkString hex(const void* bytes, size_t len) {
    SkString out;
    const auto* p = static_cast<const uint8_t*>(bytes);
    for (size_t i = 0; i < len; ++i) {
        out.appendf("%02x", p[i]);
    }
    return out;
}

// The runtime CPU tier that actually ran, so xtask can reject a run whose tier
// silently fell back (e.g. ml4 requested on a CPU without AVX-512).
const char* effective_cpu_tier() {
#if defined(SK_CPU_X86)
    if (SkCpu::Supports(SkX64::ML4)) { return "ml4"; }
    if (SkCpu::Supports(SkX64::ML3)) { return "ml3"; }
    if (SkCpu::Supports(SkX64::SSSE3)) { return "ssse3"; }
    return "baseline";
#elif defined(SK_CPU_ARM64)
    return "neon";
#else
    return "baseline";
#endif
}

bool write_file(const SkString& path, const void* data, size_t len) {
    SkFILEWStream file(path.c_str());
    return file.isValid() && file.write(data, len);
}

}  // namespace

// Defined in src/core/SkRasterPipeline.cpp by the oracle patch.
void SkOracleSetResultId(const char* id);

namespace OracleDump {

void SetResult(const SkString& sinkTag,
               const SkString& srcTag,
               const SkString& srcOptions,
               const SkString& name) {
    SkString id = sinkTag;
    id.appendf("/%s", srcTag.c_str());
    if (!srcOptions.isEmpty()) {
        id.appendf("/%s", srcOptions.c_str());
    }
    id.appendf("/%s", name.c_str());
    SkOracleSetResultId(id.c_str());
}

bool Enabled() { return !FLAGS_oracleRawPath.isEmpty(); }

void Write(const SkString& sinkTag,
           const SkString& srcTag,
           const SkString& srcOptions,
           const SkString& name,
           const SkBitmap& bitmap,
           const void* data,
           size_t len) {
    SkString dir = SkOSPath::Join(FLAGS_oracleRawPath[0], sinkTag.c_str());
    sk_mkdir(dir.c_str());
    dir = SkOSPath::Join(dir.c_str(), srcTag.c_str());
    sk_mkdir(dir.c_str());
    if (!srcOptions.isEmpty()) {
        dir = SkOSPath::Join(dir.c_str(), srcOptions.c_str());
        sk_mkdir(dir.c_str());
    }
    const SkString base = SkOSPath::Join(dir.c_str(), name.c_str());

    SkString meta;
    meta.appendf("{\n  \"sink\": \"%s\",\n", json_escape(sinkTag.c_str()).c_str());
    meta.appendf("  \"src\": \"%s\",\n", json_escape(srcTag.c_str()).c_str());
    meta.appendf("  \"options\": \"%s\",\n", json_escape(srcOptions.c_str()).c_str());
    meta.appendf("  \"name\": \"%s\",\n", json_escape(name.c_str()).c_str());
#if defined(SK_CPU_X86)
    meta.appendf("  \"cpu_x64_level\": %d,\n", SK_CPU_X64_LEVEL);
#endif
    const char* cap = getenv("SKIA_ORACLE_CPU_CAP");
    meta.appendf("  \"cpu_cap\": \"%s\",\n", json_escape(cap ? cap : "").c_str());
    meta.appendf("  \"cpu_tier\": \"%s\",\n", effective_cpu_tier());

    bool ok;
    if (len > 0) {
        meta.appendf("  \"kind\": \"bytes\",\n  \"len\": %zu\n}\n", len);
        ok = write_file(SkStringPrintf("%s.bin", base.c_str()), data, len);
    } else {
        const SkImageInfo& info = bitmap.info();
        const size_t row = info.minRowBytes();
        SkDynamicMemoryWStream pixels;
        for (int y = 0; y < info.height(); ++y) {
            pixels.write(bitmap.getAddr(0, y), row);
        }
        SkString cs("none");
        if (SkColorSpace* colorSpace = bitmap.colorSpace()) {
            sk_sp<SkData> serialized = colorSpace->serialize();
            cs = hex(serialized->data(), serialized->size());
        }
        meta.appendf("  \"kind\": \"pixels\",\n");
        meta.appendf("  \"width\": %d,\n  \"height\": %d,\n", info.width(), info.height());
        meta.appendf("  \"color_type\": \"%s\",\n", ToolUtils::colortype_name(info.colorType()));
        meta.appendf("  \"alpha_type\": \"%s\",\n", ToolUtils::alphatype_name(info.alphaType()));
        meta.appendf("  \"color_space\": \"%s\"\n}\n", cs.c_str());
        sk_sp<SkData> bytes = pixels.detachAsData();
        ok = write_file(SkStringPrintf("%s.raw", base.c_str()), bytes->data(), bytes->size());
    }
    ok = ok && write_file(SkStringPrintf("%s.json", base.c_str()), meta.c_str(), meta.size());
    if (!ok) {
        fprintf(stderr, "OracleDump: failed to write %s\n", base.c_str());
        abort();
    }
}

}  // namespace OracleDump
