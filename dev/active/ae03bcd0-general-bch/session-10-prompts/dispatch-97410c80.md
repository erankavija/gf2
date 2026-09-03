You are implementing issue 97410c80 in the current JIT-managed repository.

## Issue

**Title:** Migrate the DVB-T2 BCH consumers to the canonical model
**ID:** 97410c80 (run `jit issue show 97410c80` for the full record)

Migrate the DVB-T2 family to the canonical BCH model: the DVB-T2 BCH modules, LDPC concatenation, and the BICM harness, per the investigation inventory.

### Success Criteria

- [hard] REQ-01: The DVB-T2 modules build and pass on the canonical model with the DVB-T2 verification vectors unchanged.
- [hard] REQ-02: No file in this task's inventory constructs a BCH code through `BchCode::new` or `from_generator` any longer.

### Gates: `cargo-ci`, `code-review` (the lead runs them on the merged tree; your work must be sufficient to pass both).

### Inventory (the design's disjoint file group `cutover-dvbt2`, `dev/active/ae03bcd0-general-bch/bch-api-design.md` "Consumer migration files"): `crates/gf2-coding/src/bch/dvb_t2/mod.rs`, `crates/gf2-coding/src/bch/dvb_t2/generators.rs`, `crates/gf2-coding/src/bch/dvb_t2/params.rs`, `crates/gf2-coding/src/ldpc/dvb_t2/concat.rs`, `crates/gf2-coding/src/dvb_t2_bicm_harness.rs`. Plus, by lead ruling R-50 below, one relocation into `crates/gf2-coding/src/bch/core.rs`. Every other file that still calls `BchCode::dvb_t2` (about forty across gf2-coding benches/examples/bins/tests and gf2-sim) is migrated by the wave-12 issues 591a1c5e, ef8ff9c4, 0c21cb1e, 227ac5c8 after you; do not edit them, and keep them compiling.

## Addressable context

Resolve before acting: `jit item show @/issue/97410c80/requirement/REQ-01 @/issue/97410c80/requirement/REQ-02 @/issue/203ee826/requirement/REQ-01 @/issue/203ee826/requirement/REQ-02 @/issue/ae03bcd0/requirement/REQ-07 @/issue/ae03bcd0/requirement/REQ-09 @/issue/ae03bcd0/requirement/REQ-10 @/issue/ae03bcd0/requirement/REQ-13 @/issue/1a8f6acd/requirement/REQ-01`, invariants `@/invariant/canonical-cutover @/invariant/convention-convergence @/invariant/library-first-generality @/invariant/standards-vector-conformance @/invariant/backend-behavioral-equivalence @/invariant/deterministic-seeded-execution @/invariant/test-tier-budgets @/invariant/present-tense-prose @/invariant/single-source-prose @/invariant/no-deferred-defects`. Read `AGENTS.md` in full, the design's consumer-family tables (`bch-api-design.md` from line 925: "DVB-T2 BCH definitions and BCH/LDPC concatenation — Canonical: standards constructors form the corresponding spec and declare their user-coordinate layout; their outer encoder uses packed canonical traits"; "DVB-T2 LDPC code, cache, and concatenation utilities — boundary for LDPC, canonical for BCH outer use"; "DVB-T2 BICM FEC wrapper — boundary for its object-safe encoder role; its internal BCH outer encoder is canonical"), and the design's "Derived-code transformations" section (the `Shortened<C>` derivation paragraph).

## Where the current code is

- `crates/gf2-coding/src/bch/dvb_t2/mod.rs`: `impl BchCode { pub fn dvb_t2(frame_size, rate) -> Self }` (line ~91) builds the legacy type through `Self::from_generator(params.n, params.k, params.t, field, generator)` with `generator = product_of_generators(&field, table, t)` from `generators.rs` (the ETSI EN 302 755 Tables 6a/6b minimal polynomials) and `params.rs` (`DvbBchParams`, `FrameSize`). Its tests (lines ~111–270) pin generator degree, the product structure, and encode/decode round trips through the legacy `BchEncoder`/`BchDecoder`.
- `crates/gf2-coding/src/ldpc/dvb_t2/concat.rs`: `DvbT2Concat::new` (line ~213) builds `BchCode::dvb_t2`, `BchEncoder::new`, `BchDecoder::new`; `encode` uses `self.bch_encoder.encode(bbframe)` (~537) and `decode` uses `self.bch_decoder.decode(&bch_codeword)` (~753). The LDPC side (`LdpcCode`, `LdpcDecoder`, the `.gf2` cache) stays on its boundary untouched.
- `crates/gf2-coding/src/dvb_t2_bicm_harness.rs`: no direct BCH construction (verify with `rg -n "Bch|bch::" crates/gf2-coding/src/dvb_t2_bicm_harness.rs`); if it consumes `DvbT2Concat` only, it needs no change and you say so.
- The canonical model: `crates/gf2-coding/src/bch/spec.rs` — `BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense { extension: BinaryPrimeExt::new(Gf2mField::new(16, 0b1_0000_0000_0010_1101))?, designed_distance: 25 })` builds the normal-frame mother (65343 × 65535, generator degree 192; the t = 10 rates use designed distance 21); the short-frame mother is over GF(2^14) with the standard's primitive polynomial. `crates/gf2-coding/src/transform/mod.rs` — `Shortened::shorten_first(mother, count)` takes the systematic fast path (`ShortenedDerivation::SystematicRestriction`, 203ee826) when the mother reports `is_systematic()` and `has_canonical_message_order()` and every removed coordinate is below k; `tests/shortened_fast_path.rs` shows the DVB-T2 witness `Shortened::shorten_first(mother, 33135)` → (n, k) = (32400, 32208). Encoding goes through `traits::block::BlockEncoder` (`encode`, `encode_into`); the mother's allocation-free and workspace batch encoders are in `bch/encode.rs`.
- The hardened decoder: `crates/gf2-coding/src/bch/core.rs` `BinaryBchDecoder::new(&BinaryBchCode)` with `correct_in_place` (no allocation, status/count), `decode` (diagnostic `BchDecodeReport`), `BchDecodeOutcome::{NoErrors, Corrected, Uncorrectable}`, `workspace()`; its rustdoc states the bounded-distance guarantee and the beyond-radius miscorrection semantics.
- The standards layout: `docs/SYSTEMATIC_ENCODING_CONVENTION.md` (6a1b2404) and the ETSI stream test `the_etsi_dvb_t2_streams_encode_to_their_verified_codewords` in `crates/gf2-coding/tests/bch_oracle_agreement.rs` (3f7edef1), which already encodes every VV001-CR35 block through the canonical mother and maps to the standard's descending transmission layout; read it for the exact bit mapping between the canonical user layout and the standard's.

## Lead rulings that bind this issue

- **R-50 (legacy constructor relocation).** `BchCode::dvb_t2` has about forty callers that wave 12 migrates; it cannot be removed here and it may not stay in an inventory file calling `from_generator` (REQ-02). Move `impl BchCode { pub fn dvb_t2 }` verbatim (with its rustdoc and its own tests that exercise only the legacy type) into `crates/gf2-coding/src/bch/core.rs` beside `BchCode::new` and `from_generator`, the legacy surface that 4a2baa12 deletes as one unit; it keeps reading `dvb_t2::params`/`generators`. That is the only edit to `core.rs`. Nothing else legacy-shaped moves.
- **R-51 (decoding).** Generic decoding of derived codes is outside the epic (epic REQ-09; tracked as 1a8f6acd). The DVB-T2 outer decoder therefore decodes through the mother: it writes the received shortened word into a mother-length buffer with zero symbols at the removed leading message coordinates, runs `BinaryBchDecoder` on the mother (zero coordinates are never in error, so the mother's bounded-distance guarantee and its miscorrection semantics apply unchanged), and reads the bbframe from the kept message coordinates. Implement it once as the DVB-T2 module's decoder type (a thin composition over the canonical mother decoder with a reusable workspace, no allocation per call beyond the workspace), document it as the shortened decode via the mother, and cite `@/issue/1a8f6acd` as the tracked home of the generic form (`@/invariant/library-first-generality` names this as the tracked exception). Do not add a decoder to `Shortened<C>`.
- **R-52 (layout).** The canonical DVB-T2 code must produce the standard's bit layout: the codewords, and the decoded bbframes, must equal the legacy `BchEncoder`/`BchDecoder` results bit for bit on every one of the twelve (frame, rate) configurations. Declare the layout the standards constructor uses (epic REQ-07: standards-specific layouts are declared) in the module doc.

## What to build

1. **Canonical DVB-T2 code.** In `dvb_t2/mod.rs`, a constructor that forms the spec and shortens: `pub fn dvb_t2_bch_code(frame_size: FrameSize, rate: CodeRate) -> Result<DvbT2BchCode, BchError>` (or an inherent constructor on a small newtype — choose the shape that keeps `Shortened<BinaryBchCode>`'s canonical traits reachable without a parallel abstraction; `pub type DvbT2BchCode = Shortened<BinaryBchCode>;` plus the constructor is the least surface). It builds the primitive narrow-sense mother over the standard's field (`DvbBchParams` supplies m, the primitive polynomial, n, k, t; the designed distance is 2t + 1) and removes the leading `n_mother − n_bch` message coordinates with `Shortened::shorten_first`; assert in a test that every configuration takes `ShortenedDerivation::SystematicRestriction`, that (n, k) match the table, and that the canonical generator equals the standard's `product_of_generators` (keep `generators.rs` as the standards oracle that test reads; it is standards data, not a parallel construction). Typed errors, never panics, for a configuration the standard does not define.
2. **Canonical DVB-T2 decoder.** Per R-51, in `dvb_t2/mod.rs`: a type holding the mother and a `BchDecodeWorkspace`, with a no-allocation `correct_in_place`-style method returning the outcome and count, and a `decode` returning the bbframe with the typed outcome. Error-injection tests at the radius and beyond it, and the three outcomes, for at least one short and one normal configuration.
3. **`concat.rs`.** `DvbT2Concat` holds the canonical code, encodes through `BlockEncoder`, decodes through the new decoder; its documented behavior (`n_ldpc`, `k_bch`, `k_ldpc`, error handling) is unchanged and its existing tests pass. Keep the LDPC types and the cache untouched.
4. **Differential evidence (REQ-01, R-52).** A test in the module runs, for all twelve configurations, seeded payloads (seed `0xAE03BCD0`) through the legacy `BchEncoder` and the canonical code and asserts bit-identical codewords, then injects up to t errors and asserts the canonical decoder returns the legacy decoder's bbframe. Keep it in the fast tier (measure and paste wall times; the normal-frame mother constructs in ~16 ms). The ETSI stream tests in `tests/dvb_t2_bch_verification.rs` stay on the legacy path until 227ac5c8 and must keep passing; run them (`DVB_TEST_VECTORS_PATH` defaults to `~/dvb_test_vectors`, present on this host).
5. **REQ-02 check.** `rg -n "BchCode::new|from_generator\(|BchEncoder::new|BchDecoder::new" crates/gf2-coding/src/bch/dvb_t2/ crates/gf2-coding/src/ldpc/dvb_t2/concat.rs crates/gf2-coding/src/dvb_t2_bicm_harness.rs` returns nothing at HEAD (the legacy constructor now lives in `core.rs`).
6. **Docs.** `dvb_t2/mod.rs` module doc: canonical usage example, the declared layout, the decoder's mother-composition contract with the `@/issue/1a8f6acd` citation; `crates/gf2-coding/README.md` rows that describe DVB-T2 BCH; `bch-api-design.md` consumer-migration rows for `cutover-dvbt2` gain `@/issue/97410c80` in the same style as the `@/issue/203ee826` rows. `dev/active/4e732b56/workload-selection.md` § 9 states in un-dated prose that the canonical model "does not yet express T2S and T2N" and that the shortened presentations "reach the canonical model with 97410c80 … sequenced after this consumer" (lines ~235–241 and ~275–283): rewrite only those un-dated statements to present tense naming the presentation (`Shortened<BinaryBchCode>` through its systematic restriction, `97410c80`); leave every dated amendment subsection as the record it is, and report any other candidate rather than editing it. No dates in new prose, no root-absolute Markdown links, no "legacy"/"now"/"previously" narration outside the named boundary.

## Out of scope

The forty other `BchCode::dvb_t2` callers (wave 12), `Punctured`/`Extended`, the LDPC family, the GPU syndrome paths (c3cc5226 owns them), benchmarks, and `tests/*.rs`. Report anything you find there as a footprint finding.

## Verification before you report (paste outputs)

- `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding --features test-support --cargo-profile ci-test --profile ci -E 'test(dvb_t2) | test(concat) | test(dvb)'` with per-test wall times.
- `./scripts/cargo-ci.sh` from the worktree root passes in full; paste the per-step table.
- The step-5 grep at HEAD (empty).
- `cargo doc -p gf2-coding --all-features --no-deps` clean on the touched items.
- `git diff --stat main...HEAD`; `.agents/skills/jit-execution-lead/scripts/check-leak-into-main.sh` output.

## Commits

Conventional subjects under 72 chars with the scope: `refactor(jit:97410c80): …`, `feat(jit:97410c80): …`, `test(jit:97410c80): …`, `docs(jit:97410c80): …`. Stage explicitly; never `commit -am`.

## Report

Lead with essentials in under 2500 characters: branch tip SHA, the constructor and decoder shapes (one line each), the differential-test result across the twelve configurations with wall time, the cargo-ci table, the empty grep, deviations from the rulings with the reason, anything unverified. Details (doc edits, footprint findings) in a second message.
