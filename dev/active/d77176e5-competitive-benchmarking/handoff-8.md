# Handoff: Competitive benchmarking against Magma/Sage/AFF3CT (d77176e5) and general BCH (ae03bcd0), session 7, wind-down

**Date:** 2026-11-15
**Prior handoffs:** handoff-7 is retired (Git history); its rulings and traps still bind unless this file records their resolution. Steward handoff: `dev/active/2caf738d-1-0/handoff-3.md`. Charter: `dev/research/2caf738d-charter.md`.

## Current state

- Lead `gf2-f4` succeeded `gf2-d7`. The steward is `gf2-96`, the peer lead `gf2-7d` (b4b4b9ee tech debt), and `gf2-00` runs the history rewrite (45ecf975) and the tracker carve-out (6ba9be17). Nothing is pushed.
- **Owner order (01:15, 2026-11-15):** wind down everything except the tracker carve-out and the history rewrite. The rewrite's precondition is that every live branch is merged or abandoned. Dispatch nothing new for this epic until the owner lifts the order.
- What stays allowed: landing `9a1b24fc` branch A when it needs only its gates, after the window, in a slot that `gf2-7d` and `gf2-00` agree to (they have priority). If a gate fails, do not rework; park the branch and report. Parked branches are carried through the rewrite as rewritten refs (owner).
- No process of this lead runs. `target/` is removed in every worktree of this lead except `agent-9a1b24fc-a` (branch A's next gate) and the b8157e14 diagnosis logs under that worktree's `target/b8157e14/`.

| issue | on main | on a branch | waits for |
|---|---|---|---|
| `6379e04e` | done, merge `dc1b537f4` | none | none |
| `9a1b24fc` | approved design `dev/active/d77176e5-competitive-benchmarking/design-9a1b24fc.md` (`e4b14f68b`) | Branch A `worktree-agent-9a1b24fc-a` at `ffe050981`, with main merged (`f9d514b42`): the trait, the BCH and `LayoutView` cutover, `Shortened`, the BCH callers and the REQ-06 sweep. The Claude Opus review passed in round 3 at `90eb89c4f` after reworks 1 and 2 (the steward's last). | Branch A: cargo-ci (the 01:01 run was killed in the pool deadlock, an environment error), then the Codex code-review gate on the final head, then landing. The gate's only open finding, F1 (LDPC/RU), is branch B scope by the steward's split; the issue's gates close on branch B's final head. Branch B is not started (wound down). |
| `3461a52a` | planning record, manifest and inventory (`7a6b7dfc2`); 15 planned issues filed and wired (`cc2e3b2dd`) | none | REQ-01 and REQ-03 met; REQ-02 open until the curves establish the rule or reject it. The validation slice has not run (window cancelled). The 15 planned issues are wound down. |
| `b8157e14` | partial findings `dev/active/d77176e5-competitive-benchmarking/findings-b8157e14.md` (`7ad504671`), with the quantized DE estimate and the NR BG1 reversal recorded as a falsification | none (merged) | REQ-03 open. The next measurements are the adaptive-rule curve and the NR BG1 floor, in a window after the rewrite. Wound down. Blocks 1.0. |
| `2275f760` | filed (code fingerprint convergence) | none | `9a1b24fc` |
| `243d1160` | none | `worktree-agent-243d1160` (3 WIP commits) | parked: rebase onto A after A and `697b9178` land, then the admission gate. Wound down. |
| `7fbd1042` | none | `worktree-agent-7fbd1042` (1 WIP commit) | parked: finish on A (the lockstep `batch_granularity` override). Wound down. |
| `4443de83` | none | `worktree-agent-4443de83` (2 commits) | parked: rebase onto the trait form after branch B. Wound down. |
| `47ec8c01` | none | none | rides with `b8157e14`'s implementation. Wound down. |
| `ae03bcd0` | none | none | completion after branch A lands; REQ-13/14 recorded as they are. Wound down. |
| `92ec2e2b`, `2bca2b28`, `60c82c4c`, `5e71e993`, `7f9751af` | designs as before | `worktree-agent-2bca2b28` (1 test commit): abandon | after the rewrite |

Branches to abandon: `wip-3461a52a-cb96b9810` (superseded), `jit-259acf37-direct-plane` and `jit-befccab3-toom3` (closed issues), `worktree-agent-2bca2b28`.

Bench-queue run worktrees that hold untracked built arms that must survive the rewrite: `.agents/worktrees/agent-de311215` (the AFF3CT build under `experiments/aff3ct-ldpc-gaps/aff3ct/target`) and `.agents/worktrees/agent-fd9d5416` (the external BCH arms under `experiments/encode-compare/external/target`). The other run worktrees hold rebuildable gf2 output only. When branch A lands, the queue's fd9d5416 command is removed and the cutover's pre-cutover arm is deleted.

## Window of 2026-11-15

The owner cancelled the window before it started (the steward stopped timer `gf2-bench-window-20261115`). The two `3461a52a` lines stay in `dev/scripts/bench-window/queue.tsv` for a later window, and the issue records that the slice has not run. Do not create `jit-3461a52a-run`; the arming command does. When they run, record both curves in `3461a52a` with executable digest, host and start loads. A curve that contradicts the 0.90 estimate is a falsification. The estimate's precision holds at cap 50 only (staircase caveat in the issue).

## Rulings of this session

All handoff-7 rulings stand. Added:

- `9a1b24fc` design: round-3 F1 was fixed with the steward's option (a), and round 4 passed. The default tile cap is structural, $K=\max(G,\lfloor(C/2)/\max(1,B)\rfloor)$, with $C$ from the memoized `HostCaches` in gf2-core. Branch A reworks 1 and 2 were approved; the test injects cache sizes into the same pure cap function (no `cfg(test)` path). The issue text names the cutover directory instead of the deleted arm (steward, (a)).
- `9a1b24fc` gate F1 on branch A (LDPC/RU) is branch B scope under the steward's split.
- `3461a52a`: the plain $(\lambda,\rho)$ model's cap-200 DVB-T2 difference (0.09 dB) is recorded beside the estimate, and the DE-tool leaf requires staircase two-edge DE. Plan-review failed three times; the owner chose a fourth attempt in place, and it passed. NR key resolution $Q = 4$ is a lead planning default.
- `b8157e14`: the NR BG1 reversal is a falsification of the earlier "NR unchanged" reading; the findings landed on their own as a partial record.
- The owner lifted the cargo-ci cap. In practice a gate fits its 900 s timeout only with about two cargo-ci runs host-wide until `18276ff1` lands; that issue also stops task-level slow-test kills.
- Skills load from the user's skill directory. `49c78483d` (owner) removed the repository copies, so a branch based before it runs stale skill scripts in its gates. Merge main first.
- Flaky `fading::tests::test_interleaver_still_composes` (unseeded `quick_test()`) is folded into gf2-7d's `b5782224`.

## Dispatch record (session 7)

| issue | task | vendor, tier |
|---|---|---|
| `9a1b24fc` | design review round 3 | Claude Opus |
| `3461a52a` | plan rework 2 | Codex gpt-6.1-sol high |
| `b8157e14` | quantized DE for REQ-03 | Claude Opus |
| `3461a52a` | blocking plan review | Claude Sonnet |
| `9a1b24fc` | design rework 3 | Codex gpt-6.1-sol high |
| `9a1b24fc` | design review round 4 | Claude Opus |
| `3461a52a` | plan rework 3 (manifest) | Claude Opus |
| `9a1b24fc` | branch A implementation | Codex gpt-6.1-sol high |
| `3461a52a` | planning attempt 4 | Claude Opus |
| `3461a52a` | adversarial pre-review | Codex gpt-6.1-sol high |
| `3461a52a` | pre-review fixes | Codex gpt-6-luna xhigh |
| `9a1b24fc` | merge main into A | Claude Opus |
| `9a1b24fc` | branch A review rounds 1-3 | Claude Opus |
| `9a1b24fc` | branch A reworks 1 and 2 | Codex gpt-6.1-sol high |

The next implementation dispatch is Claude.

## Traps

- All handoff-7 traps stand.
- The host cargo pool deadlocked at about 01:04 on 2026-11-15: killed runners leak tokens, and the daemon refills only when no client holds the lease. Stop a gate by process group and by worktree path; never touch the pool, its FIFO or the daemon (the owner's host setup). The pool log is `/tmp/cargo-pool-1000/log`.
- Do not merge main into a branch or edit its `.jit` while its gate runs; the record is marked `inputs_moved`. Run a rework in a second worktree and merge it back afterwards.
- Main's commit-msg hook (`c34e8203`) runs `scripts/check-commit-subject.py`, so a branch without that script cannot commit until it merges main.
- `check-references.py | tail -1 && git commit` commits after a failing check; capture the last line and compare it before committing.
- Retiring a handoff breaks the epic's doc link and every design that cites it; relink with `jit doc remove` and `jit doc add`, and sweep the citations.
- A gate runs the skill scripts tracked in its checkout. The plan-review gate requires a jit-planning-lead manifest (`*breakdown.json` under `dev/active/`; the tree budget refuses other JSON names there) and judges leaf sizing strictly. Run an adversarial pre-review by the other vendor first.
- Gate results written after a commit stay uncommitted in the worktree's `.jit`; commit them before merging, or `jit gate status-all` on main reports the earlier verdict.
- `pgrep -f` and `pkill -f` patterns are refused by a hook; use `ps -eo pid,args | grep`.
- Rewriting `progress.json` with `json.dump` reformats the whole file.
