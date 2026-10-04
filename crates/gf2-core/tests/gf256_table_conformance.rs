//! Shared conformance suite for the GF(2^8) cached-product-table axpy lane:
//! through the public [`FieldVec::axpy`], the cached-table lane for
//! `Gf2mElement` and single-word `Gf2mWide<1, Cfg>` must agree with the scalar
//! element loop reached through [`force_scalar_gf256_table`].

#![cfg(feature = "test-support")]

use std::panic::AssertUnwindSafe;
use std::sync::Mutex;

use gf2_core::field::FieldVec;
use gf2_core::gf2m::{
    force_scalar_gf256_table, gf256_table_builds, last_gf256_table_lane, Gf2mField, Gf2mField_,
    Gf2mWide, Gf2mWideConfig, GF256_SCALAR_LANE, GF256_TABLE_LANE,
};

/// Serialises `force_scalar_gf256_table` toggle-and-observe sections: the
/// override is process-wide, so a test asserting which lane
/// [`last_gf256_table_lane`] reports could otherwise observe another thread's
/// toggle.
static DISPATCH_LANE_MUTEX: Mutex<()> = Mutex::new(());

/// The reduction polynomial `x^8 + x^4 + x^3 + x + 1`, low eight bits.
const POLY_11B: u8 = 0x1b;

/// The reduction polynomial `x^8 + x^4 + x^3 + x^2 + 1`, low eight bits.
const POLY_11D: u8 = 0x1d;

const SWEEP_POLYNOMIALS: [u8; 2] = [POLY_11B, POLY_11D];

/// GF(2^8) under `x^8 + x^4 + x^3 + x + 1` (0x11B).
struct Gf256Poly11bCfg;
impl Gf2mWideConfig<1> for Gf256Poly11bCfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [POLY_11B as u64];
}

/// GF(2^8) under `x^8 + x^4 + x^3 + x^2 + 1` (0x11D).
struct Gf256Poly11dCfg;
impl Gf2mWideConfig<1> for Gf256Poly11dCfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [POLY_11D as u64];
}

/// GF(2^16), a single-word configuration the dispatch declines on degree.
struct Gf2m16Cfg;
impl Gf2mWideConfig<1> for Gf2m16Cfg {
    const M: usize = 16;
    const MODULUS: [u64; 1] = [0x002d];
}

/// GF(2^256) over the modulus whose low word is 0x425, a multi-word
/// configuration the dispatch declines on width before any arithmetic runs.
struct Gf2m256Cfg;
impl Gf2mWideConfig<4> for Gf2m256Cfg {
    const M: usize = 256;
    const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
}

fn element_field(reduction_low: u8) -> Gf2mField {
    Gf2mField::new(8, 0x100 | u64::from(reduction_low))
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn seeded_bytes(seed: u64, count: usize) -> Vec<u8> {
    let mut state = seed;
    (0..count).map(|_| splitmix64(&mut state) as u8).collect()
}

/// Lengths the consumer sweep covers: 0, 1, 63, 64, 65 and an odd length above
/// the kernel's unrolling, each shifted by every offset 0 through 7.
fn conformance_lengths() -> Vec<usize> {
    let mut lengths = Vec::new();
    for base in [0usize, 1, 63, 64, 65, 137] {
        for offset in 0..=7usize {
            lengths.push(base + offset);
        }
    }
    lengths.sort_unstable();
    lengths.dedup();
    lengths
}

/// Runs `y += a * x` over the runtime-context element and returns the
/// destination's values together with the lane the call took.
fn element_axpy(
    field: &Gf2mField,
    coefficient: u8,
    destination: &[u8],
    source: &[u8],
) -> (Vec<u8>, &'static str) {
    let mut y = FieldVec::from(
        destination
            .iter()
            .map(|&b| field.element(u64::from(b)))
            .collect::<Vec<_>>(),
    );
    let x = FieldVec::from(
        source
            .iter()
            .map(|&b| field.element(u64::from(b)))
            .collect::<Vec<_>>(),
    );
    y.axpy(&field.element(u64::from(coefficient)), &x);
    let lane = last_gf256_table_lane();
    ((0..y.len()).map(|i| y[i].value() as u8).collect(), lane)
}

/// Runs `y += a * x` over a single-word wide value and returns the
/// destination's values together with the lane the call took.
fn wide_axpy<Cfg: Gf2mWideConfig<1>>(
    coefficient: u8,
    destination: &[u8],
    source: &[u8],
) -> (Vec<u8>, &'static str) {
    let mut y = FieldVec::from(
        destination
            .iter()
            .map(|&b| Gf2mWide::<1, Cfg>::from_u64(u64::from(b)))
            .collect::<Vec<_>>(),
    );
    let x = FieldVec::from(
        source
            .iter()
            .map(|&b| Gf2mWide::<1, Cfg>::from_u64(u64::from(b)))
            .collect::<Vec<_>>(),
    );
    y.axpy(&Gf2mWide::<1, Cfg>::from_u64(u64::from(coefficient)), &x);
    let lane = last_gf256_table_lane();
    ((0..y.len()).map(|i| y[i].words()[0] as u8).collect(), lane)
}

#[test]
fn element_axpy_agrees_with_the_scalar_lane_for_every_coefficient() {
    let lengths = conformance_lengths();
    let guard = DISPATCH_LANE_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    for reduction_low in SWEEP_POLYNOMIALS {
        let field = element_field(reduction_low);
        for &length in &lengths {
            let destination = seeded_bytes(0x5eed_0001 ^ u64::from(reduction_low), length);
            let source = seeded_bytes(0x5eed_0002 ^ u64::from(reduction_low), length);
            for coefficient in 0..=255u8 {
                let restore = force_scalar_gf256_table(true);
                let (scalar, scalar_lane) =
                    element_axpy(&field, coefficient, &destination, &source);
                force_scalar_gf256_table(restore);
                let (table, table_lane) = element_axpy(&field, coefficient, &destination, &source);

                assert_eq!(scalar_lane, GF256_SCALAR_LANE);
                assert_eq!(table_lane, GF256_TABLE_LANE);
                assert_eq!(
                    table, scalar,
                    "poly {reduction_low:#04x}, coefficient {coefficient}, length {length}"
                );
            }
        }
    }

    drop(guard);
}

#[test]
fn wide_axpy_agrees_with_the_scalar_lane_for_every_coefficient() {
    let lengths = conformance_lengths();
    let guard = DISPATCH_LANE_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    for &length in &lengths {
        let destination = seeded_bytes(0x5eed_0003, length);
        let source = seeded_bytes(0x5eed_0004, length);
        for coefficient in 0..=255u8 {
            let restore = force_scalar_gf256_table(true);
            let (scalar_b, scalar_b_lane) =
                wide_axpy::<Gf256Poly11bCfg>(coefficient, &destination, &source);
            let (scalar_d, scalar_d_lane) =
                wide_axpy::<Gf256Poly11dCfg>(coefficient, &destination, &source);
            force_scalar_gf256_table(restore);
            let (table_b, table_b_lane) =
                wide_axpy::<Gf256Poly11bCfg>(coefficient, &destination, &source);
            let (table_d, table_d_lane) =
                wide_axpy::<Gf256Poly11dCfg>(coefficient, &destination, &source);

            assert_eq!(scalar_b_lane, GF256_SCALAR_LANE);
            assert_eq!(scalar_d_lane, GF256_SCALAR_LANE);
            assert_eq!(table_b_lane, GF256_TABLE_LANE);
            assert_eq!(table_d_lane, GF256_TABLE_LANE);
            assert_eq!(
                table_b, scalar_b,
                "0x11B, coefficient {coefficient}, length {length}"
            );
            assert_eq!(
                table_d, scalar_d,
                "0x11D, coefficient {coefficient}, length {length}"
            );
        }
    }

    drop(guard);
}

#[test]
fn the_two_representations_agree_with_each_other_over_both_polynomials() {
    let lengths = conformance_lengths();
    for &length in &lengths {
        let destination = seeded_bytes(0x5eed_0005, length);
        let source = seeded_bytes(0x5eed_0006, length);
        for coefficient in [0u8, 1, 2, 0x53, 0x8d, 0xff] {
            let (element_b, _) =
                element_axpy(&element_field(POLY_11B), coefficient, &destination, &source);
            let (wide_b, _) = wide_axpy::<Gf256Poly11bCfg>(coefficient, &destination, &source);
            assert_eq!(element_b, wide_b, "0x11B, coefficient {coefficient}");

            let (element_d, _) =
                element_axpy(&element_field(POLY_11D), coefficient, &destination, &source);
            let (wide_d, _) = wide_axpy::<Gf256Poly11dCfg>(coefficient, &destination, &source);
            assert_eq!(element_d, wide_d, "0x11D, coefficient {coefficient}");
        }
    }
}

#[test]
fn a_zero_coefficient_leaves_the_destination_unchanged() {
    let destination = seeded_bytes(0x5eed_0007, 137);
    let source = seeded_bytes(0x5eed_0008, 137);
    for reduction_low in SWEEP_POLYNOMIALS {
        let (result, lane) = element_axpy(&element_field(reduction_low), 0, &destination, &source);
        assert_eq!(lane, GF256_TABLE_LANE);
        assert_eq!(result, destination);
    }
    let (wide_b, _) = wide_axpy::<Gf256Poly11bCfg>(0, &destination, &source);
    let (wide_d, _) = wide_axpy::<Gf256Poly11dCfg>(0, &destination, &source);
    assert_eq!(wide_b, destination);
    assert_eq!(wide_d, destination);
}

#[test]
fn a_one_coefficient_reduces_the_operation_to_xor() {
    let destination = seeded_bytes(0x5eed_0009, 137);
    let source = seeded_bytes(0x5eed_000a, 137);
    let expected: Vec<u8> = destination
        .iter()
        .zip(source.iter())
        .map(|(&y, &x)| y ^ x)
        .collect();

    for reduction_low in SWEEP_POLYNOMIALS {
        let (result, lane) = element_axpy(&element_field(reduction_low), 1, &destination, &source);
        assert_eq!(lane, GF256_TABLE_LANE);
        assert_eq!(result, expected);
    }
    let (wide_b, _) = wide_axpy::<Gf256Poly11bCfg>(1, &destination, &source);
    let (wide_d, _) = wide_axpy::<Gf256Poly11dCfg>(1, &destination, &source);
    assert_eq!(wide_b, expected);
    assert_eq!(wide_d, expected);
}

#[test]
fn every_irreducible_degree_8_modulus_agrees_across_lanes() {
    let destination = seeded_bytes(0x5eed_000b, 65);
    let source = seeded_bytes(0x5eed_000c, 65);
    let guard = DISPATCH_LANE_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let mut irreducible = 0usize;
    for low in 0..=255u8 {
        let field = element_field(low);
        if !field.is_irreducible_rabin() {
            continue;
        }
        irreducible += 1;
        // `with_tables` builds log/antilog tables from the powers of x, which
        // enumerate the multiplicative group only for a primitive modulus.
        let fields = if field.verify_primitive() {
            vec![element_field(low), element_field(low).with_tables()]
        } else {
            vec![element_field(low)]
        };
        for field in fields {
            for coefficient in [0u8, 1, 2, 0x53, 0xff] {
                let restore = force_scalar_gf256_table(true);
                let (scalar, _) = element_axpy(&field, coefficient, &destination, &source);
                force_scalar_gf256_table(restore);
                let (table, lane) = element_axpy(&field, coefficient, &destination, &source);
                assert_eq!(lane, GF256_TABLE_LANE);
                assert_eq!(
                    table, scalar,
                    "modulus {low:#04x}, coefficient {coefficient}"
                );
            }
        }
    }

    drop(guard);
    // Monic irreducible polynomials of degree 8 over GF(2): Mobius inversion
    // of 2^n = sum over d | n of d * N(d) gives (2^8 - 2^4) / 8.
    assert_eq!(irreducible, (256 - 16) / 8);
}

#[test]
fn the_table_lane_accepts_both_single_word_gf256_representations() {
    let destination = seeded_bytes(0x5eed_000d, 65);
    let source = seeded_bytes(0x5eed_000e, 65);
    let (_, element_lane) = element_axpy(&element_field(POLY_11D), 0x53, &destination, &source);
    let (_, wide_lane) = wide_axpy::<Gf256Poly11dCfg>(0x53, &destination, &source);
    assert_eq!(element_lane, GF256_TABLE_LANE);
    assert_eq!(wide_lane, GF256_TABLE_LANE);
}

#[test]
fn another_degree_declines_and_keeps_its_result() {
    for (m, poly) in [(4usize, 0b1_0011u64), (16, 0b1_0000_0000_0010_1101)] {
        let field = Gf2mField::new(m, poly);
        let values: Vec<u64> = (0..65).map(|i| (i * 7 + 1) % (1 << m)).collect();
        let sources: Vec<u64> = (0..65).map(|i| (i * 11 + 3) % (1 << m)).collect();
        let coefficient = field.element(3);

        let mut y = FieldVec::from(values.iter().map(|&v| field.element(v)).collect::<Vec<_>>());
        let x = FieldVec::from(
            sources
                .iter()
                .map(|&v| field.element(v))
                .collect::<Vec<_>>(),
        );
        let expected: Vec<_> = values
            .iter()
            .zip(sources.iter())
            .map(|(&v, &s)| field.element(v) + coefficient.clone() * field.element(s))
            .collect();

        y.axpy(&coefficient, &x);
        assert_eq!(last_gf256_table_lane(), GF256_SCALAR_LANE, "degree {m}");
        for (i, want) in expected.iter().enumerate() {
            assert_eq!(&y[i], want, "degree {m}, index {i}");
        }
    }
}

#[test]
fn another_backing_width_declines_and_keeps_its_result() {
    let field = Gf2mField_::<u128>::new(8, 0x11d);
    let values: Vec<u128> = (0..65u128).map(|i| (i * 7 + 1) & 0xff).collect();
    let sources: Vec<u128> = (0..65u128).map(|i| (i * 11 + 3) & 0xff).collect();
    let coefficient = field.element(0x53);

    let mut y = FieldVec::from(values.iter().map(|&v| field.element(v)).collect::<Vec<_>>());
    let x = FieldVec::from(
        sources
            .iter()
            .map(|&v| field.element(v))
            .collect::<Vec<_>>(),
    );
    let expected: Vec<_> = values
        .iter()
        .zip(sources.iter())
        .map(|(&v, &s)| field.element(v) + coefficient.clone() * field.element(s))
        .collect();

    y.axpy(&coefficient, &x);
    assert_eq!(last_gf256_table_lane(), GF256_SCALAR_LANE);
    for (i, want) in expected.iter().enumerate() {
        assert_eq!(&y[i], want, "index {i}");
    }
}

#[test]
fn a_multi_word_configuration_declines_and_keeps_its_result() {
    let values: Vec<u64> = (0..65).map(|i| i * 7 + 1).collect();
    let sources: Vec<u64> = (0..65).map(|i| i * 11 + 3).collect();
    let coefficient = Gf2mWide::<4, Gf2m256Cfg>::from_u64(0x53);

    let mut y = FieldVec::from(
        values
            .iter()
            .map(|&v| Gf2mWide::<4, Gf2m256Cfg>::from_u64(v))
            .collect::<Vec<_>>(),
    );
    let x = FieldVec::from(
        sources
            .iter()
            .map(|&v| Gf2mWide::<4, Gf2m256Cfg>::from_u64(v))
            .collect::<Vec<_>>(),
    );
    let expected: Vec<_> = values
        .iter()
        .zip(sources.iter())
        .map(|(&v, &s)| {
            Gf2mWide::<4, Gf2m256Cfg>::from_u64(v)
                + coefficient * Gf2mWide::<4, Gf2m256Cfg>::from_u64(s)
        })
        .collect();

    y.axpy(&coefficient, &x);
    assert_eq!(last_gf256_table_lane(), GF256_SCALAR_LANE);
    for (i, want) in expected.iter().enumerate() {
        assert_eq!(&y[i], want, "index {i}");
    }
}

#[test]
fn a_single_word_configuration_of_another_degree_declines_and_keeps_its_result() {
    let values: Vec<u64> = (0..65).map(|i| i * 7 + 1).collect();
    let sources: Vec<u64> = (0..65).map(|i| i * 11 + 3).collect();
    let coefficient = Gf2mWide::<1, Gf2m16Cfg>::from_u64(0x53);

    let mut y = FieldVec::from(
        values
            .iter()
            .map(|&v| Gf2mWide::<1, Gf2m16Cfg>::from_u64(v))
            .collect::<Vec<_>>(),
    );
    let x = FieldVec::from(
        sources
            .iter()
            .map(|&v| Gf2mWide::<1, Gf2m16Cfg>::from_u64(v))
            .collect::<Vec<_>>(),
    );
    let expected: Vec<_> = values
        .iter()
        .zip(sources.iter())
        .map(|(&v, &s)| {
            Gf2mWide::<1, Gf2m16Cfg>::from_u64(v)
                + coefficient * Gf2mWide::<1, Gf2m16Cfg>::from_u64(s)
        })
        .collect();

    y.axpy(&coefficient, &x);
    assert_eq!(last_gf256_table_lane(), GF256_SCALAR_LANE);
    for (i, want) in expected.iter().enumerate() {
        assert_eq!(&y[i], want, "index {i}");
    }
}

#[test]
fn a_mixed_field_context_declines_and_panics_exactly_as_the_scalar_path_does() {
    /// Runs an axpy whose source elements come from a second field handle and
    /// returns the panic message together with the lane the call selected.
    fn mixed_context_axpy() -> (String, &'static str) {
        let home = Gf2mField::gf256();
        let other = Gf2mField::gf256();
        let mut y = FieldVec::from((0..8u64).map(|v| home.element(v)).collect::<Vec<_>>());
        let x = FieldVec::from((0..8u64).map(|v| other.element(v)).collect::<Vec<_>>());
        let coefficient = home.element(0x53);

        let panic = std::panic::catch_unwind(AssertUnwindSafe(|| {
            y.axpy(&coefficient, &x);
        }))
        .expect_err("a mixed field context reaches the scalar path's assertion");
        let message = panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_string()))
            .unwrap_or_else(|| String::from("<non-string panic payload>"));
        (message, last_gf256_table_lane())
    }

    let guard = DISPATCH_LANE_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    let restore = force_scalar_gf256_table(true);
    let (without_lane, without_lane_witness) = mixed_context_axpy();
    force_scalar_gf256_table(restore);
    let (with_lane, with_lane_witness) = mixed_context_axpy();

    std::panic::set_hook(previous_hook);
    drop(guard);

    assert!(
        with_lane.contains("Cannot multiply elements from different fields"),
        "got: {with_lane}"
    );
    assert_eq!(with_lane, without_lane);
    assert_eq!(with_lane_witness, GF256_SCALAR_LANE);
    assert_eq!(without_lane_witness, GF256_SCALAR_LANE);
}

#[test]
fn the_force_switch_holds_every_caller_on_the_scalar_lane() {
    let destination = seeded_bytes(0x5eed_000f, 137);
    let source = seeded_bytes(0x5eed_0010, 137);

    let guard = DISPATCH_LANE_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let (table_element, table_element_lane) =
        element_axpy(&element_field(POLY_11D), 0x53, &destination, &source);
    let (table_wide, table_wide_lane) = wide_axpy::<Gf256Poly11dCfg>(0x53, &destination, &source);

    let restore = force_scalar_gf256_table(true);
    let (scalar_element, scalar_element_lane) =
        element_axpy(&element_field(POLY_11D), 0x53, &destination, &source);
    let (scalar_wide, scalar_wide_lane) = wide_axpy::<Gf256Poly11dCfg>(0x53, &destination, &source);
    let was_forced = force_scalar_gf256_table(restore);

    let (released_element, released_element_lane) =
        element_axpy(&element_field(POLY_11D), 0x53, &destination, &source);

    drop(guard);

    assert_eq!(table_element_lane, GF256_TABLE_LANE);
    assert_eq!(table_wide_lane, GF256_TABLE_LANE);
    assert_eq!(scalar_element_lane, GF256_SCALAR_LANE);
    assert_eq!(scalar_wide_lane, GF256_SCALAR_LANE);
    assert!(was_forced, "the switch reports the setting it replaced");
    assert_eq!(released_element_lane, GF256_TABLE_LANE);

    assert_eq!(scalar_element, table_element);
    assert_eq!(scalar_wide, table_wide);
    assert_eq!(released_element, table_element);
}

#[test]
fn repeated_calls_on_one_polynomial_build_one_table() {
    let destination = seeded_bytes(0x5eed_0011, 64);
    let source = seeded_bytes(0x5eed_0012, 64);
    let field = element_field(POLY_11D);

    // Touch the key once so the count below cannot include its first build.
    let _ = element_axpy(&field, 1, &destination, &source);
    let builds = gf256_table_builds();
    for coefficient in 0..=255u8 {
        let _ = element_axpy(&field, coefficient, &destination, &source);
        let _ = wide_axpy::<Gf256Poly11dCfg>(coefficient, &destination, &source);
    }
    assert_eq!(gf256_table_builds(), builds);
}

#[test]
fn the_call_leaves_its_source_unchanged() {
    let destination = seeded_bytes(0x5eed_0013, 137);
    let source = seeded_bytes(0x5eed_0014, 137);
    let field = element_field(POLY_11D);

    let mut y = FieldVec::from(
        destination
            .iter()
            .map(|&b| field.element(u64::from(b)))
            .collect::<Vec<_>>(),
    );
    let x = FieldVec::from(
        source
            .iter()
            .map(|&b| field.element(u64::from(b)))
            .collect::<Vec<_>>(),
    );
    y.axpy(&field.element(0x53), &x);

    assert_eq!(last_gf256_table_lane(), GF256_TABLE_LANE);
    let read_back: Vec<u8> = (0..x.len()).map(|i| x[i].value() as u8).collect();
    assert_eq!(read_back, source);
}
