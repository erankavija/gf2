//! Allocation witness: once a workspace exists, correcting a binary BCH word
//! in a declared layout, and decoding a DVB-T2 outer BCH word, reach no
//! allocator on any outcome.

use gf2_coding::bch::dvb_t2::{dvb_t2_bch_code, DvbT2BchDecoder, FrameSize, DVB_T2_LAYOUT};
use gf2_coding::bch::{
    BchDecodeOutcome, BinaryBchCode, BinaryBchDecoder, DesignedDistance, SystematicLayout,
};
use gf2_coding::traits::block::{BlockCode, BlockEncoder};
use gf2_coding::CodeRate;
use gf2_core::field::extension::BinaryPrimeExt;
use gf2_core::gf2m::Gf2mField;
use gf2_core::BitVec;
use thread_allocations::{count, CountingAllocator};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Flips the first `errors` of `0, 3, 6, ...` in a copy of `word`.
fn corrupted(word: &BitVec, errors: usize) -> BitVec {
    let mut word = word.clone();
    for position in (0..errors).map(|index| 3 * index) {
        word.set(position, !word.get(position));
    }
    word
}

#[test]
fn layout_correction_reaches_no_allocator() {
    let extension = BinaryPrimeExt::new(Gf2mField::new(7, 0b1000_0011).with_tables())
        .expect("x^7 + x + 1 presents GF(128)");
    let code = BinaryBchCode::primitive_narrow_sense(
        extension,
        DesignedDistance::try_from(21).expect("a positive designed distance"),
    )
    .expect("BCH(127, 64) constructs");
    let t = code.correction_radius();
    let decoder = BinaryBchDecoder::new(&code);
    let mut workspace = decoder.workspace();
    let message = BitVec::random_seeded(code.k(), 0xAE03_BCD0);

    for layout in [SystematicLayout::MessageParityAscending, DVB_T2_LAYOUT] {
        let codeword = code
            .encode_systematic(&message, layout)
            .expect("a k-bit message encodes");
        for errors in [0, 1, t, t + 1] {
            let mut word = corrupted(&codeword, errors);
            let (outcome, requests) = count(|| {
                decoder
                    .correct_systematic_in_place(&mut word, layout, &mut workspace)
                    .expect("a length-n word decodes")
            });
            assert!(
                requests.is_zero(),
                "{errors} errors under {layout:?} requested {requests:?}"
            );
            if errors <= t {
                assert_eq!(outcome.corrected_count(), Some(errors));
                assert_eq!(word, codeword);
            }
        }
    }
}

#[test]
fn dvb_t2_decoding_reaches_no_allocator() {
    let code = dvb_t2_bch_code(FrameSize::Short, CodeRate::Rate1_2).expect("a standard code");
    let decoder = DvbT2BchDecoder::new(&code);
    let t = decoder.correction_radius();
    let mut workspace = decoder.workspace();
    let message = BitVec::random_seeded(code.k(), 0xAE03_BCD0);
    let codeword = code.encode(&message).expect("a K_bch-bit message encodes");
    let mut bbframe = BitVec::zeros(code.k());

    for errors in [0, 1, t] {
        let received = corrupted(&codeword, errors);
        let mut word = received.clone();
        let (outcomes, requests) = count(|| {
            let corrected = decoder
                .correct_in_place(&mut word, &mut workspace)
                .expect("a length-n word decodes");
            let decoded = decoder
                .decode_into(&received, &mut bbframe, &mut workspace)
                .expect("a length-n word decodes");
            (corrected, decoded)
        });
        assert!(requests.is_zero(), "{errors} errors requested {requests:?}");
        let expected = if errors == 0 {
            BchDecodeOutcome::NoErrors
        } else {
            BchDecodeOutcome::Corrected { count: errors }
        };
        assert_eq!(outcomes, (expected, expected));
        assert_eq!(word, codeword);
        assert_eq!(bbframe, message);
    }
}
