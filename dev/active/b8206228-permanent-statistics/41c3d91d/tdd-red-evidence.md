# TDD red evidence — 41c3d91d

## 2026-08-16 — schedule API red reproduction

Scratch checkout: `/tmp/gf2-41c3d91d-red-8jCazb`, detached at
`8a2f8746~1` (`009d5a8099d6c59da47ccc87b810b0e6c1b1cc5c`). The only source
addition was the `schedule.rs` test module, with a test-only module declaration
in `permanent_campaign/mod.rs` so Cargo could compile it. No scheduling
implementation was present.

Command, verbatim:

```console
cargo nextest run -p gf2-sim -E 'test(work_item_enumeration)'
```

The command first stopped before compilation because the sandbox could not
resolve crates.io. The captured network failure was:

```text
error: failed to get `image` as a dependency of package `gf2-core v0.1.0 (/tmp/gf2-41c3d91d-red-8jCazb/crates/gf2-core)`

Caused by:
  failed to load source for dependency `image`

Caused by:
  unable to update registry `crates-io`

Caused by:
  download of config.json failed

Caused by:
  [6] Could not resolve host: index.crates.io
```

The same focused selector with the cached registry produced the compiler
failure below:

```console
CARGO_NET_OFFLINE=true cargo nextest run -p gf2-sim -E 'test(work_item_enumeration)'
```

```text
error[E0425]: cannot find function `enumerate_work_items` in this scope
  --> crates/gf2-sim/src/permanent_campaign/schedule.rs:67:19
   |
67 |         let all = enumerate_work_items(&manifest, None).unwrap();
   |                   ^^^^^^^^^^^^^^^^^^^^ not found in this scope

error[E0425]: cannot find type `WorkItem` in this scope
  --> crates/gf2-sim/src/permanent_campaign/schedule.rs:69:28
   |
69 |             all.iter().map(WorkItem::key).collect::<Vec<_>>(),
   |                            ^^^^^^^^ not found in this scope

error[E0425]: cannot find function `run_field` in this scope
  --> crates/gf2-sim/src/permanent_campaign/schedule.rs:80:19
   |
80 |         let run = run_field(&campaign, 3).unwrap();
   |                   ^^^^^^^^^ not found in this scope

error[E0425]: cannot find function `emit_field` in this scope
   --> crates/gf2-sim/src/permanent_campaign/schedule.rs:111:24
    |
111 |             let q3_paths = emit_field(&root, &campaign, &q3).unwrap();
    |                            ^^^^^^^^^^ not found in this scope

error: could not compile `gf2-sim` (lib test) due to 41 previous errors; 1 warning emitted
```

## 2026-08-16 — emission-guard red test

Patch 1 added only
`crates/gf2-sim/tests/permanent_campaign_bin.rs`. The focused command was:

```console
cargo nextest run -p gf2-sim --release --profile ci -E 'test(binary_refuses_emission_before_writing_outside_repository)'
```

The test failed against the unguarded binary with this captured output:

```text
FAIL [   0.005s] (1/1) gf2-sim::permanent_campaign_bin binary_refuses_emission_before_writing_outside_repository

thread 'binary_refuses_emission_before_writing_outside_repository' panicked at crates/gf2-sim/tests/permanent_campaign_bin.rs:88:5:
binary must refuse emission; status ExitStatus(unix_wait_status(0))
stderr:
```

## 2026-08-16 — round-2 schedule and emission red tests

Patch 1 added only the three tests in
`crates/gf2-sim/src/permanent_campaign/schedule.rs`. The focused command was:

```console
CARGO_NET_OFFLINE=true cargo nextest run -p gf2-sim --release --profile ci -E 'test(permanent_floor_rejection_is_the_preregistered_exact_test) | test(summary_verdict_matches_the_pooled_permanent_floor_decision) | test(re_emitting_into_the_same_tree_refuses_and_preserves_the_first_emission)'
```

The tests failed before execution because the production decision helper had
not been added yet. This is the complete captured output:

```text
   Compiling gf2-sim v0.1.0 (/home/vkaskivuo/Projects/gf2/crates/gf2-sim)
error[E0425]: cannot find function `permanent_acceptance` in this scope
   --> crates/gf2-sim/src/permanent_campaign/schedule.rs:648:20
    |
648 |         assert_eq!(permanent_acceptance(3, 0, 11, 1), AcceptanceVerdict::Rejected);
    |                    ^^^^^^^^^^^^^^^^^^^^ not found in this scope

error[E0425]: cannot find function `permanent_acceptance` in this scope
   --> crates/gf2-sim/src/permanent_campaign/schedule.rs:649:20
    |
649 |         assert_eq!(permanent_acceptance(3, 4, 11, 1), AcceptanceVerdict::Accepted);
    |                    ^^^^^^^^^^^^^^^^^^^^ not found in this scope

error[E0425]: cannot find function `permanent_acceptance` in this scope
   --> crates/gf2-sim/src/permanent_campaign/schedule.rs:668:13
    |
668 |             permanent_acceptance(
    |             ^^^^^^^^^^^^^^^^^^^^ not found in this scope

For more information about this error, try `rustc --explain E0425`.
error: could not compile `gf2-sim` (lib test) due to 3 previous errors
warning: build failed, waiting for other jobs to finish...
error: command `/home/vkaskivuo/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo test --no-run --message-format json-render-diagnostics --package gf2-sim --release` exited with code 101
```

The Fix-B test was also replayed against a clean `HEAD` snapshot in
`/tmp/gf2-41c3d91d-redb-AObOtx`, with only that test added. The focused command
was:

```console
CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/gf2-41c3d91d-redb-target cargo nextest run -p gf2-sim --release --profile ci -E 'test(re_emitting_into_the_same_tree_refuses_and_preserves_the_first_emission)'
```

It reached the old overwriting writer and failed as required:

```text
FAIL [   0.004s] (1/1) gf2-sim permanent_campaign::schedule::tests::re_emitting_into_the_same_tree_refuses_and_preserves_the_first_emission
  stdout ───

    running 1 test
    test permanent_campaign::schedule::tests::re_emitting_into_the_same_tree_refuses_and_preserves_the_first_emission ... FAILED

    failures:

    failures:
        permanent_campaign::schedule::tests::re_emitting_into_the_same_tree_refuses_and_preserves_the_first_emission

    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 238 filtered out; finished in 0.00s

  stderr ───

    thread 'permanent_campaign::schedule::tests::re_emitting_into_the_same_tree_refuses_and_preserves_the_first_emission' (7882) panicked at crates/gf2-sim/src/permanent_campaign/schedule.rs:718:14:
    re-emitting into an existing dataset must refuse: ["/tmp/campaign-reemit-7881/campaign-test/shards/q3/n02/shard-000000.json", "/tmp/campaign-reemit-7881/campaign-test/summaries/q3.json"]

────────────
     Summary [   0.005s] 1 test run: 0 passed, 1 failed, 379 skipped
        FAIL [   0.004s] (1/1) gf2-sim permanent_campaign::schedule::tests::re_emitting_into_the_same_tree_refuses_and_preserves_the_first_emission
error: test run failed
```
