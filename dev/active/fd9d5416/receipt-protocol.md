# Receipt protocol for the BCH performance receipts (fd9d5416)

| Field | Value |
|---|---|
| JIT issue | `fd9d5416` (Committed performance receipts: non-regression, determinism, SOTA comparison) |
| Epic | `ae03bcd0`, criteria REQ-13, REQ-14 of [plan.md](../ae03bcd0-general-bch/plan.md) |
| Run | [`dev/active/d1b4f85e/run.sh`](../d1b4f85e/run.sh), cells of the `d1b4f85e` amendment of [workload-selection.md](../4e732b56/workload-selection.md) |
| Renderer | [`render_receipt.py`](render_receipt.py); tests in [`tests/test_render_receipt.py`](tests/test_render_receipt.py) |

This note fixes how the receipt turns one window run into the REQ-01 and
REQ-02 verdicts of `fd9d5416`. The renderer applies it and computes every
figure from committed files at render time.

## 1. Inputs

| Input | Path | Role |
|---|---|---|
| Window run | `<run-dir>/` from `run.sh`: `host.txt`, `logs/`, `dispatch.jsonl`, `samples/` | New measurements and dispatch record |
| Pinned pre-cutover receipt | [`dev/bench_results/88ca7d2f/`](../../bench_results/88ca7d2f/) | Tier A baseline (Criterion samples) |
| ID mapping | [`dev/active/591a1c5e/smoke-run.md`](../591a1c5e/smoke-run.md) § ID mapping | Which Tier A cells are comparable |
| External-baseline survey | [`dev/bench_results/4e732b56/`](../../bench_results/4e732b56/) CSVs | Tier B baseline (survey `gf2` column) and REQ-02 rows |

## 2. Statistical rule

The plan's rule, quoted from [plan.md](../ae03bcd0-general-bch/plan.md)
§ `evidence-protocol`:

> Performance acceptance ("no statistically measurable regression"): the
> one-sided lower 95% bootstrap confidence bound (10,000 resamples over
> Criterion's collected samples, Criterion defaults otherwise) of the
> throughput ratio new/baseline is at or above $0.98$ on every selected
> workload. The baseline is the pre-cutover receipt at its pinned revision
> (D-11).

The plan fixes the resample count, level, and threshold; it leaves the
statistic open. The receipt fixes it:

- **Statistic.** Throughput at fixed work is the reciprocal of time, so the
  ratio is $\mathrm{median}(t_\text{base}) / \mathrm{median}(t_\text{new})$
  over per-iteration times (`sample.json` `times[i] / iters[i]`). The median
  is the figure the `88ca7d2f` receipt reports.
- **Bootstrap.** Two-sample percentile bootstrap: each of the 10,000
  resamples draws both sides with replacement at their own sizes. The lower
  bound is the 5th percentile of the resampled ratios. The RNG is seeded per
  cell from `0xAE03BCD0` and the cell ID, so a re-render reproduces every
  bound.
- **Acceptance.** Lower bound $\ge 0.98$.

## 3. Non-regression cells (REQ-01)

### Tier A: the pinned receipt `88ca7d2f`

Both sides run the same configuration: `batch_operations` and `bch_parallel`,
filter `bch_`, default features, `ccx1-bench-flock.sh --full-host`.

| Cell (same ID both sides) | 591a1c5e status | In verdict |
|---|---|---|
| `bch_batch_decode/{1,10,50,100}` | kept | yes |
| `bch_single_vs_batch/single_loop` | kept | yes |
| `bch_sequential_vs_batch/{sequential_loop,batch_operation}` | kept | yes |
| `bch_batch/{1,10,50,100}` → `bch_encode_pns_16383_16215/*` | renamed, not comparable (different code) | no, reported |
| `bch_single_vs_batch/batch_api` | removed (no batch call on the canonical DVB-T2 decoder) | no |
| `bch_single_vs_batch/decode_into_loop` | new, no baseline | no, reported |

### Tier B: the survey's pre-cutover `gf2` column (W1, DVB-T2 encode)

`88ca7d2f` has no cell of either contract workload, and its DVB-T2-sized
encode cell is the non-comparable `bch_batch`. The pre-cutover W1 and W2
measurements on the contract rows exist only as the `gf2` rows of the
`4e732b56` survey: legacy `BchEncoder::encode_batch` and
`generator_matrix` at revision `38e0335a`, 3–7 trials per cell, same host,
CCX1 pin, `powersave`, rustc 1.97.0. Between `38e0335a` and the pinned
`74871c6b`, the legacy encoder source in `crates/gf2-coding/src/bch/core.rs`
changes only in documentation, the generator-matrix cache, and the decoder.

| New cell | Survey cell | In verdict |
|---|---|---|
| `bch_encode_w1/selected=<f>/W1/fresh-alloc/{B1,B2,B3}/B=<b>` | `gf2,W1,encode-batch,<row>,<b>` | yes |
| `bch_encode_w1/route=shortened-restriction/W1/fresh-alloc/{T2S,T2N}/B=<b>` | `gf2,W1,encode-batch,<row>,<b>` | yes, except T2N `B=4096` |
| T2N `B=4096` | `encode-batch-projected` (an estimate) | no: no measured baseline |
| `bch_genmatrix_w2/materialize/fresh-alloc/{B1,B2,B3,T2S}` | `gf2,W2,generator-matrix,<row>` | no, reported |
| `bch_genmatrix_w2/materialize/fresh-alloc/T2N` | `generator-matrix-projected` | no: no measured baseline |

Both sides of a W1 pair are `fresh-alloc` at $W = 1$, the state contract § 5
names for the like-for-like pre-cutover comparison, and the renderer checks
that $n$ and $k$ agree. The baseline side's resample unit is the survey
trial: a mean over at least 5 ms of calls.

The W2 pairs are not like-for-like: the survey times
`cs.build().generator_matrix()`, construction included, and the Criterion
cell times `generator_matrix()` on a prebuilt code. The bias favours the new
side, so a pass would not show non-regression; the rows are reported and
excluded.

### Cells with no pre-cutover counterpart

Reported in the full cell table only: the `W6` cells (contract § 6; survey
§ 8.4 records no pre-cutover parallelism), the per-family and per-kernel-arm
cells, every `warm-reuse` cell, the `T2S-mother`/`T2N-mother` cells, the
nonbinary rows N1–N4, and `bch_paritycheck`.

## 4. Determinism

The evidence is the dispatch record. The renderer groups W1 records by
(group, row, $B$) and W2 records by (group, row). It requires one
`output_fnv1a` per group, and a `rayon_pool_width` equal to `workers` on every
`W6` record. A W1 group spans $W \in \{1, 6\}$, every registered family, both
kernel arms, and both cache states, so one digest per group is the
contract § 6 requirement ("the same seed produces the same codewords at both
worker counts") and family equivalence at once. `run.sh` checks the same
condition before collecting, and the benches assert it before timing. Seeds:
binary messages are `BitVec::random_seeded(k, BCH_CORPUS_SEED + i)`, and
nonbinary messages come from `StdRng::seed_from_u64(BCH_CORPUS_SEED)`. The
renderer reads `BCH_CORPUS_SEED` from `test_support.rs` at the run's revision.

## 5. Scalar-fallback coverage

Covered cells: `family=bitslice-interleaved[scalar]` and
`family=clmul-fold[scalar]` at W1 `warm-reuse`, on B1, B2, B3, `T2S-mother`, and
`T2N-mother` at every $B$, which is 40 cells. Each pairs with its detected-arm
cell (`[avx2-pclmul]` on this host) in the same determinism group. The
renderer marks a pair covered when the scalar arm has Criterion output and
both arms share the digest. The receipt cites the test-level witnesses
without re-running them: `the_forced_scalar_kernels_write_the_reference_bytes`
(`crates/gf2-coding/tests/bch_encode_dispatch.rs`), and
`the_scalar_bundle_matches_the_per_frame_reference` and
`the_detected_bundle_matches_the_scalar_bundle_word_for_word`
(`crates/gf2-kernels-simd/src/bch_encode.rs`).

## 6. Selected path and `W6`

No calibrated gf2-coding encode profile exists, because
`dev/tools/tuning-profile-compose` covers only the core and algebra owners.
Under the conservative profile every `selected=` cell, and therefore every
`W6` cell, measures `poly-remainder-scalar`. The receipt reports `W6` as the
reference family's parallel throughput and states that. The per-family `W1`
cells carry the measured crossover that a calibrated profile would consume.

## 7. External-baseline rows (REQ-02)

### Admissibility of the committed survey numbers

The protocol texts that govern re-measurement:

- Plan § `evidence-protocol`: "Hosts follow the receipt conventions of the
  SOTA target matrix (uncontended, pinned toolchain, committed
  seeds/revision/host)."
- SOTA reference acceptance protocol § 5
  ([sota_reference_acceptance_protocol.md](../../archive/026fc832-gf2-core-sota-stretch/plans/sota_reference_acceptance_protocol.md)),
  which the SOTA target matrix applies: "Cross-host runs are permitted; they
  must publish their own `host.txt`, but they cannot displace a Zen-3
  baseline. If the dev host is replaced, every promoted reference must be
  re-measured before the new host's baseline is committed".
- Survey [findings.md](../4e732b56/findings.md) § 8.3: "the `perf-receipts`
  task should re-establish its non-regression baseline on the governor it
  intends to keep rather than inheriting these numbers as absolutes. The
  *ratios* between implementations, which is what the selection rests on, are
  far too large to be explained by governor effects."

No text requires the external libraries to be re-measured in the receipt's
window. The trigger for re-measurement is a host replacement. The survey's
host records (`fraktaali`, Ryzen 9 5900X, `powersave`, rustc 1.97.0, CCX1 pin
through the same wrapper) match the window run's host. The one recorded
difference is the kernel, 7.1.11 to 7.2.6. `88ca7d2f` carries the same
difference. The § 8.3 governor condition holds: the window keeps `powersave`.
**Decision:** the committed `4e732b56` survey numbers are admissible, the
receipt records the kernel difference, and no external re-measurement window
is needed.

### Rows

At $W = 1$ (contract § 6), throughput in contract units (W1 $Bk/T$, W2
$kn/T$):

- **W1**, every row × $B$. gf2 is the fastest `warm-reuse` W1 cell of the
  row at contract length: the best family on B1–B3, and
  `route=shortened-restriction` on T2S/T2N. The externals are bchlib
  `table-remainder` (primary; B2, T2S, T2N) and AFF3CT `lfsr-simd-inter`/
  `lfsr-scalar` (secondary; every row). They are compared in their declared
  `warm-reuse` state (contract § 5).
- **W2**, every row. gf2 is `materialize/fresh-alloc`, against M4RI
  `genmatrix-rref` (primary) and AFF3CT `basis-encode-pack` (secondary), all
  `fresh-alloc` (survey § 8.2).

Each row reports the strongest external median, the gf2/external ratio, and
whether the aspirational target (gf2 at or above the strongest external) is
met. The receipt states the count either way. The external side has 3–7 trials
per cell and no bootstrap, so the REQ-02 rows are point comparisons rather
than acceptance tests. Survey and Criterion messages share the seed constant
but come from different generators, so the external rows carry no
digest cross-check.

## 8. REQ-13 comparison basis for DVB-T2 encode

| Option | Evidence | Assessment |
|---|---|---|
| A. Survey `gf2` column (Tier B) | Exists; contract rows and batches; same host, pin, governor, toolchain; per-trial data | Different harness (trial means, 3–7 trials) and revision `38e0335a`, whose legacy encode source matches the pinned revision; T2N `B=4096` is a projection |
| B. Re-measure the legacy encoder at `74871c6b` with a Criterion bench added | Same harness as the new cells | A new receipt rather than the pinned one; needs a patched detached worktree and another window: legacy T2S at about 9 ms per frame and T2N at about 48 ms per frame put T2N `B=4096` at about 3 min per iteration |
| C. `88ca7d2f` `bch_batch` against `bch_encode_pns_16383_16215` | Same IDs | Different codes (591a1c5e: not comparable) |
| D. No DVB-T2 non-regression cell | — | Leaves REQ-13's "every selected workload" unanswered on the rows the epic ships |

**Recommendation: A.** It is the only committed pre-cutover measurement of
the contract's DVB-T2 cells, and its host conditions match the window. The
two-sample bootstrap absorbs its small trial counts by widening the bound, so
any loss of power shows up as a lower bound rather than as a false pass. B is
the fallback if research review rejects a cross-harness ratio.

## 9. Known effects on reported cells

- `Shortened::encode_into` allocates two mother-length buffers per call
  (`a33fda32`). It affects the T2S/T2N `route=` cells. A Tier B failure there
  makes `a33fda32` a prerequisite.
- `Shortened::generator_matrix_into` copies bit by bit, so the T2S/T2N W2
  contract-length cells measure that copy. The receipt reports them against
  M4RI as measured.

## 10. Renderer test

`tests/test_render_receipt.py` builds a run directory in a temporary
directory. The Criterion IDs come from the two smoke-run ID lists plus the
eight `GF2_BENCH=1` W2 cells. The sample times are synthetic multiples of the
committed baseline and survey medians. Nothing it generates is committed.
The tests cover these cases:

- An all-faster run passes every check.
- A 5% slower Tier A cell fails.
- A 10% slower Tier B T2S cell fails.
- A split W1 digest fails determinism.
- A missing scalar-arm output is reported as a gap.

Usage: `python3 dev/active/fd9d5416/tests/test_render_receipt.py`; render with
`dev/active/fd9d5416/render_receipt.py <run-dir> > <receipt>.md`.
