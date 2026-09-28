use menzi_common::ids::SessionId;
use menzi_common::{MenziError, Result};

pub struct OpencodeSession {
    pub session_id: SessionId,
    pub port: u16,
    pub child: Option<tokio::process::Child>,
}

impl OpencodeSession {
    pub fn port_from_env() -> u16 {
        std::env::var("MENZI_OPENCODE_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(17999)
    }

    pub fn config_dir_from_env() -> String {
        std::env::var("MENZI_OPENCODE_CONFIG_DIR").unwrap_or_else(|_| "./.menzi".to_string())
    }

    pub fn binary_from_env() -> String {
        std::env::var("MENZI_OPENCODE_BIN").unwrap_or_else(|_| "opencode".to_string())
    }

    pub fn command(binary: &str, port: u16, config_path: &str) -> std::process::Command {
        let mut command = std::process::Command::new(binary);
        command.args([
            "serve",
            "--port",
            &port.to_string(),
            "--config",
            config_path,
        ]);
        command
    }

    pub async fn start(config_path: &str) -> Result<Self> {
        let port = Self::port_from_env();
        let binary = Self::binary_from_env();
        let command = Self::command(&binary, port, config_path);
        let child = tokio::process::Command::from(command)
            .spawn()
            .map_err(|error| {
                MenziError::Internal(anyhow::Error::msg(format!(
                    "failed to spawn opencode: {error}"
                )))
            })?;
        Ok(Self {
            session_id: SessionId::new(),
            port,
            child: Some(child),
        })
    }

    pub async fn wait(&mut self) -> Option<i32> {
        let child = self.child.as_mut()?;
        child.wait().await.ok()?.code()
    }
}

impl Drop for OpencodeSession {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_includes_serve_args() {
        let command = OpencodeSession::command("opencode", 17999, "/tmp/opencode.json");
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec!["serve", "--port", "17999", "--config", "/tmp/opencode.json"]
        );
    }

    #[test]
    fn port_from_env_uses_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("MENZI_OPENCODE_PORT");
        assert_eq!(OpencodeSession::port_from_env(), 17999);
    }

    #[test]
    fn port_from_env_parses_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("MENZI_OPENCODE_PORT", "18999");
        assert_eq!(OpencodeSession::port_from_env(), 18999);
        std::env::remove_var("MENZI_OPENCODE_PORT");
    }

    #[test]
    fn config_dir_from_env_uses_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("MENZI_OPENCODE_CONFIG_DIR");
        assert_eq!(OpencodeSession::config_dir_from_env(), "./.menzi");
    }

    #[test]
    fn binary_from_env_uses_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("MENZI_OPENCODE_BIN");
        assert_eq!(OpencodeSession::binary_from_env(), "opencode");
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}
