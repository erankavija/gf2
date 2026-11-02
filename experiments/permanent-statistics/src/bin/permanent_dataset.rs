//! Finalizes or verifies schema-3 campaign evidence.
use permanent_statistics::campaign::dataset;
use std::{path::Path, process::ExitCode};
fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 || !matches!(args[0].as_str(), "finalize" | "verify") {
        eprintln!("usage: permanent_dataset finalize|verify DIR");
        return ExitCode::from(2);
    }
    let result = match args[0].as_str() {
        "finalize" => dataset::finalize(Path::new(&args[1])),
        _ => dataset::verify(Path::new(&args[1])),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}
