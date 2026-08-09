use gf2_core::gfp::Fp;
use gf2_stats::sampler::{FieldOrder, MatrixAddress, MatrixSampler, StreamIndex, StreamPurpose};

#[test]
fn opens_an_addressed_stream_and_fills_the_callers_buffer() {
    let address = MatrixAddress::new(
        0xB488_F02C,
        FieldOrder::F3,
        2,
        StreamPurpose::CampaignCell,
        StreamIndex::new(0).expect("zero is a valid stream index"),
    );
    let mut sampler = MatrixSampler::<3>::new(address).expect("field order matches address");
    let mut matrix = [Fp::<3>::new(0); 4];

    sampler.fill_next_matrix(&mut matrix);

    assert!(matrix.iter().all(|entry| entry.value() < 3));
}
