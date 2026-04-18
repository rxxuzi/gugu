/// Identifies an agent type by its interned index.
/// Built-in agents (ERA, DUP, etc.) occupy the lowest IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentType(u32);

impl AgentType {
    pub const ERA: Self = Self(0);
    pub const DUP: Self = Self(1);
    pub const TRUE: Self = Self(2);
    pub const FALSE: Self = Self(3);
    pub const ERR: Self = Self(4);
    pub const NIL: Self = Self(5);
    pub const EQ: Self = Self(6);
    pub const NEQ: Self = Self(7);
    pub const LT: Self = Self(8);
    pub const GT: Self = Self(9);
    pub const BIT0: Self = Self(10);
    pub const BIT1: Self = Self(11);
    pub const ZERO: Self = Self(12);
    pub const NEG: Self = Self(13);
    pub const CHAR: Self = Self(14);
    pub const CONS: Self = Self(15);

    pub const FIRST_USER: u32 = 64;

    pub fn user(id: u32) -> Self {
        Self(Self::FIRST_USER + id)
    }

    pub fn raw(self) -> u32 {
        self.0
    }

    pub fn is_builtin(self) -> bool {
        self.0 < Self::FIRST_USER
    }
}

/// Definition of an agent: its type, name, and port names.
/// Port index 0 is always the fuse (implicit, unnamed).
/// `ports` lists only arms names.
#[derive(Debug, Clone)]
pub struct AgentDef {
    pub ty: AgentType,
    pub name: String,
    pub ports: Vec<String>,
}

impl AgentDef {
    pub fn new(ty: AgentType, name: impl Into<String>, ports: Vec<String>) -> Self {
        Self {
            ty,
            name: name.into(),
            ports,
        }
    }

    /// Total port count: 1 fuse + arms.
    pub fn arity(&self) -> usize {
        1 + self.ports.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_agents() {
        assert!(AgentType::ERA.is_builtin());
        assert!(AgentType::DUP.is_builtin());
        assert!(!AgentType::user(0).is_builtin());
    }

    #[test]
    fn agent_def_arity() {
        let era = AgentDef::new(AgentType::ERA, "ERA", vec![]);
        assert_eq!(era.arity(), 1);

        let dup = AgentDef::new(AgentType::DUP, "DUP", vec!["c1".into(), "c2".into()]);
        assert_eq!(dup.arity(), 3);
    }
}
