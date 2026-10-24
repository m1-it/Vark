use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarkConfig {
    pub seed: u64,
    pub neuron_count: usize,
    pub input_size: usize,
    pub vocab_size: usize,
    pub simulation_steps: usize,
    pub threshold: f32,
    pub tau: f32,
    pub refractory_steps: u32,
    pub checkpoint_dir: String,
    pub memory_db: String,
    #[serde(default)]
    pub llm_endpoint: Option<String>,
    #[serde(default)]
    pub llm_model: Option<String>,
    #[serde(default = "default_llm_timeout")]
    pub llm_timeout_ms: u64,
    #[serde(default = "default_llm_temperature")]
    pub llm_temperature: f32,
    #[serde(default = "default_user_id")]
    pub user_id: String,
    #[serde(default)]
    pub teacher: Option<LlmBackendConfig>,
    #[serde(default)]
    pub student: Option<LlmBackendConfig>,
    #[serde(default = "default_daily_tokens")]
    pub daily_token_limit: u64,
    #[serde(default = "default_teacher_threshold")]
    pub teacher_confidence_threshold: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmBackendConfig {
    pub endpoint: String,
    pub model: String,
    pub api_key_env: String,
    #[serde(default = "default_llm_timeout")]
    pub timeout_ms: u64,
    #[serde(default = "default_llm_temperature")]
    pub temperature: f32,
    #[serde(default = "default_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub cost_per_1k_tokens: f64,
}
fn default_daily_tokens() -> u64 {
    100_000
}
fn default_teacher_threshold() -> f32 {
    0.65
}
fn default_retries() -> u32 {
    2
}
fn default_llm_timeout() -> u64 {
    8_000
}
fn default_llm_temperature() -> f32 {
    0.15
}
fn default_user_id() -> String {
    "local-user".into()
}
impl Default for VarkConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            neuron_count: 1024,
            input_size: 256,
            vocab_size: 8192,
            simulation_steps: 20,
            threshold: 1.0,
            tau: 20.0,
            refractory_steps: 2,
            checkpoint_dir: "checkpoints".into(),
            memory_db: "data/memory.jsonl".into(),
            llm_endpoint: None,
            llm_model: None,
            llm_timeout_ms: default_llm_timeout(),
            llm_temperature: default_llm_temperature(),
            user_id: "local-user".into(),
            teacher: None,
            student: None,
            daily_token_limit: default_daily_tokens(),
            teacher_confidence_threshold: default_teacher_threshold(),
        }
    }
}
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("gagal membaca konfigurasi: {0}")]
    Io(#[from] std::io::Error),
    #[error("format konfigurasi tidak valid: {0}")]
    Json(#[from] serde_json::Error),
}
impl VarkConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
    }
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ConfigError> {
        if let Some(p) = path.as_ref().parent() {
            fs::create_dir_all(p)?;
        }
        fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_json_receives_explicit_runtime_defaults() {
        let mut value = serde_json::to_value(VarkConfig::default()).unwrap();
        let object = value.as_object_mut().unwrap();
        for field in [
            "user_id",
            "teacher",
            "student",
            "daily_token_limit",
            "teacher_confidence_threshold",
            "llm_timeout_ms",
            "llm_temperature",
        ] {
            object.remove(field);
        }
        let config: VarkConfig = serde_json::from_value(value).unwrap();
        assert_eq!(config.user_id, "local-user");
        assert_eq!(config.daily_token_limit, 100_000);
        assert_eq!(config.teacher_confidence_threshold, 0.65);
        assert_eq!(config.llm_timeout_ms, 8_000);
        assert_eq!(config.llm_temperature, 0.15);
        assert!(config.teacher.is_none());
        assert!(config.student.is_none());
    }

    #[test]
    fn save_and_load_preserve_service_configuration() {
        let path = std::env::temp_dir().join(format!("vark-config-{}.json", std::process::id()));
        let config = VarkConfig::default();
        config.save(&path).unwrap();
        let loaded = VarkConfig::load(&path).unwrap();
        assert_eq!(loaded.user_id, config.user_id);
        assert_eq!(loaded.memory_db, config.memory_db);
        assert_eq!(loaded.checkpoint_dir, config.checkpoint_dir);
        assert_eq!(loaded.neuron_count, config.neuron_count);
        assert_eq!(loaded.daily_token_limit, config.daily_token_limit);
        fs::remove_file(path).unwrap();
    }
}
