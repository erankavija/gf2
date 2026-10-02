#!/usr/bin/env bash
# Files the tracked issues for the bare implementation TODOs that 12907582
# replaced in code. Substitute each printed id for its `@/issue/NEWID-<n>`
# placeholder in the source comments.
set -euo pipefail

echo "NEWID-1:"
jit issue create "Parallelize CpuBackend matmul under the parallel feature" \
  --type task --orphan --label component:gf2-core --description-file - <<'EOF'
`CpuBackend::matmul` runs the serial `BitMatrix` product in every
configuration, while its batch matvec methods use rayon when the `parallel`
feature is enabled. The source comment in `crates/gf2-core/src/compute/cpu.rs`
cites this issue.

## Success Criteria

- [hard] REQ-01: With the `parallel` feature, `CpuBackend::matmul` distributes the product across rayon workers above a selection threshold and runs the serial product below it.
- [hard] REQ-02: The rayon and serial paths return identical matrices across one shared behavioral suite, including row and column counts 0, 1, 63, 64, and 65 and results independent of the worker count.
- [hard] REQ-03: The selection threshold cites a committed benchmark receipt from an uncontended host.
EOF

echo "NEWID-2:"
jit issue create "Add SIMD kernels for LLR batch saturation and hard decision" \
  --type task --orphan --label component:gf2-coding --description-file - <<'EOF'
`Llr::saturate_batch` and `Llr::hard_decision_batch` in
`crates/gf2-coding/src/llr.rs` are scalar loops, while other LLR batch
operations dispatch to `gf2-kernels-simd`. The source comments cite this issue.

## Success Criteria

- [hard] REQ-01: `Llr::saturate_batch` and `Llr::hard_decision_batch` dispatch to runtime-detected AVX2 kernels in `gf2-kernels-simd` and fall back to the scalar loops when AVX2 is unavailable.
- [hard] REQ-02: The SIMD and scalar paths return identical results across one shared behavioral suite, covering lengths 0, 1, and one below, at, and above the vector width, and the values ±0, ±infinity, and NaN.
- [hard] REQ-03: Any speedup claim cites a committed benchmark receipt from an uncontended host.
EOF
