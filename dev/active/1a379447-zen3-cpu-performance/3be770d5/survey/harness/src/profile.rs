//! The command line, profiled region and record every profile driver serves
//! to `profile-session.sh`.
//!
//! [`profiled`] performs one untimed warm dispatch on the pinned pool, then
//! the requested passes. Given the session's control channel it enables and
//! disables a `perf` session started with `-D -1 --control fifo:CTL,ACK`
//! around exactly those passes, so setup and the warm pass stay outside the
//! samples. Each driver includes this file by path rather than through the
//! library, because any library change moves the digests of the pinned
//! executables that link it.

use ldpc_survey::arm::DecoderCase;
use ldpc_throughput::pool::{self, process_threads, Finished};
use ldpc_throughput::workload::{Threads, Workload};
use serde_json::{json, Value};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::str::FromStr;
use std::time::Instant;
use tuning_campaign_support::host::{resolve_core_arm, CoreArm, CpuAffinity, CpuTopology};

const SCHEMA: &str = "ldpc-profile-record-v1";

/// The flags `profile-session.sh` passes to a profile driver.
pub struct ProfileOptions {
    /// The driver's decoder selection, which it interprets.
    pub arm: String,
    pub bundle: String,
    pub code: String,
    /// A protocol core arm, resolved by [`ProfileOptions::load`].
    pub core_arm: String,
    /// Frames per worker per pass.
    pub batch: u32,
    /// Dispatches inside the profiled region.
    pub passes: u64,
    pub iteration_cap: u32,
    pub norm: f32,
    pub perf_control: Option<(String, String)>,
}

impl ProfileOptions {
    /// Parses `--arm A --bundle DIR --code NAME --core-arm ARM [--batch 16]
    /// [--passes 1] [--iteration-cap 50] [--norm 0.75] [--perf-control CTL,ACK]`.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown flag, a missing or malformed value, a
    /// missing `--arm`, `--bundle` or `--code`, or zero passes.
    pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut options = Self {
            arm: String::new(),
            bundle: String::new(),
            code: String::new(),
            core_arm: String::new(),
            batch: 16,
            passes: 1,
            iteration_cap: 50,
            norm: 0.75,
            perf_control: None,
        };
        while let Some(flag) = args.next() {
            let mut value = || args.next().ok_or(format!("{flag} needs a value"));
            fn number<T: FromStr>(text: String, flag: &str) -> Result<T, String> {
                text.parse().map_err(|_| format!("{flag}: not a number"))
            }
            match flag.as_str() {
                "--arm" => options.arm = value()?,
                "--bundle" => options.bundle = value()?,
                "--code" => options.code = value()?,
                "--core-arm" => options.core_arm = value()?,
                "--batch" => options.batch = number(value()?, &flag)?,
                "--passes" => options.passes = number(value()?, &flag)?,
                "--iteration-cap" => options.iteration_cap = number(value()?, &flag)?,
                "--norm" => options.norm = number(value()?, &flag)?,
                "--perf-control" => options.perf_control = Some(perf_control(&value()?)?),
                other => return Err(format!("unknown argument {other}")),
            }
        }
        if options.arm.is_empty() || options.bundle.is_empty() || options.code.is_empty() {
            return Err("--arm, --bundle and --code are required".to_owned());
        }
        if options.passes == 0 {
            return Err("--passes must be positive".to_owned());
        }
        Ok(options)
    }

    /// The syndrome-stopping decoder case the flags name.
    #[must_use]
    pub fn case(&self) -> DecoderCase {
        DecoderCase {
            bundle: self.bundle.clone(),
            code: self.code.clone(),
            iteration_cap: self.iteration_cap,
            normalization_factor: self.norm,
            syndrome_stopping: true,
            batch_size: self.batch,
            quality_frames: 0,
            decisions_out: None,
        }
    }

    /// Loads the bundle and resolves the core arm on this host.
    ///
    /// # Errors
    ///
    /// Returns an error when the bundle is unusable, the core arm does not
    /// resolve, or the batch is empty or exceeds the recorded frames.
    pub fn load(&self) -> Result<(Workload, Vec<u32>), String> {
        let workload = Workload::load(&self.case())?;
        let core_arm: CoreArm =
            serde_json::from_value(json!(self.core_arm)).map_err(|e| format!("--core-arm: {e}"))?;
        let topology = CpuTopology::observe().map_err(|e| e.to_string())?;
        let affinity = CpuAffinity::observe().map_err(|e| e.to_string())?;
        let cpus = resolve_core_arm(core_arm, &topology, &affinity)?;
        if self.batch == 0 || self.batch as usize > workload.manifest.frames {
            return Err(format!(
                "--batch must lie in 1..={}",
                workload.manifest.frames
            ));
        }
        Ok((workload, cpus))
    }
}

/// The acknowledgement tag in one line of the ACK channel.
///
/// `perf` writes its tag with the trailing NUL of the C string literal, so
/// every acknowledgement is the five bytes `ack\n\0` and the NUL of one
/// acknowledgement leads the line that carries the next one.
fn ack_tag(line: &str) -> &str {
    line.trim_matches(|c: char| c == '\0' || c.is_whitespace())
}

/// A `perf` control channel: commands go to CTL, acknowledgements come on ACK.
struct PerfControl<W, R> {
    ctl: W,
    ack: R,
}

impl PerfControl<File, BufReader<File>> {
    fn open(ctl: &str, ack: &str) -> Result<Self, String> {
        let ctl = OpenOptions::new()
            .write(true)
            .open(ctl)
            .map_err(|e| format!("{ctl}: {e}"))?;
        let ack = BufReader::new(File::open(ack).map_err(|e| format!("{ack}: {e}"))?);
        Ok(Self { ctl, ack })
    }
}

impl<W: Write, R: BufRead> PerfControl<W, R> {
    fn command(&mut self, command: &str) -> Result<(), String> {
        writeln!(self.ctl, "{command}").map_err(|e| e.to_string())?;
        self.ctl.flush().map_err(|e| e.to_string())?;
        let mut line = String::new();
        self.ack.read_line(&mut line).map_err(|e| e.to_string())?;
        if ack_tag(&line) != "ack" {
            return Err(format!("perf answered {line:?} to {command}"));
        }
        Ok(())
    }
}

/// Parses the `CTL,ACK` value of a `--perf-control` flag.
///
/// # Errors
///
/// Returns an error when the value holds no comma.
pub fn perf_control(value: &str) -> Result<(String, String), String> {
    let (ctl, ack) = value
        .split_once(',')
        .ok_or("--perf-control takes CTL,ACK")?;
    Ok((ctl.to_owned(), ack.to_owned()))
}

/// Timings of the profiled region.
pub struct Region {
    /// From `setup_start` until every worker had built its state.
    pub setup_ns: u64,
    /// The passes inside the profiled region.
    pub decode_ns: u64,
    pub threads: Threads,
    /// Body invocations per worker, the warm one included.
    pub calls: u64,
}

/// Runs `passes` dispatches of `body` on one pinned worker per CPU of `cpus`
/// after one warm dispatch, inside the `perf` region of `control` when given.
///
/// # Errors
///
/// Returns an error when the pool fails or `perf` does not acknowledge a
/// command.
pub fn profiled<S>(
    cpus: &[u32],
    passes: u64,
    control: Option<&(String, String)>,
    setup_start: Instant,
    init: impl Fn(usize) -> Result<S, String> + Sync,
    body: impl Fn(&mut S) + Sync,
) -> Result<(Region, Vec<Finished<S>>), String>
where
    S: Send,
{
    let (region, finished) = pool::run(
        cpus,
        init,
        |_, state| body(state),
        |dispatch| -> Result<Region, String> {
            let setup_ns = setup_start.elapsed().as_nanos() as u64;
            let ready = process_threads();
            dispatch.call();
            let mut control = match control {
                Some((ctl, ack)) => Some(PerfControl::open(ctl, ack)?),
                None => None,
            };
            if let Some(control) = control.as_mut() {
                control.command("enable")?;
            }
            let start = Instant::now();
            for _ in 0..passes {
                dispatch.call();
            }
            let decode_ns = start.elapsed().as_nanos() as u64;
            if let Some(control) = control.as_mut() {
                control.command("disable")?;
            }
            Ok(Region {
                setup_ns,
                decode_ns,
                threads: Threads {
                    ready,
                    after: process_threads(),
                },
                calls: passes + 1,
            })
        },
    )?;
    Ok((region?, finished))
}

/// The JSON record a profile driver prints for `summarize-profile.py`.
///
/// The wall-clock figures are profile-session observations, not protocol
/// timing windows.
#[must_use]
pub fn record(
    options: &ProfileOptions,
    workload: &Workload,
    cpus: &[u32],
    workers: u32,
    region: &Region,
    selected_path: &str,
    batch_iterations: Option<&[u32]>,
) -> Value {
    let frames_per_worker = u64::from(options.batch) * options.passes;
    json!({
        "schema": SCHEMA,
        "arm": options.arm,
        "selected_path": selected_path,
        "code": options.code,
        "bundle_llr_sha256": workload.manifest.llrs_sha256,
        "h_sha256": workload.manifest.h_sha256,
        "core_arm": options.core_arm,
        "cpus": cpus,
        "workers": workers,
        "threads": region.threads,
        "batch": options.batch,
        "passes": options.passes,
        "frames_per_worker": frames_per_worker,
        "frames": frames_per_worker * u64::from(workers),
        "iteration_cap": options.iteration_cap,
        "normalization_factor": options.norm,
        "batch_iterations": batch_iterations,
        "setup_ns": region.setup_ns,
        "decode_ns": region.decode_ns,
        "ns_per_frame_per_worker": region.decode_ns as f64 / frames_per_worker as f64,
        "perf_controlled": options.perf_control.is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::{ack_tag, perf_control, PerfControl, ProfileOptions};

    /// The byte sequence `perf` writes per acknowledgement, tag and NUL.
    const ACK: &[u8] = b"ack\n\0";

    #[test]
    fn a_run_of_acknowledgements_answers_every_command() {
        let replies: Vec<u8> = ACK.iter().chain(ACK).copied().collect();
        let mut control = PerfControl {
            ctl: Vec::new(),
            ack: replies.as_slice(),
        };
        control.command("enable").expect("enable acknowledged");
        control.command("disable").expect("disable acknowledged");
        assert_eq!(control.ctl, b"enable\ndisable\n");
    }

    #[test]
    fn another_answer_is_an_error_naming_the_command() {
        let mut control = PerfControl {
            ctl: Vec::new(),
            ack: &b"nak\n"[..],
        };
        let error = control.command("enable").expect_err("nak rejected");
        assert!(error.contains("enable"), "{error}");
    }

    #[test]
    fn the_tag_survives_perf_padding() {
        assert_eq!(ack_tag("ack\n"), "ack");
        assert_eq!(ack_tag("\0ack\n"), "ack");
        assert_eq!(ack_tag("ack\n\0"), "ack");
        assert_eq!(ack_tag("\0nak\n"), "nak");
    }

    #[test]
    fn the_flag_names_both_fifos() {
        assert_eq!(
            perf_control("a,b").expect("pair"),
            ("a".to_owned(), "b".to_owned())
        );
        assert!(perf_control("a").is_err());
    }

    fn parsed(args: &[&str]) -> Result<ProfileOptions, String> {
        ProfileOptions::parse(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn the_session_flags_parse() {
        let options = parsed(&[
            "--arm",
            "a",
            "--bundle",
            "b",
            "--code",
            "c",
            "--core-arm",
            "single-core",
            "--batch",
            "8",
            "--passes",
            "3",
            "--perf-control",
            "x,y",
        ])
        .expect("parsed");
        assert_eq!((options.batch, options.passes), (8, 3));
        assert_eq!(options.perf_control, Some(("x".to_owned(), "y".to_owned())));
        assert_eq!(options.case().batch_size, 8);
    }

    #[test]
    fn incomplete_or_unknown_flags_are_refused() {
        assert!(parsed(&["--arm", "a", "--bundle", "b"]).is_err());
        assert!(parsed(&["--arm", "a", "--bundle", "b", "--code", "c", "--passes", "0"]).is_err());
        assert!(parsed(&["--arm", "a", "--bundle", "b", "--code", "c", "--x", "1"]).is_err());
        assert!(parsed(&["--arm"]).is_err());
    }
}
