## Rework Required (attempt 1 of 2)

Your work on issue **Processor backend selection for campaign cells** (`032cc754`)
passed `cargo-ci` but **failed the `code-review` gate** with one high/blocking
finding. The scheduler-level work you did is accepted and must not be redone or
restructured — the defect is that REQ-06 stops at the scheduler and never reaches
the shipping execution path. Fix only that.

The lead scoped the original dispatch to `schedule.rs` and missed the driver and
binary layers; that framing was wrong, and the correction is below. Do not treat
the original prompt's "All work is in schedule.rs unless noted" as binding.

### Required pre-commit audit (include the raw output at the top of your report)

```bash
jit gate status 032cc754 code-review --all
jit doc list 032cc754
```

`jit doc list 032cc754` currently reports no linked documents, so audit step 3
(design-doc deferred-item grep) has no target — say so explicitly rather than
skipping the row. **These two commands are read-only and are the only `jit`
commands you may run.** Do not evaluate gates, claim, or update any issue.

### Review verdict (verbatim)

> **[blocking][issue-impact] F1 — REQ-06 is not integrated into the production
> checkpointed campaign path.** The resolver returns
> `ScheduleError::BackendUnavailable`, but `driver.rs:245` immediately converts
> it to a string. The generic callback-error branch quarantines it and continues,
> and the final summary records `ExecutionFailure` rather than
> `BackendUnavailable`; the production binary then accepts the successful driver
> return and exits successfully (`permanent_campaign.rs:73`). Thus a
> manifest-selected unavailable backend is not explicitly surfaced as the
> required backend-unavailable cell halt in the shipping execution path.
>
> references: `@/invariant/accelerator-safe-fallback`

Resolve `@/invariant/accelerator-safe-fallback` with
`jit item show @/invariant/accelerator-safe-fallback` before you act on it. The
issue's own Notes section states why the halt is that invariant's explicit-failure
arm rather than a breach of it — read it and keep the reasoning consistent.

The reviewer cites a contract and says the artifact violates it. There are exactly
two valid responses: satisfy the contract literally, or report that you cannot.
Do not argue the reading down — do not write that quarantine "already halts the
cell", that the summary "records the failure anyway", or that the case is
"unreachable in practice". Any such framing fails the next round.

### What to fix

**1. An unavailable backend must be fatal and pre-flighted, not quarantined**

- Location: `crates/gf2-sim/src/permanent_campaign/driver.rs:245` (the
  `.map_err(|error| error.to_string())` closure in `run_field_checkpointed`) and
  `run_field_checkpointed_inner`.
- What's wrong: every `ScheduleError` — including `BackendUnavailable` — is
  stringified into the generic callback-error branch, which quarantines the item
  and continues. A manifest naming a backend this build cannot provide therefore
  produces a campaign that runs to completion, records `ExecutionFailure` for
  that cell, and exits 0. A configuration defect is being laundered into a
  measurement outcome, and a preregistered dataset would carry it silently.
- What's expected: resolve **every** enumerated work item's processor path in a
  pre-flight, before any checkpoint is loaded, any matrix is drawn, any shard is
  emitted, and any checkpoint is written. On the first unresolvable item, return
  `CampaignDriverError::Schedule(ScheduleError::BackendUnavailable { .. })`.
  Reuse the existing `CampaignDriverError::Schedule` variant — do **not** add a
  parallel error vocabulary (`@/inv/convention-convergence`).
- Put the pre-flight in the shared `run_field_checkpointed_inner` so both
  `run_field_checkpointed` and `run_field_checkpointed_with_evaluator` are
  covered. Check whether any existing test drives the `_with_evaluator` seam with
  a manifest whose cells name an unresolvable backend; if one does, it was
  relying on the defect — report it and adjust it to the new contract rather
  than weakening the pre-flight.
- The binary at `crates/gf2-sim/src/bin/permanent_campaign.rs:73` already routes
  a driver `Err` through `failure(error)`; confirm by reading it that this yields
  a non-zero exit and prints the cell and backend. Do not restructure the binary
  if it already does.

**2. The quarantine narrative in the docs becomes false — fix it in this change**

`driver.rs:226` states "an evaluator error quarantines that item in the field
summary while remaining work continues", `driver.rs:253` states "A callback error
quarantines that work item and does not stop remaining work", and the module
header at `driver.rs:14` carries the same claim. After this change an unavailable
backend is refused up front and nothing runs. Amend each so the carve-out is
stated where the quarantine contract is stated (`@/inv/single-source-prose`,
`@/inv/present-tense-prose` — describe the current contract, not the change).
Sweep `crates/gf2-sim/src/permanent_campaign/` for any other prose asserting that
every evaluator error quarantines.

### Tests you must add

1. `test_run_field_checkpointed_refuses_an_unavailable_backend` — a manifest with
   a cell naming `Backend::IntraMatrixParallel` at `q = 5`. Assert the returned
   error is `CampaignDriverError::Schedule(ScheduleError::BackendUnavailable { .. })`
   and that its rendered `Display` names the `q`, the `n`, and the backend token.
2. `test_run_field_checkpointed_refuses_before_writing_any_artifact` — same
   manifest, run against a fresh temporary directory, and assert afterwards that
   **no** shard file, **no** field summary, and **no** checkpoint file exist.
   This is the assertion that proves "before any matrix is drawn"; a test that
   only checks the error type does not close this finding.
3. `test_run_field_checkpointed_does_not_quarantine_an_unavailable_backend` —
   assert the refusal path produces no `QuarantinedShard` entry and no
   `ExecutionFailure` record for that cell.
4. A binary-level assertion in `crates/gf2-sim/tests/permanent_campaign_bin.rs`
   (which already drives the built binary via `std::process::Command`): the same
   manifest yields a non-zero exit status and stderr naming the cell and backend.
5. Confirm the existing quarantine tests still pass unchanged — the ordinary
   evaluator-failure path must keep quarantining and continuing. If any needed
   modification, list it and say why.

Keep the naming convention you used in round 1 (`test_<operation>_<scenario>`).

### Constraints

- Fix only the listed items. Do not restructure `resolve_processor_path`,
  `ProcessorPath`, or `evaluate_permanent` — they passed review.
- **Do not commit. Do not stage. Do not write `.jit/`.** Leave the tree
  uncommitted; the lead reviews and commits.
- Do not run `./scripts/cargo-ci.sh`, benchmarks, or GPU work.
- Your `cargo nextest -p gf2-sim` will show one failure,
  `hipGetDeviceCount → HIP 100`. That is your sandbox having no GPU; the lead
  verified it passes on the host. Do not try to fix it and do not report it as a
  regression.

### Commands to run and report verbatim

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
3. `cargo nextest run -p gf2-sim --all-features --release --profile ci`
4. `cargo doc -p gf2-sim --all-features --no-deps`

### Resolution table (required — do not submit without it)

| # | Round | Source | Finding (verbatim) | Resolution (file:line at HEAD) |
|---|-------|--------|--------------------|-------------------------------|
| 1 | R1 | reviewer | F1 — REQ-06 is not integrated into the production checkpointed campaign path … | <file:line> |
| 2 | — | audit-step-3 | <state that no documents are linked to 032cc754> | n/a |

### Mandatory sweep

For finding F1, run and paste the raw results:

```
rg -n "to_string\(\)" crates/gf2-sim/src/permanent_campaign/
rg -n "quarantin" crates/gf2-sim/src/ crates/gf2-sim/tests/
```

Fix **every** site where a `ScheduleError` is stringified into a quarantine
decision, not only `driver.rs:245`. If the sweep surfaces further instances the
reviewer did not cite, add rows for them and fix them in this same submission.

### Report back

1. The pre-commit audit output.
2. The completed resolution table.
3. The raw sweep results.
4. The four command results, verbatim.
5. Anything left unverified or deliberately not done, and why.
