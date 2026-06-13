#!/usr/bin/env python3
"""Finalize the DVB-T2 AWGN campaign (e4849f07).

For each of the six MODCODs:
  * merge the main-sweep CSV with its extension CSV (if any), sort by Es/N0;
  * write the committed flat curve_<slug>.csv;
  * concatenate the tracing logs into tracing_<slug>.jsonl;
  * interpolate the FER=1e-4 crossing (log-linear) and the gap to the ETSI
    TS 102 831 Table 44 anchor;
  * invoke plot.py to render curve_<slug>.png.

Prints a per-curve summary table (anchor, crossing, gap, deepest frames/errors).
"""
import csv
import math
import os
import subprocess
import sys
import tomllib

DIR = os.path.dirname(os.path.abspath(__file__))
REF = os.path.normpath(os.path.join(
    DIR, "..", "..", "..", "crates", "gf2-coding", "data",
    "dvb_t2_tr102831_reference.toml"))
PLOT = os.path.join(DIR, "plot.py")

# slug -> (toml_key, anchor_cn_db)
CONFIGS = [
    ("1_2_16qam", "normal_r1_2_16qam", 6.0),
    ("2_3_16qam", "normal_r2_3_16qam", 8.9),
    ("3_4_16qam", "normal_r3_4_16qam", 10.0),
    ("1_2_64qam", "normal_r1_2_64qam", 9.9),
    ("2_3_64qam", "normal_r2_3_64qam", 13.5),
    ("3_4_64qam", "normal_r3_4_64qam", 15.1),
]
FIELDS = ["es_n0_db", "fer", "ber", "frames", "errors", "mean_iters", "wall_seconds"]


def read_rows(path):
    if not os.path.exists(path):
        return []
    with open(path, newline="") as f:
        return list(csv.DictReader(f))


def merge(slug):
    main = read_rows(os.path.join(DIR, f"curve_{slug}", f"curve_{slug}.csv"))
    ext = read_rows(os.path.join(DIR, f"curve_{slug}_ext", f"curve_{slug}.csv"))
    by_snr = {}
    for r in main + ext:  # ext overrides any duplicate Es/N0
        by_snr[round(float(r["es_n0_db"]), 4)] = r
    rows = [by_snr[k] for k in sorted(by_snr)]
    out = os.path.join(DIR, f"curve_{slug}.csv")
    with open(out, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=FIELDS)
        w.writeheader()
        for r in rows:
            w.writerow({k: r.get(k, "") for k in FIELDS})
    # tracing concat
    tout = os.path.join(DIR, f"tracing_{slug}.jsonl")
    with open(tout, "w") as fo:
        for sub in (f"curve_{slug}", f"curve_{slug}_ext"):
            tp = os.path.join(DIR, sub, "tracing.jsonl")
            if os.path.exists(tp):
                with open(tp) as fi:
                    fo.write(fi.read())
    return rows, out


def crossing(rows, target=1e-4):
    """Log-linear interpolate Es/N0 where FER == target, using the straddling pair."""
    pts = [(float(r["es_n0_db"]), float(r["fer"])) for r in rows if float(r["fer"]) > 0]
    pts.sort()
    for (x0, f0), (x1, f1) in zip(pts, pts[1:]):
        if (f0 - target) * (f1 - target) <= 0 and f0 != f1:
            l0, l1, lt = math.log10(f0), math.log10(f1), math.log10(target)
            return x0 + (lt - l0) * (x1 - x0) / (l1 - l0)
    return None


def main():
    print(f"{'MODCOD':<10} {'anchor':>7} {'cross@1e-4':>11} {'gap(dB)':>8} "
          f"{'deepest':>8} {'frames':>9} {'errs':>6} {'fer':>10}")
    for slug, key, anchor in CONFIGS:
        rows, out = merge(slug)
        if not rows:
            print(f"{slug:<10}  (no data)")
            continue
        xc = crossing(rows)
        deep = max(rows, key=lambda r: float(r["es_n0_db"]))
        gap = (xc - anchor) if xc is not None else float("nan")
        gtxt = f"{gap:.3f}" if xc is not None else "n/a"
        xtxt = f"{xc:.3f}" if xc is not None else "n/a"
        print(f"{slug:<10} {anchor:>7.1f} {xtxt:>11} {gtxt:>8} "
              f"{float(deep['es_n0_db']):>8.2f} {int(deep['frames']):>9} "
              f"{int(deep['errors']):>6} {float(deep['fer']):>10.3e}")
        png = os.path.join(DIR, f"curve_{slug}.png")
        subprocess.run([sys.executable, PLOT, "--curve-csv", out,
                        "--reference-toml", REF, "--output", png,
                        "--modcod-key", key], check=True,
                       stdout=subprocess.DEVNULL)


if __name__ == "__main__":
    main()
