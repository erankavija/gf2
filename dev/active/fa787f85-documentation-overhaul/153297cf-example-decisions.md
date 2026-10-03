# 153297cf accessor-shaped example findings: decisions

Policy: `@/inv/teaching-rustdoc-examples`, which reads "A rustdoc example
teaches a workflow or clarifies a material contract that prose and focused
tests leave unclear; examples for accessors, constants, constructors,
predicates, and direct field mappings are a defect." Findings are the seven
`aabc528a` items deferred from the 2026-06-30 assessment
(`dev/active/b4b4b9ee-tech-debt-2026-06-30/b4b4b9ee-assessment-report.md`,
doc-completeness table). Each is judged against the tree at `92fa3e228`.

Every finding closes as superseded and no example is added. The `with_scale`
and `with_algorithm` docs carry a `# Panics` section stating the contract their
`DecoderConfig::new` call imposes.

| Finding | Items | Decision | Reason under the policy |
|---|---|---|---|
| `gf2-kernels-hip` getters | Each `pub fn …(&self)` in `crates/gf2-kernels-hip/src/lib.rs` and `launch_*.rs` that returns a stored field or, for `LdpcGraphLayout::edges`, the length of a stored array | Superseded | Accessors. Each one-line doc states the value; none carries a contract beyond it. |
| `gf2-sim` DVB-T2 builder | `Builder::{decoder, demap, channel, parallelism, seed, checkpoint_dir}` | Superseded | Each stores one argument into one field, a direct field mapping. The typestate workflow these setters form is taught once by the `presets::dvb_t2` module example and the `Pipeline::dvb_t2` example. |
| `gf2-sim` stage constructors | `DvbT2Encode::new`, `BitInterleave::new`, `BitDeinterleave::new`, `DvbT2Decode::new` | Superseded | Constructors that wrap one `Arc`. Each type's struct-level example already shows construction followed by `Stage::process`, the workflow. |
| `Connector::new` | `crates/gf2-sim/src/connector.rs` | Superseded | Constructor mapping two arguments onto two public fields; the struct-level example shows the call. |
| `SnrPointResult::from_counters` | `crates/gf2-sim/src/executor/results.rs` | Superseded | Constructor projecting `WorkerCounters` fields. The struct-level example is the counter-sequence example the finding requests. |
| `WorkerCounters::fer`, `mean_iters` | `crates/gf2-sim/src/parallel/mod.rs` | Superseded | Derived accessors. The one non-obvious contract, `0.0` when no frames ran, is in each doc line and asserted by `test_fer_and_mean_iters_ratios` in the same file. |
| `Nr5gRateMatchedDecoder::with_scale`, `with_algorithm` | `crates/gf2-coding/src/ldpc/nr_5g/mod.rs` | Superseded | Constructors. The `with_scale` doc states that `scale = 1.0` selects plain min-sum; normalized min-sum multiplies each check message by `scale`, so no example can observe the mapping. |

## Excluded crate

`gf2-kernels-hip` is outside the default Cargo workspace, so workspace
documentation-test runs do not compile its examples. It is held to the same
policy, and no example is added there.

## Derivation

The `gf2-kernels-hip` item set comes from:

```sh
grep -nE 'pub fn [a-z_0-9]+\(&self\)' crates/gf2-kernels-hip/src/lib.rs crates/gf2-kernels-hip/src/launch_*.rs
```

Of its matches, `HipError::code`, `HipError::is_recoverable` and each
`new_stream_scratch` compute or allocate rather than return a stored value, so
they fall outside the finding.
