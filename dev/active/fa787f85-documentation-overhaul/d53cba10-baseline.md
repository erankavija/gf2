# d53cba10 addendum schema test baseline

Before (commit `ff34a202b`) and after (commit `6a1ecf0ed`) results are
identical. Command, run from the repository root at each commit:

```sh
./scripts/cargo-budget.sh --test cargo nextest run -p tuning-campaign-support \
  --cargo-profile ci-test --profile ci -E 'binary(addendum_schema_versions)'
```

Result: `each_protocol_version_validates_against_its_own_committed_schema`
passed. The test asserts, for every pair, that the schema's `$id` equals the
addendum's `schema` field and that validation reports no violation, so the pass
gives these per-fixture results (paths as of `ff34a202b`; at `6a1ecf0ed` the
test locates the same files by SHA-256):

| Schema | Addendum | `$id` match | Violations |
|---|---|---|---|
| `dev/active/f547c394/addendum-v1-initial.schema.json` | `dev/active/f547c394/addendum-protocol-smoke-pilot.json` | yes | 0 |
| `dev/active/f547c394/addendum-v1.schema.json` | `dev/active/26465e6c/superseded/v1/addendum-popcount.json` | yes | 0 |
| `dev/active/f547c394/addendum-v2.schema.json` | `dev/active/f547c394/addendum-smoke-v2-pilot.json` | yes | 0 |
| `dev/active/f547c394/addendum-v3.schema.json` | `dev/active/eda07788/addendum-dvb-t2-bit-interleave-v3-confirmation.json` | yes | 0 |
| `dev/active/f547c394/addendum.schema.json` | `dev/active/5cbb6545/addendum-popcount-v4-confirmation.json` | yes | 0 |
