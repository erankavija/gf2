//! LLR arithmetic for soft-decision decoding: exact and min-sum box-plus, binary and multi-operand.

use gf2_coding::llr::Llr;

fn main() {
    println!("=== LLR Operations for Soft-Decision Decoding ===\n");

    println!("1. Basic LLR Operations");
    println!("{}", "-".repeat(50));

    let llr_positive = Llr::new(3.5);
    let llr_negative = Llr::new(-2.0);
    let llr_weak = Llr::new(0.5);

    println!("Strong confidence in bit 0: {:.2}", llr_positive.value());
    println!("  Hard decision: {}", llr_positive.hard_decision());
    println!("  Magnitude (confidence): {:.2}", llr_positive.magnitude());

    println!(
        "\nModerate confidence in bit 1: {:.2}",
        llr_negative.value()
    );
    println!("  Hard decision: {}", llr_negative.hard_decision());
    println!("  Magnitude (confidence): {:.2}", llr_negative.magnitude());

    println!("\nWeak confidence in bit 0: {:.2}", llr_weak.value());
    println!("  Hard decision: {}", llr_weak.hard_decision());
    println!("  Magnitude (confidence): {:.2}\n", llr_weak.magnitude());

    println!("2. Binary Box-Plus (XOR in LLR domain)");
    println!("{}", "-".repeat(50));

    let a = Llr::new(4.0);
    let b = Llr::new(3.0);
    let xor_exact = a.boxplus(b);
    let xor_minsum = a.boxplus_minsum(b);

    println!("Input LLRs: {:.2}, {:.2}", a.value(), b.value());
    println!("  Both bits likely 0 → XOR likely 0");
    println!("  Exact box-plus: {:.4}", xor_exact.value());
    println!("  Min-sum approx: {:.4}", xor_minsum.value());
    println!(
        "  Approximation error: {:.4}\n",
        (xor_exact.value() - xor_minsum.value()).abs()
    );

    let c = Llr::new(4.0);
    let d = Llr::new(-3.0);
    let xor_mixed = c.boxplus(d);
    let xor_mixed_minsum = c.boxplus_minsum(d);

    println!("Input LLRs: {:.2}, {:.2}", c.value(), d.value());
    println!("  One bit likely 0, one likely 1 → XOR likely 1");
    println!("  Exact box-plus: {:.4}", xor_mixed.value());
    println!("  Min-sum approx: {:.4}\n", xor_mixed_minsum.value());

    println!("3. Multi-Operand Box-Plus (LDPC Check Nodes)");
    println!("{}", "-".repeat(50));

    let check_node_llrs = vec![Llr::new(4.0), Llr::new(3.5), Llr::new(5.0), Llr::new(2.5)];

    println!("Check node with 4 variable nodes:");
    for (i, llr) in check_node_llrs.iter().enumerate() {
        println!("  Variable node {}: {:.2}", i, llr.value());
    }

    let exact = Llr::boxplus_n(&check_node_llrs);
    let minsum = Llr::boxplus_minsum_n(&check_node_llrs);
    let normalized = Llr::boxplus_normalized_minsum_n(&check_node_llrs, 0.875);
    let offset = Llr::boxplus_offset_minsum_n(&check_node_llrs, 0.5);

    println!("\nCheck node output:");
    println!("  Exact (tanh-based):     {:.4}", exact.value());
    println!("  Min-sum:                {:.4}", minsum.value());
    println!("  Normalized (α=0.875):   {:.4}", normalized.value());
    println!("  Offset (β=0.5):         {:.4}", offset.value());

    println!("\nApproximation errors:");
    println!(
        "  Min-sum error:          {:.4}",
        (exact.value() - minsum.value()).abs()
    );
    println!(
        "  Normalized error:       {:.4}",
        (exact.value() - normalized.value()).abs()
    );
    println!(
        "  Offset error:           {:.4}\n",
        (exact.value() - offset.value()).abs()
    );

    println!("4. Mixed Signs (Parity Check Constraint)");
    println!("{}", "-".repeat(50));

    let mixed_llrs = vec![Llr::new(4.0), Llr::new(-3.0), Llr::new(5.0), Llr::new(-2.0)];

    println!("Variable nodes with mixed beliefs:");
    for (i, llr) in mixed_llrs.iter().enumerate() {
        let bit = if llr.hard_decision() { 1 } else { 0 };
        println!("  Node {}: {:.2} → hard decision: {}", i, llr.value(), bit);
    }

    let parity = mixed_llrs
        .iter()
        .map(|llr| llr.hard_decision())
        .fold(false, |acc, b| acc ^ b);
    println!(
        "\nHard decision XOR (parity): {}",
        if parity { 1 } else { 0 }
    );

    let exact_mixed = Llr::boxplus_n(&mixed_llrs);
    let minsum_mixed = Llr::boxplus_minsum_n(&mixed_llrs);

    println!("Soft decision (check-to-variable message):");
    println!(
        "  Exact:   {:.4} → hard decision: {}",
        exact_mixed.value(),
        exact_mixed.hard_decision()
    );
    println!(
        "  Min-sum: {:.4} → hard decision: {}\n",
        minsum_mixed.value(),
        minsum_mixed.hard_decision()
    );

    println!("5. Numerical Stability");
    println!("{}", "-".repeat(50));

    let large_a = Llr::new(100.0);
    let large_b = Llr::new(100.0);

    println!(
        "Very large LLRs: {:.2}, {:.2}",
        large_a.value(),
        large_b.value()
    );
    println!(
        "  Regular box-plus: {:.4}",
        large_a.boxplus(large_b).value()
    );
    println!(
        "  Safe box-plus:    {:.4}",
        large_a.safe_boxplus(large_b).value()
    );
    println!("  Is finite: {}", large_a.safe_boxplus(large_b).is_finite());

    let saturated_a = large_a.saturate(10.0);
    let saturated_b = large_b.saturate(10.0);
    println!("\nAfter saturation to ±10.0:");
    println!(
        "  Saturated values: {:.2}, {:.2}",
        saturated_a.value(),
        saturated_b.value()
    );
    println!(
        "  Box-plus result: {:.4}\n",
        saturated_a.boxplus(saturated_b).value()
    );

    println!("6. Performance Considerations");
    println!("{}", "-".repeat(50));
    println!("For LDPC decoding with thousands of iterations:");
    println!("  • Exact box-plus: Most accurate but slowest (tanh/atanh)");
    println!("  • Min-sum: Fastest, ~0.5 dB SNR loss");
    println!("  • Normalized min-sum: Good tradeoff, ~0.2 dB loss");
    println!("  • Offset min-sum: Similar to normalized, different tuning");
    println!("\nRecommended: Start with normalized min-sum (α=0.875)");
}
