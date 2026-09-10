# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 7

**Date:** 2026-09-10 (updated as issues complete; see the log at the end)
**Session number:** 7
**Prior handoffs:** [session 1](handoff.md), [session 2](handoff-2.md), [session 3](handoff-3.md), [session 4](handoff-4.md), [session 6 halt](handoff-5.md)

## Current state

- Epic: `1a379447` — state: backlog (dependency-blocked container; assignee `agent:jit-execution-lead`).
- Wave in progress: wave 2 of 7. Wave 3 gains `12fdeb5b` and `1c602857`.
- Wave-2 children: `c077a88b` done, `1d0da41f` done, `c7113c5a` in rework round 1, `eda07788` finished on branch, `04b85d10` in rework round 1, `6fb89a3c` and `26465e6c` running, `6c6b09b1` not started.
- Active claims: lead lease on `c7113c5a` (TTL 4 h); worker assignments on `04b85d10`, `6fb89a3c`, `26465e6c`, `c7113c5a`, `eda07788` (lead).
- Open escalations: none.
- Invoker instructions in force: start no new work this session and finish the running issues. Use no Fable model after the two current Fable workers (`04b85d10` original, `6fb89a3c`) finish; use Opus for hard work and Sonnet for routine work.
- Progress file: [progress.json](progress.json).

### Per-issue state

| Issue | Branch / worktree | State and next step |
|---|---|---|
| `c077a88b` | reclaimed | Done at `ab661b4d`. |
| `1d0da41f` | reclaimed | Done at `cfb99053`. Research advisory stands: the frozen v3 addenda call v1 "superseded", but v1 remains the governing adoption evidence and digest-pinned addenda cannot change. |
| `c7113c5a` | `worktree-agent-c7113c5a-v3` at `38d2c091`, merged at `4edbadf1`, linked at `9aa6c8db` | Rework round 1 on `w-c7113c5a` (Opus, revived). On `9aa6c8db`: cargo-ci and doc-review pass, and both independent reviews pass, but code-review F1 fails (the GF(2^8) dot-product reduction probe reduces an already reduced value, REQ-03) and research-review F1 fails (tables.md cost and derived estimates are bare point estimates). After rework: merge again, re-run both reviews, then all AI gates on one commit. |
| `eda07788` | `.worktrees-local/agent-eda07788-v3`, branch `worktree-agent-eda07788-v3w` at `b2abf17a` | Worker finished. Next: leak check, `git merge --no-ff`, cargo-ci on the merge commit, run `scratchpad/link-eda07788.sh` (contents listed under Reference), two independent reviews, then gates. |
| `04b85d10` | `.agents/worktrees/agent-04b85d10-v3`, merged at `f1a1c975`, linked at `16cfcf35` | Rework round 1 on `w-04b85d10-r1` (Opus). Both independent reviews FAIL: A-F1, profile shares and times are single runs without intervals yet rank candidates; A-F2, seeded inputs lack RNG implementation and version; B-F1, the m=14 BCH cells are labelled DVB-T2 normal frame, but m=14 is the short frame [Etsi2015]. After rework: merge the branch again, re-run both reviews, then gates. |
| `6fb89a3c` | `.agents/worktrees/agent-6fb89a3c-v3` | `w-6fb89a3c` (Fable) running. v3 pilots accepted for all three families; confirmations next. |
| `26465e6c` | `.agents/worktrees/agent-26465e6c-v3` | `w-26465e6c` (Opus) running. Port done; pilots next. |
| `6c6b09b1` | old branch `worktree-agent-6c6b09b1` only | Not dispatched (invoker stop). v3 migration remains; carry the 0x11B framing correction. |

## What just happened

- Reclaimed worktrees of closed issues: `4e732b56`, `6639435f`, `73317b2e`, `a39bb161`, `eaae1b56` (x3), `f547c394-identity-tests`, `b749bdfc` docfix, `9ce34225` gf2-ee-debt and `82dd7384`'s old Claude worktree. Also reclaimed all three of `c077a88b`'s worktrees and `1d0da41f`'s after closure. Branches kept.
- Re-attributed `c077a88b`'s six untagged v3 commits (scope `(c077a88b)` without `jit:`) on main with invoker approval. The revert is `8bf02a69`; the tagged re-apply is `2cc56a51..df3630ce`. The tree is identical (`6b4599ab`).
- `c077a88b`: cargo-ci plus two independent Terra xhigh reviews plus code, doc and research gates all pass on `b8f64b5b` with zero findings. Closed.
- `1d0da41f`: merged at `e36ad350`, where cargo-ci and asm-artefact-present pass. The lead fixed stale findings notes and linked 17 v3 artifacts. doc-review F1 (FieldVec rustdoc promised VPCLMULQDQ) was fixed by the lead in `fecf9e70` (rework 1). Round-2 reviews and all gates pass on `dda44c25`. Closed.
- Created `12fdeb5b` (5G NR rate-matched encoder baselines; invoker chose a task inside the epic). Its background was corrected with invoker approval after `eda07788` falsified two premises. Registered citation `ThreeGpp2017` (TS 38.212 V15.0.0).
- Created `1c602857` (epic task): the public `clmul_wide`/`clmul_wide_slice` bypass capability dispatch, confirmed from source by `c7113c5a`. It depends on `c7113c5a` and feeds `63bad95d`.
- Filed bugs outside the epic: `157c305c` (NR rustdoc says filler LLR +inf, code writes 15.0; depends on `eda07788`) and `f40a1c8d` (gf2-core dead code under partial feature sets; depends on `c7113c5a`).
- `eda07788`: the invoker decided the NR LLR de-rate-matching cell is measured inside the issue as a new first-attempt family. It is confirmatory (m=6): gf2 is 2.87-3.13x faster than AFF3CT depuncture. The DVB-T2 v3 confirmation is accepted but all cells are not-confirmatory (m=12, t=2).
- `c7113c5a`: the host-targeting family is confirmatory; the native family is not-confirmatory (m=21, t=2). The worker sent the findings text and the lead wrote it (hook policy).
- `04b85d10`: finished by Fable; merged. Its reviews failed as above; the rework is on Opus.
- Two workers hit a provider session limit; the invoker reset it and both were revived by message.

## What to do next

- [ ] Re-review `c7113c5a` after rework (resolution table must close code-review F1 and research-review F1): merge, run two reviews on one commit, then all three AI gates; close; reclaim `c7113c5a-v3` and the old `c7113c5a` (`LEAD_RECLAIM_FORCE=1`, branch kept).
- [ ] Integrate `eda07788` as in the table. After closure, reclaim `.worktrees-local/agent-eda07788-v3` (manual harvest plus `git worktree remove`, since the script expects `.agents/worktrees/agent-<sid>`) and the old `.agents/worktrees/agent-eda07788` (force; branch kept).
- [ ] Re-review `04b85d10` after rework: the resolution table must close A-F1, A-F2 and B-F1. Merge, run two reviews on one commit, then gates.
- [ ] Integrate `6fb89a3c` and `26465e6c` when their workers report, with the same sequence.
- [ ] Next session: dispatch `6c6b09b1` (Opus). Close wave 2, advance `current_wave` to 3, dispatch wave 3 (`3be770d5`, `53c5a8c0`, `19513245`, `1d4fd63d`, `5cbb6545`, `2037941f`, `c04dd4ac`, `12fdeb5b`, `1c602857`) at four workers at a time.

## Traps — do not repeat these

- **Do NOT let the review gates see only a link commit.** Session 6 committed `c077a88b`'s work as `fix(c077a88b)` without `jit:`; the code, doc and research reviewers attribute only `jit:<id>` commits and would have reviewed nothing. Check `git log --grep 'jit:<id>'` before gating. Every dispatch prompt now demands `jit:<id>` scopes.
- **Do NOT let a subagent bypass the report-file hook.** Subagent `Write` of `findings.md` is refused ("Subagents should return findings as text"). `04b85d10` and `eda07788` wrote it through a Bash heredoc before the policy existed. Policy: the worker sends the full text to the lead with SendMessage (to `main`), the lead writes it, and the worker reviews and commits; Edit is fine for targeted fixes.
- **Do NOT expect v3 confirmatory cells from a family with history.** P-20 needs 20 expected tail draws at `bootstrap_resamples` 10000: the tail count is 125/m on attempt 1 and 41.7/m on attempt 2. `eda07788` DVB-T2 (m=12), `1d0da41f` (m=10) and `c7113c5a` native (m=21) are entirely not-confirmatory. New families: m <= 6.
- **Do NOT run gates on a commit other than the independent reviews'.** The invoker requires two converging Terra xhigh reviews, then the research gate and the remaining gates on the same commit. A fix after the reviews means re-running both reviews. Commit JIT state before starting the reviews.
- **Do NOT treat passing independent reviews as a gate forecast.** `c7113c5a` passed both Terra xhigh reviews, then failed code-review and research-review on the same commit with new blocking findings. Budget a rework round after the gates, and brief workers up front that every number in a table needs a sample count and interval or an explicit descriptive label (the same class failed `04b85d10`).
- **Do NOT trust idle-notification reports to be complete.** They truncate at about 2 KB; ask workers for the artifact table through SendMessage, which arrives untruncated.
- **Do NOT forget that an isolated bug fails `jit validate`.** A standalone bug with no edges is a repository-integrity error. Wire follow-up bugs to the issue that surfaced them (the repository pattern).
- **Do NOT treat `nice: cannot set niceness` as a defect.** `dev/scripts/ccx1-bench-flock.sh` documents `nice -n -5` as best-effort for non-root.
- Unresolved traps from [session 6](handoff-5.md), [session 4](handoff-4.md), [session 3](handoff-3.md), [session 2](handoff-2.md) and [session 1](handoff.md) remain in force.

## Open questions needing invoker input

None.

## Reference artefacts

- Epic: `jit issue show 1a379447`; [measurement contract](measurement-contract.md); [progress](progress.json); protocol `dev/active/f547c394/protocol.md` (v3).
- Independent review helper: `scratchpad/indep-review.sh <id> <tag>` from session 7. It builds the gate context (issue, gate, latest run) and runs `contrib/gates/research-review.sh` unchanged with the configured `REVIEWER_AGENT`. The scratchpad is session-local, so recreate it from that description.
- Review outputs this session: the session-7 scratchpad `reviews/<id>-<tag>.out`. Gate run records: `.jit/gate-runs/`.
- Closed-issue evidence: `dev/active/c077a88b/`, `dev/bench_results/c077a88b/`, `dev/active/1d0da41f/`, `dev/bench_results/1d0da41f/v3-*`.
- Scripts: `.agents/skills/jit-execution-lead/scripts/{dispatch-worker-worktree.sh,check-leak-into-main.sh,reclaim-worker-worktree.sh}`.

## Update log

- Initial write while `c7113c5a` gates run.
- `c7113c5a` gates: code-review and research-review fail; rework round 1 dispatched to the original Opus worker.
