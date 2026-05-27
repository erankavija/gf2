# Handoff — DVB-T2 AWGN simulation campaign (2928ccce) — session 1

**Date:** 2026-05-27
**Session number:** 1
**Prior handoffs:** none

## Current state

- Epic: `2928ccce` — state: `in_progress`
- Wave in progress: Wave 1 of 5
- Children summary: 2 done (`cef2e631`, `a18956a4`), 2 in_progress and in rework (`87a2402f` r4, `fd73e8a8` r3), 4 backlog/ready (`003e4088`, `4cdaf1c5`, `152388f4`, `e4849f07`)
- Active claims: all four Wave-1 children claimed by `agent:claude`; epic `2928ccce` claimed by `agent:project-lead`
- Open escalations:
  - `87a2402f` TP07a criterion — user authorised attempt 4 with Rate3_5 scope extension; agent dispatched in background; awaiting completion.
  - `fd73e8a8` checkpoint coverage — round 3 dispatched (final allowed); needs to wire checkpoint/resume into `run_uncoded_ber_with_channel_impl` + `run_coded_iterative_parallel`.
- Progress file: `dev/active/2928ccce-progress.json`

## What just happened

- Project discovery + 9-issue epic-DAG mapping (1 epic + 8 children).
- Wave plan persisted to `dev/active/2928ccce-progress.json`; user confirmed Wave-5 scope = "lead delivers calibration smoke; user runs prod" (option 1).
- Wave 1 dispatch via worktrees (`scripts/dispatch-worker-worktree.sh`) for `87a2402f` + `a18956a4` + `fd73e8a8` + `cef2e631`.
- `cef2e631`: PASSED on first attempt. cargo-ci + code-review both green. Closed at `22add158`.
- `a18956a4`: round 1 FAIL (missing rustdoc sections on getters + `ConcatError`; `&mut self` vs spec `&self`). Round 2 PASS (rustdoc filled; `OnceCell<LdpcEncoder>` + `Mutex<LdpcDecoder>` for interior mutability). Closed at `35be4b1f`.
- `87a2402f`: round 1 FAIL (Rate3_5 hardcoded instead of Rate3_4). Round 2 FAIL (row-major instead of column-major permutation; word-boundary tests absent; TP07a not independently evidenced). Round 3 PASS column-major; round 4 (user-authorised) dispatched in background to add TP07a vector test + Rate3_5 Normal-frame support.
- `fd73e8a8`: round 1 FAIL (no `tracing` crate; spans absent; subprocess SIGINT test substituted; weak heartbeat/JSON tests; observability only wired into `run_sequential_sweep`). Round 2 FAIL on coverage gap only (checkpoint/resume still not wired into `run_uncoded_ber_with_channel_impl` + `run_coded_iterative_parallel`; missing tests for those paths). Round 3 (final allowed) dispatched in background.
- One parallel session (probably another lead instance) merged epic `68db401b` + `0749dbad` work into main during this session; orthogonal to our files. Main is now at `97fca65a`.

## What to do next

1. [ ] On 87a2402f round-4 completion notification: review verdict, merge worktree, run `jit gate pass 87a2402f cargo-ci` + `code-review`. If still FAIL, escalate (already past MAX_REWORK_ATTEMPTS via user-authorised extension).
2. [ ] On fd73e8a8 round-3 completion notification: same flow. If FAIL, escalate.
3. [ ] After both Wave-1 issues close: snapshot Wave 1 closure in commit `chore(jit:2928ccce): wave 1 closed`, then start Wave 2 = `003e4088` (BICM integration story).
4. [ ] Wave 2 dispatch is sequential (single issue); no worktree dance needed. Use the standard `agent-prompt-template.md` from jit-parallel.
5. [ ] Waves 3 (`4cdaf1c5` TP07a) and 4 (`152388f4` campaign runner) sequential after that.
6. [ ] Wave 5 = calibration smoke only (per user choice). Lead runs `--calibrate` for all 6 (rate × modulation) configs; writes PLAN.md + README.md under `dev/benchmarks/dvb_t2_awgn/`; writes a user-facing handoff for production runs. Does NOT close `e4849f07` (production criteria require multi-day compute).
7. [ ] After Wave 5: epic `2928ccce` cannot close (transitively blocked by `e4849f07` waiting on user-side production sims). Write a final session handoff explaining this and stop.

## Traps — do not repeat these

- **Do NOT cd into a worktree in one Bash call, then run `git merge` in the next.** Shell cwd persists across Bash tool calls (also noted in `~/.claude/CLAUDE.md` memory). The merge will land on the worktree's branch instead of main. Evidence: this session, the first attempt to merge `cef2e631` landed inside `worktree-agent-fd73e8a8` instead of main; recovered via `git -C <main-path> reset` + re-merge from outside. Always use `git -C /home/vkaskivuo/Projects/gf2 merge ...` to anchor on main, or check `pwd` before running `git merge`.

- **Do NOT trust `check-leak-into-main.sh` when run from a worktree shell.** The script checks `git status -uall` of the current directory's repo, which honours cwd. If cwd is a worktree, it checks that worktree's status, not main's. Run leak-check from `/home/vkaskivuo/Projects/gf2` explicitly.

- **Do NOT believe worker self-reports verbatim.** The `87a2402f` worker's first attempt reported "all gates pass" but had hardcoded `Rate3_5` instead of the spec's `Rate3_4` — the worker miscounted the in-scope rate list. Always grep the actual code for the issue's spec values before accepting worker output. Similarly `87a2402f` round-2 reported "1165 tests passed" but the permutation math was wrong; self-roundtrip tests masked the bug. Add hand-derived spec-compliance forward-only tests when correctness depends on a permutation.

- **Do NOT authorise spec deviations in dispatch prompts that contradict [hard] criteria.** The fd73e8a8 round-1 dispatch authorised the worker to substitute checkpoint deletion for the spec-required subprocess+SIGINT resume test. The criterion is [hard]; the dispatch authorisation was effectively an autonomous criterion amendment, which violates project memory rule `feedback_no_autonomous_amendments`. Reviewer correctly flagged this. Round-2 had to redo the work. If a criterion seems implementation-burdensome, escalate to the user; do not paper it over in a dispatch prompt.

- **Do NOT skip the "Hard criteria self-satisfied, not deferred" rule.** The 87a2402f criterion "consumed end-to-end by TP07a validation in 4cdaf1c5" reads as transitive satisfaction, but per project memory `feedback_hard_criterion_self_satisfaction`, the issue must carry its own evidence. Reviewer correctly enforced this. The fix (round 4) is to add an independent TP07a vector test using VV001-CR35 (Rate3_5, currently out of 87a2402f's supported rates — extend to Rate3_5 Normal-frame only).

- **Cargo-ci 300-second JIT default is tight on cold-build cargo workspaces.** Workspace check + nextest + clippy + fmt with `--all-features` exceeds 5 minutes when new deps are introduced (observed for fd73e8a8 round-1 with `blake3 + ctrlc + rand_chacha`). Workaround: run `./scripts/cargo-ci.sh` manually first to warm `target/`, then re-call the gate. Do NOT extend the gate's timeout in `.jit/gates.json` (that's an amendment requiring user approval).

- **`run_coded_iterative_parallel` uses `StdRng`, not `ChaCha20Rng`.** Within-SNR heartbeat resume is architecturally unavailable on this path. Per-SNR-boundary resume is fine. fd73e8a8 round-3 dispatch correctly documents this limitation; do NOT try to convert the parallel runner to ChaCha20Rng in any future work without an explicit user-authorised scope decision.

## Open questions needing user input

- None currently open. Both pending escalations (87a2402f extension, fd73e8a8 round-3 final) are running. If either fails on review, the lead must escalate at that point.

## Reference artefacts

- Epic: `jit issue show 2928ccce`
- Design docs in `dev/active/`:
  - `152388f4-campaign-runner.md` (Wave 4 campaign runner spec)
  - `fd73e8a8-resumable-runner.md` (Wave 1 runner observability spec)
  - `a2026f8c-incremental-logging-plan.md` (historical — predates fd73e8a8)
  - `a4d86b3d-campaign-runner-plan.md` (historical — predates 152388f4)
- Progress file: `dev/active/2928ccce-progress.json`
- ETSI test vectors: env var `DVB_TEST_VECTORS_PATH` (or `~/dvb_test_vectors`); VV001-CR35 is Normal × Rate3_5 × QPSK
- Wave-5 artefact target: `dev/benchmarks/dvb_t2_awgn/` (per task `e4849f07`)
- Project escalation policy: `~/.claude/skills/project-lead/references/escalation-policy.md`
- Worktree dispatch protocol: `~/.claude/skills/project-lead/references/worktree-dispatch-protocol.md`
