//! Stream the tractable committed F_3 fixture corpus to either F_3 fold.
//!
//! This is an opt-in evidence driver, not a test target. It computes the
//! generic Ryser oracle for every selected fixture and sends fixture IDs,
//! canonical row-major bytes, and expected values to the prebuilt executable.
//! The executable performs device launches and fails on the first element-wise
//! mismatch. The existing measurement registry selects either the halving
//! control or zero-mask sign-popcount candidate; no driver-local path list
//! exists. Run it only on a ROCm gfx1030 host:
//!
//! ```sh
//! cargo +1.95.0 run --manifest-path dev/research/permanent_wave_gpu/Cargo.toml \
//!     --release --features hip --bin wave-gf3-device-evidence -- --path fold-gf3
//! ```

use std::env;
use std::error::Error;
use std::io::Write;
use std::process::{Command, Stdio};

use gf2_algebra::permanent::permanent_ryser;
use gf2_core::gfp::Fp;
use permanent_wave_gpu::fixtures::{Fixture, FixtureCorpus, DEFAULT_FIXTURE_SEED};
use permanent_wave_gpu::{MeasurementPath, WAVE_GF3_MAX_FIXTURE_ORDER};

const STREAM_MAGIC: [u8; 8] = *b"GF2WAVE1";

fn write_u32_le(writer: &mut impl Write, value: u32) -> Result<(), Box<dyn Error>> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn write_u64_le(writer: &mut impl Write, value: u64) -> Result<(), Box<dyn Error>> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn ryser_f3(fixture: &Fixture) -> u64 {
    let entries: Vec<_> = fixture
        .matrix_bytes()
        .iter()
        .map(|&value| Fp::<3>::new(u64::from(value)))
        .collect();
    permanent_ryser::<Fp<3>>(&entries, fixture.n()).value()
}

fn selected_f3_fixtures(corpus: &FixtureCorpus) -> Vec<&Fixture> {
    corpus
        .fixtures()
        .iter()
        .filter(|fixture| fixture.q() == 3 && fixture.n() <= WAVE_GF3_MAX_FIXTURE_ORDER)
        .collect()
}

fn write_fixture_stream(
    writer: &mut impl Write,
    fixtures: &[&Fixture],
) -> Result<(), Box<dyn Error>> {
    let fixture_count = u32::try_from(fixtures.len())?;
    writer.write_all(&STREAM_MAGIC)?;
    write_u32_le(writer, fixture_count)?;
    for fixture in fixtures {
        let id = fixture.id().as_bytes();
        let bytes = fixture.matrix_bytes();
        write_u32_le(writer, u32::try_from(id.len())?)?;
        writer.write_all(id)?;
        write_u32_le(writer, u32::try_from(fixture.n())?)?;
        write_u64_le(writer, ryser_f3(fixture))?;
        write_u64_le(writer, u64::try_from(bytes.len())?)?;
        writer.write_all(bytes)?;
    }
    Ok(())
}

fn selected_f3_path(arguments: &[String]) -> Result<MeasurementPath, Box<dyn Error>> {
    match arguments {
        [] => Ok(MeasurementPath::WaveGf3),
        [flag, path] if flag == "--path" && path == MeasurementPath::WaveGf3.name() => {
            Ok(MeasurementPath::WaveGf3)
        }
        [flag, path] if flag == "--path" && path == MeasurementPath::FoldGf3.name() => {
            Ok(MeasurementPath::FoldGf3)
        }
        _ => Err("usage: wave-gf3-device-evidence [--path wave-gf3|fold-gf3]".into()),
    }
}

fn run_evidence(path: MeasurementPath) -> Result<(), Box<dyn Error>> {
    if !matches!(path, MeasurementPath::WaveGf3 | MeasurementPath::FoldGf3) {
        return Err(format!("{} is not an F_3 wave fold", path.name()).into());
    }
    path.device_batch_kernel()
        .map_err(|reason| format!("{} is unavailable: {reason}", path.name()))?;

    let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
    let fixtures = selected_f3_fixtures(&corpus);
    if fixtures.is_empty() {
        return Err("the committed fixture corpus has no tractable F_3 fixtures".into());
    }
    let fixture_count = fixtures.len();

    let mut child = Command::new(env!("PERMANENT_WAVE_GPU_WAVE_GF3_EQUIVALENCE_BIN"))
        .args(["--fold", path.name(), "--fixture-stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    let mut input = child
        .stdin
        .take()
        .ok_or("the F_3 fold evidence executable did not provide stdin")?;
    write_fixture_stream(&mut input, &fixtures)?;
    drop(input);

    let status = child.wait()?;
    if !status.success() {
        return Err(format!("{} evidence executable failed: {status}", path.name()).into());
    }
    println!(
        "{} device evidence passed for {fixture_count} committed F_3 fixtures through n={WAVE_GF3_MAX_FIXTURE_ORDER}",
        path.name()
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = env::args().skip(1).collect();
    run_evidence(selected_f3_path(&arguments)?)
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
    fn evidence_selection_and_protocol_are_canonical() {
        assert_eq!(selected_f3_path(&[]).unwrap(), MeasurementPath::WaveGf3);
        assert_eq!(
            selected_f3_path(&["--path".to_owned(), "wave-gf3".to_owned()]).unwrap(),
            MeasurementPath::WaveGf3
        );
        assert_eq!(
            selected_f3_path(&["--path".to_owned(), "fold-gf3".to_owned()]).unwrap(),
            MeasurementPath::FoldGf3
        );
        assert!(selected_f3_path(&["--path".to_owned(), "f5-three-plane".to_owned()]).is_err());

        let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
        let fixtures = selected_f3_fixtures(&corpus);
        assert!(!fixtures.is_empty());
        assert!(fixtures
            .iter()
            .all(|fixture| fixture.q() == 3 && fixture.n() <= WAVE_GF3_MAX_FIXTURE_ORDER));

        let mut stream = Vec::new();
        write_fixture_stream(&mut stream, &fixtures).unwrap();
        let mut cursor = 0;
        assert_eq!(&stream[cursor..cursor + STREAM_MAGIC.len()], STREAM_MAGIC);
        cursor += STREAM_MAGIC.len();
        assert_eq!(read_u32_le(&stream, &mut cursor) as usize, fixtures.len());
        for fixture in fixtures {
            let id_len = read_u32_le(&stream, &mut cursor) as usize;
            assert_eq!(&stream[cursor..cursor + id_len], fixture.id().as_bytes());
            cursor += id_len;
            assert_eq!(read_u32_le(&stream, &mut cursor) as usize, fixture.n());
            assert_eq!(read_u64_le(&stream, &mut cursor), ryser_f3(fixture));
            let matrix_len = read_u64_le(&stream, &mut cursor) as usize;
            assert_eq!(&stream[cursor..cursor + matrix_len], fixture.matrix_bytes());
            cursor += matrix_len;
        }
        assert_eq!(cursor, stream.len());
    }
}
