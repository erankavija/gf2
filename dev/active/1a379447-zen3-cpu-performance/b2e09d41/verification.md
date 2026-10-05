# Plan validation by the shared runner check (jit:b2e09d41)

> **Diátaxis Type:** Reference

`benchmark-ab-runner check` is the one plan check of the campaign tooling. It
decodes a plan and the addendum the plan names, applies the addendum's and the
plan's semantic validation, and validates the addendum against the live
addendum schema declaring the schema identity the addendum names. The harness
of the LDPC throughput survey declares no plan-check binary; its binary targets
are the ones its `Cargo.toml` lists.

## Records

Every command runs from the repository root.

| Record | Generator | Content |
|---|---|---|
| [plan-checks.tsv](plan-checks.tsv) | `plan-checks.py` | The shared check's verdict for every committed plan, beside the reference verdicts for the same plan bytes |
| [mutation-checks.tsv](mutation-checks.tsv) | `plan-checks.py` | The shared check's verdict for each single-field addendum mutation on which the references disagree |
| [reference-observations.json](reference-observations.json) | `plan-checks.py --superseded ... --runner-edition ...` | What each reference executable reported, with the SHA-256 of the executable and of its source file |
| `build-identity-supersession.json` beside each affected build identity record | `build-identity-supersession.py` | The executables the record names for which no live Cargo package declares a binary target |
| The four LDPC producing-input manifests | `regenerate-manifests.py` | The closure each manifest's own generator computes over the tree |

```sh
CARGO_CI_NO_SCCACHE=1 python3 -B dev/active/1a379447-zen3-cpu-performance/b2e09d41/plan-checks.py
python3 -B dev/active/1a379447-zen3-cpu-performance/b2e09d41/build-identity-supersession.py
CARGO_CI_NO_SCCACHE=1 python3 -B dev/active/1a379447-zen3-cpu-performance/b2e09d41/regenerate-manifests.py
git diff --exit-code
```

`reference-observations.json` is an observation record. A reference executable
is identified by digest and is an argument of the run that observed it, so a
run that names no reference reads the record and leaves its bytes. The
`ldpc-plan-check` reference is the superseded checker; its source digest is the
`source_sha256` the record holds.

## What the records show

- `plan-checks.py` exits zero only when the shared check accepts every plan a
  superseded checker accepts. The rows whose `ldpc-plan-check` column is
  `accept` carry `accept` in `shared_check`.
- The superseded checker validates every addendum against the current schema
  edition, so it rejects a plan whose addendum names an earlier edition; the
  shared check validates such an addendum against the edition it names.
- Every row of `mutation-checks.tsv` is a mutation the superseded checker
  rejects and the runner without schema validation accepts; `shared_check` is
  `reject` on each. `runner_check_rejects_an_addendum_its_schema_edition_rejects`
  in the campaign support crate's protocol contract tests holds the same
  mutations on a fixture addendum.
- A plan whose addendum names schema `zen3-benchmark-addendum-v1` is rejected
  with the message in `shared_check_message`: two live schema documents with
  different bytes declare that identity, and the lookup `run` uses for the same
  schema requires one. The runner without schema validation accepts those
  plans in `check` because it does not look the schema up there.
- The rows rejected by every column are plans whose addendum is absent at the
  path the plan names or fails the addendum's semantic validation.

## Evidence left unchanged

Receipts, receipt input snapshots, ledgers, frozen addenda and the build
identity records keep their bytes. `python3 dev/scripts/check-receipt-input-snapshots.py`
reads the receipts' pins against committed content.
