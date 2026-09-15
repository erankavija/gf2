# Byte-field consumer profile

> **Diátaxis Type:** Reference

Source: `dev/active/19513245/survey/summarize-profile.py` over 9 session records under this directory, 9 of them logged complete in `repetitions.log`, and `ladder.json`, the case ladder the sessions measured.

Every wall time is nanoseconds per call. A session's value is the median of its five 40 ms windows; the value below is the median over the sessions, and the interval is the order-statistic interval [x(2), x(8)] of 9 sessions, coverage 0.961. Allocation counts and reuse counts are exact integers observed in one call and are identical across sessions unless the table says otherwise. The counting allocator adds two relaxed atomic increments per allocation, so a route's wall time here carries the cost of counting its own allocations; the A/B receipts carry the timing that decides anything.

## Routes

| Case | Route selected at run time | ns/call | interval | sessions |
|---|---|---:|---|---:|
| `axpy-4k-element-current` | `gf2-core/FieldVec<Gf2mElement>::axpy/scalar-element-loop` | 41 800 | [41 501, 42 612] | 9 |
| `axpy-4k-element-prototype` | `prototype/coefficient-table/in-place-FieldVec<Gf2mElement>` | 17 030 | [16 834, 17 304] | 9 |
| `axpy-128k-element-current` | `gf2-core/FieldVec<Gf2mElement>::axpy/scalar-element-loop` | 1 362 583 | [1 351 298, 1 379 409] | 9 |
| `axpy-128k-element-prototype` | `prototype/coefficient-table/in-place-FieldVec<Gf2mElement>` | 540 691 | [538 594, 548 831] | 9 |
| `region-4k-element-current` | `gf2-core/FieldVec<Gf2mElement>::axpy/scalar-element-loop/byte-region-boundary` | 72 405 | [72 053, 73 229] | 9 |
| `region-4k-element-prototype` | `prototype/coefficient-table/byte-region` | 1 357 | [1 351, 1 378] | 9 |
| `region-128k-element-current` | `gf2-core/FieldVec<Gf2mElement>::axpy/scalar-element-loop/byte-region-boundary` | 2 368 984 | [2 362 658, 2 381 572] | 9 |
| `region-128k-element-prototype` | `prototype/coefficient-table/byte-region` | 41 077 | [39 991, 41 784] | 9 |
| `matmul-n64-element-current` | `gf2-core/field::matrix::gemm/FieldMatrix<Gf2mElement>/per-cell-batch-dot/vpclmulqdq-batch` | 864 325 | [862 361, 877 399] | 9 |
| `matmul-n64-element-prototype` | `prototype/product-table-65536/byte-matrix` | 83 193 | [83 063, 84 292] | 9 |
| `matmul-n256-element-current` | `gf2-core/field::matrix::gemm/FieldMatrix<Gf2mElement>/per-cell-batch-dot/vpclmulqdq-batch` | 48 579 347 | [48 099 935, 49 322 991] | 9 |
| `matmul-n256-element-prototype` | `prototype/product-table-65536/byte-matrix` | 5 138 777 | [5 129 259, 5 250 636] | 9 |
| `matmul-whole-n256-element-current` | `gf2-core/field::matrix::gemm/FieldMatrix<Gf2mElement>/per-cell-batch-dot/vpclmulqdq-batch` | 48 795 448 | [48 096 444, 49 549 712] | 9 |
| `matmul-whole-n256-element-prototype` | `prototype/product-table-65536/byte-matrix/FieldMatrix-boundary` | 5 398 891 | [5 382 394, 5 499 800] | 9 |
| `matvec-n256-element-current` | `gf2-core/FieldMatrix<Gf2mElement>::matvec/dot-product-slices-scalar-chain` | 554 927 | [551 272, 564 520] | 9 |
| `matvec-n256-element-prototype` | `prototype/product-table-65536/byte-matrix` | 28 815 | [28 667, 29 301] | 9 |
| `axpy-4k-wide-current` | `gf2-core/FieldVec<Gf2mWide<1,Gf256x11d>>::axpy/scalar-element-loop` | 183 688 | [183 133, 187 117] | 9 |
| `axpy-4k-wide-prototype` | `prototype/coefficient-table/in-place-FieldVec<Gf2mWide<1,Gf256x11d>>` | 1 848 | [1 844, 1 876] | 9 |
| `axpy-128k-wide-current` | `gf2-core/FieldVec<Gf2mWide<1,Gf256x11d>>::axpy/scalar-element-loop` | 6 028 089 | [5 994 160, 6 132 230] | 9 |
| `axpy-128k-wide-prototype` | `prototype/coefficient-table/in-place-FieldVec<Gf2mWide<1,Gf256x11d>>` | 57 337 | [57 011, 57 986] | 9 |
| `region-4k-wide-current` | `gf2-core/FieldVec<Gf2mWide<1,Gf256x11d>>::axpy/scalar-element-loop/byte-region-boundary` | 184 658 | [183 394, 186 844] | 9 |
| `region-4k-wide-prototype` | `prototype/coefficient-table/byte-region` | 1 322 | [1 317, 1 343] | 9 |
| `region-128k-wide-current` | `gf2-core/FieldVec<Gf2mWide<1,Gf256x11d>>::axpy/scalar-element-loop/byte-region-boundary` | 6 082 194 | [6 060 490, 6 181 386] | 9 |
| `region-128k-wide-prototype` | `prototype/coefficient-table/byte-region` | 40 694 | [39 490, 41 204] | 9 |
| `matmul-n64-wide-current` | `gf2-core/field::matrix::gemm/FieldMatrix<Gf2mWide<1,Gf256x11d>>/whole-gemm/vpclmulqdq-gemm` | 237 589 | [236 946, 241 798] | 9 |
| `matmul-n64-wide-prototype` | `prototype/product-table-65536/byte-matrix` | 84 614 | [84 517, 85 925] | 9 |
| `matmul-n256-wide-current` | `gf2-core/field::matrix::gemm/FieldMatrix<Gf2mWide<1,Gf256x11d>>/whole-gemm/vpclmulqdq-gemm` | 15 467 183 | [15 380 458, 15 617 358] | 9 |
| `matmul-n256-wide-prototype` | `prototype/product-table-65536/byte-matrix` | 5 139 077 | [5 129 806, 5 198 481] | 9 |
| `matmul-whole-n256-wide-current` | `gf2-core/field::matrix::gemm/FieldMatrix<Gf2mWide<1,Gf256x11d>>/whole-gemm/vpclmulqdq-gemm` | 15 398 673 | [15 370 982, 15 557 494] | 9 |
| `matmul-whole-n256-wide-prototype` | `prototype/product-table-65536/byte-matrix/FieldMatrix-boundary` | 5 200 184 | [5 167 767, 5 258 137] | 9 |
| `matvec-n256-wide-current` | `gf2-core/FieldMatrix<Gf2mWide<1,Gf256x11d>>::matvec/dot-product-slices-scalar-chain` | 3 070 663 | [3 062 409, 3 117 160] | 9 |
| `matvec-n256-wide-prototype` | `prototype/product-table-65536/byte-matrix` | 28 515 | [28 427, 28 966] | 9 |
| `pairwise-4k-batch-current` | `gf2-core/gf2m::batch::batch_mul/u64-lanes/vpclmulqdq-batch` | 4 156 | [4 121, 4 252] | 9 |
| `pairwise-4k-batch-prototype` | `prototype/product-table-65536/byte-region` | 2 003 | [1 997, 2 343] | 9 |
| `pairwise-128k-batch-current` | `gf2-core/gf2m::batch::batch_mul/u64-lanes/vpclmulqdq-batch` | 135 863 | [135 169, 137 612] | 9 |
| `pairwise-128k-batch-prototype` | `prototype/product-table-65536/byte-region` | 57 644 | [57 546, 58 176] | 9 |
| `pairwise-region-128k-batch-current` | `gf2-core/gf2m::batch::batch_mul/u64-lanes/vpclmulqdq-batch/byte-region-boundary` | 206 265 | [204 734, 212 303] | 9 |
| `pairwise-region-128k-batch-prototype` | `prototype/product-table-65536/byte-region` | 57 694 | [57 573, 58 153] | 9 |

## Allocation, reuse and conversion

`allocations` and `bytes` are what one call allocates. `reuse` is the number of multiplications one prepared coefficient table serves, and is zero for a route that prepares no coefficient table. `table` is the prepared table's size in bytes. The remaining columns are untimed probes, each the median of nine repetitions inside one session and then the median over the sessions: `setup` builds the field context, `table prep` builds the table, `pack` converts the operands into the route's representation and `unpack` converts the result out of it. A zero means the route does not perform that step.

| Case | allocations | bytes | reuse | table | setup ns | table prep ns | pack ns | unpack ns |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `axpy-4k-element-current` | 0 | 0 | 0 | 0 | 60.0 | 0.000 | 0.000 | 0.000 |
| `axpy-4k-element-prototype` | 0 | 0 | 4096 | 256 | 40.0 | 150.0 | 0.000 | 0.000 |
| `axpy-128k-element-current` | 0 | 0 | 0 | 0 | 40.0 | 0.000 | 0.000 | 0.000 |
| `axpy-128k-element-prototype` | 0 | 0 | 131072 | 256 | 40.0 | 150.0 | 0.000 | 0.000 |
| `region-4k-element-current` | 0 | 0 | 0 | 0 | 40.0 | 0.000 | 33 700 | 790.0 |
| `region-4k-element-prototype` | 0 | 0 | 4096 | 256 | 40.0 | 150.0 | 0.000 | 0.000 |
| `region-128k-element-current` | 0 | 0 | 0 | 0 | 40.0 | 0.000 | 1 164 997 | 26 510 |
| `region-128k-element-prototype` | 0 | 0 | 131072 | 256 | 40.0 | 150.0 | 0.000 | 0.000 |
| `matmul-n64-element-current` | 5 | 132608 | 0 | 0 | 40.0 | 0.000 | 0.000 | 0.000 |
| `matmul-n64-element-prototype` | 0 | 0 | 64 | 65536 | 40.0 | 35 690 | 0.000 | 0.000 |
| `matmul-n256-element-current` | 5 | 2103296 | 0 | 0 | 40.0 | 0.000 | 0.000 | 0.000 |
| `matmul-n256-element-prototype` | 0 | 0 | 256 | 65536 | 40.0 | 35 530 | 0.000 | 0.000 |
| `matmul-whole-n256-element-current` | 5 | 2103296 | 0 | 0 | 40.0 | 0.000 | 0.000 | 0.000 |
| `matmul-whole-n256-element-prototype` | 0 | 0 | 256 | 65536 | 40.0 | 35 560 | 26 440 | 226 251 |
| `matvec-n256-element-current` | 1 | 4096 | 0 | 0 | 40.0 | 0.000 | 0.000 | 0.000 |
| `matvec-n256-element-prototype` | 0 | 0 | 0 | 65536 | 40.0 | 36 070 | 0.000 | 0.000 |
| `axpy-4k-wide-current` | 10214 | 163424 | 0 | 0 | 0.000 | 0.000 | 0.000 | 0.000 |
| `axpy-4k-wide-prototype` | 0 | 0 | 4096 | 256 | 0.000 | 150.0 | 0.000 | 0.000 |
| `axpy-128k-wide-current` | 327956 | 5247296 | 0 | 0 | 0.000 | 0.000 | 0.000 | 0.000 |
| `axpy-128k-wide-prototype` | 0 | 0 | 131072 | 256 | 0.000 | 150.0 | 0.000 | 0.000 |
| `region-4k-wide-current` | 10214 | 163424 | 0 | 0 | 0.000 | 0.000 | 1 420 | 740.0 |
| `region-4k-wide-prototype` | 0 | 0 | 4096 | 256 | 0.000 | 150.0 | 0.000 | 0.000 |
| `region-128k-wide-current` | 327956 | 5247296 | 0 | 0 | 0.000 | 0.000 | 44 500 | 23 441 |
| `region-128k-wide-prototype` | 0 | 0 | 131072 | 256 | 0.000 | 150.0 | 0.000 | 0.000 |
| `matmul-n64-wide-current` | 5 | 163840 | 0 | 0 | 0.000 | 0.000 | 0.000 | 0.000 |
| `matmul-n64-wide-prototype` | 0 | 0 | 64 | 65536 | 0.000 | 37 420 | 0.000 | 0.000 |
| `matmul-n256-wide-current` | 5 | 2621440 | 0 | 0 | 0.000 | 0.000 | 0.000 | 0.000 |
| `matmul-n256-wide-prototype` | 0 | 0 | 256 | 65536 | 0.000 | 37 360 | 0.000 | 0.000 |
| `matmul-whole-n256-wide-current` | 5 | 2621440 | 0 | 0 | 0.000 | 0.000 | 0.000 | 0.000 |
| `matmul-whole-n256-wide-prototype` | 0 | 0 | 256 | 65536 | 0.000 | 37 270 | 23 330 | 11 330 |
| `matvec-n256-wide-current` | 192429 | 3080896 | 0 | 0 | 0.000 | 0.000 | 0.000 | 0.000 |
| `matvec-n256-wide-prototype` | 0 | 0 | 0 | 65536 | 0.000 | 33 430 | 0.000 | 0.000 |
| `pairwise-4k-batch-current` | 0 | 0 | 0 | 0 | 40.0 | 0.000 | 0.000 | 0.000 |
| `pairwise-4k-batch-prototype` | 0 | 0 | 0 | 65536 | 40.0 | 35 460 | 0.000 | 0.000 |
| `pairwise-128k-batch-current` | 0 | 0 | 0 | 0 | 40.0 | 0.000 | 0.000 | 0.000 |
| `pairwise-128k-batch-prototype` | 0 | 0 | 0 | 65536 | 40.0 | 35 580 | 0.000 | 0.000 |
| `pairwise-region-128k-batch-current` | 0 | 0 | 0 | 0 | 40.0 | 0.000 | 45 580 | 23 750 |
| `pairwise-region-128k-batch-prototype` | 0 | 0 | 0 | 65536 | 40.0 | 35 780 | 0.000 | 0.000 |

## Prototype over current route

Each row divides the current route's session-median wall time by the prototype's, so a value above one favours the prototype. Both medians come from separate executions inside the same sessions and the quotient carries no interval, so every value here is descriptive; the A/B receipts of the three campaign families carry the paired estimates and their bootstrap intervals.

| Shape | current ns/call | prototype ns/call | descriptive ratio |
|---|---:|---:|---:|
| `axpy-4k-element` | 41 800 | 17 030 | 2.455 |
| `axpy-128k-element` | 1 362 583 | 540 691 | 2.520 |
| `region-4k-element` | 72 405 | 1 357 | 53.4 |
| `region-128k-element` | 2 368 984 | 41 077 | 57.7 |
| `matmul-n64-element` | 864 325 | 83 193 | 10.4 |
| `matmul-n256-element` | 48 579 347 | 5 138 777 | 9.453 |
| `matmul-whole-n256-element` | 48 795 448 | 5 398 891 | 9.038 |
| `matvec-n256-element` | 554 927 | 28 815 | 19.3 |
| `axpy-4k-wide` | 183 688 | 1 848 | 99.4 |
| `axpy-128k-wide` | 6 028 089 | 57 337 | 105.1 |
| `region-4k-wide` | 184 658 | 1 322 | 139.7 |
| `region-128k-wide` | 6 082 194 | 40 694 | 149.5 |
| `matmul-n64-wide` | 237 589 | 84 614 | 2.808 |
| `matmul-n256-wide` | 15 467 183 | 5 139 077 | 3.010 |
| `matmul-whole-n256-wide` | 15 398 673 | 5 200 184 | 2.961 |
| `matvec-n256-wide` | 3 070 663 | 28 515 | 107.7 |
| `pairwise-4k-batch` | 4 156 | 2 003 | 2.075 |
| `pairwise-128k-batch` | 135 863 | 57 644 | 2.357 |
| `pairwise-region-128k-batch` | 206 265 | 57 694 | 3.575 |

## Hardware counters

Counters of the first session only, over a one-second repetition of each case in its own process, user mode. Each value is divided by the calls that process reported, so a row is per call. `IPC` is instructions divided by cycles. These are single observations with no interval: they explain a limit, they do not estimate one.

| Case | calls | cycles/call | instructions/call | IPC | branch misses/call | cache misses/call |
|---|---:|---:|---:|---:|---:|---:|
| `axpy-128k-element-current` | 739 | 6 538 299 | 17 166 213 | 2.63 | 35 699 | 1 389 |
| `axpy-128k-element-prototype` | 1833 | 2 691 105 | 3 441 946 | 1.28 | 7.936 | 1 233 |
| `axpy-128k-wide-current` | 164 | 29 241 001 | 99 916 818 | 3.42 | 148 102 | 1 117 |
| `axpy-128k-wide-prototype` | 17905 | 265 490 | 1 314 895 | 4.95 | 2.606 | 633.9 |
| `matmul-n256-element-current` | 21 | 243 341 178 | 838 031 929 | 3.44 | 277 083 | 217 270 |
| `matmul-n256-element-prototype` | 194 | 26 170 770 | 81 861 749 | 3.13 | 67 205 | 5 917 |
| `matmul-n256-wide-current` | 63 | 75 969 028 | 161 572 098 | 2.13 | 51 171 | 80 288 |
| `matmul-n256-wide-prototype` | 193 | 25 357 290 | 78 437 463 | 3.09 | 66 110 | 5 638 |
| `matvec-n256-element-current` | 1790 | 2 681 497 | 8 571 965 | 3.20 | 2 350 | 564.8 |
| `matvec-n256-element-prototype` | 34300 | 138 771 | 661 029 | 4.76 | 257.4 | 959.9 |
| `pairwise-128k-batch-current` | 7270 | 666 249 | 1 609 219 | 2.42 | 3.212 | 892.9 |
| `pairwise-128k-batch-prototype` | 17090 | 277 154 | 1 444 148 | 5.21 | 1.817 | 7 357 |
| `region-128k-element-current` | 416 | 11 698 127 | 22 355 936 | 1.91 | 35 885 | 3 581 |
| `region-128k-element-prototype` | 26091 | 186 369 | 595 244 | 3.19 | 2.403 | 103.2 |

## Generated code

`dev/active/19513245/survey/asm/` holds the annotated release disassembly of the measured routes and their instruction mix, written by `disassemble.sh` from the arm executable of the same build these sessions ran; `asm/index.txt` names each file, the symbols it covers and the pattern that selected them.

