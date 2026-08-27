# Epic b7157be6 — Completion Report

**Epic:** `b7157be6` — Ordered-statistics decoding as the generator-matrix and syndrome soft-decision baseline
**Started:** 2026-08-24
**Closed:** 2026-08-27
**Final state:** `done`
**Execution lead:** agent:jit-execution-lead (Claude Fable 5), sessions 1–4

## Outcome

The epic delivers order-$m$ ordered-statistics decoding over any code exposing a
generator matrix, a syndrome-domain adapter that post-processes belief-propagation
failures, two complexity-reduction policies, and a receipted reproduction of the
[Fossorier1994] eBCH(128,64) order-2 BER curve.

The reproduction is the epic's scientific claim, and it holds: all seven matched
order-2 points accept their published value under the log-space acceptance
predicate, six of them agreeing within $0.081$ decades. The order-1 control
series, which the campaign asserts no published claim for, lands within $0.037$
decades of the independently digitized order-1 markers across the $3.2$ decades
of BER it spans — a strong check on the decoder, channel, seed derivation, and
interval machinery.

Evidence lives at `dev/simulation_results/osd-ebch-128-64/`: a schema-2 receipt,
its checkpoint, a machine-readable comparison, two figures in two formats, the
generating script, the producing host's `lscpu`, and a provenance record. Eight
documents are linked to `cef1ae5f`.

## Metrics

| Metric | Value |
|---|---|
| Children completed | 19 across 7 waves; no rejections, no deferrals |
| Waves dispatched | 7 |
| Sub-agent dispatches | ~40 (19 initial + 21 rework rounds) |
| Rework cycles | 21 |
| Escalations to owner | 8 (ESC-01 … ESC-08) |
| Issues created during execution | 7 (2 driven to done inside the epic, 5 left tracked) |
| Bugs found in the epic's own campaign tooling | 5, all closed before the campaign ran |

## Success-criteria mapping

- **[hard] REQ-01 — order-$m$ OSD over any generator-matrix code, hard decisions
  from LLRs, $m$ configurable.** Delivered by `5dd3539f` (ordered-column
  elimination) → `377a7a62` (canonical reliability permutation) → `6beaf008`
  (bounded pattern enumeration) → `abd48d99` (shared reprocessing engine) →
  `c97b2961` (generator-matrix decoder). Genericity is structural:
  `GeneratorMatrixOsdDecoder<C>` is bounded on `GeneratorMatrixAccess`
  (`crates/gf2-coding/src/osd/generator.rs:55`), and the order is a field of
  `OsdConfig` (`crates/gf2-coding/src/osd/patterns.rs:13`). `80cead18`,
  `ac78aff8`, and `dd6f1665` retired the superseded `SoftDecisionDecoder` trait
  and migrated ORBGRAND and Chase-Pyndiah onto the canonical reliability API,
  so one reliability surface serves every soft decoder.

- **[hard] REQ-02 — reproduces documented near-ML performance on a reference
  short code within simulation confidence intervals of published curves.**
  Delivered by `a82f2dd9` (digitized dataset) → `f0d6fb9a` (named eBCH(128,64)
  factory) → `bebe485c` (reusable campaign protocol) → `c8322eff` (campaign
  executable) → `cef1ae5f` (the campaign and its provenance record), after five
  campaign-tooling bugs were closed: `8a908f79`, `cf37be3b`, `52afe5ef`,
  `14029b3f`, and `258be082`.

- **[hard] REQ-03 — syndrome-guided OSD post-processes BP failure; combined
  BP+OSD exercised on an LDPC code in tests.** Delivered by `835e15fb` (LDPC
  posterior LLR access) → `583ec31c` (syndrome-domain adapter) → `d76ffd12`
  (mutable BP-first composition). The composition's tests cover BP success, BP
  failure with OSD success, exhausted OSD, inconsistent syndrome, and reset
  (`crates/gf2-coding/src/osd/bp_osd.rs:404-519`).

- **[aspirational] REQ-04 — complexity-reduction variants with measured
  elimination and pattern counts.** Delivered by `b3cab18e` (pattern
  segmentation policy) and `5194f470` (discard thresholds).

## What the campaign found

Two results are worth carrying forward beyond the accept verdicts.

**The 4.56 dB order-2 point is the series outlier.** The campaign estimates
$5.347\times10^{-6}$ against a published $1.995\times10^{-6}$: $+0.428$ decades,
$5.3\times$ the next largest gap. It accepts only because the $K = 100$
block-error budget buys an interval wide enough to reach the published value, so
acceptance there is a statement about resolving power rather than agreement.
This is the abscissa the dataset already records as internally contradictory
(PIT-07): Table 4.7 prints $\log_{10} P_e = -5.70$ while the Figure 4.14 marker
reads $-5.49$. The campaign sits closer to the figure, and the published series'
local slope is smooth under the figure reading and spikes under the table
reading. That is evidence about which published reading is self-consistent, not
a correction of the source; the contradiction stays recorded as a contradiction.

**A claim was made, reviewed, and withdrawn.** The record initially held that
the campaign's 5.23 dB estimate exceeding the tabulated union bound reversed the
relation a bound and a measurement should have. Research-review rejected it, and
correctly: that relation needs the bound and the estimate to concern the same
error probability under the same decoding rule, and the dataset records neither.
The record now states the observation, states the evidentiary gaps, and selects
no reading. The archived schema-1 record carries the mirror-image claim on the
same unestablished premise and is retained unedited as historical evidence.
`a1c801cb` tracks closing the gap in the dataset itself.

## Key decisions

- **Campaign tooling was fixed before the campaign ran.** Research-review
  contested `cef1ae5f`'s criteria on interval validity (ESC-04). Rather than
  argue the reading, the owner-approved plan fixed the tool, validated it with a
  reduced-compute probe, and only then ran the rigorous campaign. Five bugs
  closed; `8a908f79` alone took five review rounds.
- **The statistics were rebuilt twice under review.** The original BER intervals
  assumed bit independence under block-burst errors; the replacement Hoeffding
  construction proved vacuous at the campaign's operating points (intervals
  60–146× the estimate). The delivered construction is a negative-binomial
  BLER interval times a Maurer-Pontil empirical-Bernstein mean interval,
  combined by union bound, at 1.5–1.7× the estimate.
- **The stopping design is enforced structurally, not checked.** Counter
  equality cannot prove the last sampled block carried the $K$-th error, so the
  protocol owns the sampling loop and terminates on that block by construction.
- **Parallelism converged on the existing primitives.** The campaign ran
  single-threaded on a 24-thread host for 12 hours before the owner stopped it.
  The fix drives through `gf2_sim::parallel`'s worker-offset seek and ordered
  reduction rather than a bespoke mechanism, giving worker-count-invariant
  evidence at $12.40\times$ on 24 workers.

## Escalations

| ID | Subject | Outcome |
|---|---|---|
| ESC-01 | Bracketed citation keys must match `cites:` labels | Owner approved bracketing and label corrections |
| ESC-02 | Research-review convergence on the digitized dataset | Owner approved guidance reset, then generalized DEC-01/02 |
| ESC-03 | `b3cab18e` Background contradicted REQ-04's public surface | Owner approved rewording |
| ESC-04 | Reviewer held REQ-01/REQ-02 unmet on interval validity | Owner chose fix-tool → probe → rigorous run |
| ESC-05 | `8a908f79` exceeded the rework limit | Owner approved one guided round |
| ESC-06 | `258be082` REQ-01 and REQ-04 were mutually unsatisfiable | Owner approved scoping identity to cell evidence |
| ESC-07 | REQ-06 requires linked figures; `jit doc add` refuses binaries | Owner chose emitting SVG beside each PNG |
| ESC-08 | Serial campaign at 12 hours with hours remaining | Owner killed it and pulled the parallelism fix into scope |

## Issues created during execution

| ID | Title | Disposition |
|---|---|---|
| `c0bb2ab1` | Blocked schedule for ordered-column elimination | Tracked; convergence condition for `5dd3539f`'s named exception |
| `258be082` | Campaign protocol evaluates blocks single-threaded | Done inside this epic (wave 7) |
| `90a88fa9` | Converge campaign orchestration onto shared primitives | Tracked |
| `68189a0a` | Workspace README omits `gf2-sim`; `@/issue/` convention | Tracked |
| `a1c801cb` | Dataset does not record what the union-bound rows bound | Tracked |

`8a908f79` and `cf37be3b` were pre-existing filed bugs promoted to `cef1ae5f`
prerequisites by the ESC-04 resolution rather than created here.

## Holistic quality notes

- **Every gate failure in this epic was a real defect.** Across research-review,
  code-review, and doc-review, no finding was reviewer noise. Several were the
  lead's own: an unsatisfiable criterion pair, an unreceipted speedup claim, a
  figure omitting the published series a hard criterion required, broken links
  from an archival rename, stale cached asset metadata after that rename, and
  two overclaiming passages that asserted more than was measured.
- **A deferred contract violation is still a violation.** The
  `@/issue/<short-id>` citation form resolves for no issue, because no `issue`
  item kind is declared. The lead deferred it to a follow-up to avoid
  re-certifying passed code; the epic's doc-review then failed on it, correctly,
  because `258be082`'s REQ-06 required the named exception to be *backed by a
  tracker issue* and an unresolvable citation does not reach one.
- **BP+OSD coverage is bounded by design.** `d76ffd12`'s REQ-06 specified "a
  bounded LDPC fixture", and the composition is exercised only there. The epic's
  REQ-03 is satisfied as written. Exercising BP+OSD on a standard LDPC code at
  campaign scale is a natural follow-up that no criterion here promised.
- **Figures as committed evidence should become a project convention.** NOTE-07
  made every campaign of this kind commit result figures generated
  deterministically from the receipt by a committed script and link them to the
  issue. Applied here as REQ-06, it caught a legend that named one source for a
  series drawn from two. Recommended for adoption beyond this epic.
- **Deterministic figure output needs explicit pinning.** SVG carries a creation
  date and salted element identifiers by default; both must be fixed before
  "regenerates byte-identically" is true.
- **Moving a linked document silently invalidates its tracker metadata.**
  Repairing the file's links does not refresh the stored asset discovery; the
  document must be re-linked.

## References

- [Fossorier1994] Fossorier — Decoding of Linear Block Codes Based on Ordered
  Statistics. Ph.D. dissertation, University of Hawai'i at Manoa, December 1994.
