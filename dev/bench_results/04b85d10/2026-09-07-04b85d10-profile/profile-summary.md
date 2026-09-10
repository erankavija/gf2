# Current-code consumer profile sweep

Projection of `profile.jsonl`, one row per measured route. The run recorded 168 rows.

`ns/call` is the wall-clock mean over the calls the row reports, a single-run figure with no interval: the sweep locates the expensive routes and the protocol receipts carry the intervals.

## Logical row, parity and dense-matrix consumers

| Workload | Size | Route | Observed path | Calls | ns/call | Allocations/call | Bytes/call |
|---|---|---|---|---:|---:|---:|---:|
| row-xor | rows=64 words=4 | ops-dispatched | ops-dispatched/scalar | 4347826 | 124.5 | 0.00 | 0 |
| row-xor | rows=64 words=4 | ops-resolved | ops-resolved/scalar | 4545454 | 116.2 | 0.00 | 0 |
| row-xor | rows=64 words=4 | scalar-backend | scalar-backend | 4347826 | 115.2 | 0.00 | 0 |
| row-xor | rows=64 words=4 | simd-backend | simd-backend/avx2 | 5000000 | 98.0 | 0.00 | 0 |
| row-xor | rows=64 words=8 | ops-dispatched | ops-dispatched/simd | 3225806 | 125.0 | 0.00 | 0 |
| row-xor | rows=64 words=8 | ops-resolved | ops-resolved/simd | 3333333 | 117.3 | 0.00 | 0 |
| row-xor | rows=64 words=8 | scalar-backend | scalar-backend | 2777777 | 191.4 | 0.00 | 0 |
| row-xor | rows=64 words=8 | simd-backend | simd-backend/avx2 | 4761904 | 115.3 | 0.00 | 0 |
| row-xor | rows=64 words=16 | ops-dispatched | ops-dispatched/simd | 3333333 | 145.4 | 0.00 | 0 |
| row-xor | rows=64 words=16 | ops-resolved | ops-resolved/simd | 3703703 | 141.1 | 0.00 | 0 |
| row-xor | rows=64 words=16 | scalar-backend | scalar-backend | 2083333 | 275.7 | 0.00 | 0 |
| row-xor | rows=64 words=16 | simd-backend | simd-backend/avx2 | 3846153 | 142.3 | 0.00 | 0 |
| row-xor | rows=64 words=64 | ops-dispatched | ops-dispatched/simd | 1515151 | 390.6 | 0.00 | 0 |
| row-xor | rows=64 words=64 | ops-resolved | ops-resolved/simd | 1886792 | 373.2 | 0.00 | 0 |
| row-xor | rows=64 words=64 | scalar-backend | scalar-backend | 1098901 | 772.6 | 0.00 | 0 |
| row-xor | rows=64 words=64 | simd-backend | simd-backend/avx2 | 1923076 | 382.1 | 0.00 | 0 |
| row-xor | rows=64 words=512 | ops-dispatched | ops-dispatched/simd | 303030 | 2491.0 | 0.00 | 0 |
| row-xor | rows=64 words=512 | ops-resolved | ops-resolved/simd | 307692 | 2462.5 | 0.00 | 0 |
| row-xor | rows=64 words=512 | scalar-backend | scalar-backend | 142247 | 5490.6 | 0.00 | 0 |
| row-xor | rows=64 words=512 | simd-backend | simd-backend/avx2 | 303030 | 2485.3 | 0.00 | 0 |
| row-xor | rows=64 words=8192 | ops-dispatched | ops-dispatched/simd | 11208 | 39279.8 | 0.00 | 0 |
| row-xor | rows=64 words=8192 | ops-resolved | ops-resolved/simd | 19312 | 39565.8 | 0.00 | 0 |
| row-xor | rows=64 words=8192 | scalar-backend | scalar-backend | 12163 | 88893.0 | 0.00 | 0 |
| row-xor | rows=64 words=8192 | simd-backend | simd-backend/avx2 | 17385 | 39384.7 | 0.00 | 0 |
| dense-rref | cols=256 rows=256 | current | current/blocked-m4ri | 13361 | 68148.5 | 137.00 | 36832 |
| dense-rref | cols=512 rows=512 | current | current/blocked-m4ri | 3319 | 278012.4 | 266.00 | 122784 |
| dense-rref | cols=1024 rows=1024 | current | current/blocked-m4ri | 847 | 915492.0 | 267.00 | 2391008 |
| dense-rref | cols=2048 rows=2048 | current | current/blocked-m4ri | 251 | 4285355.4 | 524.00 | 9239520 |
| dense-matvec | cols=256 rows=256 | current | current/scalar-row-parity | 480769 | 938.7 | 1.00 | 32 |
| dense-matvec | cols=1024 rows=1024 | current | current/simd-and-popcnt | 94153 | 4920.6 | 1.00 | 128 |
| dense-matvec | cols=4096 rows=1024 | current | current/simd-and-popcnt | 52798 | 17661.4 | 1.00 | 128 |
| dense-matvec | cols=4096 rows=4096 | current | current/simd-and-popcnt | 11911 | 76482.1 | 1.00 | 512 |
| ldpc-syndrome | n=16200 | current | current/csr-bit-at-a-time-matvec | 12828 | 67033.0 | 1.00 | 1128 |
| ldpc-syndrome | n=64800 | current | current/csr-bit-at-a-time-matvec | 2165 | 310409.1 | 1.00 | 4056 |

## Count and fused-reduction consumers

| Workload | Size | Route | Observed path | Calls | ns/call | Allocations/call | Bytes/call |
|---|---|---|---|---:|---:|---:|---:|
| popcount | words=4 | ops-dispatched | ops-dispatched/scalar | 8333333 | 8.3 | 0.00 | 0 |
| popcount | words=4 | scalar-backend | scalar-backend | 8333333 | 5.5 | 0.00 | 0 |
| popcount | words=4 | simd-backend | simd-backend/avx2 | 9090909 | 6.1 | 0.00 | 0 |
| popcount | words=8 | ops-dispatched | ops-dispatched/simd | 7692307 | 6.3 | 0.00 | 0 |
| popcount | words=8 | scalar-backend | scalar-backend | 8333333 | 6.6 | 0.00 | 0 |
| popcount | words=8 | simd-backend | simd-backend/avx2 | 8333333 | 6.4 | 0.00 | 0 |
| popcount | words=16 | ops-dispatched | ops-dispatched/simd | 8333333 | 7.2 | 0.00 | 0 |
| popcount | words=16 | scalar-backend | scalar-backend | 7142857 | 9.9 | 0.00 | 0 |
| popcount | words=16 | simd-backend | simd-backend/avx2 | 8333333 | 7.1 | 0.00 | 0 |
| popcount | words=64 | ops-dispatched | ops-dispatched/simd | 5263157 | 14.7 | 0.00 | 0 |
| popcount | words=64 | scalar-backend | scalar-backend | 5882352 | 32.2 | 0.00 | 0 |
| popcount | words=64 | simd-backend | simd-backend/avx2 | 6666666 | 14.5 | 0.00 | 0 |
| popcount | words=127 | ops-dispatched | ops-dispatched/simd | 5555555 | 26.4 | 0.00 | 0 |
| popcount | words=127 | scalar-backend | scalar-backend | 3703703 | 62.9 | 0.00 | 0 |
| popcount | words=127 | simd-backend | simd-backend/avx2 | 4000000 | 26.0 | 0.00 | 0 |
| popcount | words=507 | ops-dispatched | ops-dispatched/simd | 4545454 | 77.9 | 0.00 | 0 |
| popcount | words=507 | scalar-backend | scalar-backend | 2325581 | 237.5 | 0.00 | 0 |
| popcount | words=507 | simd-backend | simd-backend/avx2 | 4545454 | 78.6 | 0.00 | 0 |
| popcount | words=4096 | ops-dispatched | ops-dispatched/simd | 1515151 | 578.5 | 0.00 | 0 |
| popcount | words=4096 | scalar-backend | scalar-backend | 512820 | 1829.5 | 0.00 | 0 |
| popcount | words=4096 | simd-backend | simd-backend/avx2 | 1515151 | 567.3 | 0.00 | 0 |
| popcount | words=65536 | ops-dispatched | ops-dispatched/simd | 101419 | 9235.7 | 0.00 | 0 |
| popcount | words=65536 | scalar-backend | scalar-backend | 36576 | 29151.6 | 0.00 | 0 |
| popcount | words=65536 | simd-backend | simd-backend/avx2 | 109158 | 9221.8 | 0.00 | 0 |
| zero-test | set_bit=4096 words=64 | count-ones | count-ones/simd | 7692307 | 15.3 | 0.00 | 0 |
| zero-test | set_bit=4096 words=64 | find-first-one | find-first-one/simd | 7692307 | 11.8 | 0.00 | 0 |
| zero-test | set_bit=0 words=64 | count-ones | count-ones/simd | 5882352 | 15.3 | 0.00 | 0 |
| zero-test | set_bit=0 words=64 | find-first-one | find-first-one/simd | 8333333 | 5.0 | 0.00 | 0 |
| zero-test | set_bit=8128 words=127 | count-ones | count-ones/simd | 6666666 | 27.1 | 0.00 | 0 |
| zero-test | set_bit=8128 words=127 | find-first-one | find-first-one/simd | 6666666 | 20.1 | 0.00 | 0 |
| zero-test | set_bit=0 words=127 | count-ones | count-ones/simd | 6250000 | 26.6 | 0.00 | 0 |
| zero-test | set_bit=0 words=127 | find-first-one | find-first-one/simd | 6666666 | 5.1 | 0.00 | 0 |
| zero-test | set_bit=32448 words=507 | count-ones | count-ones/simd | 3448275 | 78.3 | 0.00 | 0 |
| zero-test | set_bit=32448 words=507 | find-first-one | find-first-one/simd | 4761904 | 69.6 | 0.00 | 0 |
| zero-test | set_bit=0 words=507 | count-ones | count-ones/simd | 3703703 | 78.7 | 0.00 | 0 |
| zero-test | set_bit=0 words=507 | find-first-one | find-first-one/simd | 9090909 | 5.1 | 0.00 | 0 |
| zero-test | set_bit=262144 words=4096 | count-ones | count-ones/simd | 1515151 | 575.8 | 0.00 | 0 |
| zero-test | set_bit=262144 words=4096 | find-first-one | find-first-one/simd | 1724137 | 502.3 | 0.00 | 0 |
| zero-test | set_bit=0 words=4096 | count-ones | count-ones/simd | 1538461 | 577.3 | 0.00 | 0 |
| zero-test | set_bit=0 words=4096 | find-first-one | find-first-one/simd | 8333333 | 5.1 | 0.00 | 0 |
| ldpc-codeword-check | n=16200 | count-ones | syndrome-then-count-ones | 12479 | 67169.1 | 1.00 | 1128 |
| ldpc-codeword-check | n=16200 | find-first-one | syndrome-then-find-first-one | 12445 | 65586.3 | 1.00 | 1128 |
| ldpc-codeword-check | n=64800 | count-ones | syndrome-then-count-ones | 3427 | 311024.9 | 1.00 | 4056 |
| ldpc-codeword-check | n=64800 | find-first-one | syndrome-then-find-first-one | 3450 | 310079.8 | 1.00 | 4056 |

## Transpose, bitslice and BCH encoding consumers

| Workload | Size | Route | Observed path | Calls | ns/call | Allocations/call | Bytes/call |
|---|---|---|---|---:|---:|---:|---:|
| transpose-64x64 | blocks=1 | transpose-scalar | transpose-scalar/portable | 3030303 | 98.0 | 0.00 | 0 |
| transpose-64x64 | blocks=1 | transpose-detected | transpose-detected/avx2-bit-twiddle | 4761904 | 53.2 | 0.00 | 0 |
| transpose-64x64 | blocks=16 | transpose-scalar | transpose-scalar/portable | 613496 | 1520.2 | 0.00 | 0 |
| transpose-64x64 | blocks=16 | transpose-detected | transpose-detected/avx2-bit-twiddle | 1052631 | 789.1 | 0.00 | 0 |
| transpose-64x64 | blocks=256 | transpose-scalar | transpose-scalar/portable | 34469 | 24642.3 | 0.00 | 0 |
| transpose-64x64 | blocks=256 | transpose-detected | transpose-detected/avx2-bit-twiddle | 82440 | 12810.5 | 0.00 | 0 |
| transpose-64x64 | blocks=4096 | transpose-scalar | transpose-scalar/portable | 2587 | 416692.3 | 0.00 | 0 |
| transpose-64x64 | blocks=4096 | transpose-detected | transpose-detected/avx2-bit-twiddle | 2740 | 209212.5 | 0.00 | 0 |
| dense-transpose | cols=1024 rows=1024 | current | current/simple | 33692 | 32111.8 | 1.00 | 131072 |
| dense-transpose | cols=4096 rows=4096 | current | current/macro-tiled-8 | 1428 | 666163.3 | 1.00 | 2097152 |
| bch-encode-batch | batch=1 degree=8 | current | current/PolyRemainderScalar | 387596 | 890.0 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=8 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 460829 | 886.3 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=8 | family-table-remainder | family-pinned/TableRemainder | 813008 | 565.2 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=8 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 305810 | 2484.8 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=8 | family-clmul-fold | family-pinned/ClmulFold | 892857 | 462.8 | 1.00 | 56 |
| bch-encode-batch | batch=16 degree=8 | current | current/PolyRemainderScalar | 38639 | 13075.8 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=8 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 44206 | 13205.8 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=8 | family-table-remainder | family-pinned/TableRemainder | 67704 | 8014.2 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=8 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 82644 | 8292.6 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=8 | family-clmul-fold | family-pinned/ClmulFold | 83535 | 6266.5 | 16.00 | 896 |
| bch-encode-batch | batch=64 degree=8 | current | current/PolyRemainderScalar | 8541 | 72412.3 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=8 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 8732 | 72399.8 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=8 | family-table-remainder | family-pinned/TableRemainder | 12799 | 35136.2 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=8 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 17241 | 26916.9 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=8 | family-clmul-fold | family-pinned/ClmulFold | 19164 | 26039.1 | 64.00 | 3584 |
| bch-encode-batch | batch=256 degree=8 | current | current/PolyRemainderScalar | 1762 | 490792.0 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=8 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 2077 | 486195.3 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=8 | family-table-remainder | family-pinned/TableRemainder | 2849 | 282097.3 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=8 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 3698 | 257693.1 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=8 | family-clmul-fold | family-pinned/ClmulFold | 4213 | 235996.8 | 256.00 | 14336 |
| bch-encode-batch | batch=1024 degree=8 | current | current/PolyRemainderScalar | 456 | 2120025.3 | 1024.00 | 57344 |
| bch-encode-batch | batch=1024 degree=8 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 418 | 2178082.3 | 1024.00 | 57344 |
| bch-encode-batch | batch=1024 degree=8 | family-table-remainder | family-pinned/TableRemainder | 592 | 1301004.8 | 1024.00 | 57344 |
| bch-encode-batch | batch=1024 degree=8 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 936 | 1161754.3 | 1024.00 | 57344 |
| bch-encode-batch | batch=1024 degree=8 | family-clmul-fold | family-pinned/ClmulFold | 977 | 1115032.6 | 1024.00 | 57344 |
| bch-encode-batch | batch=1 degree=14 | current | current/PolyRemainderScalar | 5766 | 151428.7 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=14 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 4616 | 152968.7 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=14 | family-table-remainder | family-pinned/TableRemainder | 12287 | 42853.9 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=14 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 2665 | 380574.4 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=14 | family-clmul-fold | family-pinned/ClmulFold | 15913 | 27465.3 | 1.00 | 56 |
| bch-encode-batch | batch=16 degree=14 | current | current/PolyRemainderScalar | 376 | 2888606.4 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=14 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 321 | 2899221.6 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=14 | family-table-remainder | family-pinned/TableRemainder | 804 | 1335777.1 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=14 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 756 | 1446617.7 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=14 | family-clmul-fold | family-pinned/ClmulFold | 994 | 1083683.2 | 16.00 | 896 |
| bch-encode-batch | batch=64 degree=14 | current | current/PolyRemainderScalar | 91 | 11565479.4 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=14 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 89 | 11514936.1 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=14 | family-table-remainder | family-pinned/TableRemainder | 175 | 5376260.5 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=14 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 215 | 4745998.2 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=14 | family-clmul-fold | family-pinned/ClmulFold | 244 | 4365636.7 | 64.00 | 3584 |
| bch-encode-batch | batch=256 degree=14 | current | current/PolyRemainderScalar | 21 | 45682868.6 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=14 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 22 | 45373448.1 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=14 | family-table-remainder | family-pinned/TableRemainder | 47 | 21403783.7 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=14 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 54 | 18832326.6 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=14 | family-clmul-fold | family-pinned/ClmulFold | 55 | 17439317.4 | 256.00 | 14336 |
| bch-encode-batch | batch=1024 degree=14 | current | current/PolyRemainderScalar | 5 | 184092690.4 | 1024.00 | 57344 |
| bch-encode-batch | batch=1024 degree=14 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 5 | 182723132.6 | 1024.00 | 57344 |
| bch-encode-batch | batch=1024 degree=14 | family-table-remainder | family-pinned/TableRemainder | 11 | 85945816.5 | 1024.00 | 57344 |
| bch-encode-batch | batch=1024 degree=14 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 12 | 76282322.7 | 1024.00 | 57344 |
| bch-encode-batch | batch=1024 degree=14 | family-clmul-fold | family-pinned/ClmulFold | 14 | 70288567.7 | 1024.00 | 57344 |
| bch-encode-batch | batch=1 degree=16 | current | current/PolyRemainderScalar | 1516 | 687441.0 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=16 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 1164 | 689618.2 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=16 | family-table-remainder | family-pinned/TableRemainder | 3006 | 302037.0 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=16 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 397 | 1853357.3 | 1.00 | 56 |
| bch-encode-batch | batch=1 degree=16 | family-clmul-fold | family-pinned/ClmulFold | 2811 | 240849.9 | 1.00 | 56 |
| bch-encode-batch | batch=16 degree=16 | current | current/PolyRemainderScalar | 86 | 11529960.0 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=16 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 92 | 11533641.4 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=16 | family-table-remainder | family-pinned/TableRemainder | 194 | 5411944.0 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=16 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 178 | 6081169.0 | 16.00 | 896 |
| bch-encode-batch | batch=16 degree=16 | family-clmul-fold | family-pinned/ClmulFold | 221 | 4489072.3 | 16.00 | 896 |
| bch-encode-batch | batch=64 degree=16 | current | current/PolyRemainderScalar | 21 | 46086395.6 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=16 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 21 | 46033012.9 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=16 | family-table-remainder | family-pinned/TableRemainder | 44 | 21238923.9 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=16 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 53 | 19087340.8 | 64.00 | 3584 |
| bch-encode-batch | batch=64 degree=16 | family-clmul-fold | family-pinned/ClmulFold | 57 | 17324942.9 | 64.00 | 3584 |
| bch-encode-batch | batch=256 degree=16 | current | current/PolyRemainderScalar | 5 | 183149909.0 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=16 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 5 | 183121586.8 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=16 | family-table-remainder | family-pinned/TableRemainder | 11 | 84918848.8 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=16 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 13 | 75886414.1 | 256.00 | 14336 |
| bch-encode-batch | batch=256 degree=16 | family-clmul-fold | family-pinned/ClmulFold | 14 | 69484976.8 | 256.00 | 14336 |
| bch-encode-batch-alloc | batch=16 degree=8 | current | current-allocating/PolyRemainderScalar | 45829 | 13310.3 | 33.00 | 2816 |
| bch-encode-batch-alloc | batch=256 degree=8 | current | current-allocating/PolyRemainderScalar | 1925 | 504405.5 | 513.00 | 45056 |
| bch-encode-batch-alloc | batch=16 degree=14 | current | current-allocating/PolyRemainderScalar | 310 | 2828916.5 | 33.00 | 35072 |
| bch-encode-batch-alloc | batch=256 degree=14 | current | current-allocating/PolyRemainderScalar | 22 | 45680471.2 | 513.00 | 561152 |
| bch-encode-batch-parallel | batch=256 degree=14 workers=1 | current | current-parallel/PolyRemainderScalar | 21 | 45518516.2 | 256.00 | 14336 |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=1 | current | current-parallel/PolyRemainderScalar | 5 | 182925777.6 | 1024.00 | 57344 |
| bch-encode-batch-parallel | batch=256 degree=14 workers=6 | current | current-parallel/PolyRemainderScalar | 84 | 13061601.8 | 256.01 | 14354 |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=6 | current | current-parallel/PolyRemainderScalar | 19 | 50884153.8 | 1024.00 | 57344 |
| dvb-bch-encode | batch=16 n=7200 | current | current/field-polynomial-div-rem | 9 | 204868773.7 | 272.00 | 12780288 |
| dvb-bch-encode | batch=1 n=32400 | current | current/field-polynomial-div-rem | 30 | 66762778.2 | 21.00 | 3645264 |

## Reported setup and conversion costs

Whole-consumer routes report the phases the receipt schema carries. `setup` is one-shot preparation the timed body does not repeat; the remaining columns are per timed call. A zero states that the route performs no such work.

| Workload | Size | Route | setup ns | pack ns | unpack ns | batch fill ns | dispatch ns |
|---|---|---|---:|---:|---:|---:|---:|
| dense-rref | cols=256 rows=256 | current | 5150 | 0 | 0 | 0 | 0 |
| dense-rref | cols=512 rows=512 | current | 23300 | 0 | 0 | 0 | 0 |
| dense-rref | cols=1024 rows=1024 | current | 118310 | 0 | 0 | 0 | 0 |
| dense-rref | cols=2048 rows=2048 | current | 435092 | 0 | 0 | 0 | 0 |
| dense-matvec | cols=256 rows=256 | current | 4820 | 0 | 201 | 0 | 0 |
| dense-matvec | cols=1024 rows=1024 | current | 90600 | 0 | 764 | 0 | 0 |
| dense-matvec | cols=4096 rows=1024 | current | 367532 | 0 | 900 | 0 | 0 |
| dense-matvec | cols=4096 rows=4096 | current | 1466708 | 0 | 3209 | 0 | 0 |
| ldpc-syndrome | n=16200 | current | 1583319 | 0 | 6625 | 0 | 0 |
| ldpc-syndrome | n=64800 | current | 960905 | 0 | 29907 | 0 | 0 |
| ldpc-codeword-check | n=16200 | count-ones | 2032372 | 0 | 15505 | 0 | 0 |
| ldpc-codeword-check | n=16200 | find-first-one | 1623729 | 0 | 6675 | 0 | 0 |
| ldpc-codeword-check | n=64800 | count-ones | 804684 | 0 | 23939 | 0 | 0 |
| ldpc-codeword-check | n=64800 | find-first-one | 808885 | 0 | 23701 | 0 | 0 |
| dense-transpose | cols=1024 rows=1024 | current | 90861 | 0 | 0 | 0 | 3 |
| dense-transpose | cols=4096 rows=4096 | current | 1330198 | 0 | 0 | 0 | 4 |
| bch-encode-batch | batch=1 degree=8 | current | 48741 | 5681 | 0 | 172 | 4 |
| bch-encode-batch | batch=1 degree=8 | family-poly-remainder-scalar | 48520 | 4800 | 0 | 137 | 3 |
| bch-encode-batch | batch=1 degree=8 | family-table-remainder | 39360 | 4510 | 0 | 130 | 3 |
| bch-encode-batch | batch=1 degree=8 | family-bitslice-interleaved | 47411 | 5630 | 0 | 180 | 4 |
| bch-encode-batch | batch=1 degree=8 | family-clmul-fold | 43281 | 4523 | 0 | 150 | 3 |
| bch-encode-batch | batch=16 degree=8 | current | 44650 | 5754 | 0 | 2022 | 4 |
| bch-encode-batch | batch=16 degree=8 | family-poly-remainder-scalar | 45721 | 4873 | 0 | 1737 | 4 |
| bch-encode-batch | batch=16 degree=8 | family-table-remainder | 45630 | 5790 | 0 | 2017 | 4 |
| bch-encode-batch | batch=16 degree=8 | family-bitslice-interleaved | 61800 | 4768 | 0 | 1652 | 3 |
| bch-encode-batch | batch=16 degree=8 | family-clmul-fold | 48351 | 5745 | 0 | 2032 | 4 |
| bch-encode-batch | batch=64 degree=8 | current | 50640 | 4981 | 0 | 8120 | 4 |
| bch-encode-batch | batch=64 degree=8 | family-poly-remainder-scalar | 36810 | 4670 | 0 | 7317 | 4 |
| bch-encode-batch | batch=64 degree=8 | family-table-remainder | 48880 | 6701 | 0 | 10872 | 4 |
| bch-encode-batch | batch=64 degree=8 | family-bitslice-interleaved | 40591 | 5001 | 0 | 7375 | 3 |
| bch-encode-batch | batch=64 degree=8 | family-clmul-fold | 37440 | 4706 | 0 | 7392 | 3 |
| bch-encode-batch | batch=256 degree=8 | current | 37101 | 4539 | 0 | 29132 | 3 |
| bch-encode-batch | batch=256 degree=8 | family-poly-remainder-scalar | 38050 | 4567 | 0 | 30707 | 3 |
| bch-encode-batch | batch=256 degree=8 | family-table-remainder | 45470 | 5639 | 0 | 37030 | 4 |
| bch-encode-batch | batch=256 degree=8 | family-bitslice-interleaved | 45260 | 5716 | 0 | 37565 | 4 |
| bch-encode-batch | batch=256 degree=8 | family-clmul-fold | 39560 | 4548 | 0 | 29275 | 3 |
| bch-encode-batch | batch=1024 degree=8 | current | 49870 | 4506 | 0 | 119155 | 3 |
| bch-encode-batch | batch=1024 degree=8 | family-poly-remainder-scalar | 40990 | 4563 | 0 | 120208 | 3 |
| bch-encode-batch | batch=1024 degree=8 | family-table-remainder | 38680 | 4530 | 0 | 126183 | 3 |
| bch-encode-batch | batch=1024 degree=8 | family-bitslice-interleaved | 38410 | 4531 | 0 | 120300 | 3 |
| bch-encode-batch | batch=1024 degree=8 | family-clmul-fold | 36490 | 39071 | 0 | 138348 | 3 |
| bch-encode-batch | batch=1 degree=14 | current | 1910021 | 6875 | 0 | 682 | 3 |
| bch-encode-batch | batch=1 degree=14 | family-poly-remainder-scalar | 1743339 | 8595 | 0 | 682 | 3 |
| bch-encode-batch | batch=1 degree=14 | family-table-remainder | 1670619 | 6820 | 0 | 692 | 3 |
| bch-encode-batch | batch=1 degree=14 | family-bitslice-interleaved | 1526009 | 6451 | 0 | 640 | 3 |
| bch-encode-batch | batch=1 degree=14 | family-clmul-fold | 1553869 | 6738 | 0 | 657 | 3 |
| bch-encode-batch | batch=16 degree=14 | current | 2688605 | 7176 | 0 | 9100 | 3 |
| bch-encode-batch | batch=16 degree=14 | family-poly-remainder-scalar | 1636569 | 25203 | 0 | 13472 | 3 |
| bch-encode-batch | batch=16 degree=14 | family-table-remainder | 1743340 | 6871 | 0 | 9702 | 3 |
| bch-encode-batch | batch=16 degree=14 | family-bitslice-interleaved | 1834680 | 7445 | 0 | 9080 | 3 |
| bch-encode-batch | batch=16 degree=14 | family-clmul-fold | 1524678 | 6452 | 0 | 9150 | 3 |
| bch-encode-batch | batch=64 degree=14 | current | 1519779 | 6401 | 0 | 37272 | 3 |
| bch-encode-batch | batch=64 degree=14 | family-poly-remainder-scalar | 1510858 | 6450 | 0 | 37205 | 3 |
| bch-encode-batch | batch=64 degree=14 | family-table-remainder | 1517959 | 6440 | 0 | 37105 | 3 |
| bch-encode-batch | batch=64 degree=14 | family-bitslice-interleaved | 1500879 | 6408 | 0 | 37365 | 3 |
| bch-encode-batch | batch=64 degree=14 | family-clmul-fold | 1496159 | 6406 | 0 | 37352 | 3 |
| bch-encode-batch | batch=256 degree=14 | current | 1640739 | 6609 | 0 | 425502 | 3 |
| bch-encode-batch | batch=256 degree=14 | family-poly-remainder-scalar | 1502659 | 6426 | 0 | 305784 | 3 |
| bch-encode-batch | batch=256 degree=14 | family-table-remainder | 1883111 | 6415 | 0 | 315834 | 3 |
| bch-encode-batch | batch=256 degree=14 | family-bitslice-interleaved | 1498648 | 6404 | 0 | 304606 | 3 |
| bch-encode-batch | batch=256 degree=14 | family-clmul-fold | 1492018 | 6415 | 0 | 306989 | 3 |
| bch-encode-batch | batch=1024 degree=14 | current | 1787080 | 6838 | 0 | 1455515 | 3 |
| bch-encode-batch | batch=1024 degree=14 | family-poly-remainder-scalar | 1613519 | 6430 | 0 | 1361225 | 3 |
| bch-encode-batch | batch=1024 degree=14 | family-table-remainder | 1796200 | 6553 | 0 | 1417558 | 3 |
| bch-encode-batch | batch=1024 degree=14 | family-bitslice-interleaved | 1714339 | 6456 | 0 | 1340245 | 3 |
| bch-encode-batch | batch=1024 degree=14 | family-clmul-fold | 1497009 | 6388 | 0 | 1737052 | 3 |
| bch-encode-batch | batch=1 degree=16 | current | 6630077 | 6491 | 0 | 2152 | 3 |
| bch-encode-batch | batch=1 degree=16 | family-poly-remainder-scalar | 6123934 | 6524 | 0 | 2142 | 3 |
| bch-encode-batch | batch=1 degree=16 | family-table-remainder | 6575707 | 6472 | 0 | 2155 | 3 |
| bch-encode-batch | batch=1 degree=16 | family-bitslice-interleaved | 6697428 | 6555 | 0 | 2180 | 3 |
| bch-encode-batch | batch=1 degree=16 | family-clmul-fold | 6384996 | 8111 | 0 | 2672 | 4 |
| bch-encode-batch | batch=16 degree=16 | current | 7657403 | 6515 | 0 | 33362 | 3 |
| bch-encode-batch | batch=16 degree=16 | family-poly-remainder-scalar | 6667348 | 6583 | 0 | 33367 | 3 |
| bch-encode-batch | batch=16 degree=16 | family-table-remainder | 6798168 | 6510 | 0 | 33325 | 3 |
| bch-encode-batch | batch=16 degree=16 | family-bitslice-interleaved | 6457357 | 6781 | 0 | 33387 | 3 |
| bch-encode-batch | batch=16 degree=16 | family-clmul-fold | 7434942 | 6510 | 0 | 33347 | 3 |
| bch-encode-batch | batch=64 degree=16 | current | 6626028 | 6503 | 0 | 135065 | 3 |
| bch-encode-batch | batch=64 degree=16 | family-poly-remainder-scalar | 6605597 | 6563 | 0 | 133270 | 3 |
| bch-encode-batch | batch=64 degree=16 | family-table-remainder | 8440788 | 8110 | 0 | 165886 | 4 |
| bch-encode-batch | batch=64 degree=16 | family-bitslice-interleaved | 6076124 | 6680 | 0 | 133443 | 3 |
| bch-encode-batch | batch=64 degree=16 | family-clmul-fold | 6024884 | 6475 | 0 | 156683 | 3 |
| bch-encode-batch | batch=256 degree=16 | current | 7073290 | 6525 | 0 | 1287669 | 3 |
| bch-encode-batch | batch=256 degree=16 | family-poly-remainder-scalar | 6840358 | 6496 | 0 | 1519271 | 3 |
| bch-encode-batch | batch=256 degree=16 | family-table-remainder | 6085794 | 6520 | 0 | 1317480 | 3 |
| bch-encode-batch | batch=256 degree=16 | family-bitslice-interleaved | 7449802 | 6483 | 0 | 1348107 | 3 |
| bch-encode-batch | batch=256 degree=16 | family-clmul-fold | 6424836 | 6485 | 0 | 1275719 | 4 |
| bch-encode-batch-alloc | batch=16 degree=8 | current | 59790 | 4609 | 0 | 1632 | 3 |
| bch-encode-batch-alloc | batch=256 degree=8 | current | 46710 | 4578 | 0 | 34312 | 3 |
| bch-encode-batch-alloc | batch=16 degree=14 | current | 1500388 | 6413 | 0 | 9145 | 3 |
| bch-encode-batch-alloc | batch=256 degree=14 | current | 1499628 | 6365 | 0 | 320967 | 3 |
| bch-encode-batch-parallel | batch=256 degree=14 workers=1 | current | 1868431 | 6411 | 0 | 334374 | 3 |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=1 | current | 1874421 | 6435 | 0 | 1323680 | 3 |
| bch-encode-batch-parallel | batch=256 degree=14 workers=6 | current | 1627199 | 6390 | 0 | 441800 | 3 |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=6 | current | 1830430 | 6442 | 0 | 1545513 | 3 |
| dvb-bch-encode | batch=16 n=7200 | current | 970816 | 0 | 0 | 5892 | 0 |
| dvb-bch-encode | batch=1 n=32400 | current | 4221675 | 0 | 0 | 1440 | 0 |
