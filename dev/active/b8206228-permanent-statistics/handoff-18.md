# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 22

**Date:** 2026-08-29
**Session number:** 22
**Prior handoffs:** `handoff.md` (session 3), `handoff-2.md` (4), `handoff-3.md` (5), `handoff-4.md` (6), `handoff-5.md` (7), `handoff-6.md` (8), `handoff-7.md` (10), `handoff-8.md` (11), `handoff-9.md` (12), `handoff-10.md` (14), `handoff-11.md` (15), `handoff-12.md` (16), `handoff-13.md` (17), `handoff-14.md` (18), `handoff-15.md` (19), `handoff-16.md` (20), `handoff-17.md` (21). Their unresolved traps remain in force.

## Current state

- Epic: `b8206228` — state: backlog; epic gates remain pending.
- Wave in progress: wave 8 of 13.
- Children summary: 16 done, 1 in_progress, 1 ready, 13 backlog, 0 rejected across the execution-wave plan, plus new bug `e1d45c20` (ready).
- Active claims: `7a816262` — `agent:sol-freeze`, claimed 2026-08-29; rework count 2 = MAX.
- Open escalations: **two, unanswered — see "Open questions needing invoker input".** The owner ended the session with "Handoff now." rather than selecting an option, so the `research-review` Tier 1 decision is still open.
- Progress file: `progress.json` in `dev/active/b8206228-permanent-statistics`, the epic artifact directory returned by `jit doc dir b8206228 dev/active`.

## What just happened

### `ec22205e` closed

- Revalidated the committed v5 determinant receipt byte-for-byte: `PASS: 63 cells, 315 process outcomes, receipt sha256 d3779aa3817bbefc995b858527bf418b4d3931a8f5406281eb4f0b71a7b45e77`.
- Re-evaluated all four registered gates on the clean tree at HEAD `0e389483`; `cargo-ci`, `code-review`, `doc-review` and `research-review` all passed. The prior v4-era records were discarded, as `handoff-17.md` required.
- `research-review` returned `VERDICT: PASS` and explicitly confirmed its three prior v4 findings closed. It raised one **medium advisory** F4: `determinant_cost_v5.py:259` suppresses untracked files while `Cargo.lock` is gitignored and absent from the producing revision, and the receipt records `rustc` and `cargo_profile` but not the Cargo version or build argv, so the binary is identified by hash yet not reconstructible. The reviewer applied its convergence-across-rounds policy and did not block.
- Lead six-tier review PASS. Tier 1.5 verified all four prior findings closed with no regression; Tiers 2.5 and 2.75 were empty; Tier 3 confirmed the canonical cutover (only `determinant_cost_v5.py` survives, v3/v4 bytes untouched) and scope discipline.
- Closed at `cffa2fa6`. Python harness tests: 24 passed, 13 subtests passed.

### `7a816262` freeze artifacts produced and committed

- Released the stale `agent:codex` claim from 2026-08-17 and reclaimed as `agent:sol-freeze`.
- Phase 1 (Sol xhigh) produced the final selection receipt. The lead **independently re-derived all 63 cell decisions** from `premeasure-v1-cell-summary.csv` and `backend-ordering.csv` under protocol rules 1–4 and got an exact match — 0 mismatches, 0 missing, 0 extra — and recomputed all 22 asserted digests successfully. Committed at `396cf929`.
- Phase 2 (Sol xhigh) produced `manifest.json`, `checksums.sha256` and `freeze.md` under campaign id `permanent-zero-fraction-20260829`. The lead independently verified 63 cells, protocol N per cell, 14,229 shards, zero duplicate stream indices, max stream index 266,287,972,354 well under `2^56`, uniform `backend_receipt` binding, `determinant_companion: evaluate` everywhere, real `binary_sha256`, and `rng_algorithm: cha_cha20`. Committed at `57c9633f`.
- `doc-review` on `7a816262` **passed**.
- All four freeze artifacts are linked to `7a816262` via `jit doc add`.

### `research-review` on `7a816262` fails at Tier 1 — unresolved

- The failure is in the deterministic Tier 1 citation check, not the AI methodology review, and it took 117 ms.
- `7a816262` owns **no** `cites:` label. Its dependencies do: `9c5b26fb` owns `cites:GGK2025`, `cites:HKS2026`, `cites:Scheinerman2024`, and `0de41c82` owns `cites:Scheinerman2024`.
- `contrib/gates/research-review.sh` Tier 1 check 3 greps the whole gate context file for `cites:<key>` tokens. That context embeds each dependency's **labels** but only dependency **titles**, never dependency descriptions. So the check demands a `[GGK2025]` bracketed citation in text that structurally cannot contain one, and reports exactly the union of the dependencies' three keys.
- Contrast case proving the check normally works: `a82f2dd9` failed the same check on `cites:Yue2022`, but `a82f2dd9` **owns** that label, so that was genuine label/text drift on the issue itself.
- This is unsatisfiable without either a shared-gate change or a scope change to `7a816262`, so per invariant 3 and escalation policy entries 4 and 8 the lead escalated instead of working around it. **No workaround was applied.**

### Independent audit returned VERDICT: FAIL

The `freeze-auditor` (native Opus, read-only) reported after the handoff was first written. All eight hard criteria pass on their literal wording; the FAIL is driven by findings 1 and 2, which are defects in the freeze **record**, not in the decisions it records. Six findings total: two MEDIUM, four LOW. The lead confirmed findings 1 and 4 directly.

1. **MEDIUM — the committed freeze record narrates the pre-commit worker tree as current state, and commit `57c9633f` falsified those sentences in the same act that added them.** `freeze.md:192` says "the lead-owned commit does not yet contain these manifest bytes"; `:196` transcribes `emission-check` output as `...differs from its committed content`; `:201` says "This validation cannot pass in the uncommitted worker tree"; `:133` says "after that commit, repository history supplies REQ-02's final ordering evidence". The `:185-190` diff transcript no longer reproduces because `observe_provenance` (`crates/gf2-sim/src/permanent_campaign/provenance.rs:631`) overwrites `git_revision` with live HEAD. `dev/simulation_results` is a configured permanent path (`.jit/config.toml:164`), so the **present-tense-prose** invariant (`AGENTS.md:145`) binds this file. **Lead confirmed by reading `freeze.md` at HEAD.** Note that `doc-review` PASSED over this — the gate did not catch it, and neither did the lead's Tier 2.5 sweep, which checked the repository for stale references to the freeze but not the freeze record's own pre-commit tense.
2. **MEDIUM — `freeze.md` is the sole record of REQ-04, REQ-05, REQ-07 and REQ-08's construction, and no committed artifact records its digest.** `checksums.sha256` holds a single `manifest.json` entry and `manifest_content_hash` (`provenance.rs:1036`) hashes `manifest.json` alone, so an edit to the per-cell level at `freeze.md:34`, `K = 63` at `:36`, the stream-index construction at `:51`, or a measured determinant cost in the `:63-125` table passes `sha256sum -c`, `emission-check` and `verify_dataset` unchanged. REQ-06's "a later modification is detectable rather than silent" therefore does not reach the half of the frozen content the `deny_unknown_fields` schema cannot carry. **The obvious fix does not work:** adding a `freeze.md` line to `checksums.sha256` makes `verify_dataset` raise `IntegrityFault::OutsideRawSet` (`provenance.rs:1096`) because `freeze.md` is not in `DatasetLayout`'s RawData set.
3. **LOW — `freeze.md` sits at a campaign-root position the campaign README's closed layout table does not admit.** `dev/simulation_results/permanent-zero-fraction/README.md:29-36` gives the layout-and-ownership table whose only report class is `derived/`, and `:45-48` gives the reason: a checksum file cannot close if it also covers reports that quote its value — which `freeze.md:13` does. `freeze.md:3` states the placement deliberately, but no README row was added.
4. **LOW — supersession pointers are missing on the phase-1 drafts.** First half **withdrawn by the auditor**: the lead linked `backend-selection-v1.md`, `manifest.json`, `freeze.md` and `checksums.sha256` to `7a816262` via `jit doc add`, so the "final artifacts are unregistered" half no longer applies. The narrowed half stands — linking new documents does not edit old ones, so `jit doc list 7a816262` now returns the current artifacts *alongside* three phase-1 drafts that still assert in present tense that the freeze is blocked, with no pointer forward: `freeze-feasibility-draft.md:7` ("the protocol's same-cohort remeasurement clause therefore still blocks final freeze"), `receipt-inventory.md:130` ("the other 60 cells are gap rows with `generic_ryser` placeholders"), and `gap-list.md:7` ("It does not run now" — the premeasurement has since run and is receipted). Severity is lower than first reported: these live in `dev/active`, a **managed** rather than permanent path, and each self-labels as a draft, so `present-tense-prose` does not bind them; `backend-selection-v1.md:155-157` also disclaims them as ranking inputs.
5. **LOW (pre-existing, not introduced by this freeze) — the campaign README claims an execution receipt quotes the sidecar hash, but the code emits a different algorithm over different bytes.** `dev/simulation_results/permanent-zero-fraction/README.md:253-254` says "an execution receipt that identifies a manifest quotes the same value" as the sidecar SHA-256. The only manifest digest the execution machinery produces is `blake3::hash` over a fresh `serde_json::to_vec(manifest)` re-serialization, not SHA-256 over the on-disk bytes (`crates/gf2-sim/src/permanent_campaign/driver.rs:206`, consumed by `campaign_config_hash` at `driver.rs:190-199`). An auditor comparing a field checkpoint's hash against `checksums.sha256` finds two unrelated values with no way to tell which artifact is wrong.
6. **LOW — the protocol's required pre-draw execution receipt is unallocated and unflagged.** `dev/simulation_results/permanent-zero-fraction/protocol.md:3-6` requires "a committed pre-draw execution receipt [that] records this repository-relative path, content SHA-256, and corresponding root-manifest identity". No repository artifact is named as that receipt. `freeze.md:7,13,14` supplies all three bindings but never claims the role, and the deferred-item paragraph at `freeze.md:201` does not flag the obligation. The auditor places this outside REQ-01..REQ-08, so it does not block the criteria — but it is a live protocol obligation before any draw.

**What the audit already verified, so do not redo it:** all 63 cells individually (cell set, `matrix_count` against `protocol.md:67-72`, `shard_size` 50,000, shard counts `ceil(N/50000)` totalling 14,229, contiguous shard ids); **all 14,229 `stream_index` values** equal `(cell_ordinal << 32) | shard_id` and are globally distinct; max shard id 399 and max stream index 266,287,972,354 at ordinal 62/shard 2; `determinant_companion` `evaluate` in all 63; `backend_receipt` the identical pair pointing at `backend-selection-v1.md` in all 63, never at a draft; and REQ-08 checked **against the code**, confirming `MatrixAddress::seed` (`crates/gf2-stats/src/sampler.rs:181-195`) builds exactly the four little-endian `u64` words `freeze.md:51` states. The auditor's closing verification list was truncated in transit; the findings list above is complete at six.

### New bug filed

- `e1d45c20` (`type:bug`, ready, gates `cargo-ci` + `code-review`, wired under `b8206228`): `permanent_dataset emission-check` calls `approve_emission`, which hashes the **running** executable, so it compares `permanent_dataset`'s digest (`86e88750…`) against the manifest's pinned `permanent_campaign` emitter digest (`2d6edcd9…`) and refuses for every valid frozen manifest. `crates/gf2-sim/src/bin/permanent_dataset.rs:29` misdescribes the subcommand as running the writer's guard. The writer path at `crates/gf2-sim/src/bin/permanent_campaign.rs:204` applies the same guard to its own matching digest, so campaign execution is unaffected.

## What to do next

- [ ] **Resolve the open escalation first.** Until then `7a816262` cannot close and waves 9–13 stay blocked behind it. The three options, verbatim as presented, are in "Open questions needing invoker input" below.
- [ ] After the decision lands, re-run `jit gate evaluate 7a816262 research-review` and let Tier 2 (the AI methodology review) actually run — it has never executed for this issue, so the freeze has had **no** independent research-methodology review yet.
- [ ] **Resolve the independent audit's VERDICT: FAIL on `7a816262` (second open escalation).** Findings 1 and 2 below are defects in the committed freeze record. The rework counter is at MAX, so these escalate rather than rework.
- [ ] Then perform the lead six-tier review of `7a816262` and close it. Its rework counter is at MAX, so any gate or review failure escalates rather than reworking.
- [ ] Disposition the `e1d45c20` inspection-tool defect before the first campaign draw, as `freeze.md` §"Pre-draw and validation record" requires.
- [ ] After `7a816262` closes, dispatch wave 9: `3f664839` (design) and `73317b2e` (implementation). Both wait only on `7a816262`.

## Traps — do not repeat these

- **Do NOT satisfy the `research-review` Tier 1 failure by adding `cites:` labels and a References section to `7a816262`.** It is a manifest-freeze task that builds on none of those papers; that change would make the issue claim literature it does not use purely to satisfy a grep. It is also an issue scope change requiring the owner's approval, which has not been given.
- **Do NOT dispatch a codex worker expecting it to commit.** `codex exec -s workspace-write` makes `.git` read-only, and phase 1 burned a full run discovering this: it produced the receipt, then reported `fatal: Unable to create '.git/index.lock': Read-only file system` and correctly stopped rather than building a manifest whose receipt could not resolve at a git revision. Have the worker leave files in the tree and report paths plus SHA-256; the lead commits. `--add-dir` was not tried and is unverified.
- **Do NOT tell a worker both "never write under `.jit/`" and "use `jit doc add`".** That is a direct contradiction — `jit doc add` writes under `.jit/`. The phase-1 worker correctly refused to guess and reported the conflict. The lead owns document links; say so explicitly.
- **Do NOT report `permanent_dataset emission-check` failing as a manifest defect.** It cannot approve any real campaign manifest by construction (see `e1d45c20`). Its post-commit refusal advancing from `ManifestChanged` to `BinaryDigestMismatch` is positive evidence: it proves the on-disk manifest bytes match `HEAD` and the guard reached its final check.
- **Do NOT trust `ps -o etime` alone to judge a codex worker's health.** Both AI review gates and the freeze worker showed elapsed counters that reset mid-run because `codex` re-execs; the reliable liveness signals are the growing `~/.codex/sessions/**/rollout-*.jsonl` file and its `custom_tool_call` count. A helper that reads those is at `<scratchpad>/peek.py`.
- **Do NOT re-derive the 63-cell selection from scratch to review it.** The lead's independent derivation is reproducible from the committed CSVs alone and is saved at `<scratchpad>/expected-backends.json` and `expected-cells.json`; regenerate it with the same rule set rather than spot-checking.
- **Do NOT let the `(3,27)` cell mislead you.** Its rule-3-ineligible accelerator arm has the *higher* finite-only mean (38.750780/s vs 33.762700/s for the selected `intra_matrix_parallel`). The receipt records that mean explicitly marked "not ranked"; that is correct, not an error to fix.
- **Do NOT `git add`/`git commit` while a worker runs.** Lead and worker share one checkout and one git index (existing trap, re-confirmed as a live risk this session by two concurrent AI review gates plus a read-only auditor).
- **Do NOT assume `doc-review` catches present-tense-prose violations.** It passed `7a816262` while `freeze.md` narrated the pre-commit worker tree as current state in a permanent path. The independent audit caught it; the gate did not.
- **Do NOT sweep only the repository for stale references when committing a record that describes its own commit.** The lead's Tier 2.5 sweep checked whether other files referenced the freeze as pending, but not whether the freeze record's own prose was falsified by the act of committing it. A record that says "the commit does not yet contain these bytes" becomes false the moment it is committed.
- All unresolved traps in `handoff-17.md` and earlier handoffs remain in force.

## Open questions needing invoker input

- **Question: how should the `research-review` Tier 1 failure on `7a816262` be resolved?** The owner was asked and replied "Handoff now.", so this is still open.
  - Context: `7a816262` owns no `cites:` label; the three reported keys belong to dependencies `9c5b26fb` and `0de41c82`. The check greps the whole gate context, which carries dependency labels but not dependency descriptions, so no bracketed citekey can ever appear.
  - Options as presented:
    1. Scope `contrib/gates/research-review.sh` Tier 1 check 3 to the labels the issue under review actually owns, with a regression case. Shared gate infrastructure, so it changes behavior for every issue in the repo.
    2. Add the three `cites:` labels and a References section to `7a816262`. An issue scope change; makes the issue claim literature it does not use.
    3. File the scoping defect and leave the gate red, stalling the epic's critical path.
  - Lead recommendation: option 1. The script's own comment describes check 3 as a "label/text drift check" on the issue's labels; applying it to dependency labels is a defect in the check, and option 2 falsifies the issue description to satisfy a grep.

- **Question: how should the independent audit's two MEDIUM findings on `7a816262` be resolved?** Not put to the owner; the session ended first.
  - Context: `7a816262` is at MAX rework, so per escalation policy entry 5 these findings escalate rather than being reworked. Finding 1 is a present-tense-prose invariant violation in a permanent path that `doc-review` passed over. Finding 2 is a genuine gap in REQ-06's tamper-detection whose obvious fix breaks `verify_dataset`.
  - Lead recommendation: finding 1 is a bounded, mechanical correction to `freeze.md`'s validation section — rewrite the pre-commit narration as a post-commit record and re-transcribe the `emission-check` output that actually reproduces at HEAD. Finding 2 needs an owner decision on where a non-raw frozen record's digest belongs, since `checksums.sha256` structurally cannot hold it.

## Reference artefacts

- Epic: `jit issue show b8206228`
- Active freeze: `jit issue show 7a816262`
- New inspection-tool bug: `jit issue show e1d45c20`
- Progress: `dev/active/b8206228-permanent-statistics/progress.json`
- Prior handoff: `dev/active/b8206228-permanent-statistics/handoff-17.md`
- Final selection receipt: `dev/benchmarks/permanent_campaign/backend-selection-v1.md`, SHA-256 `fe5d37ba7c216a753bf3e546614a222c3c20e563f7f4e1d3c0341af9e1c464fe`, committed `396cf929`
- Frozen campaign directory: `dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/`, initial freeze commit `57c9633f`
- Current manifest SHA-256 after the exact-driver-argv rework: `5caa384d9c87f24562ee6d91c61c44dbc04674512b3dbe63e761ca0b9480ae57`
- Pinned emitter digest: `2d6edcd940abe9340143c8b724a8274fff8eca1200a491ad13deec9e386eba58` (`target/release/permanent_campaign`)
- Gate-failure evidence: `jit gate status 7a816262 research-review`
