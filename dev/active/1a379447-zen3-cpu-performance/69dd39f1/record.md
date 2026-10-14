# Delayed-spawn runs of the gf2-core allocation witnesses

`gf256_gemm_allocation` and `gf256_axpy_allocation` count through the
thread-local allocator of `dev/tools/thread-allocations`, shared with the
`gf2-coding` allocation tests. The delayed-spawn shim and runner are those of
`@/issue/ec4e71ca` (`../ec4e71ca/reproduce.sh`, `../ec4e71ca/spawn-delay.c`).

## Evidence

Output of, for each of the two test binaries on 2026-10-15,

```text
dev/active/1a379447-zen3-cpu-performance/ec4e71ca/reproduce.sh . gf2-core <test-binary> 200 150 180 200 250
```

fixed tree at `6805b31fa`, pre-fix tree its parent `d9c6c753b` (process-wide
atomic counters). A binary of two tests counts a run as failed when either
test fails.

```text
# fixed tree
gf256_gemm_allocation shim 150us: 0 of 200 runs failed
gf256_gemm_allocation shim 180us: 0 of 200 runs failed
gf256_gemm_allocation shim 200us: 0 of 200 runs failed
gf256_gemm_allocation shim 250us: 0 of 200 runs failed
gf256_gemm_allocation beside the gf2-core suite: 0 of 200 runs failed
gf256_axpy_allocation shim 150us: 0 of 200 runs failed
gf256_axpy_allocation shim 180us: 0 of 200 runs failed
gf256_axpy_allocation shim 200us: 0 of 200 runs failed
gf256_axpy_allocation shim 250us: 0 of 200 runs failed
gf256_axpy_allocation beside the gf2-core suite: 0 of 200 runs failed
# pre-fix tree
gf256_gemm_allocation shim 150us: 0 of 200 runs failed
gf256_gemm_allocation shim 180us: 1 of 200 runs failed
gf256_gemm_allocation shim 200us: 21 of 200 runs failed
gf256_gemm_allocation shim 250us: 122 of 200 runs failed
gf256_gemm_allocation beside the gf2-core suite: 0 of 200 runs failed
gf256_axpy_allocation shim 150us: 0 of 200 runs failed
gf256_axpy_allocation shim 180us: 9 of 200 runs failed
gf256_axpy_allocation shim 200us: 3 of 200 runs failed
gf256_axpy_allocation shim 250us: 0 of 200 runs failed
gf256_axpy_allocation beside the gf2-core suite: 0 of 200 runs failed
```
