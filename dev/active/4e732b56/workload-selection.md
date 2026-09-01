# Workload-selection contract (ae03bcd0)

| Field | Value |
|---|---|
| JIT issue | `4e732b56` (Reproducible external-baseline survey and workload selection) |
| Epic | `ae03bcd0` (Harden and generalize BCH codes over finite fields) |
| Contract slot | `workload-selection` of [plan.md](../ae03bcd0-general-bch/plan.md) § *Shared architectural contracts* |
| Evidence | [findings.md](findings.md) |
| Consumers | `encode-dispatch`, `avx2-batch-kernels`, `genmatrix-perf`, `bench-extension`, `perf-receipts` |

This document is the `workload-selection` contract: the concrete completion of
the plan's `evidence-protocol`. It fixes the codes, batch sizes, matrix
dimensions, cache state, worker counts, algorithm families, and selected
external baseline per workload. It is predeclared — no optimization task has
consumed it yet — and a consumer that needs a cell this document does not name
amends it here first.

The statistical acceptance rule, the conformance corpus, and the host
conventions are not restated here; they are the `evidence-protocol` section of
[plan.md](../ae03bcd0-general-bch/plan.md), which this document completes rather than replaces.

## 1. Workloads

**W1 — large-batch binary BCH systematic encoding.** Encode a batch of $B$
messages of $k$ information bits each into $B$ systematic codewords of $n$
bits, in the repository's default `[message | parity]` layout. Reported in
information bits per second: $B k / T$.

**W2 — GF(2) generator-matrix materialization.** Produce the $k \times n$
systematic generator matrix $G$ of a binary BCH code as a packed GF(2) bit
matrix. Reported in matrix bits per second: $k n / T$.

The two workloads are the pair the plan names for REQ-13, and they are
measured with the same code rows so a consumer can reason about the crossover
between encoding a batch directly and encoding it through a materialized $G$.

## 2. Benchmark codes

Every row is a binary, narrow-sense ($b = 1$), systematic BCH code. `prim` is
the primitive polynomial of the mother field, matching
`crates/gf2-core/src/primitive_polys.rs::standard(m)`, so the generator
polynomial is fixed rather than left to a library's default field choice.

| Row | $n$ | $k$ | $t$ | $\delta$ | $\deg g$ | Mother field | `prim` | Provenance |
|---|---|---|---|---|---|---|---|---|
| B1 | 15 | 5 | 3 | 7 | 10 | $\mathrm{GF}(2^4)$ | $x^4 + x + 1$ | plan corpus row B1 |
| B2 | 127 | 64 | 10 | 21 | 63 | $\mathrm{GF}(2^7)$ | $x^7 + x + 1$ | plan corpus row B2 |
| B3 | 255 | 223 | 4 | 9 | 32 | $\mathrm{GF}(2^8)$ | $x^8 + x^4 + x^3 + x^2 + 1$ | plan corpus row B3 |
| T2S | 7200 | 7032 | 12 | 25 | 168 | $\mathrm{GF}(2^{14})$ | $x^{14} + x^5 + x^3 + x + 1$ | DVB-T2 short frame, rate 1/2 |
| T2N | 32400 | 32208 | 12 | 25 | 192 | $\mathrm{GF}(2^{16})$ | $x^{16} + x^5 + x^3 + x^2 + 1$ | plan corpus row B4 |

B1, B2 and B3 are the plan's binary conformance-corpus rows carried over
unchanged, so a performance cell and a conformance cell name the same code.
T2N realizes corpus row B4: the DVB-T2 normal-frame parameters as pinned by
the in-tree table at `crates/gf2-coding/src/bch/dvb_t2/params.rs:69`.

T2S is the one row this contract adds beyond the corpus. It is a shortened
code over a *different* mother field ($m = 14$) whose message fits in 879
bytes rather than 4026, so at a fixed batch size its working set sits a factor
of 4.5 below T2N's — L2-resident at $B = 256$ where T2N is already L3-resident.
Without it, every large-code cell would share one mother field and one
working-set regime.

T2S and T2N are shortened codes: $n < 2^m - 1$. Their generator polynomials
are those of the mother codes of length $2^m - 1$, and their $\delta = 25$ is
the designed distance of that mother code.

## 3. Batch sizes

$B \in \{1,\ 16,\ 256,\ 4096\}$.

| $B$ | What the cell resolves |
|---|---|
| 1 | Per-call cost: construction, dispatch, and allocation that does not amortize. A SIMD-interleaved family cannot fill a wave here, which is itself the measurement. |
| 16 | One full SIMD wave at 16 lanes and two at 8, so a lane-parallel family is exercised at its minimum useful width. |
| 256 | Steady state with the working set L2-resident on every row but T2N, which is L3-resident. |
| 4096 | The largest working sets the ladder reaches: 31.5 MiB on T2N, against a 32 MiB L3. Memory traffic competes with the LFSR recurrence here in a way it does not at $B = 256$. |

Derived working sets, message plus codeword packed ($B(k+n)/8$ bytes),
against this host's 512 KiB L2 per core and 32 MiB L3 per CCX:

| Row | $B = 1$ | $B = 16$ | $B = 256$ | $B = 4096$ |
|---|---|---|---|---|
| B1 | 2 B | 40 B | 640 B | 10 KiB |
| B2 | 24 B | 382 B | 6 KiB | 96 KiB |
| B3 | 60 B | 956 B | 15 KiB | 239 KiB |
| T2S | 2 KiB | 28 KiB | 445 KiB | 6.9 MiB |
| T2N | 8 KiB | 126 KiB | 2.0 MiB | 31.5 MiB |

These sizes describe the packed representation the contract's throughput unit
counts, which is the working set of the packed-byte implementations (bchlib,
the gf2 side, and the packed W2 matrices). For them, every row through
$B = 256$ is L2-resident except T2N, which is L3-resident there, and at
$B = 4096$ only T2N approaches the L3 capacity boundary, at 31.5 MiB against
32 MiB. An implementation with a wider storage representation scales this
table by its own factor: AFF3CT stores one 32-bit word per bit, so its
working sets are 32× these figures — its T2N rows leave L3 at $B = 256$
(64 MiB), its T2S rows stay L3-resident through $B = 256$ (13.9 MiB), and
both are DRAM-backed at $B = 4096$ (222 MiB and 1008 MiB). Residency claims in this
document are therefore per-representation: the packed ladder supports no
reliably DRAM-bound cell, while the AFF3CT large-frame cells at high $B$ are
DRAM-backed by size, and any conclusion drawn from them must account for
that. A consumer that needs a packed DRAM-bound cell extends the ladder here
rather than in its own bench; this document records that as an open gap
rather than claiming coverage the sizes do not support.

## 4. Matrix dimensions (W2)

$G$ is $k \times n$ for each row of § 2: $5 \times 15$, $64 \times 127$,
$223 \times 255$, $7032 \times 7200$, and $32208 \times 32400$. The last is
1.04 Gbit, 124 MiB packed, and is the row that decides whether materialization
is viable at DVB-T2 normal-frame scale at all.

Materialization output is the systematic $G = [\,I_k \mid P\,]$ in the
repository's `[message | parity]` layout, packed little-endian per
`@/inv/canonical-bit-indexing`. A family that produces $P$ alone records the
$k \times (n-k)$ shape it produced; it is not a substitute for $G$ unless the
consumer only needs $P$.

## 5. Cache state

Two declared states. Which one a cell is in follows from the API it measures,
not from a per-cell choice:

* **`warm-reuse`** — input and output buffers are allocated once outside the
  timed region and reused by every repetition. Steady-state throughput;
  first-touch page faults and allocator work are excluded. This is the state
  of every workspace-style API: the bchlib, AFF3CT and M4RI W1 cells, and
  M4RI's `matmul-m4rm`.
* **`fresh-alloc`** — the measured call allocates its own result, so allocation
  is inside the timed region. This is the state of every W2 materialization
  and of any W1 API returning an owned collection, which includes the current
  `BchEncoder::encode_batch` and `GeneratorMatrixAccess::generator_matrix`.
  A like-for-like non-regression comparison against the pre-cutover receipt
  uses it.

A consumer that adds an allocation-free encoding path measures it as
`warm-reuse` and keeps the allocating path's `fresh-alloc` cell alongside, so
the pre-cutover comparison stays like-for-like.

No cell flushes the cache between repetitions. A family whose advantage
depends on a cold cache declares that as an amendment here before measuring
it.

## 6. Worker counts

$W \in \{1,\ 6\}$.

* $W = 1$ — every external-baseline comparison. The SOTA reference acceptance
  protocol requires single-thread references, and every selected baseline is
  single-threaded, so a multi-threaded gf2 figure has nothing to compare
  against.
* $W = 6$ — the parallel batch-encoding path. Six is the width of the CCX1
  core pin (`taskset -c 6-11`) that `dev/scripts/ccx1-bench-flock.sh` applies,
  so a receipt taken through the wrapper cannot use more.

A benchmark process sets `RAYON_NUM_THREADS` explicitly. The repository-wide
value in `.cargo/config.toml` reaches processes Cargo launches, and a binary
run directly from `target/` inherits none of it, so a receipt that depends on
the launch path is not reproducible.

Determinism across $W \in \{1, 6\}$ is a hard requirement of
`@/inv/deterministic-seeded-execution`, not a performance question: the same
seed produces the same codewords at both worker counts.

## 7. Encoding algorithm families to register

Decision D-04 requires the dispatch seam to carry at least one scalar
reference and at least two registered families. Four are registered; the
survey's evidence for each is in [findings.md](findings.md) § 6.

| Family | Description | Role |
|---|---|---|
| `poly-remainder-scalar` | One message at a time, remainder of $x^{r} m(x)$ modulo $g(x)$ by the bit-serial LFSR recurrence. | **Required scalar reference.** Every other family is checked bit-identical against it. |
| `table-remainder` | Consumes 32 message bits per step through four 256-entry remainder tables over packed bytes. | Registered. The strongest measured single-frame family. |
| `bitslice-interleaved` | Transposes a batch into bit-slices and advances the LFSR across lanes, one register bit per frame. | Registered. Scales with register width rather than with $\deg g$. |
| `genmatrix-multiply` | Materializes $G$ (or $P$) once and encodes by dense GF(2) matrix product. | Registered. Amortizes only when one code encodes many batches; `genmatrix-perf` owns its materialization cost. |

Registering a family fixes its name and its contract, not its selection: the
crossover between them is `encode-dispatch`'s measurement to make, over the
cells this document fixes.

## 8. Selected external baseline per workload

Selection rationale, per-cell numbers, and the rejected candidates are in
[findings.md](findings.md) § 5 and § 7.

| Workload | Role | Library | Version | Commit / checksum | Build configuration |
|---|---|---|---|---|---|
| W1 | Primary | bchlib (userspace `lib/bch.c`, Ivan Djelic / Parrot S.A.) | v2.1.3 | `8d0656ab8f37e734428635501738d360ad80eebd` | `gcc -std=c11 -O3 -march=native`, `bch_init(m, t, prim, swap_bits=false)` |
| W1 | Secondary | AFF3CT `Encoder_BCH_inter<int>` | v4.7.0 | `e8a65c5047262d97a15563b9edc961f69b2792cc` | `g++ -std=gnu++11 -O3 -march=native -funroll-loops`, `-DAFF3CT_MULTI_PREC -DAFF3CT_EXT_STRINGS -DAFF3CT_POLAR_BIT_PACKING -DMIPP_ENABLE_BACKTRACE -DSPU_COLORS -DSPU_STACKTRACE -DNDEBUG` |
| W2 | Primary | M4RI `mzd_echelonize_m4ri` over the repository-column-order generator matrix | 20260122 | tarball sha256 `7e033ca1fd36be8861e2f67d9d124c398fc0d830209bb0226462485876346404` | `./configure --disable-static`, `CFLAGS="-O3 -march=native -fPIC"` |
| W2 | Secondary | AFF3CT `Encoder_BCH<int>`, basis-vector materialization | v4.7.0 | `e8a65c5047262d97a15563b9edc961f69b2792cc` | as above |

The two W1 entries measure different representations and are both retained:
bchlib encodes packed bytes, which is the representation `BitVec` uses, while
AFF3CT stores one 32-bit word per bit. Comparing gf2 against bchlib alone
would credit gf2 for a representation advantage it does not have; comparing
against AFF3CT alone would credit it for one it does not need.

The two W2 entries are independent materialization routes — one Gaussian
elimination of the repository-column-order generator matrix versus one encode
per basis vector. The timed-route check shares M4RI's dump helper with the timed
route; the dump is the timed route's output verbatim, and the verifier compares
it with no normalization against the repository's row-major `[message | parity]`
convention. For B2, B3, and T2S, the [generator-matrix agreement
receipt](../../bench_results/4e732b56/2026-09-01-4e732b56-generator-matrix-agreement.txt)
records matching dimensions, `bit_exact_identical=True`, and `same_code=True`.
This gives a consumer an independent check on the shape of the cost curve.
The elimination route is primary because it is measurably faster at every row
above B1, by 28.2× at T2S; the basis-vector route is retained as secondary
because it is the route the current gf2 implementation takes, which makes it
the like-for-like comparison point for a non-regression receipt.

**Amendment, 2026-08-31 (`4e732b56`).** This document predeclared the
basis-vector route as the W2 primary and the elimination route as secondary.
The survey's measurement reversed that ordering and the table above records
the corrected selection; the original is preserved in
[findings.md](findings.md) § 8.1 rather than silently replaced.

## 9. Amendment

A consumer that finds a cell here unworkable amends this document before
measuring against a different one, and records the amendment as a dated
subsection naming the JIT issue that triggered it. Silently measuring a cell
this contract does not name leaves the resulting receipt outside the
`evidence-protocol`.
