use crate::{Engine, InferenceRequest, InferenceResponse};
use axum::{
    extract::{DefaultBodyLimit, Extension, State},
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex as StdMutex},
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, RwLock};
use vark_config::VarkConfig;
use vark_hashing::sha256_hex;

use crate::auth::{authenticate, ApiAuth, Principal};

const DEFAULT_MAX_ENGINES: usize = 128;
const DEFAULT_REQUESTS_PER_MINUTE: usize = 60;
const MAX_REQUEST_TEXT_BYTES: usize = 16 * 1024;
const MAX_HTTP_BODY_BYTES: usize = 64 * 1024;

struct ServerState {
    config: VarkConfig,
    engines: RwLock<HashMap<String, Arc<Mutex<Engine>>>>,
    creation_lock: Mutex<()>,
    recent_requests: StdMutex<HashMap<String, VecDeque<Instant>>>,
    max_engines: usize,
    requests_per_minute: usize,
}

pub fn router(engine: Engine, auth: ApiAuth) -> Router {
    router_with_limits(
        engine,
        auth,
        DEFAULT_MAX_ENGINES,
        DEFAULT_REQUESTS_PER_MINUTE,
    )
}

fn router_with_limits(
    engine: Engine,
    auth: ApiAuth,
    max_engines: usize,
    requests_per_minute: usize,
) -> Router {
    let user_id = engine.config.user_id.clone();
    let config = engine.config.clone();
    let mut engines = HashMap::new();
    if auth.authorizes_user_id(&user_id) {
        engines.insert(user_id, Arc::new(Mutex::new(engine)));
    }
    let state = Arc::new(ServerState {
        config,
        engines: RwLock::new(engines),
        creation_lock: Mutex::new(()),
        recent_requests: StdMutex::new(HashMap::new()),
        max_engines: max_engines.max(1),
        requests_per_minute: requests_per_minute.max(1),
    });
    Router::new()
        .route("/health", get(health))
        .route("/infer", post(infer))
        .route("/chat", post(chat))
        .route_layer(middleware::from_fn_with_state(Arc::new(auth), authenticate))
        .layer(DefaultBodyLimit::max(MAX_HTTP_BODY_BYTES))
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    Json(json!({"status":"ok","service":"vark"}))
}

fn user_config(base: &VarkConfig, user_id: &str) -> VarkConfig {
    let mut config = base.clone();
    let user_dir = sha256_hex(user_id.as_bytes());
    config.user_id = user_id.to_owned();
    config.memory_db = format!("{}.users/{user_dir}.jsonl", base.memory_db);
    config.checkpoint_dir = format!("{}/users/{user_dir}", base.checkpoint_dir);
    config
}

impl ServerState {
    fn accept_request(&self, user_id: &str) -> Result<bool, ()> {
        let now = Instant::now();
        let mut requests = self.recent_requests.lock().map_err(|_| ())?;
        let user_requests = requests.entry(user_id.to_owned()).or_default();
        while user_requests
            .front()
            .is_some_and(|seen| now.duration_since(*seen) >= Duration::from_secs(60))
        {
            user_requests.pop_front();
        }
        if user_requests.len() >= self.requests_per_minute {
            return Ok(false);
        }
        user_requests.push_back(now);
        Ok(true)
    }

    async fn engine_for(&self, user_id: &str) -> Result<Arc<Mutex<Engine>>, ApiError> {
        if let Some(engine) = self.engines.read().await.get(user_id).cloned() {
            return Ok(engine);
        }
        let _creation = self.creation_lock.lock().await;
        if let Some(engine) = self.engines.read().await.get(user_id).cloned() {
            return Ok(engine);
        }
        if self.engines.read().await.len() >= self.max_engines {
            return Err(ApiError::capacity());
        }
        let config = user_config(&self.config, user_id);
        let engine = tokio::task::spawn_blocking(move || Engine::new(config))
            .await
            .map_err(|_| ApiError::internal())?
            .map_err(|_| ApiError::internal())?;
        let engine = Arc::new(Mutex::new(engine));
        self.engines
            .write()
            .await
            .insert(user_id.to_owned(), engine.clone());
        Ok(engine)
    }
}

#[derive(Clone, Copy)]
struct ApiError {
    status: StatusCode,
    message: &'static str,
}

impl ApiError {
    fn internal() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "permintaan gagal diproses",
        }
    }

    fn capacity() -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "kapasitas pengguna aktif tercapai",
        }
    }

    fn response(self) -> impl IntoResponse {
        (self.status, Json(json!({"error": self.message})))
    }
}

fn validate_request(
    state: &ServerState,
    principal: &Principal,
    request: &InferenceRequest,
) -> Result<(), ApiError> {
    if request.text.trim().is_empty() || request.text.len() > MAX_REQUEST_TEXT_BYTES {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "text harus berisi 1–16384 byte",
        });
    }
    if request
        .user_id
        .as_deref()
        .is_some_and(|user_id| user_id != principal.user_id)
    {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "token tidak berwenang untuk user_id tersebut",
        });
    }
    match state.accept_request(&principal.user_id) {
        Ok(true) => Ok(()),
        Ok(false) => Err(ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "batas request per menit tercapai",
        }),
        Err(()) => Err(ApiError::internal()),
    }
}

async fn infer(
    State(state): State<Arc<ServerState>>,
    Extension(principal): Extension<Principal>,
    Json(request): Json<InferenceRequest>,
) -> Result<(StatusCode, Json<InferenceResponse>), impl IntoResponse> {
    validate_request(&state, &principal, &request).map_err(ApiError::response)?;
    let engine = state
        .engine_for(&principal.user_id)
        .await
        .map_err(ApiError::response)?;
    let result = engine.lock().await.infer(&request.text);
    match result {
        Ok(response) => Ok((StatusCode::OK, Json(response))),
        Err(_) => Err(ApiError::internal().response()),
    }
}

async fn chat(
    State(state): State<Arc<ServerState>>,
    Extension(principal): Extension<Principal>,
    Json(request): Json<InferenceRequest>,
) -> Result<(StatusCode, Json<vark_conversation::Answer>), impl IntoResponse> {
    validate_request(&state, &principal, &request).map_err(ApiError::response)?;
    let engine = state
        .engine_for(&principal.user_id)
        .await
        .map_err(ApiError::response)?;
    let result = engine.lock().await.chat_async(&request.text).await;
    match result {
        Ok(response) => Ok((StatusCode::OK, Json(response))),
        Err(_) => Err(ApiError::internal().response()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request as HttpRequest};
    use std::sync::atomic::{AtomicU64, Ordering};
    use tower::ServiceExt;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    fn engine() -> Engine {
        let root = std::env::temp_dir().join(format!(
            "vark-api-test-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let config = VarkConfig {
            neuron_count: 32,
            vocab_size: 32,
            memory_db: root.join("memory.jsonl").to_string_lossy().into_owned(),
            checkpoint_dir: root.join("checkpoints").to_string_lossy().into_owned(),
            ..VarkConfig::default()
        };
        Engine::new(config).unwrap()
    }

    fn credentials() -> ApiAuth {
        ApiAuth::new(HashMap::from([
            (
                "alice-token-012345678901234567890123456789".into(),
                "alice".into(),
            ),
            (
                "bob-token-0123456789012345678901234567890".into(),
                "bob".into(),
            ),
        ]))
        .unwrap()
    }

    fn request(path: &str, token: Option<&str>, body: &str) -> HttpRequest<Body> {
        let mut builder = HttpRequest::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json");
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        builder.body(Body::from(body.to_owned())).unwrap()
    }

    #[tokio::test]
    async fn bearer_auth_is_required_and_bound_to_user_id() {
        let app = router(engine(), credentials());
        let unauthorized = app
            .clone()
            .oneshot(request("/infer", None, r#"{"text":"Apa itu Rust?"}"#))
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let forbidden = app
            .clone()
            .oneshot(request(
                "/infer",
                Some("alice-token-012345678901234567890123456789"),
                r#"{"text":"Apa itu Rust?","user_id":"bob"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

        let accepted = app
            .oneshot(request(
                "/infer",
                Some("alice-token-012345678901234567890123456789"),
                r#"{"text":"Apa itu Rust?","user_id":"alice"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(accepted.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn engine_limit_and_request_rate_limit_are_enforced() {
        let app = router_with_limits(engine(), credentials(), 1, 1);
        let accepted = app
            .clone()
            .oneshot(request(
                "/infer",
                Some("alice-token-012345678901234567890123456789"),
                r#"{"text":"pernyataan pertama"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(accepted.status(), StatusCode::OK);

        let rate_limited = app
            .clone()
            .oneshot(request(
                "/infer",
                Some("alice-token-012345678901234567890123456789"),
                r#"{"text":"pernyataan kedua"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(rate_limited.status(), StatusCode::TOO_MANY_REQUESTS);

        let capacity_limited = app
            .oneshot(request(
                "/infer",
                Some("bob-token-0123456789012345678901234567890"),
                r#"{"text":"pernyataan pengguna lain"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(capacity_limited.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn user_paths_are_stable_and_collision_resistant() {
        let config = VarkConfig::default();
        let left = user_config(&config, "a/b");
        let right = user_config(&config, "a_b");
        assert_ne!(left.memory_db, right.memory_db);
        assert_eq!(left.memory_db, user_config(&config, "a/b").memory_db);
        assert_ne!(left.checkpoint_dir, right.checkpoint_dir);
    }
}
