You are implementing issue 0c21cb1e in the current JIT-managed repository.

**Title:** Migrate gf2-sim BCH consumers — `jit issue show 0c21cb1e` for the full record.
Migrate the gf2-sim campaign binaries and tests consuming legacy BCH constructors to the canonical model, per the investigation inventory. The GPU byte-identity suite for BCH syndromes is owned by the HIP equivalence task and stays out of scope here.

- [hard] REQ-01: Each gf2-sim consumer in this task's inventory builds and passes on the canonical model with unchanged observable campaign behavior.
- [hard] REQ-02: Campaign checkpoint and protocol tests keep their recorded behavioral identity or re-pin fixtures explicitly in the same change.

Gates: `cargo-ci`, `code-review` (the lead runs them on the merged tree).

Inventory (design file group `cutover-sim`): `crates/gf2-sim/src/bin/ebch_osd_awgn_campaign.rs`, `crates/gf2-sim/src/bin/gpu_bch_syndrome_throughput.rs`, `crates/gf2-sim/tests/ebch_osd_campaign_cli.rs`, `crates/gf2-sim/tests/gpu_byte_identity.rs`, `crates/gf2-sim/tests/osd_campaign_protocol.rs`. `crates/gf2-sim/tests/gpu_bch_syndrome_byte_identity.rs` still holds legacy sites but belongs to the HIP equivalence task (c3cc5226, done): report it as a footprint finding with the exact lines; do not edit it.

## What matters here

- gf2-sim is a thin consumer (`@/invariant/library-first-generality`): construction, encoding, and matrix access come from gf2-coding's canonical surface; add no BCH logic in gf2-sim.
- `@/invariant/deterministic-seeded-execution` and `@/invariant/campaign-resumability`: campaign binaries keep their seeds, outputs, and checkpoint formats; where a fixture pins a legacy generator or a byte identity, either the canonical model reproduces it (assert so) or the fixture is re-pinned explicitly in this change with the reason in the commit and the test's rustdoc (REQ-02).
- The HIP/GPU paths (c3cc5226) already run on the canonical model; `gpu_bch_syndrome_throughput.rs` should construct through the canonical code and hand the kernels what they already accept.
- The DVB-T2 code, where a consumer uses it, is the canonical DVB-T2 code (97410c80).
- GPU tests self-gate on device availability; run what the host allows and say what did not run.

## Verification before you report (paste outputs)
- `./scripts/cargo-budget.sh --test cargo nextest run -p gf2-sim --cargo-profile ci-test --profile ci -E 'test(ebch) | test(osd) | test(gpu_byte_identity) | test(campaign)'` with per-test wall times.
- `./scripts/cargo-ci.sh` from the worktree root; paste the per-step table.
- The REQ grep at HEAD (empty over the inventory); `git diff --stat main...HEAD`; the leak-check output.

## Report
Under 2500 characters first: branch tip, per-file summary, any fixture re-pin with its reason, the cargo-ci table, footprint findings, anything unverified.
