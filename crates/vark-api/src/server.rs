use crate::{Engine, InferenceRequest};
use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use vark_config::VarkConfig;

struct ServerState {
    config: VarkConfig,
    engines: HashMap<String, Engine>,
}
pub fn router(engine: Engine) -> Router {
    let user_id = engine.config.user_id.clone();
    let config = engine.config.clone();
    let mut engines = HashMap::new();
    engines.insert(user_id, engine);
    Router::new()
        .route("/health", get(health))
        .route("/infer", post(infer))
        .route("/chat", post(chat))
        .with_state(Arc::new(Mutex::new(ServerState { config, engines })))
}
async fn health() -> impl IntoResponse {
    Json(json!({"status":"ok","service":"vark"}))
}
fn user_config(base: &VarkConfig, user_id: &str) -> VarkConfig {
    let mut config = base.clone();
    let safe = user_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    config.user_id = user_id.into();
    config.memory_db = format!("{}.users/{}.jsonl", base.memory_db, safe);
    config.checkpoint_dir = format!("{}/users/{}", base.checkpoint_dir, safe);
    config
}
async fn infer(
    State(state): State<Arc<Mutex<ServerState>>>,
    Json(req): Json<InferenceRequest>,
) -> impl IntoResponse {
    let mut state = state.lock().await;
    let user_id = req
        .user_id
        .clone()
        .unwrap_or_else(|| state.config.user_id.clone());
    if !state.engines.contains_key(&user_id) {
        match Engine::new(user_config(&state.config, &user_id)) {
            Ok(engine) => {
                state.engines.insert(user_id.clone(), engine);
            }
            Err(error) => {
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": error.to_string()})),
                )
            }
        }
    }
    let engine = state.engines.get_mut(&user_id).expect("inserted");
    match engine.infer(&req.text) {
        Ok(out) => (
            axum::http::StatusCode::OK,
            Json(serde_json::to_value(out).unwrap()),
        ),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":e.to_string()})),
        ),
    }
}
async fn chat(
    State(state): State<Arc<Mutex<ServerState>>>,
    Json(req): Json<InferenceRequest>,
) -> impl IntoResponse {
    let mut state = state.lock().await;
    let user_id = req
        .user_id
        .clone()
        .unwrap_or_else(|| state.config.user_id.clone());
    if !state.engines.contains_key(&user_id) {
        match Engine::new(user_config(&state.config, &user_id)) {
            Ok(engine) => {
                state.engines.insert(user_id.clone(), engine);
            }
            Err(error) => {
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": error.to_string()})),
                )
            }
        }
    }
    let engine = state.engines.get_mut(&user_id).expect("inserted");
    match engine.chat_async(&req.text).await {
        Ok(out) => (
            axum::http::StatusCode::OK,
            Json(serde_json::to_value(out).unwrap()),
        ),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":e.to_string()})),
        ),
    }
}
