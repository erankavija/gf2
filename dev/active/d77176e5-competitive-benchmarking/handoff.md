# Handoff — Competitive benchmarking against Magma/Sage/AFF3CT (d77176e5) — session 1

**Date:** 2026-11-10
**Prior handoffs:** None. Related: `dev/active/b4b4b9ee-tech-debt-2026-06-30/handoff-2.md` (tech-debt fixes of the same day).

## Current state

- Epic `d77176e5` is `backlog`. Children in progress, all with code merged to main and gates (cargo-ci, code-review) never run: `de311215`, `c62356d2`, `259acf37`, `be6d6f45`, `040bc2b8`, `befccab3`, `c9cb0dd2`. Ready and untouched: `12a3c312`, `f768c925`. `be113cbf` (one-byte GF(2^8) element) exists only on branch `jit-be113cbf` (`ab3ecdbcb`), unmerged.
- Main is `099172261`. Nothing is pushed. `./scripts/cargo-ci.sh` last passed in full at `f05095b7f`; since then only package suites ran (5278/5278 on core, kernels, coding, sim and three experiment crates at `099172261`).
- Uncommitted in the main checkout: this file and `dev/active/b4b4b9ee-tech-debt-2026-06-30/handoff-2.md`.
- Benchmark window 4 is running: user unit `gf2-bench-window-20261110d`, started 10:56Z, log `.agents/bench-window/window.log`. Jobs in order: decode table (`a045730c38e81ec2`), profiles (`1cc901dd6700f140`), NR encode (`9bc3063cafa727ed`), BCH encode (`56796fee86b9fb0f`), gf2x (`a60eccb143f59721`), GF(2^8) (`0866d652690a1602`). Each job's output is `.agents/bench-window/<key>.out`.
- Worker sessions hold all builds until told the window ended: `gf2-2f` (polynomial products), `gf2-ee` (GF(2^8)), `gf2-e5` (encoders). `gf2-63` has wound down.
- The work ran as direct branches from worker sessions, merged by the lead; no wave plan and no `progress.json` exist for this epic.

## What just happened

- Decode against AFF3CT 4.7.0 in its fastest configurations (windows 2 and 3, host fraktaali, recorded in `de311215`, `c62356d2`, `259acf37`): gf2's best route leads every measured cell. Layered i8 leads AFF3CT's best by 4.0x (DVB-T2 R1/2, one core, batch 16) to 26.6x (12 cores) and 1.44x to 2.83x on NR BG1 Z=384. Bit-exact f32 flooding leads everywhere except NR one core batch 16 against i16 inter-frame flooding (0.78).
- Landed for decode: DVB-T2 QC description at Z=360, bit-exact QC flooding rewrite, structural route rule as plain constants (`QcRouteBounds::structural`), layered i8 decoder with NR saturation fix and cap guard, shared sign packer.
- Landed for encode: word-wise NR rate-matched encoder (indication 67.8k to 8.1M frames/s at BG2 n=256) and DVB-T2 BCH encoder through the table and fold families, with `experiments/encode-compare`.
- Landed for fields: Karatsuba long carry-less products with an 8-word kernel (`experiments/clmul-compare`), PSHUFB GF(2^8) region and product kernels with `simd` default-on in `gf2-core` (`experiments/gf256-baselines`).
- Repository direction changes by owner decision: `AGENTS.md` names throughput and ergonomics, measurement has two tiers, invariants went from 35 to 30; committed assembly listings and their gate removed; QC route thresholds left the tuning profile; three idle epics rejected; `worktree.write_policy = "allow"`.

## What to do next

1. Wait for `window end` in `.agents/bench-window/window.log`. Run nothing on the host until then.
2. Read the tables: `.agents/worktrees/agent-de311215/target/aff3ct-ldpc-gaps/quick-compare/<UTC>.txt` and `profile-routes/<UTC>/`; `.agents/worktrees/jit-be6d6f45-run/target/encode-compare/`; the `.out` files of the gf2x and GF(2^8) jobs.
3. Record each table in its issue with `jit issue update <id> --append-description`, host and main commit named, and commit the tracker change.
4. Tell `gf2-2f`, `gf2-ee` and `gf2-e5` the window ended.
5. Merge `jit-1015ae73` (`49efa2609`) and `jit-07fe99b8` (`a11eeff92`) after setting `LOAD_MAX=4.0` in the latter, by owner decision.
6. Run `python3 scripts/check-references.py` on the two handoff files and commit them.
7. Run `./scripts/cargo-ci.sh` on a quiet host, then take the merged issues through their gates.
8. Follow-ups by owner of the code: Toom-3 if gf2 trails gf2x at large sizes (`gf2-2f`); the fused generic axpy for `be113cbf` and the ratio the safe design reaches (`gf2-ee`); two-block BCH fold if the short frame trails bchlib (`gf2-e5`).
9. `de311215` REQ-04 is timed for DVB-T2 R1/2 normal only; the other rates need cells in `quick-compare.sh`.

## Traps — do not repeat these

- Comparing against a weak comparator. The earlier "ahead of AFF3CT" result was against its scalar flooding decoder; `@/inv/performance-deliverable` now requires the fastest configuration.
- A criterion naming a comparator that does not exist: `259acf37` REQ-06 names an AFF3CT 8-bit layered NMS decoder; AFF3CT 4.7.0 rejects NMS at 8 bits.
- Running or editing anything in a queue worktree, or editing `dev/scripts/bench-window/run-window.sh` in the main checkout, while a window runs: bash reads the script as it executes. Merge script changes after `window end`.
- A queue line repeats only after its `.agents/bench-window/<key>.done` marker is removed; the key is the first 16 hex digits of sha256 over issue, worktree and command joined by tabs.
- Starting a window needs `XDG_RUNTIME_DIR=/run/user/$(id -u)` and `DBUS_SESSION_BUS_ADDRESS=unix:path=$XDG_RUNTIME_DIR/bus` for `systemd-run --user`.
- `git merge` skips the pre-commit hook; main went over the tracked-tree budget that way. Run `python3 scripts/check-tracked-budget.py` after merges.
- `git add -A dev` stages three untracked host-local data directories under `dev/`. Add named paths only.
- A shell chain `cmd | tail && git commit` commits after a failed `cmd`; two issue files were committed with conflict markers that way (`42e42101a` repairs them).
- Cache-capacity explanations offered without a control. Both were falsified: the layered 12-to-24-worker plateau and the QC flooding per-edge cost. Run the control first.
- Frames/s indications at host load above about 20 swing by 2x; instruction counts and interleaved runs are usable there.
- The recorded LDPC bundles carry y/σ², half the natural-log LLR scale; fixed-point decoders see half their intended scale on them.
- A worker session that hits its usage limit stops mid-edit; treat its `fix-*` worktrees as unreviewed.
- Workers are the parallel sessions reached with `SendMessage`; the owner rejected lead-spawned subagents for that role. Sessions may fan out to their own subagents.

## Open questions needing invoker input

Decided by the owner on 2026-11-10 ("open items as recommended"):

- `259acf37` REQ-06 is reworded to name AFF3CT's fastest fixed-point layered inter-frame configuration (16-bit in 4.7.0). Apply the rewording after the window ends; the recorded numbers then decide it.
- A quantized flooding decoder stays out of scope.
- `be113cbf` stays on the safe design with no `unsafe` cast in `gf2-core`. Raise the zero-copy path again only with `gf2-ee`'s measured ratio in hand.

Still open:

- `4f21da96` (remove `scripts/cargo-budget.sh`): no recommendation was given, so the owner's answer does not cover it. See the tech-debt handoff.
- `c9022d14` was held back from rejection by the lead; confirm it stays open.

## Reference artefacts

- Issues: `de311215`, `c62356d2`, `259acf37`, `be6d6f45`, `040bc2b8`, `befccab3`, `c9cb0dd2`, `be113cbf`; measured results are appended to the first three.
- Harnesses: `experiments/aff3ct-ldpc-gaps/quick-compare.sh`, `profile-routes.sh`, `qc-route-sweep.sh`; `experiments/encode-compare/compare.sh`; `experiments/clmul-compare/compare.sh`; `experiments/gf256-baselines/run.sh`.
- Contract page: `docs/reference/ldpc-decoder-numerical-contract.md`.
- Window runner and queue: `dev/scripts/bench-window/run-window.sh`, `dev/scripts/bench-window/queue.tsv`.
- Comparator trees are host-local under `.agents/ext/` and untracked.
