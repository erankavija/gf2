//! Stream the committed F_7 permanent-equality corpus to either wave kernel.
//!
//! This opt-in evidence driver is the only normal project path that evaluates
//! generic Ryser references at orders 20 and 24. The existing measurement
//! registry selects the lookup-table control or the three-plane candidate;
//! there is no driver-local candidate registry. Run it only on a ROCm gfx1030
//! host:
//!
//! ```sh
//! cargo +1.95.0 run --manifest-path dev/research/permanent_wave_gpu/Cargo.toml \
//!     --release --features hip --bin wave-gf7-device-evidence -- \
//!     --path f7-three-plane-permanent
//! ```

use std::env;
use std::error::Error;
use std::io::Write;
use std::process::{Command, Stdio};

use gf2_algebra::packed::packed7::{ADD_LUT, MUL_LUT, SUB_LUT};
use gf2_algebra::permanent::permanent_ryser;
use gf2_core::gfp::Fp;
use permanent_wave_gpu::fixtures::{Fixture, FixtureCorpus, DEFAULT_FIXTURE_SEED};
use permanent_wave_gpu::MeasurementPath;

const FULL_STREAM_MAGIC: [u8; 8] = *b"GF2WAVE1";
const PREPARATION_STREAM_MAGIC: [u8; 8] = *b"GF2PREP1";
const DEVICE_EVIDENCE_ORDERS: [usize; 3] = [16, 20, 24];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EvidenceStage {
    FullPermanent,
    ThreePlanePreparation,
}

fn write_u32_le(writer: &mut impl Write, value: u32) -> Result<(), Box<dyn Error>> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn write_u64_le(writer: &mut impl Write, value: u64) -> Result<(), Box<dyn Error>> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn ryser_f7(fixture: &Fixture) -> u64 {
    permanent_ryser::<Fp<7>>(
        &fixture
            .matrix_bytes()
            .iter()
            .map(|&value| Fp::<7>::new(u64::from(value)))
            .collect::<Vec<_>>(),
        fixture.n(),
    )
    .value()
}

fn selected_f7_fixtures(corpus: &FixtureCorpus) -> Vec<&Fixture> {
    corpus
        .fixtures()
        .iter()
        .filter(|fixture| fixture.q() == 7 && DEVICE_EVIDENCE_ORDERS.contains(&fixture.n()))
        .collect()
}

fn write_fixture_stream(
    writer: &mut impl Write,
    fixtures: &[&Fixture],
) -> Result<(), Box<dyn Error>> {
    let fixture_count = u32::try_from(fixtures.len())?;
    // The direct lookup-control evidence uploads the public Packed7 table
    // bytes exactly as production initialization does. The HIP executable
    // treats this prefix as authoritative; it does not regenerate it.
    writer.write_all(&FULL_STREAM_MAGIC)?;
    writer.write_all(&ADD_LUT)?;
    writer.write_all(&SUB_LUT)?;
    writer.write_all(&MUL_LUT)?;
    write_u32_le(writer, fixture_count)?;
    for fixture in fixtures {
        let id = fixture.id().as_bytes();
        let bytes = fixture.matrix_bytes();
        write_u32_le(writer, u32::try_from(id.len())?)?;
        writer.write_all(id)?;
        write_u32_le(writer, u32::try_from(fixture.n())?)?;
        write_u64_le(writer, ryser_f7(fixture))?;
        write_u64_le(writer, u64::try_from(bytes.len())?)?;
        writer.write_all(bytes)?;
    }
    Ok(())
}

/// Stream only canonical input bytes for the isolated bit-plane preparation
/// stage. Unlike [`write_fixture_stream`], this intentionally never evaluates
/// a permanent or constructs a Ryser reference.
fn write_preparation_stream(
    writer: &mut impl Write,
    fixtures: &[&Fixture],
) -> Result<(), Box<dyn Error>> {
    let fixture_count = u32::try_from(fixtures.len())?;
    writer.write_all(&PREPARATION_STREAM_MAGIC)?;
    write_u32_le(writer, fixture_count)?;
    for fixture in fixtures {
        let id = fixture.id().as_bytes();
        let bytes = fixture.matrix_bytes();
        write_u32_le(writer, u32::try_from(id.len())?)?;
        writer.write_all(id)?;
        write_u32_le(writer, u32::try_from(fixture.n())?)?;
        write_u64_le(writer, u64::try_from(bytes.len())?)?;
        writer.write_all(bytes)?;
    }
    Ok(())
}

fn selected_f7_path(arguments: &[String]) -> Result<MeasurementPath, Box<dyn Error>> {
    match arguments {
        [] => Ok(MeasurementPath::F7LookupTableControl),
        [flag, path]
            if flag == "--path" && path == MeasurementPath::F7LookupTableControl.name() =>
        {
            Ok(MeasurementPath::F7LookupTableControl)
        }
        [flag, path] if flag == "--path" && path == MeasurementPath::F7ThreePlanePermanent.name() => {
            Ok(MeasurementPath::F7ThreePlanePermanent)
        }
        _ => Err(
            "usage: wave-gf7-device-evidence [--path f7-lookup-table-control|f7-three-plane-permanent]"
                .into(),
        ),
    }
}

fn selected_evidence(
    arguments: &[String],
) -> Result<(MeasurementPath, EvidenceStage), Box<dyn Error>> {
    match arguments {
        [path_flag, path, stage_flag, stage]
            if path_flag == "--path"
                && path == MeasurementPath::F7ThreePlanePermanent.name()
                && stage_flag == "--stage"
                && stage == "three-plane-preparation" =>
        {
            Ok((
                MeasurementPath::F7ThreePlanePermanent,
                EvidenceStage::ThreePlanePreparation,
            ))
        }
        _ => Ok((selected_f7_path(arguments)?, EvidenceStage::FullPermanent)),
    }
}

fn run_evidence(path: MeasurementPath, stage: EvidenceStage) -> Result<(), Box<dyn Error>> {
    if !matches!(
        path,
        MeasurementPath::F7LookupTableControl | MeasurementPath::F7ThreePlanePermanent
    ) {
        return Err(format!("{} is not an F_7 wave permanent candidate", path.name()).into());
    }
    path.dispatch().map_err(|unsupported| {
        format!("{} is unavailable: {}", path.name(), unsupported.reason())
    })?;

    let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
    let fixtures = selected_f7_fixtures(&corpus);
    if fixtures.is_empty()
        || DEVICE_EVIDENCE_ORDERS
            .iter()
            .any(|&n| !fixtures.iter().any(|fixture| fixture.n() == n))
    {
        return Err("the committed fixture corpus must cover F_7 orders 16, 20, and 24".into());
    }
    let fixture_count = fixtures.len();

    let mut command = Command::new(env!("PERMANENT_WAVE_GPU_WAVE_GF7_EQUIVALENCE_BIN"));
    command.args(["--path", path.name()]);
    if stage == EvidenceStage::ThreePlanePreparation {
        command.args(["--stage", "three-plane-preparation"]);
    }
    let mut child = command
        .arg("--fixture-stdin")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    let mut input = child
        .stdin
        .take()
        .ok_or("the F_7 wave evidence executable did not provide stdin")?;
    match stage {
        EvidenceStage::FullPermanent => write_fixture_stream(&mut input, &fixtures)?,
        EvidenceStage::ThreePlanePreparation => write_preparation_stream(&mut input, &fixtures)?,
    }
    drop(input);

    let status = child.wait()?;
    if !status.success() {
        return Err(format!("{} evidence executable failed: {status}", path.name()).into());
    }
    match stage {
        EvidenceStage::FullPermanent => println!(
            "{} device equality passed for {fixture_count} committed F_7 fixtures at n=16,20,24",
            path.name()
        ),
        EvidenceStage::ThreePlanePreparation => println!(
            "{} bit-plane preparation launched for {fixture_count} committed F_7 fixtures at n=16,20,24",
            path.name()
        ),
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = env::args().skip(1).collect();
    let (path, stage) = selected_evidence(&arguments)?;
    run_evidence(path, stage)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_u32_le(bytes: &[u8], cursor: &mut usize) -> u32 {
        let value = u32::from_le_bytes(bytes[*cursor..*cursor + 4].try_into().unwrap());
        *cursor += 4;
        value
    }

    fn read_u64_le(bytes: &[u8], cursor: &mut usize) -> u64 {
        let value = u64::from_le_bytes(bytes[*cursor..*cursor + 8].try_into().unwrap());
        *cursor += 8;
        value
    }

    #[test]
    fn evidence_selection_uses_only_registered_f7_paths_and_retains_all_orders() {
        assert_eq!(
            selected_f7_path(&[]).unwrap(),
            MeasurementPath::F7LookupTableControl
        );
        assert_eq!(
            selected_f7_path(&["--path".to_owned(), "f7-lookup-table-control".to_owned()]).unwrap(),
            MeasurementPath::F7LookupTableControl
        );
        assert_eq!(
            selected_f7_path(&["--path".to_owned(), "f7-three-plane-permanent".to_owned()])
                .unwrap(),
            MeasurementPath::F7ThreePlanePermanent
        );
        assert!(selected_f7_path(&["--path".to_owned(), "wave-gf3".to_owned()]).is_err());
        assert_eq!(
            selected_evidence(&[
                "--path".to_owned(),
                "f7-three-plane-permanent".to_owned(),
                "--stage".to_owned(),
                "three-plane-preparation".to_owned(),
            ])
            .unwrap(),
            (
                MeasurementPath::F7ThreePlanePermanent,
                EvidenceStage::ThreePlanePreparation,
            )
        );

        let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
        let fixtures = selected_f7_fixtures(&corpus);
        for n in DEVICE_EVIDENCE_ORDERS {
            assert!(fixtures.iter().any(|fixture| fixture.n() == n));
        }
    }

    #[test]
    fn full_stream_round_trips_in_the_hip_parser_field_order() {
        let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
        let fixture = corpus
            .fixtures()
            .iter()
            .find(|fixture| fixture.q() == 7 && fixture.n() == 1)
            .expect("the committed corpus must retain an F_7 singleton fixture");
        let mut stream = Vec::new();
        write_fixture_stream(&mut stream, &[fixture]).unwrap();
        let mut cursor = 0;
        assert_eq!(
            &stream[cursor..cursor + FULL_STREAM_MAGIC.len()],
            FULL_STREAM_MAGIC
        );
        cursor += FULL_STREAM_MAGIC.len();
        assert_eq!(&stream[cursor..cursor + ADD_LUT.len()], &ADD_LUT);
        cursor += ADD_LUT.len();
        assert_eq!(&stream[cursor..cursor + SUB_LUT.len()], &SUB_LUT);
        cursor += SUB_LUT.len();
        assert_eq!(&stream[cursor..cursor + MUL_LUT.len()], &MUL_LUT);
        cursor += MUL_LUT.len();
        assert_eq!(read_u32_le(&stream, &mut cursor), 1);
        let id_len = read_u32_le(&stream, &mut cursor) as usize;
        assert_eq!(&stream[cursor..cursor + id_len], fixture.id().as_bytes());
        cursor += id_len;
        assert_eq!(read_u32_le(&stream, &mut cursor) as usize, fixture.n());
        assert_eq!(read_u64_le(&stream, &mut cursor), ryser_f7(fixture));
        let matrix_len = read_u64_le(&stream, &mut cursor) as usize;
        assert_eq!(matrix_len, fixture.matrix_bytes().len());
        assert_eq!(&stream[cursor..cursor + matrix_len], fixture.matrix_bytes());
        cursor += matrix_len;
        assert_eq!(cursor, stream.len());
    }

    #[test]
    fn preparation_stream_carries_only_canonical_inputs_not_ryser_values() {
        let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
        let fixture = selected_f7_fixtures(&corpus)
            .into_iter()
            .find(|fixture| fixture.n() == 24)
            .expect("the preparation stage must retain the n=24 fixture");
        let mut stream = Vec::new();
        write_preparation_stream(&mut stream, &[fixture]).unwrap();
        let mut cursor = 0;
        assert_eq!(
            &stream[cursor..cursor + PREPARATION_STREAM_MAGIC.len()],
            PREPARATION_STREAM_MAGIC
        );
        cursor += PREPARATION_STREAM_MAGIC.len();
        assert_eq!(read_u32_le(&stream, &mut cursor), 1);
        let id_len = read_u32_le(&stream, &mut cursor) as usize;
        assert_eq!(&stream[cursor..cursor + id_len], fixture.id().as_bytes());
        cursor += id_len;
        assert_eq!(read_u32_le(&stream, &mut cursor) as usize, fixture.n());
        let matrix_len = read_u64_le(&stream, &mut cursor) as usize;
        assert_eq!(matrix_len, fixture.matrix_bytes().len());
        assert_eq!(&stream[cursor..cursor + matrix_len], fixture.matrix_bytes());
        cursor += matrix_len;
        assert_eq!(cursor, stream.len());
    }
}
