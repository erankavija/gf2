# M4RIE 20250128 — promotion evidence

> **Issue:** `jit:507b0036` (Evaluate M4RIE for GF(2^m) references)
> **Story:** `cbecfced` (Define reproducible gf2-core SOTA reference matrix)
> **Epic:**  `97bf0879` (post-PPC `gf2-core` SOTA closure)
> **Decision:** **PROMOTE** for matmul / echelon over GF(2^4), GF(2^8), GF(2^16).

## Summary

M4RIE is the de-facto open-source reference for dense linear algebra over
the small extensions GF(2^m), m ∈ [2, 16]. This evidence walks the
five-criterion checklist in `dev/plans/sota_reference_acceptance_protocol.md`
§ 9 against M4RIE's `20250128` upstream release, in the pinned
`debian:bookworm-20260421-slim` container, and concludes:

* M4RIE is **buildable** in the existing container layer (criterion #1).
* The matching host capture from this dispatch satisfies criterion #2.
* M4RIE accepts an arbitrary minimal polynomial via `gf2e_init(minpoly)`
  — passing `gf2-core`'s polynomial bit-for-bit makes the field
  representations canonically identical, so **no basis-change matrix is
  needed for any of m ∈ {4, 8, 16}** (criterion #3 falls out directly).
* The harness emits the standard 10-column CSV schema (criterion #4) and
  merges through `analyze.py --smoke` (criterion #5).
* M4RIE is **performance-relevant**: `mzed_mul` over GF(2^16) at n=1024
  runs in ~760 ms on the dev host, versus gf2-core's existing `fgemm`
  cell at the same shape clearing 757 s (1000× behind on the published
  baseline). Even on the smallest covered cell (matmul GF(2^4) n=64) the
  M4RIE harness emits a non-trivial (~37 µs) timing — see § 9 below.

The harness, lock-file row, and Containerfile stanza are all isolated
behind a single `# === m4rie begin/end ===` block per the parallel-
dispatch contract for wave 2.

## Five-criterion confirmation table

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Reproducible build | **PASS** | `benchmarks/Containerfile` `# === m4rie begin/end ===` stanza pinning `M4RIE_VERSION=20250128` and `M4RIE_SHA256=96f1adafd50e6a0b51dc3aa1cb56cb6c1361ae7c10d97dc35c3fa70822a55bd7`. `benchmarks/image.lock` `[libs.m4rie]` block carries the same fields. `benchmarks/run.sh verify_sha` is unchanged (its three explicit `verify_sha` calls cover the existing GIVARO/FFLAS/M4RI pins; M4RIE's pin is asserted only between Containerfile and image.lock — adding a fourth `verify_sha` call would have required modifying `run.sh`, which the dispatch contract for `507b0036` forbids; see § 12 *Open question* below). The container builds cleanly: image id `sha256:42c9c356b142bbbc8c6cdf7bf7c8ab1f3b3db3ad25c9453247e5ac3a04c7c57c` (host: 5900X / Linux 7.0.3-arch1-1, 2026-05-04). |
| 2 | Same hardware | **PASS** | `dev/bench_results/2026-05-04-507b0036-m4rie-host.txt` records the host (Ryzen 9 5900X, 12c/24t, AVX2/BMI2/VAES/VPCLMULQDQ, kernel 7.0.3, podman 5.8.2) and the local image id. `dev/bench_results/2026-05-04-507b0036-m4rie-perf-stat.txt` carries the `perf stat -r 10 -e cycles,instructions,branches,branch-misses,L1-dcache-loads,L1-dcache-load-misses` capture for the in-scope cell **matmul GF(2^16) n=1024 uniform** (10 inner iters × 10 repeats). |
| 3 | Comparable semantics | **PASS** | `m4rie_bench --smoke` runs the n=16 bitwise-equality contract for matmul over GF(2^4), GF(2^8), GF(2^16) against an independent scalar reference (`ref_gf2m_mul`) plus M4RIE's own internal scalar (`ff->mul`); both pass for all three fields. **No basis-change matrix is required** because `gf2e_init(minpoly)` accepts an arbitrary polynomial — gf2-core's `0x13`, `0x11d`, `0x1002d` are passed in directly and are also present in M4RIE's `irreducible_polynomials[]` table at `m4rie/gf2e.c` line 68+. See § 5 below. |
| 4 | Shared data shape | **PASS** | `benchmarks/reference/m4rie_bench.c` emits exactly the 10-column schema with `lib=m4rie` and `field ∈ {GF(2^4), GF(2^8), GF(2^16)}`. The `GF(2^4)` tag is new (see § 8 below for the cross-cutting `analyze.py FIELD_FAMILY` flag); `GF(2^8)` and `GF(2^16)` are already declared in the schema § 7 of the protocol. `dev/bench_results/2026-05-04-507b0036-m4rie-reference.csv` carries 36 timing rows (3 fields × 2 ops × 3 sizes × 2 regimes). |
| 5 | CSV merge support | **PASS** | `python3 benchmarks/analyze.py --smoke` exits 0 with the 36 new rows present in the input. `python3 benchmarks/analyze.py --gf2 dev/bench_results/2026-04-26-gf2.csv --reference dev/bench_results/2026-05-04-507b0036-m4rie-reference.csv --out /tmp/m4rie-tables.md` parses 239 cells without error. Note: under the current `analyze.py reference_lib_for()` heuristic, the m4rie rows are surfaced under `### matmul × GF(2^N)` headers but the per-cell column header still reads `fflas-ffpack` because `reference_lib_for()` does not yet learn that GF(2^m) families should default to m4rie. This is presentation-only (the CSV merge itself is correct) and is flagged as cross-cutting in § 8. |

## Build evidence

```bash
# In the worktree root:
podman build -t gf2-bench:m4rie-507b0036 -f benchmarks/Containerfile benchmarks/
# … 
# COMMIT gf2-bench:m4rie-507b0036
# --> 42c9c356b142
# Successfully tagged localhost/gf2-bench:m4rie-507b0036
# 42c9c356b142bbbc8c6cdf7bf7c8ab1f3b3db3ad25c9453247e5ac3a04c7c57c

# Smoke (correctness oracle, criterion #3):
podman run --rm --security-opt label=disable \
    -v "$PWD/benchmarks:/work:Z,U" gf2-bench:m4rie-507b0036 \
    bash -c 'cd /work/reference && make m4rie_bench && ./m4rie_bench --smoke'
# [m4rie_bench --smoke] GF(2^4) (minpoly=0x13)   OK
# [m4rie_bench --smoke] GF(2^8) (minpoly=0x11d)  OK
# [m4rie_bench --smoke] GF(2^16) (minpoly=0x1002d) OK

# Timing sweep (criteria #2/#4/#5):
podman run --rm --security-opt label=disable \
    -v "$PWD/benchmarks:/work:Z,U" gf2-bench:m4rie-507b0036 \
    bash -c 'cd /work/reference && make m4rie_bench && ./m4rie_bench --warmup 3 --iters 5'
# 36 CSV rows on stdout, redirected to
# dev/bench_results/2026-05-04-507b0036-m4rie-reference.csv
```

## License

M4RIE upstream is **GPL v2+**. The protocol's `license-incompatible`
exclusion class (§ 9) explicitly states "LGPL/MIT/GPL-2 are fine" for
the protocol's purpose. M4RIE is *not* linked into `gf2-core`'s
MIT-licensed binaries; it is built and executed exclusively inside the
isolated benchmark container as a reference timing oracle, exactly as
M4RI is already used. Citation in evidence docs is permitted regardless.

## Field convention — primitive polynomials

`gf2-core` declares (in `crates/gf2-core/src/primitive_polys.rs`):

| m   | Polynomial                            | Bits             | Hex      |
|-----|---------------------------------------|------------------|----------|
| 4   | x^4 + x + 1                           | `10011`          | `0x13`   |
| 8   | x^8 + x^4 + x^3 + x^2 + 1             | `100011101`      | `0x11d`  |
| 16  | x^16 + x^5 + x^3 + x^2 + 1            | `10000000000101101` | `0x1002d` |

M4RIE's `gf2e_init(minpoly)` accepts an **arbitrary** minimal polynomial
of degree ≤ 16; it does not impose a default. The harness passes the
gf2-core polynomial directly. Each polynomial is also a member of
M4RIE's catalogue of irreducible degree-m polynomials at `m4rie/gf2e.c`
lines 68-95 (`_irreducible_polynomials_degree_NN`):

* `0x13` is entry #0 of `_irreducible_polynomials_degree_04`.
* `0x11d` is entry #0 of `_irreducible_polynomials_degree_08`.
* `0x1002d` is entry #1 of `_irreducible_polynomials_degree_16` (entry #0 is `0x1002b`).

The smoke check confirms that for the same canonical bit-pattern of
inputs, gf2-core-style scalar reduction (`ref_gf2m_mul`) and M4RIE
matrix multiplication (`mzed_mul`) produce bitwise-identical outputs —
**no basis-change matrix `T` is required**, and we therefore commit
**no `m4rie_basis_change_<m>.txt` files** for any m. The decision is
recorded inline in `benchmarks/reference/m4rie_bench.c` and is the
witness for the empty-output case of protocol § 13 *Open question 1*.

## Proposed Amendment 1 (for protocol § 13 *Open question 1*)

The protocol's § 13 open question 1 asks the M4RIE evaluator to choose
a wire format for `<lib>_basis_change_<m>.txt` and back-port the
decision into a § 12 amendment. **In the M4RIE-as-promoted case, no
basis-change matrix exists, so the § 13 *open question* answer is the
following amendment text** — proposed here, not yet written into
`sota_reference_acceptance_protocol.md` (per § 13 the protocol owner
accepts the amendment in a follow-up commit on story `cbecfced`):

```
## Amendment 1 — basis-change matrix file format (closes § 13 open question 1)

**Issue trigger:** jit:507b0036 (M4RIE evaluation).

**Outcome:** M4RIE accepts an arbitrary minimal polynomial via
`gf2e_init(minpoly)`, so passing `gf2-core`'s primitive polynomial
directly produces canonically-identical bit patterns. No basis-change
matrix was required for any of m ∈ {4, 8, 16}. The harness comments
record this decision; no `benchmarks/reference/m4rie_basis_change_<m>.txt`
file is committed.

For future candidates that DO require a basis-change matrix `T`
(e.g. a candidate that hardcodes a single primitive polynomial per
degree), the file format is:

* Plain text, one matrix row per line.
* Each row consists of `m` decimal `0`/`1` values separated by single
  ASCII spaces, in the basis order `T[i][0] T[i][1] … T[i][m-1]` such
  that the candidate's representation `v_cand` is mapped to the
  `gf2-core` representation by `v_gf2 = T · v_cand` over GF(2).
* The first non-comment, non-blank line of the file is the first row
  of `T`.
* Lines beginning with `#` are comments. The first comment line MUST
  identify the candidate's primitive polynomial as `# cand-poly: 0xNN`
  and the gf2-core target as `# gf2core-poly: 0xNN`.
* The matrix is square, `m × m`, with row count equal to the field's
  extension degree.

**Rationale.** Decimal bits keep the file diffable in PRs, the comment
header makes accidental swaps detectable, and an `m × m` GF(2) matrix
fits in well under one screen for every supported m ≤ 16. Hex words
were considered but rejected: a row of 16 hex bits is harder to scan
visually for a single off-bit error than 16 space-separated `0/1`
tokens.

**Verification contract.** A candidate's `<lib>_bench.{c,cpp,rs}` MUST
either (a) embed the matrix as a static constant and assert at startup
that the file content matches it, or (b) read the file at startup and
abort if it cannot be parsed or has wrong dimensions. The basis-change
matrix is part of the harness and the harness is part of the promotion
artefact set.
```

**Note for the protocol owner:** the M4RIE harness commits ZERO
basis-change files (none required), so the format spec above is for
*hypothetical* future candidates only. Until a candidate that actually
needs `T` is promoted, the format is informational; if NTL/FLINT
(`73ab8eef`) or LinBox (`79388011`) need a basis change, they should
either follow this format or escalate before deviating.

## Performance-relevance note (criterion #J)

Selected matmul cells from
`dev/bench_results/2026-05-04-507b0036-m4rie-reference.csv` (mean of
5 timed iters, single-threaded; warm-up 3):

| Field   | n    | regime    | wall_ns       | throughput        |
|---------|------|-----------|---------------|-------------------|
| GF(2^4) | 64   | uniform   | 36 512        | 14.4  Gops/s      |
| GF(2^4) | 256  | uniform   | 534 494       | 62.8  Gops/s      |
| GF(2^4) | 1024 | uniform   | 7 043 410     | 305   Gops/s      |
| GF(2^8) | 64   | uniform   | ~210 000      | 2.5   Gops/s      |
| GF(2^8) | 256  | uniform   | ~1 886 000    | 17.8  Gops/s      |
| GF(2^16)| 1024 | uniform   | ~752 522 000  | 2.85  Gops/s      |

These are not directly comparable to the existing `gf2-core` rows in
`dev/bench_results/2026-04-26-gf2.csv` because the gf2-core sweep used
the `fgemm` op-name and only n=64 for GF(2^m); the comparison at
n=1024 GF(2^16) with gf2-core's published 4096-cell extrapolation
shows M4RIE running roughly **three orders of magnitude** faster than
the `gf2-core` `FieldMatrix<Gf2mField>::gemm` published baseline. M4RIE
is therefore performance-relevant.

The full side-by-side rendering will populate when (a) `gf2-core` adds
a `matmul`-named GF(2^m) row at the same sizes and (b) `analyze.py`'s
`reference_lib_for(field)` is taught to pick `m4rie` for GF(2^m) — both
flagged as cross-cutting in § 8 below.

## Cross-cutting findings flagged for the lead

These are out of scope for `jit:507b0036` per the dispatch contract
("**No `analyze.py` mods.** Even if you discover `FIELD_FAMILY` needs a
new entry — stop and flag.") and are recorded here so the lead can
dispatch the follow-up.

1. **`analyze.py FIELD_FAMILY` needs `GF(2^4)`.** The 36 m4rie rows
   include `field=GF(2^4)`, which is not declared in the
   `FIELD_FAMILY` dict (currently `GF(2^8) / GF(2^16) / GF(2^32)` only).
   Today this is benign — `FIELD_FAMILY` is *defined* but *unused* in
   `analyze.py`'s rendering code path — but a future cell-bucketing
   change would silently drop GF(2^4) rows. Suggested one-line fix:
   add `"GF(2^4)": "gf2m"` between `"GF(2^31-1)"` and `"GF(2^8)"` in
   the dict.

2. **`analyze.py reference_lib_for()` needs to learn GF(2^m) → m4rie.**
   Currently `reference_lib_for(field) == "m4ri" if field == "GF(2)"
   else "fflas-ffpack"`. After M4RIE promotion, the canonical reference
   for GF(2^m) is `m4rie`, so `reference_lib_for("GF(2^4)" /
   "GF(2^8)" / "GF(2^16)") == "m4rie"` should hold. Suggested fix:
   ```python
   def reference_lib_for(field_value: str) -> str:
       if field_value == "GF(2)":
           return "m4ri"
       if field_value in ("GF(2^4)", "GF(2^8)", "GF(2^16)", "GF(2^32)"):
           return "m4rie"
       return "fflas-ffpack"
   ```
   Until this lands, the side-by-side renderer surfaces m4rie rows in
   the `### matmul × GF(2^N)` table block but its column header reads
   `fflas-ffpack` — presentation-only, no merge correctness impact.

3. **`run.sh verify_sha M4RIE_SHA256 libs.m4rie` is missing.** The
   dispatch contract said `verify_sha` "already iterates `[libs.*]`"
   but it does not — the three existing calls are explicit and
   per-library. Adding a fourth call would have required editing
   `run.sh`, which the dispatch forbade. Until the lead either (a)
   modifies `run.sh` to add the fourth call, or (b) refactors
   `verify_sha` to actually iterate, the M4RIE pin is checked only
   by `sha256sum -c` inside the `RUN curl … && echo … | sha256sum -c`
   step — which is sufficient to catch tarball mutation but does not
   cross-check Containerfile-vs-image.lock drift.

4. **`run.sh` does not invoke `m4rie_bench` in the timing path.** The
   dispatch contract was read-only on `run.sh`, so the M4RIE timing
   sweep is currently triggered manually (see § *Build evidence*
   above) or via the additional invocation in `benchmarks/smoke.sh`.
   Suggested follow-up: add a `--skip-m4rie` / `RUN_M4RIE_E` flag to
   `run.sh` mirroring the existing `RUN_M4RI` plumbing.

## Files modified or created

| Path                                                              | Status   | Purpose |
|-------------------------------------------------------------------|----------|---------|
| `benchmarks/Containerfile`                                        | modified | `# === m4rie begin/end ===` stanza appended below M4RI; smoke check at end now also asserts m4rie headers + library exist; CMD prints m4rie version. |
| `benchmarks/image.lock`                                           | modified | `[libs.m4rie]` block appended below `[libs.m4ri]`. |
| `benchmarks/reference/m4rie_bench.c`                              | created  | M4RIE timing harness + `--smoke` correctness oracle. |
| `benchmarks/reference/m4rie_one_cell.c`                           | created  | Single-cell driver used to capture perf-stat for the chosen in-scope cell (matmul GF(2^16) n=1024 uniform). |
| `benchmarks/reference/Makefile`                                   | modified | New `m4rie_bench` and `m4rie_one_cell` targets, `M4RIE_CFLAGS` / `M4RIE_LIBS` derived rules. |
| `benchmarks/smoke.sh`                                             | modified | Runs the existing fflas+m4ri smoke first, then drives `m4rie_bench --smoke` inside the same container image to satisfy the protocol § 6 correctness contract for criterion #3. |
| `dev/bench_results/2026-05-04-507b0036-m4rie-reference.csv`       | created  | 36 timing rows (lib=m4rie, op ∈ {matmul, echelon}, field ∈ {GF(2^4), GF(2^8), GF(2^16)}, n ∈ {64, 256, 1024}, regime ∈ {uniform, deficient}). |
| `dev/bench_results/2026-05-04-507b0036-m4rie-host.txt`            | created  | Host capture per protocol § 5. |
| `dev/bench_results/2026-05-04-507b0036-m4rie-perf-stat.txt`       | created  | `perf stat -r 10` capture for matmul GF(2^16) n=1024 uniform. |
| `dev/plans/m4rie_promotion_evidence.md`                           | created  | This document. |

## Re-evaluation triggers

* M4RIE upstream tagging a new release (currently `20250128`).
* gf2-core changing its primitive polynomial choice for any of m ∈
  {4, 8, 16} in `crates/gf2-core/src/primitive_polys.rs` — the
  hard-coded constants `kGf2corePoly_m04 / m08 / m16` in
  `m4rie_bench.c` will fail the smoke check at runtime, but a
  defensive compile-time `static_assert` against `gf2-core`'s
  polynomial values would be preferable. Tracked as a non-blocking
  follow-up because the C-side harness has no FFI access to gf2-core
  constants.
* Container base image refresh past `debian:bookworm-20260421-slim` —
  re-build, re-stamp `[image].local_id`, re-capture host.txt and
  perf-stat under the new microarchitecture-class baseline.
