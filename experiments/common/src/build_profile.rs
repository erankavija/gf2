//! Resolves the LTO and codegen-unit settings that the workspace manifest and
//! the environment declare for a Cargo profile, which Cargo hides from build
//! scripts. `build.rs` includes this file; the library compiles it only for its
//! tests.

use std::collections::BTreeMap;

pub const ENVIRONMENT_PREFIX: &str = "CARGO_PROFILE_";

const SETTINGS: [(&str, &str); 2] = [("lto", "LTO"), ("codegen-units", "CODEGEN_UNITS")];

#[derive(Debug, PartialEq, Eq)]
pub struct ProfileSettings {
    pub lto: String,
    pub codegen_units: Option<u32>,
    pub consulted_environment: Vec<String>,
}

/// Resolves `directory`, the name a profile's output directory carries, against
/// `manifest` and the `CARGO_PROFILE_*` entries of `environment`, which outrank
/// it.
///
/// A profile's own value outranks an inherited one at any precedence, as in
/// Cargo.
///
/// # Errors
///
/// Fails when a value has an unrecognized form, or when the settings can't
/// describe the whole executable: a package override in the inheritance chain or
/// on the `test` or `bench` profile, or LTO or codegen units set on those two,
/// which share an output directory with `dev` and `release`.
pub fn resolve(
    directory: &str,
    manifest: &toml::Table,
    environment: &BTreeMap<String, String>,
) -> Result<ProfileSettings, String> {
    let source = Source {
        manifest,
        environment,
    };
    let (name, indistinguishable) = match directory {
        "debug" => ("dev", Some("test")),
        "release" => ("release", Some("bench")),
        name => (name, None),
    };
    if let Some(sibling) = indistinguishable {
        source.refuse_package_override(sibling)?;
        for (key, _) in SETTINGS {
            if source.value(sibling, key).is_some() {
                return Err(format!(
                    "profile `{sibling}` sets `{key}` and shares its output directory with `{name}`"
                ));
            }
        }
    }

    let mut chain = vec![name.to_owned()];
    while let Some(parent) = source.parent(chain.last().expect("the chain starts non-empty")) {
        if chain.contains(&parent) {
            return Err(format!("profile `{parent}` inherits itself"));
        }
        chain.push(parent);
    }
    for profile in &chain {
        source.refuse_package_override(profile)?;
    }

    let inherited = |key| chain.iter().find_map(|profile| source.value(profile, key));
    let lto = inherited("lto").unwrap_or_else(|| "false".to_owned());
    let codegen_units = inherited("codegen-units")
        .map(|units| {
            units
                .parse()
                .map_err(|_| format!("`codegen-units` value `{units}` is not a count"))
        })
        .transpose()?;
    let consulted_environment = chain
        .iter()
        .flat_map(|profile| {
            ["LTO", "CODEGEN_UNITS", "INHERITS"]
                .map(|suffix| format!("{}{suffix}", environment_stem(profile)))
        })
        .collect();
    Ok(ProfileSettings {
        lto,
        codegen_units,
        consulted_environment,
    })
}

struct Source<'a> {
    manifest: &'a toml::Table,
    environment: &'a BTreeMap<String, String>,
}

impl Source<'_> {
    fn table(&self, profile: &str) -> Option<&toml::Table> {
        self.manifest
            .get("profile")?
            .as_table()?
            .get(profile)?
            .as_table()
    }

    fn value(&self, profile: &str, key: &str) -> Option<String> {
        let variable = format!(
            "{}{}",
            environment_stem(profile),
            key.replace('-', "_").to_uppercase()
        );
        self.environment.get(&variable).cloned().or_else(|| {
            self.table(profile)?.get(key).map(|value| match value {
                toml::Value::String(text) => text.clone(),
                other => other.to_string(),
            })
        })
    }

    fn parent(&self, profile: &str) -> Option<String> {
        self.value(profile, "inherits").or(match profile {
            "test" => Some("dev".to_owned()),
            "bench" => Some("release".to_owned()),
            _ => None,
        })
    }

    fn refuse_package_override(&self, profile: &str) -> Result<(), String> {
        let in_environment = self
            .environment
            .keys()
            .any(|name| name.starts_with(&format!("{}PACKAGE_", environment_stem(profile))));
        let in_manifest = self
            .table(profile)
            .is_some_and(|table| table.contains_key("package"));
        if in_environment || in_manifest {
            return Err(format!(
                "profile `{profile}` has a package override, so one set of flags cannot \
                 describe the executable"
            ));
        }
        Ok(())
    }
}

fn environment_stem(profile: &str) -> String {
    format!(
        "{ENVIRONMENT_PREFIX}{}_",
        profile.replace('-', "_").to_uppercase()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(text: &str) -> toml::Table {
        text.parse().unwrap()
    }

    fn no_environment() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    #[test]
    fn a_profile_inherits_unset_values_and_overrides_set_ones() {
        let manifest = layer(
            r#"
            [profile.release]
            lto = "thin"
            codegen-units = 1
            [profile.gate]
            inherits = "release"
            codegen-units = 16
            "#,
        );
        let settings = resolve("gate", &manifest, &no_environment()).unwrap();
        assert_eq!(settings.lto, "thin");
        assert_eq!(settings.codegen_units, Some(16));
    }

    #[test]
    fn the_environment_outranks_the_manifest() {
        let manifest = layer("[profile.release]\nlto = \"thin\"\ncodegen-units = 1");
        let environment = BTreeMap::from([(
            "CARGO_PROFILE_RELEASE_CODEGEN_UNITS".to_owned(),
            "3".to_owned(),
        )]);
        let settings = resolve("release", &manifest, &environment).unwrap();
        assert_eq!(settings.lto, "thin");
        assert_eq!(settings.codegen_units, Some(3));
    }

    #[test]
    fn a_child_value_outranks_an_environment_override_of_its_parent() {
        let manifest = layer("[profile.gate]\ninherits = \"release\"\nlto = false");
        let environment =
            BTreeMap::from([("CARGO_PROFILE_RELEASE_LTO".to_owned(), "fat".to_owned())]);
        let settings = resolve("gate", &manifest, &environment).unwrap();
        assert_eq!(settings.lto, "false");
    }

    #[test]
    fn unset_values_resolve_to_cargo_defaults_and_the_compiler_choice() {
        let settings = resolve("debug", &toml::Table::new(), &no_environment()).unwrap();
        assert_eq!(settings.lto, "false");
        assert_eq!(settings.codegen_units, None);
    }

    #[test]
    fn a_package_override_anywhere_in_the_chain_is_refused() {
        let inherited = layer(
            "[profile.release.package.member]\nopt-level = 1\n\
             [profile.gate]\ninherits = \"release\"",
        );
        assert!(resolve("gate", &inherited, &no_environment()).is_err());

        let manifest = layer("[profile.gate]\ninherits = \"release\"");
        let environment = BTreeMap::from([(
            "CARGO_PROFILE_RELEASE_PACKAGE_MEMBER_OPT_LEVEL".to_owned(),
            "1".to_owned(),
        )]);
        assert!(resolve("gate", &manifest, &environment).is_err());
    }

    #[test]
    fn a_package_override_on_a_directory_sibling_is_refused() {
        let bench = layer("[profile.bench.package.member]\nopt-level = 1");
        assert!(resolve("release", &bench, &no_environment()).is_err());
        let test = layer("[profile.test.package.member]\nopt-level = 1");
        assert!(resolve("debug", &test, &no_environment()).is_err());
    }

    #[test]
    fn a_build_override_describes_no_linked_crate_and_is_accepted() {
        let manifest = layer("[profile.release.build-override]\nopt-level = 0");
        assert!(resolve("release", &manifest, &no_environment()).is_ok());
    }

    #[test]
    fn a_setting_on_a_profile_sharing_the_directory_is_refused() {
        let bench = layer("[profile.bench]\nlto = \"fat\"");
        assert!(resolve("release", &bench, &no_environment()).is_err());
        let test = layer("[profile.test]\ncodegen-units = 1");
        assert!(resolve("debug", &test, &no_environment()).is_err());
    }
}
