# Handoff: Separate experiment code from the library crates (45ecf975), session 39

**Date:** 2026-11-12
**Prior handoffs:** [handoff-35.md](handoff-35.md) and its predecessors.

## Current state

- `@/issue/daef2dc3` is in progress and landed on main (merge `d490c907c`).
  - `cargo-ci` passed.
  - `code-review` failed with two findings in run
    `c398deb4-ec1e-4dd9-afed-d5984f155bdc`.
  - The owner's rule stops the work here. The steward `gf2-08` decides the next
    step.
- The two findings:
  - F1: two tracker asset entries name deleted files (`a82f2dd9`'s Unpaywall
    JSON, `912f1008`'s `investigation.md`), and `scripts/check-references.py`
    checks only `documents[].path`.
  - F2: `dev/research/permanent_wave_gpu/README.md` names
    `MeasurementPurpose::Equivalence` and a `prototype-registry` adapter that no
    longer exist.
- Two tasks under `@/issue/45ecf975` were filed this session. `45ecf975`
  depends on both:
  - `@/issue/0fe85425`: run-time `dev/` path coupling.
  - `@/issue/da6200b8`: four checker design choices.
- `@/issue/67194d70` is untouched. Its worktree `agent-67194d70` stays
  detached at `259366a0b`, and `dev/scripts/bench-window/queue.tsv` is intact.
  Tonight's 02:00 window measures main as of the timer.
- `@/issue/3478cb98`, `@/issue/506f76d4` and `@/issue/595e2fe5` still wait on
  daef2dc3.

## What happened

- Tracker text:
  - 450 descriptions had their citations of deleted files and removed invariants
    rewritten.
  - 77 more had shortened and moved paths written in full.
  - Eight dangling document links were removed.
- The daef2dc3 description records the invariant mapping, the enforcement
  scope, and a "Ruled scope" section. That section lists today's owner rulings
  (C1, C2, C3, C5) and steward rulings (C4, C6, C7, N05, decision records,
  criteria rule, checker task).
- Four pre-gate audits ran, each by a different model: Codex sol, Claude opus,
  Claude Sonnet and Codex terra. Five terra slice audits followed. Each found
  the same classes one file further.
- Fixes from the audits:
  - measured values left comments, plans, progress files, CI and campaign
    comments, and examples;
  - the permanent-statistics analysis fixtures became synthetic;
  - `benchmarks/run.sh` writes under `target/reference-benchmarks/`;
  - the C4 implementation report left `dev/active`;
  - three frozen addenda lost their raw pilot values, with test digest pins
    updated.
- Checker changes:
  - moves count as removals (`--no-renames`);
  - only bare names resolve by suffix;
  - run records are refused by path in every root;
  - cited `/tmp` evidence and `/var/tmp` paths are host-local;
  - all allowances are listed exceptions.
- Criteria amended by the steward's rule: 3eeb57f6 REQ-04, 47c1538f REQ-01 and
  REQ-02, b8206228 REQ-01.
- The steward rewrote D-5 of the charter. The commit is `f89af629a`.
- Ignored benchmark output in the main checkout moved to
  `target/reference-benchmarks/pre-daef2dc3/`.

## Next

1. Wait for the steward's ruling on the code-review failure.
2. If a rework is allowed, the fix is:
   - remove or refresh the two asset entries;
   - make `check-references.py` check `documents[].assets[].resolved_path`, with
     a self-test case;
   - correct the two stale names in the `permanent_wave_gpu` README.

   Then rerun `cargo-ci` and `code-review` under a hold on main from the other
   lead.
3. On a pass:
   - close daef2dc3, first checking that its text names no path it deleted;
   - run 3478cb98: propose the amended 67194d70 wording to the steward before
     applying it;
   - run 506f76d4 and 595e2fe5;
   - rerun the history-rewrite rehearsal of 45ecf975 REQ-04 on a scratch clone.

## Traps

- Single-agent audits sample instead of reading the whole tree, even when told to be exhaustive. Slice the tree for full coverage, and expect every pass to apply a stricter reading than the one before.
- `codex exec` started in the background with a prompt argument waits on stdin forever. Append `< /dev/null`.
- `land.sh` merges with `--no-edit`, and the default subject breaks the commit convention on main. Merge by hand under `jit worktree exec` with a `chore(jit:<id>): merge ...` subject.
- Removing ignore lines exposes old run output in the main checkout as untracked files. Move it under `target/` before landing, or `land.sh` refuses.
- An asset entry inside a document link names a file just as the link does. Deleting a file needs both its links and its asset entries removed.
- `jit issue show` fails in a worktree ("Failed to load gate runs") because `.jit/gate-runs` lives only in the main checkout. Read issue JSON directly there.
- Merging main into a branch that rewrote issue descriptions conflicts on every issue the other lead edited. Resolve field by field, and reapply the citation edit to their newer text.
- The traps of handoff-35 still hold: closing an issue that names a path it deleted, a cold `cargo-ci` exceeding the gate timeout, and concurrent builds timing out the fast tier.

## Open questions for the invoker

- Whether a rework round follows the code-review failure (owner's stop).
- Open from handoff-35: the measured tuning profiles kept by reasoned exception.

## References

- [handoff-35.md](handoff-35.md), [plan.md](45ecf975/plan.md), [history-rewrite-runbook.md](45ecf975/history-rewrite-runbook.md)
