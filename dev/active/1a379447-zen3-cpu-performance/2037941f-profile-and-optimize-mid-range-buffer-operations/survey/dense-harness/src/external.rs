//! Run-time identity of the external shared object a comparator arm loaded.
//!
//! The arm links M4RI dynamically, exactly as the executed qualification
//! linked it, so the executable digest does not pin the library the timed
//! process runs: the loader resolves that at start-up from the link's
//! `RUNPATH`, which `LD_LIBRARY_PATH` overrides. The arm therefore reads its own
//! mappings, hashes the object it actually loaded, and refuses a timed run
//! whose object is not the one the qualification record pins.

use crate::campaign::{M4RI_ARM, M4RI_PORTABLE_ARM};
use crate::cells::{M4riShape, Question, PORTABLE_BUILD_RECORD, QUALIFICATION_RECORD};
use std::collections::BTreeSet;
use std::path::PathBuf;
use tuning_campaign_support::protocol::sha256_hex;

/// Targeting level of the external build a comparator arm links.
///
/// The library and the C shim of one arm share a level: `build.rs` compiles
/// the shim for the host only against the host-targeted install.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparatorBuild {
    /// The qualified install, compiled with `-march=native`.
    Host,
    /// The same source, compiler and configuration without a targeting flag.
    Portable,
}

impl ComparatorBuild {
    pub const ALL: [Self; 2] = [Self::Host, Self::Portable];

    /// The value of `GF2_M4RI_LEVEL` that selects this level in `build.rs`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Portable => "portable",
        }
    }

    /// Resolves a `GF2_M4RI_LEVEL` value.
    pub fn parse(id: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|level| level.id() == id)
            .ok_or_else(|| format!("{id:?} is no comparator build level"))
    }

    /// The plan name of the arm linked at this level.
    pub fn arm(self) -> &'static str {
        match self {
            Self::Host => M4RI_ARM,
            Self::Portable => M4RI_PORTABLE_ARM,
        }
    }

    /// The level whose arm carries the plan name `arm`.
    pub fn of_arm(arm: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.arm() == arm)
    }

    /// Repository-relative path of the committed record that pins this
    /// level's installed library digest.
    pub fn record(self) -> &'static str {
        match self {
            Self::Host => QUALIFICATION_RECORD,
            Self::Portable => PORTABLE_BUILD_RECORD,
        }
    }

    /// The build flags a plan records for this level's arm.
    pub fn flags(self) -> &'static str {
        match self {
            Self::Host => {
                "C -O3 -march=native -fPIC library build; Rust glue without a target-cpu override"
            }
            Self::Portable => {
                "C -O3 -fPIC library build; Rust glue without a target-cpu override"
            }
        }
    }

    /// The level of the comparator arm `question`'s cells run against, or
    /// `None` for a question without an external arm.
    pub fn of_question(question: Question) -> Option<Self> {
        match question {
            Question::MatvecVsM4ri | Question::NativeMatvecVsM4ri => Some(Self::Host),
            Question::PortableMatvecVsM4ri => Some(Self::Portable),
            _ => None,
        }
    }
}

/// This process's mapping table.
const SELF_MAPS: &str = "/proc/self/maps";

/// A shared object observed in this process's own mappings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryIdentity {
    /// Path the loader mapped, as the mapping table reports it.
    pub path: String,
    /// SHA-256 of that file's bytes.
    pub sha256: String,
}

impl LibraryIdentity {
    /// The `selected_path` fragment naming what the process loaded.
    pub fn provenance(&self) -> String {
        format!("loaded={}/sha256={}", self.path, self.sha256)
    }
}

/// Distinct mapped file paths whose name begins `lib<stem>.so`.
///
/// `maps` is the content of a mapping table. An anonymous mapping, a
/// pseudo-file such as `[heap]`, and a mapping whose backing file the kernel
/// marks deleted each name no readable object and are left out.
pub fn mapped_libraries(maps: &str, stem: &str) -> BTreeSet<String> {
    let wanted = format!("lib{stem}.so");
    maps.lines()
        .filter_map(mapped_path)
        .filter(|path| {
            path.starts_with('/')
                && !path.ends_with("(deleted)")
                && PathBuf::from(path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&wanted))
        })
        .map(str::to_owned)
        .collect()
}

/// The backing-file field of one mapping line: everything after its five
/// fixed fields, kept verbatim because a path may itself contain spaces.
fn mapped_path(line: &str) -> Option<&str> {
    let mut rest = line;
    for _ in 0..5 {
        rest = rest.trim_start();
        rest = &rest[rest.find(char::is_whitespace)?..];
    }
    Some(rest.trim())
}

/// Hashes the one `lib<stem>.so` this process has mapped.
///
/// Zero matches and several distinct matches are both refusals: the first says
/// the arm is not running the library it was linked against, and the second
/// says the timed process holds more than one candidate and the sample would
/// not name which executed.
pub fn loaded_library(stem: &str) -> Result<LibraryIdentity, String> {
    let maps = std::fs::read_to_string(SELF_MAPS)
        .map_err(|error| format!("cannot read {SELF_MAPS}: {error}"))?;
    let mut mapped = mapped_libraries(&maps, stem).into_iter();
    let path = mapped
        .next()
        .ok_or_else(|| format!("no lib{stem}.so is mapped into this process"))?;
    if let Some(other) = mapped.next() {
        return Err(format!(
            "this process maps more than one lib{stem}.so: {path}, {other}"
        ));
    }
    let bytes =
        std::fs::read(&path).map_err(|error| format!("cannot read the loaded {path}: {error}"))?;
    Ok(LibraryIdentity { path, sha256: sha256_hex(&bytes) })
}

/// Refuses an identity whose digest is not the one `record` pins.
pub fn verify_pinned(identity: &LibraryIdentity, pinned: &str, record: &str) -> Result<(), String> {
    if identity.sha256 == pinned {
        return Ok(());
    }
    Err(format!(
        "the loaded {} has sha256 {} rather than the {pinned} that {record} qualifies",
        identity.path, identity.sha256
    ))
}

/// The qualified library this process loaded, or the reason it is not that one.
pub fn qualified_library(stem: &str, pinned: &str, record: &str) -> Result<LibraryIdentity, String> {
    let identity = loaded_library(stem)?;
    verify_pinned(&identity, pinned, record)?;
    Ok(identity)
}

/// Route provenance of one comparator cell, including the loaded object.
pub fn comparator_selected_path(
    shape: M4riShape,
    retained: bool,
    library: &LibraryIdentity,
) -> String {
    format!(
        "m4ri/mzd_mul(y,A,x,0)/{}/rows={}/cols={}/words={}\
         /coordinates=mzd_write_bit+mzd_read_bit/{}",
        if retained { "retained-state" } else { "fresh-whole-consumer" },
        shape.rows,
        shape.cols,
        shape.stride_words(),
        library.provenance()
    )
}
