# Worker-aware LDPC routing

> **Diátaxis Type:** Reference

Implementation evidence for `@/issue/d06a99ed`.

## Selection and execution context

The coding-owned `DecodeSelectors::lane_max_workers` bounds configured decode
concurrency in the canonical `decide_batch_route` predicate. Its conservative
value is one; the owner codec preserves the absence of this selector when
reencoding a partial section. The committed coding envelope therefore retains
its bytes and its single-worker lane domain. At six, twelve and twenty-four
workers it selects the single-frame route, including every configuration the
[multicore outcome](../a03daf3f/outcome.md) records as `fail`. Those outcomes
remain unchanged, and no multicore lane-performance claim is added. This
satisfies REQ-02 through its conservative-selection alternative; the
production before/after confirmation is under
[Before/after measurement](#beforeafter-measurement).

`BatchSoftDecoder::decode_batch_with_workers` accepts an explicit
`NonZeroUsize` per call and remains object safe. Its default forwards to the
existing decoder behavior. LDPC, rate-matched NR and DVB concatenated wrappers
propagate the context. Their standalone `decode_batch` methods and route and
posterior conveniences use one-worker semantics. The explicit route and
posterior methods take the same context as decoding.

`PooledBatchDecoder` supplies its execution pool's current configured width,
including calls outside a Rayon worker and reuse under a differently sized
pool. It preserves a larger explicit caller width. Without coding's `parallel`
feature it preserves the explicit context while executing sequentially.
Simulation consumers pass their execution context independently of the coding
feature. No constructor caches the route context.

The offline calibration retains its measured axes. Direct lane-bound cases
use one configured worker; pooled task-size cases explicitly admit their
four-worker forced candidate. Emitted lane bounds omit the unmeasured worker
selector and consequently retain its conservative value. Selector evidence
includes the resolved worker bound. The existing campaign validator accepts
both frozen evidence with an omitted worker field and current explicit
candidate contexts. No schema or protocol version changes.

Live lane benchmark arms explicitly admit up to the existing twenty-four-worker
grid and pass cell concurrency. The tuned arm passes cell concurrency and
reports the corresponding route.

## Before/after measurement

The [measurement preparation](measurement/rework.md) states the six-cell
non-regression family, the pinned before and after revisions and the
unchanged `tuned-profile` arm; [commands](measurement/commands.md) gives the
exact commands. Both campaigns are verified from their own execution logs:
six `cell-start` and six `cell-complete` records, every cell `measured` at its
declared pairs, and a terminal `complete` record.

- The [pilot receipt](../../../bench_results/d06a99ed/worker-routing/d06a99ed-worker-routing-pilot/acceptance-summary.md)
  is accepted and exploratory; its cells decide nothing. The canonical freezer
  derives the confirmation's resolution from it
  ([derivation](measurement/confirmation/derivation.txt)).
- The [confirmation receipt](../../../bench_results/d06a99ed/worker-routing/d06a99ed-worker-routing-confirmation/acceptance-summary.md)
  is accepted and does not qualify for production selection. The
  [outcome table](measurement/outcome.md) gives each cell's decision and
  outcome as the evaluator records them; intervals, pair counts and flagged
  windows are the acceptance summary's columns.
- Each pair's `selected_path` in the confirmation receipt records the wave
  route each arm takes: single-frame on both revisions for one-frame calls,
  AVX2 lanes on both revisions for one-worker full waves, and lanes before
  versus single-frame after at both multicore cells.
- At twelve physical cores (DVB-T2) and twenty-four logical CPUs (NR), the
  multicore cells record `improved` and `pass`: the interval's lower bound
  for the after revision's single-frame route against the before revision's
  lane route reaches the improvement margin.
- The worker bound stays in production under the [invoker adoption
  ruling](measurement/rework.md#adoption-ruling).
- The small whole-call cells, one frame at one worker on both codes, include
  worker dispatch, selector evaluation, conversion and extraction in every
  timed call; both record `not-worse` and `pass`. No cell isolates the
  selector predicate.
- The NR full wave at one worker records `pass`. The DVB-T2 full wave at one
  worker, where both revisions take the lane route, records `inconclusive`:
  its interval spans a margin, and it is the only cell with flagged windows
  (acceptance summary, column *Flagged*).
- Each pair's `conversion.dispatch_ns` in the confirmation receipt
  (`cells[].pairs[].{baseline,candidate}.conversion`) is the driver's
  empty-body executor-dispatch diagnostic, the median of empty-body dispatches
  after the measured windows, not a selector-predicate timing.

## Behavioral verification

All Cargo execution below uses `CARGO_CI_NO_SCCACHE=1`, because this sandbox
rejects the compiler-cache wrapper. Test execution uses
`./scripts/cargo-budget.sh --test cargo nextest run`, the `ci-test` Cargo
profile and the `ci` nextest profile.

- All-feature coding route, batch, pooled, codec, committed-profile and offline
  calibration suites: 167 passed, nextest run
  `b15a816d-bb78-497a-b371-61a7f0c74673`.
- The final envelope-byte round trip and pooled regression selection: nine
  passed, run `1270a59b-d95a-438d-8bb2-ca3180c44b9d`.
- Coding without default features, with `test-support,tuning-profile`:
  ten focused worker, posterior, trait-object and pooled tests passed, run
  `ab8dadb7-edf5-4d30-a788-31017d7a94b3`.
- Live lane, profile and tuned-arm workspace: fifteen tests passed, run
  `c3de76dd-3c08-4976-b4dd-ce02ec05e18b`. The standalone workspace uses
  the root `ci-test` settings through nextest's `--config` options and the
  canonical `ci` profile projected without workspace-specific filter overrides
  to `/tmp/gf2-standalone-nextest.toml`.
- Live tuning composer `modes` suite: four passed, run
  `7dde2586-0630-4976-bc1d-37b8ab89c867`, with the same standalone settings.
- `python3 dev/scripts/validate_tuning_extent_campaign_test.py`: 33 existing
  tests passed. The added
  `CodingOwner.test_worker_aware_payloads_declare_only_the_measured_concurrency`
  also passes and rejects incorrectly broadened candidate contexts.
- Simulation `batch_decode_worker_determinism` and `bler_sweep_batch`
  integration suites: eight passed; batch-decode stage unit tests: seven passed.
  The execution-context regression first observed `[1, 1, 1, 1, 1]` instead
  of `[1, 1, 2, 4, 1]`, then passed after context propagation. The same scratch
  and decoder are reused across standalone calls and pools of different widths.
- Checkpoint/resume regression: both existing batch tests pass over canonical
  scalar, portable and detected native route cases at one, two and four workers
  (nextest run `e0dcd7cd-ef98-4ea7-b65d-b12d900e3bf4`). Their initial lane
  assertion fails with the default conservative factory in run
  `cf02bc29-c455-4e6b-894e-0872d7b16177`. Explicit selectors retain each case's
  batch threshold and admit four workers; route assertions cover partial tails.
  Genuine interruption, frame partitions and resumed counters remain checked.

The shared posterior assertion compares each frame to the single-frame
reference at workers 1, 6, 12 and 24, on portable and detected native lane
backends. It exercises both conservative rejection and explicitly forced lane
routes, partial final waves, and mixed convergence. Existing batch contract
suites retain their named lane paths. Pool tests reuse one decoder under
multiple widths and assert explicit context propagation with and without
coding parallelism. Trait-object tests exercise default forwarding. Codec tests
cover worker-bound round trips, missing values, zero and null rejection.
The numerical contract is unchanged, satisfying REQ-03.

## Test-first evidence and invocation corrections

The added `worker_limit_bounds_configured_decode_concurrency` test first fails
compilation with `E0061`: the selector constructor accepts three arguments and
the route predicate accepts five, so the specified worker context is absent.
The implemented constructor and predicate make that behavioral boundary test
pass. The initial red command is the all-feature coding nextest invocation
filtered with `test(worker_limit_bounds_configured_decode_concurrency)`.

Resolved invocation/build corrections are recorded here for review:

- Omitting `cargo` after `cargo-budget.sh --test` gives
  `ionice: failed to execute nextest: No such file or directory`.
- The first ordinary Cargo invocation gives
  `sccache: error: Operation not permitted (os error 1)`; the documented cache
  override permits compilation.
- The first coding `cargo check --all-features --tests --benches` identifies an
  `E0061` at the calibration route call; that caller receives explicit context.
- The first focused coding build reports `E0308` for empty array references in
  the pooled spy; explicit slice typing fixes the fixture.
- Applying the whole root nextest configuration to a standalone workspace
  rejects its absent-package filter overrides (`operator didn't match any
  packages`); projecting the unchanged canonical `ci` settings avoids them.
- Cargo `--config` options preceding `nextest` are not forwarded to its child
  Cargo build (`profile ci-test is not defined`); nextest's own `--config`
  options supply the settings.
- The first live-arm test build identifies `E0433` for a missing test-module
  `NonZeroUsize` import; the import is present.

Full CI and tracker review gates belong to the integration lead.
