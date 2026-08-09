use permanent_wave_gpu::MeasurementPath;

#[test]
fn every_planned_path_is_addressable_and_explicitly_unsupported() {
    let expected = [
        "wave-gf3",
        "fold-gf3",
        "candidates-gf5",
        "three-plane-gf7",
        "wave-gf7",
    ];
    let actual = MeasurementPath::ALL.map(MeasurementPath::name);
    assert_eq!(
        actual, expected,
        "the complete planned candidate set is stable"
    );

    for path in MeasurementPath::ALL {
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
