# Handoff — DVB-T2 AWGN simulation campaign (2928ccce) — session 1

**Date:** 2026-05-27
**Session number:** 1
**Prior handoffs:** none

## Current state

- Epic: `2928ccce` — state: `in_progress`
- Wave in progress: Wave 2 of 5
- Children summary: 4 done (`cef2e631`, `a18956a4`, `87a2402f`, `fd73e8a8`), 1 in_progress (`003e4088`), 3 backlog/ready (`4cdaf1c5`, `152388f4`, `e4849f07`)
- Active claims: `003e4088` claimed by `agent:claude`; epic `2928ccce` claimed by `agent:project-lead`
- Open escalations: none currently — two prior escalations on Wave 1 resolved with user-authorised criterion amendments (87a2402f TP07a defer + fd73e8a8 heartbeat path-scoping)
- Progress file: `dev/active/2928ccce-progress.json`

## What just happened

- **Wave 1 (closed)**: All four parallel issues now done. Took 4 dispatches each on 87a2402f and fd73e8a8; 2 on a18956a4; 1 on cef2e631. Two criterion amendments approved by user:
  - 87a2402f TP07a criterion — recognises TP07a is multi-stage (§6.1.3 + §6.1.4 + §6.1.5), validates transitively via 4cdaf1c5.
  - fd73e8a8 heartbeat criterion — scoped to sequential coded paths only (rayon parallel + StdRng architecture limitation documented).
- Worker output landed at main HEAD = `a08f030c` (Wave 1 closure commit).
- A parallel project-lead session merged `68db401b` + `0749dbad` (PLE u16-lane kernel + f64 GEMM cascade) mid-Wave-1; unrelated files, no conflicts.
- **Wave 2 (in flight)**: `003e4088` BICM chain integration dispatched as Agent at session end. Single-issue wave, operates directly on main (no worktree).

## What to do next

1. [ ] On 003e4088 completion notification: review verdict (Tier 1-3 protocol from `~/.claude/skills/project-lead/references/lead-review-protocol.md`), run `jit gate pass 003e4088 cargo-ci` + `code-review` + `doc-review`. Doc-review is manual — attest after verifying CLAUDE.md + rustdoc updates.
2. [ ] Close 003e4088, commit closure, advance progress.json `current_wave` to 3.
3. [ ] Wave 3 = `4cdaf1c5` TP07a vector validation. Dispatch single worker. The bit interleaver is already in main; the worker wires the existing TP06→TP07a parser + the bit interleaver into a real ETSI test-vector validation. Coordinate with `crates/gf2-coding/tests/dvb_t2_bit_interleaver_tp07a.rs` (created in 87a2402f r4) — extend or augment with the actual byte-equality check now that the full BICM chain (003e4088) is wired.
4. [ ] Wave 4 = `152388f4` campaign runner binary. Single-worker dispatch. Spec doc at `dev/active/152388f4-campaign-runner.md`. Reference TOML lives at `crates/gf2-coding/data/dvb_t2_tr102831_reference.toml` (worker creates if absent).
5. [ ] Wave 5 = calibration smoke only (per user choice). Lead runs `cargo run --release --bin dvb_t2_awgn_campaign -- --calibrate ...` for the 6 (rate × modulation) configs; writes PLAN.md + README.md under `dev/benchmarks/dvb_t2_awgn/`; writes the user-facing handoff for the actual multi-day production runs. Does NOT close `e4849f07`.
6. [ ] After Wave 5: epic `2928ccce` cannot close (transitively blocked by `e4849f07` waiting on user-side production sims). Write a final session handoff explaining this and stop.

## Traps — do not repeat these

- **Do NOT cd into a worktree in one Bash call, then run `git merge` in the next.** Shell cwd persists across Bash tool calls. The merge will land on the worktree's branch instead of main. Evidence: this session, the first attempt to merge `cef2e631` landed inside `worktree-agent-fd73e8a8`; recovered via `git -C <main-path> reset` + re-merge from outside. Always use `git -C /home/vkaskivuo/Projects/gf2 ...` to anchor on main.

- **Do NOT trust `check-leak-into-main.sh` when run from a worktree shell.** The script honours cwd. If cwd is a worktree, it checks that worktree's status, not main's. Always run leak-check from `/home/vkaskivuo/Projects/gf2` explicitly.

- **Do NOT believe worker self-reports verbatim.** The `87a2402f` worker's first attempt reported "all gates pass" but had hardcoded `Rate3_5` instead of `Rate3_4`. Worker round 2 had a row-major-vs-column-major permutation bug that self-roundtrip tests masked. Always grep the actual code for spec values; add hand-derived spec-compliance forward-only tests when correctness depends on a permutation.

- **Do NOT authorise spec deviations in dispatch prompts that contradict [hard] criteria.** The fd73e8a8 round-1 dispatch authorised the worker to substitute checkpoint deletion for the spec-required subprocess+SIGINT resume test. The criterion is [hard]; the dispatch authorisation was an autonomous criterion amendment. If a criterion seems implementation-burdensome, escalate to the user; do not paper it over in a dispatch prompt.

- **Do NOT skip the "Hard criteria self-satisfied, not deferred" rule.** UNLESS the criterion text itself defers (as in 87a2402f's "in 4cdaf1c5 — that validation passes"). When the spec is multi-stage and a single issue can't physically reproduce the downstream artefact, escalate for a criterion amendment with empirical evidence (the 87a2402f r4 worker did this correctly with the 808-block VV001-CR35 analysis).

- **Cargo-ci 300-second JIT default is tight on cold-build cargo workspaces.** Workspace check + nextest + clippy + fmt with `--all-features` exceeds 5 minutes when new deps are introduced. Workaround: run `./scripts/cargo-ci.sh` manually first to warm `target/`, then re-call the gate. Do NOT extend the gate's timeout in `.jit/gates.json` (that's an amendment requiring user approval).

- **`run_coded_iterative_parallel` uses `StdRng`, not `ChaCha20Rng`.** Within-SNR heartbeat resume is architecturally unavailable on this path. Per-SNR-boundary resume is fine. fd73e8a8's criterion 5 was amended to reflect this; do NOT try to convert the parallel runner to ChaCha20Rng in any future work without an explicit user-authorised scope decision.

- **TP07a is the combined output of §6.1.3 + §6.1.4 (cell-word demux) + §6.1.5 (cell interleaver) per ETSI EN 302 755 v1.4.1.** The DVB-T2 bit interleaver alone CANNOT reproduce TP07a from TP06. VV001-CR35 is 256-QAM (not QPSK as 87a2402f's pre-r4 dispatch prompt assumed). The full TP07a validation requires the full BICM chain to be wired — that's what 4cdaf1c5 does (Wave 3). The 87a2402f issue stores its evidence in the empirical investigation note inside `crates/gf2-coding/tests/dvb_t2_bit_interleaver_tp07a.rs`.

- **gf2-coding now has a `test-support` feature.** Integration tests under `tests/` that need to share helpers with crate-internal unit tests must add `gf2-coding = { path = ".", features = ["test-support"] }` to dev-dependencies (already done) and import via `use gf2_coding::test_support::{...}`. Currently exposes `parse_tp_blocks` and `tp_path` for ETSI CSP test-point files.

## Open questions needing user input

- None currently open. Wave 2 dispatch is running; Wave 3-5 plan is locked.

## Reference artefacts

- Epic: `jit issue show 2928ccce`
- Design docs in `dev/active/`:
  - `152388f4-campaign-runner.md` (Wave 4 campaign runner spec)
  - `fd73e8a8-resumable-runner.md` (Wave 1 runner observability spec; amended path-scoping is in the issue description itself)
- Progress file: `dev/active/2928ccce-progress.json`
- ETSI test vectors: env var `DVB_TEST_VECTORS_PATH` (or `~/dvb_test_vectors`); VV001-CR35 is Normal × Rate3_5 × **256-QAM** (not QPSK)
- Wave-5 artefact target: `dev/benchmarks/dvb_t2_awgn/` (per task `e4849f07`)
- Project escalation policy: `~/.claude/skills/project-lead/references/escalation-policy.md`
- Worktree dispatch protocol: `~/.claude/skills/project-lead/references/worktree-dispatch-protocol.md`
