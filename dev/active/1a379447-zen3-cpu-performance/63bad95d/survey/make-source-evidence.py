#!/usr/bin/env python3
"""Freeze the source claims of the dispatch route inventory and calibration plan (jit:63bad95d).

Every claim has an identifier the inventory and the plan cite, names a file by
Cargo package and package-relative path (or by its unique tracked file name),
a fragment of one line, how often the fragment occurs in the file and why the
claim matters. The script writes `source-evidence.json` beside itself with the
line the first occurrence holds, the verbatim line, the file digest and the
commit that last changed the file. The ledger tracks the current tree: a
fragment that is absent or changes its occurrence count fails the script.

Usage: make-source-evidence.py, with no arguments
"""

import hashlib
import json
import subprocess

from locate import HERE, ROOT, package, repository_files

CORE, SIMD, CODING, SIM, ALGEBRA, SUPPORT = (
    "gf2-core", "gf2-kernels-simd", "gf2-coding", "gf2-sim", "gf2-algebra",
    "tuning-campaign-support",
)

# A document located by file name and opening line.
SEAM_PROTOCOL = ("premeasurement-protocol.md",
                 b"# Seam threshold calibration: premeasurement protocol")

# (claim id, package, a (name, opening) document, or None for a unique tracked file name;
#  path; fragment; occurrences; why)
CLAIMS = [
    # Cargo features and build configuration.
    ("core-default-features", CORE, "Cargo.toml", 'default = ["rand", "io"]', 1,
     "A standalone `gf2-core` build enables neither `simd` nor `tuning-profile`."),
    ("core-simd-optional", CORE, "Cargo.toml", "simd = []", 1,
     "Kernel dispatch is the opt-in `simd` cargo feature."),
    ("core-tuning-profile-feature", CORE, "Cargo.toml",
     'tuning-profile = ["dep:serde", "dep:serde_json", "dep:sha2"]', 1,
     "`tuning-profile` adds profile parsing only; it selects no profile."),
    ("core-baked-cfg-declared", CORE, "Cargo.toml",
     "unexpected_cfgs = { level = \"warn\", check-cfg = ['cfg(kani)', 'cfg(verify_lean)', "
     "'cfg(gf2_tuning_baked)'] }", 1,
     "`gf2_tuning_baked` is a declared compiler configuration and no cargo feature, so "
     "`--all-features` does not set it."),
    ("coding-default-simd", CODING, "Cargo.toml", 'default = ["simd", "sim-observability"]', 1,
     "A default `gf2-coding` build enables its `simd` feature."),
    ("coding-simd-forwards", CODING, "Cargo.toml", 'simd = ["gf2-core/simd"]', 1,
     "`gf2-coding/simd` is the edge that enables `gf2-core/simd`."),
    ("sim-default-empty", SIM, "Cargo.toml", "default = []", 1,
     "`gf2-sim` has no default feature of its own."),
    ("sim-depends-coding-defaults", SIM, "Cargo.toml",
     'gf2-coding = { path = "../gf2-coding" }', 1,
     "`gf2-sim` takes `gf2-coding` with its default features, so every `gf2-sim` build "
     "enables `gf2-core/simd`."),
    ("sim-depends-algebra-defaults", SIM, "Cargo.toml",
     'gf2-algebra = { path = "../gf2-algebra" }', 1,
     "`gf2-sim` takes `gf2-algebra` with its default features."),
    ("algebra-default-simd", ALGEBRA, "Cargo.toml",
     'default = ["simd", "parallel", "f5", "f7"]', 1,
     "A default `gf2-algebra` build enables `simd` and `parallel`."),
    ("algebra-simd-forwards", ALGEBRA, "Cargo.toml",
     'simd = ["gf2-core/simd", "dep:gf2-kernels-simd"]', 1,
     "`gf2-algebra/simd` enables `gf2-core/simd`."),
    ("ci-baked-step", None, "cargo-ci.sh",
     'run_step baked-core env RUSTFLAGS="--cfg gf2_tuning_baked"', 1,
     "CI builds the baked configuration in one scoped step; every other step keeps the "
     "conservative constants."),
    ("ci-feature-flags", None, "cargo-ci.sh",
     '  FEAT_FLAGS="--features simd,parallel,visualization,llr-f64"', 1,
     "The workspace test step of CI enables `simd`, so it runs the shared fallback "
     "contract on the kernel build only."),
    # Kernel bundle detection.
    ("logical-bundle-detect", SIMD, "src/x86/mod.rs",
     '    if cfg!(any(target_arch = "x86", target_arch = "x86_64")) && '
     'is_x86_feature_detected!("avx2") {', 1,
     "The logical kernel bundle exists only after runtime AVX2 detection."),
    ("core-logical-bundle-once", CORE, "src/lib.rs",
     "        FNS.get_or_init(gf2_kernels_simd::detect).as_ref()", 1,
     "`gf2-core` detects the logical bundle once per process."),
    ("core-transpose-bundle-once", CORE, "src/lib.rs",
     "            .get_or_init(gf2_kernels_simd::transpose::detect)", 1,
     "`gf2-core` resolves the transpose lane once per process."),
    ("core-shift-bundle-once", CORE, "src/lib.rs",
     "            .get_or_init(gf2_kernels_simd::shift_funnel::detect)", 1,
     "`gf2-core` detects the shift funnel once per process."),
    ("core-wide-bundle-once", CORE, "src/lib.rs",
     "            .get_or_init(gf2_kernels_simd::gf2m_wide::detect_wide)", 1,
     "`gf2-core` detects the wide carry-less kernels once per process."),
    ("core-gf2m-bundle-once", CORE, "src/lib.rs",
     "            .get_or_init(gf2_kernels_simd::gf2m::detect)", 1,
     "`gf2-core` detects the GF(2^m) bundle through the default preference."),
    # Bit backend: logical operations and population counts.
    ("bit-threshold-conservative", CORE, "src/kernels/backend.rs",
     "pub(crate) const SIMD_MIN_WORDS_DEFAULT: usize = 8;", 1,
     "The conservative definition of `bit_backend.simd_min_words`."),
    ("bit-threshold-baked-alias", CORE, "src/kernels/backend.rs",
     "const SIMD_MIN_WORDS: usize = crate::tuning::baked::SIMD_MIN_WORDS;", 1,
     "A `gf2_tuning_baked` build substitutes the baked bit-backend threshold."),
    ("bit-threshold-default-alias", CORE, "src/kernels/backend.rs",
     "const SIMD_MIN_WORDS: usize = SIMD_MIN_WORDS_DEFAULT;", 1,
     "An ordinary build compares with the conservative constant."),
    ("bit-threshold-read", CORE, "src/kernels/backend.rs",
     "    if _size >= SIMD_MIN_WORDS {", 1,
     "One compile-time comparison, compiled under `simd`, selects the SIMD backend."),
    ("ops-xor-resolver", CORE, "src/kernels/ops.rs",
     "            .map(|backend| backend.xor_fn)", 1,
     "`resolve_xor_inplace` takes the bundle's XOR when the bundle is detected."),
    ("ops-popcount-resolver", CORE, "src/kernels/ops.rs",
     "            .map(|backend| backend.popcnt_fn)", 1,
     "`resolve_popcount` takes the nibble-lookup count; no resolver names another kernel."),
    ("ops-and-popcount-resolver", CORE, "src/kernels/ops.rs",
     "            .map(|backend| backend.and_popcnt_fn)", 1,
     "`resolve_and_popcount` takes the fused nibble-lookup count."),
    ("ops-popcount-route", CORE, "src/kernels/ops.rs",
     "pub fn popcount_route(word_len: usize) -> PopcountRoute {", 1,
     "The public reporter of the population-count route."),
    ("popcount-csa-comparator", SIMD, "src/lib.rs",
     "    pub and_popcnt_csa_fn: fn(&[u64], &[u64]) -> u64,", 1,
     "The carry-save kernels are direct bundle fields that no resolver selects."),
    # BitMatrix::matvec.
    ("matvec-threshold-conservative", CORE, "src/matrix.rs",
     "pub(crate) const MATVEC_SIMD_MIN_WORDS: usize = 8;", 1,
     "The conservative definition of `bit_matrix.matvec_simd_min_words`."),
    ("matvec-threshold-baked-alias", CORE, "src/matrix.rs",
     "const MATVEC_SIMD_MIN_WORDS_SELECTED: usize = crate::tuning::baked::MATVEC_SIMD_MIN_WORDS;",
     1, "A `gf2_tuning_baked` build substitutes the baked matvec threshold."),
    ("matvec-threshold-read", CORE, "src/matrix.rs",
     "        if stride_words >= MATVEC_SIMD_MIN_WORDS_SELECTED {", 1,
     "One compile-time comparison of the row stride selects the SIMD lane."),
    ("matvec-bundle-check", CORE, "src/matrix.rs",
     "        if let Some(fns) = crate::simd::maybe_simd() {", 1,
     "The SIMD lane runs only when the logical bundle is detected."),
    ("matvec-fused-kernel", CORE, "src/matrix.rs",
     "            y.push_bit((fns.and_popcnt_fn)(row, x_words) & 1 == 1);", 1,
     "The SIMD lane folds each row through the fused nibble-lookup kernel."),
    # BitMatrix::transpose.
    ("transpose-lane-selection", CORE, "src/matrix.rs",
     "    if let Some(fns) = crate::simd::maybe_transpose() {", 1,
     "`BitMatrix::transpose` takes the detected lane under `simd` and the portable "
     "kernel otherwise."),
    ("transpose-lane-reporter", CORE, "src/matrix.rs",
     "pub fn transpose_block_lane() -> gf2_kernels_simd::transpose::TransposeLane {", 1,
     "The public reporter of the block lane reads the selection `transpose` uses."),
    ("transpose-outer-route", CORE, "src/matrix.rs",
     "    if n_row_blocks <= transpose_simple_max_blocks && n_col_blocks <= "
     "transpose_simple_max_blocks {", 1,
     "The outer loop is chosen from two runtime profile fields."),
    ("transpose-preference", SIMD, "src/transpose.rs",
     "pub const PRODUCTION_PREFERENCE: [TransposeLane; 2] =", 1,
     "The production lane order is a constant of the kernel crate, outside the tuning "
     "sections."),
    ("transpose-preference-value", SIMD, "src/transpose.rs",
     "    [TransposeLane::Avx2BitTwiddle, TransposeLane::Scalar];", 1,
     "The published AVX2 lane is the bit-twiddle lane."),
    ("transpose-lane-feature-gate", SIMD, "src/transpose.rs",
     '                if !is_x86_feature_detected!("avx2") {', 1,
     "Every AVX2 lane is published only after runtime AVX2 detection."),
    # Residual shifts.
    ("shift-route-selection", CORE, "src/residual_shift.rs",
     "        if let Some(fns) = crate::simd::maybe_shift_funnel() {", 1,
     "The residual shift takes the kernel funnel under `simd` when it is detected."),
    ("shift-feature-gate", SIMD, "src/shift_funnel.rs",
     '        if is_x86_feature_detected!("bmi2") {', 1,
     "The funnel kernel is published only after runtime BMI2 detection; no size "
     "threshold takes part."),
    # Carry-less products.
    ("clmul-wide-width-4", CORE, "src/gf2m/wide.rs", "        if N == 4 {", 1,
     "The wide dispatch selects a kernel at four words."),
    ("clmul-wide-width-9", CORE, "src/gf2m/wide.rs", "        } else if N == 9 {", 1,
     "The wide dispatch selects a kernel at nine words; other widths are portable."),
    ("clmul-wide-ymm-predicate", SIMD, "src/gf2m_wide.rs",
     '        && is_x86_feature_detected!("vpclmulqdq")', 1,
     "The preferred wide lane needs AVX2, VPCLMULQDQ and SSE4.1."),
    ("clmul-batch-default-lane", SIMD, "src/gf2m.rs",
     "    detect_with_clmul_batch_preference(ClmulBatchLane::Sequential)", 1,
     "The default raw-batch lane is sequential PCLMULQDQ; the YMM lane needs an "
     "explicit preference."),
    ("clmul-batch-feature-gate", SIMD, "src/gf2m.rs",
     '    if is_x86_feature_detected!("pclmulqdq") && is_x86_feature_detected!("sse4.1") {', 2,
     "The GF(2^m) bundle exists only with PCLMULQDQ and SSE4.1."),
    ("dot-batch-kernel", CORE, "src/field/vec.rs",
     "        let batch_fn = sample.clmul_batch_fn()?;", 1,
     "The GF(2^m) dot product takes the batch kernel whenever the bundle offers one; "
     "no length gate takes part."),
    ("dot-chunk-read", CORE, "src/field/vec.rs",
     "        self.simd_dot_product_chunked::<DOT_CHUNK_LEN_SELECTED>(rhs)", 1,
     "The dot product reads its scratch extent from the baked selection site."),
    # GF(2^8) product table.
    ("gf256-table-predicate", CORE, "src/gf2m/byte_table.rs",
     "    if degree == 8 && single_u64_word && gf256_table_lane_enabled() && "
     "shared_field_context() {", 1,
     "The product-table lane is decided by field degree and representation alone; no "
     "cargo feature, processor feature or size threshold takes part."),
    # Tuning mechanism.
    ("tuning-baked-module-gate", CORE, "src/tuning/mod.rs",
     "#[cfg(any(test, gf2_tuning_baked))]", 1,
     "The baked constants compile only under test or the baked configuration."),
    ("tuning-baked-owner-digest", CORE, "src/tuning/baked.rs",
     "/// with SHA-256 `8904f7d0c9ef0577790b2af6c42b9da632306842e7928a54ea7251a51c3c4ffa`.", 1,
     "The baked constants pin the measured core owner by content digest."),
    ("tuning-active-accessor", CORE, "src/tuning/mod.rs",
     "pub fn active() -> ActiveSection<'static, CoreTuning> {", 1,
     "Runtime selectors resolve through one process-wide accessor."),
    ("tuning-install-one-shot", CORE, "src/tuning/mechanism.rs",
     "pub fn install(prepared: PreparedEnvelope) -> Result<(), AlreadyResolved> {", 1,
     "A profile takes effect only through an explicit installation, once per process."),
    ("tuning-frozen-before-install", CORE, "src/tuning/mechanism.rs",
     "    FrozenBeforeInstall {", 2,
     "Access before installation freezes the conservative table for the process."),
    ("coding-tuning-section", CODING, "src/tuning.rs", "pub struct EncodeSelectors {", 1,
     "The BCH encode selectors are a coding-owned section outside this issue's "
     "calibration."),
    # Offline calibration tooling.
    ("calibration-fields", CORE, "benches/tuning_calibration.rs",
     "    const ALL: [Self; 19] = [", 1,
     "The core producer sweeps a closed list of retained threshold fields."),
    ("calibration-bit-arm", CORE, "benches/tuning_calibration.rs",
     "                Arm::Conservative => &ScalarBackend,", 1,
     "The producer calibrates `bit_backend.simd_min_words` on the XOR of the two "
     "backends called directly."),
    ("calibration-launcher-declaration", None, "tuning-extent-campaign.sh",
     "# An issue (eight lowercase hex digits) selects the one campaign-declaration.json", 1,
     "The launcher runs the campaign a committed declaration names for an issue."),
    ("calibration-publishes-profile-id", None, "tuning-extent-campaign.sh",
     "# are invalid. The ID becomes the emitted profiles' ProfileId, so it stays", 1,
     "The run identifier becomes the published profile's identity."),
    ("calibration-inventory-closed", CORE, "benches/tuning_calibration.rs",
     "const EXPECTED_CORE_SCHEMA_FIELDS: usize = 37;", 1,
     "The producer publishes only when its measured and omitted fields partition a fixed "
     "codec inventory."),
    ("calibration-validator-behavior", None, "validate-tuning-extent-campaign.py",
     'CORE_BEHAVIOR = "tuning-calibration-v4"', 1,
     "The independent validator pins the producer's behavior token."),
    ("matvec-lane-entry", CORE, "src/matrix.rs",
     "    pub fn matvec_with_route(&self, x: &crate::BitVec, route: MatvecRoute) -> "
     "crate::BitVec {", 1,
     "One public entry runs the dense product on a caller-chosen lane, so one build "
     "times both lanes at one stride."),
    ("matvec-lane-entry-test", CORE, "tests/simd_equiv_matvec.rs",
     "fn each_route_matches_the_reference_at_stride_and_word_boundaries() {", 1,
     "Both lanes of the entry match the bit-level reference at the stride and word "
     "boundaries."),
    ("baked-matvec-conservative-test", CORE, "src/tuning/baked.rs",
     "    fn matvec_simd_min_words_matches_conservative_section() {", 1,
     "A unit test holds the baked matvec threshold equal to the conservative value "
     "while no measured owner states the field."),
    ("baked-measured-owner-test", CORE, "src/tuning/baked.rs",
     "    fn baked_simd_threshold_matches_the_strict_measured_owner() {", 1,
     "A unit test holds a measured baked constant equal to the measured owner's value."),
    ("codec-token-value", CORE, "src/tuning/mod.rs",
     'const CORE_HARNESS_SCHEMA: &str = "tuning-calibration-v4";', 1,
     "The core codec names one producer behavior token."),
    ("codec-accepts-one-token", CORE, "src/tuning/mod.rs",
     "                if harness_schema.as_str() == CORE_HARNESS_SCHEMA =>", 1,
     "A calibrated core section reopens only under that token, so a new token would "
     "reject every committed measured owner."),
    ("seam-token-unchanged", SEAM_PROTOCOL, None,
     "The core behaviour token stays `tuning-calibration-v4` and the owner protocol", 1,
     "The seam campaign added three swept fields under the unchanged token because case "
     "and result wire shapes did not change."),
    ("seam-identity-in-manifest", SEAM_PROTOCOL, None,
     "[`producing-build-inputs.json`](producing-build-inputs.json) carries the", 1,
     "A campaign's identity is its producing manifest, not the token."),
    # Frozen protocol arithmetic.
    ("protocol-family-alpha", SUPPORT, "src/protocol.rs", "    family_alpha: 0.05,", 1,
     "The frozen family-wise alpha."),
    ("protocol-bootstrap-resamples", SUPPORT, "src/protocol.rs",
     "    bootstrap_resamples: 10_000,", 1, "The frozen bootstrap resample count."),
    ("protocol-confirmatory-pairs", SUPPORT, "src/protocol.rs",
     "    confirmatory_pairs: 24,", 1, "The frozen pair count of a confirmatory or holdout cell."),
    ("protocol-pilot-min-pairs", SUPPORT, "src/protocol.rs", "    pilot_min_pairs: 6,", 1,
     "The frozen minimum pair count of an exploratory cell."),
    ("protocol-windows-per-execution", SUPPORT, "src/protocol.rs",
     "    windows_per_execution: 5,", 1, "The frozen window count of one execution."),
    ("protocol-window-target", SUPPORT, "src/protocol.rs", "    window_target_ms: 100,", 1,
     "The frozen window target length."),
    ("protocol-holdout-required", SUPPORT, "src/protocol.rs",
     '                "selector-calibration and final-integration families require holdout '
     'cells".into(),', 1,
     "A selector-calibration addendum without holdout cells is rejected."),
    ("ledger-attempt-alpha", SUPPORT, "src/trial_ledger.rs",
     "    Ok(family.family_wise.alpha / (attempts * (attempts + 1.0)))", 1,
     "Attempt t of a family spends alpha / (t (t + 1))."),
    ("ledger-comparisons-sum", SUPPORT, "src/trial_ledger.rs",
     "            n.checked_add(e.comparisons)", 1,
     "The comparison count is the sum of every reservation of the ledger prefix."),
    ("ledger-one-attempt-per-candidate", SUPPORT, "src/trial_ledger.rs",
     "                    || !attempted.insert((entry.protocol_version, id.clone()))", 1,
     "A candidate identity reserves at most once per protocol version."),
    ("receipt-corrected-alpha", SUPPORT, "src/receipt.rs",
     "            corrected_alpha: attempt_alpha / f64::from(comparisons),", 1,
     "The corrected per-comparison alpha divides the attempt alpha by that sum."),
    ("receipt-tail-rule", SUPPORT, "src/receipt.rs",
     "                || f64::from(settings.bootstrap_resamples) * corrected_alpha / 2.0 < 20.0;",
     1, "P-20 records a cell not-confirmatory below twenty expected draws per tail."),
    # Fallback tests.
    ("contract-function", CORE, "src/dispatch_contract.rs",
     "pub fn assert_fallback_contract() -> Vec<RouteWitness> {", 1,
     "The shared fallback contract every build configuration runs."),
    ("contract-unprofiled", CORE, "src/dispatch_contract.rs",
     "            SectionResolution::FrozenBeforeInstall { .. }", 1,
     "The contract requires the conservative table of a process without a profile."),
    ("contract-missing-feature", CORE, "src/dispatch_contract.rs",
     '            "a build without the `simd` feature takes portable routes only"', 1,
     "The contract requires portable routes only in a build without `simd`."),
    ("contract-core-test", CORE, "tests/dispatch_fallback.rs",
     "fn every_route_keeps_the_fallback_contract() {", 1,
     "The standalone `gf2-core` suite runs the contract."),
    ("contract-coding-test", CODING, "tests/core_dispatch_fallback.rs",
     "fn the_dependency_build_keeps_the_fallback_contract() {", 1,
     "The `gf2-coding` suite runs the contract on its dependency build."),
    ("contract-sim-test", SIM, "tests/core_dispatch_fallback.rs",
     "fn the_dependency_build_keeps_the_fallback_contract() {", 1,
     "The `gf2-sim` suite runs the contract on its dependency build."),
    ("fallback-clmul-forced", CORE, "tests/clmul_wide_conformance.rs",
     "fn portable_fallback_matches_the_oracle_at_every_width() {", 1,
     "The wide carry-less product is held on its portable lane and checked."),
    ("fallback-shift-forced", CORE, "tests/residual_shift_routes.rs",
     "fn every_route_answers_the_shift_corpus() {", 1,
     "The residual shift is held on its portable funnel and checked."),
    ("fallback-gf256-forced", CORE, "tests/gf256_table_conformance.rs",
     "fn the_force_switch_holds_every_caller_on_the_scalar_lane() {", 1,
     "The GF(2^8) table lane is declined by its switch and checked."),
    ("fallback-popcount-automatic", CORE, "tests/popcount_routes.rs",
     "fn automatic_routes_retain_the_established_implementations() {", 1,
     "The automatic count routes are asserted with and without the kernel bundle."),
    ("fallback-profile-access", CORE, "tests/tuning_process_lifecycle.rs",
     "fn access_before_install_freezes_and_reports_the_original_site() {", 1,
     "Access before installation freezes the conservative table."),
    ("fallback-profile-missing-section", CORE, "tests/tuning_process_lifecycle.rs",
     "fn installed_envelope_missing_the_type_defaults_conservatively() {", 1,
     "A profile without the section defaults that section conservatively."),
    ("fallback-baked-independent", CORE, "tests/tuning_conservative_cfg.rs",
     '        "an ordinary build must select the unconditional conservative pair"', 1,
     "An ordinary build selects conservative baked-family values whatever profile is "
     "installed."),
    ("fallback-coding-forced", CODING, "tests/bch_encode_dispatch.rs",
     "fn the_forced_scalar_kernels_write_the_reference_bytes() {", 1,
     "The BCH encode kernels are held on their scalar bundle and checked."),
]


def last_change(path):
    """The commit that last changed `path`; content digests decide validity."""
    return subprocess.run(
        ["git", "-C", str(ROOT), "log", "-1", "--format=%H", "--", path],
        capture_output=True, check=True, text=True,
    ).stdout.strip()


def main():
    records, failures = [], []
    if len({claim[0] for claim in CLAIMS}) != len(CLAIMS):
        raise SystemExit("claim identifiers repeat")
    for identifier, owner, relative, fragment, occurrences, why in CLAIMS:
        if isinstance(owner, tuple):
            path = repository_files.document(ROOT, *owner)
        elif owner is None:
            path = repository_files.live_file(ROOT, relative)
        else:
            path = f"{package(owner)}/{relative}"
        text = (ROOT / path).read_text()
        lines = text.splitlines()
        positions = [index + 1 for index, line in enumerate(lines) if fragment in line]
        if len(positions) != occurrences:
            failures.append(
                f"{identifier}: {path}: expected {occurrences} occurrences of {fragment!r}, "
                f"found {len(positions)}"
            )
            continue
        records.append(
            {
                "id": identifier,
                "project": "gf2",
                "commit": last_change(path),
                "path": path,
                "line": positions[0],
                "occurrences": occurrences,
                "verbatim": lines[positions[0] - 1],
                "why": why,
                "sha256": hashlib.sha256(text.encode()).hexdigest(),
            }
        )
    if failures:
        raise SystemExit("\n".join(failures))
    output = HERE / "source-evidence.json"
    output.write_text(
        json.dumps({"schema": "source-evidence-v1", "issue": "63bad95d", "claims": records}, indent=2)
        + "\n"
    )
    print(f"{output.relative_to(ROOT)}: {len(records)} source claims")


if __name__ == "__main__":
    main()
