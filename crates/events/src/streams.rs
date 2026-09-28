pub const SESSION: &str = "menzi.sessions";
pub const ENVIRONMENT: &str = "menzi.environments";
pub const PREVIEW: &str = "menzi.previews";
pub const DESIGN: &str = "menzi.design";
pub const GIT: &str = "menzi.git";
pub const NOTIFICATION: &str = "menzi.notifications";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_have_correct_names() {
        assert_eq!(SESSION, "menzi.sessions");
        assert_eq!(ENVIRONMENT, "menzi.environments");
        assert_eq!(PREVIEW, "menzi.previews");
        assert_eq!(DESIGN, "menzi.design");
        assert_eq!(GIT, "menzi.git");
        assert_eq!(NOTIFICATION, "menzi.notifications");
    }
}
