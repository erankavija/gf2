use std::process::ExitCode;

fn main() -> ExitCode {
    match ldpc_batch_tuned_arm::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gf2-batch-tuned-arm: {error}");
            ExitCode::FAILURE
        }
    }
}
