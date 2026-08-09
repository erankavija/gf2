# Session handoff: b8206228 planning brackets (2026-08-09)

> **Diátaxis Type:** Reference — resume contract for the `jit-planning-lead`
> run on epic `b8206228` and its prerequisite story `0de41c82`.

## Where planning stands

Both containers carry applied `plan` brackets, claimed by `agent:claude`:

| Container | Planning node P | Breakdown node B | Manifest state |
|---|---|---|---|
| `0de41c82` — wave-parallel GPU kernels (story) | `1b113b9d` (in_progress) | `a2b00bd6` | `dev/active/0de41c82/breakdown.json` + `plan.md`, 18 issues / 26 edges at last check |
| `b8206228` — permanent-statistics campaign (epic) | `912f1008` (in_progress) | `f3dc1bb1` | `dev/active/b8206228-permanent-statistics/breakdown.json` + `plan.md`, 34 issues / 48 edges at last check |

Investigations are complete and linked to both P nodes
(`dev/active/0de41c82/investigation.md`,
`dev/active/b8206228-permanent-statistics/investigation.md`).

## Gate state (the critical part)

`plan-review` has run three recorded rounds on each P node, all FAIL with
strictly shrinking findings (GPU 8→5→4, epic 5→6→2). The escalation at three
failures was raised; **the owner approved a fourth round for both**. The
round-4 fix lists were dispatched to session-local synthesizer agents at
session end:

- GPU: launch-overhead redefined as host-submission duration + device-only
  event span (no cross-clock subtraction); stream-bearing HIP/FFI paths added
  to `hip-event-timing` scope+footprint (kernels hard-code stream 0);
  `harness-event-integration` split into `harness-timing-column` +
  `prototype-harness-adapter`; stale `micro-benchmark-modes` plan reference
  swept.
- Epic: `satisfies:REQ-05` on `model-fit` + `perm-det-figure`,
  `satisfies:REQ-06` on `rare-event-design` + `rare-event-validation`;
  volatile backend-universe sentence removed (fixed enumerated configuration
  list only; missing backends recorded as unsupported).

**Resume step 1:** `git status` — if the manifests carry uncommitted round-4
edits, verify them; if not, apply the fix lists above by hand. Then rerun the
canonical loop for each container (helper:
`.agents/skills/jit-planning-lead/scripts/breakdown_manifest.py validate/render`,
`jit issue batch-create --dry-run`; known-source universes are listed in each
plan's "Investigation sources" tail) and rerun
`jit gate evaluate <P> plan-review` (serialize; ~3–4 min each). On PASS: mark
each P done, commit.

## Remaining pipeline after P passes

1. Invoke `jit-breakdown` per container (manifests are the authoritative
   input; batch-create + bracket wiring).
2. Wire external edges the manifests cannot express (recorded in the epic
   plan's decisions): `preregistration-freeze` → `0de41c82`;
   `interpretive-report` → `76dfd2ff`. Then drop the now-transitively-redundant
   direct edges `b8206228 → 0de41c82` and `b8206228 → 76dfd2ff` (added during
   this session to keep membership labels DAG-backed pre-breakdown;
   `jit dep add --reduce` or `jit validate --fix` handles the reduction).
3. `coverage-preview` + `breakdown-review` gates on both B nodes; repeat the
   terminal assignment simulation; escalate after a third breakdown-review
   failure.
4. Final report per the skill's step 6.

## Owner decisions taken this session (do not re-litigate)

- `0de41c82` retyped `simulation` → `story` to make it breakable
  (user-directed "break that issue as well").
- REQ-01 credit: `satisfies:REQ-01` label on done issue `b488f02c` + direct
  epic dependency edge (user-approved amendment); `protocol-draft` also
  carries in-manifest REQ-01 contribution.
- Sampler + statistics in a new narrow crate `gf2-stats`; campaign driver a
  binary in `gf2-sim` with the checkpoint writer generalised at source.
- Published dataset under `dev/simulation_results/permanent-zero-fraction/`
  (documented divergence from study §7.4).
- G6: build the four-matrix batched AVX2 path (now split across
  `avx2-batched-impl` / `avx2-dispatch-migration` / `avx2-narrative-sweep` /
  `avx2-batched-receipt`).
- Fourth gate round approved for both P nodes after three recorded failures.

## Second-opinion protocol (Codex advisor)

`agent:scientific-hpc-advisor` reviews via the forum:
`FORUM_DIR=/tmp/jit-forum`, script `~/Projects/forum-poc/forum.sh`
(`send`/`recv --as agent:claude`). It issued PASS verdicts on both manifests
under the owner's major-issues-only bar. Its binding statistical corrections
(alpha split across permanent+determinant families, exact binomial tests with
z=3.16/3.36 demoted to planning approximations, predeclared retry-after-halt,
likelihood-on-counts model comparison) are recorded in the epic plan's
decision table and supersede study §7.2/§7.5 on the record.

## Commits this session

`0388762c` (brackets applied), `62aae8ba` (investigations + REQ-01 credit),
`01be30b6` (advisor-reviewed manifests), `977fecb4` (final advisor-round
corrections). Later manifest revisions from the gate rounds are committed with
this handoff; check `git log dev/active/` for the tip.
