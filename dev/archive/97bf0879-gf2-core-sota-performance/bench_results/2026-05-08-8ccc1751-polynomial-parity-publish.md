# Polynomial Parity Evidence — Publication (`jit:8ccc1751`)

| Field | Value |
|---|---|
| Date | 2026-05-08 |
| JIT issue | `8ccc1751` (Publish polynomial parity evidence) |
| Parent story | `66190ccd` (sota-polynomial-invariants) |
| Parent epic | `97bf0879` (gf2-core SOTA performance) |
| Predecessor tasks | `d1dd266c` (Tune minimal polynomial path, closed R7); `b87362a3` (Implement winning charpoly dispatch, closed R4) |
| Purpose | Publication document. Restates the integrated 16-cell scorecard verbatim from source evidence, documents the production dispatch trees, and declares non-goal boundaries. No new measurements are run; all numbers are cited from committed evidence. |

## Linked Evidence

| Artefact | Path | Notes |
|---|---|---|
| Integrated 16-cell scorecard (charpoly + minpoly) | `dev/bench_results/2026-05-07-d1dd266c-minpoly-tuning.md` | Primary measurement doc; §§ 3.1–3.3 contain the tables restated below |
| Reference CSV — charpoly (fflas-ffpack canonical + LinBox/FLINT/NTL secondary) | `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv` | 32 data rows; see c3e79272-charpoly-minpoly-refs.md for promotion evidence |
| Reference CSV — minpoly (fflas-ffpack canonical + LinBox/FLINT secondary) | `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv` | 24 data rows; NTL excluded (no public API, § 3.1 of refs doc) |
| Reference promotion evidence | `dev/bench_results/2026-05-04-c3e79272-charpoly-minpoly-refs.md` | Per-cell five-criterion confirmation for every promoted library |

---

## 1. Integrated 16-Cell Scorecard

Numbers cited verbatim from `dev/bench_results/2026-05-07-d1dd266c-minpoly-tuning.md` §§ 3.1–3.3.
Reference wall times are the fflas-ffpack canonical rows from
`dev/bench_results/2026-05-04-c3e79272-{charpoly,minpoly}-reference.csv`.
All gf2 wall times are Criterion medians measured on AMD Ryzen 9 5900X (Zen 3),
`rustc 1.95.0`, `RUSTFLAGS="-C target-cpu=native"`, `cargo bench -p gf2-core
--bench charpoly --features simd --measurement-time 2`.

### 1.1 Minpoly (8 cells)

| Cell | gf2 wall | fflas wall | Ratio | 1.5x ceiling | Algorithm class | PASS? |
|---|---:|---:|---:|---:|---|:---:|
| GF(2^31-1)/64 | 0.942 ms | 1.679 ms | 0.56x | 2.519 ms | Wiedemann + cached SIMD matvec, n³ | PASS |
| GF(2^31-1)/256 | 57.15 ms | 81.5 ms | 0.70x | 122.3 ms | Wiedemann + cached SIMD matvec, n³ | PASS |
| GF(65521)/64 | 0.348 ms | 0.522 ms | 0.67x | 0.783 ms | Wiedemann + medium-prime u16 matvec, n³ | PASS |
| GF(65521)/256 | 12.29 ms | 17.2 ms | 0.71x | 25.8 ms | Wiedemann + medium-prime u16 matvec, n³ | PASS |
| GF(251)/64 | 0.559 ms | 0.135 ms | **4.14x** | 0.202 ms | Wiedemann + small-prime byte matvec, n³ | FAIL* |
| GF(251)/256 | 2.235 ms | 1.634 ms | 1.37x | 2.451 ms | extension-field Wiedemann (k=2) + small-prime byte, n³ | PASS |
| GF(7)/64 | 0.159 ms | 0.569 ms | 0.28x | 0.854 ms | extension-field Wiedemann (k=3) + small-prime byte, n³ | PASS |
| GF(7)/256 | 3.411 ms | 20.29 ms | 0.17x | 30.43 ms | extension-field Wiedemann (k=3) + small-prime byte, n³ | PASS |

### 1.2 Charpoly (8 cells)

| Cell | gf2 wall | fflas wall | Ratio | 1.5x ceiling | Algorithm class | PASS? |
|---|---:|---:|---:|---:|---|:---:|
| GF(2^31-1)/64 | 0.485 ms | 0.743 ms | 0.65x | 1.115 ms | cubic + cached SIMD matvec, n³ | PASS |
| GF(2^31-1)/256 | 21.76 ms | 43.92 ms | 0.50x | 65.88 ms | cubic + cached SIMD matvec, n³ | PASS |
| GF(65521)/64 | 0.379 ms | 0.674 ms | 0.56x | 1.011 ms | cubic + medium-prime u16 matvec, n³ | PASS |
| GF(65521)/256 | 14.79 ms | 12.38 ms | 1.20x | 18.57 ms | cubic + medium-prime u16 matvec, n³ | PASS |
| GF(251)/64 | 0.165 ms | 0.476 ms | 0.35x | 0.715 ms | cubic + small-prime byte matvec + Barrett-table-cached, n³ | PASS |
| GF(251)/256 | 4.20 ms | 1.317 ms | **3.18x** | 1.975 ms | cubic + canonical-byte chain_polys + small-prime byte matvec, n³ | FAIL* |
| GF(7)/64 | 0.132 ms | 0.402 ms | 0.33x | 0.603 ms | cubic + small-prime byte matvec + Barrett-table-cached, n³ | PASS |
| GF(7)/256 | 3.44 ms | 13.63 ms | 0.25x | 20.45 ms | cubic + canonical-byte chain_polys + small-prime byte matvec, n³ | PASS |

### 1.3 Aggregate

14 of 16 cells PASS the 1.5x ceiling. Two failing cells are covered by the
user-approved 2026-05-07 scope amendment routing residual closure to follow-up
task `52cce970` (bespoke small-prime AVX2 kernel) under planning issue `615db3b9`:

| Cell | Operation | Ratio | Gap to ceiling | Follow-up |
|---|---|---:|---:|---|
| GF(251)/64 | minpoly | 4.14x | 2.8x past ceiling | `52cce970` |
| GF(251)/256 | charpoly | 3.18x | 2.1x past ceiling | `52cce970` |

The `GF(251)/256 minpoly` cell (previously worst at 23.6x) was closed to 1.37x
by `jit:6c926de0` (quadratic extension-field Wiedemann). The `GF(7)/256 minpoly`
cell was closed to 0.17x by the same issue (cubic extension).

---

## 2. Production Dispatch Tree

### 2.1 `charpoly_dispatch`

**Location:** `crates/gf2-core/src/field/charpoly.rs:1074`

```
charpoly_dispatch(a: &FieldMatrix<F>) -> FieldPoly<F>
  |
  ├─ n < KG_DISPATCH_MIN_N (= usize::MAX)?         [charpoly.rs:1086]
  |    → YES: charpoly_cubic(a)
  |
  ├─ cardinality_log2_hint() == None?               [charpoly.rs:1089-1092]
  |    → YES: charpoly_cubic(a)
  |
  ├─ log_q <= 127 && q <= 2n²?                      [charpoly.rs:1097-1103]
  |    → YES: charpoly_cubic(a)
  |
  └─ Try keller_gehrig_charpoly(a, KG_DEFAULT_SEED) [charpoly.rs:1108]
       up to KG_MAX_RETRIES (= 8) times.
       → Some(p): return p
       → None (all retries failed): charpoly_cubic(a)
```

**Effective behaviour today:** `KG_DISPATCH_MIN_N = usize::MAX`
(`crates/gf2-core/src/field/charpoly.rs:276`), so the first branch always
fires and `charpoly_dispatch` is a direct alias for `charpoly_cubic`. The
Keller-Gehrig path exists and is correct but is not engaged by default dispatch
because post-Wave-9 benchmarks show cubic is ~148x faster than KG at n=256 on
`Fp<2^31-1>` (see `4a59d1f9` crossover analysis). The threshold will be tuned
downward once the K^{-1} step is replaced with a Strassen-amenable inversion.

**`charpoly_cubic`** implements Dumas-Pernet theorem 13.1: Krylov-cyclic
decomposition, `O(n³)` field operations. Each matvec in the inner loop routes
through `FieldMatrix::matvec` → `MatvecDriver`, which builds a `PackedFpMatrix<P>`
cache once and reuses it across all iterations for `Fp<P>` with `P <= 65521`.

### 2.2 `minpoly_dispatch`

**Location:** `crates/gf2-core/src/field/charpoly.rs:1998`

```
minpoly_dispatch(a: &FieldMatrix<F>) -> FieldPoly<F>
  |
  ├─ n < 2 || cardinality_log2_hint() == None?      [charpoly.rs:2008-2009]
  |    → fall through to cyclic_lcm_minpoly(a)
  |
  ├─ Wiedemann gate: q > n?                          [charpoly.rs:2013-2024]
  |    (log_q > 63 → gate passes trivially)
  |    → YES (large-field path):
  |         Try wiedemann_minpoly_attempt(a, seed)
  |         up to WIEDEMANN_MAX_RETRIES (= 8) times.
  |         → Some(m): return m
  |         → None (all retries failed): fall through to cyclic_lcm_minpoly
  |
  ├─ q <= n (low-cardinality path):                  [charpoly.rs:2034-2040]
  |    → F::try_extension_wiedemann_minpoly(a)
  |         → Some(m): return m          (Fp<7>, Fp<251> implemented)
  |         → None: fall through to cyclic_lcm_minpoly
  |
  └─ cyclic_lcm_minpoly(a)                           [charpoly.rs:2050]
       Deterministic cubic fallback: builds Krylov chains from canonical
       seeds + random seeds (multi_seed_wiedemann_minpoly), LCM via BM.
       Panics on failure (does not fall back to quartic).
```

**`try_extension_wiedemann_fp` gate:**
**Location:** `crates/gf2-core/src/field/extension_wiedemann.rs:494`

The gate engages `try_extension_wiedemann_fp<P>` when `q <= n` (i.e. the
base-field Wiedemann gate fails) and the prime P has a pre-built extension
config. Engagement rules per-prime:

| Prime | Condition | Extension | Per-attempt success probability |
|---|---|---|---|
| P=7 | n in [7, 48] | quadratic (k=2, q²=49) | > 1 - n/49 |
| P=7 | n in [49, 342] | cubic (k=3, q³=343) | > 1 - n/343 |
| P=251 | n in [251, 63000] | quadratic (k=2, q²=63001) | > 1 - n/63001 |
| any other P | — | None | (falls to cyclic_lcm_minpoly) |

For n < q the base-field Wiedemann gate already passes (`q > n`), so
`try_extension_wiedemann_fp` is not reached for those cells (it returns `None`
immediately for n < P as a safety check at `extension_wiedemann.rs:498-500,
511-513, 525-527`).

**Quartic `find_max_minpoly_generator`:** this function remains in the crate
(`crates/gf2-core/src/field/charpoly.rs`) but is no longer reachable from
`minpoly_dispatch`. It is still called by `FieldMatrix::frobenius_form()`, which
is out of scope for the current work.

---

## 3. Non-Goal Boundaries

### 3.1 Residual small-prime cells — `52cce970`

Two cells remain above the 1.5x ceiling after `d1dd266c` and `b87362a3` land:

- **GF(251)/64 minpoly at 4.14x** — `wiedemann_minpoly_attempt` engages
  directly (q=251 > n=64 satisfies the Wiedemann gate). The 4-row-per-call
  AVX2 `gemm_row_panel_fn` byte-lane kernel does not amortise its setup
  overhead at n=64; the absolute work is small enough that per-call overhead
  dominates. fflas-ffpack reports 135 µs vs our 559 µs.
- **GF(251)/256 charpoly at 3.18x** — `charpoly_cubic` runs
  `cyclic_decomposition` → `chain_polys` bookkeeping. `jit:5a3dbd5b` replaced
  scalar Montgomery polynomial bookkeeping with canonical-byte AVX2
  (`PackedFpChainPolys<P>`), cutting the gap from 9.58x to 3.18x. Remaining
  gap is per-call AVX2 boundary overhead at the `chain_polys` surface.

Residual closure for both cells is routed to follow-up task **`52cce970`**
(bespoke small-prime AVX2 kernel, hand-written register-scheduled
`gf2-kernels-simd` kernels) under planning issue **`615db3b9`**.
These cells are out of scope for the current story `66190ccd`.

### 3.2 Keller-Gehrig sub-cubic charpoly path — not engaged

`KG_DISPATCH_MIN_N = usize::MAX` (`charpoly.rs:276`), meaning `charpoly_dispatch`
never reaches the Keller-Gehrig branch under current dispatch. The path is
correct and remains available via `FieldMatrix::charpoly_keller_gehrig(seed)`
for callers who opt in explicitly. Re-engagement of the KG path under default
dispatch requires replacing the K^{-1} PLE pipeline with a Strassen-amenable
inversion; that work is out of scope for story `66190ccd`.

### 3.3 `frobenius_form()` — out of scope

`FieldMatrix::frobenius_form()` still calls the O(n⁴)
`find_max_minpoly_generator` quartic helper. This public method was not
in scope for issues `d1dd266c` or `b87362a3`, and is not in scope for
story `66190ccd`. It has no performance criterion in the current epic.

### 3.4 GF(2) and GF(2^m) charpoly/minpoly — out of scope

M4RI does not expose charpoly/minpoly (protocol `not-supported-by-library`
exclusion). GF(2) and GF(2^m) operations over those fields are excluded from
the GF(p) scorecard. See `dev/bench_results/2026-05-04-c3e79272-charpoly-minpoly-refs.md`
§ 3.2 for the full exclusion rationale.

---

## 4. Algorithm Complexity Summary

Every dispatch arm on the `minpoly()` and `charpoly()` paths is `O(n³)`.

| Algorithm path | Function | Complexity | File:line |
|---|---|---|---|
| charpoly cubic (cyclic decomposition) | `charpoly_cubic` | O(n³) | `charpoly.rs` (dispatched from `:1087`) |
| Wiedemann minpoly (large fields, q > n) | `wiedemann_minpoly_attempt` | O(n³) matvec-dominated | `charpoly.rs:2021` |
| Extension-field Wiedemann (Fp<7>, Fp<251>, q ≤ n) | `try_extension_wiedemann_fp` | O(n³) | `extension_wiedemann.rs:494` |
| Cyclic-LCM minpoly fallback | `cyclic_lcm_minpoly` | O(n³) | `charpoly.rs:2050` |
| Keller-Gehrig charpoly (not engaged by default) | `keller_gehrig_charpoly` | O(n^ω log n) | `charpoly.rs:1123` |
| Legacy quartic (frobenius_form only) | `find_max_minpoly_generator` | O(n⁴) | `charpoly.rs` (not on minpoly path) |

---

## 5. Self-Satisfaction of Success Criteria

### SC#1 — Raw CSVs and ratio tables are linked to the story

The following documents are attached to issue `8ccc1751` via `jit doc add`:

- `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv` — fflas-ffpack + secondary library charpoly reference rows (32 data rows)
- `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv` — fflas-ffpack + secondary library minpoly reference rows (24 data rows)
- `dev/bench_results/2026-05-04-c3e79272-charpoly-minpoly-refs.md` — reference promotion evidence
- `dev/bench_results/2026-05-07-d1dd266c-minpoly-tuning.md` — integrated 16-cell scorecard (source of all ratio tables above)
- `dev/bench_results/2026-05-08-8ccc1751-polynomial-parity-publish.md` — this document

Visibility: story `66190ccd` inherits through the dependency graph.

### SC#2 — Algorithmic dispatch and non-goal boundaries are documented

Covered by §§ 2 and 3 above. Production dispatch trees for `charpoly_dispatch`
and `minpoly_dispatch` cite exact file:line references. Non-goal boundaries for
the two failing cells, the dormant KG path, `frobenius_form()`, and the GF(2)/GF(2^m)
family are stated in § 3.
