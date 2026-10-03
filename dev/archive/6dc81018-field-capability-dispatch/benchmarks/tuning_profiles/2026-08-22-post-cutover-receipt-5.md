# Fifth post-cutover receipt for the pinned selector non-regression set

This is the fifth post-cutover receipt of issue `50b47eae`, the first taken
under the content-independent translation ensemble of
[`layout-attribution-verdict-v4.md`](layout-attribution-verdict-v4.md) (owner
decision DEC-K): K = 128 members per arm, axis E alone, the bit-parity
(Thue–Morse) half assignment phase-shifted by each arm's measured base phase
φ, and v4 C3's cross-arm distributional-equality precondition verified from
the build ledgers before any timed window. Each side of the plan's §5
comparison is an ensemble of 128 builds of one revision, every member a pure
32-byte translation of that arm's ordinary build.

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

The five standing receipts —
[`2026-08-19-pre-cutover-baseline.md`](2026-08-19-pre-cutover-baseline.md),
[`2026-08-20-post-cutover-receipt.md`](2026-08-20-post-cutover-receipt.md),
[`2026-08-20-post-cutover-receipt-2.md`](2026-08-20-post-cutover-receipt-2.md),
[`2026-08-20-post-cutover-receipt-3.md`](2026-08-20-post-cutover-receipt-3.md)
and [`2026-08-22-post-cutover-receipt-4.md`](2026-08-22-post-cutover-receipt-4.md)
— and the pilot receipt of `9162956b` stand as taken. None is modified,
superseded, re-run or adjusted by this receipt.

## Result

**The session establishes an attributable verdict, and it is FAIL.** The
attribution audit reports `RESULT: PASS` on all four preconditions — the
second consecutive session to attribute — and the verdict comparison reports
`RESULT: FAIL`, so the second row of v1 §7's reading table applies:

> The cutover's cost exceeds a tolerance, attributably. The verdict stands,
> the excursion is recorded with its contradiction, and its rework is tracked.

- **Exactly two cells exceed τ_cell**, both in the `mul_fast` family's
  dispatch arm: `polynomial/mul_fast/len=32` (mul_dispatch) reads
  ρ = 1.058269 — 1,258.54 ns/call in the candidate arm against 1,189.24 in
  the reference arm, +69.30 ns/call, 15.86 layout standard errors above 1 and
  2.20 above the tolerance itself — and `polynomial/mul_fast/len=64`
  (mul_dispatch) reads ρ = 1.050254 — 3,994.85 against 3,803.70,
  +191.15 ns/call, 13.47 layout standard errors above 1 and 0.066 above the
  tolerance: two hundredths of a percent over the bar. The other thirty-two
  cells pass, eighteen of them inside 1 % in either direction.
- **The set rule passes.** The geometric mean of the thirty-four ratios is
  1.001087, inside τ_set = 1.02 and the closest to 1 of the five sessions.
  Both rules must hold, so the comparison is `RESULT: FAIL`.
- **Every attribution precondition holds.** Coverage clears its floor at all
  thirty-four cells (tightest 1.402 at
  `polynomial/div_rem_auto/dividend=4096/divisor=1024`) and on the RMS
  (0.034359 against 0.033878); precision clears its three-standard-error
  margin everywhere (narrowest 4.602 at `bit_backend/xor_inplace/words=16`);
  the half-split null stays inside ±τ_cell at every cell (widest 0.993501)
  and reads 1.000059 on the geometric mean; decorrelation passes at 0.052963
  against 0.034359, a ratio of 1.541 against the √2 fully independent arms
  would give.
- **Receipt-4's excursion cell is arbitrated, and it is clean.**
  `bit_backend/not_inplace/words=1`, the record's first attributable FAIL at
  ρ = 1.100028 under the revision-dependent G axis, reads **1.000593** under
  the content-independent ensemble whose cross-arm precondition guarantees
  both arms sample one placement distribution. The `fc976a80` diagnosis —
  construction artifact, not code — is confirmed by the arbitration v4
  exists to run.

Per v1 §7's second row the verdict stands. `50b47eae` REQ-01 — a receipt
showing the tolerance holds — is not met by this receipt; per REQ-02 the
excursion is preserved here with its contradiction, and its disposition goes
to the epic's owner (Disposition below). Per plan §7 and control-arm §4.5
this session is run once and stands as taken. No predeclared value moves for
it.

## Post-cutover state

Unchanged from receipt-4, which describes it in full: the bit-backend
selection boundary is the DEC-G compile-time constant
(`SIMD_MIN_WORDS_DEFAULT = 8`, no per-call profile read), and
`TuningProfile::install()` governs the polynomial family only. **The compiled
source of this session's candidate arm is identical to receipt-4's**, stated
from the diff and witnessed by the binaries: restricting
`git diff ec857d2f..1a5812c2` to each build input in turn — `crates/`,
`Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/`, `build.rs`,
`scripts/` — reports no changed file for any of them (everything in that
range is `dev/` documentation, session records and `.jit/` state), and the
candidate arm's ordinary build is **bit-identical** to receipt-4's ordinary
candidate binary (SHA-256 `93ed5c45…`, built independently at the two
revisions). The reference arm's ordinary build is bit-identical to the
ordinary reference build of receipts 1–4 and the pilot (`c7be7a87…`).

## Reproducible protocol and provenance

Every value below is observed during this run. The two arms share one host
state, one lock, one affinity mask and one wrapper invocation; the rows that
differ between them are marked.

| Item | Value |
|---|---|
| Harness and checker | `crates/gf2-core/benches/selector_non_regression.rs`; schema `selector-non-regression-v1`; full-worktree `git status --porcelain --untracked-files=all` |
| Procedure | plan v1, as amended by verdict v1, v2 (DEC-I), v3 (DEC-J) and v4 (DEC-K), executed as committed |
| Ensemble | K = 128 members per v4 C2, axis E alone; `--execution j+1` names member `j`; every member built on its first attempt, so neither v2 §A8's retry nor v1 §3.5's replacement was exercised |
| Arm phases | φ = 0 reference (ordinary `.text` at `0x1cbc0`, P = 3008, P mod 64 = 0), φ = 1 candidate (`0x1e8a0`, P = 2208, P mod 64 = 32), each measured from the arm's probe build before enumeration and recorded in `phi-{ref,cand}.txt` |
| Source revision, candidate arm | `1a5812c269124128f64a4f9b5efc0f1e96f19894` — the build revision, the run revision and the revision every row records; build and run revisions coincide in this session (see [The candidate revision](#the-candidate-revision)); every row records `source_dirty=false` |
| Source revision, reference arm | `0c072d73ca65cf50af98b8c4b61ed876f8218df6`, the revision every row of the baseline receipt records; every row records `source_dirty=false` |
| Reference-arm checkout | `.agents/worktrees/control-0c072d73`, detached at `0c072d73` and clean at the build phase, the session start and the session end |
| Reference member 0 | SHA-256 `c7be7a87d7a01d3c2e297919033a99237afe8939f3fce147802ecd86df32c710`, bit-identical to the ordinary reference build of receipts 1–4 and the pilot |
| Candidate ordinary build | Member j = 1 (E = 0), SHA-256 `93ed5c457ed01e26e6b1b44e0723b980f5c979c2f0e4540e3454684f437869c0`, bit-identical to receipt-4's ordinary candidate build at `ec857d2f` |
| Bench binaries | 256, one per member per arm, each built in a scratch target directory outside the checkout (v2 §A8), staged to `/tmp/gf2-ens5/{ref,cand}/` and hashed before the next member was built; the scratch directory was deleted between members |
| Construction verification | v4 C2/C3 from the build ledgers, before any timed window, all three `RESULT: PASS`: reference arm P = 3008, candidate arm P = 2208, both ≡ 0 (mod 32); each arm realizes 128 distinct binaries at 128 distinct page offsets — every multiple of 32 in the page — with no whole-page displacement; each half holds 32 members at each of the two realized cache-line offsets {0, 32} and covers all 64 L1i sets exactly once; cross-arm, the page-offset sets, cache-line-offset multisets and L1i set-index multisets are equal |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)`, built and run as `cargo +1.95.0`, both arms |
| Features | `simd`, both arms; no tuning profile installed; `gf2_tuning_baked` not set |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor |
| OS/kernel and governor | `Linux 7.1.8-arch1-3 #1 SMP PREEMPT_DYNAMIC Tue, 11 Aug 2026 09:16:08 +0000 x86_64 GNU/Linux`; `powersave` on CPUs 6–11 |
| Lock and affinity | `dev/scripts/ccx1-bench-flock.sh`, one invocation holding `/tmp/gf2-ccx1.lock` (stat `dev=2d inode=39210`) for all 256 executions; CPUs 6–11, observed as `Cpus_allowed_list: 6-11` inside the wrapper. The session log records the held lock directly: `FLOCK ADVISORY WRITE 1579347 00:2d:39210` — the decimal-inode probe fix receipt-4's Deviations called for |
| Niceness | The wrapper's best-effort `nice -n -5` is denied; the child runs at niceness 5, as receipts 3 and 4 record |
| Host quiescence | The driver's idle gate held the timed phase until the one-minute load fell below 0.5 (three one-minute polls, 3.90 → 0.19); one-minute load 0.19 as the wrapper is invoked and 1.03 at its end; no builds, benches or campaign processes ran on the host during the timed phase |
| Timed work | One execution of one repetition per build, `--target-ms 250`, 256 executions in total (v4 C5, v1 §5.1), each preceded by the plan's equivalence probes |
| Build window | 2026-08-22 11:02:14–12:02:52 UTC (reference 11:02:14–11:33:10, candidate 11:33:10–12:02:52), no lock held, no compilation under the lock; 1,839 s + 1,764 s of member compiler wall-clock at 13–15 s per member |
| Measured duration | 2026-08-22 12:05:52–12:52:30 UTC (46 min 38 s), the wrapper's whole child lifetime, covering both arms; consecutive executions 10–11 s apart, mean 10.93 s, against v4 C5's ~11 s cadence |
| Raw file, reference arm | [`2026-08-22-ensemble-reference-arm-5.csv`](2026-08-22-ensemble-reference-arm-5.csv), SHA-256 `f8ac30e9d0f9c8d05a0a4ac235dc1c57ed0f6346350235e509dc4e0e5ef57dd5` |
| Raw file, candidate arm | [`2026-08-22-ensemble-candidate-arm-5.csv`](2026-08-22-ensemble-candidate-arm-5.csv), SHA-256 `91a2b893025e6ecf165f64d24afe289d0db51bf674297f2241791675aca035f8` |
| Member provenance | [`2026-08-22-member-provenance-ref-5.tsv`](2026-08-22-member-provenance-ref-5.tsv) (`034b8d7f…`), [`2026-08-22-member-provenance-cand-5.tsv`](2026-08-22-member-provenance-cand-5.tsv) (`48a4fd6a…`) — every member's `RUSTFLAGS`, binary SHA-256, `.text` address and page offset |
| Session record | `dev/active/50b47eae/s5-session/` — driver log, timed log, both build logs, both ledgers, both φ records, every script (`continue-driver.sh`, `build-arm.sh`, `gen-members.py`, `verify-construction.py`, `run-ensemble.sh`, `emit-tsv.sh`), `slots.tsv`, `members-{ref,cand}.tsv`, `comparison.txt`, `layout-audit.txt`, `single-build-comparison.txt`, `list-cells-{ref,cand}.txt`, `sha256-manifest.txt` |

### The candidate revision

**Build and run revisions coincide in this session.** The candidate binaries
were built at `1a5812c269124128f64a4f9b5efc0f1e96f19894` (the session-7
handoff commit) in a clean checkout (build log: `porcelain_lines=0`), the
timed phase ran with `HEAD` at the same commit, nothing committed anywhere in
the repository between the build phase and the session end, and every
candidate row records `1a5812c2…` with `source_dirty=false`. The
revision-ruling machinery of receipt-3 (the epic's escalation #18) and
receipt-4 is therefore exercised trivially: there is no build/run revision
split to rule on.

What the ruling's diff treatment establishes here is continuity with
receipt-4: `git diff --name-only ec857d2f..1a5812c2` restricted to each build
input in turn — `crates/`, `Cargo.toml`, `Cargo.lock`,
`rust-toolchain.toml`, `.cargo/`, `build.rs`, `scripts/` — reports no changed
file for any of them. Every file in the range is under `dev/` (session-4's
record, receipt-4, the v4 amendment, `fc976a80`'s findings, handoffs, the
progress ledger) or `.jit/`, which no build reads. The empirical witness is
sharper than the diff: the candidate ordinary build produced at `1a5812c2` is
bit-identical to the one receipt-4's session produced at `ec857d2f`
(SHA-256 `93ed5c45…`), so the two sessions measured the same compiled
candidate.

Each arm records 4,352 rows: thirty-four cells × 128 builds × one
repetition, execution indices 1 through 128 with none missing, 34 rows per
execution. Every cell's equivalence probe passed in every one of the 256
executions — a failed probe aborts the run, and the driver ran to
completion.

The raw files are written to `/tmp/gf2-50b47eae-s5-reference-arm.csv` and
`/tmp/gf2-50b47eae-s5-candidate-arm.csv`, both checked absent beforehand, and
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
The reference arm measured φ = 0, so its ordinary build is member j = 0; the
candidate arm measured φ = 1, so **its ordinary build is member j = 1
(`--execution 2`)** — every extraction below selects the ledger row with
E = 0, not an execution index.

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
793,248–797,312 bytes (+0.51 %) in the candidate arm — against +9.87 % and
+13.09 % under receipt-4's G axis: the translation-only ensemble varies
placement, not content.

### The cross-arm precondition (v4 C3)

New in this session, and the reason v4 exists: the two arms are verified to
sample the same placement distribution before any timed window. All four
checks pass, from the ledgers:

```
[ok] P_ref == P_cand (mod 32): ref%32=0 cand%32=0
[ok] page-offset sets equal
[ok] cache-line offset multisets equal: ref={0: 64, 32: 64} cand={0: 64, 32: 64}
[ok] L1i set-index multisets equal
RESULT: PASS
```

Receipt-4's excursion arose because its arms sampled unlike (G, offset)
distributions at a placement-sensitive cell; this precondition makes that
construction impossible in this session. What the verdict ratio averages
over is one placement distribution, realized identically in both arms.

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
apart, mean 10.93 s.

## Step 0 — harness against the plan

`--self-check` prints the same line from the staged ordinary member binary of
each arm (reference member 0, candidate member 1), run with its working
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
recorded in Deviations; the binaries checked are the binaries that produced
the rows.

## Step 1 — commands

The build phase runs first and holds no lock. Each arm first builds its
ordinary probe to measure P and φ and generate its member enumeration
(`gen-members.py`), then builds each member by putting its `RUSTFLAGS` in
the environment and reading the produced binary from Cargo's own JSON
(`build-arm.sh`):

```sh
RUSTFLAGS="<member flags>" CARGO_TARGET_DIR=/tmp/gf2-ens5/target-<arm> \
  cargo +1.95.0 bench -p gf2-core --features simd \
  --bench selector_non_regression --no-run --message-format=json |
  jq -r 'select(.executable != null and .target.name == "selector_non_regression") | .executable'
```

The E = 0 member takes no options and is built with `RUSTFLAGS` unset rather
than empty. The binary is copied to `/tmp/gf2-ens5/<arm>/member-<j>.bin` and
hashed before the next member is built; the scratch target directory is
deleted between members (v2 §A8), so every binary is a full fresh build.

The timed phase is one wrapper invocation over all 256 executions
(`continue-driver.sh` step 4):

```sh
GF2_BENCH=1 ./dev/scripts/ccx1-bench-flock.sh bash /tmp/gf2-ens5/run-ensemble.sh \
  /tmp/gf2-50b47eae-s5-reference-arm.csv /tmp/gf2-50b47eae-s5-candidate-arm.csv
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
  --compare dev/benchmarks/tuning_profiles/2026-08-22-ensemble-reference-arm-5.csv \
  --against dev/benchmarks/tuning_profiles/2026-08-22-ensemble-candidate-arm-5.csv
```

The run prints all thirty-four per-cell lines and a geometric mean, so the
§5 preconditions hold: both files carry schema `selector-non-regression-v1`,
their cell sets are identical and equal to the pinned set, and every cell's
arm, family and operand sizes agree. The full output is
[`comparison.txt`](/dev/active/50b47eae/s5-session/comparison.txt) in the
session record; the failure is a tolerance failure at two cells:

```
polynomial/mul_fast/len=32 mul_dispatch 1189.244810 1258.541428 1.058269 FAIL
polynomial/mul_fast/len=64 mul_dispatch 3803.703616 3994.854980 1.050254 FAIL
...
geometric_mean 1.001087 PASS
RESULT: FAIL
```

The comparison exits 1. Two cells exceed τ_cell; the set statistic is inside
τ_set; the rule requires both to hold, so the comparison fails.

| Cell | Arm | Reference ns/call | Candidate ns/call | ρ(c) | Above τ_cell by | ρ(c) in layout standard errors | Above the bar in standard errors |
|---|---|---:|---:|---:|---:|---:|---:|
| `polynomial/mul_fast/len=32` | mul_dispatch | 1,189.244810 | 1,258.541428 | 1.058269 | +0.008269 | 15.86 | 2.20 |
| `polynomial/mul_fast/len=64` | mul_dispatch | 3,803.703616 | 3,994.854980 | 1.050254 | +0.000254 | 13.47 | 0.066 |

The seventh column is `ln ρ(c) / se(c)` — how far the measured ratio stands
from 1 in units of the layout standard error the audit computes for that
cell (0.003571 and 0.003640) — and the eighth is the same distance measured
from the tolerance. Both cells stand far from 1; `len=64` clears the bar by
two hundredths of a percent, 0.066 of its own standard error. The
next-highest ratios in the set are the two neighbouring Karatsuba cells,
`polynomial/mul/len=256` at 1.046692 and `polynomial/mul/len=64` at
1.046118, both inside τ_cell.

## The attribution audit

v1 §6.5's `--layout-audit` mode computes §6.4's four preconditions. The
receipt pair naming the two committed receipts whose ratio defines `σ̂` is
mandatory, and it is supplied:

```sh
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --layout-audit dev/benchmarks/tuning_profiles/2026-08-22-ensemble-reference-arm-5.csv \
  --layout-candidate dev/benchmarks/tuning_profiles/2026-08-22-ensemble-candidate-arm-5.csv \
  --compare dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv \
  --against dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-control-arm-2.csv
```

The mode audits rather than refusing: one revision per arm, no
`source_dirty=true` row, no execution index below one, 128 members per arm
(one of the sizes `ENSEMBLE_SIZES` accepts), halves balanced on member
parity, the same member indices in both arms, and the receipt pair naming
the pinned `σ̂` sources. The full output is
[`layout-audit.txt`](/dev/active/50b47eae/s5-session/layout-audit.txt); its
last four lines:

```
set layout_se=0.001044 margin=18.971 null_geomean=1.000059 PASS
coverage ensemble_rms=0.034359 natural_rms=0.033878 PASS
decorrelation paired_rms=0.052963 reference_rms=0.034359 PASS
RESULT: PASS
```

The audit exits 0.

### The four preconditions

| Precondition | What §6.4 requires | Measured | Verdict |
|---|---|---|---|
| **Coverage, per cell** | `s_R(c) ≥ σ̂(c)/2` at every cell | Every cell clears its floor. The tightest are `polynomial/div_rem_auto/dividend=4096/divisor=1024` at 1.402×, `bit_backend/xor_inplace/words=1` at 1.404× and `bit_backend/or_inplace/words=1` at 1.411× | PASS |
| **Coverage, whole set** | RMS of `s_R` ≥ RMS of `σ̂` | 0.034359 against 0.033878, a ratio of 1.014 — the narrowest margin in the audit, as v4 C4 anticipated for a translation-only ensemble | PASS |
| **Decorrelation** | RMS of `p` ≥ RMS of `s_R` | 0.052963 against 0.034359, a ratio of 1.541 against the 1.414 fully independent arms would give | PASS |
| **Precision, per cell** | `ln(1.05) ≥ 3 · se(c)` at every cell | Every cell clears it; the narrowest margin is 4.602 at `bit_backend/xor_inplace/words=16` and the widest 87.377 at `polynomial/batch_evaluate/coeffs=2048/points=2048`. The two failing cells stand at 13.661 and 13.403 | PASS |
| **Precision, whole set** | `ln(1.02) ≥ 3 ·` set-level `se` | Set `se` = 0.001044, margin 18.971 | PASS |
| **Half-split null, per cell** | Inside ±τ_cell at every cell, read two-sided | Widest is 0.993501 at `bit_backend/xor_inplace/words=64`; every cell inside ±0.7 %. At the two failing cells the null reads 1.000330 and 1.000316 | PASS |
| **Half-split null, whole set** | Inside ±τ_set on the geometric mean | 1.000059 | PASS |

This is the second consecutive session to attribute, and the first under an
ensemble whose cross-arm distribution is verified rather than assumed. The
verdict it attributes is the comparison above.

### Which §7 reading applies

The verdict is `FAIL` and every precondition holds, so v1 §7's second row
governs:

> | FAIL | all hold | The cutover's cost exceeds a tolerance, attributably. The verdict stands, the excursion is recorded with its contradiction, and its rework is tracked. |

`50b47eae` REQ-01 is not met by this session: the receipt does not show the
tolerance holding. The excursion is preserved in this committed record with
its contradiction (Falsification record below). Neither §6.6 branch is taken
— no precondition failed — so no ladder rung and no axes amendment follows
from this session. Its disposition is the owner's (Disposition below).

## §7.1 — the recorded single-build comparison

v1 §7.1 fixes this reading in advance: the plan's §5 comparison of the
committed 2026-08-19 baseline against the candidate arm's ordinary build
alone — the E = 0 member, built under unset `RUSTFLAGS`, **member j = 1,
`--execution 2`**, binary SHA-256 `93ed5c45…` — recorded with its per-cell
lines and its `RESULT:`. **It carries no verdict standing.**

Its rows are the candidate arm's `execution=2` rows — selected by E = 0
from the ledger, not by execution index, per v4 C2's enumeration — extracted
from the committed arm file to an absolute `/tmp` path so that no
in-repository file is created:

```sh
OUT=/tmp/gf2-ens5/candidate-member-E0-ordinary.csv
head -1 dev/benchmarks/tuning_profiles/2026-08-22-ensemble-candidate-arm-5.csv > "$OUT"
awk -F, 'NR>1 && $2==2' dev/benchmarks/tuning_profiles/2026-08-22-ensemble-candidate-arm-5.csv >> "$OUT"
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --compare dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv \
  --against "$OUT"
```

It reads `RESULT: FAIL` with geometric mean 0.987708, failing at two cells:
`bit_backend/and_inplace/words=1` at 1.120041 and
`bit_backend/or_inplace/words=8` at 1.084115. Its full output is
`single-build-comparison.txt` in the session record.

**The two comparisons disagree at four of the thirty-four cells**, and §7.1
directs that every disagreeing cell be recorded with `s_R` and `σ̂`, "as the
measurement of how much one draw moved the old verdict":

| Cell | Single-build ratio | Single-build | Ensemble ρ(c) | Ensemble | `s_R` | `σ̂` |
|---|---:|---|---:|---|---:|---:|
| `bit_backend/and_inplace/words=1` | 1.120041 | FAIL | 1.024583 | PASS | 0.029250 | 0.040952 |
| `bit_backend/or_inplace/words=8` | 1.084115 | FAIL | 1.023292 | PASS | 0.022341 | 0.026910 |
| `polynomial/mul_fast/len=32` | 1.003056 | PASS | 1.058269 | FAIL | 0.006131 | 0.001877 |
| `polynomial/mul_fast/len=64` | 0.998075 | PASS | 1.050254 | FAIL | 0.004524 | 0.003848 |

The disagreement runs in both directions and the verdict cells sit on both
sides of it: two bit-backend cells trip the single ordinary draw and not the
ensemble — the same two cells, at nearly the same values (1.120719,
1.085748), as receipt-4's §7.1 draw, which is the **same bit-identical
binary** measured in a different session — while the two cells the ensemble
fails read 1.003 and 0.998 in that draw. One draw of the ordinary layout
hides the mul_dispatch excursion entirely and manufactures two bit-backend
excursions the averaged comparison reads at 1.02.

Receipt-4's verdict cell continues its own §7.1 history:
`bit_backend/not_inplace/words=1` reads 0.848371 in this draw — the fifth
consecutive ordinary-build draw of the record near 0.85 — while this
session's ensemble reads it at 1.000593 under matched placement
distributions.

## §7.2 — the candidate arm's own across-build dispersion

v1 §7.2 records this and it enters no verdict: `s_C(c)` materially above
`s_R(c)` says the cutover made that cell's cost more layout-sensitive.
Eleven cells stand at or above 1.5× their reference-arm dispersion — under
a translation-only ensemble, so the sensitivity is to pure 32-byte
translation of the image:

| Cell | Arm | `s_R` | `s_C` | `s_C`/`s_R` |
|---|---|---:|---:|---:|
| `bit_backend/xor_inplace/words=8` | simd | 0.006380 | 0.091913 | 14.406 |
| `polynomial/mul_fast/len=64` | mul_dispatch | 0.004524 | 0.040936 | 9.049 |
| `polynomial/mul_fast/len=32` | mul_dispatch | 0.006131 | 0.039938 | 6.514 |
| `polynomial/mul/len=33` | karatsuba | 0.007863 | 0.031755 | 4.039 |
| `polynomial/mul/len=256` | karatsuba | 0.009783 | 0.035827 | 3.662 |
| `bit_backend/not_inplace/words=8` | simd | 0.029059 | 0.077815 | 2.678 |
| `bit_backend/xor_inplace/words=4` | scalar | 0.028896 | 0.066977 | 2.318 |
| `bit_backend/or_inplace/words=8` | simd | 0.022341 | 0.047314 | 2.118 |
| `bit_backend/and_inplace/words=1` | scalar | 0.029250 | 0.058790 | 2.010 |
| `bit_backend/xor_inplace/words=16` | simd | 0.058732 | 0.104575 | 1.781 |
| `polynomial/mul/len=64` | karatsuba | 0.021239 | 0.035986 | 1.694 |

**Both failing cells stand in the top three**, at 9.0× and 6.5× — the
cutover made the small-length `mul_fast` dispatch cells an order of
magnitude more translation-sensitive than the reference, and the whole
`mul`/`mul_fast` sub-128 family (len 33, 64, 256 Karatsuba beside them)
moves together. The widest movements the other way are
`bit_backend/xor_inplace/words=1` at 0.046 and the two subproduct
`batch_evaluate` cells at 0.18–0.22 — cells the cutover made markedly less
translation-sensitive. This is recorded evidence about the change. It enters
no verdict.

## §7.3 — cells whose own dispersion exceeds τ_cell

Plan §7 and v1 §7.3 direct that a cell whose own dispersion inside a receipt
exceeds τ_cell be recorded as noise-dominated with its numbers, the
tolerance not widened, the cell not dropped. Under this ensemble a cell's
own dispersion is its across-build dispersion — the quantity the ensemble
deliberately injects. Seven cells in the reference arm and seven in the
candidate arm exceed 5 % on the sample coefficient of variation of their 128
per-build rates — eleven cells in one arm or the other:

| Cell | Reference arm CV | Candidate arm CV |
|---|---:|---:|
| `bit_backend/and_inplace/words=1` | 2.924 % | 5.877 % |
| `bit_backend/not_inplace/words=1` | 8.189 % | 8.144 % |
| `bit_backend/not_inplace/words=8` | 2.930 % | 8.764 % |
| `bit_backend/or_inplace/words=1` | 8.316 % | 2.979 % |
| `bit_backend/xor_inplace/words=1` | 6.946 % | 0.321 % |
| `bit_backend/xor_inplace/words=16` | 7.217 % | 13.652 % |
| `bit_backend/xor_inplace/words=4` | 2.902 % | 9.891 % |
| `bit_backend/xor_inplace/words=64` | 6.471 % | 7.101 % |
| `bit_backend/xor_inplace/words=7` | 6.189 % | 4.887 % |
| `bit_backend/xor_inplace/words=8` | 0.651 % | 13.352 % |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | 5.867 % | 3.049 % |

The tolerance is not widened for any of them and no cell is dropped. The
pooled verdict statistic carries this dispersion divided by √128: the
precision precondition measures the consequence and clears it at every cell,
4.602 at the narrowest. Neither failing cell is in this table: their
candidate-arm CVs are 4.0 % and 4.1 %, and their excursions are means, not
noise.

## The cell receipt-4 failed

Receipt-4's verdict — the record's first attributable FAIL — was
ρ = 1.100028 at `bit_backend/not_inplace/words=1` under an ensemble whose
arms realized unlike placement distributions at that cell; `fc976a80`'s
findings decomposed the excursion into that construction (matched strata
agree to 1.000124/0.999960) and v4 was written to arbitrate it. This session
is the arbitration, and the cell reads:

| Cell | Arm | Receipt-4 ρ(c) | Session-5 ρ(c) | Session-5 verdict |
|---|---|---:|---:|---|
| `bit_backend/not_inplace/words=1` | scalar | 1.100028 | 1.000593 | PASS |

Under matched placement distributions the cell's placement-averaged cost is
measured equal across the cutover to 0.06 %. Receipt-4 stands as taken —
its number is a fact of its construction — and the `fc976a80` diagnosis is
confirmed: the +10 % was the arms' unlike placement sampling, not the code.
Its `s_R` here is 0.082078, still the widest class in the set; the coverage
floor at the cell passes at many times its tiny `σ̂` (0.000222).

## The two cells this receipt fails

Both are the `mul_fast` dispatch cells below the NTT threshold —
`polynomial/mul_fast/len=32` and `len=64`, arm `mul_dispatch`, the cells
that measure `mul_fast`'s dispatch to the schoolbook and Karatsuba paths.
Their history across the record:

| Session | Comparison | `len=32` | `len=64` | Audit |
|---|---|---:|---:|---|
| Receipt 1 | single builds | 1.002877 | 0.996980 | — |
| Receipt 2 | single builds | 1.077040 FAIL | 1.073771 FAIL | — |
| Receipt 3 | K = 128, alignment axes | 1.006002 | 1.005233 | FAIL |
| Receipt 4 | K = 256, E+G axes | 1.031653 | 1.024143 | PASS |
| Receipt 4 §7.1 | single draw | 1.058070 | 0.997034 | — |
| **This receipt** | **K = 128, translation only** | **1.058269 FAIL** | **1.050254 FAIL** | **PASS** |
| This receipt §7.1 | single draw | 1.003056 | 0.998075 | — |

What the record fixes about the two cells:

- **They have tripped before.** Receipt-2's single-build comparison failed
  both at 1.077 and 1.074 — unattributable then, one layout draw against
  one layout draw. Receipt-4's ordinary draw failed `len=32` at 1.058.
  Single draws of the same revisions also read them at 1.003 and 0.998
  (receipt 1, and this session's §7.1). The cells' single-draw readings
  span the tolerance in both directions.
- **The cutover multiplied their translation sensitivity.** `s_C`/`s_R`
  reads 6.5 at `len=32` and 9.0 at `len=64` (§7.2): on the reference
  revision, translating the image moves these cells by ~0.5 %; on the
  candidate it moves them by ~4 %. The excursion is the mean over all 128
  translations of the candidate's ordinary arrangement, 15.9 and 13.5
  layout standard errors from 1 — not a tail draw.
- **The Karatsuba neighbours move with them.** `polynomial/mul/len=256`
  (1.046692), `polynomial/mul/len=64` (1.046118) and
  `polynomial/mul/len=33` (1.019216) are the next-highest ratios in the
  set, with `s_C`/`s_R` of 3.7, 1.7 and 4.0. The direct schoolbook cell
  `polynomial/mul/len=32` reads 0.981939: the excursion pattern sits on the
  dispatch and Karatsuba paths, not on schoolbook itself.
- **The two attributable sessions disagree at these cells.** Receipt-4
  (E+G, K = 256) read them at 1.031653 and 1.024143 — inside tolerance —
  over the same compiled source, witnessed bit-identical by the ordinary
  builds. The two ensembles average over different layout populations:
  receipt-4's G axis samples two arrangements per arm and its arms' unlike
  lattices are what `fc976a80` diagnosed; this session samples 128
  translations of the one ordinary arrangement, identically in both arms.
  Under v4, the governing estimand is this session's, and this session's
  verdict is the one its audit attributes. The 2.6 % gap between the two
  sessions' readings at `len=32` is recorded here as a fact about
  arrangement dependence at these cells, in the same class as the +4 %
  translation sensitivity §7.2 measures.
- **`len=64` fails by two hundredths of a percent.** ρ = 1.050254 against
  τ_cell = 1.05, 0.066 of its own layout standard error above the bar; the
  same statistic at `len=32` stands 2.20 standard errors above the bar. The
  rule is ρ > 1.05 and both cells exceed it; the margin is recorded because
  §7.3's register records what is a mean and what is noise, and at
  `len=64` the distance from the bar is inside one standard error.

## Falsification record

### The session establishes an attributable FAIL

The audit reports `RESULT: PASS` and the comparison `RESULT: FAIL`. Per v1
§7's second row the verdict stands: the candidate revision's cost at
`polynomial/mul_fast/len=32` and `polynomial/mul_fast/len=64` exceeds τ_cell
against the baseline revision, averaged over 128 translations per arm of
each revision's ordinary arrangement, with every attribution precondition
holding and both arms' placement distributions verified equal before the
timed phase. **What this contradicts:** `50b47eae` REQ-01 requires a receipt
showing the predeclared tolerance holds for the pinned set; this receipt
shows it failing to hold at two cells.

### The verdict cells' single-build readings point the other way

The committed baseline draw against this session's ordinary candidate build
reads 1.003056 and 0.998075 at the two verdict cells — comfortably inside
τ_cell — while the ensemble reads 1.058269 and 1.050254. Both are recorded.
This is the same contradiction receipt-4 recorded at its verdict cell with
the directions exchanged: a single draw of the ordinary layout carries no
evidence about a cell whose candidate-side translation dispersion is 4 %,
in either direction.

### The set rule passes and the per-cell rule fails

The geometric mean is 1.001087 — the closest to 1 of the five sessions.
Thirty-two cells inside tolerance, eighteen within 1 %, and two cells 5.8 %
and 5.0 % out on one dispatch path. The set rule passing is not evidence the
per-cell excursion is absent; both rules must hold.

### Preserved

Per `@/inv/falsification-preserved` and control-arm §4.2, both failing cells
are recorded with both pooled rates, their ratios, their layout standard
errors and their disagreeing single-build draws; receipt-2's unattributable
FAIL at the same two cells and receipt-4's inside-tolerance ensemble reading
at them are cited above rather than reconciled away. Per plan §7 and
control-arm §4.5 this session is run once and stands as taken.

## Disposition

The session establishes the record's second attributable verdict and it is
FAIL at two cells of one family. It simultaneously closes the question v4
was written to arbitrate: receipt-4's excursion cell reads 1.000593 under
matched placement distributions, so the `fc976a80` construction-artifact
diagnosis is measured correct, and that issue's excursion is resolved
without a code change.

`50b47eae` REQ-01 remains unmet — not for want of attribution but because
the tolerance does not hold at `polynomial/mul_fast/len=32` (+5.83 %) and
`polynomial/mul_fast/len=64` (+5.03 %, 0.066 standard errors over the bar).
The epic's REQ-04 gate stays unmet with it. Per the epic's standing routing
(session-7 handoff), the disposition of a further attributable excursion
after `fc976a80` is the owner's decision, with the candidate resolutions
tracked rework of the `mul_fast` dispatch path, owner acceptance, or a
further measured session under the standing procedure. No predeclared value
moves; the v4 construction stands for any next session unchanged.

What this session settles:

- **The cross-arm precondition holds by construction and by measurement.**
  The first ensemble of the record whose two arms verifiably sample one
  placement distribution, before any timed window.
- **Receipt-4's excursion was construction, and the machinery now excludes
  it.** The +10 % cell reads +0.06 % under v4.
- **The translation-only ensemble attributes.** Coverage — v4 C4's stated
  risk — clears at every cell and at 1.014 on the RMS; precision, the
  half-split null and decorrelation clear with margin.
- **The remaining excursion is real under the v4 estimand, small, and
  localized**: one dispatch path, two cells, +5.8 %/+5.0 % on means whose
  candidate-side translation sensitivity the cutover raised by 6.5×/9×.

## What stands unchanged

- **The tolerance.** τ_cell stays 5 % and τ_set stays 2 %. Neither is
  widened to admit the excursion — `len=64`'s 0.066-standard-error margin
  over the bar included.
- **The pinned set.** All thirty-four cells stand. No cell is added,
  dropped or re-bracketed.
- **The ensemble.** K stays 128 under v4 C2's enumeration and C3's
  cross-arm precondition. The margin of three standard errors stands.
- **The schema token.** `selector-non-regression-v1` is not bumped; the
  baseline, the four prior post-cutover receipts and both of this session's
  arms stay valid and comparable.
- **The standing receipts.** All five, and the pilot receipt, stand as
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
  row.
- **The first launch of the session pipeline died before its driver
  started.** Its `nohup` redirect targeted `/tmp/gf2-ens5/logs/` before that
  directory existed; the shell exited on the redirect, no process ran, no
  file was written and no build occurred. The logs directory was created
  and the driver relaunched; the committed driver log is the complete
  record of the run (single `continue-driver start` line, PID 1157173,
  detached with PPID 1).
- **The child runs at niceness 5.** The wrapper's best-effort `nice -n -5`
  is denied, exactly as receipts 3 and 4 record. The lock and affinity
  remain in force, the host carries no competing work, and both arms run at
  the same niceness inside one session, so the verdict ratio carries none
  of it.
- **The session log's `locks=` probe is fixed.** `run-ensemble.sh` now
  greps `/proc/locks` for the lock file's decimal inode and the record
  carries the held-lock line (`FLOCK ADVISORY WRITE … 00:2d:39210`)
  receipt-4's session could not print. The fix is to the recording probe
  only; the wrapper's `exec flock -x` semantics are unchanged.
- **Each arm builds one ordinary probe before its enumeration.** v4 C2
  fixes each arm's enumeration by the measured `.text` address of its
  ordinary build, so `build-arm.sh` builds it once to read P and φ
  (`ordinary-probe.bin`, `phi-{ref,cand}.txt`), then builds the 128 members
  from the generated table. The probe binaries are bit-identical to each
  arm's E = 0 member (`c7be7a87…`, `93ed5c45…`) — the reproducibility this
  relies on, verified by hash. No probe row enters any arm.
- **Builds ran in scratch target directories deleted between members**,
  per v2 §A8; each binary was staged and hashed before its target directory
  was removed, and `/tmp` free space never approached the build driver's
  3 GiB floor. All 256 ledger rows record fresh first-attempt builds of
  13–15 s.
- **The §7.1 extraction writes to `/tmp`**, one `head` and one `awk` over
  the committed arm file, reproducible from it, selecting `execution=2` —
  the E = 0 member of the φ = 1 candidate arm.
- **The session driver is the lead-authored `continue-driver.sh`**, the
  session-4 driver restructured for v4: per-arm φ probe and enumeration,
  per-arm and cross-arm verification before the timed phase, and the same
  guard, lock, ordering and copy-out mechanics. All scripts and their
  hashes are in the session record and its manifest.
