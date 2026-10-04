//! Test-only subprocess harness for `tests/hard_fail_subprocess.rs`: sends a
//! fatal `KernelLaunch` error through
//! [`gf2_sim::executor::failure::dispatch_with_fallback`] with
//! `--diagnostic-dump-dir <DIR>` as the dump directory, prints ERROR-level
//! `tracing` events to stderr as JSON lines, and exits with status `1` when the
//! error propagates.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use gf2_sim::error::{FatalError, StageError};
use gf2_sim::executor::failure::{dispatch_with_fallback, FaultContext};

/// Prints every ERROR-level event to stderr as one JSON object per line, each
/// field rendered with `Debug`.
struct StderrErrorJson;

struct FieldVisitor(BTreeMap<String, String>);
impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0
            .insert(field.name().to_string(), format!("{value:?}"));
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
        let body =
            v.0.iter()
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

/// Partial JSON string escaper: quotes, backslashes and newlines only.
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

    // 301 is `hipErrorFileNotFound`.
    let fatal: Result<u32, StageError> = Err(StageError::Fatal(FatalError::KernelLaunch {
        hip_code: 301,
        kernel: "ldpc_bp",
        args: "gfx1030: forced kernel hard-fail (hard_fail_probe)".to_string(),
    }));

    let ctx = FaultContext {
        batch_id: 42,
        snr_idx: 3,
        device_id: 0,
        worker_idx: 1,
    };

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
            eprintln!("hard_fail_probe: propagated fatal error: {e:?}");
            std::process::exit(1);
        }
    }
}
