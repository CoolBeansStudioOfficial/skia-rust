// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
//
// The C++ side of oracle/shaper-diff (docs/design/modules.md §2.4, Appendix A): shapes the corpus with
// HarfBuzz at Skia's pin (9cb1fee5, 13.1.0) and prints, per (config, font, text source), the line and
// glyph counts and the SHA-256 of the serialized result. The Rust side (src/) does the same with
// HarfRust and the committed expected files are what `cargo test` replays.
//
// Modes (configs.txt):
//   a  HarfBuzz's own OpenType font functions (hb_ot), scale none or size*65536.
//   b  Skia's font functions of SkShaper_harfbuzz.cpp#L72-L238 (nominal_glyph(s), glyph_h_advance(s),
//      glyph_extents), installed on a sub font of the hb_ot font exactly as create_sub_hb_font does
//      (#L274-L356). Their numbers come from a table file the Rust tool writes (nominal map, 16.16
//      advances, 16.16 extents) by calling the same `Font` callbacks that the Rust shaper installs
//      (skia-rust-shaper's SkiaFontFuncs), so both sides shape with identical inputs.
//   c  The "empty face" of TestTypeface: hb_face_create_for_tables with no tables, upem 2048
//      (#L274-L311), with Skia's callbacks over TestTypeface's numbers (also a table file).
//
// Usage: shaper_diff --fonts DIR --font-list FILE --configs FILE --corpus DIR --tables DIR
//                    [--config NAME] [--font NAME] [--dump]

#include <hb-ot.h>
#include <hb.h>

#include <clocale>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <algorithm>
#include <fstream>
#include <iostream>
#include <map>
#include <sstream>
#include <string>
#include <unordered_map>
#include <vector>

namespace {

// ---- SHA-256 (FIPS 180-4) ---------------------------------------------------------------------

struct Sha256 {
    uint32_t h[8] = {0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                     0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19};
    uint8_t buf[64];
    size_t buflen = 0;
    uint64_t total = 0;

    static uint32_t rotr(uint32_t x, int n) { return (x >> n) | (x << (32 - n)); }

    void block(const uint8_t* p) {
        static const uint32_t K[64] = {
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2};
        uint32_t w[64];
        for (int i = 0; i < 16; i++) {
            w[i] = (uint32_t)p[4 * i] << 24 | (uint32_t)p[4 * i + 1] << 16 |
                   (uint32_t)p[4 * i + 2] << 8 | (uint32_t)p[4 * i + 3];
        }
        for (int i = 16; i < 64; i++) {
            uint32_t s0 = rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >> 3);
            uint32_t s1 = rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16] + s0 + w[i - 7] + s1;
        }
        uint32_t v[8];
        for (int i = 0; i < 8; i++) v[i] = h[i];
        for (int i = 0; i < 64; i++) {
            uint32_t s1 = rotr(v[4], 6) ^ rotr(v[4], 11) ^ rotr(v[4], 25);
            uint32_t ch = (v[4] & v[5]) ^ (~v[4] & v[6]);
            uint32_t t1 = v[7] + s1 + ch + K[i] + w[i];
            uint32_t s0 = rotr(v[0], 2) ^ rotr(v[0], 13) ^ rotr(v[0], 22);
            uint32_t mj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            uint32_t t2 = s0 + mj;
            v[7] = v[6]; v[6] = v[5]; v[5] = v[4]; v[4] = v[3] + t1;
            v[3] = v[2]; v[2] = v[1]; v[1] = v[0]; v[0] = t1 + t2;
        }
        for (int i = 0; i < 8; i++) h[i] += v[i];
    }

    void update(const void* data, size_t n) {
        const uint8_t* p = (const uint8_t*)data;
        total += n;
        while (n > 0) {
            size_t take = std::min(n, 64 - buflen);
            memcpy(buf + buflen, p, take);
            buflen += take; p += take; n -= take;
            if (buflen == 64) { block(buf); buflen = 0; }
        }
    }

    std::string hex() {
        uint64_t bits = total * 8;
        uint8_t pad = 0x80;
        update(&pad, 1);
        uint8_t zero = 0;
        while (buflen != 56) update(&zero, 1);
        uint8_t len[8];
        for (int i = 0; i < 8; i++) len[i] = (uint8_t)(bits >> (56 - 8 * i));
        update(len, 8);
        char out[65];
        for (int i = 0; i < 8; i++) snprintf(out + 8 * i, 9, "%08x", h[i]);
        return std::string(out, 64);
    }
};

// ---- Inputs -------------------------------------------------------------------------------------

std::string read_file(const std::string& path) {
    std::ifstream in(path, std::ios::binary);
    if (!in) { fprintf(stderr, "cannot read %s\n", path.c_str()); exit(1); }
    std::stringstream ss;
    ss << in.rdbuf();
    return ss.str();
}

std::vector<std::vector<std::string>> read_table(const std::string& path) {
    std::vector<std::vector<std::string>> rows;
    std::istringstream in(read_file(path));
    std::string line;
    while (std::getline(in, line)) {
        if (line.empty() || line[0] == '#') continue;
        std::istringstream ls(line);
        std::vector<std::string> cols;
        std::string c;
        while (ls >> c) cols.push_back(c);
        rows.push_back(cols);
    }
    return rows;
}

struct Config { std::string name; char mode; std::string size; double sizeValue; bool subpixel; };

// The corpus: every *.txt in DIR, sorted by name; the text source's id is the file stem. A text source's
// lines are split at '\n' (a final empty piece after a trailing '\n' is not a line).
struct Source { std::string id; std::vector<std::string> lines; };

std::vector<Source> read_corpus(const std::string& dir, const std::vector<std::string>& names) {
    std::vector<Source> out;
    for (const std::string& name : names) {
        Source s;
        s.id = name.substr(0, name.size() - 4);
        std::string text = read_file(dir + "/" + name);
        size_t start = 0;
        while (start < text.size()) {
            size_t end = text.find('\n', start);
            if (end == std::string::npos) end = text.size();
            std::string line = text.substr(start, end - start);
            if (!line.empty() && line.back() == '\r') line.pop_back();
            s.lines.push_back(line);
            start = end + 1;
        }
        out.push_back(std::move(s));
    }
    return out;
}

// ---- Skia's font functions over tables ----------------------------------------------------------

struct FontTable {
    std::vector<int32_t> advance;         // hb_position_t (16.16), after rounding when !subpixel
    std::vector<int32_t> extents;         // 4 per glyph: x_bearing, y_bearing, width, height
    std::unordered_map<uint32_t, uint32_t> cmap;
};

uint32_t rd32(const std::string& s, size_t& pos) {
    uint32_t v;
    memcpy(&v, s.data() + pos, 4);  // little-endian host
    pos += 4;
    return v;
}

FontTable load_table(const std::string& path) {
    std::string s = read_file(path);
    size_t pos = 0;
    if (s.compare(0, 4, "SDT1") != 0) { fprintf(stderr, "bad table %s\n", path.c_str()); exit(1); }
    pos = 4;
    uint32_t n = rd32(s, pos);
    FontTable t;
    t.advance.resize(n);
    t.extents.resize(4 * (size_t)n);
    for (uint32_t i = 0; i < n; i++) t.advance[i] = (int32_t)rd32(s, pos);
    for (size_t i = 0; i < 4 * (size_t)n; i++) t.extents[i] = (int32_t)rd32(s, pos);
    uint32_t m = rd32(s, pos);
    t.cmap.reserve(m);
    for (uint32_t i = 0; i < m; i++) {
        uint32_t cp = rd32(s, pos);
        uint32_t gid = rd32(s, pos);
        t.cmap[cp] = gid;
    }
    return t;
}

// SkShaper_harfbuzz.cpp#L89-L96
hb_bool_t skhb_nominal_glyph(hb_font_t*, void* font_data, hb_codepoint_t unicode,
                             hb_codepoint_t* glyph, void*) {
    const FontTable& t = *(const FontTable*)font_data;
    auto it = t.cmap.find(unicode);
    *glyph = it == t.cmap.end() ? 0 : it->second;
    return *glyph != 0;
}

// #L98-L127: "return 'done' to allow HarfBuzz to synthesize with NFC and spaces".
unsigned skhb_nominal_glyphs(hb_font_t*, void* font_data, unsigned count,
                             const hb_codepoint_t* unicodes, unsigned unicode_stride,
                             hb_codepoint_t* glyphs, unsigned glyph_stride, void*) {
    const FontTable& t = *(const FontTable*)font_data;
    unsigned done;
    for (done = 0; done < count; done++) {
        auto it = t.cmap.find(*unicodes);
        uint32_t g = it == t.cmap.end() ? 0 : it->second;
        if (g == 0) break;
        *glyphs = g;
        glyphs = (hb_codepoint_t*)((char*)glyphs + glyph_stride);
        unicodes = (const hb_codepoint_t*)((const char*)unicodes + unicode_stride);
    }
    return done;
}

// #L129-L142
hb_position_t skhb_glyph_h_advance(hb_font_t*, void* font_data, hb_codepoint_t g, void*) {
    const FontTable& t = *(const FontTable*)font_data;
    return g < t.advance.size() ? t.advance[g] : 0;
}

// #L144-L180
void skhb_glyph_h_advances(hb_font_t*, void* font_data, unsigned count, const hb_codepoint_t* glyphs,
                           unsigned glyph_stride, hb_position_t* advances, unsigned advance_stride,
                           void*) {
    const FontTable& t = *(const FontTable*)font_data;
    for (unsigned i = 0; i < count; i++) {
        *advances = *glyphs < t.advance.size() ? t.advance[*glyphs] : 0;
        advances = (hb_position_t*)((char*)advances + advance_stride);
        glyphs = (const hb_codepoint_t*)((const char*)glyphs + glyph_stride);
    }
}

// #L182-L208: always true.
hb_bool_t skhb_glyph_extents(hb_font_t*, void* font_data, hb_codepoint_t g,
                             hb_glyph_extents_t* e, void*) {
    const FontTable& t = *(const FontTable*)font_data;
    if (g < t.advance.size()) {
        e->x_bearing = t.extents[4 * (size_t)g];
        e->y_bearing = t.extents[4 * (size_t)g + 1];
        e->width = t.extents[4 * (size_t)g + 2];
        e->height = t.extents[4 * (size_t)g + 3];
    } else {
        e->x_bearing = e->y_bearing = e->width = e->height = 0;
    }
    return true;
}

hb_font_funcs_t* make_funcs() {
    hb_font_funcs_t* funcs = hb_font_funcs_create();
    // skhb_get_font_funcs, SkShaper_harfbuzz.cpp#L228-L256
    hb_font_funcs_set_nominal_glyph_func(funcs, skhb_nominal_glyph, nullptr, nullptr);
    hb_font_funcs_set_nominal_glyphs_func(funcs, skhb_nominal_glyphs, nullptr, nullptr);
    hb_font_funcs_set_glyph_h_advance_func(funcs, skhb_glyph_h_advance, nullptr, nullptr);
    hb_font_funcs_set_glyph_h_advances_func(funcs, skhb_glyph_h_advances, nullptr, nullptr);
    hb_font_funcs_set_glyph_extents_func(funcs, skhb_glyph_extents, nullptr, nullptr);
    hb_font_funcs_make_immutable(funcs);
    return funcs;
}

hb_blob_t* no_table(hb_face_t*, hb_tag_t, void*) { return nullptr; }

// ---- Shaping ------------------------------------------------------------------------------------

struct RunResult { size_t lines = 0, glyphs = 0; std::string sha; };

std::string shape_line(hb_font_t* font, const std::string& text) {
    std::string out;
    if (text.empty()) return out;
    hb_buffer_t* buf = hb_buffer_create();
    hb_buffer_set_cluster_level(buf, HB_BUFFER_CLUSTER_LEVEL_MONOTONE_CHARACTERS);
    hb_buffer_add_utf8(buf, text.data(), (int)text.size(), 0, (int)text.size());
    hb_buffer_guess_segment_properties(buf);
    hb_shape(font, buf, nullptr, 0);
    unsigned n = 0;
    hb_glyph_info_t* info = hb_buffer_get_glyph_infos(buf, &n);
    hb_glyph_position_t* pos = hb_buffer_get_glyph_positions(buf, &n);
    char tmp[128];
    for (unsigned i = 0; i < n; i++) {
        snprintf(tmp, sizeof tmp, "%s%u:%u:%d:%d:%d:%d", i ? " " : "", info[i].codepoint,
                 info[i].cluster, pos[i].x_advance, pos[i].y_advance, pos[i].x_offset,
                 pos[i].y_offset);
        out += tmp;
    }
    hb_buffer_destroy(buf);
    return out;
}

size_t count_glyphs(const std::string& line) {
    if (line.empty()) return 0;
    return 1 + std::count(line.begin(), line.end(), ' ');
}

}  // namespace

int main(int argc, char** argv) {
    std::string fontsDir, fontList, configsFile, corpusDir, tablesDir, onlyConfig, onlyFont;
    bool dump = false;
    for (int i = 1; i < argc; i++) {
        std::string a = argv[i];
        auto next = [&]() -> std::string {
            if (i + 1 >= argc) { fprintf(stderr, "missing value for %s\n", a.c_str()); exit(2); }
            return argv[++i];
        };
        if (a == "--fonts") fontsDir = next();
        else if (a == "--font-list") fontList = next();
        else if (a == "--configs") configsFile = next();
        else if (a == "--corpus") corpusDir = next();
        else if (a == "--tables") tablesDir = next();
        else if (a == "--config") onlyConfig = next();
        else if (a == "--font") onlyFont = next();
        else if (a == "--dump") dump = true;
        else { fprintf(stderr, "unknown argument %s\n", a.c_str()); return 2; }
    }
    setlocale(LC_ALL, "C");

    std::vector<Config> configs;
    for (auto& row : read_table(configsFile)) {
        Config c;
        c.name = row[0];
        c.mode = row[1][0];
        c.size = row[2];
        c.sizeValue = row[2] == "-" ? 0 : atof(row[2].c_str());
        c.subpixel = row[3] == "1";
        configs.push_back(c);
    }
    std::vector<std::string> fontNames;
    for (auto& row : read_table(fontList)) fontNames.push_back(row[0]);
    std::vector<std::string> corpusNames;
    for (auto& row : read_table(corpusDir + "/index.txt")) corpusNames.push_back(row[0]);
    std::vector<Source> corpus = read_corpus(corpusDir, corpusNames);

    hb_font_funcs_t* skiaFuncs = make_funcs();

    for (const Config& cfg : configs) {
        if (!onlyConfig.empty() && cfg.name != onlyConfig) continue;
        std::vector<std::string> fonts = cfg.mode == 'c' ? std::vector<std::string>{"(empty)"} : fontNames;
        for (const std::string& fontName : fonts) {
            if (!onlyFont.empty() && fontName != onlyFont) continue;
            hb_face_t* face;
            std::string fontData;  // must outlive the face
            FontTable table;
            if (cfg.mode == 'c') {
                // create_hb_face's fallback (#L298-L311) for a typeface without an sfnt stream.
                face = hb_face_create_for_tables(no_table, nullptr, nullptr);
                hb_face_set_upem(face, 2048);
            } else {
                fontData = read_file(fontsDir + "/" + fontName);
                hb_blob_t* blob = hb_blob_create(fontData.data(), (unsigned)fontData.size(),
                                                 HB_MEMORY_MODE_READONLY, nullptr, nullptr);
                hb_blob_make_immutable(blob);
                face = hb_face_create(blob, 0);
                hb_blob_destroy(blob);
            }
            // create_typeface_hb_font (#L313-L337)
            hb_font_t* otFont = hb_font_create(face);
            hb_ot_font_set_funcs(otFont);
            hb_font_t* font = otFont;
            if (cfg.mode == 'b' || cfg.mode == 'c') {
                // create_sub_hb_font (#L339-L356)
                font = hb_font_create_sub_font(otFont);
                table = load_table(tablesDir + "/" + fontName + "@" + cfg.size + "@" +
                                   (cfg.subpixel ? "1" : "0") + ".tbl");
                hb_font_set_funcs(font, skiaFuncs, &table, nullptr);
            }
            if (cfg.mode != 'a' || cfg.size != "-") {
                int scale = (int)(cfg.sizeValue * 65536.0 + 0.5);
                hb_font_set_scale(font, scale, scale);
            }
            for (const Source& src : corpus) {
                RunResult r;
                Sha256 sha;
                for (const std::string& line : src.lines) {
                    std::string s = shape_line(font, line);
                    r.lines++;
                    r.glyphs += count_glyphs(s);
                    s += '\n';
                    sha.update(s.data(), s.size());
                    if (dump) printf("L %s %s %s %zu %s", cfg.name.c_str(), fontName.c_str(), src.id.c_str(), r.lines - 1, s.c_str());
                }
                if (!dump) {
                    printf("%s %s %s %zu %zu %s\n", cfg.name.c_str(), fontName.c_str(), src.id.c_str(),
                           r.lines, r.glyphs, sha.hex().c_str());
                }
            }
            if (font != otFont) hb_font_destroy(font);
            hb_font_destroy(otFont);
            hb_face_destroy(face);
        }
    }
    return 0;
}
