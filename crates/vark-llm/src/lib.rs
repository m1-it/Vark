use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub system: String,
    pub user: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredReply {
    pub intent: String,
    pub entities: Vec<String>,
    pub claims: Vec<String>,
    pub is_question: bool,
    pub answer: String,
}
#[derive(Debug, Error)]
pub enum LlmError {
    #[error("llm http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("llm json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("llm endpoint/model belum dikonfigurasi")]
    NotConfigured,
    #[error("format respons LLM tidak lengkap")]
    InvalidResponse,
    #[error("batas token harian LLM tercapai")]
    BudgetExceeded,
}
pub trait LanguageModel: Send + Sync {
    fn complete(&self, request: &LlmRequest) -> Result<StructuredReply, LlmError>;
}
#[derive(Debug, Clone)]
pub struct OpenAiCompatible {
    pub endpoint: String,
    pub model: String,
    pub timeout_ms: u64,
    pub temperature: f32,
    pub api_key: Option<String>,
}
#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    temperature: f32,
    response_format: Format,
}
#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}
#[derive(Serialize)]
struct Format {
    #[serde(rename = "type")]
    kind: &'static str,
}
#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}
#[derive(Deserialize)]
struct Choice {
    message: MessageOwned,
}
#[derive(Deserialize)]
struct MessageOwned {
    content: String,
}
impl OpenAiCompatible {
    pub fn from_config(c: &vark_config::VarkConfig) -> Option<Self> {
        Some(Self {
            endpoint: c.llm_endpoint.clone()?,
            model: c.llm_model.clone()?,
            timeout_ms: c.llm_timeout_ms,
            temperature: c.llm_temperature,
            api_key: std::env::var("VARK_LEGACY_LLM_API_KEY").ok(),
        })
    }
    pub async fn complete_async(&self, r: &LlmRequest) -> Result<StructuredReply, LlmError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .build()?;
        let prompt = format!("{}\n\nBalas JSON valid dengan field intent (string), entities (array string), claims (array string), is_question (boolean), answer (string).\n{}", r.system, r.user);
        let body = ChatRequest {
            model: &self.model,
            messages: vec![Message {
                role: "user",
                content: &prompt,
            }],
            temperature: self.temperature,
            response_format: Format {
                kind: "json_object",
            },
        };
        let mut req = client.post(&self.endpoint).json(&body);
        if let Some(k) = &self.api_key {
            req = req.bearer_auth(k);
        }
        let response: ChatResponse = req.send().await?.error_for_status()?.json().await?;
        parse_chat_response(response)
    }
}
impl LanguageModel for OpenAiCompatible {
    fn complete(&self, r: &LlmRequest) -> Result<StructuredReply, LlmError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .build()?;
        let prompt=format!("{}\n\nBalas JSON valid dengan field intent (string), entities (array string), claims (array string), is_question (boolean), answer (string). Jawaban hanya berdasarkan bukti. Jika bukti kosong, answer harus 'Saya tidak tahu'.\n{}",r.system,r.user);
        let body = ChatRequest {
            model: &self.model,
            messages: vec![Message {
                role: "user",
                content: &prompt,
            }],
            temperature: self.temperature,
            response_format: Format {
                kind: "json_object",
            },
        };
        let mut req = client.post(&self.endpoint).json(&body);
        if let Some(k) = &self.api_key {
            req = req.bearer_auth(k)
        }
        let response: ChatResponse = req.send()?.error_for_status()?.json()?;
        parse_chat_response(response)
    }
}

impl OpenAiCompatible {
    pub fn from_backend(c: &vark_config::LlmBackendConfig) -> Self {
        Self {
            endpoint: c.endpoint.clone(),
            model: c.model.clone(),
            timeout_ms: c.timeout_ms,
            temperature: c.temperature,
            api_key: std::env::var(&c.api_key_env).ok(),
        }
    }
}

fn parse_chat_response(response: ChatResponse) -> Result<StructuredReply, LlmError> {
    let content = response
        .choices
        .first()
        .ok_or(LlmError::InvalidResponse)?
        .message
        .content
        .as_str();
    Ok(serde_json::from_str(content)?)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageStats {
    pub student_calls: u64,
    pub teacher_calls: u64,
    pub student_tokens: u64,
    pub teacher_tokens: u64,
    pub estimated_cost: f64,
    pub day: u64,
}
#[derive(Clone)]
pub struct TeacherStudentRouter {
    pub student: Option<OpenAiCompatible>,
    pub teacher: Option<OpenAiCompatible>,
    student_cfg: Option<vark_config::LlmBackendConfig>,
    teacher_cfg: Option<vark_config::LlmBackendConfig>,
    cache: Arc<Mutex<HashMap<String, StructuredReply>>>,
    usage: Arc<Mutex<UsageStats>>,
    daily_limit: u64,
}
impl TeacherStudentRouter {
    pub fn from_config(c: &vark_config::VarkConfig) -> Self {
        Self {
            student: c.student.as_ref().map(OpenAiCompatible::from_backend),
            teacher: c.teacher.as_ref().map(OpenAiCompatible::from_backend),
            student_cfg: c.student.clone(),
            teacher_cfg: c.teacher.clone(),
            cache: Arc::new(Mutex::new(HashMap::new())),
            usage: Arc::new(Mutex::new(UsageStats {
                day: current_day(),
                ..UsageStats::default()
            })),
            daily_limit: c.daily_token_limit,
        }
    }
    pub fn stats(&self) -> UsageStats {
        self.usage.lock().map(|x| x.clone()).unwrap_or_default()
    }
    pub async fn call_student(&self, request: &LlmRequest) -> Result<StructuredReply, LlmError> {
        self.call(
            "student",
            self.student.as_ref(),
            self.student_cfg.as_ref(),
            request,
        )
        .await
    }
    pub async fn call_teacher(&self, request: &LlmRequest) -> Result<StructuredReply, LlmError> {
        self.call(
            "teacher",
            self.teacher.as_ref(),
            self.teacher_cfg.as_ref(),
            request,
        )
        .await
    }
    async fn call(
        &self,
        role: &str,
        backend: Option<&OpenAiCompatible>,
        cfg: Option<&vark_config::LlmBackendConfig>,
        request: &LlmRequest,
    ) -> Result<StructuredReply, LlmError> {
        let backend = backend.ok_or(LlmError::NotConfigured)?;
        let key = cache_key(role, request);
        if let Ok(cache) = self.cache.lock() {
            if let Some(reply) = cache.get(&key) {
                return Ok(reply.clone());
            }
        }
        let estimated = ((request.system.len() + request.user.len()) as u64 / 4).max(1);
        if let Ok(mut usage) = self.usage.lock() {
            if usage.day != current_day() {
                *usage = UsageStats {
                    day: current_day(),
                    ..UsageStats::default()
                };
            }
            if usage.student_tokens + usage.teacher_tokens + estimated > self.daily_limit {
                return Err(LlmError::BudgetExceeded);
            }
        }
        let retries = cfg.map(|x| x.max_retries).unwrap_or(0);
        let mut last = LlmError::InvalidResponse;
        for _ in 0..=retries {
            match backend.complete_async(request).await {
                Ok(reply) => {
                    let tokens = estimated + reply.answer.len() as u64 / 4;
                    if let Ok(mut cache) = self.cache.lock() {
                        cache.insert(key.clone(), reply.clone());
                    }
                    if let Ok(mut usage) = self.usage.lock() {
                        if role == "teacher" {
                            usage.teacher_calls += 1;
                            usage.teacher_tokens += tokens;
                        } else {
                            usage.student_calls += 1;
                            usage.student_tokens += tokens;
                        }
                        usage.estimated_cost += cfg.map(|x| x.cost_per_1k_tokens).unwrap_or(0.0)
                            * tokens as f64
                            / 1000.0;
                    }
                    return Ok(reply);
                }
                Err(error) => last = error,
            }
        }
        Err(last)
    }
}
fn current_day() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0)
}

fn cache_key(role: &str, request: &LlmRequest) -> String {
    vark_hashing::sha256_hex(format!("{role}\0{}\0{}", request.system, request.user).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_response_parser_requires_choice_and_complete_reply() {
        let response: ChatResponse = serde_json::from_str(
            r#"{"choices":[{"message":{"content":"{\"intent\":\"answer\",\"entities\":[\"Rust\"],\"claims\":[\"Rust adalah bahasa sistem\"],\"is_question\":false,\"answer\":\"Rust adalah bahasa sistem\"}"}}]}"#,
        )
        .unwrap();
        let parsed = parse_chat_response(response).unwrap();
        assert_eq!(parsed.intent, "answer");
        assert_eq!(parsed.claims.len(), 1);
        assert_eq!(parsed.answer, "Rust adalah bahasa sistem");

        let empty: ChatResponse = serde_json::from_str(r#"{"choices":[]}"#).unwrap();
        assert!(matches!(
            parse_chat_response(empty),
            Err(LlmError::InvalidResponse)
        ));

        let incomplete: ChatResponse = serde_json::from_str(
            r#"{"choices":[{"message":{"content":"{\"answer\":\"jawaban\"}"}}]}"#,
        )
        .unwrap();
        assert!(matches!(
            parse_chat_response(incomplete),
            Err(LlmError::Json(_))
        ));
    }

    #[test]
    fn cache_key_is_stable_role_scoped_and_does_not_expose_prompt() {
        let request = LlmRequest {
            system: "jangan simpan prompt ini".into(),
            user: "secret content".into(),
        };
        let student_key = cache_key("student", &request);
        assert_eq!(student_key, cache_key("student", &request));
        assert_ne!(student_key, cache_key("teacher", &request));
        assert!(!student_key.contains("secret content"));
        assert_eq!(student_key.len(), 64);
    }
}
