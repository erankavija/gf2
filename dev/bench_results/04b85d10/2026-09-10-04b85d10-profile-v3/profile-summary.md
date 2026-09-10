# Current-code consumer profile sweep

Projection of `profile.jsonl`, one row per measured route. The run recorded 180 rows.

`ns/call` is the wall-clock mean over the calls the row reports, a single-run figure with no interval: the sweep locates the expensive routes and the protocol receipts carry the intervals.

## Logical row, parity and dense-matrix consumers

| Workload | Size | Route | Observed path | Calls | ns/call | Allocations/call | Bytes/call |
|---|---|---|---|---:|---:|---:|---:|
| row-xor | rows=64 words=4 | ops-dispatched | ops-dispatched/scalar | 9523809 | 108.0 | 0.00 | 0 |
| row-xor | rows=64 words=4 | ops-resolved | ops-resolved/scalar | 9009009 | 102.8 | 0.00 | 0 |
| row-xor | rows=64 words=4 | scalar-backend | scalar-backend | 10101010 | 100.1 | 0.00 | 0 |
| row-xor | rows=64 words=4 | simd-backend | simd-backend/avx2 | 10989010 | 82.0 | 0.00 | 0 |
| row-xor | rows=64 words=8 | ops-dispatched | ops-dispatched/simd | 8547008 | 110.3 | 0.00 | 0 |
| row-xor | rows=64 words=8 | ops-resolved | ops-resolved/simd | 9803921 | 99.2 | 0.00 | 0 |
| row-xor | rows=64 words=8 | scalar-backend | scalar-backend | 5847953 | 155.0 | 0.00 | 0 |
| row-xor | rows=64 words=8 | simd-backend | simd-backend/avx2 | 7246376 | 108.2 | 0.00 | 0 |
| row-xor | rows=64 words=16 | ops-dispatched | ops-dispatched/simd | 7092198 | 127.1 | 0.00 | 0 |
| row-xor | rows=64 words=16 | ops-resolved | ops-resolved/simd | 7936507 | 118.4 | 0.00 | 0 |
| row-xor | rows=64 words=16 | scalar-backend | scalar-backend | 3521126 | 255.6 | 0.00 | 0 |
| row-xor | rows=64 words=16 | simd-backend | simd-backend/avx2 | 7092198 | 123.3 | 0.00 | 0 |
| row-xor | rows=64 words=64 | ops-dispatched | ops-dispatched/simd | 2645502 | 344.3 | 0.00 | 0 |
| row-xor | rows=64 words=64 | ops-resolved | ops-resolved/simd | 2849002 | 322.3 | 0.00 | 0 |
| row-xor | rows=64 words=64 | scalar-backend | scalar-backend | 1321003 | 679.4 | 0.00 | 0 |
| row-xor | rows=64 words=64 | simd-backend | simd-backend/avx2 | 3174603 | 318.5 | 0.00 | 0 |
| row-xor | rows=64 words=512 | ops-dispatched | ops-dispatched/simd | 475511 | 2113.7 | 0.00 | 0 |
| row-xor | rows=64 words=512 | ops-resolved | ops-resolved/simd | 424088 | 2122.3 | 0.00 | 0 |
| row-xor | rows=64 words=512 | scalar-backend | scalar-backend | 185082 | 4797.4 | 0.00 | 0 |
| row-xor | rows=64 words=512 | simd-backend | simd-backend/avx2 | 424088 | 2110.2 | 0.00 | 0 |
| row-xor | rows=64 words=8192 | ops-dispatched | ops-dispatched/simd | 27897 | 35335.8 | 0.00 | 0 |
| row-xor | rows=64 words=8192 | ops-resolved | ops-resolved/simd | 29495 | 36497.3 | 0.00 | 0 |
| row-xor | rows=64 words=8192 | scalar-backend | scalar-backend | 11016 | 75666.4 | 0.00 | 0 |
| row-xor | rows=64 words=8192 | simd-backend | simd-backend/avx2 | 27947 | 34595.9 | 0.00 | 0 |
| dense-rref | cols=256 rows=256 | current | current/blocked-m4ri | 15680 | 58488.3 | 137.00 | 36832 |
| dense-rref | cols=512 rows=512 | current | current/blocked-m4ri | 3680 | 242792.1 | 266.00 | 122784 |
| dense-rref | cols=1024 rows=1024 | current | current/blocked-m4ri | 1124 | 798061.3 | 267.00 | 2391008 |
| dense-rref | cols=2048 rows=2048 | current | current/blocked-m4ri | 278 | 3673229.0 | 524.00 | 9239520 |
| dense-matvec | cols=256 rows=256 | current | current/scalar-row-parity | 1152073 | 798.8 | 1.00 | 32 |
| dense-matvec | cols=1024 rows=1024 | current | current/simd-and-popcnt | 211461 | 4398.9 | 1.00 | 128 |
| dense-matvec | cols=4096 rows=1024 | current | current/simd-and-popcnt | 55598 | 15912.3 | 1.00 | 128 |
| dense-matvec | cols=4096 rows=4096 | current | current/simd-and-popcnt | 13616 | 69706.9 | 1.00 | 512 |
| ldpc-syndrome | n=16200 | current | current/csr-bit-at-a-time-matvec | 13856 | 54754.6 | 1.00 | 1128 |
| ldpc-syndrome | n=64800 | current | current/csr-bit-at-a-time-matvec | 3556 | 269132.1 | 1.00 | 4056 |

## Count and fused-reduction consumers

| Workload | Size | Route | Observed path | Calls | ns/call | Allocations/call | Bytes/call |
|---|---|---|---|---:|---:|---:|---:|
| popcount | words=4 | ops-dispatched | ops-dispatched/scalar | 142857142 | 7.0 | 0.00 | 0 |
| popcount | words=4 | scalar-backend | scalar-backend | 200000000 | 4.9 | 0.00 | 0 |
| popcount | words=4 | simd-backend | simd-backend/avx2 | 200000000 | 4.9 | 0.00 | 0 |
| popcount | words=8 | ops-dispatched | ops-dispatched/simd | 166666666 | 5.5 | 0.00 | 0 |
| popcount | words=8 | scalar-backend | scalar-backend | 166666666 | 6.2 | 0.00 | 0 |
| popcount | words=8 | simd-backend | simd-backend/avx2 | 200000000 | 5.3 | 0.00 | 0 |
| popcount | words=16 | ops-dispatched | ops-dispatched/simd | 166666666 | 6.3 | 0.00 | 0 |
| popcount | words=16 | scalar-backend | scalar-backend | 111111111 | 8.8 | 0.00 | 0 |
| popcount | words=16 | simd-backend | simd-backend/avx2 | 142857142 | 6.0 | 0.00 | 0 |
| popcount | words=64 | ops-dispatched | ops-dispatched/simd | 83333333 | 12.9 | 0.00 | 0 |
| popcount | words=64 | scalar-backend | scalar-backend | 27027027 | 29.4 | 0.00 | 0 |
| popcount | words=64 | simd-backend | simd-backend/avx2 | 71428571 | 13.0 | 0.00 | 0 |
| popcount | words=127 | ops-dispatched | ops-dispatched/simd | 38461538 | 23.4 | 0.00 | 0 |
| popcount | words=127 | scalar-backend | scalar-backend | 14705882 | 55.6 | 0.00 | 0 |
| popcount | words=127 | simd-backend | simd-backend/avx2 | 35714285 | 23.2 | 0.00 | 0 |
| popcount | words=507 | ops-dispatched | ops-dispatched/simd | 14705882 | 68.2 | 0.00 | 0 |
| popcount | words=507 | scalar-backend | scalar-backend | 3448275 | 207.1 | 0.00 | 0 |
| popcount | words=507 | simd-backend | simd-backend/avx2 | 11111111 | 71.6 | 0.00 | 0 |
| popcount | words=4096 | ops-dispatched | ops-dispatched/simd | 1769911 | 509.7 | 0.00 | 0 |
| popcount | words=4096 | scalar-backend | scalar-backend | 615384 | 1597.4 | 0.00 | 0 |
| popcount | words=4096 | simd-backend | simd-backend/avx2 | 2016129 | 505.3 | 0.00 | 0 |
| popcount | words=65536 | ops-dispatched | ops-dispatched/simd | 108424 | 7939.7 | 0.00 | 0 |
| popcount | words=65536 | scalar-backend | scalar-backend | 35395 | 25872.1 | 0.00 | 0 |
| popcount | words=65536 | simd-backend | simd-backend/avx2 | 112612 | 8002.0 | 0.00 | 0 |
| zero-test | set_bit=4096 words=64 | count-ones | count-ones/simd | 71428571 | 13.4 | 0.00 | 0 |
| zero-test | set_bit=4096 words=64 | find-first-one | find-first-one/simd | 76923076 | 10.5 | 0.00 | 0 |
| zero-test | set_bit=0 words=64 | count-ones | count-ones/simd | 71428571 | 13.4 | 0.00 | 0 |
| zero-test | set_bit=0 words=64 | find-first-one | find-first-one/simd | 200000000 | 4.9 | 0.00 | 0 |
| zero-test | set_bit=8128 words=127 | count-ones | count-ones/simd | 40000000 | 24.1 | 0.00 | 0 |
| zero-test | set_bit=8128 words=127 | find-first-one | find-first-one/simd | 50000000 | 17.8 | 0.00 | 0 |
| zero-test | set_bit=0 words=127 | count-ones | count-ones/simd | 40000000 | 23.3 | 0.00 | 0 |
| zero-test | set_bit=0 words=127 | find-first-one | find-first-one/simd | 166666666 | 4.9 | 0.00 | 0 |
| zero-test | set_bit=32448 words=507 | count-ones | count-ones/simd | 12820512 | 69.7 | 0.00 | 0 |
| zero-test | set_bit=32448 words=507 | find-first-one | find-first-one/simd | 14492753 | 62.3 | 0.00 | 0 |
| zero-test | set_bit=0 words=507 | count-ones | count-ones/simd | 12820512 | 69.4 | 0.00 | 0 |
| zero-test | set_bit=0 words=507 | find-first-one | find-first-one/simd | 200000000 | 4.9 | 0.00 | 0 |
| zero-test | set_bit=262144 words=4096 | count-ones | count-ones/simd | 1615508 | 504.2 | 0.00 | 0 |
| zero-test | set_bit=262144 words=4096 | find-first-one | find-first-one/simd | 2024291 | 439.6 | 0.00 | 0 |
| zero-test | set_bit=0 words=4096 | count-ones | count-ones/simd | 1736111 | 503.7 | 0.00 | 0 |
| zero-test | set_bit=0 words=4096 | find-first-one | find-first-one/simd | 200000000 | 4.9 | 0.00 | 0 |
| ldpc-codeword-check | n=16200 | count-ones | syndrome-then-count-ones | 18779 | 60700.8 | 1.00 | 1128 |
| ldpc-codeword-check | n=16200 | find-first-one | syndrome-then-find-first-one | 18242 | 52960.1 | 1.00 | 1128 |
| ldpc-codeword-check | n=64800 | count-ones | syndrome-then-count-ones | 3594 | 269203.4 | 1.00 | 4056 |
| ldpc-codeword-check | n=64800 | find-first-one | syndrome-then-find-first-one | 3736 | 265109.5 | 1.00 | 4056 |

## Transpose, bitslice and BCH encoding consumers

| Workload | Size | Route | Observed path | Calls | ns/call | Allocations/call | Bytes/call |
|---|---|---|---|---:|---:|---:|---:|
| transpose-64x64 | blocks=1 | transpose-scalar | transpose-scalar/portable | 8064516 | 92.2 | 0.00 | 0 |
| transpose-64x64 | blocks=1 | transpose-detected | transpose-detected/avx2-bit-twiddle | 18867924 | 46.4 | 0.00 | 0 |
| transpose-64x64 | blocks=16 | transpose-scalar | transpose-scalar/portable | 651465 | 1477.4 | 0.00 | 0 |
| transpose-64x64 | blocks=16 | transpose-detected | transpose-detected/avx2-bit-twiddle | 1219512 | 678.9 | 0.00 | 0 |
| transpose-64x64 | blocks=256 | transpose-scalar | transpose-scalar/portable | 31393 | 24915.7 | 0.00 | 0 |
| transpose-64x64 | blocks=256 | transpose-detected | transpose-detected/avx2-bit-twiddle | 71664 | 11511.3 | 0.00 | 0 |
| transpose-64x64 | blocks=4096 | transpose-scalar | transpose-scalar/portable | 2497 | 386527.3 | 0.00 | 0 |
| transpose-64x64 | blocks=4096 | transpose-detected | transpose-detected/avx2-bit-twiddle | 4747 | 185726.0 | 0.00 | 0 |
| dense-transpose | cols=1024 rows=1024 | current | current/simple | 29611 | 27495.0 | 1.00 | 131072 |
| dense-transpose | cols=4096 rows=4096 | current | current/macro-tiled-8 | 1460 | 599773.8 | 1.00 | 2097152 |
| bch-encode-batch | batch=1 degree=8 | current | current/PolyRemainderScalar | 1037344 | 789.8 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=8 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 1164144 | 781.8 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=8 | family-table-remainder | family-pinned/TableRemainder | 2044989 | 492.3 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=8 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 341880 | 2161.8 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=8 | family-clmul-fold | family-pinned/ClmulFold | 2336448 | 386.7 | 1.00 | 56 |
| bch-encode-batch | batch=16 degree=8 | current | current/PolyRemainderScalar | 77978 | 11558.6 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=8 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 87973 | 11473.0 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=8 | family-table-remainder | family-pinned/TableRemainder | 130310 | 6975.8 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=8 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 123639 | 7263.2 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=8 | family-clmul-fold | family-pinned/ClmulFold | 164771 | 5533.5 | 16.00 | 896 |
| bch-encode-batch | batch=64 degree=8 | current | current/PolyRemainderScalar | 12347 | 87760.5 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=8 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 12050 | 81247.7 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=8 | family-table-remainder | family-pinned/TableRemainder | 30320 | 29421.6 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=8 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 38635 | 23712.5 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=8 | family-clmul-fold | family-pinned/ClmulFold | 38732 | 31479.6 | 64.00 | 3584 |
| bch-encode-batch | batch=256 degree=8 | current | current/PolyRemainderScalar | 1621 | 450667.6 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=8 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 2118 | 443145.4 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=8 | family-table-remainder | family-pinned/TableRemainder | 3621 | 244095.2 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=8 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 3487 | 215227.9 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=8 | family-clmul-fold | family-pinned/ClmulFold | 3945 | 217262.4 | 256.00 | 14336 |
| bch-encode-batch | batch=1024 degree=8 | current | current/PolyRemainderScalar | 425 | 1888743.9 | 1024.00 | 57344 |
| bch-encode-batch | batch=1024 degree=8 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 487 | 1887272.7 | 1024.00 | 57344 |
| bch-encode-batch | batch=1024 degree=8 | family-table-remainder | family-pinned/TableRemainder | 807 | 1123334.5 | 1024.00 | 57344 |
| bch-encode-batch | batch=1024 degree=8 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 923 | 964260.5 | 1024.00 | 57344 |
| bch-encode-batch | batch=1024 degree=8 | family-clmul-fold | family-pinned/ClmulFold | 904 | 964896.5 | 1024.00 | 57344 |
| bch-encode-batch | batch=1 degree=14 | current | current/PolyRemainderScalar | 7092 | 131302.8 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=14 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 6791 | 124694.6 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=14 | family-table-remainder | family-pinned/TableRemainder | 34763 | 26799.6 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=14 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 2544 | 323960.0 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=14 | family-clmul-fold | family-pinned/ClmulFold | 42133 | 21274.9 | 1.00 | 56 |
| bch-encode-batch | batch=16 degree=14 | current | current/PolyRemainderScalar | 405 | 2497406.1 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=14 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 338 | 2485830.3 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=14 | family-table-remainder | family-pinned/TableRemainder | 830 | 1007133.5 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=14 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 821 | 1203208.8 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=14 | family-clmul-fold | family-pinned/ClmulFold | 962 | 946586.8 | 16.00 | 896 |
| bch-encode-batch | batch=64 degree=14 | current | current/PolyRemainderScalar | 93 | 9921885.8 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=14 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 94 | 9917819.6 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=14 | family-table-remainder | family-pinned/TableRemainder | 218 | 4060052.6 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=14 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 257 | 3980599.0 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=14 | family-clmul-fold | family-pinned/ClmulFold | 245 | 3861052.9 | 64.00 | 3584 |
| bch-encode-batch | batch=256 degree=14 | current | current/PolyRemainderScalar | 25 | 40041043.8 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=14 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 25 | 39909117.8 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=14 | family-table-remainder | family-pinned/TableRemainder | 59 | 16297476.2 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=14 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 64 | 15707831.2 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=14 | family-clmul-fold | family-pinned/ClmulFold | 63 | 15244365.8 | 256.00 | 14336 |
| bch-encode-batch | batch=1024 degree=14 | current | current/PolyRemainderScalar | 6 | 158725338.2 | 1024.00 | 57344 |
| bch-encode-batch | batch=1024 degree=14 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 6 | 159645917.0 | 1024.00 | 57344 |
| bch-encode-batch | batch=1024 degree=14 | family-table-remainder | family-pinned/TableRemainder | 15 | 65157057.0 | 1024.00 | 57344 |
| bch-encode-batch | batch=1024 degree=14 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 15 | 63466715.8 | 1024.00 | 57344 |
| bch-encode-batch | batch=1024 degree=14 | family-clmul-fold | family-pinned/ClmulFold | 16 | 61119549.9 | 1024.00 | 57344 |
| bch-encode-batch | batch=1 degree=16 | current | current/PolyRemainderScalar | 1591 | 598679.4 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=16 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 1671 | 599981.9 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=16 | family-table-remainder | family-pinned/TableRemainder | 4466 | 223765.4 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=16 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 628 | 1587654.3 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=16 | family-clmul-fold | family-pinned/ClmulFold | 3771 | 208777.9 | 1.00 | 56 |
| bch-encode-batch | batch=16 degree=16 | current | current/PolyRemainderScalar | 98 | 10030198.3 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=16 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 100 | 10025746.4 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=16 | family-table-remainder | family-pinned/TableRemainder | 212 | 4092784.0 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=16 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 199 | 5030585.3 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=16 | family-clmul-fold | family-pinned/ClmulFold | 250 | 3828762.9 | 16.00 | 896 |
| bch-encode-batch | batch=64 degree=16 | current | current/PolyRemainderScalar | 24 | 39969543.3 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=16 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 24 | 40137807.6 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=16 | family-table-remainder | family-pinned/TableRemainder | 60 | 16207942.3 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=16 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 61 | 16089140.8 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=16 | family-clmul-fold | family-pinned/ClmulFold | 65 | 15257317.8 | 64.00 | 3584 |
| bch-encode-batch | batch=256 degree=16 | current | current/PolyRemainderScalar | 6 | 159855981.5 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=16 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 6 | 160423506.3 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=16 | family-table-remainder | family-pinned/TableRemainder | 15 | 65260669.5 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=16 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 15 | 64046293.2 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=16 | family-clmul-fold | family-pinned/ClmulFold | 16 | 60913748.1 | 256.00 | 14336 |
| bch-encode-batch-alloc | batch=16 degree=8 | current | current-allocating/PolyRemainderScalar | 67796 | 11765.8 | 33.00 | 2816 |
| bch-encode-batch-alloc | batch=16 degree=8 | caller-buffer | caller-buffer/PolyRemainderScalar | 79314 | 11609.4 | 16.00 | 896 |
| bch-encode-batch-alloc | batch=256 degree=8 | current | current-allocating/PolyRemainderScalar | 1983 | 445075.1 | 513.00 | 45056 |
| bch-encode-batch-alloc | batch=256 degree=8 | caller-buffer | caller-buffer/PolyRemainderScalar | 2058 | 442602.0 | 256.00 | 14336 |
| bch-encode-batch-alloc | batch=16 degree=14 | current | current-allocating/PolyRemainderScalar | 385 | 2488965.6 | 33.00 | 35072 |
| bch-encode-batch-alloc | batch=16 degree=14 | caller-buffer | caller-buffer/PolyRemainderScalar | 413 | 2482110.5 | 16.00 | 896 |
| bch-encode-batch-alloc | batch=256 degree=14 | current | current-allocating/PolyRemainderScalar | 24 | 40169048.6 | 513.00 | 561152 |
| bch-encode-batch-alloc | batch=256 degree=14 | caller-buffer | caller-buffer/PolyRemainderScalar | 25 | 40133574.7 | 256.00 | 14336 |
| bch-encode-batch-parallel | batch=256 degree=14 workers=1 | current | current-parallel/PolyRemainderScalar | 24 | 41606508.7 | 256.00 | 14336 |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=1 | current | current-parallel/PolyRemainderScalar | 6 | 160914395.8 | 1024.00 | 57344 |
| bch-encode-batch-parallel | batch=256 degree=14 workers=6 | current | current-parallel/PolyRemainderScalar | 116 | 7209624.5 | 256.01 | 14349 |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=6 | current | current-parallel/PolyRemainderScalar | 33 | 27640487.4 | 1024.00 | 57344 |
| bch-encode-batch-parallel | batch=256 degree=14 workers=12 | current | current-parallel/PolyRemainderScalar | 243 | 4333569.6 | 256.01 | 14355 |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=12 | current | current-parallel/PolyRemainderScalar | 41 | 24969702.1 | 1024.00 | 57344 |
| bch-encode-batch-parallel | batch=256 degree=14 workers=24 | current | current-parallel/PolyRemainderScalar | 265 | 3604605.1 | 256.02 | 14359 |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=24 | current | current-parallel/PolyRemainderScalar | 76 | 12459705.2 | 1024.01 | 57364 |
| field-id-hint | calls=1 | current | current/arc-allocating-field-identity | 83333333 | 12.0 | 1.00 | 56 |
| field-id-hint | calls=16 | current | current/arc-allocating-field-identity | 5291005 | 169.2 | 16.00 | 896 |
| field-id-hint | calls=256 | current | current/arc-allocating-field-identity | 322684 | 2689.0 | 256.00 | 14336 |
| field-id-hint | calls=1024 | current | current/arc-allocating-field-identity | 83118 | 10856.1 | 1024.00 | 57344 |
| dvb-bch-encode | batch=16 n=7200 | current | current/field-polynomial-div-rem | 11 | 179686464.2 | 272.00 | 12780288 |
| dvb-bch-encode | batch=1 n=32400 | current | current/field-polynomial-div-rem | 34 | 58604175.9 | 21.00 | 3645264 |

## Reported setup and conversion costs

Whole-consumer routes report the phases the receipt schema carries. `setup` is one-shot preparation the timed body does not repeat; the remaining columns are per timed call. A zero states that the route performs no such work.

| Workload | Size | Route | setup ns | pack ns | unpack ns | batch fill ns | dispatch ns |
|---|---|---|---:|---:|---:|---:|---:|
| dense-rref | cols=256 rows=256 | current | 5050 | 0 | 0 | 0 | 0 |
| dense-rref | cols=512 rows=512 | current | 19340 | 0 | 0 | 0 | 0 |
| dense-rref | cols=1024 rows=1024 | current | 105520 | 0 | 0 | 0 | 0 |
| dense-rref | cols=2048 rows=2048 | current | 390353 | 0 | 0 | 0 | 0 |
| dense-matvec | cols=256 rows=256 | current | 5090 | 0 | 211 | 0 | 0 |
| dense-matvec | cols=1024 rows=1024 | current | 93871 | 0 | 810 | 0 | 0 |
| dense-matvec | cols=4096 rows=1024 | current | 466253 | 0 | 1051 | 0 | 0 |
| dense-matvec | cols=4096 rows=4096 | current | 1381728 | 0 | 3208 | 0 | 0 |
| ldpc-syndrome | n=16200 | current | 1640390 | 0 | 6742 | 0 | 0 |
| ldpc-syndrome | n=64800 | current | 1032976 | 0 | 30627 | 0 | 0 |
| ldpc-codeword-check | n=16200 | count-ones | 2004821 | 0 | 7043 | 0 | 0 |
| ldpc-codeword-check | n=16200 | find-first-one | 1641520 | 0 | 6678 | 0 | 0 |
| ldpc-codeword-check | n=64800 | count-ones | 797414 | 0 | 23335 | 0 | 0 |
| ldpc-codeword-check | n=64800 | find-first-one | 774075 | 0 | 22420 | 0 | 0 |
| dense-transpose | cols=1024 rows=1024 | current | 108110 | 0 | 0 | 0 | 4 |
| dense-transpose | cols=4096 rows=4096 | current | 1614159 | 0 | 0 | 0 | 4 |
| bch-encode-batch | batch=1 degree=8 | current | 44580 | 5745 | 0 | 142 | 4 |
| bch-encode-batch | batch=1 degree=8 | family-poly-remainder-scalar | 39671 | 4856 | 0 | 147 | 4 |
| bch-encode-batch | batch=1 degree=8 | family-table-remainder | 39611 | 4275 | 0 | 120 | 3 |
| bch-encode-batch | batch=1 degree=8 | family-bitslice-interleaved | 42191 | 5239 | 0 | 155 | 4 |
| bch-encode-batch | batch=1 degree=8 | family-clmul-fold | 47250 | 6018 | 0 | 172 | 5 |
| bch-encode-batch | batch=16 degree=8 | current | 40440 | 4880 | 0 | 1747 | 4 |
| bch-encode-batch | batch=16 degree=8 | family-poly-remainder-scalar | 36210 | 4355 | 0 | 1535 | 3 |
| bch-encode-batch | batch=16 degree=8 | family-table-remainder | 39140 | 4911 | 0 | 1750 | 4 |
| bch-encode-batch | batch=16 degree=8 | family-bitslice-interleaved | 46520 | 4917 | 0 | 1710 | 4 |
| bch-encode-batch | batch=16 degree=8 | family-clmul-fold | 52650 | 5726 | 0 | 2015 | 4 |
| bch-encode-batch | batch=64 degree=8 | current | 39651 | 4996 | 0 | 7835 | 4 |
| bch-encode-batch | batch=64 degree=8 | family-poly-remainder-scalar | 44001 | 4981 | 0 | 7852 | 4 |
| bch-encode-batch | batch=64 degree=8 | family-table-remainder | 40461 | 5005 | 0 | 7830 | 4 |
| bch-encode-batch | batch=64 degree=8 | family-bitslice-interleaved | 41291 | 4995 | 0 | 7805 | 4 |
| bch-encode-batch | batch=64 degree=8 | family-clmul-fold | 41490 | 4959 | 0 | 8710 | 4 |
| bch-encode-batch | batch=256 degree=8 | current | 39620 | 4820 | 0 | 31490 | 4 |
| bch-encode-batch | batch=256 degree=8 | family-poly-remainder-scalar | 40190 | 4811 | 0 | 31090 | 4 |
| bch-encode-batch | batch=256 degree=8 | family-table-remainder | 40461 | 4810 | 0 | 30557 | 4 |
| bch-encode-batch | batch=256 degree=8 | family-bitslice-interleaved | 38961 | 4840 | 0 | 38705 | 4 |
| bch-encode-batch | batch=256 degree=8 | family-clmul-fold | 38781 | 4812 | 0 | 30722 | 4 |
| bch-encode-batch | batch=1024 degree=8 | current | 36280 | 4810 | 0 | 127985 | 4 |
| bch-encode-batch | batch=1024 degree=8 | family-poly-remainder-scalar | 63441 | 6831 | 0 | 147433 | 4 |
| bch-encode-batch | batch=1024 degree=8 | family-table-remainder | 40981 | 5111 | 0 | 125238 | 4 |
| bch-encode-batch | batch=1024 degree=8 | family-bitslice-interleaved | 41690 | 4847 | 0 | 124938 | 4 |
| bch-encode-batch | batch=1024 degree=8 | family-clmul-fold | 39400 | 4843 | 0 | 129265 | 4 |
| bch-encode-batch | batch=1 degree=14 | current | 1593110 | 6823 | 0 | 690 | 4 |
| bch-encode-batch | batch=1 degree=14 | family-poly-remainder-scalar | 1657879 | 7254 | 0 | 705 | 4 |
| bch-encode-batch | batch=1 degree=14 | family-table-remainder | 1599639 | 6573 | 0 | 627 | 3 |
| bch-encode-batch | batch=1 degree=14 | family-bitslice-interleaved | 1650000 | 7270 | 0 | 672 | 4 |
| bch-encode-batch | batch=1 degree=14 | family-clmul-fold | 1415449 | 7629 | 0 | 745 | 4 |
| bch-encode-batch | batch=16 degree=14 | current | 1605319 | 6943 | 0 | 9662 | 4 |
| bch-encode-batch | batch=16 degree=14 | family-poly-remainder-scalar | 1605729 | 7186 | 0 | 9592 | 4 |
| bch-encode-batch | batch=16 degree=14 | family-table-remainder | 1841421 | 6868 | 0 | 9627 | 4 |
| bch-encode-batch | batch=16 degree=14 | family-bitslice-interleaved | 1601789 | 6573 | 0 | 9630 | 4 |
| bch-encode-batch | batch=16 degree=14 | family-clmul-fold | 1752240 | 7901 | 0 | 10702 | 4 |
| bch-encode-batch | batch=64 degree=14 | current | 1608990 | 6832 | 0 | 39252 | 4 |
| bch-encode-batch | batch=64 degree=14 | family-poly-remainder-scalar | 1596009 | 7418 | 0 | 51522 | 9 |
| bch-encode-batch | batch=64 degree=14 | family-table-remainder | 1580609 | 6676 | 0 | 38187 | 3 |
| bch-encode-batch | batch=64 degree=14 | family-bitslice-interleaved | 1669260 | 6802 | 0 | 40120 | 4 |
| bch-encode-batch | batch=64 degree=14 | family-clmul-fold | 1601070 | 7015 | 0 | 39420 | 4 |
| bch-encode-batch | batch=256 degree=14 | current | 1808681 | 6832 | 0 | 323829 | 4 |
| bch-encode-batch | batch=256 degree=14 | family-poly-remainder-scalar | 1711160 | 8254 | 0 | 385149 | 4 |
| bch-encode-batch | batch=256 degree=14 | family-table-remainder | 1600910 | 6295 | 0 | 305231 | 3 |
| bch-encode-batch | batch=256 degree=14 | family-bitslice-interleaved | 1621629 | 6698 | 0 | 319129 | 3 |
| bch-encode-batch | batch=256 degree=14 | family-clmul-fold | 1589119 | 6394 | 0 | 305181 | 3 |
| bch-encode-batch | batch=1024 degree=14 | current | 1584779 | 6478 | 0 | 1280170 | 3 |
| bch-encode-batch | batch=1024 degree=14 | family-poly-remainder-scalar | 1592850 | 6128 | 0 | 1446068 | 3 |
| bch-encode-batch | batch=1024 degree=14 | family-table-remainder | 1597800 | 6248 | 0 | 1282052 | 3 |
| bch-encode-batch | batch=1024 degree=14 | family-bitslice-interleaved | 1594949 | 6015 | 0 | 1228387 | 3 |
| bch-encode-batch | batch=1024 degree=14 | family-clmul-fold | 1605890 | 6230 | 0 | 1279142 | 3 |
| bch-encode-batch | batch=1 degree=16 | current | 6223206 | 6398 | 0 | 2097 | 3 |
| bch-encode-batch | batch=1 degree=16 | family-poly-remainder-scalar | 6658978 | 6070 | 0 | 2042 | 3 |
| bch-encode-batch | batch=1 degree=16 | family-table-remainder | 6826560 | 6001 | 0 | 1985 | 3 |
| bch-encode-batch | batch=1 degree=16 | family-bitslice-interleaved | 6367717 | 6828 | 0 | 2012 | 3 |
| bch-encode-batch | batch=1 degree=16 | family-clmul-fold | 6542108 | 6091 | 0 | 2035 | 3 |
| bch-encode-batch | batch=16 degree=16 | current | 6294287 | 6694 | 0 | 32877 | 3 |
| bch-encode-batch | batch=16 degree=16 | family-poly-remainder-scalar | 6159446 | 6019 | 0 | 31965 | 3 |
| bch-encode-batch | batch=16 degree=16 | family-table-remainder | 6683159 | 6961 | 0 | 37022 | 4 |
| bch-encode-batch | batch=16 degree=16 | family-bitslice-interleaved | 6224606 | 6152 | 0 | 31065 | 3 |
| bch-encode-batch | batch=16 degree=16 | family-clmul-fold | 6306607 | 6373 | 0 | 32680 | 3 |
| bch-encode-batch | batch=64 degree=16 | current | 6730809 | 6032 | 0 | 123100 | 3 |
| bch-encode-batch | batch=64 degree=16 | family-poly-remainder-scalar | 6522478 | 6112 | 0 | 125430 | 3 |
| bch-encode-batch | batch=64 degree=16 | family-table-remainder | 6587149 | 6082 | 0 | 123665 | 3 |
| bch-encode-batch | batch=64 degree=16 | family-bitslice-interleaved | 6844500 | 6396 | 0 | 127245 | 3 |
| bch-encode-batch | batch=64 degree=16 | family-clmul-fold | 6257067 | 8289 | 0 | 157248 | 4 |
| bch-encode-batch | batch=256 degree=16 | current | 6210996 | 6990 | 0 | 1550831 | 4 |
| bch-encode-batch | batch=256 degree=16 | family-poly-remainder-scalar | 6638049 | 7028 | 0 | 1442043 | 4 |
| bch-encode-batch | batch=256 degree=16 | family-table-remainder | 6789369 | 6038 | 0 | 1207487 | 3 |
| bch-encode-batch | batch=256 degree=16 | family-bitslice-interleaved | 6293286 | 6028 | 0 | 1181489 | 3 |
| bch-encode-batch | batch=256 degree=16 | family-clmul-fold | 6291957 | 6366 | 0 | 1244679 | 3 |
| bch-encode-batch-alloc | batch=16 degree=8 | current | 38580 | 4902 | 0 | 1710 | 5 |
| bch-encode-batch-alloc | batch=16 degree=8 | caller-buffer | 42340 | 5393 | 0 | 1910 | 4 |
| bch-encode-batch-alloc | batch=256 degree=8 | current | 38940 | 4833 | 0 | 31632 | 4 |
| bch-encode-batch-alloc | batch=256 degree=8 | caller-buffer | 39080 | 4810 | 0 | 31387 | 4 |
| bch-encode-batch-alloc | batch=16 degree=14 | current | 1612809 | 6640 | 0 | 9245 | 3 |
| bch-encode-batch-alloc | batch=16 degree=14 | caller-buffer | 1654879 | 6576 | 0 | 9260 | 3 |
| bch-encode-batch-alloc | batch=256 degree=14 | current | 1585669 | 6489 | 0 | 313346 | 3 |
| bch-encode-batch-alloc | batch=256 degree=14 | caller-buffer | 1607210 | 6333 | 0 | 308396 | 3 |
| bch-encode-batch-parallel | batch=256 degree=14 workers=1 | current | 1607069 | 6346 | 0 | 307591 | 3 |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=1 | current | 1683030 | 8406 | 0 | 1621077 | 7 |
| bch-encode-batch-parallel | batch=256 degree=14 workers=6 | current | 1615049 | 6500 | 0 | 312196 | 3 |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=6 | current | 1925861 | 6186 | 0 | 1312352 | 3 |
| bch-encode-batch-parallel | batch=256 degree=14 workers=12 | current | 1465159 | 6126 | 0 | 294826 | 14 |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=12 | current | 1729240 | 6350 | 0 | 1314842 | 3 |
| bch-encode-batch-parallel | batch=256 degree=14 workers=24 | current | 1587630 | 8481 | 0 | 358842 | 4 |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=24 | current | 1491619 | 8590 | 0 | 1566724 | 5 |
| dvb-bch-encode | batch=16 n=7200 | current | 1077766 | 0 | 0 | 6297 | 0 |
| dvb-bch-encode | batch=1 n=32400 | current | 4954129 | 0 | 0 | 1567 | 0 |
