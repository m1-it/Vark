use anyhow::Result;
use serde::{Deserialize, Serialize};
use vark_config::VarkConfig;
use vark_conversation::{Answer, Conversation};
use vark_llm::UsageStats;
use vark_memory::EpisodicMemory;
use vark_tokenizer::Tokenizer;
mod auth;
pub mod server;
pub use auth::{ApiAuth, ApiAuthError};
#[derive(Debug, Deserialize)]
pub struct InferenceRequest {
    pub text: String,
    #[serde(default)]
    pub user_id: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct InferenceResponse {
    pub text: String,
    pub token_ids: Vec<u32>,
    pub spikes: Vec<bool>,
    pub memory_id: u64,
    pub grounded: bool,
    pub confidence: f32,
}
pub struct Engine {
    pub config: VarkConfig,
    pub conversation: Conversation,
    pub tokenizer: Tokenizer,
}
impl Engine {
    pub fn new(config: VarkConfig) -> Result<Self> {
        let memory = EpisodicMemory::open(&config.memory_db)?;
        let conversation = Conversation::from_config(memory, &config)?;
        Ok(Self {
            tokenizer: Tokenizer::new(config.vocab_size),
            conversation,
            config,
        })
    }
    pub fn chat(&mut self, text: &str) -> Result<Answer> {
        Ok(self.conversation.respond(text)?)
    }
    pub fn set_user_id(&mut self, user_id: impl Into<String>) {
        self.conversation.set_user_id(user_id);
    }
    pub fn routing_stats(&self) -> Option<UsageStats> {
        self.conversation.routing_stats()
    }
    pub async fn chat_async(&mut self, text: &str) -> Result<Answer> {
        Ok(self.conversation.respond_async(text).await?)
    }
    pub fn infer(&mut self, text: &str) -> Result<InferenceResponse> {
        let ids = self.tokenizer.encode(text);
        let answer = self.chat(text)?;
        let memory_id = self
            .conversation
            .memory
            .load_records()?
            .last()
            .map(|r| r.id)
            .unwrap_or(0);
        let spikes = vec![answer.spike_count > 0];
        Ok(InferenceResponse {
            text: answer.text,
            token_ids: ids,
            spikes,
            memory_id,
            grounded: answer.grounded,
            confidence: answer.confidence,
        })
    }
}
