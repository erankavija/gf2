# Receipt: external-baseline survey for BCH encoding and generator-matrix materialization

Rendered by `baseline-survey/make-receipt.py` from the run directory; every
figure below is read out of the committed CSVs at render time.

| Field | Value |
|---|---|
| Issue | `4e732b56` |
| Run timestamps (UTC) | 2026-09-01T02:41:47Z, 2026-09-01T02:53:03Z |
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
| `2026-09-01-4e732b56-small-aff3ct` | [2026-09-01-4e732b56-small-aff3ct.csv](2026-09-01-4e732b56-small-aff3ct.csv) | [2026-09-01-4e732b56-small-aff3ct.log](2026-09-01-4e732b56-small-aff3ct.log) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench all` | recorded (log header) | `801d89852847afc789e9ddf26c1f1ebd3efb2a8b` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-small-bchlib` | [2026-09-01-4e732b56-small-bchlib.csv](2026-09-01-4e732b56-small-bchlib.csv) | [2026-09-01-4e732b56-small-bchlib.log](2026-09-01-4e732b56-small-bchlib.log) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/bchlib_bch_bench` | recorded (log header) | `801d89852847afc789e9ddf26c1f1ebd3efb2a8b` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-small-gf2` | [2026-09-01-4e732b56-small-gf2.csv](2026-09-01-4e732b56-small-gf2.csv) | [2026-09-01-4e732b56-small-gf2.log](2026-09-01-4e732b56-small-gf2.log) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/target/survey-target/release/survey-gf2-side all` | recorded (log header) | `801d89852847afc789e9ddf26c1f1ebd3efb2a8b` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-small-itpp` | [2026-09-01-4e732b56-small-itpp.csv](2026-09-01-4e732b56-small-itpp.csv) | [2026-09-01-4e732b56-small-itpp.log](2026-09-01-4e732b56-small-itpp.log) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/itpp_bch_bench` | recorded (log header) | `801d89852847afc789e9ddf26c1f1ebd3efb2a8b` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-small-m4ri` | [2026-09-01-4e732b56-small-m4ri.csv](2026-09-01-4e732b56-small-m4ri.csv) | [2026-09-01-4e732b56-small-m4ri.log](2026-09-01-4e732b56-small-m4ri.log) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/m4ri_genmatrix_bench dev/bench_results/4e732b56/generators.txt all` | recorded (log header) | `801d89852847afc789e9ddf26c1f1ebd3efb2a8b` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-t2n-aff3ct` | [2026-09-01-4e732b56-t2n-aff3ct.csv](2026-09-01-4e732b56-t2n-aff3ct.csv) | [2026-09-01-4e732b56-t2n-aff3ct.log](2026-09-01-4e732b56-t2n-aff3ct.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench all` | recorded (log header) | `801d89852847afc789e9ddf26c1f1ebd3efb2a8b` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |
| `2026-09-01-4e732b56-t2n-bchlib` | [2026-09-01-4e732b56-t2n-bchlib.csv](2026-09-01-4e732b56-t2n-bchlib.csv) | [2026-09-01-4e732b56-t2n-bchlib.log](2026-09-01-4e732b56-t2n-bchlib.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/bchlib_bch_bench` | recorded (log header) | `801d89852847afc789e9ddf26c1f1ebd3efb2a8b` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |
| `2026-09-01-4e732b56-t2n-gf2` | [2026-09-01-4e732b56-t2n-gf2.csv](2026-09-01-4e732b56-t2n-gf2.csv) | [2026-09-01-4e732b56-t2n-gf2.log](2026-09-01-4e732b56-t2n-gf2.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/target/survey-target/release/survey-gf2-side all` | recorded (log header) | `801d89852847afc789e9ddf26c1f1ebd3efb2a8b` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |
| `2026-09-01-4e732b56-t2n-itpp` | [2026-09-01-4e732b56-t2n-itpp.csv](2026-09-01-4e732b56-t2n-itpp.csv) | [2026-09-01-4e732b56-t2n-itpp.log](2026-09-01-4e732b56-t2n-itpp.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/itpp_bch_bench` | recorded (log header) | `801d89852847afc789e9ddf26c1f1ebd3efb2a8b` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |
| `2026-09-01-4e732b56-t2n-m4ri` | [2026-09-01-4e732b56-t2n-m4ri.csv](2026-09-01-4e732b56-t2n-m4ri.csv) | [2026-09-01-4e732b56-t2n-m4ri.log](2026-09-01-4e732b56-t2n-m4ri.log) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh timeout --foreground 1800 /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/m4ri_genmatrix_bench dev/bench_results/4e732b56/generators.txt all` | recorded (log header) | `801d89852847afc789e9ddf26c1f1ebd3efb2a8b` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |
| `2026-09-01-4e732b56-small-aff3ct-perf` | (perf counters) | [2026-09-01-4e732b56-small-aff3ct-perf-stat.txt](2026-09-01-4e732b56-small-aff3ct-perf-stat.txt) | `GF2_SURVEY_CODES=B1\,B2\,B3\,T2S /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh perf stat -e task-clock\,cycles\,instructions\,branches\,branch-misses\,cache-references\,cache-misses /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench w1` | recorded (log header) | `801d89852847afc789e9ddf26c1f1ebd3efb2a8b` | [2026-09-01-4e732b56-small-host.txt](2026-09-01-4e732b56-small-host.txt) |
| `2026-09-01-4e732b56-t2n-aff3ct-perf` | (perf counters) | [2026-09-01-4e732b56-t2n-aff3ct-perf-stat.txt](2026-09-01-4e732b56-t2n-aff3ct-perf-stat.txt) | `GF2_SURVEY_CODES=T2N /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/scripts/ccx1-bench-flock.sh perf stat -e task-clock\,cycles\,instructions\,branches\,branch-misses\,cache-references\,cache-misses /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-4e732b56/dev/active/4e732b56/baseline-survey/aff3ct_bch_bench w1` | recorded (log header) | `801d89852847afc789e9ddf26c1f1ebd3efb2a8b` | [2026-09-01-4e732b56-t2n-host.txt](2026-09-01-4e732b56-t2n-host.txt) |

## Measured cells

Throughput is information bits per second for W1 and matrix bits per second
for W2. `Trials` is the number of independent trials the cell's wall budget
allowed; a cell marked *estimate* was projected from a measured per-unit cost
and was never run at that size.

| Workload | Row | Batch | Library | Algorithm | Trials | Median | Min | Max | Spread |
|---|---|---|---|---|---|---|---|---|---|
| W1 | B1 | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 84.93 | 73.42 | 89.99 | 19.5% |
| W1 | B1 | 1 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 4.55 | 4.06 | 4.66 | 13.0% |
| W1 | B1 | 1 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 4.53 | 4.51 | 4.54 | 0.6% |
| W1 | B1 | 1 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 3.83 | 3.65 | 3.90 | 6.6% |
| W1 | B1 | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 101.23 | 83.25 | 101.40 | 17.9% |
| W1 | B1 | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 374.33 | 358.10 | 385.65 | 7.4% |
| W1 | B1 | 16 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 7.04 | 6.80 | 7.06 | 3.7% |
| W1 | B1 | 16 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 6.82 | 6.80 | 6.87 | 1.0% |
| W1 | B1 | 16 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 5.46 | 5.45 | 5.46 | 0.3% |
| W1 | B1 | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 100.91 | 100.76 | 101.23 | 0.5% |
| W1 | B1 | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 366.03 | 358.33 | 370.28 | 3.3% |
| W1 | B1 | 256 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 5.91 | 5.89 | 5.92 | 0.5% |
| W1 | B1 | 256 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 5.91 | 5.90 | 5.95 | 0.7% |
| W1 | B1 | 256 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 6.12 | 6.11 | 6.12 | 0.3% |
| W1 | B1 | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 99.70 | 99.10 | 99.93 | 0.8% |
| W1 | B1 | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 364.21 | 361.71 | 365.59 | 1.1% |
| W1 | B1 | 4096 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 6.17 | 6.16 | 6.22 | 1.0% |
| W1 | B1 | 4096 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 6.08 | 5.91 | 6.10 | 3.0% |
| W1 | B1 | 4096 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 6.08 | 6.07 | 6.09 | 0.4% |
| W1 | B2 | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 92.75 | 92.15 | 93.02 | 0.9% |
| W1 | B2 | 1 | bchlib v2.1.3 | `table-remainder` | 7 | 2001.39 | 1943.43 | 2128.18 | 9.2% |
| W1 | B2 | 1 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 1.76 | 1.73 | 1.77 | 2.1% |
| W1 | B2 | 1 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 1.77 | 1.76 | 1.77 | 1.0% |
| W1 | B2 | 1 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.73 | 0.72 | 0.73 | 1.0% |
| W1 | B2 | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 90.89 | 90.77 | 91.18 | 0.4% |
| W1 | B2 | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 170.96 | 170.54 | 199.19 | 16.8% |
| W1 | B2 | 16 | bchlib v2.1.3 | `table-remainder` | 7 | 2177.28 | 2025.19 | 2218.41 | 8.9% |
| W1 | B2 | 16 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 1.97 | 1.84 | 1.98 | 7.2% |
| W1 | B2 | 16 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 1.98 | 1.97 | 1.98 | 0.1% |
| W1 | B2 | 16 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.67 | 0.66 | 0.67 | 0.7% |
| W1 | B2 | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 85.53 | 84.56 | 86.00 | 1.7% |
| W1 | B2 | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 195.71 | 177.96 | 197.29 | 9.9% |
| W1 | B2 | 256 | bchlib v2.1.3 | `table-remainder` | 7 | 2210.77 | 2204.97 | 2220.03 | 0.7% |
| W1 | B2 | 256 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 1.88 | 1.77 | 1.90 | 7.3% |
| W1 | B2 | 256 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 1.89 | 1.65 | 1.89 | 12.4% |
| W1 | B2 | 256 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.66 | 0.65 | 0.66 | 0.8% |
| W1 | B2 | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 80.85 | 80.50 | 81.32 | 1.0% |
| W1 | B2 | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 181.30 | 178.26 | 184.24 | 3.3% |
| W1 | B2 | 4096 | bchlib v2.1.3 | `table-remainder` | 7 | 2233.08 | 2222.67 | 2241.20 | 0.8% |
| W1 | B2 | 4096 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 1.91 | 1.90 | 1.92 | 1.1% |
| W1 | B2 | 4096 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 1.90 | 1.88 | 1.91 | 1.6% |
| W1 | B2 | 4096 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.65 | 0.64 | 0.65 | 1.8% |
| W1 | B3 | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 110.85 | 110.11 | 111.10 | 0.9% |
| W1 | B3 | 1 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 3.26 | 3.20 | 3.30 | 3.1% |
| W1 | B3 | 1 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 3.22 | 3.21 | 3.25 | 1.0% |
| W1 | B3 | 1 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.38 | 0.38 | 0.38 | 0.5% |
| W1 | B3 | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 97.23 | 96.96 | 97.54 | 0.6% |
| W1 | B3 | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 414.63 | 413.44 | 416.64 | 0.8% |
| W1 | B3 | 16 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 3.50 | 3.47 | 3.50 | 0.9% |
| W1 | B3 | 16 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 3.50 | 3.48 | 3.52 | 1.0% |
| W1 | B3 | 16 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.40 | 0.40 | 0.40 | 0.6% |
| W1 | B3 | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 96.07 | 95.66 | 96.18 | 0.5% |
| W1 | B3 | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 410.57 | 409.82 | 412.32 | 0.6% |
| W1 | B3 | 256 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 3.43 | 3.42 | 3.46 | 1.1% |
| W1 | B3 | 256 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 3.44 | 3.43 | 3.46 | 1.0% |
| W1 | B3 | 256 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.39 | 0.38 | 0.39 | 1.8% |
| W1 | B3 | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 96.42 | 96.26 | 96.47 | 0.2% |
| W1 | B3 | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 410.83 | 410.41 | 411.29 | 0.2% |
| W1 | B3 | 4096 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 3.45 | 3.44 | 3.47 | 0.9% |
| W1 | B3 | 4096 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 3.46 | 3.45 | 3.47 | 0.6% |
| W1 | B3 | 4096 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.39 | 0.38 | 0.39 | 0.9% |
| W1 | T2N | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.65 | 38.62 | 38.66 | 0.1% |
| W1 | T2N | 1 | bchlib v2.1.3 | `table-remainder` | 7 | 4586.98 | 4387.54 | 4596.68 | 4.6% |
| W1 | T2N | 1 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 0.66 | 0.66 | 0.66 | 0.5% |
| W1 | T2N | 1 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 0.66 | 0.66 | 0.66 | 0.3% |
| W1 | T2N | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.46 | 38.36 | 38.67 | 0.8% |
| W1 | T2N | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 62.62 | 60.22 | 64.66 | 7.1% |
| W1 | T2N | 16 | bchlib v2.1.3 | `table-remainder` | 7 | 4569.42 | 4548.49 | 4575.00 | 0.6% |
| W1 | T2N | 16 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 0.62 | 0.47 | 0.66 | 30.0% |
| W1 | T2N | 16 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 0.65 | 0.64 | 0.67 | 3.1% |
| W1 | T2N | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.23 | 38.15 | 38.30 | 0.4% |
| W1 | T2N | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 61.65 | 60.97 | 62.53 | 2.5% |
| W1 | T2N | 256 | bchlib v2.1.3 | `table-remainder` | 7 | 4302.23 | 4222.99 | 4603.97 | 8.9% |
| W1 | T2N | 256 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 0.62 | 0.48 | 0.67 | 31.7% |
| W1 | T2N | 256 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 0.67 | 0.59 | 0.67 | 12.8% |
| W1 | T2N | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.26 | 38.08 | 38.37 | 0.8% |
| W1 | T2N | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 61.45 | 61.05 | 61.60 | 0.9% |
| W1 | T2N | 4096 | bchlib v2.1.3 | `table-remainder` | 7 | 4589.92 | 4563.36 | 4595.97 | 0.7% |
| W1 | T2N | 4096 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch-projected` | *estimate* | 0.64 | — | — | — |
| W1 | T2S | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 44.19 | 44.11 | 44.24 | 0.3% |
| W1 | T2S | 1 | bchlib v2.1.3 | `table-remainder` | 7 | 4576.20 | 4543.51 | 4597.41 | 1.2% |
| W1 | T2S | 1 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 0.78 | 0.77 | 0.79 | 2.2% |
| W1 | T2S | 1 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 0.78 | 0.77 | 0.79 | 2.0% |
| W1 | T2S | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 42.30 | 42.26 | 42.43 | 0.4% |
| W1 | T2S | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 67.46 | 67.40 | 67.61 | 0.3% |
| W1 | T2S | 16 | bchlib v2.1.3 | `table-remainder` | 7 | 4548.67 | 4527.30 | 4561.70 | 0.8% |
| W1 | T2S | 16 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 0.77 | 0.76 | 0.78 | 2.1% |
| W1 | T2S | 16 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 0.77 | 0.76 | 0.77 | 1.3% |
| W1 | T2S | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 41.95 | 39.49 | 42.01 | 6.0% |
| W1 | T2S | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 67.48 | 66.86 | 67.57 | 1.0% |
| W1 | T2S | 256 | bchlib v2.1.3 | `table-remainder` | 7 | 4554.24 | 4545.02 | 4564.36 | 0.4% |
| W1 | T2S | 256 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 7 | 0.76 | 0.54 | 0.76 | 29.2% |
| W1 | T2S | 256 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 7 | 0.76 | 0.76 | 0.77 | 0.6% |
| W1 | T2S | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 41.96 | 41.86 | 42.26 | 0.9% |
| W1 | T2S | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 48.70 | 48.67 | 48.86 | 0.4% |
| W1 | T2S | 4096 | bchlib v2.1.3 | `table-remainder` | 7 | 4515.39 | 4505.84 | 4518.40 | 0.3% |
| W1 | T2S | 4096 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-batch` | 3 | 0.73 | 0.72 | 0.76 | 4.6% |
| W1 | T2S | 4096 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `encode-loop` | 2 | 0.64 | 0.57 | 0.71 | 21.5% |
| W2 | B1 | 5 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 261.82 | 258.41 | 263.34 | 1.9% |
| W2 | B1 | 5 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `generator-matrix` | 7 | 6.30 | 6.15 | 6.31 | 2.7% |
| W2 | B1 | 5 | m4ri 20260122 | `echelonize` | 7 | 279.45 | 277.34 | 282.12 | 1.7% |
| W2 | B1 | 5 | m4ri 20260122 | `genmatrix-rref` | 7 | 270.00 | 235.99 | 288.08 | 19.3% |
| W2 | B1 | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 116.34 | 116.02 | 116.61 | 0.5% |
| W2 | B1 | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 158.34 | 157.20 | 158.45 | 0.8% |
| W2 | B2 | 64 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 160.00 | 159.45 | 161.47 | 1.3% |
| W2 | B2 | 64 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `generator-matrix` | 7 | 6.99 | 6.93 | 7.04 | 1.5% |
| W2 | B2 | 64 | m4ri 20260122 | `echelonize` | 7 | 1522.31 | 1515.08 | 1529.93 | 1.0% |
| W2 | B2 | 64 | m4ri 20260122 | `genmatrix-rref` | 7 | 1161.54 | 1148.70 | 1171.65 | 2.0% |
| W2 | B2 | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 2109.07 | 2103.10 | 2113.43 | 0.5% |
| W2 | B2 | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 2686.64 | 2677.58 | 2699.63 | 0.8% |
| W2 | B3 | 223 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 111.63 | 110.94 | 115.82 | 4.4% |
| W2 | B3 | 223 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `generator-matrix` | 7 | 7.32 | 7.26 | 7.53 | 3.6% |
| W2 | B3 | 223 | m4ri 20260122 | `echelonize` | 7 | 1564.15 | 1549.28 | 1575.07 | 1.6% |
| W2 | B3 | 223 | m4ri 20260122 | `genmatrix-rref` | 7 | 1493.91 | 1481.91 | 1505.71 | 1.6% |
| W2 | B3 | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 2461.16 | 2445.10 | 2478.89 | 1.4% |
| W2 | B3 | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 2981.71 | 2959.79 | 2994.46 | 1.2% |
| W2 | T2N | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 56.92 | 54.96 | 77.97 | 40.4% |
| W2 | T2N | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 70.96 | 54.72 | 87.81 | 46.6% |
| W2 | T2N | 32208 | aff3ct v4.7.0 | `basis-encode-pack` | 3 | 28.24 | 27.13 | 40.49 | 47.3% |
| W2 | T2N | 32208 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `generator-matrix-projected` | *estimate* | 0.64 | — | — | — |
| W2 | T2N | 32208 | m4ri 20260122 | `echelonize` | 7 | 157.16 | 120.70 | 164.79 | 28.1% |
| W2 | T2N | 32208 | m4ri 20260122 | `genmatrix-rref` | 7 | 307.76 | 217.45 | 387.73 | 55.3% |
| W2 | T2S | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 336.78 | 331.07 | 339.92 | 2.6% |
| W2 | T2S | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 450.59 | 434.63 | 453.63 | 4.2% |
| W2 | T2S | 7032 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 47.53 | 47.02 | 47.85 | 1.7% |
| W2 | T2S | 7032 | gf2 801d89852847afc789e9ddf26c1f1ebd3efb2a8b | `generator-matrix` | 3 | 1.55 | 1.42 | 1.56 | 9.3% |
| W2 | T2S | 7032 | m4ri 20260122 | `echelonize` | 7 | 790.81 | 788.49 | 794.30 | 0.7% |
| W2 | T2S | 7032 | m4ri 20260122 | `genmatrix-rref` | 7 | 1325.36 | 1319.49 | 1330.15 | 0.8% |

## Files

| File | SHA-256 | Bytes |
|---|---|---|
| `2026-08-31-4e732b56-generator-agreement.txt` | `ccd3474e8296a67e…` | 170 |
| `2026-08-31-4e732b56-kodo-retrieval.txt` | `cecfeef3993e7730…` | 6679 |
| `2026-08-31-4e732b56-m4ri-thread-evidence.txt` | `f6ecb9472812db92…` | 3258 |
| `2026-09-01-4e732b56-determinism-agreement.txt` | `314382095ab54880…` | 11640 |
| `2026-09-01-4e732b56-generator-matrix-agreement.txt` | `8b7a94fcdbb6a375…` | 2072 |
| `2026-09-01-4e732b56-small-aff3ct-perf-stat.txt` | `477b2c45da937a64…` | 3587 |
| `2026-09-01-4e732b56-small-aff3ct.csv` | `bc9d492ca85d55ee…` | 18975 |
| `2026-09-01-4e732b56-small-aff3ct.log` | `6ee59a4016952ca5…` | 2938 |
| `2026-09-01-4e732b56-small-bchlib.csv` | `d7fa607d46c7c7b1…` | 5046 |
| `2026-09-01-4e732b56-small-bchlib.log` | `3a451148acfc4a02…` | 1214 |
| `2026-09-01-4e732b56-small-generator-agreement.txt` | `ccd3474e8296a67e…` | 170 |
| `2026-09-01-4e732b56-small-gf2.csv` | `9ee2a9679682f025…` | 27195 |
| `2026-09-01-4e732b56-small-gf2.log` | `03be44fa1b4b46b8…` | 3251 |
| `2026-09-01-4e732b56-small-host.txt` | `1c721047ffb73d9d…` | 6947 |
| `2026-09-01-4e732b56-small-itpp.csv` | `6caa631f35e3ec41…` | 8175 |
| `2026-09-01-4e732b56-small-itpp.log` | `483f9a398da9353d…` | 1345 |
| `2026-09-01-4e732b56-small-m4ri.csv` | `e941c35b0084f0a3…` | 9403 |
| `2026-09-01-4e732b56-small-m4ri.log` | `a20b446e8c5f8a8c…` | 1467 |
| `2026-09-01-4e732b56-t2n-aff3ct-perf-stat.txt` | `fd0dd4d593e81600…` | 2078 |
| `2026-09-01-4e732b56-t2n-aff3ct.csv` | `aef2ed476ff63eec…` | 4866 |
| `2026-09-01-4e732b56-t2n-aff3ct.log` | `b32b948b4266b30d…` | 1125 |
| `2026-09-01-4e732b56-t2n-bchlib.csv` | `a7aba073dac40aa7…` | 2708 |
| `2026-09-01-4e732b56-t2n-bchlib.log` | `0094031cd0b79e91…` | 752 |
| `2026-09-01-4e732b56-t2n-generator-agreement.txt` | `ccd3474e8296a67e…` | 170 |
| `2026-09-01-4e732b56-t2n-gf2.csv` | `a6bc902e6feafa84…` | 5468 |
| `2026-09-01-4e732b56-t2n-gf2.log` | `032d8511328851bf…` | 1217 |
| `2026-09-01-4e732b56-t2n-host.txt` | `50255cbfa6e65647…` | 6938 |
| `2026-09-01-4e732b56-t2n-itpp.csv` | `eebab57153ce9b42…` | 90 |
| `2026-09-01-4e732b56-t2n-itpp.log` | `35e605e3c1dd47ad…` | 555 |
| `2026-09-01-4e732b56-t2n-m4ri.csv` | `66c70e11ac5a72d2…` | 2690 |
| `2026-09-01-4e732b56-t2n-m4ri.log` | `1be06c301303bd35…` | 770 |
| `generators.txt` | `612d5c84ee02ff69…` | 551 |

## Reproduction

```
dev/active/4e732b56/baseline-survey/fetch-build.sh
dev/active/4e732b56/baseline-survey/run-survey.sh dev/bench_results/4e732b56 <codes> <prefix>
dev/active/4e732b56/baseline-survey/make-receipt.py dev/bench_results/4e732b56
```
