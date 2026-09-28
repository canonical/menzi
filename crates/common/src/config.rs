use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub nats_url: String,
    pub lxd_url: String,
    pub lxd_cert_path: Option<String>,
    pub lxd_key_path: Option<String>,
    pub gateway_bind: String,
    pub log_level: String,
}

impl Config {
    pub fn from_env() -> crate::Result<Self> {
        let database_url = std::env::var("MENZI_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/menzi".to_string());
        let nats_url =
            std::env::var("MENZI_NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
        let lxd_url =
            std::env::var("MENZI_LXD_URL").unwrap_or_else(|_| "https://localhost:8443".to_string());
        let lxd_cert_path = std::env::var("MENZI_LXD_CERT_PATH").ok();
        let lxd_key_path = std::env::var("MENZI_LXD_KEY_PATH").ok();
        let gateway_bind =
            std::env::var("MENZI_GATEWAY_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        let log_level = std::env::var("MENZI_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

        Ok(Self {
            database_url,
            nats_url,
            lxd_url,
            lxd_cert_path,
            lxd_key_path,
            gateway_bind,
            log_level,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn config_from_env_returns_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("MENZI_DATABASE_URL");
        std::env::remove_var("MENZI_NATS_URL");
        std::env::remove_var("MENZI_LXD_URL");
        std::env::remove_var("MENZI_GATEWAY_BIND");
        std::env::remove_var("MENZI_LOG_LEVEL");

        let config = Config::from_env().unwrap();
        assert_eq!(config.database_url, "postgres://localhost/menzi");
        assert_eq!(config.nats_url, "nats://localhost:4222");
        assert_eq!(config.lxd_url, "https://localhost:8443");
        assert_eq!(config.gateway_bind, "0.0.0.0:8080");
        assert_eq!(config.log_level, "info");
        assert!(config.lxd_cert_path.is_none());
        assert!(config.lxd_key_path.is_none());
    }

    #[test]
    fn config_from_env_overrides_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("MENZI_DATABASE_URL", "postgres://custom/db");
        std::env::set_var("MENZI_NATS_URL", "nats://custom:4222");
        std::env::set_var("MENZI_LXD_URL", "https://custom:8443");
        std::env::set_var("MENZI_GATEWAY_BIND", "127.0.0.1:9090");
        std::env::set_var("MENZI_LOG_LEVEL", "debug");

        let config = Config::from_env().unwrap();
        assert_eq!(config.database_url, "postgres://custom/db");
        assert_eq!(config.nats_url, "nats://custom:4222");
        assert_eq!(config.lxd_url, "https://custom:8443");
        assert_eq!(config.gateway_bind, "127.0.0.1:9090");
        assert_eq!(config.log_level, "debug");

        std::env::remove_var("MENZI_DATABASE_URL");
        std::env::remove_var("MENZI_NATS_URL");
        std::env::remove_var("MENZI_LXD_URL");
        std::env::remove_var("MENZI_GATEWAY_BIND");
        std::env::remove_var("MENZI_LOG_LEVEL");
    }
}
