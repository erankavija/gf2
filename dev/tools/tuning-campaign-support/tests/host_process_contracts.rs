use std::fs::File;
use std::io::ErrorKind;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tuning_campaign_support::campaign::ProcessOutcome;
use tuning_campaign_support::host::{
    inherited_lock, lock_available, resolve_core_arm, CoreArm, CpuAffinity, CpuTopology,
    HostObservation, LogicalCpu,
};
use tuning_campaign_support::process::run_process;
use tuning_campaign_support::scratch::Scratch;

fn unique_path(label: &str) -> Scratch {
    tuning_campaign_support::scratch::scratch(&format!("tuning-campaign-support-{label}"))
}

#[test]
fn topology_and_host_observation_report_runtime_facts() {
    let topology = CpuTopology::observe().unwrap();
    assert!(!topology.cpus.is_empty());
    assert!(topology
        .cpus
        .iter()
        .all(|cpu| cpu.thread_siblings.contains(&cpu.cpu)));
    let cores = topology.physical_cores();
    assert!(!cores.is_empty());
    assert!(cores.windows(2).all(|pair| pair[0][0] < pair[1][0]));

    let observation = HostObservation::observe().unwrap();
    assert!(!observation.cpu_model.is_empty());
    assert!(!observation.cpu_flags.is_empty());
    assert!(!observation.os_kernel.is_empty());
    assert!(!observation.governors.is_empty());
    assert_eq!(observation.affinity, CpuAffinity::observe().unwrap());
}

fn synthetic_topology() -> CpuTopology {
    let cpus = (0..24)
        .map(|cpu| {
            let core_id = cpu % 12;
            let thread_siblings = vec![core_id, core_id + 12];
            let l3_shared = if core_id < 6 {
                (0..6).chain(12..18).collect()
            } else {
                (6..12).chain(18..24).collect()
            };
            LogicalCpu {
                cpu,
                core_id,
                package_id: 0,
                thread_siblings,
                l3_shared,
            }
        })
        .collect();
    CpuTopology {
        cpus,
        smt_active: Some(true),
    }
}

#[test]
fn core_arms_resolve_inside_the_mask_or_carry_a_reason() {
    let topology = synthetic_topology();
    let full = CpuAffinity::try_from((0..24).collect::<Vec<_>>()).unwrap();
    assert_eq!(
        resolve_core_arm(CoreArm::SingleCore, &topology, &full).unwrap(),
        vec![0]
    );
    assert_eq!(
        resolve_core_arm(CoreArm::PhysicalCores6, &topology, &full).unwrap(),
        (0..6).collect::<Vec<_>>()
    );
    assert_eq!(
        resolve_core_arm(CoreArm::PhysicalCores12, &topology, &full).unwrap(),
        (0..12).collect::<Vec<_>>()
    );
    assert_eq!(
        resolve_core_arm(CoreArm::LogicalCpus24, &topology, &full).unwrap(),
        (0..24).collect::<Vec<_>>()
    );

    let second_ccx = CpuAffinity::try_from((6..12).collect::<Vec<_>>()).unwrap();
    assert_eq!(
        resolve_core_arm(CoreArm::SingleCore, &topology, &second_ccx).unwrap(),
        vec![6]
    );
    assert_eq!(
        resolve_core_arm(CoreArm::PhysicalCores6, &topology, &second_ccx).unwrap(),
        (6..12).collect::<Vec<_>>()
    );
    assert!(
        resolve_core_arm(CoreArm::PhysicalCores12, &topology, &second_ccx)
            .unwrap_err()
            .contains("6 physical cores, 12 required")
    );
    assert!(
        resolve_core_arm(CoreArm::LogicalCpus24, &topology, &second_ccx)
            .unwrap_err()
            .contains("6 logical CPUs, 24 required")
    );

    let sparse = CpuAffinity::try_from(vec![3, 15]).unwrap();
    assert_eq!(
        resolve_core_arm(CoreArm::SingleCore, &topology, &sparse).unwrap(),
        vec![3]
    );
}

#[test]
fn affinity_round_trips_serde_and_rejects_unordered_sets() {
    let affinity: CpuAffinity = serde_json::from_str("[0,1,2]").unwrap();
    assert_eq!(affinity.cpus(), &[0, 1, 2]);
    assert!(serde_json::from_str::<CpuAffinity>("[2,1]").is_err());
    assert!(serde_json::from_str::<CpuAffinity>("[]").is_err());
    assert_eq!(CpuAffinity::parse("0-2,5").unwrap().cpus(), &[0, 1, 2, 5]);
}

#[test]
fn lock_observation_requires_a_held_inherited_descriptor() {
    let path = unique_path("lock");
    let lock = File::create(&path).unwrap();
    assert!(lock_available(&path).unwrap());
    lock.lock().unwrap();
    assert!(!lock_available(&path).unwrap());
    assert_eq!(inherited_lock(&path).unwrap(), std::process::id());
    lock.unlock().unwrap();
    assert!(inherited_lock(&path).is_err());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn run_process_drains_both_streams_and_times_out_cleanly() {
    let mut command = std::process::Command::new("python3");
    command.args(["-c", "import os,threading; t=threading.Thread(target=lambda: os.write(2,b'\\xff'*131072)); t.start(); os.write(1,b'x'*131072); t.join()"]) ;
    let all_reaped = AtomicBool::new(true);
    let mut stderr = Vec::new();
    let result = run_process(
        command,
        b"",
        Duration::from_secs(2),
        Duration::from_millis(100),
        || Ok(()),
        &all_reaped,
        |_| Ok(()),
        |chunk| {
            stderr.extend_from_slice(chunk);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(result.stdout, vec![b'x'; 131072]);
    assert_eq!(stderr, vec![255; 131072]);

    let mut command = std::process::Command::new("sleep");
    command.arg("5");
    let result = run_process(
        command,
        b"",
        Duration::from_millis(200),
        Duration::from_millis(100),
        || Ok(()),
        &all_reaped,
        |_| Ok(()),
        |_| Ok(()),
    )
    .unwrap();
    assert!(matches!(result.outcome, ProcessOutcome::TimedOut { .. }));
    assert!(all_reaped.load(Ordering::SeqCst));
}

/// A request larger than the 64 KiB Linux pipe buffer cannot be absorbed by
/// the kernel before the arm exits, so the writer is still blocked when the
/// read end closes and the write fails with `EPIPE` on every run. A request
/// that fits the buffer can complete before the arm exits, so this size is
/// what makes the failure deterministic.
#[test]
fn a_failed_request_write_keeps_the_captured_child_diagnostics() {
    let mut command = std::process::Command::new("sh");
    command.args(["-c", "printf 'arm rejected the request\\n' >&2; exit 9"]);
    let all_reaped = AtomicBool::new(true);
    let mut observed = Vec::new();
    let result = run_process(
        command,
        &vec![b'q'; 1 << 20],
        Duration::from_secs(5),
        Duration::from_millis(100),
        || Ok(()),
        &all_reaped,
        |_| Ok(()),
        |chunk| {
            observed.extend_from_slice(chunk);
            Ok(())
        },
    )
    .expect("a failed request write returns the captured run rather than an early error");
    let error = result
        .callback_error
        .expect("the failed request write is reported to the caller");
    assert_eq!(error.kind(), ErrorKind::BrokenPipe);
    assert!(
        error.to_string().contains("writing the request"),
        "the error names the write that failed: {error}"
    );
    assert_eq!(result.stderr, b"arm rejected the request\n");
    assert_eq!(observed, result.stderr);
    assert!(
        matches!(result.outcome, ProcessOutcome::Exited { exit_code: 9, .. }),
        "{:?}",
        result.outcome
    );
    assert!(all_reaped.load(Ordering::SeqCst));
}
