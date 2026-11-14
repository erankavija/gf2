# Handoff: Competitive benchmarking against Magma/Sage/AFF3CT (d77176e5) and general BCH (ae03bcd0), session 7

**Date:** 2026-11-14
**Prior handoffs:** handoff-7 is retired (Git history); its rulings and traps still bind unless this file records their resolution. Steward handoff: `dev/active/2caf738d-1-0/handoff-3.md`. Charter: `dev/research/2caf738d-charter.md`.

## Current state

- Lead `gf2-f4` succeeds `gf2-d7`; steward `gf2-96`; peer lead `gf2-7d` (b4b4b9ee tech debt). `gf2-00` runs the history-rewrite rehearsals for epic 6ba9be17. Nothing is pushed.
- Owner priority (via steward): the history rewrite and the `.jit` carve-out lead the project. Its precondition is that every live branch is merged or abandoned. Order work so that branches land; open no new work before the rewrite.
- The owner lifted the cargo-ci cap; parallelism is unrestricted. A run that times out at 900 s under load is rerun and is no failure.

| issue | state | branch head | what remains |
|---|---|---|---|
| `6379e04e` | done | merged `dc1b537f4` | none |
| `9a1b24fc` | ready | design merged `e4b14f68b`; branch A `worktree-agent-9a1b24fc-a` at `3aeebceae` | Branch A is implemented (Codex) and merged with main. The Claude Opus review passed on round 3 (`90eb89c4f`), after reworks 1 and 2, the steward's last. The code-review gate's only open finding is F1 (LDPC/RU), which is branch B scope; it is in scope on B's final head, where the issue's gates run. Next: cargo-ci on A (the 01:01 run was killed in the pool deadlock, an environment error), then merge main and land A. Then `243d1160` and `7fbd1042` on A, and branch B. |
| `3461a52a` | in progress | planning merged `7a6b7dfc2` | Plan-review passed at `844492b60` on attempt 4, and the 15 planned issues are filed (`cc2e3b2dd`). The issue depends on `validate-dvb-t2` (`0e84343d`) and `validate-nr` (`5abf489d`). REQ-02 is open until the curves establish the rule or reject it. After the window, record both curves (below). |
| `b8157e14` | in progress | findings merged `7ad504671`; worktree `worktree-agent-b8157e14` merged | REQ-03 open. Partial findings with the quantized DE estimate are on main. Next steps (findings "Recommended rule" list): the adaptive-rule curve and the NR BG1 floor first, in a window after the rewrite. Implementation then carries `47ec8c01`. |
| `2275f760` | backlog | none | Filed: converge code fingerprints; after `9a1b24fc`. |
| `243d1160`, `7fbd1042`, `4443de83` | as in handoff-7 | | Rebase onto A after A lands; `697b9178` (gf2-7d) lands before `243d1160` rebases. |
| `92ec2e2b`, `2bca2b28`, `60c82c4c`, `5e71e993`, `7f9751af` | backlog | | Start after the rewrite unless they can land before it. |

## Rulings that bind the next lead

All handoff-7 rulings stand. Added this session:

- `9a1b24fc` design: round-3 F1 fixed by steward option (a); round 4 passed. The default tile cap is structural, $K=\max(G,\lfloor(C/2)/\max(1,B)\rfloor)$ with $C$ from `HostCaches::detect`.
- `3461a52a` (steward): the plain $(\lambda,\rho)$ model's cap-200 DVB-T2 difference is recorded beside the estimate, and the DE-tool leaf requires staircase two-edge DE. The owner chose a fourth attempt in place over narrowing the issue.
- `b8157e14` (steward): the NR BG1 reversal is recorded as a falsification of the earlier "NR unchanged" reading; the findings land on their own as a partial record.
- Skills load from the user's skill directory; `49c78483d` (owner) removed the repository copies. Handoff traps that name scripts under `.agents/skills/` refer to the user-level skill.

## Window of 2026-11-15

Timer `gf2-bench-window-20261115` at 02:00 local runs the two `3461a52a` lines (about 155 minutes, estimate). Afterwards read `.agents/bench-window/window.log` from its last `window start` and record both curves in `3461a52a` with executable digest, host and start loads; a curve contradicting the 0.90 estimate is a falsification.

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

The next implementation dispatch is Codex.

## Traps

- The host cargo pool deadlocked at about 01:04 on 2026-11-15: killed runners leak tokens, and the daemon refills only when no client holds the lease. Kill a gate by process group and by worktree path, never the pool or its daemon (the owner's host setup); nothing uses cargo again before the steward's word.

- All handoff-7 traps stand.
- `check-references.py | tail -1 && git commit` commits after a failing check; capture the last line and compare it before committing.
- Rewriting `progress.json` with `json.dump` reformats the whole file; the first such commit shows a large diff.
- A Codex worker in a worktree must exit before the lead merges main into that worktree.
- `pgrep -f` / `pkill -f` patterns are refused by a hook; use `ps -eo pid,args | grep`.
- A gate runs the skill scripts tracked in its checkout; a branch based before `49c78483d` ran a stale manifest helper. Merge main before gating.
- The plan-review gate requires a jit-planning-lead manifest (`*breakdown.json` under `dev/active/`; the tree budget refuses other JSON names there) and judges leaf sizing strictly; local `--deny-warnings` validation passing does not predict the gate.
