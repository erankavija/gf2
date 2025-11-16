use gf2_core::BitVec;

fn main() {
    // Test with bytes that span multiple words
    let bytes: Vec<u8> = (0..256).map(|i| i as u8).collect();
    let bv = BitVec::from_bytes_le(&bytes);

    println!("Testing {} bytes ({} bits)", bytes.len(), bv.len());
    println!("Total ones: {}\n", bv.count_ones());

    // Build expected positions
    let mut expected = Vec::new();
    for i in 0..bv.len() {
        if bv.get(i) {
            expected.push(i);
        }
    }

    println!("Expected {} set bit positions", expected.len());

    // Test select for each position
    for (k, &exp_pos) in expected.iter().enumerate() {
        let actual = bv.select(k);
        if actual != Some(exp_pos) {
            println!(
                "FAIL: select({}) = {:?}, expected Some({})",
                k, actual, exp_pos
            );

            // Debug: show rank at this position
            println!("  rank({}) = {}", exp_pos, bv.rank(exp_pos));
            break;
        } else if k < 10 || k % 100 == 0 {
            println!("OK: select({}) = {}", k, exp_pos);
        }
    }
}
