# Investigation report: b7157be6

This is a read-only verification of the repository at investigation time. The
classification terms below mean: **already-done** is supported by current
code; **valid-and-open** is directionally true but has a material missing
piece; **invalid-as-stated** is contradicted by current code. No implementation
or JIT state was changed for this report.

## Claim classification

1. **“The workspace has no OSD” — already-done, with a prior-art nuance.** A
   whole-tree search of `crates/` and `dev/` found no ordered-statistics,
   most-reliable-basis, or OSD implementation. The closest code hit is only a
   test comment saying that non-pivot columns form an information set
   (`crates/gf2-core/tests/rref_comprehensive.rs:445`); the OSD text in
   `dev/active/aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md:24-30`
   is review guidance, not code. The current epic itself describes the
   intended implementation at `.jit/issues/b7157be6-16d8-4050-834c-e996d4fa27c3.json:8`.
   Thus the claim is true for implementation, but not true if “no OSD-related
   discussion” was intended.

2. **“Dense GF(2) elimination with column pivoting exists” — valid-and-open.**
   `gf2_core::alg::rref::rref` is a public dense GF(2) RREF API, explicitly
   documented as column-pivoting Gaussian elimination, with a
   `pivot_from_right` direction flag (`crates/gf2-core/src/alg/rref.rs:24-54`,
   `crates/gf2-core/src/alg/rref.rs:74-76`). It returns the reduced matrix,
   pivot columns, row permutation, and rank (`crates/gf2-core/src/alg/rref.rs:8-22`).
   It does not accept a reliability permutation, a chosen information-set
   position set, or a bounded ordered pivot list. It is therefore reusable
   after an external column permutation, but column-pivoted elimination onto a
   selected most-reliable basis is not directly expressible through the current
   API. The reusable extension point is `gf2-core`, not a private elimination
   helper in `gf2-coding`.

3. **“Generator-matrix access exists” — already-done.** The canonical trait is
   `GeneratorMatrixAccess`; it exposes `k`, `n`, `generator_matrix() -> BitMatrix`,
   and `is_systematic` (`crates/gf2-coding/src/traits.rs:10-27`,
   `crates/gf2-coding/src/traits.rs:47-107`). `LinearBlockCode` stores a
   `BitMatrix` generator (`crates/gf2-coding/src/linear.rs:11-50`) and returns
   it through the trait (`crates/gf2-coding/src/linear.rs:307-323`). Current
   implementations are `BchCode` (`crates/gf2-coding/src/bch/core.rs:297-320`),
   `ExtendedBchCode` (`crates/gf2-coding/src/bch/extended.rs:384-400`),
   `LdpcCode` (`crates/gf2-coding/src/ldpc/core.rs:426-448`), `QcGldpcCode`
   (`crates/gf2-coding/src/gldpc/mod.rs:888-915`), and wrappers for DRM and
   CRC (`crates/gf2-coding/src/drm.rs:765-781`,
   `crates/gf2-coding/src/crc.rs:337-353`). Convolutional decoding is
   streaming and has no generator-matrix implementation
   (`crates/gf2-coding/src/convolutional.rs:1-5`, `crates/gf2-coding/src/convolutional.rs:78-104`),
   so its exclusion is real. The NR rate-matched soft decoder is not itself a
   `GeneratorMatrixAccess` implementation; its mother-code access is a
   separate concern.

4. **“LLR handling exists” — already-done.** `Llr(f32)` is the canonical type;
   positive means bit 0 is more likely, negative means bit 1, and magnitude is
   confidence (`crates/gf2-coding/src/llr.rs:1-41`). `Llr::hard_decision` maps
   nonnegative values to bit 0 and negative values to bit 1, with zero tied to
   0 (`crates/gf2-coding/src/llr.rs:62-75`), while `Llr::magnitude()` supplies
   `|LLR|` (`crates/gf2-coding/src/llr.rs:77-89`). The batch helper delegates to
   that scalar method (`crates/gf2-coding/src/llr.rs:322-340`).

5. **“Existing BCH machinery constructs (127,64), t=10, and extended
   (128,64) with a generator matrix” — valid-and-open.** `BchCode::new` accepts
   arbitrary `n`, `k`, and `t`, requires `n < 2^m`, creates the field tables,
   and constructs the BCH generator (`crates/gf2-coding/src/bch/core.rs:125-148`).
   Consequently a caller can supply `n=127`, `k=64`, `t=10`, and a pinned
   GF(2^7) field. The BCH generator matrix is computed by encoding each basis
   vector and exported as `BitMatrix` (`crates/gf2-coding/src/bch/core.rs:264-294`,
   `crates/gf2-coding/src/bch/core.rs:297-318`). `ExtendedBchCode::from_bch`
   appends the overall parity bit, constructs the extended parity check, and
   retains a systematic generator (`crates/gf2-coding/src/bch/extended.rs:81-108`,
   `crates/gf2-coding/src/bch/extended.rs:115-168`,
   `crates/gf2-coding/src/bch/extended.rs:384-400`).

   What is open is reproducible reference construction: the named convenience
   constructors stop at `ebch_64_57` (`crates/gf2-coding/src/bch/extended.rs:270-348`),
   so there is no checked-in `ebch_128_64` factory. The field polynomial,
   exact construction/vector source, curve figure, channel, list order, and
   metric are not pinned by this machinery. The review explicitly requires
   those choices and a seeded receipt rather than a fast-tier test
   (`dev/active/aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md:24-30`).

6. **“BP supports a syndrome-guided post-processor and exposes what it needs” —
   valid-and-open.** LDPC BP is in `ldpc/core.rs`; the decoder supports
   min-sum, normalized/offset min-sum, and sum-product variants
   (`crates/gf2-coding/src/ldpc/core.rs:875-906`). `decode_to_codeword` is
   public, returns the full hard codeword, checks the codeword syndrome for
   early termination, and returns `converged` and
   `syndrome_check_passed` in `DecoderResult`
   (`crates/gf2-coding/src/ldpc/core.rs:1301-1342`; result fields are defined at
   `crates/gf2-coding/src/traits.rs:111-137`). The iterative API also checks the
   syndrome before the iteration cap (`crates/gf2-coding/src/ldpc/core.rs:1374-1422`).

   The posterior beliefs are present as `LdpcDecoder::beliefs` but private
   (`crates/gf2-coding/src/ldpc/core.rs:908-915`), so a post-processor cannot
   consume BP posterior LLRs through a public API. The public `SoftDecoder`
   implementation on `LdpcDecoder` is explicitly only hard-decision mapping
   of the input LLRs, not BP (`crates/gf2-coding/src/ldpc/core.rs:1354-1371`).
   GLDPC likewise stores private beliefs and has iterative syndrome-based
   stopping (`crates/gf2-coding/src/gldpc/mod.rs:996-1014`,
   `crates/gf2-coding/src/gldpc/mod.rs:1232-1347`). There is no generic
   decoder-chain or BP-fallback composition API. DVB-T2 has a domain-specific
   LDPC-to-BCH concatenation (`crates/gf2-coding/src/ldpc/dvb_t2/concat.rs:551-581`),
   and product decoding has an SISO-engine selection/fallback enum
   (`crates/gf2-coding/src/product/mod.rs:77-121`), but neither is a reusable
   BP→OSD post-processor.

7. **“An in-tree LDPC is suitable for a fast-tier BP+OSD test” — already-done.**
   The smallest explicit precedent is the test-only `[3,2]` single-parity code,
   built from one check and three edges (`crates/gf2-coding/src/ldpc/core.rs:2213-2222`).
   A normalized-min-sum BP test decodes it, checks convergence and syndrome,
   and asserts the decoded word (`crates/gf2-coding/src/ldpc/core.rs:2309-2321`);
   neighboring tests cover offset min-sum and early termination. This is an
   appropriate sizing precedent for a fast-tier integration test. The
   repository contract limits ordinary fast-tier tests to five seconds per test
   and sixty seconds per suite (`AGENTS.md:83-90`), so the large ignored LDPC
   campaigns are not suitable as the integration-test size precedent.

8. **“Campaign infrastructure exists for REQ-02 reproduction” — valid-and-open.**
   There are two current paths. The legacy configurable runner is
   `gf2-coding`'s `sim_runner`: TOML curves are restricted to `Ldpc`, `Product`,
   and `Gldpc` (`crates/gf2-coding/src/bin/sim_runner.rs:140-164`), and
   `run_curve` dispatches on that finite enum (`crates/gf2-coding/src/bin/sim_runner.rs:428-445`).
   It seeds the simulation but currently sets checkpointing and tracing to
   `None` (`crates/gf2-coding/src/bin/sim_runner.rs:411-425`), then writes CSV
   and JSON outputs (`crates/gf2-coding/src/bin/sim_runner.rs:1020-1071`).
   Existing configurations include `dev/campaigns/phase2_fig4.toml:1-47` and
   `dev/campaigns/verify_fix_fig3.toml:1-16`; an existing result and progress
   receipt are `dev/simulation_results/verify/fig3_product_fixed.json:1` and
   `dev/simulation_results/verify/fig3_product_fixed.progress.jsonl:1-8`.

   The newer `gf2-sim` path owns checkpointed production campaigns: the DVB-T2
   binary documents per-SNR checkpoints, tracing, README provenance, and resume
   invocation (`crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs:1-5`,
   `crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs:82-89`,
   `crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs:131-156`). For an isolated
   BPSK LDPC comparison, `crates/gf2-sim/src/bin/ldpc_bler_sweep.rs:1-8`
   provides a seeded runner and its CLI contract (`crates/gf2-sim/src/bin/ldpc_bler_sweep.rs:53-65`),
   but it is not an eBCH/OSD campaign configuration. Thus the concrete existing
   route closest to an OSD curve is the generic `SimulationConfig`/sweep
   machinery (`crates/gf2-coding/src/simulation.rs:642-725`,
   `crates/gf2-coding/src/simulation.rs:2725-2812`) plus a new decoder dispatch;
   the current TOML runner does not discover a new decoder automatically.

   The simulation harness records counts, rates, iterations/queries, seed and
   optional resumable per-SNR state, but its point schema is not itself a
   confidence-interval report (`crates/gf2-coding/src/simulation.rs:449-463`,
   `crates/gf2-coding/src/simulation.rs:1071-1148`). A published OSD-vs-curve
   number therefore needs a committed seeded receipt with the exact code,
   channel, metric, list order, toolchain/host provenance, and uncertainty.
   This follows the repository invariants for traceable scientific claims,
   uncertainty, external citations, and resumability (`AGENTS.md:141-154`),
   and the external review's explicit “seeded receipt, not fast-tier test”
   requirement (`dev/active/aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md:24-30`).

9. **“GRAND and Chase–Pyndiah provide soft-input reliability sorting and
   enumeration precedents” — already-done, but they are not shared OSD
   primitives.** ORBGRAND uses canonical `Llr` hard decisions, sorts positions
   by ascending `|LLR|`, precomputes the hard-word syndrome and syndrome columns,
   and computes flip/no-flip log probabilities
   (`crates/gf2-coding/src/grand/orbgrand.rs:551-598`). It enumerates logistic
   weight patterns through the private `LogisticWeightPatternIter`, obeys query,
   list, probability, and list-BLER stopping conditions
   (`crates/gf2-coding/src/grand/orbgrand.rs:637-688`; iterator definition and
   subset generation are at `crates/gf2-coding/src/grand/orbgrand.rs:981-1053`).
   ORBGRAND implements the generic `SoftDecoder` trait at
   `crates/gf2-coding/src/grand/orbgrand.rs:777`.

   Chase–Pyndiah independently maps `l < 0` to hard bits, computes `abs(l)`,
   sorts its own index vector, selects the `p` least-reliable positions, and
   enumerates all `2^p` flip masks (`crates/gf2-coding/src/product/chase_pyndiah.rs:413-445`).
   Each candidate is syndrome-checked, optionally single-error-corrected or
   re-encoded, then ranked by LLR correlation (`crates/gf2-coding/src/product/chase_pyndiah.rs:444-480`,
   `crates/gf2-coding/src/product/chase_pyndiah.rs:488-540`). Its syndrome is a
   private XOR over precomputed `u32` H columns (`crates/gf2-coding/src/product/chase_pyndiah.rs:543-564`).
   Chase–Pyndiah is a product-code SISO method, not an implementation of
   `SoftDecoder`; its public decode surface is product/turbo-specific
   (`crates/gf2-coding/src/product/chase_pyndiah.rs:245-287`).

## Convention-convergence inventory

| Mechanism OSD needs | One canonical form to reuse or extend | Current duplication or mismatch |
|---|---|---|
| Reliability sorting / index permutation | `Llr::magnitude()` and the `Llr` convention (`crates/gf2-coding/src/llr.rs:62-89`); the reusable permutation abstraction does not yet exist | ORBGRAND owns a private `pi.sort_by` (`crates/gf2-coding/src/grand/orbgrand.rs:559-567`), while Chase owns a separate private `indices.sort_by` (`crates/gf2-coding/src/product/chase_pyndiah.rs:418-425`). This is an existing two-variant duplication; OSD should not add a third private sorter. |
| Test/error-pattern enumeration | The closest existing source is private `LogisticWeightPatternIter` (`crates/gf2-coding/src/grand/orbgrand.rs:981-1053`) | Logistic-weight enumeration is not OSD's bounded Hamming-ball enumeration `Σ C(k,i)`. It is a semantic mismatch, so a shared enumerator would need an explicit generic contract or an adapter; silently reusing logistic order would not satisfy OSD semantics. |
| GF(2) row reduction / systematic form | `gf2_core::alg::rref::rref`, with `RrefResult` (`crates/gf2-core/src/alg/rref.rs:8-22`, `crates/gf2-core/src/alg/rref.rs:24-76`) | LDPC generator construction already calls the core RREF implementation (`crates/gf2-coding/src/ldpc/core.rs:274-343`); no competing private row-reducer was found. Chosen ordered information-set elimination is a missing core API, not a reason for coding-local elimination. |
| Syndrome computation | Dense canonical operation is `BitMatrix::matvec` (`crates/gf2-core/src/matrix.rs:1427`), exposed for a linear code by `LinearBlockCode::syndrome` (`crates/gf2-coding/src/linear.rs:160-196`); sparse LDPC uses its code-level `syndrome` (`crates/gf2-coding/src/ldpc/core.rs:131-143`) | GRAND uses sparse `H.matvec` and cached syndrome columns (`crates/gf2-coding/src/grand/orbgrand.rs:569-579`); Chase uses a private `u32` column-mask XOR (`crates/gf2-coding/src/product/chase_pyndiah.rs:543-564`). The semantic adapter may differ for dense versus sparse storage, but OSD should not add another private syndrome representation. |
| Hard decision from LLR | `Llr::hard_decision` and `hard_decision_batch` (`crates/gf2-coding/src/llr.rs:62-75`, `crates/gf2-coding/src/llr.rs:322-340`) | GRAND calls the scalar helper, but Chase duplicates the sign test (`crates/gf2-coding/src/product/chase_pyndiah.rs:418-420`). OSD must use the canonical helper. |
| Soft-input decoder trait | `SoftDecoder::{k,n,decode_soft,decode_soft_with_result}` (`crates/gf2-coding/src/traits.rs:242-306`), with `IterativeSoftDecoder` for BP-style controls (`crates/gf2-coding/src/traits.rs:308-325`) | GRAND implements `SoftDecoder` (`crates/gf2-coding/src/grand/orbgrand.rs:777`); Chase–Pyndiah does not, because it is a product-code SISO engine (`crates/gf2-coding/src/product/chase_pyndiah.rs:245-287`). OSD should implement `SoftDecoder`; BP+OSD metadata needs an explicit composition/result contract rather than a second soft-decoder trait. |
| Simulation harness registration | `SimulationConfig`/generic `run_coded` and `run_coded_iterative` (`crates/gf2-coding/src/simulation.rs:3292-3306`, `crates/gf2-coding/src/simulation.rs:3350-3364`) are the generic execution forms; the CLI registry is `CurveType` plus `run_curve` (`crates/gf2-coding/src/bin/sim_runner.rs:148-155`, `crates/gf2-coding/src/bin/sim_runner.rs:428-445`) | `sim_runner` has no decoder plugin registry and only three curve variants. `gf2-sim` has a separate typed pipeline and campaign binary (`crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs:12-27`). OSD needs to enter one existing registration boundary, not create a parallel campaign discovery mechanism. |

## Prior art

- The external review is the most direct design precedent. It asks for a
  shared elimination/reliability engine with separate generator-matrix and
  nonzero-syndrome parity-check semantic adapters, bounded candidate growth
  with cancellation/budget metadata, a pinned exact eBCH reference setup, and
  a seeded reproduction receipt (`dev/active/aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md:24-30`).
  These are current review findings, not existing implementation.
- The GRAND epic is a structural precedent for a decoder epic with algorithm,
  tests, campaign comparison, and committed evidence. Its archived plan names
  the comparison objective and target curves (`dev/archive/6efb756b-grand/plans/grand_5gnr_ldpc_comparison.md:1-8`,
  `dev/archive/6efb756b-grand/plans/grand_5gnr_ldpc_comparison.md:12-36`),
  while the handover records the campaign and receipt paths
  (`dev/archive/6efb756b-grand/active/6efb756b-session-handover.md:29-40`).
- The prior research handoff explicitly places b7157be6 OSD before the
  quantum-LDPC consumer (`dev/sessions/2026-08-07-research-frontier-handoff.md:70-77`).
  The quantum epic description says it expects an arbitrary-syndrome BP+OSD
  baseline (`.jit/issues/cce5da8c-acba-47f2-a629-ecc6cb7f593c.json:5-8`).
- The citation registry contains resolving entries for `Yue2022`,
  `Fossorier1995`, and `Roffe2020` at `.jit/references.toml:14`,
  `.jit/references.toml:35`, and `.jit/references.toml:51`, respectively. The
  entries include the publication citation and arXiv/DOI metadata; no missing
  key was found.
- The only other information-set search hits were RREF test terminology and
  unrelated archived frozen-bit notes (`dev/archive/6efb756b-grand/active/8b1609a8/drm-dynamic-frozen-bits.md:36-68`);
  neither is OSD prior art.

## Primitive verification

### Dense matrix and generator primitives

`BitMatrix` is the core dense GF(2) representation, and its public operations
include row XOR, row/column access, transpose, and matrix-vector products
(`crates/gf2-core/src/matrix.rs:150`, `crates/gf2-core/src/matrix.rs:678-709`,
`crates/gf2-core/src/matrix.rs:1172-1190`, `crates/gf2-core/src/matrix.rs:1427-1546`).
The GF(2) RREF path is therefore the canonical dense elimination source. Its
current pivot search scans matrix columns in the selected direction
(`crates/gf2-core/src/alg/rref.rs:127-174`); there is no selected-column-set
argument or returned column permutation.

Generator access is broad enough for a universal linear-block-code decoder:
the trait returns `BitMatrix`, and BCH, extended BCH, dense LDPC, QC-GLDPC,
DRM, CRC, and linear wrappers implement it at the locations listed in Claim 3.
The trait does not cover convolutional streaming codes, which is consistent
with OSD's linear block-code scope (`crates/gf2-coding/src/convolutional.rs:78-104`).

### LLR and codeword conventions

The repository's LLR convention is consistent between the trait documentation
and `Llr`: positive is zero, negative is one, and magnitude is reliability
(`crates/gf2-coding/src/traits.rs:248-254`, `crates/gf2-coding/src/llr.rs:62-89`).
Any OSD reliability ordering must define deterministic equal-magnitude tie
behavior because the current GRAND and Chase sorts use `sort_by` but do not
publish a common tie-order contract (`crates/gf2-coding/src/grand/orbgrand.rs:559-567`,
`crates/gf2-coding/src/product/chase_pyndiah.rs:422-425`).

### BCH/eBCH construction facts

The generic path is `Gf2mField::new(...).with_tables()` → `BchCode::new(127,
64, 10, field)` → `ExtendedBchCode::from_bch(&base)`. The source verifies only
the BCH length/field-order and positive-t checks at construction
(`crates/gf2-coding/src/bch/core.rs:125-138`); the reference fixture still
needs to pin and test the chosen primitive polynomial and the resulting
generator/parity-check dimensions. The extension bit is not missing: it is
the last generator column and is set to make every row even
(`crates/gf2-coding/src/bch/extended.rs:115-130`).

### BP and syndrome facts

`LdpcCode::syndrome` and `is_valid_codeword` provide the code-level validity
check (`crates/gf2-coding/src/ldpc/core.rs:131-158`). `LdpcDecoder` performs
BP updates into posterior `beliefs`, hard-decodes those beliefs, and checks the
syndrome (`crates/gf2-coding/src/ldpc/core.rs:908-929`,
`crates/gf2-coding/src/ldpc/core.rs:1319-1341`). The missing public surface is
posterior-Llr export or a decoder result that carries the final soft beliefs;
the current result carries only bits and status (`crates/gf2-coding/src/traits.rs:111-137`).

## Architecture fit

The repository contract assigns dense and sparse linear algebra, bit storage,
and field primitives to `gf2-core`, while codes, decoders, channels, and
composition belong in `gf2-coding` (`AGENTS.md:38-58`). The current dependency
direction is therefore a fit for a split in which:

- reliability semantics, OSD order-
  `m` policy, candidate budgets/metadata, generator/parity-check adapters,
  decoder result composition, and code-family behavior remain in
  `gf2-coding`; and
- a reusable dense GF(2) operation for elimination over an explicitly ordered
  or selected column set is added at the `gf2-core` abstraction boundary if
  that operation is genuinely general. The current RREF source is already
  there (`crates/gf2-core/src/alg/rref.rs:24-76`).

Putting dense elimination or a second BitMatrix row-reducer in `gf2-coding`
would violate the stated inward dependency and library-first rules
(`AGENTS.md:40-44`, `AGENTS.md:118-120`). Conversely, putting OSD's LLR
reliability policy or BP fallback semantics in `gf2-core` would put coding
domain behavior below its proper layer. The review's shared engine/separate
semantic-adapter recommendation is compatible with these boundaries
(`dev/active/aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md:24-30`).

There is a current orchestration boundary to account for: `gf2-sim` is the
documented simulation owner (`AGENTS.md:47-48`), and its production campaign
binary drives a typed DVB-T2 pipeline (`crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs:12-27`),
whereas the generic TOML `sim_runner` remains under `gf2-coding`
(`crates/gf2-coding/src/bin/sim_runner.rs:140-164`). An OSD eBCH campaign
should not grow an unregistered third simulation path without resolving that
boundary.

## Consumers and integration points

- **Universal generator-matrix path:** consume `GeneratorMatrixAccess` and
  `Llr`; the existing generator matrix type and systematic flag are sufficient
  to identify the code dimensions (`crates/gf2-coding/src/traits.rs:47-107`).
  The missing primitive is ordered information-set elimination, not generator
  access.
- **Nonzero-syndrome/parity-check path:** LDPC already exposes `H`-based
  validity and syndrome semantics (`crates/gf2-coding/src/ldpc/core.rs:131-158`),
  but no shared parity-check decoder adapter. The review specifically calls for
  a separate semantic adapter for this path (`dev/active/aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md:24-30`).
- **BP+OSD:** BP can signal a failed syndrome and return a full hard word
  (`crates/gf2-coding/src/ldpc/core.rs:1301-1342`), but the post-processor
  cannot currently obtain its final posterior LLR vector because `beliefs` is
  private (`crates/gf2-coding/src/ldpc/core.rs:908-915`). A composition result
  also needs to preserve BP iterations, syndrome status, OSD candidate/query
  counts, and which stage supplied the final codeword; the existing
  `DecoderResult` has only generic iteration/status fields
  (`crates/gf2-coding/src/traits.rs:111-137`).
- **Fast test:** the `[3,2]` one-check code and its normalized-min-sum test are
  the directly verified small LDPC precedent (`crates/gf2-coding/src/ldpc/core.rs:2218-2222`,
  `crates/gf2-coding/src/ldpc/core.rs:2309-2321`).
- **REQ-02 campaign:** existing TOML campaigns and raw CSV/JSON/progress
  outputs provide the operational precedent (`dev/campaigns/phase2_fig4.toml:1-47`,
  `dev/simulation_results/verify/fig3_product_fixed.json:1`). The newer
  checkpointed/resumable infrastructure is in `gf2-sim` and its
  `PipelineConfig` (`crates/gf2-sim/src/config.rs:44-64`,
  `crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs:154-156`).
- **Downstream consumer:** quantum LDPC issue cce5da8c is explicitly dependent
  on this epic and names arbitrary-syndrome BP+OSD in its description
  (`.jit/issues/cce5da8c-acba-47f2-a629-ecc6cb7f593c.json:5-8`). This makes the
  parity-check adapter and nonzero-syndrome behavior contractual, not optional
  polish.

## Open questions

1. What exact primitive polynomial and generator convention define the pinned
   extended BCH (128,64) reference? The generic constructors support the shape,
   but no named factory or reference fixture currently pins it
   (`crates/gf2-coding/src/bch/core.rs:125-148`,
   `crates/gf2-coding/src/bch/extended.rs:270-348`).
2. Does the ordered-elimination API belong as an extension of `rref` or as a
   separate core primitive that preserves the original column order and
   returns the selected information set? Current `rref` has no such input or
   output (`crates/gf2-core/src/alg/rref.rs:8-22`,
   `crates/gf2-core/src/alg/rref.rs:24-76`).
3. What is the one shared reliability-sort contract, including equal-LLR,
   NaN/finite-input handling, and the representation of the permutation? GRAND
   and Chase currently implement separate private versions
   (`crates/gf2-coding/src/grand/orbgrand.rs:559-567`,
   `crates/gf2-coding/src/product/chase_pyndiah.rs:418-425`).
4. Should the shared pattern source expose bounded Hamming weight, logistic
   weight, or a generic budgeted subset iterator? GRAND's private iterator is
   logistic-weight-specific (`crates/gf2-coding/src/grand/orbgrand.rs:1016-1045`),
   while the review requires OSD candidate-growth and cancellation/budget
   metadata (`dev/active/aed96ef9-finite-blocklength-bounds/external-review-2026-08-07.md:24-30`).
5. Should BP+OSD consume channel LLRs, final BP posterior LLRs, or an explicit
   combined soft-state object? The current posterior is private and the public
   BP result is hard bits plus status (`crates/gf2-coding/src/ldpc/core.rs:908-915`,
   `crates/gf2-coding/src/traits.rs:111-137`).
6. Which simulation boundary will own eBCH/OSD curve registration? The legacy
   TOML runner has only three curve variants and no checkpointing by default
   (`crates/gf2-coding/src/bin/sim_runner.rs:148-164`,
   `crates/gf2-coding/src/bin/sim_runner.rs:411-425`), while `gf2-sim`'s
   checkpointed binary is a DVB-T2 typed pipeline
   (`crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs:12-27`).
7. What receipt schema will carry confidence intervals and the required
   runtime-observed provenance? Current simulation results contain counts and
   rates but no CI fields (`crates/gf2-coding/src/simulation.rs:449-463`,
   `crates/gf2-coding/src/simulation.rs:1071-1148`), while the repository
   invariants require traceable numbers, uncertainty, and resumability
   (`AGENTS.md:148-154`).
