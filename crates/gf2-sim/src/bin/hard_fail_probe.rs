//! **Test-only** subprocess harness for the `42eac5cc` hard-fail criterion
//! (SC3): a forced GPU kernel error yields a non-zero process exit, a JSON
//! diagnostic dump in the configured directory, and a `tracing::error!` event
//! with the relevant context (design doc §8).
//!
//! This binary exists solely so the integration test
//! `tests/hard_fail_subprocess.rs` can assert the **actual process exit status**
//! of a hard-fail run — an in-process `Err`-return assertion does not prove the
//! process exits non-zero, and that reasoning has been rejected by formal review
//! on this project (issues `5f12e7ff` and the `42eac5cc` round-1 review). It is
//! never built into a production campaign; it is the failure-mode analogue of
//! `checkpoint_sweep`'s `--block-at-first-heartbeat` test-only flag.
//!
//! # What it does
//!
//! 1. Installs a minimal stderr ERROR-event JSON-lines `tracing` subscriber (no
//!    `tracing-subscriber` dependency) so the parent can capture and parse the
//!    `tracing::error!` events the hard-fail path emits.
//! 2. Forces the exact fatal error a
//!    [`KernelErrorInjector`](../../tests/common/mod.rs)-wrapped GPU stage
//!    produces — `StageError::Fatal(FatalError::KernelLaunch { .. })` — carrying
//!    a representative `hip_code`, kernel name, and launch-args context string.
//! 3. Drives the **production** hard-fail boundary
//!    [`dispatch_with_fallback`](gf2_sim::executor::failure::dispatch_with_fallback):
//!    the fatal arm writes the JSON diagnostic dump into the configured directory
//!    (atomic `.tmp` + rename) and emits the `tracing::error!` events, then
//!    returns the error unchanged.
//! 4. On the returned `Err` (the hard-fail), exits with status `1`.
//!
//! The `FaultContext` (device id, SNR index, batch id, worker index) and the
//! `KernelLaunch` fields all land in the JSON dump the parent test parses.
//!
//! # Usage
//!
//! ```text
//! hard_fail_probe --diagnostic-dump-dir <DIR>
//! ```

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use gf2_sim::error::{FatalError, StageError};
use gf2_sim::executor::failure::{dispatch_with_fallback, FaultContext};

/// A minimal `tracing::Subscriber` that prints every ERROR-level event to
/// stderr as one JSON object per line: `{"<field>":"<debug>", ...}`. Just enough
/// for the parent subprocess test to confirm a `tracing::error!` fired and to
/// read its fields. No `tracing-subscriber` dependency.
struct StderrErrorJson;

struct FieldVisitor(BTreeMap<String, String>);
impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.insert(field.name().to_string(), format!("{value:?}"));
    }
}

// Serialise stderr writes so concurrent events don't interleave a line.
static STDERR_LOCK: Mutex<()> = Mutex::new(());

impl tracing::Subscriber for StderrErrorJson {
    fn enabled(&self, meta: &tracing::Metadata<'_>) -> bool {
        *meta.level() == tracing::Level::ERROR
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        if *event.metadata().level() != tracing::Level::ERROR {
            return;
        }
        let mut v = FieldVisitor(BTreeMap::new());
        event.record(&mut v);
        // Render the BTreeMap as a stable JSON-lines object.
        let body = v
            .0
            .iter()
            .map(|(k, val)| format!("{}:{}", json_str(k), json_str(val)))
            .collect::<Vec<_>>()
            .join(",");
        let _guard = STDERR_LOCK.lock().unwrap();
        let mut err = std::io::stderr();
        let _ = writeln!(err, "{{\"level\":\"ERROR\",{body}}}");
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

/// Minimal JSON string escaper for the few characters that can appear in
/// rendered `Debug` field values (quotes and backslashes).
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn main() {
    let mut dump_dir: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--diagnostic-dump-dir" => {
                dump_dir = Some(PathBuf::from(
                    args.next().expect("--diagnostic-dump-dir needs a value"),
                ));
            }
            other => {
                eprintln!("hard_fail_probe: unknown argument `{other}`");
                std::process::exit(2);
            }
        }
    }
    let dump_dir = dump_dir.expect("--diagnostic-dump-dir is required");

    tracing::subscriber::set_global_default(StderrErrorJson)
        .expect("first and only global subscriber in this process");

    // The exact fatal error a `KernelErrorInjector`-wrapped GPU stage produces
    // (`tests/common/mod.rs`): a `KernelLaunch` fatal carrying a HIP error code,
    // kernel name, and launch-args context. 301 = `hipErrorFileNotFound`, the
    // code a missing kernel blob reports (the most context-rich hard-fail).
    let fatal: Result<u32, StageError> = Err(StageError::Fatal(FatalError::KernelLaunch {
        hip_code: 301,
        kernel: "ldpc_bp",
        args: "gfx1030: forced kernel hard-fail (hard_fail_probe)".to_string(),
    }));

    // Context that lands in the JSON dump (device id, SNR index, batch id).
    let ctx = FaultContext {
        batch_id: 42,
        snr_idx: 3,
        device_id: 0,
        worker_idx: 1,
    };

    // Drive the production hard-fail boundary: writes the JSON dump + emits the
    // `tracing::error!` events, returns the error unchanged.
    let result = dispatch_with_fallback(
        fatal,
        || Ok::<u32, StageError>(0), // never called on the fatal arm
        ctx,
        false, // strict_gpu irrelevant for a Fatal error
        &dump_dir,
    );

    match result {
        Ok(_) => {
            eprintln!("hard_fail_probe: BUG — fatal error did not propagate");
            std::process::exit(0);
        }
        Err(e) => {
            // The hard-fail: non-zero exit (SC3). The dump + error events were
            // already written/emitted by `dispatch_with_fallback`.
            eprintln!("hard_fail_probe: propagated fatal error: {e:?}");
            std::process::exit(1);
        }
    }
}
