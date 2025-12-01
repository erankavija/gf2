//! BitVec Basics - Essential operations tutorial
//!
//! This example demonstrates the fundamental BitVec operations:
//! - Construction and initialization
//! - Element access (get, set, push, pop)
//! - Bitwise operations (AND, OR, XOR, NOT)
//! - Searching and counting
//! - Shifts and rotations
//!
//! Run with: `cargo run --example bitvec_basics`

use gf2_core::BitVec;

fn main() {
    println!("=== BitVec Basics Tutorial ===\n");

    // ========================================
    // 1. Construction
    // ========================================
    println!("1. Construction");
    println!("   ----------------");

    let empty = BitVec::new();
    println!("   Empty BitVec: {} bits", empty.len());

    let zeros = BitVec::zeros(8);
    println!("   8 zero bits: {:08b}", zeros.to_bytes_le()[0]);

    let ones = BitVec::ones(8);
    println!("   8 one bits: {:08b}", ones.to_bytes_le()[0]);

    let from_bytes = BitVec::from_bytes_le(&[0b10101010]);
    println!(
        "   From bytes [0b10101010]: {:08b}\n",
        from_bytes.to_bytes_le()[0]
    );

    // ========================================
    // 2. Element Access
    // ========================================
    println!("2. Element Access");
    println!("   ----------------");

    let mut bv = BitVec::zeros(8);
    bv.set(0, true); // Set bit 0
    bv.set(3, true); // Set bit 3
    bv.set(7, true); // Set bit 7

    println!("   After setting bits 0, 3, 7:");
    print!("   ");
    for i in 0..8 {
        print!("{}", if bv.get(i) { '1' } else { '0' });
    }
    println!(" = {:08b}", bv.to_bytes_le()[0]);

    // Push and pop
    bv.push_bit(true);
    bv.push_bit(false);
    println!("   After push(true), push(false): {} bits", bv.len());

    let popped = bv.pop_bit();
    println!("   Popped: {:?}, now {} bits\n", popped, bv.len());

    // ========================================
    // 3. Bitwise Operations
    // ========================================
    println!("3. Bitwise Operations");
    println!("   ----------------");

    let mut a = BitVec::from_bytes_le(&[0b11001100]);
    let b = BitVec::from_bytes_le(&[0b10101010]);

    println!("   a = {:08b}", a.to_bytes_le()[0]);
    println!("   b = {:08b}", b.to_bytes_le()[0]);

    let mut result = a.clone();
    result.bit_and_into(&b);
    println!("   a AND b = {:08b}", result.to_bytes_le()[0]);

    let mut result = a.clone();
    result.bit_or_into(&b);
    println!("   a OR  b = {:08b}", result.to_bytes_le()[0]);

    let mut result = a.clone();
    result.bit_xor_into(&b);
    println!("   a XOR b = {:08b}", result.to_bytes_le()[0]);

    a.not_into();
    println!("   NOT a   = {:08b}\n", a.to_bytes_le()[0]);

    // ========================================
    // 4. Counting and Parity
    // ========================================
    println!("4. Counting and Parity");
    println!("   ----------------");

    let bv = BitVec::from_bytes_le(&[0b10101010]);
    println!("   BitVec: {:08b}", bv.to_bytes_le()[0]);
    println!("   count_ones() = {}", bv.count_ones());
    println!(
        "   parity() = {} (1 = odd, 0 = even)\n",
        if bv.parity() { 1 } else { 0 }
    );

    // ========================================
    // 5. Searching
    // ========================================
    println!("5. Searching");
    println!("   ----------------");

    let bv = BitVec::from_bytes_le(&[0b00010010]);
    println!("   BitVec: {:08b}", bv.to_bytes_le()[0]);
    println!("   find_first_one() = {:?}", bv.find_first_one());
    println!("   find_first_set() = {:?}", bv.find_first_set());

    let bv = BitVec::from_bytes_le(&[0b11101101]);
    println!("   BitVec: {:08b}", bv.to_bytes_le()[0]);
    println!("   find_first_zero() = {:?}", bv.find_first_zero());
    println!("   find_last_set() = {:?}\n", bv.find_last_set());

    // ========================================
    // 6. Shifts
    // ========================================
    println!("6. Shifts");
    println!("   ----------------");

    let mut bv = BitVec::from_bytes_le(&[0b00001111]);
    println!("   Original:      {:08b}", bv.to_bytes_le()[0]);

    bv.shift_left(2);
    println!("   shift_left(2): {:08b}", bv.to_bytes_le()[0]);

    bv.shift_right(1);
    println!("   shift_right(1): {:08b}\n", bv.to_bytes_le()[0]);

    // ========================================
    // 7. Conversion
    // ========================================
    println!("7. Conversion");
    println!("   ----------------");

    let bytes = vec![0xFF, 0x00, 0xAA];
    let bv = BitVec::from_bytes_le(&bytes);
    println!("   From bytes: {:?}", bytes);
    println!("   BitVec length: {} bits", bv.len());
    println!("   count_ones: {}", bv.count_ones());

    let back = bv.to_bytes_le();
    println!("   Back to bytes: {:?}\n", back);

    // ========================================
    // 8. Practical Example: Parity Check
    // ========================================
    println!("8. Practical Example: Even Parity Check");
    println!("   ----------------");

    let message = BitVec::from_bytes_le(&[0b10110010]);
    println!("   Message: {:08b}", message.to_bytes_le()[0]);
    println!(
        "   Parity bit needed: {}",
        if message.parity() { 1 } else { 0 }
    );

    // Add parity bit
    let mut with_parity = message.clone();
    with_parity.push_bit(message.parity());
    println!(
        "   With parity bit: {} bits, parity = {}\n",
        with_parity.len(),
        if with_parity.parity() { "odd" } else { "even" }
    );

    println!("=== End of BitVec Basics Tutorial ===");
}
