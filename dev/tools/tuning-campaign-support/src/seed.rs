//! Deterministic fixture seed derivation.

/// Root for the extent fixture streams.
pub const EXTENT_SEED_ROOT: u64 = 0x5ecc_9bf8_0000_0000;
/// Stable name of the [`fixture_seed`] mixer.
pub const SEED_DERIVATION: &str = "gf2-calibration-seed-v1";

/// Adds a fixture bank to a role in the protocol's reserved high bits.
pub const fn bank_role(role: u64, bank: usize) -> u64 {
    role.wrapping_add((bank as u64) << 16)
}

/// Mixes a root, field tag, shape key, and role exactly as preregistered.
pub const fn fixture_seed(seed_root: u64, field_tag: u64, shape_key: u64, role: u64) -> u64 {
    let mut value = seed_root ^ role.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let words = [field_tag, shape_key];
    let mut index = 0;
    while index < words.len() {
        value ^= words[index];
        value = value
            .wrapping_mul(0xbf58_476d_1ce4_e5b9)
            .rotate_left(27)
            .wrapping_add(0x94d0_49bb_1331_11eb);
        index += 1;
    }
    value ^ (value >> 31)
}

/// The generator a producer draws its fixtures from, as its executable
/// states it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureGenerator {
    /// Type path of the generator, as the compiler names it.
    pub implementation: String,
    /// The packages that define its stream, each as `name version` and
    /// joined by `, `, at the versions the build's lock file resolves.
    pub version: String,
}

impl FixtureGenerator {
    /// Names the generator type `G` and resolves, in the `Cargo.lock` text
    /// `lock` the caller embeds at build, the version of each package of
    /// `packages`: a chain of dependency names leading from the package
    /// `consumer`, the empty chain being `consumer` itself.
    pub fn locked<G>(lock: &str, consumer: &str, packages: &[&[&str]]) -> std::io::Result<Self> {
        Ok(Self {
            implementation: std::any::type_name::<G>().to_owned(),
            version: locked_versions(lock, consumer, packages).map_err(std::io::Error::other)?,
        })
    }
}

/// One `[[package]]` entry of a lock file: version and dependency entries.
type LockedPackage<'a> = (&'a str, Vec<&'a str>);

/// Resolves each dependency chain of `packages` from `consumer` in the lock
/// file text `lock` and returns `name version` of the reached packages,
/// joined by `, `.
fn locked_versions(lock: &str, consumer: &str, packages: &[&[&str]]) -> Result<String, String> {
    fn quoted(line: &str) -> Option<&str> {
        line.split('"').nth(1)
    }
    let mut entries: Vec<(&str, LockedPackage)> = Vec::new();
    for block in lock.split("[[package]]").skip(1) {
        let field = |key: &str| {
            block
                .lines()
                .find(|line| line.starts_with(key))
                .and_then(quoted)
                .ok_or_else(|| format!("a lock entry has no {key}"))
        };
        let dependencies = block
            .split_once("dependencies = [")
            .map(|(_, list)| list.split(']').next().unwrap_or_default())
            .unwrap_or_default()
            .lines()
            .filter_map(quoted)
            .collect();
        entries.push((field("name = ")?, (field("version = ")?, dependencies)));
    }
    // A dependency entry is `name`, or `name version` when the lock holds
    // several versions of the name, optionally followed by its source.
    let find = |name: &str, version: Option<&str>| {
        let mut found = entries.iter().filter(|(candidate, (locked, _))| {
            *candidate == name && version.is_none_or(|version| version == *locked)
        });
        match (found.next(), found.next()) {
            (Some(entry), None) => Ok(entry),
            _ => Err(format!("the lock file does not hold exactly one {name}")),
        }
    };
    let mut resolved = Vec::new();
    for chain in packages {
        let mut entry = find(consumer, None)?;
        for name in *chain {
            let (_, (_, dependencies)) = entry;
            let mut edge = dependencies
                .iter()
                .map(|dependency| dependency.split(' '))
                .filter(|parts| parts.clone().next() == Some(*name));
            let (Some(mut parts), None) = (edge.next(), edge.next()) else {
                return Err(format!("{} does not depend on exactly one {name}", entry.0));
            };
            entry = find(parts.next().unwrap_or_default(), parts.next())?;
        }
        resolved.push(format!("{} {}", entry.0, entry.1 .0));
    }
    Ok(resolved.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = r#"version = 4

[[package]]
name = "consumer"
version = "0.1.0"
dependencies = [
 "leaf",
 "stream 0.8.5",
]

[[package]]
name = "leaf"
version = "0.4.3"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "stream"
version = "0.8.5"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = [
 "core 0.3.1",
]

[[package]]
name = "stream"
version = "0.9.2"
dependencies = [
 "core 0.9.0",
]

[[package]]
name = "core"
version = "0.3.1"

[[package]]
name = "core"
version = "0.9.0"
"#;

    #[test]
    fn versions_follow_the_dependency_edges_of_the_consumer() {
        assert_eq!(
            locked_versions(
                LOCK,
                "consumer",
                &[&[], &["stream"], &["stream", "core"], &["leaf"]]
            ),
            Ok("consumer 0.1.0, stream 0.8.5, core 0.3.1, leaf 0.4.3".to_owned())
        );
    }

    #[test]
    fn a_package_the_consumer_does_not_reach_is_refused() {
        for chain in [&["core"][..], &["leaf", "core"], &["absent"]] {
            assert!(locked_versions(LOCK, "consumer", &[chain]).is_err());
        }
        assert!(locked_versions(LOCK, "stream", &[&[]]).is_err());
    }

    #[test]
    fn a_generator_is_named_by_its_type_path_and_locked_versions() {
        struct Marker;
        let generator =
            FixtureGenerator::locked::<Marker>(LOCK, "consumer", &[&["stream"]]).unwrap();
        assert!(generator.implementation.ends_with("Marker"));
        assert_eq!(generator.version, "stream 0.8.5");
        assert!(FixtureGenerator::locked::<Marker>(LOCK, "consumer", &[&["absent"]]).is_err());
    }
}
