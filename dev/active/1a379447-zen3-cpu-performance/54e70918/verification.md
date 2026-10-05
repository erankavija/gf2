# SIMD equivalence tests that assert on every host (jit:54e70918)

> **Diátaxis Type:** Research

Every command runs from the worktree root with Rust 1.95 selected through
`RUSTUP_TOOLCHAIN=1.95.0`. No command takes a timing window, the host lock, or
an ignored test.

## Detection has no override

`survey/source-evidence.json`, written by `survey/make-source-evidence.py` and
checked with
`python3 dev/scripts/verify-source-evidence.py <ledger> --project gf2 --frame recorded-commits`,
holds the code claims. `maybe_simd` reads a process-wide value that detection
initializes once, and detection reads the target and the processor feature
only, so a host with AVX2 cannot be made to report a missing bundle. The early
return of each test in the two named files is therefore established by reading:
the ledger rows `demo-returns-without-a-backend` and
`hoist-returns-without-a-backend` cite the guard line and its occurrence count
in the tree before the test changes.

A build without the `simd` feature is the configuration this host can run in
which every probe finds nothing; the `no-simd` logs below are that run.

## List form

`gf2_core::kernels::backend::contract` follows
`gf2_kernels_simd::m4rm::contract`: `backends` lists the scalar reference, which
every build has, then the detected SIMD backend; `assert_each` refuses an empty
list and prints one `implementation:` line per entry before running the check on
it. A test compares every entry with a definition that shares no code with the
entries:

- the `Backend` tests of `kernels::backend` and the tests of
  `tests/simd_equiv_demo.rs` run each backend against word-by-word arithmetic;
- `tests/simd_equiv_dispatch_hoist.rs` and `tests/simd_equiv_gf2m_batch.rs` test
  entry points that dispatch on every host, as a list of one entry whose label
  names what the entry point dispatches to;
- the prime-field dispatch tests of `gfp::simd_ops` list the `Fp<P>` reference
  first, checked against integer arithmetic modulo `P`, then the accelerated
  dispatch when its bundle is published.

The three `simd_equiv_*` targets carry no `required-features` entry and build
without `simd`.

## Choice per test (REQ-01, REQ-02)

Every test of the two named files takes the first outcome of REQ-01: its
reference assertions run on every host. `demo_xor_matches_the_word_loop_proptest`
is also the positive control of `helper_rejects_a_mutated_reference`.

`survey/test-implementations.md`, written by `survey/make-test-table.py`, holds
one row per list-form test with the labels it printed in each feature
configuration. The generator fails when a listed test prints no label in some
configuration or when a test of the two named targets is absent from the list;
its `Problems` line states the count.

## Runs

`survey/run-suites.sh` writes `survey/test-logs/<configuration>-<scope>.txt` for
the configurations `default`, `simd` (`--features simd`), `all-features` and
`no-simd` (every feature except `simd`) and the scopes its header defines, and
`all-features-precondition.txt` for the calibration-harness tests. Each
log opens with its command and closes with its exit status.

The `default-whole` and `simd-whole` runs stop at a compile error in a test
target that needs a feature neither configuration enables; the first `error`
line of each log names it, and jit:316150fd owns it. The `-targets` scope is the
part of those configurations that compiles: the library tests and the three
`simd_equiv_*` targets.

## Search record (REQ-03)

`survey/find-simd-early-returns.py` writes `survey/simd-early-returns.json`. The
record states its rule, the probe names it derives, and one row per test that
returns early when a SIMD probe finds nothing, with the owning package and
whether the root workspace builds it; `tests_by_package` holds the counts.
`--self-test` runs the rule over one sample per listed and unlisted form.

No row of the record belongs to `gf2-core`. The rows of `gf2-kernels-simd`
belong to jit:6605cac2 and the rows of `gf2-algebra` to jit:dcfe8386. The
remaining rows lie in packages outside the root workspace.

## Calibration-harness precondition (REQ-03)

Six owner contract tests of `crates/gf2-core/benches/tuning_calibration.rs`
decide over a synthetic campaign with two arms of the bit-backend family and
have no reference to assert without the SIMD arm. Each opens with
`require_simd_arm(simd_backend())`, which panics with a message that opens
`precondition:` when the probe finds nothing, and
`a_missing_simd_arm_fails_on_the_stated_precondition` hands the guard a missing
arm and requires that message. `survey/test-logs/all-features-precondition.txt`
holds the run.

The harness target does not compile without the `simd` feature, so no
configuration this host can run reaches the guard with a missing arm through
the probe; the ledger row `calibration-contract-returns-without-a-backend`
cites the guard line of the tree before the change and its occurrence count.

`survey/record-producer-identity.sh` builds the producer from the working tree
and from the source held under `survey/inputs/before/` and writes
`survey/producer-identity.json`. Its `binary_equal` and `source_equal` fields
state the outcome: the edit changes the bytes of the source and leaves the
release binary and the behaviour token `CoreTuningCodec::HARNESS_SCHEMA` as
they are. The measured owner that `crates/gf2-core/src/tuning/baked.rs` pins by
digest and the launcher that names the published profile are outside the edit.
