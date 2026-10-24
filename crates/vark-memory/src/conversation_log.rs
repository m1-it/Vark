use crate::{security, MemoryError};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const DEFAULT_RETENTION_DAYS: u64 = 180;
const SECONDS_PER_DAY: u64 = 86_400;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationLogRecord {
    pub turn_id: u64,
    pub user_id: String,
    pub role: String,
    pub text: String,
    pub timestamp: u64,
    pub feedback: Option<String>,
    pub schema_version: u16,
}

pub struct ConversationLog {
    path: PathBuf,
    retention_days: u64,
    last_prune_day: u64,
}

fn current_day() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() / SECONDS_PER_DAY)
        .unwrap_or(0)
}

fn read_records_unlocked(path: &Path) -> Result<Vec<ConversationLogRecord>, MemoryError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut records = Vec::new();
    for (index, line) in BufReader::new(File::open(path)?).lines().enumerate() {
        let line_number = index + 1;
        let line = line?;
        let plaintext =
            security::open_line(&line).map_err(|error| MemoryError::Storage(error.to_string()))?;
        let record =
            serde_json::from_slice::<ConversationLogRecord>(&plaintext).map_err(|error| {
                MemoryError::CorruptRecord {
                    line: line_number,
                    reason: error.to_string(),
                }
            })?;
        records.push(record);
    }
    Ok(records)
}

fn write_records_unlocked(
    path: &Path,
    records: &[ConversationLogRecord],
) -> Result<(), MemoryError> {
    let mut bytes = Vec::new();
    for record in records {
        let serialized = serde_json::to_vec(record)?;
        let line = security::seal_line(&serialized)
            .map_err(|error| MemoryError::Storage(error.to_string()))?;
        writeln!(&mut bytes, "{line}")?;
    }
    security::write_private_atomic(path, &bytes)?;
    Ok(())
}

fn prune_records(
    path: &Path,
    records: Vec<ConversationLogRecord>,
    retention_days: u64,
    now: u64,
) -> Result<(), MemoryError> {
    let retention_seconds = retention_days.saturating_mul(SECONDS_PER_DAY);
    let cutoff = now.saturating_sub(retention_seconds);
    let retained = records
        .into_iter()
        .filter(|record| record.timestamp >= cutoff)
        .collect::<Vec<_>>();
    write_records_unlocked(path, &retained)
}

impl ConversationLog {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MemoryError> {
        Self::open_with_retention(path, DEFAULT_RETENTION_DAYS)
    }

    pub fn open_with_retention(
        path: impl AsRef<Path>,
        retention_days: u64,
    ) -> Result<Self, MemoryError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            security::ensure_private_directory(parent)?;
        }
        let today = current_day();
        let _guard = security::lock_path(&path, true)?;
        if path.exists() {
            security::protect_file(&path)?;
        }
        let records = read_records_unlocked(&path)?;
        prune_records(
            &path,
            records,
            retention_days,
            today.saturating_mul(SECONDS_PER_DAY),
        )?;
        Ok(Self {
            path,
            retention_days,
            last_prune_day: today,
        })
    }

    pub fn append(
        &mut self,
        user_id: &str,
        role: &str,
        text: &str,
        feedback: Option<&str>,
    ) -> Result<ConversationLogRecord, MemoryError> {
        let _guard = security::lock_path(&self.path, true)?;
        let existing = read_records_unlocked(&self.path)?;
        let turn_id = existing
            .iter()
            .map(|record| record.turn_id)
            .max()
            .map_or(0, |max_id| max_id.saturating_add(1));
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let record = ConversationLogRecord {
            turn_id,
            user_id: security::redact(user_id),
            role: role.to_owned(),
            text: security::redact(text),
            timestamp,
            feedback: feedback.map(security::redact),
            schema_version: 1,
        };
        let mut file = security::private_append_file(&self.path)?;
        let serialized = serde_json::to_vec(&record)?;
        let line = security::seal_line(&serialized)
            .map_err(|error| MemoryError::Storage(error.to_string()))?;
        writeln!(file, "{line}")?;
        file.sync_data()?;

        let today = timestamp / SECONDS_PER_DAY;
        if today != self.last_prune_day {
            let records = read_records_unlocked(&self.path)?;
            prune_records(&self.path, records, self.retention_days, timestamp)?;
            self.last_prune_day = today;
        }
        Ok(record)
    }

    pub fn compact(&self, keep: usize) -> Result<(), MemoryError> {
        let _guard = security::lock_path(&self.path, true)?;
        let records = read_records_unlocked(&self.path)?;
        let start = records.len().saturating_sub(keep);
        write_records_unlocked(&self.path, &records[start..])
    }

    pub fn backup(&self, destination: impl AsRef<Path>) -> Result<(), MemoryError> {
        let destination = destination.as_ref();
        let _guard = security::lock_path(&self.path, false)?;
        if let Some(parent) = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            security::ensure_private_directory(parent)?;
        }
        fs::copy(&self.path, destination)?;
        security::protect_file(destination)?;
        Ok(())
    }

    pub fn similar_cases(
        &self,
        user_id: &str,
        query: &str,
        max: usize,
    ) -> Result<Vec<ConversationLogRecord>, MemoryError> {
        let _guard = security::lock_path(&self.path, false)?;
        let normalized_user = security::redact(user_id);
        let terms = query
            .to_lowercase()
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut scored = read_records_unlocked(&self.path)?
            .into_iter()
            .filter(|record| record.role == "reasoning" && record.user_id == normalized_user)
            .map(|record| {
                let normalized_text = record.text.to_lowercase();
                let score = terms
                    .iter()
                    .filter(|term| normalized_text.contains(term.as_str()))
                    .count();
                (score, record)
            })
            .filter(|(score, _)| *score > 0)
            .collect::<Vec<_>>();
        scored.sort_by_key(|entry| std::cmp::Reverse(entry.0));
        Ok(scored
            .into_iter()
            .take(max)
            .map(|(_, record)| record)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "vark-log-{label}-{}-{}.jsonl",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn history_is_scoped_by_user_and_secrets_are_redacted() {
        let path = temp_path("scope");
        let mut log = ConversationLog::open(&path).unwrap();
        log.append(
            "alice",
            "reasoning",
            "fakta keyword alice@example.com",
            None,
        )
        .unwrap();
        log.append("bob", "reasoning", "fakta keyword berbeda", None)
            .unwrap();
        let cases = log.similar_cases("alice", "keyword", 10).unwrap();
        assert_eq!(cases.len(), 1);
        assert!(!cases[0].text.contains("alice@example.com"));
        assert_eq!(
            log.similar_cases("mallory", "keyword", 10).unwrap().len(),
            0
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn corrupted_log_is_rejected_during_open() {
        let path = temp_path("corrupt");
        fs::write(&path, "not-json\n").unwrap();
        assert!(matches!(
            ConversationLog::open(&path),
            Err(MemoryError::CorruptRecord { .. })
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn retention_prunes_records_older_than_configured_days() {
        let path = temp_path("retention");
        let expired = ConversationLogRecord {
            turn_id: 0,
            user_id: "alice".into(),
            role: "user".into(),
            text: "lama".into(),
            timestamp: 1,
            feedback: None,
            schema_version: 1,
        };
        fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&expired).unwrap()),
        )
        .unwrap();
        let log = ConversationLog::open_with_retention(&path, 1).unwrap();
        assert!(log.similar_cases("alice", "lama", 1).unwrap().is_empty());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn concurrent_handles_keep_every_turn_and_allocate_unique_ids() {
        let path = temp_path("concurrency");
        let workers = (0..4)
            .map(|worker| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let mut log = ConversationLog::open(&path).unwrap();
                    for turn in 0..5 {
                        log.append(
                            &format!("user-{worker}"),
                            "reasoning",
                            &format!("fact-{worker}-{turn}"),
                            None,
                        )
                        .unwrap();
                    }
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap();
        }
        let _guard = security::lock_path(&path, false).unwrap();
        let records = read_records_unlocked(&path).unwrap();
        let turn_ids = records
            .iter()
            .map(|record| record.turn_id)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(records.len(), 20);
        assert_eq!(turn_ids.len(), records.len());
        fs::remove_file(path).unwrap();
    }
}
