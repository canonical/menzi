use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mail {
    pub to: String,
    pub subject: String,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailerError {
    Unavailable,
}

#[async_trait]
pub trait Mailer: Send + Sync {
    async fn send(&self, mail: Mail) -> Result<(), MailerError>;
}

#[derive(Default)]
pub struct LogMailer;

#[async_trait]
impl Mailer for LogMailer {
    async fn send(&self, mail: Mail) -> Result<(), MailerError> {
        tracing::info!(
            to = %mail.to,
            subject = %mail.subject,
            body = %mail.body,
            "mail not delivered, no transport is configured"
        );
        Ok(())
    }
}

#[derive(Default)]
pub struct StubMailer {
    sent: std::sync::Mutex<Vec<Mail>>,
}

impl StubMailer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sent(&self) -> Vec<Mail> {
        self.sent.lock().expect("mailer lock").clone()
    }

    pub fn last_to(&self) -> Option<String> {
        self.sent().last().map(|mail| mail.to.clone())
    }
}

#[async_trait]
impl Mailer for StubMailer {
    async fn send(&self, mail: Mail) -> Result<(), MailerError> {
        self.sent.lock().expect("mailer lock").push(mail);
        Ok(())
    }
}

pub fn mailer_from_env() -> std::sync::Arc<dyn Mailer> {
    match std::env::var("MENZI_MAILER").unwrap_or_default().as_str() {
        "stub" => std::sync::Arc::new(StubMailer::new()),
        _ => std::sync::Arc::new(LogMailer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mail(to: &str) -> Mail {
        Mail {
            to: to.to_string(),
            subject: "Reset".to_string(),
            body: "https://example.com/reset".to_string(),
        }
    }

    #[tokio::test]
    async fn the_stub_records_what_it_was_asked_to_send() {
        let mailer = StubMailer::new();
        assert!(mailer.sent().is_empty());
        mailer.send(mail("a@b")).await.unwrap();
        assert_eq!(mailer.sent().len(), 1);
        assert_eq!(mailer.last_to().as_deref(), Some("a@b"));
    }

    #[tokio::test]
    async fn the_stub_keeps_every_message_in_order() {
        let mailer = StubMailer::new();
        mailer.send(mail("first@b")).await.unwrap();
        mailer.send(mail("second@b")).await.unwrap();
        let to: Vec<String> = mailer.sent().into_iter().map(|m| m.to).collect();
        assert_eq!(to, vec!["first@b", "second@b"]);
    }

    #[tokio::test]
    async fn the_log_mailer_accepts_everything() {
        assert!(LogMailer.send(mail("a@b")).await.is_ok());
    }
}
