use crate::types::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchPlan {
    pub steps: Vec<LaunchStep>,
    pub total_steps: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchStep {
    pub component: String,
    pub action: LaunchAction,
    pub depends_on: Vec<String>,
    pub command: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchAction {
    Build,
    Start,
    Wait,
    Run,
    Expose,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchResult {
    pub success: bool,
    pub components: Vec<ComponentResult>,
    pub exposures: Vec<ExposureResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentResult {
    pub name: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExposureResult {
    pub name: String,
    pub url: String,
    pub as_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputHash {
    pub component: String,
    pub input_path: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelaunchDiff {
    pub changed_inputs: Vec<String>,
    pub components_to_rebuild: Vec<String>,
    pub components_to_restart: Vec<String>,
    pub components_to_skip: Vec<String>,
}

impl RelaunchDiff {
    pub fn is_empty(&self) -> bool {
        self.changed_inputs.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentState {
    pub name: String,
    pub status: String,
    pub components: Vec<ComponentState>,
    pub exposures: Vec<ExposureState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentState {
    pub name: String,
    pub status: String,
    pub health: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExposureState {
    pub name: String,
    pub url: String,
    pub as_type: String,
}

#[derive(Debug, Clone, Default)]
pub struct EnvironmentController;

impl EnvironmentController {
    pub fn new() -> Self {
        Self
    }

    pub fn plan_launch(&self, spec: &EnvironmentSpec) -> LaunchPlan {
        let mut steps = vec![];
        for component in &spec.components {
            if component.run.is_some() {
                steps.push(LaunchStep {
                    component: component.name.clone(),
                    action: LaunchAction::Run,
                    depends_on: component.after.clone(),
                    command: component.run.clone().unwrap_or_default(),
                });
            } else {
                steps.push(LaunchStep {
                    component: component.name.clone(),
                    action: LaunchAction::Start,
                    depends_on: component.after.clone(),
                    command: String::new(),
                });
            }
        }
        LaunchPlan {
            total_steps: steps.len() as i32,
            steps,
        }
    }

    pub fn compute_relaunch_diff(
        &self,
        spec: &EnvironmentSpec,
        changes: &ChangeSet,
    ) -> RelaunchDiff {
        let mut diff = RelaunchDiff {
            changed_inputs: vec![],
            components_to_rebuild: vec![],
            components_to_restart: vec![],
            components_to_skip: vec![],
        };

        for component in &spec.components {
            let has_changed_input = changes
                .changed_inputs
                .iter()
                .any(|input| component.name.contains(input));

            if has_changed_input {
                diff.changed_inputs.push(component.name.clone());
                diff.components_to_rebuild.push(component.name.clone());
                diff.components_to_restart.push(component.name.clone());
            } else {
                diff.components_to_skip.push(component.name.clone());
            }
        }

        diff
    }

    pub fn get_state(&self, name: &str) -> EnvironmentState {
        EnvironmentState {
            name: name.to_string(),
            status: "ready".to_string(),
            components: vec![],
            exposures: vec![],
        }
    }
}

impl EnvironmentController {
    pub fn instance_name(&self, spec: &EnvironmentSpec) -> String {
        format!("mz-{}", spec.name)
    }

    pub async fn launch(
        &self,
        spec: &EnvironmentSpec,
        driver: &dyn crate::drivers::EnvDriver,
    ) -> menzi_common::Result<LaunchResult> {
        let instance = self.instance_name(spec);
        let image = spec
            .components
            .iter()
            .find_map(|component| component.image.clone())
            .unwrap_or_else(|| "ubuntu/24.04".to_string());
        let profiles = vec!["default".to_string()];
        driver.create_instance(&instance, &image, &profiles).await?;
        driver.start_instance(&instance).await?;

        let plan = self.plan_launch(spec);
        let mut components = Vec::new();
        for step in &plan.steps {
            match step.action {
                LaunchAction::Run => {
                    let command: Vec<String> = step
                        .command
                        .split_whitespace()
                        .map(ToOwned::to_owned)
                        .collect();
                    match driver.exec(&instance, &command).await {
                        Ok(result) => components.push(ComponentResult {
                            name: step.component.clone(),
                            success: result.exit_code == 0,
                            message: format!("exit code {}", result.exit_code),
                        }),
                        Err(error) => components.push(ComponentResult {
                            name: step.component.clone(),
                            success: false,
                            message: error.to_string(),
                        }),
                    }
                }
                _ => components.push(ComponentResult {
                    name: step.component.clone(),
                    success: true,
                    message: "started".to_string(),
                }),
            }
        }

        Ok(LaunchResult {
            success: components.iter().all(|component| component.success),
            components,
            exposures: self
                .exposures(spec)
                .into_iter()
                .map(|exposure| ExposureResult {
                    name: exposure.name,
                    url: exposure.url,
                    as_type: exposure.as_type,
                })
                .collect(),
        })
    }

    pub async fn status(
        &self,
        spec: &EnvironmentSpec,
        driver: &dyn crate::drivers::EnvDriver,
    ) -> menzi_common::Result<EnvironmentState> {
        let instance = self.instance_name(spec);
        let mut components = Vec::new();
        for component in &spec.components {
            let status = driver
                .instance_status(&instance)
                .await
                .unwrap_or_else(|_| "unknown".to_string())
                .to_lowercase();
            components.push(ComponentState {
                name: component.name.clone(),
                status: status.clone(),
                health: if status == "running" {
                    "healthy".to_string()
                } else {
                    "degraded".to_string()
                },
            });
        }
        let all_running = components
            .iter()
            .all(|component| component.status == "running");
        Ok(EnvironmentState {
            name: spec.name.clone(),
            status: if all_running {
                "ready".to_string()
            } else {
                "degraded".to_string()
            },
            components,
            exposures: self.exposures(spec),
        })
    }

    pub async fn relaunch(
        &self,
        spec: &EnvironmentSpec,
        changes: &ChangeSet,
        driver: &dyn crate::drivers::EnvDriver,
    ) -> menzi_common::Result<RelaunchDiff> {
        let diff = self.compute_relaunch_diff(spec, changes);
        if diff.is_empty() {
            return Ok(diff);
        }
        let instance = self.instance_name(spec);
        driver.stop_instance(&instance, true).await?;
        driver.start_instance(&instance).await?;
        Ok(diff)
    }

    pub async fn exec(
        &self,
        spec: &EnvironmentSpec,
        _component: &str,
        command: &[String],
        driver: &dyn crate::drivers::EnvDriver,
    ) -> menzi_common::Result<menzi_lxd::ExecResult> {
        let instance = self.instance_name(spec);
        driver.exec(&instance, command).await
    }

    fn exposures(&self, spec: &EnvironmentSpec) -> Vec<ExposureState> {
        spec.components
            .iter()
            .flat_map(|component| {
                component.expose.iter().map(|expose| ExposureState {
                    name: format!("{}-{}", component.name, expose.name),
                    url: format!("http://{}-{}.dev.local", component.name, expose.name),
                    as_type: expose_type_str(expose.as_type).to_string(),
                })
            })
            .collect()
    }
}

fn expose_type_str(expose_type: ExposeType) -> &'static str {
    match expose_type {
        ExposeType::Http => "http",
        ExposeType::Tcp => "tcp",
        ExposeType::Terminal => "terminal",
        ExposeType::Logs => "logs",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_controller_plans_launch() {
        let controller = EnvironmentController::new();
        let spec = EnvironmentSpec {
            name: "dev".to_string(),
            components: vec![
                ComponentSpec {
                    name: "db".to_string(),
                    kind: ComponentKind::Service,
                    image: Some("postgres-16".to_string()),
                    run: None,
                    ready: None,
                    expose: vec![],
                    after: vec![],
                    placement: Placement::OwnInstance,
                },
                ComponentSpec {
                    name: "daemon".to_string(),
                    kind: ComponentKind::Service,
                    image: None,
                    run: Some("daemon serve".to_string()),
                    ready: None,
                    expose: vec![],
                    after: vec!["db".to_string()],
                    placement: Placement::OwnInstance,
                },
            ],
        };
        let plan = controller.plan_launch(&spec);
        assert_eq!(plan.total_steps, 2);
    }

    #[test]
    fn environment_controller_computes_relaunch_diff() {
        let controller = EnvironmentController::new();
        let spec = EnvironmentSpec {
            name: "dev".to_string(),
            components: vec![ComponentSpec {
                name: "daemon".to_string(),
                kind: ComponentKind::Service,
                image: None,
                run: Some("daemon serve".to_string()),
                ready: None,
                expose: vec![],
                after: vec![],
                placement: Placement::OwnInstance,
            }],
        };
        let changes = ChangeSet {
            changed_paths: vec!["src/main.rs".to_string()],
            changed_inputs: vec!["daemon".to_string()],
        };
        let diff = controller.compute_relaunch_diff(&spec, &changes);
        assert!(!diff.is_empty());
        assert_eq!(diff.components_to_rebuild.len(), 1);
    }

    #[test]
    fn environment_controller_gets_state() {
        let controller = EnvironmentController::new();
        let state = controller.get_state("dev");
        assert_eq!(state.name, "dev");
        assert_eq!(state.status, "ready");
    }

    #[test]
    fn launch_plan_serializes() {
        let plan = LaunchPlan {
            steps: vec![LaunchStep {
                component: "db".to_string(),
                action: LaunchAction::Start,
                depends_on: vec![],
                command: String::new(),
            }],
            total_steps: 1,
        };
        let json = serde_json::to_string(&plan).unwrap();
        assert!(json.contains("\"total_steps\":1"));
    }

    #[test]
    fn launch_action_roundtrips() {
        assert_eq!(
            serde_json::to_string(&LaunchAction::Build).unwrap(),
            "\"build\""
        );
        assert_eq!(
            serde_json::to_string(&LaunchAction::Start).unwrap(),
            "\"start\""
        );
        assert_eq!(
            serde_json::to_string(&LaunchAction::Wait).unwrap(),
            "\"wait\""
        );
        assert_eq!(
            serde_json::to_string(&LaunchAction::Run).unwrap(),
            "\"run\""
        );
        assert_eq!(
            serde_json::to_string(&LaunchAction::Expose).unwrap(),
            "\"expose\""
        );
    }

    #[test]
    fn launch_result_serializes() {
        let result = LaunchResult {
            success: true,
            components: vec![ComponentResult {
                name: "db".to_string(),
                success: true,
                message: "Started".to_string(),
            }],
            exposures: vec![],
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn relaunch_diff_serializes() {
        let diff = RelaunchDiff {
            changed_inputs: vec!["daemon".to_string()],
            components_to_rebuild: vec!["daemon".to_string()],
            components_to_restart: vec!["daemon".to_string()],
            components_to_skip: vec!["db".to_string()],
        };
        let json = serde_json::to_string(&diff).unwrap();
        assert!(json.contains("daemon"));
    }

    #[test]
    fn environment_state_serializes() {
        let state = EnvironmentState {
            name: "dev".to_string(),
            status: "ready".to_string(),
            components: vec![ComponentState {
                name: "db".to_string(),
                status: "running".to_string(),
                health: "healthy".to_string(),
            }],
            exposures: vec![],
        };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("dev"));
    }

    #[test]
    fn input_hash_serializes() {
        let hash = InputHash {
            component: "daemon".to_string(),
            input_path: "src/main.rs".to_string(),
            hash: "abc123".to_string(),
        };
        let json = serde_json::to_string(&hash).unwrap();
        assert!(json.contains("abc123"));
    }

    #[test]
    fn exposure_result_serializes() {
        let result = ExposureResult {
            name: "daemon-api".to_string(),
            url: "https://daemon.example.com".to_string(),
            as_type: "http".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("daemon-api"));
    }

    #[test]
    fn component_result_serializes() {
        let result = ComponentResult {
            name: "db".to_string(),
            success: true,
            message: "Started".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("db"));
    }

    #[test]
    fn component_state_serializes() {
        let state = ComponentState {
            name: "db".to_string(),
            status: "running".to_string(),
            health: "healthy".to_string(),
        };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("db"));
    }

    #[test]
    fn exposure_state_serializes() {
        let state = ExposureState {
            name: "daemon-api".to_string(),
            url: "https://daemon.example.com".to_string(),
            as_type: "http".to_string(),
        };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("daemon-api"));
    }

    #[derive(Default)]
    struct MockDriver {
        calls: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl crate::drivers::EnvDriver for MockDriver {
        async fn create_instance(
            &self,
            name: &str,
            _image: &str,
            _profiles: &[String],
        ) -> menzi_common::Result<()> {
            self.calls.lock().unwrap().push(format!("create:{name}"));
            Ok(())
        }

        async fn start_instance(&self, name: &str) -> menzi_common::Result<()> {
            self.calls.lock().unwrap().push(format!("start:{name}"));
            Ok(())
        }

        async fn stop_instance(&self, name: &str, force: bool) -> menzi_common::Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("stop:{name}:{force}"));
            Ok(())
        }

        async fn delete_instance(&self, name: &str) -> menzi_common::Result<()> {
            self.calls.lock().unwrap().push(format!("delete:{name}"));
            Ok(())
        }

        async fn exec(
            &self,
            name: &str,
            command: &[String],
        ) -> menzi_common::Result<menzi_lxd::ExecResult> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("exec:{name}:{}", command.join(" ")));
            Ok(menzi_lxd::ExecResult {
                exit_code: 0,
                stdout: "ok".to_string(),
                stderr: String::new(),
            })
        }

        async fn instance_status(&self, _name: &str) -> menzi_common::Result<String> {
            Ok("running".to_string())
        }

        async fn snapshot_instance(&self, name: &str, snapshot: &str) -> menzi_common::Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("snapshot:{name}:{snapshot}"));
            Ok(())
        }

        async fn copy_instance(&self, source: &str, dest: &str) -> menzi_common::Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("copy:{source}:{dest}"));
            Ok(())
        }
    }

    fn spec_with_run() -> EnvironmentSpec {
        EnvironmentSpec {
            name: "dev".to_string(),
            components: vec![
                ComponentSpec {
                    name: "db".to_string(),
                    kind: ComponentKind::Service,
                    image: Some("postgres-16".to_string()),
                    run: None,
                    ready: None,
                    expose: vec![ExposeSpec {
                        port: 5432,
                        as_type: ExposeType::Tcp,
                        name: "pg".to_string(),
                    }],
                    after: vec![],
                    placement: Placement::OwnInstance,
                },
                ComponentSpec {
                    name: "daemon".to_string(),
                    kind: ComponentKind::Service,
                    image: None,
                    run: Some("daemon serve".to_string()),
                    ready: None,
                    expose: vec![],
                    after: vec!["db".to_string()],
                    placement: Placement::OwnInstance,
                },
            ],
        }
    }

    #[tokio::test]
    async fn launch_creates_starts_and_runs_components() {
        let controller = EnvironmentController::new();
        let driver = MockDriver::default();
        let spec = spec_with_run();
        let result = controller.launch(&spec, &driver).await.unwrap();
        let calls = driver.calls.lock().unwrap();
        assert!(calls.iter().any(|call| call == "create:mz-dev"));
        assert!(calls.iter().any(|call| call == "start:mz-dev"));
        assert!(calls.iter().any(|call| call == "exec:mz-dev:daemon serve"));
        assert!(result.success);
        assert!(result.exposures.iter().any(|e| e.name == "db-pg"));
    }

    #[tokio::test]
    async fn launch_exec_failure_marks_component_failed() {
        let controller = EnvironmentController::new();
        let driver = FailingExecDriver::default();
        let spec = spec_with_run();
        let result = controller.launch(&spec, &driver).await.unwrap();
        assert!(!result.success);
        assert!(result.components.iter().any(|component| !component.success));
    }

    #[tokio::test]
    async fn status_reports_ready_when_running() {
        let controller = EnvironmentController::new();
        let driver = MockDriver::default();
        let spec = spec_with_run();
        let state = controller.status(&spec, &driver).await.unwrap();
        assert_eq!(state.status, "ready");
        assert_eq!(state.components.len(), 2);
        assert!(state.components.iter().all(|c| c.health == "healthy"));
    }

    #[tokio::test]
    async fn relaunch_restarts_changed_components() {
        let controller = EnvironmentController::new();
        let driver = MockDriver::default();
        let spec = spec_with_run();
        let changes = ChangeSet {
            changed_paths: vec!["src/main.rs".to_string()],
            changed_inputs: vec!["daemon".to_string()],
        };
        let diff = controller.relaunch(&spec, &changes, &driver).await.unwrap();
        let calls = driver.calls.lock().unwrap();
        assert!(calls.iter().any(|call| call == "stop:mz-dev:true"));
        assert!(calls.iter().any(|call| call == "start:mz-dev"));
        assert_eq!(diff.components_to_restart.len(), 1);
    }

    #[tokio::test]
    async fn relaunch_is_noop_without_changes() {
        let controller = EnvironmentController::new();
        let driver = MockDriver::default();
        let spec = spec_with_run();
        let changes = ChangeSet {
            changed_paths: vec![],
            changed_inputs: vec![],
        };
        let diff = controller.relaunch(&spec, &changes, &driver).await.unwrap();
        assert!(diff.is_empty());
        assert!(driver.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn exec_runs_command_on_instance() {
        let controller = EnvironmentController::new();
        let driver = MockDriver::default();
        let spec = spec_with_run();
        let command = vec!["ls".to_string(), "-la".to_string()];
        let result = controller
            .exec(&spec, "daemon", &command, &driver)
            .await
            .unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "ok");
    }

    #[derive(Default)]
    struct FailingExecDriver {
        calls: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl crate::drivers::EnvDriver for FailingExecDriver {
        async fn create_instance(
            &self,
            name: &str,
            _image: &str,
            _profiles: &[String],
        ) -> menzi_common::Result<()> {
            self.calls.lock().unwrap().push(format!("create:{name}"));
            Ok(())
        }

        async fn start_instance(&self, name: &str) -> menzi_common::Result<()> {
            self.calls.lock().unwrap().push(format!("start:{name}"));
            Ok(())
        }

        async fn stop_instance(&self, name: &str, force: bool) -> menzi_common::Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("stop:{name}:{force}"));
            Ok(())
        }

        async fn delete_instance(&self, name: &str) -> menzi_common::Result<()> {
            self.calls.lock().unwrap().push(format!("delete:{name}"));
            Ok(())
        }

        async fn exec(
            &self,
            name: &str,
            command: &[String],
        ) -> menzi_common::Result<menzi_lxd::ExecResult> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("exec:{name}:{}", command.join(" ")));
            Err(menzi_common::MenziError::Internal(anyhow::Error::msg(
                "command not found",
            )))
        }

        async fn instance_status(&self, _name: &str) -> menzi_common::Result<String> {
            Ok("running".to_string())
        }

        async fn snapshot_instance(
            &self,
            _name: &str,
            _snapshot: &str,
        ) -> menzi_common::Result<()> {
            Ok(())
        }

        async fn copy_instance(&self, _source: &str, _dest: &str) -> menzi_common::Result<()> {
            Ok(())
        }
    }
}
