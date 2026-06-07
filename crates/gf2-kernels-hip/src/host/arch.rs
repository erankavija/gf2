//! Multi-arch gfx target detection and per-arch kernel-blob loading
//! (design doc §6).
//!
//! [`GfxTarget`] enumerates the compile-time gfx target list from design doc
//! §6. [`GfxTarget::detect`] reads the device's compute capability via
//! `hipDeviceGetAttribute` and maps `(major, minor)` to a target. On an
//! unrecognized arch it emits a `tracing::warn!` and the caller falls back to
//! the CPU equivalent stage (consistent with the §8 OOM policy).
//!
//! Per-arch kernel blobs are produced by `build.rs` (one `*.co` per target
//! under `kernels/<target>/`) and loaded by [`GfxTarget::blob_dir`] /
//! [`GfxTarget::load_blob`]. gfx1030 is the only CI target today; the others
//! are documented seams that are unexercised until hardware is available.

use std::path::PathBuf;

use crate::{check_hip, ffi, HipError};

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

    /// Maps a HIP compute capability `(major, minor)` to a [`GfxTarget`].
    ///
    /// Returns `None` for an unrecognized capability. Note that the CDNA3
    /// steppings gfx940 and gfx942 both report compute capability `(9, 4)`; this
    /// mapping resolves the ambiguity to [`GfxTarget::Gfx942`] (the newer
    /// stepping). The distinction does not affect gfx1030, the only target
    /// exercised today.
    pub fn from_compute_capability(major: i32, minor: i32) -> Option<Self> {
        match (major, minor) {
            (10, 3) => Some(GfxTarget::Gfx1030),
            (11, 0) => Some(GfxTarget::Gfx1100),
            (12, 0) => Some(GfxTarget::Gfx1200),
            (9, 0) => Some(GfxTarget::Gfx90a),
            // gfx940 and gfx942 are indistinguishable by compute capability;
            // resolve to the newer stepping.
            (9, 4) => Some(GfxTarget::Gfx942),
            _ => None,
        }
    }

    /// Detects the gfx target of HIP device 0 at runtime.
    ///
    /// Reads the compute capability via `hipDeviceGetAttribute` and maps it to a
    /// [`GfxTarget`]. On an arch this build does not recognize, emits a
    /// `tracing::warn!` carrying the device id and capability, and returns
    /// [`HipError::Hip`] with context `"GfxTarget::detect: unsupported arch"` so
    /// the caller falls back to the CPU equivalent stage (design doc §6 / §8).
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] if no device is present, if the capability
    /// query fails, or if the capability is unrecognized.
    pub fn detect() -> Result<Self, HipError> {
        Self::detect_device(0)
    }

    /// Detects the gfx target of a specific HIP device.
    ///
    /// See [`GfxTarget::detect`]; this variant lets the §7 multi-GPU seam probe
    /// each device.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] if the device count is zero, the capability
    /// query fails, or the capability is unrecognized.
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
            return Err(HipError::Hip {
                code: 0,
                context: "GfxTarget::detect: no HIP devices",
            });
        }

        let mut major: i32 = 0;
        let mut minor: i32 = 0;
        // SAFETY: both out-pointers are valid; the runtime writes them on
        // success. `device_id` is validated by the runtime.
        check_hip(
            unsafe { ffi::hip_device_compute_capability(device_id, &mut major, &mut minor) },
            "hipDeviceGetAttribute(ComputeCapability)",
        )?;

        match Self::from_compute_capability(major, minor) {
            Some(target) => Ok(target),
            None => {
                tracing::warn!(
                    device_id,
                    cc_major = major,
                    cc_minor = minor,
                    "unsupported gfx arch (cc {major}.{minor}); falling back to CPU stage"
                );
                Err(HipError::Hip {
                    code: 0,
                    context: "GfxTarget::detect: unsupported arch",
                })
            }
        }
    }

    /// Directory holding this target's precompiled kernel blobs,
    /// `kernels/<target>/` relative to the crate root.
    ///
    /// The blobs (`*.co`) are produced by `build.rs`; consumers load a specific
    /// kernel via [`GfxTarget::load_blob`].
    pub fn blob_dir(self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("kernels")
            .join(self.as_str())
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
    /// # Errors
    ///
    /// Returns [`HipError::Hip`] with context `"GfxTarget::load_blob"` if the
    /// blob file is missing or unreadable (e.g. a seam-only arch whose blob was
    /// not compiled on this host).
    pub fn load_blob(self, kernel: &str) -> Result<Vec<u8>, HipError> {
        let path = self.blob_dir().join(format!("{kernel}.co"));
        std::fs::read(&path).map_err(|_| HipError::Hip {
            code: 0,
            context: "GfxTarget::load_blob",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_capability_mapping() {
        assert_eq!(
            GfxTarget::from_compute_capability(10, 3),
            Some(GfxTarget::Gfx1030)
        );
        assert_eq!(
            GfxTarget::from_compute_capability(11, 0),
            Some(GfxTarget::Gfx1100)
        );
        assert_eq!(
            GfxTarget::from_compute_capability(12, 0),
            Some(GfxTarget::Gfx1200)
        );
        assert_eq!(
            GfxTarget::from_compute_capability(9, 0),
            Some(GfxTarget::Gfx90a)
        );
        // gfx940/gfx942 both report (9, 4) → resolve to the newer stepping.
        assert_eq!(
            GfxTarget::from_compute_capability(9, 4),
            Some(GfxTarget::Gfx942)
        );
        assert_eq!(GfxTarget::from_compute_capability(7, 5), None);
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
}
