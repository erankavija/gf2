//! Synced write-ahead records; incomplete JSON at an interrupted append is skipped.
#[cfg(not(test))]
use experiments_common::create_durable;
use serde::{de::DeserializeOwned, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
#[cfg(test)]
use tests::create_durable;

pub(crate) struct ExecutionLog(File);
impl ExecutionLog {
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            create_durable(parent)?;
        }
        let file = OpenOptions::new()
            .append(true)
            .read(true)
            .create(true)
            .open(path)?;
        file.try_lock().map_err(io::Error::other)?;
        if let Some(parent) = path.parent() {
            File::open(parent)?.sync_all()?;
        }
        Ok(Self(file))
    }
    pub(crate) fn append<E: Serialize>(&mut self, event: &E) -> io::Result<()> {
        // Separate the next record from a torn append.
        let mut bytes = vec![b'\n'];
        serde_json::to_writer(&mut bytes, event)?;
        bytes.push(b'\n');
        self.0.write_all(&bytes)?;
        self.0.sync_all()
    }
    pub(crate) fn read<E: DeserializeOwned>(path: &Path) -> io::Result<Vec<E>> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut events = Vec::new();
        for line in bytes
            .split(|&byte| byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            match serde_json::from_slice(line) {
                Ok(event) => events.push(event),
                Err(e) if e.is_eof() => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(events)
    }
}

pub(crate) mod field_order {
    use gf2_stats::sampler::FieldOrder;
    use serde::Deserialize;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &FieldOrder, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(value.as_u64())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<FieldOrder, D::Error> {
        match u8::deserialize(deserializer)? {
            3 => Ok(FieldOrder::F3),
            5 => Ok(FieldOrder::F5),
            7 => Ok(FieldOrder::F7),
            _ => Err(serde::de::Error::custom("field order must be 3, 5, or 7")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    thread_local! {
        static FAIL_PREPARATION: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn create_durable(directory: &Path) -> io::Result<()> {
        if FAIL_PREPARATION.with(Cell::get) {
            return Err(io::Error::other("injected ancestor sync failure"));
        }
        experiments_common::create_durable(directory)
    }

    #[test]
    fn ancestor_sync_failure_prevents_opening_or_acknowledging_log_progress() {
        for already_exists in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let parent = directory.path().join("unsynced/ancestors/output");
            if already_exists {
                fs::create_dir_all(&parent).unwrap();
            }
            let path = parent.join("execution.jsonl");
            FAIL_PREPARATION.with(|fault| fault.set(true));
            let opened = ExecutionLog::open(&path);
            FAIL_PREPARATION.with(|fault| fault.set(false));
            assert!(
                matches!(opened, Err(ref error) if error.to_string().contains("ancestor sync failure"))
            );
            assert!(!path.exists());
            let mut log = ExecutionLog::open(&path).unwrap();
            log.append(&"acknowledged progress").unwrap();
            assert_eq!(
                ExecutionLog::read::<String>(&path).unwrap(),
                ["acknowledged progress"]
            );
        }
    }
}
