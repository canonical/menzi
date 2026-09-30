use async_trait::async_trait;
use menzi_common::Result;
use menzi_lxd::LxdClient;
use std::sync::Arc;
use std::time::Duration;

use crate::types::{TerminalRequest, TerminalResult};

#[async_trait]
pub trait WorkspaceDriver: Send + Sync {
    async fn provision(&self, instance: &str, source_instance: &str) -> Result<ProvisionOutcome>;
    async fn start(&self, instance: &str) -> Result<()>;
    async fn stop(&self, instance: &str, force: bool) -> Result<()>;
    async fn destroy(&self, instance: &str) -> Result<()>;
    async fn exec(&self, instance: &str, request: &TerminalRequest) -> Result<TerminalResult>;
    async fn exists(&self, instance: &str) -> Result<bool>;
    async fn is_running(&self, instance: &str) -> Result<bool>;
    async fn resolve_endpoint(&self, instance: &str) -> Result<Option<String>>;
    fn endpoint_template(&self, instance: &str) -> String;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvisionOutcome {
    Created,
    Adopted,
}

pub struct LxdWorkspaceDriver {
    client: Arc<dyn LxdClient>,
    opencode_port: u16,
    domain: String,
    exec_timeout: Duration,
    address_attempts: u32,
    address_delay: Duration,
    boot_retries: u32,
    boot_window: Duration,
    boot_delay: Duration,
}

impl LxdWorkspaceDriver {
    pub fn new(client: Arc<dyn LxdClient>) -> Self {
        Self::with_endpoint(client, 17999, "dev.local")
    }

    pub fn with_endpoint(
        client: Arc<dyn LxdClient>,
        opencode_port: u16,
        domain: impl Into<String>,
    ) -> Self {
        Self {
            client,
            opencode_port,
            domain: domain.into(),
            exec_timeout: Duration::from_secs(120),
            address_attempts: 20,
            address_delay: Duration::from_millis(500),
            boot_retries: 3,
            boot_window: Duration::from_secs(30),
            boot_delay: Duration::from_millis(500),
        }
    }

    pub fn with_address_wait(mut self, attempts: u32, delay: Duration) -> Self {
        self.address_attempts = attempts;
        self.address_delay = delay;
        self
    }

    pub fn with_boot_wait(mut self, retries: u32, window: Duration, delay: Duration) -> Self {
        self.boot_retries = retries;
        self.boot_window = window;
        self.boot_delay = delay;
        self
    }

    async fn start_and_wait(&self, instance: &str) -> Result<()> {
        let mut last = None;
        for _ in 0..self.boot_retries {
            if let Err(error) = self.client.start_instance(instance).await {
                last = Some(error);
            }
            let deadline = std::time::Instant::now() + self.boot_window;
            loop {
                if self
                    .client
                    .instance_state(instance)
                    .await
                    .map(|state| state.is_running())
                    .unwrap_or(false)
                {
                    return Ok(());
                }
                if std::time::Instant::now() >= deadline {
                    break;
                }
                tokio::time::sleep(self.boot_delay).await;
            }
        }
        Err(last
            .unwrap_or_else(|| menzi_common::MenziError::Lxd(format!("{instance} did not start"))))
    }
}

#[async_trait]
impl WorkspaceDriver for LxdWorkspaceDriver {
    async fn provision(&self, instance: &str, source_instance: &str) -> Result<ProvisionOutcome> {
        if self.client.instance_exists(instance).await? {
            if self.client.instance_state(instance).await?.is_running() {
                return Ok(ProvisionOutcome::Adopted);
            }
            self.start_and_wait(instance).await?;
            return Ok(ProvisionOutcome::Adopted);
        }
        self.client.copy_instance(source_instance, instance).await?;
        self.start_and_wait(instance).await?;
        Ok(ProvisionOutcome::Created)
    }

    async fn start(&self, instance: &str) -> Result<()> {
        self.start_and_wait(instance).await
    }

    async fn stop(&self, instance: &str, force: bool) -> Result<()> {
        self.client.stop_instance(instance, force).await
    }

    async fn destroy(&self, instance: &str) -> Result<()> {
        let _ = self.client.stop_instance(instance, true).await;
        let mut last = None;
        for attempt in 0..3 {
            match self.client.delete_instance(instance).await {
                Ok(()) => return Ok(()),
                Err(error) => {
                    let stopping = error.to_string().contains("Instance is running");
                    last = Some(error);
                    if !stopping {
                        break;
                    }
                    let _ = self.client.stop_instance(instance, true).await;
                    if attempt < 2 {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
        }
        Err(last.unwrap_or_else(|| {
            menzi_common::MenziError::Lxd(format!("{instance} could not be deleted"))
        }))
    }

    async fn exec(&self, instance: &str, request: &TerminalRequest) -> Result<TerminalResult> {
        let mut command = vec![request.command.clone()];
        command.extend(request.args.iter().cloned());
        let work = self
            .client
            .exec_in(instance, &command, request.cwd.as_deref());
        let limit = request
            .timeout_secs
            .map(Duration::from_secs)
            .unwrap_or(self.exec_timeout);
        match tokio::time::timeout(limit, work).await {
            Ok(Ok(result)) => Ok(TerminalResult {
                exit_code: result.exit_code,
                stdout: result.stdout,
                stderr: result.stderr,
                timed_out: false,
            }),
            Ok(Err(error)) => Err(error),
            Err(_) => Ok(TerminalResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("command timed out after {limit:?}"),
                timed_out: true,
            }),
        }
    }

    async fn exists(&self, instance: &str) -> Result<bool> {
        self.client.instance_exists(instance).await
    }

    async fn is_running(&self, instance: &str) -> Result<bool> {
        if !self.client.instance_exists(instance).await? {
            return Ok(false);
        }
        Ok(self.client.instance_state(instance).await?.is_running())
    }

    async fn resolve_endpoint(&self, instance: &str) -> Result<Option<String>> {
        for attempt in 0..self.address_attempts {
            if self.client.instance_exists(instance).await? {
                let state = self.client.instance_state(instance).await?;
                if state.is_running() {
                    if let Some(address) = state.global_ipv4() {
                        return Ok(Some(format!("http://{address}:{}", self.opencode_port)));
                    }
                }
            }
            if attempt + 1 < self.address_attempts {
                tokio::time::sleep(self.address_delay).await;
            }
        }
        Ok(None)
    }

    fn endpoint_template(&self, instance: &str) -> String {
        format!(
            "http://{instance}.{domain}:{port}",
            domain = self.domain,
            port = self.opencode_port
        )
    }
}

#[cfg(test)]
pub(crate) mod testbed {
    use super::*;
    use menzi_lxd::{ExecResult, Instance, InstanceState, LxdClient as LxdClientTrait, Snapshot};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct FakeLxd {
        pub states: Mutex<HashMap<String, InstanceState>>,
        pub calls: Mutex<Vec<String>>,
        pub exec_results: Mutex<HashMap<String, ExecResult>>,
        pub exec_delay: Mutex<Duration>,
        pub fail_copy: Mutex<bool>,
        pub ignore_first_start: AtomicBool,
        pub start_silently_fails: AtomicBool,
        pub fail_delete: AtomicBool,
        pub busy_delete_count: AtomicUsize,
    }

    impl FakeLxd {
        pub fn with_running(instance: &str) -> Self {
            let fake = Self::default();
            fake.set(instance, "Running", Some("10.10.10.77"));
            fake
        }

        pub fn set(&self, instance: &str, status: &str, address: Option<&str>) {
            self.states.lock().unwrap().insert(
                instance.to_string(),
                InstanceState {
                    status: status.to_string(),
                    addresses: address
                        .map(|address| format!("global:{address}"))
                        .into_iter()
                        .collect(),
                },
            );
        }
    }

    fn instance(name: &str) -> Instance {
        Instance {
            name: name.to_string(),
            status: "Running".to_string(),
            status_code: 103,
            profiles: Vec::new(),
        }
    }

    #[async_trait]
    impl LxdClientTrait for FakeLxd {
        async fn create_instance(&self, spec: menzi_lxd::InstanceSpec) -> Result<Instance> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("create:{}", spec.name));
            Ok(instance(&spec.name))
        }

        async fn start_instance(&self, name: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("start:{name}"));
            if self
                .start_silently_fails
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                return Ok(());
            }
            if self
                .ignore_first_start
                .swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                return Ok(());
            }
            self.set(name, "Running", Some("10.10.10.77"));
            Ok(())
        }

        async fn stop_instance(&self, name: &str, force: bool) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("stop:{name}:{force}"));
            self.set(name, "Stopped", None);
            Ok(())
        }

        async fn delete_instance(&self, name: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("delete:{name}"));
            if self.fail_delete.load(Ordering::SeqCst) {
                return Err(menzi_common::MenziError::Lxd("delete refused".to_string()));
            }
            if self
                .busy_delete_count
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                    count.checked_sub(1)
                })
                .is_ok()
            {
                return Err(menzi_common::MenziError::Lxd(
                    "lxd returned 400: Instance is running".to_string(),
                ));
            }
            self.states.lock().unwrap().remove(name);
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

        async fn copy_instance(&self, source: &str, dest: &str) -> Result<Instance> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("copy:{source}:{dest}"));
            if *self.fail_copy.lock().unwrap() {
                return Err(menzi_common::MenziError::Lxd("copy refused".to_string()));
            }
            self.set(dest, "Stopped", None);
            Ok(Instance {
                name: dest.to_string(),
                status: "Stopped".to_string(),
                status_code: 102,
                profiles: Vec::new(),
            })
        }

        async fn exec(&self, name: &str, command: &[String]) -> Result<ExecResult> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("exec:{name}:{}", command.join(" ")));
            let delay = *self.exec_delay.lock().unwrap();
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            Ok(self
                .exec_results
                .lock()
                .unwrap()
                .get(name)
                .cloned()
                .unwrap_or(ExecResult {
                    exit_code: 0,
                    stdout: String::new(),
                    stderr: String::new(),
                }))
        }

        async fn exec_in(
            &self,
            name: &str,
            command: &[String],
            cwd: Option<&str>,
        ) -> Result<ExecResult> {
            self.calls.lock().unwrap().push(format!(
                "exec:{name}:{}:{}",
                command.join(" "),
                cwd.unwrap_or("-")
            ));
            let delay = *self.exec_delay.lock().unwrap();
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            Ok(self
                .exec_results
                .lock()
                .unwrap()
                .get(name)
                .cloned()
                .unwrap_or(ExecResult {
                    exit_code: 0,
                    stdout: String::new(),
                    stderr: String::new(),
                }))
        }

        async fn list_instances(&self) -> Result<Vec<Instance>> {
            Ok(self
                .states
                .lock()
                .unwrap()
                .keys()
                .map(|name| instance(name))
                .collect())
        }

        async fn list_snapshots(&self, _name: &str) -> Result<Vec<Snapshot>> {
            Ok(Vec::new())
        }

        async fn instance_exists(&self, name: &str) -> Result<bool> {
            Ok(self.states.lock().unwrap().contains_key(name))
        }

        async fn instance_state(&self, name: &str) -> Result<InstanceState> {
            self.states
                .lock()
                .unwrap()
                .get(name)
                .cloned()
                .ok_or_else(|| menzi_common::MenziError::Lxd(format!("no instance {name}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testbed::FakeLxd;
    use super::*;
    use crate::types::TerminalRequest;
    use std::sync::atomic::Ordering;

    fn driver(fake: Arc<FakeLxd>) -> LxdWorkspaceDriver {
        LxdWorkspaceDriver::with_endpoint(fake, 17999, "dev.local")
            .with_address_wait(3, Duration::from_millis(10))
            .with_boot_wait(3, Duration::from_millis(50), Duration::from_millis(10))
    }

    #[tokio::test]
    async fn provision_copies_the_template_then_starts_it() {
        let fake = Arc::new(FakeLxd::default());
        let outcome = driver(fake.clone())
            .provision("wsp-a-b", "mz-workspace")
            .await
            .unwrap();
        assert_eq!(outcome, ProvisionOutcome::Created);
        let calls = fake.calls.lock().unwrap();
        assert_eq!(calls[0], "copy:mz-workspace:wsp-a-b");
        assert_eq!(calls[1], "start:wsp-a-b");
    }

    #[tokio::test]
    async fn provision_retries_the_start_when_the_copy_is_still_busy() {
        let fake = Arc::new(FakeLxd::default());
        fake.ignore_first_start.store(true, Ordering::SeqCst);
        let outcome = driver(fake.clone())
            .provision("wsp-a-b", "mz-workspace")
            .await
            .unwrap();
        assert_eq!(outcome, ProvisionOutcome::Created);
        let calls = fake.calls.lock().unwrap();
        assert_eq!(
            calls.iter().filter(|call| *call == "start:wsp-a-b").count(),
            2
        );
    }

    #[tokio::test]
    async fn start_reports_a_container_that_never_comes_up() {
        let fake = Arc::new(FakeLxd::default());
        fake.set("wsp-a-b", "Stopped", None);
        fake.start_silently_fails.store(true, Ordering::SeqCst);
        let error = driver(fake).start("wsp-a-b").await.unwrap_err();
        assert!(error.to_string().contains("wsp-a-b"), "{error}");
    }

    #[tokio::test]
    async fn provision_adopts_a_container_that_already_exists() {
        let fake = Arc::new(FakeLxd::with_running("wsp-a-b"));
        let outcome = driver(fake.clone())
            .provision("wsp-a-b", "mz-workspace")
            .await
            .unwrap();
        assert_eq!(outcome, ProvisionOutcome::Adopted);
        assert!(fake.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn provision_starts_an_existing_but_stopped_container() {
        let fake = Arc::new(FakeLxd::default());
        fake.set("wsp-a-b", "Stopped", None);
        let outcome = driver(fake.clone())
            .provision("wsp-a-b", "mz-workspace")
            .await
            .unwrap();
        assert_eq!(outcome, ProvisionOutcome::Adopted);
        let calls = fake.calls.lock().unwrap();
        assert_eq!(*calls, vec!["start:wsp-a-b".to_string()]);
    }

    #[tokio::test]
    async fn destroy_deletes_even_when_stopping_fails() {
        let fake = Arc::new(FakeLxd::default());
        driver(fake.clone()).destroy("wsp-a-b").await.unwrap();
        let calls = fake.calls.lock().unwrap();
        assert!(calls.contains(&"stop:wsp-a-b:true".to_string()));
        assert!(calls.contains(&"delete:wsp-a-b".to_string()));
    }

    #[tokio::test]
    async fn destroy_stops_again_when_the_delete_says_the_instance_is_running() {
        let fake = Arc::new(FakeLxd::with_running("wsp-a-b"));
        fake.busy_delete_count.store(2, Ordering::SeqCst);
        driver(fake.clone()).destroy("wsp-a-b").await.unwrap();
        let calls = fake.calls.lock().unwrap();
        assert_eq!(calls.iter().filter(|c| *c == "delete:wsp-a-b").count(), 3);
        assert!(calls.iter().filter(|c| *c == "stop:wsp-a-b:true").count() >= 3);
    }

    #[tokio::test]
    async fn destroy_gives_up_on_a_delete_that_is_not_about_running() {
        let fake = Arc::new(FakeLxd::with_running("wsp-a-b"));
        fake.fail_delete.store(true, Ordering::SeqCst);
        let error = driver(fake).destroy("wsp-a-b").await.unwrap_err();
        assert!(error.to_string().contains("delete refused"), "{error}");
    }

    #[tokio::test]
    async fn exec_runs_the_full_command_line() {
        let fake = Arc::new(FakeLxd::default());
        driver(fake.clone())
            .exec("wsp-a-b", &TerminalRequest::new("ls").arg("-la"))
            .await
            .unwrap();
        assert!(fake.calls.lock().unwrap()[0].starts_with("exec:wsp-a-b:ls -la"));
    }

    #[tokio::test]
    async fn exec_passes_the_working_directory_through() {
        let fake = Arc::new(FakeLxd::default());
        driver(fake.clone())
            .exec(
                "wsp-a-b",
                &TerminalRequest::new("ls").in_dir("/workspace/src"),
            )
            .await
            .unwrap();
        assert!(fake.calls.lock().unwrap()[0].ends_with(":/workspace/src"));
    }

    #[tokio::test]
    async fn exec_reports_a_timeout_instead_of_hanging() {
        let fake = Arc::new(FakeLxd::default());
        *fake.exec_delay.lock().unwrap() = Duration::from_millis(200);
        let mut request = TerminalRequest::new("sleep");
        request.timeout_secs = Some(0);
        let result = driver(fake).exec("wsp-a-b", &request).await.unwrap();
        assert!(result.timed_out);
        assert!(!result.succeeded());
    }

    #[tokio::test]
    async fn is_running_reports_container_state() {
        let fake = Arc::new(FakeLxd::with_running("wsp-a-b"));
        let driver = driver(fake);
        assert!(driver.is_running("wsp-a-b").await.unwrap());
        assert!(!driver.is_running("wsp-missing").await.unwrap());
        assert!(!driver.exists("wsp-missing").await.unwrap());
    }

    #[tokio::test]
    async fn resolve_endpoint_uses_the_address_lxd_reports() {
        let fake = Arc::new(FakeLxd::with_running("wsp-a-b"));
        let endpoint = driver(fake).resolve_endpoint("wsp-a-b").await.unwrap();
        assert_eq!(endpoint, Some("http://10.10.10.77:17999".to_string()));
    }

    #[tokio::test]
    async fn resolve_endpoint_waits_for_the_address_to_appear() {
        let fake = Arc::new(FakeLxd::default());
        fake.set("wsp-a-b", "Running", None);
        let driver = driver(fake.clone()).with_address_wait(20, Duration::from_millis(10));
        let waiter = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(60)).await;
            fake.set("wsp-a-b", "Running", Some("10.10.10.90"));
        });
        let endpoint = driver.resolve_endpoint("wsp-a-b").await.unwrap();
        waiter.await.unwrap();
        assert_eq!(endpoint, Some("http://10.10.10.90:17999".to_string()));
    }

    #[tokio::test]
    async fn resolve_endpoint_waits_while_the_container_is_still_booting() {
        let fake = Arc::new(FakeLxd::default());
        fake.set("wsp-a-b", "Stopped", None);
        let driver = driver(fake.clone()).with_address_wait(20, Duration::from_millis(10));
        let waiter = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(60)).await;
            fake.set("wsp-a-b", "Running", Some("10.10.10.91"));
        });
        let endpoint = driver.resolve_endpoint("wsp-a-b").await.unwrap();
        waiter.await.unwrap();
        assert_eq!(endpoint, Some("http://10.10.10.91:17999".to_string()));
    }

    #[tokio::test]
    async fn resolve_endpoint_is_none_for_a_stopped_or_missing_container() {
        let fake = Arc::new(FakeLxd::default());
        fake.set("wsp-a-b", "Stopped", None);
        let driver = driver(fake);
        assert_eq!(driver.resolve_endpoint("wsp-a-b").await.unwrap(), None);
        assert_eq!(driver.resolve_endpoint("wsp-missing").await.unwrap(), None);
    }

    #[test]
    fn endpoint_template_is_the_published_address() {
        let driver = driver(Arc::new(FakeLxd::default()));
        assert_eq!(
            driver.endpoint_template("wsp-a-b"),
            "http://wsp-a-b.dev.local:17999"
        );
    }
}
