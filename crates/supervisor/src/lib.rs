pub mod config_render;
pub mod env_tools;
pub mod mcp;
pub mod opencode;
pub mod safety_net;
pub mod types;

pub use types::*;

use menzi_common::Result;
use opencode::OpencodeSession;

pub async fn run(config: &menzi_common::config::Config) -> Result<()> {
    let supervisor_config = SupervisorConfig::from_env()?;
    let renderer = config_render::ConfigRenderer::new(
        &supervisor_config.control_plane_url,
        &supervisor_config.session_token,
    );
    let model = std::env::var("MENZI_OPENCODE_MODEL")
        .unwrap_or_else(|_| "openrouter/anthropic/claude-sonnet-5".to_string());
    let opencode_config = renderer.render(&model, default_permissions());
    let config_json = config_render::render_opencode_config(&opencode_config)?;

    let config_dir = OpencodeSession::config_dir_from_env();
    std::fs::create_dir_all(&config_dir).map_err(|error| {
        menzi_common::MenziError::Internal(anyhow::Error::msg(format!(
            "failed to create config dir: {error}"
        )))
    })?;
    let config_path = format!("{config_dir}/opencode.json");
    std::fs::write(&config_path, config_json).map_err(|error| {
        menzi_common::MenziError::Internal(anyhow::Error::msg(format!(
            "failed to write opencode config: {error}"
        )))
    })?;

    let mut session = OpencodeSession::start(&config_path).await?;
    let subject = format!("{}.{}", menzi_events::streams::SESSION, session.session_id);
    let bus = menzi_events::bus::NatsEventBus::connect(&config.nats_url).await?;
    bus.ensure_stream(menzi_events::streams::SESSION, &[&subject])
        .await?;

    let ready = menzi_events::Event::new(
        "session.ready",
        session.session_id.to_string(),
        "session",
        serde_json::json!({
            "opencode_version": supervisor_config.opencode_version,
            "port": session.port,
        }),
    );
    bus.publish(&subject, &ready).await?;

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(
        supervisor_config.heartbeat_interval_secs,
    ));
    loop {
        tokio::select! {
            exit_code = session.wait() => {
                let stopped = menzi_events::Event::new(
                    "session.stopped",
                    session.session_id.to_string(),
                    "session",
                    serde_json::json!({"exit_code": exit_code}),
                );
                bus.publish(&subject, &stopped).await?;
                return Ok(());
            }
            _ = interval.tick() => {
                let heartbeat = Heartbeat {
                    session_id: session.session_id,
                    status: "running".to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    resource_usage: ResourceUsage {
                        cpu_percent: 0.0,
                        memory_mb: 0,
                        disk_mb: 0,
                    },
                };
                let event = menzi_events::Event::new(
                    "session.heartbeat",
                    session.session_id.to_string(),
                    "session",
                    heartbeat,
                );
                bus.publish(&subject, &event).await?;
            }
        }
    }
}

fn default_permissions() -> Vec<PermissionRule> {
    vec![PermissionRule {
        action: "*".to_string(),
        resource: "*".to_string(),
        effect: "allow".to_string(),
    }]
}

pub fn heartbeat_payload(session_id: SessionId, status: &str) -> Heartbeat {
    Heartbeat {
        session_id,
        status: status.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        resource_usage: ResourceUsage {
            cpu_percent: 0.0,
            memory_mb: 0,
            disk_mb: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_permissions_allow_all_container_tools() {
        let permissions = default_permissions();
        assert_eq!(permissions.len(), 1);
        assert_eq!(permissions[0].action, "*");
        assert_eq!(permissions[0].resource, "*");
        assert_eq!(permissions[0].effect, "allow");
    }

    #[test]
    fn heartbeat_payload_builds_event_data() {
        let session_id = SessionId::new();
        let heartbeat = heartbeat_payload(session_id, "running");
        assert_eq!(heartbeat.session_id, session_id);
        assert_eq!(heartbeat.status, "running");
        assert_eq!(heartbeat.resource_usage.cpu_percent, 0.0);
    }
}
