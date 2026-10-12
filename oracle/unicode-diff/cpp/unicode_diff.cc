// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
//
// The C++ side of oracle/unicode-diff (docs/design/modules.md §3.3, M0c): Chromium's ICU 78.2 (Skia's
// DEPS pin d578f2e8, common/*.cpp plus common/icudtl.dat) driven the way SkUnicode_icu.cpp and
// SkUnicode_icu_bidi.cpp drive it, over a corpus. It makes every call of SKICU_EMIT_FUNCS
// (modules/skunicode/src/SkUnicode_icupriv.h#L21-L52) that has an ICU4X counterpart, and prints the
// results; the Rust twin (src/) does the same with ICU4X and unicode-bidi.
//
// Usage: unicode_diff --data icudtl.dat --corpus DIR [--ucd DIR] [--dump] <section>
// Sections (each prints the contents of expected/<section>.txt; --dump prints the detail instead of
// per-string hashes, which classify.py reads):
//   breaks       ubrk_open/setUText/first/next/current/getRuleStatus/following/preceding over every string
//   bidi         ubidi_openSized/setPara/getLength/getDirection/getLevelAt over every string
//   reorder      ubidi_reorderVisual over all level sequences of two alphabets
//   props        u_hasBinaryProperty, u_getIntPropertyValue(UCHAR_LINE_BREAK), u_iscntrl, u_isspace,
//                u_isWhitespace, uscript_getScript over all code points
//   case         u_strToUpper over every code point and string, per locale
//   conformance  the UAX #29, #14 and #9 test files through the break and bidi calls above
//   misc         u_errorName, uloc_forLanguageTag, uloc_getDefault (informational: no ICU4X counterpart)

#include <unicode/ubidi.h>
#include <unicode/ubrk.h>
#include <unicode/uchar.h>
#include <unicode/uloc.h>
#include <unicode/uscript.h>
#include <unicode/ustring.h>
#include <unicode/utext.h>
#include <unicode/udata.h>

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <functional>
#include <iostream>
#include <sstream>
#include <string>
#include <vector>

#include "sha256.h"

// The entry point of the (empty) built-in data; the real data is installed with udata_setCommonData.
extern "C" {
extern const unsigned char icudt78_dat[64];
alignas(16) const unsigned char icudt78_dat[64] = {0};
}

namespace {

bool g_dump = false;

[[noreturn]] void die(const std::string& msg) {
    fprintf(stderr, "unicode_diff: %s\n", msg.c_str());
    exit(1);
}

std::string read_file(const std::string& path) {
    std::ifstream in(path, std::ios::binary);
    if (!in) die("cannot read " + path);
    std::stringstream ss;
    ss << in.rdbuf();
    return ss.str();
}

std::string hash16(const std::string& s) {
    Sha256 h;
    h.update(s.data(), s.size());
    return h.hex().substr(0, 16);
}

// ---- UTF-8 / UTF-16 ------------------------------------------------------------------------------

void append_utf8(std::string& out, uint32_t c) {
    if (c < 0x80) {
        out += (char)c;
    } else if (c < 0x800) {
        out += (char)(0xC0 | (c >> 6));
        out += (char)(0x80 | (c & 0x3F));
    } else if (c < 0x10000) {
        out += (char)(0xE0 | (c >> 12));
        out += (char)(0x80 | ((c >> 6) & 0x3F));
        out += (char)(0x80 | (c & 0x3F));
    } else {
        out += (char)(0xF0 | (c >> 18));
        out += (char)(0x80 | ((c >> 12) & 0x3F));
        out += (char)(0x80 | ((c >> 6) & 0x3F));
        out += (char)(0x80 | (c & 0x3F));
    }
}

// Valid UTF-8 only (the corpus is generated valid).
std::vector<uint32_t> decode_utf8(const std::string& s) {
    std::vector<uint32_t> out;
    size_t i = 0;
    while (i < s.size()) {
        uint8_t b = (uint8_t)s[i];
        uint32_t c;
        int n;
        if (b < 0x80) { c = b; n = 1; }
        else if (b < 0xE0) { c = b & 0x1F; n = 2; }
        else if (b < 0xF0) { c = b & 0x0F; n = 3; }
        else { c = b & 0x07; n = 4; }
        for (int k = 1; k < n; k++) c = (c << 6) | ((uint8_t)s[i + k] & 0x3F);
        out.push_back(c);
        i += n;
    }
    return out;
}

// A pointer ICU accepts for empty text (std::u16string::c_str(), which Skia passes, is never null).
const UChar kEmpty[1] = {0};
const UChar* text_ptr(const std::vector<UChar>& v) { return v.empty() ? kEmpty : v.data(); }

std::vector<UChar> to_utf16(const std::vector<uint32_t>& cps) {
    std::vector<UChar> out;
    for (uint32_t c : cps) {
        if (c < 0x10000) {
            out.push_back((UChar)c);
        } else {
            c -= 0x10000;
            out.push_back((UChar)(0xD800 + (c >> 10)));
            out.push_back((UChar)(0xDC00 + (c & 0x3FF)));
        }
    }
    return out;
}

// ---- Corpus --------------------------------------------------------------------------------------

struct Source {
    std::string id;
    std::vector<std::string> strings;  // UTF-8
};

uint32_t hexval(const std::string& s, size_t pos, size_t n) {
    return (uint32_t)strtoul(s.substr(pos, n).c_str(), nullptr, 16);
}

std::string unescape(const std::string& line) {
    std::string out;
    for (size_t i = 0; i < line.size(); i++) {
        if (line[i] != '\\') { out += line[i]; continue; }
        char e = line[++i];
        switch (e) {
            case 'n': out += '\n'; break;
            case 'r': out += '\r'; break;
            case 't': out += '\t'; break;
            case '\\': out += '\\'; break;
            case 'x': append_utf8(out, hexval(line, i + 1, 2)); i += 2; break;
            case 'u': append_utf8(out, hexval(line, i + 1, 4)); i += 4; break;
            case 'U': append_utf8(out, hexval(line, i + 1, 8)); i += 8; break;
            default: die("bad escape in corpus line: " + line);
        }
    }
    return out;
}

std::vector<Source> read_corpus(const std::string& dir) {
    std::vector<Source> out;
    std::istringstream index(read_file(dir + "/index.txt"));
    std::string line;
    while (std::getline(index, line)) {
        if (line.empty() || line[0] == '#') continue;
        std::istringstream ls(line);
        std::string name, kind;
        ls >> name >> kind;
        Source s;
        s.id = name.substr(0, name.size() - 4);
        std::string text = read_file(dir + "/" + name);
        if (kind == "whole") {
            s.strings.push_back(text);
        } else {
            size_t start = 0;
            while (start < text.size()) {
                size_t end = text.find('\n', start);
                if (end == std::string::npos) end = text.size();
                s.strings.push_back(unescape(text.substr(start, end - start)));
                start = end + 1;
            }
        }
        out.push_back(std::move(s));
    }
    return out;
}

// One row per string: `<case> <source> <idx> <n> <hash16>`, or with --dump `<case> <source> <idx> <detail>`.
void emit_row(const std::string& kase, const Source& src, size_t idx, size_t n, const std::string& detail) {
    if (g_dump) {
        printf("%s %s %zu %s\n", kase.c_str(), src.id.c_str(), idx, detail.c_str());
    } else {
        printf("%s %s %zu %zu %s\n", kase.c_str(), src.id.c_str(), idx, n, hash16(detail).c_str());
    }
}

// ---- breaks (SkUnicode_icu.cpp#L330-L410, SkBreakIterator_icu) -------------------------------------

UBreakIteratorType break_type(const std::string& t) {
    if (t == "char") return UBRK_CHARACTER;
    if (t == "word") return UBRK_WORD;
    if (t == "line") return UBRK_LINE;
    return UBRK_SENTENCE;
}

struct BreakIter {
    UBreakIterator* bi = nullptr;
    UText* ut = nullptr;
    ~BreakIter() {
        if (ut) utext_close(ut);
        if (bi) ubrk_close(bi);
    }
};

// `p:s` for first() and every next() until UBRK_DONE, with the raw rule status after each; current()
// is checked to equal the position (SkBreakIterator_icu::current).
std::string enumerate(const std::string& type, bool utf16, const std::string& text8, int* count,
                      bool nav) {
    UErrorCode status = U_ZERO_ERROR;
    BreakIter it;
    it.bi = ubrk_open(break_type(type), "en", nullptr, 0, &status);
    if (U_FAILURE(status)) return std::string("open-error:") + u_errorName(status);
    std::vector<UChar> u16 = to_utf16(decode_utf8(text8));
    int32_t len;
    if (utf16) {
        it.ut = utext_openUChars(nullptr, text_ptr(u16), (int32_t)u16.size(), &status);
        len = (int32_t)u16.size();
    } else {
        it.ut = utext_openUTF8(nullptr, text8.data(), (int64_t)text8.size(), &status);
        len = (int32_t)text8.size();
    }
    if (U_FAILURE(status)) return std::string("text-error:") + u_errorName(status);
    ubrk_setUText(it.bi, it.ut, &status);
    if (U_FAILURE(status)) return std::string("settext-error:") + u_errorName(status);
    std::string out;
    *count = 0;
    if (!nav) {
        int32_t pos = ubrk_first(it.bi);
        while (pos != UBRK_DONE) {
            if (ubrk_current(it.bi) != pos) die("ubrk_current != position");
            if (!out.empty()) out += ' ';
            out += std::to_string(pos) + ":" + std::to_string(ubrk_getRuleStatus(it.bi));
            ++*count;
            pos = ubrk_next(it.bi);
        }
    } else {
        // ubrk_following / ubrk_preceding at every offset (SkBreakIterator_icu does not use them, but
        // SKICU_EMIT_FUNCS lists them for clients that do).
        for (int32_t i = 0; i <= len; i++) {
            if (!out.empty()) out += ' ';
            int32_t f = ubrk_following(it.bi, i);
            int32_t p = ubrk_preceding(it.bi, i);
            out += std::to_string(i) + ">" + std::to_string(f) + "<" + std::to_string(p);
            ++*count;
        }
    }
    return out;
}

struct BreakCase { const char* name; const char* type; bool utf16; bool nav; };
const BreakCase kBreakCases[] = {
    {"char-u8", "char", false, false},   {"char-u16", "char", true, false},
    {"line-u8", "line", false, false},   {"line-u16", "line", true, false},
    {"word-u8", "word", false, false},   {"word-u16", "word", true, false},
    {"sent-u8", "sent", false, false},   {"sent-u16", "sent", true, false},
    {"nav-line-u8", "line", false, true}, {"nav-word-u16", "word", true, true},
};

void run_breaks(const std::vector<Source>& corpus) {
    for (const BreakCase& c : kBreakCases) {
        for (const Source& s : corpus) {
            for (size_t i = 0; i < s.strings.size(); i++) {
                int n = 0;
                std::string d = enumerate(c.type, c.utf16, s.strings[i], &n, c.nav);
                emit_row(c.name, s, i, (size_t)n, d);
            }
        }
    }
}

// ---- bidi (SkUnicode_icu_bidi.cpp#L36-L60) ---------------------------------------------------------

void run_bidi(const std::vector<Source>& corpus) {
    struct Level { const char* name; uint8_t level; };
    const Level levels[] = {{"bidi-ltr", UBIDI_LTR}, {"bidi-rtl", UBIDI_RTL},
                            {"bidi-auto-ltr", UBIDI_DEFAULT_LTR}, {"bidi-auto-rtl", UBIDI_DEFAULT_RTL}};
    for (const Level& l : levels) {
        for (const Source& s : corpus) {
            for (size_t i = 0; i < s.strings.size(); i++) {
                std::vector<UChar> u16 = to_utf16(decode_utf8(s.strings[i]));
                UErrorCode status = U_ZERO_ERROR;
                UBiDi* bidi = ubidi_openSized((int32_t)u16.size(), 0, &status);
                std::string d;
                if (U_FAILURE(status)) {
                    d = std::string("open-error:") + u_errorName(status);
                } else {
                    ubidi_setPara(bidi, text_ptr(u16), (int32_t)u16.size(), l.level, nullptr, &status);
                    if (U_FAILURE(status)) {
                        d = std::string("setpara-error:") + u_errorName(status);
                    } else {
                        int32_t len = ubidi_getLength(bidi);
                        d = "dir=" + std::to_string((int)ubidi_getDirection(bidi)) +
                            " len=" + std::to_string(len) + " levels=";
                        int run = 0;
                        int cur = -1;
                        bool first = true;
                        for (int32_t k = 0; k <= len; k++) {
                            int lv = k < len ? (int)ubidi_getLevelAt(bidi, k) : -2;
                            if (lv != cur) {
                                if (run > 0) {
                                    if (!first) d += ',';
                                    d += std::to_string(cur) + "*" + std::to_string(run);
                                    first = false;
                                }
                                cur = lv;
                                run = 0;
                            }
                            run++;
                        }
                    }
                }
                if (bidi) ubidi_close(bidi);
                emit_row(l.name, s, i, u16.size(), d);
            }
        }
    }
}

// ---- reorderVisual (SkUnicode::reorderVisual -> ubidi_reorderVisual) -------------------------------

void reorder_group(const char* name, const std::vector<uint8_t>& alphabet, int maxLen) {
    for (int len = 1; len <= maxLen; len++) {
        size_t total = 1;
        for (int k = 0; k < len; k++) total *= alphabet.size();
        std::string all;
        for (size_t n = 0; n < total; n++) {
            std::vector<uint8_t> lv(len);
            size_t m = n;
            for (int k = len - 1; k >= 0; k--) { lv[k] = alphabet[m % alphabet.size()]; m /= alphabet.size(); }
            std::vector<int32_t> map(len);
            ubidi_reorderVisual(lv.data(), len, map.data());
            for (int k = 0; k < len; k++) all += std::to_string((int)lv[k]) + (k + 1 < len ? "," : ">");
            for (int k = 0; k < len; k++) all += std::to_string(map[k]) + (k + 1 < len ? "," : "\n");
        }
        std::string id = std::string(name) + std::to_string(len);
        if (g_dump) printf("reorder %s 0 %s", id.c_str(), all.c_str());
        else printf("reorder %s 0 %zu %s\n", id.c_str(), total, hash16(all).c_str());
    }
}

void run_reorder() {
    reorder_group("small", {0, 1, 2, 3}, 7);
    reorder_group("big", {0, 1, 2, 125, 126}, 5);
}

// ---- props ---------------------------------------------------------------------------------------------

void run_props() {
    struct Prop { const char* name; std::function<int32_t(UChar32)> f; };
    const Prop props[] = {
        {"emoji", [](UChar32 c) { return (int32_t)u_hasBinaryProperty(c, UCHAR_EMOJI); }},
        {"emoji-component", [](UChar32 c) { return (int32_t)u_hasBinaryProperty(c, UCHAR_EMOJI_COMPONENT); }},
        {"emoji-modifier-base", [](UChar32 c) { return (int32_t)u_hasBinaryProperty(c, UCHAR_EMOJI_MODIFIER_BASE); }},
        {"emoji-modifier", [](UChar32 c) { return (int32_t)u_hasBinaryProperty(c, UCHAR_EMOJI_MODIFIER); }},
        {"regional-indicator", [](UChar32 c) { return (int32_t)u_hasBinaryProperty(c, UCHAR_REGIONAL_INDICATOR); }},
        {"ideographic", [](UChar32 c) { return (int32_t)u_hasBinaryProperty(c, UCHAR_IDEOGRAPHIC); }},
        {"line-break", [](UChar32 c) { return u_getIntPropertyValue(c, UCHAR_LINE_BREAK); }},
        {"iscntrl", [](UChar32 c) { return (int32_t)u_iscntrl(c); }},
        {"isspace", [](UChar32 c) { return (int32_t)u_isspace(c); }},
        {"iswhitespace", [](UChar32 c) { return (int32_t)u_isWhitespace(c); }},
        {"script", [](UChar32 c) { UErrorCode st = U_ZERO_ERROR; int32_t v = uscript_getScript(c, &st); return U_FAILURE(st) ? -1 : v; }},
    };
    for (const Prop& p : props) {
        UChar32 start = 0;
        int32_t cur = p.f(0);
        for (UChar32 c = 1; c <= 0x110000; c++) {
            int32_t v = c < 0x110000 ? p.f(c) : cur - 1;
            if (v != cur) {
                printf("prop %s %x %x %d\n", p.name, start, c - 1, cur);
                start = c;
                cur = v;
            }
        }
    }
}

// ---- case (SkUnicode_icu.cpp#L495-L520) ---------------------------------------------------------------

std::string upper(const std::vector<UChar>& in, const char* locale) {
    std::vector<UChar> out(in.size() * 3 + 16);
    UErrorCode st = U_ZERO_ERROR;
    int32_t n = u_strToUpper(out.data(), (int32_t)out.size(), text_ptr(in), (int32_t)in.size(), locale, &st);
    if (U_FAILURE(st)) return std::string("error:") + u_errorName(st);
    out.resize(n);
    std::string r;
    char buf[8];
    for (UChar u : out) { snprintf(buf, sizeof buf, "%04X", u); if (!r.empty()) r += '.'; r += buf; }
    return r.empty() ? "-" : r;
}

void run_case(const std::vector<Source>& corpus) {
    const char* const cp_locales[] = {"", "tr", "az", "lt", "el"};
    for (const char* loc : cp_locales) {
        std::string name = *loc ? loc : "root";
        // Values: `=` the code point maps to itself, `-` a surrogate (not a scalar value), else the
        // result's UTF-16 units in hex joined by `.`. Only `=` and `-` runs span code points.
        UChar32 start = 0;
        std::string cur;
        bool have = false;
        for (UChar32 c = 0; c <= 0x110000; c++) {
            std::string v;
            if (c == 0x110000) {
                v = "end";
            } else if (c >= 0xD800 && c < 0xE000) {
                v = "-";
            } else {
                std::vector<UChar> u = to_utf16({(uint32_t)c});
                std::string r = upper(u, loc);
                std::string self;
                char buf[8];
                for (UChar x : u) { snprintf(buf, sizeof buf, "%04X", x); if (!self.empty()) self += '.'; self += buf; }
                v = r == self ? "=" : r;
            }
            if (!have) { cur = v; start = c; have = true; continue; }
            if (v == cur && (cur == "=" || cur == "-")) continue;
            printf("cp %s %x %x %s\n", name.c_str(), start, c - 1, cur.c_str());
            cur = v;
            start = c;
        }
    }
    const char* const str_locales[] = {"", "tr", "lt", "el"};
    for (const char* loc : str_locales) {
        std::string kase = std::string("upper-") + (*loc ? loc : "root");
        for (const Source& s : corpus) {
            for (size_t i = 0; i < s.strings.size(); i++) {
                std::vector<UChar> u = to_utf16(decode_utf8(s.strings[i]));
                emit_row(kase, s, i, u.size(), upper(u, loc));
            }
        }
    }
}

// ---- conformance ---------------------------------------------------------------------------------------

std::vector<std::string> split(const std::string& s, char sep) {
    std::vector<std::string> out;
    std::string cur;
    for (char c : s) {
        if (c == sep) { out.push_back(cur); cur.clear(); } else cur += c;
    }
    out.push_back(cur);
    return out;
}

void run_conformance(const std::string& ucd) {
    struct F { const char* file; const char* type; };
    const F files[] = {{"GraphemeBreakTest.txt", "char"}, {"WordBreakTest.txt", "word"},
                       {"SentenceBreakTest.txt", "sent"}, {"LineBreakTest.txt", "line"}};
    for (const F& f : files) {
        std::istringstream in(read_file(ucd + "/" + f.file));
        std::string line;
        int lineno = 0;
        int total = 0;
        std::vector<std::pair<int, std::string>> failing;  // line, rule of the first wrong slot
        while (std::getline(in, line)) {
            lineno++;
            std::string comment;
            size_t hash = line.find('#');
            if (hash != std::string::npos) { comment = line.substr(hash); line = line.substr(0, hash); }
            std::istringstream ls(line);
            std::string tok;
            std::string text;
            // Slot k is the position after k characters; `want[k]` says whether the file has a break there.
            std::vector<int> off;
            std::vector<bool> want;
            bool expectMark = true;
            while (ls >> tok) {
                if (expectMark) {
                    off.push_back((int)text.size());
                    want.push_back(tok == "\xC3\xB7");  // ÷
                    expectMark = false;
                } else {
                    append_utf8(text, hexval(tok, 0, tok.size()));
                    expectMark = true;
                }
            }
            if (off.empty()) continue;
            total++;
            // ICU always reports the boundary at 0 (the line test files start with x by LB2).
            want[0] = true;
            // The rules the test file names, one per slot: `[9.0]`.
            std::vector<std::string> rules;
            for (size_t i = 0; i < comment.size(); i++) {
                if (comment[i] != '[') continue;
                size_t j = comment.find(']', i);
                if (j == std::string::npos) break;
                std::string r = comment.substr(i + 1, j - i - 1);
                if (!r.empty() && r.find_first_not_of("0123456789.") == std::string::npos) rules.push_back(r);
                i = j;
            }
            int n = 0;
            std::string got = enumerate(f.type, false, text, &n, false);
            std::vector<int> actual;
            for (const std::string& p : split(got, ' ')) actual.push_back(atoi(p.c_str()));
            for (size_t k = 0; k < off.size(); k++) {
                bool has = std::find(actual.begin(), actual.end(), off[k]) != actual.end();
                if (has != want[k]) {
                    failing.push_back({lineno, k < rules.size() ? rules[k] : std::string("?")});
                    break;
                }
            }
        }
        printf("conf %s total %d\n", f.file, total);
        for (auto& l : failing) printf("conf %s fail %d %s\n", f.file, l.first, l.second.c_str());
    }
    // BidiCharacterTest.txt: the levels of every code point not removed by X9 ('x').
    {
        std::istringstream in(read_file(ucd + "/BidiCharacterTest.txt"));
        std::string line;
        int lineno = 0;
        int total = 0;
        std::vector<int> failing;
        while (std::getline(in, line)) {
            lineno++;
            if (line.empty() || line[0] == '#') continue;
            std::vector<std::string> f = split(line, ';');
            if (f.size() < 5) continue;
            total++;
            std::vector<uint32_t> cps;
            std::istringstream cs(f[0]);
            std::string tok;
            while (cs >> tok) cps.push_back(hexval(tok, 0, tok.size()));
            uint8_t level;
            if (f[1] == "0") level = UBIDI_LTR;
            else if (f[1] == "1") level = UBIDI_RTL;
            else level = UBIDI_DEFAULT_LTR;
            std::vector<UChar> u16 = to_utf16(cps);
            UErrorCode st = U_ZERO_ERROR;
            UBiDi* bidi = ubidi_openSized((int32_t)u16.size(), 0, &st);
            ubidi_setPara(bidi, text_ptr(u16), (int32_t)u16.size(), level, nullptr, &st);
            bool ok = !U_FAILURE(st);
            if (ok) {
                std::istringstream ls(f[3]);
                size_t unit = 0;
                for (size_t k = 0; k < cps.size() && ok; k++) {
                    std::string lv;
                    ls >> lv;
                    if (lv != "x" && (int)ubidi_getLevelAt(bidi, (int32_t)unit) != atoi(lv.c_str())) ok = false;
                    unit += cps[k] >= 0x10000 ? 2 : 1;
                }
            }
            ubidi_close(bidi);
            if (!ok) failing.push_back(lineno);
        }
        printf("conf BidiCharacterTest.txt total %d\n", total);
        for (int l : failing) printf("conf BidiCharacterTest.txt fail %d -\n", l);
    }
}

// ---- misc ----------------------------------------------------------------------------------------------

void run_misc() {
    for (int code = -128; code <= 40; code++) {
        const char* n = u_errorName((UErrorCode)code);
        printf("u_errorName %d %s\n", code, n);
    }
    const char* tags[] = {"en", "en-US", "en-us", "EN-us", "tr", "zh-Hans-CN", "zh-Hant-TW", "ja-JP", "und",
                          "ru", "ar-EG", "sr-Latn-RS", "de-DE-u-co-phonebk", "en-u-lb-strict", "x-private",
                          "", "!!", "en_US", "th-TH-u-nu-thai", "pt-BR", "he", "fa-IR", "hi-IN", "ko-KR"};
    for (const char* t : tags) {
        char buf[ULOC_FULLNAME_CAPACITY];
        UErrorCode st = U_ZERO_ERROR;
        int32_t parsed = 0;
        int32_t n = uloc_forLanguageTag(t, buf, sizeof buf, &parsed, &st);
        printf("uloc_forLanguageTag \"%s\" -> \"%s\" len=%d parsed=%d status=%s\n", t, U_FAILURE(st) ? "" : buf,
               n, parsed, u_errorName(st));
    }
    printf("uloc_getDefault %s\n", uloc_getDefault());
}

}  // namespace

int main(int argc, char** argv) {
    std::string data, corpus_dir, ucd, section;
    for (int i = 1; i < argc; i++) {
        std::string a = argv[i];
        if (a == "--data") data = argv[++i];
        else if (a == "--corpus") corpus_dir = argv[++i];
        else if (a == "--ucd") ucd = argv[++i];
        else if (a == "--dump") g_dump = true;
        else section = a;
    }
    // ICU needs the data 16-byte aligned (it is the DataHeader's alignment); the file is kept for the process.
    std::string bytes = read_file(data);
    void* mem = nullptr;
    if (posix_memalign(&mem, 16, bytes.size() + 16) != 0) die("out of memory");
    memcpy(mem, bytes.data(), bytes.size());
    UErrorCode st = U_ZERO_ERROR;
    udata_setCommonData(mem, &st);
    if (U_FAILURE(st)) die(std::string("udata_setCommonData: ") + u_errorName(st));
    uloc_setDefault("en_US", &st);

    if (section == "breaks") run_breaks(read_corpus(corpus_dir));
    else if (section == "bidi") run_bidi(read_corpus(corpus_dir));
    else if (section == "reorder") run_reorder();
    else if (section == "props") run_props();
    else if (section == "case") run_case(read_corpus(corpus_dir));
    else if (section == "conformance") run_conformance(ucd);
    else if (section == "misc") run_misc();
    else die("unknown section `" + section + "`");
    return 0;
}
