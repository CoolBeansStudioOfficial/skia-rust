#!/usr/bin/env python3
"""Generates crates/skia-rust-raster/src/aa_clip_tests/cases.txt for oracle/aaclip/aaclip.cpp.

Deterministic: all randomness comes from the LCG below (Knuth's MMIX constants).
Usage:
  python gen_cases.py [out_path]            write the case script
  python gen_cases.py --check dump.txt      verify '#chk' bounds comments against a Skia dump
The case format is documented at the top of aaclip.cpp.
"""
import math
import os
import sys

M64 = (1 << 64) - 1


class Rng:
    def __init__(self, seed):
        self.s = seed & M64

    def u32(self):
        self.s = (self.s * 6364136223846793005 + 1442695040888963407) & M64
        return self.s >> 32

    def rint(self, lo, hi):
        return lo + self.u32() % (hi - lo + 1)

    def chance(self, num, den):
        return self.u32() % den < num

    def pick(self, seq):
        return seq[self.u32() % len(seq)]

    def frac(self):
        # Fractions that give robustly partial coverage (never 0, never 1 after supersampling).
        return self.pick([0.3, 0.35, 0.4, 0.6, 0.65, 0.7])

    def fl(self, lo, hi):
        return lo + (self.u32() % int((hi - lo) * 100 + 1)) / 100.0


def f(v):
    if abs(v - round(v)) < 1e-9:
        return str(int(round(v)))
    return ("%.4f" % v).rstrip("0").rstrip(".")


def fs(*vs):
    return " ".join(f(v) for v in vs)


# ---------------------------------------------------------------------------------------------
# Tracking of what a clip must contain (used only to emit valid blits). A tracked clip is
#   dict {(x,y): depth}  exact set of pixels with non-zero coverage; depth 0 = fully covered,
#                        depth>0 = partial coverage (product of that many fractions)
#   ('BB', (l,t,r,b))    only the exact bounding box is known
#   None                 unknown
# ---------------------------------------------------------------------------------------------
def px_rect(l, t, r, b):
    return {(x, y): 0 for y in range(t, b) for x in range(l, r)}


def px_frac(l, t, r, b):
    d = {}
    for y in range(math.floor(t), math.ceil(b)):
        for x in range(math.floor(l), math.ceil(r)):
            ex = 0 if (x >= l and x + 1 <= r) else 1
            ey = 0 if (y >= t and y + 1 <= b) else 1
            d[(x, y)] = ex + ey
    return d


def t_intersect(a, b):
    if not isinstance(a, dict) or not isinstance(b, dict):
        return None
    out = {}
    for p, da in a.items():
        if p in b:
            d = da + b[p]
            if d > 2:
                return None
            out[p] = d
    return out


def t_diff(a, b):
    if not isinstance(a, dict) or not isinstance(b, dict):
        return None
    out = {}
    for p, da in a.items():
        if p in b:
            if b[p] == 0:
                continue
            d = da + 2
            if d > 2:
                return None
            out[p] = d
        else:
            out[p] = da
    return out


def t_translate(a, dx, dy):
    if isinstance(a, dict):
        return {(x + dx, y + dy): d for (x, y), d in a.items()}
    if isinstance(a, tuple):
        l, t, r, b = a[1]
        return ("BB", (l + dx, t + dy, r + dx, b + dy))
    return None


def t_bbox(a):
    if isinstance(a, tuple):
        return a[1]
    if isinstance(a, dict) and a:
        xs = [p[0] for p in a]
        ys = [p[1] for p in a]
        return (min(xs), min(ys), max(xs) + 1, max(ys) + 1)
    return None


def t_is_empty(a):
    return isinstance(a, dict) and not a


def region_px(op, rects):
    s = set()
    for (l, t, r, b) in rects:
        cur = {(x, y) for y in range(t, b) for x in range(l, r)}
        s = (s | cur) if op == "union" else (s ^ cur)
    return {p: 0 for p in s}


def spec(op, rects):
    return op + " " + " + ".join(fs(*r) for r in rects)


# ---------------------------------------------------------------------------------------------
class Case:
    def __init__(self, name):
        self.name = name
        self.lines = []
        self.st = {"aa": {}, "aa2": {}}

    def e(self, s):
        self.lines.append(s)

    def text(self):
        return "case %s\n%s\nend\n" % (self.name, "\n".join(self.lines))

    # --- aa commands with tracking
    def aa_empty(self, s="aa"):
        self.e("%s empty" % s)
        self.st[s] = {}

    def aa_rect(self, box, s="aa"):
        self.e("%s rect %s" % (s, fs(*box)))
        l, t, r, b = box
        self.st[s] = px_rect(l, t, r, b) if l < r and t < b and r - l < 5000 and b - t < 5000 else {}

    def aa_region(self, op, rects, s="aa"):
        self.e("%s region %s" % (s, spec(op, rects)))
        self.st[s] = region_px(op, rects)

    def aa_path_frac(self, fbox, aa, bounds, s="aa"):
        """Path = one fractional rect (sets the builder); setPath with the given bounds."""
        self.e("path winding")
        self.e("rect %s" % fs(*fbox))
        self.e("%s path %s %s" % (s, "aa" if aa else "bw", fs(*bounds)))
        if aa:
            cov = px_frac(*fbox)
        else:
            l, t, r, b = [int(math.floor(v + 0.5)) for v in fbox]
            cov = px_rect(l, t, r, b) if l < r and t < b else {}
        self.st[s] = t_intersect(cov, px_rect(*bounds))

    def aa_path_generic(self, bounds, aa=True, s="aa"):
        self.e("%s path %s %s" % (s, "aa" if aa else "bw", fs(*bounds)))
        self.st[s] = None

    def aa_path_hole_bb(self, fbox, s="aa"):
        """Frac rect with an inner circle hole (evenodd): bounds known exactly."""
        l, t, r, b = fbox
        cx, cy = (l + r) / 2, (t + b) / 2
        rad = max(1.0, min(r - l, b - t) / 4)
        self.e("path evenodd")
        self.e("rect %s" % fs(*fbox))
        self.e("circle %s %s %s" % (f(cx), f(cy), f(rad)))
        ib = (math.floor(l), math.floor(t), math.ceil(r), math.ceil(b))
        self.e("%s path aa %s" % (s, fs(*ib)))
        self.st[s] = ("BB", ib)

    def aa_opi(self, box, op, s="aa"):
        self.e("%s opi %s %s" % (s, fs(*box), op))
        l, t, r, b = box
        cur = self.st[s]
        rect = px_rect(l, t, r, b) if l < r and t < b else {}
        if op == "intersect":
            if isinstance(cur, tuple):
                bb = cur[1]
                self.st[s] = cur if (l <= bb[0] and t <= bb[1] and r >= bb[2] and b >= bb[3]) else None
            else:
                self.st[s] = t_intersect(cur, rect)
        else:
            self.st[s] = t_diff(cur, rect) if rect else cur if isinstance(cur, dict) else None

    def aa_opf(self, fbox, op, aa, s="aa"):
        self.e("%s opf %s %s %s" % (s, fs(*fbox), op, "aa" if aa else "bw"))
        if aa:
            cov = px_frac(*fbox)
        else:
            l, t, r, b = [int(math.floor(v + 0.5)) for v in fbox]
            cov = px_rect(l, t, r, b) if l < r and t < b else {}
        cur = self.st[s]
        self.st[s] = t_intersect(cur, cov) if op == "intersect" else t_diff(cur, cov)

    def aa_opaa(self, s, o, op):
        self.e("%s opaa %s %s" % (s, o, op))
        a, b = self.st[s], self.st[o]
        self.st[s] = t_intersect(a, b) if op == "intersect" else t_diff(a, b)

    def aa_translate(self, s, dx, dy):
        self.e("%s translate %d %d" % (s, dx, dy))
        self.st[s] = t_translate(self.st[s], dx, dy)

    def aa_translateto(self, s, o, dx, dy):
        self.e("%s translateto %s %d %d" % (s, o, dx, dy))
        self.st[o] = t_translate(self.st[s], dx, dy)

    def aa_copy(self, s, o):
        self.e("%s copy %s" % (s, o))
        self.st[s] = self.st[o]

    # --- blits (always on slot `aa`)
    def blit_check(self):
        st = self.st["aa"]
        if t_is_empty(st):
            self.e("#chk empty")
            return None
        bb = t_bbox(st)
        if bb is None:
            return None
        self.e("#chk %s" % fs(*bb))
        return bb

    def can_blit(self):
        st = self.st["aa"]
        return t_is_empty(st) or t_bbox(st) is not None

    def blit_rand(self, rng, kind=None):
        bb = self.blit_check()
        if bb is None:
            self.e("blit h 0 0 1")  # empty clip: `skip empty`
            return
        l, t, r, b = bb
        kind = kind or rng.pick(["h", "antih", "v", "rect", "mask", "mask", "antih"])
        if kind == "h":
            y = rng.rint(t, b - 1)
            x = rng.rint(l, r - 1)
            w = rng.rint(1, r - x)
            self.e("blit h %d %d %d" % (x, y, w))
        elif kind == "antih":
            y = rng.rint(t, b - 1)
            x = rng.rint(l, r - 1)
            n = rng.rint(1, r - x)
            runs = []
            left = n
            while left > 0:
                k = rng.rint(1, min(left, rng.pick([1, 2, 3, 5, 8, 40])))
                a = rng.pick([0, 1, 64, 127, 128, 200, 254, 255, rng.rint(1, 255)])
                runs.append("%d:%d" % (a, k))
                left -= k
            self.e("blit antih %d %d %s" % (x, y, " ".join(runs)))
        elif kind == "v":
            x = rng.rint(l, r - 1)
            y = rng.rint(t, b - 1)
            h = rng.rint(1, b - y)
            self.e("blit v %d %d %d %d" % (x, y, h, rng.pick([1, 90, 128, 255, rng.rint(1, 255)])))
        elif kind == "rect":
            x = rng.rint(l, r - 1)
            y = rng.rint(t, b - 1)
            w = rng.rint(1, r - x)
            h = rng.rint(1, b - y)
            self.e("blit rect %d %d %d %d" % (x, y, w, h))
        else:
            fmt = rng.pick(["a8", "bw"])
            st = self.st["aa"]
            clip = None
            if isinstance(st, dict) and rng.chance(1, 2):
                for _ in range(40):  # a clip fully inside the aa clip: quickContains often true
                    cl = rng.rint(l, r - 1)
                    ct = rng.rint(t, b - 1)
                    cr = rng.rint(cl + 1, min(r, cl + 12))
                    cb = rng.rint(ct + 1, min(b, ct + 12))
                    if all(st.get((x, y)) == 0 for x in range(cl, cr) for y in range(ct, cb)):
                        clip = (cl, ct, cr, cb)
                        break
            if clip is None:
                cl = rng.rint(l, r - 1)
                ct = rng.rint(t, b - 1)
                cr = rng.rint(cl + 1, min(r, cl + 14))
                cb = rng.rint(ct + 1, min(b, ct + 14))
                clip = (cl, ct, cr, cb)
            cl, ct, cr, cb = clip
            mx, my = cl - rng.rint(0, 2), ct - rng.rint(0, 2)
            mw = cr - mx + rng.rint(0, 2)
            mh = cb - my + rng.rint(0, 2)
            rb = mw if fmt == "a8" else (mw + 7) // 8
            data = []
            for _ in range(rb * mh):
                data.append(rng.pick([0, 255, 255, rng.rint(0, 255), rng.rint(0, 255)]) if fmt == "a8"
                            else rng.pick([0, 255, rng.rint(0, 255)]))
            self.e("blit mask %s %d %d %d %d %d %d %d %d %s"
                   % (fmt, mx, my, mw, mh, cl, ct, cr, cb, " ".join("%02x" % v for v in data)))


# ---------------------------------------------------------------------------------------------
# Shapes (path builder lines) and matrices
SHAPES = ["circle", "oval", "rrect", "rect", "tri", "quad", "cubic", "conic", "star", "hole", "ring",
          "invcircle", "invrect", "multi", "sliver", "circleccw"]


def shape(rng, kind, l, t, r, b):
    cx, cy = (l + r) / 2, (t + b) / 2
    w, h = r - l, b - t
    rad = min(w, h) / 2
    if kind == "circle":
        return ["path winding", "circle %s" % fs(cx, cy, rad)]
    if kind == "circleccw":
        return ["path winding", "circleccw %s" % fs(cx, cy, rad)]
    if kind == "oval":
        return ["path winding", "oval %s" % fs(l, t, r, b)]
    if kind == "rrect":
        return ["path winding", "rrect %s" % fs(l, t, r, b, w / 4, h / 3)]
    if kind == "rect":
        return ["path winding", "rect %s" % fs(l, t, r, b)]
    if kind == "tri":
        return ["path winding", "M %s" % fs(l, b), "L %s" % fs(cx, t), "L %s" % fs(r, b - h / 5), "Z"]
    if kind == "quad":
        return ["path winding", "M %s" % fs(l, b), "Q %s" % fs(cx, t - h / 2, r, b), "Z"]
    if kind == "cubic":
        return ["path winding", "M %s" % fs(l, cy), "C %s" % fs(l + w / 4, t - h / 3, r - w / 4, b + h / 3, r, cy), "Z"]
    if kind == "conic":
        return ["path winding", "M %s" % fs(l, b), "K %s" % fs(l, t, r, t, 0.7), "L %s" % fs(r, b), "Z"]
    if kind == "star":
        pts = []
        for i in range(5):
            a = -math.pi / 2 + i * 4 * math.pi / 5
            pts.append((round(cx + rad * math.cos(a), 2), round(cy + rad * math.sin(a), 2)))
        out = ["path evenodd", "M %s" % fs(*pts[0])]
        out += ["L %s" % fs(*p) for p in pts[1:]]
        return out + ["Z"]
    if kind == "hole":
        return ["path evenodd", "rect %s" % fs(l, t, r, b),
                "rect %s" % fs(l + w / 4, t + h / 4, r - w / 4, b - h / 4)]
    if kind == "ring":
        return ["path winding", "circle %s" % fs(cx, cy, rad), "circleccw %s" % fs(cx, cy, rad / 2)]
    if kind == "invcircle":
        return ["path invwinding", "circle %s" % fs(cx, cy, rad)]
    if kind == "invrect":
        return ["path invevenodd", "rect %s" % fs(l + w / 4, t + h / 4, r - w / 4, b - h / 4)]
    if kind == "multi":
        return ["path winding", "rect %s" % fs(l, t, l + w / 3, t + h / 2),
                "circle %s" % fs(r - w / 4, b - h / 4, min(w, h) / 5),
                "rrect %s" % fs(l + w / 3, t, r, t + h / 4, 2, 2)]
    if kind == "sliver":
        return ["path winding", "rect %s" % fs(l, cy, r, cy + 0.3)]
    raise ValueError(kind)


def rbox(rng, lo, hi, minsz=4):
    """Random float box inside [lo,hi] with 1/4 grid coords."""
    l = lo + rng.rint(0, (hi - lo - minsz) * 2) / 2.0
    t = lo + rng.rint(0, (hi - lo - minsz) * 2) / 2.0
    r = min(hi, l + minsz + rng.rint(0, (hi - minsz) * 2) / 2.0)
    b = min(hi, t + minsz + rng.rint(0, (hi - minsz) * 2) / 2.0)
    return (l, t, r, b)


def ibox(rng, lo, hi, minsz=2):
    l = rng.rint(lo, hi - minsz)
    t = rng.rint(lo, hi - minsz)
    r = rng.rint(l + minsz, hi)
    b = rng.rint(t + minsz, hi)
    return (l, t, r, b)


def mat(rng, kind, cx, cy):
    if kind == "I":
        return "I"
    if kind == "scale":
        sx, sy = rng.pick([0.5, 0.75, 1.5, 2]), rng.pick([0.5, 1.25, 2, 0.8])
        return "M9 %s" % fs(sx, 0, cx - sx * cx, 0, sy, cy - sy * cy, 0, 0, 1)
    if kind == "rot":
        c, s = rng.pick([(0.8, 0.6), (0.7071, 0.7071), (0.6, -0.8), (0.9511, 0.3090)])
        return "M9 %s" % fs(c, -s, round(cx - c * cx + s * cy, 4), s, c, round(cy - s * cx - c * cy, 4), 0, 0, 1)
    if kind == "rot90":
        return "M9 %s" % fs(0, -1, cx + cy, 1, 0, cy - cx, 0, 0, 1)
    if kind == "skew":
        return "M9 %s" % fs(1, 0.25, -0.25 * cy, 0.1, 1, -0.1 * cx, 0, 0, 1)
    if kind == "trans":
        return "M9 %s" % fs(1, 0, rng.rint(-4, 4) + 0.5, 0, 1, rng.rint(-4, 4) + 0.25, 0, 0, 1)
    if kind == "persp":
        return "M9 %s" % fs(1, 0, 0, 0, 1, 0, 0.004, 0.002, 1)
    raise ValueError(kind)


MATS = ["I", "scale", "rot", "skew", "trans", "rot90", "persp"]

# ---------------------------------------------------------------------------------------------
cases = []


def new(name):
    c = Case(name)
    cases.append(c)
    return c


def section1():
    rng = Rng(0xAAC11001)
    # test_empty / degenerate
    c = new("aatest_empty")
    c.aa_empty()
    c.e("aa opi 0 0 10 10 intersect")
    c.e("aa opi 0 0 10 10 diff")
    c.e("aa translate 3 4")
    c.aa_rect((0, 0, 0, 0))
    c.aa_rect((5, 5, 3, 3))
    c.aa_region("union", [(0, 0, 0, 0)])
    c.aa_region("xor", [(2, 2, 8, 8), (2, 2, 8, 8)])
    for i in range(4):
        c = new("aatest_empty_ops_%d" % i)
        c.aa_rect((2, 2, 12, 12))
        c.aa_opi((20, 20, 30, 30), "intersect")
        c.aa_opi((0, 0, 10, 10), "diff") if i % 2 else c.aa_opi((0, 0, 10, 10), "intersect")
        c.aa_opf((1.3, 1.3, 5.6, 5.6), "intersect", i % 2 == 0)
        c.e("aa qc 0 0 3 3")
    # test_regions: random xor regions; ops between pairs
    for i in range(10):
        c = new("aatest_regions_%02d" % i)
        n1, n2 = rng.rint(2, 9), rng.rint(2, 9)
        r1 = [ibox(rng, 0, 40, 3) for _ in range(n1)]
        r2 = [ibox(rng, 0, 40, 3) for _ in range(n2)]
        c.aa_region("xor", r1)
        c.aa_region("xor", r2, "aa2")
        c.e("aa qc %s" % fs(*ibox(rng, 0, 40, 2)))
        c.aa_opaa("aa", "aa2", "intersect")
        c.aa_region("xor", r1)
        c.aa_opaa("aa", "aa2", "diff")
        c.aa_region("xor", r1)
        c.e("aa copy aa2")
        c.e("aa qc %s" % fs(*ibox(rng, 0, 40, 2)))
    # circle / trapezoid regions through path bw
    for i in range(6):
        c = new("aatest_pathregion_%d" % i)
        box = rbox(rng, 0, 44, 10)
        c.e("path winding")
        if i % 2 == 0:
            cx, cy = (box[0] + box[2]) / 2, (box[1] + box[3]) / 2
            c.e("circle %s" % fs(cx, cy, min(box[2] - box[0], box[3] - box[1]) / 2))
        else:
            l, t, r, b = box
            c.e("M %s" % fs(l, b))
            c.e("L %s" % fs(l + 3, t))
            c.e("L %s" % fs(r - 3, t))
            c.e("L %s" % fs(r, b))
            c.e("Z")
        c.e("aa path bw 0 0 48 48")
        c.e("aa2 path bw 0 0 48 48")
        c.e("aa2 translate 5 -3")
        c.e("aa opaa aa2 %s" % ("intersect" if i < 3 else "diff"))
        c.aa_region("xor", [ibox(rng, 0, 40, 3) for _ in range(4)], "aa2")
        c.e("aa opaa aa2 intersect")
        c.e("aa opi %s %s" % (fs(*ibox(rng, 0, 40, 3)), "diff" if i % 2 else "intersect"))
    # test_rects: random rect pairs, ops
    for i in range(30):
        c = new("aatest_rects_%02d" % i)
        a = ibox(rng, 0, 40, 1)
        b = ibox(rng, 0, 40, 1)
        c.aa_rect(a)
        c.aa_opi(b, "diff" if i % 2 else "intersect")
        if i % 3 == 0:
            c.aa_opi(ibox(rng, 0, 40, 1), "diff" if rng.chance(1, 2) else "intersect")
    # path with hole (two rects)
    for aa in (True, False):
        for fill in ("evenodd", "winding"):
            c = new("aatest_path_hole_%s_%s" % ("aa" if aa else "bw", fill))
            c.e("path %s" % fill)
            c.e("rect 0 0 40 40")
            c.e("rect 8 8 32 32")
            c.e("aa pathbounds %s" % ("aa" if aa else "bw"))
            c.e("aa opi 4 4 30 30 intersect")
            c.e("path %s" % fill)
            c.e("rect 0.5 0.5 40.5 40.5")
            c.e("rect 8.25 8.25 31.75 31.75")
            c.e("aa pathbounds %s" % ("aa" if aa else "bw"))
    # rrect that is a rect, then opi
    for aa in (True, False):
        c = new("aatest_rrect_is_rect_%s" % ("aa" if aa else "bw"))
        c.e("path winding")
        c.e("rrect 0 0 20 20 0 0")
        c.e("aa pathbounds %s" % ("aa" if aa else "bw"))
        c.e("aa opi 5 5 15 15 intersect")
        c.e("aa opi 7 7 9 9 diff")
        c = new("aatest_rrect_is_rect_frac_%s" % ("aa" if aa else "bw"))
        c.e("path winding")
        c.e("rrect 0.25 0.25 20.75 20.75 0 0")
        c.e("aa pathbounds %s" % ("aa" if aa else "bw"))
        c.e("aa opi 5 5 15 15 intersect")
    # nearly integral rect ops on rc and aa
    near = [0.0001, 0.00001, 0.000001, -0.00001, 0.4999, 0.5001, 0.9999, 0.99999, 0.999999, 0.0]
    for i in range(24):
        c = new("aatest_nearly_integral_%02d" % i)
        d = [rng.pick(near) for _ in range(4)]
        base = ibox(rng, 5, 30, 4)
        fb = (base[0] + d[0], base[1] + d[1], base[2] - d[2] if i % 2 else base[2] + d[2], base[3] + d[3])
        init = rng.pick(["irect", "path"])
        if init == "irect":
            c.e("rc new irect 0 0 36 36")
        else:
            c.e("path winding")
            c.e("circle 18 18 17")
            c.e("rc new path aa 0 0 36 36")
        c.e("rc oprect %s I %s %s" % (fs(*fb), rng.pick(["intersect", "diff"]), rng.pick(["aa", "bw"])))
        c.e("rc oprect %s I intersect %s" % (fs(*base), rng.pick(["aa", "bw"])))
        c.aa_rect((0, 0, 36, 36))
        c.aa_opf(fb, rng.pick(["intersect", "diff"]), i % 3 != 0)
    # huge setRect
    c = new("aatest_huge_rect")
    c.e("aa rect -1879048192 -1879048192 1879048192 1879048192")
    c.e("aa rect -1879048192 0 1879048192 10")
    c.e("aa rect 0 0 10 10")
    c.e("aa opi -1879048192 -1879048192 1879048192 1879048192 intersect")
    # large path, small clip
    for aa in ("aa", "bw"):
        c = new("aatest_large_path_small_clip_%s" % aa)
        c.e("path winding")
        c.e("rect -1000 10 2147483647 20")
        c.e("aa path %s 5 5 15 15" % aa)
        c.e("rc new path %s 5 5 15 15" % aa)
        c.e("rc new irect 0 0 30 30")
        c.e("rc oppath I intersect %s" % aa)
        c = new("aatest_large_path_small_clip2_%s" % aa)
        c.e("path winding")
        c.e("rect -2147483647 -2147483647 2147483647 2147483647")
        c.e("aa path %s -3 -3 20 20" % aa)
        c.e("path winding")
        c.e("rect -100000 8.5 100000 11.5")
        c.e("aa path %s -3 -3 20 20" % aa)
    # crbug 422693
    for aa in ("aa", "bw"):
        c = new("aatest_crbug_422693_%s" % aa)
        c.e("rc new irect -25000 -25000 25000 25000")
        c.e("path winding")
        c.e("circle 50 50 50")
        c.e("rc oppath I intersect %s" % aa)
    # nearly-empty / thin shapes
    for i in range(6):
        c = new("aatest_thin_%d" % i)
        c.e("path winding")
        c.e("rect %s" % fs(10 + 0.3, 5, 10 + 0.3 + [0.01, 0.2, 0.5, 0.9, 1.0, 1.4][i], 25))
        c.e("aa pathbounds aa")
        c.e("aa opi 0 0 40 12 intersect")
        c.e("path winding")
        c.e("rect %s" % fs(2, 10.3, 30, 10.3 + [0.01, 0.2, 0.5, 0.9, 1.0, 1.4][i]))
        c.e("aa2 path aa 0 0 40 40")
        c.e("aa opaa aa2 diff")


# targets of the "every aa op on every kind of input" matrix
def setup_kind(c, kind, s, rng):
    if kind == "empty":
        c.aa_empty(s)
    elif kind == "rect":
        c.aa_rect((6, 6, 30, 26), s)
    elif kind == "region":
        c.aa_region("xor", [(2, 2, 24, 20), (10, 8, 34, 30), (14, 14, 20, 40)], s)
    elif kind == "soft":
        for ln in shape(rng, "circle", 4.2, 3.7, 33.3, 30.6):
            c.e(ln)
        c.aa_path_generic((0, 0, 40, 40), True, s)
    elif kind == "softrect":
        c.aa_path_frac((5.3, 6.4, 29.7, 25.6), True, (0, 0, 40, 40), s)
    elif kind == "softhole":
        c.aa_path_hole_bb((4.3, 5.4, 33.7, 31.6), s)
    else:
        raise ValueError(kind)


KINDS = ["empty", "rect", "soft", "region", "softrect", "softhole"]


def section2():
    rng = Rng(0xAAC11002)
    rects = {
        "inside": (10, 10, 22, 20),
        "overlap": (20, 14, 44, 38),
        "outside": (60, 60, 70, 70),
        "cover": (-5, -5, 60, 60),
        "empty": (12, 12, 12, 20),
        "thin": (0, 15, 50, 16),
    }
    frects = {
        "inside": (10.3, 10.6, 22.7, 20.4),
        "overlap": (20.6, 14.4, 44.3, 38.7),
        "outside": (60.5, 60.5, 70.5, 70.5),
        "edge": (6.3, 6.6, 29.7, 25.4),
        "tiny": (12.3, 12.3, 12.7, 12.7),
    }
    for k in KINDS:
        for rn, rb in rects.items():
            for op in ("intersect", "diff"):
                c = new("aaop_%s_opi_%s_%s" % (k, rn, op))
                setup_kind(c, k, "aa", rng)
                c.aa_opi(rb, op)
                if rn in ("inside", "overlap"):
                    c.aa_opi((8, 8, 26, 24), "diff" if op == "intersect" else "intersect")
        for rn, rb in frects.items():
            for op in ("intersect", "diff"):
                for aa in (True, False):
                    c = new("aaop_%s_opf_%s_%s_%s" % (k, rn, op, "aa" if aa else "bw"))
                    setup_kind(c, k, "aa", rng)
                    c.aa_opf(rb, op, aa)
        for k2 in KINDS[:4]:
            for op in ("intersect", "diff"):
                c = new("aaop_%s_opaa_%s_%s" % (k, k2, op))
                setup_kind(c, k, "aa", rng)
                setup_kind(c, k2, "aa2", rng)
                c.aa_opaa("aa", "aa2", op)
                if k2 == "soft":
                    c.aa_opaa("aa2", "aa", op)
        for dx, dy in ((0, 0), (5, 7), (-10, -3)):
            c = new("aaop_%s_translate_%d_%d" % (k, dx, dy))
            setup_kind(c, k, "aa", rng)
            c.aa_translate("aa", dx, dy)
            c.aa_translateto("aa", "aa2", dx + 1, dy - 1)
            c.aa_opaa("aa", "aa2", "diff")
            c.aa_copy("aa2", "aa")
        for qc in ((10, 10, 20, 20), (0, 0, 50, 50), (30, 26, 31, 27), (14, 14, 14, 20)):
            c = new("aaop_%s_qc_%s" % (k, "_".join(str(v).replace("-", "m") for v in qc)))
            setup_kind(c, k, "aa", rng)
            c.e("aa qc %s" % fs(*qc))
            c.aa_opi((9, 9, 25, 22), "intersect")
            c.e("aa qc %s" % fs(*qc))
    # setPath variety with many shapes, aa and bw, bounds smaller/larger than the path
    for sk in SHAPES:
        for aa in ("aa", "bw"):
            c = new("aapath_%s_%s" % (sk, aa))
            for ln in shape(rng, sk, 3.2, 4.7, 36.4, 31.3):
                c.e(ln)
            c.e("aa path %s 0 0 40 40" % aa)
            c.e("aa2 path %s 10 10 25 22" % aa)
            c.e("aa pathbounds %s" % aa)
            c.e("aa opaa aa2 intersect")
            c.e("aa opf 12.5 6.25 20.75 30.5 diff %s" % aa)


def rc_init(c, kind, rng):
    if kind == "bwrect":
        c.e("rc new irect 5 5 45 45")
    elif kind == "bwregion":
        c.e("rc new region xor 5 5 30 30 + 15 15 45 45")
    elif kind == "aasoft":
        c.e("path winding")
        c.e("circle 25 25 20")
        c.e("rc new path aa 0 0 50 50")
    elif kind == "aacomplex":
        for ln in shape(rng, "star", 3.5, 3.5, 47.5, 47.5):
            c.e(ln)
        c.e("rc new path aa 0 0 50 50")
    elif kind == "bwpath":
        c.e("path winding")
        c.e("circle 25 25 20")
        c.e("rc new path bw 0 0 50 50")
    elif kind == "empty":
        c.e("rc new empty")
    elif kind == "aahole":
        c.e("rc new irect 0 0 50 50")
        c.e("path winding")
        c.e("circle 25.5 24.5 15")
        c.e("rc oppath I diff aa")
    else:
        raise ValueError(kind)


RC_KINDS = ["bwrect", "bwregion", "aasoft", "aacomplex", "bwpath", "empty", "aahole"]


def section3():
    rng = Rng(0xAAC11003)
    ops = ("intersect", "diff")
    mats_n = [0]
    for init in RC_KINDS:
        for geo in ("rect", "rrect", "path", "region"):
            for op in ops:
                if geo == "region":
                    c = new("rcop_%s_region_%s" % (init, op))
                    rc_init(c, init, rng)
                    c.e("rc opregion %s %s" % (spec("xor", [(10, 10, 35, 40), (20, 5, 40, 25)]), op))
                    c.e("rc opregion %s %s" % (spec("union", [(0, 0, 3, 3)]), op))
                    c = new("rcop_%s_region_big_%s" % (init, op))
                    rc_init(c, init, rng)
                    c.e("rc opregion %s %s" % (spec("union", [(-10, -10, 60, 60)]), op))
                    c.e("rc opregion %s %s" % (spec("union", [(0, 0, 0, 0)]), op))
                    continue
                for aa in ("aa", "bw"):
                    mats_n[0] += 1
                    other = ("scale", "rot", "persp", "skew", "rot90")
                    for m in ("I", other[mats_n[0] % len(other)]):
                        if init == "aahole" and (aa == "bw" or geo == "rrect"):
                            continue
                        c = new("rcop_%s_%s_%s_%s_%s" % (init, geo, op, aa, m))
                        rc_init(c, init, rng)
                        box = rbox(rng, 6, 44, 10)
                        mt = mat(rng, m, 25, 25)
                        if geo == "rect":
                            c.e("rc oprect %s %s %s %s" % (fs(*box), mt, op, aa))
                        elif geo == "rrect":
                            c.e("rc oprrect %s %s %s %s %s %s" % (fs(*box), f((box[2] - box[0]) / 4), f((box[3] - box[1]) / 3), mt, op, aa))
                        else:
                            for ln in shape(rng, rng.pick(SHAPES), *box):
                                c.e(ln)
                            c.e("rc oppath %s %s %s" % (mt, op, aa))
                            if rng.chance(1, 2):
                                c.e("rc oprect 20 20 30.5 30.5 I intersect aa")
    # plain rc scenarios
    for init in RC_KINDS:
        for dx, dy in ((7, -4), (-20, 30)):
            c = new("rctrans_%s_%d_%d" % (init, dx, dy))
            rc_init(c, init, rng)
            c.e("rc translate %d %d" % (dx, dy))
            c.e("rc opi 10 10 30 30 intersect")
        c = new("rcset_%s" % init)
        rc_init(c, init, rng)
        c.e("rc opi 12 12 38 38 intersect")
        c.e("rc opi 20 20 25 25 diff")
        c.e("rc setrect 3 3 20 20")
        c.e("rc opi 5 5 10 10 diff")
        c.e("rc setempty")
        c.e("rc opi 0 0 10 10 intersect")
        c.e("rc setrect 0 0 0 0")
        c = new("rcnew_%s" % init)
        rc_init(c, init, rng)
        c.e("rc new region union 0 0 5 5 + 10 10 20 20")
        c.e("rc new irect 3 3 3 9")
        c.e("rc new empty")
        c.e("rc opi 0 0 10 10 diff")
    # rect-ish aa results collapsing back to bw (updateCacheAndReturnNonEmpty)
    for i in range(10):
        c = new("rccollapse_%d" % i)
        c.e("rc new irect 0 0 40 40")
        c.e("rc oprect %s I %s aa" % (fs(rng.rint(0, 10) + 0.0, 4.0, rng.rint(20, 40) + 0.0, 30.0), "intersect"))
        c.e("rc oprect %s I diff aa" % fs(*ibox(rng, 0, 40, 2)))
        c.e("rc oprect %s I intersect aa" % fs(*rbox(rng, 0, 40, 6)))
        c.e("rc opi %s intersect" % fs(*ibox(rng, 0, 40, 4)))
    for i in range(6):
        c = new("rcaa_to_diff_%d" % i)
        c.e("path winding")
        c.e("circle 20 20 %d" % (8 + i * 2))
        c.e("rc new path aa 0 0 40 40")
        c.e("rc opregion %s diff" % spec("union", [(0, 0, 40, 10)]))
        c.e("rc opi 5 5 35 35 intersect")
        c.e("rc oprect 0 0 40 40 I intersect aa")


def section4():
    rng = Rng(0xAAC11004)
    setups = ["softrect", "softhole", "region", "rect", "frac_diff", "frac_two"]
    for si, sk in enumerate(setups):
        for n in range(6):
            c = new("blit_%s_%d" % (sk, n))
            if sk == "softrect":
                c.aa_path_frac((rng.rint(2, 6) + rng.frac(), rng.rint(2, 6) + rng.frac(),
                                rng.rint(26, 34) + rng.frac(), rng.rint(22, 30) + rng.frac()), True, (0, 0, 50, 50))
            elif sk == "softhole":
                c.aa_path_hole_bb((rng.rint(2, 6) + rng.frac(), rng.rint(2, 6) + rng.frac(),
                                   rng.rint(26, 36) + rng.frac(), rng.rint(22, 34) + rng.frac()))
            elif sk == "region":
                c.aa_region("xor", [ibox(rng, 0, 36, 4) for _ in range(rng.rint(2, 6))])
            elif sk == "rect":
                c.aa_rect(ibox(rng, 0, 36, 6))
            elif sk == "frac_diff":
                c.aa_path_frac((3 + rng.frac(), 4 + rng.frac(), 30 + rng.frac(), 26 + rng.frac()), True, (0, 0, 50, 50))
                c.aa_opf((10 + rng.frac(), 9 + rng.frac(), 20 + rng.frac(), 18 + rng.frac()), "diff", True)
            else:
                c.aa_path_frac((3 + rng.frac(), 4 + rng.frac(), 30 + rng.frac(), 26 + rng.frac()), True, (0, 0, 50, 50))
                c.aa_path_frac((12 + rng.frac(), 9 + rng.frac(), 40 + rng.frac(), 33 + rng.frac()), True, (0, 0, 50, 50), "aa2")
                c.aa_opaa("aa", "aa2", rng.pick(["intersect", "diff"]))
            if not c.can_blit():
                c.lines.append("# (bounds unknown)")
                continue
            kinds = ["h", "antih", "v", "rect", "mask", "mask"] * 3
            for k in kinds:
                c.blit_rand(rng, k)
            for _ in range(4):
                bb = t_bbox(c.st["aa"])
                if bb:
                    c.e("aa qc %s" % fs(*ibox(rng, bb[0], max(bb[0] + 3, bb[2]), 1)))
    # blit on an empty aa
    c = new("blit_empty")
    c.aa_empty()
    c.blit_rand(rng, "h")
    c.aa_rect((3, 3, 3, 9))
    c.blit_rand(rng, "mask")
    # runs straddling every boundary of a region clip
    c = new("blit_straddle")
    c.aa_region("xor", [(0, 0, 30, 4), (5, 2, 12, 8), (20, 3, 26, 9), (8, 6, 22, 12)])
    for y in range(0, 12):
        c.e("#chk %s" % fs(*t_bbox(c.st["aa"])))
        c.e("blit h 0 %d 30" % y)
        c.e("blit antih 0 %d 255:3 128:7 64:5 200:15" % y)
    c.e("blit v 6 0 12 255")
    c.e("blit v 21 1 11 100")
    c.e("blit rect 0 0 30 12")
    c.e("blit rect 4 3 20 6")
    # single-pixel, wide mask etc.
    c = new("blit_masks_cases")
    c.aa_path_frac((2.3, 2.4, 28.6, 20.7), True, (0, 0, 40, 40))
    bb = c.blit_check()
    hexes = lambda n, v: " ".join(["%02x" % v] * n)
    c.e("blit mask a8 2 2 28 19 2 2 29 21 %s" % hexes(28 * 19, 255))
    c.e("blit mask a8 2 2 28 19 2 2 29 21 %s" % hexes(28 * 19, 128))
    c.e("blit mask a8 10 8 4 4 10 8 14 12 %s" % hexes(16, 255))
    c.e("blit mask a8 10 8 4 4 11 9 13 11 %s" % hexes(16, 77))
    c.e("blit mask bw 2 2 28 19 2 2 29 21 %s" % hexes(4 * 19, 255))
    c.e("blit mask bw 2 2 28 19 2 2 29 21 %s" % hexes(4 * 19, 0xAA))
    c.e("blit mask bw 10 8 9 4 10 8 19 12 %s" % hexes(2 * 4, 0xF0))
    c.e("blit mask bw 10 8 9 4 12 9 17 11 %s" % hexes(2 * 4, 0xF0))
    c.e("blit mask a8 5 5 1 1 5 5 6 6 ff")
    c.e("blit mask bw 5 5 1 1 5 5 6 6 80")
    c.e("blit mask bw 5 5 1 1 5 5 6 6 00")


def scan_geoms(rng, cmd):
    """Returns lines for one scan command, exercising a few geometries."""
    out = []
    if cmd in ("fillpath", "antifillpath", "hairpath", "antihairpath", "hairsquarepath",
               "antihairsquarepath", "hairroundpath", "antihairroundpath"):
        hair = "hair" in cmd
        kinds = ["circle", "rect", "tri", "star", "hole", "invcircle", "invrect", "cubic", "multi", "quad"]
        if not hair:
            kinds += ["oval", "rrect", "ring", "sliver"]
        for k in range(5):
            sk = kinds[(rng.rint(0, len(kinds) - 1))]
            box = rbox(rng, -8, 58, 8)
            out += shape(rng, sk, *box)
            out.append("scan %s" % cmd)
        # partly outside / fully outside / covering
        out += shape(rng, "circle", -20, -20, 70, 70)
        out.append("scan %s" % cmd)
        out += shape(rng, "tri", 60, 60, 80, 80)
        out.append("scan %s" % cmd)
        out += shape(rng, "invcircle", 12.5, 12.5, 38.5, 38.5)
        out.append("scan %s" % cmd)
        return out
    if cmd in ("fillirect",):
        for _ in range(5):
            out.append("scan fillirect %s" % fs(*ibox(rng, -10, 60, 1)))
        out += ["scan fillirect 0 0 50 50", "scan fillirect 20 20 20 30", "scan fillirect 100 100 120 120"]
        return out
    if cmd in ("fillxrect", "antifillxrect"):
        for _ in range(6):
            b = rbox(rng, -8, 58, 1)
            out.append("scan %s %s" % (cmd, " ".join(str(int(round(v * 65536))) for v in b)))
        out.append("scan %s %d %d %d %d" % (cmd, 10 * 65536 + 100, 10 * 65536 + 200, 30 * 65536 - 1, 30 * 65536 + 65535))
        return out
    if cmd in ("fillrect", "antifillrect", "hairrect", "antihairrect"):
        for _ in range(6):
            out.append("scan %s %s" % (cmd, fs(*rbox(rng, -8, 58, 1))))
        out += ["scan %s 10.25 10.25 10.75 30.5" % cmd, "scan %s 10 10 30 30" % cmd,
                "scan %s 0 0 50 50" % cmd, "scan %s 12.5 12.5 12.5 30" % cmd]
        return out
    if cmd == "filltriangle":
        for _ in range(8):
            out.append("scan filltriangle %s" % fs(*[rng.rint(-10, 60) + rng.pick([0, 0.5, 0.25]) for _ in range(6)]))
        out.append("scan filltriangle 10 10 40 10 25 40")
        out.append("scan filltriangle 10 10 20 20 30 30")
        return out
    if cmd in ("hairline", "antihairline"):
        for _ in range(8):
            n = rng.rint(2, 4)
            pts = [rng.rint(-15, 65) + rng.pick([0, 0.5, 0.25, 0.75]) for _ in range(2 * n)]
            out.append("scan %s %s" % (cmd, fs(*pts)))
        out += ["scan %s 5 25 45 25" % cmd, "scan %s 25 5 25 45" % cmd, "scan %s 0 0 50 50" % cmd,
                "scan %s -100 10 200 12" % cmd, "scan %s 10.5 10.5 10.5 30.5" % cmd,
                "scan %s 5.5 5.5 5.5 5.5" % cmd, "scan %s 8 40 20 8 40 40 8 40" % cmd]
        return out
    if cmd in ("framerect", "antiframerect"):
        for _ in range(7):
            b = rbox(rng, -8, 58, 6)
            out.append("scan %s %s %s" % (cmd, fs(*b), fs(rng.pick([1, 1.5, 2, 3.25, 0.5]), rng.pick([1, 2, 2.5, 4, 0.75]))))
        out += ["scan %s 10 10 40 40 1 1" % cmd, "scan %s 10.5 10.5 40.5 40.5 2 2" % cmd,
                "scan %s 10 10 14 14 3 3" % cmd, "scan %s 5 5 45 45 30 30" % cmd]
        return out
    raise ValueError(cmd)


SCAN_CMDS = ["fillpath", "antifillpath", "fillirect", "fillxrect", "fillrect", "antifillrect",
             "antifillxrect", "filltriangle", "hairline", "antihairline", "hairrect", "antihairrect",
             "hairpath", "antihairpath", "hairsquarepath", "antihairsquarepath", "hairroundpath",
             "antihairroundpath", "framerect", "antiframerect"]


def section5():
    rng = Rng(0xAAC11005)
    for init in ["aasoft", "bwregion", "aacomplex", "bwrect"]:
        for cmd in SCAN_CMDS:
            c = new("scan_%s_%s" % (init, cmd))
            rc_init(c, init, rng)
            for ln in scan_geoms(rng, cmd):
                c.e(ln)
    for cmd in SCAN_CMDS:
        c = new("scan_empty_%s" % cmd)
        c.e("rc new empty")
        for ln in scan_geoms(Rng(7), cmd)[:6]:
            c.e(ln)
    # an aa rc that collapsed to a rect, and a rc translated/offset
    for init in ["aasoft", "bwregion"]:
        for cmd in ["antifillpath", "fillpath", "antifillrect", "hairline", "antihairline", "filltriangle"]:
            c = new("scan_moved_%s_%s" % (init, cmd))
            rc_init(c, init, rng)
            c.e("rc translate -7 9")
            c.e("rc oprect 3 3 30.5 30.5 I intersect aa")
            for ln in scan_geoms(rng, cmd)[:8]:
                c.e(ln)


def section6():
    rng = Rng(0xAAC11006)
    for i in range(170):
        W = rng.rint(10, 26)
        c = new("rand_%03d" % i)
        steps = rng.rint(6, 13)
        for _ in range(steps):
            r = rng.rint(0, 99)
            s = rng.pick(["aa", "aa", "aa2"])
            o = "aa2" if s == "aa" else "aa"
            if r < 14:  # set an aa clip
                k = rng.rint(0, 5)
                if k == 0:
                    c.aa_rect(ibox(rng, 0, W, 1), s)
                elif k == 1:
                    c.aa_region(rng.pick(["union", "xor"]), [ibox(rng, 0, W, 1) for _ in range(rng.rint(1, 4))], s)
                elif k == 2:
                    fb = rbox(rng, 0, W, 3)
                    fb = tuple(int(v) + rng.frac() for v in fb)
                    if fb[2] - fb[0] < 1 or fb[3] - fb[1] < 1:
                        fb = (fb[0], fb[1], fb[0] + 2.5, fb[1] + 2.5)
                    c.aa_path_frac(fb, rng.chance(3, 4), (0, 0, W + 3, W + 3), s)
                elif k == 3:
                    fb = rbox(rng, 1, W, 4)
                    for ln in shape(rng, rng.pick(SHAPES), *fb):
                        c.e(ln)
                    c.aa_path_generic((0, 0, W + 2, W + 2), rng.chance(3, 4), s)
                elif k == 4:
                    fb = rbox(rng, 1, W, 4)
                    for ln in shape(rng, rng.pick(SHAPES), *fb):
                        c.e(ln)
                    c.e("%s pathbounds %s" % (s, rng.pick(["aa", "bw"])))
                    c.st[s] = None
                else:
                    c.aa_empty(s)
            elif r < 38:  # aa ops
                k = rng.rint(0, 8)
                op = rng.pick(["intersect", "diff"])
                if k <= 1:
                    c.aa_opi(ibox(rng, -2, W + 2, 1), op, s)
                elif k <= 3:
                    fb = rbox(rng, 0, W, 2)
                    fb = tuple(int(v) + rng.frac() for v in fb)
                    c.aa_opf(fb, op, rng.chance(2, 3), s)
                elif k <= 5:
                    c.aa_opaa(s, o, op)
                elif k == 6:
                    c.aa_translate(s, rng.rint(-5, 5), rng.rint(-5, 5))
                elif k == 7:
                    c.aa_translateto(s, o, rng.rint(-5, 5), rng.rint(-5, 5))
                else:
                    c.aa_copy(s, o)
            elif r < 46:
                c.e("%s qc %s" % (s, fs(*ibox(rng, -1, W + 1, 1))))
            elif r < 72:  # rc ops
                k = rng.rint(0, 11)
                op = rng.pick(["intersect", "diff"])
                aa = rng.pick(["aa", "bw"])
                m = rng.pick(MATS)
                fb = rbox(rng, 0, W, 3)
                mt = mat(rng, m, W / 2, W / 2)
                if k == 0:
                    c.e("rc new irect %s" % fs(*ibox(rng, 0, W, 1)))
                elif k == 1:
                    c.e("rc new region %s" % spec(rng.pick(["union", "xor"]), [ibox(rng, 0, W, 1) for _ in range(rng.rint(1, 4))]))
                elif k == 2:
                    for ln in shape(rng, rng.pick(SHAPES), *rbox(rng, 0, W, 4)):
                        c.e(ln)
                    c.e("rc new path %s 0 0 %d %d" % (aa, W + 2, W + 2))
                elif k == 3:
                    c.e("rc opi %s %s" % (fs(*ibox(rng, -1, W + 1, 1)), op))
                elif k == 4:
                    c.e("rc opregion %s %s" % (spec(rng.pick(["union", "xor"]), [ibox(rng, 0, W, 1) for _ in range(rng.rint(1, 3))]), op))
                elif k <= 6:
                    c.e("rc oprect %s %s %s %s" % (fs(*fb), mt, op, aa))
                elif k == 7:
                    c.e("rc oprrect %s %s %s %s %s %s" % (fs(*fb), f((fb[2] - fb[0]) / 3), f((fb[3] - fb[1]) / 4), mt, op, aa))
                elif k <= 9:
                    for ln in shape(rng, rng.pick(SHAPES), *fb):
                        c.e(ln)
                    c.e("rc oppath %s %s %s" % (mt, op, aa))
                elif k == 10:
                    c.e("rc translate %d %d" % (rng.rint(-4, 4), rng.rint(-4, 4)))
                else:
                    c.e(rng.pick(["rc setempty", "rc setrect %s" % fs(*ibox(rng, 0, W, 1)), "rc new empty"]))
            elif r < 86:  # blits
                if not c.can_blit():
                    fb = tuple(int(v) + rng.frac() for v in rbox(rng, 0, W, 4))
                    if fb[2] - fb[0] < 1 or fb[3] - fb[1] < 1:
                        fb = (1.3, 1.3, 6.6, 6.6)
                    c.aa_path_frac(fb, True, (0, 0, W + 3, W + 3), "aa")
                c.blit_rand(rng)
            else:  # scans
                cmd = rng.pick(SCAN_CMDS)
                for ln in scan_geoms(rng, cmd)[-2:]:
                    c.e(ln)


def main():
    section1()
    section2()
    section3()
    section4()
    section5()
    section6()
    names = set()
    for c in cases:
        assert c.name not in names, c.name
        names.add(c.name)
    return "".join(
        ["# Case script for oracle/aaclip/aaclip.cpp (format documented there).\n"
         "# Generated by oracle/aaclip/gen_cases.py; do not edit by hand.\n\n"]
        + [c.text() + "\n" for c in cases])


def check(dump_path, cases_path):
    """Replays cases.txt against the dump and verifies each '#chk' bounds comment."""
    dump = open(dump_path).read().split("\n")
    if dump and dump[-1] == "":
        dump.pop()
    i = 0
    actual = {}
    bad = 0
    case = None
    for ln in open(cases_path).read().split("\n"):
        ln = ln.strip()
        if not ln:
            continue
        if ln.startswith("#chk"):
            want = ln.split()[1:]
            a = actual.get("aa")
            if want == ["empty"]:
                ok = a is None or a[0] == 1
            else:
                ok = a is not None and a[0] == 0 and [str(v) for v in a[1:]] == want
            if not ok:
                bad += 1
                print("MISMATCH %s: want %s got %s" % (case, want, a))
            continue
        if ln.startswith("#"):
            continue
        tok = ln.split()
        cmd = tok[0]
        if cmd == "case":
            case = tok[1]
            assert dump[i] == "== " + case, (dump[i], case)
            i += 1
            actual = {}
        elif cmd in ("aa", "aa2"):
            if tok[1] == "qc":
                assert dump[i].startswith("qc"), dump[i]
                i += 1
                continue
            assert dump[i].startswith("ret"), (case, ln, dump[i])
            i += 1
            p = dump[i].split()
            assert p[0] == "clip"
            i += 1
            tgt = tok[2] if tok[1] == "translateto" else cmd
            actual[tgt] = (int(p[1]),) + tuple(int(v) for v in p[3:7])
            if tok[1] == "copy":
                pass
            while i < len(dump) and (dump[i].startswith("row ") or dump[i] == "mask skipped"):
                i += 1
        elif cmd == "rc":
            assert dump[i].startswith("ret"), (case, ln, dump[i])
            i += 1
            assert dump[i].startswith("rc "), dump[i]
            i += 1
            while i < len(dump) and (dump[i].startswith("row ") or dump[i] == "mask skipped"):
                i += 1
        elif cmd in ("blit", "scan"):
            while i < len(dump) and (dump[i].startswith("blit") or dump[i].startswith(" row") or dump[i].startswith("skip")):
                i += 1
    assert i == len(dump), (i, len(dump))
    print("check ok, %d bounds mismatches" % bad)
    return bad


if __name__ == "__main__":
    here = os.path.dirname(os.path.abspath(__file__))
    default = os.path.join(here, "../../crates/skia-rust-raster/src/aa_clip_tests/cases.txt")
    if len(sys.argv) > 2 and sys.argv[1] == "--check":
        sys.exit(1 if check(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else default) else 0)
    out = sys.argv[1] if len(sys.argv) > 1 else default
    text = main()
    with open(out, "w", newline="\n") as fh:
        fh.write(text)
    print("%d cases written to %s" % (len(cases), out))
