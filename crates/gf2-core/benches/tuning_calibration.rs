//! Host calibration for the tuning-profile selector families.
//!
//! This target is an explicit benchmark **action**, not a build step. It
//! measures both arms of each selector crossover on the current host, picks a
//! threshold per field, and emits a canonical format-2 core-owner envelope
//! that [`ProducedCoreProfile::from_json`] accepts. Nothing about a `cargo build`,
//! `cargo test`, or `cargo clippy` invokes it: `cargo` only runs a
//! `harness = false` bench target under `cargo bench`, and even then the
//! target refuses to measure or emit unless the prepared-host marker
//! `GF2_BENCH=1` is set and it observes the repository lock wrapper's mutex on
//! one of its inherited descriptors.
//!
//! # Workflow
//!
//! ## 1. Prepare the host
//!
//! Calibrate on an uncontended host, as `@/inv/benchmark-backed-performance`
//! requires of any crossover claim. `GF2_BENCH=1` is the repository's
//! prepared-host marker, and the lock wrapper serialises the run against every
//! sibling benchmark on the machine.
//!
//! `dev/scripts/ccx1-bench-flock.sh` takes `flock -x` on the mutex named by
//! `GF2_CCX1_LOCK` (default `/tmp/gf2-ccx1.lock`), creating the file when it is
//! absent, and holds it for the child command's whole lifetime. It passes
//! neither `-n` nor a timeout, so a second caller blocks until the first
//! finishes rather than failing. It then runs the child under
//! `taskset -c 6-11 nice -n -5`; the niceness is best-effort and a non-root
//! caller sees `nice` report a permission denial and continue at niceness 0,
//! while the lock and the affinity stay in force. Its `--full-host` form takes
//! the same mutex and omits `taskset`, for a benchmark whose named
//! configuration needs the whole processor.
//!
//! ## 2. Run the action
//!
//! ```sh
//! GF2_BENCH=1 ./dev/scripts/ccx1-bench-flock.sh \
//!   cargo bench -p gf2-core --features tuning-profile \
//!   --bench tuning_calibration -- \
//!   --executions 5 --repetitions 5 --target-ms 250 \
//!   --out /tmp/<unique-absent-path>.json \
//!   --lock-wrapper dev/scripts/ccx1-bench-flock.sh \
//!   --receipt dev/benchmarks/tuning_profiles/<date>-<name>.md
//! ```
//!
//! `--lock-wrapper` and `--receipt` name the wrapper that was invoked and the
//! receipt this run will be committed as; both are validated as
//! repository-relative paths and land in the emitted provenance. The wrapper
//! must exist at run time. The receipt is written after the run, so its path is
//! recorded rather than checked. `--profile-id` overrides the profile
//! identifier, which otherwise comes from the emitted file's own basename and
//! must be kebab-case.
//!
//! Add `simd` to the feature list to calibrate `bit_backend.simd_min_words`
//! against a real SIMD arm. Without that feature `SelectedBackend::Simd` does
//! not exist, the scalar arm is the only reachable one, and a profile's
//! `simd_min_words` is inert on the resulting build — the documented
//! `@/inv/accelerator-safe-fallback` behaviour. The sweep then reports the
//! field as having no comparable grid point and keeps the conservative default.
//!
//! `--self-check` prints the protocol constants and the observed host facts
//! without measuring or emitting; `--list-grid` prints the grid, the
//! conservative defaults it straddles, and which arms are measured in a child
//! process. Neither needs `GF2_BENCH=1`.
//!
//! Forced arms run through a private `--fresh-tuning-process-child` entry. The
//! parent guards it with `GF2_TUNING_FRESH_CASE=child-v1`, sends one canonical
//! compact JSON case on stdin, and accepts exactly one canonical result line
//! prefixed `GF2_TUNING_RESULT=`. The child mode accepts no case data on its
//! command line and does not consult the prepared-host marker.
//!
//! ## 3. Commit the emitted profile
//!
//! The action writes a temporary file beside the `--out` path, reads it back
//! through [`ProducedCoreProfile::from_json`], and compares the parsed value
//! against the one it serialised. It publishes the validated bytes atomically
//! without replacing an existing path. Only a document that survives that
//! round trip is reported as an artifact. Write to a unique absent `/tmp` path
//! so a partial or rejected run leaves no final artifact and nothing under
//! version control is overwritten.
//!
//! Committing an emitted profile means selecting an explicit authoritative
//! core-owner path, copying the validated file byte-for-byte there, and naming
//! that exact path in its focused artifact test. The complete repository
//! envelope is then assembled mechanically from the explicit core and algebra
//! owner paths; composition preserves each section's measurement provenance
//! and recomputes the complete envelope's assembly provenance and content
//! digest. Commit the receipt named by `--receipt` with those artifacts. It
//! carries the provenance table, the protocol, the per-field grid with the
//! selecting margin, and every tie or non-monotone case, following the
//! committed-receipt conventions of the sibling receipts under
//! `dev/benchmarks/tuning_profiles/`. A profile whose provenance records
//! `source_dirty: true` is not committable: the revision it names does not
//! reproduce the binary that produced the numbers.
//!
//! Nothing in the library reads a profile from the filesystem. A committed
//! profile reaches a process only when a caller parses it and calls
//! `gf2_core::tuning::install` before the first selection boundary runs.
//!
//! # Sweep and selection rule
//!
//! Per selector field, both arms of the crossover are measured across a size
//! grid straddling the conservative default, with one fixture per grid point
//! shared by both arms so operand construction is identical on them. Where the
//! two arms run in separate processes the fixture is rebuilt from the same
//! deterministic seed and the parent checks that the digests agree, which
//! carries the same guarantee across the process boundary. The two arms
//! alternate at execution granularity, each execution recalibrating its own
//! call count, so a frequency or thermal drift across a grid point is spread
//! over both arms instead of biasing the one measured second; a child-process
//! arm alternates the same way, one process per arm per execution. The
//! alternation stays outside the timed windows: each window is a monomorphic
//! loop over one entry point, which matters for the bit-logical arms whose
//! per-call cost is a few nanoseconds.
//!
//! An arm's rate at a grid point is the median of its `executions ×
//! repetitions` windows; its spread is the interquartile range of those windows
//! divided by that median. The noise band at a grid point is the larger of the
//! two arms' spreads, and the asymptotic arm wins there when its median beats
//! the other arm's by a relative margin exceeding that band.
//!
//! The field's value is the smallest grid point at which the asymptotic arm
//! wins, provided it also wins at every larger grid point. **On a tie, on a
//! non-monotone crossover, or where no grid point offers both arms, the
//! conservative default is kept**, so the action's output is never worse than
//! the default by construction. The measured grid is reported either way, and a
//! non-monotone sweep belongs in the receipt with its numbers rather than
//! smoothed away, per `@/inv/falsification-preserved`.
//!
//! For a `_max_` field the same crossover search runs and the value is the
//! largest grid point below the crossover, since the comparison the field feeds
//! selects the conservative arm at or below its value.
//!
//! # Measured against uncalibrated
//!
//! Design §5 condition 5 governs every field this sweep does not cover: "Until
//! the sweep covers it, a committed profile omits the field and inherits the
//! default; a profile that carries an uncalibrated value is a
//! `@/inv/benchmark-backed-performance` defect."
//!
//! The emitted document therefore states a field only when this run measured
//! it, and the omission set is the complement: every
//! `selectors.<family>.<field>` key the [`CoreTuningCodec`] writes, read off its
//! output at run time, minus the fields whose sweep reached a comparison.
//! A schema field this harness has never heard of is omitted by construction,
//! so a follow-on selector family landing its fields cannot leak an unmeasured
//! value into an emitted profile, and no field inventory is maintained here to
//! go stale against the schema.
//!
//! A tie and a non-monotone crossover are **calibration outcomes**: both arms
//! were measured across the grid, and the rule concluded that the default
//! stands. Such a field states its value in the document like any other.
//!
//! A field with **no comparable grid point**, and a field no sweep covers at
//! all, were never calibrated. An absent field is a supported state of the
//! core owner codec, which resolves it to the conservative default, so the
//! document still loads and still selects the same threshold, and it stops
//! claiming a value nothing measured.
//!
//! # Arm reachability
//!
//! Each arm has to be callable on its own for a grid point to yield a
//! comparison. Four of the five fields have both arms in the public API:
//! `ScalarBackend` against `SimdBackend`, [`FieldPoly::mul`] against
//! [`FieldPoly::mul_ntt`], [`FieldPoly::div_rem`] against
//! [`FieldPoly::div_rem_fast`], and [`FieldPoly::eval_batch`] against
//! [`batch_evaluate_subproduct_auto`].
//!
//! `polynomial.karatsuba_min_degree` has no such pair and gains none here. Its
//! two arms are the private `mul_schoolbook_impl` and `mul_karatsuba_raw`,
//! reachable only through [`FieldPoly::mul`], which picks one of them from the
//! active profile — schoolbook below `karatsuba_min_degree`, Karatsuba at or
//! above it. The sweep reaches both by installing a profile that forces the
//! arm, as [`forced_karatsuba_min_degree`] states: the grid point itself takes
//! the Karatsuba arm there, and [`FORCED_SCHOOLBOOK_MIN_DEGREE`] takes the
//! schoolbook arm.
//!
//! `gf2_core::tuning::install` resolves the process-wide profile once, so one
//! process offers one arm. Each arm at each grid point is therefore measured in
//! a **child process**: this binary re-executes its guarded private child mode,
//! and the child installs the forcing profile before any selection boundary
//! runs, asserts that the core section resolved as `Installed`, and reports
//! back
//!
//! - the arm the production selector [`mul_route`] picks under that profile,
//!   which the parent checks against the arm it asked for, so "both arms were
//!   measured" is an observation rather than an assumption;
//! - a digest of the operands it built, which the parent checks against its own
//!   fixture, so the two arms are compared on identical operands although
//!   neither process built the other's;
//! - a digest of the product, which the parent compares across the two arms as
//!   this field's equivalence probe.
//!
//! `mul_karatsuba_raw` recurses on the same profile value, so the value forced
//! for the Karatsuba arm decides which algorithm is timed. Forcing the grid
//! point makes the recursion split once at that degree and hand every
//! sub-operand, whose degree is about half of it, to the schoolbook base case —
//! exactly what the dispatcher runs when `karatsuba_min_degree` is set to that
//! grid point. Each grid point therefore compares the two arms the selection
//! rule chooses between at that point, rather than the cost of a Karatsuba
//! recursion carried to a base case no threshold would produce.

use std::env;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::hint::black_box;
use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gf2_core::field::poly::{batch_evaluate_subproduct_auto, mul_route, FieldPoly, MulRoute};
use gf2_core::gfp::Fp;
use gf2_core::kernels::{Backend, ScalarBackend};
use gf2_core::rng::Lcg;
use gf2_core::tuning;
use gf2_core::tuning::{
    AssemblyProvenance, BitBackendSelectors, CanonicalValue, CompiledProfileProvenance,
    CoreSelectors, CoreTuning, CoreTuningCodec, GitRevision, HarnessSchema, MeasurementProvenance,
    PolynomialSelectors, PreparedEnvelope, ProfileId, ProfileRegistry, ProfileRegistryBuilder,
    RepoRelPath, Rfc3339Utc, SectionCodec, Sha256,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProducedCoreProfile {
    id: ProfileId,
    measurement: MeasurementProvenance,
    assembly: AssemblyProvenance,
    section: CoreTuning,
}

impl std::ops::Deref for ProducedCoreProfile {
    type Target = CoreTuning;

    fn deref(&self) -> &Self::Target {
        &self.section
    }
}

impl ProducedCoreProfile {
    fn to_json(&self) -> String {
        let prepared = PreparedEnvelope::compiled(
            self.id.clone(),
            CompiledProfileProvenance {
                artifact_id: self.id.clone(),
            },
        )
        .insert_measured::<CoreTuning, CoreTuningCodec>(
            self.section.clone(),
            self.measurement.clone(),
        )
        .expect("calibration evidence was validated when collected")
        .build()
        .expect("one typed section builds a prepared envelope");
        core_registry()
            .expect("the core owner registry has one valid codec")
            .to_json(&prepared, &self.assembly)
            .expect("validated calibration output encodes")
    }

    fn from_json(document: &str) -> Result<Self, String> {
        let prepared = core_registry()
            .map_err(|error| format!("core registry is invalid: {error}"))?
            .from_json(document)
            .map_err(|error| format!("core owner envelope is invalid: {error}"))?;
        let ids: Vec<&str> = prepared.section_ids().collect();
        if ids != ["gf2-core/selectors"] {
            return Err(format!("core owner envelope has section IDs {ids:?}"));
        }
        let projection = prepared
            .section::<CoreTuning>()
            .map_err(|error| format!("typed core projection failed: {error}"))?
            .ok_or("core owner envelope is missing its section")?;
        let assembly = prepared
            .verified_assembly()
            .ok_or("canonical core owner lacks verified assembly")?
            .provenance
            .clone();
        Ok(Self {
            id: prepared.profile_id().clone(),
            measurement: projection.measurement.clone(),
            assembly,
            section: projection.section.clone(),
        })
    }

    fn omitting(&self, omitted: &[SchemaField]) -> Result<Self, String> {
        let body = CoreTuningCodec::encode_body(&self.section)
            .map_err(|error| format!("complete core section does not encode: {error}"))?;
        let mut selectors = serde_json::to_value(body)
            .map_err(|error| format!("complete core section is not JSON: {error}"))?;
        for field in omitted {
            let family = selectors
                .get_mut(&field.family)
                .and_then(serde_json::Value::as_object_mut)
                .ok_or_else(|| format!("complete core section has no `{}` family", field.family))?;
            if family.remove(&field.name).is_none() {
                return Err(format!("complete core section has no `{field}` field"));
            }
        }
        let canonical = CanonicalValue::serialize(&selectors)
            .map_err(|error| format!("omitted selector body is not canonical: {error}"))?;
        let section = CoreTuningCodec::decode_body(canonical)
            .map_err(|error| format!("omitted core section is invalid: {error}"))?;
        Ok(Self {
            section,
            ..self.clone()
        })
    }
}

fn core_registry() -> Result<ProfileRegistry, gf2_core::tuning::RegistryError> {
    ProfileRegistryBuilder::new()
        .register::<CoreTuning, CoreTuningCodec>()?
        .build()
}

#[allow(dead_code)]
fn complete_selector_value(section: &CoreTuning) -> Result<serde_json::Value, String> {
    let complete = CoreTuning::from_selectors(section.selectors().clone());
    let body = CoreTuningCodec::encode_body(&complete)
        .map_err(|error| format!("complete selector view does not encode: {error}"))?;
    serde_json::to_value(body)
        .map_err(|error| format!("complete selector view is not JSON: {error}"))
}

/// Prepared-host marker required before this action measures or emits.
const BENCH_MODE_VAR: &str = "GF2_BENCH";
/// Private guard for every forced tuning child.
const FRESH_CASE_VAR: &str = "GF2_TUNING_FRESH_CASE";
const FRESH_CASE_VALUE: &str = "child-v1";
const FRESH_RESULT_PREFIX: &str = "GF2_TUNING_RESULT=";
/// Wrapper-overridable mutex path, read only to explain a failed lock probe.
const LOCK_PATH_VAR: &str = "GF2_CCX1_LOCK";
const DEFAULT_EXECUTIONS: u64 = 5;
const DEFAULT_REPETITIONS: u64 = 5;
const DEFAULT_TARGET_MS: u64 = 250;
/// Upper bound on the calibrated call count of one timed window.
const MAX_CALLS: u64 = 1 << 32;
/// Fixture bank depth for the bit-backend arms, matching the sibling harness.
const BIT_FIXTURES: usize = 8;
/// `u64` words per 64-byte cache line on the supported targets.
const WORDS_PER_LINE: usize = 8;
const SEED_ROOT: u64 = 0x5ecc_9bf8_0000_0000;
const GIT_STATUS_ARGS: &[&str] = &["status", "--porcelain", "--untracked-files=all"];
/// `polynomial.karatsuba_min_degree` a child installs to force the schoolbook
/// arm.
///
/// `usize::MAX` is the top of the field's admissible range and means "never
/// take the named path"; no operand degree reaches it, so every product on the
/// grid runs schoolbook. It is an ordinary admissible value rather than a
/// reserved sentinel.
const FORCED_SCHOOLBOOK_MIN_DEGREE: usize = usize::MAX;
/// Profile identifier a child installs, recorded nowhere but its own process.
const FORCED_ARM_PROFILE_ID: &str = "calibration-forced-arm";

/// The prime field every polynomial arm is measured over.
///
/// `Fp<65537>` is `TwoAdicField`, which the NTT, Newton-iteration division and
/// subproduct-tree arms all require, and it is the field the pinned selector
/// non-regression set already measures, so the two receipts describe the same
/// arithmetic.
type F = Fp<65537>;

// ---------------------------------------------------------------------
// Calibrated fields and their grids
// ---------------------------------------------------------------------

/// The five selector fields this sweep measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum CalibratedField {
    SimdMinWords,
    KaratsubaMinDegree,
    KaratsubaMaxOutLen,
    DivRemFastMinLen,
    SubproductMinLen,
}

impl CalibratedField {
    const ALL: [Self; 5] = [
        Self::SimdMinWords,
        Self::KaratsubaMinDegree,
        Self::KaratsubaMaxOutLen,
        Self::DivRemFastMinLen,
        Self::SubproductMinLen,
    ];

    fn family(self) -> &'static str {
        match self {
            Self::SimdMinWords => "bit_backend",
            _ => "polynomial",
        }
    }

    /// The schema key this field's measured value is stated under.
    fn schema_field(self) -> SchemaField {
        SchemaField {
            family: self.family().to_owned(),
            name: self.to_string(),
        }
    }

    /// Whether the field bounds its conservative arm from above.
    ///
    /// `karatsuba_max_out_len` gates `out_len <= t` to the Karatsuba arm, so
    /// its value is the largest grid point below the crossover. Every other
    /// field gates `size >= t` to the asymptotic arm, so its value is the
    /// crossover itself.
    fn is_upper_bound(self) -> bool {
        matches!(self, Self::KaratsubaMaxOutLen)
    }

    /// The conservative table's value for this field.
    fn conservative_default(self) -> usize {
        let profile = &CoreTuning::CONSERVATIVE;
        match self {
            Self::SimdMinWords => profile.bit_backend().simd_min_words(),
            Self::KaratsubaMinDegree => profile.polynomial().karatsuba_min_degree(),
            Self::KaratsubaMaxOutLen => profile.polynomial().karatsuba_max_out_len(),
            Self::DivRemFastMinLen => profile.polynomial().div_rem_fast_min_len(),
            Self::SubproductMinLen => profile.polynomial().subproduct_min_len(),
        }
    }

    fn conservative_arm(self) -> &'static str {
        match self {
            Self::SimdMinWords => "scalar",
            Self::KaratsubaMinDegree => "schoolbook",
            Self::KaratsubaMaxOutLen => "karatsuba",
            Self::DivRemFastMinLen => "div_rem",
            Self::SubproductMinLen => "eval_batch",
        }
    }

    fn asymptotic_arm(self) -> &'static str {
        match self {
            Self::SimdMinWords => "simd",
            Self::KaratsubaMinDegree => "karatsuba",
            Self::KaratsubaMaxOutLen => "mul_ntt",
            Self::DivRemFastMinLen => "div_rem_fast",
            Self::SubproductMinLen => "subproduct_auto",
        }
    }

    fn arm_name(self, arm: Arm) -> &'static str {
        match arm {
            Arm::Conservative => self.conservative_arm(),
            Arm::Asymptotic => self.asymptotic_arm(),
        }
    }

    /// The process each of this field's arms is measured in.
    ///
    /// Every field but `karatsuba_min_degree` reaches both arms through their
    /// own entry points, so both are timed here. That one selects its arm from
    /// the active profile, which resolves once per process, so each of its arms
    /// is timed in a child that installed the profile forcing it.
    fn arm_source(self) -> ArmSource {
        match self {
            Self::KaratsubaMinDegree => ArmSource::ChildProcess,
            _ => ArmSource::InProcess,
        }
    }

    /// The unit the grid points are measured in.
    fn grid_unit(self) -> &'static str {
        match self {
            Self::SimdMinWords => "buffer words",
            Self::KaratsubaMinDegree => "operand degree",
            Self::KaratsubaMaxOutLen => "product length",
            Self::DivRemFastMinLen => "divisor length",
            Self::SubproductMinLen => "coefficients = points",
        }
    }

    /// The size grid this field is swept over.
    ///
    /// Every grid is written around `conservative_default`, so it straddles the
    /// default wherever that default sits, and each is capped where the slower
    /// arm's cost per call would dominate the run without adding evidence about
    /// the crossover.
    ///
    /// `karatsuba_max_out_len` counts product lengths, and a product of two
    /// equal-length operands has odd length `2n - 1`; its grid therefore uses
    /// the odd lengths bracketing the default. It reaches 511 so the sweep
    /// spans the region where `mul_fast`'s two arms are recorded as converging,
    /// and it carries both 127 and 129 so the step across the default's own
    /// boundary is measured directly.
    ///
    /// The two large-operand fields reach two octaves further below their
    /// default than the small-operand ones, because their conservative arms are
    /// quadratic and a crossover several octaves below the default is the
    /// ordinary case for them rather than a surprise.
    fn grid(self) -> Vec<usize> {
        let default = self.conservative_default();
        match self {
            Self::SimdMinWords | Self::KaratsubaMinDegree => vec![
                default / 8,
                default / 4,
                default / 2,
                default - 1,
                default,
                default + 1,
                default * 2,
                default * 4,
                default * 8,
            ],
            Self::KaratsubaMaxOutLen => vec![
                default / 8 - 1,
                default / 4 - 1,
                default / 2 - 1,
                default - 1,
                default + 1,
                default + default / 2 - 1,
                default * 2 - 1,
                default * 3 - 1,
                default * 4 - 1,
            ],
            Self::DivRemFastMinLen | Self::SubproductMinLen => vec![
                default / 32,
                default / 16,
                default / 8,
                default / 4,
                default / 2,
                default - 1,
                default,
                default + 1,
                default * 2,
            ],
        }
    }
}

impl fmt::Display for CalibratedField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SimdMinWords => "simd_min_words",
            Self::KaratsubaMinDegree => "karatsuba_min_degree",
            Self::KaratsubaMaxOutLen => "karatsuba_max_out_len",
            Self::DivRemFastMinLen => "div_rem_fast_min_len",
            Self::SubproductMinLen => "subproduct_min_len",
        })
    }
}

/// One `selectors.<family>.<field>` key of the emitted schema.
///
/// The schema carries many more of these than this sweep measures, and the two
/// sets are compared by value rather than by a maintained list, so a field the
/// sweep does not name is omitted from the emitted document whatever it is.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SchemaField {
    family: String,
    name: String,
}

impl fmt::Display for SchemaField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.family, self.name)
    }
}

/// Which side of a crossover an arm sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum Arm {
    /// The arm the comparison selects below the threshold.
    Conservative,
    /// The arm the comparison selects above the threshold.
    Asymptotic,
}

impl Arm {
    const BOTH: [Self; 2] = [Self::Conservative, Self::Asymptotic];
}

impl fmt::Display for Arm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Conservative => "conservative",
            Self::Asymptotic => "asymptotic",
        })
    }
}

/// Where a field's two arms are timed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArmSource {
    /// Both arms have their own entry point and are timed in this process.
    InProcess,
    /// The arm is selected from the active profile, so each is timed in a
    /// child process that installed the profile forcing it.
    ChildProcess,
}

impl fmt::Display for ArmSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InProcess => "in-process",
            Self::ChildProcess => "child-process",
        })
    }
}

// ---------------------------------------------------------------------
// Command line
// ---------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Protocol {
    executions: u64,
    repetitions: u64,
    target_ms: u64,
}

impl Protocol {
    fn target(&self) -> Duration {
        Duration::from_millis(self.target_ms)
    }

    /// Timed windows recorded per arm at one grid point.
    fn windows(&self) -> usize {
        (self.executions * self.repetitions) as usize
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Mode {
    Calibrate {
        out: PathBuf,
        profile_id: Option<String>,
        lock_wrapper: String,
        receipt: String,
    },
    SelfCheck,
    ListGrid,
    /// Fixed private entry mode; the guarded case arrives canonically on stdin.
    FreshChild,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Args {
    protocol: Protocol,
    mode: Mode,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut executions = DEFAULT_EXECUTIONS;
    let mut repetitions = DEFAULT_REPETITIONS;
    let mut target_ms = DEFAULT_TARGET_MS;
    let mut out: Option<PathBuf> = None;
    let mut profile_id: Option<String> = None;
    let mut lock_wrapper: Option<String> = None;
    let mut receipt: Option<String> = None;
    let mut self_check = false;
    let mut list_grid = false;
    let mut fresh_child = false;
    let mut protocol_override = false;
    let mut iter = args;
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--executions" => {
                executions = parse_value(&mut iter, &arg)?;
                protocol_override = true;
            }
            "--repetitions" => {
                repetitions = parse_value(&mut iter, &arg)?;
                protocol_override = true;
            }
            "--target-ms" => {
                target_ms = parse_value(&mut iter, &arg)?;
                protocol_override = true;
            }
            "--out" => out = Some(PathBuf::from(next_value(&mut iter, &arg)?)),
            "--profile-id" => profile_id = Some(next_value(&mut iter, &arg)?),
            "--lock-wrapper" => lock_wrapper = Some(next_value(&mut iter, &arg)?),
            "--receipt" => receipt = Some(next_value(&mut iter, &arg)?),
            "--self-check" => self_check = true,
            "--list-grid" => list_grid = true,
            "--fresh-tuning-process-child" => {
                if fresh_child {
                    return Err("duplicate --fresh-tuning-process-child".to_owned());
                }
                fresh_child = true;
            }
            "--bench" => {}
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    for (flag, value) in [
        ("--executions", executions),
        ("--repetitions", repetitions),
        ("--target-ms", target_ms),
    ] {
        if value == 0 {
            return Err(format!("{flag} must be positive"));
        }
    }
    if [self_check, list_grid, fresh_child]
        .into_iter()
        .filter(|selected| *selected)
        .count()
        > 1
    {
        return Err(
            "--self-check, --list-grid and --fresh-tuning-process-child are separate modes".into(),
        );
    }
    let protocol = Protocol {
        executions,
        repetitions,
        target_ms,
    };
    let mode = if self_check {
        Mode::SelfCheck
    } else if list_grid {
        Mode::ListGrid
    } else if fresh_child {
        if protocol_override
            || out.is_some()
            || profile_id.is_some()
            || lock_wrapper.is_some()
            || receipt.is_some()
        {
            return Err(
                "the fresh tuning child accepts its complete case only on standard input".into(),
            );
        }
        Mode::FreshChild
    } else {
        Mode::Calibrate {
            out: out.ok_or("--out is required; name a unique absent path under /tmp")?,
            profile_id,
            lock_wrapper: lock_wrapper.ok_or(
                "--lock-wrapper is required; name the wrapper this run was invoked through",
            )?,
            receipt: receipt
                .ok_or("--receipt is required; name the receipt this run will be committed as")?,
        }
    };
    Ok(Args { protocol, mode })
}

fn parse_value<T: std::str::FromStr>(
    iter: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T, String> {
    next_value(iter, flag)?
        .parse()
        .map_err(|_| format!("invalid value for {flag}"))
}

fn next_value(iter: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    iter.next()
        .ok_or_else(|| format!("missing value for {flag}"))
}

/// Resolves a path the caller gave on the command line.
///
/// An absolute path is used as given; a relative one is taken as
/// repository-relative, because `cargo bench` runs this binary with its working
/// directory at the package root while the receipts and profiles it names are
/// written relative to the repository root. This matches the resolution the
/// sibling receipt harness uses, so one command works from where the procedure
/// says to run it.
fn resolve_repository_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    }
}

// ---------------------------------------------------------------------
// Child-process arms
// ---------------------------------------------------------------------

/// What a child process is asked to do with the arm it forces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ChildTask {
    /// Report the arm and the digests, and time nothing.
    Probe,
    /// Time one execution's windows as well.
    Measure { execution: u64 },
}

impl fmt::Display for ChildTask {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Probe => formatter.write_str("probe"),
            Self::Measure { execution } => write!(formatter, "{execution}"),
        }
    }
}

/// One forced arm at one grid point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ChildSpec {
    field: CalibratedField,
    size: usize,
    arm: Arm,
    task: ChildTask,
}

impl fmt::Display for ChildSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}:{}",
            self.field, self.size, self.arm, self.task
        )
    }
}

/// Canonical case sent to one guarded forced-tuning child.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct FreshProcessCase {
    spec: ChildSpec,
    protocol: Protocol,
}

// ---------------------------------------------------------------------
// Observed host facts
// ---------------------------------------------------------------------

/// Everything the emitted provenance records that this run observes rather than
/// takes from its own protocol constants.
#[derive(Clone, Debug)]
struct HostFacts {
    source_revision: GitRevision,
    source_dirty: bool,
    harness: RepoRelPath,
    binary_sha256: Sha256,
    toolchain: String,
    host: String,
    cpu_model: String,
    cpu_features: Vec<String>,
    os_kernel: String,
    governor: String,
    lock_file: String,
    cpu_affinity: String,
}

fn command_output(program: &str, args: &[&str]) -> io::Result<String> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{program} {} failed with {}",
            args.join(" "),
            output.status
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Repository-relative path of this source file.
///
/// Derived from the compiler's own path for this file and the repository root
/// `git` reports, so the recorded harness identity follows the file if it moves
/// instead of restating a path written into the tool. The compiler's path is
/// relative to whichever directory `cargo` invoked it from, so both the
/// repository root and this package's directory are tried and the candidate
/// that names a real file wins; a path that names neither is an error rather
/// than a provenance claim nobody checked.
fn harness_path(repo_root: &Path) -> io::Result<String> {
    let package = Path::new(env!("CARGO_MANIFEST_DIR"))
        .strip_prefix(repo_root)
        .unwrap_or(Path::new(""));
    let candidates = [
        normalize_relative(Path::new(file!())),
        normalize_relative(&package.join(file!())),
    ];
    candidates
        .iter()
        .find(|candidate| repo_root.join(candidate).is_file())
        .cloned()
        .ok_or_else(|| {
            io::Error::other(format!(
                "no candidate harness path among {candidates:?} names a file under {}",
                repo_root.display()
            ))
        })
}

/// Collapses `a/b/../c` to `a/c` so the result passes `RepoRelPath::parse`.
///
/// The compiler's path for this file gains a parent segment when the
/// `#[cfg(test)]` module below is compiled through the integration-test wrapper
/// that includes this file from `tests/`.
fn normalize_relative(path: &Path) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.iter().filter_map(|part| part.to_str()) {
        match part {
            "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// Runtime-detected CPU feature tokens that gate the accelerated kernels.
///
/// The token list is the set `gf2-kernels-simd` probes; membership of the
/// returned vector is decided by this host's own detection at run time.
fn cpu_features() -> Vec<String> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        let detected = [
            ("avx2", std::arch::is_x86_feature_detected!("avx2")),
            ("avx512vl", std::arch::is_x86_feature_detected!("avx512vl")),
            ("fma", std::arch::is_x86_feature_detected!("fma")),
            (
                "pclmulqdq",
                std::arch::is_x86_feature_detected!("pclmulqdq"),
            ),
            ("sse4.1", std::arch::is_x86_feature_detected!("sse4.1")),
            (
                "vpclmulqdq",
                std::arch::is_x86_feature_detected!("vpclmulqdq"),
            ),
        ];
        detected
            .into_iter()
            .filter(|(_, present)| *present)
            .map(|(token, _)| token.to_owned())
            .collect()
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        Vec::new()
    }
}

/// The CPU list this process is allowed to run on, as `taskset` left it.
fn cpu_affinity() -> io::Result<String> {
    let status = fs::read_to_string("/proc/self/status")?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("Cpus_allowed_list:"))
        .map(|value| value.trim().to_owned())
        .ok_or_else(|| io::Error::other("/proc/self/status has no Cpus_allowed_list"))
}

/// The scaling governor of the first CPU this process is pinned to.
fn governor(affinity: &str) -> io::Result<String> {
    let first = affinity
        .split(',')
        .next()
        .and_then(|range| range.split('-').next())
        .and_then(|cpu| cpu.trim().parse::<u32>().ok())
        .ok_or_else(|| io::Error::other(format!("cannot read a CPU number from `{affinity}`")))?;
    Ok(fs::read_to_string(format!(
        "/sys/devices/system/cpu/cpu{first}/cpufreq/scaling_governor"
    ))?
    .trim()
    .to_owned())
}

/// The mutex path of the exclusive `flock` this process inherited, if any.
///
/// `dev/scripts/ccx1-bench-flock.sh` takes the lock with `flock -x` and does
/// not pass `-o`, so the locked descriptor stays open across the exec chain and
/// is inherited by every descendant, this binary included. Matching an open
/// descriptor of ours against the kernel's lock table therefore establishes
/// that the run really is serialised, and yields the mutex path as an observed
/// fact rather than a claim. The lock's owning PID is the wrapper's, not ours,
/// so the match is on the locked file's device and inode.
fn observed_lock_file() -> io::Result<Option<String>> {
    let mut locked: Vec<(u64, u64)> = Vec::new();
    for line in fs::read_to_string("/proc/locks")?.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        // `1: FLOCK  ADVISORY  WRITE 4711 00:2d:112936 0 EOF`
        if fields.len() < 6 || fields[1] != "FLOCK" || fields[3] != "WRITE" {
            continue;
        }
        let mut parts = fields[5].split(':');
        let major = parts.next().and_then(|v| u64::from_str_radix(v, 16).ok());
        let minor = parts.next().and_then(|v| u64::from_str_radix(v, 16).ok());
        let inode = parts.next().and_then(|v| v.parse::<u64>().ok());
        if let (Some(major), Some(minor), Some(inode)) = (major, minor, inode) {
            locked.push((makedev(major, minor), inode));
        }
    }
    for entry in fs::read_dir("/proc/self/fd")? {
        let Ok(target) = fs::read_link(entry?.path()) else {
            continue;
        };
        let Ok(metadata) = fs::metadata(&target) else {
            continue;
        };
        if metadata.is_file() && locked.contains(&(metadata.dev(), metadata.ino())) {
            return Ok(Some(target.to_string_lossy().into_owned()));
        }
    }
    Ok(None)
}

/// Linux `makedev` encoding, matching the `MAJOR:MINOR` pair `/proc/locks`
/// prints against the device id `stat` reports.
fn makedev(major: u64, minor: u64) -> u64 {
    ((major & 0xfff) << 8) | (minor & 0xff) | ((major & !0xfff) << 32) | ((minor & !0xff) << 12)
}

fn collect_host_facts() -> io::Result<HostFacts> {
    let repo_root = PathBuf::from(command_output("git", &["rev-parse", "--show-toplevel"])?);
    let revision = command_output("git", &["rev-parse", "HEAD"])?;
    let binary = env::current_exe()?;
    let digest = command_output("sha256sum", &[binary.to_string_lossy().as_ref()])?;
    let digest = digest
        .split_whitespace()
        .next()
        .ok_or_else(|| io::Error::other("sha256sum printed no digest"))?;
    let affinity = cpu_affinity()?;
    let cpuinfo = fs::read_to_string("/proc/cpuinfo")?;
    let lock_file = observed_lock_file()?.ok_or_else(|| {
        let expected = env::var(LOCK_PATH_VAR).unwrap_or_else(|_| "the wrapper's mutex".to_owned());
        io::Error::other(format!(
            "no inherited exclusive flock: run this action through the repository lock wrapper, \
             which holds {expected} for the whole run"
        ))
    })?;
    Ok(HostFacts {
        source_revision: GitRevision::parse(&revision).map_err(io::Error::other)?,
        source_dirty: !command_output("git", GIT_STATUS_ARGS)?.is_empty(),
        harness: RepoRelPath::parse(&harness_path(&repo_root)?).map_err(io::Error::other)?,
        binary_sha256: Sha256::parse(digest).map_err(io::Error::other)?,
        toolchain: command_output("rustc", &["--version"])?,
        host: command_output("hostname", &[])?,
        cpu_model: cpuinfo
            .lines()
            .find_map(|line| line.strip_prefix("model name\t: "))
            .unwrap_or("unknown")
            .to_owned(),
        cpu_features: cpu_features(),
        os_kernel: command_output("uname", &["-srvmo"])?,
        governor: governor(&affinity)?,
        lock_file,
        cpu_affinity: affinity,
    })
}

/// Formats a UTC instant in the RFC 3339 form `Rfc3339Utc::parse` accepts.
fn rfc3339_utc(instant: SystemTime) -> io::Result<String> {
    let seconds = instant
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    let (year, month, day) = civil_from_days((seconds / 86_400) as i64);
    let time = seconds % 86_400;
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time / 3600,
        (time / 60) % 60,
        time % 60
    ))
}

/// Civil date of the day `days` after 1970-01-01, by the shift-to-March
/// era arithmetic that avoids a month-length table.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

/// `BIT_FIXTURES` buffers of `len` words laid out contiguously, each starting
/// on a 64-byte boundary.
///
/// The alignment is controlled rather than left to the allocator: a 64-byte
/// cache line holds eight `u64`, a wide load that straddles two lines costs
/// materially more than one that does not, and the allocator's address phase
/// differs between processes. An uncontrolled bank would make the two arms of
/// this family measure a property of the heap.
struct BitBank {
    storage: Vec<u64>,
    offset: usize,
    stride: usize,
    len: usize,
}

impl BitBank {
    fn new(len: usize, mut word: impl FnMut(usize, usize) -> u64) -> Self {
        let stride = len.next_multiple_of(WORDS_PER_LINE).max(WORDS_PER_LINE);
        let mut storage = vec![0_u64; BIT_FIXTURES * stride + WORDS_PER_LINE];
        let misalignment = (storage.as_ptr() as usize) % (WORDS_PER_LINE * 8);
        let offset = (WORDS_PER_LINE * 8 - misalignment) % (WORDS_PER_LINE * 8) / 8;
        for bank in 0..BIT_FIXTURES {
            for index in 0..len {
                storage[offset + bank * stride + index] = word(bank, index);
            }
        }
        let bank = Self {
            storage,
            offset,
            stride,
            len,
        };
        assert!(
            bank.is_line_aligned(),
            "bit fixture bank did not land on a cache-line boundary"
        );
        bank
    }

    fn is_line_aligned(&self) -> bool {
        let base = self.storage.as_ptr() as usize + self.offset * 8;
        base.is_multiple_of(WORDS_PER_LINE * 8) && self.stride.is_multiple_of(WORDS_PER_LINE)
    }

    fn start(&self, bank: usize) -> usize {
        self.offset + bank * self.stride
    }

    fn get(&self, bank: usize) -> &[u64] {
        let start = self.start(bank);
        &self.storage[start..start + self.len]
    }

    fn get_mut(&mut self, bank: usize) -> &mut [u64] {
        let start = self.start(bank);
        &mut self.storage[start..start + self.len]
    }
}

/// One grid point's operands, built once and shared by both arms.
enum Fixture {
    Bit {
        dst: BitBank,
        src: BitBank,
    },
    Mul {
        a: FieldPoly<F>,
        b: FieldPoly<F>,
    },
    DivRem {
        dividend: FieldPoly<F>,
        divisor: FieldPoly<F>,
    },
    BatchEval {
        poly: FieldPoly<F>,
        points: Vec<F>,
    },
}

fn seed_for(field: CalibratedField, size: usize, role: u64) -> u64 {
    let mut value = SEED_ROOT ^ role.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for word in [field as usize, size] {
        value ^= word as u64;
        value = value
            .wrapping_mul(0xbf58_476d_1ce4_e5b9)
            .rotate_left(27)
            .wrapping_add(0x94d0_49bb_1331_11eb);
    }
    value ^ (value >> 31)
}

/// Builds a deterministic polynomial holding exactly `len` coefficients.
///
/// Coefficients are drawn from `1..=65536`, so none of them is the zero element
/// of `Fp<65537>` and `FieldPoly::new`'s trailing-zero normalisation cannot
/// shorten the operand. The grid points select their arm from exact
/// coefficient counts, so the assertion is a precondition rather than a
/// diagnostic.
fn make_poly(len: usize, seed: u64) -> FieldPoly<F> {
    let mut rng = Lcg::new(seed);
    let coeffs: Vec<F> = (0..len)
        .map(|_| F::new((rng.next_u64() % 65_536) + 1))
        .collect();
    let poly = FieldPoly::new(coeffs);
    assert_eq!(poly.len(), len, "fixture polynomial lost coefficients");
    poly
}

/// Builds `len` distinct non-zero evaluation points.
fn make_points(len: usize, seed: u64) -> Vec<F> {
    let stride = 1_000_003_u64;
    let modulus_minus_one = 65_536_u64;
    let offset = Lcg::new(seed).next_u64() % modulus_minus_one;
    (0..len)
        .map(|index| F::new((offset + (index as u64).wrapping_mul(stride)) % modulus_minus_one + 1))
        .collect()
}

/// Operand length whose product has length `out_len`, for equal-length
/// operands.
fn operand_len_for_product(out_len: usize) -> usize {
    out_len.div_ceil(2)
}

fn build_fixture(field: CalibratedField, size: usize) -> Fixture {
    match field {
        CalibratedField::SimdMinWords => {
            let fill = |role: u64| {
                let mut rngs: Vec<Lcg> = (0..BIT_FIXTURES)
                    .map(|bank| Lcg::new(seed_for(field, size, role + bank as u64)))
                    .collect();
                BitBank::new(size, move |bank, _| rngs[bank].next_u64())
            };
            Fixture::Bit {
                dst: fill(0xD000_0000),
                src: fill(0xA000_0000),
            }
        }
        CalibratedField::KaratsubaMinDegree => Fixture::Mul {
            a: make_poly(size + 1, seed_for(field, size, 0xA)),
            b: make_poly(size + 1, seed_for(field, size, 0xB)),
        },
        CalibratedField::KaratsubaMaxOutLen => {
            let len = operand_len_for_product(size);
            Fixture::Mul {
                a: make_poly(len, seed_for(field, size, 0xA)),
                b: make_poly(len, seed_for(field, size, 0xB)),
            }
        }
        CalibratedField::DivRemFastMinLen => Fixture::DivRem {
            dividend: make_poly(2 * size, seed_for(field, size, 0xD)),
            divisor: make_poly(size, seed_for(field, size, 0xE)),
        },
        CalibratedField::SubproductMinLen => Fixture::BatchEval {
            poly: make_poly(size, seed_for(field, size, 0xC)),
            points: make_points(size, seed_for(field, size, 0xF)),
        },
    }
}

/// FNV-1a over the little-endian bytes of each word.
fn digest_words(words: impl Iterator<Item = u64>) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for word in words {
        for byte in word.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

/// Length-sensitive digest of a polynomial's coefficients.
///
/// The parent and its children build their fixtures separately from the same
/// deterministic seed and compute their products on different arms. Comparing
/// digests establishes that they hold the same operands and reached the same
/// product without either process shipping a coefficient vector to the other.
fn poly_digest(poly: &FieldPoly<F>) -> u64 {
    digest_words(
        std::iter::once(poly.len() as u64)
            .chain(poly.iter().map(|coefficient| coefficient.value())),
    )
}

/// Digest of an ordered operand pair.
fn operand_digest(lhs: &FieldPoly<F>, rhs: &FieldPoly<F>) -> u64 {
    digest_words([poly_digest(lhs), poly_digest(rhs)].into_iter())
}

/// Asserts that the two arms of `field` agree at `size` before either is timed.
///
/// A crossover between arms that compute different results is not a crossover,
/// so this runs at every grid point where both arms exist.
fn equivalence_probe(field: CalibratedField, size: usize, fixture: &Fixture, protocol: &Protocol) {
    match (field, fixture) {
        (CalibratedField::SimdMinWords, Fixture::Bit { dst, src }) => {
            assert!(dst.is_line_aligned() && src.is_line_aligned());
            let Some(simd) = simd_backend() else {
                return;
            };
            let mut scalar_out = dst.get(0).to_vec();
            let mut simd_out = scalar_out.clone();
            ScalarBackend.xor(&mut scalar_out, src.get(1));
            simd.xor(&mut simd_out, src.get(1));
            assert_eq!(
                scalar_out, simd_out,
                "bit-backend arms disagree at {size} words"
            );
        }
        (CalibratedField::KaratsubaMinDegree, Fixture::Mul { a, b }) => {
            assert_eq!(a.len(), size + 1);
            assert_eq!(b.len(), size + 1);
            // Neither arm is callable here, so the probe runs one child per
            // arm. Each reports the arm the production selector picks under
            // the profile it installed and a digest of the operands it built
            // from the same seed; the two products are then compared across
            // processes.
            let operands = operand_digest(a, b);
            let products: Vec<u64> = Arm::BOTH
                .into_iter()
                .map(|arm| {
                    let spec = ChildSpec {
                        field,
                        size,
                        arm,
                        task: ChildTask::Probe,
                    };
                    checked_child_report(spec, operands, protocol).product
                })
                .collect();
            assert_eq!(
                products[0], products[1],
                "multiplication arms disagree at operand degree {size}"
            );
        }
        (CalibratedField::KaratsubaMaxOutLen, Fixture::Mul { a, b }) => {
            assert_eq!(
                a.len() + b.len() - 1,
                size,
                "fixture product length is not the grid point"
            );
            assert_eq!(
                a.mul(b),
                a.mul_ntt(b),
                "multiplication arms disagree at product length {size}"
            );
        }
        (CalibratedField::DivRemFastMinLen, Fixture::DivRem { dividend, divisor }) => {
            assert_eq!(divisor.len(), size);
            assert_eq!(
                dividend.div_rem(divisor),
                dividend.div_rem_fast(divisor),
                "division arms disagree at divisor length {size}"
            );
        }
        (CalibratedField::SubproductMinLen, Fixture::BatchEval { poly, points }) => {
            assert_eq!(poly.len(), size);
            assert_eq!(points.len(), size);
            assert_eq!(
                poly.eval_batch(points),
                batch_evaluate_subproduct_auto(poly, points),
                "batch-evaluation arms disagree at {size} points"
            );
        }
        _ => panic!("fixture shape does not match {field} at {size}"),
    }
}

/// The detected SIMD logical backend, or `None` when the build or the host has
/// no second arm for the bit-backend family.
fn simd_backend() -> Option<&'static dyn Backend> {
    #[cfg(feature = "simd")]
    {
        gf2_core::kernels::simd::maybe_simd().map(|backend| backend as &'static dyn Backend)
    }
    #[cfg(not(feature = "simd"))]
    {
        None
    }
}

// ---------------------------------------------------------------------
// Timing
// ---------------------------------------------------------------------

/// Destination and source bank indices for call number `index`, offset so a
/// call never reads and writes the same buffer.
fn bank_indices(index: usize) -> (usize, usize) {
    (
        index & (BIT_FIXTURES - 1),
        index.wrapping_add(3) & (BIT_FIXTURES - 1),
    )
}

/// Runs one execution of `arm` at one grid point, returning each timed
/// window's nanoseconds per call.
///
/// Returns `None` when the arm has no entry point at this grid point on this
/// build, which is the bit-backend family without a detected SIMD backend.
fn measure_arm_execution(
    field: CalibratedField,
    size: usize,
    arm: Arm,
    fixture: &mut Fixture,
    protocol: &Protocol,
    execution: u64,
) -> Option<Vec<f64>> {
    match (field, arm, fixture) {
        (CalibratedField::SimdMinWords, Arm::Conservative, Fixture::Bit { dst, src }) => {
            Some(execution_windows(protocol, execution, |index| {
                let (i, j) = bank_indices(index);
                let src_bank: &[u64] = black_box(src.get(j));
                let dst_bank: &mut [u64] = black_box(dst.get_mut(i));
                ScalarBackend.xor(dst_bank, src_bank);
                black_box(dst_bank);
            }))
        }
        (CalibratedField::SimdMinWords, Arm::Asymptotic, Fixture::Bit { dst, src }) => {
            // The concrete backend keeps the timed loop's call shape the same
            // as production's, which resolves a function pointer rather than a
            // trait object.
            #[cfg(feature = "simd")]
            {
                let simd = gf2_core::kernels::simd::maybe_simd()?;
                Some(execution_windows(protocol, execution, |index| {
                    let (i, j) = bank_indices(index);
                    let src_bank: &[u64] = black_box(src.get(j));
                    let dst_bank: &mut [u64] = black_box(dst.get_mut(i));
                    simd.xor(dst_bank, src_bank);
                    black_box(dst_bank);
                }))
            }
            #[cfg(not(feature = "simd"))]
            {
                let _ = (dst, src, protocol, execution);
                None
            }
        }
        (CalibratedField::KaratsubaMinDegree, arm, Fixture::Mul { a, b }) => {
            // `FieldPoly::mul` is the only entry to this crossover and resolves
            // the arm from the active profile, which resolves once per process.
            // A child installs the profile forcing `arm` and times the same
            // public entry there.
            let spec = ChildSpec {
                field,
                size,
                arm,
                task: ChildTask::Measure { execution },
            };
            Some(checked_child_report(spec, operand_digest(a, b), protocol).rates)
        }
        (CalibratedField::KaratsubaMaxOutLen, Arm::Conservative, Fixture::Mul { a, b }) => {
            Some(execution_windows(protocol, execution, |_| {
                black_box(black_box(&*a).mul(black_box(&*b)));
            }))
        }
        (CalibratedField::KaratsubaMaxOutLen, Arm::Asymptotic, Fixture::Mul { a, b }) => {
            Some(execution_windows(protocol, execution, |_| {
                black_box(black_box(&*a).mul_ntt(black_box(&*b)));
            }))
        }
        (
            CalibratedField::DivRemFastMinLen,
            Arm::Conservative,
            Fixture::DivRem { dividend, divisor },
        ) => Some(execution_windows(protocol, execution, |_| {
            black_box(black_box(&*dividend).div_rem(black_box(&*divisor)));
        })),
        (
            CalibratedField::DivRemFastMinLen,
            Arm::Asymptotic,
            Fixture::DivRem { dividend, divisor },
        ) => Some(execution_windows(protocol, execution, |_| {
            black_box(black_box(&*dividend).div_rem_fast(black_box(&*divisor)));
        })),
        (
            CalibratedField::SubproductMinLen,
            Arm::Conservative,
            Fixture::BatchEval { poly, points },
        ) => Some(execution_windows(protocol, execution, |_| {
            black_box(black_box(&*poly).eval_batch(black_box(&points[..])));
        })),
        (
            CalibratedField::SubproductMinLen,
            Arm::Asymptotic,
            Fixture::BatchEval { poly, points },
        ) => Some(execution_windows(protocol, execution, |_| {
            black_box(batch_evaluate_subproduct_auto(
                black_box(&*poly),
                black_box(&points[..]),
            ));
        })),
        _ => panic!("fixture shape does not match {field} at {size}"),
    }
}

/// Calibrates one call count against the target duration, then records
/// `repetitions` timed windows of exactly that many calls.
fn execution_windows(protocol: &Protocol, execution: u64, mut body: impl FnMut(usize)) -> Vec<f64> {
    let calls = calibrated_calls(protocol.target(), &mut body);
    let mut rates = Vec::with_capacity(protocol.repetitions as usize);
    for repetition in 0..protocol.repetitions {
        let start_index =
            ((execution * protocol.repetitions + repetition) as usize) & (BIT_FIXTURES - 1);
        let elapsed = time_calls(calls, start_index, &mut body);
        rates.push(elapsed.as_nanos() as f64 / calls as f64);
    }
    rates
}

fn time_calls(calls: u64, start_index: usize, mut body: impl FnMut(usize)) -> Duration {
    let start = Instant::now();
    for call in 0..calls {
        body(start_index.wrapping_add(call as usize));
    }
    start.elapsed()
}

fn calibrated_calls(target: Duration, mut call: impl FnMut(usize)) -> u64 {
    let probe_target = target.min(Duration::from_millis(20));
    let mut calls = 1_u64;
    loop {
        let start = Instant::now();
        for index in 0..calls {
            call((index as usize) & (BIT_FIXTURES - 1));
        }
        let elapsed = start.elapsed();
        if elapsed >= probe_target || calls >= MAX_CALLS {
            let elapsed_ns = elapsed.as_nanos().max(1);
            let wanted = target.as_nanos().saturating_mul(calls as u128) / elapsed_ns;
            return wanted.clamp(1, MAX_CALLS as u128) as u64;
        }
        calls = calls.saturating_mul(2).min(MAX_CALLS);
    }
}

// ---------------------------------------------------------------------
// Forcing an arm in a child process
// ---------------------------------------------------------------------

/// What a child process reports back on its standard output.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ChildReport {
    /// Arm the production selector picked under the installed profile.
    route: String,
    /// Digest of the operands the child built.
    operands: u64,
    /// Digest of the product the child's arm computed.
    product: u64,
    /// Nanoseconds per call of each timed window, empty for a probe.
    rates: Vec<f64>,
}

/// The `karatsuba_min_degree` a child installs to reach `arm` at `size`.
///
/// The Karatsuba arm forces the grid point itself, which is the smallest value
/// that still routes operands of that degree to Karatsuba. `mul_karatsuba_raw`
/// recurses on the same value, so the recursion splits once and its
/// sub-operands, at about half the degree, fall to the schoolbook base case —
/// the algorithm the dispatcher runs when `karatsuba_min_degree` is set to this
/// grid point. Forcing the bottom of the admissible range instead would time a
/// recursion carried to degree 0, which no threshold produces.
///
/// The schoolbook arm forces the top of the range, where no grid point routes
/// to Karatsuba at all.
fn forced_karatsuba_min_degree(arm: Arm, size: usize) -> usize {
    match arm {
        Arm::Conservative => FORCED_SCHOOLBOOK_MIN_DEGREE,
        Arm::Asymptotic => size,
    }
}

/// Installs the profile that forces one arm of the multiplication dispatcher.
///
/// Only `karatsuba_min_degree` moves; every other selector keeps its
/// conservative value, so the child differs from an ordinary process in exactly
/// the one comparison under study. The installed value is read back through
/// `tuning::active`, because an install that silently lost a race would leave
/// the child measuring the conservative default and reporting it as an arm.
fn install_forced_profile(karatsuba_min_degree: usize) -> Result<(), String> {
    let inherited = &CoreTuning::CONSERVATIVE;
    let conservative = inherited.polynomial();
    let polynomial = PolynomialSelectors::try_new(
        karatsuba_min_degree,
        conservative.karatsuba_max_out_len(),
        conservative.div_rem_fast_min_len(),
        conservative.subproduct_min_len(),
        conservative.interpolate_fast_min_points(),
    )
    .map_err(|error| {
        format!("karatsuba_min_degree {karatsuba_min_degree} is inadmissible: {error}")
    })?;
    let id = ProfileId::parse(FORCED_ARM_PROFILE_ID)
        .map_err(|error| format!("`{FORCED_ARM_PROFILE_ID}` is not a profile id: {error}"))?;
    let section = CoreTuning::from_selectors(CoreSelectors {
        polynomial,
        ..CoreSelectors::CONSERVATIVE
    });
    let profile =
        PreparedEnvelope::compiled(id.clone(), CompiledProfileProvenance { artifact_id: id })
            .insert(section)
            .map_err(|error| format!("the arm-forcing section does not prepare: {error}"))?
            .build()
            .map_err(|error| format!("the arm-forcing envelope does not build: {error}"))?;
    tuning::install(profile)
        .map_err(|error| format!("the arm-forcing profile was not installed: {error}"))?;
    let active_tuning = tuning::active();
    if !matches!(
        active_tuning.resolution,
        tuning::SectionResolution::Installed { .. }
    ) {
        return Err(format!(
            "the arm-forcing core section did not resolve as Installed: {:?}",
            active_tuning.resolution
        ));
    }
    let active = active_tuning.polynomial().karatsuba_min_degree();
    if active != karatsuba_min_degree {
        return Err(format!(
            "the active karatsuba_min_degree is {active}, not the forced {karatsuba_min_degree}"
        ));
    }
    Ok(())
}

/// Runs one child task and returns its structured report.
///
/// The profile is installed before anything else touches a selection boundary,
/// so `tuning::install` cannot fail on an already-resolved profile. The arm is
/// then read back from the production selector rather than assumed from the
/// value installed.
fn run_child(spec: ChildSpec, protocol: &Protocol) -> Result<ChildReport, String> {
    if spec.field.arm_source() != ArmSource::ChildProcess {
        return Err(format!(
            "{} reaches both arms in one process and needs no child",
            spec.field
        ));
    }
    install_forced_profile(forced_karatsuba_min_degree(spec.arm, spec.size))?;

    let degree = spec.size;
    let route = match mul_route(degree, degree) {
        MulRoute::Schoolbook => "schoolbook",
        MulRoute::Karatsuba => "karatsuba",
    }
    .to_owned();
    let Fixture::Mul { a, b } = build_fixture(spec.field, spec.size) else {
        return Err(format!("{} has no multiplication fixture", spec.field));
    };
    let operands = operand_digest(&a, &b);
    let product = poly_digest(&a.mul(&b));
    let rates = match spec.task {
        ChildTask::Probe => Vec::new(),
        ChildTask::Measure { execution } => execution_windows(protocol, execution, |_| {
            black_box(black_box(&a).mul(black_box(&b)));
        }),
    };
    Ok(ChildReport {
        route,
        operands,
        product,
        rates,
    })
}

/// Re-executes this binary for one guarded forced-tuning case.
fn fresh_tuning_process(case: FreshProcessCase) -> Result<ChildReport, String> {
    if env::var(BENCH_MODE_VAR).as_deref() != Ok("1") {
        return Err(format!(
            "forced calibration children require {BENCH_MODE_VAR}=1 in the parent"
        ));
    }
    let executable =
        env::current_exe().map_err(|error| format!("this binary has no path: {error}"))?;
    let input = serde_json::to_string(&case)
        .map_err(|error| format!("cannot encode the fresh tuning case: {error}"))?;
    let mut child = Command::new(&executable)
        .arg("--fresh-tuning-process-child")
        .env(FRESH_CASE_VAR, FRESH_CASE_VALUE)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            format!(
                "cannot run {} for {}: {error}",
                executable.display(),
                case.spec
            )
        })?;
    child
        .stdin
        .take()
        .ok_or("the fresh tuning child has no standard input")?
        .write_all(input.as_bytes())
        .map_err(|error| format!("cannot write the fresh tuning case: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("cannot wait for the fresh tuning child: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "the child for {} exited with {}\nstdout:\n{}\nstderr:\n{}",
            case.spec,
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| "the fresh tuning child emitted non-UTF-8 output".to_owned())?;
    parse_child_report(&stdout)
}

fn decode_fresh_case(input: &str) -> Result<FreshProcessCase, String> {
    let case: FreshProcessCase = serde_json::from_str(input)
        .map_err(|error| format!("fresh tuning stdin is not one valid case: {error}"))?;
    let canonical = serde_json::to_string(&case)
        .map_err(|error| format!("cannot re-encode the fresh tuning case: {error}"))?;
    if input != canonical {
        return Err("fresh tuning stdin is not canonical compact JSON".to_owned());
    }
    Ok(case)
}

fn read_fresh_case() -> Result<FreshProcessCase, String> {
    match env::var(FRESH_CASE_VAR) {
        Ok(value) if value == FRESH_CASE_VALUE => {}
        Ok(value) => return Err(format!("invalid fresh tuning sentinel {value:?}")),
        Err(env::VarError::NotPresent) => {
            return Err("fresh tuning child sentinel is absent".to_owned())
        }
        Err(error) => return Err(format!("cannot read fresh tuning sentinel: {error}")),
    }
    let mut input = String::new();
    io::Read::read_to_string(&mut io::stdin(), &mut input)
        .map_err(|error| format!("cannot read fresh tuning stdin: {error}"))?;
    decode_fresh_case(&input)
}

fn run_fresh_child() -> Result<(), String> {
    let case = read_fresh_case()?;
    let report = run_child(case.spec, &case.protocol)?;
    let result = serde_json::to_string(&report)
        .map_err(|error| format!("cannot encode the fresh tuning result: {error}"))?;
    println!("{FRESH_RESULT_PREFIX}{result}");
    Ok(())
}

/// Reads the one guarded structured result from a child's standard output.
fn parse_child_report(text: &str) -> Result<ChildReport, String> {
    let lines: Vec<&str> = text.lines().collect();
    let [line] = lines.as_slice() else {
        return Err(format!(
            "the child emitted {} lines rather than one structured result",
            lines.len()
        ));
    };
    let payload = line
        .strip_prefix(FRESH_RESULT_PREFIX)
        .ok_or("the child result lacks the GF2_TUNING_RESULT= prefix")?;
    let report: ChildReport = serde_json::from_str(payload)
        .map_err(|error| format!("the child result is not valid JSON: {error}"))?;
    let canonical = serde_json::to_string(&report)
        .map_err(|error| format!("cannot re-encode the child result: {error}"))?;
    if payload != canonical {
        return Err("the child result is not canonical compact JSON".to_owned());
    }
    Ok(report)
}

/// Checks a child's report against what the parent asked for and holds.
fn verify_child_report(
    spec: ChildSpec,
    operands: u64,
    protocol: &Protocol,
    report: &ChildReport,
) -> Result<(), String> {
    let expected_arm = spec.field.arm_name(spec.arm);
    if report.route != expected_arm {
        return Err(format!(
            "the child for {spec} forced karatsuba_min_degree={} and the production selector then \
             picked the `{}` arm rather than `{expected_arm}`",
            forced_karatsuba_min_degree(spec.arm, spec.size),
            report.route
        ));
    }
    if report.operands != operands {
        return Err(format!(
            "the child for {spec} built operands digesting to {} against this process's {operands}",
            report.operands
        ));
    }
    let expected_windows = match spec.task {
        ChildTask::Probe => 0,
        ChildTask::Measure { .. } => protocol.repetitions as usize,
    };
    if report.rates.len() != expected_windows {
        return Err(format!(
            "the child for {spec} timed {} windows rather than {expected_windows}",
            report.rates.len()
        ));
    }
    Ok(())
}

/// One child's verified report, or an abort.
///
/// A child that took the other arm, built other operands, or timed the wrong
/// number of windows has not measured what the sweep asked for, and a sweep
/// that proceeded past it would report a crossover between two runs of the same
/// arm. That is the same abort discipline [`equivalence_probe`] applies to the
/// in-process families.
fn checked_child_report(spec: ChildSpec, operands: u64, protocol: &Protocol) -> ChildReport {
    let case = FreshProcessCase {
        spec,
        protocol: protocol.clone(),
    };
    let report = fresh_tuning_process(case).and_then(|report| {
        verify_child_report(spec, operands, protocol, &report)?;
        Ok(report)
    });
    report.unwrap_or_else(|error| panic!("{error}"))
}

// ---------------------------------------------------------------------
// Statistics and the selection rule
// ---------------------------------------------------------------------

/// One arm's timed windows at one grid point, reduced to the statistics the
/// selection rule reads.
#[derive(Clone, Debug, PartialEq)]
struct ArmStat {
    median: f64,
    /// Interquartile range relative to the median.
    spread: f64,
    windows: usize,
}

impl ArmStat {
    fn from_rates(rates: &[f64]) -> Self {
        let mut sorted = rates.to_vec();
        sorted.sort_by(f64::total_cmp);
        let median = quantile(&sorted, 0.5);
        let spread = if median > 0.0 {
            (quantile(&sorted, 0.75) - quantile(&sorted, 0.25)) / median
        } else {
            0.0
        };
        Self {
            median,
            spread,
            windows: sorted.len(),
        }
    }
}

/// Nearest-rank quantile of an ascending slice, averaging the two central
/// samples at the median of an even-length sample.
fn quantile(sorted: &[f64], fraction: f64) -> f64 {
    assert!(!sorted.is_empty(), "quantile of an empty sample");
    if fraction == 0.5 && sorted.len().is_multiple_of(2) {
        let upper = sorted.len() / 2;
        return (sorted[upper - 1] + sorted[upper]) / 2.0;
    }
    let rank = (fraction * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

/// One grid point of one field's sweep.
#[derive(Clone, Debug, PartialEq)]
struct GridPoint {
    size: usize,
    conservative: Option<ArmStat>,
    asymptotic: Option<ArmStat>,
}

impl GridPoint {
    /// The noise band and the asymptotic arm's relative margin over the other,
    /// or `None` when the point does not offer both arms.
    fn comparison(&self) -> Option<(f64, f64)> {
        let (conservative, asymptotic) = (self.conservative.as_ref()?, self.asymptotic.as_ref()?);
        let band = conservative.spread.max(asymptotic.spread);
        let margin = (conservative.median - asymptotic.median) / conservative.median;
        Some((band, margin))
    }

    /// Whether the asymptotic arm beats the other by more than the noise band.
    fn asymptotic_wins(&self) -> Option<bool> {
        self.comparison().map(|(band, margin)| margin > band)
    }
}

/// Why a field kept its conservative default.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Fallback {
    /// No grid point offered both arms.
    NoComparableGridPoint,
    /// Both arms were measured, and the asymptotic one never won.
    NoGridPointWins,
    /// The asymptotic arm won at `first_win` and lost again at `later_loss`.
    NonMonotone { first_win: usize, later_loss: usize },
}

impl fmt::Display for Fallback {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoComparableGridPoint => {
                formatter.write_str("no grid point offers both arms on this build")
            }
            Self::NoGridPointWins => {
                formatter.write_str("the asymptotic arm never beats the noise band")
            }
            Self::NonMonotone {
                first_win,
                later_loss,
            } => write!(
                formatter,
                "non-monotone: the asymptotic arm wins at {first_win} and loses again at {later_loss}"
            ),
        }
    }
}

/// What the sweep concluded for one field.
#[derive(Clone, Debug, PartialEq)]
enum Selection {
    /// A monotone crossover was measured.
    Crossover {
        value: usize,
        crossover: usize,
        band: f64,
        margin: f64,
    },
    /// The conservative default stands.
    KeptDefault { value: usize, reason: Fallback },
}

impl Selection {
    fn value(&self) -> usize {
        match self {
            Self::Crossover { value, .. } | Self::KeptDefault { value, .. } => *value,
        }
    }
}

/// One field's whole sweep.
#[derive(Clone, Debug, PartialEq)]
struct FieldSweep {
    field: CalibratedField,
    points: Vec<GridPoint>,
    selection: Selection,
}

/// Applies the selection rule to a measured grid.
///
/// The crossover is the smallest grid point at which the asymptotic arm wins,
/// and it counts only when the asymptotic arm also wins at every larger grid
/// point. A grid point that offers only one arm can neither establish nor
/// continue a crossover, so it breaks monotonicity exactly as a loss does.
fn select(field: CalibratedField, points: &[GridPoint]) -> Selection {
    let default = field.conservative_default();
    if points.iter().all(|point| point.comparison().is_none()) {
        return Selection::KeptDefault {
            value: default,
            reason: Fallback::NoComparableGridPoint,
        };
    }
    let Some(first) = points
        .iter()
        .position(|point| point.asymptotic_wins() == Some(true))
    else {
        return Selection::KeptDefault {
            value: default,
            reason: Fallback::NoGridPointWins,
        };
    };
    if let Some(later) = points[first + 1..]
        .iter()
        .find(|point| point.asymptotic_wins() != Some(true))
    {
        return Selection::KeptDefault {
            value: default,
            reason: Fallback::NonMonotone {
                first_win: points[first].size,
                later_loss: later.size,
            },
        };
    }
    let (band, margin) = points[first]
        .comparison()
        .expect("a winning grid point has both arms");
    let crossover = points[first].size;
    // A `_max_` field bounds the conservative arm from above, so its value is
    // the largest measured grid point that still belongs to that arm. Below the
    // first grid point there is none, and zero is admissible for the one field
    // this branch serves.
    let value = if field.is_upper_bound() {
        if first == 0 {
            0
        } else {
            points[first - 1].size
        }
    } else {
        crossover
    };
    Selection::Crossover {
        value,
        crossover,
        band,
        margin,
    }
}

/// The five swept values, named one per field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectedValues {
    simd_min_words: usize,
    karatsuba_min_degree: usize,
    karatsuba_max_out_len: usize,
    div_rem_fast_min_len: usize,
    subproduct_min_len: usize,
}

impl SelectedValues {
    fn from_sweeps(sweeps: &[FieldSweep]) -> Self {
        let value_of = |field: CalibratedField| {
            sweeps
                .iter()
                .find(|sweep| sweep.field == field)
                .map(|sweep| sweep.selection.value())
                .unwrap_or_else(|| field.conservative_default())
        };
        Self {
            simd_min_words: value_of(CalibratedField::SimdMinWords),
            karatsuba_min_degree: value_of(CalibratedField::KaratsubaMinDegree),
            karatsuba_max_out_len: value_of(CalibratedField::KaratsubaMaxOutLen),
            div_rem_fast_min_len: value_of(CalibratedField::DivRemFastMinLen),
            subproduct_min_len: value_of(CalibratedField::SubproductMinLen),
        }
    }
}

/// Builds the profile from the swept values.
///
/// `PolynomialSelectors::try_new` takes five consecutive `usize` parameters, so
/// a transposition among the polynomial thresholds would compile. Each value is
/// bound to a local named for its own field immediately before the call, which
/// puts the argument order and the field names on one screen.
fn build_profile(
    id: ProfileId,
    measurement: MeasurementProvenance,
    assembled_at: Rfc3339Utc,
    selected: &SelectedValues,
) -> Result<ProducedCoreProfile, String> {
    let simd_min_words: usize = selected.simd_min_words;
    let karatsuba_min_degree: usize = selected.karatsuba_min_degree;
    let karatsuba_max_out_len: usize = selected.karatsuba_max_out_len;
    let div_rem_fast_min_len: usize = selected.div_rem_fast_min_len;
    let subproduct_min_len: usize = selected.subproduct_min_len;
    let interpolate_fast_min_points: usize = CoreTuning::CONSERVATIVE
        .polynomial()
        .interpolate_fast_min_points();

    let bit_backend =
        BitBackendSelectors::try_new(simd_min_words).map_err(|error| error.to_string())?;
    let polynomial = PolynomialSelectors::try_new(
        karatsuba_min_degree,
        karatsuba_max_out_len,
        div_rem_fast_min_len,
        subproduct_min_len,
        interpolate_fast_min_points,
    )
    .map_err(|error| error.to_string())?;
    let assembly = match &measurement {
        MeasurementProvenance::Calibrated {
            source_revision,
            source_dirty,
            harness,
            binary_sha256,
            ..
        } => AssemblyProvenance {
            assembled_at,
            source_revision: source_revision.clone(),
            source_dirty: *source_dirty,
            tool: harness.clone(),
            tool_sha256: binary_sha256.clone(),
        },
        MeasurementProvenance::Inherited => {
            return Err("the measurement harness cannot emit inherited evidence".to_owned())
        }
    };
    Ok(ProducedCoreProfile {
        id,
        measurement,
        assembly,
        section: CoreTuning::from_selectors(CoreSelectors {
            bit_backend,
            polynomial,
            ..CoreSelectors::CONSERVATIVE
        }),
    })
}

// ---------------------------------------------------------------------
// Reporting and output
// ---------------------------------------------------------------------

fn print_grid() {
    println!("field\tfamily\tdefault\tunit\tconservative_arm\tasymptotic_arm\tarm_source\tgrid");
    for field in CalibratedField::ALL {
        let grid: Vec<String> = field.grid().iter().map(usize::to_string).collect();
        println!(
            "{field}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            field.family(),
            field.conservative_default(),
            field.grid_unit(),
            field.conservative_arm(),
            field.asymptotic_arm(),
            field.arm_source(),
            grid.join(",")
        );
    }
}

fn print_protocol(protocol: &Protocol) {
    println!(
        "protocol: harness_schema={} executions={} repetitions={} target_ms={} windows_per_arm={}",
        CoreTuningCodec::HARNESS_SCHEMA,
        protocol.executions,
        protocol.repetitions,
        protocol.target_ms,
        protocol.windows()
    );
}

fn print_host_facts(facts: &HostFacts) {
    println!("host: {}", facts.host);
    println!("cpu_model: {}", facts.cpu_model);
    println!("cpu_features: {}", facts.cpu_features.join(","));
    println!("os_kernel: {}", facts.os_kernel);
    println!("governor: {}", facts.governor);
    println!("cpu_affinity: {}", facts.cpu_affinity);
    println!(
        "lock_file: {} (observed on an inherited descriptor)",
        facts.lock_file
    );
    println!(
        "source_revision: {} source_dirty={}",
        facts.source_revision.as_str(),
        facts.source_dirty
    );
    println!("harness: {}", facts.harness.as_str());
    println!("binary_sha256: {}", facts.binary_sha256.as_str());
    println!("toolchain: {}", facts.toolchain);
    println!(
        "simd_backend: {}",
        simd_backend().map_or("none", Backend::name)
    );
}

fn print_sweep(sweep: &FieldSweep) {
    let field = sweep.field;
    println!(
        "\n{field} ({}): default {} in {}; arms {} vs {}",
        field.family(),
        field.conservative_default(),
        field.grid_unit(),
        field.conservative_arm(),
        field.asymptotic_arm()
    );
    if field.arm_source() == ArmSource::ChildProcess {
        println!(
            "each arm measured in a child process installing \
             polynomial.karatsuba_min_degree={} for {}, and the grid point itself for {}, so the \
             {} arm splits once at that degree over schoolbook base cases",
            FORCED_SCHOOLBOOK_MIN_DEGREE,
            field.conservative_arm(),
            field.asymptotic_arm(),
            field.asymptotic_arm()
        );
    }
    println!(
        "size\t{}_ns\tspread\t{}_ns\tspread\twindows\tmargin\tband\tverdict",
        field.conservative_arm(),
        field.asymptotic_arm()
    );
    for point in &sweep.points {
        let render = |stat: &Option<ArmStat>| match stat {
            Some(stat) => (format!("{:.3}", stat.median), format!("{:.4}", stat.spread)),
            None => ("-".to_owned(), "-".to_owned()),
        };
        let (conservative_ns, conservative_spread) = render(&point.conservative);
        let (asymptotic_ns, asymptotic_spread) = render(&point.asymptotic);
        let windows = point
            .conservative
            .as_ref()
            .or(point.asymptotic.as_ref())
            .map_or(0, |stat| stat.windows);
        let (margin, band, verdict) = match point.comparison() {
            Some((band, margin)) => (
                format!("{margin:.4}"),
                format!("{band:.4}"),
                if margin > band {
                    field.asymptotic_arm()
                } else {
                    field.conservative_arm()
                },
            ),
            None => ("-".to_owned(), "-".to_owned(), "no comparison"),
        };
        println!(
            "{}\t{conservative_ns}\t{conservative_spread}\t{asymptotic_ns}\t{asymptotic_spread}\t{windows}\t{margin}\t{band}\t{verdict}",
            point.size
        );
    }
    match &sweep.selection {
        Selection::Crossover {
            value,
            crossover,
            band,
            margin,
        } => println!(
            "selected: {field}={value} from crossover at {crossover} (margin {margin:.4} > band {band:.4})"
        ),
        Selection::KeptDefault { value, reason } => {
            println!("selected: {field}={value} (conservative default kept: {reason})");
        }
    }
}

/// Prints the omission set with each field's inherited value and why it is
/// omitted, which is the inventory the receipt records.
///
/// The inherited values are read out of the conservative table's own
/// serialization, so the report states what the loader will resolve an absent
/// key to rather than a figure written into this tool.
fn print_omitted(omitted: &[SchemaField], sweeps: &[FieldSweep]) -> Result<(), String> {
    let conservative = CoreTuningCodec::encode_body(&CoreTuning::CONSERVATIVE)
        .map_err(|error| format!("the conservative section does not encode: {error}"))?;
    let conservative = serde_json::to_value(conservative)
        .map_err(|error| format!("the conservative section is not JSON: {error}"))?;
    let uncomparable: Vec<SchemaField> = uncalibrated_fields(sweeps)
        .into_iter()
        .map(CalibratedField::schema_field)
        .collect();
    println!(
        "\nomitted ({}): the emitted document states no value for these schema fields, and the \
         loader resolves each absent key to the inherited conservative default",
        omitted.len()
    );
    println!("family\tfield\tinherited\treason");
    for field in omitted {
        let inherited = conservative
            .pointer(&format!("/{}/{}", field.family, field.name))
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("the conservative table states no `{field}`"))?;
        let reason = if uncomparable.contains(field) {
            "no grid point offers both arms"
        } else {
            "no sweep covers this field"
        };
        println!("{}\t{}\t{inherited}\t{reason}", field.family, field.name);
    }
    Ok(())
}

/// The swept fields whose value the sweep could not measure at any grid point.
///
/// A field that keeps its default after a comparison — no grid point beat the
/// noise band, or the crossover was non-monotone — is a calibration outcome and
/// stays in the document. A field with no comparable grid point at all was
/// never calibrated, and design §5 condition 5 requires the document to omit it
/// rather than state a value: "a profile that carries an uncalibrated value is a
/// `@/inv/benchmark-backed-performance` defect".
///
/// These are not the whole omission set: [`omitted_fields`] adds every schema
/// field no sweep covers, which the same rule governs for the same reason.
fn uncalibrated_fields(sweeps: &[FieldSweep]) -> Vec<CalibratedField> {
    sweeps
        .iter()
        .filter(|sweep| {
            matches!(
                sweep.selection,
                Selection::KeptDefault {
                    reason: Fallback::NoComparableGridPoint,
                    ..
                }
            )
        })
        .map(|sweep| sweep.field)
        .collect()
}

/// The schema fields whose value this run measured.
///
/// A field the sweep covered and concluded on — including one that kept its
/// default on a tie or a non-monotone crossover — is measured. A field with no
/// comparable grid point is not, and neither is any schema field no sweep
/// names.
fn measured_fields(sweeps: &[FieldSweep]) -> Vec<SchemaField> {
    let uncalibrated = uncalibrated_fields(sweeps);
    sweeps
        .iter()
        .map(|sweep| sweep.field)
        .filter(|field| !uncalibrated.contains(field))
        .map(CalibratedField::schema_field)
        .collect()
}

/// Every `selectors.<family>.<field>` key `document` states.
///
/// The inventory is read off the serialized profile at run time rather than
/// listed in this tool. A list here would be a hand-maintained copy of the
/// schema — the staleness defect `@/inv/runtime-observed-provenance` names —
/// and the moment it fell behind, an unswept field would be emitted with a
/// value nothing measured.
fn schema_fields(document: &str) -> Result<Vec<SchemaField>, String> {
    let parsed: serde_json::Value = serde_json::from_str(document)
        .map_err(|error| format!("the profile document is not JSON: {error}"))?;
    let families = parsed
        .pointer("/sections/gf2-core~1selectors/selectors")
        .and_then(serde_json::Value::as_object)
        .ok_or("the profile document has no `selectors` object")?;
    let mut fields = Vec::new();
    for (family, members) in families {
        let members = members
            .as_object()
            .ok_or_else(|| format!("the `{family}` selector family is not an object"))?;
        fields.extend(members.keys().map(|name| SchemaField {
            family: family.clone(),
            name: name.clone(),
        }));
    }
    Ok(fields)
}

/// Every schema field the emitted document omits.
///
/// The set is the complement of what this run measured, so it covers both a
/// swept field with no comparable grid point and every schema field outside the
/// sweep, whatever the schema has grown since. Design §5 condition 5 admits an
/// omitted field and forbids an unmeasured stated one, so the complement is the
/// rule rather than a conservative approximation of it.
fn omitted_fields(document: &str, sweeps: &[FieldSweep]) -> Result<Vec<SchemaField>, String> {
    let measured = measured_fields(sweeps);
    Ok(schema_fields(document)?
        .into_iter()
        .filter(|field| !measured.contains(field))
        .collect())
}

/// Re-encodes `profile` after omitting fields this run did not measure.
///
/// The owner codec decodes the reduced selector body so private presence state
/// records each omission, then the one registry encoder recomputes the format-2
/// content digest. Absent fields retain their conservative dispatch semantics
/// without making a measurement claim.
fn calibrated_document(
    profile: &ProducedCoreProfile,
    omitted: &[SchemaField],
) -> Result<String, String> {
    Ok(profile.omitting(omitted)?.to_json())
}

/// Writes the emitted document to a temporary file, reparses it, then publishes it.
///
/// Only a document the loader accepts is an artifact, so a document this
/// harness could not have loaded never reaches a receipt. The comparison is
/// against the measured profile: an omitted field resolves back to the
/// conservative default, which is the value the sweep left it at.
fn emit_profile(path: &Path, json: &str) -> io::Result<String> {
    let path = resolve_repository_path(path);
    if path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "refusing to overwrite an existing path: {}; name a unique absent path",
                path.display()
            ),
        ));
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("the output path has no file name: {}", path.display()),
        )
    })?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos();
    let mut temporary = None;
    for attempt in 0..128_u8 {
        let candidate = parent.join(format!(
            ".{}.tmp-{}-{nonce}-{attempt}",
            file_name.to_string_lossy(),
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                temporary = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    let (temporary_path, mut temporary_file) = temporary.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "could not reserve a temporary output beside {}",
                path.display()
            ),
        )
    })?;

    let result = (|| {
        temporary_file.write_all(json.as_bytes())?;
        temporary_file.sync_all()?;
        drop(temporary_file);

        let written = fs::read_to_string(&temporary_path)?;
        let reparsed = ProducedCoreProfile::from_json(&written).map_err(|error| {
            io::Error::other(format!(
                "the emitted document does not load back: {error}; this is a harness defect"
            ))
        })?;
        if reparsed.to_json() != written {
            return Err(io::Error::other(
                "the emitted document does not round-trip canonically",
            ));
        }

        fs::hard_link(&temporary_path, &path).map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "refusing to overwrite an existing path: {}; name a unique absent path",
                        path.display()
                    ),
                )
            } else {
                error
            }
        })?;
        Ok(written)
    })();

    // The final path is either absent or a complete hard link to the validated
    // bytes. A cleanup failure can leave only the hidden temporary link, never
    // a partial final artifact, so it must not turn a successfully published
    // artifact into an ambiguous reported failure.
    if let Err(error) = fs::remove_file(&temporary_path) {
        eprintln!(
            "warning: could not remove temporary tuning artifact {}: {error}",
            temporary_path.display()
        );
    }
    result
}

/// The profile identifier, taken from `--profile-id` or the emitted file's own
/// basename, which is what a committed profile is named by.
fn profile_id_for(out: &Path, explicit: Option<&str>) -> Result<ProfileId, String> {
    let candidate = match explicit {
        Some(value) => value.to_owned(),
        None => out
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("--out has no file name to take a profile id from")?
            .to_owned(),
    };
    ProfileId::parse(&candidate).map_err(|_| {
        format!("`{candidate}` is not a kebab-case profile id; pass --profile-id with one")
    })
}

// ---------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args(env::args().skip(1))?;
    let (out, profile_id, lock_wrapper, receipt) = match &args.mode {
        Mode::ListGrid => {
            print_protocol(&args.protocol);
            print_grid();
            return Ok(());
        }
        Mode::SelfCheck => {
            print_protocol(&args.protocol);
            match collect_host_facts() {
                Ok(facts) => print_host_facts(&facts),
                Err(error) => println!("host facts unavailable: {error}"),
            }
            return Ok(());
        }
        // The guarded case contains the complete child protocol. The parent is
        // already behind the prepared-host gate and lock probe, so the child
        // consults neither of those ambient inputs.
        Mode::FreshChild => {
            run_fresh_child()?;
            return Ok(());
        }
        Mode::Calibrate {
            out,
            profile_id,
            lock_wrapper,
            receipt,
        } => (out, profile_id, lock_wrapper, receipt),
    };

    if env::var(BENCH_MODE_VAR).as_deref() != Ok("1") {
        eprintln!(
            "calibration is an explicit benchmark action and did not run: set {BENCH_MODE_VAR}=1 \
             on a prepared uncontended host and invoke through the repository lock wrapper. \
             Nothing was measured and nothing was written."
        );
        return Ok(());
    }

    let id = profile_id_for(out, profile_id.as_deref())?;
    let lock_wrapper_path = RepoRelPath::parse(lock_wrapper).map_err(|_| {
        format!("--lock-wrapper `{lock_wrapper}` is not a repository-relative path")
    })?;
    let receipt_path = RepoRelPath::parse(receipt)
        .map_err(|_| format!("--receipt `{receipt}` is not a repository-relative path"))?;
    let repo_root = PathBuf::from(command_output("git", &["rev-parse", "--show-toplevel"])?);
    if !repo_root.join(lock_wrapper_path.as_str()).is_file() {
        return Err(format!(
            "--lock-wrapper {} does not name a file in this repository",
            lock_wrapper_path.as_str()
        )
        .into());
    }
    let facts = collect_host_facts()?;

    print_protocol(&args.protocol);
    print_host_facts(&facts);
    println!("lock_wrapper: {}", lock_wrapper_path.as_str());
    print_grid();

    let measured_at = rfc3339_utc(SystemTime::now())?;
    let started = Instant::now();
    let mut sweeps = Vec::with_capacity(CalibratedField::ALL.len());
    for field in CalibratedField::ALL {
        let mut points = Vec::new();
        for size in field.grid() {
            let mut fixture = build_fixture(field, size);
            equivalence_probe(field, size, &fixture, &args.protocol);
            let mut conservative_rates: Option<Vec<f64>> = None;
            let mut asymptotic_rates: Option<Vec<f64>> = None;
            for execution in 0..args.protocol.executions {
                for (arm, rates) in [
                    (Arm::Conservative, &mut conservative_rates),
                    (Arm::Asymptotic, &mut asymptotic_rates),
                ] {
                    if let Some(window) = measure_arm_execution(
                        field,
                        size,
                        arm,
                        &mut fixture,
                        &args.protocol,
                        execution,
                    ) {
                        rates.get_or_insert_with(Vec::new).extend(window);
                    }
                }
            }
            points.push(GridPoint {
                size,
                conservative: conservative_rates.as_deref().map(ArmStat::from_rates),
                asymptotic: asymptotic_rates.as_deref().map(ArmStat::from_rates),
            });
        }
        let selection = select(field, &points);
        let sweep = FieldSweep {
            field,
            points,
            selection,
        };
        print_sweep(&sweep);
        sweeps.push(sweep);
    }
    println!("\ntimed work: {:.1} s", started.elapsed().as_secs_f64());

    let selected = SelectedValues::from_sweeps(&sweeps);
    let assembled_at = Rfc3339Utc::parse(&rfc3339_utc(SystemTime::now())?)?;
    let provenance = MeasurementProvenance::Calibrated {
        measured_at: Rfc3339Utc::parse(&measured_at)?,
        source_revision: facts.source_revision.clone(),
        source_dirty: facts.source_dirty,
        harness: facts.harness.clone(),
        harness_schema: HarnessSchema::parse(CoreTuningCodec::HARNESS_SCHEMA)?,
        binary_sha256: facts.binary_sha256.clone(),
        toolchain: facts.toolchain.clone(),
        host: facts.host.clone(),
        cpu_model: facts.cpu_model.clone(),
        cpu_features: facts.cpu_features.clone(),
        os_kernel: facts.os_kernel.clone(),
        governor: facts.governor.clone(),
        receipt: receipt_path,
    };
    let profile = build_profile(id, provenance, assembled_at, &selected)?;
    let omitted = omitted_fields(&profile.to_json(), &sweeps)?;
    print_omitted(&omitted, &sweeps)?;
    let document = calibrated_document(&profile, &omitted)?;
    let json = emit_profile(out, &document)?;

    println!("\nemitted and re-loaded {}", out.display());
    println!("{json}");
    if facts.source_dirty {
        println!(
            "\nthe source tree is dirty, so this profile is not committable: the revision it \
             names does not reproduce the binary that produced these numbers"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    struct TestOutput {
        directory: PathBuf,
        path: PathBuf,
    }

    #[allow(dead_code)]
    impl TestOutput {
        fn new(label: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let directory = env::temp_dir().join(format!(
                "gf2-tuning-calibration-{}-{serial}-{label}",
                std::process::id()
            ));
            let path = directory.join("profile.json");
            Self { directory, path }
        }
    }

    impl Drop for TestOutput {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    /// Five distinct values, so a transposition among the four polynomial
    /// thresholds cannot satisfy the per-field assertions below.
    #[allow(dead_code)]
    const DISTINCT: SelectedValues = SelectedValues {
        simd_min_words: 11,
        karatsuba_min_degree: 22,
        karatsuba_max_out_len: 33,
        div_rem_fast_min_len: 44,
        subproduct_min_len: 55,
    };

    #[allow(dead_code)]
    fn calibrated_provenance() -> MeasurementProvenance {
        MeasurementProvenance::Calibrated {
            measured_at: Rfc3339Utc::parse("2026-08-20T00:00:00Z").unwrap(),
            source_revision: GitRevision::parse(&"a".repeat(40)).unwrap(),
            source_dirty: false,
            harness: RepoRelPath::parse("crates/gf2-core/benches/tuning_calibration.rs").unwrap(),
            harness_schema: HarnessSchema::parse(CoreTuningCodec::HARNESS_SCHEMA).unwrap(),
            binary_sha256: Sha256::parse(&"b".repeat(64)).unwrap(),
            toolchain: "rustc 1.95.0".to_owned(),
            host: "test-host".to_owned(),
            cpu_model: "test-cpu".to_owned(),
            cpu_features: vec!["avx2".to_owned()],
            os_kernel: "Linux".to_owned(),
            governor: "performance".to_owned(),
            receipt: RepoRelPath::parse("dev/benchmarks/tuning_profiles/receipt.md").unwrap(),
        }
    }

    fn assembly_instant() -> Rfc3339Utc {
        Rfc3339Utc::parse("2026-08-20T01:00:00Z").unwrap()
    }

    #[allow(dead_code)]
    fn profile_from(selected: &SelectedValues) -> ProducedCoreProfile {
        build_profile(
            ProfileId::parse("test-profile").unwrap(),
            calibrated_provenance(),
            assembly_instant(),
            selected,
        )
        .expect("the swept values are in range")
    }

    /// A grid point with both arms, stated as medians and spreads directly.
    #[allow(dead_code)]
    fn point(size: usize, conservative: f64, asymptotic: f64, spread: f64) -> GridPoint {
        GridPoint {
            size,
            conservative: Some(ArmStat {
                median: conservative,
                spread,
                windows: 25,
            }),
            asymptotic: Some(ArmStat {
                median: asymptotic,
                spread,
                windows: 25,
            }),
        }
    }

    #[test]
    fn assembly_uses_the_supplied_post_sweep_instant_not_measurement_start() {
        let profile = profile_from(&DISTINCT);
        let MeasurementProvenance::Calibrated { measured_at, .. } = &profile.measurement else {
            panic!("the test profile must carry calibrated measurement evidence");
        };

        assert_eq!(measured_at.as_str(), "2026-08-20T00:00:00Z");
        assert_eq!(
            profile.assembly.assembled_at.as_str(),
            "2026-08-20T01:00:00Z"
        );
        assert_ne!(profile.assembly.assembled_at, *measured_at);
    }

    #[test]
    fn emitted_profile_carries_the_swept_simd_min_words() {
        assert_eq!(
            profile_from(&DISTINCT).bit_backend().simd_min_words(),
            DISTINCT.simd_min_words
        );
    }

    #[test]
    fn emitted_profile_carries_the_swept_karatsuba_min_degree() {
        assert_eq!(
            profile_from(&DISTINCT).polynomial().karatsuba_min_degree(),
            DISTINCT.karatsuba_min_degree
        );
    }

    #[test]
    fn emitted_profile_carries_the_swept_karatsuba_max_out_len() {
        assert_eq!(
            profile_from(&DISTINCT).polynomial().karatsuba_max_out_len(),
            DISTINCT.karatsuba_max_out_len
        );
    }

    #[test]
    fn emitted_profile_carries_the_swept_div_rem_fast_min_len() {
        assert_eq!(
            profile_from(&DISTINCT).polynomial().div_rem_fast_min_len(),
            DISTINCT.div_rem_fast_min_len
        );
    }

    #[test]
    fn emitted_profile_carries_the_swept_subproduct_min_len() {
        assert_eq!(
            profile_from(&DISTINCT).polynomial().subproduct_min_len(),
            DISTINCT.subproduct_min_len
        );
    }

    /// A sweep of `field` that concluded with `selection`, carrying no grid.
    #[allow(dead_code)]
    fn concluded(field: CalibratedField, reason: Fallback) -> FieldSweep {
        FieldSweep {
            field,
            points: Vec::new(),
            selection: Selection::KeptDefault {
                value: field.conservative_default(),
                reason,
            },
        }
    }

    #[test]
    fn the_emitted_document_omits_the_field_the_sweep_could_not_compare() {
        let profile = profile_from(&DISTINCT);
        let document = calibrated_document(
            &profile,
            &[CalibratedField::KaratsubaMinDegree.schema_field()],
        )
        .unwrap();
        assert!(
            !document.contains("karatsuba_min_degree"),
            "the uncalibrated field is still stated: {document}"
        );
        let loaded = ProducedCoreProfile::from_json(&document)
            .expect("an omitted field is a supported state of the schema");
        assert_eq!(
            loaded.polynomial().karatsuba_min_degree(),
            CoreTuning::CONSERVATIVE.polynomial().karatsuba_min_degree(),
            "the loader resolves the absent field to the conservative default"
        );
    }

    #[test]
    fn omitting_one_field_leaves_the_other_four_stated() {
        let profile = profile_from(&DISTINCT);
        let document = calibrated_document(
            &profile,
            &[CalibratedField::KaratsubaMinDegree.schema_field()],
        )
        .unwrap();
        let loaded = ProducedCoreProfile::from_json(&document).unwrap();
        assert_eq!(
            loaded.bit_backend().simd_min_words(),
            DISTINCT.simd_min_words
        );
        assert_eq!(
            loaded.polynomial().karatsuba_max_out_len(),
            DISTINCT.karatsuba_max_out_len
        );
        assert_eq!(
            loaded.polynomial().div_rem_fast_min_len(),
            DISTINCT.div_rem_fast_min_len
        );
        assert_eq!(
            loaded.polynomial().subproduct_min_len(),
            DISTINCT.subproduct_min_len
        );
    }

    #[test]
    fn a_document_with_nothing_omitted_is_the_serialized_profile_verbatim() {
        let profile = profile_from(&DISTINCT);
        assert_eq!(
            calibrated_document(&profile, &[]).unwrap(),
            profile.to_json()
        );
    }

    #[test]
    fn omission_changes_only_the_selector_body_and_recomputes_the_digest() {
        let profile = profile_from(&DISTINCT);
        let document = calibrated_document(
            &profile,
            &[CalibratedField::KaratsubaMinDegree.schema_field()],
        )
        .unwrap();
        let complete: serde_json::Value = serde_json::from_str(&profile.to_json()).unwrap();
        let omitted: serde_json::Value = serde_json::from_str(&document).unwrap();
        let mut expected = complete["sections"]["gf2-core/selectors"]["selectors"].clone();
        expected["polynomial"]
            .as_object_mut()
            .unwrap()
            .remove("karatsuba_min_degree");
        assert_eq!(
            omitted["sections"]["gf2-core/selectors"]["selectors"],
            expected
        );
        assert_ne!(
            omitted["assembly"]["content_sha256"],
            complete["assembly"]["content_sha256"]
        );
    }

    #[test]
    fn omitting_the_last_field_of_a_family_leaves_an_empty_object() {
        let profile = profile_from(&DISTINCT);
        let document =
            calibrated_document(&profile, &[CalibratedField::SimdMinWords.schema_field()]).unwrap();
        assert!(document.contains(r#""bit_backend":{}"#), "{document}");
        let loaded = ProducedCoreProfile::from_json(&document).unwrap();
        assert_eq!(
            loaded.bit_backend().simd_min_words(),
            CoreTuning::CONSERVATIVE.bit_backend().simd_min_words()
        );
    }

    #[test]
    fn a_provenance_string_is_never_mistaken_for_a_selector_key() {
        // The receipt path names the field, so a document-wide search would cut
        // the wrong bytes.
        let mut provenance = calibrated_provenance();
        if let MeasurementProvenance::Calibrated { receipt, .. } = &mut provenance {
            *receipt =
                RepoRelPath::parse("dev/benchmarks/tuning_profiles/karatsuba_min_degree-notes.md")
                    .unwrap();
        }
        let profile = build_profile(
            ProfileId::parse("test-profile").unwrap(),
            provenance,
            assembly_instant(),
            &DISTINCT,
        )
        .unwrap();
        let document = calibrated_document(
            &profile,
            &[CalibratedField::KaratsubaMinDegree.schema_field()],
        )
        .unwrap();
        let loaded = ProducedCoreProfile::from_json(&document).unwrap();
        let MeasurementProvenance::Calibrated { receipt, .. } = &loaded.measurement else {
            panic!("the document stays calibrated");
        };
        assert_eq!(
            receipt.as_str(),
            "dev/benchmarks/tuning_profiles/karatsuba_min_degree-notes.md"
        );
        assert_eq!(
            loaded.polynomial().karatsuba_min_degree(),
            CoreTuning::CONSERVATIVE.polynomial().karatsuba_min_degree()
        );
    }

    #[test]
    fn only_an_uncomparable_field_is_omitted() {
        let sweeps = [
            concluded(
                CalibratedField::KaratsubaMinDegree,
                Fallback::NoComparableGridPoint,
            ),
            concluded(
                CalibratedField::KaratsubaMaxOutLen,
                Fallback::NonMonotone {
                    first_win: 255,
                    later_loss: 383,
                },
            ),
            concluded(CalibratedField::SimdMinWords, Fallback::NoGridPointWins),
        ];
        assert_eq!(
            uncalibrated_fields(&sweeps),
            vec![CalibratedField::KaratsubaMinDegree],
            "a tie and a non-monotone crossover are calibration outcomes, not absent measurements"
        );
    }

    /// A sweep for every field, each concluding on a comparison, so nothing is
    /// omitted for want of a grid point and the omission set is exactly the
    /// schema fields outside the sweep.
    #[allow(dead_code)]
    fn measured_sweeps() -> Vec<FieldSweep> {
        CalibratedField::ALL
            .into_iter()
            .map(|field| {
                let points = vec![point(field.conservative_default(), 100.0, 50.0, 0.01)];
                FieldSweep {
                    field,
                    selection: select(field, &points),
                    points,
                }
            })
            .collect()
    }

    #[test]
    fn every_swept_field_names_a_key_the_schema_states() {
        let schema = schema_fields(&profile_from(&DISTINCT).to_json()).unwrap();
        for field in CalibratedField::ALL {
            let key = field.schema_field();
            assert!(
                schema.contains(&key),
                "the sweep states {key}, which the schema does not carry; a renamed schema field \
                 would leave the measured value silently omitted"
            );
        }
    }

    #[test]
    fn the_schema_carries_fields_no_sweep_covers() {
        let schema = schema_fields(&profile_from(&DISTINCT).to_json()).unwrap();
        assert!(
            schema.len() > CalibratedField::ALL.len(),
            "the schema states {} fields against {} swept ones; the omission-set tests below are \
             vacuous once the two coincide",
            schema.len(),
            CalibratedField::ALL.len()
        );
    }

    #[test]
    fn an_emitted_document_states_only_the_fields_the_run_measured() {
        let profile = profile_from(&DISTINCT);
        let sweeps = measured_sweeps();
        let omitted = omitted_fields(&profile.to_json(), &sweeps).unwrap();
        let document = calibrated_document(&profile, &omitted).unwrap();
        let mut stated = schema_fields(&document).unwrap();
        let mut swept: Vec<SchemaField> = CalibratedField::ALL
            .into_iter()
            .map(CalibratedField::schema_field)
            .collect();
        stated.sort();
        swept.sort();
        assert_eq!(
            stated, swept,
            "the emitted document states a field this run did not measure"
        );
    }

    #[test]
    fn an_unswept_schema_field_is_omitted_and_resolves_to_its_inherited_value() {
        let profile = profile_from(&DISTINCT);
        let sweeps = measured_sweeps();
        let omitted = omitted_fields(&profile.to_json(), &sweeps).unwrap();
        let document = calibrated_document(&profile, &omitted).unwrap();
        let stated = schema_fields(&document).unwrap();
        let loaded = ProducedCoreProfile::from_json(&document)
            .expect("an omitted field is a supported state of the schema");
        let loaded = complete_selector_value(&loaded.section).unwrap();
        let inherited = complete_selector_value(&CoreTuning::CONSERVATIVE).unwrap();
        assert!(!omitted.is_empty());
        for field in &omitted {
            let pointer = format!("/{}/{}", field.family, field.name);
            assert!(!stated.contains(field), "{field} is still stated");
            assert_eq!(
                loaded.pointer(&pointer),
                inherited.pointer(&pointer),
                "the loader does not resolve the absent {field} to its inherited value"
            );
        }
    }

    #[test]
    fn the_omission_set_is_the_complement_of_what_the_run_measured() {
        let document = profile_from(&DISTINCT).to_json();
        let sweeps = measured_sweeps();
        let measured = measured_fields(&sweeps);
        let omitted = omitted_fields(&document, &sweeps).unwrap();
        for field in &measured {
            assert!(
                !omitted.contains(field),
                "{field} is both measured and omitted"
            );
        }
        let mut union: Vec<SchemaField> = measured.into_iter().chain(omitted).collect();
        union.sort();
        let mut schema = schema_fields(&document).unwrap();
        schema.sort();
        assert_eq!(union, schema, "the two sets do not partition the schema");
    }

    #[test]
    fn a_swept_field_without_a_comparison_joins_the_unswept_fields_in_the_omission_set() {
        let profile = profile_from(&DISTINCT);
        let mut sweeps = measured_sweeps();
        sweeps.retain(|sweep| sweep.field != CalibratedField::SubproductMinLen);
        sweeps.push(concluded(
            CalibratedField::SubproductMinLen,
            Fallback::NoComparableGridPoint,
        ));
        let omitted = omitted_fields(&profile.to_json(), &sweeps).unwrap();
        assert!(omitted.contains(&CalibratedField::SubproductMinLen.schema_field()));
        let document = calibrated_document(&profile, &omitted).unwrap();
        assert_eq!(
            ProducedCoreProfile::from_json(&document)
                .unwrap()
                .polynomial()
                .subproduct_min_len(),
            CoreTuning::CONSERVATIVE.polynomial().subproduct_min_len()
        );
    }

    #[test]
    fn a_family_scoped_removal_leaves_a_similarly_named_field_of_another_family() {
        let profile = profile_from(&DISTINCT);
        let document =
            calibrated_document(&profile, &[CalibratedField::SimdMinWords.schema_field()]).unwrap();
        let stated = schema_fields(&document).unwrap();
        assert!(!stated.contains(&CalibratedField::SimdMinWords.schema_field()));
        assert!(stated.contains(&SchemaField {
            family: "bit_matrix".to_owned(),
            name: "matvec_simd_min_words".to_owned(),
        }));
    }

    #[test]
    fn a_measured_field_is_never_omitted() {
        let points = vec![point(16, 100.0, 50.0, 0.01)];
        let sweep = FieldSweep {
            field: CalibratedField::DivRemFastMinLen,
            points: points.clone(),
            selection: select(CalibratedField::DivRemFastMinLen, &points),
        };
        assert!(uncalibrated_fields(&[sweep]).is_empty());
    }

    #[test]
    fn emitted_profile_round_trips_through_the_loader() {
        let profile = profile_from(&DISTINCT);
        let reparsed = ProducedCoreProfile::from_json(&profile.to_json())
            .expect("the harness emits a document the loader accepts");
        assert_eq!(reparsed, profile);
    }

    #[test]
    fn valid_output_is_published_only_after_canonical_reopen() {
        let output = TestOutput::new("valid");
        let document = profile_from(&DISTINCT).to_json();

        assert_eq!(emit_profile(&output.path, &document).unwrap(), document);
        assert_eq!(fs::read_to_string(&output.path).unwrap(), document);
        assert_eq!(
            fs::read_dir(&output.directory).unwrap().count(),
            1,
            "the validated final file is the only surviving directory entry"
        );
    }

    #[test]
    fn invalid_output_leaves_no_final_or_temporary_artifact() {
        let output = TestOutput::new("invalid");

        let error = emit_profile(&output.path, "{}")
            .expect_err("a document outside the strict owner schema must not publish");
        assert!(error.to_string().contains("does not load back"));
        assert!(!output.path.exists());
        assert_eq!(fs::read_dir(&output.directory).unwrap().count(), 0);
    }

    #[test]
    fn output_publication_never_replaces_an_existing_path() {
        let output = TestOutput::new("occupied");
        fs::create_dir_all(&output.directory).unwrap();
        fs::write(&output.path, "sentinel").unwrap();

        let error = emit_profile(&output.path, &profile_from(&DISTINCT).to_json())
            .expect_err("an existing artifact must win the publication race");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&output.path).unwrap(), "sentinel");
        assert_eq!(fs::read_dir(&output.directory).unwrap().count(), 1);
    }

    #[test]
    fn emitted_provenance_is_calibrated_and_populated() {
        let profile = profile_from(&DISTINCT);
        let MeasurementProvenance::Calibrated {
            harness_schema,
            toolchain,
            host,
            ..
        } = &profile.measurement
        else {
            panic!("the calibration action emits calibrated provenance");
        };
        assert_eq!(harness_schema.as_str(), CoreTuningCodec::HARNESS_SCHEMA);
        assert_eq!(toolchain, "rustc 1.95.0");
        assert_eq!(host, "test-host");
    }

    #[test]
    fn every_grid_straddles_its_conservative_default() {
        for field in CalibratedField::ALL {
            let grid = field.grid();
            let default = field.conservative_default();
            assert!(
                grid.iter().any(|&size| size < default),
                "{field} has no grid point below {default}"
            );
            assert!(
                grid.iter().any(|&size| size > default),
                "{field} has no grid point above {default}"
            );
            assert!(
                grid.windows(2).all(|pair| pair[0] < pair[1]),
                "{field} grid is not strictly ascending: {grid:?}"
            );
            assert!(grid.iter().all(|&size| size > 0), "{field} grid has a zero");
        }
    }

    #[test]
    fn karatsuba_out_len_grid_spans_the_recorded_mul_fast_step() {
        let grid = CalibratedField::KaratsubaMaxOutLen.grid();
        assert!(grid.contains(&127) && grid.contains(&129));
        assert!(grid.iter().any(|&size| size >= 255));
        assert!(
            grid.iter().all(|&size| !size.is_multiple_of(2)),
            "equal-length operands give an odd product length: {grid:?}"
        );
    }

    #[test]
    fn a_monotone_crossover_selects_its_smallest_winning_grid_point() {
        let points = vec![
            point(16, 100.0, 120.0, 0.01),
            point(32, 100.0, 90.0, 0.01),
            point(64, 100.0, 50.0, 0.01),
        ];
        let selection = select(CalibratedField::DivRemFastMinLen, &points);
        assert!(matches!(
            selection,
            Selection::Crossover {
                value: 32,
                crossover: 32,
                ..
            }
        ));
    }

    #[test]
    fn an_upper_bound_field_selects_the_grid_point_below_the_crossover() {
        let points = vec![
            point(63, 100.0, 120.0, 0.01),
            point(127, 100.0, 110.0, 0.01),
            point(255, 100.0, 50.0, 0.01),
        ];
        let selection = select(CalibratedField::KaratsubaMaxOutLen, &points);
        assert!(matches!(
            selection,
            Selection::Crossover {
                value: 127,
                crossover: 255,
                ..
            }
        ));
    }

    #[test]
    fn a_margin_inside_the_noise_band_does_not_win() {
        let points = vec![point(32, 100.0, 97.0, 0.05)];
        assert_eq!(
            select(CalibratedField::DivRemFastMinLen, &points),
            Selection::KeptDefault {
                value: CalibratedField::DivRemFastMinLen.conservative_default(),
                reason: Fallback::NoGridPointWins,
            }
        );
    }

    #[test]
    fn a_non_monotone_crossover_keeps_the_conservative_default() {
        let points = vec![
            point(16, 100.0, 120.0, 0.01),
            point(32, 100.0, 50.0, 0.01),
            point(64, 100.0, 130.0, 0.01),
        ];
        assert_eq!(
            select(CalibratedField::SubproductMinLen, &points),
            Selection::KeptDefault {
                value: CalibratedField::SubproductMinLen.conservative_default(),
                reason: Fallback::NonMonotone {
                    first_win: 32,
                    later_loss: 64,
                },
            }
        );
    }

    #[test]
    fn a_grid_with_one_arm_everywhere_keeps_the_conservative_default() {
        let points: Vec<GridPoint> = [16_usize, 32, 64]
            .into_iter()
            .map(|size| GridPoint {
                size,
                conservative: Some(ArmStat {
                    median: 100.0,
                    spread: 0.01,
                    windows: 25,
                }),
                asymptotic: None,
            })
            .collect();
        assert_eq!(
            select(CalibratedField::KaratsubaMinDegree, &points),
            Selection::KeptDefault {
                value: CalibratedField::KaratsubaMinDegree.conservative_default(),
                reason: Fallback::NoComparableGridPoint,
            }
        );
    }

    #[test]
    fn selected_values_fall_back_to_the_defaults_of_unswept_fields() {
        let selected = SelectedValues::from_sweeps(&[]);
        assert_eq!(
            selected,
            SelectedValues {
                simd_min_words: CalibratedField::SimdMinWords.conservative_default(),
                karatsuba_min_degree: CalibratedField::KaratsubaMinDegree.conservative_default(),
                karatsuba_max_out_len: CalibratedField::KaratsubaMaxOutLen.conservative_default(),
                div_rem_fast_min_len: CalibratedField::DivRemFastMinLen.conservative_default(),
                subproduct_min_len: CalibratedField::SubproductMinLen.conservative_default(),
            }
        );
    }

    #[test]
    fn a_default_valued_sweep_reproduces_the_conservative_selectors() {
        let selected = SelectedValues::from_sweeps(&[]);
        let profile = profile_from(&selected);
        let conservative = &CoreTuning::CONSERVATIVE;
        assert_eq!(profile.bit_backend(), conservative.bit_backend());
        assert_eq!(profile.polynomial(), conservative.polynomial());
    }

    #[test]
    fn statistics_summarise_a_window_sample() {
        let stat = ArmStat::from_rates(&[10.0, 12.0, 8.0, 11.0, 9.0]);
        assert_eq!(stat.median, 10.0);
        assert_eq!(stat.windows, 5);
        // Nearest-rank quartiles of the five windows are 9 and 11.
        assert!((stat.spread - 0.2).abs() < 1e-12);
    }

    #[test]
    fn an_even_sample_medians_between_its_central_windows() {
        assert_eq!(ArmStat::from_rates(&[10.0, 20.0]).median, 15.0);
    }

    #[test]
    fn the_calibrate_mode_requires_its_output_and_provenance_paths() {
        for missing in ["--lock-wrapper", "--receipt", "--out"] {
            let args: Vec<String> = [
                "--out",
                "/tmp/x.json",
                "--lock-wrapper",
                "w",
                "--receipt",
                "r",
            ]
            .chunks(2)
            .filter(|pair| pair[0] != missing)
            .flat_map(|pair| pair.iter().map(|value| (*value).to_owned()))
            .collect();
            assert!(
                parse_args(args.into_iter()).is_err(),
                "{missing} is required"
            );
        }
    }

    #[test]
    fn a_zero_protocol_constant_is_rejected() {
        for flag in ["--executions", "--repetitions", "--target-ms"] {
            let args = [flag, "0", "--self-check"].map(str::to_owned);
            assert!(
                parse_args(args.into_iter()).is_err(),
                "{flag} accepted zero"
            );
        }
    }

    #[test]
    fn the_reporting_modes_need_no_output_path() {
        let self_check = parse_args(["--self-check".to_owned()].into_iter()).unwrap();
        assert_eq!(self_check.mode, Mode::SelfCheck);
        let list_grid = parse_args(["--list-grid".to_owned()].into_iter()).unwrap();
        assert_eq!(list_grid.mode, Mode::ListGrid);
    }

    /// A spec whose four components are pairwise distinguishable, so a
    /// transposition in the rendered form cannot round-trip.
    #[allow(dead_code)]
    const CHILD_SPEC: ChildSpec = ChildSpec {
        field: CalibratedField::KaratsubaMinDegree,
        size: 31,
        arm: Arm::Asymptotic,
        task: ChildTask::Measure { execution: 2 },
    };

    #[allow(dead_code)]
    fn child_report(rates: usize) -> ChildReport {
        ChildReport {
            route: "karatsuba".to_owned(),
            operands: 0x1111,
            product: 0x2222,
            rates: vec![10.0; rates],
        }
    }

    #[allow(dead_code)]
    fn child_protocol(repetitions: u64) -> Protocol {
        Protocol {
            executions: 1,
            repetitions,
            target_ms: 1,
        }
    }

    #[test]
    fn a_fresh_process_case_has_one_pinned_canonical_stdin_encoding() {
        let case = FreshProcessCase {
            spec: CHILD_SPEC,
            protocol: child_protocol(3),
        };
        let encoded = serde_json::to_string(&case).unwrap();
        assert_eq!(
            encoded,
            r#"{"spec":{"field":"karatsuba_min_degree","size":31,"arm":"asymptotic","task":{"kind":"measure","execution":2}},"protocol":{"executions":1,"repetitions":3,"target_ms":1}}"#
        );
        assert_eq!(decode_fresh_case(&encoded), Ok(case));
        assert!(decode_fresh_case(&format!("{encoded}\n")).is_err());
        assert!(decode_fresh_case(&format!(" {encoded}")).is_err());
    }

    #[test]
    fn the_child_mode_accepts_no_case_data_outside_stdin() {
        let parsed = parse_args(["--fresh-tuning-process-child".to_owned()].into_iter()).unwrap();
        assert_eq!(parsed.mode, Mode::FreshChild);
        for args in [
            vec!["--fresh-tuning-process-child", "--list-grid"],
            vec!["--fresh-tuning-process-child", "--repetitions", "3"],
            vec!["--child-arm", "karatsuba_min_degree:31:asymptotic:2"],
        ] {
            assert!(
                parse_args(args.iter().copied().map(str::to_owned)).is_err(),
                "alternate child input {args:?} was accepted"
            );
        }
    }

    #[test]
    fn a_child_report_is_one_canonical_prefixed_json_line() {
        let report = ChildReport {
            route: "karatsuba".to_owned(),
            operands: 4369,
            product: 8738,
            rates: vec![10.0, 12.5],
        };
        let text = format!(
            "{FRESH_RESULT_PREFIX}{}\n",
            serde_json::to_string(&report).unwrap()
        );
        assert_eq!(parse_child_report(&text), Ok(report));
    }

    #[test]
    fn an_incomplete_unprefixed_or_repeated_child_result_is_rejected() {
        for text in [
            "{}\n",
            "route\tkaratsuba\n",
            "GF2_TUNING_RESULT={\"operands\":1,\"product\":2,\"rates\":[]}\n",
            "GF2_TUNING_RESULT={\"route\":\"karatsuba\",\"operands\":1,\"product\":2,\"rates\":[]}\nextra\n",
        ] {
            assert!(
                parse_child_report(text).is_err(),
                "{text:?} was read as a report"
            );
        }
    }

    #[test]
    fn a_child_that_took_the_other_arm_is_rejected() {
        let mut report = child_report(1);
        report.route = "schoolbook".to_owned();
        let error = verify_child_report(
            ChildSpec {
                task: ChildTask::Measure { execution: 0 },
                ..CHILD_SPEC
            },
            report.operands,
            &child_protocol(1),
            &report,
        )
        .unwrap_err();
        assert!(error.contains("schoolbook"), "{error}");
    }

    #[test]
    fn a_child_that_built_other_operands_is_rejected() {
        let report = child_report(1);
        assert!(verify_child_report(
            ChildSpec {
                task: ChildTask::Measure { execution: 0 },
                ..CHILD_SPEC
            },
            report.operands ^ 1,
            &child_protocol(1),
            &report,
        )
        .is_err());
    }

    #[test]
    fn a_child_that_timed_the_wrong_number_of_windows_is_rejected() {
        let report = child_report(2);
        let spec = ChildSpec {
            task: ChildTask::Measure { execution: 0 },
            ..CHILD_SPEC
        };
        assert!(verify_child_report(spec, report.operands, &child_protocol(3), &report).is_err());
        assert!(verify_child_report(spec, report.operands, &child_protocol(2), &report).is_ok());
    }

    #[test]
    fn a_probe_child_times_nothing() {
        let spec = ChildSpec {
            task: ChildTask::Probe,
            ..CHILD_SPEC
        };
        let report = child_report(0);
        assert!(verify_child_report(spec, report.operands, &child_protocol(5), &report).is_ok());
        assert!(
            verify_child_report(spec, report.operands, &child_protocol(5), &child_report(5))
                .is_err()
        );
    }

    #[test]
    fn the_karatsuba_arm_forces_the_grid_point_and_the_schoolbook_arm_the_range_top() {
        let inherited = &CoreTuning::CONSERVATIVE;
        for size in CalibratedField::KaratsubaMinDegree.grid() {
            assert_eq!(
                forced_karatsuba_min_degree(Arm::Asymptotic, size),
                size,
                "the Karatsuba arm times the recursion a threshold of {size} produces"
            );
            assert_eq!(
                forced_karatsuba_min_degree(Arm::Conservative, size),
                FORCED_SCHOOLBOOK_MIN_DEGREE
            );
            // Both forced values are ordinary admissible values, so forcing an
            // arm reserves no sentinel and installs no profile the loader would
            // reject.
            for arm in Arm::BOTH {
                let conservative = inherited.polynomial();
                assert!(
                    PolynomialSelectors::try_new(
                        forced_karatsuba_min_degree(arm, size),
                        conservative.karatsuba_max_out_len(),
                        conservative.div_rem_fast_min_len(),
                        conservative.subproduct_min_len(),
                        conservative.interpolate_fast_min_points(),
                    )
                    .is_ok(),
                    "{arm} at {size} forces an inadmissible threshold"
                );
            }
        }
    }

    #[test]
    fn the_forced_thresholds_route_every_grid_point_to_their_own_arm() {
        // `mul_route_resolved` is private, so the rule is exercised through the
        // same comparison the dispatcher applies: schoolbook strictly below the
        // threshold, Karatsuba at or above it.
        for size in CalibratedField::KaratsubaMinDegree.grid() {
            assert!(
                size >= forced_karatsuba_min_degree(Arm::Asymptotic, size),
                "{size} does not reach the threshold forced for the Karatsuba arm"
            );
            assert!(
                size < forced_karatsuba_min_degree(Arm::Conservative, size),
                "{size} reaches the threshold forced for the schoolbook arm"
            );
        }
    }

    #[test]
    fn the_karatsuba_arm_hands_its_sub_operands_to_the_schoolbook_base_case() {
        // A split at degree `size` produces sub-operands of about half that
        // degree, which fall below the same threshold and take the schoolbook
        // arm. That is the recursion shape a chosen threshold produces, and it
        // is what distinguishes this forcing from one at the bottom of the
        // range, where the recursion would reach degree 0.
        for size in CalibratedField::KaratsubaMinDegree.grid() {
            let forced = forced_karatsuba_min_degree(Arm::Asymptotic, size);
            assert!(
                size.div_ceil(2) < forced,
                "a split at {size} recurses again"
            );
        }
    }

    #[test]
    fn only_the_karatsuba_degree_field_needs_a_child_process() {
        for field in CalibratedField::ALL {
            let expected = if field == CalibratedField::KaratsubaMinDegree {
                ArmSource::ChildProcess
            } else {
                ArmSource::InProcess
            };
            assert_eq!(field.arm_source(), expected, "{field}");
        }
    }

    #[test]
    fn identical_operands_digest_alike_and_a_changed_one_does_not() {
        let field = CalibratedField::KaratsubaMinDegree;
        let digest_at = |size: usize| {
            let Fixture::Mul { a, b } = build_fixture(field, size) else {
                panic!("the multiplication field builds a multiplication fixture");
            };
            (operand_digest(&a, &b), operand_digest(&b, &a))
        };
        // A child rebuilds the fixture from the same seed, so the parent's
        // digest identifies the operands rather than the process.
        assert_eq!(digest_at(16).0, digest_at(16).0);
        assert_ne!(digest_at(16).0, digest_at(16).1);
        assert_ne!(digest_at(16).0, digest_at(8).0);
    }

    #[test]
    fn cargo_bench_passes_a_bench_flag_the_parser_ignores() {
        let args = ["--bench", "--list-grid"].map(str::to_owned);
        assert_eq!(parse_args(args.into_iter()).unwrap().mode, Mode::ListGrid);
    }

    #[test]
    fn a_profile_id_comes_from_the_emitted_file_name() {
        let id = profile_id_for(Path::new("/tmp/gf2-calibration-fraktaali.json"), None).unwrap();
        assert_eq!(id.as_str(), "gf2-calibration-fraktaali");
    }

    #[test]
    fn a_non_kebab_case_file_name_is_rejected_rather_than_mangled() {
        assert!(profile_id_for(Path::new("/tmp/Gf2_Calibration.json"), None).is_err());
        let explicit = profile_id_for(Path::new("/tmp/Gf2_Calibration.json"), Some("host-avx2"));
        assert_eq!(explicit.unwrap().as_str(), "host-avx2");
    }

    #[test]
    fn a_parent_segment_is_collapsed_out_of_the_harness_path() {
        assert_eq!(
            normalize_relative(Path::new("crates/gf2-core/tests/../benches/x.rs")),
            "crates/gf2-core/benches/x.rs"
        );
        assert!(RepoRelPath::parse(&normalize_relative(Path::new(
            "crates/gf2-core/tests/../benches/x.rs"
        )))
        .is_ok());
    }

    #[test]
    fn timestamps_are_formatted_in_the_form_the_loader_accepts() {
        let epoch = rfc3339_utc(UNIX_EPOCH).unwrap();
        assert_eq!(epoch, "1970-01-01T00:00:00Z");
        assert!(Rfc3339Utc::parse(&epoch).is_ok());
        let leap_day = rfc3339_utc(UNIX_EPOCH + Duration::from_secs(1_772_323_200)).unwrap();
        assert_eq!(leap_day, "2026-03-01T00:00:00Z");
        assert!(Rfc3339Utc::parse(&rfc3339_utc(SystemTime::now()).unwrap()).is_ok());
    }

    #[test]
    fn the_device_encoding_matches_the_kernel_lock_table() {
        assert_eq!(makedev(0x00, 0x2d), 45);
        assert_eq!(makedev(0x08, 0x02), 2050);
    }

    #[test]
    fn a_product_length_maps_back_to_equal_operand_lengths() {
        for out_len in CalibratedField::KaratsubaMaxOutLen.grid() {
            let len = operand_len_for_product(out_len);
            assert_eq!(2 * len - 1, out_len);
        }
    }
}
