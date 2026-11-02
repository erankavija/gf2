fn main() -> std::process::ExitCode {
    match aff3ct_ldpc_gaps::tuned::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gf2-batch-tuned-arm: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
