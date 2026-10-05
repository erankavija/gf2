//! The fallback contract of the kernel dispatch, shared by this crate's suite
//! and by each dependent crate's suite, which runs it against its own
//! dependency build of this crate.

use crate::gf2m::wide::{clmul_wide, last_clmul_wide_lane, PORTABLE_LANE};
use crate::kernels::backend::select_backend_for_size;
use crate::kernels::ops::{and_popcount_route, popcount_route, PopcountRoute};
use crate::matrix::{matvec_route, transpose_block_lane, MatvecRoute};
use crate::residual_shift::{residual_shift_route, ResidualShiftRoute};
use crate::tuning::{self, CoreTuning, SectionResolution};
use gf2_kernels_simd::transpose::TransposeLane;

/// What decides the routes of one build of this crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DispatchBuild {
    /// The `simd` cargo feature, which compiles the kernel dispatch.
    pub simd_feature: bool,
    /// The `gf2_tuning_baked` configuration, which substitutes the baked
    /// selector constants for the conservative ones.
    pub baked_selectors: bool,
}

/// This build of the crate.
#[must_use]
pub fn build() -> DispatchBuild {
    DispatchBuild {
        simd_feature: cfg!(feature = "simd"),
        baked_selectors: cfg!(gf2_tuning_baked),
    }
}

/// One reporter's answer for one input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteWitness {
    /// The public entry point or build fact observed.
    pub entry: &'static str,
    /// The input the reporter answered for.
    pub input: String,
    /// The stable name of the reported route or value.
    pub route: String,
    /// Whether the route needs no kernel crate dispatch and no processor
    /// feature.
    pub portable: bool,
}

impl RouteWitness {
    /// The tab-separated line a witness record holds for this observation.
    #[must_use]
    pub fn record(&self) -> String {
        format!(
            "GF2_ROUTE_WITNESS\t{}\t{}\t{}\t{}",
            self.entry,
            self.input,
            self.route,
            if self.portable { "portable" } else { "kernel" }
        )
    }
}

fn witness(entry: &'static str, input: String, route: &str, portable: bool) -> RouteWitness {
    RouteWitness {
        entry,
        input,
        route: route.to_owned(),
        portable,
    }
}

/// Asserts the established fallback of every kernel route that has a reporter
/// in every build, and returns what each reporter answered.
///
/// A process without an installed profile resolves the conservative table. A
/// width below its compile-time threshold, a build without the `simd` cargo
/// feature and a host without the kernel bundle each take the portable route;
/// the thresholds are the conservative ones unless the build is baked.
///
/// # Panics
///
/// Panics when a reporter names another route than the contract states, or
/// when a profile is installed in this process.
pub fn assert_fallback_contract() -> Vec<RouteWitness> {
    let active = tuning::active();
    assert!(
        matches!(
            active.resolution,
            SectionResolution::FrozenBeforeInstall { .. }
        ),
        "a process without an installed profile freezes the conservative table"
    );
    assert_eq!(active.section, &CoreTuning::CONSERVATIVE);

    #[cfg(not(gf2_tuning_baked))]
    let (bit_min, matvec_min) = (
        active.bit_backend().simd_min_words(),
        active.bit_matrix().matvec_simd_min_words(),
    );
    #[cfg(gf2_tuning_baked)]
    let (bit_min, matvec_min) = (
        crate::tuning::baked::SIMD_MIN_WORDS,
        crate::tuning::baked::MATVEC_SIMD_MIN_WORDS,
    );

    let build = build();
    let simd = build.simd_feature;
    let logical_kernels = simd && gf2_kernels_simd::detect().is_some();
    let mut witnesses = vec![
        witness(
            "build.simd-feature",
            String::new(),
            &simd.to_string(),
            !simd,
        ),
        witness(
            "build.baked-selectors",
            String::new(),
            &build.baked_selectors.to_string(),
            true,
        ),
        witness(
            "tuning.active",
            "gf2-core/selectors".to_owned(),
            "frozen-before-install",
            true,
        ),
    ];

    let mut widths = vec![
        0,
        1,
        bit_min - 1,
        bit_min,
        bit_min + 1,
        matvec_min - 1,
        matvec_min,
        matvec_min + 1,
        63,
        64,
        65,
    ];
    widths.sort_unstable();
    widths.dedup();
    for words in widths {
        let input = format!("words={words}");

        let backend = select_backend_for_size(words).name();
        let expected = if simd && words >= bit_min {
            "simd"
        } else {
            "scalar"
        };
        assert_eq!(backend, expected, "bit backend at {input}");
        witnesses.push(witness(
            "kernels::select_backend_for_size",
            input.clone(),
            backend,
            backend == "scalar",
        ));

        let expected = if logical_kernels && words >= bit_min {
            PopcountRoute::SimdNibbleLut
        } else {
            PopcountRoute::Scalar
        };
        for (entry, route) in [
            ("kernels::ops::popcount", popcount_route(words)),
            ("kernels::ops::and_popcount", and_popcount_route(words)),
        ] {
            assert_eq!(route, expected, "{entry} at {input}");
            witnesses.push(witness(
                entry,
                input.clone(),
                route.as_str(),
                route == PopcountRoute::Scalar,
            ));
        }

        let route = matvec_route(words);
        let expected = if simd && words >= matvec_min {
            MatvecRoute::Simd
        } else {
            MatvecRoute::Scalar
        };
        assert_eq!(route, expected, "matvec at stride {input}");
        witnesses.push(witness(
            "BitMatrix::matvec",
            input,
            match route {
                MatvecRoute::Scalar => "scalar",
                MatvecRoute::Simd => "simd",
            },
            route == MatvecRoute::Scalar,
        ));
    }

    let lane = transpose_block_lane();
    let expected = if simd {
        gf2_kernels_simd::transpose::detect().map_or(TransposeLane::Scalar, |fns| fns.lane)
    } else {
        TransposeLane::Scalar
    };
    assert_eq!(lane, expected, "transpose block lane");
    witnesses.push(witness(
        "BitMatrix::transpose",
        "block=64x64".to_owned(),
        lane.name(),
        lane == TransposeLane::Scalar,
    ));

    let route = residual_shift_route();
    let expected = if simd && gf2_kernels_simd::shift_funnel::detect().is_some() {
        ResidualShiftRoute::Bmi2Funnel
    } else {
        ResidualShiftRoute::ScalarFunnel
    };
    assert_eq!(route, expected, "residual shift route");
    witnesses.push(witness(
        "BitVec::shift_left",
        "residual".to_owned(),
        route.name(),
        route == ResidualShiftRoute::ScalarFunnel,
    ));

    let wide_kernels = if simd {
        gf2_kernels_simd::gf2m_wide::detect_wide()
    } else {
        None
    };
    let mut wide = |words: usize, kernel: Option<&'static str>| {
        let lane = last_clmul_wide_lane();
        assert_eq!(
            lane,
            kernel.unwrap_or(PORTABLE_LANE),
            "{words}-word product"
        );
        witnesses.push(witness(
            "gf2m::wide::clmul_wide",
            format!("words={words}"),
            lane,
            lane == PORTABLE_LANE,
        ));
    };
    let _: [u64; 2] = clmul_wide(&[3u64; 1], &[5u64; 1]);
    wide(1, None);
    let _: [u64; 8] = clmul_wide(&[3u64; 4], &[5u64; 4]);
    wide(4, wide_kernels.as_ref().map(|fns| fns.wide256.name));
    let _: [u64; 18] = clmul_wide(&[3u64; 9], &[5u64; 9]);
    wide(9, wide_kernels.as_ref().map(|fns| fns.wide571.name));
    let _: [u64; 32] = clmul_wide(&[3u64; 16], &[5u64; 16]);
    wide(16, None);

    if !simd {
        assert!(
            witnesses.iter().all(|witness| witness.portable),
            "a build without the `simd` feature takes portable routes only"
        );
    }
    witnesses
}
