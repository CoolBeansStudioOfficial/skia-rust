// Skia side of the analytic AA scan conversion tests (task C3): runs the cases of
// crates/skia-rust-raster/src/scan_aaa_tests/cases.txt through Skia's own scan converters with a
// blitter that records every call, and prints, per case, `== <name>` followed by one line per
// blitter call (the format of skia_rust_raster::blitter_dump::DumpBlitter::oracle_text). The Rust test
// (crates/skia-rust-raster/src/scan_aaa_tests.rs) runs the same cases through skia-rust and
// compares with this output, committed as scan_aaa_tests/skia_dump.txt.
//
// Case format (one directive per line, `#` comments):
//   case <name>
//   clip l t r b [+ l t r b ...]   region (union of rects); or `noclip` (rect ops only)
//   fill winding|evenodd|invwinding|invevenodd
//   op path|rle|rect|xrect|frame   AntiFillPath (forceRLE false/true), AntiFillRect,
//                                  AntiFillXRect, AntiFrameRect
//   M x y | L x y | Q x1 y1 x2 y2 | C x1 y1 x2 y2 x3 y3 | K x1 y1 x2 y2 w | Z
//   circle cx cy r | circleccw cx cy r | oval l t r b | rrect l t r b rx ry
//   rect l t r b | xrect l t r b (16.16 ints) | frame l t r b strokeX strokeY
//   end
//
// Build and run (Windows): oracle/scan-aaa/build.ps1.

#include "include/core/SkPath.h"
#include "include/core/SkPathBuilder.h"
#include "include/core/SkRRect.h"
#include "include/core/SkRect.h"
#include "include/core/SkRegion.h"
#include "src/core/SkBlitter.h"
#include "src/core/SkMask.h"
#include "src/core/SkPathPriv.h"
#include "src/core/SkPathRaw.h"
#include "src/core/SkRasterClip.h"

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

#include "src/core/SkScan.h"

// The region overloads (and forceRLE) are private to SkScan. Explicit instantiations may name
// private members, so these templates hand out pointers to them.
using AntiFillPathProc = void (*)(const SkPathRaw&, const SkRegion&, SkBlitter*, bool);
using AntiFillRectProc = void (*)(const SkRect&, const SkRegion*, SkBlitter*);
using AntiFillXRectProc = void (*)(const SkXRect&, const SkRegion*, SkBlitter*);
using AntiFrameRectProc = void (*)(const SkRect&, const SkPoint&, const SkRegion*, SkBlitter*);

template <typename Tag, typename Tag::type Ptr> struct Steal {
    friend typename Tag::type get(Tag) { return Ptr; }
};
struct AntiFillPathTag {
    using type = AntiFillPathProc;
    friend type get(AntiFillPathTag);
};
struct AntiFillRectTag {
    using type = AntiFillRectProc;
    friend type get(AntiFillRectTag);
};
struct AntiFillXRectTag {
    using type = AntiFillXRectProc;
    friend type get(AntiFillXRectTag);
};
struct AntiFrameRectTag {
    using type = AntiFrameRectProc;
    friend type get(AntiFrameRectTag);
};
template struct Steal<AntiFillPathTag, &SkScan::AntiFillPath>;
template struct Steal<AntiFillRectTag, &SkScan::AntiFillRect>;
template struct Steal<AntiFillXRectTag, &SkScan::AntiFillXRect>;
template struct Steal<AntiFrameRectTag, &SkScan::AntiFrameRect>;

class DumpBlitter final : public SkBlitter {
public:
    void blitH(int x, int y, int width) override { printf("blitH %d %d %d\n", x, y, width); }
    void blitAntiH(int x, int y, const SkAlpha aa[], const int16_t runs[]) override {
        printf("blitAntiH %d %d", x, y);
        for (int i = 0; runs[i] > 0; i += runs[i]) {
            printf(" %d:%d", aa[i], runs[i]);
        }
        printf("\n");
    }
    void blitV(int x, int y, int height, SkAlpha alpha) override {
        printf("blitV %d %d %d %d\n", x, y, height, alpha);
    }
    void blitRect(int x, int y, int width, int height) override {
        printf("blitRect %d %d %d %d\n", x, y, width, height);
    }
    void blitAntiRect(int x, int y, int width, int height, SkAlpha l, SkAlpha r) override {
        printf("blitAntiRect %d %d %d %d %d %d\n", x, y, width, height, l, r);
    }
    void blitMask(const SkMask& mask, const SkIRect& clip) override {
        static const char* kNames[] = {"BW", "A8", "3D", "ARGB32", "LCD16", "SDF"};
        const SkIRect& b = mask.fBounds;
        printf("blitMask %s %d %d %d %d clip %d %d %d %d\n", kNames[mask.fFormat], b.fLeft,
               b.fTop, b.fRight, b.fBottom, clip.fLeft, clip.fTop, clip.fRight, clip.fBottom);
        if (mask.fFormat == SkMask::kA8_Format) {
            for (int y = b.fTop; y < b.fBottom; ++y) {
                printf(" row %d:", y);
                const uint8_t* row = mask.fImage + (y - b.fTop) * mask.fRowBytes;
                for (int x = 0; x < b.width(); ++x) {
                    printf(" %02x", row[x]);
                }
                printf("\n");
            }
        }
    }
    void blitAntiH2(int x, int y, U8CPU a0, U8CPU a1) override {
        printf("blitAntiH2 %d %d %u %u\n", x, y, a0, a1);
    }
    void blitAntiV2(int x, int y, U8CPU a0, U8CPU a1) override {
        printf("blitAntiV2 %d %d %u %u\n", x, y, a0, a1);
    }
};

static float F(std::istringstream& in) {
    std::string s;
    in >> s;
    return strtof(s.c_str(), nullptr);
}

static int I(std::istringstream& in) {
    int v = 0;
    in >> v;
    return v;
}

int main(int argc, char** argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: scan_aaa <cases.txt>\n");
        return 1;
    }
    std::ifstream file(argv[1]);
    std::string line;
    std::string name, op;
    SkRegion clip;
    bool noClip = false;
    SkPathBuilder builder;
    std::vector<std::string> operands;
    while (std::getline(file, line)) {
        if (!line.empty() && line.back() == '\r') {
            line.pop_back();
        }
        std::istringstream in(line);
        std::string cmd;
        if (!(in >> cmd) || cmd[0] == '#') {
            continue;
        }
        if (cmd == "case") {
            in >> name;
            clip.setEmpty();
            noClip = false;
            builder.reset();
            operands.clear();
        } else if (cmd == "clip") {
            for (;;) {
                int l = I(in), t = I(in), r = I(in), b = I(in);
                clip.op(SkIRect::MakeLTRB(l, t, r, b), SkRegion::kUnion_Op);
                std::string plus;
                if (!(in >> plus)) {
                    break;
                }
            }
        } else if (cmd == "noclip") {
            noClip = true;
        } else if (cmd == "fill") {
            std::string f;
            in >> f;
            builder.setFillType(f == "winding"      ? SkPathFillType::kWinding
                                : f == "evenodd"    ? SkPathFillType::kEvenOdd
                                : f == "invwinding" ? SkPathFillType::kInverseWinding
                                                    : SkPathFillType::kInverseEvenOdd);
        } else if (cmd == "op") {
            in >> op;
        } else if (cmd == "M") {
            float x = F(in), y = F(in);
            builder.moveTo(x, y);
        } else if (cmd == "L") {
            float x = F(in), y = F(in);
            builder.lineTo(x, y);
        } else if (cmd == "Q") {
            float a = F(in), b = F(in), c = F(in), d = F(in);
            builder.quadTo(a, b, c, d);
        } else if (cmd == "C") {
            float a = F(in), b = F(in), c = F(in), d = F(in), e = F(in), f = F(in);
            builder.cubicTo(a, b, c, d, e, f);
        } else if (cmd == "K") {
            float a = F(in), b = F(in), c = F(in), d = F(in), w = F(in);
            builder.conicTo(a, b, c, d, w);
        } else if (cmd == "Z") {
            builder.close();
        } else if (cmd == "circle" || cmd == "circleccw") {
            float x = F(in), y = F(in), r = F(in);
            builder.addCircle(x, y, r,
                              cmd == "circle" ? SkPathDirection::kCW : SkPathDirection::kCCW);
        } else if (cmd == "oval") {
            float l = F(in), t = F(in), r = F(in), b = F(in);
            builder.addOval(SkRect::MakeLTRB(l, t, r, b));
        } else if (cmd == "rrect") {
            float l = F(in), t = F(in), r = F(in), b = F(in), rx = F(in), ry = F(in);
            builder.addRRect(SkRRect::MakeRectXY(SkRect::MakeLTRB(l, t, r, b), rx, ry));
        } else if (cmd == "rect" || cmd == "xrect" || cmd == "frame") {
            operands.push_back(line);
        } else if (cmd == "end") {
            printf("== %s\n", name.c_str());
            DumpBlitter blitter;
            const SkRegion* rgn = noClip ? nullptr : &clip;
            if (op == "path" || op == "rle") {
                SkPath path = builder.detach();
                auto raw = SkPathPriv::Raw(path, SkResolveConvexity::kYes);
                if (!raw) {
                    printf("(no raw)\n");
                    continue;
                }
                if (op == "path") {
                    SkScan::AntiFillPath(*raw, SkRasterClip(clip), &blitter);
                } else {
                    get(AntiFillPathTag{})(*raw, clip, &blitter, true);
                }
            } else {
                for (const std::string& o : operands) {
                    std::istringstream oin(o);
                    std::string kind;
                    oin >> kind;
                    if (kind == "rect") {
                        float l = F(oin), t = F(oin), r = F(oin), b = F(oin);
                        get(AntiFillRectTag{})(SkRect::MakeLTRB(l, t, r, b), rgn, &blitter);
                    } else if (kind == "xrect") {
                        int l = I(oin), t = I(oin), r = I(oin), b = I(oin);
                        get(AntiFillXRectTag{})(SkIRect::MakeLTRB(l, t, r, b), rgn, &blitter);
                    } else {
                        float l = F(oin), t = F(oin), r = F(oin), b = F(oin);
                        float sx = F(oin), sy = F(oin);
                        get(AntiFrameRectTag{})(SkRect::MakeLTRB(l, t, r, b), {sx, sy}, rgn,
                                                &blitter);
                    }
                }
            }
        }
    }
    return 0;
}
