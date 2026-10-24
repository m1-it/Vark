use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::fs;
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;
use vark_hashing::{hamming_distance, sha256_hex, simhash};

mod conversation_log;
mod security;
pub mod semantic;
pub use conversation_log::{ConversationLog, ConversationLogRecord};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum MemoryKind {
    #[default]
    Fact,
    Dialogue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: u64,
    pub text: String,
    pub hash: String,
    pub signature: u64,
    pub salience: f32,
    #[serde(default)]
    pub kind: MemoryKind,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub fact_id: Option<u64>,
    #[serde(default)]
    pub parent_id: Option<u64>,
    #[serde(default)]
    pub node_kind: Option<String>,
}

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("I/O memori: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON memori: {0}")]
    Json(#[from] serde_json::Error),
    #[error("record memori rusak pada baris {line}: {reason}")]
    CorruptRecord { line: usize, reason: String },
    #[error("proteksi storage: {0}")]
    Storage(String),
}

pub struct EpisodicMemory {
    path: PathBuf,
}

struct AppendMetadata {
    kind: MemoryKind,
    source: String,
    is_fact: bool,
    parent_id: Option<u64>,
    node_kind: Option<String>,
}

fn read_records_unlocked(path: &Path) -> Result<Vec<MemoryRecord>, MemoryError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut records = Vec::new();
    for (index, line) in BufReader::new(File::open(path)?).lines().enumerate() {
        let line_number = index + 1;
        let line = line?;
        let plaintext =
            security::open_line(&line).map_err(|error| MemoryError::Storage(error.to_string()))?;
        let record = serde_json::from_slice::<MemoryRecord>(&plaintext).map_err(|error| {
            MemoryError::CorruptRecord {
                line: line_number,
                reason: error.to_string(),
            }
        })?;
        if sha256_hex(norm(&record.text).as_bytes()) != record.hash {
            return Err(MemoryError::CorruptRecord {
                line: line_number,
                reason: "checksum teks tidak cocok".into(),
            });
        }
        records.push(record);
    }
    Ok(records)
}

fn write_records_unlocked(path: &Path, records: &[MemoryRecord]) -> Result<(), MemoryError> {
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

fn norm(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || character.is_whitespace() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn redact_sensitive_text(text: &str) -> String {
    security::redact(text)
}

impl EpisodicMemory {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MemoryError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            security::ensure_private_directory(parent)?;
        }
        let _guard = security::lock_path(&path, true)?;
        if path.exists() {
            security::protect_file(&path)?;
        }
        let records = read_records_unlocked(&path)?;
        if security::encryption_configured() && path.exists() {
            let lines = BufReader::new(File::open(&path)?)
                .lines()
                .collect::<Result<Vec<_>, _>>()?;
            let has_plaintext = lines.iter().any(|line| !security::is_encrypted_line(line));
            if has_plaintext {
                write_records_unlocked(&path, &records)?;
            }
        }
        Ok(Self { path })
    }

    pub fn load_records(&self) -> Result<Vec<MemoryRecord>, MemoryError> {
        let _guard = security::lock_path(&self.path, false)?;
        read_records_unlocked(&self.path)
    }

    pub fn append(&mut self, text: &str, salience: f32) -> Result<MemoryRecord, MemoryError> {
        self.append_kind(text, salience, MemoryKind::Fact, "unknown")
    }

    pub fn append_kind(
        &mut self,
        text: &str,
        salience: f32,
        kind: MemoryKind,
        source: &str,
    ) -> Result<MemoryRecord, MemoryError> {
        self.append_record(
            text,
            salience,
            AppendMetadata {
                kind,
                source: source.to_owned(),
                is_fact: false,
                parent_id: None,
                node_kind: None,
            },
        )
    }

    pub fn append_fact(
        &mut self,
        text: &str,
        salience: f32,
        source: &str,
        parent_id: Option<u64>,
        node_kind: &str,
    ) -> Result<MemoryRecord, MemoryError> {
        self.append_record(
            text,
            salience,
            AppendMetadata {
                kind: MemoryKind::Fact,
                source: source.to_owned(),
                is_fact: true,
                parent_id,
                node_kind: Some(node_kind.to_owned()),
            },
        )
    }

    fn append_record(
        &mut self,
        text: &str,
        salience: f32,
        metadata: AppendMetadata,
    ) -> Result<MemoryRecord, MemoryError> {
        let _guard = security::lock_path(&self.path, true)?;
        let existing = read_records_unlocked(&self.path)?;
        let id = existing
            .iter()
            .map(|record| record.id)
            .max()
            .map_or(0, |max_id| max_id.saturating_add(1));
        let text = security::redact(text);
        let clean = norm(&text);
        let effective_salience = if salience.is_finite() {
            salience.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let record = MemoryRecord {
            id,
            hash: sha256_hex(clean.as_bytes()),
            signature: simhash(&clean.split_whitespace().collect::<Vec<_>>()),
            text,
            salience: effective_salience,
            kind: metadata.kind,
            source: security::redact(&metadata.source),
            fact_id: metadata.is_fact.then_some(id),
            parent_id: metadata.parent_id,
            node_kind: metadata.node_kind,
        };
        let mut file = security::private_append_file(&self.path)?;
        let serialized = serde_json::to_vec(&record)?;
        let line = security::seal_line(&serialized)
            .map_err(|error| MemoryError::Storage(error.to_string()))?;
        writeln!(file, "{line}")?;
        file.sync_data()?;
        Ok(record)
    }

    pub fn search(&self, query: &str, max_distance: u32) -> Result<Vec<MemoryRecord>, MemoryError> {
        let query = norm(query);
        let signature = simhash(&query.split_whitespace().collect::<Vec<_>>());
        Ok(self
            .load_records()?
            .into_iter()
            .filter(|record| hamming_distance(signature, record.signature) <= max_distance)
            .collect())
    }

    pub fn snapshot(&self, destination: impl AsRef<Path>) -> Result<(), MemoryError> {
        let destination = destination.as_ref();
        let _guard = security::lock_path(&self.path, false)?;
        let records = read_records_unlocked(&self.path)?;
        write_records_unlocked(destination, &records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "vark-memory-{label}-{}-{}.jsonl",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn persists_recovers_and_snapshots_records() {
        let path = temp_path("snapshot-source");
        let snapshot = temp_path("snapshot-copy");
        let mut memory = EpisodicMemory::open(&path).unwrap();
        memory.append("Neuron Aktif!", 1.0).unwrap();
        assert_eq!(memory.search("neuron aktif", 0).unwrap().len(), 1);
        memory.snapshot(&snapshot).unwrap();
        assert_eq!(
            EpisodicMemory::open(&path)
                .unwrap()
                .load_records()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            EpisodicMemory::open(&snapshot)
                .unwrap()
                .load_records()
                .unwrap()
                .len(),
            1
        );
        fs::remove_file(path).unwrap();
        fs::remove_file(snapshot).unwrap();
    }

    #[test]
    fn append_fact_writes_metadata_in_one_record_and_redacts_sensitive_text() {
        let path = temp_path("fact");
        let mut memory = EpisodicMemory::open(&path).unwrap();
        let record = memory
            .append_fact(
                "Kontak alice@example.com password: rahasia token=abc123456789",
                0.9,
                "user input",
                Some(7),
                "fact",
            )
            .unwrap();
        assert_eq!(record.fact_id, Some(record.id));
        assert_eq!(record.parent_id, Some(7));
        assert!(!record.text.contains("alice@example.com"));
        assert!(!record.text.contains("rahasia"));
        assert!(!record.text.contains("abc123456789"));
        assert_eq!(memory.load_records().unwrap().len(), 1);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn corruption_is_reported_and_never_dropped_by_a_later_append() {
        let path = temp_path("corruption");
        fs::write(&path, "{not valid json}\n").unwrap();
        assert!(matches!(
            EpisodicMemory::open(&path),
            Err(MemoryError::CorruptRecord { .. })
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn concurrent_handles_allocate_unique_ids_without_lost_appends() {
        let path = temp_path("concurrency");
        let workers = (0..4)
            .map(|worker| {
                let path = path.clone();
                std::thread::spawn(move || {
                    for index in 0..8 {
                        EpisodicMemory::open(&path)
                            .unwrap()
                            .append(&format!("worker {worker} record {index}"), 0.5)
                            .unwrap();
                    }
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap();
        }
        let records = EpisodicMemory::open(&path).unwrap().load_records().unwrap();
        assert_eq!(records.len(), 32);
        let ids = records
            .iter()
            .map(|record| record.id)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(ids.len(), records.len());
        fs::remove_file(path).unwrap();
    }
}
