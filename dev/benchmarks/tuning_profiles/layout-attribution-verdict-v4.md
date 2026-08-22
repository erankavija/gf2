# Layout-attribution amendment v4: the content-independent translation ensemble

This document replaces the ensemble construction for the next measured
post-cutover session of epic `6dc81018`, and nothing else. It is filed by the
epic's execution lead under owner decision DEC-K of 2026-08-22, and it is
committed before any session under it exists, so the rule cannot be shaped by
the numbers it governs.

[`layout-attribution-verdict-v1.md`](layout-attribution-verdict-v1.md),
[`layout-attribution-verdict-v2.md`](layout-attribution-verdict-v2.md) and
[`layout-attribution-verdict-v3.md`](layout-attribution-verdict-v3.md) stand
as written; **this document changes none of their text**. What the next
session reads through this document is the enumeration of v2 §A3, its K, and
v2 §A4's verification; everything the standing documents fix that this
document does not name is what governs. τ_cell stays 5 %. τ_set stays 2 %.
The pinned set keeps all thirty-four cells. The schema token
`selector-non-regression-v1` is not bumped. The attribution margin stays
three standard errors. `--target-ms 250`, v1 §5.1's sampling, v1 §5.2's
ordering, v1 §5.4's recording and v1 §6's statistics, rules, preconditions,
audit and consequences all stand unmodified.

## C1. What sends this document here

[`2026-08-22-post-cutover-receipt-4.md`](2026-08-22-post-cutover-receipt-4.md)
records the procedure's first attributable verdict — FAIL at
`bit_backend/not_inplace/words=1`, ρ = 1.100028 — and the diagnosis of issue
`fc976a80` ([`findings.md`](/dev/active/fc976a80/findings.md)) decomposes it:
at matched placement strata the two arms agree to 1.000124 and 0.999960, and
the pooled excursion arises because the arms sampled unlike placement
distributions at the pinned set's most placement-sensitive cell. The receipt
stands as taken; nothing here recomputes or reopens it.

The root cause is that axis G (`-C link-dead-code`) realizes placement that
**depends on the revision being measured**: the reference revision's
dead-code members land at cache-line offsets {16, 48} and the candidate
revision's at {0, 32}, and v3 §B2 with the `fc976a80` probes establish that
neither can be padded onto the other — a 16-byte translation rounds to
nothing on both revisions. No content-dependent axis can guarantee that two
arbitrary revisions realize matching lattices. Axis E, the build-id
translation, is the opposite: its placement is arithmetic — 32-byte steps
from the arm's own ordinary-build base — identical in form on every revision,
and the pilot receipt verified it moves the image and provably nothing else.
This amendment therefore removes G and builds the ensemble from translation
alone, with the cross-arm distributional equality that v2 assumed by design
stated as an explicit precondition and verified before any timed window.

## C2. The enumeration

K = **128** members per arm. Each arm's enumeration is fixed by one measured
constant of that arm: the `.text` address of its ordinary build, `P`, read
from the staged member binary. Define

    φ(arm) = 1 if (P mod 64) ≥ 32, else 0

    h₀ = the 64 values E in 0…127 with popcount((E + φ) mod 128) even, ascending
    h₁ = the 64 values E in 0…127 with popcount((E + φ) mod 128) odd, ascending

    E(2i) = h₀[i]      E(2i + 1) = h₁[i]      for i = 0…63

Member `j` (0…127) builds with
`-C link-arg=-Wl,--build-id=0x<hexadecimal string of 2·(20 + 32·E(j)) zeros>`
— v2 §A3's axis E exactly, a level of 0 omitting the option so the member
with E = 0 is the ordinary build — and records `--execution j+1`. Every
split runs over `j`, as v1 §3.2 requires.

φ exists because the map from E to the L1 instruction-cache set index depends
on the arm's base phase: when `P mod 64 < 32` the translations `2s` and
`2s+1` share a set, and when `P mod 64 ≥ 32` the carry moves the sharing to
`2s−1` and `2s`, with E = 0 and E = 127 sharing a set modulo the page. The
bit-parity (Thue–Morse) assignment over `(E + φ) mod 128` splits every
sharing pair across the two member-index-parity halves in both regimes. φ is
a measured build-time fact of each arm, recorded in the ledger, and the two
arms' φ may differ; what must agree across arms is the distribution each arm
samples, which C3 states and the verification checks.

**Balance, for every value of P** (verified by enumeration over all phase
classes before this document was committed, and re-verified on the realized
ledgers before any timed window):

- The 128 members realize 128 distinct binaries at 128 distinct `.text` page
  offsets — every multiple of 32 in the page when `P ≡ 0 (mod 32)` — under
  the page-offset translation law of v3 §B3.1.
- Each member-index-parity half holds 64 members: 32 at each of the two
  realized cache-line offsets, 32 at each parity of E, and **each half covers
  all 64 L1i sets exactly once**; the two halves' offset and set multisets
  are equal.

## C3. The cross-arm precondition

What receipt-4 measured is that within-arm balance is not enough: the verdict
ratio is unbiased only if **the two arms sample the same placement
distribution**. This amendment states that as a precondition of the timed
phase, verified from the build ledgers, with the session aborting before any
timed window on failure:

1. **Base congruence.** The two arms' ordinary-build `.text` addresses agree
   modulo 32: `P_ref ≡ P_cand (mod 32)`. This makes the two arms' page-offset
   sets equal (both enumerate the same 128 residues) and their cache-line
   offset sets equal. It is checkable from two builds before either arm's
   build phase, and the session driver checks it there as well as at the full
   verification.
2. **Distributional equality.** The two arms' multisets of realized
   cache-line offset and of L1i set index over all 128 members are equal, and
   both arms' page-offset sets are equal as sets.
3. **Within-arm balance.** Each arm passes C2's balance properties, read as
   v3 §B3 reads v2 §A4 (page-offset law with whole-page displacements
   recorded; distinct placements; half matching; axis balance over E).

A failure of 1 or 2 is a construction failure of this amendment on the
revision pair, and it goes to the owner of epic `6dc81018` before any timed
window — there is no in-session remedy, because the base residue is a fact of
each revision's section content.

## C4. Sizing

**Precision.** v1 §3.4's rule sizes K against
`3 · √(s_R² + s_C²) / √K ≤ ln(1.05)` per cell. Session 4 measured both
dispersions at every cell under a 256-member ensemble whose axes included
G; at K = 128 its measured margins scale by √2, and the narrowest becomes
5.604 / √2 = 3.96 at `bit_backend/not_inplace/words=1`, above the margin of
three, with every other cell wider. The translation-only ensemble's own
dispersion cannot be assumed equal to the measured one — removing G removes
arrangement variance — but a smaller dispersion widens the precision margin
further, and the precondition is computed from the session's own rows in
any case.

**Coverage.** Removing G risks under-covering `σ̂` where natural rebuild
variation is arrangement-driven. The committed evidence is pilot 1
(v2 §A6): its thirty-two-member group varying **the translation axis alone**
clears every one of the thirty-four coverage floors. A 128-member translation
ensemble samples the same axis at four times that size. If coverage
nonetheless fails, v1 §6.6's coverage branch governs unchanged: the session
reports which cells fail and by how much, establishes no verdict, and the
axes return to the owner as a tracked amendment.

**K = 128 and the ladder.** v2 §A7 sized K = 256 for a two-axis ensemble
under a conservative pre-session reading; this enumeration provides exactly
128 distinct placements — the page's capacity at 32-byte granularity — and
128 is a size v1 §6.5's audit accepts. A precision or half-split-null
failure at this K routes per v1 §6.6 as written (K doubles only by adding an
axis, which after this record is the owner's question, not a mechanical
step).

## C5. The session

- 256 builds (~1 h of compiler time at the measured 12–15 s per member) and
  256 timed executions (~47 min in one lock session at the measured 11 s
  cadence), half of v2 §A8's budgets.
- v2 §A8's mechanics otherwise stand: scratch target directories outside the
  checkout, deleted between members; each binary staged and hashed before the
  next build; a member that fails to build is retried once, then v1 §3.5
  applies (the continuation past K = 128 repeats a page offset, as v2 §A3
  records, and is recorded if used); one execution of one repetition per
  build at `--target-ms 250`; v1 §5.2's adjacent-arm ordering over members
  0…127; v1 §5.4's recording rule; the reference arm builds in
  `.agents/worktrees/control-0c072d73` and the candidate arm in the main
  checkout.
- The verification of C2/C3 runs from the ledgers after the build phase and
  before the timed phase, in the v3-amended verifier extended with the
  cross-arm checks; both verifier texts enter the session record.

## C6. What this amendment does not touch

- **v1, v2 and v3 in full.** What the next session reads through this
  document is v2 §A3's enumeration table (replaced by C2), v2 §A7's K
  (replaced by C4) and v2 §A4-as-read-by-v3 (extended by C3). Every other
  provision of all three stands.
- **Receipt 4 and its verdict.** It stands as taken, attributable FAIL and
  all, with its falsification record intact. This amendment does not reopen
  it; `fc976a80`'s findings are the measurement this document answers.
- **All standing receipts, both plans, every predeclared value.** τ_cell,
  τ_set, the pinned set, the schema token, the margin, the protocol, the
  pinned host, the wrapper, the affinity, the governor, the toolchain.
- **v1 §7's declared readings.** All rows and further readings govern the
  next session exactly as written, §7.1's recorded single-build comparison
  included.

## C7. Standing of this document

This rule is written after receipt-4 and its diagnosis were committed, and
before any session under it exists: no member of a translation-only ensemble
has been built or measured on either revision. Its inputs are the committed
receipt-4 record, the committed findings of `fc976a80`, and probe builds —
build-time and already-committed facts, not numbers this rule will govern.
The balance claims of C2 are arithmetic over the enumeration, verified for
every base-phase class before commit and re-verified on realized ledgers
before any timed window.

The owner of epic `6dc81018` approves this text before the next measured
session is dispatched. Until that approval is recorded, no session runs
under it.
