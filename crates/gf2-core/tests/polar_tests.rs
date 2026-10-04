//! Tests for polar transform operations.

use gf2_core::BitVec;
use proptest::prelude::*;

#[test]
fn test_bit_reversed_empty() {
    let bv = BitVec::new();
    let reversed = bv.bit_reversed(0);
    assert_eq!(reversed.len(), 0);
}

#[test]
fn test_bit_reversed_single_bit() {
    let mut bv = BitVec::new();
    bv.push_bit(true);
    let reversed = bv.bit_reversed(1);
    assert_eq!(reversed.len(), 1);
    assert!(reversed.get(0));
}

#[test]
fn test_bit_reversed_two_bits() {
    // For n=2, bit-reversal permutation is identity (no swap)
    let mut bv = BitVec::new();
    bv.push_bit(true);
    bv.push_bit(false);
    let reversed = bv.bit_reversed(2);
    assert!(reversed.get(0));
    assert!(!reversed.get(1));
}

#[test]
fn test_bit_reversed_four_bits() {
    // For n=4, bit-reversal permutation is [0, 2, 1, 3]
    let mut bv = BitVec::new();
    bv.push_bit(false);
    bv.push_bit(true);
    bv.push_bit(false);
    bv.push_bit(true);
    let reversed = bv.bit_reversed(4);
    assert!(!reversed.get(0));
    assert!(!reversed.get(1));
    assert!(reversed.get(2));
    assert!(reversed.get(3));
}

#[test]
fn test_bit_reversed_eight_bits() {
    // For n=8, permutation maps i -> reverse(i, 3 bits)
    // 0b11001010 = bit pattern [0,1,0,1,0,0,1,1] (LSB first)
    // Result = 0b11011000 = 216
    let bv = BitVec::from_bytes_le(&[0b11001010]);
    let reversed = bv.bit_reversed(8);
    assert_eq!(reversed.to_bytes_le()[0], 216);
}

#[test]
fn test_bit_reverse_into_involution() {
    let mut bv = BitVec::from_bytes_le(&[0b11001010, 0b01010101]);
    let original = bv.clone();
    bv.bit_reverse_into(16);
    bv.bit_reverse_into(16);
    assert_eq!(bv, original);
}

#[test]
fn test_bit_reverse_into_matches_functional() {
    let bv = BitVec::from_bytes_le(&[0b11001010, 0b01010101]);
    let mut bv_mut = bv.clone();
    let bv_func = bv.bit_reversed(16);
    bv_mut.bit_reverse_into(16);
    assert_eq!(bv_mut, bv_func);
}

#[test]
#[should_panic(expected = "n_bits must be a power of 2")]
fn test_bit_reversed_non_power_of_2() {
    let bv = BitVec::from_bytes_le(&[0xFF]);
    bv.bit_reversed(3);
}

#[test]
fn test_polar_transform_n1() {
    // G_1 = [1], identity transform
    let mut bv = BitVec::new();
    bv.push_bit(true);
    let transformed = bv.polar_transform(1);
    assert!(transformed.get(0));
}

#[test]
fn test_polar_transform_n2() {
    // G_2 = [1 0; 1 1]
    // Butterfly: (a, b) -> (a, a XOR b)
    let mut bv = BitVec::new();
    bv.push_bit(false);
    bv.push_bit(true);
    let transformed = bv.polar_transform(2);
    assert!(!transformed.get(0));
    assert!(transformed.get(1));

    let mut bv2 = BitVec::new();
    bv2.push_bit(true);
    bv2.push_bit(false);
    let transformed2 = bv2.polar_transform(2);
    assert!(transformed2.get(0));
    assert!(transformed2.get(1));
}

#[test]
fn test_polar_transform_n4() {
    // G_4 = G_2 ⊗ G_2
    let mut bv = BitVec::new();
    bv.push_bit(true);
    bv.push_bit(false);
    bv.push_bit(false);
    bv.push_bit(false);
    let transformed = bv.polar_transform(4);
    assert!(transformed.get(0));
}

#[test]
fn test_polar_transform_inverse_roundtrip() {
    let mut bv = BitVec::new();
    for i in 0..8 {
        bv.push_bit(i % 2 == 0);
    }
    let original = bv.clone();
    let transformed = bv.polar_transform(8);
    let recovered = transformed.polar_transform_inverse(8);
    assert_eq!(recovered, original);
}

#[test]
fn test_polar_transform_into_matches_functional() {
    let mut bv = BitVec::new();
    for i in 0..8 {
        bv.push_bit(i % 3 == 0);
    }
    let mut bv_mut = bv.clone();
    let bv_func = bv.polar_transform(8);
    bv_mut.polar_transform_into(8);
    assert_eq!(bv_mut, bv_func);
}

#[test]
fn test_polar_transform_inverse_into_matches_functional() {
    let mut bv = BitVec::new();
    for i in 0..8 {
        bv.push_bit(i % 3 == 0);
    }
    let mut bv_mut = bv.clone();
    let bv_func = bv.polar_transform_inverse(8);
    bv_mut.polar_transform_inverse_into(8);
    assert_eq!(bv_mut, bv_func);
}

#[test]
#[should_panic(expected = "n must be a power of 2")]
fn test_polar_transform_non_power_of_2() {
    let bv = BitVec::from_bytes_le(&[0xFF]);
    bv.polar_transform(3);
}

#[test]
fn test_polar_transform_large() {
    let mut bv = BitVec::zeros(1024);
    bv.set(0, true);
    bv.set(512, true);
    let transformed = bv.polar_transform(1024);
    let recovered = transformed.polar_transform_inverse(1024);
    assert_eq!(recovered, bv);
}

proptest! {
    #[test]
    fn prop_bit_reverse_involution(bytes in prop::collection::vec(any::<u8>(), 1..=16)) {
        let n_bits = (bytes.len() * 8).next_power_of_two();
        let mut bv = BitVec::from_bytes_le(&bytes);
        bv.resize(n_bits, false);
        let original = bv.clone();

        let reversed = bv.bit_reversed(n_bits);
        let double_reversed = reversed.bit_reversed(n_bits);

        assert_eq!(double_reversed, original);
    }

    #[test]
    fn prop_bit_reverse_into_matches_functional(bytes in prop::collection::vec(any::<u8>(), 1..=16)) {
        let n_bits = (bytes.len() * 8).next_power_of_two();
        let mut bv = BitVec::from_bytes_le(&bytes);
        bv.resize(n_bits, false);

        let mut bv_mut = bv.clone();
        let bv_func = bv.bit_reversed(n_bits);
        bv_mut.bit_reverse_into(n_bits);

        assert_eq!(bv_mut, bv_func);
    }

    #[test]
    fn prop_polar_transform_linearity(
        bytes1 in prop::collection::vec(any::<u8>(), 1..=16),
        bytes2 in prop::collection::vec(any::<u8>(), 1..=16)
    ) {
        let n_bits = bytes1.len().max(bytes2.len()) * 8;
        let n = n_bits.next_power_of_two();

        let mut bv1 = BitVec::from_bytes_le(&bytes1);
        let mut bv2 = BitVec::from_bytes_le(&bytes2);
        bv1.resize(n, false);
        bv2.resize(n, false);

        let mut sum = bv1.clone();
        sum.bit_xor_into(&bv2);
        let fht_sum = sum.polar_transform(n);

        let fht1 = bv1.polar_transform(n);
        let fht2 = bv2.polar_transform(n);
        let mut fht_xor = fht1.clone();
        fht_xor.bit_xor_into(&fht2);

        assert_eq!(fht_sum, fht_xor);
    }

    #[test]
    fn prop_polar_transform_inverse_roundtrip(bytes in prop::collection::vec(any::<u8>(), 1..=32)) {
        let n_bits = bytes.len() * 8;
        let n = n_bits.next_power_of_two();

        let mut bv = BitVec::from_bytes_le(&bytes);
        bv.resize(n, false);
        let original = bv.clone();

        let transformed = bv.polar_transform(n);
        let recovered = transformed.polar_transform_inverse(n);

        assert_eq!(recovered, original);
    }

    #[test]
    fn prop_polar_transform_into_matches_functional(bytes in prop::collection::vec(any::<u8>(), 1..=16)) {
        let n_bits = bytes.len() * 8;
        let n = n_bits.next_power_of_two();

        let mut bv = BitVec::from_bytes_le(&bytes);
        bv.resize(n, false);

        let mut bv_mut = bv.clone();
        let bv_func = bv.polar_transform(n);
        bv_mut.polar_transform_into(n);

        assert_eq!(bv_mut, bv_func);
    }
}

#[test]
fn test_polar_transform_matrix_equivalence_n2() {
    use gf2_core::matrix::BitMatrix;

    // G_2 = [1 0; 1 1]
    let mut g2 = BitMatrix::zeros(2, 2);
    g2.set(0, 0, true);
    g2.set(1, 0, true);
    g2.set(1, 1, true);

    for input_val in 0..4u8 {
        let mut input = BitVec::new();
        input.push_bit(input_val & 1 != 0);
        input.push_bit(input_val & 2 != 0);

        let mut matrix_result = BitVec::zeros(2);
        for row in 0..2 {
            let mut bit = false;
            for col in 0..2 {
                if g2.get(row, col) && input.get(col) {
                    bit = !bit;
                }
            }
            matrix_result.set(row, bit);
        }

        let transform_result = input.polar_transform(2);

        assert_eq!(transform_result, matrix_result);
    }
}

#[test]
fn test_polar_transform_matrix_equivalence_n4() {
    use gf2_core::matrix::BitMatrix;

    // G_4 = G_2 ⊗ G_2 = [G_2  0  ]
    //                    [G_2  G_2]
    let mut g4 = BitMatrix::zeros(4, 4);
    g4.set(0, 0, true);
    g4.set(1, 0, true);
    g4.set(1, 1, true);
    g4.set(2, 0, true);
    g4.set(3, 0, true);
    g4.set(3, 1, true);
    g4.set(2, 2, true);
    g4.set(3, 2, true);
    g4.set(3, 3, true);

    for input_val in [0b0001, 0b0101, 0b1010, 0b1111].iter() {
        let mut input = BitVec::new();
        for i in 0..4 {
            input.push_bit(input_val & (1 << i) != 0);
        }

        let mut matrix_result = BitVec::zeros(4);
        for row in 0..4 {
            let mut bit = false;
            for col in 0..4 {
                if g4.get(row, col) && input.get(col) {
                    bit = !bit;
                }
            }
            matrix_result.set(row, bit);
        }

        let transform_result = input.polar_transform(4);

        assert_eq!(transform_result, matrix_result);
    }
}
