//! Streamed device batch evaluation for the registered candidates.
//!
//! A candidate's kernels live in a prebuilt HIP executable, which is this
//! crate's established host/device boundary: the fixture evidence drivers
//! already reach their kernels by streaming canonical bytes to one of those
//! executables. Batch evaluation reuses that boundary rather than adding a
//! second one. The frame format is owned by `hip/wave_batch_stream.h`.
//!
//! One worker stays resident, so a measured batch pays the pipe transfer and
//! the executable's own per-batch device allocation, never a process start.
//! At most one worker exists at a time: a request for another candidate stops
//! the previous one, keeping exactly one extra device context alive beside the
//! caller's own.

use crate::MeasurementPath;

/// A candidate's implemented device batch kernel and the bounds it accepts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceBatchKernel {
    field_order: u64,
    max_order: usize,
    executable: DeviceExecutable,
    /// Arguments that select this candidate inside its executable.
    selector: &'static [&'static str],
    /// Whether the kernel reads the canonical two-nibble F_7 tables.
    uploads_lookup_tables: bool,
}

impl DeviceBatchKernel {
    pub(crate) const fn new(
        field_order: u64,
        max_order: usize,
        executable: DeviceExecutable,
        selector: &'static [&'static str],
        uploads_lookup_tables: bool,
    ) -> Self {
        Self {
            field_order,
            max_order,
            executable,
            selector,
            uploads_lookup_tables,
        }
    }

    /// Field order whose permanents this kernel evaluates.
    #[must_use]
    pub const fn field_order(self) -> u64 {
        self.field_order
    }

    /// Largest matrix order the kernel's packing and Gray bound accept.
    #[must_use]
    pub const fn max_order(self) -> usize {
        self.max_order
    }
}

/// The prebuilt executable that hosts a candidate's kernels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeviceExecutable {
    WaveGf3,
    F5Wave,
    WaveGf7,
}

/// Device-clock spans of one evaluated batch.
#[derive(Clone, Copy, Debug)]
pub struct DeviceSpans {
    /// Host-to-device transfer of the batch's matrix bytes.
    pub h2d: std::time::Duration,
    /// The candidate's own kernel launches for this batch.
    pub kernel: std::time::Duration,
    /// Device-to-host transfer of the batch's permanent values.
    pub d2h: std::time::Duration,
    /// Stream interval from the transfer's completion marker to the kernel
    /// start marker, which is this boundary's launch overhead.
    pub submission_to_kernel: std::time::Duration,
}

/// One batch's permanent values with the device spans that produced them.
#[derive(Clone, Debug)]
pub struct BatchEvaluation {
    /// One canonical permanent value per input matrix, in input order.
    pub values: Vec<u64>,
    /// Device-event spans reported by the candidate's executable.
    pub spans: DeviceSpans,
}

/// Why a registered candidate has no reachable device batch evaluator.
#[must_use]
pub(crate) fn missing_kernel_reason(path: MeasurementPath, detail: &str) -> String {
    format!("{} has no device batch evaluator: {detail}", path.name())
}

#[cfg(not(feature = "hip"))]
mod backend {
    use super::{BatchEvaluation, DeviceBatchKernel};
    use crate::MeasurementPath;

    /// Start or reuse this candidate's resident device worker.
    ///
    /// Without the crate's HIP feature no executable was compiled, so the
    /// candidate keeps an explicit reason rather than a silent omission.
    pub(super) fn prepare(_kernel: DeviceBatchKernel, path: MeasurementPath) -> Result<(), String> {
        Err(format!(
            "{} device batch evaluation requires the permanent-wave-gpu hip feature; \
no candidate executable was compiled",
            path.name()
        ))
    }

    pub(super) fn evaluate(
        kernel: DeviceBatchKernel,
        path: MeasurementPath,
        _n: usize,
        _matrices: &[u8],
    ) -> Result<BatchEvaluation, String> {
        Err(prepare(kernel, path).expect_err("the no-HIP build has no candidate executable"))
    }
}

#[cfg(feature = "hip")]
mod backend {
    use std::io::{BufReader, Read, Write};
    use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
    use std::sync::Mutex;
    use std::time::Duration;

    use super::{BatchEvaluation, DeviceBatchKernel, DeviceExecutable, DeviceSpans};
    use crate::MeasurementPath;

    const REQUEST_MAGIC: [u8; 8] = *b"GF2BEVAL";
    const FRAME_EVALUATE: u32 = 0;
    const FRAME_SHUTDOWN: u32 = 1;
    const FRAME_LOOKUP_TABLES: u32 = 2;
    const STATUS_OK: u32 = 0;

    /// The single resident worker, shared by every caller in this process.
    static WORKER: Mutex<Option<Worker>> = Mutex::new(None);

    impl DeviceExecutable {
        fn program(self) -> &'static str {
            match self {
                Self::WaveGf3 => env!("PERMANENT_WAVE_GPU_WAVE_GF3_EQUIVALENCE_BIN"),
                Self::F5Wave => env!("PERMANENT_WAVE_GPU_F5_EQUIVALENCE_BIN"),
                Self::WaveGf7 => env!("PERMANENT_WAVE_GPU_WAVE_GF7_EQUIVALENCE_BIN"),
            }
        }
    }

    struct Worker {
        path: MeasurementPath,
        child: Child,
        stdin: ChildStdin,
        stdout: BufReader<ChildStdout>,
    }

    impl Worker {
        fn start(kernel: DeviceBatchKernel, path: MeasurementPath) -> Result<Self, String> {
            let program = kernel.executable.program();
            let mut child = Command::new(program)
                .arg("--batch-stdin")
                .args(kernel.selector)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(|error| format!("start {} worker {program}: {error}", path.name()))?;
            let stdin = child
                .stdin
                .take()
                .ok_or_else(|| format!("{} worker provided no stdin", path.name()))?;
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| format!("{} worker provided no stdout", path.name()))?;
            let mut worker = Self {
                path,
                child,
                stdin,
                stdout: BufReader::new(stdout),
            };
            worker.write_all(&REQUEST_MAGIC)?;
            if kernel.uploads_lookup_tables {
                worker.upload_lookup_tables()?;
            }
            worker.stdin.flush().map_err(|error| worker.io(error))?;
            Ok(worker)
        }

        /// Send the canonical Packed7 tables the control's kernel reads.
        ///
        /// These are the same public table bytes the F_7 fixture evidence
        /// streams, so a measured batch and a fixture check upload identical
        /// arithmetic.
        fn upload_lookup_tables(&mut self) -> Result<(), String> {
            use gf2_algebra::packed::packed7::{ADD_LUT, MUL_LUT, SUB_LUT};

            self.write_all(&FRAME_LOOKUP_TABLES.to_le_bytes())?;
            self.write_all(&ADD_LUT)?;
            self.write_all(&SUB_LUT)?;
            self.write_all(&MUL_LUT)?;
            self.stdin.flush().map_err(|error| self.io(error))?;
            match self.read_u32()? {
                STATUS_OK => Ok(()),
                status => Err(format!(
                    "{} worker refused the canonical lookup-table upload with status {status}",
                    self.path.name()
                )),
            }
        }

        fn write_all(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.stdin
                .write_all(bytes)
                .map_err(|error| format!("write to {} worker: {error}", self.path.name()))
        }

        fn io(&self, error: std::io::Error) -> String {
            format!("{} worker stream failed: {error}", self.path.name())
        }

        fn read_exact(&mut self, buffer: &mut [u8]) -> Result<(), String> {
            self.stdout
                .read_exact(buffer)
                .map_err(|error| format!("read from {} worker: {error}", self.path.name()))
        }

        fn read_u32(&mut self) -> Result<u32, String> {
            let mut bytes = [0u8; 4];
            self.read_exact(&mut bytes)?;
            Ok(u32::from_le_bytes(bytes))
        }

        fn read_f64_seconds(&mut self, span: &str) -> Result<Duration, String> {
            let mut bytes = [0u8; 8];
            self.read_exact(&mut bytes)?;
            let seconds = f64::from_le_bytes(bytes);
            if !seconds.is_finite() || seconds < 0.0 {
                return Err(format!(
                    "{} worker reported a {span} span of {seconds}, which is not a duration",
                    self.path.name()
                ));
            }
            Ok(Duration::from_secs_f64(seconds))
        }

        fn evaluate(&mut self, n: usize, matrices: &[u8]) -> Result<BatchEvaluation, String> {
            let batch = matrices.len() / (n * n);
            self.write_all(&FRAME_EVALUATE.to_le_bytes())?;
            self.write_all(&u32::try_from(n).expect("bounded order").to_le_bytes())?;
            self.write_all(
                &u32::try_from(batch)
                    .map_err(|_| format!("batch of {batch} matrices exceeds the frame header"))?
                    .to_le_bytes(),
            )?;
            self.write_all(matrices)?;
            self.stdin.flush().map_err(|error| self.io(error))?;

            let status = self.read_u32()?;
            if status != STATUS_OK {
                return Err(format!(
                    "{} worker reported device failure status {status} for a batch of {batch} \
matrices at n = {n}; see its stderr",
                    self.path.name()
                ));
            }
            let spans = DeviceSpans {
                h2d: self.read_f64_seconds("host-to-device")?,
                kernel: self.read_f64_seconds("kernel")?,
                d2h: self.read_f64_seconds("device-to-host")?,
                submission_to_kernel: self.read_f64_seconds("submission-to-kernel")?,
            };
            let mut values = Vec::with_capacity(batch);
            for _ in 0..batch {
                values.push(u64::from(self.read_u32()?));
            }
            Ok(BatchEvaluation { values, spans })
        }
    }

    impl Drop for Worker {
        fn drop(&mut self) {
            let _ = self.stdin.write_all(&FRAME_SHUTDOWN.to_le_bytes());
            let _ = self.stdin.flush();
            let _ = self.child.wait();
        }
    }

    fn locked() -> std::sync::MutexGuard<'static, Option<Worker>> {
        WORKER
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn resident<'a>(
        worker: &'a mut Option<Worker>,
        kernel: DeviceBatchKernel,
        path: MeasurementPath,
    ) -> Result<&'a mut Worker, String> {
        if worker
            .as_ref()
            .is_some_and(|resident| resident.path != path)
        {
            *worker = None;
        }
        if worker.is_none() {
            *worker = Some(Worker::start(kernel, path)?);
        }
        Ok(worker.as_mut().expect("the worker was just started"))
    }

    pub(super) fn prepare(kernel: DeviceBatchKernel, path: MeasurementPath) -> Result<(), String> {
        resident(&mut locked(), kernel, path).map(|_| ())
    }

    pub(super) fn evaluate(
        kernel: DeviceBatchKernel,
        path: MeasurementPath,
        n: usize,
        matrices: &[u8],
    ) -> Result<BatchEvaluation, String> {
        let mut guard = locked();
        let evaluation = resident(&mut guard, kernel, path)?.evaluate(n, matrices);
        if evaluation.is_err() {
            // A broken stream leaves the worker unusable; the next cell starts
            // a fresh one rather than reading a desynchronized response.
            *guard = None;
        }
        evaluation
    }
}

impl MeasurementPath {
    /// Start or reuse this candidate's resident device batch worker.
    ///
    /// Callers run this before timing a cell so that a single-matrix
    /// calibration measures the kernel rather than a process start.
    ///
    /// # Errors
    ///
    /// Returns the reason this candidate cannot evaluate a batch on the
    /// device: no implemented kernel, a build without the HIP feature, or a
    /// host on which the executable did not start.
    pub fn prepare_batch_evaluator(self) -> Result<DeviceBatchKernel, String> {
        let kernel = self.device_batch_kernel()?;
        backend::prepare(kernel, self)?;
        Ok(kernel)
    }

    /// Evaluate `matrices.len() / (n * n)` matrices of order `n` on the device.
    ///
    /// `matrices` holds consecutive `n * n` row-major canonical field values,
    /// one byte per entry, exactly as `gf2_algebra::gpu` serialises a packed
    /// batch for the shipped kernels.
    ///
    /// # Errors
    ///
    /// Returns the reason the batch did not evaluate. A device failure inside
    /// the worker is reported here rather than substituting a host result.
    ///
    /// # Panics
    ///
    /// Panics if `n` is zero or `matrices.len()` is not a multiple of `n * n`.
    pub fn evaluate_batch(self, n: usize, matrices: &[u8]) -> Result<BatchEvaluation, String> {
        assert!(n > 0, "a device batch needs a positive matrix order");
        assert_eq!(
            matrices.len() % (n * n),
            0,
            "a device batch holds whole n x n matrices"
        );
        let kernel = self.device_batch_kernel()?;
        backend::evaluate(kernel, self, n, matrices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_candidate_states_its_kernel_or_names_its_absence() {
        for path in MeasurementPath::ALL {
            match path.device_batch_kernel() {
                Ok(kernel) => {
                    assert!(
                        matches!(kernel.field_order(), 3 | 5 | 7),
                        "{} must name the field its kernel evaluates",
                        path.name()
                    );
                    assert!(
                        (1..=63).contains(&kernel.max_order()),
                        "{} must bound its device order",
                        path.name()
                    );
                }
                Err(reason) => assert!(
                    reason.contains(path.name()),
                    "{} must name itself in its unavailable reason, got {reason}",
                    path.name()
                ),
            }
        }
    }

    #[test]
    fn the_accumulator_probe_names_its_missing_permanent_kernel_in_every_build() {
        let reason = MeasurementPath::F7ThreePlaneAccumulator
            .device_batch_kernel()
            .expect_err("the accumulator candidate has no full-permanent kernel");
        assert!(reason.contains("f7-three-plane-accumulator"));
        assert!(reason.contains("accumulator"));
        assert!(
            !reason.contains("hip feature"),
            "a structurally absent kernel must not be reported as a build gap: {reason}"
        );
    }

    #[cfg(not(feature = "hip"))]
    #[test]
    fn implemented_kernels_are_unreachable_without_the_hip_feature() {
        let reason = MeasurementPath::WaveGf3
            .prepare_batch_evaluator()
            .expect_err("no executable is compiled without the hip feature");
        assert!(reason.contains("hip feature"));
    }
}
