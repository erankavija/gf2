use std::fmt;
use std::fs::File;
use std::io;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

macro_rules! canonical_token {
    ($(#[$meta:meta])* $name:ident, $kind:literal, $is_canonical:expr) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Returns the canonical text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = IdentityError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                let is_canonical: fn(&str) -> bool = $is_canonical;
                if is_canonical(value) {
                    Ok(Self(value.to_owned()))
                } else {
                    Err(IdentityError {
                        kind: $kind,
                        value: value.to_owned(),
                    })
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                String::deserialize(deserializer)?
                    .parse()
                    .map_err(serde::de::Error::custom)
            }
        }
    };
}

canonical_token!(
    /// A lowercase hexadecimal SHA-256 digest.
    Sha256Digest,
    "SHA-256 digest",
    |value| is_lowercase_hex(value, 64)
);

canonical_token!(
    /// A full 40-character lowercase git object name. An abbreviation resolves
    /// only against the repository that produced it.
    GitRevision,
    "git revision",
    |value| is_lowercase_hex(value, 40)
);

canonical_token!(
    /// A normalized repository-relative path: forward slashes, no empty, `.` or
    /// `..` component, and no absolute or drive-qualified form.
    ArtifactPath,
    "repository-relative artifact path",
    is_normalized_relative
);

/// Error returned for text outside a canonical identity grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityError {
    kind: &'static str,
    value: String,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a canonical {}", self.value, self.kind)
    }
}

impl std::error::Error for IdentityError {}

impl Sha256Digest {
    /// Digests `bytes`.
    pub fn of(bytes: &[u8]) -> Self {
        Self::from_hasher(Sha256::new_with_prefix(bytes))
    }

    /// Digests the contents of the file at `path`, streaming it.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be opened or read.
    pub fn of_file(path: &Path) -> io::Result<Self> {
        let mut hasher = Sha256::new();
        io::copy(&mut File::open(path)?, &mut hasher)?;
        Ok(Self::from_hasher(hasher))
    }

    fn from_hasher(hasher: Sha256) -> Self {
        Self(format!("{:x}", hasher.finalize()))
    }
}

/// Writes `sha256 <digest> <path>` for the running executable and for each of
/// `others`, the files it loads: the identity a harness prints beside its host
/// line for a measurement that spans benchmark windows.
///
/// # Errors
///
/// Fails when a file cannot be read or `out` refuses the write.
pub fn write_executable_digests<P: AsRef<Path>>(
    out: &mut impl io::Write,
    others: impl IntoIterator<Item = P>,
) -> io::Result<()> {
    let own = std::env::current_exe()?;
    let own_digest = Sha256Digest::of_file(Path::new("/proc/self/exe"))?;
    writeln!(out, "sha256 {own_digest} {}", own.display())?;
    for path in others {
        let path = path.as_ref();
        writeln!(
            out,
            "sha256 {} {}",
            Sha256Digest::of_file(path)?,
            path.display()
        )?;
    }
    Ok(())
}

fn is_lowercase_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_normalized_relative(value: &str) -> bool {
    let first = value.split('/').next().unwrap_or_default().as_bytes();
    let drive_qualified = first.get(1) == Some(&b':') && first[0].is_ascii_alphabetic();
    !drive_qualified
        && !value.contains('\\')
        && !value.chars().any(char::is_control)
        && value
            .split('/')
            .all(|component| !matches!(component, "" | "." | ".."))
}
