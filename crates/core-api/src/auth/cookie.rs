#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SameSite {
    Lax,
}

impl SameSite {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Lax => "Lax",
        }
    }
}

fn build(name: &str, value: &str, max_age: i64, http_only: bool, secure: bool) -> String {
    let mut cookie = format!(
        "{name}={value}; Path=/; Max-Age={max_age}; SameSite={}",
        SameSite::Lax.as_str()
    );
    if http_only {
        cookie.push_str("; HttpOnly");
    }
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

pub fn session(name: &str, value: &str, max_age: i64, secure: bool) -> String {
    build(name, value, max_age, true, secure)
}

pub fn csrf(name: &str, value: &str, max_age: i64, secure: bool) -> String {
    build(name, value, max_age, false, secure)
}

pub fn clear(name: &str, secure: bool) -> String {
    build(name, "", 0, true, secure)
}

pub fn value_of<'a>(headers: &'a axum::http::HeaderMap, name: &str) -> Option<&'a str> {
    let raw = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=') {
            if key == name {
                return Some(value);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{header, HeaderMap, HeaderValue};

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, HeaderValue::from_str(value).unwrap());
        }
        map
    }

    #[test]
    fn the_session_cookie_is_http_only_and_lax() {
        let cookie = session("menzi_session", "mz_abc", 3600, false);
        assert!(cookie.starts_with("menzi_session=mz_abc;"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Max-Age=3600"));
        assert!(cookie.contains("Path=/"));
        assert!(!cookie.contains("Secure"));
    }

    #[test]
    fn the_csrf_cookie_is_readable_by_script() {
        let cookie = csrf("menzi_csrf", "tok", 3600, false);
        assert!(cookie.contains("menzi_csrf=tok"));
        assert!(!cookie.contains("HttpOnly"));
    }

    #[test]
    fn a_secure_base_url_marks_both_cookies_secure() {
        assert!(session("a", "b", 1, true).contains("; Secure"));
        assert!(csrf("a", "b", 1, true).contains("; Secure"));
    }

    #[test]
    fn clearing_a_cookie_expires_it() {
        let cookie = clear("menzi_session", false);
        assert!(cookie.contains("Max-Age=0"));
        assert!(cookie.contains("HttpOnly"));
    }

    #[test]
    fn a_cookie_is_read_out_of_a_header() {
        let map = headers(&[("cookie", "menzi_csrf=tok; menzi_session=mz_abc")]);
        assert_eq!(value_of(&map, "menzi_session"), Some("mz_abc"));
        assert_eq!(value_of(&map, "menzi_csrf"), Some("tok"));
    }

    #[test]
    fn a_missing_cookie_is_none() {
        let map = headers(&[]);
        assert_eq!(value_of(&map, "menzi_session"), None);
        assert_eq!(value_of(&HeaderMap::new(), "anything"), None);
    }

    #[test]
    fn a_cookie_of_a_different_name_is_not_returned() {
        let map = headers(&[("cookie", "other=nope; menzi_session=mz_abc")]);
        assert_eq!(value_of(&map, "menzi_session"), Some("mz_abc"));
    }

    #[test]
    fn a_value_containing_an_equals_sign_survives() {
        let map = headers(&[("cookie", "menzi_session=mz_a=b=c")]);
        assert_eq!(value_of(&map, "menzi_session"), Some("mz_a=b=c"));
    }

    #[test]
    fn a_cookie_value_is_url_safe() {
        let cookie = session("menzi_session", "mz_-AbC123_xY", 1, false);
        assert!(cookie.starts_with("menzi_session=mz_-AbC123_xY;"));
        let _ = header::COOKIE;
    }
}
