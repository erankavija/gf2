# Replicated permanent-backend ordering receipt

Issue `296a41c9`, measured 2026-08-09 on the repository benchmark host. The
machine-readable companion is [backend-ordering.csv](backend-ordering.csv).
It contains all 48 execution rows, four derived summary rows, the full stream
addresses used by every timed execution, and SHA-256 identities for the
uncommitted scratch raws.

## Verdict

At q=3, n=28, the M=1024 accelerator is separated from intra-matrix Rayon at
95% confidence and **the accelerator leads**. The primary pooled composite
rates are 19.3598 and 17.9943 matrices/s respectively. Across the 12 fresh
processes per configuration, the arithmetic-mean rate difference
(accelerator minus Rayon) is 1.2951 matrices/s. A conservative two-sample 95%
interval is [0.5691, 2.0211] matrices/s, wholly above zero.

Exactly the four preregistered configurations were measured:

| ID | Configuration | Executions | Matrices | Pooled seconds | Pooled composite matrices/s | Process-rate SD (RSD) | Within-process repetition RSD, median [range] | Envelope rate | Delta from envelope | Contradiction? |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| A | q=3, n=28, GPU HIP, M=1024 | 12 | 36,864 | 1,904.156081 | 19.359758 | 0.172727 (0.892%) | 0.166% [0.127%, 0.950%] | 19.2675 | +0.479% | no |
| B | q=3, n=28, intra-matrix Rayon | 12 | 5,760 | 320.101334 | 17.994302 | 1.129539 (6.252%) | 0.600% [0.053%, 12.493%] | 19.5785 | -8.092% | **yes** |
| C | q=5, n=24, batch Rayon | 12 | 5,760 | 498.197232 | 11.561686 | 0.385706 (3.333%) | 0.853% [0.069%, 11.214%] | 11.8308 | -2.275% | no |
| D | q=7, n=20, GPU HIP, M=1024 | 12 | 61,440 | 1,061.085070 | 57.902992 | 0.042238 (0.073%) | 0.020% [0.007%, 0.201%] | 57.9343 | -0.054% | no |

The q=3 interval uses the independent process rates, not the within-process
repetitions. With sample SDs 0.172727 and 1.129539 matrices/s and 12 processes
in each arm, its deliberately conservative common critical value is

`(mean_A - mean_B) +/- t(0.975, 11) * sqrt(sd_A^2/12 + sd_B^2/12)`,

where `t(0.975, 11) = 2.201`. The process means are 19.361167 and
18.066058 matrices/s. The pooled rates differ slightly from those means because
an execution's contribution is weighted by its timed duration; both comparisons
give the same ordering.

## Protocol

Each execution was a fresh executable process. One outer invocation of the
canonical `dev/scripts/ccx1-bench-flock.sh` held `/tmp/gf2-ccx1.lock` without a
gap for the entire 48-process run. `--full-host` deliberately omitted the
wrapper's six-core `taskset`, allowing the named Rayon configurations to use all
24 logical CPUs while retaining the same lock domain. CPU and GPU work by other
agents was quiesced for the measurement window. The wrapper's best-effort
`nice -n -5` was denied by the non-root host, so the child ran at inherited
niceness; locking and full-host placement were unaffected.

The first process performed the one locked 90-second whole-machine Rayon
warmup. Processes 1 through 47 used `--skip-machine-warmup`; every selected cell
still performed its own at-least-three-second warmup. A and the other three
configurations then used 3 and 5 timed repetitions respectively. The run began
at 2026-08-09T16:29:37Z and the final scratch receipt completed at
2026-08-09T17:58:35Z, about 88 minutes 58 seconds. All 48 processes exited
successfully. None was restarted, replaced, or discarded.

The 16-position balanced block was

`A B C D  B C D A  C D A B  D A B C`

and it was repeated three times without reordering. Thus every configuration
appears once in each four-position segment and three times at each of the four
within-segment positions. Execution IDs 0 through 47 in the CSV are the exact
schedule order.

The executable was built before measurement with:

```sh
cargo +1.95.0 build --release \
  --manifest-path dev/research/permanent-sampling-feas/Cargo.toml \
  --features hip
```

The locked measurement driver was:

```sh
./dev/scripts/ccx1-bench-flock.sh --full-host zsh -c '
set -euo pipefail
bench=./dev/research/permanent-sampling-feas/target/release/permanent_sampling_feas
raw=dev/research/permanent-sampling-feas/target/backend-ordering-raw-296a41c9
mkdir "$raw"
typeset -A filter
filter[a]="q=3,n=28,backend=gpu_hip,batch_size=1024"
filter[b]="q=3,n=28,backend=cpu_rayon_intra_matrix"
filter[c]="q=5,n=24,backend=cpu_rayon_batch_scalar"
filter[d]="q=7,n=20,backend=gpu_hip,batch_size=1024"
typeset -a block schedule
block=(a b c d b c d a c d a b d a b c)
schedule=(${block[@]} ${block[@]} ${block[@]})
for execution_id in {0..47}; do
  config=${schedule[$((execution_id + 1))]}
  output=$(printf "%s/exec-%02d-%s.csv" "$raw" "$execution_id" "$config")
  args=(grid --out "$output" --only "${filter[$config]}" --execution-id "$execution_id")
  (( execution_id == 0 )) || args+=(--skip-machine-warmup)
  "$bench" "${args[@]}"
done
'
```

Every individual expanded invocation is retained in the CSV's corresponding
raw scratch file and was checked against its execution ID and expected filter.
No configuration outside A-D was emitted.

## Rate and dispersion definitions

The timed composite is draw and pack (`gen_s`), permanent evaluation
(`eval_s`), zero count/reduction (`reduce_s`), and scratch-record write
(`store_s`). For an execution and for a configuration summary alike, the
reported primary rate is formed from pooled totals:

`composite rate = sum(matrices) / sum(gen_s + eval_s + reduce_s + store_s)`.

It is not a mean of per-repetition reciprocals. The validator independently
recomputed each execution rate from `matrices / total_s` and checked that the
four component times sum to `total_s` within raw receipt precision.
The timing fixture's zero counts are retained in the CSV for traceability but
were never consulted by the schedule, rate calculation, or backend verdict.

Two distinct dispersions are reported:

- Within an execution, `rep_sd_s / (total_s / reps)`, the sample SD of timed
  repetition durations relative to their mean. The table reports the median
  and full range of these 12 RSDs.
- Across executions, the sample SD and RSD of the 12 independent fresh-process
  composite rates. These do not pool repetitions across process boundaries.

For the envelope check, a disagreement is a contradiction exactly when the
absolute percent difference between the new pooled rate and the envelope rate
exceeds the new across-execution rate RSD for that configuration.

## Seeds and stream addresses

The fixed timing root is `0xb488f02c00000001`; a full address is
`(seed_root, q, n, stream_index)`. Execution ID `e` reserves the checked,
non-overlapping inclusive stream-index block
`e * 12,000,000 + 1 ..= (e + 1) * 12,000,000`. The per-cell warmup consumes the
first stream. The CSV records the reserved block, first and last timed indexes,
full first and last addresses, and exact matrix count for every process. The
first timed address is therefore index 2 for execution 0; the last execution
reserves 564,000,001 through 576,000,000 and times indexes 564,000,002 through
564,000,481. The mechanical check proved all 48 blocks disjoint and every timed
range contained in its block.

## Provenance and host state

- Harness/HEAD revision: `414d31f8184a398deee946f151134511522dfca3`
  (`fix(296a41c9): support replicated backend timings`), clean at build and
  measurement.
- Measured dependency-tree revision: `d950bbb883845429d378aa2708ae7406b06fa6bc`.
- Executable SHA-256:
  `6e24533cfbac987a0cec20af02f9dfb0a7bd9ce12c9e80cbdc00cad72150ccad`.
- Build: Rust/Cargo 1.95.0, release profile, `hip` feature. The executable's
  embedded compiler string is `rustc 1.95.0 (59807616e 2026-04-14)`.
- CPU: AMD Ryzen 9 5900X, 12 cores / 24 logical CPUs; Rayon threads 24; governor
  reported `powersave`; AVX2 available and AVX-512F unavailable.
- GPU: AMD Radeon RX 6950 XT, `gfx1030`; ROCm 7.2.4; kernel
  `7.1.6-arch1-1`. GPU access used approved host execution because the ordinary
  sandbox does not expose `/dev/kfd`.
- Post-run health after releasing the lock: GPU idle 0%, VRAM 2%, 7 W;
  56 C edge, 59 C junction, 60 C memory. `rocm-smi` emitted only its normal
  low-power-state warning for the now-idle device.

The raw harness preamble invokes unqualified `rustc --version` and
`cargo --version` at runtime, so it recorded the host default 1.97.0 even though
the pinned build command used 1.95.0. This provenance mismatch is preserved,
not rewritten: the CSV has separate `rust_build_toolchain`,
`cargo_build_toolchain`, `runtime_probe_rustc`, and `runtime_probe_cargo`
columns. The binary hash and embedded 1.95.0 compiler string identify what was
actually timed.

## Preserved contradictions and anomalies

The q=3 intra-matrix Rayon pooled rate is 8.092% below the envelope's 19.5785
matrices/s, wider than its measured 6.252% across-process RSD. This is an
explicit contradiction. In particular, execution 27 fell to 15.0791
matrices/s with 12.493% within-process RSD, and execution 36 reached only
17.3850 matrices/s with 11.722% within-process RSD. They remain in the pooled
result. The interleaved q=3 GPU arm remained much steadier, and its replicated
lead is not obtained by removing these CPU observations.

The q=5 batch-Rayon arm also contained slow/noisy executions: execution 24 was
11.0616 matrices/s and execution 40 was 10.9463 matrices/s with 11.214%
within-process RSD. They too remain included. Its pooled-envelope difference
of 2.275% does not exceed its measured 3.333% across-process RSD, so it is not
classified as an envelope contradiction under the stated rule. The q=3 GPU
and q=7 GPU envelope differences likewise remain within their measured
cross-execution dispersion.

The earlier feasibility receipt's point estimates placed q=3 intra-matrix
Rayon 1.6% ahead but correctly declined to assert an ordering. This replication
resolves that formerly open comparison in the opposite point-estimate direction
and preserves the earlier measurement rather than replacing it. The separate
28.65x/30.32x accelerator headline remains qualified exactly as the feasibility
study records: it reproduces only against a single-thread AVX2 baseline; against
the best measured CPU path its ratios are 0.46x and 0.44x. The present four-cell
run did not remeasure those M=256 comparisons and does not relabel them.

## Mechanical validation

Before measurement, the permanent-receipt structure check was RED because the
receipt was absent and therefore had zero executions and no interleave. The
existing lock wrapper test was also observed RED before `--full-host` existed.
CLI filter/address tests were authored before implementation but, to respect the
shared Cargo hold, were first executed only after implementation when Cargo GO
was granted. The authorized GREEN validation was:

- lock-wrapper shell test: 1/1 passed;
- Rust 1.95 release harness tests: 26/26 passed, including exact GPU batch-size
  filtering, execution-ID parsing, disjoint address blocks, and overflow checks;
- Rust 1.95 release HIP build: passed at `414d31f8`;
- post-measurement collation: 48/48 execution files, 12/12/12/12 configuration
  counts, exact three-block schedule, unique execution IDs, exact tuple and
  batch filters, disjoint contained stream ranges, one row per raw, common
  revisions and binary, and pooled-rate arithmetic all passed;
- permanent CSV shape: 48 execution rows plus four summary rows and no other
  configuration rows.

No execution failed. No Cargo CI run was performed locally because it was not
authorized for this measurement-only issue.
