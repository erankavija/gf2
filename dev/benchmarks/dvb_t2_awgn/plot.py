#!/usr/bin/env python3
"""Plot a single DVB-T2 BICM AWGN campaign curve against ETSI TS 102 831 reference points.

Reads:
  - ``<output_dir>/curve_<rate>_<mod>.csv``  (produced by dvb_t2_awgn_campaign)
  - ``<reference_toml>``                      (crates/gf2-coding/data/dvb_t2_tr102831_reference.toml)

Produces:
  - ``<output_dir>/curve_<rate>_<mod>.png``

Usage (from repo root)::

    python3 dev/benchmarks/dvb_t2_awgn/plot.py \\
        --curve-csv /tmp/dvb_smoke/curve_1_2_16qam.csv \\
        --reference-toml crates/gf2-coding/data/dvb_t2_tr102831_reference.toml \\
        --output /tmp/dvb_smoke/curve_1_2_16qam.png

The script is non-interactive (no plt.show() call) and safe to run in headless
CI environments.  Only matplotlib and tomllib (stdlib >= 3.11) or tomli are
required.
"""

import argparse
import csv
import os
import sys

try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib
    except ImportError:
        print(
            "Error: 'tomllib' (Python >= 3.11) or 'tomli' package required.\n"
            "Install tomli: pip install tomli",
            file=sys.stderr,
        )
        sys.exit(1)

try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
except ImportError:
    print(
        "Error: matplotlib is required.  Install with: pip install matplotlib",
        file=sys.stderr,
    )
    sys.exit(1)


def parse_args():
    p = argparse.ArgumentParser(
        description="Plot DVB-T2 BICM AWGN simulated FER vs ETSI TS 102 831 reference.",
    )
    p.add_argument(
        "--curve-csv",
        required=True,
        help="Path to the campaign CSV produced by dvb_t2_awgn_campaign "
             "(e.g. /tmp/run/curve_1_2_16qam.csv).",
    )
    p.add_argument(
        "--reference-toml",
        required=True,
        help="Path to the reference TOML "
             "(crates/gf2-coding/data/dvb_t2_tr102831_reference.toml).",
    )
    p.add_argument(
        "--output",
        required=True,
        help="Destination PNG file path.",
    )
    p.add_argument(
        "--modcod-key",
        default=None,
        help="Override TOML section key (e.g. 'normal_r1_2_16qam').  "
             "Inferred from the CSV filename when omitted.",
    )
    return p.parse_args()


def infer_modcod_key(csv_path):
    """Derive the TOML section key from a 'curve_<rate>_<mod>.csv' filename."""
    basename = os.path.basename(csv_path)
    # Strip prefix and suffix
    if basename.startswith("curve_") and basename.endswith(".csv"):
        inner = basename[len("curve_"):-len(".csv")]
        return f"normal_r{inner}"
    return None


def read_curve_csv(path):
    """Return (es_n0_list, fer_list, ber_list) from the campaign CSV."""
    es_n0 = []
    fer = []
    ber = []
    with open(path, newline="") as f:
        reader = csv.DictReader(f)
        for row in reader:
            try:
                es = float(row["es_n0_db"])
                f_er = float(row["fer"])
                b_er = float(row.get("ber", 0.0))
                if f_er > 0.0:
                    es_n0.append(es)
                    fer.append(f_er)
                    ber.append(b_er)
            except (KeyError, ValueError):
                continue
    return es_n0, fer, ber


def read_reference_points(toml_path, key):
    """Return (es_n0_list, fer_list) from the TOML section matching 'key'."""
    with open(toml_path, "rb") as f:
        data = tomllib.load(f)
    section = data.get(key)
    if section is None:
        return None, None
    points = section.get("points", [])
    if not points:
        return None, None
    es_n0 = [p[0] for p in points]
    fer_vals = [p[1] for p in points]
    return es_n0, fer_vals


def make_title(modcod_key):
    """Build a human-readable title from a modcod key like 'normal_r1_2_16qam'."""
    key = modcod_key.replace("normal_r", "")
    # key is like "1_2_16qam" or "3_4_64qam"
    parts = key.rsplit("_", 1)
    if len(parts) == 2:
        rate_part, mod_part = parts
        rate_display = rate_part.replace("_", "/")
        mod_display = mod_part.upper()
        return f"DVB-T2 Normal BICM  Rate {rate_display}  {mod_display}"
    return f"DVB-T2 {modcod_key}"


def main():
    args = parse_args()

    # Infer TOML key from filename if not provided.
    modcod_key = args.modcod_key
    if modcod_key is None:
        modcod_key = infer_modcod_key(args.curve_csv)
    if modcod_key is None:
        print(
            f"Error: could not infer modcod key from '{args.curve_csv}'.  "
            "Use --modcod-key to specify it explicitly.",
            file=sys.stderr,
        )
        sys.exit(1)

    # Read simulated curve.
    sim_esn0, sim_fer, _sim_ber = read_curve_csv(args.curve_csv)
    if not sim_esn0:
        print(
            f"Warning: no non-zero FER points found in '{args.curve_csv}'.  "
            "The plot will contain only reference points.",
            file=sys.stderr,
        )

    # Read ETSI reference points.
    ref_esn0, ref_fer = read_reference_points(args.reference_toml, modcod_key)

    # Build the plot.
    fig, ax = plt.subplots(figsize=(8, 5))
    ax.set_yscale("log")

    if sim_esn0:
        ax.plot(
            sim_esn0,
            sim_fer,
            "o-",
            color="royalblue",
            linewidth=1.5,
            markersize=5,
            label="Simulated FER",
        )

    if ref_esn0:
        ax.plot(
            ref_esn0,
            ref_fer,
            "s--",
            color="darkorange",
            linewidth=1.5,
            markersize=5,
            label="ETSI TS 102 831 reference",
        )

    ax.set_xlabel("Es/N0 (dB)", fontsize=12)
    ax.set_ylabel("FER", fontsize=12)
    ax.set_title(make_title(modcod_key), fontsize=13)
    ax.legend(fontsize=10)
    ax.grid(True, which="both", linestyle="--", linewidth=0.5, alpha=0.7)
    ax.set_ylim(bottom=1e-5, top=1.0)

    # Ensure output directory exists.
    out_dir = os.path.dirname(args.output)
    if out_dir:
        os.makedirs(out_dir, exist_ok=True)

    fig.tight_layout()
    fig.savefig(args.output, dpi=150)
    plt.close(fig)

    print(f"Wrote plot to {args.output}")


if __name__ == "__main__":
    main()
