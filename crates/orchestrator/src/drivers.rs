use async_trait::async_trait;
use menzi_common::{MenziError, Result};
use menzi_lxd::ExecResult;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[async_trait]
pub trait EnvDriver: Send + Sync {
    async fn create_instance(&self, name: &str, image: &str, profiles: &[String]) -> Result<()>;
    async fn start_instance(&self, name: &str) -> Result<()>;
    async fn stop_instance(&self, name: &str, force: bool) -> Result<()>;
    async fn delete_instance(&self, name: &str) -> Result<()>;
    async fn exec(&self, name: &str, command: &[String]) -> Result<ExecResult>;
    async fn instance_status(&self, name: &str) -> Result<String>;
    async fn snapshot_instance(&self, name: &str, snapshot: &str) -> Result<()>;
    async fn copy_instance(&self, source: &str, dest: &str) -> Result<()>;
}

pub struct CollocateDriver {
    pub lxd: Arc<dyn menzi_lxd::LxdClient>,
}

impl CollocateDriver {
    pub fn new(lxd: Arc<dyn menzi_lxd::LxdClient>) -> Self {
        Self { lxd }
    }
}

#[async_trait]
impl EnvDriver for CollocateDriver {
    async fn create_instance(&self, name: &str, image: &str, profiles: &[String]) -> Result<()> {
        let spec = menzi_lxd::InstanceSpec {
            name: name.to_string(),
            image: image.to_string(),
            profiles: profiles.to_vec(),
            config: Default::default(),
        };
        self.lxd.create_instance(spec).await.map(|_| ())
    }

    async fn start_instance(&self, name: &str) -> Result<()> {
        self.lxd.start_instance(name).await
    }

    async fn stop_instance(&self, name: &str, force: bool) -> Result<()> {
        self.lxd.stop_instance(name, force).await
    }

    async fn delete_instance(&self, name: &str) -> Result<()> {
        self.lxd.delete_instance(name).await
    }

    async fn exec(&self, name: &str, command: &[String]) -> Result<ExecResult> {
        self.lxd.exec(name, command).await
    }

    async fn instance_status(&self, name: &str) -> Result<String> {
        let instances = self.lxd.list_instances().await?;
        instances
            .iter()
            .find(|instance| instance.name == name)
            .map(|instance| instance.status.clone())
            .ok_or_else(|| MenziError::NotFound(format!("instance {name}")))
    }

    async fn snapshot_instance(&self, name: &str, snapshot: &str) -> Result<()> {
        self.lxd.snapshot_instance(name, snapshot).await
    }

    async fn copy_instance(&self, source: &str, dest: &str) -> Result<()> {
        self.lxd.copy_instance(source, dest).await.map(|_| ())
    }
}

#[derive(Debug, Clone, Default)]
pub struct SystemdDriver;

impl SystemdDriver {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EnvDriver for SystemdDriver {
    async fn create_instance(&self, name: &str, _image: &str, _profiles: &[String]) -> Result<()> {
        let unit = format!("menzi-{name}");
        let status = tokio::process::Command::new("systemctl")
            .args(["start", &unit])
            .status()
            .await
            .map_err(|e| {
                MenziError::Internal(anyhow::Error::msg(format!("systemctl failed: {e}")))
            })?;
        if status.success() {
            Ok(())
        } else {
            Err(MenziError::Internal(anyhow::Error::msg(format!(
                "failed to start systemd unit {unit}"
            ))))
        }
    }

    async fn start_instance(&self, name: &str) -> Result<()> {
        run_systemctl(&["start", &format!("menzi-{name}")]).await
    }

    async fn stop_instance(&self, name: &str, force: bool) -> Result<()> {
        let mut args = vec!["stop".to_string()];
        if force {
            args.push("--force".to_string());
        }
        args.push(format!("menzi-{name}"));
        run_systemctl(&args).await
    }

    async fn delete_instance(&self, name: &str) -> Result<()> {
        let unit = format!("menzi-{name}");
        run_systemctl(&["stop", &unit]).await?;
        run_systemctl(&["disable", &unit]).await
    }

    async fn exec(&self, _name: &str, _command: &[String]) -> Result<ExecResult> {
        Err(MenziError::Internal(anyhow::Error::msg(
            "systemd driver does not support exec",
        )))
    }

    async fn instance_status(&self, name: &str) -> Result<String> {
        let status = tokio::process::Command::new("systemctl")
            .args(["is-active", &format!("menzi-{name}")])
            .status()
            .await
            .map_err(|e| {
                MenziError::Internal(anyhow::Error::msg(format!("systemctl failed: {e}")))
            })?;
        if status.success() {
            Ok("running".to_string())
        } else {
            Ok("stopped".to_string())
        }
    }

    async fn snapshot_instance(&self, _name: &str, _snapshot: &str) -> Result<()> {
        Err(MenziError::Internal(anyhow::Error::msg(
            "systemd driver does not support snapshots",
        )))
    }

    async fn copy_instance(&self, _source: &str, _dest: &str) -> Result<()> {
        Err(MenziError::Internal(anyhow::Error::msg(
            "systemd driver does not support instance copies",
        )))
    }
}

async fn run_systemctl<S: AsRef<str> + std::fmt::Debug>(args: &[S]) -> Result<()> {
    let status = tokio::process::Command::new("systemctl")
        .args(args.iter().map(AsRef::as_ref))
        .status()
        .await
        .map_err(|e| MenziError::Internal(anyhow::Error::msg(format!("systemctl failed: {e}"))))?;
    if status.success() {
        Ok(())
    } else {
        Err(MenziError::Internal(anyhow::Error::msg(format!(
            "systemctl {:?} failed",
            args
        ))))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverConfig {
    pub driver_type: DriverType,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriverType {
    Systemd,
    Collocate,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_type_roundtrips() {
        assert_eq!(
            serde_json::to_string(&DriverType::Systemd).unwrap(),
            "\"systemd\""
        );
        assert_eq!(
            serde_json::to_string(&DriverType::Collocate).unwrap(),
            "\"collocate\""
        );
    }

    #[test]
    fn driver_config_serializes() {
        let config = DriverConfig {
            driver_type: DriverType::Systemd,
            config: serde_json::json!({}),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"driver_type\":\"systemd\""));
    }

    #[test]
    fn systemd_driver_creates() {
        let driver = SystemdDriver::new();
        assert!(std::matches!(driver, SystemdDriver));
    }

    #[test]
    fn collocate_driver_creates() {
        let client = menzi_lxd::HttpLxdClient::new("https://127.0.0.1:8443").unwrap();
        let driver = CollocateDriver::new(Arc::new(client));
        assert_eq!(Arc::strong_count(&driver.lxd), 1);
    }

    #[derive(Clone)]
    struct RecordingLxdClient {
        calls: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl Default for RecordingLxdClient {
        fn default() -> Self {
            Self {
                calls: std::sync::Arc::new(std::sync::Mutex::new(vec![])),
            }
        }
    }

    #[async_trait]
    impl menzi_lxd::LxdClient for RecordingLxdClient {
        async fn create_instance(
            &self,
            spec: menzi_lxd::InstanceSpec,
        ) -> Result<menzi_lxd::Instance> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("create:{}", spec.name));
            Ok(menzi_lxd::Instance {
                name: spec.name,
                status: "Stopped".to_string(),
                status_code: 102,
                profiles: vec![],
            })
        }

        async fn start_instance(&self, name: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("start:{name}"));
            Ok(())
        }

        async fn stop_instance(&self, name: &str, force: bool) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("stop:{name}:{force}"));
            Ok(())
        }

        async fn delete_instance(&self, name: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("delete:{name}"));
            Ok(())
        }

        async fn snapshot_instance(&self, name: &str, snapshot: &str) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("snapshot:{name}:{snapshot}"));
            Ok(())
        }

        async fn restore_snapshot(&self, name: &str, snapshot: &str) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("restore:{name}:{snapshot}"));
            Ok(())
        }

        async fn copy_instance(&self, source: &str, dest: &str) -> Result<menzi_lxd::Instance> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("copy:{source}:{dest}"));
            Ok(menzi_lxd::Instance {
                name: dest.to_string(),
                status: "Stopped".to_string(),
                status_code: 102,
                profiles: vec![],
            })
        }

        async fn exec(&self, name: &str, command: &[String]) -> Result<menzi_lxd::ExecResult> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("exec:{name}:{}", command.join(" ")));
            Ok(menzi_lxd::ExecResult {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
            })
        }

        async fn list_instances(&self) -> Result<Vec<menzi_lxd::Instance>> {
            Ok(vec![])
        }

        async fn list_snapshots(&self, name: &str) -> Result<Vec<menzi_lxd::Snapshot>> {
            self.calls.lock().unwrap().push(format!("snapshots:{name}"));
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn collocate_copy_delegates_to_lxd() {
        let client = RecordingLxdClient::default();
        let driver = CollocateDriver::new(Arc::new(client.clone()));
        driver.copy_instance("mz-source", "mz-dest").await.unwrap();
        let calls = client.calls.lock().unwrap();
        assert_eq!(*calls, vec!["copy:mz-source:mz-dest"]);
    }

    #[tokio::test]
    async fn collocate_snapshot_delegates_to_lxd() {
        let client = RecordingLxdClient::default();
        let driver = CollocateDriver::new(Arc::new(client.clone()));
        driver.snapshot_instance("mz-dev", "snap-1").await.unwrap();
        let calls = client.calls.lock().unwrap();
        assert_eq!(*calls, vec!["snapshot:mz-dev:snap-1"]);
    }

    #[tokio::test]
    async fn systemd_driver_rejects_copy() {
        let driver = SystemdDriver::new();
        assert!(driver.copy_instance("a", "b").await.is_err());
    }

    #[tokio::test]
    async fn systemd_driver_rejects_snapshot() {
        let driver = SystemdDriver::new();
        assert!(driver.snapshot_instance("a", "s").await.is_err());
    }
}
