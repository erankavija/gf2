//! Protocol-v4 arm of the profile families (jit:63bad95d).
//!
//! The plan's environment names the build level this executable must be and,
//! on a profile level, the measured owner it installs before its first
//! selector read. A timed request measures the frozen window protocol on warm
//! operands; the validation position checks one call against a portable
//! reference and reports no window.

use profile_build_arms::{
    check_executable, compiled_simd_boundary, install_owner, parse_build, profiled, Case, Workload,
    BUILD_VAR, OWNER_SHA256_VAR, OWNER_VAR,
};
use std::hint::black_box;
use std::io;
use std::time::{Duration, Instant};
use tuning_campaign_support::arm::{ArmRequest, ArmResult, ARM_RESULT_SCHEMA};
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::protocol::{CacheState, SHARED_SETTINGS};
use tuning_campaign_support::receipt::{ConversionCosts, WindowRecord};
use tuning_campaign_support::timing::execution_windows_fixed_or_calibrated;
use tuning_campaign_support::transport;

fn main() {
    if let Err(error) = run() {
        eprintln!("profile-arm: {error}");
        std::process::exit(2);
    }
}

fn variable(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|error| format!("{name} is unavailable: {error}"))
}

fn accept(request: &ArmRequest) -> Result<Case, String> {
    if request.workers_declared != 1 || request.decoder.is_some() {
        return Err("every profile cell is a one-worker kernel or consumer cell".into());
    }
    if request.cache_state != CacheState::Warm || request.cold_calls.is_some() {
        return Err("every profile cell is a warm, calibrated cell".into());
    }
    let frozen = (
        SHARED_SETTINGS.windows_per_execution,
        SHARED_SETTINGS.window_target_ms,
    );
    let observed = (request.windows, request.window_target_ms);
    if observed != frozen && observed != (0, 0) {
        return Err(format!(
            "cell {} requests {} windows of {} ms rather than the frozen {} of {} ms",
            request.cell_id, observed.0, observed.1, frozen.0, frozen.1
        ));
    }
    serde_json::from_value(request.case.clone())
        .map_err(|error| format!("case does not decode: {error}"))
}

fn run() -> Result<(), String> {
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: ArmRequest =
        transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock())?;
    let case = accept(&request)?;
    let build = parse_build(&variable(BUILD_VAR)?)?;
    check_executable(build)?;
    let profile = if profiled(build) {
        install_owner(&variable(OWNER_VAR)?, &variable(OWNER_SHA256_VAR)?)?
    } else {
        if std::env::var_os(OWNER_VAR).is_some() {
            return Err(format!(
                "{build:?} installs no profile, yet {OWNER_VAR} is set"
            ));
        }
        "none".to_owned()
    };

    let setup = Instant::now();
    let mut workload = Workload::build(&case)?;
    let setup_ns = u64::try_from(setup.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let boundary =
        compiled_simd_boundary().map_or_else(|| "none".to_owned(), |words| words.to_string());
    let selected_path = format!(
        "{}/build={}/bit-backend-simd-from={boundary}w/profile={profile}",
        workload.observed_route(),
        variable(BUILD_VAR)?
    );

    let windows = if request.windows == 0 {
        workload.verify()?;
        Vec::new()
    } else {
        let mut sink = workload.call();
        let mut body = |_bank: usize| sink ^= workload.call();
        let samples = execution_windows_fixed_or_calibrated(
            0,
            u64::from(request.windows),
            Duration::from_millis(u64::from(request.window_target_ms)),
            None,
            &mut body,
            |_| Ok(()),
        )
        .map_err(|error| format!("timing failed: {error}"))?;
        black_box(sink);
        samples
            .iter()
            .map(|sample| WindowRecord {
                calls: sample.calls,
                elapsed_ns: sample.elapsed_ns,
            })
            .collect()
    };

    let result = ArmResult {
        schema: ARM_RESULT_SCHEMA.to_owned(),
        windows,
        cache_state_applied: request.cache_state,
        workers_observed: 1,
        cpus_observed: CpuAffinity::observe()
            .map(|affinity| affinity.cpus().to_vec())
            .unwrap_or_default(),
        selected_path: Some(selected_path),
        conversion: Some(ConversionCosts {
            setup_ns,
            pack_ns: 0,
            unpack_ns: 0,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        }),
        quality: None,
        calibrated: Some(true),
    };
    transport::write_result_line(io::stdout().lock(), &result).map_err(|error| error.to_string())
}
