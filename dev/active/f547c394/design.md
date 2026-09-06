# Zen 3 benchmark protocol tooling: design and canonical homes

This note records where the deliverables of issue `f547c394` live, why each
home is the canonical one under `@/inv/convention-convergence`, and the named
exception it leaves. The normative rules are in [protocol.md](protocol.md);
the schema is [`addendum.schema.json`](addendum.schema.json).

## Canonical homes

| Deliverable | Home | Reason |
|---|---|---|
| Shared runner primitives: append-only execution log, immutable checkpoints, fresh-child framing, fixed-window timing | `tuning-campaign-support` crate, existing `journal`, `transport` and `timing` modules | The a835 protocol made this workspace member the crate-neutral development support library; the benchmark runner consumes those modules unchanged rather than duplicating them. |
| Host observation and lock evidence (`host`), child execution with concurrent draining and process-tree reaping (`process`) | New library modules lifted out of the a835 driver binary | Two consumers (the a835 driver and the benchmark runner) need the same mechanics; a private copy in a second binary would be the parallel variant the invariant forbids. The driver keeps thin wrappers so its behavior and journal output are byte-identical. |
| Paired A/B statistics (`abtest`) | New library module beside `statistics` | `statistics` implements the a835 extent argmin rule over fixed strata; the protocol's paired resampling, bootstrap interval and equivalence decisions are a distinct estimator family with its own deterministic generator, so they sit beside it rather than inside it. |
| Protocol identity, frozen settings, addendum and plan contracts (`protocol`), receipt schema and acceptance evaluation (`receipt`), JSON Schema subset validator (`schema`) | New library modules | One typed source for the receipt and addendum contracts serves the runner, the acceptance tool and the tests (`@/inv/semantic-types`); the committed JSON Schema is the declared machine-readable contract and is enforced at acceptance time by the subset validator, so the two cannot drift silently. |
| Acceptance tool (`benchmark-acceptance`) | Rust binary in the crate rather than a generalization of `dev/scripts/validate-tuning-extent-campaign.py` | The Python validator is deliberately specific to the a835 campaign: it hard-codes cell counts, grids, defaults and owner protocols. It keeps its convention of trusting no producer counter, which the Rust tool follows by recomputing every digest, interval and decision from raw samples. A Rust home lets the deterministic fixtures run in the fast test tier through the existing crate steps of `scripts/cargo-ci.sh`. |
| Protocol runner (`benchmark-ab-runner`) and the smoke arm (`ab-smoke-workload`) | Thin binaries over the library primitives | Binaries stay consumers: the runner owns only plan parsing, cell iteration and receipt assembly; every durable-evidence rule comes from `journal`, `process`, `host`, `abtest` and `receipt`. |
| Protocol, schema, addenda | `dev/active/f547c394/` | The owning development directory named by `jit doc dir`; receipts pin these files by path, commit and digest. |
| Smoke receipt | `dev/bench_results/f547c394/` | The owning issue's receipt area named by the measurement contract. |

## Named exception: session lifecycle store

`campaign::SessionStore` implements the three-mode prepare/run/finalize
lifecycle with durable mode claims and release proof. Its descriptor validation
pins the a835 feature contract, thread contract and declared unit counts, so a
second protocol cannot reuse it without changing those constants into
parameters. The benchmark runner therefore uses `journal::ExecutionLog`,
`journal::CheckpointStore`, `campaign::LockEvidence` and `host::inherited_lock`
directly and journals lock evidence itself. Convergence condition: when a
second consumer needs the full lifecycle store, parameterize
`SessionDescriptor::validate` over its contracts and counts and move the runner
onto it. Until then the runner's lock, log and checkpoint discipline is the
shared one; only the mode-claim bookkeeping is absent.

## Decisions worth knowing

- **Resampling unit.** The paired execution of two fresh children is the unit
  because it is what the runner can randomize and what removes carried-over
  process state; windows inside an execution are summarized by their median
  and retained raw.
- **Bonferroni over Holm.** Confidence-interval decisions need a fixed
  per-comparison level; Holm's step-down needs ordered p-values. Bonferroni
  costs some power at family sizes below ten and buys validity under arbitrary
  dependence and a rule a reviewer can recompute from the addendum alone.
- **Fixed confirmatory sample size.** Precision-based stopping would make
  confirmation data-adaptive; a fixed 24 pairs keeps every confirmatory trial
  comparable and the search bounded by the family ledger instead.
- **Float round-tripping.** Journal records, checkpoint units and receipts are
  compared byte for byte after re-encoding, so the crate enables
  `serde_json/float_roundtrip`; without it a per-execution median such as
  `1035.4630584192441` parsed back to a neighbouring double and every resume
  failed as a non-canonical record.
- **Core arm resolution.** The runner sets its own affinity to the resolved
  CPU set before spawning an arm and restores it afterwards, so arms inherit
  the set without an unsafe pre-exec hook; every arm reports the mask it
  observed and the acceptance tool compares it with the resolved set.
- **Smoke family.** The synthetic XOR-fold arms differ only through the
  `GF2_SMOKE_PASSES` environment of the baseline arm; the frozen smoke
  addendum's margins are pipeline checks, and its receipt is labelled `smoke`
  so nothing in it can be cited as a performance result.

## Pre-existing fragility observed

The a835 producing-input manifest lists
`dev/tools/tuning-profile-compose/Cargo.lock` as a build input, but that file
is untracked and only appears after the composer builds. In a fresh worktree
the driver unit test `producing_input_manifest_rejects_authority_and_path_mutations`
fails until `scripts/cargo-ci.sh` reaches its `tuning-composer` step, which
runs after the crate's own test step. This is an `a83583e0` artifact and is
left for its owner; it is reported rather than patched here.
