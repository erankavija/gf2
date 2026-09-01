# Receipt: external-baseline survey for BCH encoding and generator-matrix materialization

Rendered by `baseline-survey/make-receipt.py` from the run directory; every
figure below is read out of the committed CSVs at render time.

| Field | Value |
|---|---|
| Issue | `4e732b56` |
| Run timestamps (UTC) | 2026-09-01T04:13:04Z, 2026-09-01T04:23:21Z |
| gf2 revision | recorded per stage below |
| Host | AMD Ryzen 9 5900X 12-Core Processor |
| Cores pinned | CCX1 via `dev/scripts/ccx1-bench-flock.sh` (`taskset -c 6-11`; nice -n -5 requested, denied, and the child ran at inherited default priority) |
| Governor | powersave |
| Kernel | Linux fraktaali 7.1.11-arch1-1 #1 SMP PREEMPT_DYNAMIC Fri, 28 Aug 2026 03:36:07 +0000 x86_64 GNU/Linux |
| C compiler | gcc (GCC) 16.2.1 20260810 |
| C++ compiler | g++ (GCC) 16.2.1 20260810 |
| Rust | rustc 1.97.0 (2d8144b78 2026-07-07) |

## Baseline pins

| Pin | Value |
|---|---|
| aff3ct tag | `v4.7.0` |
| aff3ct commit | `e8a65c5047262d97a15563b9edc961f69b2792cc` |
| bchlib tag | `v2.1.3` |
| bchlib commit | `8d0656ab8f37e734428635501738d360ad80eebd` |
| m4ri version | `20260122` |
| m4ri tarball sha256 | `7e033ca1fd36be8861e2f67d9d124c398fc0d830209bb0226462485876346404` |
| itpp release | `4.3.1 (tarball sha256 50717621c5dfb5ed22f8492f8af32b17776e6e06641dfe3a3a8f82c8d353b877)` |
| itpp soname | `8.2.1` |

## Reference build configuration

* `aff3ct library:  -O3 -march=native -funroll-loops -O3 -DNDEBUG -std=gnu++11 -fPIC`
* `aff3ct defines:  -DAFF3CT_EXT_STRINGS -DAFF3CT_MULTI_PREC -DAFF3CT_POLAR_BIT_PACKING -DMIPP_ENABLE_BACKTRACE -DSPU_COLORS -DSPU_STACKTRACE`
* `m4ri library: CFLAGS='-O3 -march=native -fPIC'`

## Per-stage provenance

Each row identifies the committed output, the stage's invocation(s), the basis for
each invocation, the gf2 revision supplying that stage's data, and the host manifest
covering it. Basis is `recorded (log header)` when the tool's own log carries a
`# command:`/`# environment:` header; for a stage whose log predates header emission,
it is `reconstructed` (derived from the runner's argument construction, cited from the
committed invocation-derivation record) or `session-recorded` (executed directly by an
agent session, also cited from that record). For non-gf2 stages the revision is
inherited from the host manifest; a differing data revision is shown as
`data (host: manifest)` in the same row.

| Stage | Output | Provenance log | Exact invocation(s) | Basis | gf2 revision for this stage | Host manifest |
|---|---|---|---|---|---|---|
| `2026-09-01-4e732b56-small-aff3ct` | [2026-09-01-4e732b56-small-aff3ct.csv](2026-09-01-4e732b56-small-aff3ct.csv) | [2026-09-01-4e732b56-small-aff3ct.log](2026-09-01-4e732b56-small-aff3ct.log) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench all` | recorded (log header) | `38e0335a092bb1d7dc851ff91ee7d156d1c72996` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-small-bchlib` | [2026-09-01-4e732b56-small-bchlib.csv](2026-09-01-4e732b56-small-bchlib.csv) | [2026-09-01-4e732b56-small-bchlib.log](2026-09-01-4e732b56-small-bchlib.log) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/bchlib_bch_bench` | recorded (log header) | `38e0335a092bb1d7dc851ff91ee7d156d1c72996` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-small-gf2` | [2026-09-01-4e732b56-small-gf2.csv](2026-09-01-4e732b56-small-gf2.csv) | [2026-09-01-4e732b56-small-gf2.log](2026-09-01-4e732b56-small-gf2.log) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/target/survey-target/release/survey-gf2-side all` | recorded (log header) | `38e0335a092bb1d7dc851ff91ee7d156d1c72996` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-small-itpp` | [2026-09-01-4e732b56-small-itpp.csv](2026-09-01-4e732b56-small-itpp.csv) | [2026-09-01-4e732b56-small-itpp.log](2026-09-01-4e732b56-small-itpp.log) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/itpp_bch_bench` | recorded (log header) | `38e0335a092bb1d7dc851ff91ee7d156d1c72996` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-small-m4ri` | [2026-09-01-4e732b56-small-m4ri.csv](2026-09-01-4e732b56-small-m4ri.csv) | [2026-09-01-4e732b56-small-m4ri.log](2026-09-01-4e732b56-small-m4ri.log) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/m4ri_genmatrix_bench dev/bench_results/4e732b56/generators.txt all` | recorded (log header) | `38e0335a092bb1d7dc851ff91ee7d156d1c72996` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-t2n-aff3ct` | [2026-09-01-4e732b56-t2n-aff3ct.csv](2026-09-01-4e732b56-t2n-aff3ct.csv) | [2026-09-01-4e732b56-t2n-aff3ct.log](2026-09-01-4e732b56-t2n-aff3ct.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench all` | recorded (log header) | `38e0335a092bb1d7dc851ff91ee7d156d1c72996` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |
| `2026-09-01-4e732b56-t2n-bchlib` | [2026-09-01-4e732b56-t2n-bchlib.csv](2026-09-01-4e732b56-t2n-bchlib.csv) | [2026-09-01-4e732b56-t2n-bchlib.log](2026-09-01-4e732b56-t2n-bchlib.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/bchlib_bch_bench` | recorded (log header) | `38e0335a092bb1d7dc851ff91ee7d156d1c72996` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |
| `2026-09-01-4e732b56-t2n-gf2` | [2026-09-01-4e732b56-t2n-gf2.csv](2026-09-01-4e732b56-t2n-gf2.csv) | [2026-09-01-4e732b56-t2n-gf2.log](2026-09-01-4e732b56-t2n-gf2.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/target/survey-target/release/survey-gf2-side all` | recorded (log header) | `38e0335a092bb1d7dc851ff91ee7d156d1c72996` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |
| `2026-09-01-4e732b56-t2n-itpp` | [2026-09-01-4e732b56-t2n-itpp.csv](2026-09-01-4e732b56-t2n-itpp.csv) | [2026-09-01-4e732b56-t2n-itpp.log](2026-09-01-4e732b56-t2n-itpp.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/itpp_bch_bench` | recorded (log header) | `38e0335a092bb1d7dc851ff91ee7d156d1c72996` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |
| `2026-09-01-4e732b56-t2n-m4ri` | [2026-09-01-4e732b56-t2n-m4ri.csv](2026-09-01-4e732b56-t2n-m4ri.csv) | [2026-09-01-4e732b56-t2n-m4ri.log](2026-09-01-4e732b56-t2n-m4ri.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/m4ri_genmatrix_bench dev/bench_results/4e732b56/generators.txt all` | recorded (log header) | `38e0335a092bb1d7dc851ff91ee7d156d1c72996` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |
| `2026-09-01-4e732b56-small-aff3ct-perf` | (perf counters) | [2026-09-01-4e732b56-small-aff3ct-perf-stat.txt](2026-09-01-4e732b56-small-aff3ct-perf-stat.txt) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh perf stat -e task-clock\,cycles\,instructions\,branches\,branch-misses\,cache-references\,cache-misses /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench w1` | recorded (log header) | `38e0335a092bb1d7dc851ff91ee7d156d1c72996` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-t2n-aff3ct-perf` | (perf counters) | [2026-09-01-4e732b56-t2n-aff3ct-perf-stat.txt](2026-09-01-4e732b56-t2n-aff3ct-perf-stat.txt) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh perf stat -e task-clock\,cycles\,instructions\,branches\,branch-misses\,cache-references\,cache-misses /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench w1` | recorded (log header) | `38e0335a092bb1d7dc851ff91ee7d156d1c72996` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |

## Measured cells

Throughput is information bits per second for W1 and for the W2
`matmul-m4rm` substrate rows (scaled by batch times k so the multiply
family compares directly with the W1 encoders), and matrix bits per
second for the W2 materialization rows. `Trials` is the number of
independent trials the cell's wall budget allowed; a cell marked
*estimate* was projected from a measured per-unit cost and was never
run at that size.

| Workload | Row | Batch | Library | Algorithm | Trials | Median | Min | Max | Spread |
|---|---|---|---|---|---|---|---|---|---|
| W1 | B1 | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 111.49 | 101.00 | 111.78 | 9.7% |
| W1 | B1 | 1 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 4.64 | 4.34 | 4.67 | 7.1% |
| W1 | B1 | 1 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 4.56 | 4.53 | 4.58 | 1.2% |
| W1 | B1 | 1 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 3.89 | 3.50 | 3.91 | 10.4% |
| W1 | B1 | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 102.43 | 102.27 | 102.76 | 0.5% |
| W1 | B1 | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 421.22 | 400.49 | 431.78 | 7.4% |
| W1 | B1 | 16 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 7.02 | 6.88 | 7.13 | 3.5% |
| W1 | B1 | 16 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 6.84 | 6.81 | 6.90 | 1.3% |
| W1 | B1 | 16 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 5.57 | 5.53 | 5.57 | 0.7% |
| W1 | B1 | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 103.98 | 103.24 | 104.32 | 1.0% |
| W1 | B1 | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 426.63 | 422.97 | 432.46 | 2.2% |
| W1 | B1 | 256 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 5.92 | 5.89 | 6.00 | 1.9% |
| W1 | B1 | 256 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 5.99 | 5.96 | 6.00 | 0.7% |
| W1 | B1 | 256 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 6.29 | 6.24 | 6.30 | 1.0% |
| W1 | B1 | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 101.01 | 100.73 | 101.39 | 0.6% |
| W1 | B1 | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 413.44 | 408.88 | 416.84 | 1.9% |
| W1 | B1 | 4096 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 6.19 | 6.18 | 6.21 | 0.4% |
| W1 | B1 | 4096 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 6.21 | 6.18 | 6.23 | 0.7% |
| W1 | B1 | 4096 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 6.29 | 6.27 | 6.30 | 0.4% |
| W1 | B2 | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 93.27 | 92.93 | 94.53 | 1.7% |
| W1 | B2 | 1 | bchlib v2.1.3 | `table-remainder` | 7 | 2005.79 | 1914.13 | 2111.61 | 9.8% |
| W1 | B2 | 1 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 1.79 | 1.79 | 1.81 | 0.8% |
| W1 | B2 | 1 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 1.79 | 1.78 | 1.80 | 1.2% |
| W1 | B2 | 1 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.74 | 0.74 | 0.74 | 0.2% |
| W1 | B2 | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 95.34 | 94.99 | 95.42 | 0.4% |
| W1 | B2 | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 206.10 | 184.18 | 206.97 | 11.1% |
| W1 | B2 | 16 | bchlib v2.1.3 | `table-remainder` | 7 | 2230.97 | 2016.53 | 2240.37 | 10.0% |
| W1 | B2 | 16 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 1.99 | 1.98 | 1.99 | 0.5% |
| W1 | B2 | 16 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 1.98 | 1.98 | 1.99 | 0.5% |
| W1 | B2 | 16 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.67 | 0.67 | 0.68 | 0.5% |
| W1 | B2 | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 83.86 | 82.74 | 84.06 | 1.6% |
| W1 | B2 | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 203.14 | 194.81 | 204.26 | 4.7% |
| W1 | B2 | 256 | bchlib v2.1.3 | `table-remainder` | 7 | 2257.44 | 2243.69 | 2265.62 | 1.0% |
| W1 | B2 | 256 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 1.91 | 1.90 | 1.92 | 0.9% |
| W1 | B2 | 256 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 1.92 | 1.84 | 1.92 | 4.0% |
| W1 | B2 | 256 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.66 | 0.66 | 0.67 | 1.0% |
| W1 | B2 | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 80.20 | 79.79 | 80.38 | 0.7% |
| W1 | B2 | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 184.03 | 177.09 | 188.03 | 5.9% |
| W1 | B2 | 4096 | bchlib v2.1.3 | `table-remainder` | 7 | 2269.18 | 2246.35 | 2273.85 | 1.2% |
| W1 | B2 | 4096 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 1.91 | 1.88 | 1.93 | 2.2% |
| W1 | B2 | 4096 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 1.93 | 1.92 | 1.93 | 0.6% |
| W1 | B2 | 4096 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.66 | 0.65 | 0.66 | 0.8% |
| W1 | B3 | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 111.89 | 111.43 | 112.41 | 0.9% |
| W1 | B3 | 1 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 3.31 | 3.29 | 3.32 | 0.7% |
| W1 | B3 | 1 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 3.30 | 3.29 | 3.32 | 0.8% |
| W1 | B3 | 1 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.38 | 0.38 | 0.39 | 0.4% |
| W1 | B3 | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 98.73 | 98.41 | 99.27 | 0.9% |
| W1 | B3 | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 435.52 | 433.88 | 437.45 | 0.8% |
| W1 | B3 | 16 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 3.57 | 3.53 | 3.58 | 1.2% |
| W1 | B3 | 16 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 3.54 | 3.53 | 3.55 | 0.6% |
| W1 | B3 | 16 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.40 | 0.39 | 0.40 | 0.9% |
| W1 | B3 | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 97.24 | 96.86 | 97.33 | 0.5% |
| W1 | B3 | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 429.65 | 427.78 | 430.61 | 0.7% |
| W1 | B3 | 256 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 3.47 | 3.45 | 3.49 | 0.9% |
| W1 | B3 | 256 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 3.48 | 3.47 | 3.49 | 0.5% |
| W1 | B3 | 256 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.39 | 0.39 | 0.39 | 0.6% |
| W1 | B3 | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 97.93 | 96.15 | 98.44 | 2.3% |
| W1 | B3 | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 430.38 | 429.24 | 431.86 | 0.6% |
| W1 | B3 | 4096 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 3.46 | 3.45 | 3.48 | 0.9% |
| W1 | B3 | 4096 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 3.47 | 3.46 | 3.48 | 0.6% |
| W1 | B3 | 4096 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.39 | 0.38 | 0.39 | 1.2% |
| W1 | T2N | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 39.00 | 38.74 | 39.22 | 1.2% |
| W1 | T2N | 1 | bchlib v2.1.3 | `table-remainder` | 7 | 4653.66 | 4519.40 | 4661.75 | 3.1% |
| W1 | T2N | 1 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 0.68 | 0.67 | 0.68 | 1.0% |
| W1 | T2N | 1 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 0.68 | 0.67 | 0.68 | 0.7% |
| W1 | T2N | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.65 | 38.45 | 38.87 | 1.1% |
| W1 | T2N | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 78.72 | 77.66 | 78.96 | 1.6% |
| W1 | T2N | 16 | bchlib v2.1.3 | `table-remainder` | 7 | 4675.59 | 4640.03 | 4686.60 | 1.0% |
| W1 | T2N | 16 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 0.68 | 0.68 | 0.68 | 0.2% |
| W1 | T2N | 16 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 0.68 | 0.68 | 0.68 | 0.4% |
| W1 | T2N | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.46 | 38.39 | 38.70 | 0.8% |
| W1 | T2N | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 77.86 | 77.30 | 78.07 | 1.0% |
| W1 | T2N | 256 | bchlib v2.1.3 | `table-remainder` | 7 | 4677.70 | 4645.85 | 4693.63 | 1.0% |
| W1 | T2N | 256 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 0.68 | 0.68 | 0.68 | 0.4% |
| W1 | T2N | 256 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 0.68 | 0.68 | 0.68 | 0.2% |
| W1 | T2N | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.61 | 38.59 | 38.64 | 0.1% |
| W1 | T2N | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 77.84 | 77.75 | 77.98 | 0.3% |
| W1 | T2N | 4096 | bchlib v2.1.3 | `table-remainder` | 7 | 4683.90 | 4679.02 | 4688.86 | 0.2% |
| W1 | T2N | 4096 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch-projected` | *estimate* | 0.65 | — | — | — |
| W1 | T2S | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 44.04 | 43.68 | 44.06 | 0.9% |
| W1 | T2S | 1 | bchlib v2.1.3 | `table-remainder` | 7 | 4598.06 | 4541.23 | 4614.51 | 1.6% |
| W1 | T2S | 1 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 0.78 | 0.78 | 0.79 | 0.8% |
| W1 | T2S | 1 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 0.79 | 0.78 | 0.79 | 1.8% |
| W1 | T2S | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 42.88 | 42.77 | 43.26 | 1.2% |
| W1 | T2S | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 69.05 | 68.81 | 69.20 | 0.6% |
| W1 | T2S | 16 | bchlib v2.1.3 | `table-remainder` | 7 | 4552.85 | 4541.80 | 4599.31 | 1.3% |
| W1 | T2S | 16 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 0.77 | 0.76 | 0.78 | 2.6% |
| W1 | T2S | 16 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 0.76 | 0.76 | 0.77 | 0.6% |
| W1 | T2S | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 42.59 | 42.55 | 42.78 | 0.6% |
| W1 | T2S | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 46.94 | 46.66 | 47.04 | 0.8% |
| W1 | T2S | 256 | bchlib v2.1.3 | `table-remainder` | 7 | 4587.41 | 4565.85 | 4590.47 | 0.5% |
| W1 | T2S | 256 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 7 | 0.77 | 0.77 | 0.77 | 0.2% |
| W1 | T2S | 256 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 7 | 0.77 | 0.77 | 0.77 | 0.4% |
| W1 | T2S | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 42.37 | 42.30 | 42.48 | 0.4% |
| W1 | T2S | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 68.12 | 67.98 | 68.19 | 0.3% |
| W1 | T2S | 4096 | bchlib v2.1.3 | `table-remainder` | 7 | 4581.15 | 4569.83 | 4594.38 | 0.5% |
| W1 | T2S | 4096 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-batch` | 3 | 0.77 | 0.77 | 0.77 | 0.1% |
| W1 | T2S | 4096 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `encode-loop` | 3 | 0.77 | 0.77 | 0.77 | 0.2% |
| W2 | B1 | 5 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 297.72 | 296.60 | 298.96 | 0.8% |
| W2 | B1 | 5 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `generator-matrix` | 7 | 6.40 | 6.35 | 6.41 | 0.9% |
| W2 | B1 | 5 | m4ri 20260122 | `echelonize` | 7 | 279.99 | 270.63 | 283.77 | 4.7% |
| W2 | B1 | 5 | m4ri 20260122 | `genmatrix-rref` | 7 | 247.12 | 204.68 | 255.75 | 20.7% |
| W2 | B1 | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 117.41 | 116.80 | 117.59 | 0.7% |
| W2 | B1 | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 159.15 | 158.01 | 160.68 | 1.7% |
| W2 | B2 | 64 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 168.05 | 165.24 | 168.48 | 1.9% |
| W2 | B2 | 64 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `generator-matrix` | 7 | 7.02 | 6.81 | 7.08 | 3.7% |
| W2 | B2 | 64 | m4ri 20260122 | `echelonize` | 7 | 1441.38 | 1427.62 | 1450.11 | 1.6% |
| W2 | B2 | 64 | m4ri 20260122 | `genmatrix-rref` | 7 | 1067.28 | 1060.11 | 1075.01 | 1.4% |
| W2 | B2 | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 2213.42 | 2201.04 | 2221.68 | 0.9% |
| W2 | B2 | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 2733.66 | 2730.47 | 2739.80 | 0.3% |
| W2 | B3 | 223 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 118.43 | 115.88 | 121.36 | 4.6% |
| W2 | B3 | 223 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `generator-matrix` | 7 | 7.37 | 7.35 | 7.52 | 2.3% |
| W2 | B3 | 223 | m4ri 20260122 | `echelonize` | 7 | 1590.02 | 1585.34 | 1601.18 | 1.0% |
| W2 | B3 | 223 | m4ri 20260122 | `genmatrix-rref` | 7 | 1464.47 | 1453.37 | 1476.70 | 1.6% |
| W2 | B3 | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 2477.23 | 2465.61 | 2491.04 | 1.0% |
| W2 | B3 | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 3013.48 | 3003.21 | 3050.10 | 1.6% |
| W2 | T2N | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 83.60 | 83.41 | 83.85 | 0.5% |
| W2 | T2N | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 92.49 | 92.32 | 92.59 | 0.3% |
| W2 | T2N | 32208 | aff3ct v4.7.0 | `basis-encode-pack` | 4 | 41.37 | 41.33 | 41.39 | 0.1% |
| W2 | T2N | 32208 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `generator-matrix-projected` | *estimate* | 0.65 | — | — | — |
| W2 | T2N | 32208 | m4ri 20260122 | `echelonize` | 7 | 173.58 | 166.40 | 175.23 | 5.1% |
| W2 | T2N | 32208 | m4ri 20260122 | `genmatrix-rref` | 7 | 391.99 | 383.41 | 395.55 | 3.1% |
| W2 | T2S | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 339.48 | 337.55 | 341.10 | 1.0% |
| W2 | T2S | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 457.20 | 454.51 | 457.99 | 0.8% |
| W2 | T2S | 7032 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 47.64 | 47.52 | 47.82 | 0.6% |
| W2 | T2S | 7032 | gf2 38e0335a092bb1d7dc851ff91ee7d156d1c72996 | `generator-matrix` | 3 | 1.60 | 1.60 | 1.60 | 0.1% |
| W2 | T2S | 7032 | m4ri 20260122 | `echelonize` | 7 | 807.27 | 766.08 | 808.89 | 5.3% |
| W2 | T2S | 7032 | m4ri 20260122 | `genmatrix-rref` | 7 | 1344.98 | 1266.02 | 1351.45 | 6.4% |

## Files

| File | SHA-256 | Bytes |
|---|---|---|
| `2026-08-31-4e732b56-generator-agreement.txt` | `ccd3474e8296a67e…` | 170 |
| `2026-08-31-4e732b56-kodo-retrieval.txt` | `cecfeef3993e7730…` | 6679 |
| `2026-08-31-4e732b56-m4ri-thread-evidence.txt` | `f6ecb9472812db92…` | 3258 |
| `2026-09-01-4e732b56-determinism-agreement.txt` | `306e7e625ada9977…` | 11701 |
| `2026-09-01-4e732b56-generator-matrix-agreement.txt` | `bb5ce6c27b5c44c0…` | 2072 |
| `2026-09-01-4e732b56-small-aff3ct-perf-stat.txt` | `7e5cd4e65ba7bca4…` | 3589 |
| `2026-09-01-4e732b56-small-aff3ct.csv` | `78997130a40ef02b…` | 18990 |
| `2026-09-01-4e732b56-small-aff3ct.log` | `e6f98d1e11984447…` | 3028 |
| `2026-09-01-4e732b56-small-bchlib.csv` | `62e962aca5491a42…` | 5046 |
| `2026-09-01-4e732b56-small-bchlib.log` | `466a6bb1e607c8bc…` | 1213 |
| `2026-09-01-4e732b56-small-generator-agreement.txt` | `ccd3474e8296a67e…` | 170 |
| `2026-09-01-4e732b56-small-gf2.csv` | `114480a340214225…` | 27312 |
| `2026-09-01-4e732b56-small-gf2.log` | `9b36595cea87b1e0…` | 3251 |
| `2026-09-01-4e732b56-small-host.txt` | `4b7f34cce0731512…` | 6947 |
| `2026-09-01-4e732b56-small-itpp.csv` | `b7a0fca5f3692e3d…` | 8175 |
| `2026-09-01-4e732b56-small-itpp.log` | `8a02846e9622618c…` | 1345 |
| `2026-09-01-4e732b56-small-m4ri.csv` | `e489581831e20c8e…` | 9403 |
| `2026-09-01-4e732b56-small-m4ri.log` | `f495baa983deb075…` | 1467 |
| `2026-09-01-4e732b56-t2n-aff3ct-perf-stat.txt` | `73c6f7a976b649e1…` | 2078 |
| `2026-09-01-4e732b56-t2n-aff3ct.csv` | `0c2f937df8c69be5…` | 4962 |
| `2026-09-01-4e732b56-t2n-aff3ct.log` | `0556a6b7dcd68b91…` | 1147 |
| `2026-09-01-4e732b56-t2n-bchlib.csv` | `b1cd28210a86d6a5…` | 2708 |
| `2026-09-01-4e732b56-t2n-bchlib.log` | `adf8679b1e1472d2…` | 752 |
| `2026-09-01-4e732b56-t2n-generator-agreement.txt` | `ccd3474e8296a67e…` | 170 |
| `2026-09-01-4e732b56-t2n-gf2.csv` | `c84e385bfcb94111…` | 5468 |
| `2026-09-01-4e732b56-t2n-gf2.log` | `aed2248704efb366…` | 1217 |
| `2026-09-01-4e732b56-t2n-host.txt` | `73e8f203ceac9f17…` | 6938 |
| `2026-09-01-4e732b56-t2n-itpp.csv` | `eebab57153ce9b42…` | 90 |
| `2026-09-01-4e732b56-t2n-itpp.log` | `35e605e3c1dd47ad…` | 555 |
| `2026-09-01-4e732b56-t2n-m4ri.csv` | `9a23de2050cc37f1…` | 2683 |
| `2026-09-01-4e732b56-t2n-m4ri.log` | `665d243751078c63…` | 769 |
| `generators.txt` | `612d5c84ee02ff69…` | 551 |

## Reproduction

```
dev/active/4e732b56/baseline-survey/fetch-build.sh
dev/active/4e732b56/baseline-survey/run-survey.sh dev/bench_results/4e732b56 <codes> <prefix>
dev/active/4e732b56/baseline-survey/make-receipt.py dev/bench_results/4e732b56
```
