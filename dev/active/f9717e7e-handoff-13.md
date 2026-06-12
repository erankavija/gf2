# Handoff — Research-grade CPU+GPU FEC simulation pipeline (gf2-sim) (f9717e7e) — session 11 (part 2)

**Date:** 2026-06-12
**Session number:** 11 (continuation of handoff-12's session)
**Prior handoffs:** `f9717e7e-handoff.md` (s1) … `f9717e7e-handoff-12.md` (s11 part 1). Progress: `dev/active/f9717e7e-progress.json`. **All traps from s1–s11p1 remain in force.**

## Current state

- Epic: `f9717e7e` — `in_progress` (claimed `agent:project-lead`). Main HEAD = this handoff commit (on top of `c1125abb`).
- **E.1 `acf9b11a` DONE** (zero rework; Sionna external validation; per-BG rate-scope AMENDMENT 2026-06-12).
- **E.2 `e478daa8` DONE** (1 rework round; §5.4.2.2 interleaver per user AMENDMENT "non-goals are wrong if they contradict the ability to simulate 3GPP NR LDPC"; shared `GrayQamMapCore`/`GrayQamDemapCore`).
- **Study `43fb19e2` DONE** (created mid-session on user direction; both gates green; doc `dev/active/43fb19e2-nr-kernel-feasibility.md`).
- **E.3 `23d3525f` DONE** (all 3 gates green at `c1125abb` after 5 review rounds; THREE user-approved amendments — see below).
- **E.4 `18e69a1a` DISPATCHED** (Opus bg worker a331c3f851f887efe, worktree `agent-18e69a1a` on main `c1125abb`, claimed + in_progress; THE FINAL ACTION of this session per user directive). Running at handoff time.
- Stories: A/B/C/D done; E `a5635da5` backlog (close after E.4 + E.5).
- Remaining after E.4: **E.5 `110e45cc`** (epic close: CLAUDE.md sweep + story close), then Section 10 (coverage map, epic `gate check-all`, completion report, archive).
- Active claims: epic (`agent:project-lead`), `18e69a1a` (`agent:claude`). No open escalations. Foreign locked worktree `agent-a7c37e2288e3dc230` (issue 82dd7384) — leave alone.

## What just happened (session 11 part 2)

- **E.3 200 Mbps saga (the session's core decision chain):**
  1. Worker delivered all correctness deliverables green but STOPPED per protocol: flat-kernel ceiling **17.45 ± 0.03 Mbps**, ~11.5× below the 200 Mbps [hard] target (root cause: full-mother-code BP, 26112 vars, bandwidth-bound, batch-saturated at 128; GPU verified performing to expectation vs the a930be7f anchor). Merged the green work (`30bf6b72`), held the issue open.
  2. ESCALATED → user chose **feasibility study first**. Created + dispatched `43fb19e2`; its verdict: kernel bandwidth-bound at 44% of peak VRAM (arithmetic intensity 0.40 FLOP/B vs 41.3 ridge); levers layered-BP **1.756× (measured, 4000 blocks)**, fp16 ≈1.95× (breaks §11 CPU-vs-GPU byte-identity → statistical equivalence), QC layout 1.16× (measured probe), reduced-graph 1.07× → **combined 50–83 Mbps**. 200 Mbps unreachable on gfx1030. Study gated + closed same session.
  3. ESCALATED the verdict → user chose **OPTION B**: AMENDMENT 2026-06-12b — criterion = the attested flat-kernel measurement; kernel work deferred to a future epic (study is its design basis). Epic criterion amended in the same approval (incl. the `i_LS` = 8 → 1 label fix).
  4. Review r1 surfaced F2: the issue's PRB reference figures were WRONG (≈459/≈920; correct TS 38.214 μ=1/273-PRB values are ≈91.7 QPSK-r1/2 / ≈183.4 16QAM-r1/2 — the original 200 Mbps target exceeded the spec line rate of its own canonical config). User approved AMENDMENT 2026-06-12c correcting them.
  5. Five review rounds total: r1 (F1 receipt-verdict ambiguity → lead; F2 PRB → user amendment; F3 `LlrSource` triplication → worker folded FOUR sites into the new `gf2_sim::testutil::AwgnLlrSource` [feature `test-support`, gf2-algebra precedent], bit-identity proven by verbatim-embedded equality over 9 seeds × 7 sigmas; `gpu_ldpc_throughput` bin gained `required-features = ["test-support"]` + receipts re-run command updated); r2 (receipt target-row + missing driver field → lead); r3+r4 (doc contract on `GpuNr5gDecoder` — lead added `no_run` Examples + Complexity to ALL 12 public items); r5 PASS.
  6. Lead attestation: independent re-measure **17.50 ± 0.08 Mbps** reproduces the receipt; `parallelism-pays` attested PASS under the amended criterion. Bench verdict logic updated to the attested band (`b4dabaea`) — it no longer prints the stale 200 Mbps comparison.
- **E.4 dispatched** with the full pre-audit: aff3ct/IT++ absent from host + repos → hermetic pinned-tag aff3ct build (gitignored); the **H-matrix AList export trick** (feed aff3ct OUR exact H via `--dec-h-path` — code-mismatch killed by construction); the **chain-parity warning** (full-T2-chain vs LDPC-only is not apples-to-apples — match configs or add a gf2-sim LDPC-only arm via the graph API; the README must state which comparison carries ±0.2 dB); NR needs a NEW CPU sweep driver (`Pipeline::run` is DVB-T2-only); config matching (NMS 0.75 / iters / early-term / Gray labeling / Eb-vs-Es / `--sim-seed`); escalate-not-relax triggers.

## What to do next

- [ ] **E.4 worker completes** → lead review (read the FULL review outputs — see traps): scrutinize the apples-to-apples design + which comparison the ±0.2 dB criterion is evaluated on; the aff3ct version pin + H-export checksums in the README; that CSVs/PNGs are committed and `run.sh` reproduces them; that no slow tests leaked into the fast tier. Then merge → `cargo-ci` + `code-review` → close.
- [ ] If E.4 STOPS (aff3ct build infeasible / >0.2 dB after honest investigation / no matching config) → ESCALATE to user per the issue's non-goal clause.
- [ ] **E.5 `110e45cc`** (epic-close task): CLAUDE.md sweep + whatever its body specifies — re-read it at dispatch; verify it reflects ALL session-11 amendments (E.3 target, PRB numbers, per-BG rates, i_LS labels).
- [ ] Close story `a5635da5` (criteria check against E.1–E.5), then epic Section 10: coverage map of all epic criteria (NOTE: the epic's Phase E criterion was amended 2026-06-12b — the coverage map cites the amended text), `jit gate check-all f9717e7e` (epic has NO gates configured — confirm), completion report per `references/completion-report-template.md`, archive progress file per `[documentation]` config, transition epic → done, present report, STOP.

## Traps — do not repeat these (NEW this session part 2; all prior remain)

- **Read the FULL code-review output every round.** The r1 review listed a doc-contract finding BELOW my 40-line head-truncation; I fixed only what I saw, and the same finding failed r3 and r4 (two avoidable rounds). Pipe the full result.json stdout, grep for ALL `FAIL` lines, and fix every named item plus the same class everywhere else in the file (r4 named `new`/`build_decoder`/`decode_batch` only AFTER r3's items were fixed — the class sweep on round one would have cost a single round).
- **Doc-standards findings extend to EVERY public item including trivial accessors** — when the reviewer cites "missing # Examples on X", add Examples + Complexity to the whole `impl` block's public surface in one pass (`no_run` blocks for GPU items are the established idiom).
- **A bench/tool with the criterion's number baked in goes stale on amendment** — the E.3 bench printed "BELOW TARGET … do NOT weaken" after option B made it wrong; sweep BINARIES (not just docs) for embedded target constants in post-amendment sweeps.
- **The loadavg<1.5 gate-wait can deadlock on ambient desktop load** (browser at ~2.6 on 24 threads). The settled-loadavg rule exists for the perf-sensitive gates and the 5 s fast tier under HEAVY load (bg3-class, >300%); correctness gates (cargo-ci/code-review) are safe at ~10% ambient. Use judgment: kill the waiter when the load is ambient, not a battery tail.
- **`jit issue update -d` replacement via python: always assert the EXACT old text first** (one amendment attempt failed on a whitespace mismatch — refetch and match verbatim, never retype).
- **Worker `git add -A` in a worktree can sweep build-cache dirs** — the E.3 worker caught and soft-reset it; check `git show --stat` of worker commits for unexpected paths during review.

## Open questions needing user input

None pending. (FIVE user decisions this session part 2, all recorded: 43fb19e2 study creation [from the 'pause for study' choice]; E.3 OPTION B target amendment 2026-06-12b; E.3 PRB correction 2026-06-12c; earlier: E.3 i_LS label fix; E.2 §5.4.2.2 interleaver. E.4's escalate triggers may produce the next one.)

## Reference artefacts

- Progress: `dev/active/f9717e7e-progress.json` (waves E.3 + E.3b-study closed_notes carry the full journey; wave E.4 `dispatch_note` = dispatch facts).
- E.3 landed surfaces: `crates/gf2-sim/src/gpu/nr_5g_ldpc.rs` (`GpuNr5gDecoder`, fully documented), `crates/gf2-sim/src/testutil.rs` (`AwgnLlrSource`, feature `test-support`), `benches/nr_5g_realtime.rs` (env-overridable; amended verdict band), `tests/gpu_nr_5g_byte_identity.rs`; receipts `dev/benchmarks/gf2-sim/5g-nr-realtime.md` (+ lead attestation §) and `parallelism-receipts.md` (amended PASS verdict).
- Study: `dev/active/43fb19e2-nr-kernel-feasibility.md` + `dev/research/nr_kernel_feasibility/` (roofline_bytes, layered_convergence, qc_layout_probe) — the design basis for any future kernel epic.
- E.4 dispatch prompt summary: progress.json wave E.4; worker transcript a331c3f851f887efe.
- Locked worktree `agent-a7c37e2288e3dc230` (foreign, issue 82dd7384) — leave alone.
