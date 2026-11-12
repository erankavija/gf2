# Handoff: Competitive benchmarking against Magma/Sage/AFF3CT (d77176e5), session 4

**Date:** 2026-11-12
**Prior handoffs:** `dev/active/d77176e5-competitive-benchmarking/handoff.md`, `dev/active/d77176e5-competitive-benchmarking/handoff-2.md`, `dev/active/d77176e5-competitive-benchmarking/handoff-3.md`, `dev/active/d77176e5-competitive-benchmarking/handoff-4.md`

## Current state

- Lead `gf2-3d` succeeds `gf2-7b`; steward `gf2-08`. Nothing is pushed and no worker of this lead is running.
- The owner armed the night window for 2026-11-13 02:00 local. The queue, `dev/scripts/bench-window/queue.tsv`, is unchanged from handoff-4. The run worktrees `agent-de311215`, `agent-fd9d5416` and `jit-259acf37-run` are untouched.
- Charter D-7 (owner, `a027eae86`): the counted BCH cell times `encode_batch_into` with a reused workspace, whose output contract equals the reference's. The allocating `encode_batch` is its own row without a 1.0 requirement.

| issue | state | what remains |
|---|---|---|
| `ebfaff31` | done | doc-review passed at `8f30ed4f0`; closed `19b7adbc9`. |
| `a6039c72` | in progress | cargo-ci and code-review passed at `a6b8921f0` (merge of `c37094754`). REQ-01 causes are recorded in the issue. REQ-03 needs a window line for `experiments/gf256-baselines/run.sh` after tonight's window; then record the table and close. |
| `60c82c4c`, `5e71e993`, `92ec2e2b` | backlog | Designs on branch `worktree-agent-60c82c4c` (worktree kept), unmerged: B2 `5f599c36d`, B1 `52ecb542d`, W2 `6f5b09e78`. Merge after the window, then `jit doc add` each `dev/active/<id>/design.md`. D-7 is cited in the B1 and B2 issue notes. |
| `c6b20bec` | backlog, new | `FieldId::prime` allocates an `Arc` per call; child of d77176e5. |
| `2bca2b28` | backlog, new | `Shortened` parity-check restriction materializes the mother matrix; child of d77176e5. |
| `fd9d5416`, `259acf37`, `de311215` | as handoff-4 | Record tonight's tables (handoff-4, "Tonight's window"). |

## What happened

- ebfaff31: rework 2 (Claude Sonnet) fixed F1 to F5 and found four more instances. The lead dropped the `55c3e433` citation, because that issue records no values. Doc-review run 2 failed on the srsRAN adapter build and on missing citations for the comparisons without an in-tree harness. Rework 3 (Codex gpt-6-luna) fixed both, and run 3 passed.
- F3: seven issues (`07ca8585`, `1358d6ab`, `31727c57`, `eda07788`, `12fdeb5b`, `53c5a8c0`, `ddd0c6ee`) carry a doc reference of type `evidence`. It pins `docs/reference/performance-evidence.md` at `0dda434248df6f1241fa541797e47698c867d32f`, the page with the measured values. gf2-69 confirms the 45ecf975 history rewrite remaps pins.
- a6039c72 (Codex gpt-6.1-sol, high): two causes. The axpy kernel's scalar alignment head and tail are replaced by overlapping edge vectors. The harness's misaligned right operand made the product's loads split cache lines; the product now copies the operand once into aligned storage when rows and blocks are whole strips.
- BCH gap designs (Claude Opus): the steward accepted the 92ec2e2b trait changes, and the owner ruled D-7.

## Next

1. After the window: record the handoff-4 tables, then close `fd9d5416` and `ae03bcd0` and gate `259acf37`.
2. Merge `worktree-agent-60c82c4c` and link its three designs.
3. Queue a `a6039c72` window line after the window has run; record the table and close.
4. Get the steward's answer to the open question below, then break down the BCH gap issues for implementation.

## Traps

- The traps of handoff-2 to handoff-4 stay in force.
- `jit doc add` scans the document for assets by default and copied 22 dead external receipt URLs into each issue. Pass `--skip-scan`.
- The cargo-ci gate times out at 900 s on a cold main build after a merge. Run `./scripts/cargo-ci.sh` in the main checkout first.
- The execution-lead skill's scripts include no landing script. Merge with `git merge --no-ff` and a conforming `chore(jit:<id>): merge ...` subject.
- Count subject lengths before committing on main: two subjects this session came out at 72 characters. One was rewritten before its merge; one was amended at HEAD.
- `cd` into a worktree in a Bash call moves the session's working directory. Use `git -C` and absolute paths.
- gf2-69 lands `daef2dc3` tonight (gated by 01:15). After it, `scripts/check-tracked-budget.py` refuses result tables and run records, and `progress.json` files hold no measured values (owner ruling C2).

## Open questions for the invoker

- How should the allocating `encode_batch` drop its per-codeword allocation (D-4)? Option (b1) stores short `BitVec`s inline in gf2-core and keeps the `Vec<BitVec>` batch convention shared with LDPC. Option (b2) adds a matrix-returning method, which every `encode_batch` would need under convention-convergence. The design recommends (b1), which changes gf2-core's most-used type.

## References

- Progress: `dev/active/d77176e5-competitive-benchmarking/progress.json`.
- Charter: `dev/research/2caf738d-charter.md`.
