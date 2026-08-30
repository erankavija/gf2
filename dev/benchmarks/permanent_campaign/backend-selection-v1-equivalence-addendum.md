# Cell-exhaustive equivalence addendum to backend selection v1

> The equivalence run has executed. The cited receipt is
> `dev/benchmarks/permanent_campaign/backend-selection-v1-equivalence.csv`,
> SHA-256
> `fda669f399700a968e3feae75c0f34cec2743c0df76fa404c0177ffba70100bc`.

This addendum supplies cell-applicable behavioural evidence for the
configurations nominated by
`dev/benchmarks/permanent_campaign/backend-selection-v1.md`, SHA-256
`fe5d37ba7c216a753bf3e546614a222c3c20e563f7f4e1d3c0341af9e1c464fe`.
It changes no backend selection, rate, campaign draw, or source receipt.

## Receipt identity

- Receipt:
  `dev/benchmarks/permanent_campaign/backend-selection-v1-equivalence.csv`,
  SHA-256
  `fda669f399700a968e3feae75c0f34cec2743c0df76fa404c0177ffba70100bc`.
- Execution transcript:
  `dev/benchmarks/permanent_campaign/backend-selection-v1-equivalence.log`,
  SHA-256
  `cc61211ef9760ea9a442bda6be64083b8768a3a416901ff324e4d815157bbe95`.
- Rows: 124 total ($q=3$: 50, $q=5$: 41, $q=7$: 33).
- Mismatches: 0; unexecuted nominations: 0; every row's status is `identical`.
- Wall time: 230 s, started `2026-08-30T03:22:05+03:00` and finished
  `2026-08-30T03:25:55+03:00`, inside the owner's 02:00-local safe window.
- Executable SHA-256:
  `3650d6cc1f1e1413c6ebbc0d1db7b35849fb4a990fd848b052c6c94c08f359f0`.
- Measured source closure, each a committed Git tree: harness
  `c72fcf9c2ca54023fe6f2c11760d736e4dda8b28`, `crates/`
  `4de075f6b54e0d5f6ddd69bff7c40b5ce1cc31ee`, and `permanent_wave_gpu`
  `8355e34ecc179439fb9c4c3ec10408f5262b8fe7`; `harness_dirty`, `deps_dirty`
  and `wave_gpu_dirty` are all `false`.
- Informational, not binding provenance: `git_sha`
  `db0c67e4e9561de50507eeab3c81219946d50182`.

The CSV preamble's `git_worktree_dirty: true` flag is attributable to the
untracked `.log`, which the runbook's `tee` creates in the worktree before the
harness starts. `HostInfo::probe()` captures the flag as the first statement
of `cmd_equivalence`, at
`dev/research/permanent-sampling-feas/src/main.rs:517`; the CSV is created
later in that same function, by the `open_csv` call at line 638. Nothing else
in the worktree was modified. `harness_dirty`, `deps_dirty` and
`wave_gpu_dirty` are all `false`, and the three committed trees plus
`binary_sha256` pin the measured source outright, so the closure's provenance
is unaffected.

The CSV preamble labels `harness_tree`, `deps_tree`, `wave_gpu_tree`, and
`binary_sha256` as binding provenance, with `harness_dirty`, `deps_dirty`, and
`wave_gpu_dirty` qualifying the committed trees. It labels `git_sha` and
`git_worktree_dirty` as informational. The preamble also records Rust and Cargo
toolchains, host identity, accelerator identity, ROCm version, timestamp, and
invocation, and records the matrix RNG as
`rand_chacha::ChaCha20Rng` (ChaCha20), version 0.9.0, and the root seed
`0xb488f02c00000001`. Each data row records its complete stream identity as
$(\text{seed root},q,n,\text{equivalence},0)$.

`equivalence` is `MeasurementPurpose::Equivalence`, purpose tag 1. It is
separate from `GridProbe`, `GridWarmup`, `GridTimed`, and every campaign
sampling purpose. An equivalence run therefore draws no campaign matrix.

## Recorded contradiction and prior scope

The selection receipt states that all 124 selection-cohort configurations pass
correctness rules 1 and 2 at their nominated cells. The evidence cited when
that statement was frozen does not establish that cell-exhaustive scope:

- `dev/active/b8206228-permanent-statistics/e31b1918-equiv-host-full.csv`,
  SHA-256
  `bccf37c6c89de00ee7dc56c98e326d38e447806869032e3068e3c00e9a88968d`,
  covers orders $\{8,12,16,20,24,28\}$ for each field;
- `dev/studies/047b62ed/permanent-campaign-20260814T230032Z-2085453-shared-equivalence.csv`,
  SHA-256
  `7d75c06e0f6761c4ffd166e4c1b0aa174a4fdc434b50c540cb7ef7b6d0527cd7`,
  has the same order grid; and
- `dev/benchmarks/permanent_campaign/exact-anchors.csv`, SHA-256
  `688ac3f425d1eb57c7f81f0538c4464375a1b8bba1426dfd1b3ed79a8582adc7`,
  supplies the additional exact-anchor cell $(3,4)$.

The resulting evidence scope is the six-order grid per field plus $(3,4)$,
not all 63 exact campaign cells. That discrepancy is retained here as the
contradiction required by `falsification-preserved`; the selection receipt
keeps its original bytes.

## Cell and configuration coverage

The harness embeds and parses the `Cell decisions` table in
`backend-selection-v1.md`. It does not re-rank or otherwise derive selections
from premeasurement data. The authority yields:

| Field | Exact cells | Nominated configurations |
|---:|---:|---:|
| 3 | 25 ($n=4,\ldots,28$) | 50 |
| 5 | 21 ($n=4,\ldots,24$) | 41 |
| 7 | 17 ($n=4,\ldots,20$) | 33 |
| **Total** | **63** | **124** |

Every cell has two nominated rows except $(5,24)$ and $(7,20)$, whose
authoritative table records one planned arm each. For every row the harness
draws one shared matrix corpus, computes one permanent value per matrix with
the reference implementation, evaluates the nominated backend on the same
corpus, and records the mismatch count. The packed scalar kernel is the
reference where supported. At $q=7,n>16$, the generic Ryser implementation is
the reference; a nominated generic-Ryser arm still retains its own row.

The receipt records 124 executed rows and zero mismatches, so the
cell-exhaustive receipt closes the coverage claim for all nominated
configurations at their exact nominated cells. A nonzero mismatch or an
unexecuted nomination would have kept the claim open and would have been
retained as falsifying evidence.

## Matrix-count budget derivation

Commit `de5f7414` fixes the derivation: start at the order ceiling, sum the
committed `probe_matrix_s` costs of the reference plus nominated backends, and
halve the matrix count until the projection fits 240 seconds, never going below
two. The ceilings are 512 through $n=20$, 32 through $n=24$, and 4
through $n=28$.

The finite costs come from the committed derivation record
`dev/benchmarks/permanent_campaign/probe-costs-de5f7414.csv`, SHA-256
`525100d986b8c2632b7c378c9059da8709f962c0d93d7b560b3393200debc3f0`,
which the harness embeds and parses. That record carries the `probe_matrix_s`
observations of run `20260813T230032Z-1321576` in the tree of `de5f7414` and
names its three source paths with the SHA-256 of each at that commit. An
exact-order cost is used when present. A missing exact-order cost uses the
nearest finite cost for the same field and backend at a higher order. This
rule is conservative and checkable: it never substitutes a cheaper lower-order
probe. Reference and nominated paths are de-duplicated when the nomination is
itself the reference.

Fixed-batch GPU rows above $n=12$ contain `NaN` rather than a single-matrix
probe. When no same-or-higher-order finite probe exists, the cell takes the
two-matrix floor. No latency is guessed, and the run preamble names every cell
where this rule applies.

| Cells | Ceiling | Committed cost arithmetic (seconds per matrix) | Result |
|---|---:|---|---:|
| $q=3,n=4\ldots12$ | 512 | $0.000025+0.000026+0.000188=0.000239$ from $n=12$; $512(0.000239)=0.122368$ s | 512 |
| $q=3,n=13\ldots15$ | 512 | $0.000270+0.000273+0.003321=0.003864$ from $n=16$; $512(0.003864)=1.978368$ s | 512 |
| $q=3,n=16\ldots28$ | 512, 32, or 4 | No same-or-higher-order finite GPU probe | 2 |
| $q=5,n=4\ldots12$ | 512 | $0.000157+0.000161+0.052684=0.053002$ from $n=12$; $512(0.053002)=27.137024$ s | 512 |
| $q=5,n=13\ldots23$ | 512 or 32 | No same-or-higher-order finite GPU probe | 2 |
| $q=5,n=24$ | 32 | $1.197137+1.196064=2.393201$ at $n=24$; $32(2.393201)=76.582432$ s | 32 |
| $q=7,n=4\ldots12$ | 512 | $0.000164+0.000161+0.065682=0.066007$ from $n=12$; $512(0.066007)=33.795584$ s | 512 |
| $q=7,n=13\ldots20$ | 512 | No same-or-higher-order finite GPU probe | 2 |

The deterministic tests reproduce the `de5f7414` halving example
$4(22.054209+22.269200+24.030546)>240$ and
$2(22.054209+22.269200+24.030546)\leq240$, verify the higher-order substitution,
verify the missing-cost floor, and check every campaign cell's final count.
They also check the embedded record against its committed SHA-256 and check
that the parser rejects a malformed row and an unknown backend name.

## Host-window runbook and runtime reservation

Build with the repository MSRV before the safe window:

```sh
test -z "$(git status --porcelain -- dev/research/permanent-sampling-feas dev/research/permanent_wave_gpu crates)"
# hipcc adds -I$ROCM_PATH/include only when ROCM_PATH is set. A non-login
# context (systemd unit, cron, CI) does not source /etc/profile.d/rocm.sh, so
# without this export every HIP translation unit fails on
# #include <hip/hip_runtime.h>. That is what happened on the first
# scheduled attempt.
export ROCM_PATH=/opt/rocm
cargo +1.95.0 build --locked \
  --manifest-path dev/research/permanent-sampling-feas/Cargo.toml \
  --release --features hip
```

The executed run was driven from a systemd user unit, where the `ROCM_PATH`
export is mandatory.

At or after 02:00 local time, run the complete receipt under the full-host
benchmark lock:

```sh
set -o pipefail
start=$(date +%s)
./dev/scripts/ccx1-bench-flock.sh --full-host \
  dev/research/permanent-sampling-feas/target/release/permanent_sampling_feas \
  equivalence \
  --out dev/benchmarks/permanent_campaign/backend-selection-v1-equivalence.csv \
  2>&1 | tee dev/benchmarks/permanent_campaign/backend-selection-v1-equivalence.log
status=$?
end=$(date +%s)
printf 'wall_seconds=%d exit=%d\n' "$((end - start))" "$status" \
  | tee -a dev/benchmarks/permanent_campaign/backend-selection-v1-equivalence.log
```

The expected receipt filename is
`dev/benchmarks/permanent_campaign/backend-selection-v1-equivalence.csv`; its
comment preamble is the provenance record. The sibling `.log` is the execution
transcript; GNU time is not installed on this host, so the wall time is the
shell-measured `wall_seconds=` line appended to the log. The command exits
nonzero for any mismatch or nominated configuration that does not execute.

Validate the finished row set and recover the executable identity with:

```sh
awk -F, '
  /^#/ || $1 == "q" { next }
  { rows++; per_q[$1]++; mismatches += $8; unexecuted += ($7 == 0) }
  END {
    printf "rows=%d q3=%d q5=%d q7=%d mismatches=%d unexecuted=%d\n", \
      rows, per_q[3], per_q[5], per_q[7], mismatches, unexecuted
  }
' dev/benchmarks/permanent_campaign/backend-selection-v1-equivalence.csv
sed -n 's/^# binary_sha256: //p' \
  dev/benchmarks/permanent_campaign/backend-selection-v1-equivalence.csv
tail -n 3 dev/benchmarks/permanent_campaign/backend-selection-v1-equivalence.log
```

The expected summary is
`rows=124 q3=50 q5=41 q7=33 mismatches=0 unexecuted=0`.

The harness has one global reference-plus-nominations invocation. `--q` and
`--n` can narrow cells, but there is no GPU-only/CPU-only split and no append
mode. The single CSV above must therefore start after 02:00 local time so every
GPU row stays inside the owner's window.

For cells whose complete probe sum is known, the planning estimate is the
arithmetic in the preceding table. A missing-cost cell reserves the entire
240-second cell budget instead of assigning an invented GPU latency. The
per-field reservations are:

| Field | Arithmetic | Reservation |
|---:|---|---:|
| 3 | $9(0.122368)+3(1.978368)+13(240)$ | 3,127.036416 s = 52 min 7.036 s |
| 5 | $9(27.137024)+11(240)+76.582432$ | 2,960.815648 s = 49 min 20.816 s |
| 7 | $9(33.795584)+8(240)$ | 2,224.160256 s = 37 min 4.160 s |
| **Total** | $3127.036416+2960.815648+2224.160256$ | **8,312.012320 s = 2 h 18 min 32.012 s** |

The completed receipt replaces the reservation with a recorded wall time of
230 s against a reservation of 8,312.012320 s. The reservation is not a
measured wall-time claim or upper bound. It is dominated by the
missing-probe-cost cells: 32 of the 63 cells reserve the full 240-second cell
budget each, so $32(240)=7,680$ s, about 92% of the total reservation. Those
same 32 cells execute the two-matrix floor, so their actual cost is a tiny
fraction of the budget reserved for them. That is why the recorded wall time
is far below the reservation, and it is the expected outcome of reserving a
full budget rather than inventing a GPU latency. The run's speed is not
evidence of anything about backend performance; this section makes no
performance claim.
