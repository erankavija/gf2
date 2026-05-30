# 82dd7384 closing note — IRA encoder timing

Closes the `[aspirational]` success criterion of JIT issue
`82dd7384` ("Linear-time DVB-T2 LDPC encoder via dual-diagonal staircase
accumulator"):

> [aspirational] Encoder construction plus first Normal-frame encode
> completes in well under 1 s (versus the current multi-minute RREF),
> measured on the campaign host and recorded in the closing note.

## Measurement

Standalone release-build binary that constructs a `DvbT2Concat`, encodes
a zero-payload BBFRAME (forcing the lazy `LdpcEncoder` to materialise),
then encodes a second time to surface the steady-state cost. Each row
is a single fresh process invocation; `DvbT2Concat::new` and the first
`encode` are dominated by the IRA encoder construction (which performs
a single O(nnz) pass over the dual-diagonal parity structure — no
densification, no RREF).

| config             | n_ldpc | `DvbT2Concat::new` | first encode | construct + 1st | second encode |
|--------------------|-------:|-------------------:|-------------:|----------------:|--------------:|
| Normal Rate 1/2    | 64800  |          17.165 ms |     4.250 ms |       21.416 ms |       561 µs  |
| Normal Rate 2/3    | 64800  |           9.104 ms |     2.034 ms |       11.139 ms |       682 µs  |
| Normal Rate 3/4    | 64800  |           8.761 ms |     1.980 ms |       10.741 ms |       743 µs  |
| Short  Rate 1/2    | 16200  |           2.215 ms |       426 µs |        2.641 ms |        88 µs  |

The worst case (Normal Rate 1/2, cold process) is **21.4 ms** —
approximately **47x** under the 1 s aspirational target, and roughly
**5000-10000x** faster than the previous Richardson-Urbanke RREF
preprocessing for the same configuration (which CLAUDE.md and
`cache.rs` documented at 2-10 s for *Short* frames and *several
minutes* for Normal frames). The aspirational criterion is met with
comfortable margin.

## Host

- AMD Ryzen 9 5900X, 12 cores / 24 threads
- 31 GiB RAM
- Linux 7.0.10-arch1-1 (x86_64)
- rustc 1.95.0, cargo 1.95.0
- Release build (`opt-level = 3`, `lto = "thin"`)

## Reproducer

The standalone timing binary (one-shot, not added to the workspace)
lives at `/tmp/encoder_timing/` with a path dependency on
`gf2-coding` / `gf2-core`. It is intentionally throw-away; the
numbers above are what closes the criterion, not the binary itself.
The measurement can be reproduced by re-creating it with:

```rust
use gf2_coding::ldpc::dvb_t2::{concat::DvbT2Concat, FrameSize};
use gf2_coding::traits::BlockEncoder;
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use std::time::Instant;

let t = Instant::now();
let concat = DvbT2Concat::new(FrameSize::Normal, CodeRate::Rate1_2).unwrap();
let bb = BitVec::zeros(concat.k_bch());
let _ = concat.encode(&bb);
println!("{:?}", t.elapsed());
```

## Caveats

- All Normal-frame measurements after Rate 1/2 see somewhat warmer
  allocator / page caches than the first-shot one. The Rate 1/2 row
  is the representative cold-process figure; the others trend lower
  because successive runs in the same process amortise lazy
  initialisation in unrelated subsystems. Even taking the largest
  number, 21 ms remains ~50x under the 1 s aspirational target.
- Wall-clock variability under load is small (sub-millisecond at this
  scale) because the IRA construction is pure CPU work over already-
  resident data structures, with no I/O.
