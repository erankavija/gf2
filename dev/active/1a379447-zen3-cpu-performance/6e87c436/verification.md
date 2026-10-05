# Dense-parity no-change verification (jit:6e87c436)

> **Diátaxis Type:** Research

Every command runs from the worktree root with Rust 1.95 selected through
`RUSTUP_TOOLCHAIN=1.95` and with `CARGO_CI_NO_SCCACHE=1`. No command takes a
timing window, the host lock, or an ignored test.

## Production sources

`python3 -B survey/make-production-drift.py --anchor 2b09efee98a4b0592bd4badf263b84cb6139c2ba`
(path relative to this directory) writes the
[production drift record](survey/production-drift.json). Its `task_change`
object lists the `gf2-core` and `gf2-kernels-simd` paths this task changes
against its anchor; the script fails when one of them is a producing input of
the dense campaigns or lies under a package `src/` directory. The listed paths
are shared test suites only.

The same record compares the working tree with the production sources the four
dense receipts measured. The receipts record one common digest set for the two
packages, so the record holds one baseline. Each file carries the measured and
current SHA-256 and one class; the record's `classifier_rule` states the rule
that separates `comment-or-blank-only` from `code-differs`, and
`class_counts` totals each class. The current tree is not byte-identical to the
measured tree. Each `code-differs` file lists, under `changed_by`, the commits
that changed it after the newest commit holding the measured bytes, with the
issue each commit names. The differences on the dense `matvec` path:

- `BitVec::from_words` truncates surplus words and masks the tail
  (`crates/gf2-core/src/bitvec.rs`, jit:79873126).
- The baked `bit_backend.simd_min_words` constant and its profile owner change
  (`crates/gf2-core/src/tuning/baked.rs`, jit:dbd8787d and jit:a83583e0). The
  [source ledger](survey/source-evidence.json) pins the separate baked matvec
  threshold, which equals the conservative constant in both trees.
- `crates/gf2-core/src/matrix.rs` and
  `crates/gf2-kernels-simd/src/x86/avx2.rs` are `comment-or-blank-only`.

The remaining `code-differs` rows lie outside the dense `matvec` path and are
listed with their commits in the record.

## Route selection

`survey/make-route-comparison.py` joins each receipt's per-pair
`selected_path` values with an untimed arm smoke of the same campaign addendum
on the working tree and writes the
[route comparison](survey/route-comparison.json). Each smoke record under
[`survey/route-smoke/`](survey/route-smoke/dense-allocated-matvec.json) is the
output of the story launcher's smoke mode, which builds the arms from the
current tree in release mode and drives every cell through the shared runner's
validation position with no timing window:

```sh
S=dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations
D=dev/active/1a379447-zen3-cpu-performance/6e87c436/survey/route-smoke
for f in dense-isolated-fused-parity dense-allocated-matvec; do
  $S/survey/run-dense-harness.sh window --family 2037941f-$f \
    --addendum $S/campaigns/$f.json --run-id 6e87c436-route --smoke $D/$f.json
done
$S/survey/run-dense-harness.sh window --family 2037941f-dense-matvec-vs-m4ri \
  --addendum $S/campaigns/dense-matvec-vs-m4ri.json --run-id 6e87c436-route \
  --m4ri --smoke $D/dense-matvec-vs-m4ri.json
$S/survey/run-dense-harness.sh window --family 2037941f-dense-matvec-vs-m4ri \
  --addendum $S/campaigns/dense-matvec-vs-m4ri-confirmation.json \
  --run-id 6e87c436-route --m4ri --confirmation \
  --smoke $D/dense-matvec-vs-m4ri-confirmation.json
python3 -B dev/active/1a379447-zen3-cpu-performance/6e87c436/survey/make-route-comparison.py \
  v4-r1-2037941f-dense-isolated-fused-parity=$D/dense-isolated-fused-parity.json \
  v4-r1-2037941f-dense-allocated-matvec=$D/dense-allocated-matvec.json \
  v4-r1-2037941f-dense-matvec-vs-m4ri=$D/dense-matvec-vs-m4ri.json \
  v4-r1-confirmation-2037941f-dense-matvec-vs-m4ri=$D/dense-matvec-vs-m4ri-confirmation.json
```

The launcher first needs its campaign tool, built by
`CARGO_TARGET_DIR=$PWD/target/e1f9a78f-arms ./scripts/cargo-budget.sh cargo build --release --manifest-path $S/survey/dense-harness/Cargo.toml --bins`.

The comparison script refuses a smoke whose addendum digest differs from the
receipt's and exits nonzero when any cell arm's current path differs from the
one path its measured pairs report. It exits 0: every row of every campaign
carries `selection: same`, and each campaign's `differing_cell_arms` field
records the count. The rows cover the SIMD build, the scalar-reference build
without the `simd` feature, and the M4RI peer arm, which are the build
configurations the dense receipts use. Each row also records the measured and
current arm executable digests; they differ, as the source drift implies.

The receipts measure no `gf2_tuning_baked` build. For that configuration the
[source ledger](survey/source-evidence.json) pins the baked matvec threshold
and the selector line that reads it, and the existing baked route witness
passes on the current tree (last row of the table below).

## Semantic suites

| Command | Result |
|---|---|
| `nice -n 19 ./scripts/cargo-budget.sh --test cargo nextest run -p gf2-core --all-features --test simd_equiv_matvec --test popcount_routes --test matrix_vector --cargo-profile ci-test --profile ci` | Exit 0; [log](survey/test-logs/simd-build.txt). |
| `nice -n 19 ./scripts/cargo-budget.sh --test cargo nextest run -p gf2-core --test simd_equiv_matvec --test popcount_routes --test matrix_vector --cargo-profile ci-test --profile ci` | Exit 0; [log](survey/test-logs/scalar-build.txt). |
| `nice -n 19 ./scripts/cargo-budget.sh --test cargo nextest run -p gf2-core --all-features --lib --cargo-profile ci-test --profile ci -E 'test(test_from_words_) \| test(test_boundary_) \| test(test_mask_tail_invariant)'` | Exit 0; [log](survey/test-logs/packed-bits.txt). |
| `RUSTFLAGS="--cfg gf2_tuning_baked" nice -n 19 ./scripts/cargo-budget.sh --test cargo nextest run -p gf2-core --features simd,test-support,tuning-profile --test matrix_selection_baked --cargo-profile ci-test --profile ci` | Exit 0; [log](survey/test-logs/baked-route.txt). |

Each log opens with the toolchain and command and closes with the exit status;
its nextest `Summary` line carries the test counts. The first two rows run the
same three suites with and without the `simd` feature, so the SIMD lane and
the scalar lane answer the same assertions. The suites compare the public
product with a bit-level reference built from `get` and `set`, which fixes
canonical indexing. The [source ledger](survey/source-evidence.json) pins the
test entry points and their case lists:

| Named case | Shared suite coverage |
|---|---|
| Empty | Zero rows and zero columns in the boundary-shape test; the zero-length fused routes. |
| Aligned | Every fused route on whole buffers of the boundary and block lengths; the eight-word stride shapes. |
| Offset | Every fused route on slices at each word offset of both operands; the nine-word stride shapes, whose rows start at successive word offsets. |
| 0/1/63/64/65 boundaries | Row and column counts of the boundary-shape test; the square word-boundary test. |
| Canonical indexing | Each output bit equals the parity computed through `get` at the canonical indices. |
| Zero tails | Each product's padding is asserted zero; a dirty-tailed, over-long `from_words` operand gives the all-ones product; the packed-bit tests assert `from_words` and tail masking directly. |

This task adds the boundary-shape test to `simd_equiv_matvec.rs` and the
word-offset test to `popcount_routes.rs`, because the existing boundary test
reaches the scalar lane only and the fused suite ran on whole buffers only.
The same change makes the automatic-route expectation
of `popcount_routes.rs` honour a build without the `simd` feature; that
expectation fails in the scalar build at the task anchor.

## Assembly gate and CI

`./scripts/asm-artefact-present.sh` exits 0 and reports that no SIMD source
file changed. `CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-ci.sh` exits 0 from the
worktree root with every step reported `ok`.
