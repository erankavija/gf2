//! A nonbinary BCH code with every construction input explicit.
//! The code-symbol field is GF(3), the splitting field is GF(3^5) under a
//! caller-chosen modulus, the order-11 root is supplied by the caller, and
//! the length, first root, and designed distance are named directly. The
//! result is the ternary Golay code (11, 6). The example encodes in two
//! layouts, checks codeword membership through `H`, establishes the minimum
//! distance by exhaustive enumeration to contrast it with the witnessed
//! bound, shows the typed construction errors, and saves `G` and `H` through
//! the canonical checksummed `FieldMatrix` format before reloading them.

use gf2_coding::bch::error::BchError;
use gf2_coding::bch::{
    BchLength, BchSpec, DenseBchCode, DesignedDistance, LayoutView, RootExponent, RootSelection,
    SystematicLayout,
};
use gf2_coding::traits::block::{BlockEncoder, GeneratorMatrixAccess, ParityCheckMatrixAccess};
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::{ConstField, FieldPoly, FieldVec, FiniteField, FiniteFieldExt};
use gf2_core::gfp::Fp;
use gf2_core::gfpn::QuotientField;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type F3 = Fp<3>;

pub fn main() -> Result<()> {
    // The splitting field GF(3^5) = GF(3)[x] / (x^5 + 2x + 1), coefficients
    // in ascending degree. The modulus is primitive, so x has order
    // 3^5 - 1 = 242 and x^22 has exact order 11.
    let zero = F3::zero();
    let modulus = FieldPoly::new(vec![
        F3::new(1),
        F3::new(2),
        F3::new(0),
        F3::new(0),
        F3::new(0),
        F3::new(1),
    ]);
    let extension = QuotientField::new(zero, modulus)?;
    let root = extension.indeterminate().pow(22);

    // Every independent input is explicit: length 11 is a proper divisor of
    // 242, the root is the caller's, and delta = 4 requests the consecutive
    // roots beta^3, beta^4, beta^5. Construction closes them under the
    // Frobenius map x -> x^3, giving the defining set {1, 3, 4, 5, 9}.
    let spec = BchSpec::NonPrimitiveConsecutive {
        extension: extension.clone(),
        length: BchLength::try_from(11)?,
        root: RootSelection::Explicit(root.clone()),
        first_root: RootExponent::from(3),
        designed_distance: DesignedDistance::try_from(4)?,
    };
    let code = DenseBchCode::construct(spec)?;
    let bound = code.distance_bound();
    let defining_set: Vec<u64> = code
        .defining_set()
        .iter()
        .map(|exponent| exponent.get())
        .collect();
    println!(
        "BCH({}, {}) over GF(3): defining set {defining_set:?}, generator coefficients {:?}",
        code.n(),
        code.k(),
        code.generator()
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!((code.n(), code.k()), (11, 6));
    assert_eq!(defining_set, [1, 3, 4, 5, 9]);
    assert_eq!(bound.minimum_distance_lower_bound(), 4);
    assert_eq!(code.correction_radius(), 1);

    // Typed construction errors: a length that does not divide |E*| has no
    // root of that order, and a supplied root of the wrong order is refused.
    let no_root = DenseBchCode::construct(BchSpec::NonPrimitiveConsecutive {
        extension: extension.clone(),
        length: BchLength::try_from(13)?,
        root: RootSelection::Canonical,
        first_root: RootExponent::from(1),
        designed_distance: DesignedDistance::try_from(3)?,
    });
    assert!(matches!(
        no_root,
        Err(BchError::LengthDoesNotDivideUnitGroup { length: 13, .. })
    ));
    let wrong_order = DenseBchCode::construct(BchSpec::NonPrimitiveConsecutive {
        extension: extension.clone(),
        length: BchLength::try_from(11)?,
        root: RootSelection::Explicit(extension.indeterminate()),
        first_root: RootExponent::from(3),
        designed_distance: DesignedDistance::try_from(4)?,
    });
    assert!(matches!(
        wrong_order,
        Err(BchError::RootOrderMismatch {
            expected: 11,
            actual: 242
        })
    ));

    // Systematic encoding in the default layout, and the same message under
    // the descending layout. Both put the message in the first k coordinates.
    let mut message = FieldVec::zeros_from(code.k(), &zero);
    message.set(0, F3::new(2));
    message.set(4, F3::new(1));
    let codeword = code.encode(&message)?;
    let descending = code.encode_systematic(&message, SystematicLayout::MessageParityDescending)?;
    assert_eq!(
        code.systematic_message(&codeword, SystematicLayout::default())?,
        message
    );
    assert_eq!(
        code.systematic_message(&descending, SystematicLayout::MessageParityDescending)?,
        message
    );

    // H annihilates every codeword. The binary decoder does not apply to a
    // GF(3) code; membership is checked through the syndrome instead. The
    // minimum distance follows from enumerating all 3^6 codewords: 5,
    // exceeding the witnessed bound 4.
    let generator = code.generator_matrix()?;
    let parity_check = code.parity_check_matrix()?;
    assert!(code.is_systematic()?);
    let mut minimum_weight = usize::MAX;
    for index in 1..3u64.pow(code.k() as u32) {
        let mut message = FieldVec::zeros_from(code.k(), &zero);
        let mut digits = index;
        for coordinate in 0..code.k() {
            message.set(coordinate, F3::new(digits % 3));
            digits /= 3;
        }
        let word = code.encode(&message)?;
        assert!(parity_check
            .matvec(&word)
            .iter()
            .all(|symbol| symbol.is_zero()));
        minimum_weight = minimum_weight.min(word.iter().filter(|symbol| !symbol.is_zero()).count());
    }
    println!(
        "witnessed bound d_min >= {}, enumerated minimum distance {minimum_weight}",
        bound.minimum_distance_lower_bound()
    );
    assert_eq!(minimum_weight, 5);

    // A declared layout presents both matrices in its own coordinates.
    let view = LayoutView::new(code.clone(), SystematicLayout::MessageParityDescending);
    let view_generator = view.generator_matrix()?;
    let mut basis = FieldVec::zeros_from(code.k(), &zero);
    basis.set(2, F3::one());
    let basis_word = view.encode(&basis)?;
    for column in 0..code.n() {
        assert_eq!(view_generator.get(2, column), *basis_word.get(column));
    }

    let directory = tempfile::Builder::new()
        .prefix("gf2-bch-nonbinary-")
        .tempdir()?;
    let generator_path = directory.path().join("golay_11_6_generator.gf2fmat");
    let parity_check_path = directory.path().join("golay_11_6_parity_check.gf2fmat");
    generator.save_to_file(&generator_path)?;
    parity_check.save_to_file(&parity_check_path)?;
    assert_eq!(
        FieldMatrix::load_from_file(&generator_path, &zero)?,
        generator
    );
    assert_eq!(
        FieldMatrix::load_from_file(&parity_check_path, &zero)?,
        parity_check
    );
    println!(
        "saved and reloaded G ({}x{}) and H ({}x{})",
        generator.rows(),
        generator.cols(),
        parity_check.rows(),
        parity_check.cols()
    );
    directory.close()?;
    Ok(())
}
