# Handoff — Artifact removal (daef2dc3, epic b8206228) — session 36

**Date:** 2026-11-02
**Session number:** 36
**Prior handoffs:** [handoff-31.md](handoff-31.md) and its predecessors.

## Owner decisions

- The protocol defaults of `@/issue/1634c185` are accepted: Wilson intervals only with one z, shard size 200,000 in precision tiers, a killed process uses no retry, the 12-hour ceiling, and the `--on-unavailable` modes.
- Nothing is published. The repository holds no artifacts and no host-local or unresolvable references.
- Performance claims are sparse, and their evidence is a reproducible harness configuration. A host-specific value serves only as an example.
- `claims-trace-to-artifacts` is misplaced: it left the invariants, and its reporting rule moved to `experiments/AGENTS.md`.

## Current state

- `@/issue/daef2dc3` is in progress. `cargo-ci` passes at `c709110e5`. `code-review` failed three times, so the rework limit is reached and the issue waits for the owner.
- `@/issue/595e2fe5`, `@/issue/506f76d4` and `@/issue/3478cb98` wait on it. Nothing has been pushed.

## Open review findings

1. `gap.out` under the `ae03bcd0` oracle attempt remains, and the eight survey family JSON files hold frozen campaign configuration under `dev/active`.
2. The budget accepts `gap.out`, `run.stdout` and `receipt.yaml`.
3. The tree limit is 12.93% above the tree; the criterion allows 10%, so it falls to at most 35,700,727 bytes.
4. The reference check misses a home-relative stream path, unresolved `.jit` paths, and relative Markdown references inside issue JSON.
5. `resume_identity_test.py` cannot initialize, because the profile launchers it locates are gone. CI does not run it.
6. `dev/index.md` still describes the archive and studies directories.

## Traps — do not repeat these

- **Do NOT commit in the same command as a check whose result you only print.** It happened again this session on the branch.
- **Do NOT add a host-path pattern without anchoring it.** An unanchored data-directory pattern matched ordinary relative paths and rewrote text before it was caught.
- **Do NOT scrub open issues' descriptions blindly.** They name files they plan to create.
- **Do NOT run tracker writes in a linked checkout.** `jit` refuses them by policy; they belong on main.
- **Do NOT use a process-matching kill from a shell whose command line holds the pattern.** It kills that shell.

## Limits

- The Aeneas dependency in `proofs/lakefile.lean` and `proofs/lake-manifest.json` now pins upstream commit `5220259c` by hand. `lake update` and the proof build were not run, because fetching external code was denied here. The `AENEAS_LEAN_DIR` patch in `scripts/verify-lean.sh` is also unrun.
- The `test` step failed once in `cargo-ci` before the second round of fixes, with no recorded test name. A later failure at `frozen_campaign` was a real one and is fixed; the first cause is unknown.
- The gate `permanent-sampling-feas-ci` was removed, including from six closed issues.
- Issue descriptions were rewritten mechanically: removed paths became base names, and host paths became `<local path>`.
- Untracked files remain on this host, which I did not create and did not delete: `benchmarks` (3.3 MB) and `simulation_results` (60 KB) under `dev`, and a `d06a99ed` directory in the zen3 epic directory under `dev/active` (136 MB).
- The history rewrite's path list and rehearsal do not yet reflect this removal, so its force-push decision waits for a new rehearsal.
