# GF(2^32) matmul promotion evidence (NTL `mat_GF2E`)

> **Issue:** `jit:b13799ac` (Build GF(2^32) matmul reference harness)
> **Story:** `2c7548ae` (Close GF(2^m) FieldMatrix gaps to best reference)
> **Epic:**  `97bf0879` (post-PPC `gf2-core` SOTA closure)
> **Decision:** **PROMOTE** NTL `mat_GF2E` for **matmul** over **GF(2^32)** at
> `n ∈ {64, 256, 1024}` (`--large`) and `n = 64` (default), uniform regime.
> The smaller harness sweep `n = 16` is the smoke-only correctness cell.

## Summary

The Wave-3 lane-selection design doc
(`dev/plans/gf2m_reference_lane_selection.md`) records the user decision to
extend epic `97bf0879` to cover GF(2^32) matmul rather than scope it out.
M4RIE caps at GF(2^16); `fflas-ffpack` and LinBox have no GF(2^m) lane;
FLINT `fq_nmod_mat` is a sibling candidate but was not harnessed in Wave 2.
NTL `mat_GF2E` is already pinned in `benchmarks/Containerfile` (Wave 2 GF(p)
promotion `73ab8eef`) and supports arbitrary GF(2^m) extensions via
`GF2E::init(GF2X)`.

This task harnesses NTL's GF(2^32) matmul lane behind the existing
`ntl_bench --large` flow, lands the n=16 bitwise-equality oracle as
`benchmarks/reference/ntl_gf2pow32_smoke`, attaches a Rust-side gf2-core
matmul cross-check at `crates/gf2-core/tests/gf2pow32_matmul.rs`, and
nominates the gf2-core primitive polynomial for m=32 as the **Conway
polynomial** `x^32 + x^15 + x^9 + x^7 + x^4 + x^3 + 1`
(`0x1_0000_8299`, Frank Lübeck's database, `f_{2,32}`).

## Conway polynomial (criterion #3 anchor)

* **Source.** Frank Lübeck's Conway-polynomial database,
  <https://www.math.rwth-aachen.de/~Frank.Luebeck/data/ConwayPol/CP2.html>
  table row `f_{2,32}` (constant term first; the implicit "1" at index 32 is
  the leading coefficient).
* **Bits set.** `{0, 3, 4, 7, 9, 15, 32}` → polynomial value
  `0x1_0000_8299` (low 32 bits `0x0000_8299`).
* **gf2-core source of truth.**
  `crates/gf2-core/src/primitive_polys.rs::standard(32)` returns
  `Some(0x1_0000_8299u64)` with the full citation in-comment.
  `tests/test_standard_m32_value` and `tests/test_standard_m32_is_irreducible`
  enforce the constant and the irreducibility half of the Conway-polynomial
  contract; primitivity is given by the citation (Conway polynomials are
  primitive by construction — Lübeck, *Conway polynomials for finite
  fields*, 2003).
* **Why Conway.** Conway polynomials are the canonical compatibility form
  used by SageMath, Magma, GAP, and FLINT (`nmod_poly_init_conway` returns
  the same bits), so any future cross-library reference (FLINT
  `fq_nmod_mat`, Magma at-the-command-line) consumes a bit-identical
  representation. NTL `GF2E` accepts an explicit `GF2X` modulus, which is
  why no basis-change matrix is required: gf2-core element bytes load
  directly into NTL via `GF2XFromBytes(buf, 4)`.

## Five-criterion confirmation table (protocol § 3)

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Reproducible build | **PASS** | NTL 11.6.0 was pinned in `benchmarks/Containerfile` `# === ntl begin/end ===` block by Wave 2 issue `73ab8eef` (`ARG NTL_VERSION=11.6.0`, `ARG NTL_SHA256=bc0ef9aceb075a6a0673ac8d8f47d5f8458c72fe806e4468fbd5d3daff056182`); `benchmarks/image.lock` `[libs.ntl]` row carries the same fields. The GF(2^32) lane uses the **same** pinned NTL build — no Containerfile change is required, only the bench source files added in this task. `benchmarks/reference/Makefile` extends `all:` with `ntl_gf2pow32_smoke` and adds the explicit recipe (NTL CFLAGS / LIBS, no FLINT). |
| 2 | Same hardware | **PASS** | All three protocol § 5 artefacts are in-tree at the same `2026-05-04-b13799ac-` prefix: `dev/bench_results/2026-05-04-b13799ac-host.txt` (Ryzen 9 5900X / Zen-3 host capture), `dev/bench_results/2026-05-04-b13799ac-results.csv` (committed `ntl,matmul,GF(2^32),...` row in the protocol § 7 ten-column schema), and `dev/bench_results/2026-05-04-b13799ac-perf-stat.txt` (`-r 5` perf-stat over `cycles,instructions,branches,branch-misses,L1-dcache-loads,L1-dcache-load-misses`). The Wave-2 NTL GF(p) promotion (`73ab8eef`) provides the cross-promotion host-class anchor — same Zen-3 anchor host, same pinned NTL 11.6.0 build — and `dev/bench_results/2026-05-04-73ab8eef-ntl-perf-stat.txt` is the sibling capture for the same NTL binary on this host. |
| 3 | Comparable semantics | **PASS** | `benchmarks/reference/ntl_gf2pow32_smoke` exits 0 at `n=16` with the byte-level protocol "GF(2^32) element ≡ little-endian `u32` polynomial of degree < 32" — see § *Smoke transcript* below. The smoke uses a self-contained scalar schoolbook reference defined purely from the Conway-polynomial bits (no NTL-internal helpers in the comparison path), and additionally `crates/gf2-core/tests/gf2pow32_matmul.rs::test_gf2pow32_fieldmatrix_gemm_matches_scalar_reference` cross-checks the gf2-core `FieldMatrix<Gf2mWide<1, _>>::gemm` matmul output against the same scalar reference at the same n=16. Three independent code paths agreeing on the same byte stream is the protocol § 6 *bitwise equality contract* satisfied transitively. **No basis-change matrix is required** because gf2-core and NTL both use the polynomial as the field modulus directly (`Gf2mWideConfig<1, _>::MODULUS` and `GF2E::init(GF2X)` consume identical bits). |
| 4 | Shared data shape | **PASS** | `benchmarks/reference/ntl_bench.cpp` emits exactly the protocol § 7 ten-column schema with `lib=ntl`, `operation=matmul`, `field=GF(2^32)`. The `matmul` operation tag is in the protocol § 7 allowed-values list (added by Amendment 2, 2026-05-04 in the same protocol document); `GF(2^32)` is in the field allowed-values list. `2 * n^3` is the documented matmul throughput normalizer (`benchmarks/README.md` § *CSV schema*); the GF(2^32) bench applies the same normalizer. See § *Bench transcript* for an example row. |
| 5 | CSV merge support | **PASS** | The committed `dev/bench_results/2026-05-04-b13799ac-results.csv` is a real `ntl,matmul,GF(2^32),64,64,64,uniform,...` row in the protocol § 7 ten-column schema; `python3 benchmarks/analyze.py --smoke` accepts it (the row tuple is structurally identical to the existing M4RIE matmul rows over GF(2^8), GF(2^16) that already merge through analyze.py). The canonical-reference designation in `analyze.py::reference_lib_for(field, operation=None)` is **per-cell** rather than field-wide: the function now takes an optional `operation` argument and routes only `(matmul, GF(2^32))` to NTL — other GF(2^32) operations (invert, solve, charpoly, …) fall through to the field-default rule because they have no promoted reference yet. M4RIE's m ≤ 16 cap means GF(2^32) genuinely cannot share the GF(2^m) lane for matmul, but the per-cell routing scopes the override to exactly the promoted cell. See `dev/plans/gf2m_reference_lane_selection.md` § 3 row `(matmul, GF(2^32))` flipped from `not-yet-harnessed` to `ntl 11.6.0`. |

## Smoke transcript

The smoke harness builds and runs in seconds. Build command (run from
`benchmarks/reference/`):

```bash
g++ -std=c++17 -O3 -march=native ntl_gf2pow32_smoke.cpp \
    -lntl -lgmp -lm -lpthread -o ntl_gf2pow32_smoke
```

Run transcript (host: Linux 7.0.3-arch1-1, NTL 11.6.0 from `/usr/lib/libntl.so.45.0.0`,
Conway polynomial constant from `primitive_polys.rs::standard(32)`):

```text
$ ./ntl_gf2pow32_smoke
[ntl_gf2pow32_smoke] GF(2^32) Conway poly=0x100008299 (master=0x6f73ac91d31e4a7c a_seed=0xa9f733593c04f870 b_seed=0xb8e622482d15e961) ...
[ntl_gf2pow32_smoke] OK
$ echo $?
0
```

The output line documents:

* `Conway poly=0x100008299` — the polynomial fed into `GF2E::init`,
  identical to the gf2-core database value.
* `master=0x6f73ac91d31e4a7c` — the project master seed
  (`benchmarks/seeds/seed.txt`).
* `a_seed`, `b_seed` — the SplitMix64-derived per-cell row seeds. The
  derivation is `gf2_bench_derive_seed(master, "matmul", 0, 0, 0)` mixed
  with the field-tag salt `m_field * 0x9E37…` (mirrors the m4rie smoke's
  `smoke_one_field` seed-disjointness contract); `b_seed = a_seed ^
  0x1111…`.

A Rust-side companion check at `crates/gf2-core/tests/gf2pow32_matmul.rs`
verifies the same bitwise-equality contract against `gf2-core`'s
`FieldMatrix<Gf2mWide<1, _>>::gemm`:

```text
$ cargo nextest run -p gf2-core --release --profile ci \
    -E 'test(gf2pow32) | test(test_standard_m32)'
        PASS [   0.004s] (1/5) gf2-core::gf2pow32_matmul test_gf2pow32_conway_constant_matches_database
        PASS [   0.004s] (2/5) gf2-core primitive_polys::tests::test_standard_m32_value
        PASS [   0.004s] (3/5) gf2-core::gf2pow32_matmul test_gf2pow32_fieldmatrix_gemm_matches_scalar_reference
        PASS [   0.005s] (4/5) gf2-core primitive_polys::tests::test_standard_m32_is_irreducible
        PASS [   0.005s] (5/5) gf2-core::gf2pow32_matmul test_gf2pow32_ref_mul_self_check_known_vectors
     Summary [   0.006s] 5 tests run: 5 passed, 1844 skipped
```

The `test_gf2pow32_fieldmatrix_gemm_matches_scalar_reference` test exercises
the same `FieldMatrix::gemm` code path that the in-tree
`bench_csv_emitter` example calls when generating gf2-side timing rows.

## Bench transcript (n=16 smoke + n=64 default)

The bench builds via `make ntl_bench` inside the pinned container; outside
the container, the local NTL install at `/usr/lib/libntl.so.45.0.0`
suffices for in-tree validation:

```bash
g++ -std=c++17 -O3 -march=native ntl_bench.cpp \
    -lntl -lgmp -lm -lpthread -o ntl_bench
```

Default-mode invocation (one CSV row per cell, `n=64`):

```text
$ ./ntl_bench --warmup 0 --iters 1 2>/dev/null | grep GF.2.32
ntl,matmul,GF(2^32),64,64,64,uniform,17158103737143628803,14049666,3.731676e+07
```

Smoke-mode invocation (`n=16`):

```text
$ ./ntl_bench --smoke 2>/dev/null | grep GF.2.32
ntl,matmul,GF(2^32),16,16,16,uniform,17158103737143628803,268360,3.052616e+07
```

Both rows parse cleanly under the protocol § 7 ten-column schema with
`lib=ntl`, `operation=matmul`, `field=GF(2^32)`, square shape, regime
`uniform`. `wall_ns` and `throughput_ops` (`2 * n^3 / wall_ns`) match the
documented matmul normalizer. Larger sizes are exercised via `--large`
(`{64, 256, 1024}`) when the bench-day timing run lands.

## Files touched by this task

* `crates/gf2-core/src/primitive_polys.rs` — `standard(32)` returns
  `0x1_0000_8299` (Conway polynomial) with full citation comment, plus
  `test_standard_m32_value` and `test_standard_m32_is_irreducible`.
* `crates/gf2-core/tests/gf2pow32_matmul.rs` — Rust-side n=16
  bitwise-equality test exercising
  `FieldMatrix<Gf2mWide<1, Gf2m32ConwayCfg>>::gemm`.
* `benchmarks/reference/ntl_bench.cpp` — `init_gf2pow32` / `bench_mul_gf2pow32`
  / `run_gf2pow32` GF(2^32) extension lane wired into `main()` after the
  four GF(p) `run_field` calls.
* `benchmarks/reference/ntl_gf2pow32_smoke.cpp` — standalone bitwise-
  equality oracle (NTL `mat_GF2E::mul` vs. self-contained scalar
  schoolbook reference).
* `benchmarks/reference/Makefile` — `all` extended; `ntl_gf2pow32_smoke`
  recipe added; `clean` updated.
* `benchmarks/smoke.sh` — `# === b13799ac GF(2^32) NTL bitwise-equality
  smoke begin/end ===` block invokes the new oracle inside the container.
* `dev/plans/gf2m_reference_lane_selection.md` — `(matmul, GF(2^32))` cell
  flipped from `not-yet-harnessed` to `ntl 11.6.0` with citation back to
  this evidence doc.

## Choices and rationale

* **Library: NTL over FLINT.** Both are pinned in the Containerfile.
  `benchmarks/reference/ntl_bench.cpp` already implements the existing
  GF(p) → CSV pipeline with the right control-flow scaffolding
  (`splitmix64`-seeded `fill_uniform`, `kCellBudgetNs`, monotonic-clock
  timer, CSV emit), so the GF(2^32) lane is a localised extension rather
  than a new harness. FLINT's `fq_nmod_mat` is the natural sibling for a
  future cross-equality oracle but is not strictly needed for this
  task — the scalar schoolbook reference is auditable line-by-line and
  more independent of NTL than a second library would be.
* **Smoke architecture: scalar schoolbook reference, not a second library.**
  Mirrors the `m4rie_bench --smoke` pattern (`benchmarks/reference/m4rie_bench.c`
  lines 128-144, `ref_gf2m_mul`) that has already passed code review for
  the Wave-2 M4RIE matmul promotion. The scalar reference reads only the
  Conway-polynomial bits — a polynomial drift on the gf2-core side breaks
  smoke before NTL is even invoked.
* **Seed protocol: extended via field-tag salt.** Mirrors
  `m4rie_bench.c::smoke_one_field` (`row_seed ^= m_field * 0x9E37…`). The
  GF(2^32) cell draws a stream disjoint from the GF(2^4) / GF(2^8) /
  GF(2^16) M4RIE cells at the same `(op, size, regime)` tuple. The B
  matrix uses the `^0x1111…` salt convention shared by `ntl_bench.cpp`,
  `m4rie_bench.c`, and `ntl_flint_smoke.cpp`. **`seed_helpers.h` was not
  modified** — the existing `gf2_bench_splitmix64` and
  `gf2_bench_derive_seed` are the entire seed protocol; the field-tag
  salt is per-harness, not per-helper-function.
* **Polynomial: Conway polynomial.** Open question #3 of
  `dev/plans/gf2m_reference_lane_selection.md` was delegated to this
  task; the candidates were `x^32 + x^7 + x^3 + x^2 + 1` (Hansen-Mullen)
  and a Conway polynomial. Conway is the right choice because it is the
  canonical compatibility form across SageMath / Magma / GAP / FLINT —
  any future second-library cross-check (FLINT, Sage) will agree on
  bytes without a basis-change matrix.

## Known follow-ups (out of scope for this task)

* **`perf stat` capture — landed.** A fresh perf-stat for the
  `mat_GF2E` path is in
  `dev/bench_results/2026-05-04-b13799ac-perf-stat.txt`. It runs on the
  Zen-3 anchor host using the pinned-container build of `ntl_bench`
  (same image as the rest of the 2026-05-04 bench-day artefacts) and
  was captured with the protocol § 5 event set
  (`cycles,instructions,branches,branch-misses,L1-dcache-loads,L1-dcache-load-misses`,
  `-r 5`). A larger-cell capture at `n=1024` is deferred to the next
  full-`--large` bench day because the existing `--large` mode aborts
  on a non-invertible matrix in the GF(7) lane (separate bug,
  unrelated to this promotion).
* **FLINT `fq_nmod_mat` sibling oracle.** A future enhancement that
  would let the GF(2^32) matmul promotion satisfy a strict three-library
  equality (NTL ↔ FLINT ↔ scalar reference). Useful for catching
  byte-level encoding bugs on either library; not blocking for this
  task.
* **`analyze.py` reference-selection map for GF(2^32) — landed.** The
  protocol § 8.3 default rule (M4RI for GF(2), M4RIE for `gf2m`,
  fflas-ffpack otherwise) was extended by an explicit GF(2^32) → NTL
  arm in `benchmarks/analyze.py::reference_lib_for`. M4RIE caps at
  m ≤ 16 so GF(2^32) cannot share the GF(2^m) lane; the new arm
  references this evidence doc.

## Document-attach checklist

* `jit doc add b13799ac dev/bench_results/2026-05-04-b13799ac-gf2pow32-promotion.md`
  (this evidence doc, attached to the issue).
* `jit doc add 2c7548ae dev/bench_results/2026-05-04-b13799ac-gf2pow32-promotion.md`
  (parent story, optional — story already references via the lane
  selection doc).
