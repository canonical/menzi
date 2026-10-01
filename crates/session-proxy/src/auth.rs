use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

pub const HEADER_USER_ID: &str = "x-menzi-user-id";
pub const HEADER_SERVICE: &str = "x-menzi-service";
pub const IDENTITY_HEADERS: [&str; 2] = [HEADER_USER_ID, HEADER_SERVICE];

const GUARDED_PREFIXES: [&str; 2] = ["/session/", "/api/session/"];

pub fn is_guarded(path: &str) -> bool {
    GUARDED_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix) && path.len() > prefix.len())
}

pub fn has_credential(headers: &HeaderMap) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(|raw| {
            raw.strip_prefix("Bearer ")
                .or_else(|| raw.strip_prefix("bearer "))
                .map(|token| !token.trim().is_empty())
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

pub fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": {"code": "unauthenticated", "message": "sign in to continue"}})),
    )
        .into_response()
}

pub fn strip_identity(headers: &HeaderMap) -> HeaderMap {
    let mut out = HeaderMap::new();
    for (name, value) in headers.iter() {
        if IDENTITY_HEADERS.contains(&name.as_str()) || name == header::AUTHORIZATION {
            continue;
        }
        out.append(name.clone(), value.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, HeaderValue::from_str(value).unwrap());
        }
        map
    }

    #[test]
    fn a_session_read_is_guarded() {
        assert!(is_guarded("/session/ses_1/message"));
        assert!(is_guarded("/session/ses_1/diff"));
        assert!(is_guarded("/api/session/ses_1/diff"));
    }

    #[test]
    fn a_control_route_is_not_guarded() {
        assert!(!is_guarded("/api/event"));
        assert!(!is_guarded("/health"));
        assert!(!is_guarded("/agent"));
    }

    #[test]
    fn a_bare_prefix_is_not_a_session_read() {
        assert!(!is_guarded("/session/"));
    }

    #[test]
    fn a_bearer_token_counts_as_a_credential() {
        assert!(has_credential(&headers(&[("authorization", "Bearer abc")])));
        assert!(has_credential(&headers(&[("authorization", "bearer abc")])));
    }

    #[test]
    fn an_empty_or_missing_token_is_no_credential() {
        assert!(!has_credential(&headers(&[])));
        assert!(!has_credential(&headers(&[("authorization", "Bearer ")])));
        assert!(!has_credential(&headers(&[("authorization", "Basic abc")])));
    }

    #[test]
    fn a_client_cannot_forge_an_identity() {
        let out = strip_identity(&headers(&[
            ("x-menzi-user-id", "someone-else"),
            ("authorization", "Bearer real-token"),
            ("x-request-id", "keep-me"),
        ]));

        assert!(out.get(HEADER_USER_ID).is_none());
        assert!(out.get(header::AUTHORIZATION).is_none());
        assert_eq!(out.get("x-request-id").unwrap(), "keep-me");
    }
}
