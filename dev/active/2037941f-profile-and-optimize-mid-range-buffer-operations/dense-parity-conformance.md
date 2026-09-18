# Dense-parity harness conformance record

> **Diátaxis Type:** Reference
>
> **Owning issue:** `e1f9a78f`
>
> **Subject:** [`dense-parity-harness.md`](dense-parity-harness.md), the harness
> under [`survey/dense-harness/`](survey/dense-harness/)
>
> **Against:** [`dense-parity-addendum.md`](dense-parity-addendum.md), identity
> `2037941f-dense-parity-v1`, and the
> [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
> and [protocol version 4](../f547c394/protocol.md) it specializes

This record walks the frozen addendum from top to bottom and, for every
normative clause, names the harness location that carries it, the test that
binds it, and the verdict. Paths are relative to this document's directory and
their line numbers are valid at the commit that carries this file; tests are
named rather than numbered, so a test reference survives a line shift. The
record holds pointers only: no figure is copied from a receipt, and a clause
whose numbers are frozen is cited by its own section.

A clause the runner, the evaluator or the canonical freezer owns is listed with
that owner in the location column, because the harness's part is to leave the
decision to it.

## Totals

| Clauses checked | Conforming | Fixed here | Cannot conform |
|---:|---:|---:|---:|
| 112 | 105 | 7 | 0 |

The seven fixed clauses are the rows whose verdict names a commit. Every other
row conformed before this round; no clause of the addendum, the contract or the
protocol is unsatisfiable by this harness, and no frozen or pinned byte changed.

## Operations and cost boundaries

| Clause | Harness location | Test | Verdict |
|---|---|---|---|
| "The campaign uses four distinct questions. They never share a timing cell." | `survey/dense-harness/src/cells.rs:358` builds one cell per question and axis; the `scalar-reference` rows are their own cells | `the_cell_table_reproduces_the_frozen_matrices` | conforms |
| `isolated-fused-parity` baseline: one call to the bundle's `and_popcnt_fn` "reached through the bundle function pointer behind an optimisation barrier" | `survey/dense-harness/src/routes.rs:150` | `a_cell_workload_names_the_question_every_arm_of_that_cell_serves` | conforms |
| Isolated boundary excludes fixture generation, row selection, the parity fold, output allocation and result observation | `survey/dense-harness/src/bin/dense-arm.rs:49` builds and reports outside the window; the body calls the entry alone | `whole_consumer_cells_charge_conversion_costs` | conforms |
| `allocated-matvec` charges route selection, the per-row bundle call, the parity fold, the output allocation and every append; construction, validation and disposal stay outside | `survey/dense-harness/src/routes.rs:156`, `survey/dense-harness/src/bin/dense-arm.rs:78` | `a_windowed_execution_retains_and_releases_through_the_arm_arrangement` | conforms |
| `matvec-vs-m4ri` gf2 arm allocates *and releases* its `BitVec` inside the window | `survey/dense-harness/src/bin/dense-arm.rs:133` drops each output at the end of the measured call | `a_cell_workload_names_the_question_every_arm_of_that_cell_serves` | conforms |
| `matvec-vs-m4ri` external arm charges `mzd_init`, packing, `mzd_mul(y, A, x, 0)`, `mzd_read_bit` unpacking and disposal | `survey/m4ri_matvec_arm.c:59`, `survey/dense-harness/src/bin/dense-m4ri-arm.rs:69` | `dense-m4ri-arm --oracle` (record `survey/dense-runner-smoke.txt`) | conforms |
| `scalar-reference` uses "the `allocated-matvec` boundary exactly" from a build without `simd` | `survey/dense-harness/src/routes.rs:102` and the same `Workload::Matvec` arm body | `a_route_refuses_the_build_it_cannot_serve` | conforms |
| The scalar route is reachable only through the build with `simd` disabled | `survey/dense-harness/src/routes.rs:127`; `survey/run-dense-harness.sh:72` builds it | `a_route_refuses_the_build_it_cannot_serve` | conforms |
| "Every arm is serial." | `survey/dense-harness/src/campaign.rs:193` declares one worker; `survey/dense-harness/src/wire.rs:260` refuses another declaration; `survey/dense-harness/src/wire.rs:241` reports one | `the_request_mirror_accepts_exactly_the_runner_request` | conforms |
| "Source and destination never alias." | `survey/dense-harness/src/fixture.rs:125` allocates a separate slab per operand; `matvec` returns a fresh output | `every_declared_cell_builds_the_working_set_its_cache_state_fixes` | conforms |
| The gf2 output is a freshly allocated `BitVec` and no cell reuses a caller-supplied buffer | `survey/dense-harness/src/routes.rs:156` | `the_retention_bound_is_derived_from_the_largest_declared_retaining_cell` | conforms |
| Component 2, `mzd_init` with zero initialization, inside timing (fresh) and `y` only (retained) | `survey/m4ri_matvec_arm.c:62`, `survey/m4ri_matvec_arm.c:101` | `dense-m4ri-arm --oracle` | conforms |
| Component 3, packing through public coordinates, inside timing (fresh) and retained outside (retained) | `survey/m4ri_matvec_arm.c:38`, `survey/m4ri_matvec_arm.c:81` | `dense-m4ri-arm --oracle` | conforms |
| Component 5, `mzd_read_bit` unpacking into a gf2 `BitVec`, inside timing on both arrangements | `survey/m4ri_matvec_arm.c:49`, `survey/dense-harness/src/bin/dense-m4ri-arm.rs:105` | `dense-m4ri-arm --oracle` | conforms |
| Component 6, disposal of every owned `mzd_t` inside timing, `y` only when retained | `survey/m4ri_matvec_arm.c:74` frees on every exit path; `survey/dense-harness/src/bin/dense-m4ri-arm.rs:113` releases retained state once | `dense-m4ri-arm --oracle` drives repeated fresh and retained calls on all six shapes | conforms |
| Component 1 and its gf2 counterparts stay outside timing | `survey/dense-harness/src/bin/dense-m4ri-arm.rs:259` converts before the window | `every_comparator_cell_is_a_warm_whole_consumer_cell` | conforms |
| Both arms of a comparator cell "declare conversion and setup costs included" | `survey/dense-harness/src/campaign.rs:197` | `every_comparator_cell_is_a_warm_whole_consumer_cell` | conforms |
| The comparator and allocated families' per-call numbers are not interchangeable; neither is a baseline or resolution source for the other | `survey/dense-harness/src/cells.rs:93` gives each question its own ledger and family identity | `every_family_ledger_exists_at_genesis` | conforms |

## Selected production route

| Clause | Harness location | Test | Verdict |
|---|---|---|---|
| The primary route is the public `BitMatrix::matvec` | `survey/dense-harness/src/routes.rs:156` | `every_anchor_stride_resolves_the_lane_its_build_fixes` | conforms |
| The selector takes the SIMD lane at and above the conservative eight-word threshold, so every anchor stride is on that lane | `survey/dense-harness/src/cells.rs:49`, `survey/dense-harness/src/routes.rs:304` | `every_anchor_stride_resolves_the_lane_its_build_fixes` | conforms |
| "Every arm of this addendum places the same optimisation barrier around the bundle" | `survey/dense-harness/src/routes.rs:150`, `survey/dense-harness/src/routes.rs:156`; the harness release profile mirrors the workspace's (`survey/dense-harness/Cargo.toml:64`) | `a_cell_workload_names_the_question_every_arm_of_that_cell_serves` | conforms |
| "each arm reports the route it observed for its own width" | `survey/dense-harness/src/bin/dense-arm.rs:60`, `survey/dense-harness/src/bin/dense-arm.rs:99`, `survey/dense-harness/src/external.rs:108` | `the_comparator_provenance_names_the_object_it_loaded` | conforms |
| Anchor strides, their column counts and the fixed row count | `survey/dense-harness/src/cells.rs:39`, `survey/dense-harness/src/cells.rs:47`, `survey/dense-harness/src/cells.rs:130` | `the_cell_table_reproduces_the_frozen_matrices` | conforms |
| "The harness verifies every stated stride, column count, row count, and route from the constructed public objects before enabling timing." | `survey/dense-harness/src/routes.rs:270`, `survey/dense-harness/src/routes.rs:304`, called at `survey/dense-harness/src/bin/dense-arm.rs:83` | `every_declared_cell_builds_the_working_set_its_cache_state_fixes` checks every item of every bank | conforms |
| "A mismatch makes the cell unavailable; it never substitutes another shape." | `survey/dense-harness/src/routes.rs:279` returns the mismatch | `a_mismatched_shape_makes_the_cell_unavailable` | conforms |

## Frozen sizes and cell matrices

| Clause | Harness location | Test | Verdict |
|---|---|---|---|
| The anchor word counts are exactly five; the neighbours are exactly two | `survey/dense-harness/src/cells.rs:39`, `survey/dense-harness/src/cells.rs:41` | `the_cell_table_reproduces_the_frozen_matrices` | conforms |
| "The anchor cells, not the neighbours, form the confirmatory multiple-comparison families." | `survey/dense-harness/src/cells.rs:372` marks the anchors | `every_anchor_stride_is_a_confirmatory_cell_of_both_gf2_questions` | conforms |
| Neighbour, partial-tail, cold, streaming, retained-state and scalar-reference cells stay exploratory | `survey/dense-harness/src/campaign.rs:180` gives every transcribed cell the exploratory role | `every_family_transcribes_into_a_valid_version_four_addendum` | conforms |
| Every cell declares `single-core-latency`, `single-core`, one worker, no nested pools | `survey/dense-harness/src/campaign.rs:191` | `every_family_transcribes_into_a_valid_version_four_addendum` | conforms |
| The multicore arms are inapplicable and receipts retain each with its reason | runner `dev/tools/tuning-campaign-support/src/host.rs`, per protocol P-14; the harness declares only the single-core arm | `every_family_transcribes_into_a_valid_version_four_addendum` | conforms |
| Isolated primary product: word count, one row and one vector of $W$ words each at address $0 \pmod{64}$, warm, `kernel-isolated`, conversion excluded, `improvement`, identifier `and-popcnt-{W}w-warm` | `survey/dense-harness/src/cells.rs:363`, `survey/dense-harness/src/fixture.rs:16` | `isolated_operands_begin_on_the_declared_boundary`, `the_cell_table_reproduces_the_frozen_matrices` | conforms |
| The exact isolated exploratory additions: the two neighbours and one streaming cell per anchor | `survey/dense-harness/src/cells.rs:373` | `the_cell_table_reproduces_the_frozen_matrices` | conforms |
| "One reported operation is one call to the bundle entry. The arm reports the addresses of both operands modulo 64." | `survey/dense-harness/src/bin/dense-arm.rs:69`, `survey/dense-harness/src/bin/dense-arm.rs:60` | `every_declared_cell_builds_the_working_set_its_cache_state_fixes` | conforms |
| Allocated primary product: $64W$ columns, 1024 rows, warm, `whole-consumer`, conversion included, `improvement`, identifier `matvec-r1024-{W}w-warm` | `survey/dense-harness/src/cells.rs:377`, `survey/dense-harness/src/campaign.rs:176` | `the_cell_table_reproduces_the_frozen_matrices` | conforms |
| The exact allocated exploratory additions: two neighbours, one `tail1` per anchor, two named cold cells, one streaming cell per anchor, one scalar-reference cell per anchor | `survey/dense-harness/src/cells.rs:391` through `survey/dense-harness/src/cells.rs:398` | `the_cell_table_reproduces_the_frozen_matrices` | conforms |
| "One reported operation is one public `matvec` call, including the allocation and the 1024 appends of its returned `BitVec`." | `survey/dense-harness/src/bin/dense-arm.rs:112` | `a_windowed_execution_retains_and_releases_through_the_arm_arrangement` | conforms |
| Every comparator cell declares `whole-consumer`, `single-core-latency`, `single-core`, `warm`, conversion included, the `conservative-portable` gf2 arm and the `external` arm | `survey/dense-harness/src/cells.rs:403`, `survey/dense-harness/src/campaign.rs:163` | `every_comparator_cell_is_a_warm_whole_consumer_cell`, `the_comparator_arm_is_the_external_build_and_the_gf2_arms_are_not` | conforms |
| The confirmatory comparator cells are exactly the two qualified mid-range shapes | `survey/dense-harness/src/cells.rs:208` marks them | `every_qualified_comparator_shape_is_a_frozen_cell` | conforms |
| The exploratory comparator matrix adds the four remaining qualified shapes and the two retained-state cells; a retained-state cell never replaces a fresh one | `survey/dense-harness/src/cells.rs:413` | `the_cell_table_reproduces_the_frozen_matrices` | conforms |
| The external arm converts once per retained-state cell and both arms still allocate a fresh output | `survey/dense-harness/src/bin/dense-m4ri-arm.rs:262`, `survey/m4ri_matvec_arm.c:99` | `every_comparator_cell_is_a_warm_whole_consumer_cell` | conforms |
| The external arm must arrange the one warm item the family declares | `survey/dense-harness/src/bin/dense-m4ri-arm.rs:249` refuses another cache state | `every_comparator_cell_is_a_warm_whole_consumer_cell` | FIXED in `5c111d32` |
| The three unqualified anchor strides are retained as `unavailable` rows with a reason and zero samples, take no ordinal, and carry the exploratory role | `survey/dense-harness/src/cells.rs:232`, printed by `survey/dense-harness/src/bin/dense-campaign.rs:230` | `the_unavailable_comparator_rows_take_no_ordinal` | conforms |
| Admitting an unqualified shape needs a newly qualified shape and a versioned amendment | `survey/dense-harness/src/wire.rs:145` refuses a case whose shape is not qualified | `an_unfrozen_case_is_refused` | conforms |

## Input identities and correctness

| Clause | Harness location | Test | Verdict |
|---|---|---|---|
| The campaign seed | `survey/dense-harness/src/cells.rs:37` | `seeds_follow_one_campaign_splitmix_stream` | conforms |
| "Cells are ordered first by the question order in this document, then by the order of each listed axis." | `survey/dense-harness/src/cells.rs:358` | `the_cell_table_reproduces_the_frozen_matrices` | conforms |
| Ordinal zero uses the campaign seed; ordinal $i$ the $i$-th subsequent SplitMix64 output | `survey/dense-harness/src/cells.rs:435` | `seeds_follow_one_campaign_splitmix_stream` | conforms |
| "Each additional fixture bank consumes the next SplitMix64 output from that cell's stream." | `survey/dense-harness/src/fixture.rs:125` and `survey/dense-harness/src/fixture.rs:217` start one stream per cell at its workload seed and consume it bank after bank | `one_canonical_stream_fills_every_bank_in_order` | conforms |
| The generator is the canonical `abtest::SplitMix64`; no substitute and no hashing of cell-name text | `survey/dense-harness/src/cells.rs:14`, `survey/dense-harness/src/fixture.rs:13` | `seeds_follow_one_campaign_splitmix_stream` | conforms |
| "SplitMix64 fills the matrix words in canonical row-major order and then the vector words." | `survey/dense-harness/src/fixture.rs:236` | `one_canonical_stream_fills_every_bank_in_order` | conforms |
| "Every generated word is one full 64-bit output", so the pinned density is one half | `survey/dense-harness/src/fixture.rs:95` | `one_canonical_stream_fills_every_bank_in_order` | conforms |
| In `tail1` cells the final word of each row and of the vector is masked immediately after generation | `survey/dense-harness/src/fixture.rs:240`, `survey/dense-harness/src/fixture.rs:246` | `a_tail1_fixture_carries_canonical_zero_tail_padding`, `one_canonical_stream_fills_every_bank_in_order` | conforms |
| The mask is reapplied after every operation; the matrix and the vector are read-only, so no cell resets inside a window | `survey/dense-harness/src/routes.rs:156` takes both by shared reference | `the_oracle_covers_every_frozen_boundary_and_passes` (the `tail1` cases check the padding after the call) | conforms |
| "Both arms of a cell see the same fixture bytes." | `survey/dense-harness/src/fixture.rs:217`, reached from both arms with the cell's own seed | `a_working_set_is_a_pure_function_of_its_cell` | conforms |
| The external arm receives the same canonical gf2 words and reaches only `mzd_write_bit`/`mzd_read_bit`; no arm exposes M4RI storage as a word slice or compares physical strides | `survey/dense-harness/src/bin/dense-m4ri-arm.rs:59`, `survey/m4ri_matvec_arm.c:38` | `dense-m4ri-arm --oracle` | conforms |
| The untimed smoke covers logical column counts 0, 1, 63, 64, 65, all seven word counts and both shapes | `survey/dense-harness/src/oracle.rs:99`, `survey/dense-harness/src/oracle.rs:127` | `the_oracle_covers_every_frozen_boundary_and_passes` | conforms |
| The untimed smoke covers both cold and warm entry and resume after each cell boundary | `survey/dense-harness/src/smoke.rs:131` with `max_cells_per_session` one; `survey/make-dense-smoke-addenda.py:23` contributes the frozen cold cell | record `survey/dense-runner-smoke.txt`, judged by `survey/check-dense-smoke.py` | conforms |
| "The oracle is an independent row-parity computation in the harness, not a gf2 route" | `survey/dense-harness/src/oracle.rs:41` | `the_oracle_covers_every_frozen_boundary_and_passes` | conforms |
| The oracle checks every output bit, the output length, canonical LSB-first indexing, zero tail padding and input immutability | `survey/dense-harness/src/oracle.rs:48`, `survey/dense-harness/src/oracle.rs:108` | `the_oracle_covers_every_frozen_boundary_and_passes` | conforms |
| The external arm additionally checks its allocations, the zeroing of excess bits, the coordinates and disposal | `survey/dense-harness/src/bin/dense-m4ri-arm.rs:150` | `dense-m4ri-arm --oracle` | conforms |
| "Smoke execution emits no timing samples and cannot serve as a pilot." | `survey/dense-harness/src/routes.rs:374` returns before the timing protocol; `survey/dense-harness/src/smoke.rs:426` refuses a window | `a_zero_window_arrangement_pass_collects_no_timing_sample` | conforms |

## Cache, warmup, and sampling

| Clause | Harness location | Test | Verdict |
|---|---|---|---|
| "All builds finish before timing and use release mode." | `survey/run-dense-harness.sh:293` builds every launched executable before the closure checks and the launch | record `survey/dense-harness-validation.txt` | conforms |
| "Measurements run on the prepared Ryzen 9 5900X under the repository's full-host benchmark-window lock" | `survey/dense-harness/src/campaign.rs:430`, `survey/run-dense-harness.sh:375`; the arms refuse a hand invocation at `survey/dense-harness/src/wire.rs:283` | `the_request_mirror_accepts_exactly_the_runner_request` | conforms |
| The runner records CPU IDs, topology, affinity, SMT, governors, clocks, capabilities, toolchain, executable digests and selected routes | runner `benchmark-ab-runner` per protocol P-06/P-09; each arm reports its own affinity and route at `survey/dense-harness/src/wire.rs:225` | `the_comparator_provenance_names_the_object_it_loaded` | conforms |
| The gf2 arms are `conservative-portable` with no `target-cpu` override; the reference arm removes `simd`; the external arm is the pinned build | `survey/dense-harness/src/campaign.rs:366`, `survey/dense-harness/src/campaign.rs:326`, `survey/dense-harness/build.rs:58` | `the_comparator_arm_is_the_external_build_and_the_gf2_arms_are_not` | conforms |
| `warm`: "execute exactly one untimed pass of the measured operation over that cell's complete working set before calibration", using the same buffers including the output | `survey/dense-harness/src/routes.rs:363` | `warm_runs_one_untimed_pass_and_streaming_runs_none` | FIXED in `76c24b30` |
| `streaming`: eight fixture banks of at least 8 MiB each, rounded up to an integral number of complete tuples | `survey/dense-harness/src/fixture.rs:86`, `survey/dense-harness/src/fixture.rs:120`, `survey/dense-harness/src/fixture.rs:212` | `every_declared_cell_builds_the_working_set_its_cache_state_fixes` | FIXED in `f684c330` |
| `streaming`: touch every initialized byte outside timing and execute no measured operation as warmup | `survey/dense-harness/src/fixture.rs:166`, called at `survey/dense-harness/src/bin/dense-arm.rs:54`; `survey/dense-harness/src/routes.rs:363` warms only a warm cell | `warm_runs_one_untimed_pass_and_streaming_runs_none` | conforms |
| `streaming`: rotate banks once per operation | `survey/dense-harness/src/routes.rs:379` over `timing::execution_windows_fixed_or_calibrated` | `every_declared_cell_builds_the_working_set_its_cache_state_fixes` | conforms |
| `streaming`: "The reported working set is at least 64 MiB." | `survey/dense-harness/src/fixture.rs:160`, reported at `survey/dense-harness/src/bin/dense-arm.rs:60` | `every_declared_cell_builds_the_working_set_its_cache_state_fixes` | conforms |
| A receipt retains the fixture-bank sizes | `survey/dense-harness/src/bin/dense-arm.rs:60`, `survey/dense-harness/src/bin/dense-arm.rs:99`, `survey/dense-harness/src/bin/dense-arm.rs:142` | `survey/check-dense-smoke.py:33` refuses a handshake that names none | FIXED in `29ccc550` |
| `cold`: fresh child, fresh fixture, no measured operation before the first window, `cold_calls` fixed to one | `survey/dense-harness/src/cells.rs:168`, `survey/dense-harness/src/routes.rs:356` | `a_cache_state_and_its_frozen_call_count_must_agree` | conforms |
| Every exploratory cell runs exactly the pilot maximum of paired executions; the rows declared unavailable run none | `survey/dense-harness/src/campaign.rs:36`, applied at `survey/dense-harness/src/campaign.rs:377` | `every_projected_cell_runs_the_frozen_number_of_paired_executions` | FIXED in `b9510dca` |
| Each pair launches adjacent fresh children in the seed-determined, two-pair-counterbalanced order | runner `abtest::pair_orders` per protocol P-16 | `every_projected_cell_runs_the_frozen_number_of_paired_executions` (the plan carries the campaign seed) | conforms |
| Each warm or streaming execution uses five windows targeted at 100 ms after calibration; a cold execution uses five one-call windows | `survey/dense-harness/src/campaign.rs:431` leaves the protocol's shared settings in force; `survey/dense-harness/src/wire.rs:64` refuses any other window protocol | `an_arm_refuses_a_window_protocol_the_addendum_does_not_declare` | FIXED in `eace1439` |
| "No adaptive sample extension, early significance stop, or reuse of exploratory pairs is allowed." | `survey/dense-harness/src/campaign.rs:244`; the harness adds no sampling path of its own | `every_family_transcribes_into_a_valid_version_four_addendum` | conforms |
| "The child timeout is 120 seconds." | runner setting `child_timeout_seconds`; the smoke uses the same at `survey/dense-harness/src/smoke.rs:52` | `a_launched_arm_child_carries_the_frozen_timeout` | FIXED in `66649494` |
| The outlier policy removes nothing; a flagged fraction above its limit makes a cell unstable | evaluator `benchmark-acceptance` over the protocol's shared settings | `every_family_transcribes_into_a_valid_version_four_addendum` | conforms |

## Estimator, confidence, and multiple comparisons

| Clause | Harness location | Test | Verdict |
|---|---|---|---|
| The resampling unit is one whole paired execution and the estimator is the ratio of medians | evaluator, protocol § Estimator and interval | — | conforms |
| The interval is the protocol-v4 nearest-rank percentile bootstrap over its own generator | evaluator, protocol § Estimator and interval | — | conforms |
| The family error rate, the attempt alpha and the Bonferroni correction come from the ledger chain | `survey/dense-harness/src/campaign.rs:236` declares the family alpha and the ledger; the evaluator applies the correction | `every_family_transcribes_into_a_valid_version_four_addendum` | conforms |
| "A reservation counts a family's non-exploratory cells only" | `survey/dense-harness/src/campaign.rs:180`: a pilot transcription declares no non-exploratory cell | `every_family_transcribes_into_a_valid_version_four_addendum` | conforms |
| Every row declared unavailable in advance carries the exploratory role and spends no comparison | `survey/dense-harness/src/cells.rs:232`: those rows are not cells at all | `the_unavailable_comparator_rows_take_no_ordinal` | conforms |
| "Each canonical question owns a distinct ledger and family identity." | `survey/dense-harness/src/cells.rs:84`, `survey/dense-harness/src/cells.rs:93` | `every_family_ledger_exists_at_genesis` | conforms |
| "The version-1 prior-counter fields stay empty in every transcription" | `survey/dense-harness/src/campaign.rs:237` | `every_family_transcribes_into_a_valid_version_four_addendum` | conforms |
| Candidate selection families set `holdout.required` to false with no holdout cells | `survey/dense-harness/src/campaign.rs:247` | `every_family_transcribes_into_a_valid_version_four_addendum` | conforms |
| Each family's ledger is created once as an empty genesis file and retained | `dev/bench_results/2037941f/`, named at `survey/dense-harness/src/cells.rs:93`; the smoke names a throwaway path instead (`survey/make-dense-smoke-addenda.py:55`) | `every_family_ledger_exists_at_genesis` | conforms |

## Effect, resolution, and complexity rules

| Clause | Harness location | Test | Verdict |
|---|---|---|---|
| The pilot-derived resolution is pinned with its receipt and digest by the canonical freezer; a family whose resolution exceeds its ceiling runs no confirmation | canonical freezer `dev/active/c7113c5a/survey/freeze-confirmation.py`; the harness writes no confirmation addendum and leaves the field unresolved (`survey/dense-harness/src/campaign.rs:223`) | `every_family_transcribes_into_a_valid_version_four_addendum` | conforms |
| Each family's worthwhile speedup, equivalence margin and material gap are fixed here | `survey/dense-harness/src/campaign.rs:61`, `survey/dense-harness/src/campaign.rs:85`, `survey/dense-harness/src/campaign.rs:113` | `the_frozen_margins_reach_each_family_unchanged`, `a_changed_margin_or_cell_no_longer_matches_the_transcription` | conforms |
| The comparator family declares no worthwhile speedup and the gf2 families no material gap, by design rather than omission | `survey/dense-harness/src/campaign.rs:107`, `survey/dense-harness/src/campaign.rs:68` | `the_frozen_margins_reach_each_family_unchanged` | conforms |
| The complexity budget: one new unsafe kernel and 96 added nonblank lines for the gf2 families, zero and zero for the comparator | `survey/dense-harness/src/campaign.rs:72`, `survey/dense-harness/src/campaign.rs:117` | `the_frozen_margins_reach_each_family_unchanged` | conforms |
| Adoption requires all four conditions, and no candidate is selected here | the harness changes no production selection; `survey/dense-harness/src/campaign.rs:208` transcribes a pilot only | `every_family_transcribes_into_a_valid_version_four_addendum` | conforms |

## Search and stopping rules

| Clause | Harness location | Test | Verdict |
|---|---|---|---|
| At most four exploratory pilot trials per cell and exactly one confirmatory attempt per identity | `survey/dense-harness/src/campaign.rs:244` | `every_family_transcribes_into_a_valid_version_four_addendum` | conforms |
| "The pre-candidate campaigns use byte-identical public identity arms on both sides" | `survey/dense-harness/src/campaign.rs:253` pairs `-a` with `-b`; `survey/dense-harness/src/campaign.rs:322` resolves both to one executable unless a candidate is named | `the_projected_plan_covers_every_declared_cell_with_declared_builds` | conforms |
| "Candidate campaigns replace only the compared build." | `survey/dense-harness/src/campaign.rs:322` | `each_external_and_reference_arm_needs_its_own_executable` | conforms |
| No production change is selected from isolated throughput, a comparator gap or a retained-state cell | the harness selects nothing and edits no production code | `every_family_transcribes_into_a_valid_version_four_addendum` | conforms |

## Preserved no-win and scope exclusions

| Clause | Harness location | Test | Verdict |
|---|---|---|---|
| No candidate re-runs the carry-save comparator, edits the selector threshold or relabels the established route | `survey/dense-harness/src/routes.rs:52` declares only the fused entry and the public route; the threshold is read, never written (`survey/dense-harness/src/cells.rs:49`) | `a_route_refuses_the_build_it_cannot_serve` | conforms |
| The transpose and BCH-generator M4RI families and the sparse route are excluded | `survey/dense-harness/src/cells.rs:208` admits only the six qualified product shapes; `survey/m4ri_matvec_arm.c:70` calls only `mzd_mul` | `every_qualified_comparator_shape_is_a_frozen_cell`, `an_unfrozen_case_is_refused` | conforms |
| "no cell here proposes hoisting that resolution out of the call" | `survey/dense-harness/src/routes.rs:156` calls the public entry once per operation | `a_cell_workload_names_the_question_every_arm_of_that_cell_serves` | conforms |

## Receipt and stopping evidence

| Clause | Harness location | Test | Verdict |
|---|---|---|---|
| Every campaign snapshots this document, the contract, protocol, schema, campaign JSON, producing-source closure, executable, toolchain and inputs by SHA-256 before opening its log | `survey/make-dense-producing-inputs.py:32` enumerates the closure; `survey/run-dense-harness.sh:314` guards it with the campaign JSON and the ledger; `survey/dense-harness/src/smoke.rs:459` pins the same identities | `the_closure_names_every_manifest_a_timed_executable_builds_from`, `a_committed_and_clean_closure_admits_a_timed_run` | conforms |
| "The log path is printed before the first bounded run" | `survey/dense-harness/src/smoke.rs:163`, `survey/run-dense-harness.sh:360` | record `survey/dense-runner-smoke.txt` | conforms |
| "completed cells resume without repetition" | `survey/dense-harness/src/smoke.rs:210` | record `survey/dense-runner-smoke.txt`, judged at `survey/check-dense-smoke.py:94` | conforms |
| A session that stops inside a cell is closed with an `interrupted` record and abandoned once before that cell is measured again | canonical `journal` and runner, protocol v4 P-11 | record `survey/dense-runner-smoke.txt` (no abandonment in a clean smoke) | conforms |
| Receipts retain commands, raw pairs and windows, seeds, cache claims, observed routes and operand alignment, fixture-bank sizes, output validation, host facts and every negative row | runner receipt; the arm's contribution is `survey/dense-harness/src/wire.rs:225` | `the_comparator_provenance_names_the_object_it_loaded` | conforms |
| "only producing content identities decide validity" | `survey/dense-harness/src/inputs.rs` checks tracked and clean content, never a revision | `a_dirty_harness_source_refuses_a_timed_run`, `a_dirty_runner_source_refuses_a_timed_run` | conforms |

## Measurement contract and protocol clauses that bind the harness

| Clause | Harness location | Test | Verdict |
|---|---|---|---|
| Contract: every receipt pins the contract, protocol and addendum by content digest, and git locators are never critical provenance | `survey/dense-harness/src/cells.rs:20`, `survey/dense-harness/src/smoke.rs:459` | `the_carried_pins_are_the_frozen_addendums_own_identity` | conforms |
| Contract: bounded resumable runs open and print the append-only log before the first bounded run and checkpoint completed cells | `survey/dense-harness/src/smoke.rs:131` | record `survey/dense-runner-smoke.txt` | conforms |
| Contract: never add a parallel private campaign framework | the harness drives the canonical `journal`, `transport` and `timing` (`survey/dense-harness/src/smoke.rs:29`, `survey/dense-harness/src/routes.rs:14`) | `the_request_mirror_accepts_exactly_the_runner_request` | conforms |
| Contract: preserve canonical little-endian indexing, zero tail padding and the 0/1/63/64/65 boundaries | `survey/dense-harness/src/oracle.rs:74`, `survey/dense-harness/src/oracle.rs:79` | `the_oracle_covers_every_frozen_boundary_and_passes` | conforms |
| Contract: keep deterministic seeded behaviour across checkpoint/resume | `survey/dense-harness/src/cells.rs:358` is a pure function; `survey/dense-harness/src/fixture.rs:217` is a pure function of its cell | `cell_generation_is_deterministic`, `a_working_set_is_a_pure_function_of_its_cell` | conforms |
| `@/inv/runtime-observed-provenance`: a tool's source and emitted preambles carry no hand-written figure, file inventory or prior-run narrative | every pin is read at run time (`survey/dense-harness/src/bin/dense-m4ri-arm.rs:124`, `survey/dense-harness/build.rs:54`); the records are written by their generators | `the_carried_pins_are_the_frozen_addendums_own_identity` | conforms |
| Protocol: the warm pass is the timed body over the timed call's working set | `survey/dense-harness/src/routes.rs:363` runs the same `body` the windows run | `warm_runs_one_untimed_pass_and_streaming_runs_none` | FIXED in `76c24b30` (counted above) |
| Protocol: an omitted exploratory `pilot_pairs` selects the pilot minimum, so a plan states the count it wants | `survey/dense-harness/src/campaign.rs:377` | `every_projected_cell_runs_the_frozen_number_of_paired_executions` | FIXED in `b9510dca` (counted above) |
| Protocol: the resume identity pins plan, protocol, closure, cell list, arm descriptors and executable digests | `survey/dense-harness/src/smoke.rs:481` | record `survey/dense-runner-smoke.txt` | conforms |
| Protocol: `cache_state_applied` is reported by the arm and a mismatch invalidates the cell | `survey/dense-harness/src/wire.rs:240`, checked at `survey/dense-harness/src/smoke.rs:432` | record `survey/dense-runner-smoke.txt`, judged at `survey/check-dense-smoke.py:144` | conforms |

The two protocol rows marked "counted above" restate an addendum clause already
counted in its own section, so the totals count each clause once.
