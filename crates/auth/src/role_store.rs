use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::role::Role;

#[async_trait]
pub trait RoleStore: Send + Sync {
    async fn highest_for(&self, user_id: Uuid) -> Option<String>;
    async fn role_in_project(&self, user_id: Uuid, project_id: Uuid) -> Option<Role>;
    async fn is_member(&self, user_id: Uuid, project_id: Uuid) -> bool;
    async fn org_of(&self, user_id: Uuid) -> Option<Uuid>;
    async fn set_org(&self, user_id: Uuid, org_id: Uuid) -> Result<(), sqlx::Error>;
}

#[derive(Default)]
pub struct InMemoryRoleStore {
    roles: Mutex<HashMap<(Uuid, Uuid), Role>>,
    orgs: Mutex<HashMap<Uuid, Uuid>>,
}

impl InMemoryRoleStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn grant(&self, user_id: Uuid, project_id: Uuid, role: Role) {
        self.roles
            .lock()
            .expect("role store lock")
            .insert((user_id, project_id), role);
    }

    pub fn set_org_for(&self, user_id: Uuid, org_id: Uuid) {
        self.orgs
            .lock()
            .expect("role store lock")
            .insert(user_id, org_id);
    }
}

fn rank(role: &Role) -> u8 {
    match role {
        Role::Reviewer => 0,
        Role::Developer => 1,
        Role::DesignApprover => 2,
        Role::Maintainer => 3,
        Role::Owner => 4,
    }
}

#[async_trait]
impl RoleStore for InMemoryRoleStore {
    async fn highest_for(&self, user_id: Uuid) -> Option<String> {
        let roles = self.roles.lock().expect("role store lock");
        roles
            .iter()
            .filter(|((owner, _), _)| *owner == user_id)
            .map(|(_, role)| *role)
            .max_by_key(rank)
            .map(|role| role.as_str().to_string())
    }

    async fn role_in_project(&self, user_id: Uuid, project_id: Uuid) -> Option<Role> {
        self.roles
            .lock()
            .expect("role store lock")
            .get(&(user_id, project_id))
            .copied()
    }

    async fn is_member(&self, user_id: Uuid, project_id: Uuid) -> bool {
        self.roles
            .lock()
            .expect("role store lock")
            .contains_key(&(user_id, project_id))
    }

    async fn org_of(&self, user_id: Uuid) -> Option<Uuid> {
        self.orgs
            .lock()
            .expect("role store lock")
            .get(&user_id)
            .copied()
    }

    async fn set_org(&self, user_id: Uuid, org_id: Uuid) -> Result<(), sqlx::Error> {
        self.set_org_for(user_id, org_id);
        Ok(())
    }
}

pub struct PostgresRoleStore {
    pool: sqlx::PgPool,
}

impl PostgresRoleStore {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RoleStore for PostgresRoleStore {
    async fn highest_for(&self, user_id: Uuid) -> Option<String> {
        sqlx::query_as::<_, (String,)>(
            "SELECT pm.role
             FROM project_members pm
             JOIN projects p ON p.id = pm.project_id
             WHERE pm.user_id = $1
             ORDER BY
                CASE pm.role
                    WHEN 'owner' THEN 4
                    WHEN 'maintainer' THEN 3
                    WHEN 'design_approver' THEN 2
                    WHEN 'developer' THEN 1
                    ELSE 0
                END DESC
             LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
        .map(|row| row.0)
    }

    async fn role_in_project(&self, user_id: Uuid, project_id: Uuid) -> Option<Role> {
        let row = sqlx::query_as::<_, (String,)>(
            "SELECT role FROM project_members WHERE user_id = $1 AND project_id = $2",
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()?;
        Role::parse(&row.0)
    }

    async fn is_member(&self, user_id: Uuid, project_id: Uuid) -> bool {
        sqlx::query_scalar::<_, i64>(
            "SELECT 1 FROM project_members WHERE user_id = $1 AND project_id = $2",
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
        .is_some()
    }

    async fn org_of(&self, user_id: Uuid) -> Option<Uuid> {
        sqlx::query_scalar::<_, Uuid>("SELECT org_id FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
    }

    async fn set_org(&self, user_id: Uuid, org_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET org_id = $2 WHERE id = $1 AND org_id IS NULL")
            .bind(user_id)
            .bind(org_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_highest_role_wins() {
        let store = InMemoryRoleStore::new();
        let user = Uuid::new_v4();
        let weak = Uuid::new_v4();
        let strong = Uuid::new_v4();
        store.grant(user, weak, Role::Reviewer);
        store.grant(user, strong, Role::Maintainer);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert_eq!(
            runtime.block_on(store.highest_for(user)),
            Some("maintainer".to_string())
        );
    }

    #[test]
    fn an_owner_outranks_a_maintainer() {
        assert!(rank(&Role::Owner) > rank(&Role::Maintainer));
        assert!(rank(&Role::Maintainer) > rank(&Role::Developer));
        assert!(rank(&Role::Developer) > rank(&Role::Reviewer));
    }

    #[test]
    fn a_user_with_no_roles_has_none() {
        let store = InMemoryRoleStore::new();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert_eq!(runtime.block_on(store.highest_for(Uuid::new_v4())), None);
    }

    #[test]
    fn membership_is_per_project() {
        let store = InMemoryRoleStore::new();
        let user = Uuid::new_v4();
        let member_of = Uuid::new_v4();
        let other = Uuid::new_v4();
        store.grant(user, member_of, Role::Developer);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert!(runtime.block_on(store.is_member(user, member_of)));
        assert!(!runtime.block_on(store.is_member(user, other)));
        assert_eq!(
            runtime.block_on(store.role_in_project(user, member_of)),
            Some(Role::Developer)
        );
    }

    #[test]
    fn an_org_is_remembered_per_user() {
        let store = InMemoryRoleStore::new();
        let user = Uuid::new_v4();
        let org = Uuid::new_v4();
        store.set_org_for(user, org);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert_eq!(runtime.block_on(store.org_of(user)), Some(org));
        assert_eq!(runtime.block_on(store.org_of(Uuid::new_v4())), None);
    }
}
