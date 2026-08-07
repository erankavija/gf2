# Handoff — Research-grade CPU+GPU FEC simulation pipeline (gf2-sim) (f9717e7e) — session 2

**Date:** 2026-06-07
**Session number:** 2
**Prior handoffs:** `dev/active/f9717e7e-handoff.md` (session 1). Progress file: `dev/active/f9717e7e-progress.json`.

## Current state

- Epic: `f9717e7e` — `in_progress` (claimed `agent:project-lead`).
- Wave: A.2b in progress; B.1 + A.1 + A.2a + Phase 0 done.
- Children: done = 118a0091, c0b1702d, 19ae6540, ec530af9, 36075e4c. in_progress/held = c09d3e95 (held for 81d05bab), 3fcb7025 (gated — code done, gates pending quiet machine). Remaining pending: db9836e4 (A.2c), 81d05bab (A.2d), 5f12e7ff, 48a0db6c, f6004add/a930be7f/d3f1616a (B.2), ed575f15, 14f59c2d, all of C/D/E.
- Active claims: 3fcb7025 (agent:claude), c09d3e95 (agent:claude, held), epic (agent:project-lead). Release stale `agent:claude` claims if needed.
- Open escalations: none awaiting input (4 resolved this session).
- Worktree `worktree-agent-3fcb7025` (HEAD ee0e793b) is fully merged into main (ec30b3e1) — safe to `git worktree remove` once 3fcb7025 closes.

## What just happened (session 2)

- **36075e4c (HIP host infra) — DONE.** Merged round-6; code-review caught `DeviceBuffer::new` allocating on the current device not `device_id`; user-approved lead fix `8f990d15` (select_device/restore_device, device-scoped alloc). All gates green. Unblocked B.2.
- **c09d3e95 (graph API) — HELD OPEN.** Merged; fixed a `build()` panic on malformed fallback regs (`659b2249`) + added DVB-T2 rustdoc example (`9d10ed18`). code-review fails only on criterion-1 (graph-vs-preset equality → downstream 81d05bab). User chose hold-open (not amend). cargo-ci green.
- **3fcb7025 (within-SNR parallelism) — CODE DONE, NOT CLOSED.** Merged through 4 reworks (all real findings): SSOT BICM-chain duplication → sentinel `mean_iters` → FRAME_STRIDE inter-frame RNG overlap → QPSK is the true worst case (+ latent QPSK panic fix). Final: FRAME_STRIDE=2^20, real mean_iters, shared BICM chain, regression guard enumerating all modulations, §3 amended (user-approved, twice). **Blocked from closing** by invalid benchmarks (host under heavy external `bg3` load) + gates needing a quiet machine — see Traps + What-to-do-next.
- Lead-direct hygiene: `.gitignore` for ephemeral `dev/benchmarks/dvb_t2_awgn/curve_*` dirs (`3ba61d8c`).

## What to do next

- [ ] **Close 3fcb7025 — needs a VERIFIED-QUIET machine** (`cat /proc/loadavg` ≈ 0, no `bg3`/game/foreign `cargo`/`rustc`). At HEAD ec30b3e1: (1) `./scripts/cargo-ci.sh` pre-warm; (2) re-measure throughput: `cargo run -p gf2-sim --release --bin parallel_throughput -- --frames 144 --workers 1,2,4,8,24 --repeats 3 --es-n0 6.5`, update the receipt with the clean figure, confirm ≥12×; (3) gate `cargo-ci` then **attest `parallelism-pays`** (currently FAILED — set back by the lead on the invalid benchmark) then `code-review` LAST (so the only non-green gate when the reviewer runs is itself — clears the circular meta-finding); (4) mark done. Then `git worktree remove .claude/worktrees/agent-3fcb7025`.
- [ ] After 3fcb7025: A.2c `db9836e4` (channels; needs 3fcb7025 WorkerCtx + 19ae6540 batch types).
- [ ] A.2d `81d05bab` (preset) — its `tests/preset_vs_graph.rs` closes c09d3e95 criterion-1. Resolve the c09d3e95↔81d05bab coupling here (close both together).
- [ ] B.2 GPU kernels (`f6004add`, `a930be7f`, `d3f1616a`) — only after 3fcb7025's CPU-24 receipt is clean/valid (they reference it). Single gfx1030 → don't run multiple GPU test suites concurrently.

## Traps — do not repeat these

- **NEVER measure throughput / run cargo-ci-tier gates under external CPU load.** This session a `bg3` process at ~340% CPU (load 5/15-min ≈ 80) invalidated the worker's throughput benchmarks and would flake the 5s-per-test nextest in cargo-ci. ALWAYS `cat /proc/loadavg` + `ps -eo pcpu,comm --sort=-pcpu | head` first; a contaminated benchmark only *understates* throughput but is not a valid receipt. The 3fcb7025 receipt has a ⚠ INVALID banner; re-measure clean before attesting `parallelism-pays`.
- **The per-frame ChaCha20 RNG budget must be sized for QPSK, the LOWEST-order modulation.** Fewer bits/symbol ⇒ more symbols ⇒ more AWGN noise draws. QPSK Normal draws ~260,208 ChaCha **32-bit** words/frame (16-QAM 130,608; 64-QAM 87,408). `rand_chacha 0.9` `set_word_pos`/`get_word_pos` are in **32-bit words** (not u64 — the original §3 said "u64 words/512 KB", wrong). f64 Box-Muller = 4 words/noise sample. `FRAME_STRIDE` is now `2^20` (`parallel/mod.rs`), ~4× over QPSK; the regression guard `test_worst_case_frame_draw_under_stride` enumerates all modulations. **`f6004add` (GPU AWGN) MUST match this exact seek scheme and the QPSK worst case** — do not let it reintroduce a 16-QAM-only or u64-unit budget. (4 review rounds were spent converging this; 4 adversarial pre-reviews missed it before the formal gate / the final pre-review caught QPSK.)
- **The adversarial pre-reviewer is NOT an oracle.** This session it returned PASS while the formal gate later caught: the device-scoped-alloc bug (36075e4c), the `build()` panic (c09d3e95), the `mean_iters` sentinel, and the FRAME_STRIDE overlap (3fcb7025). Treat the mandated pre-review as risk-reduction; the formal gate is authoritative. Always still run it.
- **A *backgrounded* `jit gate pass` reports wrapper exit 0 even when the gate FAILS.** Don't trust the task-completion "exit code 0" for gate verdicts — always `jit gate check-all <id>` and read the recorded status / `.jit/gate-runs/<id>/result.json`.
- **`jit gate pass` takes no `--reason` flag** (usage: `jit gate pass [--json] <id> <gate_key>`). `jit gate fail` (used to invalidate parallelism-pays) accepts `--by`.
- **Don't mark c09d3e95 done until 81d05bab's `preset_vs_graph.rs` exists** (criterion-1 names the downstream preset; formal gate rejected self-satisfying it). 81d05bab DAG-depends on c09d3e95 → resolve the coupling at A.2d (close both together).
- **Don't dispatch `f6004add`/`a930be7f` before 3fcb7025's CPU-24 receipt is valid** — their `parallelism-pays` receipts must compare against it (soft coupling, not in the DAG).
- Carry-forward from handoff 1 (still in force): pre-warm `./scripts/cargo-ci.sh` after every merge; gate BARE (never `| tail`); restore lead-owned `.jit/`+progress.json **and** `dev/benchmarks/dvb_t2_awgn/.gitignore` to main HEAD after a worker merge (worktree predates them); targeted `-p` builds (disk 93%); the worktree-dispatch scripts the skill references don't exist (hand-roll `git worktree add ... main`); 5G NR LDPC needs per-i_LS shift tables (`acf9b11a`).

## Open questions needing user input

None. (Four escalations resolved this session: 36075e4c device-scoping fix; c09d3e95 hold-open; 3fcb7025 FRAME_STRIDE 2^16→2^19; 3fcb7025 QPSK correction 2^19→2^20.)

## Reference artefacts

- Epic: `jit issue show f9717e7e`
- Design doc: `dev/active/ec530af9-pipeline-design.md` (§3 seek scheme — now amended for 32-bit-word unit + QPSK worst case + FRAME_STRIDE 2^20; §6 HIP dispatch; §8 fallback; §9 graph API)
- Project plan: `dev/active/f9717e7e-project-plan.md` (§2 run-book, §5–§6 receipt schema)
- Progress file: `dev/active/f9717e7e-progress.json` (resume_2026_06_07 block; escalations array)
- Receipts: `dev/benchmarks/gf2-sim/parallelism-receipts.md` (3fcb7025 entry has the ⚠ INVALID-throughput banner), `baseline-single-thread.md` (1.6216 fps single-thread)
- 3fcb7025 latest code-review (the FRAME_STRIDE finding): `.jit/gate-runs/26d11c79-*/result.json`
