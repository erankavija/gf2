//! Concurrent-drain child execution with process-group termination and adoption reaping.

use crate::campaign::{ProcessOutcome, CHILD_KILL_GRACE_SECONDS};
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

static RUNNER_LOCK: Mutex<()> = Mutex::new(());

/// The captured streams and terminal state of one child process.
pub struct ProcessResult {
    /// All bytes read from the child's standard output.
    pub stdout: Vec<u8>,
    /// All bytes read from the child's standard error.
    pub stderr: Vec<u8>,
    /// The observed terminal state of the child process tree.
    pub outcome: ProcessOutcome,
    /// The first error returned by the standard-error callback, if any.
    pub callback_error: Option<io::Error>,
}

enum Stream {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
    End,
    Error(io::Error),
}

fn drain(mut input: impl Read, tx: mpsc::Sender<Stream>, stderr: bool) {
    let mut buffer = [0u8; 8192];
    loop {
        match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                if tx
                    .send(if stderr {
                        Stream::Stderr(buffer[..n].to_vec())
                    } else {
                        Stream::Stdout(buffer[..n].to_vec())
                    })
                    .is_err()
                {
                    return;
                }
            }
            Err(e) => {
                let _ = tx.send(Stream::Error(e));
                break;
            }
        }
    }
    let _ = tx.send(Stream::End);
}

// Fresh owners do not spawn children. Process groups also contain accidental
// descendants, which are killed before a completed process can be accepted.
/// Reports whether a process group still contains a non-zombie process.
///
/// Returns an error when procfs cannot be read.
pub fn live_group(group: u32) -> io::Result<bool> {
    for entry in fs::read_dir("/proc")? {
        let path = entry?.path();
        if path
            .file_name()
            .and_then(|s| s.to_str())
            .is_none_or(|s| s.parse::<u32>().is_err())
        {
            continue;
        }
        let Ok(stat) = fs::read_to_string(path.join("stat")) else {
            continue;
        };
        let Some((_, tail)) = stat.rsplit_once(") ") else {
            continue;
        };
        let fields: Vec<_> = tail.split_whitespace().collect();
        if fields.get(2).and_then(|s| s.parse::<u32>().ok()) == Some(group)
            && fields.first() != Some(&"Z")
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn invalid(message: impl ToString) -> io::Error {
    io::Error::other(message.to_string())
}

fn signal_group(group: u32, signal: rustix::process::Signal) -> io::Result<()> {
    match rustix::process::kill_process_group(
        rustix::process::Pid::from_raw(group as i32)
            .ok_or_else(|| invalid("invalid process group"))?,
        signal,
    ) {
        Ok(()) => Ok(()),
        Err(rustix::io::Errno::SRCH) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn signal_descendants(signal: rustix::process::Signal) -> io::Result<()> {
    let mut parentage = Vec::new();
    for entry in fs::read_dir("/proc")? {
        let path = entry?.path();
        let Some(pid) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let stat = match fs::read_to_string(path.join("stat")) {
            Ok(stat) => stat,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let (_, tail) = stat
            .rsplit_once(')')
            .ok_or_else(|| invalid("invalid process stat"))?;
        let parent = tail
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| invalid("missing parent pid"))?
            .parse::<u32>()
            .map_err(invalid)?;
        parentage.push((pid, parent));
    }
    let mut descendants = std::collections::BTreeSet::from([std::process::id()]);
    loop {
        let before = descendants.len();
        for (pid, parent) in &parentage {
            if descendants.contains(parent) {
                descendants.insert(*pid);
            }
        }
        if descendants.len() == before {
            break;
        }
    }
    descendants.remove(&std::process::id());
    for pid in descendants {
        match rustix::process::kill_process(
            rustix::process::Pid::from_raw(pid as i32)
                .ok_or_else(|| invalid("invalid descendant PID"))?,
            signal,
        ) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn reap_adopted() -> io::Result<bool> {
    loop {
        match rustix::process::wait(rustix::process::WaitOptions::NOHANG) {
            Ok(Some(_)) => continue,
            Ok(None) => return Ok(false),
            Err(rustix::io::Errno::CHILD) => return Ok(true),
            Err(error) => return Err(error.into()),
        }
    }
}

struct ChildGuard<'a> {
    child: std::process::Child,
    group: u32,
    all_reaped: &'a AtomicBool,
    done: bool,
}

impl std::ops::Deref for ChildGuard<'_> {
    type Target = std::process::Child;

    fn deref(&self) -> &Self::Target {
        &self.child
    }
}

impl std::ops::DerefMut for ChildGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.child
    }
}

impl Drop for ChildGuard<'_> {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        let _ = signal_group(self.group, rustix::process::Signal::KILL);
        let _ = signal_descendants(rustix::process::Signal::KILL);
        let _ = self.child.kill();
        let _ = self.child.wait();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(CHILD_KILL_GRACE_SECONDS) {
            if reap_adopted().unwrap_or(false) {
                self.all_reaped.store(true, Ordering::SeqCst);
                return;
            }
            let _ = signal_descendants(rustix::process::Signal::KILL);
            thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Runs one child under a process-wide runner lock and subreaper.
///
/// The budget is polled once before spawn and its error is returned unchanged.
/// `all_reaped` is cleared after spawn and set once every descendant is reaped.
/// `started` receives the child PID. The callback receives raw standard-error
/// chunks; its first error stops the child and is returned in `callback_error`.
/// Errors from spawning, stream setup, waiting, or process-tree management are
/// returned directly. The callback is not called after its first error.
#[allow(clippy::too_many_arguments)]
pub fn run_process(
    mut command: Command,
    input: &[u8],
    timeout: Duration,
    grace: Duration,
    mut budget: impl FnMut() -> io::Result<()>,
    all_reaped: &AtomicBool,
    mut started: impl FnMut(u32) -> io::Result<()>,
    mut stderr_callback: impl FnMut(&[u8]) -> io::Result<()>,
) -> io::Result<ProcessResult> {
    let _runner = RUNNER_LOCK
        .lock()
        .map_err(|_| invalid("process runner lock poisoned"))?;
    rustix::process::set_child_subreaper(Some(rustix::process::getpid()))?;
    command
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let begin = Instant::now();
    budget()?;
    let child = command.spawn()?;
    let mut child = ChildGuard {
        group: child.id(),
        child,
        all_reaped,
        done: false,
    };
    all_reaped.store(false, Ordering::SeqCst);
    let pid = child.id();
    let mut callback_error = started(pid).err();
    let stdin = child.stdin.take().ok_or_else(|| invalid("missing stdin"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| invalid("missing stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| invalid("missing stderr"))?;
    let (tx, rx) = mpsc::channel();
    let input = input.to_vec();
    let writer = thread::spawn(move || {
        let mut stdin = stdin;
        stdin.write_all(&input)
    });
    let out_tx = tx.clone();
    let err_tx = tx.clone();
    drop(tx);
    let out_thread = thread::spawn(move || drain(stdout, out_tx, false));
    let err_thread = thread::spawn(move || drain(stderr, err_tx, true));
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut ends = 0;
    let mut status = None;
    let mut stopped = None;
    let mut timed_out = false;
    let mut killed = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(10)) {
            Ok(Stream::Stdout(bytes)) => out.extend_from_slice(&bytes),
            Ok(Stream::Stderr(bytes)) => {
                err.extend_from_slice(&bytes);
                if callback_error.is_none() {
                    callback_error = stderr_callback(&bytes).err();
                }
            }
            Ok(Stream::End) => ends += 1,
            Ok(Stream::Error(e)) => {
                if callback_error.is_none() {
                    callback_error = Some(e);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if status.is_none() {
            status = child.try_wait()?;
        }
        if stopped.is_none()
            && (begin.elapsed() >= timeout
                || callback_error.is_some()
                || (status.is_some() && (live_group(pid)? || !reap_adopted()?)))
        {
            timed_out = begin.elapsed() >= timeout;
            signal_group(pid, rustix::process::Signal::TERM)?;
            signal_descendants(rustix::process::Signal::TERM)?;
            stopped = Some(Instant::now());
        }
        if stopped.is_some_and(|s| s.elapsed() >= grace) && !killed {
            signal_group(pid, rustix::process::Signal::KILL)?;
            signal_descendants(rustix::process::Signal::KILL)?;
            killed = true;
        }
        if stopped.is_some() {
            signal_descendants(if killed {
                rustix::process::Signal::KILL
            } else {
                rustix::process::Signal::TERM
            })?;
        }
        if status.is_some() && ends == 2 && !live_group(pid)? && reap_adopted()? {
            break;
        }
        if stopped.is_some_and(|s| s.elapsed() > grace + Duration::from_secs(1)) {
            return Err(invalid(
                "child process tree did not terminate/drain within kill grace",
            ));
        }
    }
    let status = child.wait()?;
    writer
        .join()
        .map_err(|_| invalid("stdin writer panicked"))??;
    out_thread
        .join()
        .map_err(|_| invalid("stdout reader panicked"))?;
    err_thread
        .join()
        .map_err(|_| invalid("stderr reader panicked"))?;
    child.done = true;
    all_reaped.store(true, Ordering::SeqCst);
    let elapsed_ns = u64::try_from(begin.elapsed().as_nanos())
        .unwrap_or(u64::MAX)
        .max(1);
    let outcome = if timed_out {
        ProcessOutcome::TimedOut {
            pid,
            elapsed_ns,
            kill_grace_exhausted: killed,
            all_descendants_reaped: true,
        }
    } else if let Some(exit_code) = status.code() {
        ProcessOutcome::Exited {
            pid,
            exit_code,
            elapsed_ns,
            all_descendants_reaped: true,
        }
    } else {
        ProcessOutcome::Signaled {
            pid,
            signal: status
                .signal()
                .ok_or_else(|| invalid("unknown process termination"))?,
            elapsed_ns,
            all_descendants_reaped: true,
        }
    };
    Ok(ProcessResult {
        stdout: out,
        stderr: err,
        outcome,
        callback_error,
    })
}
