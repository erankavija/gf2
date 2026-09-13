//! Agreement between what a cell declares, what the harness constructs and
//! what an addendum's words say about it.
//!
//! Three registers must say the same thing about every code this survey
//! measures: the committed row registry `survey/code-rows.json`, the rows
//! this harness builds, and each frozen addendum's declared cells and family
//! description. A cell whose declared sizes reach a different code than its
//! addendum describes is a measurement-validity defect, so these checks fail
//! on the disagreement unless the survey's evidence record already carries it
//! as a recorded contradiction.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use consumer_profile_gf2_side::{constructed_code, Case, BCH_ROWS};
use serde_json::Value;

/// The issue's active directory, from the harness manifest.
fn issue_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the harness sits under the issue's survey directory")
}

fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} is committed: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("{} parses as JSON: {error}", path.display()))
}

fn code_rows() -> Value {
    read_json(&issue_dir().join("survey/code-rows.json"))
}

/// The evidence record beside the receipts, which carries every recorded
/// contradiction between a frozen addendum and the workload it measured.
fn evidence_record() -> Value {
    read_json(
        &issue_dir()
            .join("../../bench_results/04b85d10/receipt-evidence-record.json")
            .canonicalize()
            .expect("the evidence record is committed beside the receipts"),
    )
}

fn addendum_paths() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(issue_dir())
        .expect("the issue directory is readable")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let name = path.file_name()?.to_str()?.to_owned();
            (name.starts_with("addendum-") && name.ends_with(".json")).then_some(path)
        })
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "the issue declares family addenda");
    paths
}

fn usize_of(value: &Value) -> usize {
    usize::try_from(value.as_u64().expect("a declared size is a number")).expect("size fits usize")
}

fn case_of(cell: &Value) -> Case {
    let workload = &cell["workload"];
    Case {
        workload: workload["identity"]
            .as_str()
            .expect("a cell declares a workload identity")
            .to_owned(),
        size: workload["size"]
            .as_object()
            .expect("a cell declares sizes")
            .iter()
            .map(|(key, value)| {
                (
                    key.clone(),
                    value.as_u64().expect("a declared size is a number"),
                )
            })
            .collect::<BTreeMap<_, _>>(),
        seed: workload["seed"].as_u64().expect("a cell declares a seed"),
    }
}

/// The registry rows and the rows this harness builds are the same rows.
#[test]
fn the_row_registry_and_the_harness_agree() {
    let rows = code_rows();
    let registered: Vec<(usize, u64, u64)> = rows["packed_bch_mother_codes"]
        .as_array()
        .expect("the registry lists packed BCH rows")
        .iter()
        .map(|row| {
            (
                usize_of(&row["degree"]),
                row["modulus"].as_u64().expect("a row declares a modulus"),
                row["designed_distance"]
                    .as_u64()
                    .expect("a row declares a designed distance"),
            )
        })
        .collect();
    let built: Vec<(usize, u64, u64)> = BCH_ROWS
        .iter()
        .map(|row| (row.degree, row.modulus, row.designed_distance))
        .collect();
    assert_eq!(registered, built, "packed BCH rows of code-rows.json");
}

/// Every registered code's declared length is the length the harness's
/// construction reaches.
#[test]
fn registered_lengths_are_the_constructed_lengths() {
    let rows = code_rows();
    for (group, identity, key) in [
        ("packed_bch_mother_codes", "bch-encode-batch", "degree"),
        ("dvb_t2_bch_codes", "dvb-bch-encode", "length"),
        ("dvb_t2_ldpc_codes", "ldpc-syndrome", "length"),
    ] {
        for row in rows[group].as_array().expect("the registry lists rows") {
            let declared = usize_of(&row[key]);
            let size_key = if key == "degree" { "degree" } else { "n" };
            let case = Case {
                workload: identity.to_owned(),
                size: [(size_key.to_owned(), declared as u64)]
                    .into_iter()
                    .collect(),
                seed: 0,
            };
            let built = constructed_code(&case)
                .unwrap_or_else(|error| panic!("{group} row {declared}: {error}"))
                .expect("a code-bearing workload constructs a code");
            assert_eq!(
                built.length,
                usize_of(&row["length"]),
                "{group} row {declared} block length",
            );
            if size_key == "degree" {
                assert_eq!(
                    built.field_degree,
                    Some(declared),
                    "{group} row {declared} mother-field degree",
                );
            }
        }
    }
}

/// Every declared cell of every addendum reaches the code its sizes name, and
/// its constructed length is the registered one.
#[test]
fn declared_cells_reach_their_registered_code() {
    let rows = code_rows();
    let mut lengths: BTreeMap<(&str, usize), usize> = BTreeMap::new();
    for (group, kind, key) in [
        ("packed_bch_mother_codes", "packed-bch", "degree"),
        ("dvb_t2_bch_codes", "dvb-bch", "length"),
        ("dvb_t2_ldpc_codes", "ldpc", "length"),
    ] {
        for row in rows[group].as_array().expect("the registry lists rows") {
            lengths.insert((kind, usize_of(&row[key])), usize_of(&row["length"]));
        }
    }
    for path in addendum_paths() {
        let addendum = read_json(&path);
        for cell in addendum["cells"].as_array().expect("an addendum has cells") {
            let case = case_of(cell);
            let built = constructed_code(&case).unwrap_or_else(|error| {
                panic!("{} cell {}: {error}", path.display(), cell["cell_id"])
            });
            let Some(built) = built else { continue };
            let (kind, declared) = match case.workload.as_str() {
                "dvb-bch-encode" => ("dvb-bch", case.size["n"]),
                "ldpc-syndrome" | "ldpc-codeword-check" => ("ldpc", case.size["n"]),
                _ => ("packed-bch", case.size["degree"]),
            };
            let registered = lengths
                .get(&(kind, usize::try_from(declared).expect("size fits usize")))
                .unwrap_or_else(|| {
                    panic!(
                        "{} cell {} declares {kind} {declared}, which the registry does not carry",
                        path.display(),
                        cell["cell_id"]
                    )
                });
            assert_eq!(
                built.length,
                *registered,
                "{} cell {} block length",
                path.display(),
                cell["cell_id"],
            );
        }
    }
}

/// Every profile case list declares sizes that reach a registered code, so a
/// counter, call-graph or allocation-trace case cannot name one code and
/// measure another.
#[test]
fn profile_cases_reach_their_registered_code() {
    let rows = code_rows();
    let mut lengths: BTreeMap<(&str, usize), usize> = BTreeMap::new();
    for (group, kind, key) in [
        ("packed_bch_mother_codes", "packed-bch", "degree"),
        ("dvb_t2_bch_codes", "dvb-bch", "length"),
        ("dvb_t2_ldpc_codes", "ldpc", "length"),
    ] {
        for row in rows[group].as_array().expect("the registry lists rows") {
            lengths.insert((kind, usize_of(&row[key])), usize_of(&row["length"]));
        }
    }
    let survey = issue_dir().join("survey");
    for name in [
        "counter-cases.txt",
        "report-cases.txt",
        "alloc-trace-cases.txt",
    ] {
        let path = survey.join(name);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{} is committed: {error}", path.display()));
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut fields = line.split('|');
            let label = fields.next().expect("a case row carries a label");
            let payload = fields
                .next()
                .unwrap_or_else(|| panic!("{name} row {label} has a case"));
            let case: Case = serde_json::from_str(payload)
                .unwrap_or_else(|error| panic!("{name} row {label} case parses: {error}"));
            let built = constructed_code(&case)
                .unwrap_or_else(|error| panic!("{name} row {label}: {error}"));
            let Some(built) = built else { continue };
            let (kind, declared) = match case.workload.as_str() {
                "dvb-bch-encode" => ("dvb-bch", case.size["n"]),
                "ldpc-syndrome" | "ldpc-codeword-check" => ("ldpc", case.size["n"]),
                _ => ("packed-bch", case.size["degree"]),
            };
            let registered = lengths
                .get(&(kind, usize::try_from(declared).expect("size fits usize")))
                .unwrap_or_else(|| {
                    panic!("{name} row {label} declares {kind} {declared}, unregistered")
                });
            assert_eq!(built.length, *registered, "{name} row {label} block length");
        }
    }
}

/// The code an addendum's family description names is the code its own cells
/// reach, unless the evidence record carries the disagreement as a recorded
/// contradiction.
#[test]
fn family_descriptions_name_the_codes_their_cells_reach() {
    let rows = code_rows();
    let mut label_of_degree: BTreeMap<usize, String> = BTreeMap::new();
    for row in rows["packed_bch_mother_codes"]
        .as_array()
        .expect("the registry lists packed BCH rows")
    {
        label_of_degree.insert(
            usize_of(&row["degree"]),
            row["label"]
                .as_str()
                .expect("a row carries a label")
                .to_owned(),
        );
    }
    let record = evidence_record();
    let recorded: BTreeSet<String> = record["contradictions"]
        .as_array()
        .expect("the evidence record lists contradictions")
        .iter()
        .flat_map(|entry| {
            entry["declared_in"]
                .as_array()
                .expect("a contradiction names the artifacts that declare it")
                .iter()
                .map(|path| {
                    path.as_str()
                        .expect("a declaring artifact is a path")
                        .to_owned()
                })
        })
        .collect();

    for path in addendum_paths() {
        let addendum = read_json(&path);
        let relative = format!(
            "dev/active/04b85d10/{}",
            path.file_name().expect("a file name").to_string_lossy()
        );
        let mut reached: BTreeSet<&String> = BTreeSet::new();
        for cell in addendum["cells"].as_array().expect("an addendum has cells") {
            let case = case_of(cell);
            if !case.workload.starts_with("bch-encode-batch") {
                continue;
            }
            let degree =
                usize::try_from(case.size["degree"]).expect("a declared degree fits usize");
            reached.insert(
                label_of_degree
                    .get(&degree)
                    .unwrap_or_else(|| panic!("{relative} declares unregistered degree {degree}")),
            );
        }
        if reached.is_empty() {
            continue;
        }
        let description = addendum["family"]["description"]
            .as_str()
            .expect("a family declares a description");
        let named: BTreeSet<&String> = label_of_degree
            .values()
            .filter(|label| description.contains(label.as_str()))
            .collect();
        if named == reached {
            continue;
        }
        assert!(
            recorded.contains(&relative),
            "{relative} names {named:?} where its packed BCH cells reach {reached:?}, \
             and the evidence record carries no contradiction for it",
        );
    }
}
