//! gfx target detection and per-arch kernel-blob loading: [`GfxTarget::detect`]
//! maps the device's `gcnArchName` (e.g. `"gfx1030"`, `"gfx942"`) to a target.
//! The name is the discriminator because compute capability cannot distinguish
//! the gfx940 and gfx942 CDNA3 steppings. `build.rs` produces one `*.co` blob
//! per source under `kernels/<target>/`; gfx1030 is the only target exercised
//! by tests.

use std::path::PathBuf;

use crate::{check_hip, ffi, HipError};

/// Comma-separated list of gfx targets this build compiled a kernel blob for,
/// emitted by `build.rs`. Empty when none compiled.
const COMPILED_ARCHS: &str = env!("GF2_HIP_COMPILED_ARCHS");

/// Capacity of the arch-name buffer handed to `hip_device_get_arch_name`.
/// `gcnArchName` plus any feature suffix (e.g. `"gfx942:sramecc+:xnack-"`) is
/// comfortably under this; truncation only affects the (already-stripped)
/// suffix, never the `gfxNNNN` head.
const ARCH_NAME_BUF_LEN: usize = 256;

/// A compile-time gfx kernel target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GfxTarget {
    /// RDNA2.
    Gfx1030,
    /// RDNA3.
    Gfx1100,
    /// RDNA4.
    Gfx1200,
    /// CDNA2.
    Gfx90a,
    /// CDNA3, gfx940 stepping.
    Gfx940,
    /// CDNA3, gfx942 stepping.
    Gfx942,
}

impl GfxTarget {
    /// Every target in declaration order.
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

    /// Maps a `gcnArchName` string to a [`GfxTarget`].
    ///
    /// Any feature suffix (e.g. `":sramecc+:xnack-"`) is stripped before
    /// matching, so `"gfx942:sramecc+"` maps to [`GfxTarget::Gfx942`]. Returns
    /// `None` for a name outside [`GfxTarget::ALL`].
    pub fn from_arch_name(name: &str) -> Option<Self> {
        let head = name.split(':').next().unwrap_or(name).trim();
        GfxTarget::ALL.into_iter().find(|t| t.as_str() == head)
    }

    /// Detects the gfx target of HIP device 0; see
    /// [`detect_device`](Self::detect_device).
    ///
    /// # Errors
    ///
    /// Returns [`HipError::NoDevice`] if no device is present,
    /// [`HipError::UnsupportedArch`] for an arch with no blob, or
    /// [`HipError::Hip`] if the underlying HIP query fails.
    pub fn detect() -> Result<Self, HipError> {
        Self::detect_device(0)
    }

    /// Detects the gfx target of HIP device `device_id` from its `gcnArchName`.
    ///
    /// An arch without a compiled kernel blob in this build is logged through
    /// `tracing::warn!` with the device id and arch name.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::NoDevice`] if the device count is zero,
    /// [`HipError::UnsupportedArch`] for an arch with no blob, or
    /// [`HipError::Hip`] if the underlying HIP query fails.
    pub fn detect_device(device_id: i32) -> Result<Self, HipError> {
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
            Some(target) if target.has_compiled_blob() => Ok(target),
            // Reporting a recognized arch without a compiled blob as a target
            // would fail at blob load.
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
        let nul = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        let bytes: Vec<u8> = buf[..nul].iter().map(|&b| b as u8).collect();
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Directory holding this target's kernel blobs, `kernels/<target>/` under
    /// the crate root.
    pub fn blob_dir(self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("kernels")
            .join(self.as_str())
    }

    /// Returns `true` if this build compiled a kernel blob for this target.
    ///
    /// The answer comes from the `GF2_HIP_COMPILED_ARCHS` manifest emitted by
    /// `build.rs`, which compiles gfx1030 unconditionally and skips any other
    /// target whose compilation fails.
    pub fn has_compiled_blob(self) -> bool {
        arch_in_manifest(self.as_str(), COMPILED_ARCHS)
    }

    /// Reads the kernel blob `<kernel>.co` of this target.
    ///
    /// # Errors
    ///
    /// Returns [`HipError::BlobLoad`] carrying the path if the blob file is
    /// missing or unreadable.
    pub fn load_blob(self, kernel: &str) -> Result<Vec<u8>, HipError> {
        let path = self.blob_dir().join(format!("{kernel}.co"));
        std::fs::read(&path).map_err(|e| HipError::BlobLoad {
            path,
            source: e.to_string(),
        })
    }
}

/// Returns `true` if `name` is an entry of the comma-separated manifest `csv`.
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
        assert_eq!(GfxTarget::from_arch_name("gfx908"), None);
        assert_eq!(GfxTarget::from_arch_name(""), None);
    }

    #[test]
    fn test_gfx940_vs_gfx942_distinct() {
        assert_eq!(GfxTarget::from_arch_name("gfx940"), Some(GfxTarget::Gfx940));
        assert_eq!(GfxTarget::from_arch_name("gfx942"), Some(GfxTarget::Gfx942));
        assert_ne!(
            GfxTarget::from_arch_name("gfx940"),
            GfxTarget::from_arch_name("gfx942")
        );
    }

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

    #[test]
    fn test_load_blob_missing_is_typed_blobload_not_success_code() {
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
        // hipErrorFileNotFound is 301.
        assert_ne!(
            err.code(),
            0,
            "blob-load failure must not report hipSuccess"
        );
        assert_eq!(err.code(), 301);
        assert!(err.to_string().contains("definitely_no_such_kernel_xyz"));
    }

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

    #[test]
    fn test_arch_in_manifest_membership() {
        let csv = "gfx1030,gfx942";
        assert!(arch_in_manifest("gfx1030", csv));
        assert!(arch_in_manifest("gfx942", csv));
        assert!(
            !arch_in_manifest("gfx940", csv),
            "an arch absent from the manifest (build skipped it) has no blob"
        );
        assert!(!arch_in_manifest("gfx1100", csv));

        assert!(arch_in_manifest("gfx1030", "gfx1030"));
        assert!(!arch_in_manifest("gfx942", "gfx1030"));
    }

    /// `"".split(',')` yields one empty entry, which must match no arch name.
    #[test]
    fn test_arch_in_manifest_empty_matches_nothing() {
        assert!(!arch_in_manifest("gfx1030", ""));
        assert!(!arch_in_manifest("gfx942", ""));
        assert!(!arch_in_manifest("", "gfx1030"));
    }
}
