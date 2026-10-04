use async_trait::async_trait;
use menzi_common::{MenziError, Result};
use sqlx::postgres::PgPool;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::types::{BudgetStatus, DataClassification, PolicyConfig, UsageRecord};

#[async_trait]
pub trait GatewayStateStore: Send + Sync {
    async fn is_model_allowed(&self, scope: &str, model: &str) -> Result<bool>;
    async fn can_spend(&self, key: &str, amount: f64) -> Result<bool>;
    async fn record_spend(&self, key: &str, amount: f64) -> Result<()>;
    async fn budget(&self, key: &str) -> Result<Option<BudgetStatus>>;
    async fn record_usage(&self, key: &str, record: &UsageRecord) -> Result<()>;
}

#[derive(Default)]
pub struct InMemoryGatewayStateStore {
    policies: Mutex<HashMap<String, PolicyConfig>>,
    budgets: Mutex<HashMap<String, BudgetStatus>>,
}

impl InMemoryGatewayStateStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_policy(mut self, scope: String, config: PolicyConfig) -> Self {
        self.policies
            .get_mut()
            .expect("policy lock")
            .insert(scope, config);
        self
    }

    pub fn with_budget(mut self, key: String, budget: BudgetStatus) -> Self {
        self.budgets
            .get_mut()
            .expect("budget lock")
            .insert(key, budget);
        self
    }
}

fn model_matches(model: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if pattern.ends_with("/*") {
        let prefix = pattern.strip_suffix("/*").unwrap_or(pattern);
        return model.starts_with(prefix);
    }
    if pattern.starts_with("*/") {
        let suffix = pattern.strip_prefix("*/").unwrap_or(pattern);
        let suffix_no_wildcard = suffix.trim_end_matches('*');
        return model.contains(suffix_no_wildcard);
    }
    model == pattern
}

#[async_trait]
impl GatewayStateStore for InMemoryGatewayStateStore {
    async fn is_model_allowed(&self, scope: &str, model: &str) -> Result<bool> {
        let policies = self.policies.lock().expect("policy lock");
        let Some(config) = policies.get(scope) else {
            return Ok(true);
        };
        for denied in &config.denied_models {
            if model_matches(model, denied) {
                return Ok(false);
            }
        }
        if config.allowed_models.is_empty() {
            return Ok(true);
        }
        Ok(config
            .allowed_models
            .iter()
            .any(|allowed| model_matches(model, allowed)))
    }

    async fn can_spend(&self, key: &str, amount: f64) -> Result<bool> {
        let budgets = self.budgets.lock().expect("budget lock");
        Ok(match budgets.get(key) {
            Some(budget) => budget.spent_usd + amount <= budget.budget_usd,
            None => true,
        })
    }

    async fn record_spend(&self, key: &str, amount: f64) -> Result<()> {
        let mut budgets = self.budgets.lock().expect("budget lock");
        let budget = budgets.entry(key.to_string()).or_insert(BudgetStatus {
            project_id: None,
            user_id: None,
            session_id: None,
            feature: "default".to_string(),
            spent_usd: 0.0,
            budget_usd: 0.0,
        });
        budget.spent_usd += amount;
        Ok(())
    }

    async fn budget(&self, key: &str) -> Result<Option<BudgetStatus>> {
        Ok(self.budgets.lock().expect("budget lock").get(key).cloned())
    }

    async fn record_usage(&self, _key: &str, _record: &UsageRecord) -> Result<()> {
        Ok(())
    }
}

pub struct PostgresGatewayStateStore {
    pool: PgPool,
}

impl PostgresGatewayStateStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("../db/migrations")
            .run(&self.pool)
            .await
            .map_err(|e| MenziError::Database(e.to_string()))
    }
}

#[async_trait]
impl GatewayStateStore for PostgresGatewayStateStore {
    async fn is_model_allowed(&self, scope: &str, model: &str) -> Result<bool> {
        let row = sqlx::query_as::<_, (serde_json::Value, serde_json::Value)>(
            "SELECT allowed_models, denied_models
             FROM llm_gateway_policies
             WHERE scope_key = $1",
        )
        .bind(scope)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        let Some((allowed, denied)) = row else {
            return Ok(true);
        };
        let denied_models: Vec<String> =
            serde_json::from_value(denied).unwrap_or_else(|_| Vec::new());
        if denied_models
            .iter()
            .any(|pattern| model_matches(model, pattern))
        {
            return Ok(false);
        }
        let allowed_models: Vec<String> =
            serde_json::from_value(allowed).unwrap_or_else(|_| Vec::new());
        if allowed_models.is_empty() {
            return Ok(true);
        }
        Ok(allowed_models
            .iter()
            .any(|pattern| model_matches(model, pattern)))
    }

    async fn can_spend(&self, key: &str, amount: f64) -> Result<bool> {
        let row = sqlx::query_as::<_, (f64, f64)>(
            "SELECT spent_usd::double precision, budget_usd::double precision
             FROM llm_gateway_budgets
             WHERE scope_key = $1",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        Ok(match row {
            Some((spent, budget)) => spent + amount <= budget,
            None => true,
        })
    }

    async fn record_spend(&self, key: &str, amount: f64) -> Result<()> {
        sqlx::query(
            "INSERT INTO llm_gateway_budgets
                (scope_key, feature, spent_usd, budget_usd)
             VALUES ($1, 'default', $2, 0)
             ON CONFLICT (scope_key) DO UPDATE SET
                spent_usd = llm_gateway_budgets.spent_usd + EXCLUDED.spent_usd,
                updated_at = now()",
        )
        .bind(key)
        .bind(amount)
        .execute(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        Ok(())
    }

    async fn budget(&self, key: &str) -> Result<Option<BudgetStatus>> {
        let row = sqlx::query_as::<
            _,
            (
                Option<uuid::Uuid>,
                Option<uuid::Uuid>,
                Option<uuid::Uuid>,
                String,
                f64,
                f64,
            ),
        >(
            "SELECT project_id, user_id, session_id, feature,
                    spent_usd::double precision, budget_usd::double precision
             FROM llm_gateway_budgets
             WHERE scope_key = $1",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        Ok(row.map(|row| BudgetStatus {
            project_id: row.0.map(menzi_common::ids::ProjectId::from_uuid),
            user_id: row.1.map(menzi_common::ids::UserId::from_uuid),
            session_id: row.2.map(menzi_common::ids::SessionId::from_uuid),
            feature: row.3,
            spent_usd: row.4,
            budget_usd: row.5,
        }))
    }

    async fn record_usage(&self, key: &str, record: &UsageRecord) -> Result<()> {
        sqlx::query(
            "INSERT INTO llm_gateway_usage
                (scope_key, project_id, user_id, session_id,
                 feature, provider, model, input_tokens, output_tokens, cost_usd)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(key)
        .bind(record.project_id.map(|v| *v.as_uuid()))
        .bind(record.user_id.map(|v| *v.as_uuid()))
        .bind(record.session_id.map(|v| *v.as_uuid()))
        .bind(&record.feature)
        .bind(&record.provider)
        .bind(&record.model)
        .bind(record.input_tokens)
        .bind(record.output_tokens)
        .bind(record.cost_usd)
        .execute(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        Ok(())
    }
}

pub fn classification_name(classification: DataClassification) -> &'static str {
    match classification {
        DataClassification::Public => "public",
        DataClassification::Internal => "internal",
        DataClassification::Restricted => "restricted",
    }
}
