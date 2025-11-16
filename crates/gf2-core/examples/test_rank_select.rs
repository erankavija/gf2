use gf2_core::BitVec;

fn main() {
    // Simple test case
    let bv = BitVec::from_bytes_le(&[0b00101101]); // bits: 10110100
    println!("BitVec len: {}, count_ones: {}", bv.len(), bv.count_ones());

    for i in 0..bv.len() {
        println!("bit[{}] = {}", i, bv.get(i));
    }

    println!("\nRank tests:");
    for i in 0..bv.len() {
        println!("rank({}) = {}", i, bv.rank(i));
    }

    println!("\nSelect tests:");
    for k in 0..=bv.count_ones() {
        println!("select({}) = {:?}", k, bv.select(k));
    }
}
