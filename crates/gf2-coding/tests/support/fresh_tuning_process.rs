//! Guarded fresh-process protocol for the coding tuning section.
//! Installing a profile resolves the process-wide tuning authority once, so
//! every case that needs its own installed value runs in a child process
//! started from the test binary. This mirrors the algebra owner's protocol at
//! `crates/gf2-algebra/tests/support/fresh_tuning_process.rs`.

use std::io::Write;
use std::num::NonZeroUsize;
use std::process::{Command, Stdio};

use gf2_coding::bch::encode::{
    clmul_fold_accumulators, clmul_fold_run_blocks, EncodeFamily, SystematicLayout,
    TABLE_REMAINDER_BLOCK_BITS,
};
use gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
use gf2_coding::test_support;
use gf2_coding::tuning::{self, CodingSelectors, CodingTuning, DecodeSelectors, EncodeSelectors};
use gf2_core::field::extension::BinaryPrimeExt;
use gf2_core::gf2m::Gf2mField;
use gf2_core::tuning::{CompiledProfileProvenance, PreparedEnvelope, ProfileId, SectionResolution};
use gf2_core::BitVec;

#[cfg(feature = "tuning-profile")]
use gf2_coding::ldpc::{
    BatchRoute, DecodedWord, DecoderAlgorithm, DecoderConfig, LdpcBatchDecoder, LdpcCode,
    LdpcDecoder,
};
#[cfg(feature = "tuning-profile")]
use gf2_coding::traits::conformance::{batch_soft_decoder_contract, ConvergenceMix};
#[cfg(feature = "tuning-profile")]
use gf2_coding::tuning::CodingTuningCodec;
#[cfg(feature = "tuning-profile")]
use gf2_core::tuning::{
    AssemblyProvenance, GitRevision, ProfileRegistry, ProfileRegistryBuilder, RepoRelPath,
    Rfc3339Utc, Sha256,
};
#[cfg(feature = "tuning-profile")]
use gf2_kernels_simd::ldpc_lanes::LANES;

const SENTINEL: &str = "GF2_TUNING_FRESH_CASE";
const SENTINEL_VALUE: &str = "child-v1";
const RESULT_PREFIX: &str = "GF2_TUNING_RESULT=";

/// The batch length every case's selectors admit from, and the shorter of the
/// two lengths every case encodes; it is a rung of the workload-selection
/// contract's ladder.
const BATCH: usize = 16;

/// The longer length every case encodes, four whole lane groups and one frame
/// of a bit-sliced family, so a partition of it crosses lane groups at every
/// worker count.
const LANE_GROUP_BATCH: usize = 257;

/// The fold run length [`FreshProcessCase::ClmulFoldOverrides`] installs:
/// a multiple of no accumulator count the fold derives, and above the
/// structural run of [`corpus_row_b3`].
pub const FOLD_RUN_BLOCKS: usize = 9;

/// The fold accumulator count [`FreshProcessCase::ClmulFoldOverrides`]
/// installs, admissible at every width the chunk fold unrolls.
pub const FOLD_ACCUMULATORS: usize = 5;

/// The decode family the decode cases install, as `lane_min_frames`,
/// `lane_max_edges` and `granules_per_task`; every value differs from its
/// conservative one.
pub const INSTALLED_DECODE: (usize, usize, usize) = (4, 1 << 20, 3);

/// One named scenario of the protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreshProcessCase {
    /// No installation: the conservative section governs.
    ConservativeDefault,
    /// A programmatically compiled envelope admitting the table family.
    TableRemainderCompiled,
    /// The same selectors, with the buffer shapes watched across repeated
    /// batches rather than the codewords compared.
    TableRemainderAllocation,
    /// The same selectors carried through the owner codec's canonical JSON.
    #[cfg(feature = "tuning-profile")]
    TableRemainderEncoded,
    /// A compiled envelope admitting the bit-sliced interleaved family, with
    /// the buffer shapes watched across repeated batches.
    BitsliceInterleavedAllocation,
    /// Selectors admitting the bit-sliced interleaved family, carried through
    /// the owner codec's canonical JSON.
    #[cfg(feature = "tuning-profile")]
    BitsliceInterleavedEncoded,
    /// A compiled envelope admitting the carry-less-multiply fold family,
    /// with the buffer shapes watched across repeated batches.
    ClmulFoldAllocation,
    /// Selectors admitting the carry-less-multiply fold family, carried
    /// through the owner codec's canonical JSON.
    #[cfg(feature = "tuning-profile")]
    ClmulFoldEncoded,
    /// A compiled envelope admitting the carry-less-multiply fold family with
    /// [`FOLD_ACCUMULATORS`] and [`FOLD_RUN_BLOCKS`] as its overrides, with a
    /// workspace's fold shape reported when built and after a batch.
    ClmulFoldOverrides,
    /// A compiled envelope carrying a decode family beside selectors
    /// admitting the table family.
    DecodeFamilyCompiled,
    /// The same two families carried through the owner codec's canonical
    /// JSON.
    #[cfg(feature = "tuning-profile")]
    DecodeFamilyEncoded,
    /// [`MEASURED_OWNER`] installed, with the batch decoder's routes
    /// reported on either side of its decode selectors.
    #[cfg(feature = "tuning-profile")]
    MeasuredOwnerRoutes,
    /// [`MEASURED_OWNER`] installed, with the batch decoder held to the
    /// shared batch contract on the LDPC batch suite's frames, with the
    /// suite code's QC structure retained and without it.
    #[cfg(feature = "tuning-profile")]
    MeasuredOwnerDecode,
    /// [`MEASURED_OWNER`] installed, with the batch decoder's routes and
    /// results reported on NR BG1 Z=384 at each of [`QC_CALLS`], on the
    /// detected lane backend and held on the portable one.
    #[cfg(feature = "tuning-profile")]
    MeasuredOwnerQc,
}

impl FreshProcessCase {
    fn name(self) -> &'static str {
        match self {
            Self::ConservativeDefault => "conservative-default",
            Self::TableRemainderCompiled => "table-remainder-compiled",
            Self::TableRemainderAllocation => "table-remainder-allocation",
            #[cfg(feature = "tuning-profile")]
            Self::TableRemainderEncoded => "table-remainder-encoded",
            Self::BitsliceInterleavedAllocation => "bitslice-interleaved-allocation",
            #[cfg(feature = "tuning-profile")]
            Self::BitsliceInterleavedEncoded => "bitslice-interleaved-encoded",
            Self::ClmulFoldAllocation => "clmul-fold-allocation",
            #[cfg(feature = "tuning-profile")]
            Self::ClmulFoldEncoded => "clmul-fold-encoded",
            Self::ClmulFoldOverrides => "clmul-fold-overrides",
            Self::DecodeFamilyCompiled => "decode-family-compiled",
            #[cfg(feature = "tuning-profile")]
            Self::DecodeFamilyEncoded => "decode-family-encoded",
            #[cfg(feature = "tuning-profile")]
            Self::MeasuredOwnerRoutes => "measured-owner-routes",
            #[cfg(feature = "tuning-profile")]
            Self::MeasuredOwnerDecode => "measured-owner-decode",
            #[cfg(feature = "tuning-profile")]
            Self::MeasuredOwnerQc => "measured-owner-qc",
        }
    }

    fn canonical_json(self) -> String {
        format!("{{\"case\":\"{}\"}}", self.name())
    }

    fn parse(value: &serde_json::Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or("fresh-process case is not an object")?;
        if object.len() != 1 {
            return Err("fresh-process case has unexpected fields".to_owned());
        }
        match object.get("case").and_then(serde_json::Value::as_str) {
            Some("conservative-default") => Ok(Self::ConservativeDefault),
            Some("table-remainder-compiled") => Ok(Self::TableRemainderCompiled),
            Some("table-remainder-allocation") => Ok(Self::TableRemainderAllocation),
            #[cfg(feature = "tuning-profile")]
            Some("table-remainder-encoded") => Ok(Self::TableRemainderEncoded),
            Some("bitslice-interleaved-allocation") => Ok(Self::BitsliceInterleavedAllocation),
            #[cfg(feature = "tuning-profile")]
            Some("bitslice-interleaved-encoded") => Ok(Self::BitsliceInterleavedEncoded),
            Some("clmul-fold-allocation") => Ok(Self::ClmulFoldAllocation),
            #[cfg(feature = "tuning-profile")]
            Some("clmul-fold-encoded") => Ok(Self::ClmulFoldEncoded),
            Some("clmul-fold-overrides") => Ok(Self::ClmulFoldOverrides),
            Some("decode-family-compiled") => Ok(Self::DecodeFamilyCompiled),
            #[cfg(feature = "tuning-profile")]
            Some("decode-family-encoded") => Ok(Self::DecodeFamilyEncoded),
            #[cfg(feature = "tuning-profile")]
            Some("measured-owner-routes") => Ok(Self::MeasuredOwnerRoutes),
            #[cfg(feature = "tuning-profile")]
            Some("measured-owner-decode") => Ok(Self::MeasuredOwnerDecode),
            #[cfg(feature = "tuning-profile")]
            Some("measured-owner-qc") => Ok(Self::MeasuredOwnerQc),
            _ => Err("fresh-process case has an unknown case name".to_owned()),
        }
    }
}

pub fn fresh_tuning_process(case: FreshProcessCase) -> Result<serde_json::Value, String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("test binary has no path: {error}"))?;
    let mut child = Command::new(&executable)
        .args(["--exact", "fresh_tuning_process_child", "--nocapture"])
        .env(SENTINEL, SENTINEL_VALUE)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot spawn {}: {error}", executable.display()))?;
    child
        .stdin
        .take()
        .ok_or("fresh-process child has no stdin")?
        .write_all(case.canonical_json().as_bytes())
        .map_err(|error| format!("cannot write child case: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("cannot wait for child: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "fresh-process child exited with {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| "fresh-process child stdout is not UTF-8".to_owned())?;
    let results: Vec<&str> = stdout
        .lines()
        // With one test thread, libtest's `test ... ` opens the same line.
        .filter_map(|line| line.split_once(RESULT_PREFIX).map(|(_, result)| result))
        .collect();
    if results.len() != 1 {
        return Err(format!(
            "fresh-process child emitted {} structured results, expected one\n{stdout}",
            results.len()
        ));
    }
    serde_json::from_str(results[0])
        .map_err(|error| format!("fresh-process result is not JSON: {error}"))
}

pub fn child_case() -> Result<Option<FreshProcessCase>, String> {
    let sentinel = match std::env::var(SENTINEL) {
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(error) => return Err(format!("cannot read fresh-process sentinel: {error}")),
        Ok(value) => value,
    };
    if sentinel != SENTINEL_VALUE {
        return Err(format!("invalid fresh-process sentinel {sentinel:?}"));
    }
    let mut input = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)
        .map_err(|error| format!("cannot read fresh-process stdin: {error}"))?;
    let value: serde_json::Value = serde_json::from_str(&input)
        .map_err(|error| format!("fresh-process case is not JSON: {error}"))?;
    let case = FreshProcessCase::parse(&value)?;
    if input != case.canonical_json() {
        return Err("fresh-process case is not in canonical compact form".to_owned());
    }
    Ok(Some(case))
}

/// The corpus row B3 of the workload-selection contract: the smallest row
/// whose redundancy makes the table family available.
fn corpus_row_b3() -> BinaryBchCode {
    let extension = BinaryPrimeExt::new(Gf2mField::new(8, 0b1_0001_1101))
        .expect("the contract's B3 primitive polynomial");
    BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
        extension,
        designed_distance: DesignedDistance::try_from(9).expect("a positive designed distance"),
    })
    .expect("a narrow-sense construction over GF(2^8)")
}

/// `encode` beside the conservative decode family.
fn with_conservative_decode(encode: EncodeSelectors) -> CodingSelectors {
    CodingSelectors {
        encode,
        decode: DecodeSelectors::CONSERVATIVE,
    }
}

/// A selector bound no redundancy or batch length reaches.
const CLOSED: usize = usize::MAX;

/// Selectors admitting the table family, with the bit-sliced family closed so the table arm is the one the seam reaches.
fn selectors_admitting_the_table_family() -> CodingSelectors {
    with_conservative_decode(
        EncodeSelectors::try_new(TABLE_REMAINDER_BLOCK_BITS, BATCH, CLOSED, CLOSED)
            .expect("every selector bound is admissible"),
    )
}

/// The table-family selectors beside [`INSTALLED_DECODE`], so one installed
/// section moves both families off their conservative values.
fn selectors_carrying_a_decode_family() -> CodingSelectors {
    let (lane_min_frames, lane_max_edges, granules_per_task) = INSTALLED_DECODE;
    CodingSelectors {
        decode: DecodeSelectors::try_new(lane_min_frames, lane_max_edges, 1, granules_per_task)
            .expect("a positive group count"),
        ..selectors_admitting_the_table_family()
    }
}

/// The decode family as the child reports it and the parent compares it.
pub fn decode_report(decode: &DecodeSelectors) -> serde_json::Value {
    serde_json::json!({
        "lane_min_frames": decode.lane_min_frames(),
        "lane_max_edges": decode.lane_max_edges(),
        "lane_max_workers": decode.lane_max_workers().get(),
        "granules_per_task": decode.granules_per_task().get(),
    })
}

/// Selectors admitting the bit-sliced interleaved family, with the others
/// closed.
fn selectors_admitting_the_bitslice_family() -> CodingSelectors {
    with_conservative_decode(
        EncodeSelectors::try_new(CLOSED, CLOSED, BATCH, CLOSED)
            .expect("every selector bound is admissible"),
    )
}

/// Selectors admitting the carry-less-multiply fold family, with the others
/// closed.
fn selectors_admitting_the_clmul_family() -> CodingSelectors {
    with_conservative_decode(
        EncodeSelectors::try_new(CLOSED, CLOSED, CLOSED, BATCH)
            .expect("every selector bound is admissible"),
    )
}

fn compiled_envelope(selectors: CodingSelectors) -> PreparedEnvelope {
    let id = ProfileId::parse("encode-dispatch-test").unwrap();
    PreparedEnvelope::compiled(
        id.clone(),
        CompiledProfileProvenance {
            artifact_id: id.clone(),
        },
    )
    .insert(CodingTuning::from_selectors(selectors))
    .unwrap()
    .build()
    .unwrap()
}

/// The registry of the coding owner codec alone.
#[cfg(feature = "tuning-profile")]
pub fn coding_registry() -> ProfileRegistry {
    ProfileRegistryBuilder::new()
        .register::<CodingTuning, CodingTuningCodec>()
        .unwrap()
        .build()
        .unwrap()
}

/// The committed production coding owner envelope: the calibrated selectors
/// of the decoder calibration campaign `@/issue/20af7e6e`.
#[cfg(feature = "tuning-profile")]
pub const MEASURED_OWNER: &str =
    include_str!("../../data/tuning-profiles/gf2-c62356d2-20261109t233411z-3067230.json");

/// [`MEASURED_OWNER`] read through the owner codec.
#[cfg(feature = "tuning-profile")]
pub fn measured_owner() -> PreparedEnvelope {
    coding_registry().from_json(MEASURED_OWNER).unwrap()
}

#[cfg(feature = "tuning-profile")]
fn encoded_envelope(selectors: CodingSelectors) -> PreparedEnvelope {
    let typed = compiled_envelope(selectors);
    let registry = coding_registry();
    let assembly = AssemblyProvenance {
        assembled_at: Rfc3339Utc::parse("2026-09-01T00:00:00Z").unwrap(),
        source_revision: GitRevision::parse("0123456789abcdef0123456789abcdef01234567").unwrap(),
        source_dirty: false,
        tool: RepoRelPath::parse("crates/gf2-coding/tests/support/fresh_tuning_process.rs")
            .unwrap(),
        tool_sha256: Sha256::parse(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap(),
    };
    let json = registry.to_json(&typed, &assembly).unwrap();
    registry.from_json(&json).unwrap()
}

/// Runs `case` in the child process.
///
/// # Panics
///
/// Panics when the profile install or a case assertion fails.
pub fn execute_child(case: FreshProcessCase) -> serde_json::Value {
    match case {
        FreshProcessCase::ConservativeDefault => execute_conservative_child(),
        FreshProcessCase::TableRemainderCompiled => {
            execute_install_child(compiled_envelope(selectors_admitting_the_table_family()))
        }
        FreshProcessCase::TableRemainderAllocation => execute_allocation_child(
            selectors_admitting_the_table_family(),
            EncodeFamily::TableRemainder,
        ),
        #[cfg(feature = "tuning-profile")]
        FreshProcessCase::TableRemainderEncoded => {
            execute_install_child(encoded_envelope(selectors_admitting_the_table_family()))
        }
        FreshProcessCase::BitsliceInterleavedAllocation => execute_allocation_child(
            selectors_admitting_the_bitslice_family(),
            EncodeFamily::BitsliceInterleaved,
        ),
        #[cfg(feature = "tuning-profile")]
        FreshProcessCase::BitsliceInterleavedEncoded => {
            execute_install_child(encoded_envelope(selectors_admitting_the_bitslice_family()))
        }
        FreshProcessCase::ClmulFoldAllocation => execute_allocation_child(
            selectors_admitting_the_clmul_family(),
            EncodeFamily::ClmulFold,
        ),
        #[cfg(feature = "tuning-profile")]
        FreshProcessCase::ClmulFoldEncoded => {
            execute_install_child(encoded_envelope(selectors_admitting_the_clmul_family()))
        }
        FreshProcessCase::ClmulFoldOverrides => execute_fold_overrides_child(),
        FreshProcessCase::DecodeFamilyCompiled => {
            execute_install_child(compiled_envelope(selectors_carrying_a_decode_family()))
        }
        #[cfg(feature = "tuning-profile")]
        FreshProcessCase::DecodeFamilyEncoded => {
            execute_install_child(encoded_envelope(selectors_carrying_a_decode_family()))
        }
        #[cfg(feature = "tuning-profile")]
        FreshProcessCase::MeasuredOwnerRoutes => execute_measured_routes_child(),
        #[cfg(feature = "tuning-profile")]
        FreshProcessCase::MeasuredOwnerDecode => execute_measured_decode_child(),
        #[cfg(feature = "tuning-profile")]
        FreshProcessCase::MeasuredOwnerQc => execute_measured_qc_child(),
    }
}

/// The worker counts every case encodes at, spanning the contract's
/// $W \in \{1, 6\}$ and the boundaries either side of it.
const WORKERS: &[usize] = &[1, 2, 6, 7];

/// Encodes the row through every batch entry point and compares the result
/// with the reference family's bytes.
///
/// Returns the selected family and whether every route agreed. The parallel
/// routes are what witness that a worker count changes neither the selected
/// family nor the bytes it writes.
fn encode_and_compare(code: &BinaryBchCode) -> (EncodeFamily, bool) {
    let layout = SystematicLayout::default();
    let selected = code.selected_encode_family(layout, BATCH);
    let mut agrees = selected == code.selected_encode_family(layout, LANE_GROUP_BATCH);
    for length in [BATCH, LANE_GROUP_BATCH] {
        agrees &= encode_and_compare_at(code, length);
    }
    (selected, agrees)
}

/// One batch length of [`encode_and_compare`], through every batch entry
/// point and every declared worker count.
fn encode_and_compare_at(code: &BinaryBchCode, length: usize) -> bool {
    let layout = SystematicLayout::default();
    let messages: Vec<BitVec> = (0..length)
        .map(|index| BitVec::random_seeded(code.k(), index as u64 + 1))
        .collect();

    let mut reference = vec![BitVec::zeros(code.n()); messages.len()];
    let mut workspace = code.encode_workspace();
    code.encode_batch_family_into(
        EncodeFamily::REFERENCE,
        &messages,
        layout,
        &mut workspace,
        &mut reference,
    )
    .expect("the reference family is available for every code");

    let mut agrees = code
        .encode_batch(&messages, layout)
        .expect("a validated batch encodes")
        == reference;
    for &workers in WORKERS {
        let workers = NonZeroUsize::new(workers).expect("a positive worker count");
        agrees &= code
            .encode_batch_parallel(&messages, layout, workers)
            .expect("a validated batch encodes")
            == reference;
    }
    agrees
}

fn execute_conservative_child() -> serde_json::Value {
    let code = corpus_row_b3();
    assert!(code.encode_family_available(EncodeFamily::TableRemainder, SystematicLayout::default()));
    let (selected, agrees) = encode_and_compare(&code);
    let active = tuning::active();
    assert_eq!(active.section, &CodingTuning::CONSERVATIVE);
    assert!(matches!(
        active.resolution,
        SectionResolution::FrozenBeforeInstall { .. }
    ));
    serde_json::json!({
        "family": selected.name(),
        "agrees_with_reference": agrees,
        "decode": decode_report(active.section.decode()),
        "resolution": "frozen-before-install",
    })
}

/// Installs selectors admitting the fold family with [`FOLD_ACCUMULATORS`]
/// and [`FOLD_RUN_BLOCKS`], then reports the fold shape a workspace holds when
/// built and after a batch under them.
///
/// Installation comes first because constructing the code's splitting field
/// resolves the process. A workspace built under the installed overrides
/// still holding the structural shape is the witness that building it reads
/// no profile; the batch is where the overrides take effect.
fn execute_fold_overrides_child() -> serde_json::Value {
    let mut selectors = selectors_admitting_the_clmul_family();
    selectors.encode = selectors
        .encode
        .with_clmul_fold_accumulators(NonZeroUsize::new(FOLD_ACCUMULATORS).unwrap())
        .with_clmul_fold_run_blocks(NonZeroUsize::new(FOLD_RUN_BLOCKS).unwrap());
    gf2_core::tuning::install(compiled_envelope(selectors)).unwrap();

    let code = corpus_row_b3();
    let layout = SystematicLayout::default();
    let mut workspace = code.encode_workspace();
    let built =
        test_support::encode_fold_run(&workspace).expect("a workspace prepares the fold family");
    let messages: Vec<BitVec> = (0..BATCH)
        .map(|index| BitVec::random_seeded(code.k(), index as u64 + 1))
        .collect();
    let mut codewords = vec![BitVec::zeros(code.n()); messages.len()];
    code.encode_batch_into(&messages, layout, &mut workspace, &mut codewords)
        .expect("a validated batch encodes");
    let encoded = test_support::encode_fold_run(&workspace)
        .expect("a workspace keeps the fold family prepared");
    let mut reference = vec![BitVec::zeros(code.n()); messages.len()];
    code.encode_batch_family_into(
        EncodeFamily::REFERENCE,
        &messages,
        layout,
        &mut code.encode_workspace(),
        &mut reference,
    )
    .expect("the reference family is available for every code");
    let (selected, agrees) = encode_and_compare(&code);
    serde_json::json!({
        "family": selected.name(),
        "agrees_with_reference": agrees && codewords == reference,
        "built": [built.0, built.1],
        "structural_accumulators": clmul_fold_accumulators(code.n() - code.k())
            .map_or(0, NonZeroUsize::get),
        "structural_run_blocks": clmul_fold_run_blocks(code.k()).get(),
        "overridden": [encoded.0, encoded.1],
        "resolution": "installed",
    })
}

fn execute_install_child(prepared: PreparedEnvelope) -> serde_json::Value {
    gf2_core::tuning::install(prepared).unwrap();
    let active = tuning::active();
    assert!(matches!(
        active.resolution,
        SectionResolution::Installed { .. }
    ));
    let code = corpus_row_b3();
    let (selected, agrees) = encode_and_compare(&code);
    serde_json::json!({
        "family": selected.name(),
        "agrees_with_reference": agrees,
        "decode": decode_report(active.section.decode()),
        "resolution": "installed",
    })
}

/// Watches the buffers each batch path owns while the installed profile has
/// `expected` selected.
///
/// The workspace claim is the strict one: its shape is read before any
/// encode, so an equal shape after the first batch witnesses that the family
/// preparation happened when the workspace was built, not when the batch
/// selected it. The thread scratch is the entry point that allocates its own
/// result, so its claim is steady state: it sizes on the first batch in this
/// family and reuses that sizing afterwards.
fn execute_allocation_child(
    selectors: CodingSelectors,
    expected: EncodeFamily,
) -> serde_json::Value {
    gf2_core::tuning::install(compiled_envelope(selectors)).unwrap();
    let code = corpus_row_b3();
    let layout = SystematicLayout::default();
    assert_eq!(
        code.selected_encode_family(layout, BATCH),
        expected,
        "this case is only evidence while {expected} is the selected family"
    );

    let messages: Vec<BitVec> = (0..BATCH)
        .map(|index| BitVec::random_seeded(code.k(), index as u64 + 1))
        .collect();
    let mut codewords = vec![BitVec::zeros(code.n()); messages.len()];

    let mut workspace = code.encode_workspace();
    let built = test_support::encode_workspace_shape(&workspace);
    let prepared = match expected {
        EncodeFamily::BitsliceInterleaved => !workspace.registers().lanes.is_empty(),
        EncodeFamily::ClmulFold => !workspace.registers().fold.is_empty(),
        _ => !workspace.registers().tables.is_empty(),
    };
    let mut workspace_stable = prepared;
    for _ in 0..8 {
        code.encode_batch_into(&messages, layout, &mut workspace, &mut codewords)
            .expect("a validated batch encodes");
        workspace_stable &= test_support::encode_workspace_shape(&workspace) == built;
    }

    code.encode_batch(&messages, layout)
        .expect("a validated batch encodes");
    let sized = test_support::encode_scratch_shape::<u64>()
        .expect("the first packed batch sizes this thread's scratch");
    let mut scratch_stable = true;
    for _ in 0..8 {
        code.encode_batch(&messages, layout)
            .expect("a validated batch encodes");
        scratch_stable &= test_support::encode_scratch_shape::<u64>().as_ref() == Some(&sized);
    }

    serde_json::json!({
        "family": code.selected_encode_family(layout, BATCH).name(),
        "workspace_shape_stable_from_creation": workspace_stable,
        "scratch_shape_stable_after_first_batch": scratch_stable,
        "resolution": "installed",
    })
}

/// A batch route as the child reports it and the parent compares it.
#[cfg(feature = "tuning-profile")]
pub fn route_report(route: BatchRoute) -> serde_json::Value {
    format!("{route:?}").into()
}

/// Installs [`MEASURED_OWNER`] and returns its decode family.
#[cfg(feature = "tuning-profile")]
fn install_measured_owner() -> &'static DecodeSelectors {
    gf2_core::tuning::install(measured_owner()).unwrap();
    let active = tuning::active();
    assert!(matches!(
        active.resolution,
        SectionResolution::Installed { .. }
    ));
    active.section.decode()
}

/// A code of exactly `edges` Tanner-graph edges, each variable of degree one
/// over eight checks; only its edge count matters to a route.
#[cfg(feature = "tuning-profile")]
fn code_with_edges(edges: usize) -> LdpcCode {
    const CHECKS: usize = 8;
    let coordinates: Vec<(usize, usize)> = (0..edges).map(|v| (v % CHECKS, v)).collect();
    LdpcCode::from_edges(CHECKS, edges, &coordinates)
}

/// Reports, for codes at the decode family's edge bound and one edge past
/// it, the route of every wave size up to two waves, on the detected lane
/// backend and held on the portable one.
#[cfg(feature = "tuning-profile")]
fn execute_measured_routes_child() -> serde_json::Value {
    let decode = install_measured_owner();
    let bound = decode.lane_max_edges();
    let mut routes = Vec::new();
    for edges in [bound, bound + 1] {
        let decoder = LdpcBatchDecoder::new(
            code_with_edges(edges),
            DecoderConfig::default(),
            test_support::LDPC_BATCH_CAP,
            DecodedWord::Codeword,
        );
        let detected: Vec<BatchRoute> = (0..=2 * LANES)
            .map(|frames| decoder.batch_route(frames))
            .collect();
        let portable = decoder.with_portable_lanes();
        for (frames, detected) in detected.into_iter().enumerate() {
            routes.push(serde_json::json!({
                "edges": edges,
                "frames": frames,
                "detected": route_report(detected),
                "portable": route_report(portable.batch_route(frames)),
            }));
        }
    }
    serde_json::json!({
        "decode": decode_report(decode),
        "routes": routes,
        "resolution": "installed",
    })
}

/// The LDPC batch suite's code without its retained QC structure.
#[cfg(feature = "tuning-profile")]
pub fn unstructured_batch_code() -> LdpcCode {
    test_support::ldpc_batch_code().without_quasi_cyclic()
}

/// Holds the installed decoder of every min-sum rule to the shared batch
/// contract against the single-frame decoder, on the suite's code with its
/// QC structure and without it, and reports the route of each wave size of
/// both.
#[cfg(feature = "tuning-profile")]
fn execute_measured_decode_child() -> serde_json::Value {
    let decode = install_measured_owner();
    let frames = test_support::ldpc_batch_frames();
    let cap = test_support::LDPC_BATCH_CAP;
    let mut wave_routes = serde_json::Map::new();
    for (name, code) in [
        ("structured", test_support::ldpc_batch_code()),
        ("unstructured", unstructured_batch_code()),
    ] {
        for algorithm in [
            DecoderAlgorithm::MinSum,
            DecoderAlgorithm::NormalizedMinSum(0.75),
            DecoderAlgorithm::OffsetMinSum(0.5),
        ] {
            let config = DecoderConfig::new(algorithm, true);
            let mut decoder =
                LdpcBatchDecoder::new(code.clone(), config, cap, DecodedWord::Codeword);
            let mut single = LdpcDecoder::with_config(code.clone(), config);
            batch_soft_decoder_contract(
                &mut decoder,
                |llrs| single.decode_to_codeword(llrs, cap),
                &frames,
                ConvergenceMix::EarlyLateAndNonConverging,
            );
        }
        let decoder =
            LdpcBatchDecoder::new(code, DecoderConfig::default(), cap, DecodedWord::Codeword);
        let routes: Vec<serde_json::Value> = (0..=LANES)
            .map(|frames| route_report(decoder.batch_route(frames)))
            .collect();
        wave_routes.insert(name.to_owned(), routes.into());
    }
    serde_json::json!({
        "decode": decode_report(decode),
        "wave_routes": wave_routes,
        "resolution": "installed",
    })
}

/// The `(workers, frames)` calls [`FreshProcessCase::MeasuredOwnerQc`]
/// reports: a wave of one, two and eight frames, a longer batch, and worker
/// counts on either side of the lane route's worker bound.
#[cfg(feature = "tuning-profile")]
pub const QC_CALLS: &[(usize, usize)] = &[
    (1, 1),
    (1, 2),
    (1, 8),
    (1, 9),
    (6, 8),
    (12, 1),
    (12, 8),
    (24, 8),
];

/// Reports the route of each of [`QC_CALLS`] on NR BG1 Z=384 with its
/// structure attached and without it, and decodes the attached code's calls
/// against the single-frame decoder on both lane backends.
#[cfg(feature = "tuning-profile")]
fn execute_measured_qc_child() -> serde_json::Value {
    use gf2_coding::ldpc::QuasiCyclicLdpc;
    use gf2_coding::llr::Llr;
    use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome};

    let decode = install_measured_owner();
    let structure = QuasiCyclicLdpc::nr_5g(1, 384);
    let attached = LdpcCode::from_quasi_cyclic(&structure);
    let unattached = LdpcCode::from_edges(
        structure.expanded_rows(),
        structure.expanded_cols(),
        &structure.to_edges(),
    );
    let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
    let cap = 2;
    let mut single = LdpcDecoder::with_config(attached.clone(), config);
    let mut reports = Vec::new();
    for (structured, code) in [(true, &attached), (false, &unattached)] {
        for portable in [false, true] {
            let mut decoder =
                LdpcBatchDecoder::new(code.clone(), config, cap, DecodedWord::Codeword);
            if portable {
                decoder = decoder.with_portable_lanes();
            }
            for &(workers, frames) in QC_CALLS {
                let workers = NonZeroUsize::new(workers).unwrap();
                let route = decoder.batch_route_with_workers(frames, workers);
                let mut executed = serde_json::Value::Null;
                if structured {
                    let llrs: Vec<Vec<Llr>> = (0..frames)
                        .map(|frame| {
                            (0..code.n())
                                .map(|i| {
                                    Llr::new(if (i + frame).is_multiple_of(31) {
                                        -1.0
                                    } else {
                                        2.0
                                    })
                                })
                                .collect()
                        })
                        .collect();
                    let views: Vec<&[Llr]> = llrs.iter().map(Vec::as_slice).collect();
                    let mut decoded = vec![BitVec::new(); frames];
                    let mut outcomes = vec![DecodeOutcome::default(); frames];
                    decoder.decode_batch_with_workers(&views, &mut decoded, &mut outcomes, workers);
                    for (frame, view) in views.iter().enumerate() {
                        let expected = single.decode_to_codeword(view, cap);
                        assert_eq!(decoded[frame], expected.decoded_bits, "frame {frame}");
                        assert_eq!(outcomes[frame], expected.outcome(), "frame {frame}");
                    }
                    executed = route_report(
                        decoder
                            .last_wave_route()
                            .expect("a decoded call has a last wave"),
                    );
                }
                reports.push(serde_json::json!({
                    "structured": structured,
                    "portable": portable,
                    "workers": workers.get(),
                    "frames": frames,
                    "route": route_report(route),
                    "executed": executed,
                }));
            }
        }
    }
    serde_json::json!({
        "decode": decode_report(decode),
        "routes": reports,
        "resolution": "installed",
    })
}

pub fn emit_result(value: serde_json::Value) {
    println!(
        "{RESULT_PREFIX}{}",
        serde_json::to_string(&value).expect("structured result is serializable")
    );
}
