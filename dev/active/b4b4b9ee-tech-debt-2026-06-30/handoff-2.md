# Handoff — Technical debt assessment remediation (2026-06-30) (b4b4b9ee) — session 2

**Date:** 2026-11-10
**Prior handoffs:** `dev/active/b4b4b9ee-tech-debt-2026-06-30/handoff.md`

## Current state

- Epic `b4b4b9ee` is `backlog`: 59 ready, 8 in progress, 21 done, 65 rejected of 153 children.
- Sibling epic `86b9c719` (quality, documentation, tech debt) was triaged in the same session: 13 ready, 5 backlog, 9 done, 26 rejected of 53.
- Main at handoff: `099172261`.
- Work ran as direct fix branches from a worker session under a lead session, without waves. `progress.json` was not updated this session and does not reflect the state below.
- Active claims, all `in_progress` under `agent:gf2-63`, all merged to main, none taken through its gates (cargo-ci, code-review): `069e3c33`, `a57cb8d0`, `bc4eb125`, `1ffa72fc`, `39cbde20`, `7a5ee7c7`, `a44566c0`, `01877f76`.
- `8a8a3bc9` is merged to main and still `ready`, unassigned.
- Open escalations: `4f21da96` (see open questions).

## What just happened

- Triage of all 183 open children of both epics against the code at `a64712c6f`: 83 rejected (36 already done or their code removed, 20 naming or prose conventions and umbrellas, 27 evidence machinery), 22 rewritten to lean criteria, 5 duplicates folded into `bb64ee0e`, `d73ac633` and `f21b25ac`.
- `c9022d14` was held back from rejection: it is a rename, but it enforces `@/inv/caller-trusted-fast-paths`.
- Correctness fixes merged, each with a test that failed before the fix:
  - `069e3c33` (`2260410e2`): encoder cache persists the column selection in a `.cols` file.
  - `a57cb8d0` (`a6e7196ea`): k = 0 GEMM entries write zeros.
  - `bc4eb125` (`5fdfe7eb7`): panel GEMM u32 accumulator overflow, shared pack helper, PLE window check.
  - `1ffa72fc` (`0bfbc3bc6`): GF(2^m) GEMM safe wrapper checks shapes in every build.
  - `39cbde20` (`89e0910d3`): min-sum sign of ±0.0 and NaN agrees across AVX2, remainder and scalar.
  - `7a5ee7c7` (`6b89f1bcb`): Brännström J-function fit, citation `Brannstrom2005`.
  - `a44566c0` (`4872af8e7`): Barrett wide degree range asserted, `new_unchecked` and unused constants removed.
  - `01877f76` (`19bf59139`): k = 0 field-expression products write zeros, Sum checks its shape.
- `8a8a3bc9` (`8ca0778b6`): DVB-T2 bit-to-cell demultiplexer and cell mapping per ETSI EN 302 755 V1.4.1 clauses 6.2.1 and 6.2.2. All 8100 TP08a cells of VV001-CR35 (256-QAM, rate 3/5) match; before, 8043 were wrong. The checkpoint hash covers the cell mapping. Four pinned waterfall points moved down by 0.1 to 0.5 dB.
- `3464c241` rejected as obsolete: the NR encoder rewrite (`be6d6f45`) removed the code path.
- Accepted deviations: `39cbde20` REQ-03 has no object since the assembly listings are removed; `7a5ee7c7` REQ-03 uses a quadrature reference in place of tabulated values; `a44566c0` keeps `NAME`, which `test_random_matrix.rs` uses.
- Repository changes made by the lead in the same session: committed assembly listings and the `asm-artefact-present` gate removed; `worktree.write_policy = "allow"`.

## What to do next

1. Merge `jit-1015ae73` (`49efa2609`, `GF2_CCX1_CPUS` override for the lock wrapper) and `jit-07fe99b8` (`a11eeff92`, quiet-host wait in `run-window.sh`) after the running benchmark window ends. Set `LOAD_MAX` to 4.0 in `07fe99b8` first, by owner decision.
2. Take the nine merged fixes through their gates and complete them, or release the claims of `agent:gf2-63`.
3. `07a1813a` (word-XOR butterfly for stride ≥ 64): worktree `.agents/worktrees/fix-07a1813a` on branch `jit-07a1813a` at `1e9b9f95e` holds an uncommitted, unreviewed edit to `crates/gf2-core/tests/polar_tests.rs`. Review or reset it, rebase onto main.
4. `c9022d14` (`ConstExt::new` to the `_unchecked` convention): not started.
5. `32a8bc0e` (one index-ordered parallel primitive in `gf2-core`): not started; `.agents/worktrees/fix-32a8bc0e` is clean.
6. `e29fd776` (one `SoftDecoder` contract; `LdpcDecoder::decode_soft` returns hard decisions): not started; the LDPC decoder files are free.
7. `a33fda32` and `ccaed736`: start after reading the shortened-code encode path as rewritten in `040bc2b8`.
8. Remove stale worktrees whose branches are merged: `agent-fix`, `fix-01877f76`, `fix-1ffa72fc`, `fix-39cbde20`, `fix-7a5ee7c7`, `fix-8a8a3bc9`, `fix-a44566c0`, `fix-a57cb8d0`, `fix-bc4eb125`, `tracker`, `agent-de311215-pin`.

## Traps — do not repeat these

- Editing a worktree a benchmark-window queue line runs from. Incident: edits to `quick-compare.sh` in `.agents/worktrees/agent-de311215` at 07:03Z changed the script under the running shell; the job exited 2 without its NR table and the next job profiled uncommitted code. Do task work in a separate worktree; queue lines run from detached run worktrees nobody edits.
- Running builds during a window. Incident: a 40 s `cargo clippy` at 07:03Z overlapped the NR w24-b16 cells, which had to be marked disturbed.
- Transcribing standard tables from memory. `8a8a3bc9` stopped until ETSI EN 302 755 was on the host; `pdftotext -layout` garbles table columns, so each row is confirmed against the PDF page.
- Judging an issue by its title. 36 open issues were already done or their code removed; verify against the code before starting one.
- Merging two kernel branches that both regenerate a shared generated file. The listings are gone, so this cause is resolved; the general form stays: rebase the second branch onto main after the first merges.
- `git merge` does not run the pre-commit hook. Main went 43 bytes over the tracked-tree budget through merges; run `python3 scripts/check-tracked-budget.py` after a merge.
- Running `jit issue claim` in the main checkout from a worker session leaves uncommitted tracker changes there that block the lead's merges. Run `jit` from the worker's own worktree.

## Open questions needing invoker input

- `4f21da96` removes `scripts/cargo-budget.sh`, which `AGENTS.md` requires as a prefix and which takes the shared side of the bench mutex inside a window. Options: go ahead and change the contract, or reject. Its duplicate `20f5343b` follows the same decision. No recommendation recorded.
- `10a729d3` (divide-and-conquer `batch_gcd`): rewritten with an aspirational target, but its premise is likely false. Options: keep, or reject. Recommendation: reject unless an indication shows a gain.
- `de3ec390` was kept separate from `4dd5372a`; it is fixable now with index loops in `gfp/specialized.rs`. Confirm it stays open.

## Reference artefacts

- Epic `b4b4b9ee`; assessment report `dev/active/b4b4b9ee-tech-debt-2026-06-30/b4b4b9ee-assessment-report.md`.
- The triage list with per-issue verdicts and evidence was a session scratch file and is not tracked; the tracker holds its outcome.
- Decode comparison harness: `experiments/aff3ct-ldpc-gaps/quick-compare.sh` and `profile-routes.sh`.
- ETSI EN 302 755 V1.4.1 is host-local under `.agents/ext/etsi/`; the VV001-CR35 vectors are read through `DVB_TEST_VECTORS_PATH`.
