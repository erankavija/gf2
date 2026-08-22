# Fourth post-cutover receipt for the pinned selector non-regression set

This is the fourth post-cutover receipt of issue `50b47eae`, the first taken at
K = 256 under the amended ensemble axes of
[`layout-attribution-verdict-v2.md`](layout-attribution-verdict-v2.md) (owner
decision DEC-I), and the first whose realized construction is read through
[`layout-attribution-verdict-v3.md`](layout-attribution-verdict-v3.md) (owner
decision DEC-J). Each side of the plan's §5 comparison is an ensemble of 256
builds of one revision, so across-build code layout enters the verdict
statistic as a sampled quantity whose dispersion this session measures.

The procedure runs as
[`selector-non-regression-plan-v1.md`](selector-non-regression-plan-v1.md),
[`layout-attribution-verdict-v1.md`](layout-attribution-verdict-v1.md), v2 and
v3 together fix it. The pinned cell set of plan §2, the tolerance of plan §4,
the comparison rule of plan §5, the run protocol of plan §3 and the schema
token `selector-non-regression-v1` are unchanged; τ_cell is 5 % and τ_set is
2 % at their predeclared values. No tuning profile is installed anywhere in
this session, and no build of it sets `gf2_tuning_baked`.

The four standing receipts —
[`2026-08-19-pre-cutover-baseline.md`](2026-08-19-pre-cutover-baseline.md),
[`2026-08-20-post-cutover-receipt.md`](2026-08-20-post-cutover-receipt.md),
[`2026-08-20-post-cutover-receipt-2.md`](2026-08-20-post-cutover-receipt-2.md)
and [`2026-08-20-post-cutover-receipt-3.md`](2026-08-20-post-cutover-receipt-3.md)
— and the pilot receipt of `9162956b` stand as taken. None is modified,
superseded, re-run or adjusted by this receipt.

## Result

**The session establishes an attributable verdict, and it is FAIL.** The
attribution audit reports `RESULT: PASS` on all four preconditions — the first
session of the four to attribute — and the verdict comparison reports
`RESULT: FAIL`, so the second row of v1 §7's reading table applies:

> The cutover's cost exceeds a tolerance, attributably. The verdict stands,
> the excursion is recorded with its contradiction, and its rework is tracked.

- **Exactly one cell exceeds τ_cell.** `bit_backend/not_inplace/words=1`
  (scalar) reads ρ = 1.100028 — 2.679814 ns/call in the candidate arm against
  2.436132 in the reference arm, +0.243682 ns/call — 0.050028 above the bar
  and 10.95 layout standard errors above 1. The other thirty-three cells pass,
  twenty of them below 1.01 in either direction.
- **The set rule passes.** The geometric mean of the thirty-four ratios is
  1.002950, inside τ_set = 1.02, and the closest to 1 of the four sessions.
  Both rules must hold, so the comparison is `RESULT: FAIL`.
- **Every attribution precondition holds.** Coverage clears its floor at all
  thirty-four cells (tightest 1.221 at
  `polynomial/div_rem_auto/dividend=4096/divisor=1024`) and on the RMS
  (0.044676 against 0.033878); precision clears its three-standard-error
  margin everywhere (narrowest 5.604, at the failing cell itself); the
  half-split null stays inside ±τ_cell at every cell (widest 0.991358) and
  reads 0.999055 on the geometric mean; decorrelation passes at 0.063449
  against 0.044676. The two cells v2 exists to repair clear their floors at
  1.563 (`bit_backend/or_inplace/words=1`) and 1.501
  (`bit_backend/xor_inplace/words=1`).
- **The excursion of the three prior sessions is gone.** The two
  `bit_backend/popcount` cells, outside τ_cell in every earlier post-cutover
  session, read 0.996574 and 1.002381. What this session measures is the
  candidate revision after `676f55a2` baked the bit-backend threshold as a
  compile-time constant (owner decision DEC-G); the residual selection-boundary
  cost those sessions attributed is not in this record.

Per v1 §7's second row the verdict stands. `50b47eae` REQ-01 — a receipt
showing the tolerance holds — is not met by this receipt; per REQ-02 the
excursion is preserved here with its contradiction, and its rework is tracked
as issue `fc976a80`. Per plan §7 and control-arm §4.5 this session is run once
and stands as taken. No predeclared value moves for it.

## Post-cutover state

The bit-backend selection boundary is a compile-time constant.
`select_backend_for_size` at `crates/gf2-core/src/kernels/backend.rs:110`
compares against `SIMD_MIN_WORDS` (`:89`), which resolves to
`SIMD_MIN_WORDS_DEFAULT = 8` (`:83`) in every build that does not set the
`gf2_tuning_baked` cfg — every build in this session — and carries zero
per-call profile reads: no atomic load, no resolved flag, no cold path. This
is `676f55a2`'s DEC-G state; the runtime atomic threshold cache receipt-3
describes is removed, and `TuningProfile::install()` governs the polynomial
family only.

The complete diff in `crates/` between receipt-3's candidate revision
`46d21d6d` and this session's compiled source `ec857d2f` is the `676f55a2`
chain (`7163e2a9` and its documentation sweeps): 16 files, +197/−94, the bake
and its witnesses. Behaviour is unchanged at every pinned size: `--self-check`
resolves `simd_min_words=8` from the staged member-0 binary of each checkout,
and `--list-cells` prints thirty-four byte-identical rows in the two
checkouts, so the comparison's per-cell identity precondition holds and no
cell changes arm.

## Reproducible protocol and provenance

Every value below is observed during this run. The two arms share one host
state, one lock, one affinity mask and one wrapper invocation; the rows that
differ between them are marked.

| Item | Value |
|---|---|
| Harness and checker | `crates/gf2-core/benches/selector_non_regression.rs`; schema `selector-non-regression-v1`; full-worktree `git status --porcelain --untracked-files=all` |
| Procedure | plan v1, as amended by verdict v1, v2 (DEC-I) and v3 (DEC-J), executed as committed |
| Ensemble | K = 256 members per v2 §A3; `--execution j+1` names member `j`; every member built on its first attempt, so neither v2 §A8's retry nor v1 §3.5's replacement was exercised |
| Source revision, candidate arm | Binaries built at `ec857d2fbd57a2a862c1a51c3e32dcb6c4070322` in a clean checkout (build log: `porcelain_lines=0`); every row records `26df493968883d6dbf653d2e2fc73ef3bc0f3074` and `source_dirty=false` — see [The candidate revision](#the-candidate-revision) |
| Source revision, reference arm | `0c072d73ca65cf50af98b8c4b61ed876f8218df6`, the revision every row of the baseline receipt records; every row records `source_dirty=false` |
| Reference-arm checkout | `.agents/worktrees/control-0c072d73`, detached at `0c072d73` and clean at the build phase, the session start and the session end |
| Reference member 0 | SHA-256 `c7be7a87d7a01d3c2e297919033a99237afe8939f3fce147802ecd86df32c710`, bit-identical to the ordinary reference build of receipts 1–3 and the pilot's provenance reproduction |
| Bench binaries | 512, one per member per arm, each built in a scratch target directory outside the checkout (v2 §A8), staged to `/tmp/gf2-ens4/{ref,cand}/` and hashed before the next member was built; the scratch directory was deleted between members |
| Construction verification | v2 §A4 read through v3 §B3, both arms `RESULT: PASS` before any timed window: reference P(0)=3008, P(1)=1488, four cache-line offsets {0,16,32,48} at 32 per half; candidate P(0)=2208, P(1)=480, two cache-line offsets {0,32} at 64 per half, one whole-page displacement (member 227, −1 page, on-law); both arms cover all 64 L1i sets twice per half and balance every axis. The v2-as-written FAIL this reading answers is in the driver log and in v3 §B1 |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)`, built and run as `cargo +1.95.0`, both arms |
| Features | `simd`, both arms; no tuning profile installed; `gf2_tuning_baked` not set |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor |
| OS/kernel and governor | `Linux 7.1.8-arch1-3 #1 SMP PREEMPT_DYNAMIC Tue, 11 Aug 2026 09:16:08 +0000 x86_64 GNU/Linux`; `powersave` on CPUs 6–11 |
| Lock and affinity | `dev/scripts/ccx1-bench-flock.sh`, one invocation holding `/tmp/gf2-ccx1.lock` (stat `dev=2d inode=39210`) for all 512 executions; CPUs 6–11, observed as `Cpus_allowed_list: 6-11` inside the wrapper. The session log's `locks=` probe recorded empty for the reason in Deviations; the wrapper's `exec flock -x` holds the lock for its child's whole lifetime by construction |
| Niceness | The wrapper's best-effort `nice -n -5` is denied; the child runs at niceness 5, as receipt-3 records and Deviations discusses |
| Host quiescence | One-minute load average 0.08 as the wrapper is invoked (five- and fifteen-minute 0.12 and 0.10) and 1.00 at its end; no builds, benches or campaign processes ran on the host during the timed phase |
| Timed work | One execution of one repetition per build, `--target-ms 250`, 512 executions in total (v2 §A8, v1 §5.1), each preceded by the plan's equivalence probes |
| Build window | 2026-08-21 20:07:44–22:03:51 UTC (reference 20:07:44–21:07:09, candidate 21:07:09–22:03:51), no lock held, no compilation under the lock; 3,553 s + 3,391 s of compiler wall-clock |
| Measured duration | 2026-08-22 08:41:29–10:15:39 UTC (94 min 10 s), the wrapper's whole child lifetime, covering both arms; consecutive executions 10–12 s apart, mean 11.04 s, against v2 §A8's ~11 s budget |
| Raw file, reference arm | [`2026-08-22-ensemble-reference-arm-4.csv`](2026-08-22-ensemble-reference-arm-4.csv), SHA-256 `1d5b39b8e85a867e684d67b84742ab04af55a6297d32c4de15b3e52ac7ed0f76` |
| Raw file, candidate arm | [`2026-08-22-ensemble-candidate-arm-4.csv`](2026-08-22-ensemble-candidate-arm-4.csv), SHA-256 `3f39676023074cdd5bb6a3e138d2d0f987acd41538c34ed250c44fc32b3135be` |
| Member provenance | [`2026-08-22-member-provenance-ref-4.tsv`](2026-08-22-member-provenance-ref-4.tsv) (`4dc5c75c…`), [`2026-08-22-member-provenance-cand-4.tsv`](2026-08-22-member-provenance-cand-4.tsv) (`aa8c89ce…`) — every member's `RUSTFLAGS`, binary SHA-256, `.text` address and page offset |
| Session record | `dev/active/50b47eae/s4-session/` — driver log, timed log, both build logs, both ledgers, every script (including `verify-construction.py` as amended per v3 and its v2-as-written text `verify-construction-v2.py`), `slots.tsv`, `comparison.txt`, `layout-audit.txt`, `sha256-manifest.txt` |

### The candidate revision

The candidate binaries were built at `ec857d2f` on the evening of 2026-08-21
UTC; the timed phase ran on the morning of 2026-08-22 UTC. Between the two,
exactly two commits landed on main: `bbc4094f`, the epic's session-6 handoff,
and `26df4939`, the DEC-J commit adding amendment v3 and the epic's progress
ledger. The harness records `git_revision` at run time from the binary's
working directory (`crates/gf2-core/benches/selector_non_regression.rs:1020`),
so every candidate row records `26df4939`.

**The compiled source is identical at the two revisions**, stated from the
diff rather than assumed. The complete list of files
`git diff --name-only ec857d2f..26df4939` reports is three:

```
dev/active/6dc81018-field-capability-dispatch/handoff-2.md
dev/active/6dc81018-field-capability-dispatch/progress.json
dev/benchmarks/tuning_profiles/layout-attribution-verdict-v3.md
```

Restricting the same diff to each build input in turn — `crates/`,
`Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/`, `build.rs`,
`scripts/` — reports no changed file for any of them. All three files are
documentation and a progress ledger under `dev/`, which no build reads. This
is the treatment the owner ruling of receipt-3 (the epic's escalation #18)
fixes: the arm records the revision the checkout stood at, and the identity of
the compiled source is stated from the diff. The DEC-J commit was made
deliberately between the build phase and the timed phase, with this
verification planned in advance; nothing committed while the timed phase ran,
and the session guard records `HEAD` at `26df4939` with an empty full-worktree
status at the start and the end.

Each arm records 8,704 rows: thirty-four cells × 256 builds × one repetition.
Every row of the candidate arm carries `source_dirty=false` and revision
`26df4939…`; every row of the reference arm carries `source_dirty=false` and
revision `0c072d73…`. Each arm records exactly 34 rows per execution and
execution indices 1 through 256 with none missing. Every cell's equivalence
probe passed in every one of the 512 executions — a failed probe aborts the
run, and the driver ran to completion.

The raw files are written to `/tmp/gf2-50b47eae-s4-reference-arm.csv` and
`/tmp/gf2-50b47eae-s4-candidate-arm.csv`, both checked absent beforehand, and
copied into this directory afterwards with the SHA-256 above recorded in the
session manifest. No in-repository file is created while the run is in flight.

## The ensemble

Member `j` takes `E(j) = ⌊j/2⌋` and `G(j) = (⌊j/4⌋ + j) mod 2` per v2 §A3:
axis E lengthens the `.note.gnu.build-id` payload to `20 + 32·E` bytes,
translating the loaded image by 32·E bytes, and axis G passes
`-C link-dead-code`, retaining dead code and rearranging live code inside the
image. A level of 0 omits its option, so member 0 of each arm is the ordinary
build.

**All 512 members built on the first attempt** (both ledgers record
`attempts=1` throughout) and **all 512 binaries are distinct** — 256 distinct
SHA-256 per arm, where receipt-3's alignment axes produced collisions.

**The realized placements.** The reference arm realizes v2 §A4's construction
exactly as written: 256 distinct `.text` page offsets on two disjoint 32-byte
lattices (P(0)=3008, P(1)=1488), four cache-line offsets {0, 16, 32, 48} at
32 members per half each. The candidate arm realizes the v3 §B3 reading: its
two G-level bases separate by ≡ 0 (mod 32) (P(0)=2208, P(1)=480), so the
levels share their 128 page offsets — each shared offset holding level E of
G = 0 and level E+54 (mod 128) of G = 1, two distinct arrangements — and two
cache-line offsets {0, 32} at 64 members per half each. One whole-page
displacement: member 227 (E=113, G=1) sits one page below its group's plane,
at exactly the page offset the law puts it. In both arms the two halves of
the member-index parity split hold equal counts at each realized cache-line
offset, their `(address >> 6) mod 64` multisets are equal, each half covers
all 64 L1i sets exactly twice, and the split is balanced on every axis. Why
the candidate revision cannot realize a 16-mod-32 base separation —
`.rodata` at `Align = 32` between the build-id note and `.text` — is v3
§B2's probe record.

Binary sizes span 726,808–798,536 bytes (+9.87 %) in the reference arm and
793,248–897,120 bytes (+13.09 %) in the candidate arm.

### Member provenance

Every member's `RUSTFLAGS`, the SHA-256 of the binary it produced, its
`.text` virtual address and its page offset, for both arms, are the two
committed provenance TSVs named above, generated from the build ledgers the
session record preserves.

## Arm ordering

Member `j` runs its two arms adjacently, reference first when `j` is even and
candidate first when `j` is odd, per v1 §5.2. The session driver implements
exactly that: over the 512 slots there is no violation of the rule, each arm
holds exactly 128 of the first-of-pair positions, and the mean slot position
of the two arms is equal at 256.5. The slot-by-slot record is
`slots.tsv` in the session record. Consecutive executions start 10 s to 12 s
apart, mean 11.04 s.

## Step 0 — harness against the plan

`--self-check` prints the same line from the staged member-0 binary of each
checkout, run with its working directory at its own checkout:

```
protocol: schema=selector-non-regression-v1 cells=34 repetitions=5 target_ms=250 per_cell_tolerance=0.050000 set_tolerance=0.020000 simd_min_words=8
self-check PASS
```

`per_cell_tolerance=0.050000` and `set_tolerance=0.020000` equal the plan's §4
values, and `cells=34` equals its §2 count. `simd_min_words=8` is the value
both checkouts resolve — the candidate through the DEC-G compile-time
constant, the reference through its compiled-in constant — so the two agree
on the guard the bit-backend cells straddle.

`--list-cells` prints thirty-four rows in each checkout and the two listings
are byte-identical (`list-cells-ref.txt`, `list-cells-cand.txt` in the
session record beside the logs), reproducing the plan's §2 table cell for
cell with the arm it predicts for each.

## Step 1 — commands

The build phase runs first and holds no lock. Each member is built by putting
its `RUSTFLAGS` in the environment and reading the produced binary from
Cargo's own JSON (`build-arm.sh`, `member-flags.sh`):

```sh
RUSTFLAGS="<member flags>" CARGO_TARGET_DIR=/tmp/gf2-ens4/target-<arm> \
  cargo +1.95.0 bench -p gf2-core --features simd \
  --bench selector_non_regression --no-run --message-format=json |
  jq -r 'select(.executable != null and .target.name == "selector_non_regression") | .executable'
```

Member 0 takes no options and is built with `RUSTFLAGS` unset rather than
empty. The binary is copied to `/tmp/gf2-ens4/<arm>/member-<j>.bin` and
hashed before the next member is built; the scratch target directory is
deleted between members (v2 §A8), so every binary is a full fresh build.

The timed phase is one wrapper invocation over all 512 executions
(`continue-driver.sh` step 4):

```sh
GF2_BENCH=1 ./dev/scripts/ccx1-bench-flock.sh bash /tmp/gf2-ens4/run-ensemble.sh \
  /tmp/gf2-50b47eae-s4-reference-arm.csv /tmp/gf2-50b47eae-s4-candidate-arm.csv
```

`run-ensemble.sh` records the host facts this receipt cites, then runs the
512 executions in v1 §5.2's order, each staged binary with its working
directory at its own checkout, `--output` to an absolute `/tmp` path checked
absent beforehand. The guard receipt-3 introduced is kept: before every
candidate execution the driver re-reads the main checkout's `HEAD` and its
full-worktree porcelain status and aborts the session if either has moved. It
took no action.

## Arm rates

Pooled nanoseconds per call per the plan's §5 — the sum of `elapsed_ns` over
every recorded window of a cell in an arm divided by the sum of `calls`, 256
windows per cell per arm — are the `baseline_ns_per_call` and
`candidate_ns_per_call` columns of the verdict comparison below, which prints
all thirty-four cells. They are not repeated here.

## Comparison of the two arms — the verdict

This is the comparison v1 §6.2 defines and the epic gates on: the reference
arm against the candidate arm, on pooled ns/call per cell, judged by
τ_cell = 5 % and τ_set = 2 %, computed by the plan's §5 mode unmodified.

```sh
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --compare dev/benchmarks/tuning_profiles/2026-08-22-ensemble-reference-arm-4.csv \
  --against dev/benchmarks/tuning_profiles/2026-08-22-ensemble-candidate-arm-4.csv
```

The run prints all thirty-four per-cell lines and a geometric mean, so the §5
preconditions hold: both files carry schema `selector-non-regression-v1`,
their cell sets are identical and equal to the pinned set, and every cell's
arm, family and operand sizes agree. The full output is
[`comparison.txt`](/dev/active/50b47eae/s4-session/comparison.txt) in the
session record; the failure is a tolerance failure at one cell:

```
bit_backend/not_inplace/words=1 scalar 2.436132 2.679814 1.100028 FAIL
...
geometric_mean 1.002950 PASS
RESULT: FAIL
```

The comparison exits 1. One cell exceeds τ_cell; the set statistic is inside
τ_set; the rule requires both to hold, so the comparison fails.

| Cell | Arm | Reference ns/call | Candidate ns/call | ρ(c) | Above τ_cell by | ρ(c) in layout standard errors |
|---|---|---:|---:|---:|---:|---:|
| `bit_backend/not_inplace/words=1` | scalar | 2.436132 | 2.679814 | 1.100028 | +0.050028 | 10.95 |

The last column is `ln ρ(c) / se(c)` — how far the measured ratio stands from
1 in units of the layout standard error the audit computes for that cell
(0.008706). The excursion also stands 5.35 standard errors above the
tolerance itself. The next-highest ratios in the set are
`polynomial/mul_fast/len=32` at 1.031653 and `polynomial/mul/len=16` at
1.025804, both inside τ_cell.

## The attribution audit

v1 §6.5's `--layout-audit` mode computes §6.4's four preconditions. The
receipt pair naming the two committed receipts whose ratio defines `σ̂` is
mandatory, and it is supplied:

```sh
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --layout-audit dev/benchmarks/tuning_profiles/2026-08-22-ensemble-reference-arm-4.csv \
  --layout-candidate dev/benchmarks/tuning_profiles/2026-08-22-ensemble-candidate-arm-4.csv \
  --compare dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv \
  --against dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-control-arm-2.csv
```

The mode audits rather than refusing: one revision per arm, no
`source_dirty=true` row, no execution index below one, 256 members per arm
(one of the sizes `ENSEMBLE_SIZES` accepts, per v2 §A5), halves balanced on
member parity, the same member indices in both arms, and the receipt pair
naming the pinned `σ̂` sources. The full output is
[`layout-audit.txt`](/dev/active/50b47eae/s4-session/layout-audit.txt); its
last four lines:

```
set layout_se=0.001051 margin=18.844 null_geomean=0.999055 PASS
coverage ensemble_rms=0.044676 natural_rms=0.033878 PASS
decorrelation paired_rms=0.063449 reference_rms=0.044676 PASS
RESULT: PASS
```

The audit exits 0.

### The four preconditions

| Precondition | What §6.4 requires | Measured | Verdict |
|---|---|---|---|
| **Coverage, per cell** | `s_R(c) ≥ σ̂(c)/2` at every cell | Every cell clears its floor. The tightest is `polynomial/div_rem_auto/dividend=4096/divisor=1024` at 1.221× its floor; the two cells receipt-3 failed clear theirs at 1.563× (`bit_backend/or_inplace/words=1`, `s_R` = 0.092329 against 0.059077) and 1.501× (`bit_backend/xor_inplace/words=1`, 0.074373 against 0.049556) | PASS |
| **Coverage, whole set** | RMS of `s_R` ≥ RMS of `σ̂` | 0.044676 against 0.033878 | PASS |
| **Decorrelation** | RMS of `p` ≥ RMS of `s_R` | 0.063449 against 0.044676, a ratio of 1.420 against the 1.414 fully independent arms would give | PASS |
| **Precision, per cell** | `ln(1.05) ≥ 3 · se(c)` at every cell | Every cell clears it; the narrowest margin is 5.604 at `bit_backend/not_inplace/words=1` — the failing cell — and the widest 68.697 at `polynomial/batch_evaluate/coeffs=4095/points=4095` | PASS |
| **Precision, whole set** | `ln(1.02) ≥ 3 ·` set-level `se` | Set `se` = 0.001051, margin 18.844 | PASS |
| **Half-split null, per cell** | Inside ±τ_cell at every cell, read two-sided | Widest is 0.991358 at `bit_backend/xor_inplace/words=16`; every cell inside ±0.9 % | PASS |
| **Half-split null, whole set** | Inside ±τ_set on the geometric mean | 0.999055 | PASS |

This is the first of the four post-cutover sessions whose audit passes, and it
does so at K = 256 under the v2 axes: the enumeration whose pilot cleared
every floor on the reference revision clears every floor in the measured
session too. The verdict it attributes is the comparison above.

### Which §7 reading applies

The verdict is `FAIL` and every precondition holds, so v1 §7's second row
governs:

> | FAIL | all hold | The cutover's cost exceeds a tolerance, attributably. The verdict stands, the excursion is recorded with its contradiction, and its rework is tracked. |

`50b47eae` REQ-01 is not met by this session: the receipt does not show the
tolerance holding. Per REQ-02 the excursion is preserved in this committed
record with its contradiction (Falsification record below), and its rework is
tracked as issue `fc976a80`. Neither §6.6 branch is taken — no precondition
failed — so no ladder rung and no axes amendment follows from this session.

## §7.1 — the recorded single-build comparison

v1 §7.1 fixes this reading in advance: the plan's §5 comparison of the
committed 2026-08-19 baseline against the candidate arm's ordinary build
alone — member E = G = 0, built under empty `RUSTFLAGS`, binary SHA-256
`93ed5c457ed01e26e6b1b44e0723b980f5c979c2f0e4540e3454684f437869c0` — recorded
with its per-cell lines and its `RESULT:`. **It carries no verdict standing.**

Its rows are the candidate arm's `execution=1` rows, extracted from the
committed arm file to an absolute `/tmp` path so that no in-repository file
is created:

```sh
OUT=/tmp/gf2-ens4/candidate-member0-ordinary.csv
head -1 dev/benchmarks/tuning_profiles/2026-08-22-ensemble-candidate-arm-4.csv > "$OUT"
awk -F, 'NR>1 && $2==1' dev/benchmarks/tuning_profiles/2026-08-22-ensemble-candidate-arm-4.csv >> "$OUT"
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --compare dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv \
  --against "$OUT"
```

It reads `RESULT: FAIL` with geometric mean 0.989670, failing at four cells:
`bit_backend/and_inplace/words=1` at 1.120719, `bit_backend/or_inplace/words=8`
at 1.085748, `polynomial/mul_fast/len=32` at 1.058070 and
`polynomial/mul/len=64` at 1.050854.

**The two comparisons disagree at five of the thirty-four cells**, and §7.1
directs that every disagreeing cell be recorded with `s_R` and `σ̂`, "as the
measurement of how much one draw moved the old verdict":

| Cell | Single-build ratio | Single-build | Ensemble ρ(c) | Ensemble | `s_R` | `σ̂` |
|---|---:|---|---:|---|---:|---:|
| `bit_backend/not_inplace/words=1` | 0.849929 | PASS | 1.100028 | FAIL | 0.093325 | 0.000222 |
| `bit_backend/and_inplace/words=1` | 1.120719 | FAIL | 1.009508 | PASS | 0.062032 | 0.040952 |
| `bit_backend/or_inplace/words=8` | 1.085748 | FAIL | 1.007744 | PASS | 0.048574 | 0.026910 |
| `polynomial/mul_fast/len=32` | 1.058070 | FAIL | 1.031653 | PASS | 0.040498 | 0.001877 |
| `polynomial/mul/len=64` | 1.050854 | FAIL | 1.016567 | PASS | 0.039709 | 0.008774 |

The direction is informative in both directions. Four cells trip the single
ordinary build and not the ensemble, by 5.1 % to 12.1 % — one layout draw
moving cells the averaged comparison reads at 1.008 to 1.032. The remaining
disagreement is the verdict cell itself, and it is the sharpest measurement
§7.1 has produced: the ordinary build reads `bit_backend/not_inplace/words=1`
at **0.849929** — 15 % *faster* than the baseline draw — while the
256-member ensemble reads +10.0 %. `s_R` at that cell is 0.093325, the
largest in the pinned set: single builds of these two revisions land anywhere
inside roughly ±10 % at this cell, and one draw of it says nothing about the
cutover. Receipt-3's §7.1 recorded the same cell at 0.851143 from its own
ordinary build — the same hiding draw, twice. The averaged excursion the
verdict reports is invisible to every single-build comparison this record has
ever taken, in either direction.

## §7.2 — the candidate arm's own across-build dispersion

v1 §7.2 records this and it enters no verdict: `s_C(c)` materially above
`s_R(c)` says the cutover made that cell's cost more layout-sensitive. Three
cells stand at or above 1.5× their reference-arm dispersion:

| Cell | Arm | `s_R` | `s_C` | `s_C`/`s_R` |
|---|---|---:|---:|---:|
| `bit_backend/xor_inplace/words=8` | simd | 0.042683 | 0.074170 | 1.738 |
| `bit_backend/xor_inplace/words=4` | scalar | 0.033413 | 0.056109 | 1.679 |
| `polynomial/mul/len=32` | schoolbook | 0.020953 | 0.033258 | 1.587 |

None of the three is the verdict cell: `bit_backend/not_inplace/words=1`
reads 1.108 — the cutover barely changes that cell's layout sensitivity; it
changes its average. The widest movements the other way are
`polynomial/div_rem_auto/dividend=4096/divisor=1024` at 0.552 and
`bit_backend/and_inplace/words=8` at 0.697. This is recorded evidence about
the change. It enters no verdict.

## §7.3 — cells whose own dispersion exceeds τ_cell

Plan §7 and v1 §7.3 direct that a cell whose own dispersion inside a receipt
exceeds τ_cell be recorded as noise-dominated with its numbers, the tolerance
not widened, the cell not dropped. Under this ensemble a cell's own
dispersion is its across-build dispersion — the quantity the ensemble
deliberately injects. Twelve cells in the reference arm and thirteen in the
candidate arm exceed 5 % on the sample coefficient of variation of their 256
per-build rates — fourteen cells in one arm or the other, every one of them
bit-backend or `div_rem` at `divisor=1024`:

| Cell | Reference arm CV | Candidate arm CV |
|---|---:|---:|
| `bit_backend/and_inplace/words=1` | 8.150 % | 5.734 % |
| `bit_backend/and_inplace/words=8` | 7.041 % | 5.043 % |
| `bit_backend/not_inplace/words=1` | 9.550 % | 10.394 % |
| `bit_backend/not_inplace/words=8` | 7.568 % | 5.648 % |
| `bit_backend/or_inplace/words=1` | 11.280 % | 7.479 % |
| `bit_backend/or_inplace/words=8` | 5.592 % | 5.335 % |
| `bit_backend/xor_inplace/words=1` | 7.662 % | 10.046 % |
| `bit_backend/xor_inplace/words=16` | 11.578 % | 9.567 % |
| `bit_backend/xor_inplace/words=4` | 3.587 % | 7.793 % |
| `bit_backend/xor_inplace/words=64` | 6.817 % | 6.153 % |
| `bit_backend/xor_inplace/words=7` | 6.128 % | 9.273 % |
| `bit_backend/xor_inplace/words=8` | 5.404 % | 10.030 % |
| `bit_backend/popcount/words=8` | 4.979 % | 8.307 % |
| `polynomial/div_rem_auto/dividend=4096/divisor=1024` | 5.262 % | 2.872 % |

The tolerance is not widened for any of them and no cell is dropped. The
pooled verdict statistic carries this dispersion divided by √256: the
precision precondition measures the consequence and clears it at every cell,
5.604 at the narrowest.

## The four cells receipt-3 failed

Receipt-3's comparison — 128 alignment-axis builds per arm, unattributable by
its audit — failed four cells. All four pass here under the v2 axes at
K = 256, with the audit passing:

| Cell | Arm | Receipt-3 ρ(c) | Session-4 ρ(c) | Session-4 verdict |
|---|---|---:|---:|---|
| `bit_backend/popcount/words=1` | scalar | 1.206586 | 0.996574 | PASS |
| `bit_backend/popcount/words=8` | simd | 1.057456 | 1.002381 | PASS |
| `bit_backend/xor_inplace/words=16` | simd | 1.051039 | 0.994109 | PASS |
| `bit_backend/xor_inplace/words=1` | scalar | 1.050491 | 0.971564 | PASS |

The two comparisons measure different candidate revisions: receipt-3's
candidate predates `676f55a2`, whose DEC-G bake removed the runtime threshold
read receipt-3's Disposition names as the open question at the `popcount`
cells. The residual 0.395 ns/call and 0.210 ns/call receipt-3 measured there
are not in this record — the cells read −0.34 % and +0.24 % — so the bake is
measured to have removed the cost the three prior sessions attributed to the
selection boundary. The two `xor_inplace` cells, which stood under 0.1 %
above the bar in receipt-3 with its coverage failing at one of them, read
0.994 and 0.972 under the passing audit.

## The one cell this receipt fails

`bit_backend/not_inplace/words=1` has never failed a comparison in this
record before — and every single-build comparison the record holds reads it
*below* the baseline: 0.851509 in the first receipt's gated comparison,
0.853275 in the second's, 0.851143 in receipt-3's §7.1 draw and 0.849929 in
this session's, four independently built candidate binaries all near 0.85.
Receipt-3's alignment-axis ensemble read it at 0.951684 under a failing
audit. It fails here at 1.100028, attributably, under the first passing
audit.

What the record fixes about the cell:

- **It is the most layout-sensitive cell in the pinned set** — `s_R`
  0.093325, `s_C` 0.103409, per-build CV 9.6 %/10.4 % — and its natural pair
  moved it by nearly nothing (`σ̂` = 0.000222). Averaging 256 draws per arm is
  what makes a +10.0 % mean visible under a ±10 % per-draw spread; the
  precision margin at the cell is 5.604 and the half-split null reads
  1.001808.
- **The three unattributable sessions could not have caught it.** Receipts 1
  and 2 compared single draws (0.994, 0.943); receipt-3's ensemble read it at
  0.952 under alignment axes whose audit failed elsewhere. Whether the
  excursion predates this session's candidate revision or arrives with it is
  not decided by this record: no prior session both measured a candidate
  carrying the DEC-G bake and passed its audit.
- **What is composed of it.** The scalar `not_inplace` kernel at one word is
  the cheapest cell in the pinned set after `popcount/words=1`; +0.244
  ns/call against a 2.436 ns/call reference is the largest relative excursion
  the record now carries. The candidate compiles the `676f55a2` chain on top
  of receipt-3's candidate; the diff in `crates/` between the two is exactly
  that chain.

The diagnosis is `fc976a80`'s, not this receipt's. Its tracking issue records
what this receipt measures and what the next measured session must arbitrate.

## Falsification record

### The session establishes an attributable FAIL

The audit reports `RESULT: PASS` and the comparison `RESULT: FAIL`. Per v1
§7's second row the verdict stands: the candidate revision's cost at
`bit_backend/not_inplace/words=1` exceeds τ_cell against the baseline
revision, averaged over 256 layout draws per arm, with every attribution
precondition holding. **What this contradicts:** `50b47eae` REQ-01 requires a
receipt showing the predeclared tolerance holds for the pinned set; this
receipt shows it failing to hold at one cell. The tracked rework is
`fc976a80`, per REQ-02.

### The verdict cell's single-build reading points the other way

The committed baseline draw against this session's ordinary candidate build
reads 0.849929 at the verdict cell — outside τ_cell in the *favourable*
direction — while the ensemble reads 1.100028. Both are recorded. The
contradiction is the measurement v1 §7.1 exists to record: one draw of a cell
whose across-build dispersion is 9 % carries no evidence about the cutover,
in either direction, and the record now contains that draw twice reading
"faster" at a cell the attributable comparison reads 10 % slower.

### The set rule passes and the per-cell rule fails

The geometric mean is 1.002950 — the closest to 1 of the four sessions, and
inside τ_set for the second time. Plan §4 predicts that a fixed per-call cost
at every selection boundary shows as "a small shift shared by all thirty-four
cells rather than a large shift in one"; what this session records is the
opposite shape — thirty-three cells inside tolerance, twenty of them within
1 %, and one cell 10 % out. The set rule passing is not evidence the per-cell
excursion is absent; both rules must hold.

### Preserved

Per `@/inv/falsification-preserved` and control-arm §4.2, the failing cell is
recorded with both pooled rates, its ratio, its layout standard error and its
disagreeing single-build draw; the v2-as-written construction FAIL of the
candidate arm is preserved in the driver log, in v3 §B1 and in Deviations
below, with the verifier's superseded text kept beside its amended text in
the session record. Per plan §7 and control-arm §4.5 this session is run once
and stands as taken.

## Disposition

The session establishes the record's first attributable verdict and it is
FAIL at one cell. `50b47eae` REQ-01 remains unmet — not for want of
attribution but because the tolerance does not hold — and the epic's REQ-04
gate stays unmet with it. The excursion's rework is tracked as `fc976a80`:
diagnose the +10 % at `bit_backend/not_inplace/words=1` on the candidate
revision, fix it or put its acceptance to the epic's owner, and let the next
measured session arbitrate under the standing procedure. No predeclared value
moves; the v2 axes and the v3 reading stand for the next session unchanged.

Two things this session settles that its predecessors could not:

- **The ensemble machinery now attributes.** Four sessions in, coverage,
  precision, half-split and decorrelation all hold at once. The open question
  receipt-3's Disposition left — what axes reach `σ̂`'s floor at the two
  words=1 cells — is answered by the v2 axes measured here: 1.563× and 1.501×
  their floors.
- **The DEC-G bake removed the selection-boundary cost.** The two `popcount`
  cells that tripped every prior session read 0.996574 and 1.002381 under the
  passing audit. The residual cost receipt-3 could not attribute is measured
  here to be gone.

## What stands unchanged

- **The tolerance.** τ_cell stays 5 % and τ_set stays 2 %. Neither is widened
  to admit the excursion.
- **The pinned set.** All thirty-four cells stand. No cell is added, dropped
  or re-bracketed, including the fourteen recorded under §7.3 and the one the
  verdict trips.
- **The ensemble.** K stays 256 and the v2 §A3 enumeration stands, read
  through v3 for a candidate arm whose realized bases coincide mod 32. The
  margin of three standard errors stands.
- **The schema token.** `selector-non-regression-v1` is not bumped; the
  baseline, the three post-cutover receipts and both of this session's arms
  stay valid and comparable.
- **The standing receipts.** All four, and the pilot receipt, stand as taken.
  The baseline and the second receipt's control arm are read here only as the
  natural pair defining `σ̂`, the role v1 §8 assigns them.
- **This run.** It stands as taken.

## Deviations

Every departure from the procedure as written, however small, and what each
one does or does not touch.

- **The candidate arm's construction failed v2 §A4 as written and the session
  ran under owner-approved amendment v3.** The realized candidate bases
  separate by ≡ 0 (mod 32), collapsing the four-offset cache-line lattice to
  two and the 256 distinct page offsets to 128; one member (227) additionally
  realized a whole-page displacement. The pre-timed verification caught both,
  the driver aborted before taking the lock, and no timed execution existed
  when [`layout-attribution-verdict-v3.md`](layout-attribution-verdict-v3.md)
  was authored, approved (DEC-J) and committed. The verifier was amended to
  the v3 reading with its v2-as-written text preserved beside it
  (`verify-construction-v2.py`, SHA-256 `1f97c222…`); both arms then read
  `RESULT: PASS` and the pipeline resumed. All 512 binaries are the original
  build phase's; none was rebuilt.
- **The candidate rows record `26df4939`, not the `ec857d2f` the binaries
  were built at.** The two commits between build and timed phase are the
  epic's handoff and the DEC-J commit itself; the empty build-input diff is
  recorded under [The candidate revision](#the-candidate-revision), per the
  owner ruling receipt-3 established. Nothing committed while the timed phase
  ran, and the driver's guard verified `HEAD` and a clean worktree before
  every candidate execution.
- **The child runs at niceness 5.** The wrapper's best-effort `nice -n -5` is
  denied, exactly as receipt-3 records: the child inherits the session's
  niceness 5 where the first two receipts record 0. The lock and affinity
  remain in force, the host carries no competing work, and both arms run at
  the same niceness inside one session, so the verdict ratio carries none of
  it.
- **The session log's `locks=` line recorded empty.** `run-ensemble.sh`
  greps `/proc/locks` for the lock file's inode printed in hexadecimal, but
  `/proc/locks` prints inodes in decimal, so the probe matched nothing. The
  line is a recording defect in the session tooling, not a lock failure: the
  wrapper is `exec flock -x` on `/tmp/gf2-ccx1.lock`, which holds the lock
  for the whole child lifetime by construction, the file's `dev=2d
  inode=39210` is recorded, and the lock was verified unheld before launch.
  Receipt-3's session recorded the `FLOCK ADVISORY WRITE` line with a correct
  probe; this session's record lacks that line.
- **Builds ran in scratch target directories deleted between members.**
  v2 §A8 permits exactly this; each binary was staged and hashed before its
  target directory was removed, and `/tmp` free space never approached the
  build driver's 3 GiB floor. Unlike receipt-3, no member reused a smoke-test
  artifact: all 512 ledger rows record fresh builds of 12–15 s.
- **One unlocked throwaway execution per arm preceded the build phase.** The
  smoke test verified that a staged binary records its own checkout's
  revision and `source_dirty=false`; its two output files are throwaways in
  the session record's logs, no row of either belongs to an arm, and neither
  is copied into this directory.
- **`--self-check` and `--list-cells` were run from the staged member-0
  binaries**, the binaries that produced the rows.
- **The §7.1 extraction writes to `/tmp`**, one `head` and one `awk` over the
  committed arm file, reproducible from it.
- **The session driver is the lead-authored `continue-driver.sh`.** The
  dispatched worker's pipeline detached only its build phase; the lead
  replaced the attached remainder with a detached driver wrapping the
  worker's verified `run-ensemble.sh` and `verify-construction.py` unchanged,
  and relaunched it once after DEC-J. Both launches, the abort and the
  completion are in `continue-driver.log` in the session record.
