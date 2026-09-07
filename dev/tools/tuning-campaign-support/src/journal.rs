//! Durable append-only campaign logging and immutable checkpoints.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Journal schema identity fixed by the a835 protocol.
pub const JOURNAL_SCHEMA: &str = "tuning-campaign-journal-v1";
/// Resume-manifest schema identity fixed by the a835 protocol.
pub const CHECKPOINT_SCHEMA: &str = "tuning-campaign-checkpoint-v1";
const RECOVERY_SCHEMA: &str = "tuning-campaign-log-recovery-v1";
const UNIT_SCHEMA: &str = "tuning-campaign-checkpoint-unit-v1";
const PENDING_RECOVERY_SCHEMA: &str = "tuning-campaign-pending-recovery-v1";
const EXECUTION_LOG_NAME: &str = "execution.log";
const PENDING_RECOVERY_NAME: &str = "pending-recovery.json";
const PENDING_RECOVERY_STAGING_NAME: &str = ".pending-recovery.intent";

/// Closed vocabulary for campaign progress records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum JournalEvent {
    CampaignStart,
    SessionStart,
    SessionPrepared,
    SessionRecovery,
    WorkFinished,
    WrapperReturned,
    ReleaseUnobserved,
    ResultValidated,
    PendingRecovery,
    PhaseStart,
    PhaseComplete,
    CellStart,
    CellComplete,
    ExecutionProgress,
    WindowProgress,
    ChildSpawn,
    ChildDiagnostic,
    ChildExit,
    ChildTimeout,
    /// Untimed owner/driver operation; separate from measurement progress.
    OrchestrationStart,
    /// Synced termination evidence for an untimed orchestration operation.
    OrchestrationExit,
    /// Driver diagnostics whose schema is independent of child stderr lines.
    DriverDiagnostic,
    /// Synced typed child-completion evidence with outcome and raw artifacts.
    RawStreams,
    CheckpointAccepted,
    Omission,
    Fallback,
    OwnerWrite,
    OwnerReopen,
    Composition,
    CompositionReopen,
    LockHold,
    LockRelease,
    Recovery,
    Interrupted,
    Complete,
    Failed,
    Paused,
    BudgetExhausted,
}

impl JournalEvent {
    fn requires_sync(self) -> bool {
        matches!(
            self,
            Self::SessionStart
                | Self::SessionPrepared
                | Self::SessionRecovery
                | Self::OrchestrationExit
                | Self::RawStreams
                | Self::ChildTimeout
                | Self::ChildExit
                | Self::WorkFinished
                | Self::WrapperReturned
                | Self::LockHold
                | Self::LockRelease
                | Self::ReleaseUnobserved
                | Self::ResultValidated
                | Self::PendingRecovery
                | Self::CheckpointAccepted
                | Self::Recovery
                | Self::Interrupted
                | Self::Complete
                | Self::Failed
                | Self::Paused
                | Self::BudgetExhausted
        )
    }

    fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Complete | Self::Failed | Self::Paused | Self::BudgetExhausted
        )
    }
}

/// Terminal outcome written before a campaign process exits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalState {
    Complete,
    Failed,
    Paused,
    BudgetExhausted,
}

impl From<TerminalState> for JournalEvent {
    fn from(value: TerminalState) -> Self {
        match value {
            TerminalState::Complete => Self::Complete,
            TerminalState::Failed => Self::Failed,
            TerminalState::Paused => Self::Paused,
            TerminalState::BudgetExhausted => Self::BudgetExhausted,
        }
    }
}

/// One canonical JSON Lines journal record.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct JournalRecord {
    pub schema: String,
    pub timestamp_utc: String,
    pub campaign_id: String,
    pub session_id: String,
    pub sequence: u64,
    pub event: JournalEvent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case: Option<Value>,
    pub details: Value,
}

/// Single-writer append-only execution journal.
pub struct ExecutionLog {
    path: PathBuf,
    file: File,
    campaign_id: String,
    session_id: String,
    next_sequence: u64,
    campaign_started: bool,
    terminal: bool,
}

impl ExecutionLog {
    /// Idempotently opens a preparation handoff from an already closed prior
    /// session. Unlike ordinary resume, this never supplies a missing terminal.
    pub fn resume_prepared(
        path: impl AsRef<Path>,
        campaign_id: &str,
        session_id: &str,
        prior_session: &str,
    ) -> io::Result<Self> {
        let path = fs::canonicalize(path)?;
        if let Some((intent_path, intent)) = find_incomplete_recovery(&path)? {
            if intent.mode == RecoveryMode::Resume
                && intent.campaign_id == campaign_id
                && intent.prior_session_id == prior_session
                && intent.resume_session_id == session_id
            {
                return continue_log_recovery(&path, &intent_path, intent);
            }
            if intent.mode == RecoveryMode::RepairActive && intent.prior_session_id == session_id {
                return Self::repair_active(&path, campaign_id, session_id);
            }
            return Err(invalid(
                "preparation has a different pending journal recovery",
            ));
        }
        let data = fs::read(&path)?;
        let complete = data
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |index| index + 1);
        let text = std::str::from_utf8(&data[..complete])
            .map_err(|_| invalid("preparation journal prefix is not UTF-8"))?;
        let replay = replay_journal(text, campaign_id)?;
        if replay.current_session == session_id {
            return Self::repair_active(path, campaign_id, session_id);
        }
        if replay.current_session != prior_session
            || !matches!(
                replay.terminal,
                Some(
                    JournalEvent::Paused
                        | JournalEvent::BudgetExhausted
                        | JournalEvent::Interrupted
                )
            )
        {
            return Err(invalid(
                "preparation handoff lacks the exact closed prior session",
            ));
        }
        Self::resume(path, campaign_id, session_id)
    }
    /// Creates `<canonical-stage>/execution.log` without replacing any file.
    pub fn create_new(
        stage: impl AsRef<Path>,
        campaign_id: impl Into<String>,
        session_id: impl Into<String>,
    ) -> io::Result<Self> {
        let campaign_id = checked_identity(campaign_id.into(), "campaign")?;
        let session_id = checked_identity(session_id.into(), "session")?;
        let stage = fs::canonicalize(stage.as_ref())?;
        let path = stage.join(EXECUTION_LOG_NAME);
        let file = OpenOptions::new()
            .append(true)
            .create_new(true)
            .open(&path)?;
        file.sync_all()?;
        File::open(&stage)?.sync_all()?;
        let path = fs::canonicalize(path)?;
        Ok(Self {
            path,
            file,
            campaign_id,
            session_id,
            next_sequence: 0,
            campaign_started: false,
            terminal: false,
        })
    }

    /// Reopens and validates an existing journal for a new append-only session.
    ///
    /// `paused`, `budget-exhausted`, and `interrupted` sessions are resumable.
    /// An otherwise unterminated session receives one synced `interrupted`
    /// record before the new synced `session-start`. A torn final append first
    /// creates and syncs byte-preserving diagnostic and recovery-intent files,
    /// truncates and syncs only the incomplete suffix, then idempotently writes
    /// the recovery/session boundaries and removes the completed intent with a
    /// parent-directory sync.
    ///
    /// # Errors
    ///
    /// Rejects complete or failed campaigns, reused or mismatched session
    /// identities, malformed or noncanonical complete records, invalid global
    /// sequences or lifecycle transitions, altered recovery artifacts, and a
    /// log that matches neither a recovery intent's original nor recovered
    /// prefix state. I/O and durability failures are returned unchanged.
    pub fn resume(
        path: impl AsRef<Path>,
        campaign_id: impl Into<String>,
        session_id: impl Into<String>,
    ) -> io::Result<Self> {
        let campaign_id = checked_identity(campaign_id.into(), "campaign")?;
        let session_id = checked_identity(session_id.into(), "session")?;
        let path = fs::canonicalize(path.as_ref())?;
        if let Some((intent_path, intent)) = find_incomplete_recovery(&path)? {
            if intent.campaign_id != campaign_id
                || intent.resume_session_id != session_id
                || intent.mode != RecoveryMode::Resume
            {
                return Err(invalid(
                    "incomplete log recovery belongs to a different campaign or resume session",
                ));
            }
            return continue_log_recovery(&path, &intent_path, intent);
        }
        let bytes = fs::read(&path)?;
        if let Some((intent_path, intent)) = begin_log_recovery(
            &path,
            &bytes,
            &campaign_id,
            &session_id,
            RecoveryMode::Resume,
        )? {
            return continue_log_recovery(&path, &intent_path, intent);
        }
        let contents = String::from_utf8(bytes)
            .map_err(|_| invalid("newline-terminated execution log is not UTF-8"))?;
        let replay = replay_journal(&contents, &campaign_id)?;
        if matches!(
            replay.terminal,
            Some(JournalEvent::Complete | JournalEvent::Failed)
        ) {
            return Err(invalid("a complete or failed campaign cannot resume"));
        }
        if replay.sessions.contains(&session_id) {
            return Err(invalid("resume session identity was already used"));
        }
        let file = OpenOptions::new().append(true).open(&path)?;
        let prior_session = replay.current_session;
        let mut log = Self {
            path,
            file,
            campaign_id,
            session_id: prior_session.clone(),
            next_sequence: replay.next_sequence,
            campaign_started: true,
            terminal: false,
        };
        if replay.terminal.is_none() {
            log.append_inner(
                JournalEvent::Interrupted,
                None,
                serde_json::json!({
                    "session": prior_session.clone(),
                    "cause": "missing-terminal-record",
                }),
            )?;
        }
        log.session_id = session_id;
        log.append_inner(
            JournalEvent::SessionStart,
            None,
            serde_json::json!({"resumed_from": prior_session}),
        )?;
        Ok(log)
    }

    /// Repairs an active session's torn final append without changing sessions.
    ///
    /// The caller holds the campaign sole-writer guard. Exact suffix bytes and
    /// their original/prefix hashes are synced before truncation. Repair adds
    /// only a synced Recovery record: it never invents release, interruption,
    /// or a replacement session. The campaign layer obtains independent release
    /// proof, projects LockRelease and Interrupted, then retires the old claim.
    pub fn repair_active(
        path: impl AsRef<Path>,
        campaign_id: impl Into<String>,
        session_id: impl Into<String>,
    ) -> io::Result<Self> {
        let campaign_id = checked_identity(campaign_id.into(), "campaign")?;
        let session_id = checked_identity(session_id.into(), "session")?;
        let path = fs::canonicalize(path)?;
        if let Some((intent_path, intent)) = find_incomplete_recovery(&path)? {
            if intent.mode != RecoveryMode::RepairActive
                || intent.campaign_id != campaign_id
                || intent.prior_session_id != session_id
                || intent.resume_session_id != session_id
            {
                return Err(invalid(
                    "active recovery intent belongs to another session or mode",
                ));
            }
            return continue_log_recovery(&path, &intent_path, intent);
        }
        let bytes = fs::read(&path)?;
        if let Some((intent_path, intent)) = begin_log_recovery(
            &path,
            &bytes,
            &campaign_id,
            &session_id,
            RecoveryMode::RepairActive,
        )? {
            return continue_log_recovery(&path, &intent_path, intent);
        }
        Self::reopen_active(path, campaign_id, session_id)
    }

    /// Reopens the exact current session between sequential driver modes.
    ///
    /// The caller must hold the campaign layer's sole-writer guard. This does
    /// not create a resume/session boundary or infer interruption/release.
    /// Complete canonical records are mandatory; torn-tail repair belongs to
    /// explicit recovery after the campaign layer establishes release proof.
    pub fn reopen_active(
        path: impl AsRef<Path>,
        campaign_id: impl Into<String>,
        session_id: impl Into<String>,
    ) -> io::Result<Self> {
        let campaign_id = checked_identity(campaign_id.into(), "campaign")?;
        let session_id = checked_identity(session_id.into(), "session")?;
        let path = fs::canonicalize(path)?;
        let bytes = fs::read(&path)?;
        let contents = std::str::from_utf8(&bytes).map_err(|_| invalid("journal is not UTF-8"))?;
        let replay = replay_journal(contents, &campaign_id)?;
        if replay.current_session != session_id {
            return Err(invalid("active journal session mismatch"));
        }
        let file = OpenOptions::new().append(true).open(&path)?;
        Ok(Self {
            path,
            file,
            campaign_id,
            session_id,
            next_sequence: replay.next_sequence,
            campaign_started: true,
            terminal: replay.terminal.is_some(),
        })
    }

    /// Canonical path which must be announced before bounded work begins.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Validates and syncs the exact complete current prefix before checkpoint
    /// publication. The returned bytes are the evidence bound by the caller.
    pub fn validated_synced_prefix(&mut self) -> io::Result<Vec<u8>> {
        self.file.flush()?;
        let bytes = fs::read(&self.path)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| invalid("journal is not UTF-8"))?;
        let replay = replay_journal(text, &self.campaign_id)?;
        if replay.next_sequence != self.next_sequence || replay.current_session != self.session_id {
            return Err(invalid("journal changed outside its sole writer"));
        }
        self.file.sync_data()?;
        Ok(bytes)
    }

    /// Validates complete canonical journal bytes with the shared replay rules.
    pub fn validate_prefix(bytes: &[u8], campaign_id: &str) -> io::Result<Vec<JournalRecord>> {
        let text =
            std::str::from_utf8(bytes).map_err(|_| invalid("journal prefix is not UTF-8"))?;
        Ok(replay_journal(text, campaign_id)?.records)
    }

    /// Immutable campaign identity of this sole journal writer.
    pub fn campaign_id(&self) -> &str {
        &self.campaign_id
    }

    /// Current session identity; sequential modes reopen this exact session.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Exact readily visible log-path line required by the project invariant.
    pub fn announcement(&self) -> String {
        format!("GF2_CAMPAIGN_EXECUTION_LOG={}", self.path.to_string_lossy())
    }

    /// Prints and flushes the canonical-path announcement.
    pub fn announce(&self, mut writer: impl Write) -> io::Result<()> {
        writeln!(writer, "{}", self.announcement())?;
        writer.flush()
    }

    /// Sequence which the next record will receive.
    pub fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    /// Appends and immediately flushes one progress record.
    pub fn append(
        &mut self,
        event: JournalEvent,
        case: Option<Value>,
        details: Value,
    ) -> io::Result<JournalRecord> {
        if self.terminal {
            return Err(invalid(
                "cannot append after this session's terminal record",
            ));
        }
        if event.is_terminal()
            || matches!(
                event,
                JournalEvent::SessionStart | JournalEvent::Recovery | JournalEvent::Interrupted
            )
        {
            return Err(invalid(
                "session and terminal boundary events are managed by resume/terminal",
            ));
        }
        if !self.campaign_started && event != JournalEvent::CampaignStart {
            return Err(invalid("the first journal record must be campaign-start"));
        }
        if self.campaign_started && event == JournalEvent::CampaignStart {
            return Err(invalid("campaign-start may occur exactly once"));
        }
        self.campaign_started = true;
        self.append_inner(event, case, details)
    }

    /// Appends and syncs the terminal record for this session.
    pub fn terminal(&mut self, state: TerminalState, details: Value) -> io::Result<JournalRecord> {
        if self.terminal {
            return Err(invalid("session already has a terminal record"));
        }
        if !self.campaign_started {
            return Err(invalid("campaign-start must precede a terminal record"));
        }
        let record = self.append_inner(state.into(), None, details)?;
        self.terminal = true;
        Ok(record)
    }

    /// Closes an interrupted session only after a durably recorded observed
    /// release. The campaign layer supplies the immutable transition evidence.
    pub fn interrupted_after_release(&mut self, transition: Value) -> io::Result<JournalRecord> {
        if self.terminal {
            return Err(invalid("session is already closed"));
        }
        let bytes = self.validated_synced_prefix()?;
        let records = Self::validate_prefix(&bytes, &self.campaign_id)?;
        if records
            .iter()
            .rev()
            .find(|record| {
                !matches!(
                    record.event,
                    JournalEvent::SessionRecovery | JournalEvent::Recovery
                )
            })
            .is_none_or(|record| record.event != JournalEvent::LockRelease)
        {
            return Err(invalid(
                "interruption requires immediately preceding observed release",
            ));
        }
        let details = serde_json::json!({
            "session": self.session_id,
            "cause": "release-observed-recovery",
            "session_transition": transition,
        });
        let record = self.append_inner(JournalEvent::Interrupted, None, details)?;
        self.terminal = true;
        Ok(record)
    }

    /// Closes a prepared session after the campaign layer has durably bound
    /// independent writer-death evidence. The journal independently rejects
    /// any prior held-lock, wrapper, work, or child evidence in this session.
    pub fn interrupted_before_lock(&mut self, transition: Value) -> io::Result<JournalRecord> {
        if self.terminal {
            return Err(invalid("session is already closed"));
        }
        let bytes = self.validated_synced_prefix()?;
        let records = Self::validate_prefix(&bytes, &self.campaign_id)?;
        validate_prelock_prefix(&records, &self.session_id)?;
        let record = self.append_inner(
            JournalEvent::Interrupted,
            None,
            serde_json::json!({
                "session": self.session_id,
                "cause": "prelock-writer-dead",
                "session_transition": transition,
            }),
        )?;
        self.terminal = true;
        Ok(record)
    }

    fn append_inner(
        &mut self,
        event: JournalEvent,
        case: Option<Value>,
        details: Value,
    ) -> io::Result<JournalRecord> {
        let record = JournalRecord {
            schema: JOURNAL_SCHEMA.to_owned(),
            timestamp_utc: utc_now()?,
            campaign_id: self.campaign_id.clone(),
            session_id: self.session_id.clone(),
            sequence: self.next_sequence,
            event,
            case,
            details,
        };
        let mut encoded = serde_json::to_vec(&record)?;
        encoded.push(b'\n');
        self.file.write_all(&encoded)?;
        self.file.flush()?;
        if event.requires_sync() {
            self.file.sync_data()?;
        }
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| invalid("journal sequence overflow"))?;
        Ok(record)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum RecoveryPhase {
    IntentSynced,
    LogTruncated,
    RecoveryRecorded,
    InterruptedRecorded,
    SessionStarted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum RecoveryMode {
    Resume,
    RepairActive,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RecoveryIntent {
    schema: String,
    mode: RecoveryMode,
    recovery_id: String,
    campaign_id: String,
    prior_session_id: String,
    resume_session_id: String,
    original_log_len: u64,
    original_log_sha256: String,
    complete_prefix_len: u64,
    complete_prefix_sha256: String,
    complete_prefix_records: u64,
    suffix_sha256: String,
    diagnostic_path: String,
    diagnostic_sha256: String,
    prefix_terminal: Option<JournalEvent>,
    phase: RecoveryPhase,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RecoveryRecordDetails {
    recovery_id: String,
    mode: RecoveryMode,
    diagnostic_path: String,
    diagnostic_sha256: String,
    original_log_len: u64,
    original_log_sha256: String,
    complete_prefix_len: u64,
    complete_prefix_sha256: String,
    complete_prefix_records: u64,
    suffix_sha256: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InterruptedDetails {
    session: String,
    cause: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_transition: Option<Value>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SessionStartDetails {
    resumed_from: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery_id: Option<String>,
}

fn begin_log_recovery(
    path: &Path,
    bytes: &[u8],
    campaign_id: &str,
    resume_session_id: &str,
    mode: RecoveryMode,
) -> io::Result<Option<(PathBuf, RecoveryIntent)>> {
    let Some(last_newline) = bytes.iter().rposition(|byte| *byte == b'\n') else {
        return Err(invalid(
            "execution log has no complete newline-terminated record",
        ));
    };
    let prefix_len = last_newline + 1;
    if prefix_len == bytes.len() {
        return Ok(None);
    }
    let prefix = std::str::from_utf8(&bytes[..prefix_len])
        .map_err(|_| invalid("complete journal prefix is not UTF-8"))?;
    let replay = replay_journal(prefix, campaign_id)?;
    if matches!(
        replay.terminal,
        Some(JournalEvent::Complete | JournalEvent::Failed)
    ) {
        return Err(invalid(
            "a complete or failed campaign cannot recover a torn tail",
        ));
    }
    match mode {
        RecoveryMode::Resume if replay.sessions.contains(resume_session_id) => {
            return Err(invalid("resume session identity was already used"))
        }
        RecoveryMode::RepairActive if replay.current_session != resume_session_id => {
            return Err(invalid("active repair session identity mismatch"))
        }
        _ => {}
    }
    let suffix = &bytes[prefix_len..];
    let original_sha256 = sha256_bytes(bytes);
    let prefix_sha256 = sha256_bytes(&bytes[..prefix_len]);
    let suffix_sha256 = sha256_bytes(suffix);
    let recovery_id = sha256_bytes(
        format!("{campaign_id}:{original_sha256}:{prefix_sha256}:{suffix_sha256}").as_bytes(),
    );
    let diagnostic = path.with_file_name(format!("{EXECUTION_LOG_NAME}.torn-{recovery_id}.bin"));
    write_new_or_validate(&diagnostic, suffix)?;
    let diagnostic = fs::canonicalize(diagnostic)?;
    let intent = RecoveryIntent {
        schema: RECOVERY_SCHEMA.to_owned(),
        mode,
        recovery_id: recovery_id.clone(),
        campaign_id: campaign_id.to_owned(),
        prior_session_id: replay.current_session,
        resume_session_id: resume_session_id.to_owned(),
        original_log_len: bytes.len() as u64,
        original_log_sha256: original_sha256,
        complete_prefix_len: prefix_len as u64,
        complete_prefix_sha256: prefix_sha256,
        complete_prefix_records: replay.next_sequence,
        suffix_sha256: suffix_sha256.clone(),
        diagnostic_path: diagnostic.to_string_lossy().into_owned(),
        diagnostic_sha256: suffix_sha256,
        prefix_terminal: replay.terminal,
        phase: RecoveryPhase::IntentSynced,
    };
    validate_recovery_intent(&intent)?;
    let intent_path = path.with_file_name(format!(
        "{EXECUTION_LOG_NAME}.recovery-intent-{recovery_id}.json"
    ));
    write_new_or_validate(&intent_path, &canonical_bytes(&intent)?)?;
    Ok(Some((intent_path, intent)))
}

fn find_incomplete_recovery(path: &Path) -> io::Result<Option<(PathBuf, RecoveryIntent)>> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut incomplete = Vec::new();
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with(&format!("{EXECUTION_LOG_NAME}.recovery-intent-"))
            || !name.ends_with(".json")
        {
            continue;
        }
        let intent: RecoveryIntent = read_canonical_json(&entry.path())?;
        validate_recovery_intent(&intent)?;
        let expected = format!(
            "{EXECUTION_LOG_NAME}.recovery-intent-{}.json",
            intent.recovery_id
        );
        if name != expected {
            return Err(invalid(
                "log recovery intent filename does not match its identity",
            ));
        }
        incomplete.push((entry.path(), intent));
    }
    match incomplete.len() {
        0 => Ok(None),
        1 => Ok(incomplete.pop()),
        _ => Err(invalid("multiple incomplete log recovery intents exist")),
    }
}

fn continue_log_recovery(
    path: &Path,
    intent_path: &Path,
    mut intent: RecoveryIntent,
) -> io::Result<ExecutionLog> {
    validate_recovery_intent(&intent)?;
    if fs::canonicalize(&intent.diagnostic_path)? != Path::new(&intent.diagnostic_path) {
        return Err(invalid("log recovery diagnostic path is not canonical"));
    }
    let diagnostic = fs::read(&intent.diagnostic_path)?;
    if sha256_bytes(&diagnostic) != intent.diagnostic_sha256
        || intent.diagnostic_sha256 != intent.suffix_sha256
    {
        return Err(invalid("log recovery diagnostic bytes changed"));
    }
    normalize_recovery_log(path, &intent)?;
    set_recovery_phase(intent_path, &mut intent, RecoveryPhase::LogTruncated)?;

    let mut replay = replay_journal(&fs::read_to_string(path)?, &intent.campaign_id)?;
    let recovery_records: Vec<_> = replay
        .records
        .iter()
        .filter(|record| {
            record.event == JournalEvent::Recovery
                && record.details.get("recovery_id").and_then(Value::as_str)
                    == Some(intent.recovery_id.as_str())
        })
        .collect();
    if recovery_records.len() > 1 {
        return Err(invalid("log recovery record is duplicated"));
    }
    if let Some(record) = recovery_records.first() {
        if record.sequence != intent.complete_prefix_records
            || record.session_id != intent.prior_session_id
            || record.details != recovery_details(&intent)
        {
            return Err(invalid("log recovery record occurs at the wrong boundary"));
        }
    } else {
        if replay.next_sequence != intent.complete_prefix_records {
            return Err(invalid(
                "journal advanced without its required recovery record",
            ));
        }
        let mut log = log_from_replay(path, &intent.campaign_id, &replay)?;
        log.append_inner(JournalEvent::Recovery, None, recovery_details(&intent))?;
        drop(log);
    }
    set_recovery_phase(intent_path, &mut intent, RecoveryPhase::RecoveryRecorded)?;
    replay = replay_journal(&fs::read_to_string(path)?, &intent.campaign_id)?;

    if intent.mode == RecoveryMode::RepairActive {
        if replay.current_session != intent.prior_session_id
            || replay.next_sequence != intent.complete_prefix_records + 1
        {
            return Err(invalid("active repair advanced beyond its recovery record"));
        }
        fs::remove_file(intent_path)?;
        File::open(intent_path.parent().unwrap_or_else(|| Path::new(".")))?.sync_all()?;
        let mut log = log_from_replay(path, &intent.campaign_id, &replay)?;
        log.terminal = replay.terminal.is_some();
        return Ok(log);
    }

    let needs_interrupted = intent.prefix_terminal.is_none();
    let interrupted_sequence = intent.complete_prefix_records + 1;
    let interrupted_records: Vec<_> = replay
        .records
        .iter()
        .filter(|record| {
            record.event == JournalEvent::Interrupted
                && record.sequence == interrupted_sequence
                && record.session_id == intent.prior_session_id
        })
        .collect();
    if interrupted_records.len() > 1 {
        return Err(invalid("log recovery interruption is duplicated"));
    }
    if let Some(record) = interrupted_records.first() {
        if record.details
            != serde_json::json!({
                "session": intent.prior_session_id,
                "cause": "torn-journal-tail",
                "recovery_id": intent.recovery_id,
            })
        {
            return Err(invalid("log recovery interruption details changed"));
        }
    }
    if needs_interrupted && interrupted_records.is_empty() {
        if replay.next_sequence != interrupted_sequence {
            return Err(invalid(
                "journal advanced without its recovery interruption",
            ));
        }
        let mut log = log_from_replay(path, &intent.campaign_id, &replay)?;
        log.append_inner(
            JournalEvent::Interrupted,
            None,
            serde_json::json!({
                "session": intent.prior_session_id,
                "cause": "torn-journal-tail",
                "recovery_id": intent.recovery_id,
            }),
        )?;
        drop(log);
    } else if !needs_interrupted && !interrupted_records.is_empty() {
        return Err(invalid(
            "closed prefix gained a spurious interrupted record",
        ));
    }
    set_recovery_phase(intent_path, &mut intent, RecoveryPhase::InterruptedRecorded)?;
    replay = replay_journal(&fs::read_to_string(path)?, &intent.campaign_id)?;

    let session_starts: Vec<_> = replay
        .records
        .iter()
        .filter(|record| {
            record.event == JournalEvent::SessionStart
                && record.session_id == intent.resume_session_id
        })
        .collect();
    if session_starts.len() > 1 {
        return Err(invalid("recovery resume session boundary is duplicated"));
    }
    let expected_session_sequence =
        intent.complete_prefix_records + 1 + u64::from(needs_interrupted);
    if let Some(record) = session_starts.first() {
        if record.sequence != expected_session_sequence
            || record.details
                != serde_json::json!({
                    "resumed_from": intent.prior_session_id,
                    "recovery_id": intent.recovery_id,
                })
        {
            return Err(invalid(
                "recovery session boundary occurs at the wrong sequence",
            ));
        }
    } else {
        if replay.next_sequence != expected_session_sequence {
            return Err(invalid(
                "journal advanced without its recovery session boundary",
            ));
        }
        let mut log = log_from_replay(path, &intent.campaign_id, &replay)?;
        log.session_id = intent.resume_session_id.clone();
        log.append_inner(
            JournalEvent::SessionStart,
            None,
            serde_json::json!({
                "resumed_from": intent.prior_session_id,
                "recovery_id": intent.recovery_id,
            }),
        )?;
        drop(log);
    }
    set_recovery_phase(intent_path, &mut intent, RecoveryPhase::SessionStarted)?;
    fs::remove_file(intent_path)?;
    File::open(intent_path.parent().unwrap_or_else(|| Path::new(".")))?.sync_all()?;
    replay = replay_journal(&fs::read_to_string(path)?, &intent.campaign_id)?;
    if replay.current_session != intent.resume_session_id || replay.terminal.is_some() {
        return Err(invalid(
            "recovery did not establish one active resume session",
        ));
    }
    log_from_replay(path, &intent.campaign_id, &replay)
}

fn recovery_details(intent: &RecoveryIntent) -> Value {
    serde_json::json!({
        "recovery_id": intent.recovery_id,
        "mode": intent.mode,
        "diagnostic_path": intent.diagnostic_path,
        "diagnostic_sha256": intent.diagnostic_sha256,
        "original_log_len": intent.original_log_len,
        "original_log_sha256": intent.original_log_sha256,
        "complete_prefix_len": intent.complete_prefix_len,
        "complete_prefix_sha256": intent.complete_prefix_sha256,
        "complete_prefix_records": intent.complete_prefix_records,
        "suffix_sha256": intent.suffix_sha256,
    })
}

fn normalize_recovery_log(path: &Path, intent: &RecoveryIntent) -> io::Result<()> {
    let mut bytes = fs::read(path)?;
    let prefix_len = intent.complete_prefix_len as usize;
    if bytes.len() == intent.original_log_len as usize
        && sha256_bytes(&bytes) == intent.original_log_sha256
    {
        if sha256_bytes(&bytes[prefix_len..]) != intent.suffix_sha256 {
            return Err(invalid(
                "original torn suffix no longer matches recovery intent",
            ));
        }
        truncate_and_sync(path, prefix_len)?;
        return Ok(());
    }
    if bytes.len() < prefix_len
        || sha256_bytes(&bytes[..prefix_len]) != intent.complete_prefix_sha256
    {
        return Err(invalid(
            "journal is neither the recovery's original nor prefix state",
        ));
    }
    if !bytes.ends_with(b"\n") {
        let boundary = bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map(|index| index + 1)
            .ok_or_else(|| invalid("recovery journal lost its complete prefix"))?;
        if boundary < prefix_len {
            return Err(invalid(
                "recovery journal lost its complete prefix boundary",
            ));
        }
        let retry_suffix = &bytes[boundary..];
        let retry_sha = sha256_bytes(retry_suffix);
        let diagnostic = path.with_file_name(format!(
            "{EXECUTION_LOG_NAME}.recovery-{}-retry-{retry_sha}.bin",
            intent.recovery_id
        ));
        write_new_or_validate(&diagnostic, retry_suffix)?;
        truncate_and_sync(path, boundary)?;
        bytes.truncate(boundary);
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| invalid("recovery journal complete records are not UTF-8"))?;
    replay_journal(&text, &intent.campaign_id)?;
    Ok(())
}

fn truncate_and_sync(path: &Path, len: usize) -> io::Result<()> {
    let file = OpenOptions::new().write(true).open(path)?;
    file.set_len(len as u64)?;
    file.sync_all()?;
    File::open(path.parent().unwrap_or_else(|| Path::new(".")))?.sync_all()
}

fn set_recovery_phase(
    path: &Path,
    intent: &mut RecoveryIntent,
    phase: RecoveryPhase,
) -> io::Result<()> {
    if phase > intent.phase {
        intent.phase = phase;
        atomic_replace(path, &canonical_bytes(intent)?)?;
    }
    Ok(())
}

fn validate_recovery_intent(intent: &RecoveryIntent) -> io::Result<()> {
    if intent.schema != RECOVERY_SCHEMA {
        return Err(invalid("unknown log recovery schema"));
    }
    validate_sha256(&intent.recovery_id, "recovery ID")?;
    validate_sha256(&intent.original_log_sha256, "original log digest")?;
    validate_sha256(&intent.complete_prefix_sha256, "complete prefix digest")?;
    validate_sha256(&intent.suffix_sha256, "torn suffix digest")?;
    validate_sha256(&intent.diagnostic_sha256, "recovery diagnostic digest")?;
    checked_identity(intent.campaign_id.clone(), "recovery campaign")?;
    checked_identity(intent.prior_session_id.clone(), "recovery prior session")?;
    checked_identity(intent.resume_session_id.clone(), "recovery resume session")?;
    if (intent.mode == RecoveryMode::RepairActive)
        != (intent.prior_session_id == intent.resume_session_id)
    {
        return Err(invalid("recovery mode and session identities disagree"));
    }
    let expected_recovery_id = sha256_bytes(
        format!(
            "{}:{}:{}:{}",
            intent.campaign_id,
            intent.original_log_sha256,
            intent.complete_prefix_sha256,
            intent.suffix_sha256
        )
        .as_bytes(),
    );
    let diagnostic_name = Path::new(&intent.diagnostic_path)
        .file_name()
        .and_then(|name| name.to_str());
    let expected_diagnostic_name = format!("{EXECUTION_LOG_NAME}.torn-{}.bin", intent.recovery_id);
    if intent.complete_prefix_len == 0
        || intent.complete_prefix_len >= intent.original_log_len
        || intent.complete_prefix_records == 0
        || intent.recovery_id != expected_recovery_id
        || intent.diagnostic_sha256 != intent.suffix_sha256
        || diagnostic_name != Some(expected_diagnostic_name.as_str())
        || !matches!(
            intent.prefix_terminal,
            None | Some(
                JournalEvent::Paused | JournalEvent::BudgetExhausted | JournalEvent::Interrupted
            )
        )
    {
        return Err(invalid("log recovery intent has invalid boundaries"));
    }
    Ok(())
}

fn write_new_or_validate(path: &Path, bytes: &[u8]) -> io::Result<()> {
    match atomic_write_new(path, bytes) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let existing = fs::read(path)?;
            if existing == bytes {
                Ok(())
            } else {
                Err(invalid(format!(
                    "existing recovery artifact {} has different bytes",
                    path.display()
                )))
            }
        }
        Err(error) => Err(error),
    }
}

fn log_from_replay(
    path: &Path,
    campaign_id: &str,
    replay: &JournalReplay,
) -> io::Result<ExecutionLog> {
    Ok(ExecutionLog {
        path: path.to_owned(),
        file: OpenOptions::new().append(true).open(path)?,
        campaign_id: campaign_id.to_owned(),
        session_id: replay.current_session.clone(),
        next_sequence: replay.next_sequence,
        campaign_started: true,
        terminal: false,
    })
}

struct JournalReplay {
    next_sequence: u64,
    current_session: String,
    terminal: Option<JournalEvent>,
    sessions: BTreeSet<String>,
    records: Vec<JournalRecord>,
}

fn replay_journal(contents: &str, campaign_id: &str) -> io::Result<JournalReplay> {
    let mut expected = 0_u64;
    let mut current_session = None::<String>;
    let mut terminal = None;
    let mut sessions = BTreeSet::new();
    let mut recoveries = BTreeSet::new();
    let mut pending_recovery = None::<String>;
    let mut records = Vec::new();
    let mut byte_offset = 0_usize;
    for encoded_line in contents.split_inclusive('\n') {
        let line = encoded_line
            .strip_suffix('\n')
            .ok_or_else(|| invalid("journal ends without a newline"))?;
        let record: JournalRecord = decode_canonical(line, "journal record")?;
        if record.schema != JOURNAL_SCHEMA
            || record.campaign_id != campaign_id
            || record.sequence != expected
        {
            return Err(invalid(format!(
                "journal identity or sequence mismatch at {expected}"
            )));
        }
        checked_identity(record.session_id.clone(), "journal session")?;
        if expected == 0 {
            if record.event != JournalEvent::CampaignStart {
                return Err(invalid("journal does not begin with campaign-start"));
            }
            sessions.insert(record.session_id.clone());
            current_session = Some(record.session_id.clone());
        } else if record.event == JournalEvent::SessionStart {
            if !matches!(
                terminal,
                Some(
                    JournalEvent::Paused
                        | JournalEvent::BudgetExhausted
                        | JournalEvent::Interrupted
                )
            ) {
                return Err(invalid(
                    "session-start does not follow a resumable boundary",
                ));
            }
            if !sessions.insert(record.session_id.clone()) {
                return Err(invalid("journal reuses a session identity"));
            }
            let details: SessionStartDetails = serde_json::from_value(record.details.clone())
                .map_err(|error| invalid(format!("invalid session-start details: {error}")))?;
            if details.resumed_from != current_session.as_deref().unwrap_or_default()
                || details.recovery_id != pending_recovery
            {
                return Err(invalid("session-start does not match its prior boundary"));
            }
            current_session = Some(record.session_id.clone());
            terminal = None;
            pending_recovery = None;
        } else {
            if record.event == JournalEvent::CampaignStart {
                return Err(invalid("journal contains duplicate campaign-start"));
            }
            let recovery_after_boundary = record.event == JournalEvent::Recovery
                && matches!(
                    terminal,
                    Some(
                        JournalEvent::Paused
                            | JournalEvent::BudgetExhausted
                            | JournalEvent::Interrupted
                    )
                );
            if terminal.is_some() && !recovery_after_boundary {
                return Err(invalid("journal event occurs after a closed session"));
            }
            if current_session.as_deref() != Some(record.session_id.as_str()) {
                return Err(invalid("journal event crosses session identity"));
            }
        }
        if record.event == JournalEvent::Recovery {
            let details: RecoveryRecordDetails = serde_json::from_value(record.details.clone())
                .map_err(|error| invalid(format!("invalid recovery details: {error}")))?;
            validate_recovery_record_details(
                &details,
                record.sequence,
                campaign_id,
                &contents.as_bytes()[..byte_offset],
            )?;
            if !recoveries.insert(details.recovery_id.clone()) {
                return Err(invalid("journal duplicates a recovery identity"));
            }
            if details.mode == RecoveryMode::Resume {
                pending_recovery = Some(details.recovery_id);
            }
        }
        if record.event == JournalEvent::Interrupted {
            let details: InterruptedDetails = serde_json::from_value(record.details.clone())
                .map_err(|error| invalid(format!("invalid interrupted details: {error}")))?;
            let valid_cause = match (details.cause.as_str(), details.recovery_id.as_ref()) {
                ("missing-terminal-record", None) => {
                    pending_recovery.is_none() && details.session_transition.is_none()
                }
                ("release-observed-recovery", None) => {
                    pending_recovery.is_none()
                        && details.session_transition.is_some()
                        && records
                            .iter()
                            .rev()
                            .find(|prior: &&JournalRecord| {
                                !matches!(
                                    prior.event,
                                    JournalEvent::SessionRecovery | JournalEvent::Recovery
                                )
                            })
                            .is_some_and(|prior| prior.event == JournalEvent::LockRelease)
                }
                ("prelock-writer-dead", None) => {
                    pending_recovery.is_none()
                        && details.session_transition.is_some()
                        && validate_prelock_prefix(&records, &record.session_id).is_ok()
                }
                ("torn-journal-tail", Some(recovery_id)) => {
                    pending_recovery.as_ref() == Some(recovery_id)
                }
                _ => false,
            };
            if details.session != record.session_id || !valid_cause {
                return Err(invalid(
                    "interrupted record does not match its session state",
                ));
            }
        }
        if record.event.is_terminal() || record.event == JournalEvent::Interrupted {
            terminal = Some(record.event);
        }
        records.push(record);
        expected = expected
            .checked_add(1)
            .ok_or_else(|| invalid("journal sequence overflow"))?;
        byte_offset += encoded_line.len();
    }
    Ok(JournalReplay {
        next_sequence: expected,
        current_session: current_session.ok_or_else(|| invalid("journal is empty"))?,
        terminal,
        sessions,
        records,
    })
}

pub(crate) fn validate_prelock_prefix(records: &[JournalRecord], session: &str) -> io::Result<()> {
    let mut prepared = false;
    for record in records.iter().filter(|record| record.session_id == session) {
        prepared |= record.event == JournalEvent::SessionPrepared;
        if record.event.is_terminal()
            || matches!(
                record.event,
                JournalEvent::LockHold
                    | JournalEvent::LockRelease
                    | JournalEvent::WrapperReturned
                    | JournalEvent::ReleaseUnobserved
                    | JournalEvent::WorkFinished
                    | JournalEvent::ChildSpawn
                    | JournalEvent::ChildExit
                    | JournalEvent::ChildDiagnostic
                    | JournalEvent::ExecutionProgress
                    | JournalEvent::WindowProgress
                    | JournalEvent::Interrupted
            )
        {
            return Err(invalid(
                "prelock interruption conflicts with session execution evidence",
            ));
        }
    }
    if !prepared {
        return Err(invalid("prelock interruption lacks session preparation"));
    }
    Ok(())
}

fn validate_recovery_record_details(
    details: &RecoveryRecordDetails,
    sequence: u64,
    campaign_id: &str,
    complete_prefix: &[u8],
) -> io::Result<()> {
    for (digest, name) in [
        (&details.recovery_id, "journal recovery ID"),
        (
            &details.diagnostic_sha256,
            "journal recovery diagnostic digest",
        ),
        (&details.original_log_sha256, "journal original log digest"),
        (
            &details.complete_prefix_sha256,
            "journal complete prefix digest",
        ),
        (&details.suffix_sha256, "journal torn suffix digest"),
    ] {
        validate_sha256(digest, name)?;
    }
    let expected = sha256_bytes(
        format!(
            "{campaign_id}:{original}:{prefix}:{suffix}",
            original = details.original_log_sha256,
            prefix = details.complete_prefix_sha256,
            suffix = details.suffix_sha256,
        )
        .as_bytes(),
    );
    let diagnostic_name = Path::new(&details.diagnostic_path)
        .file_name()
        .and_then(|name| name.to_str());
    let expected_diagnostic_name = format!("{EXECUTION_LOG_NAME}.torn-{}.bin", details.recovery_id);
    let diagnostic_path = fs::canonicalize(&details.diagnostic_path)?;
    let diagnostic = fs::read(&diagnostic_path)?;
    let mut original = Sha256::new();
    original.update(complete_prefix);
    original.update(&diagnostic);
    let reconstructed_original_sha256 = format!("{:x}", original.finalize());
    if details.complete_prefix_records != sequence
        || details.complete_prefix_len != complete_prefix.len() as u64
        || details.complete_prefix_len == 0
        || details.complete_prefix_len >= details.original_log_len
        || details.complete_prefix_sha256 != sha256_bytes(complete_prefix)
        || details.original_log_len != details.complete_prefix_len + diagnostic.len() as u64
        || details.original_log_sha256 != reconstructed_original_sha256
        || diagnostic_path != Path::new(&details.diagnostic_path)
        || details.diagnostic_sha256 != sha256_bytes(&diagnostic)
        || details.diagnostic_sha256 != details.suffix_sha256
        || diagnostic_name != Some(expected_diagnostic_name.as_str())
        || expected != details.recovery_id
    {
        return Err(invalid(
            "recovery record has inconsistent identity or boundaries",
        ));
    }
    Ok(())
}

/// Immutable facts which must compare exactly before a campaign resumes.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResumeIdentity {
    /// Digest of the reviewed premeasurement protocol.
    pub protocol_digest: String,
    /// Optional source-control locator for display and historical receipts.
    /// This field is informational and is excluded from resume equivalence.
    #[serde(default)]
    pub source_revision: String,
    /// Digest of the complete selected producing-input content snapshot.
    pub source_sha256: String,
    /// Digest of the fixed ordered unit manifest.
    pub ordered_work_manifest_sha256: String,
    /// Digest of the staged process descriptors.
    pub process_descriptors_sha256: String,
    /// Nonempty map from executable identity to its digest.
    pub executable_sha256: BTreeMap<String, String>,
    /// Nonempty map from measurement component to behavioral digest.
    pub behavior_sha256: BTreeMap<String, String>,
    /// Closed lifecycle schema interpreted by the campaign layer.
    pub lifecycle_schema: String,
    /// Digest of lifecycle transition behavior.
    pub lifecycle_behavior_sha256: String,
    /// Exact feature-set contract used for the producing build.
    pub feature_contract: String,
    /// Exact thread and worker contract used for measurement.
    pub thread_contract: String,
    /// Stable identity of the measurement host.
    pub host_identity: String,
}

impl ResumeIdentity {
    /// Whether two identities describe the same measurement behavior and
    /// inputs. Source-control locators are informational and do not affect
    /// checkpoint compatibility.
    pub fn resume_equivalent(&self, other: &Self) -> bool {
        self.protocol_digest == other.protocol_digest
            && self.source_sha256 == other.source_sha256
            && self.ordered_work_manifest_sha256 == other.ordered_work_manifest_sha256
            && self.process_descriptors_sha256 == other.process_descriptors_sha256
            && self.executable_sha256 == other.executable_sha256
            && self.behavior_sha256 == other.behavior_sha256
            && self.lifecycle_schema == other.lifecycle_schema
            && self.lifecycle_behavior_sha256 == other.lifecycle_behavior_sha256
            && self.feature_contract == other.feature_contract
            && self.thread_contract == other.thread_contract
            && self.host_identity == other.host_identity
    }
}

/// Immutable checkpoint manifest.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointManifest {
    /// Checkpoint manifest schema.
    pub schema: String,
    /// Campaign owning every unit under this manifest.
    pub campaign_id: String,
    /// Immutable resume identity shared by all units.
    pub identity: ResumeIdentity,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CheckpointUnit {
    schema: String,
    campaign_id: String,
    identity_sha256: String,
    key: String,
    case_sha256: String,
    result_sha256: String,
    case: Value,
    result: Value,
}

/// One immutable, atomically accepted unit's path and digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedUnit {
    /// Canonical accepted-unit path.
    pub path: PathBuf,
    /// SHA-256 of the exact accepted bytes.
    pub sha256: String,
}

/// Pending publication found during resume and removed after validation.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveredPending {
    /// Exact recognized pending filename.
    pub file_name: String,
    /// SHA-256 of the removed pending bytes.
    pub sha256: String,
    /// True when the accepted hard link already existed with identical bytes.
    pub accepted_unit_present: bool,
}

/// One durable pending-file recovery batch awaiting journal acknowledgement.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PendingRecovery {
    /// Stable digest identity used to deduplicate the recovery journal record.
    pub recovery_id: String,
    /// Exact pending files and state captured before any file was removed.
    pub entries: Vec<RecoveredPending>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PendingRecoveryManifest {
    schema: String,
    campaign_id: String,
    identity_sha256: String,
    recovery_id: String,
    entries: Vec<RecoveredPending>,
}

#[derive(Serialize)]
struct PendingRecoveryIdentity<'a> {
    schema: &'a str,
    campaign_id: &'a str,
    identity_sha256: &'a str,
    entries: &'a [RecoveredPending],
}

/// An immutable manifest plus create-new checkpoint unit files.
pub struct CheckpointStore {
    root: PathBuf,
    manifest: CheckpointManifest,
    identity_sha256: String,
    completed: BTreeMap<String, CompletedUnit>,
    pending_recovery: Option<PendingRecovery>,
}

impl CheckpointStore {
    /// Creates or reopens checkpoint initialization through a caller-supplied
    /// durable immutable publisher. Partial directory/manifest setup may replay;
    /// a missing manifest beside any unit evidence is never reconstructed.
    pub fn initialize_with(
        root: impl AsRef<Path>,
        campaign_id: impl Into<String>,
        identity: ResumeIdentity,
        mut publish: impl FnMut(&Path, &[u8]) -> io::Result<()>,
    ) -> io::Result<Self> {
        let campaign_id = checked_identity(campaign_id.into(), "campaign")?;
        validate_resume_identity(&identity)?;
        let root = root.as_ref();
        fs::create_dir_all(root)?;
        File::open(root.parent().unwrap_or_else(|| Path::new(".")))?.sync_all()?;
        let manifest = CheckpointManifest {
            schema: CHECKPOINT_SCHEMA.into(),
            campaign_id: campaign_id.clone(),
            identity: identity.clone(),
        };
        let expected = canonical_bytes(&manifest)?;
        let path = root.join("manifest.json");
        let manifest_exists = path.try_exists()?;
        let publication = if manifest_exists {
            let stored: CheckpointManifest = read_canonical_json(&path)?;
            if stored.schema != CHECKPOINT_SCHEMA
                || stored.campaign_id != campaign_id
                || !stored.identity.resume_equivalent(&identity)
            {
                return Err(invalid("checkpoint initialization identity changed"));
            }
            canonical_bytes(&stored)?
        } else {
            expected
        };
        for name in ["units", "pending"] {
            let directory = root.join(name);
            if !path.try_exists()?
                && directory.try_exists()?
                && fs::read_dir(&directory)?.next().is_some()
            {
                return Err(invalid(
                    "checkpoint evidence exists without its initialization manifest",
                ));
            }
            fs::create_dir_all(directory)?;
        }
        File::open(root)?.sync_all()?;
        publish(&path, &publication)?;
        Self::resume(root, campaign_id, identity)
    }
    /// Creates an absent checkpoint directory and immutable manifest.
    pub fn create_new(
        root: impl AsRef<Path>,
        campaign_id: impl Into<String>,
        identity: ResumeIdentity,
    ) -> io::Result<Self> {
        let campaign_id = checked_identity(campaign_id.into(), "campaign")?;
        validate_resume_identity(&identity)?;
        let root = root.as_ref();
        fs::create_dir(root)?;
        File::open(root.parent().unwrap_or_else(|| Path::new(".")))?.sync_all()?;
        fs::create_dir(root.join("units"))?;
        fs::create_dir(root.join("pending"))?;
        let manifest = CheckpointManifest {
            schema: CHECKPOINT_SCHEMA.to_owned(),
            campaign_id,
            identity,
        };
        let bytes = canonical_bytes(&manifest)?;
        atomic_write_new(&root.join("manifest.json"), &bytes)?;
        let root = fs::canonicalize(root)?;
        let identity_sha256 = sha256_bytes(&canonical_bytes(&manifest.identity)?);
        Ok(Self {
            root,
            manifest,
            identity_sha256,
            completed: BTreeMap::new(),
            pending_recovery: None,
        })
    }

    /// Validates the immutable manifest and every completed unit before resume.
    ///
    /// The expected identity is validated and compared before pending files
    /// are changed. Recognized pending units are captured in a canonical,
    /// synced recovery manifest before the first unlink. Recovery then removes
    /// each listed file idempotently, syncs `pending/`, and returns the same
    /// durable recovery batch until [`Self::acknowledge_pending_recovery`]
    /// confirms that its facts were journaled.
    ///
    /// # Errors
    ///
    /// Rejects identity or schema mismatches, noncanonical manifests or units,
    /// unknown filenames, changed unit/case/result digests, conflicting
    /// accepted links, unmanifested pending files, and malformed recovery
    /// state. I/O and directory-sync failures leave the recovery manifest for
    /// deterministic replay on the next same-identity resume.
    pub fn resume(
        root: impl AsRef<Path>,
        campaign_id: impl Into<String>,
        expected_identity: ResumeIdentity,
    ) -> io::Result<Self> {
        let campaign_id = checked_identity(campaign_id.into(), "campaign")?;
        validate_resume_identity(&expected_identity)?;
        Self::open(root.as_ref(), Some((&campaign_id, &expected_identity)))
    }

    /// Reads and validates a portable checkpoint export without changing files.
    ///
    /// Shares manifest, canonical encoding, unit and digest validation with
    /// [`Self::resume`]. An absent empty `pending/` directory is permitted in
    /// committed exports. Pending evidence rejects without recovery or cleanup.
    /// Cost is linear in the total checkpoint bytes. Returns an I/O error for
    /// unavailable content or invalid checkpoint evidence; never panics.
    pub fn inspect(root: impl AsRef<Path>) -> io::Result<Self> {
        Self::open(root.as_ref(), None)
    }

    fn open(root: &Path, resume: Option<(&str, &ResumeIdentity)>) -> io::Result<Self> {
        let root = fs::canonicalize(root)?;
        reject_unexpected_root_entries(&root)?;
        let manifest: CheckpointManifest = read_canonical_json(&root.join("manifest.json"))?;
        checked_identity(manifest.campaign_id.clone(), "campaign")?;
        validate_resume_identity(&manifest.identity)?;
        if manifest.schema != CHECKPOINT_SCHEMA
            || resume.is_some_and(|(campaign, identity)| {
                manifest.campaign_id != campaign || !manifest.identity.resume_equivalent(identity)
            })
        {
            return Err(invalid("checkpoint resume manifest identity mismatch"));
        }
        let campaign_id = &manifest.campaign_id;
        let identity_sha256 = sha256_bytes(&canonical_bytes(&manifest.identity)?);
        let pending_recovery = if resume.is_some() {
            recover_checkpoint_pending(&root, campaign_id, &identity_sha256)?
        } else {
            let pending = root.join("pending");
            if pending.try_exists()? && fs::read_dir(pending)?.next().is_some() {
                return Err(invalid(
                    "checkpoint inspection requires no pending evidence",
                ));
            }
            None
        };
        let mut completed = BTreeMap::new();
        for entry in fs::read_dir(root.join("units"))? {
            let entry = entry?;
            if !entry.file_type()?.is_file()
                || entry.path().extension().and_then(|value| value.to_str()) != Some("json")
            {
                return Err(invalid(format!(
                    "unexpected checkpoint entry {}",
                    entry.path().display()
                )));
            }
            let bytes = fs::read(entry.path())?;
            let text = std::str::from_utf8(&bytes)
                .map_err(|_| invalid("checkpoint unit is not UTF-8 JSON"))?;
            let unit: CheckpointUnit = decode_canonical(text, "checkpoint unit")?;
            validate_unit(&unit, campaign_id, &identity_sha256, &entry.path())?;
            if completed
                .insert(
                    unit.key,
                    CompletedUnit {
                        path: entry.path(),
                        sha256: sha256_bytes(&bytes),
                    },
                )
                .is_some()
            {
                return Err(invalid("duplicate canonical checkpoint key"));
            }
        }
        Ok(Self {
            root,
            manifest,
            identity_sha256,
            completed,
            pending_recovery,
        })
    }

    /// Writes one previously absent, already owner-validated case/result pair.
    ///
    /// The caller must complete semantic and route validation before this call.
    /// This boundary serializes both typed values, records their digests, and
    /// syncs the complete immutable unit before returning acceptance. A
    /// recovered pending-file batch must be journaled and acknowledged first.
    pub fn accept<C: Serialize, R: Serialize>(
        &mut self,
        key: &str,
        case: &C,
        result: &R,
    ) -> io::Result<CompletedUnit> {
        if self.pending_recovery.is_some() {
            return Err(invalid(
                "pending recovery must be acknowledged before accepting new units",
            ));
        }
        if key.is_empty() {
            return Err(invalid("checkpoint key is empty"));
        }
        if self.completed.contains_key(key) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("checkpoint key {key:?} is already complete"),
            ));
        }
        let case = serde_json::to_value(case)?;
        let result = serde_json::to_value(result)?;
        let case_bytes = canonical_bytes(&case)?;
        let result_bytes = canonical_bytes(&result)?;
        let unit = CheckpointUnit {
            schema: UNIT_SCHEMA.to_owned(),
            campaign_id: self.manifest.campaign_id.clone(),
            identity_sha256: self.identity_sha256.clone(),
            key: key.to_owned(),
            case_sha256: sha256_bytes(&case_bytes),
            result_sha256: sha256_bytes(&result_bytes),
            case,
            result,
        };
        let bytes = canonical_bytes(&unit)?;
        let path = self
            .root
            .join("units")
            .join(format!("{}.json", sha256_bytes(key.as_bytes())));
        atomic_write_at(&path, &bytes, false, &self.root.join("pending"))?;
        let completed = CompletedUnit {
            path,
            sha256: sha256_bytes(&bytes),
        };
        self.completed.insert(key.to_owned(), completed.clone());
        Ok(completed)
    }

    /// Validated immutable checkpoint manifest.
    pub fn manifest(&self) -> &CheckpointManifest {
        &self.manifest
    }

    /// Campaign identity pinned by the immutable checkpoint manifest.
    pub fn campaign_id(&self) -> &str {
        &self.manifest.campaign_id
    }

    /// Full immutable resume identity pinned by the checkpoint manifest.
    pub fn identity(&self) -> &ResumeIdentity {
        &self.manifest.identity
    }

    /// Returns completed keys in deterministic order.
    pub fn completed_keys(&self) -> Vec<&str> {
        self.completed.keys().map(String::as_str).collect()
    }

    /// Immutable accepted-file identity for journal binding and owner exports.
    pub fn completed_unit(&self, key: &str) -> Option<&CompletedUnit> {
        self.completed.get(key)
    }

    /// Entries in the durable recovery batch, or empty after acknowledgement.
    pub fn recovered_pending(&self) -> &[RecoveredPending] {
        self.pending_recovery
            .as_ref()
            .map_or(&[], |recovery| recovery.entries.as_slice())
    }

    /// Durable recovery batch which must be journaled before acknowledgement.
    pub fn pending_recovery(&self) -> Option<&PendingRecovery> {
        self.pending_recovery.as_ref()
    }

    /// Removes a journaled recovery manifest and syncs its directory.
    ///
    /// The caller must first durably append a recovery record containing the
    /// same `recovery_id` and entries. A mismatched or absent identity rejects
    /// without changing checkpoint state.
    pub fn acknowledge_pending_recovery(&mut self, recovery_id: &str) -> io::Result<()> {
        let recovery = self
            .pending_recovery
            .as_ref()
            .ok_or_else(|| invalid("there is no pending recovery to acknowledge"))?;
        if recovery.recovery_id != recovery_id {
            return Err(invalid(
                "pending recovery acknowledgement identity mismatch",
            ));
        }
        let pending = self.root.join("pending");
        match fs::remove_file(pending.join(PENDING_RECOVERY_NAME)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        File::open(&pending)?.sync_all()?;
        self.pending_recovery = None;
        Ok(())
    }

    /// Strictly decodes a completed unit into owner wire types.
    pub fn load<C, R>(&self, key: &str) -> io::Result<(C, R)>
    where
        C: DeserializeOwned + Serialize,
        R: DeserializeOwned + Serialize,
    {
        let completed = self
            .completed
            .get(key)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, key.to_owned()))?;
        let bytes = fs::read(&completed.path)?;
        if sha256_bytes(&bytes) != completed.sha256 {
            return Err(invalid("completed checkpoint bytes changed after resume"));
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| invalid("completed checkpoint is not UTF-8"))?;
        let unit: CheckpointUnit = decode_canonical(text, "completed checkpoint")?;
        validate_unit(
            &unit,
            &self.manifest.campaign_id,
            &self.identity_sha256,
            &completed.path,
        )?;
        if unit.key != key {
            return Err(invalid("completed checkpoint key changed after resume"));
        }
        let case = decode_value_canonical(unit.case, "checkpoint case")?;
        let result = decode_value_canonical(unit.result, "checkpoint result")?;
        Ok((case, result))
    }
}

/// Atomically publishes bytes to a previously absent path and syncs its parent.
pub fn atomic_write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    atomic_write(path, bytes, false)
}

/// Atomically replaces a mutable control file and syncs its parent.
///
/// Immutable evidence should use [`atomic_write_new`]. This replacement helper
/// exists for non-evidence pointers and may overwrite its target.
pub fn atomic_replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    atomic_write(path, bytes, true)
}

fn atomic_write(path: &Path, bytes: &[u8], replace: bool) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    atomic_write_at(path, bytes, replace, parent)
}

fn atomic_write_at(
    path: &Path,
    bytes: &[u8],
    replace: bool,
    temporary_parent: &Path,
) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            invalid(format!(
                "output path has no UTF-8 filename: {}",
                path.display()
            ))
        })?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos();
    let mut reserved = None;
    for attempt in 0..128_u8 {
        let temporary = temporary_parent.join(format!(
            ".{file_name}.tmp-{}-{nonce}-{attempt}",
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => {
                reserved = Some((temporary, file));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    let (temporary, mut file) = reserved.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("cannot reserve a temporary file beside {}", path.display()),
        )
    })?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        if replace {
            fs::rename(&temporary, path)?;
        } else {
            fs::hard_link(&temporary, path).map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!("refusing to overwrite {}", path.display()),
                    )
                } else {
                    error
                }
            })?;
        }
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    let cleanup = if temporary.exists() {
        fs::remove_file(&temporary)
            .and_then(|()| File::open(temporary_parent))
            .and_then(|directory| directory.sync_all())
    } else {
        Ok(())
    };
    match result {
        Ok(()) => cleanup,
        Err(error) => {
            let _ = cleanup;
            Err(error)
        }
    }
}

fn reject_unexpected_root_entries(root: &Path) -> io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        if name != "manifest.json" && name != "units" && name != "pending" {
            return Err(invalid(format!(
                "unexpected checkpoint-root entry {}",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

fn recover_checkpoint_pending(
    root: &Path,
    campaign_id: &str,
    identity_sha256: &str,
) -> io::Result<Option<PendingRecovery>> {
    let directory = root.join("pending");
    let manifest_path = directory.join(PENDING_RECOVERY_NAME);
    let staging_path = directory.join(PENDING_RECOVERY_STAGING_NAME);
    if staging_path.exists() {
        if manifest_path.exists() {
            return Err(invalid(
                "checkpoint pending recovery has both staged and published intents",
            ));
        }
        fs::remove_file(&staging_path)?;
        File::open(&directory)?.sync_all()?;
    }

    let manifest = if manifest_path.exists() {
        let manifest: PendingRecoveryManifest = read_canonical_json(&manifest_path)?;
        validate_pending_recovery_manifest(&manifest, campaign_id, identity_sha256)?;
        manifest
    } else {
        let entries = scan_checkpoint_pending(root)?;
        if entries.is_empty() {
            return Ok(None);
        }
        let recovery_id = pending_recovery_id(campaign_id, identity_sha256, &entries)?;
        let manifest = PendingRecoveryManifest {
            schema: PENDING_RECOVERY_SCHEMA.to_owned(),
            campaign_id: campaign_id.to_owned(),
            identity_sha256: identity_sha256.to_owned(),
            recovery_id,
            entries,
        };
        publish_pending_recovery_manifest(&directory, &manifest)?;
        manifest
    };

    validate_pending_recovery_replay(root, &manifest)?;
    for pending in &manifest.entries {
        let path = directory.join(&pending.file_name);
        if path.exists() {
            fs::remove_file(path)?;
        }
    }
    File::open(&directory)?.sync_all()?;
    validate_pending_recovery_replay(root, &manifest)?;
    Ok(Some(PendingRecovery {
        recovery_id: manifest.recovery_id,
        entries: manifest.entries,
    }))
}

fn scan_checkpoint_pending(root: &Path) -> io::Result<Vec<RecoveredPending>> {
    let directory = root.join("pending");
    let mut recovered = Vec::new();
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| invalid("checkpoint pending filename is not UTF-8"))?;
        if name == PENDING_RECOVERY_NAME || name == PENDING_RECOVERY_STAGING_NAME {
            continue;
        }
        let Some(unit_name) = pending_unit_name(name) else {
            return Err(invalid(format!(
                "unrecognized checkpoint pending file {}",
                entry.path().display()
            )));
        };
        if !entry.file_type()?.is_file() {
            return Err(invalid("checkpoint pending entry is not a file"));
        }
        let bytes = fs::read(entry.path())?;
        let sha256 = sha256_bytes(&bytes);
        let accepted = root.join("units").join(unit_name);
        let accepted_unit_present = accepted.exists();
        if accepted_unit_present && fs::read(&accepted)? != bytes {
            return Err(invalid(
                "post-link pending bytes differ from the accepted checkpoint unit",
            ));
        }
        recovered.push(RecoveredPending {
            file_name: name.to_owned(),
            sha256,
            accepted_unit_present,
        });
    }
    recovered.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    Ok(recovered)
}

fn pending_recovery_id(
    campaign_id: &str,
    identity_sha256: &str,
    entries: &[RecoveredPending],
) -> io::Result<String> {
    Ok(sha256_bytes(&canonical_bytes(&PendingRecoveryIdentity {
        schema: PENDING_RECOVERY_SCHEMA,
        campaign_id,
        identity_sha256,
        entries,
    })?))
}

fn validate_pending_recovery_manifest(
    manifest: &PendingRecoveryManifest,
    campaign_id: &str,
    identity_sha256: &str,
) -> io::Result<()> {
    validate_sha256(&manifest.recovery_id, "pending recovery ID")?;
    validate_sha256(
        &manifest.identity_sha256,
        "pending recovery identity digest",
    )?;
    if manifest.schema != PENDING_RECOVERY_SCHEMA
        || manifest.campaign_id != campaign_id
        || manifest.identity_sha256 != identity_sha256
        || manifest.entries.is_empty()
        || manifest.recovery_id
            != pending_recovery_id(campaign_id, identity_sha256, &manifest.entries)?
    {
        return Err(invalid(
            "checkpoint pending recovery manifest identity mismatch",
        ));
    }
    let mut names = BTreeSet::new();
    for entry in &manifest.entries {
        validate_sha256(&entry.sha256, "pending recovery entry digest")?;
        if pending_unit_name(&entry.file_name).is_none() || !names.insert(&entry.file_name) {
            return Err(invalid(
                "checkpoint pending recovery manifest has invalid or duplicate entries",
            ));
        }
    }
    if !manifest
        .entries
        .windows(2)
        .all(|pair| pair[0].file_name < pair[1].file_name)
    {
        return Err(invalid(
            "checkpoint pending recovery entries are not canonical order",
        ));
    }
    Ok(())
}

fn publish_pending_recovery_manifest(
    directory: &Path,
    manifest: &PendingRecoveryManifest,
) -> io::Result<()> {
    let staging = directory.join(PENDING_RECOVERY_STAGING_NAME);
    let published = directory.join(PENDING_RECOVERY_NAME);
    let bytes = canonical_bytes(manifest)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(staging, published)?;
    File::open(directory)?.sync_all()
}

fn validate_pending_recovery_replay(
    root: &Path,
    manifest: &PendingRecoveryManifest,
) -> io::Result<()> {
    let directory = root.join("pending");
    let expected: BTreeSet<_> = manifest
        .entries
        .iter()
        .map(|entry| entry.file_name.as_str())
        .collect();
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| invalid("checkpoint pending filename is not UTF-8"))?;
        if name == PENDING_RECOVERY_NAME {
            continue;
        }
        if name == PENDING_RECOVERY_STAGING_NAME || !expected.contains(name) {
            return Err(invalid(format!(
                "pending file {name:?} is outside the durable recovery manifest"
            )));
        }
    }
    for entry in &manifest.entries {
        let unit_name = pending_unit_name(&entry.file_name)
            .expect("validated pending recovery entries have unit names");
        let pending = directory.join(&entry.file_name);
        if pending.exists() && sha256_bytes(&fs::read(&pending)?) != entry.sha256 {
            return Err(invalid("checkpoint pending bytes changed during recovery"));
        }
        let accepted = root.join("units").join(unit_name);
        if accepted.exists() != entry.accepted_unit_present {
            return Err(invalid(
                "checkpoint accepted-link state changed during pending recovery",
            ));
        }
        if entry.accepted_unit_present && sha256_bytes(&fs::read(accepted)?) != entry.sha256 {
            return Err(invalid(
                "accepted checkpoint bytes differ from their pending recovery entry",
            ));
        }
    }
    Ok(())
}

fn pending_unit_name(name: &str) -> Option<String> {
    let (file, suffix) = name.strip_prefix('.')?.split_once(".json.tmp-")?;
    if !is_lower_hex_64(file) {
        return None;
    }
    let parts: Vec<_> = suffix.split('-').collect();
    if parts.len() != 3
        || !parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }
    Some(format!("{file}.json"))
}

fn is_lower_hex_64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_unit(
    unit: &CheckpointUnit,
    campaign_id: &str,
    identity_sha256: &str,
    path: &Path,
) -> io::Result<()> {
    if unit.schema != UNIT_SCHEMA
        || unit.campaign_id != campaign_id
        || unit.identity_sha256 != identity_sha256
        || unit.key.is_empty()
        || unit.case_sha256 != sha256_bytes(&canonical_bytes(&unit.case)?)
        || unit.result_sha256 != sha256_bytes(&canonical_bytes(&unit.result)?)
        || path.file_name().and_then(|value| value.to_str())
            != Some(&format!("{}.json", sha256_bytes(unit.key.as_bytes())))
    {
        return Err(invalid(format!(
            "checkpoint unit {} failed identity or digest validation",
            path.display()
        )));
    }
    Ok(())
}

fn canonical_bytes<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(io::Error::other)
}

fn read_canonical_json<T>(path: &Path) -> io::Result<T>
where
    T: DeserializeOwned + Serialize,
{
    let mut bytes = Vec::new();
    File::open(path)?.read_to_end(&mut bytes)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid("JSON file is not UTF-8"))?;
    decode_canonical(text, &path.display().to_string())
}

fn decode_canonical<T>(text: &str, name: &str) -> io::Result<T>
where
    T: DeserializeOwned + Serialize,
{
    let value: T = serde_json::from_str(text)
        .map_err(|error| invalid(format!("{name} is invalid JSON: {error}")))?;
    if canonical_bytes(&value)? != text.as_bytes() {
        return Err(invalid(format!("{name} is not canonical compact JSON")));
    }
    Ok(value)
}

fn decode_value_canonical<T>(value: Value, name: &str) -> io::Result<T>
where
    T: DeserializeOwned + Serialize,
{
    let bytes = canonical_bytes(&value)?;
    let typed: T = serde_json::from_value(value)
        .map_err(|error| invalid(format!("{name} has invalid schema: {error}")))?;
    // Values use sorted object keys; owner structs use declaration order.
    // Compare the same canonical Value representation on both sides.
    if canonical_bytes(&serde_json::to_value(&typed)?)? != bytes {
        return Err(invalid(format!("{name} does not re-encode canonically")));
    }
    Ok(typed)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn checked_identity(value: String, kind: &str) -> io::Result<String> {
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        Err(invalid(format!(
            "{kind} identity is empty or contains whitespace"
        )))
    } else {
        Ok(value)
    }
}

pub(crate) fn validate_resume_identity(identity: &ResumeIdentity) -> io::Result<()> {
    validate_sha256(&identity.protocol_digest, "protocol digest")?;
    validate_sha256(&identity.source_sha256, "source digest")?;
    validate_sha256(
        &identity.ordered_work_manifest_sha256,
        "ordered-work manifest digest",
    )?;
    validate_sha256(
        &identity.process_descriptors_sha256,
        "process descriptors digest",
    )?;
    validate_sha256(
        &identity.lifecycle_behavior_sha256,
        "lifecycle behavior digest",
    )?;
    validate_digest_map(&identity.executable_sha256, "executable")?;
    validate_digest_map(&identity.behavior_sha256, "behavior")?;
    checked_identity(identity.lifecycle_schema.clone(), "lifecycle schema")?;
    for (name, value) in [
        ("feature contract", &identity.feature_contract),
        ("thread contract", &identity.thread_contract),
        ("host identity", &identity.host_identity),
    ] {
        if value.trim().is_empty() {
            return Err(invalid(format!("{name} is empty")));
        }
    }
    Ok(())
}

fn validate_digest_map(values: &BTreeMap<String, String>, kind: &str) -> io::Result<()> {
    if values.is_empty() {
        return Err(invalid(format!("{kind} digest map is empty")));
    }
    for (name, digest) in values {
        checked_identity(name.clone(), kind)?;
        validate_sha256(digest, &format!("{kind} digest for {name}"))?;
    }
    Ok(())
}

fn validate_sha256(value: &str, name: &str) -> io::Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(format!(
            "{name} is not exactly 64 lowercase hex digits"
        )));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

pub(crate) fn utc_now() -> io::Result<String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid("system clock is before the Unix epoch"))?;
    let seconds = i64::try_from(duration.as_secs())
        .map_err(|_| invalid("UTC timestamp exceeds supported range"))?;
    let days = seconds.div_euclid(86_400);
    let second_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = second_of_day / 3_600;
    let minute = second_of_day % 3_600 / 60;
    let second = second_of_day % 60;
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{:09}Z",
        duration.subsec_nanos()
    ))
}

// Howard Hinnant's proleptic-Gregorian civil-date conversion.
fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod active_repair_tests {
    use super::*;
    use crate::campaign::{
        DeclaredCounts, LockEvidence, SessionChannels, SessionDescriptor, SessionMode,
        SessionState, SessionStore, SessionTransition, Token, FEATURE_CONTRACT, LIFECYCLE_SCHEMA,
        THREAD_CONTRACT,
    };

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("active-log-repair-{}-{nonce}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn descriptor(stage: &Path) -> SessionDescriptor {
        let hash = sha256_bytes(b"fixed");
        SessionDescriptor {
            schema: LIFECYCLE_SCHEMA.into(),
            preparer: crate::campaign::ProcessIdentity::current().unwrap(),
            campaign_id: Token::new("campaign").unwrap(),
            session_id: Token::new("session").unwrap(),
            channels: SessionChannels::for_stage(stage).unwrap(),
            identity: ResumeIdentity {
                protocol_digest: hash.clone(),
                source_revision: "a".repeat(40),
                source_sha256: hash.clone(),
                ordered_work_manifest_sha256: hash.clone(),
                process_descriptors_sha256: hash.clone(),
                executable_sha256: BTreeMap::from([("producer".into(), hash.clone())]),
                behavior_sha256: BTreeMap::from([("producer".into(), hash.clone())]),
                lifecycle_schema: LIFECYCLE_SCHEMA.into(),
                lifecycle_behavior_sha256: hash,
                feature_contract: FEATURE_CONTRACT.into(),
                thread_contract: THREAD_CONTRACT.into(),
                host_identity: "host".into(),
            },
            counts: DeclaredCounts::for_cells(1).unwrap(),
            lock_path: "/tmp/campaign.lock".into(),
        }
    }
    fn held_session(stage: &Path) -> SessionDescriptor {
        let expected = descriptor(stage);
        let mut log = ExecutionLog::create_new(stage, "campaign", "session").unwrap();
        log.append(JournalEvent::CampaignStart, None, serde_json::json!({}))
            .unwrap();
        let mut session = SessionStore::prepare(expected.clone()).unwrap();
        session.consume_mode(SessionMode::RunSession).unwrap();
        session
            .transition(
                &mut log,
                SessionTransition::LockHeld {
                    evidence: LockEvidence {
                        lock_path: expected.lock_path.clone(),
                        holder_pid: 123,
                        observation: Token::new("inherited").unwrap(),
                    },
                },
            )
            .unwrap();
        expected
    }

    #[test]
    fn active_claim_survives_every_same_session_torn_repair_boundary() {
        for boundary in 0..5 {
            let scratch = Scratch::new();
            let expected = held_session(&scratch.0);
            let path = expected.channels.execution_log.clone();
            let suffix = b"{\"uncommitted-child\":";
            OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap()
                .write_all(suffix)
                .unwrap();
            let original = fs::read(&path).unwrap();
            let (intent_path, mut intent) = begin_log_recovery(
                &path,
                &original,
                "campaign",
                "session",
                RecoveryMode::RepairActive,
            )
            .unwrap()
            .unwrap();
            if boundary >= 1 {
                normalize_recovery_log(&path, &intent).unwrap();
                set_recovery_phase(&intent_path, &mut intent, RecoveryPhase::LogTruncated).unwrap();
            }
            if boundary == 2 {
                OpenOptions::new()
                    .append(true)
                    .open(&path)
                    .unwrap()
                    .write_all(b"{\"partial-recovery\":")
                    .unwrap();
            }
            if boundary >= 3 {
                let replay =
                    replay_journal(&fs::read_to_string(&path).unwrap(), "campaign").unwrap();
                let mut log = log_from_replay(&path, "campaign", &replay).unwrap();
                log.append_inner(JournalEvent::Recovery, None, recovery_details(&intent))
                    .unwrap();
            }
            if boundary == 4 {
                set_recovery_phase(&intent_path, &mut intent, RecoveryPhase::RecoveryRecorded)
                    .unwrap();
            }
            let session = SessionStore::reopen(expected.clone()).unwrap();
            let log = session.repair_log().unwrap();
            assert_eq!(session.lifecycle().state(), SessionState::LockHeld);
            assert_eq!(log.session_id(), "session");
            assert!(scratch.0.join("active-session.json").exists());
            assert!(!intent_path.exists());
            let records =
                ExecutionLog::validate_prefix(&fs::read(&path).unwrap(), "campaign").unwrap();
            assert_eq!(
                records
                    .iter()
                    .filter(|record| record.event == JournalEvent::Recovery)
                    .count(),
                1
            );
            assert!(records.iter().all(|record| !matches!(
                record.event,
                JournalEvent::LockRelease | JournalEvent::Interrupted | JournalEvent::SessionStart
            )));
            assert_eq!(fs::read(&intent.diagnostic_path).unwrap(), suffix);
            let sequence = log.next_sequence();
            drop(log);
            assert_eq!(session.repair_log().unwrap().next_sequence(), sequence);
        }
    }

    #[test]
    fn active_repair_preserves_complete_malformed_records_and_rejects_wrong_session() {
        let scratch = Scratch::new();
        let expected = held_session(&scratch.0);
        let path = &expected.channels.execution_log;
        OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(b"not-json\n")
            .unwrap();
        let original = fs::read(path).unwrap();
        let session = SessionStore::reopen(expected.clone()).unwrap();
        assert!(session.repair_log().is_err());
        assert_eq!(fs::read(path).unwrap(), original);
        assert!(scratch.0.join("active-session.json").exists());
        fs::write(path, &original[..original.len() - 1]).unwrap();
        let torn = fs::read(path).unwrap();
        assert!(ExecutionLog::repair_active(path, "campaign", "different-session").is_err());
        assert_eq!(fs::read(path).unwrap(), torn);
    }
}
