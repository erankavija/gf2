# Rework — accelerator execution for campaign cells (`a39bb161`), round 2

This is attempt 2 of 2. Read the original dispatch at
`dev/active/b8206228-permanent-statistics/dispatch-a39bb161.md` and the first
rework at `dev/active/b8206228-permanent-statistics/rework-a39bb161-r1.md`
before editing. Their worker constraints remain in force: do not touch `.jit/`,
do not stage or commit, and do no GPU/device work.

## Review: Accelerator execution for campaign cells (`a39bb161`)

**Verdict:** FAIL

### Gate status

- `cargo-ci`: passed at `f55cf2fe`.
- `code-review`: failed at `f55cf2fe` with two blocking findings.

### Prior-findings regression table (Tier 1.5)

| Round | Finding | Status at HEAD | Evidence |
|---|---|---|---|
| R1 | The lock-wrapped runner omitted accelerator costs, so accelerator manifests could not execute. | closed | `dev/scripts/permanent-campaign-runner.sh:638` |
| R1 | One field-wide CLI cost sized every accelerator cell. | closed on the production binary/driver path | `crates/gf2-sim/src/bin/permanent_campaign.rs:135`; `crates/gf2-sim/src/permanent_campaign/driver.rs:291` |
| R2 | New public accelerator APIs omitted error, panic, and complexity contracts. | REGRESSED/PARTIAL | `schedule.rs:497` still lacks all three; `schedule.rs:664` omits `AcceleratorCostMissing` |
| R3 | Default public schedulers executed accelerator cells with an unmeasured default cost. | closed | `schedule.rs:494` passes no cost, which refuses accelerator cells |
| R4 | The explicit public field scheduler applies one `AcceleratorConfig` to every cell. | OPEN | `schedule.rs:502` and `schedule.rs:522` |
| R4 | Public API documentation is incomplete. | OPEN | `schedule.rs:497` and `schedule.rs:664` |

### Success criteria

- [ ] REQ-01 — UNMET: the production driver is per-cell, but
  `run_field_with_worker_count_and_accelerator` remains a public bypass that
  reuses one cost across all accelerator cells in a field.
- [x] REQ-02 — the inverse-cost launch test uses 50 microseconds and 1
  millisecond and checks the launch-count direction.
- [x] REQ-03 — the production runner invokes the campaign binary through
  `ccx1-bench-flock.sh --full-host` and passes the cost table.
- [x] REQ-04 — device absence returns a cell- and device-naming error before
  dispatch, with no processor fallback.
- [x] REQ-05 — non-HIP builds retain processor execution and refuse accelerator
  cells through `BackendUnavailable`.
- [x] REQ-06 — `gf2-sim`'s `hip` feature includes `gf2-algebra/hip`.

### Stale-narrative sweep (Tier 2.5)

No live forward-looking accelerator narrative was found. Matches for the short
ID and API name are execution records and dispatch material, not permanent
claims that the capability is pending.

### Deferred-items audit (Tier 2.75)

`jit doc list a39bb161` reports no linked design documents, so there are no
deferred-item matches to classify.

### Holistic findings

The checkpointed production path and the field-level library path expose two
different accelerator-cost conventions. That violates the repository's one
canonical-abstraction rule and lets a caller bypass the per-cell evidence
contract. The incomplete Rustdoc conceals that distinction from callers.

### Required changes

1. Replace the field-level single-`AcceleratorConfig` API with the canonical
   `AcceleratorCostTable` convention. Resolve the configuration by `(q, n)` for
   each accelerator work item, refuse every missing entry before any work runs,
   and retain processor-only behaviour with an empty/default table. Remove or
   make private any public bypass that can apply one cost across several cells.
2. Add a behavioural test with at least two accelerator cells in one field and
   distinct measured costs. It must demonstrate per-cell resolution and a named
   missing-entry preflight without requiring a device. A table-only unit test is
   not sufficient evidence for the field-level API.
3. Complete the public Rustdoc for the affected scheduler/evaluator APIs:
   purpose, every returned error including `AcceleratorCostMissing`, panic
   behaviour, and non-obvious complexity. Sweep every public API introduced by
   the issue for the same omission.

## Mandatory pre-commit audit and resolution table

Before editing, run `jit gate status a39bb161 code-review --all` and
`jit doc list a39bb161`. Include their raw output in the final report. Build a
resolution table with one row for every finding from every gate round and cite
the closing `file:line` at HEAD. Also run workspace sweeps for every use of
`AcceleratorConfig`, `AcceleratorCostTable`, and
`run_field_with_worker_count_and_accelerator`; account for every path that can
execute more than one manifest cell.

## Verification constraints

- Work test-first and run focused release-mode CPU tests first.
- Do not run device tests, ROCm commands, simulations, profiling, benchmarks,
  or any command that opens the GPU. Baldur's Gate 3 has priority on this host.
- Do not run multiple Cargo or nextest commands concurrently.
- Run formatting and focused non-device checks sufficient to hand the work back
  for the lead's registered gates. Report every command and result verbatim.
- Modify only the issue-attributable scheduler/tests/docs surfaces needed to
  close the findings. Do not change issue criteria, gates, manifests, or `.jit/`.
