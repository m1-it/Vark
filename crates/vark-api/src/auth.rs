use axum::{
    extract::Request,
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::{collections::HashMap, env, sync::Arc};
use thiserror::Error;

#[derive(Clone)]
pub struct ApiAuth {
    users_by_token: Arc<HashMap<String, String>>,
}

#[derive(Debug, Error)]
pub enum ApiAuthError {
    #[error("VARK_API_TOKEN atau VARK_API_TOKENS wajib diatur")]
    Missing,
    #[error("VARK_API_TOKEN dan VARK_API_TOKENS tidak boleh dipakai bersamaan")]
    Ambiguous,
    #[error("VARK_API_TOKENS harus berupa JSON object token-ke-user-id yang tidak kosong")]
    InvalidMap,
    #[error("token dan user_id API tidak boleh kosong")]
    EmptyEntry,
    #[error("setiap bearer token API harus memiliki setidaknya 32 byte")]
    WeakToken,
}

#[derive(Clone, Debug)]
pub(super) struct Principal {
    pub user_id: String,
}

impl ApiAuth {
    pub fn from_env(default_user_id: &str) -> Result<Self, ApiAuthError> {
        let single = env::var("VARK_API_TOKEN").ok();
        let mapped = env::var("VARK_API_TOKENS").ok();
        match (single, mapped) {
            (Some(_), Some(_)) => Err(ApiAuthError::Ambiguous),
            (Some(token), None) => Self::new(HashMap::from([(token, default_user_id.to_owned())])),
            (None, Some(json)) => {
                let users = serde_json::from_str::<HashMap<String, String>>(&json)
                    .map_err(|_| ApiAuthError::InvalidMap)?;
                Self::new(users)
            }
            (None, None) => Err(ApiAuthError::Missing),
        }
    }

    pub fn new(users_by_token: HashMap<String, String>) -> Result<Self, ApiAuthError> {
        if users_by_token.is_empty() {
            return Err(ApiAuthError::InvalidMap);
        }
        if users_by_token
            .iter()
            .any(|(token, user_id)| token.trim().is_empty() || user_id.trim().is_empty())
        {
            return Err(ApiAuthError::EmptyEntry);
        }
        if users_by_token.keys().any(|token| token.len() < 32) {
            return Err(ApiAuthError::WeakToken);
        }
        Ok(Self {
            users_by_token: Arc::new(users_by_token),
        })
    }

    fn user_for_token(&self, supplied: &str) -> Option<&str> {
        let mut user_id = None;
        for (token, candidate_user) in self.users_by_token.iter() {
            if constant_time_eq(token.as_bytes(), supplied.as_bytes()) {
                user_id = Some(candidate_user.as_str());
            }
        }
        user_id
    }

    pub(super) fn authorizes_user_id(&self, user_id: &str) -> bool {
        self.users_by_token
            .values()
            .any(|authorized| authorized == user_id)
    }
}

pub(super) async fn authenticate(
    axum::extract::State(auth): axum::extract::State<Arc<ApiAuth>>,
    mut request: Request,
    next: Next,
) -> Response {
    let supplied = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let Some(user_id) = supplied.and_then(|token| auth.user_for_token(token)) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "autentikasi bearer token gagal"})),
        )
            .into_response();
    };
    request.extensions_mut().insert(Principal {
        user_id: user_id.to_owned(),
    });
    next.run(request).await
}

fn constant_time_eq(expected: &[u8], supplied: &[u8]) -> bool {
    let mut difference = expected.len() ^ supplied.len();
    for index in 0..expected.len().max(supplied.len()) {
        let left = expected.get(index).copied().unwrap_or_default();
        let right = supplied.get(index).copied().unwrap_or_default();
        difference |= usize::from(left ^ right);
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_requires_nonempty_credentials_and_matches_identity() {
        assert!(matches!(
            ApiAuth::new(HashMap::new()),
            Err(ApiAuthError::InvalidMap)
        ));
        assert!(matches!(
            ApiAuth::new(HashMap::from([(String::new(), "alice".into())])),
            Err(ApiAuthError::EmptyEntry)
        ));
        assert!(matches!(
            ApiAuth::new(HashMap::from([("short".into(), "alice".into())])),
            Err(ApiAuthError::WeakToken)
        ));
        let token = "very-long-test-token-012345678901234567890123";
        let auth = ApiAuth::new(HashMap::from([(token.into(), "alice".into())])).unwrap();
        assert_eq!(auth.user_for_token(token), Some("alice"));
        assert_eq!(auth.user_for_token("not-secret"), None);
    }

    #[test]
    fn token_comparison_rejects_different_lengths_and_contents() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(!constant_time_eq(b"abc", b"abd"));
    }
}
