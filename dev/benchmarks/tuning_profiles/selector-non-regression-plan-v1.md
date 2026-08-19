# Pinned selector non-regression set and tolerance v1

This is the frozen non-regression contract for the tuning-profile selector
cutover of epic `6dc81018`, filed by issue `e8fe47f5` under step 3 of
[`design.md`](/dev/active/220cab0b/design.md) §6. It fixes three things before
any cutover measurement exists: the pinned benchmark cells, the run protocol
that produces a receipt, and the tolerance a later receipt is judged against.

A migrated selector goes public only after a receipt taken under this protocol
compares within the tolerance below against the pre-cutover baseline. Issue
`278acf3a` takes that baseline, `50b47eae` takes the post-cutover receipt, and
both run this procedure unmodified.

The runnable implementation is the bench target
[`selector_non_regression`](/crates/gf2-core/benches/selector_non_regression.rs),
which pins its behavioural identity with the schema token
`selector-non-regression-v1`. A receipt carrying that token and a cell set other
than the one below is not comparable, and the comparison mode rejects it.

[`2026-08-19-procedure-verification.md`](2026-08-19-procedure-verification.md)
records one end-to-end execution of everything below, including the noise this
protocol carries on the host it is written for. That run is not a baseline.

## 1. What the set covers

The two pilot selector families of design §4 gate behaviourally equivalent
execution paths on operand size. The cutover replaces each compiled-in constant
with a `OnceLock`-backed profile read, so the risk it carries is a per-call cost
added to the selection boundary rather than a change of result. The pinned set
is therefore built to make that cost visible:

- Both arms of every conservative default are inside the measured set, so a cell
  that changes arm is a protocol error rather than a silent re-measurement of a
  different algorithm.
- Sizes bracket each default as tightly as its guard permits, so a moved default
  cannot leave the set measuring one arm twice.
- The cheapest bit-logical operation is inside the set at a buffer well under
  the SIMD default, because a fixed per-call cost is a large relative cost only
  where the operation itself is a handful of instructions. Design §8 records
  this as the acute risk of the cutover, and design §6 step 3 requires the cell.

`SUBPRODUCT_THRESHOLD` gates two public entry points, `batch_evaluate` and
`batch_evaluate_auto`. Both are pinned, per lead decision DEC-B1: cutting one
without the other leaves the entry points selecting on different authorities,
and a set covering only one would not detect it.

## 2. The pinned cells

This table is normative. The bench target implements it, and step 0 of the
protocol below checks that the two agree.

Sizes are chosen by one rule per threshold: take the guard's controlling
quantity, pin the largest value that selects the conservative arm and the
smallest that selects the asymptotic arm, and add outlying sizes on each arm so
a receipt shows the cost curve rather than two isolated points.

### Bit-backend family

The controlling quantity is the buffer's `u64` word count, compared against the
SIMD default of 8 words by `select_backend_for_size`. All five entry points that
read it are pinned. `words=1` is the required small-buffer cell.

| Entry point | Pinned word counts | Arms |
|---|---|---|
| `kernels::ops::xor_inplace` | 1, 4, 7, 8, 16, 64 | scalar at 1–7, SIMD at 8–64 |
| `kernels::ops::and_inplace` | 1, 8 | scalar, SIMD |
| `kernels::ops::or_inplace` | 1, 8 | scalar, SIMD |
| `kernels::ops::not_inplace` | 1, 8 | scalar, SIMD |
| `kernels::ops::popcount` | 1, 8 | scalar, SIMD |

### Polynomial family

All cells run over `Fp<65537>`. The controlling quantity differs per threshold
and is named with the sizes.

| Entry point | Threshold and controlling quantity | Pinned sizes | Arms |
|---|---|---|---|
| `FieldPoly::mul` | `KARATSUBA_THRESHOLD` = 32 on operand degree, `len - 1` | `len` 16, 32, 33, 64, 256 | schoolbook at 16–32, Karatsuba at 33–256 |
| `poly::mul_fast` | `NTT_THRESHOLD` = 128 on `out_len = 2·len - 1` | `len` 32, 64, 65, 128, 512 | `mul` dispatcher at 32–64, NTT at 65–512 |
| `FieldPoly::div_rem_auto` | `DIV_REM_THRESHOLD` = 2048 on both operand lengths | (dividend, divisor) = (4096, 1024), (4096, 2047), (4096, 2048), (8192, 4096) | schoolbook at divisor 1024–2047, Newton at 2048–4096 |
| `FieldPoly::batch_evaluate` | `SUBPRODUCT_THRESHOLD` = 4096 on point count and coefficient count | (coeffs, points) = (2048, 2048), (4095, 4095), (4096, 4096) | Horner at 2048–4095, subproduct at 4096 |
| `FieldPoly::batch_evaluate_auto` | same threshold, second entry point | same three sizes | Horner, subproduct |

Thirty-four cells in total: fourteen bit-backend and twenty polynomial.

`out_len` is odd for equal-length operands, so 128 is bracketed by 127 and 129
rather than by 128 and 129; that is the tightest bracket the guard admits.

### Arm recording

Every recorded row carries the arm its cell selects. The bit-backend arm is read
from `select_backend_for_size` itself, so the receipt states what the library
decided rather than what the harness predicted. The polynomial arms are computed
from the four public threshold constants, which no public API exposes a
selection observation for.

This procedure installs no tuning profile, so every cell resolves on the
conservative defaults on both sides of the cutover. A receipt taken with a
profile installed is outside this protocol.

## 3. Run protocol

Prepared benchmark host, nothing else running, `GF2_BENCH=1`, and the repository
lock wrapper holding `/tmp/gf2-ccx1.lock` for the whole run. The wrapper pins to
CPUs 6–11 and blocks on contention; it passes neither `-n` nor a timeout, so a
contended run waits rather than proceeding unpinned.

Five fresh executions of five repetitions each, `--target-ms 250` — the
repository's established receipt protocol, matching
[`batched-f3-avx2-provenance-fixed.md`](/dev/benchmarks/permanent_campaign/batched-f3-avx2-provenance-fixed.md).
Each execution is a separate process, so process-to-process variation is
measured rather than averaged away inside one process. Call counts are
calibrated per process against the 250 ms target; pooling by totals (§5) makes
differing call counts across executions correct rather than merely tolerable.

Build with the MSRV toolchain so the recorded `rustc` string is the compiler
that produced the measured binary.

**Step 0 — check the harness against this document.**

```sh
cargo +1.95.0 bench -p gf2-core --features simd \
  --bench selector_non_regression -- --self-check
cargo +1.95.0 bench -p gf2-core --features simd \
  --bench selector_non_regression -- --list-cells
```

`--self-check` asserts the straddle properties of §1 and prints the protocol
line, whose `per_cell_tolerance` and `set_tolerance` must equal §4. `--list-cells`
prints the enumerated set, which must reproduce §2. Record both outputs in the
receipt.

**Step 1 — measure.** Write to a unique absent `/tmp` path; never write a
receipt directly into the repository, because an in-repository output file is
itself an untracked source change and would make every subsequent row record
`source_dirty=true`.

```sh
OUT=/tmp/gf2-e8fe47f5-<purpose>-<short-revision>.csv
test ! -e "$OUT"
GF2_BENCH=1 ./dev/scripts/ccx1-bench-flock.sh bash -lc '
  for e in 1 2 3 4 5; do
    cargo +1.95.0 bench -p gf2-core --features simd \
      --bench selector_non_regression -- \
      --execution "$e" --repetitions 5 --target-ms 250 \
      --output "'"$OUT"'" --append
  done'
```

**Step 2 — copy.** Copy the `/tmp` file byte-for-byte into
`dev/benchmarks/tuning_profiles/` and record its SHA-256 in the receipt.

Every cell runs an equivalence probe before its first timed window: operand
lengths against the pinned sizes, the bit-backend arm against
`select_backend_for_size`, `mul_fast` against `FieldPoly::mul`, the division
identity `q·d + r = a` with a remainder degree below the divisor's, and each
batch-evaluation result against per-point Horner. A failed probe aborts the run,
so a receipt exists only for a run whose cells computed the right answers.

### Controlled fixture alignment

Each bit-logical cell holds its eight fixture buffers in one contiguous
allocation with every buffer starting on a 64-byte boundary, and the probe
asserts that before timing. The allocator's address phase differs between
processes, so buffers left at its default 8-byte alignment make a whole
execution measure whichever side of a cache line its bank happened to land on.
That is a property of the heap rather than of the selection boundary, and it
dominates the SIMD arm at small buffers: the verification receipt records the
measurement it produced before the banks were aligned, where two of thirty-four
cells moved by 8.9 % and 13.3 % between two runs of identical code, against a
worst case of 1.91 % once aligned.

## 4. Predeclared tolerance

Declared here, before any cutover measurement exists. Neither value is derived
from a cutover receipt, and neither is widened to admit one.

- **Per cell, τ_cell = 5 %.** A cell fails when its pooled ns/call rises above
  1.05 × the baseline's.
- **Whole set, τ_set = 2 %.** The set fails when the geometric mean of the
  thirty-four per-cell ratios rises above 1.02.

Both must hold; either alone failing fails the comparison.

**Derivation.** The grounding is the dispersion recorded by
[`batched-f3-avx2-provenance-fixed.md`](/dev/benchmarks/permanent_campaign/batched-f3-avx2-provenance-fixed.md)
§Dispersion, taken on this host under this wrapper at the same five-execution,
five-repetition, 250 ms protocol. Its two well-behaved backends record
across-execution coefficients of variation of 0.045 %–0.5 %. τ_cell at 5 % is an
order of magnitude above the top of that band, so a single cell trips it only on
a real cost rather than on process-to-process noise.

τ_set is deliberately much tighter than τ_cell because the two rules catch
different failures. The cutover adds a fixed per-call cost at every selection
boundary, so its characteristic signature is a small shift shared by all
thirty-four cells rather than a large shift in one. Averaging thirty-four cells
suppresses independent noise by roughly `1/√34`, which puts the noise floor of
the geometric mean near 0.1 % and leaves 2 % as a wide margin against noise
while still failing a uniform 2 % regression that no per-cell rule would catch.

**Confirming measurement.** Both values were fixed before any run of this
protocol produced a number. The verification receipt then compares two
independent cohorts of identical pre-cutover code, where the true ratio is 1, and
reports the noise the tolerance has to absorb: the widest per-cell deviation is
1.91 % and the geometric mean sits at 1.000810. τ_cell therefore carries a
factor of 2.6 over the worst observed cell and τ_set a factor of 25 over the
observed set statistic, so neither rule is marginal at its declared value.

**The small-buffer cells are expected to be the binding constraint.** A
`OnceLock` read is a fixed cost of roughly a nanosecond against operations that
cost a few nanoseconds at one word, so `bit_backend/popcount/words=1` and its
siblings are where a 5 % band bites first. That is the intent: design §8 asks
for the cost to be bounded by measurement rather than by argument, and a
tolerance that those cells could not trip would not bound it.

## 5. Comparison rule

Per cell, the statistic is **pooled** nanoseconds per call: the sum of
`elapsed_ns` over every recorded window of that cell divided by the sum of
`calls`. It is never a mean of the per-row `ns_per_call` values, which would
weight a short window equally with a long one.

The comparison is:

```sh
cargo +1.95.0 bench -p gf2-core --features simd \
  --bench selector_non_regression -- \
  --compare dev/benchmarks/tuning_profiles/2026-08-19-procedure-verification-cohort-a.csv \
  --against dev/benchmarks/tuning_profiles/2026-08-19-procedure-verification-cohort-b.csv
```

Every path argument, `--output` included, is taken as repository-relative unless
it is absolute, so the command above runs from the repository root even though
`cargo bench` gives the binary a working directory at the package root.

It refuses to compare unless both receipts carry schema
`selector-non-regression-v1`, their cell sets are identical, that set is the
pinned set of §2, and every cell's recorded identity — arm, family, and operand
sizes — is equal in the two receipts. The arm precondition follows from the
cutover's contract of default behaviour identical by construction: both receipts
are taken with no profile installed, so a cell that resolves to a different arm
is two different code paths, and a ratio between them measures no regression at
all. That is a distinct failure class from a tolerance failure, and the run
reports it as `RESULT: FAIL (selector identity mismatch)`, naming each disagreeing
cell with both arms, without printing any per-cell verdict or geometric mean.

Otherwise it prints one line per cell with both pooled rates, the ratio, and a
verdict, then the geometric mean and a final `RESULT: PASS` or `RESULT: FAIL`.
It exits non-zero on every failure, of either class.

## 6. What a receipt records

A receipt is a Markdown file beside its raw CSV in this directory, carrying the
provenance rows of the repository's receipt convention: harness and schema
token, clean source revision and dirty flag, bench binary SHA-256, toolchain,
host and CPU model, OS/kernel and governor, lock wrapper and affinity, the
timing protocol constants, the measured wall-clock window, and the raw file's
SHA-256.

A receipt that gates a cutover requires `source_dirty=false` in every row. The
`nice -n -5` the wrapper attempts is best-effort and is denied to a non-root
user; a receipt records whether it took effect, and the lock and affinity remain
in force either way.

## 7. Falsification and re-pinning

A cell whose own dispersion within a receipt exceeds τ_cell is recorded as
noise-dominated in that receipt, together with its numbers. The tolerance is not
widened to accommodate it, and the cell is not dropped: per
`@/inv/falsification-preserved`, measurement that contradicts this plan's noise
assumption is recorded with the contradiction.

Moving a conservative default re-pins the set. The defaults fix the bracket
sizes of §2, so a changed default leaves the set no longer straddling it; the
harness's `pinned_sizes_bracket_each_default_at_adjacent_guard_values` test fails
in that case, and the set is re-pinned and the schema token bumped rather than
compared across the change. Receipts taken under different schema tokens do not
compare, and the comparison mode refuses them.
