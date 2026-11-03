fn main() -> std::process::ExitCode {
    match aff3ct_ldpc_gaps::tuned::run_candidate() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gf2-batch-qc-candidate-arm: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
