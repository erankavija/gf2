//! Cross-validates the Rust bench seed scheme (`benches/common/seed.rs`)
//! against a line-by-line port of `benchmarks/reference/seed_helpers.h`: both
//! derive the same seed for the same `(master, tag, op, size, regime)` cell.

#[path = "../benches/common/seed.rs"]
mod seed;

// `wrapping_*` matches C's unsigned-overflow semantics.

#[allow(non_snake_case)]
fn c_ref_splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[allow(non_snake_case)]
fn c_ref_derive_seed(master: u64, tag: &[u8], op_idx: u64, size_idx: u64, regime_idx: u64) -> u64 {
    let mut s = master;
    for b in tag {
        s ^= u64::from(*b);
        let _ = c_ref_splitmix64(&mut s);
    }
    s ^= op_idx;
    let _ = c_ref_splitmix64(&mut s);
    s ^= size_idx;
    let _ = c_ref_splitmix64(&mut s);
    s ^= regime_idx;
    let _ = c_ref_splitmix64(&mut s);
    c_ref_splitmix64(&mut s)
}

#[test]
fn rust_splitmix_matches_c_reference() {
    for &s0 in &[
        0u64,
        1,
        0xDEAD_BEEF,
        0x6F73_AC91_D31E_4A7C,
        u64::MAX,
        0x8000_0000_0000_0000,
        0x0123_4567_89AB_CDEF,
    ] {
        let mut a = s0;
        let mut b = s0;
        for _ in 0..16 {
            assert_eq!(seed::splitmix64(&mut a), c_ref_splitmix64(&mut b));
            assert_eq!(a, b, "internal state must stay synchronised");
        }
    }
}

#[test]
fn rust_derive_matches_c_reference_across_tags_and_indices() {
    let masters = [0u64, 0x6F73_AC91_D31E_4A7C, 0xCAFE_BABE_DEAD_BEEF];
    let tags = [
        "fgemm",
        "fgemm_b",
        "pluq",
        "echelon",
        "invert",
        "solve",
        "charpoly",
        "minpoly",
        "rref",
        "rank",
        "nullspace",
        "spmv",
        "spmv_vec",
        "det",
        "fgemm_rect",
        "fgemm_rect_b",
        "solve_rhs",
    ];
    for &m in &masters {
        for tag in &tags {
            for op in 0u64..6 {
                for si in 0u64..5 {
                    for ri in 0u64..3 {
                        let r = seed::derive_seed(m, tag, op, si, ri);
                        let c = c_ref_derive_seed(m, tag.as_bytes(), op, si, ri);
                        assert_eq!(
                            r, c,
                            "diverged at master=0x{m:016x} tag={tag} op={op} \
                             size={si} regime={ri}: rust=0x{r:016x} c=0x{c:016x}"
                        );
                    }
                }
            }
        }
    }
}

/// Pinned outputs: a seed-pipeline change that breaks reproducibility against
/// the reference harness fails here.
#[test]
fn pinned_seed_values_at_master_0_and_pinned_master() {
    let r0 = seed::derive_seed(0, "fgemm", 0, 0, 0);
    assert_eq!(r0, 0xa1f5_dbf0_5125_7436);

    // The master seed pinned in `benchmarks/seeds/seed.txt`.
    let pinned_master = 0x6F73_AC91_D31E_4A7C;
    let r1 = seed::derive_seed(pinned_master, "fgemm", 0, 0, 0);
    assert_eq!(r1, 0x47e4_989d_742b_754f);

    let mut st = r1;
    assert_eq!(seed::splitmix64(&mut st), 0x350b_8ce7_e52d_880c);
    assert_eq!(seed::splitmix64(&mut st), 0x00b2_abfd_4b04_5d88);
    assert_eq!(seed::splitmix64(&mut st), 0x9573_9178_dbda_8b98);
    assert_eq!(seed::splitmix64(&mut st), 0x7313_2303_c672_288f);
}
