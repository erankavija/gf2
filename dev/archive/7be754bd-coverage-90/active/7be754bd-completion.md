# Epic 7be754bd — Raise per-crate library coverage to 90% — Completion Report

## Outcome

All five production crates report library line coverage ≥ 90%, verified by the CI
coverage run and reflected in the deployed README badges.

| Crate | Before | After (CI-deployed badge) |
|-------|-------:|--------------------------:|
| gf2-core | 89.5% | **90.6%** |
| gf2-coding | 87.7%¹ | **91.1%** |
| gf2-algebra | 89.6% | **91.4%** |
| gf2-sim | 78.9%¹ | **92.2%** |
| gf2-kernels-simd | 92.6% | **92.6%** |
| workspace | 88.3%¹ | **91.2%** |

¹ Headline badge before the measurement fix; the drag was CLI binaries in the denominator.

## Metrics

- Children: 5 (1 config + 3 crate-test + 1 closeout), all `done`.
- Waves: 3 (config → 3 crate tasks serialized → closeout).
- Worker dispatches: 4 (algebra, core, sim, sim-rework).
- Rework cycles: 3 total — exclusion config ×1 (missed stale artifacts + test-support `src/` modules), gf2-sim ×1 (REQ-2 drain/failure targeting) + a lead-applied clippy fix (hip-gated `GpuBitId2` dead code).
- Escalations: 1 (test-code metric — see below).

## Success-criteria mapping

- Epic REQ-1 (every crate ≥90%): delivered by `e192b14d` (measurement) + `1fa9c5d0`/`ee304787`/`6a5f0305` (tests); confirmed by CI.
- Epic REQ-2 (exclude non-library code, documented): `e192b14d` — `--ignore-filename-regex` over `src/bin/`, `tests/`, `benches/`, and the cfg(test-support)-gated helper modules; rationale documented inline in `.github/workflows/ci.yml` and `scripts/generate-coverage-badges.sh`.
- Epic REQ-3 (README badges show ≥90%): CI coverage job deployed ≥90% badges to the `badges` branch; README references resolve.

## Key autonomous decisions

- **Two-lever approach.** Split the gap into a measurement fix (exclude CLI binaries + test-support modules) and genuine test additions. Lever 1 alone cleared gf2-coding and kernels-simd; only ~356 real-test lines remained.
- **`strassen.rs` kept in the denominator.** It is cfg(test-support)-gated but is genuine algorithm code (94% covered), not a test helper — excluding it would have under-counted.
- **Serialized the crate tasks.** The project's worktree dispatch scripts are absent and same-checkout parallel cargo/git is trap-prone; serial execution avoided target/ races and commit interleave.
- **Genuineness enforced in review.** Every crate's coverage gain was spot-checked to confirm it exercised previously-uncovered *library* functions (oracle/invariant assertions), not just self-covered test lines.

## Escalation log

- **Test-code metric (resolved by user).** cargo-llvm-cov counts `#[cfg(test)]` code, so the metric is partly self-covering. Investigated exclusion mechanisms: `#[coverage(off)]` is nightly-only (× 193 modules), region post-processing is inaccurate, and the accurate `llvm-cov -name-allowlist` path needs fragile custom two-pass tooling. Presented options; user chose to **keep the file-level metric and enforce genuineness in review** rather than build fragile tooling.

## Issues discovered during execution

- Pre-existing dangling JIT dependency (`118a0091` → deleted `ec530af9`) surfaced by `jit validate`; not touched (out of scope).
- The working-tree `coverage.json`/`badges/` are gitignored generated artifacts that go stale as tests land and can mislead the code-review gate; refreshed them from authoritative runs before each review.
- Local `cargo-ci` gate builds with the `hip` feature; a test helper gated only for non-hip became dead code under it (`GpuBitId2`). Fixed by matching the cfg.
- The README referenced coverage badges for four crates but not `gf2-kernels-simd`, which was being generated and deployed all along; the closeout added it (all five production crates now shown).
- Closeout task was created with a `code-review` gate only, so its REQ-3 (no-regression) had no issue-scoped evidence; a `cargo-ci` gate was added with user approval and passed.
- Nightly "Slow tests" is red on pre-existing issues unrelated to this epic (permanent `n28/n32` and IRA cross-checks exceeding the 120s slow-tier cap; a DVB-T2 test needing ETSI vectors absent from CI). Left for separate follow-up.

## Trace

Commits `d7e92262`..`57921df6` on `main` (pushed); CI run green (Test, Coverage Badges, Kani, Lean). Coverage badges live on the `badges` branch.
