#!/usr/bin/env python3
"""Analyse the eb9b324c pilot against the committed session-5 arms.

Pooling is the harness's own: sum(elapsed_ns) / sum(calls).
"""
import csv
import collections

REPO = "/home/vkaskivuo/Projects/gf2"
S5 = f"{REPO}/dev/benchmarks/tuning_profiles"
LED = f"{REPO}/dev/active/50b47eae/s5-session"
PILOT = f"{REPO}/dev/active/eb9b324c/pilot"
PILOT_CSV = f"{PILOT}/pilot-cand-fixed.csv"
PILOT_LED = f"{PILOT}/ledger-cand-fixed.tsv"

CELLS = [
    "polynomial/mul_fast/len=32",
    "polynomial/mul_fast/len=64",
    "polynomial/mul/len=64",
    "polynomial/mul/len=256",
    "polynomial/mul/len=16",
    "polynomial/mul/len=32",
    "polynomial/mul/len=33",
    "polynomial/mul_fast/len=65",
    "polynomial/mul_fast/len=128",
    "polynomial/mul_fast/len=512",
]
WINDOW = [2112 + 32 * i for i in range(16)]


def rows_by_execution(path):
    out = collections.defaultdict(dict)
    for r in csv.DictReader(open(path)):
        out[int(r["execution"])][r["cell"]] = (int(r["elapsed_ns"]), int(r["calls"]))
    return out


def ledger(path, joff, poffoff):
    """j -> page offset."""
    m = {}
    for i, line in enumerate(open(path)):
        if i == 0:
            continue
        f = line.rstrip("\n").split("\t")
        m[int(f[joff])] = int(f[poffoff])
    return m


def stats(rows, led, cell, offsets=None):
    """Return (bit5_0, bit5_1, pooled, n) over members whose page offset is selected."""
    acc = {0: [0, 0], 1: [0, 0]}
    pool = [0, 0]
    n = 0
    for j, poff in led.items():
        if offsets is not None and poff not in offsets:
            continue
        b5 = 1 if poff % 64 >= 32 else 0
        e, c = rows[j + 1][cell]
        acc[b5][0] += e
        acc[b5][1] += c
        pool[0] += e
        pool[1] += c
        n += 1
    f = lambda a: a[0] / a[1] if a[1] else float("nan")
    return f(acc[0]), f(acc[1]), f(pool), n


cand5 = rows_by_execution(f"{S5}/2026-08-22-ensemble-candidate-arm-5.csv")
ref5 = rows_by_execution(f"{S5}/2026-08-22-ensemble-reference-arm-5.csv")
pilot = rows_by_execution(PILOT_CSV)
lc5 = ledger(f"{LED}/ledger-cand.tsv", 0, 6)
lr5 = ledger(f"{LED}/ledger-ref.tsv", 0, 6)
lp = ledger(PILOT_LED, 0, 6)

win = set(WINDOW)
assert sorted(lp.values()) == WINDOW, sorted(lp.values())

print("### A. Matched-page-offset window (16 offsets 2112..2592, step 32; both bit-5 phases, 8 each)")
print()
hdr = ("cell", "ref5 b5=0", "ref5 b5=1", "ref5 pool", "cand5 pool", "pilot b5=0",
       "pilot b5=1", "pilot pool", "cand5/ref5", "pilot/ref5")
print("\t".join(hdr))
for c in CELLS:
    r0, r1, rp, _ = stats(ref5, lr5, c, win)
    c0, c1, cp, _ = stats(cand5, lc5, c, win)
    p0, p1, pp, npil = stats(pilot, lp, c, win)
    print(f"{c}\t{r0:.1f}\t{r1:.1f}\t{rp:.1f}\t{cp:.1f}\t{p0:.1f}\t{p1:.1f}\t{pp:.1f}"
          f"\t{cp / rp:.6f}\t{pp / rp:.6f}")

print()
print("### B. Fetch-block phase ratio (bit5=0 over bit5=1), same window")
print()
print("cell\tref5 b0/b1\tcand5 b0/b1\tpilot b0/b1")
for c in CELLS:
    r0, r1, _, _ = stats(ref5, lr5, c, win)
    c0, c1, _, _ = stats(cand5, lc5, c, win)
    p0, p1, _, _ = stats(pilot, lp, c, win)
    print(f"{c}\t{r0 / r1:.4f}\t{c0 / c1:.4f}\t{p0 / p1:.4f}")

print()
print("### C. Full committed arms (all 128 members) for reference; pilot repeated from A")
print()
print("cell\tref5 pool (128)\tcand5 pool (128)\tcand5/ref5\tpilot pool (16)\tpilot/ref5(128)")
for c in CELLS:
    _, _, rp, _ = stats(ref5, lr5, c)
    _, _, cp, _ = stats(cand5, lc5, c)
    _, _, pp, _ = stats(pilot, lp, c, win)
    print(f"{c}\t{rp:.1f}\t{cp:.1f}\t{cp / rp:.6f}\t{pp:.1f}\t{pp / rp:.6f}")

print()
print("### D. Wrapper isolation: (mul_fast/len=L) - (mul/len=L), ns/call, matched window")
print()
print("len\tarm\tb5=0\tb5=1\tphase mean")
for L in (32, 64):
    for name, rows, led in (("ref5", ref5, lr5), ("cand5", cand5, lc5), ("pilot", pilot, lp)):
        f0, f1, _, _ = stats(rows, led, f"polynomial/mul_fast/len={L}", win)
        m0, m1, _, _ = stats(rows, led, f"polynomial/mul/len={L}", win)
        print(f"{L}\t{name}\t{f0 - m0:.2f}\t{f1 - m1:.2f}\t{((f0 - m0) + (f1 - m1)) / 2:.2f}")

print()
print("### E. Pilot per-member ns/call at the two verdict cells")
print()
print("E\tpage_offset\tbit5\tmul_fast/len=32\tmul_fast/len=64")
for j in sorted(lp):
    poff = lp[j]
    a = pilot[j + 1]["polynomial/mul_fast/len=32"]
    b = pilot[j + 1]["polynomial/mul_fast/len=64"]
    print(f"{j}\t{poff}\t{1 if poff % 64 >= 32 else 0}\t{a[0] / a[1]:.1f}\t{b[0] / b[1]:.1f}")

print()
print("### F. All thirty-four pinned cells, matched window — pilot-scale diagnostic, not a verdict")
print()
import math
all_cells = [l.split("\t")[0] for l in open(f"{LED}/list-cells-cand.txt").read().splitlines()[1:]]
print("cell\tref5 window\tcand5 window\tpilot\tcand5/ref5\tpilot/ref5\tpilot b0/b1")
lg_c = lg_p = 0.0
for c in all_cells:
    r0, r1, rp, _ = stats(ref5, lr5, c, win)
    _, _, cp, _ = stats(cand5, lc5, c, win)
    p0, p1, pp, _ = stats(pilot, lp, c, win)
    lg_c += math.log(cp / rp)
    lg_p += math.log(pp / rp)
    print(f"{c}\t{rp:.6g}\t{cp:.6g}\t{pp:.6g}\t{cp / rp:.6f}\t{pp / rp:.6f}\t{p0 / p1:.4f}")
n = len(all_cells)
print(f"GEOMETRIC MEAN ({n} cells)\t\t\t\t{math.exp(lg_c / n):.6f}\t{math.exp(lg_p / n):.6f}\t")
