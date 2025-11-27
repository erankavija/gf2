//! Debug first few parity bits computation
//!
//! Manually compute the first few parity bits to understand the encoding process
//! and identify where divergence occurs.

mod test_vectors;

use gf2_coding::ldpc::LdpcCode;
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use test_vectors::{test_vectors_available, test_vectors_path, TestVectorSet};

#[test]
#[ignore]
fn debug_first_parity_bits() {
    if !test_vectors_available() {
        eprintln!("Test vectors not available");
        return;
    }

    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35")
        .expect("Failed to load test vectors");

    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let tp05 = vectors.tp05.as_ref().expect("TP05 not found");
    let tp06 = vectors.tp06.as_ref().expect("TP06 not found");

    let input_block = &tp05.frame(0)[0];
    let expected_output = &tp06.frame(0)[0];

    let k = code.k();
    let n = code.n();
    let m = n - k;

    println!("\n=== Manual Parity Bit Computation ===");
    println!("Code: n={}, k={}, m={}", n, k, m);
    println!("DVB-T2 3/5 Normal frame\n");

    println!("H matrix structure: [A | B_dual_diagonal]");
    println!("  A: {}×{} (information to parity)", m, k);
    println!("  B: {}×{} (dual-diagonal parity structure)\n", m, m);

    // Manually compute first few parity bits using the encoding equation:
    // For systematic LDPC: c = [u | p] where H·c = 0
    // This means: A·u + B·p = 0, so B·p = A·u
    // For dual-diagonal B, we can solve iteratively
    
    println!("=== Using Syndrome Method to Compute Parity Bits ===\n");
    
    // Use the code's syndrome method to check individual bits
    // For systematic LDPC, we build codeword incrementally: [info_bits | parity_bits]
    
    // Start with just information bits, zero parity
    let mut test_codeword = input_block.data.clone();
    test_codeword.resize(n, false);
    
    // Compute full syndrome with all parity bits = 0
    let syndrome_zero_parity = code.syndrome(&test_codeword);
    
    println!("Syndrome with zero parity bits (first 10 checks):");
    for i in 0..10 {
        println!("  s[{}] = {}", i, syndrome_zero_parity.get(i) as u8);
    }
    println!();
    
    // Now check what parity bits make syndrome zero
    // According to dual-diagonal:
    //   Row 0: info_contribution + p0 = 0  =>  p0 = info_contribution
    //   Row i>0: info_contribution + p[i-1] + p[i] = 0  =>  p[i] = info_contribution + p[i-1]
    
    println!("Expected parity bits from dual-diagonal encoding:");
    let mut computed_parity = BitVec::zeros(m);
    
    // p0 should equal syndrome[0] to cancel it
    computed_parity.set(0, syndrome_zero_parity.get(0));
    println!("  p[0] = s[0] = {}", computed_parity.get(0) as u8);
    
    // Subsequent parity bits
    for i in 1..10 {
        let p_prev = computed_parity.get(i - 1);
        let s_i = syndrome_zero_parity.get(i);
        let p_i = s_i ^ p_prev;  // To cancel syndrome: s[i] + p[i-1] + p[i] = 0 => p[i] = s[i] + p[i-1]
        computed_parity.set(i, p_i);
        
        println!("  p[{}] = s[{}] ⊕ p[{}] = {} ⊕ {} = {}", 
            i, i, i-1, s_i as u8, p_prev as u8, p_i as u8);
    }
    
    println!("\n=== Comparison with Expected Codeword ===\n");
    println!("Expected parity bits from test vector:");
    for i in 0..10 {
        let computed = computed_parity.get(i);
        let expected = expected_output.data.get(k + i);
        let status = if computed == expected { "✓" } else { "✗" };
        
        println!("  p[{}]: computed={}, expected={}, {}", 
            i, computed as u8, expected as u8, status);
    }
    
    // Verify that the expected codeword actually satisfies H·c = 0
    println!("\n=== Verifying Test Vector ===\n");
    let expected_syndrome = code.syndrome(&expected_output.data);
    let syndrome_weight = expected_syndrome.count_ones();
    
    println!("Test vector syndrome weight: {}/{}", syndrome_weight, m);
    if syndrome_weight == 0 {
        println!("✓ Test vector is a valid codeword");
    } else {
        println!("✗ WARNING: Test vector FAILS parity check!");
        println!("  First 10 syndrome bits:");
        for i in 0..10 {
            println!("    s[{}] = {}", i, expected_syndrome.get(i) as u8);
        }
    }
    
    // Verify our computed parity also works
    println!("\n=== Verifying Computed Parity ===\n");
    let mut our_codeword = input_block.data.clone();
    our_codeword.resize(n, false);
    
    // Set all parity bits according to dual-diagonal
    for i in 0..m {
        if i == 0 {
            computed_parity.set(0, syndrome_zero_parity.get(0));
        } else {
            let p_prev = computed_parity.get(i - 1);
            let s_i = syndrome_zero_parity.get(i);
            computed_parity.set(i, s_i ^ p_prev);
        }
        our_codeword.set(k + i, computed_parity.get(i));
    }
    
    let our_syndrome = code.syndrome(&our_codeword);
    let our_syndrome_weight = our_syndrome.count_ones();
    
    println!("Our computed codeword syndrome weight: {}/{}", our_syndrome_weight, m);
    if our_syndrome_weight == 0 {
        println!("✓ Our dual-diagonal encoding produces valid codeword");
    } else {
        println!("✗ Our encoding FAILS parity check!");
        println!("  First 10 syndrome bits:");
        for i in 0..10 {
            println!("    s[{}] = {}", i, our_syndrome.get(i) as u8);
        }
    }
}
