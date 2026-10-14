# Cause of the intermittent BCH allocation-witness failure

`gf2-coding::bch_matrix_allocation caller_buffer_materialization_reaches_no_allocator`
counted allocations in process-wide atomics. libtest's main thread allocates
its running-test bookkeeping right after it spawns the test thread; when the
main thread is descheduled at the spawn, those allocations land inside the
test's counting window. The failures observed under the shim count 2 or 4
allocations, and the same assertions hold in every run once only the test
thread is counted.

The witness counts through the thread-local counter of
`dev/tools/thread-allocations`, which every allocation test of `gf2-core` and
`gf2-coding` shares, and asserts that the measured path requests no
allocation, reallocation or deallocation.

## Evidence

[`reproduce.sh`](reproduce.sh) runs the test binary 200 times under
[`spawn-delay.c`](spawn-delay.c), an `LD_PRELOAD` shim that busy-waits in the
creating thread after `pthread_create` returns, at four delays, then 200 times
beside the full `gf2-coding` suite. Output of
`reproduce.sh <checkout> gf2-coding bch_matrix_allocation` on 2026-10-15,
fixed tree at `827f36387`, pre-fix tree its parent's test source:

```text
# fixed tree
bch_matrix_allocation shim 150us: 0 of 200 runs failed
bch_matrix_allocation shim 200us: 0 of 200 runs failed
bch_matrix_allocation shim 250us: 0 of 200 runs failed
bch_matrix_allocation shim 300us: 0 of 200 runs failed
bch_matrix_allocation beside the gf2-coding suite: 0 of 200 runs failed
# pre-fix tree
bch_matrix_allocation shim 150us: 20 of 200 runs failed
bch_matrix_allocation shim 200us: 40 of 200 runs failed
bch_matrix_allocation shim 250us: 5 of 200 runs failed
bch_matrix_allocation shim 300us: 0 of 200 runs failed
bch_matrix_allocation beside the gf2-coding suite: 0 of 200 runs failed
```
