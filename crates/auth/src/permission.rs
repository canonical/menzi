use menzi_common::ids::{ProjectId, UserId};

use crate::role::Role;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    ProjectRead,
    ProjectWrite,
    ProjectAdmin,
    SessionStart,
    SessionRead,
    SessionReadAll,
    PreviewCreate,
    PreviewShare,
    DesignRead,
    DesignPropose,
    DesignApprove,
    DesignWaiverGrant,
    Admin,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: UserId,
    pub email: String,
    pub name: String,
    pub roles: Vec<(ProjectId, Role)>,
}

impl AuthenticatedUser {
    pub fn role_in_project(&self, project_id: ProjectId) -> Option<Role> {
        self.roles
            .iter()
            .find(|(pid, _)| *pid == project_id)
            .map(|(_, role)| *role)
    }

    pub fn has_permission(&self, project_id: ProjectId, permission: Permission) -> bool {
        let role = match self.role_in_project(project_id) {
            Some(r) => r,
            None => return false,
        };

        match permission {
            Permission::ProjectRead => true,
            Permission::ProjectWrite => matches!(role, Role::Owner | Role::Maintainer),
            Permission::ProjectAdmin => matches!(role, Role::Owner),
            Permission::SessionStart => true,
            Permission::SessionRead => true,
            Permission::SessionReadAll => matches!(role, Role::Owner | Role::Maintainer),
            Permission::PreviewCreate => !matches!(role, Role::Reviewer),
            Permission::PreviewShare => {
                matches!(role, Role::Owner | Role::Maintainer | Role::Developer)
            }
            Permission::DesignRead => true,
            Permission::DesignPropose => true,
            Permission::DesignApprove => matches!(role, Role::Owner | Role::DesignApprover),
            Permission::DesignWaiverGrant => matches!(role, Role::Owner | Role::DesignApprover),
            Permission::Admin => matches!(role, Role::Owner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_has_all_permissions() {
        let user = AuthenticatedUser {
            user_id: UserId::new(),
            email: "owner@test.com".to_string(),
            name: "Owner".to_string(),
            roles: vec![(ProjectId::new(), Role::Owner)],
        };
        let pid = user.roles[0].0;
        assert!(user.has_permission(pid, Permission::ProjectRead));
        assert!(user.has_permission(pid, Permission::ProjectWrite));
        assert!(user.has_permission(pid, Permission::ProjectAdmin));
        assert!(user.has_permission(pid, Permission::SessionStart));
        assert!(user.has_permission(pid, Permission::SessionReadAll));
        assert!(user.has_permission(pid, Permission::PreviewCreate));
        assert!(user.has_permission(pid, Permission::PreviewShare));
        assert!(user.has_permission(pid, Permission::DesignApprove));
        assert!(user.has_permission(pid, Permission::DesignWaiverGrant));
        assert!(user.has_permission(pid, Permission::Admin));
    }

    #[test]
    fn reviewer_lacks_admin_permissions() {
        let user = AuthenticatedUser {
            user_id: UserId::new(),
            email: "reviewer@test.com".to_string(),
            name: "Reviewer".to_string(),
            roles: vec![(ProjectId::new(), Role::Reviewer)],
        };
        let pid = user.roles[0].0;
        assert!(user.has_permission(pid, Permission::ProjectRead));
        assert!(!user.has_permission(pid, Permission::ProjectWrite));
        assert!(!user.has_permission(pid, Permission::ProjectAdmin));
        assert!(!user.has_permission(pid, Permission::PreviewCreate));
        assert!(!user.has_permission(pid, Permission::DesignApprove));
        assert!(!user.has_permission(pid, Permission::Admin));
    }

    #[test]
    fn user_without_role_has_no_permissions() {
        let user = AuthenticatedUser {
            user_id: UserId::new(),
            email: "none@test.com".to_string(),
            name: "None".to_string(),
            roles: vec![],
        };
        assert!(!user.has_permission(ProjectId::new(), Permission::ProjectRead));
    }
}
