use chrono::{DateTime, Utc};

pub fn now() -> DateTime<Utc> {
    Utc::now()
}

pub fn format_relative(dt: DateTime<Utc>) -> String {
    let duration = Utc::now().signed_duration_since(dt);
    let secs = duration.num_seconds();

    if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_relative_seconds() {
        let dt = Utc::now() - chrono::Duration::seconds(30);
        assert_eq!(format_relative(dt), "30s ago");
    }

    #[test]
    fn format_relative_minutes() {
        let dt = Utc::now() - chrono::Duration::minutes(5);
        assert_eq!(format_relative(dt), "5m ago");
    }

    #[test]
    fn format_relative_hours() {
        let dt = Utc::now() - chrono::Duration::hours(2);
        assert_eq!(format_relative(dt), "2h ago");
    }

    #[test]
    fn format_relative_days() {
        let dt = Utc::now() - chrono::Duration::days(3);
        assert_eq!(format_relative(dt), "3d ago");
    }
}
