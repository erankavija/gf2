//! Protocol-v4 campaign arm of the entry-check cost family (jit:bfc2ceab).
//!
//! The runner launches one fresh process per execution, writes the request on
//! stdin and reads one result line. The request and result are
//! `tuning_campaign_support::arm`'s own types.

use entry_check_arm::{prepare, verify_entry_form, Case, Workload};
use std::io;
use std::time::{Duration, Instant};
use tuning_campaign_support::arm::{ArmRequest, ArmResult, ARM_RESULT_SCHEMA};
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::protocol::CacheState;
use tuning_campaign_support::receipt::{ConversionCosts, WindowRecord};
use tuning_campaign_support::timing::execution_windows_fixed_or_calibrated;
use tuning_campaign_support::transport;

fn main() {
    if let Err(error) = run() {
        eprintln!("entry-check-arm: {error}");
        std::process::exit(2);
    }
}

/// Threads of this process, as the kernel reports them.
fn observed_threads() -> Result<u32, String> {
    let status = std::fs::read_to_string("/proc/self/status").map_err(|error| error.to_string())?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("Threads:"))
        .and_then(|count| count.trim().parse().ok())
        .ok_or_else(|| "the process status reports no thread count".to_owned())
}

fn run() -> Result<(), String> {
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: ArmRequest =
        transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock())?;
    if request.decoder.is_some() || request.workers_declared != 1 {
        return Err("every cell of this family is a one-worker cell without a decoder".into());
    }
    if request.cache_state != CacheState::Warm {
        return Err("every cell of this family is warm".into());
    }
    let case: Case = serde_json::from_value(request.case.clone())
        .map_err(|error| format!("case does not decode: {error}"))?;
    let form = verify_entry_form()?;

    let setup = Instant::now();
    let Workload { mut body, observe } = prepare(&case, form)?;
    let setup_ns = u64::try_from(setup.elapsed().as_nanos()).unwrap_or(u64::MAX);

    // One untimed pass warms the working set; in the validation position it is
    // the whole dispatch.
    body();
    let samples = if request.role.is_timed() {
        execution_windows_fixed_or_calibrated(
            0,
            u64::from(request.windows),
            Duration::from_millis(u64::from(request.window_target_ms)),
            request.cold_calls,
            &mut |_| body(),
            |_| Ok(()),
        )
        .map_err(|error| format!("timing failed: {error}"))?
    } else {
        Vec::new()
    };
    drop(body);

    let result = ArmResult {
        schema: ARM_RESULT_SCHEMA.to_owned(),
        windows: samples
            .iter()
            .map(|sample| WindowRecord {
                calls: sample.calls,
                elapsed_ns: sample.elapsed_ns,
            })
            .collect(),
        cache_state_applied: request.cache_state,
        workers_observed: observed_threads()?,
        cpus_observed: CpuAffinity::observe()
            .map(|affinity| affinity.cpus().to_vec())
            .unwrap_or_default(),
        selected_path: Some(observe()),
        // The consumers take operands in their own representation and the
        // window covers the whole call, so no conversion stands outside it.
        conversion: Some(ConversionCosts {
            setup_ns,
            pack_ns: 0,
            unpack_ns: 0,
            batch_fill_ns: 0,
            dispatch_ns: 0,
        }),
        quality: None,
        calibrated: Some(request.role.is_timed() && request.cold_calls.is_none()),
    };
    transport::write_result_line(io::stdout().lock(), &result).map_err(|error| error.to_string())
}
