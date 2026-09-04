You are implementing issue ef8ff9c4 in the current JIT-managed repository.

**Title:** Migrate gf2-coding binaries and examples — `jit issue show ef8ff9c4` for the full record.
Migrate the gf2-coding binaries and examples consuming legacy BCH constructors to the canonical model, per the investigation inventory.

- [hard] REQ-01: Each binary and example in this task's inventory builds and runs on the canonical model with unchanged observable behavior.

Gates: `cargo-ci`, `code-review` (the lead runs them on the merged tree).

Inventory (design file group `cutover-bins-examples`): `crates/gf2-coding/src/bin/sim_runner.rs`, `crates/gf2-coding/src/bin/check_encoding.rs`, `crates/gf2-coding/examples/dvb_t2_bch_demo.rs`, `crates/gf2-coding/examples/block_code_intro.rs`. `sim_runner.rs` may already be on `ExtendedBchComponent` (6d67e57e, ruling R-28); verify and report. Also check `crates/gf2-coding/examples/ldpc_cache_file_io.rs`, `examples/ldpc_encoding_with_cache.rs`, and `src/bin/generate_ldpc_cache.rs`: they call `BchCode::dvb_t2` only to size the LDPC cache — if the call is a BCH construction, migrate it in the same way (they are examples/bins of this crate and no other task owns them); report what you did.

## What matters here

- "Unchanged observable behavior": capture each binary's and example's output on a fixed input/seed before migrating (`./scripts/cargo-budget.sh cargo run --release -p gf2-coding --example <name> -- <args>`), then after, and paste the diff (empty, or explained where the canonical model's typed error replaces a panic).
- `dvb_t2_bch_demo.rs` demonstrates DVB-T2 BCH: switch it to the canonical DVB-T2 code and decoder (97410c80) and keep its printed facts identical.
- `block_code_intro.rs` is a teaching example: use the canonical construction, `BlockEncoder`, and matrix access in the shape the crate README recommends; it is a candidate for 4ad869d6's runnable-example inventory, so keep it small and correct.
- Every documented `cargo run` line in these files carries the `./scripts/cargo-budget.sh` prefix.

## Verification before you report (paste outputs)
- Build: `./scripts/cargo-budget.sh cargo build --release -p gf2-coding --bins --examples`.
- The before/after output diffs.
- `./scripts/cargo-ci.sh` from the worktree root; paste the per-step table.
- The REQ grep at HEAD (empty over the inventory); `git diff --stat main...HEAD`; the leak-check output.

## Report
Under 2500 characters first: branch tip, per-file summary, the output diffs, the cargo-ci table, footprint findings, anything unverified.
