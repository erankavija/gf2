//! Descriptor-relative artifact directory boundary.
//!
//! Every lookup, check, creation, read, rename, and removal below a pinned
//! dataset root travels through a held directory descriptor and a validated
//! single path component, with `O_NOFOLLOW` on every open. A concurrent
//! rename or symlink swap between a type check and the operation that relies
//! on it cannot change which inode is used: the check runs on the descriptor
//! that is subsequently read, written, or synchronized.
//!
//! Only [`DirHandle::open_root`] resolves a multi-component path, and it does
//! so once against an operator-supplied dataset root before any artifact work
//! begins. Everything beneath that root is descriptor-relative.

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use rustix::fs::{
    fsync, mkdirat, openat, renameat_with, statat, unlinkat, AtFlags, Dir, FileType, Mode, OFlags,
    RenameFlags, CWD,
};
use rustix::io::Errno;

use super::ArtifactError;

/// Permission bits for every artifact directory this module creates.
const DIRECTORY_MODE: u32 = 0o755;
/// Permission bits for every artifact file this module creates.
const FILE_MODE: u32 = 0o644;

/// One validated single path component usable as a `*at` name.
///
/// A component is non-empty, at most 255 bytes, neither `.` nor `..`, and
/// drawn from ASCII alphanumerics, `-`, `_`, and `.`. It therefore contains no
/// separator, so a descriptor-relative operation on it touches exactly one
/// directory entry of the held directory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Component<'a>(&'a str);

impl<'a> Component<'a> {
    /// Accepts one path component or refuses it as a publication name.
    pub(super) fn new(name: &'a str) -> Result<Self, ArtifactError> {
        let acceptable = !name.is_empty()
            && name.len() <= 255
            && name != "."
            && name != ".."
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
        if !acceptable {
            return Err(ArtifactError::Publication(format!(
                "{name} is not an accepted single path component"
            )));
        }
        Ok(Self(name))
    }

    /// Returns the exact component text.
    pub(super) fn as_str(self) -> &'a str {
        self.0
    }
}

/// One directory entry observed through the holding descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DirEntryInfo {
    /// Exact UTF-8 entry name.
    pub(super) name: String,
    /// Entry type resolved without following symbolic links.
    pub(super) file_type: FileType,
}

/// One held directory descriptor rooted at a known path.
///
/// The stored path is diagnostic only; no operation resolves it again.
#[derive(Debug)]
pub(super) struct DirHandle {
    fd: OwnedFd,
    path: PathBuf,
}

impl DirHandle {
    /// Pins one existing directory, refusing a symbolic link at the last component.
    pub(super) fn open_root(path: &Path) -> Result<Self, ArtifactError> {
        let fd = openat(
            CWD,
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| directory_error(path, error))?;
        Ok(Self {
            fd,
            path: path.to_owned(),
        })
    }

    /// Returns the path this descriptor was opened from.
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    /// Opens one child directory through the held descriptor.
    pub(super) fn open_dir(&self, name: Component<'_>) -> Result<Self, ArtifactError> {
        let child = self.path.join(name.as_str());
        let fd = openat(
            &self.fd,
            name.as_str(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| directory_error(&child, error))?;
        Ok(Self { fd, path: child })
    }

    /// Opens one child directory, reporting absence rather than failing.
    pub(super) fn try_open_dir(&self, name: Component<'_>) -> Result<Option<Self>, ArtifactError> {
        match openat(
            &self.fd,
            name.as_str(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => Ok(Some(Self {
                fd,
                path: self.path.join(name.as_str()),
            })),
            Err(Errno::NOENT) => Ok(None),
            Err(error) => Err(directory_error(&self.path.join(name.as_str()), error)),
        }
    }

    /// Creates one child directory, reporting whether this call created it.
    pub(super) fn create_dir(&self, name: Component<'_>) -> Result<bool, ArtifactError> {
        match mkdirat(&self.fd, name.as_str(), Mode::from_raw_mode(DIRECTORY_MODE)) {
            Ok(()) => Ok(true),
            Err(Errno::EXIST) => Ok(false),
            Err(error) => Err(ArtifactError::Io(error.into())),
        }
    }

    /// Creates one child directory when absent and returns its descriptor.
    pub(super) fn open_or_create_dir(&self, name: Component<'_>) -> Result<Self, ArtifactError> {
        if self.create_dir(name)? {
            self.sync()?;
        }
        self.open_dir(name)
    }

    /// Reads one child regular file, refusing any other entry type.
    pub(super) fn read_regular_file(&self, name: Component<'_>) -> Result<Vec<u8>, ArtifactError> {
        let fd = openat(
            &self.fd,
            name.as_str(),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| file_error(&self.path.join(name.as_str()), error))?;
        let stat = rustix::fs::fstat(&fd).map_err(|error| ArtifactError::Io(error.into()))?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(ArtifactError::Publication(format!(
                "published artifact entry {} is not a regular file",
                name.as_str()
            )));
        }
        let mut bytes = Vec::new();
        File::from(fd).read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// Creates and writes one child regular file that must not exist.
    ///
    /// The returned handle is not yet synchronized; the caller decides when
    /// the bytes become durable.
    pub(super) fn write_new_regular_file(
        &self,
        name: Component<'_>,
        bytes: &[u8],
    ) -> Result<File, ArtifactError> {
        let fd = openat(
            &self.fd,
            name.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(FILE_MODE),
        )
        .map_err(|error| file_error(&self.path.join(name.as_str()), error))?;
        let mut file = File::from(fd);
        file.write_all(bytes)?;
        Ok(file)
    }

    /// Lists this directory, resolving every entry type without following links.
    pub(super) fn entries(&self) -> Result<Vec<DirEntryInfo>, ArtifactError> {
        let mut listing = Vec::new();
        let mut directory =
            Dir::read_from(&self.fd).map_err(|error| ArtifactError::Io(error.into()))?;
        while let Some(entry) = directory.read() {
            let entry = entry.map_err(|error| ArtifactError::Io(error.into()))?;
            let raw = entry.file_name().to_bytes();
            if raw == b"." || raw == b".." {
                continue;
            }
            let name = std::str::from_utf8(raw)
                .map_err(|_| {
                    ArtifactError::Publication(
                        "publication parent has a non-UTF-8 entry name".into(),
                    )
                })?
                .to_owned();
            let component = Component::new(&name)?;
            let stat = statat(&self.fd, component.as_str(), AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|error| ArtifactError::Io(error.into()))?;
            listing.push(DirEntryInfo {
                name,
                file_type: FileType::from_raw_mode(stat.st_mode),
            });
        }
        listing.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(listing)
    }

    /// Synchronizes this directory's own entries.
    pub(super) fn sync(&self) -> Result<(), ArtifactError> {
        fsync(&self.fd).map_err(|error| ArtifactError::Io(error.into()))
    }

    /// Renames one child to another child of the same directory without replacing.
    pub(super) fn rename_no_replace(
        &self,
        from: Component<'_>,
        to: Component<'_>,
    ) -> Result<(), Errno> {
        renameat_with(
            &self.fd,
            from.as_str(),
            &self.fd,
            to.as_str(),
            RenameFlags::NOREPLACE,
        )
    }

    /// Removes one child regular file.
    pub(super) fn unlink_file(&self, name: Component<'_>) -> Result<(), ArtifactError> {
        unlinkat(&self.fd, name.as_str(), AtFlags::empty())
            .map_err(|error| ArtifactError::Io(error.into()))
    }

    /// Removes one empty child directory.
    pub(super) fn remove_dir(&self, name: Component<'_>) -> Result<(), ArtifactError> {
        unlinkat(&self.fd, name.as_str(), AtFlags::REMOVEDIR)
            .map_err(|error| ArtifactError::Io(error.into()))
    }

    /// Removes one child directory and its regular-file contents.
    ///
    /// Only staging directories this module created are removed, so the tree
    /// is one level of regular files; any other entry type refuses.
    pub(super) fn remove_staging_tree(&self, name: Component<'_>) -> Result<(), ArtifactError> {
        let staging = self.open_dir(name)?;
        for entry in staging.entries()? {
            if entry.file_type != FileType::RegularFile {
                return Err(ArtifactError::Publication(
                    "staging directory contains an entry that is not a regular file".into(),
                ));
            }
            staging.unlink_file(Component::new(&entry.name)?)?;
        }
        staging.sync()?;
        drop(staging);
        self.remove_dir(name)
    }
}

/// Maps a directory-open failure onto a closed publication refusal.
fn directory_error(path: &Path, error: Errno) -> ArtifactError {
    match error {
        Errno::LOOP | Errno::NOTDIR => ArtifactError::Publication(format!(
            "{} is a symbolic link or not a directory",
            path.display()
        )),
        other => ArtifactError::Io(other.into()),
    }
}

/// Maps a file-open failure onto a closed publication refusal.
fn file_error(path: &Path, error: Errno) -> ArtifactError {
    match error {
        Errno::LOOP => ArtifactError::Publication(format!("{} is a symbolic link", path.display())),
        other => ArtifactError::Io(other.into()),
    }
}
