use super::types::*;

pub struct BudgetManager {
    budgets: std::collections::HashMap<String, BudgetStatus>,
}

impl BudgetManager {
    pub fn new() -> Self {
        Self {
            budgets: std::collections::HashMap::new(),
        }
    }

    pub fn check_budget(&self, key: &str) -> Option<&BudgetStatus> {
        self.budgets.get(key)
    }

    pub fn can_spend(&self, key: &str, amount: f64) -> bool {
        match self.budgets.get(key) {
            Some(budget) => budget.spent_usd + amount <= budget.budget_usd,
            None => true,
        }
    }

    pub fn record_spend(&mut self, key: &str, amount: f64) {
        let budget = self.budgets.entry(key.to_string()).or_insert(BudgetStatus {
            project_id: None,
            user_id: None,
            session_id: None,
            feature: "default".to_string(),
            spent_usd: 0.0,
            budget_usd: 0.0,
        });
        budget.spent_usd += amount;
    }

    pub fn set_budget(&mut self, key: String, budget: BudgetStatus) {
        self.budgets.insert(key, budget);
    }

    pub fn export(&self) -> Vec<(String, BudgetStatus)> {
        self.budgets
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }
}

impl Default for BudgetManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_manager_allows_spend_under_budget() {
        let mut manager = BudgetManager::new();
        manager.set_budget(
            "user-1".to_string(),
            BudgetStatus {
                project_id: None,
                user_id: None,
                session_id: None,
                feature: "coding".to_string(),
                spent_usd: 50.0,
                budget_usd: 100.0,
            },
        );
        assert!(manager.can_spend("user-1", 30.0));
    }

    #[test]
    fn budget_manager_denies_spend_over_budget() {
        let mut manager = BudgetManager::new();
        manager.set_budget(
            "user-1".to_string(),
            BudgetStatus {
                project_id: None,
                user_id: None,
                session_id: None,
                feature: "coding".to_string(),
                spent_usd: 90.0,
                budget_usd: 100.0,
            },
        );
        assert!(!manager.can_spend("user-1", 20.0));
    }

    #[test]
    fn budget_manager_allows_when_no_budget_set() {
        let manager = BudgetManager::new();
        assert!(manager.can_spend("unknown", 100.0));
    }

    #[test]
    fn budget_manager_records_spend() {
        let mut manager = BudgetManager::new();
        manager.set_budget(
            "user-1".to_string(),
            BudgetStatus {
                project_id: None,
                user_id: None,
                session_id: None,
                feature: "coding".to_string(),
                spent_usd: 0.0,
                budget_usd: 100.0,
            },
        );
        manager.record_spend("user-1", 10.0);
        let budget = manager.check_budget("user-1").unwrap();
        assert_eq!(budget.spent_usd, 10.0);
    }

    #[test]
    fn budget_manager_check_returns_none_for_unknown() {
        let manager = BudgetManager::new();
        assert!(manager.check_budget("unknown").is_none());
    }
}
