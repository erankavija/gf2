use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// Publishes `bytes` at `path`, creating missing parent directories and
/// replacing any file already there. A reader sees the previous file or the
/// complete new one, and the new file survives a crash once this returns.
///
/// # Errors
///
/// Fails when `path` has no file name or a filesystem step fails; the target is
/// then unchanged. A crash can leave a hidden `.tmp` sibling behind.
pub fn publish_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
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
    fs::create_dir_all(directory)?;
    let (temporary, mut file) = reserve_sibling(directory, name)?;
    let published = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::rename(&temporary, path));
    if published.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    published?;
    File::open(directory)?.sync_all()
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
