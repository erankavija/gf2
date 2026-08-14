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
        // The accumulator candidate's device source is a single-thread
        // arithmetic probe, so it has no batch kernel to reach; every other
        // registered path owns one.
        if path == MeasurementPath::F7ThreePlaneAccumulator {
            let reason = path
                .device_batch_kernel()
                .expect_err("an accumulator probe is not a full-permanent batch kernel");
            assert!(
                reason.contains(path.name()),
                "{} must state why it has no batch kernel",
                path.name()
            );
        } else {
            let kernel = path.device_batch_kernel().unwrap_or_else(|reason| {
                panic!("{} must own a batch kernel: {reason}", path.name())
            });
            assert!(
                matches!(kernel.field_order(), 3 | 5 | 7),
                "{} must name the field its kernel evaluates",
                path.name()
            );
        }
    }
}
