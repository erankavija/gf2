//! Multi-arch gfx target detection and per-arch kernel-blob loading
//! (design doc §6).
//!
//! [`GfxTarget`] enumerates the compile-time gfx target list from design doc
//! §6. [`GfxTarget::detect`] reads the device's GCN arch **name** via
//! `hipGetDeviceProperties().gcnArchName` and maps the name string (e.g.
//! `"gfx1030"`, `"gfx940"`, `"gfx942"`) to a target. The name is the
//! authoritative discriminator: compute capability cannot distinguish the
//! gfx940 and gfx942 CDNA3 steppings, which load different kernel blobs.
//!
//! On an arch this build has no blob for, detection returns
//! [`HipError::UnsupportedArch`]; the dispatcher emits a `tracing::warn!` and
//! falls back to the CPU-equivalent stage (consistent with the §8 OOM policy).
//! A host with no visible device returns [`HipError::NoDevice`], which the
//! `gf2-sim` boundary maps to `FatalError::DeviceUnavailable` (design doc §8).
//!
//! Per-arch kernel blobs are produced by `build.rs` (one `*.co` per target
//! under `kernels/<target>/`) and loaded by [`GfxTarget::blob_dir`] /
//! [`GfxTarget::load_blob`]. gfx1030 is the only CI target today; the others
//! are documented seams that are unexercised until hardware is available.

use std::path::PathBuf;

use crate::{check_hip, ffi, HipError};

/// Comma-separated list of gfx targets THIS build actually compiled a usable
/// kernel blob for (emitted by `build.rs` as `GF2_HIP_COMPILED_ARCHS`).
/// gfx1030 is always present (mandatory); best-effort arches appear only when
/// hipcc succeeded. Empty string when none compiled.
///
/// `has_compiled_blob` consults THIS build-time manifest — not a runtime scan
/// of the gitignored `kernels/` output dir, where stale residue from a prior
/// build could wrongly report support.
const COMPILED_ARCHS: &str = env!("GF2_HIP_COMPILED_ARCHS");

/// Capacity of the arch-name buffer handed to `hip_device_get_arch_name`.
/// `gcnArchName` plus any feature suffix (e.g. `"gfx942:sramecc+:xnack-"`) is
/// comfortably under this; truncation only affects the (already-stripped)
/// suffix, never the `gfxNNNN` head.
const ARCH_NAME_BUF_LEN: usize = 256;

/// A compile-time gfx kernel target (design doc §6).
///
/// gfx1030 (RDNA2) is the only target exercised in CI today; the remaining
/// variants are seams whose kernel blobs are best-effort-compiled by `build.rs`
/// and whose runtime detection is wired but unexercised until matching hardware
/// is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GfxTarget {
    /// RDNA2 — RX 6800 / 6900 / 6950 XT. Compute capability 10.3. CI target.
    Gfx1030,
    /// RDNA3 — RX 7900 XT / XTX. Compute capability 11.0. Seam only.
    Gfx1100,
    /// RDNA4. Compute capability 12.0. Seam only.
    Gfx1200,
    /// CDNA2 — MI200. Compute capability 9.0. Seam only.
    Gfx90a,
    /// CDNA3 — MI300 (gfx940 stepping). Compute capability 9.4. Seam only.
    Gfx940,
    /// CDNA3 — MI300 (gfx942 stepping). Compute capability 9.4. Seam only.
    Gfx942,
}

impl GfxTarget {
    /// Every target in declaration order. Used by `build.rs` to iterate the
    /// best-effort compile list and by tests to round-trip the string form.
    pub const ALL: [GfxTarget; 6] = [
        GfxTarget::Gfx1030,
        GfxTarget::Gfx1100,
        GfxTarget::Gfx1200,
        GfxTarget::Gfx90a,
        GfxTarget::Gfx940,
        GfxTarget::Gfx942,
    ];

    /// The canonical gfx identifier string (e.g. `"gfx1030"`), matching the
    /// `--offload-arch=<target>` argument and the `kernels/<target>/` blob
    /// directory name.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_kernels_hip::host::GfxTarget;
    ///
    /// assert_eq!(GfxTarget::Gfx1030.as_str(), "gfx1030");
    /// assert_eq!(GfxTarget::Gfx942.as_str(), "gfx942");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            GfxTarget::Gfx1030 => "gfx1030",
            GfxTarget::Gfx1100 => "gfx1100",
            GfxTarget::Gfx1200 => "gfx1200",
            GfxTarget::Gfx90a => "gfx90a",
            GfxTarget::Gfx940 => "gfx940",
            GfxTarget::Gfx942 => "gfx942",
        }
    }

    /// Maps a GCN arch **name** string to a [`GfxTarget`].
    ///
    /// The name is matched against the canonical `gfxNNNN` head of
    /// `gcnArchName`. Any feature suffix (e.g. `":sramecc+:xnack-"`) is stripped
    /// before matching, so `"gfx942:sramecc+"` and `"gfx942"` both map to
    /// [`GfxTarget::Gfx942`]. Returns `None` for an arch this build has no blob
    /// for.
    ///
    /// Unlike compute-capability matching, this distinguishes the CDNA3
    /// steppings gfx940 and gfx942 — which report the **same** compute
    /// capability but load **different** kernel blobs — by their distinct name
    /// strings.
    ///
    /// # Arguments
    ///
    /// * `name` - The `gcnArchName` string from `hipGetDeviceProperties`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_kernels_hip::host::GfxTarget;
    ///
    /// // The feature suffix is stripped before matching.
    /// assert_eq!(
    ///     GfxTarget::from_arch_name("gfx942:sramecc+:xnack-"),
    ///     Some(GfxTarget::Gfx942)
    /// );
    /// // An arch this build has no blob for maps to None.
    /// assert_eq!(GfxTarget::from_arch_name("gfx908"), None);
    /// ```
    pub fn from_arch_name(name: &str) -> Option<Self> {
        // Strip the optional feature suffix: "gfx942:sramecc+:xnack-" → "gfx942".
        let head = name.split(':').next().unwrap_or(name).trim();
        GfxTarget::ALL.into_iter().find(|t| t.as_str() == head)
    }

    /// Detects the gfx target of HIP device 0 at runtime.
    ///
    /// Reads the device's GCN arch name via `gcnArchName` and maps it to a
    /// [`GfxTarget`]. On an arch this build has no kernel blob for, emits a
    /// `tracing::warn!` carrying the device id and arch name and returns
    /// [`HipError::UnsupportedArch`] so the caller falls back to the CPU
    /// equivalent stage (design doc §6 / §8). A host with no visible device
    /// returns [`HipError::NoDevice`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::GfxTarget;
    ///
    /// // Requires a real HIP device, so this is `no_run`.
    /// match GfxTarget::detect() {
    ///     Ok(target) => println!("detected {}", target.as_str()),
    ///     Err(e) => eprintln!("no usable GPU: {e}"),
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`HipError::NoDevice`] if no device is present,
    /// [`HipError::UnsupportedArch`] for an arch with no blob, or
    /// [`HipError::Hip`] if the underlying HIP query fails.
    pub fn detect() -> Result<Self, HipError> {
        Self::detect_device(0)
    }

    /// Detects the gfx target of a specific HIP device.
    ///
    /// See [`GfxTarget::detect`]; this variant lets the §7 multi-GPU seam probe
    /// each device.
    ///
    /// # Arguments
    ///
    /// * `device_id` - The HIP device index to probe.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::GfxTarget;
    ///
    /// // Probe device 0 (requires a real HIP device, hence `no_run`).
    /// let target = GfxTarget::detect_device(0).expect("a usable GPU on device 0");
    /// println!("device 0 is {}", target.as_str());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`HipError::NoDevice`] if the device count is zero,
    /// [`HipError::UnsupportedArch`] for an arch with no blob, or
    /// [`HipError::Hip`] if the underlying HIP query fails.
    pub fn detect_device(device_id: i32) -> Result<Self, HipError> {
        // Guard: a host with no GPU must not be probed (design doc §8 maps a
        // zero device count to DeviceUnavailable at the gf2-sim boundary).
        let mut count: i32 = 0;
        // SAFETY: `&mut count` is a valid out-pointer; the runtime writes it.
        check_hip(
            unsafe { ffi::hip_device_get_count(&mut count) },
            "hipGetDeviceCount",
        )?;
        if count <= 0 {
            return Err(HipError::NoDevice);
        }

        let arch_name = Self::query_arch_name(device_id)?;
        match Self::from_arch_name(&arch_name) {
            // Recognized AND this build produced a usable blob for it.
            Some(target) if target.has_compiled_blob() => Ok(target),
            // Recognized by name, but this build has no compiled blob for it
            // (best-effort arches are skipped when the toolchain lacks their
            // device libraries — e.g. gfx940 here). Treat as unsupported so the
            // dispatcher warns and falls back, instead of reporting a target
            // whose blob would fail to load at launch time.
            Some(target) => {
                tracing::warn!(
                    device_id,
                    gcn_arch_name = %arch_name,
                    "recognized gfx arch '{arch_name}' has no compiled kernel blob \
                     in this build; falling back to CPU stage"
                );
                let _ = target;
                Err(HipError::UnsupportedArch {
                    gcn_arch_name: arch_name,
                })
            }
            None => {
                tracing::warn!(
                    device_id,
                    gcn_arch_name = %arch_name,
                    "unsupported gfx arch '{arch_name}'; falling back to CPU stage"
                );
                Err(HipError::UnsupportedArch {
                    gcn_arch_name: arch_name,
                })
            }
        }
    }

    /// Reads the raw `gcnArchName` string of `device_id`.
    ///
    /// Internal helper for [`detect_device`](GfxTarget::detect_device).
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] if the underlying `hipGetDeviceProperties`
    /// query fails.
    fn query_arch_name(device_id: i32) -> Result<String, HipError> {
        let mut buf = [0i8; ARCH_NAME_BUF_LEN];
        // SAFETY: `buf` is a valid writable buffer of `ARCH_NAME_BUF_LEN` bytes;
        // the shim writes at most that many bytes and always NUL-terminates.
        check_hip(
            unsafe {
                ffi::hip_device_get_arch_name(
                    device_id,
                    buf.as_mut_ptr().cast::<std::os::raw::c_char>(),
                    ARCH_NAME_BUF_LEN,
                )
            },
            "hipGetDeviceProperties(gcnArchName)",
        )?;
        // Find the NUL terminator and decode the leading bytes as UTF-8 (arch
        // names are ASCII).
        let nul = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        let bytes: Vec<u8> = buf[..nul].iter().map(|&b| b as u8).collect();
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Directory holding this target's precompiled kernel blobs,
    /// `kernels/<target>/` relative to the crate root.
    ///
    /// The blobs (`*.co`) are produced by `build.rs`; consumers load a specific
    /// kernel via [`GfxTarget::load_blob`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_kernels_hip::host::GfxTarget;
    ///
    /// let dir = GfxTarget::Gfx1030.blob_dir();
    /// assert!(dir.ends_with("kernels/gfx1030"));
    /// ```
    pub fn blob_dir(self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("kernels")
            .join(self.as_str())
    }

    /// Returns `true` if **this build** compiled a usable kernel blob for this
    /// target.
    ///
    /// The answer comes from the compile-time `GF2_HIP_COMPILED_ARCHS` manifest
    /// emitted by `build.rs` — NOT a runtime scan of the gitignored
    /// `kernels/<target>/` output dir, where stale residue from a prior build
    /// could wrongly report support. `build.rs` compiles `gfx1030` blobs
    /// unconditionally (so this is always `true` for gfx1030 after a build) but
    /// treats every other target as best-effort — skipped when the toolchain
    /// lacks that arch's device libraries. A target whose name
    /// [`from_arch_name`](Self::from_arch_name) recognizes may therefore still
    /// have no usable blob in this build, in which case
    /// [`detect_device`](Self::detect_device) reports it as
    /// [`HipError::UnsupportedArch`] so the dispatcher warns and falls back to
    /// the CPU stage rather than failing at kernel-launch time.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::GfxTarget;
    ///
    /// // gfx1030 blobs are compiled unconditionally, so after a build this is
    /// // true on a host with the ROCm toolchain.
    /// assert!(GfxTarget::Gfx1030.has_compiled_blob());
    /// ```
    pub fn has_compiled_blob(self) -> bool {
        arch_in_manifest(self.as_str(), COMPILED_ARCHS)
    }

    /// Loads a named kernel blob (`<kernel>.co`) for this target.
    ///
    /// Returns the raw bytes for handoff to `hipModuleLoadData` by the kernel
    /// owner (next wave). Today the `kernels/<target>/` directory holds only a
    /// build-probe artefact for gfx1030; real kernels arrive with
    /// `f6004add` / `a930be7f` / `d3f1616a`.
    ///
    /// # Arguments
    ///
    /// * `kernel` - The blob basename without the `.co` extension.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gf2_kernels_hip::host::GfxTarget;
    ///
    /// // Reads `kernels/gfx1030/bcjr.co` (requires the blob to exist on disk,
    /// // hence `no_run`); a missing blob yields a typed `HipError::BlobLoad`.
    /// let bytes = GfxTarget::Gfx1030.load_blob("bcjr").expect("blob present");
    /// println!("loaded {} bytes", bytes.len());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`HipError::BlobLoad`] carrying the offending path if the blob
    /// file is missing or unreadable (e.g. a seam-only arch whose blob was not
    /// compiled on this host). This is a host-side file-I/O failure, so it is
    /// **not** reported as a [`HipError::Hip`] with a fabricated `hipError_t`
    /// code (code `0` would falsely read as `hipSuccess`).
    pub fn load_blob(self, kernel: &str) -> Result<Vec<u8>, HipError> {
        let path = self.blob_dir().join(format!("{kernel}.co"));
        std::fs::read(&path).map_err(|e| HipError::BlobLoad {
            path,
            source: e.to_string(),
        })
    }
}

/// Returns `true` if `name` appears as a comma-separated entry in the
/// build-time `GF2_HIP_COMPILED_ARCHS` manifest `csv`.
///
/// Pure string membership over the comma-separated `as_str()` names emitted by
/// `build.rs`; empty entries (from an empty manifest) never match a real arch
/// name. Factored out so the parsing is unit-testable with fixed strings.
fn arch_in_manifest(name: &str, csv: &str) -> bool {
    csv.split(',').any(|entry| entry == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arch_name_mapping() {
        assert_eq!(
            GfxTarget::from_arch_name("gfx1030"),
            Some(GfxTarget::Gfx1030)
        );
        assert_eq!(
            GfxTarget::from_arch_name("gfx1100"),
            Some(GfxTarget::Gfx1100)
        );
        assert_eq!(
            GfxTarget::from_arch_name("gfx1200"),
            Some(GfxTarget::Gfx1200)
        );
        assert_eq!(GfxTarget::from_arch_name("gfx90a"), Some(GfxTarget::Gfx90a));
        assert_eq!(GfxTarget::from_arch_name("gfx940"), Some(GfxTarget::Gfx940));
        assert_eq!(GfxTarget::from_arch_name("gfx942"), Some(GfxTarget::Gfx942));
        // Unknown arch → None (caller emits warn + CPU fallback).
        assert_eq!(GfxTarget::from_arch_name("gfx908"), None);
        assert_eq!(GfxTarget::from_arch_name(""), None);
    }

    /// gfx940 and gfx942 share a compute capability but are distinct targets;
    /// name-based detection must keep them apart (Finding 2).
    #[test]
    fn test_gfx940_vs_gfx942_distinct() {
        assert_eq!(GfxTarget::from_arch_name("gfx940"), Some(GfxTarget::Gfx940));
        assert_eq!(GfxTarget::from_arch_name("gfx942"), Some(GfxTarget::Gfx942));
        assert_ne!(
            GfxTarget::from_arch_name("gfx940"),
            GfxTarget::from_arch_name("gfx942")
        );
    }

    /// The `gcnArchName` feature suffix (e.g. ":sramecc+:xnack-") must be
    /// stripped before matching.
    #[test]
    fn test_arch_name_strips_feature_suffix() {
        assert_eq!(
            GfxTarget::from_arch_name("gfx942:sramecc+:xnack-"),
            Some(GfxTarget::Gfx942)
        );
        assert_eq!(
            GfxTarget::from_arch_name("gfx90a:xnack-"),
            Some(GfxTarget::Gfx90a)
        );
        assert_eq!(
            GfxTarget::from_arch_name("gfx1030:xnack-"),
            Some(GfxTarget::Gfx1030)
        );
    }

    #[test]
    fn test_as_str_roundtrip() {
        for t in GfxTarget::ALL {
            assert!(t.as_str().starts_with("gfx"));
        }
        assert_eq!(GfxTarget::Gfx1030.as_str(), "gfx1030");
    }

    #[test]
    fn test_blob_dir_layout() {
        let dir = GfxTarget::Gfx1030.blob_dir();
        assert!(dir.ends_with("kernels/gfx1030"));
    }

    /// A missing blob must surface a typed [`HipError::BlobLoad`] carrying the
    /// offending path — NOT a `HipError::Hip { code: 0 }`, which would falsely
    /// read as `hipSuccess` (Round-2 Finding B).
    #[test]
    fn test_load_blob_missing_is_typed_blobload_not_success_code() {
        // No arch has a kernel named this, so the read must fail.
        let err = GfxTarget::Gfx1030
            .load_blob("definitely_no_such_kernel_xyz")
            .expect_err("missing blob must fail");
        match &err {
            HipError::BlobLoad { path, source } => {
                assert!(
                    path.ends_with("definitely_no_such_kernel_xyz.co"),
                    "BlobLoad must carry the offending path, got {path:?}"
                );
                assert!(!source.is_empty(), "BlobLoad should describe the io error");
            }
            other => panic!("expected HipError::BlobLoad, got {other:?}"),
        }
        // The reported code must be a real, non-zero sentinel — never 0
        // (hipSuccess). hipErrorFileNotFound is canonically 301.
        assert_ne!(
            err.code(),
            0,
            "blob-load failure must not report hipSuccess"
        );
        assert_eq!(err.code(), 301);
        // Display must mention the path.
        assert!(err.to_string().contains("definitely_no_such_kernel_xyz"));
    }

    /// Runtime arch detection on this gfx1030 CI host. Gated to the `hip`
    /// feature so it only runs where a real GPU + runtime are present.
    #[cfg(feature = "hip")]
    #[test]
    fn test_detect_is_gfx1030_on_this_host() {
        match GfxTarget::detect() {
            Ok(target) => assert_eq!(
                target,
                GfxTarget::Gfx1030,
                "this CI host is a gfx1030 (RX 6950 XT)"
            ),
            Err(e) => panic!("arch detection failed on gfx1030 host: {e}"),
        }
    }

    #[test]
    fn test_gfx1030_has_compiled_blob_after_build() {
        // build.rs compiles gfx1030 unconditionally, so its blob dir holds at
        // least the probe `.co` after a build.
        assert!(
            GfxTarget::Gfx1030.has_compiled_blob(),
            "gfx1030 blob must exist after build"
        );
    }

    /// `arch_in_manifest` is pure comma-separated membership over the build-time
    /// `GF2_HIP_COMPILED_ARCHS` manifest. A recognized arch absent from the
    /// manifest (build SKIPPED it) reports no blob, so `detect_device` treats it
    /// as UnsupportedArch (warn + CPU fallback).
    #[test]
    fn test_arch_in_manifest_membership() {
        // A multi-arch manifest: only listed names match.
        let csv = "gfx1030,gfx942";
        assert!(arch_in_manifest("gfx1030", csv));
        assert!(arch_in_manifest("gfx942", csv));
        assert!(
            !arch_in_manifest("gfx940", csv),
            "an arch absent from the manifest (build skipped it) has no blob"
        );
        assert!(!arch_in_manifest("gfx1100", csv));

        // A single-entry manifest (the common gfx1030-only build).
        assert!(arch_in_manifest("gfx1030", "gfx1030"));
        assert!(!arch_in_manifest("gfx942", "gfx1030"));
    }

    /// An empty manifest (no arch compiled) must never match a real arch name —
    /// in particular the empty string produced by `"".split(',')` must not be
    /// taken as a wildcard.
    #[test]
    fn test_arch_in_manifest_empty_matches_nothing() {
        assert!(!arch_in_manifest("gfx1030", ""));
        assert!(!arch_in_manifest("gfx942", ""));
        // A real arch name is never empty, so the lone empty split entry is inert.
        assert!(!arch_in_manifest("", "gfx1030"));
    }
}
