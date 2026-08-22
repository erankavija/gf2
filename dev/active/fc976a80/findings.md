# Findings: the receipt-4 excursion at `bit_backend/not_inplace/words=1`

Diagnosis for issue `fc976a80` REQ-01, performed by the epic's execution lead
on 2026-08-22, entirely from the committed session-4 record — the two arm
CSVs, the build ledgers and the member provenance TSVs of
[`2026-08-22-post-cutover-receipt-4.md`](/dev/benchmarks/tuning_profiles/2026-08-22-post-cutover-receipt-4.md)
— plus two probe builds. No timed measurement was taken; every number below
recomputes from committed rows by the harness's own pooling (sum of
`elapsed_ns` over sum of `calls`).

## Verdict of the diagnosis

**The candidate revision carries no measurable code cost at this cell.** At
matched placement the two arms agree to a tenth of a permille. The receipt's
+10.0 % pooled ratio is an ensemble-construction artifact: under amendment
v3's realized lattices the two arms sample *different* placement
distributions, at the one cell in the pinned set whose cost swings ~29 % with
placement, and the layout term that v1 §6.3 assumes averages out of the
verdict ratio does not cancel between unlike distributions.

## The decomposition

Pooled ns/call at `bit_backend/not_inplace/words=1`, by G level and realized
`.text` cache-line offset (from the ledgers: in the reference arm G=0 lands
at offsets {0,32} and G=1 at {16,48}; in the candidate arm both G levels land
at {0,32}, per v3 §B3; within each arm, even E → one offset of the pair, odd
E → the other):

| Stratum | Reference arm | Candidate arm | Cross-arm ratio |
|---|---:|---:|---:|
| G=0, offset 0 | 2.8602 (64 members) | 2.8606 (64) | **1.000124** |
| G=0, offset 32 | 2.4300 (64) | 2.4299 (64) | **0.999960** |
| G=1, offset 16 | 2.4353 (64) | — | — |
| G=1, offset 48 | 2.2118 (64) | — | — |
| G=1, offset 0 | — | 3.0721 (64) | — |
| G=1, offset 32 | — | 2.4345 (64) | — |
| **Pooled (the verdict)** | **2.436132** | **2.679814** | **1.100028** |

Three facts carry the whole excursion:

1. **The cell's cost is strongly placement-dependent in both revisions** —
   2.21 to 3.07 ns/call across strata, a ~29 % swing, by far the widest in
   the pinned set (its `s_R` = 0.093325 is the set's largest; its natural
   pair moved it by σ̂ = 0.000222, so receipt-2's control rebuild happened to
   reproduce the baseline's placement class).
2. **At matched strata the arms are identical.** G=0 members are built with
   identical flags from the two revisions and land on the same offsets; the
   ratios are 1.000124 and 0.999960. There is nothing to fix in `crates/`.
3. **The G=1 strata are unlike across arms and their costs differ by
   placement, not by code.** The reference's dead-code members land at
   offsets {16,48} and run fast (2.44/2.21); the candidate's land at {0,32},
   fast at 32 (2.4345, equal to G=0 there) and slow at 0 (3.0721). Averaging
   fast-placed reference G=1 against slow-placed candidate G=1 produces the
   pooled 1.100.

The matched-offset pooled comparison (reference members at offsets {0,32} —
its G=0 half — against the whole candidate arm) reads **1.019873**, inside
τ_cell; the per-offset ratios are 1.037 at offset 0 and 1.002 at offset 32,
and the residual at offset 0 is the G=1-at-offset-0 placement effect above,
not a G=0 code effect.

**No other cell's verdict is construction-dominated, and no post-hoc
estimator is clean.** The finding at the verdict cell is robust across every
alternative reading — 1.019873 under the matched-offset pooling, 1.000158
under a G=0-vs-G=0 pooling, 1.000124/0.999960 per matched stratum — but the
alternative readings disagree among themselves at marginal cells elsewhere:
`polynomial/mul_fast/len=32` (1.031653 under the predeclared estimator,
inside τ_cell) reads 1.057538 under matched-offset and 1.053281 under
G=0-vs-G=0, both marginally outside, on half-sized samples that drop the
arrangement axis from one or both sides. This is itself a finding: no
estimator chosen after seeing these numbers can carry a verdict — v1 §7.6
and `@/inv/falsification-preserved` forbid exactly that — so the remedial
estimator must be predeclared before the next measured session and take its
verdict from that session's own data.

## Why the audit could not catch this

v1 §6.4's four preconditions test dispersion magnitude (coverage), estimator
noise (precision), within-arm half consistency (half-split null) and
across-arm independence (decorrelation). All four pass, correctly: each arm's
ensemble is a genuine, well-dispersed, internally consistent sample of *its
own* placement lattice. What no precondition tests is whether the two arms
sample the *same* placement distribution — v2 §A4's construction made that
true by design ("the two multisets … are equal" across halves, and one
enumeration for both arms), and it held physically until the candidate
revision's base residue collapsed the lattice. Amendment v3 §B3.4 said the
narrowed lattice's consequences would be "decided by the session's own
audit"; that was wrong in one respect this record now measures — the audit
decides dispersion, not cross-arm distributional equality, and the verdict
estimator inherited the asymmetry silently.

## Why the asymmetry cannot be built away by padding

Both revisions round a 16-byte translation to nothing — probe builds with a
`--build-id` payload lengthened by 16 bytes reproduce the unpadded `.text`
address exactly, on the candidate (v3 §B2, 0x221e0) and on the reference
(this diagnosis, 0x1f5d0) — because a 32-byte-aligned section (`.rodata`)
sits between the build-id note and `.text` in both binaries. Translation can
only move `.text` in 32-byte steps; the 16-mod-32 residue of an arm's G=1
base is a fact of that revision's section content between `.rodata` and
`.text`, and the two revisions' facts differ (16 for the reference, 0 for the
candidate). Neither arm's G=1 lattice can be padded onto the other's.

## Disposition under the issue's criteria

- **REQ-01 (cause identified with evidence): met by this document.** The
  cause is a specific, measured property of the session-4 ensemble
  construction — unlike placement distributions across arms at a
  placement-sensitive cell — not a code or codegen property of the candidate
  revision. The matched-stratum ratios of 1.000124 and 0.999960 are the
  evidence that no code cost exists to remove.
- **REQ-02 (fix or owner acceptance): goes to the owner**, because no
  `crates/` fix exists to land, and every remedial path amends the frozen
  measurement procedure or the epic's criteria, which the lead does not touch
  without approval. The receipt stands as taken either way; nothing here
  recomputes or reopens it.
- **REQ-03 (other cells intact, session 5 arbitrates):** no code change is
  proposed, and the matched-offset sweep above records that no other cell's
  verdict is construction-dominated.
