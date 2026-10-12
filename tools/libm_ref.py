#!/usr/bin/env python3
"""Correctly-rounded reference check for the probe dump (clean-room tooling).

Usage: libm_ref.py <dumpdir> [fn ...]
Reads <fn>.bin records (a, b, result) written by `probe dump`, computes the correctly rounded
result with mpmath and reports how often the platform result differs, and by how many ulps.
"""
import struct, sys, os
import mpmath as mp

mp.mp.prec = 300

F32 = {"sinf", "cosf", "tanf", "asinf", "acosf", "atanf", "expf", "logf", "log2f", "cbrtf", "atan2f", "powf"}


def f32_of(bits):
    return struct.unpack("<f", struct.pack("<I", bits & 0xFFFFFFFF))[0]


def f64_of(bits):
    return struct.unpack("<d", struct.pack("<Q", bits))[0]


def round_f32(v):
    """Round mp value to nearest f32 (ties-to-even), return bits."""
    if v == 0:
        return 0
    if mp.isinf(v) or mp.isnan(v):
        raise ValueError
    s = 1 if v < 0 else 0
    a = abs(v)
    m, e = mp.frexp(a)  # a = m * 2**e, m in [0.5,1)
    e = int(e)
    # normal f32: 24 bit mantissa; value = M * 2**(e-24), M in [2^23, 2^24)
    q = e - 24
    if q < -149:
        q = -149
    M = mp.nint(mp.ldexp(a, -q))  # round half even
    M = int(M)
    # overflow handling
    val_exp = q
    while M >= (1 << 24):
        M >>= 1
        val_exp += 1
    # compose
    if M < (1 << 23):
        bits = M  # subnormal (val_exp == -149)
    else:
        be = val_exp + 150  # biased exponent: value = M*2^val_exp, M has 24 bits
        if be >= 255:
            return (s << 31) | 0x7F800000
        bits = (be << 23) | (M & 0x7FFFFF)
    return (s << 31) | bits


def round_f64(v):
    return struct.unpack("<Q", struct.pack("<d", float(v)))[0] if abs(v) < 1e300 and v != 0 and abs(v) > 1e-300 else generic_f64(v)


def generic_f64(v):
    if v == 0:
        return 0
    s = 1 if v < 0 else 0
    a = abs(v)
    m, e = mp.frexp(a)
    e = int(e)
    q = max(e - 53, -1074)
    M = int(mp.nint(mp.ldexp(a, -q)))
    while M >= (1 << 53):
        M >>= 1
        q += 1
    if M < (1 << 52):
        bits = M
    else:
        be = q + 1075
        if be >= 2047:
            return (s << 63) | 0x7FF0000000000000
        bits = (be << 52) | (M & ((1 << 52) - 1))
    return (s << 63) | bits


def ref(fn, a, b, is32):
    cv = f32_of if is32 else f64_of
    x = mp.mpf(cv(a))
    y = mp.mpf(cv(b))
    base = fn[:-1] if is32 else fn
    if base == "sin": return mp.sin(x)
    if base == "cos": return mp.cos(x)
    if base == "tan": return mp.tan(x)
    if base == "asin": return mp.asin(x)
    if base == "acos": return mp.acos(x)
    if base == "atan": return mp.atan(x)
    if base == "exp": return mp.exp(x)
    if base == "exp2": return mp.power(2, x)
    if base == "log": return mp.log(x)
    if base == "log2": return mp.log(x) / mp.log(2)
    if base == "cbrt": return mp.cbrt(x)
    if base == "atan2": return mp.atan2(x, y)
    if base == "pow": return mp.power(x, y)
    raise KeyError(fn)


def ord_bits(bits, is32):
    # monotone integer ordering of float bit patterns
    sign = (1 << 31) if is32 else (1 << 63)
    return -(bits & (sign - 1)) if bits & sign else bits


def main():
    d = sys.argv[1]
    names = sys.argv[2:] or sorted(f[:-4] for f in os.listdir(d) if f.endswith(".bin"))
    for fn in names:
        is32 = fn in F32
        data = open(os.path.join(d, fn + ".bin"), "rb").read()
        n = len(data) // 24
        bad = 0
        hist = {}
        samples = []
        offs = []
        for i in range(n):
            a, b, r = struct.unpack_from("<QQQ", data, i * 24)
            try:
                v = ref(fn, a, b, is32)
                if is32 and (mp.isinf(v) or mp.isnan(v)):
                    continue
                exp_bits = round_f32(v) if is32 else generic_f64(v)
            except Exception:
                continue
            if exp_bits != r:
                bad += 1
                dist = ord_bits(r, is32) - ord_bits(exp_bits, is32)
                hist[dist] = hist.get(dist, 0) + 1
                if len(samples) < 8:
                    samples.append((hex(a), hex(b), hex(r), hex(exp_bits)))
                # distance of the true value from the rounding boundary between the two results,
                # in units of the spacing of the result format (the platform's pre-rounding error)
                if abs(dist) == 1:
                    ur = f32_of(r) if is32 else f64_of(r)
                    ue = f32_of(exp_bits) if is32 else f64_of(exp_bits)
                    mid = (mp.mpf(ur) + mp.mpf(ue)) / 2
                    sp = abs(mp.mpf(ur) - mp.mpf(ue))
                    offs.append(float(abs(v - mid) / sp))
        print(f"{fn}: {bad}/{n} differ from correct rounding; ulp-distance hist {dict(sorted(hist.items()))}")
        if offs:
            offs.sort()
            print(f"    boundary distance (result ulps): median {offs[len(offs)//2]:.3e} p90 {offs[int(len(offs)*0.9)]:.3e} max {offs[-1]:.3e}")
        for s in samples:
            print("   ", s)


main()
