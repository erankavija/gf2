# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 8

**Date:** 2026-08-16
**Session number:** 8
**Prior handoffs:** `handoff.md` (3), `handoff-2.md` (4), `handoff-3.md` (5), `handoff-4.md` (6), `handoff-5.md` (7). Their traps remain in force except where superseded below.

## Current state

- Epic: `b8206228` — state: backlog (children executing)
- Wave: **wave 1 of 13 COMPLETE**; wave 2 (`41c3d91d`, `a9284086`) is current, dispatch starting this session
- Children summary: 7/11 direct epic deps done; all wave-1 issues done; interleaved bugs `a2c0db52`, `4fdd781a`, `0b66973a` done
- Active claims: `41c3d91d`/`a9284086` unclaimed at handoff time (check `jit issue show` for later state)
- Open escalations: none
- Progress file: `progress.json` here (current_wave 2, all wave-1 statuses done)

## What just happened

- Session 7 died mid-run after committing `bd4faeb5` (receipts §12/§13) without a handoff; this session reconstructed state from git + jit: `4fdd781a` already closed, profiled run `20260815T181923Z` committed, follow-up `023233c5` (rocprofv2 dynamic-LDS pass) filed under the tech-debt epic.
- Ran `6c7fcb38` gates round 1: both FAILED, one blocking finding each. Code-review: receipts denied any committed F_7 per-lane register prediction while `wave_gf7_equivalence.hip:8-17` states source lower bounds (11 units three-plane; 7/9 units lookup `<1>`/`<2>`). Doc-review: provenance round-2 SQ_WAVES-consistency admissibility text contradicts the applied §13 geometry-based policy.
- Both findings verified real; rework dispatched to codex Luna (attempt 1 of 2), landed as `39e3e3e7`: bounds paired per F_3 convention across §7.1/§7.2/§17 + REQ-07/REQ-09 rows; dated provenance amendment supersedes the SQ_WAVES test, §13 cross-cited; `profiled-run.sh` comment and `analysis.py` diagnostic label aligned.
- Found the same register-model defect in the closed F_5 receipts; filed `0b66973a` (interleaved bug, a2c0db52 precedent), second parallel Luna instance fixed it (`682d5942`) — measured 77/66 VGPRs paired with the `f5_wave_equivalence.hip:9-15` models (n+20 bytes, 11 units).
- Round 2: `6c7fcb38` code-review + doc-review PASS (persisted at `39e3e3e7`, 0 findings); `0b66973a` doc-review PASS first round. Lead six-tier reviews PASS on both; both closed (`d54416b9`).
- Linked `handoff-5.md` to the epic (session 7 had not).

## What to do next

- [ ] Dispatch wave 2 sequentially, `41c3d91d` first (implementation: campaign driver scheduling/shard emission in gf2-sim; gates cargo-ci + code-review + doc-review; deps done, state ready).
- [ ] Then `a9284086` (runtime qualification receipt; now ready). Read its FULL description first — it consumes the three campaigns' committed resource figures and the F_7 paired profiled artifact (`dev/studies/6c7fcb38/profiled-20260815T181923Z/`), and likely needs new lead-run rocprofv3 passes for F_3/F_5 kernels under the bench lock (device work is lead-run per session-7 precedent; `profiled-run.sh` here is the working recipe).
- [ ] Do not run `a9284086` measurement passes while `41c3d91d` compiles/tests — bench host must be uncontended (that is why the wave is serialized into sub-waves).
- [ ] Post-wave 2: `jit validate`, advance progress, commit.

## Traps — do not repeat these

- **Receipt authors search only the README for committed resource predictions and miss HIP translation-unit headers.** Cost gate round 1 on `6c7fcb38` and produced the same latent defect in closed `91605d4d` (fixed under `0b66973a`). Evidence: `wave_gf7_equivalence.hip:8-17`, `f5_wave_equivalence.hip:9-15` versus the receipts' "no committed design states" claims. For any future evidence-bound issue, require the worker to sweep kernel source headers (`dev/research/permanent_wave_gpu/hip/*.hip`, `crates/gf2-kernels-hip/hip/**`) for budget/model statements before writing any absence claim.
- **A run-provenance file that states a planned analysis policy becomes a contradiction when the analysis supersedes the policy.** Doc-review reads provenance and receipts as one prose body (`@/inv/single-source-prose`). Fix pattern that passed review: dated amendment appended to the provenance (original text preserved per `@/inv/falsification-preserved`), receipts section named as the authoritative policy, cross-cited both ways. Do not silently rewrite the original paragraph.
- **Two parallel Luna workers on one checkout are safe only with disjoint file sets, no staging, and lead-only commits with explicit paths** (this session: `dev/studies/6c7fcb38/*` + `dev/active/.../profiled-run.sh` versus `dev/studies/91605d4d/receipts.md`; both told "do not stage, do not commit, do not touch .jit"; lead ran `git diff --cached --stat` before each commit). Do not run parallel gate evaluations against a tree a worker is still editing — this session serialized: worker A commit → worker B commit → all gates.
- All unresolved traps from sessions 3–7 remain in force (see `handoff-5.md` §Traps): staged-diff check before every lead commit; gate-evaluate-then-status confirmation; cargo-ci lock/timeout signature; breakdown.json never edited/resynced; superseded-evidence rule; no two writers on one checkout; Luna cannot commit or touch `.jit`.

## Open questions needing invoker input

None.

## Reference artefacts

- Epic: `jit issue show b8206228`; progress + escalation log: `progress.json` here
- F_7 receipts (final): `dev/studies/6c7fcb38/receipts.md` + `analysis.py`, paired profiled run `dev/studies/6c7fcb38/profiled-20260815T181923Z/` (gates passed at `39e3e3e7`)
- F_5 receipts (corrected): `dev/studies/91605d4d/receipts.md` (`682d5942`); F_3: `dev/studies/047b62ed/`
- Profiled-run recipe (working, policy comment updated): `dev/active/b8206228-permanent-statistics/profiled-run.sh`
- LDS follow-up (tech-debt epic, not a blocker): `jit issue show 023233c5`
