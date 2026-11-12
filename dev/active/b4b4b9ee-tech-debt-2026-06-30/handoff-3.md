# Handoff: Technical debt assessment remediation (2026-06-30) (b4b4b9ee), session 3

**Date:** 2026-11-13
**Prior handoffs:** `dev/active/b4b4b9ee-tech-debt-2026-06-30/handoff.md`, `dev/active/b4b4b9ee-tech-debt-2026-06-30/handoff-2.md`

## Current state

- Lead session `gf2-3d` under steward `gf2-08`; the epic and every claim this session made are assigned to `agent:gf2-3d`.
- Children: 37 done, 10 in progress, 42 ready, 69 rejected.
- Landed on main with all gates passed and closed: `ad978596`, `d1e5db10`, `75a71fab`, `8178ef79`.
- In progress, each on branch `worktree-agent-<id>` in `.agents/worktrees/agent-<id>`, rebased on main during the night:
  - Gated branches, held by the steward's landing rule until the steward releases them: `32a8bc0e` (rework 1 done, re-gate due), `e29fd776` (rework 1 done, re-gate due), `9ff05f12` (done, gates due), `1b7228b9` (`46a2feeef`, partial: final-design suite result and the four-concurrent-run evidence still to re-run; lands after `f21b25ac`, whose `profile.ci` gpu-serial override must merge into the `profile.default` one, which `scripts/check-gpu-lock-filter.py` on the branch enforces).
  - Code-review failed, rework 1 due: `22bb2dcd` (4 findings), `697b9178` (2 findings), `d73ac633` (3 findings; brief written).
  - `f21b25ac`: cargo-ci passed; code-review fails only on REQ-04 (upstream report, owner action) and REQ-05 (idle-host runs, after the window). Steward ruling: hold; re-gate after the REQ-05 runs; nothing lands against a failing review.
  - `5a3abee1`: committed (`b3f54754f`), tests and gates not run.
  - `51f299d2`: done by the worker (`e2bcb3d84`, `scripts/cargo-features.sh`), checks with and without ROCm passed; gates not run.
- Filed this session: `8178ef79` (done), `f2c15867`, `1b7228b9`, `903dc74c`, `2e6e3557`.
- Nothing pushed.

## What happened

- Triage of the 50 ready children against main by six read-only workers: 38 valid, 11 amended for stale text (`40912427c`), `eb5cae22` rejected as obsolete (the be6d6f45 encoder rewrite removed its code). Codex's obsolete verdict on `c04d3f1c` was overruled: `bb11c2e6` chose abort-resumably, `c04d3f1c` asks for the deterministic fallback beyond it.
- `d1e5db10`'s verifier found seven `standard_u128` entries irreducible but not primitive (degrees 84, 88, 100, 108, 110, 112, 124), confirmed with PARI/GP; `8178ef79` replaced them with the lowest-weight primitive polynomial of each degree, each confirmed with PARI/GP.
- `ad978596` removed every `missing_safety_doc` allow, but the lint skips crate-private items, so nothing enforces it in the `x86` modules: filed as `f2c15867`.
- `f21b25ac` isolated the GPU test aborts to the amdgpu driver under hardware runlist oversubscription, reproduced by a minimal HIP program with no gf2 code. The draft upstream report is under `.agents/scratch/upstream/amdgpu-runlist-fault/` for the owner to file.
- `1b7228b9`: the owner enabled nextest's experimental `wrapper-scripts` for a host-wide GPU lock, applied to `gpu-serial` tests only. nextest rejects `group()` in script filters, so the filter repeats the group's expression with a cargo-ci drift check; the decision is recorded in the issue on its branch.
- e29fd776 design choice: `LdpcDecoder::decode_soft` decodes at the shared `SOFT_DECODE_MAX_ITERATIONS` (REQ-02's first branch), reported to the steward.
- Three main holds granted to lead `gf2-69` (daef2dc3, 3478cb98, 595e2fe5) took about two hours of the night.

## Next

1. Wait for the steward's release after the window. First work: the three reworks of step 2. Then re-run gates in this order: `5a3abee1`, `51f299d2`, then the gated branches `32a8bc0e`, `e29fd776`, `9ff05f12` (land them only when the steward releases them).
2. Dispatch rework 1 for `d73ac633` (brief at the lead's scratchpad is lost with the session; rebuild it from the failed code-review findings), `22bb2dcd` and `697b9178` from their failed code-review findings.
3. `f21b25ac`: run REQ-05 on an idle host before the leads start, record the runs, and re-gate. Then land `1b7228b9` after it; its rebase must move `f21b25ac`'s new `[[profile.ci.overrides]]` gpu-serial entry into the `profile.default` override.
4. Next wave from the ready set; held until steward release: `0afbd34a` (bench-window scripts), `2e6e3557` (CI), `903dc74c` (Kani toolchain).
5. `697b9178` does not close on its three unverified Kani harnesses; `903dc74c` covers running them.

## Traps

- Gate runs live in each worktree's ignored `.jit/gate-runs/`; after merging a branch, copy `.agents/worktrees/agent-<id>/.jit/gate-runs/<id>-*` into the main checkout's `.jit/gate-runs/` before `jit issue update --state done`, or jit fails with "Failed to load gate runs".
- Running a gate's code-review before its cargo-ci passed fails the review on "cargo-ci has never run"; run cargo-ci first.
- Concurrent worker test runs pushed load above 30 and fast-tier tests hit the 15 s kill. Every test run and cargo-ci gate takes `flock /tmp/gf2-lead-tests.lock` with `--test-threads 8`; a Codex worker waiting on that lock gives up silently.
- A lead triage note dismissed the `trtrm` item of `d73ac633` as fixed; `u.submat(h..m, h..m)` is the whole U22 block, so the report holds. Verify a dismissal against the code path, not the call site.
- Killing other processes by PID was refused by the permission classifier; stop only tasks this session started (TaskStop) and pause workers by message.
- `.agents/upstream/` is not ignored; staged material goes under `.agents/scratch/`.
- Kani cannot start on this host: `cargo kani` uses rustc 1.93 nightly, older than MSRV 1.95.

## Open questions for the invoker

- `standard_u128_irreducibility_note` now states primitivity, so its name no longer fits; rename with a cutover in a new issue, or keep. Recommendation: rename.

## References

- Epic `b4b4b9ee`; charter `dev/research/2caf738d-charter.md`; steward handoff `dev/active/2caf738d-1-0/handoff.md`.
- Upstream report draft: `.agents/scratch/upstream/amdgpu-runlist-fault/` (host-local, ignored).
