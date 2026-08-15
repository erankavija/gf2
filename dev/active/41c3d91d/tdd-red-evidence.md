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
