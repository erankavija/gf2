#!/usr/bin/env python3
"""Render the jit:88ca7d2f pre-cutover BCH baseline receipt.

Reads only files `run.sh` produced at run time (the host-facts text file and
the per-benchmark Criterion `estimates.json`/`sample.json`/`benchmark.json`
triples under `samples/`) and prints the receipt markdown to stdout. The
prose sections below (inclusion list, seed statement) describe the fixed
source files this receipt baselines and are not measured figures, so they
are not derived from run output.

Usage: render_receipt.py <out_dir> <issue> <date_utc>
"""
import hashlib
import json
import sys
from pathlib import Path


def sha256_of(path: Path) -> str:
    h = hashlib.sha256()
    h.update(path.read_bytes())
    return h.hexdigest()


def parse_host_facts(host_file: Path) -> dict:
    facts = {
        "generated": "",
        "revision": "",
        "tree_state": "",
        "uptime": "",
        "uname": "",
        "cpu_model": "",
        "nproc": "",
        "governor": "",
        "rustc": "",
        "cargo": "",
        "measurement_start_utc": "",
        "measurement_end_utc": "",
    }
    section = None
    section_lines: dict[str, list[str]] = {}
    for raw_line in host_file.read_text().splitlines():
        line = raw_line.rstrip("\n")
        if line.startswith("# generated: "):
            facts["generated"] = line[len("# generated: ") :]
        elif line.startswith("# gf2 revision: "):
            facts["revision"] = line[len("# gf2 revision: ") :]
        elif line.startswith("# tree state at run start: "):
            facts["tree_state"] = line[len("# tree state at run start: ") :]
        elif line.startswith("## "):
            section = line[3:]
            section_lines[section] = []
        elif section is not None and line.strip():
            section_lines[section].append(line.strip())

    facts["uptime"] = " ".join(section_lines.get("uptime at run start", []))
    facts["uname"] = " ".join(section_lines.get("uname", []))
    facts["cpu_model"] = " ".join(section_lines.get("CPU model (/proc/cpuinfo)", []))
    facts["nproc"] = " ".join(section_lines.get("nproc", []))
    facts["governor"] = " ".join(section_lines.get("CPU frequency governor (cpu0)", []))
    toolchain_lines = section_lines.get("toolchain", [])
    facts["rustc"] = next((l for l in toolchain_lines if l.startswith("rustc ")), "")
    facts["cargo"] = next((l for l in toolchain_lines if l.startswith("cargo ")), "")
    for line in section_lines.get("measurement window", []):
        if line.startswith("start_utc: "):
            facts["measurement_start_utc"] = line[len("start_utc: ") :]
        elif line.startswith("end_utc: "):
            facts["measurement_end_utc"] = line[len("end_utc: ") :]
    return facts


def parse_log_header(log_file: Path) -> dict:
    fields = {
        "command": "",
        "started_utc": "",
        "load_avg_start": "",
        "finished_utc": "",
        "load_avg_end": "",
    }
    prefixes = {
        "# command: ": "command",
        "# started_utc: ": "started_utc",
        "# load_avg_start: ": "load_avg_start",
        "# finished_utc: ": "finished_utc",
        "# load_avg_end: ": "load_avg_end",
    }
    for line in log_file.read_text().splitlines():
        for prefix, key in prefixes.items():
            if line.startswith(prefix):
                fields[key] = line[len(prefix) :]
    return fields


def load_benchmark_row(sample_dir: Path) -> dict:
    benchmark = json.loads((sample_dir / "benchmark.json").read_text())
    estimates = json.loads((sample_dir / "estimates.json").read_text())
    sample = json.loads((sample_dir / "sample.json").read_text())
    median = estimates["median"]
    ci = median["confidence_interval"]
    return {
        "full_id": benchmark["full_id"],
        "group_id": benchmark["group_id"],
        "function_id": benchmark.get("function_id") or "",
        "value_str": benchmark.get("value_str") or "",
        "sample_count": len(sample["times"]),
        "sampling_mode": sample.get("sampling_mode", ""),
        "median_ns": median["point_estimate"],
        "ci_level": ci["confidence_level"],
        "ci_lower_ns": ci["lower_bound"],
        "ci_upper_ns": ci["upper_bound"],
    }


def main() -> None:
    out_dir = Path(sys.argv[1])
    issue = sys.argv[2]
    date_utc = sys.argv[3]

    host_file = out_dir / f"{date_utc}-{issue}-host.txt"
    facts = parse_host_facts(host_file)

    bench_targets = ["bch_parallel", "batch_operations"]
    log_files = {
        bench: out_dir / f"{date_utc}-{issue}-{bench}-criterion.txt"
        for bench in bench_targets
    }
    log_headers = {bench: parse_log_header(path) for bench, path in log_files.items()}

    samples_dir = out_dir / "samples"
    rows = [
        load_benchmark_row(d) for d in sorted(samples_dir.iterdir()) if d.is_dir()
    ]
    rows.sort(key=lambda r: r["full_id"])

    print(f"# Receipt: pre-cutover BCH throughput baseline (jit:{issue})")
    print()
    print(
        "Rendered by `render_receipt.py` from the run directory; every measured "
        "figure below is read from the committed Criterion `estimates.json` and "
        "`sample.json` files at render time."
    )
    print()
    print("| Field | Value |")
    print("|---|---|")
    print(f"| Issue | `{issue}` |")
    print(f"| gf2 revision | `{facts['revision']}` |")
    print("| Tree state | " + facts["tree_state"] + " |")
    print(f"| Host | {facts['cpu_model']} |")
    print(f"| Cores | full host (`nproc` = {facts['nproc']}); serialized via the CCX1 lock wrapper's `--full-host` mode, no `taskset` pin |")
    print(f"| Governor | {facts['governor']} |")
    print(f"| Kernel | {facts['uname']} |")
    print(f"| Rust | {facts['rustc']} |")
    print(f"| Cargo | {facts['cargo']} |")
    print(
        "| Measurement window (UTC) | "
        f"{facts['measurement_start_utc']} to {facts['measurement_end_utc']} |"
    )
    print(f"| Load average at run start | {facts['uptime']} |")
    print()

    print("## Inclusion list")
    print()
    print(
        "Every Criterion bench target in `crates/gf2-coding/benches/` whose "
        "measured code path is the legacy "
        "`gf2_coding::bch::{BchCode,BchEncoder,BchDecoder}` surface, found by "
        "reading `bch_parallel.rs` and `batch_operations.rs` in full and "
        "grepping the remaining bench files for `bch`/`Bch`:"
    )
    print()
    print("| File | Included groups | Reason |")
    print("|---|---|---|")
    print(
        "| [`bch_parallel.rs`](../../../crates/gf2-coding/benches/bch_parallel.rs) "
        "| `bch_batch_decode`, `bch_single_vs_batch` | Both groups decode DVB-T2 "
        "short-frame codewords through `BchDecoder::decode`/`decode_batch`, the "
        "legacy decode path. |"
    )
    print(
        "| [`batch_operations.rs`](../../../crates/gf2-coding/benches/batch_operations.rs) "
        "| `bch_batch`, `bch_sequential_vs_batch` | Both groups encode through "
        "`BchEncoder::encode`/`encode_batch` on legacy `BchCode` constructions "
        "(one DVB-T2-sized, one `BchCode::new(15, 11, 1, ...)`). The file's other "
        "two groups (`ldpc_batch_with_backend`, `ldpc_sequential_vs_batch`) "
        "measure LDPC, not BCH, and are excluded from this receipt by the "
        "runner's `bch_` filter. |"
    )
    print(
        "| `bch_genmatrix.rs` | none | Its `bch_genmatrix_w2` and `bch_paritycheck` "
        "groups measure `BinaryBchCode`/`BchSpec`, the canonical construction "
        "model, not the legacy `BchCode`/`BchEncoder`/`BchDecoder` path this "
        "receipt baselines. |"
    )
    print(
        "| `allocation_optimization.rs`, `bench_support.rs`, "
        "`cpu_dispatch_probe.rs`, `ldpc_decode.rs`, `ldpc_throughput.rs`, "
        "`linear_codes.rs`, `llr_simd.rs`, `modem_cpu.rs`, "
        "`modem_generic_vs_fast.rs`, `profile_ldpc_decode.rs`, "
        "`profile_ldpc_encode.rs`, `quick_parallel.rs`, "
        "`simulation_no_analysis_overhead.rs`, `sparse_preprocessing.rs` | none | "
        "`grep -lE \"bch|Bch\"` over `crates/gf2-coding/benches/*.rs` found no "
        "match in any of these files. |"
    )
    print()

    print("## Seed statement")
    print()
    print(
        "None of the included groups draw from a random-number generator; each "
        "constructs its inputs deterministically from loop indices in the bench "
        "source itself, so there is no seed constant to record:"
    )
    print()
    print(
        "- `bch_batch_decode`, `bch_single_vs_batch` "
        "(`bch_parallel.rs`): each message is an all-zero `BitVec` with bits "
        "`0..8` set from the low 8 bits of the batch index `i`; encoded once "
        "through `BchEncoder::encode` to produce the codewords the group "
        "decodes."
    )
    print(
        "- `bch_batch` (`batch_operations.rs`): the message is a single "
        "all-zero `BitVec::zeros(k)`, cloned `batch_size` times."
    )
    print(
        "- `bch_sequential_vs_batch` (`batch_operations.rs`): each message bit "
        "`j` of row `i` is set to `(i + j) % 2 == 0`."
    )
    print()

    print("## Exact invocations")
    print()
    print(
        "Load average is `uptime`'s three-figure line, captured immediately "
        "before and after each bench target ran; it is an observed fact about "
        "host contention during this run, not a gating threshold."
    )
    print()
    print("| Bench target | Invocation | Started (UTC) | Load avg at start | Finished (UTC) | Load avg at end |")
    print("|---|---|---|---|---|---|")
    for bench in bench_targets:
        h = log_headers[bench]
        print(
            f"| `{bench}` | `{h['command']}` | {h['started_utc']} | "
            f"{h['load_avg_start']} | {h['finished_utc']} | {h['load_avg_end']} |"
        )
    print()

    print("## Measured benchmarks")
    print()
    print(
        "One row per Criterion benchmark ID, exactly as Criterion names it "
        "(`<group>/<function-or-parameter>`). Median and the 95% CI bounds are "
        "`estimates.json`'s `median` estimate (wall-clock nanoseconds per "
        "iteration); Samples is the length of `sample.json`'s `times` array."
    )
    print()
    print("| Benchmark ID | Group | Function/Parameter | Samples | Sampling mode | Median (ns/iter) | 95% CI lower (ns) | 95% CI upper (ns) |")
    print("|---|---|---|---|---|---|---|---|")
    for row in rows:
        func_or_param = row["function_id"] or row["value_str"]
        print(
            f"| `{row['full_id']}` | `{row['group_id']}` | `{func_or_param}` | "
            f"{row['sample_count']} | {row['sampling_mode']} | "
            f"{row['median_ns']:.2f} | {row['ci_lower_ns']:.2f} | "
            f"{row['ci_upper_ns']:.2f} |"
        )
    print()

    print("## Files")
    print()
    print("| File | SHA-256 | Bytes |")
    print("|---|---|---|")
    top_level_files = [host_file] + [log_files[b] for b in bench_targets]
    for path in sorted(top_level_files):
        digest = sha256_of(path)
        print(f"| `{path.name}` | `{digest}` | {path.stat().st_size} |")
    for sample_dir in sorted(samples_dir.iterdir()):
        if not sample_dir.is_dir():
            continue
        for fname in ("benchmark.json", "estimates.json", "sample.json"):
            path = sample_dir / fname
            digest = sha256_of(path)
            print(
                f"| `samples/{sample_dir.name}/{fname}` | `{digest}` | "
                f"{path.stat().st_size} |"
            )
    print()

    print("## Reproduction")
    print()
    print("```")
    print(f"dev/bench_results/{issue}/run.sh")
    print("```")


if __name__ == "__main__":
    main()
