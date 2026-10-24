use serde::{Deserialize, Serialize};
use thiserror::Error;
use vark_memory::MemoryError;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sentence {
    pub text: String,
    pub tokens: Vec<String>,
    pub source: String,
    pub confidence: f32,
    #[serde(default)]
    pub status: FactStatus,
    #[serde(default)]
    pub confirmations: u32,
    #[serde(default)]
    pub supersedes: Option<String>,
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub parent_id: Option<u64>,
    #[serde(default)]
    pub node_kind: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum FactStatus {
    #[default]
    Claimed,
    Tentative,
    Confirmed,
    Refuted,
    Retracted,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub user: String,
    pub assistant: String,
    pub evidence: Vec<Sentence>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    pub text: String,
    pub confidence: f32,
    pub grounded: bool,
    pub evidence: Vec<Sentence>,
    pub reasoning: Vec<String>,
    pub spike_count: usize,
}
#[derive(Debug, Error)]
pub enum ConversationError {
    #[error("memori percakapan: {0}")]
    Memory(#[from] MemoryError),
    #[error("llm: {0}")]
    Llm(String),
    #[error("checkpoint: {0}")]
    Checkpoint(String),
}
