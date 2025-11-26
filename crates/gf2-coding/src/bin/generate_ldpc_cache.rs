//! Generate DVB-T2 LDPC cache files

use gf2_coding::ldpc::encoding::EncodingCache;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let output_dir = Path::new("data/ldpc/dvb_t2");

    if args.len() > 1 {
        match args[1].as_str() {
            "short" => generate_short_frames(output_dir),
            "all" => generate_all(output_dir),
            _ => {
                eprintln!("Usage: {} [short|all]", args[0]);
                eprintln!("  short: Generate 6 Short frames only (~10 seconds)");
                eprintln!("  all:   Generate all 12 frames (~5-10 minutes)");
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("Usage: {} [short|all]", args[0]);
        eprintln!("  short: Generate 6 Short frames only (~10 seconds)");
        eprintln!("  all:   Generate all 12 frames (~5-10 minutes)");
        std::process::exit(1);
    }
}

fn generate_short_frames(output_dir: &Path) {
    use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
    use gf2_coding::CodeRate;

    println!("Generating DVB-T2 SHORT frame caches (6 configs)...");
    println!("Output directory: {}", output_dir.display());
    println!("Each cache saved immediately after preprocessing.\n");

    let cache = EncodingCache::new();
    let rates = [
        CodeRate::Rate1_2,
        CodeRate::Rate3_5,
        CodeRate::Rate2_3,
        CodeRate::Rate3_4,
        CodeRate::Rate4_5,
        CodeRate::Rate5_6,
    ];

    let total_start = std::time::Instant::now();

    for (i, rate) in rates.iter().enumerate() {
        println!("[{}/6] Short {:?}...", i + 1, rate);
        let start = std::time::Instant::now();

        let code = LdpcCode::dvb_t2_short(*rate);
        let _encoder = LdpcEncoder::with_cache(code, &cache);

        let elapsed = start.elapsed();
        println!("  Preprocessed in {:.1}s", elapsed.as_secs_f64());

        // Save immediately
        match cache.save_to_directory(output_dir) {
            Ok(()) => println!("  ✓ Saved to disk"),
            Err(e) => {
                eprintln!("  ✗ Error saving: {}", e);
                std::process::exit(1);
            }
        }
    }

    let total_elapsed = total_start.elapsed();
    println!(
        "\n✓ All Short frames generated in {:.1}s",
        total_elapsed.as_secs_f64()
    );
    println!("✓ Total cache files: {}", cache.stats().entries);
}

fn generate_all(output_dir: &Path) {
    println!("Generating ALL 12 DVB-T2 LDPC cache files...");
    println!("Output directory: {}", output_dir.display());

    // Load existing cache files if any
    let cache = match EncodingCache::from_directory(output_dir) {
        Ok(cache) => {
            let existing = cache.stats().entries;
            if existing > 0 {
                println!(
                    "Found {} existing cache files, will skip those.\n",
                    existing
                );
            }
            cache
        }
        Err(_) => {
            println!("No existing cache found, generating all.\n");
            EncodingCache::new()
        }
    };

    println!("This will take ~5-10 minutes with SIMD (was 2-5 hours before!).\n");

    let start = std::time::Instant::now();

    // Precompute remaining configs
    cache.precompute_dvb_t2();

    // Save all to directory
    match cache.save_to_directory(output_dir) {
        Ok(()) => {
            let elapsed = start.elapsed();
            println!(
                "\n✓ All cache files generated in {:.1}s ({:.1} min)",
                elapsed.as_secs_f64(),
                elapsed.as_secs_f64() / 60.0
            );
            println!("✓ Total cache files: {}", cache.stats().entries);
        }
        Err(e) => {
            eprintln!("✗ Error: {}", e);
            std::process::exit(1);
        }
    }
}
