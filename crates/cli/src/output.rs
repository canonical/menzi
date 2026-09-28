#[derive(Debug, Default)]
pub struct TableFormatter;

impl TableFormatter {
    pub fn new() -> Self {
        Self
    }

    pub fn format(&self, headers: &[String], rows: &[Vec<String>]) -> String {
        let mut result = String::new();
        result.push_str(&headers.join(" | "));
        result.push('\n');
        result.push_str(&"-".repeat(result.len() - 1));
        result.push('\n');
        for row in rows {
            result.push_str(&row.join(" | "));
            result.push('\n');
        }
        result
    }
}

#[derive(Debug, Default)]
pub struct JsonFormatter;

impl JsonFormatter {
    pub fn new() -> Self {
        Self
    }

    pub fn format(&self, data: &serde_json::Value) -> String {
        serde_json::to_string_pretty(data).unwrap_or_default()
    }
}

#[derive(Debug, Default)]
pub struct PlainFormatter;

impl PlainFormatter {
    pub fn new() -> Self {
        Self
    }

    pub fn format(&self, text: &str) -> String {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_formatter_formats() {
        let formatter = TableFormatter::new();
        let output = formatter.format(
            &["Name".to_string(), "Status".to_string()],
            &[vec!["daemon".to_string(), "ready".to_string()]],
        );
        assert!(output.contains("daemon"));
    }

    #[test]
    fn json_formatter_formats() {
        let formatter = JsonFormatter::new();
        let output = formatter.format(&serde_json::json!({"key": "value"}));
        assert!(output.contains("\"key\": \"value\""));
    }

    #[test]
    fn plain_formatter_formats() {
        let formatter = PlainFormatter::new();
        let output = formatter.format("Hello");
        assert_eq!(output, "Hello");
    }
}
