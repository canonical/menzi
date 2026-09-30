use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use menzi_common::{MenziError, Result};
use reqwest::Identity;

use crate::types::*;

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn urlencode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                encoded.push(byte as char)
            }
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}

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
    async fn exec_in(
        &self,
        name: &str,
        command: &[String],
        cwd: Option<&str>,
    ) -> Result<ExecResult>;
    async fn list_instances(&self) -> Result<Vec<Instance>>;
    async fn list_snapshots(&self, name: &str) -> Result<Vec<Snapshot>>;
    async fn instance_exists(&self, name: &str) -> Result<bool>;
    async fn instance_state(&self, name: &str) -> Result<InstanceState>;
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
            .use_rustls_tls()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(90));
        if let (Some(cert), Some(key)) = (cert_path, key_path) {
            let cert_pem = std::fs::read(cert)
                .map_err(|e| MenziError::Lxd(format!("failed to read client certificate: {e}")))?;
            let key_pem = std::fs::read(key)
                .map_err(|e| MenziError::Lxd(format!("failed to read client key: {e}")))?;
            let mut pem = cert_pem;
            pem.extend_from_slice(&key_pem);
            let identity = Identity::from_pem(&pem).map_err(|e| {
                MenziError::Lxd(format!(
                    "invalid client identity: {e} (a pkcs1 or pkcs8 pem key is required)"
                ))
            })?;
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

    pub fn operation_url(&self, path: &str) -> String {
        if path.starts_with("http") {
            return format!("{}?project={}", path, self.project);
        }
        format!("{}{}?project={}", self.base_url, path, self.project)
    }

    pub fn parse_operation(&self, body: &str) -> Result<Operation> {
        let value: serde_json::Value = serde_json::from_str(body)
            .map_err(|e| MenziError::Lxd(format!("invalid operation response: {e}")))?;
        let metadata = value
            .get("metadata")
            .ok_or_else(|| MenziError::Lxd("operation response missing metadata".to_string()))?;
        let status_code = metadata
            .get("status_code")
            .and_then(|code| code.as_i64())
            .ok_or_else(|| MenziError::Lxd("operation has no status code".to_string()))?;
        let declared = metadata
            .get("location")
            .and_then(|location| location.as_str())
            .filter(|location| !location.is_empty() && *location != "none")
            .map(str::to_string);
        let id = metadata
            .get("id")
            .and_then(|id| id.as_str())
            .filter(|id| !id.is_empty())
            .map(str::to_string);
        let location = match (declared, id) {
            (Some(location), _) => Some(location),
            (None, Some(id)) => Some(format!("/1.0/operations/{id}")),
            (None, None) => None,
        };
        Ok(Operation {
            status: metadata
                .get("status")
                .and_then(|status| status.as_str())
                .unwrap_or("Unknown")
                .to_string(),
            status_code: status_code as i32,
            error: metadata
                .get("err")
                .and_then(|err| err.as_str())
                .unwrap_or_default()
                .to_string(),
            location,
        })
    }

    pub fn parse_instance(&self, body: &str) -> Result<Instance> {
        let value: serde_json::Value = serde_json::from_str(body)
            .map_err(|e| MenziError::Lxd(format!("invalid instance: {e}")))?;
        let metadata = value
            .get("metadata")
            .ok_or_else(|| MenziError::Lxd("instance response missing metadata".to_string()))?;
        self.parse_instance_entry(metadata)
            .ok_or_else(|| MenziError::Lxd("instance response is not an instance".to_string()))
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

    pub fn parse_instance_state(&self, body: &str) -> Result<InstanceState> {
        let value: serde_json::Value = serde_json::from_str(body)
            .map_err(|e| MenziError::Lxd(format!("invalid instance state: {e}")))?;
        let metadata = value
            .get("metadata")
            .ok_or_else(|| MenziError::Lxd("instance state missing metadata".to_string()))?;
        let status = metadata
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();
        let mut addresses = Vec::new();
        if let Some(network) = metadata.get("network").and_then(|n| n.as_object()) {
            for interface in network.values() {
                let Some(list) = interface.get("addresses").and_then(|a| a.as_array()) else {
                    continue;
                };
                for entry in list {
                    let Some(address) = entry.get("address").and_then(|a| a.as_str()) else {
                        continue;
                    };
                    let family = entry
                        .get("family")
                        .and_then(|f| f.as_str())
                        .unwrap_or_default();
                    let scope = entry
                        .get("scope")
                        .and_then(|s| s.as_str())
                        .unwrap_or("global");
                    if family == "inet" {
                        addresses.push(format!("{scope}:{address}"));
                    }
                }
            }
        }
        Ok(InstanceState { status, addresses })
    }

    pub fn instance_exec_url(&self, name: &str) -> String {
        format!(
            "{}/1.0/instances/{name}/exec?project={}",
            self.base_url, self.project
        )
    }

    pub fn instance_file_url(&self, name: &str, path: &str) -> String {
        format!(
            "{}/1.0/instances/{name}/files?project={}&path={}",
            self.base_url,
            self.project,
            urlencode(path)
        )
    }

    pub fn exec_body(&self, command: &[String]) -> String {
        serde_json::json!({
            "command": command,
            "wait-for-websocket": false,
            "interactive": false,
            "environment": {},
        })
        .to_string()
    }

    pub fn exec_script(&self, command: &[String], cwd: Option<&str>) -> String {
        let mut script = String::new();
        if let Some(cwd) = cwd {
            if !cwd.is_empty() {
                script.push_str(&format!("cd {} || exit 127\n", quote(cwd)));
            }
        }
        for (index, argument) in command.iter().enumerate() {
            if index > 0 {
                script.push(' ');
            }
            script.push_str(&quote(argument));
        }
        script
    }

    async fn read_file(&self, name: &str, path: &str) -> Result<Option<String>> {
        let response = self
            .http
            .get(self.instance_file_url(name, path))
            .send()
            .await
            .map_err(|e| MenziError::Lxd(format!("request failed: {e}")))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(MenziError::Lxd(format!(
                "lxd returned {}",
                response.status()
            )));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| MenziError::Lxd(format!("response read failed: {e}")))?;
        Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
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

    async fn send_operation(&self, request: reqwest::RequestBuilder) -> Result<Operation> {
        let value = self.send(request).await?;
        let body = value.to_string();
        let mut operation = self.parse_operation(&body)?;
        let Some(pending) = operation.location.clone() else {
            return Ok(operation);
        };
        let url = self.operation_url(&pending);
        for _ in 0..600 {
            if operation.status_code >= 200 {
                break;
            }
            if operation.status_code >= 400 {
                let detail = if operation.error.is_empty() {
                    operation.status.clone()
                } else {
                    operation.error.clone()
                };
                return Err(MenziError::Lxd(format!("lxd operation failed: {detail}")));
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let value = self.send(self.http.get(&url)).await?;
            operation = self.parse_operation(&value.to_string())?;
        }
        if operation.status_code >= 400 {
            return Err(MenziError::Lxd(format!(
                "lxd operation failed: {}",
                operation.error
            )));
        }
        if operation.status_code < 200 {
            return Err(MenziError::Lxd("lxd operation did not finish".to_string()));
        }
        Ok(operation)
    }

    pub async fn get_instance(&self, name: &str) -> Result<Instance> {
        let value = self.send(self.http.get(self.instance_url(name))).await?;
        self.parse_instance(&value.to_string())
    }
}

#[async_trait]
impl LxdClient for HttpLxdClient {
    async fn create_instance(&self, spec: InstanceSpec) -> Result<Instance> {
        let body = self.create_body(&spec);
        self.send_operation(
            self.http
                .post(self.instances_url())
                .header("content-type", "application/json")
                .body(body),
        )
        .await?;
        self.get_instance(&spec.name).await
    }

    async fn start_instance(&self, name: &str) -> Result<()> {
        let body = serde_json::json!({ "action": "start" }).to_string();
        self.send_operation(
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
        self.send_operation(
            self.http
                .put(self.instance_state_url(name))
                .header("content-type", "application/json")
                .body(body),
        )
        .await?;
        Ok(())
    }

    async fn delete_instance(&self, name: &str) -> Result<()> {
        self.send_operation(self.http.delete(self.instance_url(name)))
            .await?;
        Ok(())
    }

    async fn snapshot_instance(&self, name: &str, snapshot: &str) -> Result<()> {
        let body = serde_json::json!({ "name": snapshot }).to_string();
        self.send_operation(
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
        self.send_operation(
            self.http
                .post(self.instances_url())
                .header("content-type", "application/json")
                .body(body),
        )
        .await?;
        self.get_instance(dest).await
    }

    async fn exec(&self, name: &str, command: &[String]) -> Result<ExecResult> {
        self.exec_in(name, command, None).await
    }

    async fn exec_in(
        &self,
        name: &str,
        command: &[String],
        cwd: Option<&str>,
    ) -> Result<ExecResult> {
        let token = format!("{}-{}", std::process::id(), uuid::Uuid::new_v4());
        let out = format!("/tmp/menzi-exec-{token}.out");
        let code = format!("{out}.rc");
        let script = self.exec_script(command, cwd);
        let wrapped = format!(
            "{{ {script}\n}} > {out} 2>&1 < /dev/null\nprintf %s \"$?\" > {code}",
            script = script,
            out = quote(&out),
            code = quote(&code)
        );
        self.send_operation(
            self.http
                .post(self.instance_exec_url(name))
                .header("content-type", "application/json")
                .body(self.exec_body(&["/bin/sh".to_string(), "-c".to_string(), wrapped])),
        )
        .await?;
        let stdout = self.read_file(name, &out).await?.unwrap_or_default();
        let exit_code = self
            .read_file(name, &code)
            .await?
            .and_then(|value| value.trim().parse::<i32>().ok())
            .unwrap_or(-1);
        let _ = self
            .http
            .delete(self.instance_file_url(name, &out))
            .send()
            .await;
        let _ = self
            .http
            .delete(self.instance_file_url(name, &code))
            .send()
            .await;
        Ok(ExecResult {
            exit_code,
            stdout,
            stderr: String::new(),
        })
    }

    async fn list_instances(&self) -> Result<Vec<Instance>> {
        let value = self.send(self.http.get(self.instances_url())).await?;
        self.parse_instances(&value.to_string())
    }

    async fn list_snapshots(&self, name: &str) -> Result<Vec<Snapshot>> {
        let value = self.send(self.http.get(self.snapshots_url(name))).await?;
        self.parse_snapshots(&value.to_string())
    }

    async fn instance_exists(&self, name: &str) -> Result<bool> {
        let response = self
            .http
            .get(self.instance_url(name))
            .send()
            .await
            .map_err(|e| MenziError::Lxd(format!("request failed: {e}")))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(false);
        }
        if !response.status().is_success() {
            return Err(MenziError::Lxd(format!(
                "lxd returned {}",
                response.status()
            )));
        }
        Ok(true)
    }

    async fn instance_state(&self, name: &str) -> Result<InstanceState> {
        let value = self
            .send(self.http.get(self.instance_state_url(name)))
            .await?;
        self.parse_instance_state(&value.to_string())
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

    #[test]
    fn parse_instance_state_reads_status_and_addresses() {
        let client = client();
        let body = r#"{
            "type": "sync",
            "status_code": 200,
            "metadata": {
                "status": "Running",
                "network": {
                    "eth0": {
                        "addresses": [
                            {"family": "inet6", "address": "fe80::1", "scope": "global"},
                            {"family": "inet", "address": "10.10.10.251", "scope": "global"},
                            {"family": "inet", "address": "127.0.0.1", "scope": "local"}
                        ]
                    }
                }
            }
        }"#;
        let state = client.parse_instance_state(body).unwrap();
        assert!(state.is_running());
        assert_eq!(state.global_ipv4(), Some("10.10.10.251"));
    }

    #[test]
    fn parse_instance_state_handles_a_container_with_no_addresses() {
        let client = client();
        let body = r#"{"type":"sync","status_code":200,"metadata":{"status":"Stopped"}}"#;
        let state = client.parse_instance_state(body).unwrap();
        assert!(!state.is_running());
        assert_eq!(state.global_ipv4(), None);
    }

    #[test]
    fn parse_instance_state_errors_on_invalid_body() {
        let client = client();
        assert!(client.parse_instance_state("not json").is_err());
        assert!(client.parse_instance_state("{}").is_err());
    }

    #[test]
    fn exec_body_requests_a_synchronous_exec() {
        let client = client();
        let body: serde_json::Value =
            serde_json::from_str(&client.exec_body(&["id".to_string()])).unwrap();
        assert_eq!(body["command"][0], "id");
        assert_eq!(body["wait-for-websocket"], false);
        assert_eq!(body["interactive"], false);
    }

    #[test]
    fn exec_script_quotes_every_argument() {
        let client = client();
        let script = client.exec_script(
            &[
                "sh".to_string(),
                "-lc".to_string(),
                "echo 'hi there'".to_string(),
            ],
            None,
        );
        assert_eq!(script, "'sh' '-lc' 'echo '\\''hi there'\\'''");
    }

    #[test]
    fn exec_script_changes_directory_first() {
        let client = client();
        let script = client.exec_script(&["ls".to_string()], Some("/workspace"));
        assert_eq!(script, "cd '/workspace' || exit 127\n'ls'");
    }

    #[test]
    fn exec_script_without_a_directory_is_a_plain_command() {
        let client = client();
        assert_eq!(client.exec_script(&["id".to_string()], None), "'id'");
        assert_eq!(client.exec_script(&["id".to_string()], Some("")), "'id'");
    }

    #[test]
    fn file_url_escapes_the_path() {
        let client = client();
        assert_eq!(
            client.instance_file_url("web", "/tmp/a b.txt"),
            "https://127.0.0.1:8443/1.0/instances/web/files?project=default&path=/tmp/a%20b.txt"
        );
    }

    #[test]
    fn exec_url_targets_the_exec_route() {
        let client = client();
        assert_eq!(
            client.instance_exec_url("web"),
            "https://127.0.0.1:8443/1.0/instances/web/exec?project=default"
        );
    }

    #[test]
    fn parse_operation_reads_a_running_task() {
        let client = client();
        let body = r#"{
            "type": "async",
            "status_code": 100,
            "metadata": {
                "id": "73db4fe5",
                "status": "Running",
                "status_code": 103,
                "err": "",
                "location": "/1.0/operations/73db4fe5"
            }
        }"#;
        let operation = client.parse_operation(body).unwrap();
        assert!(operation.is_running());
        assert!(!operation.succeeded());
        assert_eq!(
            operation.location.as_deref(),
            Some("/1.0/operations/73db4fe5")
        );
    }

    #[test]
    fn parse_operation_falls_back_to_the_operation_id() {
        let client = client();
        let body = r#"{
            "type": "async",
            "metadata": { "id": "abc-123", "status": "Running", "status_code": 103, "location": "none" }
        }"#;
        let operation = client.parse_operation(body).unwrap();
        assert_eq!(
            operation.location.as_deref(),
            Some("/1.0/operations/abc-123")
        );
    }

    #[test]
    fn parse_operation_without_a_handle_is_treated_as_finished() {
        let client = client();
        let body = r#"{"type":"sync","metadata":{"status":"Success","status_code":200}}"#;
        let operation = client.parse_operation(body).unwrap();
        assert!(operation.succeeded());
        assert_eq!(operation.location, None);
    }

    #[test]
    fn parse_operation_reads_a_finished_task() {
        let client = client();
        let body = r#"{
            "type": "sync",
            "status_code": 200,
            "metadata": {
                "status": "Success",
                "status_code": 200,
                "err": "",
                "location": "none"
            }
        }"#;
        let operation = client.parse_operation(body).unwrap();
        assert!(operation.succeeded());
        assert_eq!(operation.location, None);
    }

    #[test]
    fn parse_operation_reads_a_failure() {
        let client = client();
        let body = r#"{
            "type": "sync",
            "metadata": { "status": "Failure", "status_code": 400, "err": "copy failed" }
        }"#;
        let operation = client.parse_operation(body).unwrap();
        assert!(operation.failed());
        assert_eq!(operation.error, "copy failed");
    }

    #[test]
    fn parse_operation_errors_on_a_body_without_a_status() {
        let client = client();
        assert!(client.parse_operation("not json").is_err());
        assert!(client.parse_operation("{}").is_err());
        assert!(client.parse_operation(r#"{"metadata":{}}"#).is_err());
    }

    #[test]
    fn parse_instance_reads_a_single_instance_response() {
        let client = client();
        let body = r#"{"type":"sync","metadata":{"name":"wsp-a-b","status":"Running","status_code":103,"profiles":["menzi"]}}"#;
        let instance = client.parse_instance(body).unwrap();
        assert_eq!(instance.name, "wsp-a-b");
        assert!(instance.status_code == 103);
    }

    #[test]
    fn operation_url_appends_the_project() {
        let client = client();
        assert_eq!(
            client.operation_url("/1.0/operations/abc"),
            "https://127.0.0.1:8443/1.0/operations/abc?project=default"
        );
        assert_eq!(
            client.operation_url("https://elsewhere/1.0/operations/abc"),
            "https://elsewhere/1.0/operations/abc?project=default"
        );
    }

    #[test]
    fn parse_instance_state_ignores_non_string_addresses() {
        let client = client();
        let body = r#"{
            "type": "sync",
            "status_code": 200,
            "metadata": {
                "status": "Running",
                "network": {
                    "eth0": {
                        "addresses": [
                            {"family": 6, "address": null},
                            {"family": "inet", "address": "10.0.0.9", "scope": "global"}
                        ]
                    }
                }
            }
        }"#;
        let state = client.parse_instance_state(body).unwrap();
        assert_eq!(state.global_ipv4(), Some("10.0.0.9"));
    }
}
