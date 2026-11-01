//! Append-only family attempt accounting for versioned protocol families.
//! Reservations precede measurement and never disappear after a failure. Each
//! exact JSON line binds its predecessor. A receipt freezes the entire prefix,
//! including its reservation; no supplied prior-trial count decides correction.
use crate::protocol::{
    sha256_hex, ArtifactPin, CellRole, FamilyAddendum, SharedSettings, PROTOCOL_VERSION,
};
use crate::receipt::tail_supported;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::{self, Write},
    path::Path,
};

/// One durable reservation. Completion, failure and interruption all spend the
/// same predeclared comparisons; the campaign ID locates its execution record.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reservation {
    pub sequence: u64,
    pub predecessor: String,
    pub family: String,
    pub campaign: String,
    pub addendum_sha256: String,
    pub comparisons: u32,
    pub protocol_version: u32,
    pub candidates: Vec<String>,
}

fn invalid(message: impl ToString) -> io::Error {
    io::Error::other(message.to_string())
}

/// Validates every link and returns reservations in acquisition order.
/// Missing, reordered, repeated or changed entries fail, including a removed
/// failed attempt. An empty ledger is the explicit genesis state.
pub fn decode(bytes: &[u8], family: &str) -> io::Result<Vec<Reservation>> {
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        return Err(invalid("partial ledger entry"));
    }
    let mut predecessor = "0".repeat(64);
    let mut entries = Vec::new();
    let mut campaigns = BTreeSet::new();
    let mut attempted = BTreeSet::new();
    for line in bytes.split_inclusive(|b| *b == b'\n') {
        let entry: Reservation = serde_json::from_slice(line).map_err(invalid)?;
        if entry.sequence != entries.len() as u64
            || entry.predecessor != predecessor
            || entry.family != family
            || !campaigns.insert(entry.campaign.clone())
            || !crate::protocol::is_hex(&entry.addendum_sha256, 64)
        {
            return Err(invalid("family ledger chain is incomplete or inconsistent"));
        }
        if entry.candidates.windows(2).any(|w| w[0] >= w[1])
            || (entry.comparisons == 0) != entry.candidates.is_empty()
            || entry.candidates.iter().any(|id| {
                !crate::protocol::is_hex(id, 64)
                    || !attempted.insert((entry.protocol_version, id.clone()))
            })
        {
            return Err(invalid(
                "candidate attempt limit exceeded or candidate identities invalid",
            ));
        }
        predecessor = sha256_hex(line);
        entries.push(entry);
    }
    Ok(entries)
}

/// Counts reserved confirmatory and holdout comparisons, rejecting overflow.
pub fn comparisons(entries: &[Reservation]) -> io::Result<u32> {
    entries
        .iter()
        .try_fold(0u32, |n, e| {
            n.checked_add(e.comparisons)
                .ok_or_else(|| invalid("family comparison count overflow"))
        })
        .map(|n| n.max(1))
}

/// Reserves this campaign under the caller's exclusive timing lock and records
/// the ledger prefix digest. A start interrupted before its execution log opens
/// reuses its reservation; a campaign with a log uses [`resume_reservation`].
/// The family ledger must already exist (even at genesis); a missing ledger is
/// an error so deleting a family history cannot silently restart accounting.
pub fn reserve(
    root: &Path,
    family: &FamilyAddendum,
    campaign: &str,
    addendum_sha: &str,
    candidates: &[String],
) -> io::Result<ArtifactPin> {
    let relative = family
        .family_wise
        .ledger_path
        .as_deref()
        .ok_or_else(|| invalid("missing family ledger"))?;
    let path = root.join(relative);
    let mut file = fs::OpenOptions::new().read(true).append(true).open(&path)?;
    file.lock()?;
    let bytes = fs::read(&path)?;
    let entries = decode(&bytes, &family.family.id)?;
    let count = reserved_comparisons(family);
    let prefix = if let Some(entry) = entries.iter().find(|e| e.campaign == campaign) {
        reservation_prefix(&bytes, entry, family, addendum_sha, candidates)?
    } else {
        if entries.iter().any(|entry| {
            entry.protocol_version == family.protocol.version
                && entry.candidates.iter().any(|id| candidates.contains(id))
        }) {
            return Err(invalid(
                "candidate attempt limit exceeded; failed attempts remain spent",
            ));
        }
        if candidates.windows(2).any(|w| w[0] >= w[1]) || (count == 0) != candidates.is_empty() {
            return Err(invalid(
                "candidate identities must be unique, ordered and match the role",
            ));
        }
        let entry = Reservation {
            protocol_version: family.protocol.version,
            candidates: candidates.to_vec(),
            sequence: entries.len() as u64,
            predecessor: bytes
                .split_inclusive(|b| *b == b'\n')
                .next_back()
                .map(sha256_hex)
                .unwrap_or_else(|| "0".repeat(64)),
            family: family.family.id.clone(),
            campaign: campaign.into(),
            addendum_sha256: addendum_sha.into(),
            comparisons: count,
        };
        let mut line = serde_json::to_vec(&entry)?;
        line.push(b'\n');
        file.write_all(&line)?;
        file.sync_all()?;
        let mut prefix = bytes;
        prefix.extend(line);
        prefix
    };
    file.unlock()?;
    Ok(ArtifactPin {
        path: relative.to_owned(),
        sha256: sha256_hex(&prefix),
    })
}

/// The pin of a resuming campaign's reservation, read without appending: a
/// ledger without it refuses, so a lost history spends no second attempt.
pub fn resume_reservation(
    root: &Path,
    family: &FamilyAddendum,
    campaign: &str,
    addendum_sha: &str,
    candidates: &[String],
) -> io::Result<ArtifactPin> {
    let relative = family
        .family_wise
        .ledger_path
        .as_deref()
        .ok_or_else(|| invalid("missing family ledger"))?;
    let bytes = fs::read(root.join(relative))?;
    let entries = decode(&bytes, &family.family.id)?;
    let entry = entries
        .iter()
        .find(|e| e.campaign == campaign)
        .ok_or_else(|| invalid("authoritative ledger lost this campaign"))?;
    Ok(ArtifactPin {
        path: relative.to_owned(),
        sha256: sha256_hex(&reservation_prefix(
            &bytes,
            entry,
            family,
            addendum_sha,
            candidates,
        )?),
    })
}

/// The ledger bytes through `entry`, once `entry` matches the frozen addendum.
fn reservation_prefix(
    bytes: &[u8],
    entry: &Reservation,
    family: &FamilyAddendum,
    addendum_sha: &str,
    candidates: &[String],
) -> io::Result<Vec<u8>> {
    if entry.addendum_sha256 != addendum_sha
        || entry.comparisons != reserved_comparisons(family)
        || entry.protocol_version != family.protocol.version
        || entry.candidates != candidates
    {
        return Err(invalid("campaign reservation differs from frozen addendum"));
    }
    Ok(bytes
        .split_inclusive(|b| *b == b'\n')
        .take(entry.sequence as usize + 1)
        .flatten()
        .copied()
        .collect())
}

fn reserved_comparisons(family: &FamilyAddendum) -> u32 {
    family
        .cells
        .iter()
        .filter(|c| c.role != CellRole::Exploratory)
        .count() as u32
}

/// Reads the digest-pinned prefix from the authoritative append-only ledger.
/// Later reservations do not change an earlier campaign's input identity.
pub fn read_prefix(pin: &ArtifactPin, root: &Path) -> io::Result<Vec<u8>> {
    let bytes = pin.read_pinned(root).map_err(invalid)?;
    let mut end = 0;
    for line in bytes.split_inclusive(|b| *b == b'\n') {
        end += line.len();
        if sha256_hex(&bytes[..end]) == pin.sha256 {
            return Ok(bytes[..end].to_vec());
        }
    }
    Err(invalid(
        "authoritative ledger lacks the recorded prefix digest",
    ))
}

/// Verifies a receipt's immutable prefix, binding its terminal reservation to
/// the declared cells and deriving the full family correction from the chain.
pub fn verify(
    pin: &ArtifactPin,
    dir: &Path,
    family: &FamilyAddendum,
    campaign: &str,
    addendum_sha: &str,
    candidates: &[String],
) -> io::Result<u32> {
    if family.family_wise.ledger_path.as_deref() != Some(&pin.path) {
        return Err(invalid("ledger path differs from family declaration"));
    }
    let bytes = read_prefix(pin, dir)?;
    let entries = decode(&bytes, &family.family.id)?;
    let last = entries
        .last()
        .ok_or_else(|| invalid("ledger lacks this reservation"))?;
    let count = reserved_comparisons(family);
    if last.campaign != campaign
        || last.addendum_sha256 != addendum_sha
        || last.comparisons != count
        || last.protocol_version != family.protocol.version
        || last.candidates != candidates
    {
        return Err(invalid("terminal ledger reservation differs from campaign"));
    }
    comparisons(&entries)
}

/// Sequential attempt budget from the frozen chain. Attempt t spends at most
/// alpha / (t(t+1)); summing over any number of attempts is bounded by alpha.
/// Exploratory reservations spend no budget. This avoids repeated alpha/m
/// decisions silently exhausting the family's total type-I error budget.
pub fn attempt_alpha(pin: &ArtifactPin, dir: &Path, family: &FamilyAddendum) -> io::Result<f64> {
    let bytes = read_prefix(pin, dir)?;
    let entries = decode(&bytes, &family.family.id)?;
    Ok(attempt_budget(&entries, family.family_wise.alpha))
}

fn attempt_budget(entries: &[Reservation], alpha: f64) -> f64 {
    let attempts = entries.iter().filter(|e| e.comparisons > 0).count().max(1) as f64;
    alpha / (attempts * (attempts + 1.0))
}

/// The next confirmatory attempt on a family ledger.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NextAttempt {
    /// Reservations the ledger holds.
    pub reservations: usize,
    /// Those that spend comparisons.
    pub spending_reservations: usize,
    /// Confirmatory and holdout comparisons those reserved.
    pub prior_comparisons: u32,
    /// The attempt's sequence number among spending reservations.
    pub attempt: usize,
    /// The attempt's share of the family alpha.
    pub attempt_alpha: f64,
    /// Largest confirmatory and holdout cell count whose corrected alpha keeps
    /// P-20's tail support; zero when none does.
    pub max_cells: u32,
}

/// Evaluates one more reservation on the ledger `bytes` as acceptance counts
/// it: an attempt of its own in [`attempt_alpha`], whose cells join every
/// comparison the chain already holds in the Bonferroni divisor of [`verify`].
pub fn next_attempt(
    bytes: &[u8],
    family: &str,
    settings: &SharedSettings,
) -> io::Result<NextAttempt> {
    let entries = decode(bytes, family)?;
    let with = |cells: u32| {
        let mut chain = entries.clone();
        chain.push(Reservation {
            sequence: entries.len() as u64,
            predecessor: String::new(),
            family: family.to_owned(),
            campaign: String::new(),
            addendum_sha256: String::new(),
            comparisons: cells,
            protocol_version: PROTOCOL_VERSION,
            candidates: Vec::new(),
        });
        chain
    };
    let attempt_alpha = attempt_budget(&with(1), settings.family_alpha);
    let mut max_cells = 0;
    while tail_supported(
        settings.bootstrap_resamples,
        attempt_alpha / f64::from(comparisons(&with(max_cells + 1))?),
    ) {
        max_cells += 1;
    }
    let spending_reservations = entries.iter().filter(|e| e.comparisons > 0).count();
    Ok(NextAttempt {
        reservations: entries.len(),
        spending_reservations,
        prior_comparisons: entries
            .iter()
            .try_fold(0u32, |n, e| n.checked_add(e.comparisons))
            .ok_or_else(|| invalid("family comparison count overflow"))?,
        attempt: spending_reservations + 1,
        attempt_alpha,
        max_cells,
    })
}

/// Canonical attempted candidate identities from the frozen plan and arm
/// records. Navigation paths, arm names and prose descriptions do not create
/// another candidate; executable bytes, launch/build settings and profiles do.
pub fn candidate_ids(
    family: &FamilyAddendum,
    plan: &crate::protocol::RunnerPlan,
    arms: &std::collections::BTreeMap<String, crate::receipt::ArmRecord>,
) -> io::Result<Vec<String>> {
    let mut ids = BTreeSet::new();
    for cell in &plan.cells {
        if family
            .cell(&cell.cell_id)
            .is_some_and(|c| c.role != CellRole::Exploratory)
        {
            let arm = arms
                .get(&cell.candidate_arm)
                .ok_or_else(|| invalid("candidate lacks arm record"))?;
            let mut identity = serde_json::to_value(arm)?;
            let object = identity.as_object_mut().unwrap();
            object.remove("description");
            object.remove("executable_path");
            ids.insert(sha256_hex(&serde_json::to_vec(&identity)?));
        }
    }
    Ok(ids.into_iter().collect())
}
