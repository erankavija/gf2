# babcf05e handoff 5 — sub-wave 4a recovery closed

## Current status

Sub-wave 4a recovery is complete. The stale-worktree incident from session 6 has been contained: C1 and B1 were salvaged, C3 and B2 were restarted from current main using stale WIP only as reference, all four were integrated on `main`, all required gates now pass, and all four issues are marked `done`.

Current main tip after recovery fixes: `1594b81` (`docs(jit:54a0e75c): document M4RM SIMD safety`).

## Closed 4a issues

| Issue | Outcome | Final notable commits | Gate result |
|---|---|---|---|
| C1 `ec286cee` GF(2^m) batch mul/square | Salvaged and integrated | Original C1 commits plus `340a50d`, `18a4c7d`, `c087d48` | `cargo-ci`, `asm-artefact-present`, `criterion-1.5x`, `code-review` all passed |
| B1 `1c1c4242` BitMatrix transpose | Salvaged and integrated | Original B1 commits plus `82fa25c` | all gates passed |
| C3 `86c09a51` generic Montgomery SIMD | Restarted from current main; stale WIP used as reference | `816f5ed`, `ae5bd22`, `14c4a71` | all gates passed |
| B2 `54a0e75c` M4RM Gray-code table build | Restarted from current main; stale WIP used as reference | `ffbcf68`, `8f628ac`, `aa79de4`, `a9ac70c`, `ee2e2c8`, `1594b81` | all gates passed |

## Recovery decisions and amendments

- User approved the mixed recovery strategy: salvage C1+B1; restart C3+B2 from current main using stale WIP as reference.
- User approved B1 baseline correction from stale `ppc-v0-2026-04-25` to canonical `ppc-v0-2026-04-27`.
- User accepted C1's bundled V3+V4 commit if substantive gates/review passed.
- User clarified helper scripts accidentally committed to this repo belonged under `~/.claude/skills`; keep the repo revert (`b568fc2`) and do not reintroduce them.
- During B1 review, user selected implementing the PSHUFB lane and asm artefact rather than amending the hard criterion. Commit `82fa25c` adds the tested PSHUFB lane while leaving production dispatch on the faster bit-twiddle lane.
- During B2 review, user approved accepting LLVM-emitted `vxorps` as `vpxor`-equivalent for the hard V3 asm mnemonic criterion. The B2 issue description records this amendment.
- C1's aspirational criterion is visibly amended in the issue description: the criterion gate compares `gf2m_batch_unroll4` against `pclmulqdq_barrett_loop_v0` over `m = 8,16,32`, geomean 5.131x.

## Validation performed

- Full fast tier after C1/C3 recovery: `cargo fmt --all -- --check`, `cargo nextest run --workspace --all-features --release --profile ci`, and `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- Full fast tier after B1 PSHUFB/proptest recovery passed; output was large and saved by the tool at `/tmp/copilot-tool-output-1777423802383-ql49fi.txt`.
- B2 final safety-comment fix passed `cargo fmt --all -- --check` and workspace clippy.
- JIT gate matrices for C1/B1/C3/B2 all show 4/4 passed after final fixes.

## Remaining epic work

The epic is not complete. Sub-wave 4b has not been dispatched yet. Before dispatching any new work:

1. Start workers only from current `main`, and verify their worktree merge-base equals current main before handoff.
2. Include the no-JIT-state rule in every worker prompt: workers must not pass/fail gates, mark issues done, or edit `.jit/`.
3. Keep cargo/bench jobs serialized unless workers use separate `CARGO_TARGET_DIR`s.
4. Require per-spiral-step commits and asm/perf evidence before worker completion.
5. For hard criteria mentioning exact mnemonics or algorithms, pre-audit feasibility before dispatch; if LLVM emits an equivalent mnemonic, get user approval before amending the issue.

## Files updated for recovery tracking

- `dev/active/babcf05e-progress.json` records 4a closure, rework counts, user decisions, and final gate summaries.
- `.jit/issues/{ec286cee,1c1c4242,86c09a51,54a0e75c}*.json` record gate results, done states, and issue amendments where applicable.
- `.jit/events.jsonl` contains the gate and state-transition audit trail.

