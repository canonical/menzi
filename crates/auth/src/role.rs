use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Owner,
    Maintainer,
    DesignApprover,
    Developer,
    Reviewer,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Maintainer => "maintainer",
            Role::DesignApprover => "design_approver",
            Role::Developer => "developer",
            Role::Reviewer => "reviewer",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "owner" => Some(Role::Owner),
            "maintainer" => Some(Role::Maintainer),
            "design_approver" => Some(Role::DesignApprover),
            "developer" => Some(Role::Developer),
            "reviewer" => Some(Role::Reviewer),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_as_str_roundtrips() {
        assert_eq!(Role::Owner.as_str(), "owner");
        assert_eq!(Role::Maintainer.as_str(), "maintainer");
        assert_eq!(Role::DesignApprover.as_str(), "design_approver");
        assert_eq!(Role::Developer.as_str(), "developer");
        assert_eq!(Role::Reviewer.as_str(), "reviewer");
    }

    #[test]
    fn role_from_str_parses_valid() {
        assert_eq!(Role::parse("owner"), Some(Role::Owner));
        assert_eq!(Role::parse("maintainer"), Some(Role::Maintainer));
        assert_eq!(Role::parse("design_approver"), Some(Role::DesignApprover));
        assert_eq!(Role::parse("developer"), Some(Role::Developer));
        assert_eq!(Role::parse("reviewer"), Some(Role::Reviewer));
    }

    #[test]
    fn role_from_str_rejects_invalid() {
        assert_eq!(Role::parse("invalid"), None);
        assert_eq!(Role::parse(""), None);
    }
}
