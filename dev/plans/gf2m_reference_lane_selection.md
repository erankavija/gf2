# GF(2^m) reference lane — selection design doc

> **Issue:** `jit:9a715d75` (Select GF(2^m) reference lane).
> **Parent story:** `2c7548ae` (Close GF(2^m) FieldMatrix gaps to best reference).
> **Consumer:** `4c0d0202` (Publish SOTA target matrix design doc) — Wave 4.
> **Authority:** `dev/plans/sota_reference_acceptance_protocol.md` (§ 3 five
>   criteria, § 6 correctness oracle, § 8 / § 9 exclusion classes).
> **Decision authority for exclusions:** the user, via the lead's escalation
>   path. This document originally **proposed** exclusions for the lead to
>   escalate; the lead executed the escalation on 2026-05-04 and the user's
>   decisions are now recorded in § 6 (Acceptance) and § 7 (resolved Open
>   questions). The proposals listed in § 4 are no longer pending — they
>   are either approved-as-stated or have been re-shaped by the user
>   (proposal #1 rejected in favour of harnessing GF(2^32) via task
>   `b13799ac`).

## 1. Purpose

Decide, for each cell `(operation, field) ∈ {matmul, echelon, invert,
solve, charpoly, minpoly, spmv} × {GF(2^8), GF(2^16), GF(2^32)}`,
either:

* The **single accepted hard reference** (with its evidence-doc citation), or
* The **exclusion class** (per protocol § 8) under which the cell is
  declared uncoverable for the current epic, accompanied by an escalation
  recommendation for user approval before the consumer matrix
  (`4c0d0202`) ingests it.

GF(2^m) sparse `spmv` is included for completeness even though it has no
existing reference harness in the workspace; both rationale and an
exclusion class are recorded below.

## 2. Inputs and constraints

* **gf2-core's GF(2^m) production surface.** `Gf2mField_<V: UintExt>` in
  `crates/gf2-core/src/gf2m/` supports any `m < V::BITS`. With
  `V = u64`, `m ∈ {2, …, 63}` is representable. Log/antilog tables are
  built only for `m ≤ 16` (`crates/gf2-core/src/gf2m/field.rs:172-174`);
  for `m > 16` the path is the SIMD PCLMULQDQ-+-Barrett kernel
  (`gf2m/barrett.rs` + `gf2-kernels-simd::gf2m_clmul_*`) or the scalar
  Barrett fallback. **GF(2^32) is therefore in production scope** and
  needs an external reference if at all reproducible.
* **`crates/gf2-core/src/primitive_polys.rs::standard()`.** Returns a
  primitive polynomial only for `m ∈ [2, 16]`. For `m = 32` the database
  returns `None`, and the user must supply their own polynomial (the
  CLAUDE.md `gf2m/` description and the `Gf2mField::new` constructor
  both make this explicit). Any external reference at GF(2^32) must
  also accept an arbitrary primitive polynomial so the basis matches.
* **M4RIE matmul promotion (sealed in Wave 2).** `dev/plans/m4rie_promotion_evidence.md`
  promotes M4RIE 20250128 for **matmul only**, over GF(2^4), GF(2^8),
  GF(2^16). Echelon and m > 16 are explicitly out of scope and may not
  be reopened without user approval (per the Wave 2 close in
  `dev/active/97bf0879-handoff-2.md` Trap 4).
* **Other Wave 2 references.** fflas-ffpack 2.5.0 has **no GF(2^m)
  specialization** (`dev/plans/fflas_ffpack_analysis.md` § "No GF(2^m)
  specialization"); LinBox 1.7.1 dispatches GF(p) work to fflas-ffpack
  internally and has no harnessed GF(2^m) lane (`dev/plans/linbox_promotion_evidence.md`
  § "Out of scope"); NTL 11.6.0 was harnessed only for GF(p)
  (`dev/plans/ntl_promotion_evidence.md` Operations table: only GF(7),
  GF(251), GF(65521), GF(2^31-1)); FLINT 3.5.0 likewise only for GF(p)
  (`dev/plans/flint_promotion_evidence.md` § "Scope of promotion": four
  GF(p) prime columns, no GF(2^m) column). **No Wave 2 candidate today
  produces a single GF(2^m) reference row beyond M4RIE's matmul rows.**
* **Protocol § 6 echelon contract is bitwise.** Per Trap 4 of
  `dev/active/97bf0879-handoff-2.md`: the protocol's RREF correctness
  contract is "Bitwise equality of the canonical RREF" against an
  *independent* reference. Structural-RREF-invariants (pivot/rank/zero-
  row checks) are insufficient. M4RIE's `mzed_echelonize` cannot serve
  as its own oracle, and no scalar GF(2^m) RREF reference exists in the
  workspace today. The same logic extends to invert / solve / charpoly /
  minpoly: each protocol § 6 row demands bitwise equality after
  canonical reduction, which requires an independent oracle of the
  output, not just an internal smoke check.

## 3. Operation × field matrix

Cells are `selected_reference (citation)` or
`EXCLUDED:<class>:<one-line reason>`. Citations are evidence-doc paths
plus the relevant CSV row group when applicable. Exclusion classes
are protocol § 8 entries unless explicitly proposed as an extension
(see § 4 for the proposed extension `no-independent-oracle` and the
re-use rationale for `not-yet-evaluated`-style cells, both surfaced
for user approval).

| Operation | GF(2^8) | GF(2^16) | GF(2^32) |
|---|---|---|---|
| `matmul` (`fgemm`) | **m4rie 20250128** (`dev/plans/m4rie_promotion_evidence.md` *Target-matrix designation*; CSV rows in `dev/bench_results/2026-05-04-507b0036-m4rie-reference.csv` lines 8-13 for GF(2^8) and 14-19 for GF(2^16)) | **m4rie 20250128** (same evidence doc; CSV rows lines 14-19 for GF(2^16)) | **NTL 11.6.0 `mat_GF2E`** — promoted by task `b13799ac` (`dev/bench_results/2026-05-04-b13799ac-gf2pow32-promotion.md`). Conway polynomial `x^32 + x^15 + x^9 + x^7 + x^4 + x^3 + 1` (`0x1_0000_8299`, Frank Lübeck's database) shared between gf2-core (`crates/gf2-core/src/primitive_polys.rs::standard(32)`) and NTL `GF2E::init`; no basis-change matrix required. Bitwise-equality oracle at `n=16` is the direct gf2-core ↔ NTL byte-equality check in `benchmarks/reference/ntl_gf2pow32_smoke.cpp` (R2 rewrite, jit:b13799ac), driven by the gf2-core ground-truth file `benchmarks/expected/gf2pow32_smoke_n16.bin` emitted by `crates/gf2-coding/examples/gf2pow32_smoke_emit_expected.rs`. The Rust-side `crates/gf2-core/tests/gf2pow32_matmul.rs::test_gf2pow32_fieldmatrix_gemm_matches_scalar_reference` is retained as a separate Rust-internal gf2-core ↔ scalar witness. |
| `echelon` (RREF) | EXCLUDED:`no-independent-oracle`:protocol § 6 requires bitwise canonical RREF equality vs an *independent* reference; no scalar GF(2^m) RREF exists in the workspace, and M4RIE was explicitly down-scoped out of echelon in Wave 2. See § 4 proposal #2. | same as GF(2^8). | same as GF(2^8) — and additionally M4RIE itself is unsupported at m > 16 (compounds with proposal #1 / #2). |
| `invert` | EXCLUDED:`no-independent-oracle`:protocol § 6 row "invert" requires bitwise equality of `A^{-1}` after canonical reduction vs an independent reference; no GF(2^m) inverse oracle is harnessed. M4RIE provides `mzed_invert` but cannot serve as its own oracle. | same as GF(2^8). | same as GF(2^8) — compounds with proposal #1. |
| `solve` (`Ax=b`) | EXCLUDED:`no-independent-oracle`:protocol § 6 row "solve" requires equality of `x` after canonical reduction; no independent GF(2^m) solver oracle is harnessed. | same as GF(2^8). | same as GF(2^8) — compounds with proposal #1. |
| `charpoly` | EXCLUDED:`no-independent-oracle`:no independent GF(2^m) characteristic-polynomial reference is harnessed. NTL `mat_GF2E::CharPoly` and FLINT `fq_nmod_mat_charpoly` exist upstream but neither has a harness in `benchmarks/reference/` for this epic. See § 4 proposal #3. | same as GF(2^8). | same as GF(2^8) — also unsupported at m > 16 in M4RIE if a future M4RIE-based oracle were considered (compounds with proposal #1). |
| `minpoly` | EXCLUDED:`no-independent-oracle`:no independent GF(2^m) minimal-polynomial reference is harnessed. NTL provides only `MinPolyMod` for polynomials, not `MinPoly(mat_GF2E)`; FLINT's `fq_nmod_mat_minpoly` is a candidate but has no harness. See § 4 proposal #3. | same as GF(2^8). | same as GF(2^8) — compounds with proposal #1. |
| `spmv` (sparse) | EXCLUDED:`not-performance-relevant`-adjacent / `no-independent-oracle`:no GF(2^m) sparse reference is harnessed today; sparse-corpus selection is the subject of issue `a3412e15` (Wave 3). The cell is deferred pending that issue's output. | same as GF(2^8). | same as GF(2^8). |

**Read (post-2026-05-04 user decision; b13799ac landed 2026-05-04).** Of
21 cells, **3 are selected** (M4RIE matmul over GF(2^8) and GF(2^16);
NTL 11.6.0 `mat_GF2E` matmul over GF(2^32) — promoted by task
`b13799ac`, evidence at
`dev/bench_results/2026-05-04-b13799ac-gf2pow32-promotion.md`) and **18
are excluded** (per protocol § 9). The exclusions cluster into two
reasons:

* **Reason B (15 cells):** non-matmul × any GF(2^m) — no independent
  bitwise oracle exists. Proposal #2 (echelon) and proposal #3
  (invert/solve/charpoly/minpoly) in § 4 below.
* **Reason C (3 cells):** sparse `spmv` × any GF(2^m) — sparse-corpus
  decision is owned by the parallel issue `a3412e15`. Proposal #4
  in § 4 below.

## 4. Exclusion proposals for user approval

Each item is phrased so the lead can copy it verbatim into an
`AskUserQuestion` block (or escalation note) without further editing.
The four items are independent and may be approved (or rejected, or
counter-proposed) one-at-a-time.

### Proposal #1 — `(matmul, GF(2^32))` — **REJECTED 2026-05-04 / RESOLVED via `b13799ac`**

> **Status note (2026-05-05).** The user rejected this exclusion
> proposal in favour of harnessing the cell. Task `b13799ac` (Build
> GF(2^32) matmul reference harness) landed on 2026-05-04 with NTL
> 11.6.0 `mat_GF2E` selected as the canonical reference under the
> Conway polynomial `0x1_0000_8299`. Direct gf2-core ↔ NTL byte-equality
> smoke at n=16 via the ground-truth file mechanism (`b13799ac` R2)
> closes the protocol § 6 contract; canonical bench-day CSV row in
> `benchmarks/results/20260505T091600Z.csv`. See
> `dev/bench_results/2026-05-04-b13799ac-gf2pow32-promotion.md` for
> the full evidence trail. The text below is retained for historical
> context — it documents the original exclusion proposal that was
> superseded by the harness decision.

**Cell:** `matmul × GF(2^32)`.
**Proposed exclusion class:** *new class* `not-yet-harnessed` (or, if
the user prefers to use the existing § 8 vocabulary verbatim,
`unbuildable-on-R3-container` against the M4RIE candidate specifically,
acknowledging that the underlying root cause is "candidate cannot
represent the field" rather than a build failure).
**Rationale.** The protocol § 8 registry does not have a clean
"no candidate proposed" class. The closest existing class is
`unbuildable-on-R3-container` (M4RIE *can* be built but its
`gf2e_init` rejects m > 16, so the m=32 build path simply does not
exist in M4RIE). Two upstream candidates do support extension fields
of arbitrary degree — NTL `mat_GF2E` and FLINT `fq_nmod_mat` — but
neither has been evaluated in this epic; harnessing either is a
~half-day to one-day task (Containerfile stanza already exists for
both libraries from Wave 2; only the `*_bench.{c,cpp}` GF(2^m) entry
points and the cross-equality oracle would need to be added).
**What would unblock promotion.** Either (a) add a GF(2^32) lane to
the existing NTL or FLINT harness, with a basis-change matrix derived
from gf2-core's user-supplied primitive polynomial (the polynomial
itself is not in the gf2-core database for m=32; the user must
nominate one — the protocol's GF(2^m) basis convention then applies),
or (b) declare GF(2^32) matmul out of scope for the
`Close GF(2^m) FieldMatrix gaps` story (`2c7548ae`) and amend its
[hard] criterion #2 ("GF(2^8), GF(2^16), and GF(2^32) target rows are
within 1.5x of the selected reference or faster") to `(GF(2^8),
GF(2^16))` only.
**Recommended escalation phrasing (lead may quote verbatim).**
"M4RIE caps at GF(2^16) and no other Wave 2 reference covers GF(2^m).
Two viable candidates exist (NTL `mat_GF2E`, FLINT `fq_nmod_mat`) but
harnessing either is fresh work. Proposal: amend `2c7548ae` criterion
#2 to scope GF(2^32) matmul out for this epic, with a follow-up issue
(post-Wave-4) to evaluate one candidate. If you want GF(2^32) covered
in this epic, please name which candidate (NTL or FLINT) to harness
first; otherwise the cell is recorded as
`EXCLUDED:not-yet-harnessed` in the consumer matrix `4c0d0202`."

### Proposal #2 — non-matmul GF(2^m) (echelon, invert, solve)

**Cells:** `echelon × {GF(2^8), GF(2^16), GF(2^32)}`,
`invert × {GF(2^8), GF(2^16), GF(2^32)}`,
`solve × {GF(2^8), GF(2^16), GF(2^32)}` — 9 cells total.
**Proposed exclusion class:** *new class* `no-independent-oracle`
(the protocol's § 8 list does not enumerate this; the closest existing
class is `semantics-mismatch`, but that mis-states the problem —
M4RIE's outputs are not semantically wrong, they just have no
*independent* reference to compare against).
**Rationale.** The Wave 2 close (`dev/active/97bf0879-handoff-2.md`
Trap 4) recorded the protocol § 6 echelon contract as **bitwise
canonical RREF equality** against an *independent* reference; the
analogous invert and solve rows of the protocol § 6 table demand
bitwise equality of `A^{-1}` and `x` after canonical reduction. M4RIE
provides `mzed_echelonize`, `mzed_invert`, and `mzed_solve` upstream,
but using M4RIE as both candidate and oracle violates the
"independent oracle" requirement. fflas-ffpack does not cover
GF(2^m); M4RI covers only GF(2). No scalar GF(2^m)
Gauss-Jordan reference exists in the workspace today; the
`m4rie_promotion_evidence.md` § "Future work" section explicitly flags
adding `ref_gf2m_rref` (analogous to the existing `ref_gf2m_mul`
helper) as a future task. Until that scalar reference exists, the
contract for these three operations cannot be satisfied for any
GF(2^m).
**What would unblock promotion.** Add a scalar `ref_gf2m_rref`
(Gauss-Jordan over GF(2^m) using `ref_gf2m_mul` for products and
Fermat-little-theorem-based scalar inverse `a^(2^m − 2) = a^{−1}` in
GF(2^m)) to `benchmarks/reference/`, then re-evaluate M4RIE for
echelon promotion under the bitwise contract. Once the scalar
reference exists, `invert` falls out as `A·A^{-1} == I` plus
`A^{-1}` ≡ ref, and `solve` as `A·x ≡ b` plus `x` ≡ ref.
**Recommended escalation phrasing (lead may quote verbatim).**
"For non-matmul GF(2^m) operations (echelon / invert / solve), the
protocol § 6 contract is bitwise equality vs an *independent*
reference. No independent GF(2^m) Gauss-Jordan reference exists in
the workspace; M4RIE upstream provides these operations but cannot
serve as its own oracle. Proposal: record these 9 cells as
`EXCLUDED:no-independent-oracle` in the consumer matrix `4c0d0202`
and file a follow-up issue 'Add scalar ref_gf2m_rref reference and
re-evaluate M4RIE non-matmul scope'. If you want any of these cells
covered in this epic, please flag which one(s); the harness work is
~one to two days for the scalar reference plus the M4RIE bitwise
oracle, and is not blocked by Wave 3."

### Proposal #3 — non-matmul GF(2^m) (charpoly, minpoly)

**Cells:** `charpoly × {GF(2^8), GF(2^16), GF(2^32)}`,
`minpoly × {GF(2^8), GF(2^16), GF(2^32)}` — 6 cells total.
**Proposed exclusion class:** `no-independent-oracle` (same
proposed class as proposal #2; the structural mismatch is the same).
**Rationale.** Two upstream candidates exist for GF(2^m)
characteristic / minimal polynomials: NTL `CharPoly(mat_GF2E)` and
FLINT `fq_nmod_mat_charpoly` / `fq_nmod_mat_minpoly`. Neither has been
evaluated in Wave 2 — the NTL and FLINT harnesses
(`benchmarks/reference/{ntl,flint}_bench.{c,cpp}`) were authored for
GF(p) only. The Wave 2 NTL evidence doc (§ "Operations explicitly not
covered" lines 29-41) lists `minpoly` as out of scope for NTL because
NTL's user-facing API exposes only `MinPolyMod` for polynomials, not
`MinPoly(mat_zz_p)`. By contrast, FLINT covers both `charpoly` and
`minpoly` over GF(p) and *does* expose `fq_nmod_mat_charpoly` /
`fq_nmod_mat_minpoly` for GF(2^m) — so FLINT is the recommended
candidate for any future GF(2^m) charpoly / minpoly promotion.
**What would unblock promotion.** Add a GF(2^m) lane to the existing
FLINT harness `benchmarks/reference/flint_bench.c`, using
`fq_nmod_ctx_init` with gf2-core's primitive polynomial. The
existing FLINT Containerfile stanza is already pinned, so this is
purely a harness extension. Cross-equality with a scalar reference
(or with NTL `mat_GF2E::CharPoly` / Cayley-Hamilton check
`p(A) = 0`) is the protocol § 6 contract.
**Recommended escalation phrasing (lead may quote verbatim).**
"For non-matmul GF(2^m) polynomial invariants (charpoly / minpoly),
FLINT 3.5.0 has the right upstream API (`fq_nmod_mat_charpoly`,
`fq_nmod_mat_minpoly`) but it was not harnessed in Wave 2 — only the
GF(p) lane of FLINT was. Proposal: record these 6 cells as
`EXCLUDED:no-independent-oracle` in the consumer matrix `4c0d0202`,
filed alongside a follow-up issue 'Extend FLINT harness with GF(2^m)
lane for charpoly / minpoly'. If you want any of these cells covered
in this epic, the harness work is ~one day plus a Cayley-Hamilton
self-check. Not blocked by Wave 3."

### Proposal #4 — GF(2^m) sparse `spmv`

**Cells:** `spmv × {GF(2^8), GF(2^16), GF(2^32)}` — 3 cells total.
**Proposed exclusion class:** `not-performance-relevant`-adjacent
(marker only; no rejection) — recommend recording these as
*deferred-to-sibling-issue* `a3412e15` (Wave 3 sparse-corpus
selection).
**Rationale.** Sparse `spmv` is its own Wave 3 issue. The
sparse-corpus selection task `a3412e15` is the right home for the
GF(2^m) sparse reference decision; this matmul-and-friends-focused
issue should not pre-empt that decision. The protocol § 8
`not-performance-relevant` class is a *marker, not a fail-fast*;
using it here records the deferral without classifying the cell as
either selected or rejected.
**What would unblock promotion.** The output of `a3412e15` will
either (a) name a GF(2^m) sparse reference (LinBox `Sparse` with
custom domains over GF(2^m) is the candidate of last resort, but its
GF(2^m) coverage is uncertain), or (b) propose a separate exclusion
that the consumer matrix `4c0d0202` then ingests.
**Recommended escalation phrasing (lead may quote verbatim).**
"Sparse `spmv` over GF(2^m) is owned by Wave 3 issue `a3412e15`
(sparse-corpus selection). Proposal: defer these 3 cells to that
issue's output and let `a3412e15` either name a reference or propose
an exclusion. No user approval needed today; the consumer matrix
`4c0d0202` should ingest whichever resolution `a3412e15` produces."

### Summary of approval requests

| Proposal | Cells | Recommended decision in `4c0d0202` |
|---|---|---|
| #1 | `(matmul, GF(2^32))` (1 cell) | **REJECTED 2026-05-04** — user opted to harness rather than exclude. Resolved by `b13799ac`: NTL 11.6.0 `mat_GF2E` promoted with Conway polynomial `0x1_0000_8299`; direct gf2-core ↔ NTL n=16 smoke via ground-truth file (R2); canonical bench row in `benchmarks/results/20260505T091600Z.csv`. |
| #2 | echelon / invert / solve × all GF(2^m) (9 cells) | EXCLUDE with `no-independent-oracle`; file follow-up `Add scalar ref_gf2m_rref reference`. |
| #3 | charpoly / minpoly × all GF(2^m) (6 cells) | EXCLUDE with `no-independent-oracle`; file follow-up `Extend FLINT harness with GF(2^m) lane`. |
| #4 | spmv × all GF(2^m) (3 cells) | DEFER to `a3412e15`; consumer matrix ingests that issue's output. |

**Status (2026-05-05).** Proposal #1 was rejected by the user on
2026-05-04 in favour of harnessing — resolved via `b13799ac` (above).
Proposals #2, #3, #4 were user-approved on 2026-05-04 (see § 6
criterion #2 evidence row); the consumer issue `4c0d0202` will ingest
those three exclusions plus the `(matmul, GF(2^32))` selection (NTL
11.6.0 `mat_GF2E`).

## 5. Re-run cost estimate

For each "selected" cell, the citation points to an existing CSV row;
no fresh benchmark run is required for this issue's deliverable. The
GF(2^m) m4rie evidence is intact at
`dev/bench_results/2026-05-04-507b0036-m4rie-reference.csv` (18 rows
covering GF(2^4) / GF(2^8) / GF(2^16), n ∈ {64, 256, 1024}, regimes
{uniform, deficient}). Selected cells used by this doc:

| Cell | Existing CSV rows used | Re-run wall-clock (if fresh promotion run requested) |
|---|---|---|
| `matmul × GF(2^8)` | `2026-05-04-507b0036-m4rie-reference.csv` lines 8-13 (six rows: 3 sizes × 2 regimes) | ~1 minute (single-thread M4RIE matmul, n ≤ 1024). The `m4rie_bench --warmup 3 --iters 5` invocation in `dev/plans/m4rie_promotion_evidence.md` § "Build evidence" already produced these rows; rerunning produces nanosecond-level numerical differences but the cell selection is unchanged. |
| `matmul × GF(2^16)` | same CSV, lines 14-19 | ~15 minutes (n=1024 GF(2^16) is the slowest matmul cell at ~752 ms per iteration; warmup-3 + iters-5 = ~6 s for that one cell, plus warmup overhead and faster smaller cells). |

For excluded cells, no re-run is involved (no candidate harness exists).
The follow-up tasks proposed in § 4 each carry their own re-run cost
estimate (one to two days of harness work plus a fresh bench-run on
the Zen-3 anchor host).

## 6. Acceptance — mapping to issue 9a715d75 [hard] criteria

> **User decision recorded 2026-05-04 (Wave-3 closure escalation).** The
> user **rejected proposal #1's exclusion** and elected to extend the
> epic scope by harnessing GF(2^32) matmul; **approved proposals #2,
> #3, #4** under the new exclusion classes; **approved the protocol § 8
> registry extension** to add `not-yet-harnessed` and
> `no-independent-oracle` as recognized exclusion classes (open
> question #1 resolved); and **delegated the GF(2^32) primitive-poly
> selection** to the new harness task (open question #3). New task
> filed: `b13799ac` ("Build GF(2^32) matmul reference harness"), wired
> as a Wave-12-aggregation prerequisite. Open question #2 resolved by
> filing the harness task now (during Wave 3 closure).

| Issue criterion | Status | Evidence in this document |
|---|---|---|
| **#1 [hard]** "The selected lane covers GF(2^8), GF(2^16), and GF(2^32) where feasible." | **MET** | § 3 covers all three fields across all `FieldMatrix` operations. GF(2^8) and GF(2^16) `matmul` are covered by M4RIE 20250128 (Wave-2-promoted; citations in § 3). GF(2^32) `matmul` was originally proposed for exclusion (§ 4 proposal #1) but the user rejected the exclusion in favour of harnessing the cell. Task `b13799ac` (Build GF(2^32) matmul reference harness, story `2c7548ae`) **landed 2026-05-04** with NTL 11.6.0 `mat_GF2E` promoted under the Conway polynomial `0x1_0000_8299`; evidence in `dev/bench_results/2026-05-04-b13799ac-gf2pow32-promotion.md`. 9a715d75 closes with criterion #1 satisfied. |
| **#2 [hard]** "If no hard reference is viable, the exclusion is user-approved and documented." | **MET** | § 4 names every exclusion (3 surviving grouped proposals — #2 echelon/invert/solve, #3 charpoly/minpoly, #4 spmv-deferred — covering 18 cells). Each has a precise exclusion class and a one-paragraph rationale. **User approval recorded 2026-05-04** for proposals #2, #3, #4 plus the protocol § 8 registry extension adding `not-yet-harnessed` (used by `b13799ac`'s open exclusion-not-yet-resolved status until that task closes) and `no-independent-oracle` (used by proposals #2 and #3). The exclusions are now both user-approved (this section) and documented (§ 4). |

## 7. Open questions — resolved 2026-05-04

All three open questions originally listed here were resolved by the
2026-05-04 Wave-3 closure escalation:

1. **Exclusion-class registry extension. — RESOLVED.** Option (a)
   selected: protocol § 9 (formerly § 8) extended via Amendment 2 in
   `dev/plans/sota_reference_acceptance_protocol.md` § 14, adding
   `not-yet-harnessed` and `no-independent-oracle` as recognized
   exclusion classes.
2. **Sequence of follow-up issues. — RESOLVED.** Filed concurrently
   with the user-approval escalation: `b13799ac` (Build GF(2^32)
   matmul reference harness) is now an open task under story
   `2c7548ae`, wired as a Wave-12-aggregation prerequisite via
   `dece4e73`'s JIT dep edge.
3. **GF(2^32) primitive-polynomial choice. — DELEGATED.** Carried
   into the new `b13799ac` task as one of its `[hard]` criteria;
   its dispatch prompt will surface the candidate polynomials
   (`x^32 + x^7 + x^3 + x^2 + 1` from Hansen-Mullen, or a trinomial
   alternative) for user nomination before harness work begins.

## 8. Files referenced (for reviewer convenience)

* `dev/plans/sota_reference_acceptance_protocol.md` — protocol of record.
* `dev/plans/m4rie_promotion_evidence.md` — sealed Wave 2 promotion of
  M4RIE for matmul over GF(2^4), GF(2^8), GF(2^16).
* `dev/plans/fflas_ffpack_analysis.md` — confirms fflas-ffpack has no
  GF(2^m) specialization.
* `dev/plans/linbox_promotion_evidence.md` — confirms LinBox dispatches
  GF(p) work to fflas-ffpack and has no harnessed GF(2^m) lane.
* `dev/plans/ntl_promotion_evidence.md` — Wave 2 NTL evaluation,
  GF(p) only.
* `dev/plans/flint_promotion_evidence.md` — Wave 2 FLINT evaluation,
  GF(p) only.
* `dev/bench_results/2026-05-04-507b0036-m4rie-reference.csv` — the
  18 M4RIE matmul rows cited in § 3.
* `dev/bench_results/2026-04-29-gf2m-batch-fieldmatrix-gemm.md` —
  gf2-core's own internal-only GF(2^m) batch-GEMM evidence (5.01x to
  17.65x speedups vs scalar-eager) — context for "why GF(2^m)
  performance is interesting" but **not** an external reference.
* `dev/active/97bf0879-handoff-2.md` — session-2 handoff; Trap 4
  records the bitwise-RREF protocol contract and the M4RIE down-scope.
* `crates/gf2-core/src/gf2m/` — production GF(2^m) module map.
* `crates/gf2-core/src/primitive_polys.rs` — primitive polynomial
  database; standard polys only for `m ∈ [2, 16]`.

## 9. Document-attach checklist

Per the dispatch contract for `9a715d75`:

* `jit doc add 9a715d75 dev/plans/gf2m_reference_lane_selection.md
  --doc-type design --label "GF(2^m) reference lane decision"`.
* `jit doc add 2c7548ae dev/plans/gf2m_reference_lane_selection.md
  --doc-type design --label "GF(2^m) reference lane decision"`
  (parent story).

The lead's 2026-05-04 escalation resolved the § 4 exclusion proposals
(see § 6 *User decision recorded*), so consumer issue `4c0d0202` may
attach this document and ingest its decisions as soon as Wave 4
dispatches. The Wave-3 closure of 9a715d75 unblocks `4c0d0202`.
