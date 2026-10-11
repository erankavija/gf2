"""Review record of the caller-trusted safe kernel entry points (jit:bfc2ceab).

The reviewer's reading of `gf2-kernels-simd`, as data the generators check
against the tree: `make-source-evidence.py` resolves every claim to a line,
`make-inventory.py` requires a verdict for every safe function that holds an
`unsafe` block and an assignment for every use of a published kernel pointer,
and fails on a gap or a stale row.
"""

SIMD, CORE, ALGEBRA, CODING = "gf2-kernels-simd", "gf2-core", "gf2-algebra", "gf2-coding"
PROTOTYPE = "caller-trusted-check-prototype"
# Owner of a claim about the tree the family measured: its path is relative to
# the producing snapshot of the confirmation receipt.
MEASURED = "measured-tree"
TOOL = "tuning-campaign-support"
PROTOTYPE_CRATE = "caller_trusted_check_prototype"

# (claim id, package or None for a unique tracked file name, path, fragment, occurrences, why)
CLAIMS = [
    # Sparse row kernels.
    ("small-spmm-type-doc", SIMD, "src/fp_small.rs",
     '/// `a_vals.len() == a_cols.len()`, every `a_cols[h]` names a whole row of', 1,
     'The pointer type lists the column bound under Panics.'),
    ("small-spmm-wrapper", SIMD, "src/fp_small.rs",
     'crate::entry_check::assert_spmm_columns("fp_small_spmm", a_cols, b.len(), n);', 1,
     'The safe entry checks every column index once per product, ahead of its row loop.'),
    ("small-spmm-length-assert", SIMD, "src/x86/fp_small.rs",
     '"fp_small_spmm_row: a_vals/a_cols length mismatch"', 1,
     "The kernel asserts its other two length conditions in release builds."),
    ("small-spmm-vector-read", SIMD, "src/x86/fp_small.rs",
     "let b_row_ptr = b.as_ptr().add(col * b_stride + j);", 1,
     "The 16-byte load address comes from an unchecked column index and a wrapping product."),
    ("small-spmm-tail-read", SIMD, "src/x86/fp_small.rs",
     "let b_kj = *b.get_unchecked(col * b_stride + j) as u64;", 1,
     "The scalar tail reads `b` without a bounds check."),
    ("medium-spmm-type-doc", SIMD, "src/fp_medium.rs",
     '/// `a_vals.len() == a_cols.len()`, every `a_cols[h]` names a whole row of', 1,
     'The pointer type lists the column bound under Panics.'),
    ("medium-spmm-wrapper", SIMD, "src/fp_medium.rs",
     'crate::entry_check::assert_spmm_columns("fp_medium_spmm", a_cols, b.len(), n);', 1,
     'The safe entry checks every column index once per product, ahead of its row loop.'),
    ("medium-spmm-length-assert", SIMD, "src/x86/fp_medium.rs",
     '"fp_medium_spmm_row: a_vals/a_cols length mismatch"', 1,
     "The kernel asserts its other two length conditions in release builds."),
    ("medium-spmm-vector-read", SIMD, "src/x86/fp_medium.rs",
     "let b_row_ptr = b.as_ptr().add(col * b_stride + j) as *const __m256i;", 1,
     "The 32-byte load address comes from an unchecked column index and a wrapping product."),
    ("medium-spmm-tail-read", SIMD, "src/x86/fp_medium.rs",
     "let b_kj = *b.get_unchecked(col * b_stride + j) as u64;", 1,
     "The scalar tail reads `b` without a bounds check."),
    # PLE panel base kernels.
    ("small-ple-type-doc", SIMD, "src/fp_small_ple.rs",
     '/// Panics unless `window.len() == m * win`, `inv_table.len() >= p as usize`', 1,
     'The pointer type lists the window shape, the table length and the canonical window under Panics.'),
    ("small-ple-wrapper", SIMD, "src/fp_small_ple.rs",
     'crate::entry_check::assert_ple_panel(', 1,
     'The safe entry checks the window shape, the table length and every window entry ahead of the kernel.'),
    ("small-ple-window-debug", SIMD, "src/x86/fp_small_ple.rs",
     '"ple_panel_base_canonical: window shape"', 1,
     "The window shape is a `debug_assert_eq!`."),
    ("small-ple-table-debug", SIMD, "src/x86/fp_small_ple.rs",
     '"ple_panel_base_canonical: inv_table length"', 1,
     "The inverse-table length is a `debug_assert_eq!`; no assertion bounds a window entry."),
    ("small-ple-window-read", SIMD, "src/x86/fp_small_ple.rs",
     "if *window.get_unchecked(i * win + col) != 0 {", 1,
     "The pivot search reads the window without a bounds check."),
    ("small-ple-table-read", SIMD, "src/x86/fp_small_ple.rs",
     "let inv = *inv_table.get_unchecked(pivot_val as usize) as u32;", 1,
     "The pivot value indexes the inverse table without a bounds check, once per pivot."),
    ("small-ple-perm-swap", SIMD, "src/x86/fp_small_ple.rs",
     "row_perm.swap(rank, piv);", 2,
     "`row_perm` is reached through a bounds-checked swap, so its length is no soundness "
     "condition; the test reference repeats the statement."),
    ("small-ple-scratch-write", SIMD, "src/x86/fp_small_ple.rs",
     "pivot_buf[c] = *window.get_unchecked(pivot_base + c);", 1,
     "The fixed scratch row is written through a bounds-checked index, so a window wider "
     "than the scratch panics."),
    ("medium-ple-type-doc", SIMD, "src/fp_medium_ple.rs",
     '/// Panics unless `window.len() == m * win`, `inv_table.len() >= p as usize`', 1,
     'The pointer type lists the window shape, the table length and the canonical window under Panics.'),
    ("medium-ple-wrapper", SIMD, "src/fp_medium_ple.rs",
     'crate::entry_check::assert_ple_panel(', 1,
     'The safe entry checks the window shape, the table length and every window entry ahead of the kernel.'),
    ("medium-ple-window-debug", SIMD, "src/x86/fp_medium_ple.rs",
     '"ple_panel_base_canonical_u16: window shape"', 1,
     "The window shape is a `debug_assert_eq!`."),
    ("medium-ple-table-debug", SIMD, "src/x86/fp_medium_ple.rs",
     '"ple_panel_base_canonical_u16: inv_table length"', 1,
     "The inverse-table length is a `debug_assert_eq!`; no assertion bounds a window entry."),
    ("medium-ple-window-read", SIMD, "src/x86/fp_medium_ple.rs",
     "if *window.get_unchecked(i * win + col) != 0 {", 1,
     "The pivot search reads the window without a bounds check."),
    ("medium-ple-table-read", SIMD, "src/x86/fp_medium_ple.rs",
     "let inv = *inv_table.get_unchecked(pivot_val as usize) as u32;", 1,
     "The pivot value indexes the inverse table without a bounds check, once per pivot."),
    ("medium-ple-perm-swap", SIMD, "src/x86/fp_medium_ple.rs",
     "row_perm.swap(rank, piv);", 2,
     "`row_perm` is reached through a bounds-checked swap; the test reference repeats "
     "the statement."),
    ("medium-ple-scratch-write", SIMD, "src/x86/fp_medium_ple.rs",
     "pivot_buf[c] = *window.get_unchecked(pivot_base + c);", 1,
     "The fixed scratch row is written through a bounds-checked index."),
    # Packed batch kernels.
    ("bipedal-binary-type-doc", SIMD, "src/bipedal/mod.rs",
     '/// Panics unless all six slices share one length that is a multiple of 4.', 1,
     'The pointer type lists the slice shape under Panics.'),
    ("bipedal-unary-type-doc", SIMD, "src/bipedal/mod.rs",
     '/// Panics unless all four slices share one length that is a multiple of 4.', 1,
     'The pointer type lists the slice shape under Panics.'),
    ("bipedal-binary-wrapper", SIMD, "src/bipedal/mod.rs",
     '[m1.len(), s1.len(), m2.len(), s2.len(), om.len(), os.len()],', 3,
     'Each of the three binary entries checks the slice shape ahead of the kernel.'),
    ("bipedal-unary-wrapper", SIMD, "src/bipedal/mod.rs",
     '[m.len(), s.len(), om.len(), os.len()],', 1,
     'The unary entry checks the slice shape ahead of the kernel.'),
    ("bipedal-binary-debug", SIMD, "src/x86/bipedal_avx2.rs",
     "debug_assert_eq!(mag1.len() % 4, 0);", 3,
     "Each binary kernel opens with debug assertions of the slice shape."),
    ("bipedal-unary-debug", SIMD, "src/x86/bipedal_avx2.rs",
     "debug_assert_eq!(mag.len() % 4, 0);", 1,
     "The unary kernel opens with debug assertions of the slice shape."),
    ("bipedal-lane-load", SIMD, "src/bipedal/lanes.rs",
     "src.as_ptr().add(offset) as *const __m256i", 1,
     "A lane load reads four words from a raw pointer."),
    ("bipedal-lane-store", SIMD, "src/bipedal/lanes.rs",
     "_mm256_storeu_si256(dst.as_mut_ptr().add(offset) as *mut __m256i, v.0);", 1,
     "A lane store writes four words through a raw pointer."),
    ("bipedal-unsafe-entry", SIMD, "src/bipedal/mod.rs",
     "pub unsafe fn run_add_batch(", 1,
     "The crate publishes the same kernels as `unsafe fn` items as well, with the slice "
     "shape under Safety."),
    ("f5-binary-type-doc", SIMD, "src/bipedal/packed5.rs",
     '/// The AVX2 kernels panic unless all nine slices share one length that is a', 1,
     'The pointer type lists the slice shape under Panics.'),
    ("f5-unary-type-doc", SIMD, "src/bipedal/packed5.rs",
     '/// The AVX2 kernel panics unless all six slices share one length that is a', 1,
     'The pointer type lists the slice shape under Panics.'),
    ("f5-binary-wrapper", SIMD, "src/bipedal/packed5.rs",
     'b2b.len(),', 3,
     'Each of the three binary entries checks the slice shape ahead of the kernel.'),
    ("f5-unary-wrapper", SIMD, "src/bipedal/packed5.rs",
     '"F_5 neg kernel",', 1,
     'The unary entry checks the slice shape ahead of the kernel.'),
    ("f5-binary-debug", SIMD, "src/x86/bipedal_avx2_packed5.rs",
     "debug_assert_eq!(b0a.len() % 4, 0);", 3,
     "Each binary kernel opens with debug assertions of the slice shape."),
    ("f5-unary-debug", SIMD, "src/x86/bipedal_avx2_packed5.rs",
     "debug_assert_eq!(b0.len() % 4, 0);", 1,
     "The unary kernel opens with debug assertions of the slice shape."),
    ("f5-load", SIMD, "src/x86/bipedal_avx2_packed5.rs",
     "_mm256_loadu_si256(src.as_ptr().add(offset) as *const __m256i)", 1,
     "A plane load reads four words from a raw pointer."),
    ("f5-store", SIMD, "src/x86/bipedal_avx2_packed5.rs",
     "_mm256_storeu_si256(dst.as_mut_ptr().add(offset) as *mut __m256i, v);", 1,
     "A plane store writes four words through a raw pointer."),
    ("f7-binary-type-doc", SIMD, "src/bipedal/packed7.rs",
     '/// The AVX2 kernels panic unless all three slices share one length that is a', 1,
     'The pointer type lists the slice shape under Panics.'),
    ("f7-unary-type-doc", SIMD, "src/bipedal/packed7.rs",
     '/// The AVX2 kernel panics unless both slices share one length that is a', 1,
     'The pointer type lists the slice shape under Panics.'),
    ("f7-binary-wrapper", SIMD, "src/bipedal/packed7.rs",
     'crate::entry_check::assert_packed_lengths("F_7 add kernel", [a.len(), b.len(), out.len()]);',
     1,
     "The add entry checks the slice shape ahead of the kernel; the sub and mul entries hold "
     "the same call under their own names."),
    ("f7-unary-wrapper", SIMD, "src/bipedal/packed7.rs",
     'crate::entry_check::assert_packed_lengths("F_7 neg kernel", [a.len(), out.len()]);', 1,
     'The unary entry checks the slice shape ahead of the kernel.'),
    ("f7-debug", SIMD, "src/x86/bipedal_avx2_packed7.rs",
     "debug_assert_eq!(a.len() % 4, 0);", 4,
     "Each kernel opens with debug assertions of the slice shape."),
    ("f7-load", SIMD, "src/x86/bipedal_avx2_packed7.rs",
     "_mm256_loadu_si256(src.as_ptr().add(offset) as *const __m256i)", 1,
     "A word load reads four words from a raw pointer."),
    ("f7-store", SIMD, "src/x86/bipedal_avx2_packed7.rs",
     "_mm256_storeu_si256(dst.as_mut_ptr().add(offset) as *mut __m256i, v)", 1,
     "A word store writes four words through a raw pointer."),
    # Dense shape products.
    ("release-profile", None, "Cargo.toml", "[profile.release]", 1,
     "The release profile sets no `overflow-checks` key, so a product in a release "
     "assertion wraps."),
    ("row-panel-assert", SIMD, "src/x86/fp_small.rs",
     '"fp_small_gemm_row_panel: bt.len() != n * k"', 1,
     "The assertion compares `bt.len()` with a wrapping product."),
    ("row-panel-read", SIMD, "src/x86/fp_small.rs",
     "let bt0 = bt.as_ptr().add(j * k);", 1,
     "Row addresses of `bt` come from raw pointer arithmetic."),
    ("small-panel-assert", SIMD, "src/x86/fp_small_panel.rs",
     'assert_eq!(c.len(), m * n, "fp_small_panel_gemm: c shape");', 1,
     "The assertion compares `c.len()` with a wrapping product."),
    ("small-panel-write", SIMD, "src/x86/fp_small_panel.rs",
     "*c.get_unchecked_mut(c_row_base + j_off) = src;", 1,
     "Output cells are written without a bounds check."),
    ("small-f32-assert", SIMD, "src/x86/fp_small_f32.rs",
     'assert_eq!(a.len(), m * k, "fp_small_f32_gemm: a shape");', 1,
     "The assertion compares `a.len()` with a wrapping product."),
    ("small-f32-route-a-assert", SIMD, "src/x86/fp_small_f32.rs",
     'assert_eq!(a.len(), m * k, "fp_small_f32_gemm_route_a: a shape");', 1,
     "The assertion compares `a.len()` with a wrapping product."),
    ("small-f32-pack-read", SIMD, "src/x86/fp_small_f32.rs",
     "*dst_row = *a_base.add(t);", 1,
     "The A-pack reads `a` through a raw pointer; both f32 kernels share it."),
    ("medium-panel-assert", SIMD, "src/x86/fp_medium.rs",
     'assert_eq!(a.len(), m * k, "fp_medium_gemm_panel: a shape");', 1,
     "The assertion compares `a.len()` with a wrapping product."),
    ("medium-panel-pack-read", SIMD, "src/x86/fp_medium.rs",
     "*dst = *a.get_unchecked(i_blk * k + t);", 1,
     "The A-pack reads `a` without a bounds check."),
    ("medium-f64-assert", SIMD, "src/x86/fp_medium_f64.rs",
     'assert_eq!(a.len(), m * k, "fp_medium_f64_gemm: a shape");', 1,
     "The assertion compares `a.len()` with a wrapping product."),
    ("medium-f64-pack-read", SIMD, "src/x86/fp_medium_f64.rs",
     "*dst_row = *a_base.add(t);", 1,
     "The A-pack reads `a` through a raw pointer."),
    ("pam-assert", SIMD, "src/modem.rs",
     '"out.len() must equal num_symbols * pam_levels.len()"', 2,
     "Both wrappers compare `out.len()` with a wrapping product of two slice lengths."),
    ("pam-f32-store", SIMD, "src/modem.rs",
     "_mm256_storeu_ps(out_ptr.add(c * 8), v_d);", 1,
     "The f32 kernel stores through a raw pointer into `out`."),
    ("pam-f64-store", SIMD, "src/modem.rs",
     "_mm256_storeu_pd(out_ptr.add(c * 4), v_d);", 1,
     "The f64 kernel stores through a raw pointer into `out`."),
    ("row-panel-check", SIMD, "src/fp_small.rs",
     'crate::entry_check::assert_shape("fp_small_gemm_row_panel", "bt", bt.len(), n, k);', 1,
     "The safe entry forms `n * k` with `checked_mul` ahead of the kernel's assertion."),
    ("small-panel-check", SIMD, "src/fp_small_panel.rs",
     'crate::entry_check::assert_shape("fp_small_panel_gemm", "c", c.len(), m, n);', 1,
     "The safe entry forms the three shape products with `checked_mul`."),
    ("small-f32-check", SIMD, "src/fp_small_f32.rs",
     'crate::entry_check::assert_shape("fp_small_f32_gemm", "c", c.len(), m, n);', 1,
     "The safe entry forms the three shape products with `checked_mul`."),
    ("small-f32-route-a-check", SIMD, "src/fp_small_f32.rs",
     'crate::entry_check::assert_shape("fp_small_f32_gemm_route_a", "c", c.len(), m, n);', 1,
     "The safe entry forms the three shape products with `checked_mul`."),
    ("medium-panel-check", SIMD, "src/fp_medium.rs",
     'crate::entry_check::assert_shape("fp_medium_gemm_panel", "c", c.len(), m, n);', 1,
     "The safe entry forms the three shape products with `checked_mul`."),
    ("medium-f64-check", SIMD, "src/fp_medium_f64.rs",
     'crate::entry_check::assert_shape("fp_medium_f64_gemm", "c", c.len(), m, n);', 1,
     "The safe entry forms the three shape products with `checked_mul`."),
    ("pam-check", SIMD, "src/modem.rs", "crate::entry_check::checked_cells(", 2,
     "Both wrappers form the product of the two slice lengths with `checked_mul` ahead of "
     "the length assertion that compares with it."),
    # The checked precedent.
    ("gray-check-products", SIMD, "src/m4rm.rs",
     "let Some(table_words) = table_size.checked_mul(stride_words) else {", 1,
     "The shared Gray-table check forms its products with `checked_mul` and panics under "
     "the builder's name."),
    ("gray-check-call", SIMD, "src/x86/avx2.rs",
     "crate::m4rm::assert_gray_build_lengths(", 2,
     "Each specialized Gray builder runs the shared check before its `unsafe` block."),
    ("gray-check-panics-doc", SIMD, "src/lib.rs",
     "/// Panics if `stride_words` is not the width this builder specializes for, if", 1,
     "The pointer type of the checked builders lists its conditions under Panics."),
    ("gray-check-contract", SIMD, "src/m4rm.rs", "pub mod contract {", 1,
     "The builders share one argument contract, as cases any implementation can be "
     "handed; a rejection case requires a panic under the builder's name and an "
     "untouched buffer."),
    ("tile-index-check", SIMD, "src/x86/avx2.rs",
     "idx.iter().all(|&entry| {", 1,
     "The M4RM tile wrappers check eight table indices per call with checked arithmetic: "
     "the house form of a per-element index check."),
    # Callers.
    ("core-denies-unsafe", CORE, "src/lib.rs", "#![deny(unsafe_code)]", 1,
     "`gf2-core` cannot hold the `unsafe` block an `unsafe fn` pointer needs."),
    ("algebra-denies-unsafe", ALGEBRA, "src/lib.rs", "#![deny(unsafe_code)]", 1,
     "`gf2-algebra` cannot hold the `unsafe` block an `unsafe fn` pointer needs."),
    ("coding-denies-unsafe", CODING, "src/lib.rs", "#![deny(unsafe_code)]", 1,
     "`gf2-coding` cannot hold the `unsafe` block an `unsafe fn` pointer needs."),
    ("core-spmm-small-call", CORE, "src/gfp/simd_ops.rs",
     '&a_vals_u8,', 1,
     '`fp_try_spmm` calls the byte-lane entry once per product.'),
    ("core-spmm-medium-call", CORE, "src/gfp/simd_ops.rs",
     '&a_vals_u16,', 1,
     '`fp_try_spmm` calls the u16-lane entry once per product.'),
    ("core-spmm-columns-forwarded", CORE, "src/gfp/simd_ops.rs",
     '            a_col_idx,', 2,
     "Both calls pass the caller's column indices on without comparing them with `b_rows`."),
    ("core-spmm-debug", CORE, "src/gfp/simd_ops.rs",
     "debug_assert_eq!(b.len(), b_rows * n);", 1,
     "`fp_try_spmm` relates `b` to its row count in a debug assertion only."),
    ("core-spmm-hook", CORE, "src/field/traits.rs", "fn try_simd_spmm(", 1,
     "The sparse product hook is a safe method of the public `FiniteField` trait, so a "
     "caller outside `gf2-core` reaches `fp_try_spmm` with indices of its choosing."),
    ("core-spmm-matmat-assert", CORE, "src/field/sparse_matrix.rs",
     '"SparseFieldMatrix::matmat: A.cols ({}) != B.rows ({})",', 1,
     "`SparseFieldMatrix::matmat` asserts the column count against `B`'s rows before the "
     "hook."),
    ("core-sparse-triplet-assert", CORE, "src/field/sparse_matrix.rs",
     '"SparseFieldMatrix::from_triplets: col {c} out of bounds (cols={cols})"', 1,
     "A sparse matrix built from triplets holds column indices below its column count."),
    ("core-ple-small-window", CORE, "src/gfp/simd_ops.rs",
     "window.push(from_mont[raw]);", 1,
     "The byte-lane dispatcher builds the window itself, one push per cell of `m * win`, "
     "from the per-prime canonical table."),
    ("core-ple-small-table", CORE, "src/gfp/simd_ops.rs",
     "let mut inv_table = vec![0u8; p];", 1,
     "The byte-lane inverse table has one entry per residue."),
    ("core-ple-medium-window", CORE, "src/gfp/simd_ops.rs",
     "let canon = matrix[row_base + c].value() as u16;", 1,
     "The u16-lane dispatcher builds the window itself from canonical values."),
    ("core-ple-medium-table", CORE, "src/gfp/simd_ops.rs",
     "let mut table = vec![0u16; p_u64 as usize];", 1,
     "The u16-lane inverse table has one entry per residue."),
    ("core-gemm-small-debug", CORE, "src/gfp/simd_ops.rs",
     'debug_assert_eq!(a.len(), m * k, "fp_small_try_gemm_classical: a shape");', 1,
     "The small-prime dispatcher relates its operands to the dimensions in debug "
     "assertions only."),
    ("core-gemm-medium-debug", CORE, "src/gfp/simd_ops.rs",
     'debug_assert_eq!(a.len(), m * k, "fp_medium_try_gemm_panel: a shape");', 1,
     "The medium-prime dispatcher relates its operands to the dimensions in debug "
     "assertions only."),
    ("core-gemm-f32-scratch", CORE, "src/gfp/simd_ops.rs",
     "a_f32_scratch.resize(m * k, 0.0);", 1,
     "The scratch operands are sized from the same wrapping products the kernel "
     "compares, so they pass its assertions whatever the dimensions."),
    ("core-gemm-u8-scratch", CORE, "src/gfp/simd_ops.rs",
     "a_u8.resize(m * k, 0u8);", 1,
     "The byte-panel scratch is sized from the wrapping products."),
    ("core-gemm-row-scratch", CORE, "src/gfp/simd_ops.rs", "let bt_len = n * k;", 1,
     "The byte-lane baseline sizes the transposed operand from a wrapping product."),
    ("core-gemm-f32-direct", CORE, "src/gfp/simd_ops.rs",
     "let mut out_u8 = vec![0u8; m * n];", 1,
     "The direct f32 route sizes its output from a wrapping product."),
    ("core-gemm-row-loop", CORE, "src/gfp/simd_ops.rs",
     "let a_row = &a_u8[i * k..(i + 1) * k];", 1,
     "The byte-lane baseline calls the row kernel once per row of `a`, with row slices "
     "taken through bounds-checked ranges."),
    ("core-gemm-u16-scratch", CORE, "src/gfp/simd_ops.rs",
     "a_u16.resize(m * k, 0u16);", 1,
     "The u16-panel scratch is sized from the wrapping products."),
    ("core-gemm-f64-scratch", CORE, "src/gfp/simd_ops.rs",
     "a_f64.resize(m * k, 0.0f64);", 1,
     "The f64 scratch is sized from the wrapping products."),
    ("core-gemm-hook", CORE, "src/gfp/mod.rs", "fn try_simd_gemm_classical(", 1,
     "`Fp` implements the dense product hook of the public `FiniteField` trait by "
     "forwarding its dimensions to the dispatchers."),
    ("core-matvec-pack-debug", CORE, "src/gfp/simd_ops.rs",
     "debug_assert_eq!(rows.len(), m * k);", 1,
     "The packed matrix relates its data to `m * k` in a debug assertion only."),
    ("core-matvec-call", CORE, "src/gfp/simd_ops.rs",
     "(fns.gemm_row_panel_fn)(&x_u8[..*k], data, *k, *m, p_u8, &mut out_u8[..*m]);", 1,
     "The packed matvec calls the row kernel once per product, with `x` and `out` cut "
     "to `k` and `m` through bounds-checked ranges."),
    ("algebra-bipedal-buffers", ALGEBRA, "src/permanent/bipedal3.rs",
     "let mut buf_sum_mag = [0u64; 4];", 1,
     "The single-word permanent passes six four-word arrays, so the slice shape holds "
     "by construction."),
    ("algebra-bipedal-loop", ALGEBRA, "src/permanent/bipedal3.rs",
     "for (flip, parity) in gray_code_iter(n) {", 2,
     "The single-word SIMD permanent calls one kernel per Gray step."),
    ("coding-pam-output", CODING, "src/modem/fast_gray_qam_demapper.rs",
     "let mut d_i: Vec<f64> = vec![0.0; num_symbols * axis_len];", 1,
     "The demapper sizes the distance slab from the wrapping product of the lengths of "
     "two vectors it holds."),
    ("coding-pam-bench-output", CODING, "benches/cpu_dispatch_probe.rs",
     "let mut out = vec![0.0f64; batch * pam.len()];", 1,
     "The benchmark sizes the slab from the product of two small counts."),
    # Grain of the sparse row check.
    ("entry-check-row-scan", SIMD, "src/entry_check.rs",
     'let Some(largest) = a_cols.iter().copied().max() else {', 1,
     'The column check takes the largest index of the array it is handed: one pass over the stored indices.'),
    ("core-spmm-row-loop", SIMD, "src/fp_small.rs",
     'let row = a_row_ptr[r]..a_row_ptr[r + 1];', 1,
     'The row loop stands in the kernel crate, behind the check, and holds the `unsafe` call of the row kernel; the u16 lane repeats it.'),
    ("spmm-kernel-safety", SIMD, "src/x86/fp_small.rs",
     "/// - every `a_cols[h] * b_stride + n` is within `b.len()`,", 1,
     "The row kernel is an `unsafe fn` whose Safety section states the column bound; the "
     "u16 kernel states it in its own words."),
    ("measured-row-check", MEASURED, "crates/gf2-kernels-simd/src/fp_small.rs",
     'crate::entry_check::assert_spmm_row_columns("fp_small_spmm_row", a_cols, b.len(), '
     'b_stride, n);', 1,
     "In the measured tree the checked row wrapper runs the column check on every call."),
    ("measured-row-division", MEASURED, "crates/gf2-kernels-simd/src/entry_check.rs",
     "let Some(max_col) = last_start.checked_div(b_stride) else {", 1,
     "In the measured tree the check divides once per row call."),
    ("measured-row-loop", MEASURED, "crates/gf2-core/src/gfp/simd_ops.rs",
     "let end = a_row_ptr[r + 1];", 2,
     "In the measured tree the row loop stands in `gf2-core` and calls the row pointer once "
     "per non-empty sparse row."),
    ("p20-tail-support", TOOL, "src/receipt.rs",
     "|| f64::from(settings.bootstrap_resamples) * corrected_alpha / 2.0 < 20.0;", 1,
     "P-20 records a non-exploratory cell as not confirmatory when a bootstrap tail holds "
     "fewer than twenty expected draws at the corrected alpha."),
    ("ledger-attempt-alpha", TOOL, "src/trial_ledger.rs",
     "Ok(family.family_wise.alpha / (attempts * (attempts + 1.0)))", 1,
     "Attempt `t` of a family spends `alpha / (t (t + 1))`, counted from the ledger's "
     "non-exploratory reservations."),
    ("core-spmm-hook-call", CORE, "src/field/sparse_matrix.rs", "if F::try_simd_spmm(", 1,
     "`SparseFieldMatrix::matmat` passes its own CSR arrays to the hook."),
    ("core-sparse-type-doc", CORE, "src/field/sparse_matrix.rs",
     "/// All stored values are non-zero (constructors canonicalise) and per-row", 1,
     "The documented invariants of the type are non-zero values and ascending columns; the "
     "column bound is not among them."),
    ("core-sparse-private-fields", CORE, "src/field/sparse_matrix.rs",
     "col_idx: Vec<usize>,", 1,
     "The column indices are a private field, so only the type's own constructors write them."),
    ("core-sparse-raw-parts", CORE, "src/field/sparse_matrix.rs",
     "pub fn as_raw_parts(&self) -> (&[usize], &[usize], &[F]) {", 2,
     "The raw parts leave the type as shared borrows; the CSC type repeats the signature."),
    ("core-sparse-zeros", CORE, "src/field/sparse_matrix.rs", "col_idx: Vec::new(),", 2,
     "A matrix without stored entries holds no column index; `zeros` and the empty case of "
     "`rref` build one."),
    ("core-sparse-dense-columns", CORE, "src/field/sparse_matrix.rs", "for c in 0..cols {", 3,
     "`from_dense` takes each index from a loop over the column count; two tests repeat "
     "the loop header."),
    ("core-sparse-identity-columns", CORE, "src/field/sparse_matrix.rs",
     "let col_idx: Vec<usize> = (0..n).collect();", 1,
     "`identity` takes its indices from the range of its dimension."),
    ("core-sparse-matmul-columns", CORE, "src/field/sparse_matrix.rs",
     "let c = other.col_idx[kb];", 1,
     "`matmul` stores indices read from its right operand, whose column count the result "
     "takes."),
    ("core-sparse-rref-columns", CORE, "src/field/sparse_matrix.rs", "self.col_idx[s..e]", 1,
     "`rref` starts from the matrix's own indices and merges rows of that matrix, so it "
     "stores no index the input does not hold."),
    ("core-sparse-csr-columns", CORE, "src/field/sparse_matrix.rs",
     "for c in 0..self.cols {", 1,
     "`SparseFieldMatrixCsc::to_csr` takes each index from a loop over the column count."),
    # Measurement plan references.
    ("bench-sparse-matmat", CORE, "benches/sparse_field_matmul.rs",
     "fn bench_fp65521(c: &mut Criterion) {", 1,
     "A criterion bench names the sparse-times-dense consumer over a medium prime; it is "
     "no protocol arm."),
    ("bench-ple", CORE, "benches/fieldmatrix_ple.rs",
     "let r = black_box(&a).ple();", 1,
     "A criterion bench names the `FieldMatrix::ple` consumer; it is no protocol arm."),
    ("bench-permanent", ALGEBRA, "benches/batched_f3_permanent.rs",
     "black_box(permanent_bipedal3_singleword_simd(black_box(matrix), fns));", 1,
     "A criterion bench names the single-word SIMD permanent; it is no protocol arm."),
]

# Why a safe function that holds an `unsafe` block needs no caller condition
# beyond the processor feature its detection function establishes.
BASES = {
    "release-assertion": "A release-mode assertion in the wrapper or the kernel bounds every "
                         "unchecked access.",
    "length-derived": "Every access index derives from the lengths of the slices the call "
                      "passes.",
    "fixed-size": "Every operand is a value or a reference to a fixed-size object.",
    "values-only": "Every argument is a value.",
    "bounds-checked": "Every slice access is bounds-checked; the only unsafe operations "
                      "take values.",
    "hint": "The instruction is a cache hint that dereferences nothing.",
}

# (package-relative path, safe function, basis, evidence path, fragment, occurrences)
CLEARED = [
    ("src/bch_encode.rs", "bitslice_reduce_avx2_safe", "release-assertion",
     "src/x86/bch_encode.rs",
     '"a bit-sliced register holds one pad word below its {redundancy} coefficient slices"', 1),
    ("src/bch_encode.rs", "fold_block_pclmul_safe", "bounds-checked",
     "src/x86/bch_encode.rs", "crate::bch_encode::fold_block_over(", 1),
    ("src/bipedal/mod.rs", "permanent4_safe", "release-assertion",
     "src/x86/bipedal_avx2.rs", '"run_permanent4: magnitude/sign column counts must match"', 1),
    ("src/fp65537.rs", "batch_mul_safe", "release-assertion",
     "src/x86/fp65537.rs", '"fp65537_batch_mul: output length"', 1),
    ("src/fp65537.rs", "batch_add_safe", "release-assertion",
     "src/x86/fp65537.rs", '"fp65537_batch_add: output length"', 1),
    ("src/fp65537.rs", "batch_sub_safe", "release-assertion",
     "src/x86/fp65537.rs", '"fp65537_batch_sub: output length"', 1),
    ("src/fp65537.rs", "batch_karatsuba_safe", "release-assertion",
     "src/x86/fp65537.rs", '"fp65537_batch_karatsuba: out_c1 length"', 1),
    ("src/fp65537.rs", "batch_cubic_karatsuba_safe", "release-assertion",
     "src/x86/fp65537.rs", '"fp65537_batch_cubic_karatsuba: out_c1 length"', 1),
    ("src/fp_generic.rs", "batch_mul_safe", "release-assertion",
     "src/x86/fp_generic.rs", '"fp_montgomery_batch_mul: output length"', 1),
    ("src/fp_generic.rs", "batch_add_safe", "release-assertion",
     "src/x86/fp_generic.rs", '"fp_montgomery_batch_add: output length"', 1),
    ("src/fp_generic.rs", "batch_sub_safe", "release-assertion",
     "src/x86/fp_generic.rs", '"fp_montgomery_batch_sub: output length"', 1),
    ("src/fp_medium.rs", "batch_mul_safe", "release-assertion",
     "src/x86/fp_medium.rs", '"fp_medium_batch_mul: output length"', 1),
    ("src/fp_medium.rs", "batch_add_safe", "release-assertion",
     "src/x86/fp_medium.rs", '"fp_medium_batch_add: output length"', 1),
    ("src/fp_medium.rs", "batch_sub_safe", "release-assertion",
     "src/x86/fp_medium.rs", '"fp_medium_batch_sub: output length"', 1),
    ("src/fp_medium.rs", "batch_dot_safe", "release-assertion",
     "src/x86/fp_medium.rs", '"fp_medium_batch_dot: length mismatch"', 1),
    ("src/fp_small.rs", "batch_mul_safe", "release-assertion",
     "src/x86/fp_small.rs", '"fp_small_batch_mul: output length"', 1),
    ("src/fp_small.rs", "batch_add_safe", "release-assertion",
     "src/x86/fp_small.rs", '"fp_small_batch_add: output length"', 1),
    ("src/fp_small.rs", "batch_sub_safe", "release-assertion",
     "src/x86/fp_small.rs", '"fp_small_batch_sub: output length"', 1),
    ("src/fp_small.rs", "batch_dot_safe", "release-assertion",
     "src/x86/fp_small.rs", '"fp_small_batch_dot: length mismatch"', 1),
    ("src/fp_small.rs", "sub_scaled_safe", "release-assertion",
     "src/x86/fp_small.rs", '"fp_small_sub_scaled: buf shorter than chain_j ({} < {})",', 1),
    ("src/gf2m.rs", "clmul_u64_safe", "values-only",
     "src/x86/clmul.rs", "pub unsafe fn clmul_u64(a: u64, b: u64) -> u128 {", 1),
    ("src/gf2m.rs", "clmul_batch_safe", "bounds-checked",
     "src/x86/clmul.rs", "out[i] = clmul_u64(a[i], b[i]);", 2),
    ("src/gf2m.rs", "clmul_batch_ymm_safe", "bounds-checked",
     "src/x86/clmul.rs", "out[i + 1] = (hi1 as u128) << 64 | (lo1 as u128);", 1),
    ("src/gf2m.rs", "clmul_barrett_reduce_safe", "values-only",
     "src/x86/clmul.rs",
     "pub unsafe fn clmul_barrett_reduce(a: u64, b: u64, mu: u64, modulus: u64, degree: u32) "
     "-> u64 {", 1),
    ("src/gf2m.rs", "gf2m_mul_pclmul_safe", "values-only",
     "src/gf2m.rs",
     "unsafe fn gf2m_mul_pclmul(a: u64, b: u64, m: usize, primitive_poly: u64) -> u64 {", 1),
    ("src/gf2m_batch.rs", "gf2m_batch_mul_ymm_unroll4_safe", "bounds-checked",
     "src/x86/gf2m_batch.rs", "out[i..i + 4].copy_from_slice(&r);", 3),
    ("src/gf2m_batch.rs", "gf2m_batch_square_ymm_unroll4_safe", "bounds-checked",
     "src/x86/gf2m_batch.rs", "gf2m_batch_mul_ymm_unroll4(a, a, out, mu, modulus, degree);", 1),
    ("src/gf2m_gemm.rs", "gf2m_gemm_panelized_safe", "bounds-checked",
     "src/x86/gf2m_gemm.rs", "let b_row = &b_flat[ki * n..(ki + 1) * n];", 3),
    ("src/gf2m_wide.rs", "clmul_wide4_xmm_safe", "fixed-size",
     "src/x86/gf2m_wide.rs",
     "pub unsafe fn clmul_wide4_xmm(a: &[u64; 4], b: &[u64; 4], out: &mut [u64; 8]) {", 1),
    ("src/gf2m_wide.rs", "clmul_wide4_ymm_safe", "fixed-size",
     "src/x86/gf2m_wide.rs",
     "pub unsafe fn clmul_wide4_ymm(a: &[u64; 4], b: &[u64; 4], out: &mut [u64; 8]) {", 1),
    ("src/gf2m_wide.rs", "clmul_wide9_xmm_safe", "fixed-size",
     "src/x86/gf2m_wide.rs",
     "pub unsafe fn clmul_wide9_xmm(a: &[u64; 9], b: &[u64; 9], out: &mut [u64; 18]) {", 1),
    ("src/gf2m_wide.rs", "clmul_wide9_ymm_safe", "fixed-size",
     "src/x86/gf2m_wide.rs",
     "pub unsafe fn clmul_wide9_ymm(a: &[u64; 9], b: &[u64; 9], out: &mut [u64; 18]) {", 1),
    ("src/llr.rs", "minsum_avx2_f32_safe", "length-derived",
     "src/llr.rs", "unsafe fn minsum_avx2_f32(inputs: &[f32]) -> f32 {", 1),
    ("src/llr.rs", "maxabs_avx2_f32_safe", "length-derived",
     "src/llr.rs", "unsafe fn maxabs_avx2_f32(inputs: &[f32]) -> f32 {", 1),
    ("src/llr.rs", "minsum_avx2_f64_safe", "length-derived",
     "src/llr.rs", "unsafe fn minsum_avx2_f64(inputs: &[f64]) -> f64 {", 1),
    ("src/llr.rs", "maxabs_avx2_f64_safe", "length-derived",
     "src/llr.rs", "unsafe fn maxabs_avx2_f64(inputs: &[f64]) -> f64 {", 1),
    ("src/mersenne.rs", "m31_batch_mul_safe", "release-assertion",
     "src/x86/mersenne.rs", '"mersenne31_batch_mul: output length"', 1),
    ("src/mersenne.rs", "m31_batch_mul_add_safe", "release-assertion",
     "src/x86/mersenne.rs", '"mersenne31_batch_mul_add: acc length"', 1),
    ("src/mersenne.rs", "m31_batch_dot_safe", "release-assertion",
     "src/x86/mersenne.rs", '"mersenne31_batch_dot: length mismatch"', 1),
    ("src/prefetch.rs", "prefetch_read_l1", "hint",
     "src/prefetch.rs", "unsafe { _mm_prefetch(ptr.cast::<i8>(), _MM_HINT_T0) };", 1),
    ("src/shift_funnel.rs", "shift_left_funnel_bmi2_safe", "release-assertion",
     "src/shift_funnel.rs",
     "fn assert_funnel_arguments(words: usize, word_shift: usize, bit_shift: u32) {", 1),
    ("src/shift_funnel.rs", "shift_right_funnel_bmi2_safe", "release-assertion",
     "src/shift_funnel.rs", "assert_funnel_arguments(data.len(), word_shift, bit_shift);", 2),
    ("src/transpose.rs", "transpose_64x64_avx2_safe", "fixed-size",
     "src/x86/transpose.rs",
     "pub(crate) unsafe fn transpose_64x64_avx2(input: &[u64; 64], output: &mut [u64; 64]) {",
     1),
    ("src/transpose.rs", "transpose_64x64_avx2_ymm6_safe", "fixed-size",
     "src/x86/transpose.rs",
     "pub(crate) unsafe fn transpose_64x64_avx2_ymm6(input: &[u64; 64], output: &mut [u64; 64]) "
     "{", 1),
    ("src/transpose.rs", "transpose_64x64_avx2_pshufb_safe", "fixed-size",
     "src/x86/transpose.rs",
     "pub(crate) unsafe fn transpose_64x64_avx2_pshufb(input: &[u64; 64], output: &mut [u64; "
     "64]) {", 1),
    ("src/transpose.rs", "transpose_64x64_avx2_movemask_safe", "fixed-size",
     "src/x86/transpose.rs",
     "pub(crate) unsafe fn transpose_64x64_avx2_movemask(input: &[u64; 64], output: &mut [u64; "
     "64]) {", 1),
    ("src/x86/avx2.rs", "and_fn", "length-derived",
     "src/x86/avx2.rs", "unsafe fn avx2_and_into(dst: &mut [u64], src: &[u64]) {", 1),
    ("src/x86/avx2.rs", "or_fn", "length-derived",
     "src/x86/avx2.rs", "unsafe fn avx2_or_into(dst: &mut [u64], src: &[u64]) {", 1),
    ("src/x86/avx2.rs", "xor_fn", "length-derived",
     "src/x86/avx2.rs", "unsafe fn avx2_xor_into(dst: &mut [u64], src: &[u64]) {", 1),
    ("src/x86/avx2.rs", "m4rm_gray_xor16_fn", "release-assertion",
     "src/x86/avx2.rs", '"m4rm_gray_xor16: src must contain at least 16 words"', 1),
    ("src/x86/avx2.rs", "m4rm_gray_build4_fn", "release-assertion",
     "src/x86/avx2.rs", '"m4rm_gray_build4",', 1),
    ("src/x86/avx2.rs", "m4rm_gray_build8_fn", "release-assertion",
     "src/x86/avx2.rs", '"m4rm_gray_build8",', 1),
    ("src/x86/avx2.rs", "m4rm_tile8x4_fn", "release-assertion",
     "src/x86/avx2.rs",
     '"m4rm_tile8x4: table_buffer must cover all indexed 8x4 table rows"', 1),
    ("src/x86/avx2.rs", "m4rm_tile8xn_fn", "release-assertion",
     "src/x86/avx2.rs",
     '"m4rm_tile8xn: table_buffer must cover all indexed 8xN table rows"', 1),
    ("src/x86/avx2.rs", "not_fn", "length-derived",
     "src/x86/avx2.rs", "unsafe fn avx2_not_into(buf: &mut [u64]) {", 1),
    ("src/x86/avx2.rs", "popcnt_fn", "length-derived",
     "src/x86/avx2.rs", "unsafe fn avx2_popcnt(buf: &[u64]) -> u64 {", 1),
    ("src/x86/avx2.rs", "and_popcnt_fn", "length-derived",
     "src/x86/avx2.rs", "unsafe fn avx2_and_popcnt(lhs: &[u64], rhs: &[u64]) -> u64 {", 1),
    ("src/x86/avx2.rs", "and_parity_fn", "length-derived",
     "src/x86/avx2.rs", "unsafe fn avx2_and_parity(lhs: &[u64], rhs: &[u64]) -> bool {", 1),
    ("src/x86/avx2.rs", "popcnt_csa_fn", "length-derived",
     "src/x86/popcount.rs", "pub(crate) unsafe fn avx2_popcnt_csa(buf: &[u64]) -> u64 {", 1),
    ("src/x86/avx2.rs", "and_popcnt_csa_fn", "length-derived",
     "src/x86/popcount.rs",
     "pub(crate) unsafe fn avx2_and_popcnt_csa(lhs: &[u64], rhs: &[u64]) -> u64 {", 1),
    ("src/x86/avx2.rs", "popcnt_instruction_fn", "length-derived",
     "src/x86/popcount.rs", "pub(crate) unsafe fn popcnt_words(buf: &[u64]) -> u64 {", 1),
    ("src/x86/avx2.rs", "find_first_one_fn", "length-derived",
     "src/x86/avx2.rs", "unsafe fn avx2_find_first_one(buf: &[u64]) -> Option<usize> {", 1),
    ("src/x86/avx2.rs", "find_first_zero_fn", "length-derived",
     "src/x86/avx2.rs", "unsafe fn avx2_find_first_zero(buf: &[u64]) -> Option<usize> {", 1),
    ("src/x86/avx2.rs", "shift_left_words_fn", "length-derived",
     "src/x86/avx2.rs", "unsafe fn avx2_shift_left_words(buf: &mut [u64], word_shift: usize) {",
     1),
    ("src/x86/avx2.rs", "shift_right_words_fn", "length-derived",
     "src/x86/avx2.rs",
     "unsafe fn avx2_shift_right_words(buf: &mut [u64], word_shift: usize) {", 1),
    ("src/x86/ldpc_lanes.rs", "lane_check_step_avx2", "release-assertion",
     "src/x86/ldpc_lanes.rs", '"lane_check_step_avx2",', 1),
    ("src/x86/ldpc_lanes.rs", "lane_variable_step_avx2", "release-assertion",
     "src/x86/ldpc_lanes.rs", '"lane_variable_step_avx2",', 1),
    ("src/x86/ldpc_lanes.rs", "load", "fixed-size",
     "src/x86/ldpc_lanes.rs", "fn load(lanes: &F32x8) -> __m256 {", 1),
    ("src/x86/ldpc_lanes.rs", "store", "fixed-size",
     "src/x86/ldpc_lanes.rs", "fn store(lanes: &mut F32x8, value: __m256) {", 1),
]

# One cost class per kind of check; interfaces of one class are decided together.
CLASSES = {
    "sparse-row-index": {
        "title": "Sparse row kernels: one index bound per stored non-zero",
        "check": "One pass over the stored column indices of the whole product and one "
                 "division, ahead of the row loop.",
        "granularity": "per element, once per product",
        "hoist": "The row loop stands in the kernel crate behind the check and leaves the "
                 "row kernel's loops as they are: the kernel reads each index once per 16 "
                 "output columns and once per remaining column.",
        "hot_path": "yes: one scan per sparse product",
    },
    "ple-panel": {
        "title": "PLE panel base kernels: window shape and inverse-table index",
        "check": "One checked product and one comparison per call for the window shape, "
                 "one comparison for the table length and one pass that requires every "
                 "window entry below `p`.",
        "granularity": "per call, plus one comparison per window entry",
        "hoist": "The whole check stands in the safe entry point, ahead of the "
                 "elimination. The kernel keeps every entry below `p`, so each pivot "
                 "indexes inside the table and the kernel's pivot read stays unchecked.",
        "hot_path": "per panel, outside the row and lane loops",
    },
    "packed-lengths": {
        "title": "Packed F_3, F_5 and F_7 batch kernels: equal lengths, a multiple of four",
        "check": "One mask test and one length comparison per further slice, per call.",
        "granularity": "per call",
        "hoist": "The check stands ahead of the word loop. A caller that passes four "
                 "words per call runs it once per vector iteration.",
        "hot_path": "yes for the two F_3 kernels the single-word SIMD permanent calls "
                    "once per Gray step; no tracked Rust file calls the other entry "
                    "points",
    },
    "dense-shape-product": {
        "title": "Dense kernels: shape products that wrap",
        "check": "`checked_mul` in place of each wrapping product the release "
                 "assertions compare.",
        "granularity": "per call",
        "hoist": "The comparisons exist; only their products change, ahead of every "
                 "loop.",
        "hot_path": "no: once per matrix product, per row product or per demapped axis",
    },
}

# The safe entry points whose kernel reaches an unchecked access when a caller
# condition fails, grouped by published pointer type.
#
# `conditions`: (text, claims that show where the kernel checks it in a debug
# build, claims of the unchecked access it guards). `contract_claims`: the
# Panics text of the pointer type and the release check of the safe entry.
# `release_checked`: claims of what the kernel itself checks in a release
# build. `witness`: an argument tuple that passes the kernel's own release
# assertions with an operand shorter than the kernel reads or writes.
INTERFACES = [
    {
        "id": "small-prime-spmm-row", "class": "sparse-row-index",
        "type": "fp_small::SmallPrimeSpmmFn", "field": "spmm_fn",
        "bundle": "fp_small::SmallPrimeFns",
        "entries": [("small-spmm", "src/fp_small.rs", "spmm_safe",
                     "x86::fp_small::fp_small_spmm_row")],
        "conditions": [
            ("every `a_cols[h] * n + n` is at most `b.len()`, without overflow",
             [], ["small-spmm-vector-read", "small-spmm-tail-read"]),
        ],
        "contract_claims": ["small-spmm-type-doc", "small-spmm-wrapper"],
        "release_checked": ["small-spmm-length-assert"],
        "prototype": ("spmm_small_current", "spmm_small_checked"),
        "kernel_listing": ("fp_small.asm.txt",
                           "gf2_kernels_simd::x86::fp_small::fp_small_spmm_row"),
    },
    {
        "id": "medium-prime-spmm-row", "class": "sparse-row-index",
        "type": "fp_medium::MediumPrimeSpmmFn", "field": "spmm_fn",
        "bundle": "fp_medium::MediumPrimeFns",
        "entries": [("medium-spmm", "src/fp_medium.rs", "spmm_safe",
                     "x86::fp_medium::fp_medium_spmm_row")],
        "conditions": [
            ("every `a_cols[h] * n + n` is at most `b.len()`, without overflow",
             [], ["medium-spmm-vector-read", "medium-spmm-tail-read"]),
        ],
        "contract_claims": ["medium-spmm-type-doc", "medium-spmm-wrapper"],
        "release_checked": ["medium-spmm-length-assert"],
        "prototype": ("spmm_medium_current", "spmm_medium_checked"),
    },
    {
        "id": "small-prime-ple-panel", "class": "ple-panel",
        "type": "fp_small_ple::SmallPrimePlePanelBaseFn", "field": "ple_panel_base_fn",
        "bundle": "fp_small_ple::SmallPrimePlePanelFns",
        "entries": [("small-ple", "src/fp_small_ple.rs", "ple_panel_base_safe",
                     "x86::fp_small_ple::ple_panel_base_canonical")],
        "conditions": [
            ("`window.len()` is `m * win`, without overflow",
             ["small-ple-window-debug"], ["small-ple-window-read"]),
            ("every pivot value is below `inv_table.len()`",
             ["small-ple-table-debug"], ["small-ple-table-read"]),
        ],
        "contract_claims": ["small-ple-type-doc", "small-ple-wrapper"],
        "release_checked": ["small-ple-perm-swap", "small-ple-scratch-write"],
        "prototype": ("ple_small_current", "ple_small_checked"),
        "prototype_extra": [
            ("entry scan in the safe entry point", "ple_small_current",
             "ple_small_scan_checked"),
            ("pivot inverse read", "pivot_inverse_u8_current", "pivot_inverse_u8_checked"),
        ],
        "kernel_listing": ("fp_small_ple.asm.txt",
                           "gf2_kernels_simd::x86::fp_small_ple::ple_panel_base_canonical"),
    },
    {
        "id": "medium-prime-ple-panel", "class": "ple-panel",
        "type": "fp_medium_ple::MediumPrimePlePanelBaseFn", "field": "ple_panel_base_fn",
        "bundle": "fp_medium_ple::MediumPrimePlePanelFns",
        "entries": [("medium-ple", "src/fp_medium_ple.rs", "ple_panel_base_safe",
                     "x86::fp_medium_ple::ple_panel_base_canonical_u16")],
        "conditions": [
            ("`window.len()` is `m * win`, without overflow",
             ["medium-ple-window-debug"], ["medium-ple-window-read"]),
            ("every pivot value is below `inv_table.len()`",
             ["medium-ple-table-debug"], ["medium-ple-table-read"]),
        ],
        "contract_claims": ["medium-ple-type-doc", "medium-ple-wrapper"],
        "release_checked": ["medium-ple-perm-swap", "medium-ple-scratch-write"],
        "prototype": ("ple_medium_current", "ple_medium_checked"),
        "prototype_extra": [
            ("entry scan in the safe entry point", "ple_medium_current",
             "ple_medium_scan_checked"),
            ("pivot inverse read", "pivot_inverse_u16_current", "pivot_inverse_u16_checked"),
        ],
        "kernel_listing": (
            "fp_medium_ple.asm.txt",
            "gf2_kernels_simd::x86::fp_medium_ple::ple_panel_base_canonical_u16"),
    },
    {
        "id": "bipedal-binary", "class": "packed-lengths",
        "type": "bipedal::BipedalBinaryKernelFn", "field": "add_fn, sub_fn, mul_fn",
        "bundle": "bipedal::BipedalAvx2Fns",
        "entries": [
            ("bipedal-add", "src/bipedal/mod.rs", "add_safe",
             "x86::bipedal_avx2::run_add_batch::<Config3>"),
            ("bipedal-sub", "src/bipedal/mod.rs", "sub_safe",
             "x86::bipedal_avx2::run_sub_batch::<Config3>"),
            ("bipedal-mul", "src/bipedal/mod.rs", "mul_safe",
             "x86::bipedal_avx2::run_mul_batch::<Config3>"),
        ],
        "conditions": [
            ("the six slices share one length that is a multiple of four",
             ["bipedal-binary-debug"], ["bipedal-lane-load", "bipedal-lane-store"]),
        ],
        "contract_claims": ["bipedal-binary-type-doc", "bipedal-binary-wrapper"],
        "release_checked": [],
        "prototype": ("bipedal_binary_current", "bipedal_binary_checked"),
        "kernel_listing": ("bipedal_avx2.asm.txt",
                           "gf2_kernels_simd::x86::bipedal_avx2::run_add_batch"),
        "unchecked_precedent": "bipedal-unsafe-entry",
    },
    {
        "id": "bipedal-unary", "class": "packed-lengths",
        "type": "bipedal::BipedalUnaryKernelFn", "field": "neg_fn",
        "bundle": "bipedal::BipedalAvx2Fns",
        "entries": [("bipedal-neg", "src/bipedal/mod.rs", "neg_safe",
                     "x86::bipedal_avx2::run_neg_batch::<Config3>")],
        "conditions": [
            ("the four slices share one length that is a multiple of four",
             ["bipedal-unary-debug"], ["bipedal-lane-load", "bipedal-lane-store"]),
        ],
        "contract_claims": ["bipedal-unary-type-doc", "bipedal-unary-wrapper"],
        "release_checked": [],
        "prototype": ("bipedal_unary_current", "bipedal_unary_checked"),
        "kernel_listing": ("bipedal_avx2.asm.txt",
                           "gf2_kernels_simd::x86::bipedal_avx2::run_neg_batch"),
        "unchecked_precedent": "bipedal-unsafe-entry",
    },
    {
        "id": "f5-binary", "class": "packed-lengths",
        "type": "bipedal::packed5::F5BinaryKernelFn", "field": "add_fn, sub_fn, mul_fn",
        "bundle": "bipedal::packed5::F5AvxFns",
        "entries": [
            ("f5-add", "src/bipedal/packed5.rs", "add5_safe",
             "x86::bipedal_avx2_packed5::run_add5_batch"),
            ("f5-sub", "src/bipedal/packed5.rs", "sub5_safe",
             "x86::bipedal_avx2_packed5::run_sub5_batch"),
            ("f5-mul", "src/bipedal/packed5.rs", "mul5_safe",
             "x86::bipedal_avx2_packed5::run_mul5_batch"),
        ],
        "conditions": [
            ("the nine slices share one length that is a multiple of four",
             ["f5-binary-debug"], ["f5-load", "f5-store"]),
        ],
        "contract_claims": ["f5-binary-type-doc", "f5-binary-wrapper"],
        "release_checked": [],
        "prototype": ("f5_binary_current", "f5_binary_checked"),
        "kernel_listing": ("bipedal_avx2_packed5.asm.txt",
                           "gf2_kernels_simd::x86::bipedal_avx2_packed5::run_add5_batch"),
    },
    {
        "id": "f5-unary", "class": "packed-lengths",
        "type": "bipedal::packed5::F5UnaryKernelFn", "field": "neg_fn",
        "bundle": "bipedal::packed5::F5AvxFns",
        "entries": [("f5-neg", "src/bipedal/packed5.rs", "neg5_safe",
                     "x86::bipedal_avx2_packed5::run_neg5_batch")],
        "conditions": [
            ("the six slices share one length that is a multiple of four",
             ["f5-unary-debug"], ["f5-load", "f5-store"]),
        ],
        "contract_claims": ["f5-unary-type-doc", "f5-unary-wrapper"],
        "release_checked": [],
        "prototype": ("f5_unary_current", "f5_unary_checked"),
        "kernel_listing": ("bipedal_avx2_packed5.asm.txt",
                           "gf2_kernels_simd::x86::bipedal_avx2_packed5::run_neg5_batch"),
    },
    {
        "id": "f7-binary", "class": "packed-lengths",
        "type": "bipedal::packed7::F7BinaryKernelFn", "field": "add_fn, sub_fn, mul_fn",
        "bundle": "bipedal::packed7::F7AvxFns",
        "entries": [
            ("f7-add", "src/bipedal/packed7.rs", "add7_safe",
             "x86::bipedal_avx2_packed7::run_add7_batch"),
            ("f7-sub", "src/bipedal/packed7.rs", "sub7_safe",
             "x86::bipedal_avx2_packed7::run_sub7_batch"),
            ("f7-mul", "src/bipedal/packed7.rs", "mul7_safe",
             "x86::bipedal_avx2_packed7::run_mul7_batch"),
        ],
        "conditions": [
            ("the three slices share one length that is a multiple of four",
             ["f7-debug"], ["f7-load", "f7-store"]),
        ],
        "contract_claims": ["f7-binary-type-doc", "f7-binary-wrapper"],
        "release_checked": [],
        "prototype": ("f7_binary_current", "f7_binary_checked"),
        "kernel_listing": ("bipedal_avx2_packed7.asm.txt",
                           "gf2_kernels_simd::x86::bipedal_avx2_packed7::run_add7_batch"),
    },
    {
        "id": "f7-unary", "class": "packed-lengths",
        "type": "bipedal::packed7::F7UnaryKernelFn", "field": "neg_fn",
        "bundle": "bipedal::packed7::F7AvxFns",
        "entries": [("f7-neg", "src/bipedal/packed7.rs", "neg7_safe",
                     "x86::bipedal_avx2_packed7::run_neg7_batch")],
        "conditions": [
            ("the two slices share one length that is a multiple of four",
             ["f7-debug"], ["f7-load", "f7-store"]),
        ],
        "contract_claims": ["f7-unary-type-doc", "f7-unary-wrapper"],
        "release_checked": [],
        "prototype": ("f7_unary_current", "f7_unary_checked"),
        "kernel_listing": ("bipedal_avx2_packed7.asm.txt",
                           "gf2_kernels_simd::x86::bipedal_avx2_packed7::run_neg7_batch"),
    },
    {
        "id": "small-prime-gemm-row-panel", "class": "dense-shape-product",
        "type": "fp_small::SmallPrimeGemmRowPanelFn", "field": "gemm_row_panel_fn",
        "bundle": "fp_small::SmallPrimeFns",
        "entries": [("gemm-row-panel", "src/fp_small.rs", "gemm_row_panel_safe",
                     "x86::fp_small::fp_small_gemm_row_panel")],
        "conditions": [
            ("`n * k` does not overflow `usize`", ["row-panel-assert"], ["row-panel-read"]),
        ],
        "contract_claims": ["row-panel-check"],
        "release_checked": ["row-panel-assert"],
        "prototype": ("gemm_row_panel_current", "gemm_row_panel_checked"),
        "witness": {"dimensions": {"k": 2**32, "n": 2**32},
                    "lengths": {"a": ("k",), "bt": ("n", "k"), "out": ("n",)},
                    "short": "bt"},
    },
    {
        "id": "small-prime-panel-gemm", "class": "dense-shape-product",
        "type": "fp_small_panel::SmallPrimePanelGemmFn", "field": "batch_gemm_fn",
        "bundle": "fp_small_panel::SmallPrimePanelFns",
        "entries": [("small-panel-gemm", "src/fp_small_panel.rs", "batch_gemm_safe",
                     "x86::fp_small_panel::fp_small_panel_gemm")],
        "conditions": [
            ("`m * k`, `n * k` and `m * n` do not overflow `usize`",
             ["small-panel-assert"], ["small-panel-write"]),
        ],
        "contract_claims": ["small-panel-check"],
        "release_checked": ["small-panel-assert"],
        "prototype": ("small_panel_gemm_current", "small_panel_gemm_checked"),
        "witness": {"dimensions": {"m": 2**40, "k": 1, "n": 2**24},
                    "lengths": {"a": ("m", "k"), "bt": ("n", "k"), "c": ("m", "n")},
                    "short": "c"},
    },
    {
        "id": "small-prime-f32-gemm", "class": "dense-shape-product",
        "type": "fp_small_f32::SmallPrimeF32GemmFn",
        "field": "batch_gemm_fn, batch_gemm_route_a_fn",
        "bundle": "fp_small_f32::SmallPrimeF32Fns",
        "entries": [
            ("small-f32-gemm", "src/fp_small_f32.rs", "batch_gemm_safe",
             "x86::fp_small_f32::fp_small_f32_gemm"),
            ("small-f32-gemm-route-a", "src/fp_small_f32.rs", "batch_gemm_route_a_safe",
             "x86::fp_small_f32::fp_small_f32_gemm_route_a"),
        ],
        "conditions": [
            ("`m * k`, `n * k` and `m * n` do not overflow `usize`",
             ["small-f32-assert", "small-f32-route-a-assert"], ["small-f32-pack-read"]),
        ],
        "contract_claims": ["small-f32-check", "small-f32-route-a-check"],
        "release_checked": ["small-f32-assert", "small-f32-route-a-assert"],
        "prototype": ("small_f32_gemm_current", "small_f32_gemm_checked"),
        "witness": {"dimensions": {"m": 2**63, "k": 2, "n": 2},
                    "lengths": {"a": ("m", "k"), "bt": ("n", "k"), "c": ("m", "n")},
                    "short": "a"},
    },
    {
        "id": "medium-prime-gemm-panel", "class": "dense-shape-product",
        "type": "fp_medium::MediumPrimeGemmPanelFn", "field": "gemm_panel_fn",
        "bundle": "fp_medium::MediumPrimeFns",
        "entries": [("medium-gemm-panel", "src/fp_medium.rs", "gemm_panel_safe",
                     "x86::fp_medium::fp_medium_gemm_panel")],
        "conditions": [
            ("`m * k`, `n * k` and `m * n` do not overflow `usize`",
             ["medium-panel-assert"], ["medium-panel-pack-read"]),
        ],
        "contract_claims": ["medium-panel-check"],
        "release_checked": ["medium-panel-assert"],
        "prototype": ("medium_panel_gemm_current", "medium_panel_gemm_checked"),
        "witness": {"dimensions": {"m": 2**63, "k": 2, "n": 2},
                    "lengths": {"a": ("m", "k"), "bt": ("n", "k"), "c": ("m", "n")},
                    "short": "a"},
    },
    {
        "id": "medium-prime-f64-gemm", "class": "dense-shape-product",
        "type": "fp_medium_f64::FpMediumF64GemmFn", "field": "batch_gemm_fn",
        "bundle": "fp_medium_f64::FpMediumF64Fns",
        "entries": [("medium-f64-gemm", "src/fp_medium_f64.rs", "batch_gemm_safe",
                     "x86::fp_medium_f64::fp_medium_f64_gemm")],
        "conditions": [
            ("`m * k`, `n * k` and `m * n` do not overflow `usize`",
             ["medium-f64-assert"], ["medium-f64-pack-read"]),
        ],
        "contract_claims": ["medium-f64-check"],
        "release_checked": ["medium-f64-assert"],
        "prototype": ("medium_f64_gemm_current", "medium_f64_gemm_checked"),
        "witness": {"dimensions": {"m": 2**63, "k": 2, "n": 2},
                    "lengths": {"a": ("m", "k"), "bt": ("n", "k"), "c": ("m", "n")},
                    "short": "a"},
    },
    {
        "id": "pam-distances-f32", "class": "dense-shape-product",
        "type": "modem::PamSqDistancesF32Fn", "field": "pam_sq_distances_fn",
        "bundle": "modem::GrayPamDistanceFnsF32",
        "entries": [("pam-f32", "src/modem.rs", "pam_sq_distances_f32_avx2_safe",
                     "modem::avx2::pam_sq_distances_f32_avx2")],
        "conditions": [
            ("`z.len() * pam_levels.len()` does not overflow `usize`",
             ["pam-assert"], ["pam-f32-store"]),
        ],
        "contract_claims": ["pam-check"],
        "release_checked": ["pam-assert"],
        "prototype": ("pam_f32_current", "pam_f32_checked"),
        "witness": {"dimensions": {"symbols": 2**32, "levels": 2**32},
                    "lengths": {"z": ("symbols",), "pam_levels": ("levels",),
                                "out": ("symbols", "levels")},
                    "short": "out"},
    },
    {
        "id": "pam-distances-f64", "class": "dense-shape-product",
        "type": "modem::PamSqDistancesF64Fn", "field": "pam_sq_distances_fn",
        "bundle": "modem::GrayPamDistanceFnsF64",
        "entries": [("pam-f64", "src/modem.rs", "pam_sq_distances_f64_avx2_safe",
                     "modem::avx2::pam_sq_distances_f64_avx2")],
        "conditions": [
            ("`z.len() * pam_levels.len()` does not overflow `usize`",
             ["pam-assert"], ["pam-f64-store"]),
        ],
        "contract_claims": ["pam-check"],
        "release_checked": ["pam-assert"],
        "prototype": ("pam_f64_current", "pam_f64_checked"),
        "witness": {"dimensions": {"symbols": 2**32, "levels": 2**32},
                    "lengths": {"z": ("symbols",), "pam_levels": ("levels",),
                                "out": ("symbols", "levels")},
                    "short": "out"},
    },
]

# The contract test calls every pointer with arguments that break its
# condition; it is outside the caller scan.
CONDITION_TEST = (SIMD, "tests/entry_conditions.rs")

# How a call site meets the entry point's conditions.
DISCHARGES = {
    "by-construction": "the caller builds the operands so that the condition holds in "
                       "every build",
    "forwarded": "the caller passes its own arguments on; a debug assertion or the "
                 "invariant of one upstream type is all that holds the condition",
    "wrapping": "the caller sizes its operands from the same wrapping products, so it "
                "passes the release assertions without establishing that the products "
                "fit",
    "test-fixture": "a test, benchmark or example builds operands of the stated shape",
}

# Every use of a published pointer field in a tracked Rust file, by file and
# field: (package or None for a root-relative path, path, field, kind,
# [(entry id or None, count)], discharge key or the reason the use is outside
# the inventory, claims).
USES = [
    (CORE, "src/gfp/simd_ops.rs", "spmm_fn", "production",
     [("small-spmm", 1), ("medium-spmm", 1)], "forwarded",
     ["core-spmm-small-call", "core-spmm-medium-call", "core-spmm-columns-forwarded",
      "core-spmm-debug", "core-spmm-hook", "core-spmm-matmat-assert",
      "core-sparse-triplet-assert"]),
    (CORE, "src/gfp/simd_ops.rs", "ple_panel_base_fn", "production",
     [("small-ple", 1), ("medium-ple", 1)], "by-construction",
     ["core-ple-small-window", "core-ple-small-table", "core-ple-medium-window",
      "core-ple-medium-table"]),
    (CORE, "src/gfp/simd_ops.rs", "batch_gemm_fn", "production",
     [("small-panel-gemm", 1), ("small-f32-gemm", 1), ("medium-f64-gemm", 1)], "wrapping",
     ["core-gemm-u8-scratch", "core-gemm-f32-direct", "core-gemm-f64-scratch",
      "core-gemm-small-debug", "core-gemm-medium-debug", "core-gemm-hook"]),
    (CORE, "src/gfp/simd_ops.rs", "batch_gemm_route_a_fn", "production",
     [("small-f32-gemm-route-a", 1)], "wrapping",
     ["core-gemm-f32-scratch", "core-gemm-small-debug", "core-gemm-hook"]),
    (CORE, "src/gfp/simd_ops.rs", "gemm_panel_fn", "production",
     [("medium-gemm-panel", 1)], "wrapping",
     ["core-gemm-u16-scratch", "core-gemm-medium-debug", "core-gemm-hook"]),
    (CORE, "src/gfp/simd_ops.rs", "gemm_row_panel_fn", "production",
     [("gemm-row-panel", 1)], "wrapping",
     ["core-gemm-row-loop", "core-gemm-row-scratch", "core-gemm-small-debug"]),
    (CORE, "src/gfp/simd_ops.rs", "gemm_row_panel_fn", "production",
     [("gemm-row-panel", 1)], "forwarded",
     ["core-matvec-call", "core-matvec-pack-debug"]),
    (ALGEBRA, "src/permanent/bipedal3.rs", "add_fn", "production",
     [("bipedal-add", 1)], "by-construction",
     ["algebra-bipedal-buffers", "algebra-bipedal-loop"]),
    (ALGEBRA, "src/permanent/bipedal3.rs", "sub_fn", "production",
     [("bipedal-sub", 1)], "by-construction",
     ["algebra-bipedal-buffers", "algebra-bipedal-loop"]),
    (CODING, "src/modem/fast_gray_qam_demapper.rs", "pam_sq_distances_fn", "production",
     [("pam-f64", 1)], "wrapping", ["coding-pam-output"]),
    (CODING, "benches/cpu_dispatch_probe.rs", "pam_sq_distances_fn", "bench",
     [("pam-f64", 1), (None, 1)], "test-fixture", ["coding-pam-bench-output"]),
    (SIMD, "src/fp_small_ple.rs", "ple_panel_base_fn", "test",
     [("small-ple", 2)], "test-fixture", []),
    (SIMD, "src/fp_medium_ple.rs", "ple_panel_base_fn", "test",
     [("medium-ple", 2)], "test-fixture", []),
    (SIMD, "src/fp_small_panel.rs", "batch_gemm_fn", "test",
     [("small-panel-gemm", 2)], "test-fixture", []),
    (SIMD, "src/fp_small_f32.rs", "batch_gemm_fn", "test",
     [("small-f32-gemm", 2)], "test-fixture", []),
    (SIMD, "src/fp_small_f32.rs", "batch_gemm_route_a_fn", "test",
     [("small-f32-gemm-route-a", 1)], "test-fixture", []),
    (SIMD, "src/fp_medium_f64.rs", "batch_gemm_fn", "test",
     [("medium-f64-gemm", 2)], "test-fixture", []),
    (SIMD, "src/modem.rs", "pam_sq_distances_fn", "test",
     [("pam-f32", 4), ("pam-f64", 2)], "test-fixture", []),
    (CORE, "src/gf2m/batch.rs", "mul_fn", "production", [(None, 1)],
     "`Gf2mBatchFns::mul_fn`, a GF(2^m) bundle outside the inventory", []),
    (CORE, "src/gf2m/field.rs", "mul_fn", "production", [(None, 1)],
     "`Gf2mFns::mul_fn`, a GF(2^m) bundle outside the inventory", []),
    (CORE, "benches/gf2m_mul_strategies.rs", "mul_fn", "bench", [(None, 1)],
     "`Gf2mFns::mul_fn`, a GF(2^m) bundle outside the inventory", []),
    (SIMD, "src/gf2m.rs", "mul_fn", "test", [(None, 7)],
     "`Gf2mFns::mul_fn`, a GF(2^m) bundle outside the inventory", []),
    (SIMD, "src/gf2m_batch.rs", "mul_fn", "test", [(None, 3)],
     "`Gf2mBatchFns::mul_fn`, a GF(2^m) bundle outside the inventory", []),
]

# The use of the scalar bundle in the dispatch-probe bench.
OUTSIDE_NOTE = "`scalar_fns_f64`, the scalar implementation, which holds no `unsafe` block"

# Crates whose library target denies `unsafe_code`, with the claim.
DENYING = {CORE: "core-denies-unsafe", ALGEBRA: "algebra-denies-unsafe",
           CODING: "coding-denies-unsafe"}

# Safe methods of the public `FiniteField` trait that pass their caller's
# arguments on to an entry point: interface id to (hook, claim).
FORWARDING_HOOKS = {
    "small-prime-spmm-row": ("FiniteField::try_simd_spmm", "core-spmm-hook"),
    "medium-prime-spmm-row": ("FiniteField::try_simd_spmm", "core-spmm-hook"),
    "small-prime-gemm-row-panel": ("FiniteField::try_simd_gemm_classical", "core-gemm-hook"),
    "small-prime-panel-gemm": ("FiniteField::try_simd_gemm_classical", "core-gemm-hook"),
    "small-prime-f32-gemm": ("FiniteField::try_simd_gemm_classical", "core-gemm-hook"),
    "medium-prime-gemm-panel": ("FiniteField::try_simd_gemm_classical", "core-gemm-hook"),
    "medium-prime-f64-gemm": ("FiniteField::try_simd_gemm_classical", "core-gemm-hook"),
}

# The invoker's dated decisions, which `make-inventory.py` reads from the file
# beside the documents.
DECISIONS_FILE = "decisions.json"

# Per class: the reviewer's basis for the decided form.
BASIS = {
    "dense-shape-product":
        "The kernels' assertions compare with a wrapping product; `checked_mul` in the safe "
        "entry closes the gap once per call. The calling dispatchers size their operands "
        "from the same wrapping products, and no production caller can hold an `unsafe` "
        "block.",
    "ple-panel":
        "The shape check is one product per panel and the entry scan one pass over the "
        "window, both ahead of the elimination; the two production callers meet both "
        "conditions by construction.",
    "sparse-row-index":
        "The confirmation records the per-row form of the check as a regression on both "
        "interfaces, which contradicts the family's non-regression criterion for that "
        "form. The one production caller forwards column indices from a safe public hook "
        "and cannot hold an `unsafe` block, so the row loop stands in the kernel crate "
        "behind one check per product.",
    "packed-lengths":
        "The check is a handful of comparisons per call. No tracked Rust file calls the "
        "F_5, F_7, F_3 mul and F_3 neg pointers. The F_3 add and sub pointers are "
        "called once per Gray step with four words, which puts the check beside one "
        "vector iteration; the crate publishes the unchecked form of those "
        "kernels as `unsafe fn` items.",
}

# Where the check of the sparse row class stands, and the claims.
GRAIN_CLASS = "sparse-row-index"
GRAIN_POSITION = (
    "The safe entry of each lane checks the column indices once per product, ahead of its "
    "row loop: one pass over the whole index array and one division. The row loop stands "
    "in the kernel crate behind that check and holds the `unsafe` call of the row kernel, "
    "whose Safety section states the column bound. `gf2-core` calls the entry once per "
    "product.",
    ["small-spmm-wrapper", "medium-spmm-wrapper", "entry-check-row-scan", "core-spmm-row-loop",
     "spmm-kernel-safety", "core-spmm-small-call", "core-spmm-medium-call"],
)

# The grains at which the column bound can be established. `sites` names what
# holds a grain in the tree: `wrappers` the kernel crate's entry points of the
# class, `entry-calls` the production call sites of their pointers, `hook`
# every tracked line that defines or calls the forwarding hook. `measured` is
# whether the family's cells price the grain.
GRAINS = [
    {
        "id": "per-product",
        "title": "unchecked row kernel behind one checked entry per product",
        "state": "in the tree",
        "design": "The row kernel is an `unsafe fn` of the kernel crate with the column "
                  "bound under Safety. One safe entry per lane takes the CSR arrays and the "
                  "dense operand, checks the largest index of the whole index array once "
                  "against `b.len()` and `n`, and holds the row loop with its `// SAFETY:` "
                  "comment. `fp_try_spmm` converts the operands and the whole product "
                  "between the field type and the lane type around one call.",
        "check": "once per product, ahead of the row loop",
        "constraint": "none",
        "sites": ["wrappers", "entry-calls"],
        "measured": False,
        "claims": ["small-spmm-wrapper", "medium-spmm-wrapper", "core-spmm-row-loop",
                   "spmm-kernel-safety", "core-denies-unsafe"],
    },
    {
        "id": "per-row",
        "title": "check in a safe row entry",
        "state": "in the tree the family measured",
        "design": "A safe row entry runs the column check on every call: one pass over the "
                  "row's indices, one subtraction and one division. The row loop stands in "
                  "`gf2-core`, which calls the entry once per non-empty sparse row.",
        "check": "once per sparse row, inside the row loop of `gf2-core`",
        "constraint": "The confirmation records this form as a regression on both lanes.",
        "sites": [],
        "measured": True,
        "claims": ["measured-row-check", "measured-row-division", "measured-row-loop"],
    },
    {
        "id": "construction",
        "title": "bound held by the sparse matrix type",
        "state": "not built",
        "design": "Every constructor of `SparseFieldMatrix` yields column indices below its "
                  "column count (the constructor table), no method takes the matrix by "
                  "`&mut self`, the index field is private, the raw parts leave as shared "
                  "borrows, and `matmat` asserts the column count against the rows of the "
                  "dense operand before the hook. A product that starts at `matmat` "
                  "therefore meets the bound, and the per-product check never fires for it.",
        "check": "at construction; none per product",
        "constraint": "The bound does not reach the kernel as a type: the hook is a safe "
                      "public trait method over plain slices, and the type's documentation "
                      "does not state the bound. Relying on it in place of the per-product "
                      "check takes a hook that receives the matrix type or is unreachable "
                      "from outside the crate, the bound stated as an invariant of the "
                      "type, and an `unsafe` call of the row kernel in `gf2-core`, which "
                      "denies `unsafe_code`.",
        "sites": ["hook", "entry-calls"],
        "measured": False,
        "claims": ["core-sparse-private-fields", "core-sparse-raw-parts",
                   "core-spmm-matmat-assert", "core-spmm-hook-call", "core-spmm-hook",
                   "core-sparse-type-doc", "core-denies-unsafe"],
    },
]

# The family that prices the per-product grain against the unchecked row loop.
GRAIN_FAMILY_ADDENDUM = "addendum-v4-spmm-check-pilot.json"
# The confirmation addendum the family launcher's `freeze` action writes, and
# the record `make-spmm-freeze-refusal.py` writes when the action refuses.
GRAIN_FAMILY_CONFIRMATION = "addendum-v4-spmm-check-confirmation.json"
GRAIN_FAMILY_REFUSAL = "confirmation-freeze-refusal-spmm-check.json"
# That family's result tables, written beside its ledger by `make-spmm-tables.py`.
GRAIN_FAMILY_TABLES = "spmm-check-tables.md"

# P-20's tail-support floor, in expected bootstrap draws per tail, and the
# claims of the two rules the admissibility arithmetic applies.
TAIL_DRAWS = 20
ADMISSIBILITY_CLAIMS = ["ledger-attempt-alpha", "p20-tail-support"]

# The forwarding hook of the sparse row class, and the file and type whose
# constructors `SPARSE_CONSTRUCTORS` covers.
GRAIN_HOOK = "try_simd_spmm"
SPARSE_TYPE = (CORE, "src/field/sparse_matrix.rs", "SparseFieldMatrix")
# Every function that builds the type: (function, struct literals it holds,
# why each stored column index is below the column count, claim).
SPARSE_CONSTRUCTORS = [
    ("zeros", 1, "stores no index", "core-sparse-zeros"),
    ("from_triplets", 1, "asserts each index against `cols`", "core-sparse-triplet-assert"),
    ("from_dense", 1, "takes each index from a loop over `cols`", "core-sparse-dense-columns"),
    ("identity", 1, "takes the indices from `0..n`", "core-sparse-identity-columns"),
    ("matmul", 2, "copies indices of its right operand, whose column count it takes",
     "core-sparse-matmul-columns"),
    ("rref", 2, "merges rows of the matrix itself", "core-sparse-rref-columns"),
    ("to_csr", 1, "takes each index from a loop over `cols`", "core-sparse-csr-columns"),
]

# The pilot cells of the entry-check cost family, by the interface whose
# checked form each measures; the frozen pilot addendum holds the cells.
PILOT_ADDENDUM = "addendum-v4-entry-check-pilot.json"
# cell: (interface, grain of the check, the checked form the cell measures)
PILOT_CELLS = {
    "spmm-small-n16": ("small-prime-spmm-row", "per row",
                       "one column scan per sparse row, against one 16-column block per "
                       "non-zero"),
    "spmm-small-n1024": ("small-prime-spmm-row", "per row",
                         "one column scan per sparse row, amortized over 64 blocks per "
                         "non-zero"),
    "spmm-medium-n16": ("medium-prime-spmm-row", "per row", "the same scan on the u16 lane"),
    "spmm-medium-n1024": ("medium-prime-spmm-row", "per row",
                          "the same scan on the u16 lane, amortized"),
    "permanent-f3-n14": ("bipedal-binary", "per kernel call",
                         "one length check per kernel call, and the consumer makes one call "
                         "per Gray step"),
    "ple-small-n128": ("small-prime-ple-panel", "per panel",
                       "one shape check and one scan of the window entries per panel"),
    "ple-medium-n128": ("medium-prime-ple-panel", "per panel",
                        "the same panel check on the u16 lane"),
    "gemm-small-n32": ("small-prime-gemm-row-panel", "per row",
                       "one checked shape product per row of the product, which the "
                       "byte-lane route sends through the row kernel"),
    "gemm-medium-n32": ("medium-prime-gemm-panel", "per whole product",
                        "three checked shape products per product call, which the u16 "
                        "panel route makes once"),
}
# The cells the confirmation retains: one per check grain of each cost class.
RETAINED_CELLS = ["spmm-small-n16", "spmm-medium-n16", "permanent-f3-n14", "ple-small-n128",
                  "gemm-small-n32", "gemm-medium-n32"]
# The confirmation addendum pins the pilot receipt as its resolution evidence.
CONFIRMATION_ADDENDUM = "addendum-v4-entry-check-confirmation.json"
# The family's result tables, written beside its ledger by `make-tables.py`.
TABLES = "tables.md"

# Text that would name one of the inventory's consumers in a measurement
# family of the dispatch route inventory (jit:63bad95d).
FAMILY_RECORD = "route-inventory.json"
CONSUMER_TOKENS = ["Fp<", "gfp::", "fp_small", "fp_medium", "SparseFieldMatrix", "::ple",
                   "permanent", "bipedal", "pam_sq", "demap"]
