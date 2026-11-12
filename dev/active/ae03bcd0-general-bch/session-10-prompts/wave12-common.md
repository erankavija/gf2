## The canonical surface you migrate to

- Construction: `gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance, RootExponent, RootSelection, BchLength}` with the delegating conveniences `BinaryBchCode::primitive_narrow_sense(extension, designed_distance)`, `primitive_narrow_sense_auto(m, designed_distance)`, `from_generator(...)` etc. (see `crates/gf2-coding/src/bch/spec.rs` module docs and the crate README). A legacy `BchCode::new(n, k, t, field)` becomes the primitive narrow-sense construction over the same field with designed distance `2t + 1`; assert once that the derived `k` equals the legacy `k` and the generators agree, then drop the legacy call.
- DVB-T2: the canonical DVB-T2 code and decoder in `crates/gf2-coding/src/bch/dvb_t2/mod.rs` (landed by 97410c80; read its module doc for the constructor, the decoder, and the declared layout). The legacy `BchCode::dvb_t2` now lives in `bch/core.rs` and is deleted by 4a2baa12 after you.
- Encoding: `traits::block::BlockEncoder` (`encode`, allocation-free `encode_into`, workspace batch, `crates/gf2-coding/src/bch/encode.rs`); parallel batch encoding and the dispatch seam (`EncodeFamily`) are in the same module.
- Decoding: `bch::core::BinaryBchDecoder::new(&BinaryBchCode)` with `correct_in_place` (no allocation) and `decode` (diagnostic `BchDecodeReport`), outcomes `BchDecodeOutcome::{NoErrors, Corrected, Uncorrectable}`.
- Matrices: `traits::block::{GeneratorMatrixAccess, ParityCheckMatrixAccess}`; canonical BCH matrices are `G = [I_k | P]` in the user layout with `is_systematic() == true` and `H = [-Pᵀ | I]`; `bch::matrix::CachedMatrices` is the opt-in cache.
- Extended BCH: `Extended<BinaryBchCode>` / `ExtendedBchComponent` (6d67e57e); legacy `ExtendedBchCode` is deleted by 4a2baa12.
- The compatibility boundary `traits::compat::binary_v1` stays for non-BCH families; a BCH value passed through a v1 interface uses the blanket adapter. Do not migrate non-BCH families.
- Test support: `gf2_coding::test_support` (feature `test-support`) has the corpus visitor, seeded messages, and shared conformance cases; `crates/gf2-coding/tests/bch_conformance.rs` shows their use.

## Shared rules

- Resolve before acting: `jit item show @/issue/ae03bcd0/requirement/REQ-10 @/issue/ae03bcd0/requirement/REQ-13`, invariants `@/invariant/canonical-cutover @/invariant/convention-convergence @/invariant/backend-behavioral-equivalence @/invariant/deterministic-seeded-execution @/invariant/behavioral-evidence-validity @/invariant/present-tense-prose @/invariant/no-deferred-defects @/invariant/test-tier-budgets`, plus your own issue's requirements. Read `AGENTS.md` in full and the design's consumer-family tables (`dev/active/ae03bcd0-general-bch/bch-api-design.md` from line 925).
- Test-first: before changing a site, identify the existing assertion or output that pins its observable behavior; where none exists, add a minimal one on the current behavior first, so "unchanged" is evidenced.
- REQ check at the end: `rg -n "BchCode::dvb_t2\b|BchCode::new\(|BchCode::from_generator\(|BchEncoder::new\(|BchDecoder::new\(|ExtendedBchCode\b|bch::core::" <your inventory files>` returns nothing. Report any file outside your inventory that still holds a legacy site as a footprint finding; do not edit it.
- Nothing "legacy"/"now"/"previously" in permanent prose; no dates; no root-absolute Markdown links; no TODO/deferred markers.
- Stage explicitly; never `commit -am`; conventional subjects under 72 chars with the scope `(jit:<id>)`.
