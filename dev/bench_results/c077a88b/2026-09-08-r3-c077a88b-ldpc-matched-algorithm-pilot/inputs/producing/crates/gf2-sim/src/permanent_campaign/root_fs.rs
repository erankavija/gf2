//! Directory-handle-anchored I/O for one campaign root.
//!
//! Every mutable campaign path is resolved one component at a time beneath a
//! held root directory descriptor. Directory components and final files are
//! opened with `NOFOLLOW`, so a pathname replacement cannot redirect a writer
//! after admission.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use rustix::fs::{
    fstat, fsync, linkat, mkdirat, openat, renameat, statat, unlinkat, AtFlags, FileType, Mode,
    OFlags,
};

static TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);
static PROCESS_NONCE: OnceLock<u128> = OnceLock::new();

const DIRECTORY_MODE: Mode = Mode::from_raw_mode(0o755);
const FILE_MODE: Mode = Mode::from_raw_mode(0o644);

#[derive(Debug)]
struct RootInner {
    descriptor: OwnedFd,
    diagnostic_path: PathBuf,
}

/// A held identity for one campaign directory.
#[derive(Clone, Debug)]
pub(crate) struct CampaignRoot {
    inner: Arc<RootInner>,
}

/// Type of one entry observed without following it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntryKind {
    Missing,
    RegularFile,
    Other,
}

impl CampaignRoot {
    /// Opens and retains one exact directory identity.
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        let descriptor = open_root(path)?;
        let metadata = fstat(&descriptor).map_err(to_io)?;
        if FileType::from_raw_mode(metadata.st_mode) != FileType::Directory {
            return Err(invalid_input("campaign root is not a directory"));
        }
        let diagnostic_path = descriptor_path(&descriptor, path)?;
        Ok(Self {
            inner: Arc::new(RootInner {
                descriptor,
                diagnostic_path,
            }),
        })
    }

    /// Returns the path associated with the held directory for diagnostics.
    pub(crate) fn path(&self) -> &Path {
        &self.inner.diagnostic_path
    }

    /// Tests whether `path` currently opens the held directory identity.
    pub(crate) fn matches_path(&self, path: &Path) -> io::Result<bool> {
        let other = Self::open(path)?;
        let expected = fstat(&self.inner.descriptor).map_err(to_io)?;
        let actual = fstat(&other.inner.descriptor).map_err(to_io)?;
        Ok((expected.st_dev, expected.st_ino) == (actual.st_dev, actual.st_ino))
    }

    /// Reads one regular file without following any component symlink.
    pub(crate) fn read(&self, relative: &Path) -> io::Result<Vec<u8>> {
        let (parent, name) = self.parent(relative, false)?;
        let descriptor = openat(
            &parent,
            &name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(to_io)?;
        let metadata = fstat(&descriptor).map_err(to_io)?;
        if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile {
            return Err(invalid_input("campaign evidence is not a regular file"));
        }
        let mut file = File::from(descriptor);
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// Observes a final entry without following it or any parent component.
    pub(crate) fn entry_kind(&self, relative: &Path) -> io::Result<EntryKind> {
        let (parent, name) = match self.parent(relative, false) {
            Ok(value) => value,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(EntryKind::Missing),
            Err(error) => return Err(error),
        };
        let metadata = match statat(&parent, &name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(metadata) => metadata,
            Err(error) if error == rustix::io::Errno::NOENT => return Ok(EntryKind::Missing),
            Err(error) => return Err(to_io(error)),
        };
        Ok(
            if FileType::from_raw_mode(metadata.st_mode) == FileType::RegularFile {
                EntryKind::RegularFile
            } else {
                EntryKind::Other
            },
        )
    }

    /// Creates a regular file beneath the anchor and returns its held handle.
    pub(crate) fn open_lock_file(&self, relative: &Path) -> io::Result<File> {
        let (parent, name) = self.parent(relative, true)?;
        let descriptor = openat(
            &parent,
            &name,
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            FILE_MODE,
        )
        .map_err(to_io)?;
        let metadata = fstat(&descriptor).map_err(to_io)?;
        if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile {
            return Err(invalid_input("campaign lock is not a regular file"));
        }
        Ok(File::from(descriptor))
    }

    /// Atomically replaces one regular document within its verified parent.
    pub(crate) fn write_atomic_replace(&self, relative: &Path, bytes: &[u8]) -> io::Result<()> {
        let (parent, name) = self.parent(relative, true)?;
        let temporary = temporary_name(&name);
        let descriptor = openat(
            &parent,
            &temporary,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            FILE_MODE,
        )
        .map_err(to_io)?;
        if let Err(error) = write_and_sync(descriptor, bytes) {
            let _ = unlinkat(&parent, &temporary, AtFlags::empty());
            return Err(error);
        }
        if let Err(error) = renameat(&parent, &temporary, &parent, &name) {
            let _ = unlinkat(&parent, &temporary, AtFlags::empty());
            return Err(to_io(error));
        }
        fsync(&parent).map_err(to_io)
    }

    /// Atomically publishes a create-new document from a durable temporary.
    pub(crate) fn write_atomic_new(&self, relative: &Path, bytes: &[u8]) -> io::Result<()> {
        let (parent, name) = self.parent(relative, true)?;
        let temporary = temporary_name(&name);
        let descriptor = openat(
            &parent,
            &temporary,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            FILE_MODE,
        )
        .map_err(to_io)?;
        if let Err(error) = write_and_sync(descriptor, bytes) {
            let _ = unlinkat(&parent, &temporary, AtFlags::empty());
            return Err(error);
        }
        if let Err(error) = linkat(&parent, &temporary, &parent, &name, AtFlags::empty()) {
            let _ = unlinkat(&parent, &temporary, AtFlags::empty());
            return Err(to_io(error));
        }
        unlinkat(&parent, &temporary, AtFlags::empty()).map_err(to_io)?;
        fsync(&parent).map_err(to_io)
    }

    /// Publishes a create-new document or adopts byte-identical evidence.
    pub(crate) fn write_atomic_new_or_adopt(
        &self,
        relative: &Path,
        bytes: &[u8],
    ) -> io::Result<()> {
        match self.write_atomic_new(relative, bytes) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                if self.read(relative)? == bytes {
                    Ok(())
                } else {
                    Err(error)
                }
            }
            Err(error) => Err(error),
        }
    }

    /// Moves a regular entry between verified directories without replacement.
    pub(crate) fn move_new(&self, source: &Path, destination: &Path) -> io::Result<()> {
        let (source_parent, source_name) = self.parent(source, false)?;
        let (destination_parent, destination_name) = self.parent(destination, true)?;
        let source_metadata =
            statat(&source_parent, &source_name, AtFlags::SYMLINK_NOFOLLOW).map_err(to_io)?;
        if FileType::from_raw_mode(source_metadata.st_mode) != FileType::RegularFile {
            return Err(invalid_input(
                "quarantined campaign evidence is not a regular file",
            ));
        }
        if self.entry_kind(destination)? != EntryKind::Missing {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "quarantine destination already exists",
            ));
        }
        #[cfg(any(target_os = "linux", target_os = "android"))]
        rustix::fs::renameat_with(
            &source_parent,
            &source_name,
            &destination_parent,
            &destination_name,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(to_io)?;
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            linkat(
                &source_parent,
                &source_name,
                &destination_parent,
                &destination_name,
                AtFlags::empty(),
            )
            .map_err(to_io)?;
            unlinkat(&source_parent, &source_name, AtFlags::empty()).map_err(to_io)?;
        }
        fsync(&destination_parent).map_err(to_io)?;
        fsync(&source_parent).map_err(to_io)
    }

    fn parent(&self, relative: &Path, create: bool) -> io::Result<(OwnedFd, OsString)> {
        let components = relative_components(relative)?;
        let (name, parents) = components
            .split_last()
            .ok_or_else(|| invalid_input("campaign-relative path is empty"))?;
        let mut directory = rustix::io::dup(&self.inner.descriptor).map_err(to_io)?;
        for component in parents {
            directory = match openat(
                &directory,
                component,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            ) {
                Ok(next) => next,
                Err(error) if create && error == rustix::io::Errno::NOENT => {
                    match mkdirat(&directory, component, DIRECTORY_MODE) {
                        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                        Err(error) => return Err(to_io(error)),
                    }
                    openat(
                        &directory,
                        component,
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(to_io)?
                }
                Err(error) => return Err(to_io(error)),
            };
        }
        Ok((directory, name.clone()))
    }
}

fn relative_components(relative: &Path) -> io::Result<Vec<OsString>> {
    let mut components = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) if !value.is_empty() => components.push(value.to_owned()),
            _ => {
                return Err(invalid_input(
                    "campaign path must be normalized and relative",
                ))
            }
        }
    }
    Ok(components)
}

fn temporary_name(name: &OsStr) -> OsString {
    let id = TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
    let nonce = PROCESS_NONCE.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    });
    let mut temporary = OsString::from(".");
    temporary.push(name);
    temporary.push(format!(".{}.{nonce}.{id}.tmp", std::process::id()));
    temporary
}

fn write_and_sync(descriptor: OwnedFd, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::from(descriptor);
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(target_os = "linux")]
fn descriptor_path(descriptor: &OwnedFd, _supplied: &Path) -> io::Result<PathBuf> {
    std::fs::read_link(format!("/proc/self/fd/{}", descriptor.as_raw_fd()))
}

#[cfg(target_os = "linux")]
fn open_root(path: &Path) -> io::Result<OwnedFd> {
    rustix::fs::openat2(
        rustix::fs::CWD,
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
        rustix::fs::ResolveFlags::NO_SYMLINKS | rustix::fs::ResolveFlags::NO_MAGICLINKS,
    )
    .map_err(to_io)
}

#[cfg(not(target_os = "linux"))]
fn open_root(path: &Path) -> io::Result<OwnedFd> {
    rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(to_io)
}

/// Reads one absolute regular file through a single no-symlink resolution.
#[cfg(target_os = "linux")]
pub(crate) fn read_absolute(path: &Path) -> io::Result<Vec<u8>> {
    let descriptor = rustix::fs::openat2(
        rustix::fs::CWD,
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
        rustix::fs::ResolveFlags::NO_SYMLINKS | rustix::fs::ResolveFlags::NO_MAGICLINKS,
    )
    .map_err(to_io)?;
    read_regular_descriptor(descriptor)
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn read_absolute(path: &Path) -> io::Result<Vec<u8>> {
    let descriptor = rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(to_io)?;
    read_regular_descriptor(descriptor)
}

fn read_regular_descriptor(descriptor: OwnedFd) -> io::Result<Vec<u8>> {
    let metadata = fstat(&descriptor).map_err(to_io)?;
    if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile {
        return Err(invalid_input("campaign evidence is not a regular file"));
    }
    let mut file = File::from(descriptor);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(not(target_os = "linux"))]
fn descriptor_path(_descriptor: &OwnedFd, supplied: &Path) -> io::Result<PathBuf> {
    std::fs::canonicalize(supplied)
}

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn to_io(error: rustix::io::Errno) -> io::Error {
    io::Error::from_raw_os_error(error.raw_os_error())
}
