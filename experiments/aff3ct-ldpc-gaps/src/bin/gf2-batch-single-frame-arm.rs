fn main() -> std::process::ExitCode {
    match aff3ct_ldpc_gaps::batch::run(
        "gf2-batch-single-frame",
        aff3ct_ldpc_gaps::batch::BatchRoute::SingleFrame,
    ) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gf2-batch-single-frame-arm: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
