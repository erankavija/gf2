You are implementing issue 227ac5c8 in the current JIT-managed repository.

**Title:** Migrate gf2-coding integration tests — `jit issue show 227ac5c8` for the full record.
Migrate the gf2-coding integration tests consuming legacy BCH constructors to the canonical model, per the investigation inventory.

- [hard] REQ-01: Each test in this task's inventory passes on the canonical model with its assertions preserved, or re-pins fixtures explicitly in the same change.

Gates: `cargo-ci`, `code-review` (the lead runs them on the merged tree).

Inventory (design file group `cutover-test-suite`): `crates/gf2-coding/tests/bch_tests.rs` (about a hundred legacy sites), `crates/gf2-coding/tests/dvb_t2_bch_verification.rs`, `crates/gf2-coding/tests/ebch_128_64_reference.rs`, `crates/gf2-coding/tests/backend_integration.rs`, `crates/gf2-coding/tests/grand_phase1_smoke.rs`, `crates/gf2-coding/tests/bch_primitive_verification.rs`. `crates/gf2-coding/tests/bch_oracle_agreement.rs` (3f7edef1) holds one legacy site: report its line as a footprint finding; do not edit it.

## What matters here

- Assertions are preserved: every numeric expectation, vector comparison, and error-case assertion survives; only the construction and the encode/decode/matrix calls change. Where a legacy test asserts a legacy-only behavior (a panic on invalid parameters that the canonical model reports as a typed error; a matrix in polynomial coordinates rather than the systematic user layout), rewrite the assertion to the canonical contract and say so in the test's rustdoc and the commit message (that is the explicit re-pin REQ-01 allows).
- `bch_tests.rs` is large: migrate it in coherent commits (construction, encoding, decoding, matrices) and deduplicate against `tests/bch_conformance.rs` (e1e0e7ff) only where a test asserts exactly a law that suite already asserts for all field classes; keep every binary-specific and decoder law.
- `dvb_t2_bch_verification.rs` moves to the canonical DVB-T2 code and decoder (97410c80); the ETSI VV001-CR35 streams are at the default path on this host (`~/dvb_test_vectors`); the encode, error-free decode, and error-correction cases keep their bit-exact comparisons.
- The decoder tests move to `BinaryBchDecoder` and its typed outcomes; `Uncorrectable` means "no verified correction", never a claim about the transmitted word, so assertions that inferred more must be rewritten (say so).
- Fast tier: keep every test under the 8 s per-test kill on this host; tag anything heavier `#[ignore = "slow: …"]` and measure.

## Verification before you report (paste outputs)
- `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-coding --features test-support --cargo-profile ci-test --profile ci --test bch_tests --test dvb_t2_bch_verification --test ebch_128_64_reference --test backend_integration --test grand_phase1_smoke --test bch_primitive_verification` with per-test wall times.
- `./scripts/cargo-ci.sh` from the worktree root; paste the per-step table.
- The REQ grep at HEAD (empty over the inventory); `git diff --stat main...HEAD`; the leak-check output.

## Report
Under 2500 characters first: branch tip, per-file summary with the count of re-pinned assertions and why, the cargo-ci table, footprint findings, anything unverified. Details (the re-pin list) in a second message.
