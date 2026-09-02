//! Guarded fresh-process protocol for the coding tuning section.
//!
//! Installing a profile resolves the process-wide tuning authority once, so
//! every case that needs its own installed value runs in a child process
//! started from the test binary. This mirrors the algebra owner's protocol at
//! `crates/gf2-algebra/tests/support/fresh_tuning_process.rs`.

use std::io::Write;
use std::num::NonZeroUsize;
use std::process::{Command, Stdio};

use gf2_coding::bch::encode::{
    EncodeFamily, SystematicLayout, BITSLICE_INTERLEAVED_MIN_BATCH, TABLE_REMAINDER_BLOCK_BITS,
    TABLE_REMAINDER_MIN_BATCH, TABLE_REMAINDER_MIN_REDUNDANCY,
};
use gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
use gf2_coding::test_support;
use gf2_coding::tuning::{self, CodingTuning, EncodeSelectors};
use gf2_core::field::extension::BinaryPrimeExt;
use gf2_core::gf2m::Gf2mField;
use gf2_core::tuning::{CompiledProfileProvenance, PreparedEnvelope, ProfileId, SectionResolution};
use gf2_core::BitVec;

#[cfg(feature = "tuning-profile")]
use gf2_coding::tuning::CodingTuningCodec;
#[cfg(feature = "tuning-profile")]
use gf2_core::tuning::{
    AssemblyProvenance, GitRevision, ProfileRegistryBuilder, RepoRelPath, Rfc3339Utc, Sha256,
};

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
        .filter_map(|line| line.strip_prefix(RESULT_PREFIX))
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

/// Selectors admitting the table family, with the bit-sliced family left on
/// its conservative exclusion so the table arm is the one the seam reaches.
fn selectors_admitting_the_table_family() -> EncodeSelectors {
    EncodeSelectors::try_new(
        TABLE_REMAINDER_BLOCK_BITS,
        BATCH,
        BITSLICE_INTERLEAVED_MIN_BATCH,
    )
    .expect("every selector bound is admissible")
}

/// Selectors admitting the bit-sliced interleaved family, with the table
/// family left on its conservative exclusion.
fn selectors_admitting_the_bitslice_family() -> EncodeSelectors {
    EncodeSelectors::try_new(
        TABLE_REMAINDER_MIN_REDUNDANCY,
        TABLE_REMAINDER_MIN_BATCH,
        BATCH,
    )
    .expect("every selector bound is admissible")
}

fn compiled_envelope(selectors: EncodeSelectors) -> PreparedEnvelope {
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

#[cfg(feature = "tuning-profile")]
fn encoded_envelope(selectors: EncodeSelectors) -> PreparedEnvelope {
    let typed = compiled_envelope(selectors);
    let registry = ProfileRegistryBuilder::new()
        .register::<CodingTuning, CodingTuningCodec>()
        .unwrap()
        .build()
        .unwrap();
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
        "resolution": "frozen-before-install",
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
    selectors: EncodeSelectors,
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

pub fn emit_result(value: serde_json::Value) {
    println!(
        "{RESULT_PREFIX}{}",
        serde_json::to_string(&value).expect("structured result is serializable")
    );
}
