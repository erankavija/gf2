# Handoff — DVB-T2 AWGN simulation campaign (2928ccce) — session 1

**Date:** 2026-05-27
**Session number:** 1
**Prior handoffs:** none

## Current state

- Epic: `2928ccce` — state: `in_progress`
- Wave in progress: Wave 3 (extended)
- Children summary:
  - **Done (5)**: `cef2e631`, `a18956a4`, `87a2402f`, `fd73e8a8`, `003e4088`
  - **In progress (2)**: `4cdaf1c5` (TP07a — scaffold landed at `263ca764`; needs forward-chain validation after a7b1bb21), `a7b1bb21` (§6.1.4 demux — dispatched directly on main, running in background)
  - **Backlog/ready (2)**: `152388f4` (Wave 4), `e4849f07` (Wave 5)
- Active claims: `a7b1bb21` claimed by `agent:claude`; `4cdaf1c5` claimed by `agent:claude` (worker done, awaiting follow-up); epic `2928ccce` claimed by `agent:project-lead`
- Open escalations: none currently — `a7b1bb21` is the lead's response to the user's "expand scope" decision on TP07a multi-stage finding
- Progress file: `dev/active/2928ccce-progress.json` (current_wave=2; needs update to reflect Wave 3 extension when next-session lead can verify gate states)

## What just happened

- **Wave 1 (closed)**: 4 issues, many reworks (87a2402f r3 + r4 user-authorised, fd73e8a8 r1 + r2 + r3 + r4 user-authorised), 2 user-authorised criterion amendments (87a2402f TP07a defer, fd73e8a8 heartbeat path-scoping).
- **Wave 2 (closed)**: 003e4088 BICM chain integration. r1 fixed a QAM-bypass bug in the 6-config roundtrip helper + added a fast-tier QPSK roundtrip test. User-directed audit removed a TP07a criterion that belonged in 4cdaf1c5.
- **Wave 3 (extended scope)**:
  - 4cdaf1c5 worker (commit `263ca764`) discovered empirically that ALL 6 in-scope ETSI vectors (VV004, VV007, VV009, VV014, VV020, VV035 — found at `/data/specs/dvb/t2/streams/`) show ~50% bit-diff between bit-interleaver output and TP07a. NONE are §6.1.3-only.
  - The ETSI reference-streams documentation block diagram (verified at `/data/specs/dvb/t2/documentation/DVB-T2-ReferenceStreamsDocumentation1_2.txt:30-50ish`) shows TP7a = §6.1.4 (bit-to-cell-word demux) output. §6.1.5 (cell interleaver) is TP10 — well downstream, not needed for TP07a.
  - User authorised scope expansion. Created `a7b1bb21` for §6.1.4 implementation. Wired as dep of `4cdaf1c5`. Dispatched a7b1bb21 worker on main.

## What to do next

1. [ ] On a7b1bb21 completion notification: review verdict, run gates (`jit gate pass a7b1bb21 cargo-ci`, then `code-review`). The dispatch prompt requires EMPIRICAL VERIFICATION against the ETSI vectors as part of the worker's checklist — confirm the worker actually did this before accepting (it's the only reliable test that §6.1.4 is correct).
2. [ ] Close a7b1bb21.
3. [ ] Re-claim + re-dispatch 4cdaf1c5 with the new §6.1.4 available. The dispatch prompt should ask the worker to extend `tests/dvb_t2_chain_tp07a.rs` to: (a) compose `DvbT2BitInterleaver::interleave` → `DvbT2CellWordDemux::demux`, (b) assert bit-exact match against TP07a for the 6 in-scope ETSI vectors, (c) also implement the reverse chain. Should be a small dispatch since the scaffold exists.
4. [ ] Close 4cdaf1c5.
5. [ ] Wave 4 = `152388f4` campaign runner binary. Single-worker dispatch. Spec doc at `dev/active/152388f4-campaign-runner.md`. Reference TOML at `crates/gf2-coding/data/dvb_t2_tr102831_reference.toml` (worker creates if absent).
6. [ ] Wave 5 = calibration smoke only (per user choice). Lead runs `cargo run --release --bin dvb_t2_awgn_campaign -- --calibrate ...` for the 6 (rate × modulation) configs; writes PLAN.md + README.md under `dev/benchmarks/dvb_t2_awgn/`; writes the user-facing handoff for the multi-day production runs. Does NOT close `e4849f07`.
7. [ ] After Wave 5: epic `2928ccce` cannot close (transitively blocked by `e4849f07` waiting on user-side production sims). Write a final session handoff explaining this and stop.

## Traps — do not repeat these

- **Do NOT cd into a worktree in one Bash call, then run `git merge` in the next.** Shell cwd persists across Bash tool calls. Use `git -C /home/vkaskivuo/Projects/gf2 ...` to anchor on main. Trap evidence: this session's first merge of cef2e631 landed in `worktree-agent-fd73e8a8` instead of main; recovered via reset.

- **Do NOT trust `check-leak-into-main.sh` when run from a worktree shell.** Always run from `/home/vkaskivuo/Projects/gf2/`.

- **Do NOT believe worker self-reports verbatim.** Multiple bugs slipped through worker reports: 87a2402f's `Rate3_5` typo (correct: Rate3_4), 87a2402f's row-major-instead-of-column-major permutation (self-roundtrip masked it), 003e4088's QAM-bypass helper. Always grep the actual code; add hand-derived spec-compliance forward-only tests when correctness depends on a permutation.

- **Do NOT authorise spec deviations in dispatch prompts that contradict [hard] criteria.** Escalate to user instead.

- **TP07a in published ETSI vectors is §6.1.4 output (NOT §6.1.3 alone).** Confirmed empirically on 6 in-scope ETSI vectors + block diagram check. The 87a2402f bit interleaver implements only §6.1.3. To validate TP07a, the full §6.1.3 + §6.1.4 chain is required (a7b1bb21 closes this gap). §6.1.5 cell interleaver is TP10, NOT needed for TP07a.

- **VV001-CR35 is Rate3/5 × 256-QAM** (NOT QPSK as 87a2402f's pre-r4 dispatch assumed). Both Rate3/5 and 256-QAM are OUT of the epic's in-scope set (Normal × {1/2, 2/3, 3/4} × {16-QAM, 64-QAM}).

- **In-scope ETSI vectors at `/data/specs/dvb/t2/streams/`** (per 4cdaf1c5 worker's discovery):
  - `VV004-8KFFT_CSP` (64-QAM × Rate3/4)
  - `VV007-16KFFT_CSP` (16-QAM × Rate2/3)
  - `VV009-4KFFT_CSP` (64-QAM × Rate2/3)
  - `VV014-64QAM34_CSP` (64-QAM × Rate3/4)
  - `VV020-FEF_CSP` (16-QAM × Rate1/2)
  - `VV035-DTG052_CSP` (64-QAM × Rate3/4)
  Modulation inferred from TP08 sample count / block count (32400=QPSK, 16200=16-QAM, 10800=64-QAM, 8100=256-QAM per 64800-bit Normal frame). Code rate from TP05 k value relative to TP06 N=64800.

- **`run_coded_iterative_parallel` uses `StdRng`, not `ChaCha20Rng`.** Within-SNR heartbeat resume is architecturally unavailable on this path; fd73e8a8's criterion 5 was amended to scope heartbeats to sequential coded paths.

- **gf2-coding has a `test-support` feature.** Integration tests under `tests/` that need to share helpers with crate-internal unit tests must depend on `gf2-coding = { path = ".", features = ["test-support"] }` (already added). Currently exposes `parse_tp_blocks`, `tp_path` (VV001-CR35 hardcoded), and `tp_path_for` (arbitrary VV*_CSP directory).

- **Cargo-ci 300-second JIT default is tight on cold-build cargo workspaces.** Run `./scripts/cargo-ci.sh` manually first to warm `target/`, then re-call the gate.

- **The reviewer is always right** (per user 2026-05-27): when reviewer flags a criterion as unmet, fix structurally (move criterion to the right issue, or implement what's needed) rather than amend-to-defer. Past amendments (87a2402f, fd73e8a8) stand because the user approved them at the time; future findings default to STRUCTURAL fix unless user explicitly authorises amendment.

## Open questions needing user input

- None currently open. a7b1bb21 is running.

## Reference artefacts

- Epic: `jit issue show 2928ccce`
- Design docs in `dev/active/`:
  - `152388f4-campaign-runner.md` (Wave 4 campaign runner spec)
  - `fd73e8a8-resumable-runner.md` (Wave 1 runner observability spec; amended path-scoping is in the issue description)
- Progress file: `dev/active/2928ccce-progress.json`
- ETSI test vectors: `/data/specs/dvb/t2/streams/` (80 streams total); reference-streams documentation: `/data/specs/dvb/t2/documentation/DVB-T2-ReferenceStreamsDocumentation1_2.txt`
- Wave-5 artefact target: `dev/benchmarks/dvb_t2_awgn/` (per task `e4849f07`)
- Project escalation policy: `~/.claude/skills/project-lead/references/escalation-policy.md`
- Worktree dispatch protocol: `~/.claude/skills/project-lead/references/worktree-dispatch-protocol.md`
