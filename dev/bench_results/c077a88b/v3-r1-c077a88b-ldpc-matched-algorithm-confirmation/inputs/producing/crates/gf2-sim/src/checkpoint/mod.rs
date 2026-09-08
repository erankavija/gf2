//! Generic crash-safe checkpoint persistence.
//!
//! [`CheckpointWriter`] and [`CheckpointReader`] persist any
//! [`CheckpointPayload`] behind a caller-supplied [`ConfigHashProvider`]. The
//! on-disk envelope records the payload's stable identity and schema version
//! alongside the configuration hash. A reader accepts a file only when all
//! three values match its live caller contract; absence means fresh work,
//! while a present invalid or mismatched file is a hard [`CheckpointLoadError`].
//!
//! # Caller contract
//!
//! A caller must:
//!
//! * implement [`CheckpointPayload`] for an owned serde payload, choosing a
//!   stable, globally unambiguous [`CheckpointPayload::IDENTITY`] and bumping
//!   [`CheckpointPayload::SCHEMA_VERSION`] whenever the serialized meaning is
//!   not backward-compatible;
//! * implement [`ConfigHashProvider`] so its hash covers every configuration
//!   value that can affect resumed results, using a deterministic canonical
//!   encoding and excluding only output-location values that cannot affect the
//!   computation;
//! * put every value needed for deterministic continuation in the payload. The
//!   mechanism does not prescribe a resume key: absolute generator positions,
//!   shard counters, or another caller-owned representation are all valid;
//! * give one canonical file path to the matching writer and reader and treat
//!   every [`CheckpointLoadError`] as a refusal to resume, never as fresh work.
//!
//! [`CheckpointWriter::for_payload`] creates the parent directory. Each write
//! uses a PID-tagged temporary file, file fsync, atomic rename, and directory
//! fsync. Concurrent writers must still be externally coordinated: PID tagging
//! separates processes, not multiple writers in one process targeting the same
//! file.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// A serializable, owned payload stored by the checkpoint mechanism.
///
/// `IDENTITY` distinguishes unrelated payload families even when their JSON
/// happens to have the same shape. It must be stable across compatible program
/// versions, non-empty, and globally unambiguous within the application (for
/// example, `"permanent-campaign/shard-progress"`). `SCHEMA_VERSION` describes
/// the serialized meaning of this payload and must change when an older reader
/// cannot safely interpret the new representation.
///
/// The payload owns its resume semantics. It must contain every state value
/// needed for deterministic continuation; the persistence layer neither
/// derives nor interprets generator positions, counters, or completion keys.
/// Deserialization must not depend on ambient mutable state.
pub trait CheckpointPayload: Serialize + DeserializeOwned {
    /// Stable identity written into the checkpoint envelope.
    const IDENTITY: &'static str;
    /// Schema version written into the checkpoint envelope.
    const SCHEMA_VERSION: u32;
}

/// Supplies the deterministic hash that binds a checkpoint to live config.
///
/// Implementations must hash every input that can affect resumed results using
/// a canonical encoding. Paths and other output-only settings may be excluded
/// only when they cannot affect computation. The returned string is persisted
/// verbatim and compared for exact equality; callers should therefore include
/// an algorithm prefix such as `"blake3:"` and keep the encoding stable.
pub trait ConfigHashProvider {
    /// Returns the configuration identity for the current run.
    fn config_hash(&self) -> String;
}

impl ConfigHashProvider for String {
    fn config_hash(&self) -> String {
        self.clone()
    }
}

impl ConfigHashProvider for str {
    fn config_hash(&self) -> String {
        self.to_string()
    }
}

impl<T: ConfigHashProvider + ?Sized> ConfigHashProvider for &T {
    fn config_hash(&self) -> String {
        (*self).config_hash()
    }
}

#[derive(Serialize)]
struct CheckpointEnvelope<'a, P> {
    schema_version: u32,
    payload_identity: &'static str,
    config_hash: String,
    payload: &'a P,
}

#[derive(Deserialize)]
struct RawCheckpointEnvelope {
    schema_version: u32,
    payload_identity: String,
    config_hash: String,
    payload: serde_json::Value,
}

/// A hard refusal to resume from a present checkpoint.
///
/// Only file absence is represented as fresh work (`Ok(None)` from
/// [`CheckpointReader::load_payload`]). Every variant here means a checkpoint
/// was present but could not be safely associated with the caller's payload
/// and configuration contract.
#[derive(Debug)]
pub enum CheckpointLoadError {
    /// The checkpoint exists but could not be read.
    Io(std::io::Error),
    /// The file is not a structurally valid checkpoint envelope or payload.
    Invalid(serde_json::Error),
    /// The envelope's schema version differs from the payload contract.
    SchemaVersionMismatch {
        /// Version found on disk.
        loaded: u32,
        /// Version required by the payload type.
        expected: u32,
    },
    /// The envelope belongs to a different payload family.
    PayloadIdentityMismatch {
        /// Payload identity found on disk.
        loaded: String,
        /// Identity required by the payload type.
        expected: &'static str,
    },
    /// The checkpoint was written under a different configuration.
    ConfigHashMismatch {
        /// Configuration hash found on disk.
        loaded: String,
        /// Hash produced by the live provider.
        expected: String,
    },
}

impl std::fmt::Display for CheckpointLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "checkpoint read failed: {error}"),
            Self::Invalid(error) => write!(f, "invalid checkpoint: {error}"),
            Self::SchemaVersionMismatch { loaded, expected } => write!(
                f,
                "checkpoint schema version mismatch: loaded {loaded}, expected {expected}"
            ),
            Self::PayloadIdentityMismatch { loaded, expected } => write!(
                f,
                "checkpoint payload identity mismatch: loaded {loaded:?}, expected {expected:?}"
            ),
            Self::ConfigHashMismatch { loaded, expected } => write!(
                f,
                "checkpoint configuration hash mismatch: loaded {loaded:?}, expected {expected:?}"
            ),
        }
    }
}

impl std::error::Error for CheckpointLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Invalid(error) => Some(error),
            Self::SchemaVersionMismatch { .. }
            | Self::PayloadIdentityMismatch { .. }
            | Self::ConfigHashMismatch { .. } => None,
        }
    }
}

fn atomic_write_json(path: &Path, json: &[u8], on_pre_fsync: impl FnOnce()) -> std::io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let dir = parent.unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "checkpoint path must name a file",
        )
    })?;
    let tmp = dir.join(format!(
        "{}.{}.tmp",
        stem.to_string_lossy(),
        std::process::id()
    ));

    {
        use std::io::Write as _;
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(json)?;
        on_pre_fsync();
        file.sync_all()?;
    }

    std::fs::rename(&tmp, path)?;
    let directory = std::fs::File::open(dir)?;
    directory.sync_all()?;
    Ok(())
}

/// Atomic, crash-safe writer for a serializable checkpoint payload.
///
/// The writer serializes its payload inside an
/// identity/version/configuration-hash envelope. Every write uses a PID-tagged
/// temporary sibling, file fsync, rename, and directory fsync, so the canonical
/// path is always absent or a complete old or new checkpoint, never partially
/// written JSON. The rename is atomic on POSIX; directory fsync durably
/// persists the rename itself.
///
/// # Examples
///
/// ```no_run
/// use gf2_sim::checkpoint::{CheckpointPayload, CheckpointWriter, ConfigHashProvider};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct Progress { completed_shards: u64 }
/// impl CheckpointPayload for Progress {
///     const IDENTITY: &'static str = "example/shard-progress";
///     const SCHEMA_VERSION: u32 = 1;
/// }
/// struct ConfigHash;
/// impl ConfigHashProvider for ConfigHash {
///     fn config_hash(&self) -> String { "blake3:example".to_string() }
/// }
/// let writer = CheckpointWriter::<Progress, _>::for_payload(
///     "/tmp/progress.json",
///     ConfigHash,
/// ).unwrap();
/// writer.write_payload(&Progress { completed_shards: 3 }).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct CheckpointWriter<P, H> {
    path: PathBuf,
    hash_provider: H,
    payload: PhantomData<fn() -> P>,
}

impl<P, H> CheckpointWriter<P, H>
where
    P: CheckpointPayload,
    H: ConfigHashProvider,
{
    /// Creates a writer for one canonical checkpoint file.
    ///
    /// `path` names the final file, not a directory. Its parent directory is
    /// created when needed. `hash_provider` is evaluated for every write so a
    /// caller whose live configuration changes cannot stamp a stale cached
    /// hash onto a new checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the parent directory cannot be created or the
    /// path does not name a file.
    pub fn for_payload(path: impl Into<PathBuf>, hash_provider: H) -> std::io::Result<Self> {
        let path = path.into();
        if path.file_name().is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "checkpoint path must name a file",
            ));
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self {
            path,
            hash_provider,
            payload: PhantomData,
        })
    }

    /// Returns the writer's canonical checkpoint path.
    #[must_use]
    pub fn payload_path(&self) -> &Path {
        &self.path
    }

    /// Atomically writes `payload` in the validated envelope.
    ///
    /// # Errors
    ///
    /// Returns [`std::io::ErrorKind::InvalidInput`] if the payload declares an
    /// empty identity. A serialization error is wrapped as an I/O error;
    /// filesystem write, file fsync, rename, open-directory, and
    /// directory-fsync failures are propagated without weakening the
    /// durability contract.
    pub fn write_payload(&self, payload: &P) -> std::io::Result<()> {
        self.write_payload_with_fsync_hook(payload, || {})
    }

    /// Writes with a callback immediately before the temporary-file fsync.
    ///
    /// The callback runs after all temporary-file bytes have been written and
    /// immediately before file fsync. It exists for crash-safety testing;
    /// production callers use [`write_payload`](Self::write_payload).
    ///
    /// # Errors
    ///
    /// The same errors as [`write_payload`](Self::write_payload).
    #[doc(hidden)]
    pub fn write_payload_with_fsync_hook(
        &self,
        payload: &P,
        on_pre_fsync: impl FnOnce(),
    ) -> std::io::Result<()> {
        if P::IDENTITY.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "checkpoint payload identity must not be empty",
            ));
        }
        let envelope = CheckpointEnvelope {
            schema_version: P::SCHEMA_VERSION,
            payload_identity: P::IDENTITY,
            config_hash: self.hash_provider.config_hash(),
            payload,
        };
        let json = serde_json::to_vec_pretty(&envelope).map_err(std::io::Error::other)?;
        atomic_write_json(&self.path, &json, on_pre_fsync)
    }
}

/// Reader for a generic checkpoint payload.
///
/// The reader validates the envelope's schema version, payload identity, and
/// configuration hash before deserializing the payload. Missing files return
/// `Ok(None)`; present invalid files return [`CheckpointLoadError`] and must not
/// be treated as fresh work.
#[derive(Debug, Clone)]
pub struct CheckpointReader<P, H> {
    path: PathBuf,
    hash_provider: H,
    payload: PhantomData<fn() -> P>,
}

impl<P, H> CheckpointReader<P, H>
where
    P: CheckpointPayload,
    H: ConfigHashProvider,
{
    /// Creates a reader for one canonical checkpoint file.
    ///
    /// This does not create the parent directory: a missing directory and a
    /// missing file both mean fresh work.
    #[must_use]
    pub fn for_payload(path: impl Into<PathBuf>, hash_provider: H) -> Self {
        Self {
            path: path.into(),
            hash_provider,
            payload: PhantomData,
        }
    }

    /// Returns the reader's canonical checkpoint path.
    #[must_use]
    pub fn payload_path(&self) -> &Path {
        &self.path
    }

    /// Loads a payload only after all envelope identities match.
    ///
    /// # Returns
    ///
    /// `Ok(None)` only when the canonical file is absent. A valid matching file
    /// returns `Ok(Some(payload))`.
    ///
    /// # Errors
    ///
    /// Returns [`CheckpointLoadError::SchemaVersionMismatch`],
    /// [`CheckpointLoadError::PayloadIdentityMismatch`], or
    /// [`CheckpointLoadError::ConfigHashMismatch`] for the corresponding hard
    /// mismatch. Unreadable files and invalid JSON/envelopes are also hard
    /// errors. Callers must not convert these errors into fresh work.
    pub fn load_payload(&self) -> Result<Option<P>, CheckpointLoadError> {
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(CheckpointLoadError::Io(error)),
        };
        let raw: RawCheckpointEnvelope =
            serde_json::from_slice(&bytes).map_err(CheckpointLoadError::Invalid)?;
        if raw.schema_version != P::SCHEMA_VERSION {
            return Err(CheckpointLoadError::SchemaVersionMismatch {
                loaded: raw.schema_version,
                expected: P::SCHEMA_VERSION,
            });
        }
        if raw.payload_identity != P::IDENTITY {
            return Err(CheckpointLoadError::PayloadIdentityMismatch {
                loaded: raw.payload_identity,
                expected: P::IDENTITY,
            });
        }
        let expected_hash = self.hash_provider.config_hash();
        if raw.config_hash != expected_hash {
            return Err(CheckpointLoadError::ConfigHashMismatch {
                loaded: raw.config_hash,
                expected: expected_hash,
            });
        }
        serde_json::from_value(raw.payload)
            .map(Some)
            .map_err(CheckpointLoadError::Invalid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct UnrelatedPayload {
        shard_name: String,
        samples: u64,
    }

    impl CheckpointPayload for UnrelatedPayload {
        const IDENTITY: &'static str = "gf2-sim-test/unrelated-payload";
        const SCHEMA_VERSION: u32 = 7;
    }

    #[derive(Debug, Clone)]
    struct TestConfigHash(&'static str);

    impl ConfigHashProvider for TestConfigHash {
        fn config_hash(&self) -> String {
            self.0.to_string()
        }
    }

    #[test]
    fn test_generic_checkpoint_round_trips_unrelated_payload() {
        let dir = tempdir();
        let path = dir.join("shard-progress.json");
        let payload = UnrelatedPayload {
            shard_name: "q5-n12-s003".to_string(),
            samples: 41_000,
        };

        let writer = CheckpointWriter::<UnrelatedPayload, _>::for_payload(
            &path,
            TestConfigHash("blake3:campaign-a"),
        )
        .unwrap();
        writer.write_payload(&payload).unwrap();

        let reader = CheckpointReader::<UnrelatedPayload, _>::for_payload(
            &path,
            TestConfigHash("blake3:campaign-a"),
        );
        assert_eq!(reader.load_payload().unwrap(), Some(payload));
    }

    #[test]
    fn test_generic_reader_refuses_schema_identity_and_config_mismatches() {
        let dir = tempdir();
        let path = dir.join("generic.json");
        let payload = UnrelatedPayload {
            shard_name: "q7-n10-s001".to_string(),
            samples: 13,
        };
        CheckpointWriter::<UnrelatedPayload, _>::for_payload(
            &path,
            TestConfigHash("blake3:expected"),
        )
        .unwrap()
        .write_payload(&payload)
        .unwrap();

        let original: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let reader = || {
            CheckpointReader::<UnrelatedPayload, _>::for_payload(
                &path,
                TestConfigHash("blake3:expected"),
            )
        };

        let mut wrong_schema = original.clone();
        wrong_schema["schema_version"] = serde_json::json!(8);
        std::fs::write(&path, serde_json::to_vec(&wrong_schema).unwrap()).unwrap();
        assert!(matches!(
            reader().load_payload(),
            Err(CheckpointLoadError::SchemaVersionMismatch { .. })
        ));

        let mut wrong_identity = original.clone();
        wrong_identity["payload_identity"] = serde_json::json!("another-campaign/payload");
        std::fs::write(&path, serde_json::to_vec(&wrong_identity).unwrap()).unwrap();
        assert!(matches!(
            reader().load_payload(),
            Err(CheckpointLoadError::PayloadIdentityMismatch { .. })
        ));

        let mut wrong_hash = original;
        wrong_hash["config_hash"] = serde_json::json!("blake3:stale");
        std::fs::write(&path, serde_json::to_vec(&wrong_hash).unwrap()).unwrap();
        assert!(matches!(
            reader().load_payload(),
            Err(CheckpointLoadError::ConfigHashMismatch { .. })
        ));
    }

    #[test]
    fn test_generic_reader_treats_absence_as_fresh_work() {
        let dir = tempdir();
        let reader = CheckpointReader::<UnrelatedPayload, _>::for_payload(
            dir.join("absent.json"),
            TestConfigHash("blake3:fresh"),
        );
        assert_eq!(reader.load_payload().unwrap(), None);
    }

    fn tempdir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "gf2sim-generic-ck-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}
