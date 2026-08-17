# Draft manifest validation

Status: draft validation record.

The temporary reader test invokes the public `gf2_sim::permanent_campaign::schema::read_manifest` function against `manifest-draft/manifest.json` and checks the 63-cell count, the three resolved backends, the 60 `generic_ryser` placeholders, and `Evaluate` on every cell.

```text
flock -o "${XDG_RUNTIME_DIR:-/tmp}/cargo-ci.lock" cargo test -p gf2-sim --test phase1_manifest_reader --release
```

Outcome: exit 0; `1 passed, 0 failed`. The test parses the manifest through the real reader and confirms 63 cells, exactly three dischargeable cells—(3,28), (5,24), and (7,20)—and 60 placeholders. It does not run a campaign arm, draw matrices, create shards, or run a measurement.

The temporary test source is absent from the draft artifact set. The draft remains schema-valid only; the 60 placeholder backend rows still block final freeze under the protocol.
