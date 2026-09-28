#[derive(Debug, thiserror::Error)]
pub enum MenziError {
    #[error("authentication required")]
    Unauthorized,
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("validation: {0}")]
    Validation(String),
    #[error("internal: {0}")]
    Internal(#[from] anyhow::Error),
    #[error("database: {0}")]
    Database(String),
    #[error("lxd: {0}")]
    Lxd(String),
    #[error("gateway: {0}")]
    Gateway(String),
    #[error("event bus: {0}")]
    Bus(String),
}

pub type Result<T> = std::result::Result<T, MenziError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unauthorized_display() {
        let err = MenziError::Unauthorized;
        assert_eq!(format!("{}", err), "authentication required");
    }

    #[test]
    fn forbidden_display() {
        let err = MenziError::Forbidden("insufficient permissions".to_string());
        assert_eq!(format!("{}", err), "forbidden: insufficient permissions");
    }

    #[test]
    fn not_found_display() {
        let err = MenziError::NotFound("project".to_string());
        assert_eq!(format!("{}", err), "not found: project");
    }

    #[test]
    fn conflict_display() {
        let err = MenziError::Conflict("already exists".to_string());
        assert_eq!(format!("{}", err), "conflict: already exists");
    }

    #[test]
    fn validation_display() {
        let err = MenziError::Validation("invalid slug".to_string());
        assert_eq!(format!("{}", err), "validation: invalid slug");
    }

    #[test]
    fn lxd_display() {
        let err = MenziError::Lxd("connection refused".to_string());
        assert_eq!(format!("{}", err), "lxd: connection refused");
    }

    #[test]
    fn gateway_display() {
        let err = MenziError::Gateway("provider timeout".to_string());
        assert_eq!(format!("{}", err), "gateway: provider timeout");
    }

    #[test]
    fn bus_display() {
        let err = MenziError::Bus("connection refused".to_string());
        assert_eq!(format!("{}", err), "event bus: connection refused");
    }

    #[test]
    fn result_ok() {
        let result: Result<i32> = Ok(42);
        assert_eq!(result.ok(), Some(42));
    }

    #[test]
    fn result_err() {
        let result: Result<i32> = Err(MenziError::Unauthorized);
        assert!(result.is_err());
    }
}
