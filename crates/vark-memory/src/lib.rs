use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use vark_hashing::{hamming_distance, sha256_hex, simhash};
pub mod semantic;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryKind {
    Fact,
    Dialogue,
}
impl Default for MemoryKind {
    fn default() -> Self {
        Self::Fact
    }
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
}
pub struct EpisodicMemory {
    path: PathBuf,
    next_id: u64,
}
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
    next_id: u64,
}
impl ConversationLog {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MemoryError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let next_id = if path.exists() {
            BufReader::new(File::open(&path)?).lines().count() as u64
        } else {
            0
        };
        Ok(Self { path, next_id })
    }
    pub fn append(
        &mut self,
        user_id: &str,
        role: &str,
        text: &str,
        feedback: Option<&str>,
    ) -> Result<ConversationLogRecord, MemoryError> {
        let record = ConversationLogRecord {
            turn_id: self.next_id,
            user_id: redact(user_id),
            role: role.into(),
            text: redact(text),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            feedback: feedback.map(redact),
            schema_version: 1,
        };
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{}", serde_json::to_string(&record)?)?;
        file.sync_data()?;
        self.next_id += 1;
        Ok(record)
    }
    pub fn compact(&self, keep: usize) -> Result<(), MemoryError> {
        let lines = BufReader::new(File::open(&self.path)?)
            .lines()
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        let tmp = self.path.with_extension("compact");
        let mut file = File::create(&tmp)?;
        for line in lines
            .into_iter()
            .rev()
            .take(keep)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            writeln!(file, "{}", line)?;
        }
        file.sync_all()?;
        fs::rename(tmp, &self.path)?;
        Ok(())
    }
    pub fn backup(&self, destination: impl AsRef<Path>) -> Result<(), MemoryError> {
        if let Some(parent) = destination.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&self.path, destination)?;
        Ok(())
    }
    pub fn similar_cases(
        &self,
        query: &str,
        max: usize,
    ) -> Result<Vec<ConversationLogRecord>, MemoryError> {
        let terms = query
            .to_ascii_lowercase()
            .split_whitespace()
            .map(String::from)
            .collect::<Vec<_>>();
        let mut scored = BufReader::new(File::open(&self.path)?)
            .lines()
            .filter_map(Result::ok)
            .filter_map(|line| serde_json::from_str::<ConversationLogRecord>(&line).ok())
            .filter(|r| r.role == "reasoning")
            .map(|r| {
                let score = terms
                    .iter()
                    .filter(|t| r.text.to_ascii_lowercase().contains(t.as_str()))
                    .count();
                (score, r)
            })
            .filter(|(score, _)| *score > 0)
            .collect::<Vec<_>>();
        scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        Ok(scored.into_iter().take(max).map(|(_, r)| r).collect())
    }
}
fn redact(input: &str) -> String {
    input
        .split_whitespace()
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            if word.contains('@') {
                "[EMAIL]".into()
            } else if word.chars().filter(|c| c.is_ascii_digit()).count() >= 7 {
                "[PHONE_OR_KEY]".into()
            } else if lower.contains("api_key") || lower.contains("bearer") {
                "[SECRET]".into()
            } else {
                word.into()
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}
fn norm(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
impl EpisodicMemory {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MemoryError> {
        let p = path.as_ref().to_path_buf();
        if let Some(d) = p.parent() {
            fs::create_dir_all(d)?
        }
        let mut next = 0;
        if p.exists() {
            for line in BufReader::new(File::open(&p)?).lines() {
                if let Ok(l) = line {
                    if let Ok(r) = serde_json::from_str::<MemoryRecord>(&l) {
                        next = next.max(r.id + 1)
                    }
                }
            }
        }
        Ok(Self {
            path: p,
            next_id: next,
        })
    }
    pub fn load_records(&self) -> Result<Vec<MemoryRecord>, MemoryError> {
        if !self.path.exists() {
            return Ok(vec![]);
        }
        let mut out = vec![];
        for line in BufReader::new(File::open(&self.path)?).lines() {
            let l = line?;
            if let Ok(r) = serde_json::from_str::<MemoryRecord>(&l) {
                if sha256_hex(norm(&r.text).as_bytes()) == r.hash {
                    out.push(r)
                }
            }
        }
        Ok(out)
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
        let clean = norm(text);
        let r = MemoryRecord {
            id: self.next_id,
            text: text.into(),
            hash: sha256_hex(clean.as_bytes()),
            signature: simhash(&clean.split_whitespace().collect::<Vec<_>>()),
            salience: salience.clamp(0.0, 1.0),
            kind,
            source: source.into(),
            fact_id: None,
            parent_id: None,
            node_kind: None,
        };
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(f, "{}", serde_json::to_string(&r)?)?;
        f.sync_data()?;
        self.next_id += 1;
        Ok(r)
    }
    pub fn append_fact(
        &mut self,
        text: &str,
        salience: f32,
        source: &str,
        parent_id: Option<u64>,
        node_kind: &str,
    ) -> Result<MemoryRecord, MemoryError> {
        let mut record = self.append_kind(text, salience, MemoryKind::Fact, source)?;
        record.fact_id = Some(record.id);
        record.parent_id = parent_id;
        record.node_kind = Some(node_kind.into());
        let mut lines = self.load_records()?;
        if let Some(last) = lines.iter_mut().find(|r| r.id == record.id) {
            *last = record.clone();
        }
        let tmp = self.path.with_extension("rewrite");
        let mut f = File::create(&tmp)?;
        for item in lines {
            writeln!(f, "{}", serde_json::to_string(&item)?)?;
        }
        f.sync_all()?;
        fs::rename(tmp, &self.path)?;
        Ok(record)
    }
    pub fn search(&self, query: &str, max_distance: u32) -> Result<Vec<MemoryRecord>, MemoryError> {
        let q = norm(query);
        let sig = simhash(&q.split_whitespace().collect::<Vec<_>>());
        Ok(self
            .load_records()?
            .into_iter()
            .filter(|r| hamming_distance(sig, r.signature) <= max_distance)
            .collect())
    }
    pub fn snapshot(&self, dest: impl AsRef<Path>) -> Result<(), MemoryError> {
        let dest = dest.as_ref();
        if let Some(d) = dest.parent() {
            fs::create_dir_all(d)?
        }
        let tmp = dest.with_extension("tmp");
        let records = self.load_records()?;
        let mut f = File::create(&tmp)?;
        for r in records {
            writeln!(f, "{}", serde_json::to_string(&r)?)?
        }
        f.sync_all()?;
        fs::rename(tmp, dest)?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persists_recover_snapshot() {
        let p = std::env::temp_dir().join("vark_memory_test.jsonl");
        let s = std::env::temp_dir().join("vark_memory_snapshot.jsonl");
        let _ = fs::remove_file(&p);
        let _ = fs::remove_file(&s);
        let mut m = EpisodicMemory::open(&p).unwrap();
        m.append("Neuron Aktif!", 1.0).unwrap();
        assert_eq!(m.search("neuron aktif", 0).unwrap().len(), 1);
        m.snapshot(&s).unwrap();
        assert_eq!(
            EpisodicMemory::open(&p)
                .unwrap()
                .load_records()
                .unwrap()
                .len(),
            1
        );
    }
}
