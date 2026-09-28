use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use menzi_common::{MenziError, Result};
use reqwest::Identity;

use crate::types::*;

#[async_trait]
pub trait LxdClient: Send + Sync {
    async fn create_instance(&self, spec: InstanceSpec) -> Result<Instance>;
    async fn start_instance(&self, name: &str) -> Result<()>;
    async fn stop_instance(&self, name: &str, force: bool) -> Result<()>;
    async fn delete_instance(&self, name: &str) -> Result<()>;
    async fn snapshot_instance(&self, name: &str, snapshot: &str) -> Result<()>;
    async fn restore_snapshot(&self, name: &str, snapshot: &str) -> Result<()>;
    async fn copy_instance(&self, source: &str, dest: &str) -> Result<Instance>;
    async fn exec(&self, name: &str, command: &[String]) -> Result<ExecResult>;
    async fn list_instances(&self) -> Result<Vec<Instance>>;
    async fn list_snapshots(&self, name: &str) -> Result<Vec<Snapshot>>;
}

pub struct HttpLxdClient {
    http: reqwest::Client,
    base_url: String,
    project: String,
}

impl HttpLxdClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        Self::build(base_url.into(), "default".to_string(), None, None)
    }

    pub fn connect(
        base_url: impl Into<String>,
        project: impl Into<String>,
        cert_path: Option<&str>,
        key_path: Option<&str>,
    ) -> Result<Self> {
        Self::build(base_url.into(), project.into(), cert_path, key_path)
    }

    fn build(
        base_url: String,
        project: String,
        cert_path: Option<&str>,
        key_path: Option<&str>,
    ) -> Result<Self> {
        let mut builder = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(90));
        if let (Some(cert), Some(key)) = (cert_path, key_path) {
            let cert_pem = std::fs::read(cert)
                .map_err(|e| MenziError::Lxd(format!("failed to read client certificate: {e}")))?;
            let key_pem = std::fs::read(key)
                .map_err(|e| MenziError::Lxd(format!("failed to read client key: {e}")))?;
            let mut pem = cert_pem;
            pem.extend_from_slice(&key_pem);
            let identity = Identity::from_pem(&pem)
                .map_err(|e| MenziError::Lxd(format!("invalid client identity: {e}")))?;
            builder = builder.identity(identity);
        }
        let http = builder
            .build()
            .map_err(|e| MenziError::Lxd(format!("failed to build client: {e}")))?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            project,
        })
    }

    pub fn instances_url(&self) -> String {
        format!("{}/1.0/instances?project={}", self.base_url, self.project)
    }

    pub fn instance_url(&self, name: &str) -> String {
        format!(
            "{}/1.0/instances/{name}?project={}",
            self.base_url, self.project
        )
    }

    pub fn instance_state_url(&self, name: &str) -> String {
        format!(
            "{}/1.0/instances/{name}/state?project={}",
            self.base_url, self.project
        )
    }

    pub fn snapshots_url(&self, name: &str) -> String {
        format!(
            "{}/1.0/instances/{name}/snapshots?project={}",
            self.base_url, self.project
        )
    }

    pub fn snapshot_url(&self, name: &str, snapshot: &str) -> String {
        format!(
            "{}/1.0/instances/{name}/snapshots/{snapshot}?project={}",
            self.base_url, self.project
        )
    }

    pub fn snapshot_restore_url(&self, name: &str, snapshot: &str) -> String {
        format!(
            "{}/1.0/instances/{name}/snapshots/{snapshot}/restore?project={}",
            self.base_url, self.project
        )
    }

    pub fn create_body(&self, spec: &InstanceSpec) -> String {
        serde_json::json!({
            "name": spec.name,
            "source": { "type": "image", "alias": spec.image },
            "profiles": spec.profiles,
            "config": spec.config,
        })
        .to_string()
    }

    pub fn copy_body(&self, dest: &str, source: &str) -> String {
        serde_json::json!({
            "name": dest,
            "source": { "type": "copy", "source": source },
        })
        .to_string()
    }

    pub fn parse_instances(&self, body: &str) -> Result<Vec<Instance>> {
        let value: serde_json::Value = serde_json::from_str(body)
            .map_err(|e| MenziError::Lxd(format!("invalid instance list: {e}")))?;
        let metadata = value
            .get("metadata")
            .ok_or_else(|| MenziError::Lxd("missing metadata".to_string()))?;
        let entries = metadata
            .as_array()
            .ok_or_else(|| MenziError::Lxd("metadata is not a list".to_string()))?;
        let mut instances = Vec::new();
        for entry in entries {
            if let Some(instance) = self.parse_instance_entry(entry) {
                instances.push(instance);
            }
        }
        Ok(instances)
    }

    pub fn parse_snapshots(&self, body: &str) -> Result<Vec<Snapshot>> {
        let value: serde_json::Value = serde_json::from_str(body)
            .map_err(|e| MenziError::Lxd(format!("invalid snapshot list: {e}")))?;
        let metadata = value
            .get("metadata")
            .ok_or_else(|| MenziError::Lxd("missing metadata".to_string()))?;
        let entries = metadata
            .as_array()
            .ok_or_else(|| MenziError::Lxd("metadata is not a list".to_string()))?;
        let mut snapshots = Vec::new();
        for entry in entries {
            if let Some(snapshot) = self.parse_snapshot_entry(entry) {
                snapshots.push(snapshot);
            }
        }
        Ok(snapshots)
    }

    pub fn parse_exec_result(&self, body: &str) -> Result<ExecResult> {
        let value: serde_json::Value = serde_json::from_str(body)
            .map_err(|e| MenziError::Lxd(format!("invalid exec result: {e}")))?;
        let metadata = value
            .get("metadata")
            .ok_or_else(|| MenziError::Lxd("exec result missing metadata".to_string()))?;
        let exit_code = metadata
            .get("return")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);
        let stdout = self.decode_output(metadata.get("stdout").and_then(|v| v.as_str()));
        let stderr = self.decode_output(metadata.get("stderr").and_then(|v| v.as_str()));
        Ok(ExecResult {
            exit_code: exit_code as i32,
            stdout,
            stderr,
        })
    }

    fn parse_instance_entry(&self, entry: &serde_json::Value) -> Option<Instance> {
        let name = entry.get("name")?.as_str()?.to_string();
        let status = entry
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();
        let status_code = entry
            .get("status_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1) as i32;
        let profiles = entry
            .get("profiles")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|p| p.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        Some(Instance {
            name,
            status,
            status_code,
            profiles,
        })
    }

    fn parse_snapshot_entry(&self, entry: &serde_json::Value) -> Option<Snapshot> {
        let name = entry.get("name")?.as_str()?.to_string();
        let created_at = entry
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let stateful = entry
            .get("stateful")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Some(Snapshot {
            name,
            created_at,
            stateful,
        })
    }

    fn decode_output(&self, encoded: Option<&str>) -> String {
        match encoded {
            Some(raw) => BASE64
                .decode(raw)
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .unwrap_or_else(|_| raw.to_string()),
            None => String::new(),
        }
    }

    async fn send(&self, request: reqwest::RequestBuilder) -> Result<serde_json::Value> {
        let response = request
            .send()
            .await
            .map_err(|e| MenziError::Lxd(format!("request failed: {e}")))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|e| MenziError::Lxd(format!("response read failed: {e}")))?;
        if !status.is_success() {
            return Err(MenziError::Lxd(format!(
                "lxd returned {status}: {}",
                String::from_utf8_lossy(&bytes)
            )));
        }
        serde_json::from_slice(&bytes)
            .map_err(|e| MenziError::Lxd(format!("invalid lxd response: {e}")))
    }
}

#[async_trait]
impl LxdClient for HttpLxdClient {
    async fn create_instance(&self, spec: InstanceSpec) -> Result<Instance> {
        let body = self.create_body(&spec);
        let value = self
            .send(
                self.http
                    .post(self.instances_url())
                    .header("content-type", "application/json")
                    .body(body),
            )
            .await?;
        let metadata = value
            .get("metadata")
            .ok_or_else(|| MenziError::Lxd("create response missing metadata".to_string()))?;
        self.parse_instance_entry(metadata)
            .ok_or_else(|| MenziError::Lxd("create response is not an instance".to_string()))
    }

    async fn start_instance(&self, name: &str) -> Result<()> {
        let body = serde_json::json!({ "action": "start" }).to_string();
        self.send(
            self.http
                .put(self.instance_state_url(name))
                .header("content-type", "application/json")
                .body(body),
        )
        .await?;
        Ok(())
    }

    async fn stop_instance(&self, name: &str, force: bool) -> Result<()> {
        let body = serde_json::json!({ "action": "stop", "force": force }).to_string();
        self.send(
            self.http
                .put(self.instance_state_url(name))
                .header("content-type", "application/json")
                .body(body),
        )
        .await?;
        Ok(())
    }

    async fn delete_instance(&self, name: &str) -> Result<()> {
        self.send(self.http.delete(self.instance_url(name))).await?;
        Ok(())
    }

    async fn snapshot_instance(&self, name: &str, snapshot: &str) -> Result<()> {
        let body = serde_json::json!({ "name": snapshot }).to_string();
        self.send(
            self.http
                .post(self.snapshots_url(name))
                .header("content-type", "application/json")
                .body(body),
        )
        .await?;
        Ok(())
    }

    async fn restore_snapshot(&self, name: &str, snapshot: &str) -> Result<()> {
        self.send(
            self.http
                .post(self.snapshot_restore_url(name, snapshot))
                .header("content-type", "application/json")
                .body("{}"),
        )
        .await?;
        Ok(())
    }

    async fn copy_instance(&self, source: &str, dest: &str) -> Result<Instance> {
        let body = self.copy_body(dest, source);
        let value = self
            .send(
                self.http
                    .post(self.instances_url())
                    .header("content-type", "application/json")
                    .body(body),
            )
            .await?;
        let metadata = value
            .get("metadata")
            .ok_or_else(|| MenziError::Lxd("copy response missing metadata".to_string()))?;
        self.parse_instance_entry(metadata)
            .ok_or_else(|| MenziError::Lxd("copy response is not an instance".to_string()))
    }

    async fn exec(&self, name: &str, command: &[String]) -> Result<ExecResult> {
        let body = serde_json::json!({
            "command": command,
            "wait-for-websocket": false,
            "interactive": false,
            "environment": {},
        })
        .to_string();
        let value = self
            .send(
                self.http
                    .post(self.instance_url(name))
                    .header("content-type", "application/json")
                    .body(body),
            )
            .await?;
        self.parse_exec_result(&value.to_string())
    }

    async fn list_instances(&self) -> Result<Vec<Instance>> {
        let value = self.send(self.http.get(self.instances_url())).await?;
        self.parse_instances(&value.to_string())
    }

    async fn list_snapshots(&self, name: &str) -> Result<Vec<Snapshot>> {
        let value = self.send(self.http.get(self.snapshots_url(name))).await?;
        self.parse_snapshots(&value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> HttpLxdClient {
        HttpLxdClient::new("https://127.0.0.1:8443").unwrap()
    }

    #[test]
    fn instances_url_includes_project() {
        let client = client();
        assert_eq!(
            client.instances_url(),
            "https://127.0.0.1:8443/1.0/instances?project=default"
        );
    }

    #[test]
    fn instance_url_includes_name() {
        let client = client();
        assert_eq!(
            client.instance_url("web"),
            "https://127.0.0.1:8443/1.0/instances/web?project=default"
        );
    }

    #[test]
    fn snapshot_restore_url_is_well_formed() {
        let client = client();
        assert_eq!(
            client.snapshot_restore_url("web", "snap1"),
            "https://127.0.0.1:8443/1.0/instances/web/snapshots/snap1/restore?project=default"
        );
    }

    #[test]
    fn create_body_uses_image_source() {
        let client = client();
        let spec = InstanceSpec {
            name: "web".to_string(),
            image: "ubuntu/24.04".to_string(),
            profiles: vec!["default".to_string()],
            config: std::collections::HashMap::new(),
        };
        let body: serde_json::Value = serde_json::from_str(&client.create_body(&spec)).unwrap();
        assert_eq!(body["name"], "web");
        assert_eq!(body["source"]["type"], "image");
        assert_eq!(body["source"]["alias"], "ubuntu/24.04");
    }

    #[test]
    fn parse_instances_handles_lxd_list_response() {
        let client = client();
        let body = r#"{
            "type": "sync",
            "status_code": 200,
            "metadata": [
                {
                    "name": "web",
                    "status": "Running",
                    "status_code": 103,
                    "profiles": ["default"]
                },
                {
                    "name": "db",
                    "status": "Stopped",
                    "status_code": 102,
                    "profiles": ["default"]
                }
            ]
        }"#;
        let instances = client.parse_instances(body).unwrap();
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].name, "web");
        assert_eq!(instances[0].status, "Running");
        assert_eq!(instances[1].profiles, vec!["default".to_string()]);
    }

    #[test]
    fn parse_snapshots_handles_lxd_response() {
        let client = client();
        let body = r#"{
            "type": "sync",
            "status_code": 200,
            "metadata": [
                {
                    "name": "snap1",
                    "created_at": "2026-09-28T00:00:00Z",
                    "stateful": false
                }
            ]
        }"#;
        let snapshots = client.parse_snapshots(body).unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].name, "snap1");
    }

    #[test]
    fn parse_exec_result_decodes_base64_output() {
        let client = client();
        let stdout = BASE64.encode("hello world");
        let body = format!(
            r#"{{"type":"sync","status_code":200,"metadata":{{"return":0,"stdout":"{stdout}","stderr":""}}}}"#
        );
        let result = client.parse_exec_result(&body).unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "hello world");
    }

    #[test]
    fn parse_exec_result_falls_back_to_raw_output() {
        let client = client();
        let body = r#"{"type":"sync","status_code":200,"metadata":{"return":1,"stdout":"!!","stderr":"not-base64!!"}}"#;
        let result = client.parse_exec_result(body).unwrap();
        assert_eq!(result.exit_code, 1);
        assert_eq!(result.stdout, "!!");
        assert_eq!(result.stderr, "not-base64!!");
    }

    #[test]
    fn parse_instances_errors_on_invalid_body() {
        let client = client();
        assert!(client.parse_instances("not json").is_err());
    }
}
