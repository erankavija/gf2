#!/usr/bin/env python3
"""Tests for `dev/active/fd9d5416/render_receipt.py` on a synthetic run.

The run directory is built in a temporary directory at test time and holds
test data only: the Criterion IDs come from the committed smoke-run ID lists
(`dev/active/d1b4f85e/smoke-run.md`, `dev/active/591a1c5e/smoke-run.md`), and
the sample times are synthetic multiples of the committed baseline and survey
medians, chosen to drive each verdict. Nothing generated here is committed.
The baseline and survey inputs are the committed ones.

Usage: python3 dev/active/fd9d5416/tests/test_render_receipt.py
"""
from __future__ import annotations

import hashlib
import importlib.util
import json
import random
import re
import statistics
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
spec = importlib.util.spec_from_file_location("render_receipt", HERE.parent / "render_receipt.py")
rr = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rr)

# `GF2_BENCH=1` cells the d1b4f85e smoke run names in its Outcome section but
# leaves out of its timing table.
BENCH_MODE_IDS = [
    f"bch_genmatrix_w2/{p}/{c}/T2N-mother" for p in ("materialize", "reference") for c in ("fresh-alloc", "warm-reuse")
] + [
    "bch_genmatrix_w2/materialize/fresh-alloc/T2N",
    "bch_genmatrix_w2/materialize/warm-reuse/T2N",
    "bch_paritycheck/materialize/warm-reuse/T2N-mother",
    "bch_paritycheck/reference/warm-reuse/T2N-mother",
]
SHAPES = {  # (n, k) of the binary rows, workload-selection.md section 2 and the mother codes
    "B1": (15, 5), "B2": (127, 64), "B3": (255, 223), "T2S": (7200, 7032), "T2N": (32400, 32208),
    "T2S-mother": (16383, 16215), "T2N-mother": (65535, 65343),
    "N1": (8, 4), "N2": (26, 20), "N3": (80, 70), "N4": (624, 600),  # synthetic test shapes
}


def digest(*key) -> str:
    return hashlib.sha256(repr(key).encode()).hexdigest()[:16]


def smoke_ids(path: Path) -> list[str]:
    ids, inside = [], False
    for line in path.read_text().splitlines():
        if line.startswith("## "):
            inside = line.startswith("## Criterion IDs")
        elif inside and line.startswith("| `"):
            ids.append(line.split("`")[1])
    return ids


def all_ids() -> list[str]:
    return (smoke_ids(REPO / "dev/active/d1b4f85e/smoke-run.md") + BENCH_MODE_IDS
            + [i for i in smoke_ids(REPO / "dev/active/591a1c5e/smoke-run.md") if i.startswith(("bch_batch_decode", "bch_single_vs_batch"))])


def record_for(cid: str) -> dict | None:
    parts = cid.split("/")
    if parts[0] == "bch_encode_w1":
        path, workers, cache, row, batch = parts[1], int(parts[2][1:]), parts[3], parts[4], int(parts[5][2:])
        m = re.match(r"(family|selected|route)=([^\[]+)(?:\[(.+)\])?", path)
        n, k = SHAPES[row]
        return {"id": cid, "workload": "W1", "row": row, "n": n, "k": k, "batch": batch, "workers": workers,
                "cache": cache, "entry": "test", "selection": m.group(1),
                "family": "poly-remainder-scalar" if m.group(1) == "route" else m.group(2),
                "kernel": m.group(3), "rayon_pool_width": 6,
                "output_fnv1a": digest(row, batch)}
    if parts[0] in ("bch_genmatrix_w2", "bch_paritycheck"):
        row = parts[3]
        n, k = SHAPES[row]
        rows, cols = (k, n) if parts[0] == "bch_genmatrix_w2" else (n - k, n)
        return {"id": cid, "workload": "W2" if parts[0] == "bch_genmatrix_w2" else "W2-parity-check", "row": row,
                "rows": rows, "cols": cols, "cache": parts[2], "path": parts[1], "entry": "test", "matrix": "test",
                "workers": 1, "rayon_pool_width": 6,
                "output_fnv1a": digest(parts[0], row)}
    if parts[0] == "bch_encode_pns_16383_16215":
        return {"id": cid, "workload": "baseline-comparable", "n": 16383, "k": 16215, "batch": int(parts[1]),
                "workers": 1, "entry": "test", "family": "poly-remainder-scalar"}
    if parts[0] == "bch_sequential_vs_batch":
        return {"id": cid, "workload": "baseline-comparable", "n": 15, "k": 11, "batch": 100,
                "workers": 1, "entry": "test", "family": "poly-remainder-scalar"}
    return None


def write_cell(samples: Path, cid: str, centre_ns: float, count: int = 30) -> None:
    rng = random.Random(cid)
    times = [centre_ns * (1 + rng.uniform(-0.005, 0.005)) for _ in range(count)]
    d = samples / cid.replace("/", "__")
    d.mkdir(parents=True)
    group, _, rest = cid.partition("/")
    (d / "benchmark.json").write_text(json.dumps({"group_id": group, "function_id": None, "value_str": rest, "full_id": cid}))
    (d / "sample.json").write_text(json.dumps({"sampling_mode": "Flat", "iters": [1.0] * count, "times": times}))
    med = statistics.median(times)
    (d / "estimates.json").write_text(json.dumps({"median": {"point_estimate": med, "confidence_interval": {
        "confidence_level": 0.95, "lower_bound": med * 0.999, "upper_bound": med * 1.001}}}))


class Fixture:
    """A synthetic run directory where every comparison is `speedup`× faster."""

    def __init__(self, root: Path, speedup: float = 2.0, override: dict | None = None):
        self.dir = root / "run"
        samples = self.dir / "samples"
        (self.dir / "logs").mkdir(parents=True)
        base = rr.load_criterion(REPO / "dev/bench_results/88ca7d2f/samples")
        survey, _ = rr.load_survey(REPO / "dev/bench_results/4e732b56")
        records = []
        for cid in all_ids():
            rec = record_for(cid)
            if rec is not None:
                records.append(rec)
            centre = 1000.0
            if cid in base:
                centre = base[cid]["median_ns"] / speedup
            elif rec and rec.get("workload") == "W1" and rec["workers"] == 1 and rec["cache"] == "fresh-alloc":
                legacy = survey.get(("gf2", "W1", "encode-batch", rec["row"], rec["batch"]))
                if legacy:
                    centre = statistics.median(legacy["times"]) / speedup
            if override and cid in override:
                centre = override[cid](base, survey, cid)
            write_cell(samples, cid, centre)
        self.records = records
        self.write_records()
        (self.dir / "host.txt").write_text(
            "# command: test\n# issue: d1b4f85e\n# generated: TEST\n# gf2 revision: TESTREV\n"
            "# tree state at run start: clean (git status --porcelain was empty)\n\n"
            "## uptime at run start\ntest\n\n## uname\nTest OS\n\n## CPU model (/proc/cpuinfo)\nTest CPU\n\n"
            "## CPU flags relevant to the encode kernel bundle\navx2 pclmulqdq\n\n## nproc\n24\n\n"
            "## CPU frequency governor (cpu0)\ntest\n\n## toolchain\nrustc test\ncargo test\n\n"
            "## measurement window\nstart_utc: TEST\nend_utc: TEST\n")
        for label in rr.BENCH_LABELS:
            (self.dir / "logs" / f"{label}-criterion.txt").write_text(
                f"# command: test {label}\n# started_utc: TEST\n# load_avg_start: TEST\n"
                "# finished_utc: TEST\n# load_avg_end: TEST\n")

    def write_records(self) -> None:
        (self.dir / "dispatch.jsonl").write_text("".join(json.dumps(r) + "\n" for r in self.records))

    def render(self) -> tuple[str, bool]:
        lines, ok = rr.render(self.dir, REPO / "dev/bench_results/88ca7d2f", REPO / "dev/bench_results/4e732b56")
        return "\n".join(lines), ok


class RenderReceiptTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.resamples = rr.BOOTSTRAP_RESAMPLES

    def tearDown(self):
        rr.BOOTSTRAP_RESAMPLES = self.resamples
        self.tmp.cleanup()

    def test_smoke_id_lists_cover_the_run(self):
        ids = all_ids()
        self.assertEqual(len(ids), len(set(ids)))
        self.assertEqual(sum(i.startswith("bch_encode_w1/") for i in ids), 256)
        self.assertEqual(sum(i.startswith(("bch_genmatrix_w2/", "bch_paritycheck/")) for i in ids), 58)
        for cid in rr.TIER_A_KEPT:
            self.assertIn(cid, ids)

    def test_a_faster_run_passes_every_check(self):
        text, ok = Fixture(self.root).render()
        self.assertTrue(ok, text[-400:])
        section = text.split("## REQ-01 non-regression against the pinned")[1].split("Baseline cells outside")[0]
        self.assertEqual(section.count("| pass |"), len(rr.TIER_A_KEPT))
        tier_b = text.split("survey's pre-cutover gf2 cells")[1].split("## Determinism")[0]
        self.assertEqual(tier_b.count("| pass |"), 19)
        self.assertEqual(tier_b.count("excluded (no measured baseline)"), 2)  # T2N W1 B=4096, T2N W2
        self.assertEqual(tier_b.count("reported; excluded"), 4)
        determinism = text.split("## Determinism")[1].split("## Scalar-fallback")[0]
        self.assertNotIn("DISAGREE", determinism)
        scalar = text.split("## Scalar-fallback coverage")[1].split("## REQ-02")[0]
        self.assertEqual(scalar.count("| covered |"), 40)
        self.assertIn("`avx2-pclmul`", scalar)
        external = text.split("## REQ-02")[1].split("## Every measured cell")[0]
        self.assertEqual(sum(l.startswith("| W1 ") for l in external.splitlines()), 20)
        self.assertEqual(sum(l.startswith("| W2 ") for l in external.splitlines()), 5)
        self.assertIn("Aspirational outcome: target met on", external)
        every = text.split("## Every measured cell")[1].split("## Pinned inputs")[0]
        for cid in all_ids():
            self.assertIn(f"`{cid}`", every)
        self.assertIn("not resolvable at revision `TESTREV`", text)
        self.assertIn("**pass**", text.split("## Verdict")[1])

    def test_a_regressed_baseline_cell_fails(self):
        rr.BOOTSTRAP_RESAMPLES = 1000
        slower = {"bch_batch_decode/10": lambda base, survey, cid: base[cid]["median_ns"] * 1.05}
        text, ok = Fixture(self.root, override=slower).render()
        self.assertFalse(ok)
        row = next(l for l in text.splitlines() if l.startswith("| `bch_batch_decode/10` |"))
        self.assertIn("**FAIL**", row)

    def test_a_regressed_dvb_t2_cell_fails(self):
        rr.BOOTSTRAP_RESAMPLES = 1000
        cid = "bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/T2S/B=16"
        slower = {cid: lambda base, survey, c: statistics.median(survey[("gf2", "W1", "encode-batch", "T2S", 16)]["times"]) * 1.1}
        text, ok = Fixture(self.root, override=slower).render()
        self.assertFalse(ok)
        self.assertIn("**FAIL**", next(l for l in text.splitlines() if l.startswith(f"| `{cid}` |")))

    def test_a_split_digest_fails_determinism(self):
        rr.BOOTSTRAP_RESAMPLES = 1000
        fx = Fixture(self.root)
        target = next(r for r in fx.records if r.get("workload") == "W1" and r["workers"] == 6 and r["row"] == "B2")
        target["output_fnv1a"] = "0" * 16
        fx.write_records()
        text, ok = fx.render()
        self.assertFalse(ok)
        self.assertIn("**DISAGREE**", text)

    def test_a_missing_scalar_arm_is_a_gap(self):
        rr.BOOTSTRAP_RESAMPLES = 1000
        fx = Fixture(self.root)
        cid = "bch_encode_w1/family=clmul-fold[scalar]/W1/warm-reuse/B3/B=256"
        for name in ("benchmark.json", "estimates.json", "sample.json"):
            (fx.dir / "samples" / cid.replace("/", "__") / name).unlink()
        (fx.dir / "samples" / cid.replace("/", "__")).rmdir()
        text, ok = fx.render()
        self.assertFalse(ok)
        self.assertIn("**GAP**", text)
        self.assertIn(f"Dispatch-record IDs without Criterion output: `{cid}`", text)


if __name__ == "__main__":
    unittest.main()
