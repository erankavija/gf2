# Current-code consumer profile sweep

Projection of `rep-*/profile.jsonl`: n = 9 profile sessions (rep-01, rep-02, rep-03, rep-04, rep-05, rep-06, rep-07, rep-08, rep-09), each one `sweep.sh` run under its own `dev/scripts/ccx1-bench-flock.sh --full-host` invocation, over 180 routes. Within a session each route is one process, and its `ns/call` is the wall-clock mean over the calls that process made.

Each `ns/call` below is the median over the n = 9 sessions with the distribution-free order-statistic interval for the median: for n independent sessions the count below the population median is Binomial(n, 1/2), so [x(j), x(n+1-j)] covers it with probability 1 - 2 P(Binomial(n, 1/2) <= j - 1), here 96.1% (`dev/active/04b85d10/survey/intervals.py`). Calls per session are descriptive. Allocation counters are exact counts from the counting allocator over the timed calls; one value means every session reported it.

Fixture generator, named by every seeded record's `fixture_rng` beside its `seed`: gf2_core BitVec::random_seeded and BitMatrix::random_seeded: rand::rngs::StdRng::seed_from_u64(seed), then Rng::fill over u64 words; per-row, per-block and per-bank seeds are the case-seed offsets `prepare` defines. Algorithm: ChaCha12 (rand 0.8 StdRng is rand_chacha::ChaCha12Rng); seed_from_u64 expands the u64 seed to the 32-byte key with PCG32 (rand_core 0.6). Resolved versions: rand 0.8.8, rand_chacha 0.3.1, rand_core 0.6.4. A seed marked unused belongs to a deterministic fixture that draws nothing.

BCH rows: degree 14 is the mother field of the DVB-T2 short-frame BCH code and degree 16 that of the normal-frame code [Etsi2015] (rows T2S and T2N of `dev/active/4e732b56/workload-selection.md`); degree 8 is its row B3. `dvb-bch-encode` measures the shortened rate-1/2 frames themselves, n = 7200 short and n = 32400 normal.

## Logical row, parity and dense-matrix consumers

| Workload | Size | Route | Observed path | Seed (generator) | Calls/session (median) | ns/call, median [interval], n = 9 | Allocations/call | Bytes/call | Peak live bytes |
|---|---|---|---|---|---:|---:|---:|---:|---:|
| row-xor | rows=64 words=4 | ops-dispatched | ops-dispatched/scalar | 101 (rand 0.8.8 StdRng) | 8130081 | 106 [105, 108] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=4 | ops-resolved | ops-resolved/scalar | 101 (rand 0.8.8 StdRng) | 9009009 | 100 [99.9, 101] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=4 | scalar-backend | scalar-backend | 101 (rand 0.8.8 StdRng) | 9090909 | 98.9 [98.7, 99.6] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=4 | simd-backend | simd-backend/avx2 | 101 (rand 0.8.8 StdRng) | 11494252 | 77.7 [77.6, 78.8] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=8 | ops-dispatched | ops-dispatched/simd | 101 (rand 0.8.8 StdRng) | 8695652 | 104 [104, 104] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=8 | ops-resolved | ops-resolved/simd | 101 (rand 0.8.8 StdRng) | 9259259 | 95.2 [94.7, 98.8] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=8 | scalar-backend | scalar-backend | 101 (rand 0.8.8 StdRng) | 5988023 | 152 [151, 152] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=8 | simd-backend | simd-backend/avx2 | 101 (rand 0.8.8 StdRng) | 9433962 | 96.0 [95.5, 96.3] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=16 | ops-dispatched | ops-dispatched/simd | 101 (rand 0.8.8 StdRng) | 7194244 | 125 [125, 132] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=16 | ops-resolved | ops-resolved/simd | 101 (rand 0.8.8 StdRng) | 7633587 | 117 [116, 150] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=16 | scalar-backend | scalar-backend | 101 (rand 0.8.8 StdRng) | 3508771 | 250 [250, 259] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=16 | simd-backend | simd-backend/avx2 | 101 (rand 0.8.8 StdRng) | 7575757 | 118 [117, 119] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=64 | ops-dispatched | ops-dispatched/simd | 101 (rand 0.8.8 StdRng) | 2617801 | 340 [338, 342] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=64 | ops-resolved | ops-resolved/simd | 101 (rand 0.8.8 StdRng) | 2816901 | 318 [312, 320] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=64 | scalar-backend | scalar-backend | 101 (rand 0.8.8 StdRng) | 1328021 | 665 [665, 670] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=64 | simd-backend | simd-backend/avx2 | 101 (rand 0.8.8 StdRng) | 2770083 | 322 [320, 326] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=512 | ops-dispatched | ops-dispatched/simd | 101 (rand 0.8.8 StdRng) | 424808 | 2082 [2075, 2140] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=512 | ops-resolved | ops-resolved/simd | 101 (rand 0.8.8 StdRng) | 423728 | 2074 [2071, 2100] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=512 | scalar-backend | scalar-backend | 101 (rand 0.8.8 StdRng) | 186323 | 4751 [4665, 4806] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=512 | simd-backend | simd-backend/avx2 | 101 (rand 0.8.8 StdRng) | 425531 | 2079 [2072, 2122] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=8192 | ops-dispatched | ops-dispatched/simd | 101 (rand 0.8.8 StdRng) | 28446 | 34058 [33787, 34238] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=8192 | ops-resolved | ops-resolved/simd | 101 (rand 0.8.8 StdRng) | 27499 | 34089 [33896, 34447] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=8192 | scalar-backend | scalar-backend | 101 (rand 0.8.8 StdRng) | 13192 | 72822 [71152, 74152] | 0.00 | 0 | 0 |
| row-xor | rows=64 words=8192 | simd-backend | simd-backend/avx2 | 101 (rand 0.8.8 StdRng) | 28278 | 34343 [34055, 34848] | 0.00 | 0 | 0 |
| dense-rref | cols=256 rows=256 | current | current/blocked-m4ri | 102 (rand 0.8.8 StdRng) | 15705 | 56298 [55944, 56650] | 137.00 | 36832 | 13344 |
| dense-rref | cols=512 rows=512 | current | current/blocked-m4ri | 102 (rand 0.8.8 StdRng) | 3846 | 227848 [226981, 228991] | 266.00 | 122784 | 43040 |
| dense-rref | cols=1024 rows=1024 | current | current/blocked-m4ri | 102 (rand 0.8.8 StdRng) | 1141 | 773923 [769129, 778232] | 267.00 | 2391008 | 172608 |
| dense-rref | cols=2048 rows=2048 | current | current/blocked-m4ri | 102 (rand 0.8.8 StdRng) | 258 | 3722119 [3682886, 3731430] | 524.00 | 9239520 | 606784 |
| dense-matvec | cols=256 rows=256 | current | current/scalar-row-parity | 103 (rand 0.8.8 StdRng) | 1157407 | 764 [761, 766] | 1.00 | 32 | 32 |
| dense-matvec | cols=1024 rows=1024 | current | current/simd-and-popcnt | 103 (rand 0.8.8 StdRng) | 201491 | 4302 [4261, 4353] | 1.00 | 128 | 128 |
| dense-matvec | cols=4096 rows=1024 | current | current/simd-and-popcnt | 103 (rand 0.8.8 StdRng) | 56650 | 15821 [15641, 16084] | 1.00 | 128 | 128 |
| dense-matvec | cols=4096 rows=4096 | current | current/simd-and-popcnt | 103 (rand 0.8.8 StdRng) | 13988 | 67230 [66970, 69201] | 1.00 | 512 | 512 |
| ldpc-syndrome | n=16200 | current | current/csr-bit-at-a-time-matvec | 104 (rand 0.8.8 StdRng) | 18828 | 52163 [51249, 52830] | 1.00 | 1128 | 1128 |
| ldpc-syndrome | n=64800 | current | current/csr-bit-at-a-time-matvec | 104 (rand 0.8.8 StdRng) | 3825 | 260479 [258511, 261365] | 1.00 | 4056 | 4056 |

## Count and fused-reduction consumers

| Workload | Size | Route | Observed path | Seed (generator) | Calls/session (median) | ns/call, median [interval], n = 9 | Allocations/call | Bytes/call | Peak live bytes |
|---|---|---|---|---|---:|---:|---:|---:|---:|
| popcount | words=4 | ops-dispatched | ops-dispatched/scalar | 201 (rand 0.8.8 StdRng) | 142857142 | 6.88 [6.85, 6.90] | 0.00 | 0 | 0 |
| popcount | words=4 | scalar-backend | scalar-backend | 201 (rand 0.8.8 StdRng) | 200000000 | 4.78 [4.75, 4.87] | 0.00 | 0 | 0 |
| popcount | words=4 | simd-backend | simd-backend/avx2 | 201 (rand 0.8.8 StdRng) | 250000000 | 4.38 [4.37, 4.60] | 0.00 | 0 | 0 |
| popcount | words=8 | ops-dispatched | ops-dispatched/simd | 201 (rand 0.8.8 StdRng) | 200000000 | 5.21 [5.20, 5.27] | 0.00 | 0 | 0 |
| popcount | words=8 | scalar-backend | scalar-backend | 201 (rand 0.8.8 StdRng) | 166666666 | 5.94 [5.93, 5.96] | 0.00 | 0 | 0 |
| popcount | words=8 | simd-backend | simd-backend/avx2 | 201 (rand 0.8.8 StdRng) | 200000000 | 5.00 [4.99, 5.04] | 0.00 | 0 | 0 |
| popcount | words=16 | ops-dispatched | ops-dispatched/simd | 201 (rand 0.8.8 StdRng) | 166666666 | 6.04 [6.03, 6.06] | 0.00 | 0 | 0 |
| popcount | words=16 | scalar-backend | scalar-backend | 201 (rand 0.8.8 StdRng) | 111111111 | 8.56 [8.54, 8.60] | 0.00 | 0 | 0 |
| popcount | words=16 | simd-backend | simd-backend/avx2 | 201 (rand 0.8.8 StdRng) | 166666666 | 5.89 [5.83, 6.07] | 0.00 | 0 | 0 |
| popcount | words=64 | ops-dispatched | ops-dispatched/simd | 201 (rand 0.8.8 StdRng) | 71428571 | 12.7 [12.6, 12.7] | 0.00 | 0 | 0 |
| popcount | words=64 | scalar-backend | scalar-backend | 201 (rand 0.8.8 StdRng) | 31250000 | 27.8 [27.7, 27.9] | 0.00 | 0 | 0 |
| popcount | words=64 | simd-backend | simd-backend/avx2 | 201 (rand 0.8.8 StdRng) | 71428571 | 12.4 [12.4, 12.5] | 0.00 | 0 | 0 |
| popcount | words=127 | ops-dispatched | ops-dispatched/simd | 201 (rand 0.8.8 StdRng) | 40000000 | 22.4 [22.3, 22.5] | 0.00 | 0 | 0 |
| popcount | words=127 | scalar-backend | scalar-backend | 201 (rand 0.8.8 StdRng) | 16393442 | 53.3 [53.2, 53.7] | 0.00 | 0 | 0 |
| popcount | words=127 | simd-backend | simd-backend/avx2 | 201 (rand 0.8.8 StdRng) | 40000000 | 22.4 [22.3, 22.5] | 0.00 | 0 | 0 |
| popcount | words=507 | ops-dispatched | ops-dispatched/simd | 201 (rand 0.8.8 StdRng) | 13157894 | 66.9 [66.7, 67.1] | 0.00 | 0 | 0 |
| popcount | words=507 | scalar-backend | scalar-backend | 201 (rand 0.8.8 StdRng) | 4444444 | 199 [197, 199] | 0.00 | 0 | 0 |
| popcount | words=507 | simd-backend | simd-backend/avx2 | 201 (rand 0.8.8 StdRng) | 12987012 | 67.1 [66.7, 67.3] | 0.00 | 0 | 0 |
| popcount | words=4096 | ops-dispatched | ops-dispatched/simd | 201 (rand 0.8.8 StdRng) | 1769911 | 495 [493, 498] | 0.00 | 0 | 0 |
| popcount | words=4096 | scalar-backend | scalar-backend | 201 (rand 0.8.8 StdRng) | 565930 | 1538 [1529, 1551] | 0.00 | 0 | 0 |
| popcount | words=4096 | simd-backend | simd-backend/avx2 | 201 (rand 0.8.8 StdRng) | 1760563 | 495 [492, 499] | 0.00 | 0 | 0 |
| popcount | words=65536 | ops-dispatched | ops-dispatched/simd | 201 (rand 0.8.8 StdRng) | 110595 | 7907 [7820, 8077] | 0.00 | 0 | 0 |
| popcount | words=65536 | scalar-backend | scalar-backend | 201 (rand 0.8.8 StdRng) | 34862 | 25103 [24249, 25413] | 0.00 | 0 | 0 |
| popcount | words=65536 | simd-backend | simd-backend/avx2 | 201 (rand 0.8.8 StdRng) | 109373 | 8036 [7833, 8259] | 0.00 | 0 | 0 |
| zero-test | set_bit=4096 words=64 | count-ones | count-ones/simd | 202 (unused) | 71428571 | 13.0 [13.0, 13.0] | 0.00 | 0 | 0 |
| zero-test | set_bit=4096 words=64 | find-first-one | find-first-one/simd | 202 (unused) | 83333333 | 10.8 [10.8, 10.9] | 0.00 | 0 | 0 |
| zero-test | set_bit=0 words=64 | count-ones | count-ones/simd | 202 (unused) | 71428571 | 13.0 [13.0, 13.1] | 0.00 | 0 | 0 |
| zero-test | set_bit=0 words=64 | find-first-one | find-first-one/simd | 202 (unused) | 200000000 | 4.59 [4.58, 4.60] | 0.00 | 0 | 0 |
| zero-test | set_bit=8128 words=127 | count-ones | count-ones/simd | 202 (unused) | 38461538 | 22.9 [22.7, 23.0] | 0.00 | 0 | 0 |
| zero-test | set_bit=8128 words=127 | find-first-one | find-first-one/simd | 202 (unused) | 47619047 | 18.2 [18.2, 18.3] | 0.00 | 0 | 0 |
| zero-test | set_bit=0 words=127 | count-ones | count-ones/simd | 202 (unused) | 38461538 | 22.9 [22.7, 23.0] | 0.00 | 0 | 0 |
| zero-test | set_bit=0 words=127 | find-first-one | find-first-one/simd | 202 (unused) | 200000000 | 4.61 [4.58, 4.63] | 0.00 | 0 | 0 |
| zero-test | set_bit=32448 words=507 | count-ones | count-ones/simd | 202 (unused) | 12987012 | 67.5 [67.4, 67.8] | 0.00 | 0 | 0 |
| zero-test | set_bit=32448 words=507 | find-first-one | find-first-one/simd | 202 (unused) | 14492753 | 60.6 [60.4, 61.1] | 0.00 | 0 | 0 |
| zero-test | set_bit=0 words=507 | count-ones | count-ones/simd | 202 (unused) | 12987012 | 67.4 [67.1, 68.2] | 0.00 | 0 | 0 |
| zero-test | set_bit=0 words=507 | find-first-one | find-first-one/simd | 202 (unused) | 200000000 | 4.59 [4.59, 4.60] | 0.00 | 0 | 0 |
| zero-test | set_bit=262144 words=4096 | count-ones | count-ones/simd | 202 (unused) | 1751313 | 493 [492, 496] | 0.00 | 0 | 0 |
| zero-test | set_bit=262144 words=4096 | find-first-one | find-first-one/simd | 202 (unused) | 2012072 | 436 [435, 440] | 0.00 | 0 | 0 |
| zero-test | set_bit=0 words=4096 | count-ones | count-ones/simd | 202 (unused) | 1757469 | 492 [490, 494] | 0.00 | 0 | 0 |
| zero-test | set_bit=0 words=4096 | find-first-one | find-first-one/simd | 202 (unused) | 200000000 | 4.59 [4.58, 4.60] | 0.00 | 0 | 0 |
| ldpc-codeword-check | n=16200 | count-ones | syndrome-then-count-ones | 203 (rand 0.8.8 StdRng) | 17266 | 57410 [53720, 58600] | 1.00 | 1128 | 1128 |
| ldpc-codeword-check | n=16200 | find-first-one | syndrome-then-find-first-one | 203 (rand 0.8.8 StdRng) | 17721 | 56526 [54538, 57972] | 1.00 | 1128 | 1128 |
| ldpc-codeword-check | n=64800 | count-ones | syndrome-then-count-ones | 203 (rand 0.8.8 StdRng) | 3646 | 260863 [258800, 262867] | 1.00 | 4056 | 4056 |
| ldpc-codeword-check | n=64800 | find-first-one | syndrome-then-find-first-one | 203 (rand 0.8.8 StdRng) | 3779 | 259027 [258133, 261965] | 1.00 | 4056 | 4056 |

## Transpose, bitslice and BCH encoding consumers

| Workload | Size | Route | Observed path | Seed (generator) | Calls/session (median) | ns/call, median [interval], n = 9 | Allocations/call | Bytes/call | Peak live bytes |
|---|---|---|---|---|---:|---:|---:|---:|---:|
| transpose-64x64 | blocks=1 | transpose-scalar | transpose-scalar/portable | 301 (rand 0.8.8 StdRng) | 10000000 | 84.4 [84.2, 85.0] | 0.00 | 0 | 0 |
| transpose-64x64 | blocks=1 | transpose-detected | transpose-detected/avx2-bit-twiddle | 301 (rand 0.8.8 StdRng) | 19607843 | 44.9 [44.8, 46.3] | 0.00 | 0 | 0 |
| transpose-64x64 | blocks=16 | transpose-scalar | transpose-scalar/portable | 301 (rand 0.8.8 StdRng) | 658761 | 1329 [1305, 1343] | 0.00 | 0 | 0 |
| transpose-64x64 | blocks=16 | transpose-detected | transpose-detected/avx2-bit-twiddle | 301 (rand 0.8.8 StdRng) | 1310615 | 664 [660, 787] | 0.00 | 0 | 0 |
| transpose-64x64 | blocks=256 | transpose-scalar | transpose-scalar/portable | 301 (rand 0.8.8 StdRng) | 41042 | 21617 [21474, 21912] | 0.00 | 0 | 0 |
| transpose-64x64 | blocks=256 | transpose-detected | transpose-detected/avx2-bit-twiddle | 301 (rand 0.8.8 StdRng) | 77309 | 11494 [11270, 13538] | 0.00 | 0 | 0 |
| transpose-64x64 | blocks=4096 | transpose-scalar | transpose-scalar/portable | 301 (rand 0.8.8 StdRng) | 2434 | 366634 [365555, 369524] | 0.00 | 0 | 0 |
| transpose-64x64 | blocks=4096 | transpose-detected | transpose-detected/avx2-bit-twiddle | 301 (rand 0.8.8 StdRng) | 4877 | 184080 [182154, 187639] | 0.00 | 0 | 0 |
| dense-transpose | cols=1024 rows=1024 | current | current/simple | 302 (rand 0.8.8 StdRng) | 31217 | 27249 [27027, 28858] | 1.00 | 131072 | 131072 |
| dense-transpose | cols=4096 rows=4096 | current | current/macro-tiled-8 | 302 (rand 0.8.8 StdRng) | 1590 | 585460 [582056, 602612] | 1.00 | 2097152 | 2097152 |
| bch-encode-batch | batch=1 degree=8 | current | current/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 1136363 | 777 [777, 782] | 1.00 | 56 | 56 |
| bch-encode-batch | batch=1 degree=8 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 1131221 | 772 [771, 777] | 1.00 | 56 | 56 |
| bch-encode-batch | batch=1 degree=8 | family-table-remainder | family-pinned/TableRemainder | 303 (rand 0.8.8 StdRng) | 1792114 | 491 [489, 495] | 1.00 | 56 | 56 |
| bch-encode-batch | batch=1 degree=8 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 303 (rand 0.8.8 StdRng) | 412711 | 2133 [2104, 2207] | 1.00 | 56 | 56 |
| bch-encode-batch | batch=1 degree=8 | family-clmul-fold | family-pinned/ClmulFold | 303 (rand 0.8.8 StdRng) | 2298850 | 386 [384, 389] | 1.00 | 56 | 56 |
| bch-encode-batch | batch=16 degree=8 | current | current/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 73195 | 11725 [11646, 11988] | 16.00 | 896 | 56 |
| bch-encode-batch | batch=16 degree=8 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 73529 | 11753 [11729, 11999] | 16.00 | 896 | 56 |
| bch-encode-batch | batch=16 degree=8 | family-table-remainder | family-pinned/TableRemainder | 303 (rand 0.8.8 StdRng) | 126855 | 6915 [6892, 6930] | 16.00 | 896 | 56 |
| bch-encode-batch | batch=16 degree=8 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 303 (rand 0.8.8 StdRng) | 123213 | 7149 [7118, 7179] | 16.00 | 896 | 56 |
| bch-encode-batch | batch=16 degree=8 | family-clmul-fold | family-pinned/ClmulFold | 303 (rand 0.8.8 StdRng) | 161498 | 5457 [5447, 5474] | 16.00 | 896 | 56 |
| bch-encode-batch | batch=64 degree=8 | current | current/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 9886 | 83358 [82416, 87466] | 64.00 | 3584 | 56 |
| bch-encode-batch | batch=64 degree=8 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 9946 | 82955 [82187, 82998] | 64.00 | 3584 | 56 |
| bch-encode-batch | batch=64 degree=8 | family-table-remainder | family-pinned/TableRemainder | 303 (rand 0.8.8 StdRng) | 28388 | 38575 [33600, 43290] | 64.00 | 3584 | 56 |
| bch-encode-batch | batch=64 degree=8 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 303 (rand 0.8.8 StdRng) | 38121 | 23259 [23151, 23423] | 64.00 | 3584 | 56 |
| bch-encode-batch | batch=64 degree=8 | family-clmul-fold | family-pinned/ClmulFold | 303 (rand 0.8.8 StdRng) | 38918 | 28732 [22947, 36472] | 64.00 | 3584 | 56 |
| bch-encode-batch | batch=256 degree=8 | current | current/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 2054 | 432945 [431606, 433699] | 256.00 | 14336 | 56 |
| bch-encode-batch | batch=256 degree=8 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 1967 | 431122 [430191, 433812] | 256.00 | 14336 | 56 |
| bch-encode-batch | batch=256 degree=8 | family-table-remainder | family-pinned/TableRemainder | 303 (rand 0.8.8 StdRng) | 3519 | 243542 [243102, 254602] | 256.00 | 14336 | 56 |
| bch-encode-batch | batch=256 degree=8 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 303 (rand 0.8.8 StdRng) | 3891 | 222067 [220602, 224607] | 256.00 | 14336 | 56 |
| bch-encode-batch | batch=256 degree=8 | family-clmul-fold | family-pinned/ClmulFold | 303 (rand 0.8.8 StdRng) | 4298 | 205655 [204334, 206807] | 256.00 | 14336 | 56 |
| bch-encode-batch | batch=1024 degree=8 | current | current/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 487 | 1847481 [1845648, 1859052] | 1024.00 | 57344 | 56 |
| bch-encode-batch | batch=1024 degree=8 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 499 | 1846152 [1841478, 1866122] | 1024.00 | 57344 | 56 |
| bch-encode-batch | batch=1024 degree=8 | family-table-remainder | family-pinned/TableRemainder | 303 (rand 0.8.8 StdRng) | 784 | 1099205 [1098156, 1102621] | 1024.00 | 57344 | 56 |
| bch-encode-batch | batch=1024 degree=8 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 303 (rand 0.8.8 StdRng) | 895 | 967997 [966269, 974257] | 1024.00 | 57344 | 56 |
| bch-encode-batch | batch=1024 degree=8 | family-clmul-fold | family-pinned/ClmulFold | 303 (rand 0.8.8 StdRng) | 913 | 944775 [941372, 948004] | 1024.00 | 57344 | 56 |
| bch-encode-batch | batch=1 degree=14 | current | current/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 7379 | 130730 [130241, 131006] | 1.00 | 56 | 56 |
| bch-encode-batch | batch=1 degree=14 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 7350 | 130634 [130519, 131336] | 1.00 | 56 | 56 |
| bch-encode-batch | batch=1 degree=14 | family-table-remainder | family-pinned/TableRemainder | 303 (rand 0.8.8 StdRng) | 25399 | 36407 [35464, 36686] | 1.00 | 56 | 56 |
| bch-encode-batch | batch=1 degree=14 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 303 (rand 0.8.8 StdRng) | 2900 | 315990 [315485, 317252] | 1.00 | 56 | 56 |
| bch-encode-batch | batch=1 degree=14 | family-clmul-fold | family-pinned/ClmulFold | 303 (rand 0.8.8 StdRng) | 34192 | 23068 [22572, 23530] | 1.00 | 56 | 56 |
| bch-encode-batch | batch=16 degree=14 | current | current/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 377 | 2479090 [2474573, 2483661] | 16.00 | 896 | 56 |
| bch-encode-batch | batch=16 degree=14 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 374 | 2476095 [2473011, 2485562] | 16.00 | 896 | 56 |
| bch-encode-batch | batch=16 degree=14 | family-table-remainder | family-pinned/TableRemainder | 303 (rand 0.8.8 StdRng) | 839 | 1134155 [1132276, 1138420] | 16.00 | 896 | 56 |
| bch-encode-batch | batch=16 degree=14 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 303 (rand 0.8.8 StdRng) | 782 | 1213123 [1210317, 1225059] | 16.00 | 896 | 56 |
| bch-encode-batch | batch=16 degree=14 | family-clmul-fold | family-pinned/ClmulFold | 303 (rand 0.8.8 StdRng) | 1008 | 927450 [925040, 928795] | 16.00 | 896 | 56 |
| bch-encode-batch | batch=64 degree=14 | current | current/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 95 | 9965487 [9938645, 9979415] | 64.00 | 3584 | 56 |
| bch-encode-batch | batch=64 degree=14 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 97 | 9953699 [9931769, 9983690] | 64.00 | 3584 | 56 |
| bch-encode-batch | batch=64 degree=14 | family-table-remainder | family-pinned/TableRemainder | 303 (rand 0.8.8 StdRng) | 209 | 4576874 [4562869, 4585879] | 64.00 | 3584 | 56 |
| bch-encode-batch | batch=64 degree=14 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 303 (rand 0.8.8 StdRng) | 234 | 3999557 [3994897, 4015276] | 64.00 | 3584 | 56 |
| bch-encode-batch | batch=64 degree=14 | family-clmul-fold | family-pinned/ClmulFold | 303 (rand 0.8.8 StdRng) | 253 | 3753432 [3735160, 3755883] | 64.00 | 3584 | 56 |
| bch-encode-batch | batch=256 degree=14 | current | current/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 25 | 39821904 [39737220, 40016459] | 256.00 | 14336 | 56 |
| bch-encode-batch | batch=256 degree=14 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 24 | 39733868 [39706536, 39852502] | 256.00 | 14336 | 56 |
| bch-encode-batch | batch=256 degree=14 | family-table-remainder | family-pinned/TableRemainder | 303 (rand 0.8.8 StdRng) | 53 | 18245429 [18229525, 18359258] | 256.00 | 14336 | 56 |
| bch-encode-batch | batch=256 degree=14 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 303 (rand 0.8.8 StdRng) | 60 | 15913119 [15901510, 15953432] | 256.00 | 14336 | 56 |
| bch-encode-batch | batch=256 degree=14 | family-clmul-fold | family-pinned/ClmulFold | 303 (rand 0.8.8 StdRng) | 63 | 14978902 [14935525, 15039770] | 256.00 | 14336 | 56 |
| bch-encode-batch | batch=1024 degree=14 | current | current/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 6 | 158920487 [158872084, 159413499] | 1024.00 | 57344 | 56 |
| bch-encode-batch | batch=1024 degree=14 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 6 | 159247980 [158822297, 159648200] | 1024.00 | 57344 | 56 |
| bch-encode-batch | batch=1024 degree=14 | family-table-remainder | family-pinned/TableRemainder | 303 (rand 0.8.8 StdRng) | 13 | 73031695 [72946330, 73202770] | 1024.00 | 57344 | 56 |
| bch-encode-batch | batch=1024 degree=14 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 303 (rand 0.8.8 StdRng) | 15 | 64031386 [63863441, 64213508] | 1024.00 | 57344 | 56 |
| bch-encode-batch | batch=1024 degree=14 | family-clmul-fold | family-pinned/ClmulFold | 303 (rand 0.8.8 StdRng) | 16 | 59852990 [59713495, 59901565] | 1024.00 | 57344 | 56 |
| bch-encode-batch | batch=1 degree=16 | current | current/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 1569 | 597236 [594863, 603987] | 1.00 | 56 | 56 |
| bch-encode-batch | batch=1 degree=16 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 1664 | 596225 [594835, 597861] | 1.00 | 56 | 56 |
| bch-encode-batch | batch=1 degree=16 | family-table-remainder | family-pinned/TableRemainder | 303 (rand 0.8.8 StdRng) | 3721 | 255362 [254999, 256217] | 1.00 | 56 | 56 |
| bch-encode-batch | batch=1 degree=16 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 303 (rand 0.8.8 StdRng) | 629 | 1570519 [1568620, 1587272] | 1.00 | 56 | 56 |
| bch-encode-batch | batch=1 degree=16 | family-clmul-fold | family-pinned/ClmulFold | 303 (rand 0.8.8 StdRng) | 4797 | 203539 [202824, 203941] | 1.00 | 56 | 56 |
| bch-encode-batch | batch=16 degree=16 | current | current/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 95 | 10035802 [10003178, 10063305] | 16.00 | 896 | 56 |
| bch-encode-batch | batch=16 degree=16 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 98 | 10032585 [9997105, 10065477] | 16.00 | 896 | 56 |
| bch-encode-batch | batch=16 degree=16 | family-table-remainder | family-pinned/TableRemainder | 303 (rand 0.8.8 StdRng) | 215 | 4577117 [4570409, 4600096] | 16.00 | 896 | 56 |
| bch-encode-batch | batch=16 degree=16 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 303 (rand 0.8.8 StdRng) | 196 | 5057481 [5044676, 5093838] | 16.00 | 896 | 56 |
| bch-encode-batch | batch=16 degree=16 | family-clmul-fold | family-pinned/ClmulFold | 303 (rand 0.8.8 StdRng) | 263 | 3764498 [3749140, 3777063] | 16.00 | 896 | 56 |
| bch-encode-batch | batch=64 degree=16 | current | current/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 25 | 39980194 [39933641, 40233685] | 64.00 | 3584 | 56 |
| bch-encode-batch | batch=64 degree=16 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 25 | 40003319 [39986370, 40137696] | 64.00 | 3584 | 56 |
| bch-encode-batch | batch=64 degree=16 | family-table-remainder | family-pinned/TableRemainder | 303 (rand 0.8.8 StdRng) | 54 | 18250789 [18222739, 18278436] | 64.00 | 3584 | 56 |
| bch-encode-batch | batch=64 degree=16 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 303 (rand 0.8.8 StdRng) | 61 | 16152747 [16140153, 16299932] | 64.00 | 3584 | 56 |
| bch-encode-batch | batch=64 degree=16 | family-clmul-fold | family-pinned/ClmulFold | 303 (rand 0.8.8 StdRng) | 66 | 14968025 [14934065, 15024103] | 64.00 | 3584 | 56 |
| bch-encode-batch | batch=256 degree=16 | current | current/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 6 | 160150483 [159779607, 160963920] | 256.00 | 14336 | 56 |
| bch-encode-batch | batch=256 degree=16 | family-poly-remainder-scalar | family-pinned/PolyRemainderScalar | 303 (rand 0.8.8 StdRng) | 6 | 159838002 [159749381, 160161000] | 256.00 | 14336 | 56 |
| bch-encode-batch | batch=256 degree=16 | family-table-remainder | family-pinned/TableRemainder | 303 (rand 0.8.8 StdRng) | 13 | 72790134 [72711787, 72910999] | 256.00 | 14336 | 56 |
| bch-encode-batch | batch=256 degree=16 | family-bitslice-interleaved | family-pinned/BitsliceInterleaved | 303 (rand 0.8.8 StdRng) | 15 | 64345353 [64319562, 64703884] | 256.00 | 14336 | 56 |
| bch-encode-batch | batch=256 degree=16 | family-clmul-fold | family-pinned/ClmulFold | 303 (rand 0.8.8 StdRng) | 16 | 59662220 [59572149, 59829715] | 256.00 | 14336 | 56 |
| bch-encode-batch-alloc | batch=16 degree=8 | current | current-allocating/PolyRemainderScalar | 304 (rand 0.8.8 StdRng) | 72700 | 12063 [11840, 12164] | 33.00 | 2816 | 1976 |
| bch-encode-batch-alloc | batch=16 degree=8 | caller-buffer | caller-buffer/PolyRemainderScalar | 304 (rand 0.8.8 StdRng) | 71782 | 11850 [11689, 11954] | 16.00 | 896 | 56 |
| bch-encode-batch-alloc | batch=256 degree=8 | current | current-allocating/PolyRemainderScalar | 304 (rand 0.8.8 StdRng) | 1983 | 437767 [435052, 440317] | 513.00 | 45056 | 30776 |
| bch-encode-batch-alloc | batch=256 degree=8 | caller-buffer | caller-buffer/PolyRemainderScalar | 304 (rand 0.8.8 StdRng) | 2045 | 431877 [430911, 435832] | 256.00 | 14336 | 56 |
| bch-encode-batch-alloc | batch=16 degree=14 | current | current-allocating/PolyRemainderScalar | 304 (rand 0.8.8 StdRng) | 391 | 2479892 [2473794, 2500275] | 33.00 | 35072 | 34232 |
| bch-encode-batch-alloc | batch=16 degree=14 | caller-buffer | caller-buffer/PolyRemainderScalar | 304 (rand 0.8.8 StdRng) | 381 | 2479335 [2473750, 2486067] | 16.00 | 896 | 56 |
| bch-encode-batch-alloc | batch=256 degree=14 | current | current-allocating/PolyRemainderScalar | 304 (rand 0.8.8 StdRng) | 25 | 39981472 [39954845, 40162262] | 513.00 | 561152 | 546872 |
| bch-encode-batch-alloc | batch=256 degree=14 | caller-buffer | caller-buffer/PolyRemainderScalar | 304 (rand 0.8.8 StdRng) | 25 | 39749907 [39728593, 39902226] | 256.00 | 14336 | 56 |
| bch-encode-batch-parallel | batch=256 degree=14 workers=1 | current | current-parallel/PolyRemainderScalar | 305 (rand 0.8.8 StdRng) | 25 | 39751471 [39731392, 40240568] | 256.00 | 14336 | 56 |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=1 | current | current-parallel/PolyRemainderScalar | 305 (rand 0.8.8 StdRng) | 6 | 159114710 [159081603, 159352785] | 1024.00 | 57344 | 56 |
| bch-encode-batch-parallel | batch=256 degree=14 workers=6 | current | current-parallel/PolyRemainderScalar | 305 (rand 0.8.8 StdRng) | 122 | 7189776 [7077444, 7474521] | 256.01 (range 256.01-256.02) | 14349 (range 14348-14359) | 1520 |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=6 | current | current-parallel/PolyRemainderScalar | 305 (rand 0.8.8 StdRng) | 33 | 28764166 [27919799, 29755604] | 1024.00 | 57344 | 56 |
| bch-encode-batch-parallel | batch=256 degree=14 workers=12 | current | current-parallel/PolyRemainderScalar | 305 (rand 0.8.8 StdRng) | 210 | 4252027 [4187618, 4337746] | 256.01 (range 256.01-256.02) | 14358 (range 14355-14359) | 1520 |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=12 | current | current-parallel/PolyRemainderScalar | 305 (rand 0.8.8 StdRng) | 39 | 25468862 [25296733, 25970939] | 1024.00 | 57344 | 56 |
| bch-encode-batch-parallel | batch=256 degree=14 workers=24 | current | current-parallel/PolyRemainderScalar | 305 (rand 0.8.8 StdRng) | 235 | 3354015 [3248580, 3392946] | 256.01 (range 256.01-256.02) | 14356 (range 14355-14360) | 1520 |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=24 | current | current-parallel/PolyRemainderScalar | 305 (rand 0.8.8 StdRng) | 83 | 11969098 [11812762, 12061612] | 1024.01 (range 1024.01-1024.01) | 57362 (range 57359-57364) | 1520 |
| field-id-hint | calls=1 | current | current/arc-allocating-field-identity | 307 (unused) | 83333333 | 11.7 [11.6, 11.7] | 1.00 | 56 | 56 |
| field-id-hint | calls=16 | current | current/arc-allocating-field-identity | 307 (unused) | 5319148 | 167 [166, 168] | 16.00 | 896 | 56 |
| field-id-hint | calls=256 | current | current/arc-allocating-field-identity | 307 (unused) | 333111 | 2648 [2629, 2651] | 256.00 | 14336 | 56 |
| field-id-hint | calls=1024 | current | current/arc-allocating-field-identity | 307 (unused) | 83305 | 10591 [10580, 10627] | 1024.00 | 57344 | 56 |
| dvb-bch-encode | batch=16 n=7200 | current | current/field-polynomial-div-rem | 306 (rand 0.8.8 StdRng) | 11 | 177390401 [176812105, 177808488] | 272.00 | 12780288 | 516480 |
| dvb-bch-encode | batch=1 n=32400 | current | current/field-polynomial-div-rem | 306 (rand 0.8.8 StdRng) | 34 | 57730171 [57651852, 57836779] | 21.00 | 3645264 | 2344608 |

## Reported setup and conversion probes

Whole-consumer routes report the probes the receipt schema carries. `setup` is one-shot preparation the timed body does not repeat; the remaining columns are per timed call. Each cell is the median over the sessions with the same order-statistic interval, in nanoseconds; a probe times many repetitions and reports the integer mean, so a zero is either a phase the route does not have or a per-call cost below 1 ns. What each probe times:

- `dense-rref`: setup: one fixture matrix materialized; dispatch: `resolve_xor_inplace(stride)`, the kernel resolution `rref` performs per call.
- `dense-matvec`: setup: one fixture matrix materialized; unpack: allocating a `BitVec` with `rows` bits of capacity and appending `rows` bits, the output construction `matvec` performs; dispatch: `matvec_route(stride)` only, because the kernel-table read that follows it is crate-private.
- `dense-transpose`: setup: one fixture matrix materialized; dispatch: `transpose_route` only, because the block-kernel lookup is crate-private.
- `ldpc-syndrome, ldpc-codeword-check`: setup: construction of the declared code; unpack: allocating a `BitVec` with one bit of capacity per check and appending one bit per check, the output construction of the CSR matvec.
- `bch-encode-batch*`: setup: `BinaryBchCode` construction; pack: one `encode_workspace()` construction, the workspace `encode_batch_into` borrows, because the families pack messages inside the timed call; batch fill: regenerating the batch (allocation plus the fixture generator's fill), a harness cost a consumer replaces with a copy of its own data; dispatch: `selected_encode_family`, the plan construction and family selection the entry point performs per call.
- `dvb-bch-encode`: setup: `BchCode::dvb_t2` construction; batch fill: regenerating the batch as above.

| Workload | Size | Route | setup ns | pack ns | unpack ns | batch fill ns | dispatch ns |
|---|---|---|---:|---:|---:|---:|---:|
| dense-rref | cols=256 rows=256 | current | 5440 [5210, 10950] | 0 | 0 | 0 | 0 |
| dense-rref | cols=512 rows=512 | current | 22370 [20150, 27221] | 0 | 0 | 0 | 0 |
| dense-rref | cols=1024 rows=1024 | current | 128291 [99190, 158650] | 0 | 0 | 0 | 0.00 [0.00, 1.00] |
| dense-rref | cols=2048 rows=2048 | current | 375502 [358412, 398142] | 0 | 0 | 0 | 0 |
| dense-matvec | cols=256 rows=256 | current | 5650 [5210, 6600] | 0 | 213 [213, 266] | 0 | 0 |
| dense-matvec | cols=1024 rows=1024 | current | 101911 [98911, 126250] | 0 | 813 [809, 993] | 0 | 0 |
| dense-matvec | cols=4096 rows=1024 | current | 366972 [350171, 412862] | 0 | 812 [809, 827] | 0 | 0 |
| dense-matvec | cols=4096 rows=4096 | current | 1424748 [1323697, 1453607] | 0 | 3206 [3087, 3238] | 0 | 0 |
| ldpc-syndrome | n=16200 | current | 1662380 [1644669, 1738650] | 0 | 6813 [6464, 7026] | 0 | 0 |
| ldpc-syndrome | n=64800 | current | 5755811 [5416660, 6036242] | 0 | 22283 [22128, 25387] | 0 | 0 |
| ldpc-codeword-check | n=16200 | count-ones | 1669799 [1621298, 1979621] | 0 | 6671 [6386, 7715] | 0 | 0 |
| ldpc-codeword-check | n=16200 | find-first-one | 1649278 [1593738, 1775960] | 0 | 6648 [6440, 7949] | 0 | 0 |
| ldpc-codeword-check | n=64800 | count-ones | 5665810 [5341248, 5789596] | 0 | 23197 [22486, 23494] | 0 | 0 |
| ldpc-codeword-check | n=64800 | find-first-one | 5670230 [5431249, 5866692] | 0 | 22612 [21944, 23479] | 0 | 0 |
| dense-transpose | cols=1024 rows=1024 | current | 98400 [97010, 115511] | 0 | 0 | 0 | 4.00 [4.00, 5.00] |
| dense-transpose | cols=4096 rows=4096 | current | 1417987 [1393737, 1602070] | 0 | 0 | 0 | 4.00 [4.00, 4.00] |
| bch-encode-batch | batch=1 degree=8 | current | 40240 [38570, 49320] | 4848 [4801, 5986] | 0 | 132 [130, 165] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=1 degree=8 | family-poly-remainder-scalar | 38360 [36750, 40810] | 4816 [4257, 5306] | 0 | 135 [122, 150] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=1 degree=8 | family-table-remainder | 38280 [34800, 45710] | 4795 [4773, 5968] | 0 | 135 [130, 162] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=1 degree=8 | family-bitslice-interleaved | 40241 [36820, 42020] | 4811 [4776, 5615] | 0 | 137 [127, 147] | 4.00 [3.00, 4.00] |
| bch-encode-batch | batch=1 degree=8 | family-clmul-fold | 39060 [37390, 43310] | 4857 [4810, 5988] | 0 | 132 [130, 160] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=16 degree=8 | current | 40880 [36110, 42970] | 4910 [4883, 6159] | 0 | 1732 [1722, 2130] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=16 degree=8 | family-poly-remainder-scalar | 38470 [38100, 41680] | 4930 [4728, 5440] | 0 | 1725 [1542, 1907] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=16 degree=8 | family-table-remainder | 37350 [34891, 41470] | 4903 [4882, 5495] | 0 | 1727 [1712, 2035] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=16 degree=8 | family-bitslice-interleaved | 38030 [36740, 41180] | 4925 [4901, 5330] | 0 | 1720 [1710, 1737] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=16 degree=8 | family-clmul-fold | 41260 [35641, 43931] | 5370 [4906, 6090] | 0 | 1892 [1710, 2150] | 4.00 [3.00, 4.00] |
| bch-encode-batch | batch=64 degree=8 | current | 39711 [35040, 43920] | 5001 [4975, 6213] | 0 | 7825 [7755, 10170] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=64 degree=8 | family-poly-remainder-scalar | 38320 [37460, 41120] | 4998 [4975, 5485] | 0 | 7785 [7725, 8600] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=64 degree=8 | family-table-remainder | 40471 [35000, 45940] | 4992 [4952, 5883] | 0 | 7825 [7690, 9177] | 4.00 [3.00, 4.00] |
| bch-encode-batch | batch=64 degree=8 | family-bitslice-interleaved | 42490 [38370, 45121] | 5243 [4975, 5575] | 0 | 8595 [7780, 10550] | 4.00 [3.00, 4.00] |
| bch-encode-batch | batch=64 degree=8 | family-clmul-fold | 42181 [36051, 45900] | 5541 [4985, 6223] | 0 | 8615 [7750, 9790] | 4.00 [3.00, 4.00] |
| bch-encode-batch | batch=256 degree=8 | current | 37990 [36010, 44640] | 4826 [4800, 5185] | 0 | 31235 [30550, 33405] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=256 degree=8 | family-poly-remainder-scalar | 41490 [34620, 46480] | 4833 [4789, 6025] | 0 | 30810 [30590, 38382] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=256 degree=8 | family-table-remainder | 43230 [35400, 46050] | 5166 [4815, 6036] | 0 | 34832 [31602, 39462] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=256 degree=8 | family-bitslice-interleaved | 44720 [39280, 46180] | 5670 [4808, 6035] | 0 | 34680 [33242, 39450] | 4.00 [3.00, 4.00] |
| bch-encode-batch | batch=256 degree=8 | family-clmul-fold | 41350 [38601, 45910] | 4810 [4793, 4896] | 0 | 31407 [30575, 36120] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=1024 degree=8 | current | 42151 [37160, 46260] | 4826 [4815, 4836] | 0 | 126815 [125175, 131415] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=1024 degree=8 | family-poly-remainder-scalar | 38180 [34660, 44810] | 4835 [4800, 6095] | 0 | 126603 [125218, 157400] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=1024 degree=8 | family-table-remainder | 38550 [34710, 44200] | 4820 [4800, 4851] | 0 | 126003 [125013, 133338] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=1024 degree=8 | family-bitslice-interleaved | 38480 [35780, 41361] | 4815 [4810, 4839] | 0 | 126958 [125035, 127895] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=1024 degree=8 | family-clmul-fold | 40090 [37230, 44620] | 4828 [4276, 4841] | 0 | 126858 [125575, 129975] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=1 degree=14 | current | 1623829 [1585408, 1701730] | 6844 [5971, 7108] | 0 | 670 [592, 675] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=1 degree=14 | family-poly-remainder-scalar | 1605549 [1583109, 1626799] | 6702 [6198, 6803] | 0 | 672 [597, 682] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=1 degree=14 | family-table-remainder | 1633269 [1445948, 1646019] | 6830 [6063, 7172] | 0 | 667 [592, 672] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=1 degree=14 | family-bitslice-interleaved | 1612888 [1589159, 1658379] | 6820 [6771, 6846] | 0 | 667 [660, 677] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=1 degree=14 | family-clmul-fold | 1610048 [1597358, 1651799] | 6783 [6627, 7094] | 0 | 675 [655, 690] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=16 degree=14 | current | 1619798 [1601768, 1629850] | 6844 [6753, 7258] | 0 | 9595 [9510, 9627] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=16 degree=14 | family-poly-remainder-scalar | 1631629 [1594449, 1726089] | 6850 [6526, 7363] | 0 | 9585 [9147, 9670] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=16 degree=14 | family-table-remainder | 1596208 [1583398, 1634818] | 6636 [6526, 6851] | 0 | 9505 [9125, 9605] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=16 degree=14 | family-bitslice-interleaved | 1602308 [1584179, 1812220] | 6828 [6803, 6863] | 0 | 9600 [9515, 10390] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=16 degree=14 | family-clmul-fold | 1602008 [1578338, 1720169] | 6832 [6520, 8127] | 0 | 9597 [9092, 11510] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=64 degree=14 | current | 1606878 [1581748, 1690309] | 6789 [6472, 6845] | 0 | 39465 [37472, 42937] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=64 degree=14 | family-poly-remainder-scalar | 1613038 [1586518, 1722700] | 6803 [6540, 6833] | 0 | 39455 [38442, 44690] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=64 degree=14 | family-table-remainder | 1605648 [1584068, 1663169] | 6782 [6465, 6825] | 0 | 39417 [38090, 39802] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=64 degree=14 | family-bitslice-interleaved | 1674479 [1587878, 1749869] | 6804 [6751, 7119] | 0 | 40347 [39812, 40885] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=64 degree=14 | family-clmul-fold | 1597639 [1574788, 1827500] | 6782 [6469, 6805] | 0 | 39422 [37322, 39707] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=256 degree=14 | current | 1600208 [1591109, 1613509] | 6457 [6248, 6851] | 0 | 315679 [309151, 323582] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=256 degree=14 | family-poly-remainder-scalar | 1600969 [1595058, 1606549] | 6463 [6224, 6850] | 0 | 315644 [305016, 324549] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=256 degree=14 | family-table-remainder | 1601329 [1593309, 1717989] | 6820 [6208, 6878] | 0 | 324686 [305729, 374547] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=256 degree=14 | family-bitslice-interleaved | 1610499 [1604119, 1636739] | 6209 [6107, 6419] | 0 | 308894 [303839, 318204] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=256 degree=14 | family-clmul-fold | 1603160 [1590509, 1639329] | 6479 [6178, 6733] | 0 | 310849 [305384, 322079] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=1024 degree=14 | current | 1605958 [1587169, 1674729] | 5944 [5916, 6433] | 0 | 1279391 [1239171, 1366329] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=1024 degree=14 | family-poly-remainder-scalar | 1592009 [1576838, 1736049] | 6104 [5929, 6851] | 0 | 1276907 [1240321, 1404084] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=1024 degree=14 | family-table-remainder | 1639699 [1601049, 1722799] | 6001 [5854, 6568] | 0 | 1239474 [1222834, 1383852] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=1024 degree=14 | family-bitslice-interleaved | 1606048 [1588799, 1609788] | 6176 [5926, 6449] | 0 | 1272126 [1229704, 1340772] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=1024 degree=14 | family-clmul-fold | 1592448 [1414737, 1614898] | 5973 [5873, 6231] | 0 | 1235189 [1226226, 1292856] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=1 degree=16 | current | 6229604 [6150803, 6365404] | 6343 [6116, 7063] | 0 | 2082 [2005, 2435] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=1 degree=16 | family-poly-remainder-scalar | 6237764 [6123503, 6293764] | 6141 [5961, 6478] | 0 | 2005 [1975, 2195] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=1 degree=16 | family-table-remainder | 6268929 [6207333, 6764316] | 6321 [6050, 6951] | 0 | 2075 [1987, 2297] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=1 degree=16 | family-bitslice-interleaved | 6214974 [6130742, 6606341] | 6085 [6037, 7666] | 0 | 2000 [1977, 2540] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=1 degree=16 | family-clmul-fold | 6201013 [5979263, 6413105] | 6316 [5998, 7555] | 0 | 2082 [1980, 2505] | 3.00 [3.00, 4.00] |
| bch-encode-batch | batch=16 degree=16 | current | 6283273 [6241554, 6650565] | 6472 [6034, 7000] | 0 | 33000 [30937, 38782] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=16 degree=16 | family-poly-remainder-scalar | 6260054 [6186794, 6593754] | 6281 [5993, 6544] | 0 | 32370 [31297, 34992] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=16 degree=16 | family-table-remainder | 6292114 [5674155, 6359235] | 6196 [6010, 6386] | 0 | 31310 [30525, 32922] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=16 degree=16 | family-bitslice-interleaved | 6341304 [5697860, 6589636] | 6130 [5953, 6338] | 0 | 31212 [30437, 32605] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=16 degree=16 | family-clmul-fold | 6184914 [5712690, 6822823] | 6088 [5970, 6391] | 0 | 31020 [30545, 32680] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=64 degree=16 | current | 6315023 [6207873, 6518115] | 6365 [6091, 7343] | 0 | 130733 [124105, 146740] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=64 degree=16 | family-poly-remainder-scalar | 6408365 [6097862, 6785343] | 6000 [5969, 6410] | 0 | 123615 [122225, 136720] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=64 degree=16 | family-table-remainder | 6273553 [6225428, 6623136] | 6095 [6075, 6608] | 0 | 125205 [123720, 139875] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=64 degree=16 | family-bitslice-interleaved | 6253744 [6170924, 6603955] | 6136 [6025, 6310] | 0 | 124655 [121783, 131048] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=64 degree=16 | family-clmul-fold | 6280703 [6221264, 6326295] | 6130 [6044, 6921] | 0 | 126793 [123653, 149476] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=256 degree=16 | current | 6281763 [6226503, 6627306] | 6212 [5966, 6617] | 0 | 1227684 [1167451, 1275266] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=256 degree=16 | family-poly-remainder-scalar | 6276493 [6224642, 6621145] | 6047 [5950, 6337] | 0 | 1165229 [1151908, 1233802] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=256 degree=16 | family-table-remainder | 6226973 [6132628, 6511995] | 6186 [6046, 6340] | 0 | 1242981 [1166591, 1301771] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=256 degree=16 | family-bitslice-interleaved | 6234454 [5632471, 6278764] | 5985 [5949, 7042] | 0 | 1168141 [1156063, 1359934] | 3.00 [3.00, 3.00] |
| bch-encode-batch | batch=256 degree=16 | family-clmul-fold | 6284849 [5631509, 6653847] | 5996 [5952, 6281] | 0 | 1166228 [1160543, 1272953] | 3.00 [3.00, 3.00] |
| bch-encode-batch-alloc | batch=16 degree=8 | current | 38231 [35040, 41111] | 4895 [4336, 4936] | 0 | 1712 [1512, 1732] | 3.00 [3.00, 3.00] |
| bch-encode-batch-alloc | batch=16 degree=8 | caller-buffer | 41741 [38320, 44461] | 5413 [4904, 5778] | 0 | 1905 [1715, 2030] | 4.00 [3.00, 4.00] |
| bch-encode-batch-alloc | batch=256 degree=8 | current | 43670 [35240, 46010] | 5165 [4808, 5835] | 0 | 32387 [30615, 38867] | 4.00 [3.00, 4.00] |
| bch-encode-batch-alloc | batch=256 degree=8 | caller-buffer | 41450 [35750, 46560] | 4815 [4800, 4835] | 0 | 32587 [31515, 33757] | 3.00 [3.00, 4.00] |
| bch-encode-batch-alloc | batch=16 degree=14 | current | 1587318 [1417358, 1602919] | 6831 [6010, 7098] | 0 | 9562 [8432, 9592] | 3.00 [3.00, 3.00] |
| bch-encode-batch-alloc | batch=16 degree=14 | caller-buffer | 1630309 [1591049, 1802210] | 6838 [6693, 7834] | 0 | 9600 [9427, 11535] | 3.00 [3.00, 4.00] |
| bch-encode-batch-alloc | batch=256 degree=14 | current | 1623128 [1592859, 1690400] | 6808 [6209, 6866] | 0 | 328469 [305121, 363921] | 3.00 [3.00, 4.00] |
| bch-encode-batch-alloc | batch=256 degree=14 | caller-buffer | 1624389 [1412988, 1734880] | 6320 [5978, 6540] | 0 | 312996 [286016, 323684] | 3.00 [3.00, 7.00] |
| bch-encode-batch-parallel | batch=256 degree=14 workers=1 | current | 1605718 [1590149, 1797800] | 6313 [6207, 8491] | 0 | 306209 [303754, 330269] | 3.00 [3.00, 4.00] |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=1 | current | 1646240 [1593449, 1771760] | 6744 [5876, 7211] | 0 | 1417795 [1267841, 1470325] | 3.00 [3.00, 3.00] |
| bch-encode-batch-parallel | batch=256 degree=14 workers=6 | current | 1601610 [1582788, 1741489] | 6306 [6218, 6438] | 0 | 304184 [302549, 314966] | 3.00 [3.00, 3.00] |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=6 | current | 1595398 [1447688, 1671329] | 6503 [6150, 6910] | 0 | 1378159 [1274269, 1490375] | 3.00 [3.00, 3.00] |
| bch-encode-batch-parallel | batch=256 degree=14 workers=12 | current | 1507699 [1482958, 1682019] | 6423 [6156, 6518] | 0 | 302987 [292684, 312329] | 3.00 [3.00, 3.00] |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=12 | current | 1495628 [1477768, 1619159] | 6432 [6235, 6883] | 0 | 1432807 [1322787, 1496095] | 3.00 [3.00, 3.00] |
| bch-encode-batch-parallel | batch=256 degree=14 workers=24 | current | 1590619 [1581738, 1616159] | 6322 [6265, 6876] | 0 | 312011 [299299, 364264] | 3.00 [3.00, 3.00] |
| bch-encode-batch-parallel | batch=1024 degree=14 workers=24 | current | 1504419 [1488728, 1705391] | 6561 [6275, 7563] | 0 | 1440677 [1304149, 1485066] | 3.00 [3.00, 4.00] |
| dvb-bch-encode | batch=16 n=7200 | current | 1072816 [1057346, 1159447] | 0 | 0 | 6270 [6197, 6397] | 0 |
| dvb-bch-encode | batch=1 n=32400 | current | 4495344 [4274503, 4565854] | 0 | 0 | 1355 [1335, 1410] | 0 |
