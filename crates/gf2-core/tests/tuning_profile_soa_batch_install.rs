use gf2_core::compute::field::{soa_parallel_route, SoaParallelRoute};
use gf2_core::tuning::{self, TuningProfile};

#[test]
fn installed_soa_batch_profile_moves_the_parallel_route_boundary() {
    let conservative_min_len = TuningProfile::CONSERVATIVE.soa_batch().parallel_min_len();
    let profile = TuningProfile::from_json(
        r#"
        {
          "schema_version": 1,
          "profile_id": "soa-batch-route-test",
          "provenance": {"kind": "inherited"},
          "selectors": {
            "soa_batch": {"parallel_min_len": 40, "parallel_chunk_len": 16}
          }
        }
        "#,
    )
    .expect("test profile is valid");

    assert_ne!(
        40, conservative_min_len,
        "the installed boundary must differ from the conservative default to prove the route moved"
    );
    assert_eq!(tuning::install(profile), Ok(()));

    let soa_batch = tuning::active().soa_batch();
    assert_eq!(soa_batch.parallel_min_len(), 40);
    assert_eq!(soa_batch.parallel_chunk_len(), 16);

    // Below the installed boundary: sequential.
    assert_eq!(soa_parallel_route(0), SoaParallelRoute::Sequential);
    assert_eq!(soa_parallel_route(39), SoaParallelRoute::Sequential);
    // At and above the installed boundary: parallel.
    assert_eq!(soa_parallel_route(40), SoaParallelRoute::Parallel);
    assert_eq!(soa_parallel_route(41), SoaParallelRoute::Parallel);
}
