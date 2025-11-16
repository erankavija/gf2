use gf2_core::BitVec;

fn main() {
    let bytes: Vec<u8> = (0..256).map(|i| i as u8).collect();
    let bv = BitVec::from_bytes_le(&bytes);

    println!("Testing position 509:");
    println!("  bit value: {}", bv.get(509));
    println!("  rank(509): {}", bv.rank(509));
    println!("  select(191): {:?}", bv.select(191));

    // What about nearby positions?
    for k in 189..=193 {
        println!("  select({}) = {:?}", k, bv.select(k));
    }
}
