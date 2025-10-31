//! Property-based tests for BitVec using proptest.

use gf2::BitVec;
use proptest::prelude::*;

/// A simple reference implementation using Vec<u8> to store individual bits.
/// Used for property testing against BitVec.
#[derive(Debug, Clone)]
struct ReferenceBits {
    bits: Vec<u8>,
}

impl ReferenceBits {
    #[allow(dead_code)]
    fn new() -> Self {
        Self { bits: Vec::new() }
    }

    fn from_bytes_le(bytes: &[u8]) -> Self {
        let mut bits = Vec::new();
        for &byte in bytes {
            for i in 0..8 {
                bits.push((byte >> i) & 1);
            }
        }
        Self { bits }
    }

    fn to_bytes_le(&self) -> Vec<u8> {
        let num_bytes = self.bits.len().div_ceil(8);
        let mut bytes = vec![0u8; num_bytes];
        for (i, &bit) in self.bits.iter().enumerate() {
            if bit != 0 {
                bytes[i / 8] |= 1 << (i % 8);
            }
        }
        bytes
    }

    fn len(&self) -> usize {
        self.bits.len()
    }

    fn get(&self, idx: usize) -> bool {
        self.bits[idx] != 0
    }

    fn set(&mut self, idx: usize, bit: bool) {
        self.bits[idx] = if bit { 1 } else { 0 };
    }

    fn bit_and(&mut self, other: &Self) {
        for (a, &b) in self.bits.iter_mut().zip(other.bits.iter()) {
            *a &= b;
        }
    }

    fn bit_or(&mut self, other: &Self) {
        for (a, &b) in self.bits.iter_mut().zip(other.bits.iter()) {
            *a |= b;
        }
    }

    fn bit_xor(&mut self, other: &Self) {
        for (a, &b) in self.bits.iter_mut().zip(other.bits.iter()) {
            *a ^= b;
        }
    }

    fn not(&mut self) {
        for bit in self.bits.iter_mut() {
            *bit = if *bit == 0 { 1 } else { 0 };
        }
    }

    fn shift_left(&mut self, k: usize) {
        if k >= self.bits.len() {
            self.bits.fill(0);
        } else if k > 0 {
            self.bits.rotate_right(k);
            for i in 0..k {
                self.bits[i] = 0;
            }
        }
    }

    fn shift_right(&mut self, k: usize) {
        if k >= self.bits.len() {
            self.bits.fill(0);
        } else if k > 0 {
            self.bits.rotate_left(k);
            let len = self.bits.len();
            for i in 0..k {
                self.bits[len - 1 - i] = 0;
            }
        }
    }

    fn count_ones(&self) -> u64 {
        self.bits.iter().filter(|&&b| b != 0).count() as u64
    }

    fn find_first_set(&self) -> Option<usize> {
        self.bits.iter().position(|&b| b != 0)
    }

    fn find_last_set(&self) -> Option<usize> {
        self.bits.iter().rposition(|&b| b != 0)
    }

    fn resize(&mut self, new_len: usize, fill_bit: bool) {
        self.bits.resize(new_len, if fill_bit { 1 } else { 0 });
    }
}

proptest! {
    #[test]
    fn prop_from_bytes_to_bytes_roundtrip(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let bv = BitVec::from_bytes_le(&bytes);
        let result = bv.to_bytes_le();
        prop_assert_eq!(result, bytes);
    }

    #[test]
    fn prop_get_set_equivalence(
        bytes in prop::collection::vec(any::<u8>(), 1..50),
        idx in 0usize..400,
        bit in any::<bool>()
    ) {
        let mut bv = BitVec::from_bytes_le(&bytes);
        let mut rb = ReferenceBits::from_bytes_le(&bytes);

        if idx < bv.len() {
            prop_assert_eq!(bv.get(idx), rb.get(idx));

            bv.set(idx, bit);
            rb.set(idx, bit);

            prop_assert_eq!(bv.get(idx), rb.get(idx));
        }
    }

    #[test]
    fn prop_bit_and_equivalence(
        bytes1 in prop::collection::vec(any::<u8>(), 1..50),
        bytes2 in prop::collection::vec(any::<u8>(), 1..50)
    ) {
        // Make sure lengths match
        let len = bytes1.len().min(bytes2.len());
        let b1 = &bytes1[..len];
        let b2 = &bytes2[..len];

        let mut bv1 = BitVec::from_bytes_le(b1);
        let bv2 = BitVec::from_bytes_le(b2);
        let mut rb1 = ReferenceBits::from_bytes_le(b1);
        let rb2 = ReferenceBits::from_bytes_le(b2);

        bv1.bit_and_into(&bv2);
        rb1.bit_and(&rb2);

        prop_assert_eq!(bv1.to_bytes_le(), rb1.to_bytes_le());
    }

    #[test]
    fn prop_bit_or_equivalence(
        bytes1 in prop::collection::vec(any::<u8>(), 1..50),
        bytes2 in prop::collection::vec(any::<u8>(), 1..50)
    ) {
        let len = bytes1.len().min(bytes2.len());
        let b1 = &bytes1[..len];
        let b2 = &bytes2[..len];

        let mut bv1 = BitVec::from_bytes_le(b1);
        let bv2 = BitVec::from_bytes_le(b2);
        let mut rb1 = ReferenceBits::from_bytes_le(b1);
        let rb2 = ReferenceBits::from_bytes_le(b2);

        bv1.bit_or_into(&bv2);
        rb1.bit_or(&rb2);

        prop_assert_eq!(bv1.to_bytes_le(), rb1.to_bytes_le());
    }

    #[test]
    fn prop_bit_xor_equivalence(
        bytes1 in prop::collection::vec(any::<u8>(), 1..50),
        bytes2 in prop::collection::vec(any::<u8>(), 1..50)
    ) {
        let len = bytes1.len().min(bytes2.len());
        let b1 = &bytes1[..len];
        let b2 = &bytes2[..len];

        let mut bv1 = BitVec::from_bytes_le(b1);
        let bv2 = BitVec::from_bytes_le(b2);
        let mut rb1 = ReferenceBits::from_bytes_le(b1);
        let rb2 = ReferenceBits::from_bytes_le(b2);

        bv1.bit_xor_into(&bv2);
        rb1.bit_xor(&rb2);

        prop_assert_eq!(bv1.to_bytes_le(), rb1.to_bytes_le());
    }

    #[test]
    fn prop_not_equivalence(bytes in prop::collection::vec(any::<u8>(), 1..50)) {
        let mut bv = BitVec::from_bytes_le(&bytes);
        let mut rb = ReferenceBits::from_bytes_le(&bytes);

        bv.not_into();
        rb.not();

        prop_assert_eq!(bv.to_bytes_le(), rb.to_bytes_le());
    }

    #[test]
    fn prop_shift_left_equivalence(
        bytes in prop::collection::vec(any::<u8>(), 1..50),
        k in 0usize..400
    ) {
        let mut bv = BitVec::from_bytes_le(&bytes);
        let mut rb = ReferenceBits::from_bytes_le(&bytes);

        bv.shift_left(k);
        rb.shift_left(k);

        prop_assert_eq!(bv.to_bytes_le(), rb.to_bytes_le());
    }

    #[test]
    fn prop_shift_right_equivalence(
        bytes in prop::collection::vec(any::<u8>(), 1..50),
        k in 0usize..400
    ) {
        let mut bv = BitVec::from_bytes_le(&bytes);
        let mut rb = ReferenceBits::from_bytes_le(&bytes);

        bv.shift_right(k);
        rb.shift_right(k);

        prop_assert_eq!(bv.to_bytes_le(), rb.to_bytes_le());
    }

    #[test]
    fn prop_count_ones_equivalence(bytes in prop::collection::vec(any::<u8>(), 0..50)) {
        let bv = BitVec::from_bytes_le(&bytes);
        let rb = ReferenceBits::from_bytes_le(&bytes);

        prop_assert_eq!(bv.count_ones(), rb.count_ones());
    }

    #[test]
    fn prop_find_first_set_equivalence(bytes in prop::collection::vec(any::<u8>(), 0..50)) {
        let bv = BitVec::from_bytes_le(&bytes);
        let rb = ReferenceBits::from_bytes_le(&bytes);

        prop_assert_eq!(bv.find_first_set(), rb.find_first_set());
    }

    #[test]
    fn prop_find_last_set_equivalence(bytes in prop::collection::vec(any::<u8>(), 0..50)) {
        let bv = BitVec::from_bytes_le(&bytes);
        let rb = ReferenceBits::from_bytes_le(&bytes);

        prop_assert_eq!(bv.find_last_set(), rb.find_last_set());
    }

    #[test]
    fn prop_resize_equivalence(
        bytes in prop::collection::vec(any::<u8>(), 1..50),
        new_len in 0usize..400,
        fill_bit in any::<bool>()
    ) {
        let mut bv = BitVec::from_bytes_le(&bytes);
        let mut rb = ReferenceBits::from_bytes_le(&bytes);

        bv.resize(new_len, fill_bit);
        rb.resize(new_len, fill_bit);

        prop_assert_eq!(bv.len(), rb.len());
        prop_assert_eq!(bv.to_bytes_le(), rb.to_bytes_le());
    }
}
