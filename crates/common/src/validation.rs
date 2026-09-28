use crate::MenziError;

pub fn validate_not_empty(field: &str, value: &str) -> crate::Result<()> {
    if value.trim().is_empty() {
        Err(MenziError::Validation(format!(
            "{} must not be empty",
            field
        )))
    } else {
        Ok(())
    }
}

pub fn validate_max_length(field: &str, value: &str, max: usize) -> crate::Result<()> {
    if value.len() > max {
        Err(MenziError::Validation(format!(
            "{} must not exceed {} characters",
            field, max
        )))
    } else {
        Ok(())
    }
}

pub fn validate_slug(field: &str, value: &str) -> crate::Result<()> {
    if !value
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        || value.starts_with('-')
        || value.ends_with('-')
    {
        Err(MenziError::Validation(format!(
            "{} must be a valid slug (lowercase alphanumeric and hyphens)",
            field
        )))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_not_empty_accepts_non_empty() {
        assert!(validate_not_empty("name", "hello").is_ok());
    }

    #[test]
    fn validate_not_empty_rejects_empty() {
        let result = validate_not_empty("name", "");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MenziError::Validation(_)));
    }

    #[test]
    fn validate_not_empty_rejects_whitespace() {
        let result = validate_not_empty("name", "   ");
        assert!(result.is_err());
    }

    #[test]
    fn validate_max_length_accepts_within_limit() {
        assert!(validate_max_length("name", "hello", 10).is_ok());
    }

    #[test]
    fn validate_max_length_rejects_over_limit() {
        let result = validate_max_length("name", "hello world", 5);
        assert!(result.is_err());
    }

    #[test]
    fn validate_slug_accepts_valid() {
        assert!(validate_slug("slug", "my-project").is_ok());
        assert!(validate_slug("slug", "project123").is_ok());
        assert!(validate_slug("slug", "a-b-c").is_ok());
    }

    #[test]
    fn validate_slug_rejects_uppercase() {
        assert!(validate_slug("slug", "MyProject").is_err());
    }

    #[test]
    fn validate_slug_rejects_leading_hyphen() {
        assert!(validate_slug("slug", "-project").is_err());
    }

    #[test]
    fn validate_slug_rejects_trailing_hyphen() {
        assert!(validate_slug("slug", "project-").is_err());
    }

    #[test]
    fn validate_slug_rejects_special_chars() {
        assert!(validate_slug("slug", "my_project").is_err());
        assert!(validate_slug("slug", "my.project").is_err());
    }
}
