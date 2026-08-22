# Sixth post-cutover receipt for the pinned selector non-regression set

This is the sixth post-cutover receipt of issue `50b47eae`, the second taken
under the content-independent translation ensemble of
[`layout-attribution-verdict-v4.md`](layout-attribution-verdict-v4.md) (owner
decision DEC-K) and the first taken after the `eb9b324c` rework of the
`mul_fast` dispatch path that receipt-5's attributable FAIL tracked (owner
decision DEC-L; the measured session is owner decision DEC-N): K = 128
members per arm, axis E alone, the bit-parity (Thue–Morse) half assignment
phase-shifted by each arm's measured base phase φ, and v4 C3's cross-arm
distributional-equality precondition verified from the build ledgers before
any timed window. Each side of the plan's §5 comparison is an ensemble of 128
builds of one revision, every member a pure 32-byte translation of that arm's
ordinary build.

The procedure runs as
[`selector-non-regression-plan-v1.md`](selector-non-regression-plan-v1.md);
[`layout-attribution-verdict-v1.md`](layout-attribution-verdict-v1.md), v2
(DEC-I), v3 (DEC-J) and v4 (DEC-K) together fix it. The pinned cell set of
plan §2, the tolerance of plan §4, the comparison rule of plan §5, the run
protocol of plan §3 and the schema token `selector-non-regression-v1` are
unchanged; τ_cell is 5 % and τ_set is 2 % at their predeclared values, and
the attribution margin stays three standard errors. No tuning profile is
installed anywhere in this session, and no build of it sets
`gf2_tuning_baked`.

The six standing receipts —
[`2026-08-19-pre-cutover-baseline.md`](2026-08-19-pre-cutover-baseline.md),
[`2026-08-20-post-cutover-receipt.md`](2026-08-20-post-cutover-receipt.md),
[`2026-08-20-post-cutover-receipt-2.md`](2026-08-20-post-cutover-receipt-2.md),
[`2026-08-20-post-cutover-receipt-3.md`](2026-08-20-post-cutover-receipt-3.md),
[`2026-08-22-post-cutover-receipt-4.md`](2026-08-22-post-cutover-receipt-4.md)
and [`2026-08-22-post-cutover-receipt-5.md`](2026-08-22-post-cutover-receipt-5.md)
— and the pilot receipt of `9162956b` stand as taken. None is modified,
superseded, re-run or adjusted by this receipt.

## Result

**The session establishes an attributable verdict, and it is PASS.** The
attribution audit reports `RESULT: PASS` on all four preconditions — the
third consecutive session to attribute — and the verdict comparison reports
`RESULT: PASS`, so the first row of v1 §7's reading table applies:

> The cutover's cost at every pinned cell stands inside τ_cell and the set
> inside τ_set, attributably. `50b47eae` REQ-01 is met by that receipt.

- **Every cell is inside τ_cell.** The three highest ratios in the set are
  `polynomial/mul/len=32` (schoolbook) at 1.040742,
  `polynomial/mul/len=16` (schoolbook) at 1.037715 and
  `bit_backend/xor_inplace/words=8` (simd) at 1.024300; the other
  thirty-one cells read at or below 1.0029, twenty-one of them at or below
  1.
- **The set rule passes.** The geometric mean of the thirty-four ratios is
  0.994048, inside τ_set = 1.02.
- **Every attribution precondition holds.** Coverage clears its floor at all
  thirty-four cells (tightest 1.373 at
  `polynomial/div_rem_auto/dividend=4096/divisor=1024`, then 1.403 at
  `bit_backend/or_inplace/words=1` and 1.421 at
  `polynomial/mul_fast/len=65`) and on the RMS (0.036202 against 0.033878);
  precision clears its three-standard-error margin everywhere (narrowest
  5.072 at `bit_backend/xor_inplace/words=16`, widest 174.681 at
  `bit_backend/popcount/words=8`; set margin 18.971 in receipt-5 reads
  21.928 here at set `se` = 0.000903); the half-split null stays inside
  ±τ_cell at every cell (widest 1.011131 at `bit_backend/xor_inplace/words=16`)
  and reads 0.999316 on the geometric mean; decorrelation passes at 0.052177
  against 0.036202, a ratio of 1.441 against the √2 fully independent arms
  would give.
- **Receipt-5's two excursion cells are re-measured over the reworked code,
  and they are clean.** `polynomial/mul_fast/len=32` (mul_dispatch), FAIL at
  ρ = 1.058269 in receipt-5, reads **1.000966**; `polynomial/mul_fast/len=64`
  (mul_dispatch), FAIL at ρ = 1.050254, reads **0.999108**. The `eb9b324c`
  fix — one `mul_dispatch` inner dispatcher on a resolved threshold, a
  single `tuning::active()` read per call — is confirmed by the measurement
  it was made for, on the estimand that failed it: the placement-averaged
  mean over 128 translations per arm under verified-equal placement
  distributions.
- **The pilot-flagged relocation cells hold.** The `eb9b324c` pilot
  disclosed that translation sensitivity relocates toward
  `polynomial/mul/len=16` and `len=32`; read with that care, both cells
  stand inside tolerance — 1.037715 and 1.040742, which is 6.06 and 3.09 of
  their own layout standard errors *below* the τ_cell bar (and 19.1 and 13.9
  above 1, so both are real small costs, recorded, not noise). The
  Karatsuba neighbours read 0.999115 (`len=33`), 0.999157 (`len=64`) and
  0.997552 (`len=256`).

Per v1 §7's first row the verdict stands and `50b47eae` REQ-01 — a receipt
showing the predeclared tolerance holds for the pinned set — **is met by
this receipt**. Per plan §7 and control-arm §4.5 this session is run once and
stands as taken. No predeclared value moves for it.

## Post-cutover state

The bit-backend selection boundary is unchanged from receipts 4 and 5: the
DEC-G compile-time constant (`SIMD_MIN_WORDS_DEFAULT = 8`, no per-call
profile read), and `TuningProfile::install()` governs the polynomial family
only. **The compiled source of this session's candidate arm differs from
receipts 1–5 by exactly one build input**: restricting
`git diff --name-only 1a5812c2..ec8bc66b` to each build input in turn —
`crates/`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/`,
`build.rs`, `scripts/` — reports exactly one changed file,
`crates/gf2-core/src/field/poly.rs`, the `eb9b324c` fix (commits `dd3eee59`
and `619a9666`); every other file in the range is `dev/` documentation,
session records or `.jit/` state, which no build reads. The reference arm's
ordinary build is bit-identical to the ordinary reference build of receipts
1–5 and the pilot (`c7be7a87…`); the candidate arm's ordinary build is a new
binary (`a6cb9172…`), as a source change requires.

## Reproducible protocol and provenance

Every value below is observed during this run. The two arms share one host
state, one lock, one affinity mask and one wrapper invocation; the rows that
differ between them are marked.

| Item | Value |
|---|---|
| Harness and checker | `crates/gf2-core/benches/selector_non_regression.rs`; schema `selector-non-regression-v1`; full-worktree `git status --porcelain --untracked-files=all` |
| Procedure | plan v1, as amended by verdict v1, v2 (DEC-I), v3 (DEC-J) and v4 (DEC-K), executed as committed; session approved as DEC-N |
| Ensemble | K = 128 members per v4 C2, axis E alone; `--execution j+1` names member `j`; every member built on its first attempt, so neither v2 §A8's retry nor v1 §3.5's replacement was exercised |
| Arm phases | φ = 0 in **both** arms this session: reference ordinary `.text` at `0x1cbc0` (P = 3008, P mod 64 = 0) and candidate ordinary `.text` at `0x1e840` (P = 2112, P mod 64 = 0) — the `eb9b324c` source change moved the candidate base from receipt-5's `0x1e8a0`; each φ measured from the arm's probe build before enumeration and recorded in `phi-{ref,cand}.txt` |
| Source revision, candidate arm | `ec8bc66b82d07a06a46fb024e52c49446a75286e` — the build revision, the run revision and the revision every row records; build and run revisions coincide in this session (see [The candidate revision](#the-candidate-revision)); every row records `source_dirty=false` |
| Source revision, reference arm | `0c072d73ca65cf50af98b8c4b61ed876f8218df6`, the revision every row of the baseline receipt records; every row records `source_dirty=false` |
| Reference-arm checkout | `.agents/worktrees/control-0c072d73`, detached at `0c072d73` and clean at the build phase, the session start and the session end |
| Reference member 0 | SHA-256 `c7be7a87d7a01d3c2e297919033a99237afe8939f3fce147802ecd86df32c710`, bit-identical to the ordinary reference build of receipts 1–5 and the pilot |
| Candidate ordinary build | Member j = 0 (E = 0, `--execution 1` — φ = 0 this session), SHA-256 `a6cb91728e6d74ace69f25567d0b61ee47faebcae51b679010e51cffa2f85a39` |
| Bench binaries | 256, one per member per arm, each built in a scratch target directory outside the checkout (v2 §A8), staged to `/tmp/gf2-ens6/{ref,cand}/` and hashed before the next member was built; the scratch directory was deleted between members |
| Construction verification | v4 C2/C3 from the build ledgers, before any timed window, all three `RESULT: PASS`: reference arm P = 3008, candidate arm P = 2112, both ≡ 0 (mod 32); each arm realizes 128 distinct binaries at 128 distinct page offsets — every multiple of 32 in the page — with no whole-page displacement; each half holds 32 members at each of the two realized cache-line offsets {0, 32} and covers all 64 L1i sets exactly once; cross-arm, the page-offset sets, cache-line-offset multisets and L1i set-index multisets are equal |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)`, built and run as `cargo +1.95.0`, both arms |
| Features | `simd`, both arms; no tuning profile installed; `gf2_tuning_baked` not set |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor |
| OS/kernel and governor | `Linux 7.1.8-arch1-3 #1 SMP PREEMPT_DYNAMIC Tue, 11 Aug 2026 09:16:08 +0000 x86_64 GNU/Linux`; `powersave` on CPUs 6–11 |
| Lock and affinity | `dev/scripts/ccx1-bench-flock.sh`, one invocation holding `/tmp/gf2-ccx1.lock` (stat `dev=2d inode=39210`) for all 256 executions; CPUs 6–11, observed as `Cpus_allowed_list: 6-11` inside the wrapper. The session log records the held lock directly: `FLOCK ADVISORY WRITE 2415486 00:2d:39210` |
| Niceness | The wrapper's best-effort `nice -n -5` is denied; the child runs at niceness 5, as receipts 3–5 record |
| Host quiescence | The driver's idle gate held the timed phase until the one-minute load fell below 0.5 (three one-minute waits, 5.48 → 2.01 → 0.74, then 0.27); one-minute load 0.27 as the wrapper is invoked and 1.00 at its end; no builds, benches or campaign processes ran on the host during the timed phase |
| Timed work | One execution of one repetition per build, `--target-ms 250`, 256 executions in total (v4 C5, v1 §5.1), each preceded by the plan's equivalence probes |
| Build window | 2026-08-22 16:39:44–17:40:15 UTC (reference 16:39:44–17:10:35, candidate 17:10:35–17:40:15), no lock held, no compilation under the lock; 1,831 s + 1,759 s of member compiler wall-clock at 13–15 s per member |
| Measured duration | 2026-08-22 17:43:15–18:30:00 UTC (46 min 45 s), the wrapper's whole child lifetime, covering both arms; consecutive executions 10–11 s apart, mean 10.96 s, against v4 C5's ~11 s cadence |
| Raw file, reference arm | [`2026-08-22-ensemble-reference-arm-6.csv`](2026-08-22-ensemble-reference-arm-6.csv), SHA-256 `ed00b6f720214f7294c760d23ac736155f33d453dc8ed151efe69e8625619a07` |
| Raw file, candidate arm | [`2026-08-22-ensemble-candidate-arm-6.csv`](2026-08-22-ensemble-candidate-arm-6.csv), SHA-256 `af5d8090dd4e059125970482b436caf8385d1cdc44f06e9cb7fcf11acbd05946` |
| Member provenance | [`2026-08-22-member-provenance-ref-6.tsv`](2026-08-22-member-provenance-ref-6.tsv) (`034b8d7f…`), [`2026-08-22-member-provenance-cand-6.tsv`](2026-08-22-member-provenance-cand-6.tsv) (`49f1922e…`) — every member's `RUSTFLAGS`, binary SHA-256, `.text` address and page offset |
| Session record | `dev/active/50b47eae/s6-session/` — driver log, timed log, both build logs, both ledgers, both φ records, every script (`continue-driver.sh`, `build-arm.sh`, `gen-members.py`, `verify-construction.py`, `run-ensemble.sh`, `emit-tsv.sh`), `slots.tsv`, `members-{ref,cand}.tsv`, `comparison.txt`, `layout-audit.txt`, `single-build-comparison.txt`, `list-cells-{ref,cand}.txt`, `sha256-manifest.txt` |

### The candidate revision

**Build and run revisions coincide in this session.** The candidate binaries
were built at `ec8bc66b82d07a06a46fb024e52c49446a75286e` (the session-8
handoff commit) in a clean checkout (build log: `porcelain_lines=0`), the
timed phase ran with `HEAD` at the same commit, nothing committed anywhere in
the repository between the build phase and the session end (the timed log
records both checkouts' `HEAD` and empty porcelain status at the wrapper's
start and end), and every candidate row records `ec8bc66b…` with
`source_dirty=false`. The revision-ruling machinery of receipt-3 (the epic's
escalation #18) is therefore exercised trivially: there is no build/run
revision split to rule on.

What the ruling's diff treatment establishes here is the identity of the
change under test: `git diff --name-only 1a5812c2..ec8bc66b` (receipt-5's
candidate revision to this one) restricted to the build inputs — `crates/`,
`Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/`, `build.rs`,
`scripts/` — reports exactly one file, `crates/gf2-core/src/field/poly.rs`:
the `eb9b324c` rework, and nothing else. Every other file in the range is
under `dev/` (receipt-5, session-5's record, `eb9b324c`'s findings, perf and
pilot records, handoffs, the progress ledger) or `.jit/`, which no build
reads. This receipt measures the fix and only the fix against the unchanged
reference.

Each arm records 4,352 rows: thirty-four cells × 128 builds × one
repetition, execution indices 1 through 128 with none missing, 34 rows per
execution. Every cell's equivalence probe passed in every one of the 256
executions — a failed probe aborts the run, and the driver ran to
completion.

The raw files are written to `/tmp/gf2-50b47eae-s6-reference-arm.csv` and
`/tmp/gf2-50b47eae-s6-candidate-arm.csv`, both checked absent beforehand, and
copied into this directory afterwards with the SHA-256 above recorded in the
session manifest. No in-repository file is created while the run is in
flight.

## The ensemble

Member `j` (0…127) takes the translation axis E alone, per v4 C2: the
member's `.note.gnu.build-id` payload is lengthened to `20 + 32·E(j)` bytes
(`-C link-arg=-Wl,--build-id=0x<2·(20+32·E) zeros>`), translating the loaded
image by `32·E(j)` bytes; E = 0 omits the option, so that member is the
arm's ordinary build. `E(j)` interleaves the two Thue–Morse halves of
`(E + φ) mod 128`: even `j` walks the popcount-even values ascending, odd
`j` the popcount-odd values ascending, with φ the arm's measured base phase.
**Both arms measured φ = 0 this session**, so each arm's ordinary build is
member j = 0 (`--execution 1`) — unlike session 5, whose candidate arm had
φ = 1. Every extraction below selects the ledger row with E = 0, the rule
that is invariant across the two sessions.

**All 256 members built on the first attempt** (both ledgers record
`attempts=1` throughout) and **all 256 binaries are distinct** — 128
distinct SHA-256 per arm.

**The realized placements.** Both arms realize v4 C2's balance exactly as
enumerated, verified from the ledgers before any timed window: 128 distinct
`.text` page offsets per arm — every multiple of 32 in the page, both bases
≡ 0 (mod 32) — no whole-page displacement in either arm, two realized
cache-line offsets {0, 32} at 32 members per half each, each member-parity
half covering all 64 L1i sets exactly once, and the two halves' offset and
set multisets equal. The full verifier output for both arms is in the driver
log in the session record.

Binary sizes span 726,808–730,872 bytes (+0.56 %) in the reference arm and
792,560–796,624 bytes (+0.51 %) in the candidate arm: the translation-only
ensemble varies placement, not content.

### The cross-arm precondition (v4 C3)

The two arms are verified to sample the same placement distribution before
any timed window. All four checks pass, from the ledgers:

```
[ok] P_ref == P_cand (mod 32): ref%32=0 cand%32=0
[ok] page-offset sets equal
[ok] cache-line offset multisets equal: ref={0: 64, 32: 64} cand={0: 64, 32: 64}
[ok] L1i set-index multisets equal
RESULT: PASS
```

What the verdict ratio averages over is one placement distribution, realized
identically in both arms.

### Member provenance

Every member's `RUSTFLAGS`, the SHA-256 of the binary it produced, its
`.text` virtual address and its page offset, for both arms, are the two
committed provenance TSVs named above, generated from the build ledgers the
session record preserves.

## Arm ordering

Member `j` runs its two arms adjacently, reference first when `j` is even
and candidate first when `j` is odd, per v1 §5.2. The session driver
implements exactly that: over the 256 slots there is no violation of the
rule, each arm holds exactly 64 of the first-of-pair positions, and the mean
slot position of the two arms is equal at 128.5. The slot-by-slot record is
`slots.tsv` in the session record. Consecutive executions start 10 s to 11 s
apart, mean 10.96 s.

## Step 0 — harness against the plan

`--self-check` prints the same line from the staged ordinary member binary of
each arm (member 0 in both arms this session), run with its working
directory at its own checkout:

```
protocol: schema=selector-non-regression-v1 cells=34 repetitions=5 target_ms=250 per_cell_tolerance=0.050000 set_tolerance=0.020000 simd_min_words=8
self-check PASS
```

`per_cell_tolerance=0.050000` and `set_tolerance=0.020000` equal the plan's
§4 values, and `cells=34` equals its §2 count. `simd_min_words=8` is the
value both checkouts resolve. `--list-cells` prints thirty-four rows in each
checkout and the two listings are byte-identical
(`list-cells-{ref,cand}.txt` in the session record), reproducing the plan's
§2 table cell for cell with the arm it predicts for each. Both checks ran
from the staged binaries **after** the timed phase rather than before it —
recorded in Deviations, as in receipt-5; the binaries checked are the
binaries that produced the rows, hash-verified against the ledgers.

## Step 1 — commands

The build phase runs first and holds no lock. Each arm first builds its
ordinary probe to measure P and φ and generate its member enumeration
(`gen-members.py`), then builds each member by putting its `RUSTFLAGS` in
the environment and reading the produced binary from Cargo's own JSON
(`build-arm.sh`):

```sh
RUSTFLAGS="<member flags>" CARGO_TARGET_DIR=/tmp/gf2-ens6/target-<arm> \
  cargo +1.95.0 bench -p gf2-core --features simd \
  --bench selector_non_regression --no-run --message-format=json |
  jq -r 'select(.executable != null and .target.name == "selector_non_regression") | .executable'
```

The E = 0 member takes no options and is built with `RUSTFLAGS` unset rather
than empty. The binary is copied to `/tmp/gf2-ens6/<arm>/member-<j>.bin` and
hashed before the next member is built; the scratch target directory is
deleted between members (v2 §A8), so every binary is a full fresh build.

The timed phase is one wrapper invocation over all 256 executions
(`continue-driver.sh` step 4):

```sh
GF2_BENCH=1 ./dev/scripts/ccx1-bench-flock.sh bash /tmp/gf2-ens6/run-ensemble.sh \
  /tmp/gf2-50b47eae-s6-reference-arm.csv /tmp/gf2-50b47eae-s6-candidate-arm.csv
```

`run-ensemble.sh` records the host facts this receipt cites, then runs the
256 executions in v1 §5.2's order, each staged binary with its working
directory at its own checkout, `--output` to an absolute `/tmp` path checked
absent beforehand. The guard receipt-3 introduced is kept: before every
execution the driver re-reads both checkouts' `HEAD` and full-worktree
porcelain status and aborts the session if either has moved. It took no
action.

## Comparison of the two arms — the verdict

This is the comparison v1 §6.2 defines and the epic gates on: the reference
arm against the candidate arm, on pooled ns/call per cell, judged by
τ_cell = 5 % and τ_set = 2 %, computed by the plan's §5 mode unmodified.

```sh
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --compare dev/benchmarks/tuning_profiles/2026-08-22-ensemble-reference-arm-6.csv \
  --against dev/benchmarks/tuning_profiles/2026-08-22-ensemble-candidate-arm-6.csv
```

The run prints all thirty-four per-cell lines and a geometric mean, so the
§5 preconditions hold: both files carry schema `selector-non-regression-v1`,
their cell sets are identical and equal to the pinned set, and every cell's
arm, family and operand sizes agree. The full output is
[`comparison.txt`](/dev/active/50b47eae/s6-session/comparison.txt) in the
session record; every cell reads `PASS`:

```
polynomial/mul_fast/len=32 mul_dispatch 1188.052030 1189.199567 1.000966 PASS
polynomial/mul_fast/len=64 mul_dispatch 3802.990227 3799.597687 0.999108 PASS
...
geometric_mean 0.994048 PASS
RESULT: PASS
```

The comparison exits 0. The cells nearest the bar, with their distance from
it in layout standard errors:

| Cell | Arm | Reference ns/call | Candidate ns/call | ρ(c) | Below τ_cell by | ρ(c) in layout standard errors | Below the bar in standard errors |
|---|---|---:|---:|---:|---:|---:|---:|
| `polynomial/mul/len=32` | schoolbook | 1,232.840426 | 1,283.069240 | 1.040742 | −0.009258 | 13.94 | 3.09 |
| `polynomial/mul/len=16` | schoolbook | 319.111621 | 331.146971 | 1.037715 | −0.012285 | 19.06 | 6.06 |
| `bit_backend/xor_inplace/words=8` | simd | 3.750334 | 3.841466 | 1.024300 | −0.025700 | 8.93 | 9.22 |

The seventh column is `ln ρ(c) / se(c)` — how far the measured ratio stands
from 1 in units of the layout standard error the audit computes for that
cell — and the eighth is the same distance measured from the tolerance. All
three are real small costs, many standard errors above 1, and all three
stand clear of the bar.

## The attribution audit

v1 §6.5's `--layout-audit` mode computes §6.4's four preconditions. The
receipt pair naming the two committed receipts whose ratio defines `σ̂` is
mandatory, and it is supplied:

```sh
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --layout-audit dev/benchmarks/tuning_profiles/2026-08-22-ensemble-reference-arm-6.csv \
  --layout-candidate dev/benchmarks/tuning_profiles/2026-08-22-ensemble-candidate-arm-6.csv \
  --compare dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv \
  --against dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-control-arm-2.csv
```

The mode audits rather than refusing: one revision per arm, no
`source_dirty=true` row, no execution index below one, 128 members per arm
(one of the sizes `ENSEMBLE_SIZES` accepts), halves balanced on member
parity, the same member indices in both arms, and the receipt pair naming
the pinned `σ̂` sources. The full output is
[`layout-audit.txt`](/dev/active/50b47eae/s6-session/layout-audit.txt); its
last four lines:

```
set layout_se=0.000903 margin=21.928 null_geomean=0.999316 PASS
coverage ensemble_rms=0.036202 natural_rms=0.033878 PASS
decorrelation paired_rms=0.052177 reference_rms=0.036202 PASS
RESULT: PASS
```

The audit exits 0.

### The four preconditions

| Precondition | What §6.4 requires | Measured | Verdict |
|---|---|---|---|
| **Coverage, per cell** | `s_R(c) ≥ σ̂(c)/2` at every cell | Every cell clears its floor. The tightest are `polynomial/div_rem_auto/dividend=4096/divisor=1024` at 1.373×, `bit_backend/or_inplace/words=1` at 1.403× and `polynomial/mul_fast/len=65` at 1.421× | PASS |
| **Coverage, whole set** | RMS of `s_R` ≥ RMS of `σ̂` | 0.036202 against 0.033878, a ratio of 1.069 | PASS |
| **Decorrelation** | RMS of `p` ≥ RMS of `s_R` | 0.052177 against 0.036202, a ratio of 1.441 against the 1.414 fully independent arms would give | PASS |
| **Precision, per cell** | `ln(1.05) ≥ 3 · se(c)` at every cell | Every cell clears it; the narrowest margin is 5.072 at `bit_backend/xor_inplace/words=16` and the widest 174.681 at `bit_backend/popcount/words=8` | PASS |
| **Precision, whole set** | `ln(1.02) ≥ 3 ·` set-level `se` | Set `se` = 0.000903, margin 21.928 | PASS |
| **Half-split null, per cell** | Inside ±τ_cell at every cell, read two-sided | Widest is 1.011131 at `bit_backend/xor_inplace/words=16`; every cell inside ±1.2 % | PASS |
| **Half-split null, whole set** | Inside ±τ_set on the geometric mean | 0.999316 | PASS |

This is the third consecutive session to attribute, and the second under an
ensemble whose cross-arm distribution is verified rather than assumed. The
verdict it attributes is the comparison above.

### Which §7 reading applies

The verdict is `PASS` and every precondition holds, so v1 §7's first row
governs:

> | PASS | all hold | The cutover's cost at every pinned cell stands inside
> τ_cell and the set inside τ_set, attributably. `50b47eae` REQ-01 is met by
> that receipt. |

Neither §6.6 branch is taken — no precondition failed — and no excursion
exists to route. `50b47eae` REQ-01 is met by this session.

## §7.1 — the recorded single-build comparison

v1 §7.1 fixes this reading in advance: the plan's §5 comparison of the
committed 2026-08-19 baseline against the candidate arm's ordinary build
alone — the E = 0 member, built under unset `RUSTFLAGS`, **member j = 0,
`--execution 1`** this session (φ = 0), binary SHA-256 `a6cb9172…` —
recorded with its per-cell lines and its `RESULT:`. **It carries no verdict
standing.**

Its rows are the candidate arm's `execution=1` rows — selected by E = 0
from the ledger, per v4 C2's enumeration — extracted from the committed arm
file to an absolute `/tmp` path so that no in-repository file is created:

```sh
OUT=/tmp/gf2-ens6/candidate-member-E0-ordinary.csv
head -1 dev/benchmarks/tuning_profiles/2026-08-22-ensemble-candidate-arm-6.csv > "$OUT"
awk -F, 'NR>1 && $2==1' dev/benchmarks/tuning_profiles/2026-08-22-ensemble-candidate-arm-6.csv >> "$OUT"
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --compare dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv \
  --against "$OUT"
```

It reads `RESULT: FAIL` with geometric mean 0.991066, failing at one cell:
`bit_backend/xor_inplace/words=16` at 1.053883. Its full output is
`single-build-comparison.txt` in the session record.

**The two comparisons disagree at one of the thirty-four cells**, and §7.1
directs that every disagreeing cell be recorded with `s_R` and `σ̂`, "as the
measurement of how much one draw moved the old verdict":

| Cell | Single-build ratio | Single-build | Ensemble ρ(c) | Ensemble | `s_R` | `σ̂` |
|---|---:|---|---:|---|---:|---:|
| `bit_backend/xor_inplace/words=16` | 1.053883 | FAIL | 0.993789 | PASS | 0.065510 | 0.007439 |

One draw of the ordinary layout manufactures a 5.4 % excursion at a cell
whose across-build dispersion is 6.6 % — the placement-averaged comparison
reads the same cell 0.6 % *below* the baseline. The two verdict-history
cells of receipt-5 read 0.998525 and 0.994615 in this draw, agreeing with
the ensemble's 1.000966 and 0.999108 on the reworked code. Receipt-4's
verdict cell continues its own §7.1 history: `bit_backend/not_inplace/words=1`
reads 0.923209 in this draw — the sixth consecutive ordinary-build draw of
the record well below 1 at that cell — while the ensemble reads it 0.959698.

## §7.2 — the candidate arm's own across-build dispersion

v1 §7.2 records this and it enters no verdict: `s_C(c)` materially above
`s_R(c)` says the cutover made that cell's cost more layout-sensitive. Nine
cells stand at or above 1.5× their reference-arm dispersion under pure
32-byte translation:

| Cell | Arm | `s_R` | `s_C` | `s_C`/`s_R` |
|---|---|---:|---:|---:|
| `polynomial/batch_evaluate_auto/coeffs=2048/points=2048` | horner | 0.000929 | 0.003650 | 3.930 |
| `polynomial/mul/len=33` | karatsuba | 0.002543 | 0.008999 | 3.539 |
| `polynomial/mul/len=32` | schoolbook | 0.009934 | 0.030862 | 3.107 |
| `polynomial/batch_evaluate/coeffs=2048/points=2048` | horner | 0.001255 | 0.003837 | 3.058 |
| `bit_backend/popcount/words=8` | simd | 0.001017 | 0.002992 | 2.942 |
| `bit_backend/or_inplace/words=8` | simd | 0.021622 | 0.053904 | 2.493 |
| `bit_backend/xor_inplace/words=8` | simd | 0.011362 | 0.028225 | 2.484 |
| `polynomial/mul_fast/len=32` | mul_dispatch | 0.001887 | 0.004001 | 2.120 |
| `polynomial/mul/len=16` | schoolbook | 0.010701 | 0.019192 | 1.794 |

Four of the nine (`batch_evaluate` ×2, `popcount/words=8`,
`mul_fast/len=32`) are ratios of dispersions each below 0.5 % — cells
nearly translation-insensitive in both arms. The substantive rows are the
schoolbook `mul` cells and `mul/len=33`, at 1–3 % candidate-side
dispersion: the residue of the relocation the `eb9b324c` pilot disclosed,
inside tolerance at every affected cell (their ratios are the two
highest-but-passing of the verdict, 1.0407 and 1.0377, and 1.019 at
`len=33` in receipt-5's session against 0.999 here).

**Receipt-5's two verdict cells are no longer outliers.** In session 5 they
stood at `s_C`/`s_R` of 6.514 (`mul_fast/len=32`, `s_C` = 0.039938) and
9.049 (`mul_fast/len=64`, `s_C` = 0.040936) — the top of the table after
one 14× cell. Here `mul_fast/len=32` reads `s_C` = 0.004001 (2.120×, a
tenth of session 5's dispersion) and `mul_fast/len=64` reads
`s_C` = 0.005178 at 1.081× — indistinguishable from the reference. The
`eb9b324c` fix removed the translation sensitivity itself, not only the
ordinary arrangement's mean. This is recorded evidence about the change. It
enters no verdict.

## §7.3 — cells whose own dispersion exceeds τ_cell

Plan §7 and v1 §7.3 direct that a cell whose own dispersion inside a receipt
exceeds τ_cell be recorded as noise-dominated with its numbers, the
tolerance not widened, the cell not dropped. Under this ensemble a cell's
own dispersion is its across-build dispersion — the quantity the ensemble
deliberately injects. Six cells in the reference arm and four in the
candidate arm exceed 5 % on the sample coefficient of variation of their 128
per-build rates — seven cells in one arm or the other:

| Cell | Reference arm CV | Candidate arm CV |
|---|---:|---:|
| `bit_backend/not_inplace/words=1` | 9.250 % | 4.529 % |
| `bit_backend/not_inplace/words=8` | 11.918 % | 0.361 % |
| `bit_backend/or_inplace/words=1` | 8.243 % | 6.048 % |
| `bit_backend/or_inplace/words=8` | 2.171 % | 6.910 % |
| `bit_backend/xor_inplace/words=1` | 7.087 % | 4.856 % |
| `bit_backend/xor_inplace/words=16` | 7.916 % | 10.604 % |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | 5.756 % | 5.766 % |

The tolerance is not widened for any of them and no cell is dropped. The
pooled verdict statistic carries this dispersion divided by √128: the
precision precondition measures the consequence and clears it at every cell,
5.072 at the narrowest. Neither of the two highest-ratio cells
(`polynomial/mul/len=16`, `len=32`) is in this table: their candidate-arm
CVs are 1.920 % and 3.144 %, and their near-bar readings are means, not
noise.

## The cells receipt-5 failed

Both are the `mul_fast` dispatch cells below the NTT threshold —
`polynomial/mul_fast/len=32` and `len=64`, arm `mul_dispatch`, the cells
that measure `mul_fast`'s dispatch to the schoolbook and Karatsuba paths,
reworked by `eb9b324c` between receipt-5 and this session. Their history
across the record:

| Session | Comparison | `len=32` | `len=64` | Audit |
|---|---|---:|---:|---|
| Receipt 1 | single builds | 1.002877 | 0.996980 | — |
| Receipt 2 | single builds | 1.077040 FAIL | 1.073771 FAIL | — |
| Receipt 3 | K = 128, alignment axes | 1.006002 | 1.005233 | FAIL |
| Receipt 4 | K = 256, E+G axes | 1.031653 | 1.024143 | PASS |
| Receipt 5 | K = 128, translation only | 1.058269 FAIL | 1.050254 FAIL | PASS |
| **This receipt** | **K = 128, translation only, post-`eb9b324c`** | **1.000966 PASS** | **0.999108 PASS** | **PASS** |
| This receipt §7.1 | single draw | 0.998525 | 0.994615 | — |

What this session fixes about the two cells: under the same estimand that
failed them — the placement-averaged mean over 128 translations per arm,
cross-arm distribution verified equal — the reworked dispatch path reads
them at 0.10 % and −0.09 %, with candidate-side translation dispersion at
0.40 % and 0.52 % against session 5's 3.99 % and 4.09 %. The ensemble
reading, the single-draw reading and the dispersion all now agree across
the cutover.

## Falsification record

### The single-build draw contradicts the ensemble at one cell

The committed baseline draw against this session's ordinary candidate build
reads 1.053883 at `bit_backend/xor_inplace/words=16` — over τ_cell — while
the ensemble reads that cell 0.993789, and the draw's whole-set result is
`RESULT: FAIL` where the session's verdict is `RESULT: PASS`. Both are
recorded; §7.1 predeclares that the single draw carries no verdict standing,
and the cell's 6.6 % reference-arm translation dispersion (§7.3) is the
measured reason a single draw cannot read it. The contradiction is the same
class receipt-4 and receipt-5 recorded at their §7.1 draws, in the
direction that manufactures an excursion rather than hiding one.

### The near-bar cells are real costs, recorded

`polynomial/mul/len=32` at 1.040742 and `len=16` at 1.037715 stand 13.9 and
19.1 layout standard errors above 1: genuine small regressions of the
schoolbook `mul` cells under the reworked dispatch's code placement, inside
the predeclared tolerance by 3.1 and 6.1 standard errors. They are the
relocation residue the `eb9b324c` pilot disclosed in advance, and they are
recorded here rather than smoothed away; a future session that finds them
above the bar has this receipt's numbers to compare against.

### Preserved

Per `@/inv/falsification-preserved` and control-arm §4.2, the disagreeing
§7.1 cell is recorded with both readings, `s_R` and `σ̂`; receipts 2 and 5's
FAILs at the `mul_fast` cells and receipt-4's FAIL at
`bit_backend/not_inplace/words=1` stand as taken and are cited above rather
than reconciled away. Per plan §7 and control-arm §4.5 this session is run
once and stands as taken.

## Disposition

The session establishes the record's third attributable verdict and the
record's **first PASS**: the pinned set's cost across the cutover, averaged
over 128 verified-equal translations per arm, stands inside τ_cell at every
cell and inside τ_set on the geometric mean, attributably, on the revision
carrying the `eb9b324c` rework.

- **`50b47eae` REQ-01 is met by this receipt**: the predeclared tolerance
  holds for the pinned set, and the receipt is committed with its raw
  artifacts and session record.
- **`50b47eae` REQ-02 is met by the record**: receipts 1–5 are preserved as
  taken with their contradictions; the two excursions that were attributable
  are each resolved by a tracked, closed issue (`fc976a80` — construction,
  no code change; `eb9b324c` — the `mul_fast` dispatch rework this session
  measures).
- The epic's REQ-04 — the pinned benchmark set with its predeclared
  non-regression tolerance passing before the reconciled capability traits
  became public API — is satisfied by this receipt on the measurement side.

What this session settles beyond the verdict:

- **The `eb9b324c` fix is effective on the estimand that demanded it.** The
  two failing cells read 1.001 and 0.999, and their translation sensitivity
  fell from 4.0 %/4.1 % to 0.40 %/0.52 % — reference-arm level.
- **The v4 machinery closes its arc.** Three consecutive attributable
  sessions: receipt-4 attributed a construction artifact and `fc976a80`
  diagnosed it; receipt-5 attributed a real code excursion and `eb9b324c`
  fixed it; this receipt attributes the fixed state and passes.
- **The relocation residue is bounded.** The pilot-flagged `mul/len=16,32`
  cells carry real 3.8–4.1 % costs, inside tolerance, recorded with their
  standard errors.

## What stands unchanged

- **The tolerance.** τ_cell stays 5 % and τ_set stays 2 %. The verdict is
  read against them exactly as predeclared.
- **The pinned set.** All thirty-four cells stand. No cell is added,
  dropped or re-bracketed.
- **The ensemble.** K stays 128 under v4 C2's enumeration and C3's
  cross-arm precondition. The margin of three standard errors stands.
- **The schema token.** `selector-non-regression-v1` is not bumped; the
  baseline, the five prior post-cutover receipts and both of this session's
  arms stay valid and comparable.
- **The standing receipts.** All six, and the pilot receipt, stand as
  taken. The baseline and the second receipt's control arm are read here
  only as the natural pair defining `σ̂`, the role v1 §8 assigns them.
- **This run.** It stands as taken.

## Deviations

Every departure from the procedure as written, however small, and what each
one does or does not touch.

- **`--self-check` and `--list-cells` ran after the timed phase, not before
  it.** The session driver carried no step-0 stanza; the checks were run
  during receipt authoring from the staged ordinary member binaries — the
  binaries that produced the rows, hash-verified against the ledgers — each
  with its working directory at its own checkout. Both pass, the two
  `--list-cells` listings are byte-identical, and the protocol constants
  equal the plan's; the ordering deviation changes no measurement and no
  row. Receipt-5 records the same deviation for the same reason.
- **The child runs at niceness 5.** The wrapper's best-effort `nice -n -5`
  is denied, exactly as receipts 3–5 record. The lock and affinity remain
  in force, the host carries no competing work, and both arms run at the
  same niceness inside one session, so the verdict ratio carries none of it.
- **Each arm builds one ordinary probe before its enumeration.** v4 C2
  fixes each arm's enumeration by the measured `.text` address of its
  ordinary build, so `build-arm.sh` builds it once to read P and φ
  (`ordinary-probe.bin`, `phi-{ref,cand}.txt`), then builds the 128 members
  from the generated table. The probe binaries are bit-identical to each
  arm's E = 0 member (`c7be7a87…`, `a6cb9172…`) — the reproducibility this
  relies on, verified by hash. No probe row enters any arm.
- **Builds ran in scratch target directories deleted between members**,
  per v2 §A8; each binary was staged and hashed before its target directory
  was removed. All 256 ledger rows record fresh first-attempt builds of
  13–15 s.
- **The §7.1 extraction writes to `/tmp`**, one `head` and one `awk` over
  the committed arm file, reproducible from it, selecting `execution=1` —
  the E = 0 member of this session's φ = 0 candidate arm.
- **The session driver and scripts are session 5's, re-pathed.** Every
  pipeline text is byte-equivalent to the committed
  `dev/active/50b47eae/s5-session/` scripts modulo the `ens5→ens6`,
  `s5→s6` and `-5→-6` path substitutions, verified by round-trip diff at
  launch; the `locks=` probe fix receipt-5 recorded is kept, and the session
  log carries the held-lock line (`FLOCK ADVISORY WRITE 2415486
  00:2d:39210`). All scripts and their hashes are in the session record and
  its manifest.
