use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// The source of the [`io::Error`] that [`publish_atomic`] returns when the
/// target already holds the new bytes but the rename may not survive a crash.
/// Repeating the publish is safe.
#[derive(Debug)]
pub struct DurabilityUnconfirmed {
    path: PathBuf,
    source: io::Error,
}

impl fmt::Display for DurabilityUnconfirmed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "replaced {} without confirming durability: {}",
            self.path.display(),
            self.source
        )
    }
}

impl Error for DurabilityUnconfirmed {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

/// Publishes `bytes` at `path`, creating missing parent directories and
/// replacing any file already there. A reader sees the previous file or the
/// complete new one, and the new file survives a crash once this returns.
///
/// Each call syncs every ancestor directory; see [`create_durable`].
///
/// # Errors
///
/// Fails when `path` has no file name or a filesystem step fails. The target is
/// then unchanged, except that an error wrapping [`DurabilityUnconfirmed`] means
/// it was replaced. A crash can leave a hidden `.tmp` sibling behind.
pub fn publish_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    publish_atomic_with(path, bytes, sync_directory)
}

fn publish_atomic_with(
    path: &Path,
    bytes: &[u8],
    mut sync: impl FnMut(&Path) -> io::Result<()>,
) -> io::Result<()> {
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("publish path {} has no file name", path.display()),
        )
    })?;
    let directory = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    create_durable_with(directory, &mut sync)?;
    let (temporary, mut file) = reserve_sibling(directory, name)?;
    let published = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::rename(&temporary, path));
    if published.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    published?;
    sync(directory).map_err(|source| {
        io::Error::new(
            source.kind(),
            DurabilityUnconfirmed {
                path: path.to_owned(),
                source,
            },
        )
    })
}

/// Creates `directory` and its missing ancestors, and makes the whole chain
/// survive a crash once this returns, whichever process created each link.
///
/// Each call syncs every ancestor up to the filesystem root, because a directory
/// that exists may belong to a creator that has not synced it yet. The process
/// therefore needs read permission on every ancestor.
///
/// # Errors
///
/// Fails when a directory cannot be created or an ancestor cannot be opened or
/// synced, `PermissionDenied` on an unreadable ancestor included. Directories
/// already created then remain, without confirmed durability.
pub fn create_durable(directory: &Path) -> io::Result<()> {
    create_durable_with(directory, sync_directory)
}

fn create_durable_with(
    directory: &Path,
    mut sync: impl FnMut(&Path) -> io::Result<()>,
) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    let directory = if directory.as_os_str().is_empty() {
        Path::new(".")
    } else {
        directory
    };
    fs::canonicalize(directory)?
        .ancestors()
        .try_for_each(&mut sync)
}

fn sync_directory(directory: &Path) -> io::Result<()> {
    File::open(directory)?.sync_all()
}

/// Whether `candidate` is the hidden sibling an interrupted [`publish_atomic`]
/// left behind. Nothing reads such a file, so a caller with no publish running
/// may remove it.
#[must_use]
pub fn is_interrupted_publish(candidate: &Path) -> bool {
    let name = candidate.file_name().map(OsStr::to_string_lossy);
    name.as_deref()
        .and_then(|name| name.strip_prefix('.')?.strip_suffix(".tmp"))
        .and_then(|rest| {
            let (rest, sequence) = rest.rsplit_once('.')?;
            let (published, process) = rest.rsplit_once('.')?;
            let numeric = process.parse::<u32>().is_ok() && sequence.parse::<u64>().is_ok();
            Some(numeric && !published.is_empty())
        })
        .unwrap_or(false)
}

fn reserve_sibling(directory: &Path, name: &OsStr) -> io::Result<(PathBuf, File)> {
    loop {
        let mut temporary = OsString::from(".");
        temporary.push(name);
        temporary.push(format!(
            ".{}.{}.tmp",
            std::process::id(),
            NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
        ));
        let temporary = directory.join(temporary);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            // A crashed process with this identifier left the name behind.
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chain_that_already_exists_is_still_synced_to_the_root() {
        let directory = tempfile::tempdir().unwrap();
        let nested = directory.path().join("a/b");
        fs::create_dir_all(&nested).unwrap();

        let mut synced = Vec::new();
        create_durable_with(&nested, |ancestor| {
            synced.push(ancestor.to_owned());
            Ok(())
        })
        .unwrap();

        let canonical = fs::canonicalize(&nested).unwrap();
        for ancestor in canonical.ancestors() {
            assert!(synced.iter().any(|path| path == ancestor), "{ancestor:?}");
        }
    }

    #[test]
    fn only_a_reserved_sibling_is_an_interrupted_publish() {
        let directory = tempfile::tempdir().unwrap();
        let (temporary, _file) = reserve_sibling(directory.path(), OsStr::new("a.b")).unwrap();
        assert!(is_interrupted_publish(&temporary));
        for name in ["a.b", ".a.b", "a.1.2.tmp", ".a.x.2.tmp", ".1.2.tmp"] {
            assert!(!is_interrupted_publish(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn every_sync_failure_propagates() {
        let directory = tempfile::tempdir().unwrap();
        for kind in [io::ErrorKind::PermissionDenied, io::ErrorKind::Other] {
            let failure = create_durable_with(directory.path(), |_| Err(kind.into()));
            assert_eq!(failure.unwrap_err().kind(), kind);
        }
    }

    #[test]
    fn a_directory_sync_failure_after_the_rename_is_distinguished() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("parent/state");
        publish_atomic(&target, b"first").unwrap();

        let error = publish_atomic_with(&target, b"second", |_| {
            if fs::read(&target)? == b"second" {
                Err(io::ErrorKind::Other.into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();

        assert!(error
            .get_ref()
            .is_some_and(|source| source.is::<DurabilityUnconfirmed>()));
        assert_eq!(fs::read(&target).unwrap(), b"second");
    }
}
