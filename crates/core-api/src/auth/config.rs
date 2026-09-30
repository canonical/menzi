use chrono::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationMode {
    Closed,
    Invite,
    Open,
}

impl RegistrationMode {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "open" => Self::Open,
            "invite" => Self::Invite,
            _ => Self::Closed,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Invite => "invite",
            Self::Open => "open",
        }
    }

    pub fn allows_registration(&self) -> bool {
        matches!(self, Self::Open)
    }
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub session_cookie: String,
    pub csrf_cookie: String,
    pub csrf_header: String,
    pub public_base_url: String,
    pub secure_cookies: bool,
    pub session_ttl: Duration,
    pub session_idle: Duration,
    pub registration_mode: RegistrationMode,
    pub password_min_length: usize,
    pub login_max_attempts: i64,
    pub login_lockout: Duration,
    pub oidc_auto_provision: bool,
    pub oidc_state_ttl: Duration,
}

fn env_or(key: &str, fallback: &str) -> String {
    match std::env::var(key) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => fallback.to_string(),
    }
}

fn env_u64(key: &str, fallback: u64) -> u64 {
    match std::env::var(key) {
        Ok(value) => value.trim().parse().unwrap_or(fallback),
        Err(_) => fallback,
    }
}

fn env_bool(key: &str, fallback: bool) -> bool {
    match std::env::var(key) {
        Ok(value) => matches!(value.trim(), "1" | "true" | "yes" | "on"),
        Err(_) => fallback,
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            session_cookie: env_or("MENZI_SESSION_COOKIE", "menzi_session"),
            csrf_cookie: env_or("MENZI_CSRF_COOKIE", "menzi_csrf"),
            csrf_header: env_or("MENZI_CSRF_HEADER", "x-menzi-csrf"),
            public_base_url: env_or("MENZI_PUBLIC_BASE_URL", "http://127.0.0.1:5173"),
            secure_cookies: false,
            session_ttl: Duration::hours(env_u64("MENZI_SESSION_TTL_HOURS", 720) as i64),
            session_idle: Duration::hours(env_u64("MENZI_SESSION_IDLE_HOURS", 168) as i64),
            registration_mode: RegistrationMode::parse(&env_or(
                "MENZI_REGISTRATION_MODE",
                "closed",
            )),
            password_min_length: env_u64("MENZI_PASSWORD_MIN_LENGTH", 12) as usize,
            login_max_attempts: env_u64("MENZI_LOGIN_MAX_ATTEMPTS", 10) as i64,
            login_lockout: Duration::minutes(env_u64("MENZI_LOGIN_LOCKOUT_MINUTES", 15) as i64),
            oidc_auto_provision: env_bool("MENZI_OIDC_AUTO_PROVISION", false),
            oidc_state_ttl: Duration::minutes(env_u64("MENZI_OIDC_STATE_MINUTES", 10) as i64),
        }
    }
}

impl AuthConfig {
    pub fn for_tests() -> Self {
        Self {
            session_cookie: "menzi_session".to_string(),
            csrf_cookie: "menzi_csrf".to_string(),
            csrf_header: "x-menzi-csrf".to_string(),
            public_base_url: "http://127.0.0.1:5173".to_string(),
            secure_cookies: false,
            session_ttl: Duration::hours(720),
            session_idle: Duration::hours(168),
            registration_mode: RegistrationMode::Open,
            password_min_length: 12,
            login_max_attempts: 10,
            login_lockout: Duration::minutes(15),
            oidc_auto_provision: false,
            oidc_state_ttl: Duration::minutes(10),
        }
    }

    pub fn from_env() -> Self {
        let mut config = Self::default();
        config.secure_cookies = config.public_base_url.starts_with("https://");
        config
    }

    pub fn redirect_uri(&self, provider_id: &str) -> String {
        format!(
            "{}/api/v1/auth/oidc/{provider_id}/callback",
            self.public_base_url.trim_end_matches('/')
        )
    }

    pub fn max_age(&self) -> i64 {
        self.session_ttl.num_seconds()
    }

    pub fn clear_max_age(&self) -> i64 {
        -self.max_age()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    const ENV_KEYS: [&str; 6] = [
        "MENZI_SESSION_COOKIE",
        "MENZI_CSRF_COOKIE",
        "MENZI_CSRF_HEADER",
        "MENZI_REGISTRATION_MODE",
        "MENZI_PASSWORD_MIN_LENGTH",
        "MENZI_LOGIN_MAX_ATTEMPTS",
    ];

    const ENV_WITH_VALUE: [&str; 5] = [
        "MENZI_PUBLIC_BASE_URL",
        "MENZI_SESSION_TTL_HOURS",
        "MENZI_SESSION_IDLE_HOURS",
        "MENZI_LOGIN_LOCKOUT_MINUTES",
        "MENZI_OIDC_AUTO_PROVISION",
    ];

    fn with_env<T>(body: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap();
        let saved: Vec<(String, Option<String>)> = ENV_KEYS
            .iter()
            .chain(ENV_WITH_VALUE.iter())
            .map(|key| (key.to_string(), std::env::var(key).ok()))
            .collect();
        for key in ENV_KEYS.iter().chain(ENV_WITH_VALUE.iter()) {
            std::env::remove_var(key);
        }
        let outcome = body();
        for (key, value) in saved {
            match value {
                Some(value) => std::env::set_var(&key, value),
                None => std::env::remove_var(&key),
            }
        }
        outcome
    }

    #[test]
    fn the_default_config_is_closed_registration() {
        with_env(|| {
            std::env::remove_var("MENZI_REGISTRATION_MODE");
            let config = AuthConfig::default();
            assert_eq!(config.registration_mode, RegistrationMode::Closed);
            assert!(!config.registration_mode.allows_registration());
        });
    }

    #[test]
    fn registration_mode_parses_every_spelling() {
        assert_eq!(RegistrationMode::parse("open"), RegistrationMode::Open);
        assert_eq!(RegistrationMode::parse(" OPEN "), RegistrationMode::Open);
        assert_eq!(RegistrationMode::parse("invite"), RegistrationMode::Invite);
        assert_eq!(RegistrationMode::parse("closed"), RegistrationMode::Closed);
        assert_eq!(
            RegistrationMode::parse("nonsense"),
            RegistrationMode::Closed
        );
        assert_eq!(RegistrationMode::parse(""), RegistrationMode::Closed);
    }

    #[test]
    fn only_open_mode_allows_registration() {
        assert!(RegistrationMode::Open.allows_registration());
        assert!(!RegistrationMode::Invite.allows_registration());
        assert!(!RegistrationMode::Closed.allows_registration());
    }

    #[test]
    fn the_mode_round_trips_through_its_name() {
        for mode in [
            RegistrationMode::Closed,
            RegistrationMode::Invite,
            RegistrationMode::Open,
        ] {
            assert_eq!(RegistrationMode::parse(mode.as_str()), mode);
        }
    }

    #[test]
    fn the_test_configuration_never_reads_the_environment() {
        with_env(|| {
            std::env::set_var("MENZI_SESSION_COOKIE", "hijacked");
            std::env::set_var("MENZI_PASSWORD_MIN_LENGTH", "99");
            std::env::set_var("MENZI_REGISTRATION_MODE", "closed");
            let config = AuthConfig::for_tests();
            assert_eq!(config.session_cookie, "menzi_session");
            assert_eq!(config.password_min_length, 12);
            assert_eq!(config.registration_mode, RegistrationMode::Open);
        });
    }

    #[test]
    fn the_default_cookies_are_the_documented_names() {
        with_env(|| {
            std::env::remove_var("MENZI_SESSION_COOKIE");
            std::env::remove_var("MENZI_CSRF_COOKIE");
            std::env::remove_var("MENZI_CSRF_HEADER");
            let config = AuthConfig::default();
            assert_eq!(config.session_cookie, "menzi_session");
            assert_eq!(config.csrf_cookie, "menzi_csrf");
            assert_eq!(config.csrf_header, "x-menzi-csrf");
        });
    }

    #[test]
    fn a_public_https_base_marks_cookies_secure() {
        with_env(|| {
            std::env::set_var("MENZI_PUBLIC_BASE_URL", "https://menzi.example.com");
            assert!(AuthConfig::from_env().secure_cookies);
            std::env::set_var("MENZI_PUBLIC_BASE_URL", "http://127.0.0.1:5173");
            assert!(!AuthConfig::from_env().secure_cookies);
        });
    }

    #[test]
    fn the_redirect_uri_is_built_from_the_public_base() {
        with_env(|| {
            std::env::set_var("MENZI_PUBLIC_BASE_URL", "https://menzi.example.com/");
            let config = AuthConfig::from_env();
            assert_eq!(
                config.redirect_uri("google"),
                "https://menzi.example.com/api/v1/auth/oidc/google/callback"
            );
        });
    }

    #[test]
    fn clearing_a_cookie_expiries_it_immediately() {
        let config = AuthConfig::default();
        assert!(config.max_age() > 0);
        assert!(config.clear_max_age() < 0);
    }

    #[test]
    fn the_default_lifetime_is_thirty_days() {
        with_env(|| {
            std::env::remove_var("MENZI_SESSION_TTL_HOURS");
            assert_eq!(AuthConfig::default().session_ttl, Duration::hours(720));
        });
    }

    #[test]
    fn env_overrides_the_defaults() {
        with_env(|| {
            std::env::set_var("MENZI_PASSWORD_MIN_LENGTH", "20");
            std::env::set_var("MENZI_LOGIN_MAX_ATTEMPTS", "3");
            std::env::set_var("MENZI_OIDC_AUTO_PROVISION", "1");
            let config = AuthConfig::default();
            assert_eq!(config.password_min_length, 20);
            assert_eq!(config.login_max_attempts, 3);
            assert!(config.oidc_auto_provision);
        });
    }

    #[test]
    fn an_unparsable_number_falls_back_to_the_default() {
        with_env(|| {
            std::env::set_var("MENZI_PASSWORD_MIN_LENGTH", "not-a-number");
            assert_eq!(AuthConfig::default().password_min_length, 12);
        });
    }

    #[test]
    fn a_boolean_reads_the_usual_spellings() {
        with_env(|| {
            for value in ["1", "true", "yes", "on"] {
                std::env::set_var("MENZI_OIDC_AUTO_PROVISION", value);
                assert!(AuthConfig::default().oidc_auto_provision, "{value}");
            }
            for value in ["0", "false", "no", ""] {
                std::env::set_var("MENZI_OIDC_AUTO_PROVISION", value);
                assert!(!AuthConfig::default().oidc_auto_provision, "{value}");
            }
        });
    }
}
