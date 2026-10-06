//! Deterministic untimed semantic oracle for the gf2 logical-buffer routes.
//!
//! It prints one `PASS <case>: <n> checks` line per case and emits no timing
//! sample, so it cannot serve as a pilot.

fn main() {
    match logical_buffer_harness::oracle::run() {
        Ok(report) => {
            for case in report {
                println!("{case}");
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
