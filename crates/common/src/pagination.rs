use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationParams {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

impl PaginationParams {
    pub fn limit_or_default(&self) -> u32 {
        self.limit.unwrap_or(50).min(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_params_default_limit() {
        let params = PaginationParams {
            cursor: None,
            limit: None,
        };
        assert_eq!(params.limit_or_default(), 50);
    }

    #[test]
    fn pagination_params_custom_limit() {
        let params = PaginationParams {
            cursor: None,
            limit: Some(25),
        };
        assert_eq!(params.limit_or_default(), 25);
    }

    #[test]
    fn pagination_params_max_limit_capped() {
        let params = PaginationParams {
            cursor: None,
            limit: Some(500),
        };
        assert_eq!(params.limit_or_default(), 100);
    }

    #[test]
    fn page_serializes_correctly() {
        let page = Page {
            items: vec![1, 2, 3],
            next_cursor: Some("abc".to_string()),
        };
        let json = serde_json::to_string(&page).unwrap();
        assert!(json.contains("\"items\":[1,2,3]"));
        assert!(json.contains("\"next_cursor\":\"abc\""));
    }
}
