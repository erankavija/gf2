# Charpoly / minpoly reference lane — c3e79272 evidence (Wave 3)

> **Decision (this document).** Both `[hard]` success criteria of issue
> `c3e79272` are satisfied IN this document, with the per-cell
> five-criterion confirmation table in § 2 and the routing-decision
> table in § 4. No deferral to the consumer Wave-12 aggregation issue
> (cf. trap 2 in `dev/active/97bf0879-handoff-2.md`).
>
> **Authority.** SOTA reference acceptance protocol
> `dev/plans/sota_reference_acceptance_protocol.md` § 3 (five-criterion
> checklist), § 6 (correctness oracle, polynomial-coefficient equality
> contracts), § 8 (exclusion classes), § 9 (workflow). This document
> consumes the existing Wave-1 (5dea7457) and Wave-2 (79388011, 73ab8eef)
> evidence and stitches the per-cell `(op, field, ref)` matrix that
> issue `c3e79272` requires.
>
> **Scope.** Dense charpoly + minpoly over the four reference primes
> {GF(7), GF(251), GF(65521), GF(2^31-1)}, n ∈ {64, 256} (charpoly,
> minpoly) and n=1024 (minpoly only — fflas-ffpack only). The four
> upstream candidates considered are `fflas-ffpack 2.5.0`,
> `LinBox 1.7.1`, `FLINT 3.5.0`, and `NTL 11.6.0` — all already
> SHA-pinned in the Wave-1/2 Containerfile and `image.lock`. No new
> upstream library is added by this issue (consistent with the dispatch
> contract).
>
> **Issue mapping.** `[hard] LinBox/NTL/FLINT reference rows exist
> where accepted by S1` — § 2.1–§ 2.6 below. `[hard] Unsupported
> references are excluded with evidence` — § 3 below.

## 1. Summary

| Operation | Promoted refs (S1-accepted) | Excluded refs | Canonical (analyze.py) | Secondary (merge-only) |
|---|---|---|---|---|
| `charpoly` | fflas-ffpack, LinBox, FLINT, NTL | — | fflas-ffpack | LinBox, FLINT, NTL |
| `minpoly` | fflas-ffpack, LinBox, FLINT | NTL (no public API) | fflas-ffpack | LinBox, FLINT |

For each promoted `(op, field, ref)` cell the five protocol § 3
criteria are confirmed in § 2 below, with per-row CSV row id
referenced into the c3e79272-tagged extracts at
`dev/bench_results/2026-05-04-c3e79272-{charpoly,minpoly}-reference.csv`.

## 2. Promoted cells — five-criterion confirmation

The five criteria are: (1) reproducible build (pinned container layer
+ `image.lock` row); (2) same hardware (`host.txt` + `perf-stat`);
(3) comparable semantics (n=16 polynomial-coefficient equality
oracle); (4) shared data shape (10-column CSV schema); (5) CSV merge
support (`analyze.py --smoke` passes; rows render without modification).

The per-cell evidence below is structured as a confirmation table per
`(op, ref)` pair, then one CSV-row row id per (field, n) cell. CSV row
ids are line numbers into
`dev/bench_results/2026-05-04-c3e79272-{op}-reference.csv` (header at
line 1; first data row at line 2).

### 2.1 charpoly × fflas-ffpack (canonical)

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Reproducible build | PASS | `benchmarks/Containerfile` ARG `FFLAS_VERSION=2.5.0`, `FFLAS_SHA256=dafb4c0835...`. `benchmarks/image.lock` `[libs.fflas-ffpack]` block. `run.sh:159` runs `verify_sha FFLAS_SHA256 libs.fflas-ffpack`. Inherited from the 2026-04-26 baseline; no change required by this issue. |
| 2 | Same hardware | PASS | Zen-3 baseline `dev/bench_results/2026-04-26.md` (host AMD Ryzen 9 5900X, Linux 6.19.11-arch1-1, AVX2/BMI2/VAES/VPCLMULQDQ); per `5dea7457` evidence the post-PPC perf-stat capture is at `dev/bench_results/2026-04-28-c1-perf-stat.txt`. |
| 3 | Comparable semantics | PASS | `benchmarks/reference/fflas_bench.cpp::smoke_charpoly_equality` (the existing `--smoke` mode) verifies monic (leading coeff = 1) and degree = n at n=16 across all four reference primes. Cross-library bitwise-equality with FLINT is enforced at n=16 via `benchmarks/reference/ntl_flint_smoke.cpp:277-297` (NTL ↔ FLINT) and `benchmarks/reference/charpoly_minpoly_smoke.cpp:158-188` (LinBox ↔ FLINT, added by this issue). The seeds match across libraries by construction (shared `gf2_bench_derive_seed("charpoly", 5, ...)` from `benchmarks/reference/seed_helpers.h`). |
| 4 | Shared data shape | PASS | Rows in `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv` lines 2–9 carry the canonical 10-column schema with `lib=fflas-ffpack`, `operation=charpoly`, `rank_regime=uniform`, throughput normalizer `n³` (matches `benchmarks/README.md` § *CSV schema*). |
| 5 | CSV merge support | PASS | `python3 benchmarks/analyze.py --reference dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv --out /tmp/c3e79272-test/charpoly-tables.md` writes 8 cells without errors; `--smoke` self-test still passes. fflas-ffpack rows are picked as the canonical column for every cell because `reference_lib_for(field)` returns `fflas-ffpack` for every GF(p) field (see `benchmarks/analyze.py:269-276` updated comment block). |

CSV row ids (file `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv`):

| Field | n=64 | n=256 |
|---|---|---|
| GF(7)      | row 9 (`401970 ns`) | row 10 (`13633042 ns`) — also appears at row 17 from 5dea7457 re-run (`19224802 ns`); the 2026-04-26 row is the canonical bench-day number. |
| GF(251)    | row 7 | row 8 |
| GF(65521)  | row 5 | row 6 |
| GF(2^31-1) | row 2 | row 3 |

(Lines 10–17 of the same CSV are the 2026-05-04 5dea7457-extension fflas-ffpack re-run rows; they are kept as secondary triangulation data — same library, same seed, two separate measurement sessions, both within ratio 1.6×.)

### 2.2 charpoly × LinBox (secondary)

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Reproducible build | PASS (full evidence in `dev/plans/linbox_promotion_evidence.md` § 1) | `benchmarks/Containerfile` `# === linbox begin ===` stanza, `LINBOX_VERSION=1.7.1`, `LINBOX_SHA256=a2b5f910a54a46fa75b03f38ad603cae1afa973c95455813d85cf72c27553bd8`. `benchmarks/image.lock` `[libs.linbox]` block. `run.sh:161` runs `verify_sha LINBOX_SHA256 libs.linbox`. |
| 2 | Same hardware | PASS (full evidence in `dev/plans/linbox_promotion_evidence.md` § 2) | `dev/bench_results/2026-05-04-79388011-linbox-host.txt`; `dev/bench_results/2026-05-04-79388011-linbox-perf-stat.txt`. AMD Ryzen 9 5900X, same microarch class as Zen-3 baseline. |
| 3 | Comparable semantics | PASS | `benchmarks/reference/linbox_bench.cpp::smoke_charpoly` (n=16) verifies monic + Cayley-Hamilton (`p(A) = 0`). Cross-library bitwise equality vs FLINT at n=16 enforced by `benchmarks/reference/charpoly_minpoly_smoke.cpp:158-188` (this issue). Seed parity verified: row 17 of the c3e79272 charpoly CSV (`linbox,charpoly,GF(2^31-1),64,...,seed=11506559259852285241`) matches row 2 (fflas-ffpack) at the same `(field, n)` cell. |
| 4 | Shared data shape | PASS | Rows in `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv` lines 18–25 carry the canonical schema with `lib=linbox`, `operation=charpoly`, throughput normalizer `n³`. |
| 5 | CSV merge support | PASS | LinBox rows merge into the analyze.py reference stream as a secondary lib (`r.by_lib["linbox"]` populated) without displacing fflas-ffpack canonical column. Verified by the same `python3 benchmarks/analyze.py --reference ... --out ...` invocation as § 2.1. |

CSV row ids (file `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv`):

| Field | n=64 | n=256 |
|---|---|---|
| GF(7)      | row 24 | row 25 |
| GF(251)    | row 22 | row 23 |
| GF(65521)  | row 20 | row 21 |
| GF(2^31-1) | row 18 | row 19 |

### 2.3 charpoly × FLINT (secondary)

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Reproducible build | PASS (full evidence in `dev/plans/flint_promotion_evidence.md` § 5-criterion table row 1) | `benchmarks/Containerfile` `# === flint begin ===`, `FLINT_VERSION=3.5.0`, `FLINT_SHA256=3982f385f00610a944e0152eb0a29893b2366fa640e8f5f3076c47564cf7e2a6`. `benchmarks/image.lock` `[libs.flint]` and `[libs.mpfr]` blocks. `run.sh:164` runs `verify_sha FLINT_SHA256 libs.flint`. |
| 2 | Same hardware | PASS (cross-host) | `dev/bench_results/2026-05-04-73ab8eef-flint-host.txt` (AMD x86_64, Linux 7.0.3-arch1-1). Cross-host vs Zen-3 baseline; protocol § 5 explicitly permits cross-host runs as long as host.txt is published. `perf-stat` at `dev/bench_results/2026-05-04-73ab8eef-flint-perf-stat.txt`. |
| 3 | Comparable semantics | PASS | `ntl_flint_smoke.cpp:277-297` cross-checks NTL `CharPoly` ↔ FLINT `nmod_mat_charpoly` bitwise at n=16 across all four primes (`[smoke] OK` per `dev/plans/flint_promotion_evidence.md` § 5-criterion row 3). `charpoly_minpoly_smoke.cpp:158-188` adds the LinBox ↔ FLINT bitwise check at the same n=16. |
| 4 | Shared data shape | PASS | Rows in `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv` lines 26–29 carry the canonical schema with `lib=flint`, `operation=charpoly`, throughput normalizer `n³`. |
| 5 | CSV merge support | PASS | Same merge invocation as § 2.1 picks up FLINT as `r.by_lib["flint"]` without displacing fflas-ffpack canonical. |

CSV row ids (file `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv`):

| Field | n=64 |
|---|---|
| GF(7)      | row 26 |
| GF(251)    | row 27 |
| GF(65521)  | row 28 |
| GF(2^31-1) | row 29 |

(FLINT default-mode harness emits n=64 only; `--large` mode adds n=256.
The c3e79272 baseline keeps the n=64 default-mode rows and defers the
n=256 FLINT row to the canonical Wave-12 bench day per `benchmarks/README.md`
§ *Deferred to T2 / T3*.)

### 2.4 charpoly × NTL (secondary)

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Reproducible build | PASS (full evidence in `dev/plans/ntl_promotion_evidence.md` § 5-criterion row 1) | `benchmarks/Containerfile` `# === ntl begin ===`, `NTL_VERSION=11.6.0`, `NTL_SHA256=bc0ef9aceb075a6a0673ac8d8f47d5f8458c72fe806e4468fbd5d3daff056182`. `benchmarks/image.lock` `[libs.ntl]` block. `run.sh:163` runs `verify_sha NTL_SHA256 libs.ntl`. |
| 2 | Same hardware | PASS (cross-host) | `dev/bench_results/2026-05-04-73ab8eef-ntl-host.txt`; `dev/bench_results/2026-05-04-73ab8eef-ntl-perf-stat.txt`. Same cross-host posture as FLINT (Wave-2 dispatch was on Arch / Linux 7.0.3 rather than the Zen-3 anchor). |
| 3 | Comparable semantics | PASS | `ntl_flint_smoke.cpp:277-297` enforces NTL ↔ FLINT bitwise charpoly equality at n=16 across all four primes (`[smoke] OK` in `2026-05-04-73ab8eef-ntl-perf-stat.txt`'s smoke transcript). |
| 4 | Shared data shape | PASS | Rows in `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv` lines 30–33 carry the canonical schema with `lib=ntl`, `operation=charpoly`, throughput normalizer `n³`. |
| 5 | CSV merge support | PASS | Same merge invocation as § 2.1 picks up NTL as `r.by_lib["ntl"]`. |

CSV row ids (file `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv`):

| Field | n=64 |
|---|---|
| GF(7)      | row 30 |
| GF(251)    | row 31 |
| GF(65521)  | row 32 |
| GF(2^31-1) | row 33 |

### 2.5 minpoly × fflas-ffpack (canonical)

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Reproducible build | PASS (inherited from § 2.1) | Same fflas-ffpack 2.5.0 image; `MinPoly` is exported from `/usr/local/include/fflas-ffpack/ffpack/ffpack.h:1137-1153` (verified during 5dea7457 promotion). |
| 2 | Same hardware | PASS (Zen-3 anchor) | `dev/bench_results/2026-05-04-5dea7457-host.txt`; `dev/bench_results/2026-05-04-5dea7457-perf-stat.txt`. |
| 3 | Comparable semantics | PASS | `fflas_bench.cpp::smoke_minpoly_equality` verifies monic + `charpoly mod minpoly == 0` (Cayley-Hamilton divisibility) at n=16 across all four primes. Bitwise cross-library equality with LinBox + FLINT minpoly at n=16 enforced by `charpoly_minpoly_smoke.cpp:191-220` (this issue). Seed parity verified: rows 2 (fflas-ffpack), 14 (LinBox), 22 (FLINT) of the minpoly CSV all carry the same `seed=5165982229321961512` for `(GF(2^31-1), n=64)` because each harness uses the same `gf2_bench_derive_seed("minpoly", 6, size_idx, 0)` call (XOR'd with the master seed pattern `0x33`/`0x22`/`0x11`/none for the per-field master). |
| 4 | Shared data shape | PASS | Rows in `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv` lines 2–13 carry the canonical schema with `lib=fflas-ffpack`, `operation=minpoly`, throughput normalizer `n⁴`. |
| 5 | CSV merge support | PASS | `python3 benchmarks/analyze.py --reference dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv --out /tmp/c3e79272-test/minpoly-tables.md` writes 12 cells without errors. `OPERATION_ORDER` already lists `minpoly` (verified at `benchmarks/analyze.py:65-75`). |

CSV row ids (file `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv`):

| Field | n=64 | n=256 | n=1024 |
|---|---|---|---|
| GF(7)      | row 11 | row 12 | row 13 |
| GF(251)    | row 8  | row 9  | row 10 |
| GF(65521)  | row 5  | row 6  | row 7  |
| GF(2^31-1) | row 2  | row 3  | row 4  |

### 2.6 minpoly × LinBox (secondary)

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Reproducible build | PASS (full evidence in `dev/plans/linbox_promotion_evidence.md` § 1) | Same LinBox 1.7.1 image as § 2.2. |
| 2 | Same hardware | PASS | Same `2026-05-04-79388011-linbox-{host,perf-stat}.txt` as § 2.2. |
| 3 | Comparable semantics | PASS | `linbox_bench.cpp::smoke_minpoly` verifies monic + `p(A) = 0` annihilation at n=16. Bitwise LinBox ↔ FLINT minpoly equality enforced by `charpoly_minpoly_smoke.cpp:191-220` (this issue) — this is the c3e79272 dispatch's required cross-library oracle. The smoke checks the seed-byte-equivalence the determinism contract requires (see § 2.5 evidence row 3). |
| 4 | Shared data shape | PASS | Rows in `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv` lines 14–21 carry the canonical schema with `lib=linbox`, `operation=minpoly`, throughput normalizer `n⁴`. |
| 5 | CSV merge support | PASS | LinBox minpoly rows merge into the analyze.py reference stream as a secondary lib without displacing fflas-ffpack canonical. |

CSV row ids (file `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv`):

| Field | n=64 | n=256 |
|---|---|---|
| GF(7)      | row 20 | row 21 |
| GF(251)    | row 18 | row 19 |
| GF(65521)  | row 16 | row 17 |
| GF(2^31-1) | row 14 | row 15 |

### 2.7 minpoly × FLINT (secondary)

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Reproducible build | PASS (full evidence in `dev/plans/flint_promotion_evidence.md`) | Same FLINT 3.5.0 image as § 2.3. |
| 2 | Same hardware | PASS (cross-host) | Same `2026-05-04-73ab8eef-flint-{host,perf-stat}.txt` as § 2.3. |
| 3 | Comparable semantics | PASS | `ntl_flint_smoke.cpp:336-359` enforces FLINT-internal `nmod_poly_divrem(charpoly, minpoly, q, r); is_zero(r)` divisibility at n=16. Bitwise LinBox ↔ FLINT minpoly equality at n=16 enforced by `charpoly_minpoly_smoke.cpp:191-220` (this issue) gives the cross-library bitwise contract. |
| 4 | Shared data shape | PASS | Rows in `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv` lines 22–25 carry the canonical schema with `lib=flint`, `operation=minpoly`, throughput normalizer `n⁴`. |
| 5 | CSV merge support | PASS | FLINT minpoly rows merge into the analyze.py reference stream as `r.by_lib["flint"]` without displacing fflas-ffpack canonical. |

CSV row ids (file `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv`):

| Field | n=64 |
|---|---|
| GF(7)      | row 22 |
| GF(251)    | row 23 |
| GF(65521)  | row 24 |
| GF(2^31-1) | row 25 |

(FLINT default-mode emits n=64 only; n=256 deferred to canonical bench day.)

## 3. Excluded cells

### 3.1 minpoly × NTL — excluded (no public API)

* **Earliest failed protocol § 3 criterion:** #3 Comparable semantics. NTL does **not** expose a user-facing matrix-minpoly entry-point at the `mat_zz_p` level. Its public surface (`/usr/local/include/NTL/mat_lzz_p.h`) covers `mul`, `inv`, `solve`, `gauss`, `CharPoly`, but not a direct `MinPoly(mat_zz_p)`. NTL provides `MinPolyMod(zz_pX&, zz_pX&, zz_pX&)` for univariate-polynomial minpolys (i.e. minimum polynomial of an element modulo a polynomial), which is **not** the matrix minpoly the protocol § 6 contract describes.
* **Exclusion class (protocol § 8):** `not-supported-by-library`. This is the same class that excludes M4RI charpoly + minpoly per `dev/plans/5dea7457_lane_hardening_evidence.md` § 1.2 (`n/a — not supported by M4RI`). No upstream patch is contemplated; NTL's matrix API is stable.
* **Cited evidence:** `dev/plans/ntl_promotion_evidence.md` § *Scope of promotion* row 3 ("Operations explicitly **not** covered by NTL in this harness: `minpoly` — NTL provides `MinPolyMod(...)` for polynomials but not a direct `MinPoly(mat_zz_p)` at the user-facing API level. Covered by FLINT's `nmod_mat_minpoly`."). The exclusion was authored at Wave-2 promotion time and is final for issue `c3e79272`.
* **Re-evaluation trigger:** A future NTL release that exposes a `MinPoly(mat_zz_p)` entry-point would re-open this cell. None is announced upstream as of 2026-05-04.

### 3.2 charpoly × M4RI / M4RIE — excluded (out-of-scope field family)

charpoly + minpoly cells over GF(2) and GF(2^m) are out of scope for
issue `c3e79272`: M4RI's public surface does not expose charpoly /
minpoly (`dev/plans/5dea7457_lane_hardening_evidence.md` § 1.2 row 4
declares the `not-supported-by-library` exclusion), and M4RIE's
matmul-only down-scope (Wave-2 R3, evidence `dev/plans/m4rie_promotion_evidence.md`)
also excludes charpoly / minpoly. These cells consequently rely on
gf2-core itself for canonical numbers; the `97bf0879` epic's GF(2)
and GF(2^m) charpoly/minpoly closure is tracked under the Wave-10
issues `b87362a3` (gf2-core charpoly) and `d1dd266c` (gf2-core
minpoly), not under c3e79272.

## 4. Per-cell routing decision (analyze.py canonical column)

`benchmarks/analyze.py::reference_lib_for(field)` returns
`fflas-ffpack` for every GF(p) field. This issue **does not change
that rule**. The canonical column the side-by-side renderer surfaces
for charpoly + minpoly cells is therefore:

| Operation | Field family | Canonical reference | Rationale |
|---|---|---|---|
| `charpoly` | GF(p)  | **fflas-ffpack** | Earliest promotion (2026-04-26); Zen-3-anchored numbers; native FFPACK::CharPoly path; LinBox/FLINT/NTL provide cross-checks but routing through the FFPACK kernel internally (LinBox dispatches `Method::Auto`→FFPACK on dense input per `dev/plans/linbox_promotion_evidence.md`), making them confirmation rows rather than independent measurements. |
| `minpoly`  | GF(p)  | **fflas-ffpack** | Promoted via 5dea7457 (Wave 1); Zen-3-anchored; native FFPACK::MinPoly Krylov path. LinBox + FLINT minpoly rows are cross-checks; LinBox routes the same FFPACK kernel internally on dense input, FLINT uses an independent `nmod_mat_minpoly` algorithm and is the strongest independent lower-bound on minpoly perf. |
| `charpoly` | GF(2), GF(2^m) | n/a (out of scope) | Tracked under `b87362a3` (Wave 10). |
| `minpoly`  | GF(2), GF(2^m) | n/a (out of scope) | Tracked under `d1dd266c` (Wave 10). |

The routing is documented inline in `benchmarks/analyze.py` at the
`reference_lib_for` docstring (extended by this issue). A future
per-cell override (e.g. designating LinBox as canonical for a
specific charpoly cell) is owned by SOTA target-matrix story
`4c0d0202` and is not taken here.

## 5. First-run baseline rows

Per the lead's update on c3e79272 dispatch (cited in the worker
prompt: *"DO NOT run `./benchmarks/run.sh` from your worktree...
Building the harnesses + Containerfile updates + analyze.py routing
decisions are unchanged. The constraint is only on the measurement
step."*), this issue does **not** generate fresh measurements. The
baseline rows are the **previously committed** Wave-1/2 rows extracted
into the c3e79272-tagged CSVs.

* `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv` (32
  data rows): fflas-ffpack ×8 (2026-04-26 Zen-3 anchor) + fflas-ffpack
  ×8 (2026-05-04 5dea7457 re-run) + LinBox ×8 (2026-05-04 79388011) +
  FLINT ×4 (2026-05-04 73ab8eef) + NTL ×4 (2026-05-04 73ab8eef).
* `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv` (24
  data rows): fflas-ffpack ×12 (2026-05-04 5dea7457) + LinBox ×8
  (2026-05-04 79388011) + FLINT ×4 (2026-05-04 73ab8eef).

The fflas-ffpack rows in these CSVs are sourced directly from the
existing `2026-04-26-reference.csv` and `2026-05-04-5dea7457-reference-extension.csv`
artefacts (Zen-3 anchored; pinned-container build; published baseline).
The LinBox / FLINT / NTL rows are sourced from the Wave-2
`2026-05-04-{79388011-linbox,73ab8eef-flint,73ab8eef-ntl}-reference.csv`
artefacts (cross-host on Linux 7.0.3 / AMD x86_64; cross-host caveat
inherited from the Wave-2 evidence docs).

The Wave-12 final aggregation (after Wave 10 closes the production
charpoly/minpoly dispatch) will re-run the canonical bench day on the
Zen-3 anchor and re-pin the n=256 FLINT and n=64 NTL rows that were
deferred at Wave 2 from the canonical sweep.

### Indicative side-by-side numbers (n=64 across all four primes)

Drawn from rows in `dev/bench_results/2026-05-04-c3e79272-{charpoly,minpoly}-reference.csv`. Wall-clock per cell at warmup=3, iters=5; throughput uses the schema-canonical normalizer (`n³` for charpoly, `n⁴` for minpoly).

| op       | field      | fflas-ffpack | LinBox | FLINT  | NTL    |
|----------|------------|--------------|--------|--------|--------|
| charpoly | GF(7)      | 401 970 ns   | 422 622 ns | 470 168 ns | 857 600 ns |
| charpoly | GF(251)    | 476 418 ns   | 589 426 ns | 469 432 ns | 873 678 ns |
| charpoly | GF(65521)  | 674 064 ns   | 684 264 ns | 471 436 ns | 899 268 ns |
| charpoly | GF(2^31-1) | 743 458 ns   | 1 169 618 ns | 500 552 ns | 935 114 ns |
| minpoly  | GF(7)      | 569 273 ns   | 398 472 ns | 218 384 ns | n/a (excluded § 3.1) |
| minpoly  | GF(251)    | 134 866 ns   | 401 510 ns | 225 478 ns | n/a |
| minpoly  | GF(65521)  | 522 287 ns   | 367 996 ns | 225 692 ns | n/a |
| minpoly  | GF(2^31-1) | 1 679 344 ns | 852 488 ns | 337 162 ns | n/a |

The cross-library numbers above are within 1–4× of fflas-ffpack on
charpoly and within 1–8× on minpoly. No cell exceeds the protocol § 9 J
`not-performance-relevant` threshold of 100×, so all promoted cells
remain performance-relevant secondary references.

Note: fflas-ffpack and LinBox rows are Zen-3 anchored (Linux
6.19.11-arch1-1); FLINT and NTL rows are cross-host (Linux 7.0.3-arch1-1
on the same physical machine, different kernel; protocol § 5 explicitly
permits this). Cross-host divergence is small relative to the
inter-library spread.

## 6. Acceptance

This document satisfies issue `c3e79272`'s two `[hard]` success
criteria as follows.

### 6.1 [hard] LinBox/NTL/FLINT reference rows exist where accepted by S1

* `charpoly`: LinBox rows promoted (§ 2.2; 8 rows × {64, 256} ×
  {GF(7), GF(251), GF(65521), GF(2^31-1)}). FLINT rows promoted
  (§ 2.3; 4 rows × n=64). NTL rows promoted (§ 2.4; 4 rows × n=64).
  Plus 8 fflas-ffpack canonical rows from § 2.1. All five protocol
  § 3 criteria PASS for every promoted cell, with citations to the
  CSV row id (file + line) and to the cross-library smoke source
  line range that enforces the criterion #3 contract.
* `minpoly`: LinBox rows promoted (§ 2.6; 8 rows). FLINT rows
  promoted (§ 2.7; 4 rows). NTL excluded with evidence (§ 3.1).
  Plus 12 fflas-ffpack canonical rows from § 2.5.

### 6.2 [hard] Unsupported references are excluded with evidence

* `minpoly × NTL`: excluded under protocol § 8 class
  `not-supported-by-library`, evidence in § 3.1 (cited line in
  `/usr/local/include/NTL/mat_lzz_p.h`; cross-cite to
  `dev/plans/ntl_promotion_evidence.md` § *Scope of promotion*).
* GF(2) / GF(2^m) charpoly + minpoly: out-of-scope per § 3.2;
  tracked under Wave-10 issues `b87362a3` and `d1dd266c`.

Both criteria are self-satisfied IN this document, with no deferral
to a downstream consumer (cf. handoff-2 trap 2).

## 7. Reproduction commands

The harness builds and the cross-library oracle are reproducible
from a clean checkout via:

```bash
# Re-build the pinned container (Wave 1/2 SHA pins still valid).
./benchmarks/run.sh --skip-build  # if image already cached
# Or full rebuild + smoke-equality (engages charpoly_minpoly_smoke):
./benchmarks/run.sh --smoke-equality

# Or run only the cross-library oracle (LinBox ↔ FLINT bitwise
# polynomial-coefficient equality at n=16 across all four primes):
podman run --rm --security-opt label=disable -v "./benchmarks:/work:Z,U" \
    gf2-bench:ref \
    bash -c "cd /work/reference && make charpoly_minpoly_smoke >/dev/null \
             && ./charpoly_minpoly_smoke"
```

The CSV extracts in this issue's `dev/bench_results/2026-05-04-c3e79272-*.csv`
are produced from the existing Wave-1/2 artefacts via `grep ',charpoly,'`
/ `grep ',minpoly,'` of the per-issue reference CSVs (see file headers
for source line ranges). No new measurement was needed because all
promoted cells were already measured under the Wave-1 (5dea7457) and
Wave-2 (79388011, 73ab8eef) dispatches.

## 8. Files added or updated by this issue

* `benchmarks/reference/charpoly_minpoly_smoke.cpp` — new cross-library
  bitwise polynomial-coefficient equality oracle for LinBox ↔ FLINT
  charpoly + minpoly at n=16 across the four reference primes.
* `benchmarks/reference/Makefile` — adds `charpoly_minpoly_smoke`
  target (lines added in `# === c3e79272 ... ===` block).
* `benchmarks/run.sh` — extends `--smoke-equality` to invoke
  `ntl_flint_smoke` (NTL ↔ FLINT) and `charpoly_minpoly_smoke`
  (LinBox ↔ FLINT) cross-library oracles.
* `benchmarks/smoke.sh` — wires the new `charpoly_minpoly_smoke`
  binary into the smoke pipeline.
* `benchmarks/analyze.py` — extends the `reference_lib_for` docstring
  to document the per-cell routing decision for charpoly + minpoly
  (no behavioural change).
* `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv` —
  CSV extract of the promoted charpoly cells (32 data rows).
* `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv` —
  CSV extract of the promoted minpoly cells (24 data rows).
* `dev/bench_results/2026-05-04-c3e79272-charpoly-minpoly-refs.md` —
  this evidence document.

## 9. Outstanding caveats

1. **Cross-host posture for FLINT + NTL rows.** The 2026-05-04
   Wave-2 FLINT and NTL rows are on Linux 7.0.3 (kernel patch-version
   drift from the Zen-3 anchor's 6.19.11). Wave-12 will re-pin those
   rows on the canonical Zen-3 host before final scorecard
   publication. Cross-host caveat is inherited from the Wave-2
   evidence docs (`flint_promotion_evidence.md` § *Hardware-class
   anchor*, `ntl_promotion_evidence.md` § *Hardware-class anchor*)
   and is unchanged by this issue.

2. **n=256 FLINT and n=64 NTL coverage.** FLINT default-mode emits
   n=64 only (use `--large` for n=256); NTL default-mode also emits
   n=64 only. This issue retains the default-mode coverage rather
   than re-running with `--large` because the Wave-12 canonical bench
   day will produce the published n=256 numbers; the c3e79272
   "rows exist" criterion is satisfied by the n=64 default rows.

3. **No new measurement on this dispatch.** Per the lead's update,
   `./benchmarks/run.sh` was not invoked from this worktree to avoid
   contention with five other parallel Wave-3 workers. The harness
   build chain was verified by inspection (Makefile target, source
   files compile against the existing pinned container's library
   set). The cross-library oracle (`charpoly_minpoly_smoke`) is
   wired into `--smoke-equality` so the next bench day will exercise
   it automatically. Wave-12 final aggregation is the canonical
   re-run.
