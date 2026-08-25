# Handoff — Ordered-statistics decoding as the generator-matrix and syndrome soft-decision baseline (b7157be6) — session 1

**Date:** 2026-08-25T01:10+03:00
**Session number:** 1
**Prior handoffs:** None.

## Current state

- Epic: `b7157be6` — state: backlog (dependency-blocked container, assigned `agent:jit-execution-lead` assign-only)
- Wave in progress: wave 1 of 6 (6 of 7 merged; 835e15fb review-passed, merge pending); wave 2 partially dispatched early (2 of 3 workers running)
- Children summary: bracket P/B done; 0 impl issues done (6 merged awaiting review gates), 4 in_progress with live workers (835e15fb, ac78aff8, f0d6fb9a) or committed-pending-merge (dd6f1665 at 24962b69), 8 backlog
- Active claims: wave-1 seven + wave-2 three claimed `agent:worker`; epic assign-only
- Open escalations: none (D-21 source/metric amendment was approved by the owner this session and is fully reconciled)
- Progress file: `progress.json` here (per-issue status strings carry gate detail)

## What just happened

- Wave 1 (7 issues) dispatched to worktrees; all seven produced work. Codex crashes (tmpfs ENOSPC) forced continuation redispatches for 6beaf008/835e15fb; lead-preserve commits salvaged WIP.
- Merged to main with cargo-ci green on the merged tree: 5dd3539f, 80cead18, 377a7a62, a82f2dd9, 6beaf008, bebe485c. cargo-ci gates passed for the five code issues.
- bebe485c failed lead review R1 (public re-export, duplicated settled-state, pre-amendment metric); Sol rework fixed all three, verified closed at HEAD `38e2175f`.
- Owner-approved amendment D-21: Fossorier1995 is closed-access → pinned source is the 1994 dissertation (`@/citation/Fossorier1994`, registry entry added); comparison metric is BER; eBCH construction identity is a recorded factory decision. Manifest, plan, 4 issue descriptions, dataset provenance doc all reconciled; plan-review and breakdown-review re-passed.
- 5dd3539f code-review: R1 FAIL (2 doc findings) fixed `dc81c2bd`; R2 FAIL (wrong /64 in pivot-probe complexity, introduced by the R1 fix) fixed `f1f4043a`; R3 FAIL (public result fields allow panic past documented check) fixed `ff5dcd6e`; R4 PASSED at 01:30 (`7f037ed3`); only doc-review remains for 5dd3539f.
- Convention-convergence exception for the ordered elimination loop recorded: tracked issue `c0bb2ab1`, cited in rustdoc.
- Coordinated with two neighbor efforts: codex lead (b8206228) reserved the host 02:00–07:40 EEST for its premeasure run; 679cf170 (provenance guard) landed on main at `8be289e9` including the required Provenance-literal fix in `osd_campaign_protocol.rs`.
- Skill improvement (user-directed): worktree build-cache recycling added to jit-execution-lead (seed-on-dispatch + `reclaim-worker-worktree.sh` harvest; toolchain-neutral, git-ignore-based). Repo copy synced at `8a289e9b`. Host pool: `LEAD_CACHE_POOL=/data/gf2-osd-cache-pool` (seeded, 4.8G).
- Reported chronic 5s-budget-edge gf2-sim tests (hybrid_* set, executor_oom_fallback) to the codex lead; 679cf170 session is filing the tracked issue.

## What to do next

- [ ] HOST FREEZE until 07:40 EEST 2026-08-25: no writes to the primary checkout (no merges, no `.jit` writes — gate evaluation writes `.jit` at completion — no cargo-ci on main). Worktree-confined worker activity is fine.
- [ ] Collect reports from still-live workers: ac78aff8 (bccre9o6j), f0d6fb9a (bnj01f0j9). Content-review; commit their trees. DONE already: 835e15fb (b3d76014, review passed) and dd6f1665 (24962b69, review passed) — both merge-ready after 07:40, cargo-ci after each merge (their bases predate the wave-1/679cf170 merges — semantic-merge risk is why they were not merged un-gated tonight).
- [ ] f0d6fb9a (Sol) dispatched ~01:07 (task bnj01f0j9) — collect its report too; worktree agent-f0d6fb9a, target pre-seeded.
- [ ] After 07:40: run code-review+doc-review for 80cead18, 377a7a62, 6beaf008, bebe485c; doc-review+research-review for a82f2dd9; then mark wave-1 issues done and commit per state-commit-patterns.
- [ ] After 07:40: merge 835e15fb (if review passed) then wave-2 branches, cargo-ci after each batch, leak check (`/tmp/lead-pre-dispatch-latest.txt` → snapshot of 213836), reclaim with `LEAD_CACHE_POOL=/data/gf2-osd-cache-pool .agents/skills/jit-execution-lead/scripts/reclaim-worker-worktree.sh <sids>`.
- [ ] Then wave 3 (abd48d99 osd-reprocessing-engine, hard → Opus per model plan in progress.json).
- [ ] Consider follow-up task: digitize Coskun2019 Fig.4 (open CER anchor) as second reference — see NOTE-01 in progress.json.

## Traps — do not repeat these

- **Do NOT let codex workers choose their own build-cache location.** They default to `/tmp` (tmpfs = RAM): two workers filled 16G tmpfs, which OOM-killed shells host-wide (exit 101 codex crashes, Bash tool exit-1 failures). Evidence: tonight's ENOSPC incident, ticks 23:24–23:49. Instead: dispatch from inside the worktree with the in-worktree `target/` (pool-seeded).
- **Do NOT dispatch codex with cwd outside the target worktree.** Codex treats a linked worktree as a separate workspace: cwd=repo-root or another worktree makes the target worktree read-only ("mounted read-only" reports from two workers). Dispatch with cwd INSIDE the worktree.
- **Codex workers cannot commit in linked worktrees** (shared `.git` metadata outside their sandbox). Expect "index.lock: Read-only file system"; the lead commits their trees on the worker branch. Prompts must say to leave changes in-tree and report.
- **Do NOT reclaim worktrees with bare `git worktree remove`** — it deletes warm build caches (lost 9.5G tonight, user-flagged). Use `reclaim-worker-worktree.sh` (harvests to pool first).
- **Do NOT pass a codex prompt via `$(cat file)` written in the same command during disk pressure**, and always redirect stdin from `/dev/null`: an empty prompt makes `codex exec` block forever on "Reading additional input from stdin..." (lost ~1h on bebe485c rework).
- **Do NOT run `jit` state-mutating commands from a worker worktree** — `worktree.write_policy` refuses and earlier in this session the refusal was masked by a wrapper printing rc 0. Run them from the primary checkout and verify the write landed.
- **Do NOT treat gf2-sim `hybrid_*` / `executor_oom_fallback` TIMEOUTs as caused by your merge.** They are chronic 5s-budget-edge tests (pass 3.9–4.95s quiet, flip under load). Verified against an anchored baseline tree. Being tracked by the 679cf170 session.
- **zsh eats `=====`-style separators** (`=word` expansion) — use `---` or quote.
- **Known-red neighbor test on main (NOT ours):** dev/scripts/permanent-campaign-premeasure.test.sh t4 asserts the clean-worktree refusal that 679cf170's landing removed. Red since 8be289e9-lineage; the 679cf170 session owns the fix. Do not chase it as an OSD regression.
- **The manifest is authoritative:** any issue-description change must be made in `breakdown.json` AND the issue, revalidated (`breakdown_manifest.py validate/render`), and plan-review + breakdown-review re-run. Done once this session for D-21; follow the same sequence for any future amendment.

## Open questions needing invoker input

None. (D-21 resolved this session. The codex lead never confirmed whether main cleanliness is load-bearing for the 02:00 run — treat the freeze as binding regardless.)

## Reference artefacts

- Epic: `jit issue show b7157be6`; bracket: P=312200e4 (done), B=8d650253 (done)
- Plan/manifest: `dev/active/b7157be6-osd/plan.md`, `breakdown.json` (D-21 amendment at `ec4b2047`)
- Dataset: `dev/reference_data/osd_ebch_128_64_fossorier1995.{csv,md}` (merged)
- Progress: `dev/active/b7157be6-osd/progress.json` (waves, model plan, pitfalls PIT-01..07 + NOTE-01, rework counts)
- Convergence follow-up: `jit issue show c0bb2ab1`
- Neighbor coordination: peer session gf2-a1 (679cf170, landed)
- Worktrees live: agent-835e15fb, agent-ac78aff8, agent-dd6f1665, agent-f0d6fb9a (+ b820's agent-a39bb161/agent-389aa4de — DO NOT TOUCH; also baseline-check at f7e17a3c — safe to remove)
