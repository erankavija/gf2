//! A primitive narrow-sense binary BCH code, end to end.
//! Builds BCH(63, 45) from a relative degree and a designed distance alone,
//! encodes systematically one message and a batch, corrects errors with the
//! bounded-distance decoder, materializes `G` and `H`, and saves both through
//! the canonical checksummed `FieldMatrix` format before reloading them.

use gf2_coding::bch::{
    BchDecodeOutcome, BinaryBchCode, BinaryBchDecoder, DenseBchCode, DesignedDistance,
    SystematicLayout,
};
use gf2_coding::traits::block::{BlockEncoder, GeneratorMatrixAccess, ParityCheckMatrixAccess};
use gf2_core::field::extension::BinaryPrimeExt;
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::ConstField;
use gf2_core::gfp::Fp;
use gf2_core::BitVec;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn main() -> Result<()> {
    // Construction: GF(2^6) is selected deterministically, the length is
    // derived as 2^6 - 1, and the defining set is the cyclotomic closure of
    // the requested consecutive roots 1..=6.
    let designed_distance = DesignedDistance::try_from(7)?;
    let code =
        BinaryBchCode::<u64>::primitive_narrow_sense_auto(Fp::<2>::zero(), 6, designed_distance)?;
    let bound = code.distance_bound();
    println!(
        "BCH({}, {}): witnessed run of {} roots, so d_min >= {} and t = {}",
        code.n(),
        code.k(),
        bound.consecutive_root_count(),
        bound.minimum_distance_lower_bound(),
        code.correction_radius()
    );
    assert_eq!((code.n(), code.k(), code.correction_radius()), (63, 45, 3));

    // Systematic encoding in the default layout: the message occupies the
    // first k coordinates, the parity the last n - k.
    let layout = SystematicLayout::default();
    let message = BitVec::random_seeded(code.k(), 0x00B0_C4A1);
    let codeword = code.encode(&message)?;
    assert_eq!(code.systematic_message(&codeword, layout)?, message);

    // Decoding: t errors are inside the radius, so the corrected word is the
    // transmitted codeword. The default layout is a cyclic rotation of the
    // polynomial coordinates, so a default-layout word decodes directly.
    let decoder = BinaryBchDecoder::new(&code);
    let mut workspace = decoder.workspace();
    let mut received = codeword.clone();
    for position in [3, 30, 61] {
        received.set(position, !received.get(position));
    }
    let outcome = decoder.correct_in_place(&mut received, &mut workspace)?;
    println!("decode outcome with 3 errors: {outcome:?}");
    assert_eq!(outcome, BchDecodeOutcome::Corrected { count: 3 });
    assert_eq!(received, codeword);
    assert_eq!(code.systematic_message(&received, layout)?, message);

    // Batch encoding: the dispatch selects one algorithm family per batch;
    // every family writes the same bits as the single-message path.
    let messages: Vec<BitVec> = (0..256)
        .map(|seed| BitVec::random_seeded(code.k(), seed))
        .collect();
    let family = code.selected_encode_family(layout, messages.len());
    let codewords = code.encode_batch(&messages, layout)?;
    println!("batch of {} encoded with family {family}", messages.len());
    for (message, codeword) in messages.iter().zip(&codewords) {
        assert_eq!(*codeword, code.encode(message)?);
    }

    // Matrices in the packed representation: G = [I_k | P], H = [P^T | I],
    // and every codeword has zero syndrome under H.
    let generator = code.generator_matrix()?;
    let parity_check = code.parity_check_matrix()?;
    assert!(code.is_systematic()?);
    assert_eq!((generator.rows(), generator.cols()), (code.k(), code.n()));
    assert_eq!(
        (parity_check.rows(), parity_check.cols()),
        (code.n() - code.k(), code.n())
    );
    assert_eq!(parity_check.matvec(&codeword).count_ones(), 0);

    // The canonical serialization stores FieldMatrix values with their field
    // identity. DenseBchCode is the same code in the field-generic
    // representation, so its matrices are the FieldMatrix<Fp<2>> twins of the
    // packed ones.
    let dense = DenseBchCode::<BinaryPrimeExt>::primitive_narrow_sense(
        code.extension().clone(),
        designed_distance,
    )?;
    let dense_generator = dense.generator_matrix()?;
    let dense_parity_check = dense.parity_check_matrix()?;
    for row in 0..code.k() {
        for col in 0..code.n() {
            assert_eq!(
                dense_generator.get(row, col) == Fp::<2>::one(),
                generator.get(row, col)
            );
        }
    }

    let directory = tempfile::Builder::new()
        .prefix("gf2-bch-quickstart-")
        .tempdir()?;
    let generator_path = directory.path().join("bch_63_45_generator.gf2fmat");
    let parity_check_path = directory.path().join("bch_63_45_parity_check.gf2fmat");
    dense_generator.save_to_file(&generator_path)?;
    dense_parity_check.save_to_file(&parity_check_path)?;
    let witness = Fp::<2>::zero();
    assert_eq!(
        FieldMatrix::load_from_file(&generator_path, &witness)?,
        dense_generator
    );
    assert_eq!(
        FieldMatrix::load_from_file(&parity_check_path, &witness)?,
        dense_parity_check
    );
    println!(
        "saved and reloaded G ({} bytes) and H ({} bytes)",
        std::fs::metadata(&generator_path)?.len(),
        std::fs::metadata(&parity_check_path)?.len()
    );
    directory.close()?;
    Ok(())
}
