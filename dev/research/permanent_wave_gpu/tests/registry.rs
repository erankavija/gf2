use permanent_wave_gpu::MeasurementPath;

#[test]
fn every_planned_path_is_addressable_with_explicit_status() {
    let expected = [
        "wave-gf3",
        "fold-gf3",
        "f5-byte-control",
        "f5-three-plane",
        "f7-three-plane-accumulator",
        "f7-lookup-table-control",
        "f7-three-plane-permanent",
    ];
    let actual = MeasurementPath::ALL.map(MeasurementPath::name);
    assert_eq!(
        actual, expected,
        "the complete planned candidate set is stable"
    );

    for path in MeasurementPath::ALL {
        if matches!(
            path,
            MeasurementPath::WaveGf3
                | MeasurementPath::FoldGf3
                | MeasurementPath::F7ThreePlaneAccumulator
        ) {
            path.dispatch()
                .expect("each landed candidate must be dispatchable");
        } else {
            let unsupported = path
                .dispatch()
                .expect_err("each unimplemented path must explicitly report unsupported");
            assert!(
                !unsupported.reason().is_empty(),
                "{} must state why it is unsupported",
                path.name()
            );
        }
    }
}
