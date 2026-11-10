# AFF3CT LDPC comparisons

The harness for @/issue/55c3e433 compares forced single-frame gf2, independently
pinned pre-change gf2, tuned gf2 and AFF3CT 4.7.0 flooding f32 NMS. Workers
reuse decoders and repeat the declared input prefix. gf2 reports direct
per-frame iteration counts after timing. AFF3CT replays that prefix on its
counting prototype, checks equality with the timed clone's decisions and
reports replay-derived counts.

The arms use the canonical request, result, window and schema definitions from
`tuning-campaign-support`. Steady-state cells require warm cache state. Requests
in the validation position run one untimed dispatch and return no timing windows.

Set `GF2_LDPC_INPUT_ROOT` to an absolute artifact directory containing `bundles/`
and `quality/`. Bundles use the names in [arms.json](arms.json); each contains
a manifest, an AList, codewords and little-endian f32 LLRs. Quality filenames
combine the catalogue's quality name and code key. The harness verifies bundle
digests and compares decoded frame errors with prepared quality. The frozen
input identities are configuration in [families/](families/); input bytes and
prepared quality stay outside the tracked tree.

Build records, materialized plans, campaign stages, ledgers, summaries and
freeze derivations live under the repository's `target/` directory. Python
tools resolve the repository with Git from their installed location; the
working directory does not affect the default output. Each build record names
its own source worktree and executable digest. Build an unchanged arm in its
unchanged worktree, each patched arm in its separately patched worktree, and
the candidate in its candidate worktree. The two scalar patches are in
[patches/](patches/). A build record never substitutes another executable when
its pinned bytes are absent.

From a worktree root, build and record an arm without measuring:

```sh
export CARGO_NET_OFFLINE=true
python3 experiments/aff3ct-ldpc-gaps/record-gf2-arms-build.py --arm gf2-batch-tuned
```

The AFF3CT adapter is an independent nested workspace. Set `GF2_AFF3CT_ROOT`
to the AFF3CT source/build directory containing its 4.7.0 static library, then
use the same recorder with `--arm aff3ct-flooding-nms-f32-iterations`. Ordinary
workspace builds and tests need no AFF3CT installation.

The adapter's second executable, `aff3ct-fast-bench`, times one AFF3CT NMS
decoder outside the campaign lifecycle: scalar or inter-frame SIMD, flooding,
horizontal-layered or the layered ONMS decoder, in f32 or i16. It takes a
bundle directory, the code name, the decoder type, `none` or `INTER`, the
precision, and the worker, batch and call counts, decodes the whole bundle
untimed for frame errors, and prints one JSON line with the median call time.
The shim compiles as C++17, which AFF3CT's SIMD update rules require.
`gf2-fast-bench` takes the same arguments for a gf2 route: `single`, `lanes`,
`qc` (`QcDecoder` one frame at a time), `default` (`LdpcBatchDecoder` as
shipped) or `layered` (`LayeredDecoder` in i8 under the corpus code's
`LayeredConfig`, cap 25), with SIMD `none` or `avx2` (`none` for `single`,
`auto` for `default`) and input precision `f32`; its JSON line adds the
iteration cap. It attaches the corpus block description after
an exact parity-check comparison. Both executables share the argument parsing,
pinned call loop and JSON line of `src/fast.rs`.
[quick-compare.sh](quick-compare.sh) builds both, runs every gf2 route and
AFF3CT configuration whose wave divides the batch over both bundles and a fixed
worker and batch grid inside a benchmark window, and writes one table of frames
per second, information throughput, frame errors and the best-gf2 to
best-AFF3CT ratio under `target/`; its header names the environment it reads.

Generate portable configuration from a declaration:

```sh
python3 experiments/aff3ct-ldpc-gaps/make-family.py \
  experiments/aff3ct-ldpc-gaps/families/55c3e433-check-variants.json \
  target/aff3ct-ldpc-gaps/configuration --configuration
```

[plans/](plans/) contains the generated pilot configurations. Their symbolic
input-root and build references require materialization before dispatch. The NR
pilot retains its QC-prototype comparisons as configuration; this harness
refuses to dispatch that prototype. The tuned arm, `gf2-batch-tuned`, embeds
the committed coding envelope. The candidate arm, `gf2-batch-qc-candidate`,
embeds
[tuning-profiles/qc-route-candidate.json](tuning-profiles/qc-route-candidate.json),
the same selectors under an inherited measurement. Both arms attach NR BG1
Z=384 structure after an exact parity-check comparison outside timing, refuse
a cell where a worker decodes off the route `decide_batch_route` gives it
under the installed selectors, and state each worker's executed route in
`selected_path` and their diagnostics.

`qc-route-sweep.sh` is the benchmark-window queue line of @/issue/c62356d2.
It builds `qc-route-sweep` and runs it under
`dev/scripts/ccx1-bench-flock.sh --full-host`, writing its execution log to
`target/aff3ct-ldpc-gaps/qc-route-sweep/`. For each code, worker count and
frames per call of `SweepConfig::issue_grid` in
[src/route_sweep.rs](src/route_sweep.rs), the QC, lane and single-frame routes
decode the same frames in rotating order, and each completed cell records
every repetition's time and each route's median time ratio to the QC route in
the same repetition. A rerun resumes after the last completed cell and refuses
a log of another run identity or configuration.

For a runnable plan, omit `--configuration` and supply one `--build` argument
per build record, including records in other worktrees. Generation checks the
input manifest identities, prepared-quality files and executable pins. A
confirmation declaration uses the same family format, adds a `confirmation`
campaign, and retains the pilot's input contract and independently scoped
ledger. The adoption and comparator contracts are in the epic's
[plan](../../dev/active/55c3e433-aff3ct-ldpc-gaps/plan.md).

`smoke-family.py PLAN BUILD_RECORD...` runs the materialized plan through the
canonical staged untimed lifecycle. `run-family-campaign.py` uses the same
arguments for bounded timed sessions and refuses them outside the benchmark
window. Both resume interrupted work. The shared Rust runner owns checkpoint,
execution-log and finalization validation; its acceptance command writes the
summary tables.

`freeze-confirmation.py --help` lists the freeze arguments. Supply the pilot
addendum, accepted pilot directory, fixed freeze timestamp, output addendum and
derivation path under `target/`. It derives resolution from every pilot cell,
including cells omitted from confirmation, and refuses invalid margins.
Generate the confirmation plan in that output directory with `--confirmation`;
the generator preserves the frozen addendum.

`closing-table.py` projects preserved comparator receipts, their acceptance
summaries and prepared quality records into a Markdown table under `target/`.
It keeps historical pilots separate, preserves candidate-oriented evaluator
labels and reports losing-cell closures without claiming confirmation. The
closing record for @/issue/5f401907 gives the reproduction command and
interprets the gf2-time/AFF3CT-time ratios.

The CI wrapper includes the gf2 package through the experiments workspace glob
and runs the Python suite through its `aff3ct-ldpc-gaps-python` step.
