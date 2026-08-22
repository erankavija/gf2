# Findings: the receipt-5 excursion at the `mul_fast` dispatch cells

Diagnosis and fix for issue `eb9b324c`, performed on 2026-08-22 from the
committed session-5 record — the two arm CSVs, the build ledgers and the member
provenance TSVs of
[`2026-08-22-post-cutover-receipt-5.md`](/dev/benchmarks/tuning_profiles/2026-08-22-post-cutover-receipt-5.md)
— the staged session-5 member binaries, a `perf` mechanism probe over four of
them, and a sixteen-member pilot translation ensemble of the fixed candidate.
It continues the committed two-site brainstorm
([`diagnosis-brainstorm.md`](diagnosis-brainstorm.md)), verifying and narrowing
it rather than repeating it. Every rate below recomputes by the harness's own
pooling (sum of `elapsed_ns` over sum of `calls`). No receipt is recomputed,
reopened or superseded, no predeclared value of the frozen procedure moves, and
no measured session of the pinned set was run.

## Verdict of the diagnosis

**The excursion is a code property of the candidate revision, at one site, and
it is removable.** The cutover split the sub-NTT arm of `mul_fast` into a
four-region call chain where the reference revision had two, and that chain
carries a per-call term of **+93.4 ns** at `polynomial/mul_fast/len=32`,
phase-averaged over the two 64-byte fetch-block phases. The term is not the
extra instructions' own execution — their retired cost is 7 to 13 cycles per
call — it is the placement-dependent delivery cost of a path that touches twice
as many code regions per call, which is also why its size swings by a factor of
three between the two phases and why the cell's translation sensitivity rose
6.5×. Collapsing the chain removes both the mean excursion and the sensitivity:
in a sixteen-member pilot ensemble of the fixed revision the cell reads
**0.9972** against the session-5 reference arm at matched page offsets, with a
bit5 = 0 over bit5 = 1 phase ratio of **0.9994** against the pre-fix
**1.0800**.

## 1. The property: what the cutover did to the dispatch path

Static, from the session-5 ordinary member binaries (reference
`/tmp/gf2-ens5/ref/member-0.bin` at `0c072d73`, candidate
`/tmp/gf2-ens5/cand/member-1.bin` at `1a5812c2`); the extracts are in
[`perf/static-dispatch-disasm.txt`](perf/static-dispatch-disasm.txt).

**Reference — two regions, one transfer.** `mul_fast` (`0x512c0`, 0x531 bytes)
resolves the whole sub-NTT decision inside its first 0x48 bytes: two empty
tests, `cmp $0x82` against the compiled-in NTT gate, two `cmp $0x21` against the
compiled-in Karatsuba threshold, then `call mul_schoolbook_impl` at `0x51308`.
One prologue, one epilogue, no profile load.

**Candidate — four regions, three transfers.** `mul_fast` (`0x5f8f0`, 0x53e
bytes) loads the `tuning::ACTIVE` `OnceLock` state word at `+0x2a`, branches
past the initializer, compares `karatsuba_max_out_len`, and takes a forward
branch to `+0x49f`, 1,114 bytes ahead; that block pops all six callee-saved
registers and tail-jumps to `mul_impl` (`0x5fe30`, 0x133 bytes); `mul_impl`
pushes the same six registers back, **repeats both empty checks**, loads the
`OnceLock` state word a **second** time, loads `karatsuba_min_degree`, and
tail-jumps 4,784 bytes back to `mul_schoolbook_impl` (`0x5ec60`) from `+0xd2`.
The Karatsuba arm of `mul_impl` loads the state word a **third** time and
reloads the threshold before calling `mul_karatsuba_raw`.

Per sub-NTT `mul_fast` call the cutover therefore added: two taken control
transfers, two hot code regions, two `OnceLock` state loads, one redundant pair
of empty checks and one redundant prologue/epilogue pair.

## 2. The measured decomposition

From the committed session-5 rows, pooled per arm and per realized `.text`
64-byte phase (bit 5 of the page offset), full 128-member arms; the table is
[`perf/session5-dispatch-decomposition.tsv`](perf/session5-dispatch-decomposition.tsv).

| Cell | ref b5=0 | ref b5=1 | ref pooled | cand b5=0 | cand b5=1 | cand pooled | ρ |
|---|---:|---:|---:|---:|---:|---:|---:|
| `mul_fast/len=32` | 1189.04 | 1189.45 | 1189.24 | 1302.81 | 1217.27 | 1258.54 | **1.058269** |
| `mul_fast/len=64` | 3793.81 | 3813.64 | 3803.70 | 4142.38 | 3857.33 | 3994.85 | **1.050254** |
| `mul/len=32` | 1245.65 | 1221.08 | 1233.01 | 1219.23 | 1202.25 | 1210.74 | 0.981939 |
| `mul/len=33` | 1147.26 | 1144.81 | 1146.03 | 1205.22 | 1133.14 | 1168.05 | 1.019216 |
| `mul/len=64` | 3891.12 | 3819.38 | 3854.72 | 4156.50 | 3915.45 | 4032.50 | 1.046118 |
| `mul/len=256` | 36690.21 | 36763.95 | 36727.14 | 39593.95 | 37352.40 | 38442.00 | 1.046692 |

**Isolating the wrapper.** `mul_fast/len=L` and `mul/len=L` run the same operand
sizes over the same kernels; their difference is the `mul_fast` entry path
alone. In ns/call:

| len | arm | b5=0 | b5=1 | phase mean |
|---|---|---:|---:|---:|
| 32 | reference | −56.61 | −31.63 | −44.12 |
| 32 | candidate | +83.58 | +15.02 | **+49.30** |
| 64 | reference | −97.32 | −5.73 | −51.53 |
| 64 | candidate | −14.13 | −58.12 | −36.12 |

The cutover moved the len=32 wrapper term by **+93.42 ns/call** phase-averaged
(+140.19 at b5=0, +46.65 at b5=1) and the len=64 term by only +15.41. That is
the whole answer to why the two `mul_fast` cells fail and their neighbours do
not, and to REQ-01's question about the direct `mul` cells:

- `mul/len=32` is schoolbook-only and its path was not restructured — one
  dispatcher, one profile read, unchanged region count. It reads 0.981939.
- `mul/len=64` and `mul/len=256` **do** carry a mean excursion, 1.046118 and
  1.046692, from a second, arrangement-borne term shared with the whole
  Karatsuba family (the brainstorm's Site B). It sits just under τ_cell = 1.05.
- Only the two `mul_fast` cells carry the wrapper term on top of that, and only
  they cross the bar. At len=32 the wrapper term is the entire excursion; at
  len=64 it is 15.4 of the 191.1 ns, the rest being the shared Karatsuba term.

**Mean excursion versus sensitivity are the same term seen twice.** The wrapper
term is 140.19 ns at one fetch-block phase and 46.65 at the other; a
translation ensemble averages the two, so the cell shows both a mean excursion
(the average, +93.4) and a tripled phase response (the difference, 93.5 ns of
2-periodic swing) from one cause. The reference's own phase response at that
cell is 0.41 ns.

## 3. What the mechanism probe establishes, and what it does not

E2 ran the four staged session-5 members of opposite phase — candidate `j=1`
(E=0, page offset 2208, bit5=1, the fast phase) and `j=3` (E=1, offset 2240,
bit5=0), reference `j=0` (E=0, offset 3008, bit5=0) and `j=1` (E=1, offset 3040,
bit5=1) — under `perf stat` (two event sets, two repetitions each) and
`perf record` (`cycles:u` and `ex_ret_brn_misp:u` at 9,999 Hz), all inside one
`ccx1-bench-flock.sh` session on the idle host, each binary run from its own
checkout. Script and log:
[`perf/e2-perf.sh`](perf/e2-perf.sh), [`perf/e2-run.log`](perf/e2-run.log).

**Whole-suite counters are null by construction, and this is a finding about
the protocol, not about the code.** The harness gives every cell a fixed
`--target-ms 250` budget, so a slower cell performs fewer calls and the run's
totals barely move: `cycles:u` reads 50.20 G on the fast-phase candidate member
against 49.69 G on the slow-phase one, `instructions:u` 203.5 G against 200.6 G
([`perf/whole-suite-counters.tsv`](perf/whole-suite-counters.tsv)). No
whole-binary counter ratio can carry a claim about one cell here — not because
the events are wrong, but because the estimand is wrong. The brainstorm's
recorded protocol gap is therefore sharper than stated: whole-suite counters
cannot *support* the mechanism either, and none of the counter ratios above is
used as evidence anywhere in this document.

**Symbol-level attribution rules out the obvious mechanism.** With ~110 k
samples per member ([`perf/symbol-cycles.tsv`](perf/symbol-cycles.tsv)), the
dispatch code's own retired cycles are small: the whole `mul_impl` symbol takes
1.81 M cycles on the fast-phase member and 3.18 M on the slow one, over roughly
271 k and 251 k entries — **6.7 and 12.7 cycles per call** — against ~49.9 G
cycles in the run. At the measured clock (49.86 G user cycles over 11.0 s of
user CPU, ≈ 4.5 GHz) the wrapper term of 93.4 ns is ≈ 420 cycles. The extra
instructions' execution is therefore *not* the cost; the cost is the delivery
of a path that touches four code regions instead of two, paid once per entry
into the chain and modulated by the fetch-block phase of those regions.

**What remains unresolved.** Which front-end structure carries it — op-cache
set conflict, L1i fetch-block splitting, BTB pressure, or a combination — is
**not** established. `perf record` attributes samples to symbols shared across
several cells (`mul_fast` also holds the inlined `mul_ntt` body; the poly cells
share `mul_schoolbook_impl`), and no per-cell filter exists in the harness. The
claim this document makes is at the region-count and delivery-cost level, which
the static structure and the timing decomposition both support directly. The
run-level reproduction is recorded in
[`perf/reproduced-cell-rates.tsv`](perf/reproduced-cell-rates.tsv): under
`perf stat`, the slow-phase candidate member reads 1281.4–1327.5 ns/call at
`mul_fast/len=32` over its four runs against 1191.8–1261.8 for the fast-phase
member, while the two reference members read 1185.8–1190.3.

## 4. The fix

`perf(jit:eb9b324c): collapse the mul_fast double dispatch`, in
`crates/gf2-core/src/field/poly.rs` only. One internal dispatcher,
`mul_dispatch`, takes an already-resolved `karatsuba_min_degree`. `mul_impl`
short-circuits the empty operands, resolves the active profile once and calls
it; `mul_fast` resolves once, applies the NTT gate, and calls the same
dispatcher instead of `FieldPoly::mul`. The public selectors `mul_route` and
`mul_fast_route` keep their signatures and delegate to resolved-threshold
helpers, so each routing predicate stays single-sourced and `mul_fast` owns no
routing logic beyond the NTT gate.

Installed-profile semantics are unchanged: `TuningProfile::install()` still
governs both thresholds, `TuningProfile::CONSERVATIVE` is still the default, and
resolving `tuning::active()` once per call rather than three times is identical
because it returns the same `&'static TuningProfile` for the life of the
process. `mul_karatsuba_raw` and its threshold parameter are untouched.

The realized codegen drops `mul_impl` and `mul_dispatch` as separate symbols,
leaves one `OnceLock` state load in place of three, and tail-jumps from
`mul_fast` straight into `mul_schoolbook_impl`: **three hot regions per sub-NTT
call instead of four**. It does not reach the reference's two, because LLVM
sinks the sub-NTT arm to `mul_fast+0x4a0` behind the inlined NTT body; an
early-return source form was tried and produces byte-identical code, so the
placement is a block-placement decision, not a source-shape one.

`./scripts/cargo-ci.sh` passes in full (check, test, clippy, fmt, baked), as do
`cargo nextest run -p gf2-core --release --profile ci` (2,107 passed, 16
skipped), the doctests (554 passed) and
`cargo clippy --workspace --all-targets --all-features -- -D warnings`.

## 5. Pilot verification (REQ-02 evidence)

Sixteen members of the fixed candidate, E = 0…15, built with session-5's own
`build-arm.sh` RUSTFLAGS construction (`-Wl,--build-id` payload of `20 + 32·E`
bytes), each in a scratch `CARGO_TARGET_DIR` outside the checkout deleted
between members. All sixteen built on the first attempt, all sixteen SHA-256
distinct, `.text` at `0x1e840` + 32·E, page offsets 2112…2592 in 32-byte steps,
eight members at each 64-byte phase
([`pilot/ledger-cand-fixed.tsv`](pilot/ledger-cand-fixed.tsv)). One execution
per member, `--target-ms 250`, one `ccx1-bench-flock.sh` session holding
`/tmp/gf2-ccx1.lock` for all sixteen (2026-08-22 15:32:31–15:35:26 UTC), CPUs
6–11, `powersave`, one-minute load 0.37 at the start, working directory the repo
root, output written to `/tmp/gf2-eb9b324c-pilot.csv` checked absent beforehand
([`pilot/timed.log`](pilot/timed.log),
[`pilot/pilot-cand-fixed.csv`](pilot/pilot-cand-fixed.csv)).

Because the fixed revision's base `.text` moved from `0x1e8a0` to `0x1e840`, E
no longer maps to the same absolute placement as in session 5. The comparison
below is therefore taken at **matched page offsets**: the sixteen offsets
2112…2592 that the pilot realizes, restricted to the same sixteen offsets in
each committed session-5 arm — identical L1i sets, identical fetch-block phases,
eight members per phase everywhere
([`pilot/pilot-comparison.tsv`](pilot/pilot-comparison.tsv), section A).

| Cell | ref-5 window | cand-5 window | pilot | cand-5/ref-5 | **pilot/ref-5** |
|---|---:|---:|---:|---:|---:|
| `mul_fast/len=32` | 1194.0 | 1256.6 | 1190.7 | 1.052368 | **0.997219** |
| `mul_fast/len=64` | 3800.1 | 3994.9 | 3802.3 | 1.051272 | **1.000586** |
| `mul/len=64` | 3856.9 | 4042.0 | 3836.7 | 1.048000 | 0.994754 |
| `mul/len=256` | 36949.8 | 38551.9 | 36738.6 | 1.043361 | 0.994286 |

Against the **full 128-member** reference arm the same pilot values read
1.001235, 0.999631, 0.995315 and 1.000313.

Ratio of the two 64-byte fetch-block phases, bit5 = 0 over bit5 = 1, same
window; a value above 1 means the bit5 = 0 phase is the slower one, and every
"parity split" named elsewhere in this document is this ratio:

| Cell | reference | candidate (pre-fix) | **pilot (fixed)** |
|---|---:|---:|---:|
| `mul_fast/len=32` | 1.0087 | 1.0800 | **0.9994** |
| `mul_fast/len=64` | 0.9957 | 1.0819 | **1.0043** |
| `mul/len=64` | 1.0216 | 1.0684 | 0.9947 |
| `mul/len=256` | 1.0101 | 1.0699 | 0.9967 |

Both success signals the issue names are met at pilot scale: the parity split at
`mul_fast/len=32` collapses from 8.0 % to 0.06 %, below the reference's own
0.9 %, and the pooled values sit within 0.3 % of the reference at both
`mul_fast` cells, against a ~2 % target.

**Host drift is bounded.** Re-running the staged reference members `j=0` and
`j=1` today reproduces their session-5 executions to 1.0002 and 0.9989 at
`mul_fast/len=32` and 1.0002 and 1.0010 at `mul_fast/len=64`, so the
cross-session ratios above are not carrying a drift artifact.

### Limits of this evidence

- Sixteen translations of one arm, not 128 of two. It is pilot-scale evidence
  under the standing lock discipline, exactly what REQ-02 asks for, and it is
  **not** a receipt, not a verdict, and not run through the comparison checker.
- The pilot samples sixteen of the page's 128 offsets — eight L1i-set pairs of
  64, balanced across the fetch-block phase but not across the set index.
- One execution per member, so per-member noise is uncancelled; the `mul_fast`
  cells' per-member spread is 1186.0–1215.6 ns/call, with the single 1215.6
  draw at the last execution.
- The candidate members were built from a checkout carrying untracked session
  artifacts, so the pilot's rows record `source_dirty=true`. No tracked file was
  modified during the build or the timed phase, and the run guard verified
  `HEAD` was `dd3eee59` throughout.

## 6. What the pilot also shows: the sensitive term relocates

The fixed revision's sharpest translation response is no longer at the
`mul_fast` cells. It is at the two direct schoolbook cells, and it is sharp and
systematic across all sixteen members:

| Cell | ref-5 window | pilot | pilot/ref-5 | pilot b5=0 | pilot b5=1 | b5=1 / ref-5 b5=1 |
|---|---:|---:|---:|---:|---:|---:|
| `mul/len=16` | 319.3 | 330.3 | 1.0344 | 323.2 | 337.3 | 1.0668 |
| `mul/len=32` | 1231.5 | 1275.3 | 1.0356 | 1237.5 | 1313.0 | 1.0769 |

Per member the two phases do not overlap at all: 321.6–327.6 against 336.8–338.1
at len=16, and 1229.5–1250.4 against 1311.6–1314.8 at len=32
([`pilot/pilot-comparison.tsv`](pilot/pilot-comparison.tsv), section E and the
per-member rows). The pre-fix candidate's split at `mul/len=32` was 1.0149 and
the reference's 1.0206, so this response is new with the fixed build.

Both cells stay inside τ_cell = 1.05 at the phase-balanced pooled value that v4
estimates, and the pilot's geometric mean over all thirty-four cells is 0.994082
against the reference window (the pre-fix candidate reads 1.000488 there). But
at the unfavourable phase alone they read 1.067 and 1.077, and the honest
reading of this pilot is that the fix removes the two failing cells' excursion
while an arrangement-borne term relocates to two cells that were previously
quiet. That is a fact about this revision's arrangement, not a defect the fix
introduced into any algorithm: no code on the `FieldPoly::mul` path changed
except that `mul_impl`'s body now inlines into its callers, and
`mul_schoolbook_impl` moved from `0x5ec60` to `0x5ead0`.

Section F of [`pilot/pilot-comparison.tsv`](pilot/pilot-comparison.tsv) carries
all thirty-four cells for completeness. It is a pilot-scale diagnostic over
sixteen translations of one arm and establishes no verdict; the fourteen
`bit_backend` cells in particular are the set's most placement-sensitive and are
under-sampled at this K.

## 7. Site B: the evidence, and why no change is proposed

The brainstorm's Site B — `mul_karatsuba_raw`'s threshold becoming a register
parameter, growing the function 0x911 → 0x941 bytes — remains untouched, and the
pilot argues it should stay that way. In the fixed build the Karatsuba cells'
parity split collapses as well (`mul/len=64` 1.0684 → 0.9947, `mul/len=256`
1.0699 → 0.9967, `mul/len=33` 1.0597 → 0.9835) although `mul_karatsuba_raw`'s
code is byte-for-byte what it was; only its address moved, from `0x5b610` to
`0x5b480`. The recursive parity term therefore tracks the function's placement,
not its parameterization, and an immediate-threshold specialization would trade
installed-profile semantics for a layout outcome that a relink already changes.

The minimal change, if the owner ever wants it, is a `const`-generic or
compile-time-baked threshold on the recursion body with the profile-driven body
retained for installed profiles — a second dispatch path, which
`@/inv/convention-convergence` makes a defect without a named, cited exception
and a tracked convergence condition. This document does not propose it. What
Site A's fix cannot explain is stated plainly in §2: at len=64 the wrapper term
is only 15.4 of the 191.1 ns pooled excursion. The pilot nonetheless reads
1.000586 there, because the remaining Karatsuba term is arrangement-borne and
this arrangement does not carry it. That relief is the least durable claim in
this document.

## 8. Disposition under the issue's criteria

- **REQ-01 (cause identified with evidence): met by §§1–3.** The cause is a
  codegen property of the candidate revision — the cutover's four-region,
  three-transfer, three-`OnceLock`-load sub-NTT dispatch chain — measured as a
  +93.42 ns/call phase-averaged wrapper term at `mul_fast/len=32` isolated
  against `mul/len=32` in the same builds. The mean excursion and the 6.5×
  sensitivity increase are the two moments of one phase-dependent term
  (+140.19 ns at one fetch-block phase, +46.65 at the other). The direct `mul`
  cells do not carry it because they never enter the chain: `mul/len=32` reads
  0.981939, and the direct Karatsuba cells' 1.046 excursion is the separate,
  arrangement-borne term that also sits under the two `mul_fast` cells. The
  front-end structure that carries the delivery cost is **not** resolved;
  symbol-level attribution establishes only that the dispatch instructions'
  own retired cost is 7–13 cycles per call, an order of magnitude below the
  ≈ 420-cycle term.
- **REQ-02 (fix with pilot-scale bench evidence): met by §§4–5.** The fix lands
  in `crates/gf2-core/src/field/poly.rs`; the sixteen-member translation pilot
  under the standing lock discipline reads 0.997219 and 1.000586 at the two
  failing cells against the session-5 reference arm at matched page offsets,
  with the `mul_fast/len=32` parity split collapsed from 1.0800 to 0.9994. No
  predeclared value of the frozen procedure moves.
- **REQ-03 (other cells intact; no unapproved measured session): met.**
  `./scripts/cargo-ci.sh` passes in full; nothing outside the one source file
  changed; nothing was written under `dev/benchmarks/tuning_profiles/`; the
  pinned-set comparison was not run and no verdict is claimed. §6 records, at
  pilot scale, the one place where the fixed revision's behaviour has moved
  materially — `mul/len=16` and `mul/len=32`, inside τ_cell pooled and outside
  it at one phase — so the next session's owner is not surprised by it.

## 9. Recommended next step

A next measured session under the standing v4 procedure, unchanged, on the
fixed revision, at the owner's decision. Nothing here is a substitute for it:
the pilot is one arm over sixteen translations and its two weakest claims — the
Karatsuba relief of §7 and the relocated sensitivity of §6 — are exactly the
ones a 128-member two-arm ensemble is built to arbitrate. Two things are worth
the owner's attention before dispatching it: the fixed revision's base `.text`
address is `0x1e840`, ≡ 0 (mod 32), so v4 C3's base-congruence precondition
against the reference's `0x1cbc0` still holds; and `polynomial/mul/len=16` and
`polynomial/mul/len=32` should be read with the same care receipt 5 gave the
`mul_fast` cells.
