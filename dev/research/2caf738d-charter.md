# Charter: gf2 1.0 release

> Strategic container: 2caf738d · milestone:1.0

## Vision

gf2 1.0 is the fastest open implementation of finite-field arithmetic and
channel coding on its supported hosts. It is delivered when every stated
benchmark cell, measured in the benchmark window against the reference's
fastest configuration, is at 1.0 or above, and the DVB-T2 and 5G NR chains run
end to end against external conformance vectors. A child container's work is
judged by its default path: the fastest conforming route behind a simple API,
with further speed behind an optional parameter or an `_unchecked` method.
Optimization never relaxes mathematical or standards conformance.

## Decision Log

- D-1: The primary outcome of 1.0 is measured throughput leadership.
- D-2: 1.0 is done when every stated cell is at 1.0 or above and the standards chains run end to end.
- D-3: Polar CA-SCL, SIMD and platform expansion, and decoder improvements stay in 1.0.
- D-4: Trade-offs resolve to the fastest conforming default behind a simple API.
- D-5: A ratio is recorded against the reference's fastest build configuration.
- D-6: Permanent documentation carries harness and method; measured ratios live in issue history.
- D-7: A cell is measured at the output contract its reference is timed at.
- D-8: The tracker lives in its own repository, mounted at `.jit/`, and cuts over inside the history rewrite.
- D-9: Fast-tier time budgets are enforced by an epic gate on a quiet host.

## Decision Details

### D-1: Throughput leadership is the primary outcome

- **Chosen:** 1.0 is the fastest open implementation; measured leadership over
  the strongest open references is the outcome the milestone delivers.
- **Rejected:** Adoption first: external uptake follows from leadership and is
  not the measure of 1.0.
- **Rejected:** The owner's research instrument first: the studies consume the
  library and do not define its release.
- **Rejected:** Reference quality first: conformance is a constraint on every
  change, stated in the vision, and not the differentiating outcome.
- **Reasoning:** A single measurable outcome decides escalations; throughput
  against a named reference is measurable in the benchmark window.
- **Date:** 2026-11-12

### D-2: The 1.0 bar

- **Chosen:** Every stated benchmark cell against a reference's fastest
  configuration is at 1.0 or above, and the DVB-T2 and 5G NR chains run end to
  end against external conformance vectors.
- **Rejected:** A frozen public API as a condition: the bar is measured
  behavior, and `@/inv/canonical-cutover` already governs superseded forms.
- **Rejected:** Publication to crates.io as a condition: publication is an
  owner action outside the milestone's criteria.
- **Reasoning:** Both conditions are checkable: one by window tables, the other
  by conformance suites under `@/inv/standards-vector-conformance`.
- **Date:** 2026-11-12

### D-3: The milestone scope stays

- **Chosen:** Epics `@/issue/b81c239c` (5G NR polar codes with CA-SCL),
  `@/issue/7f809931` (SIMD and platform expansion) and `@/issue/0fc3c9d0`
  (decoder algorithmic improvements) remain dependencies of 1.0.
- **Rejected:** Moving any of the three after 1.0: each bears on the vision,
  through the 5G NR chain or through throughput on supported hosts.
- **Reasoning:** The vision names 5G NR chains and supported hosts; the three
  epics deliver them.
- **Date:** 2026-11-12

### D-4: Defaults fast, API simple

- **Chosen:** The default path is the fastest conforming one behind a simple
  API. Speed that needs a harder API sits behind an optional parameter or an
  `_unchecked` method, under `@/inv/ergonomic-defaults` and
  `@/inv/caller-trusted-fast-paths`.
- **Rejected:** Throughput at any API cost: a hard default path loses the
  researcher the defaults serve.
- **Rejected:** Ergonomics at a throughput cost: a slow default contradicts D-1.
- **Reasoning:** The two goals conflict only on the default path; separating
  the default from the opt-in fast path satisfies both.
- **Date:** 2026-11-12

### D-5: Ratios use the reference's fastest build

- **Chosen:** Before a ratio is recorded, the reference's build options are
  read and its fastest configuration is built, under
  `@/inv/performance-deliverable`.
- **Rejected:** The reference's default build: for bchlib it ran well below the
  `CONFIG_BCH_CONST_PARAMS` build and turned a recorded gf2 lead into a deficit
  (`@/issue/040bc2b8` records the values).
- **Reasoning:** D-2 counts cells at 1.0 or above; a cell measured against a
  weaker configuration does not count.
- **Date:** 2026-11-12

### D-6: Documentation carries harness and method only

- **Chosen:** A permanent page names each comparison, its harness, its
  configuration and how to reproduce it. Measured ratios, intervals and result
  tables live in issue history, under `@/inv/committed-evidence-budget`.
- **Rejected:** One host-labelled example ratio per comparison: a
  hand-maintained copy goes stale, as `@/inv/single-source-prose` states.
- **Rejected:** Treating the performance reference page as a decision record:
  it decides nothing by a pre-registered rule.
- **Reasoning:** The NR encoding entry of the performance reference page stated
  a reference lead after `@/issue/be6d6f45` had recorded a gf2 lead; a page
  without measured values cannot contradict the latest window.
- **Date:** 2026-11-12

### D-7: A cell matches the reference's output contract

- **Chosen:** A D-2 cell times the gf2 entry point whose output contract equals
  the reference's. Where the reference writes into caller-provided buffers, the
  cell is the `_into` form with a reused workspace; the allocating call is
  reported beside it as its own row. This reads the vision's "default path" for
  buffer-reuse forms and leaves D-4 in force.
- **Rejected:** The allocating call as the cell: it charges gf2 an allocation
  per codeword that bchlib and AFF3CT are not timed for.
- **Rejected:** Both forms at 1.0 or above: the allocating row has no
  like-for-like reference.
- **Reasoning:** `@/issue/60c82c4c` and `@/issue/5e71e993` found the cutover
  harness timing `BchCode::encode_batch` against comparators that write into
  preallocated buffers; D-5 asks for the fair comparison in both directions.
- **Date:** 2026-11-12

### D-8: The tracker has its own repository

- **Chosen:** The tracker store is a separate Git repository mounted as an
  independent, ignored checkout at `.jit/` of the primary checkout. The cutover
  of `@/issue/6ba9be17` runs inside the quiet window of the history rewrite of
  `@/issue/45ecf975`, before its extraction step, so the rewrite drops the
  tracker paths from history. A required `landed` gate holds an issue's
  dependents until its code is on `main`. A gate verdict counts for another
  commit when the gate's declared inputs are identical. No criterion orders a
  push; the tracker repository is local and publication is an owner action.
- **Rejected:** A submodule, an orphan branch, a subtree, and staying in the
  tree with the tracker excluded from the budget (`@/issue/fe2221d5`).
- **Rejected:** Cutting over before or after the rewrite: either keeps tracker
  history in the code repository or needs a second quiet window.
- **Reasoning:** Tracker writes from several leads contend on one tracked file
  and consume the tree budget; a shared store outside the code history removes
  both, and one quiet window serves the rewrite and the cutover.
- **Date:** 2026-11-14

### D-9: Time budgets are an epic gate

- **Chosen:** Task-level test runs report a fast-tier test past its limit as
  slow and never kill it. The epic gate `test-budgets` enforces the per-test
  and suite budgets of `@/inv/test-tier-budgets` on a quiet host. Test
  processes hold host pool tokens equal to the threads they run
  (`@/issue/18276ff1`).
- **Rejected:** A per-test kill at task level: a test waiting for the host
  pool is killed on a loaded host, and a busy-host override then widens the
  limit it was meant to enforce.
- **Rejected:** No kill at any level: the budgets exist to catch a test that
  does too much.
- **Reasoning:** A budget is meaningful only on a quiet host; unit tests run in
  a fraction of a second, so a breach is a property of the test and is found
  once per epic.
- **Date:** 2026-11-15
