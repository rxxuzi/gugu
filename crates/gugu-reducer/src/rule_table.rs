use gugu_core::{AgentType, Web, PortId};

/// Saved arm-port peers from the two atoms involved in a bloom.
pub struct BloomCtx {
    /// Peers of the LHS atom's arm ports (index 0 = first arm).
    /// `None` means the arm port was unconnected.
    pub lhs_aux: Vec<Option<PortId>>,
    /// Peers of the RHS atom's arm ports. Empty for wildcard matches — those
    /// peers get auto-ERA'd by the reducer before the rule body runs.
    pub rhs_aux: Vec<Option<PortId>>,
}

// Box<dyn> required: rewrite closures are heterogeneous and stored in a Vec.
// impl Trait can't be used in struct fields or type aliases with storage.
pub type RewriteFn = Box<dyn Fn(&mut Web, &BloomCtx)>;

struct UserEntry {
    lhs: AgentType,
    #[allow(dead_code)]
    rhs: AgentType,
    rewrite: RewriteFn,
}

struct WildcardEntry {
    lhs: AgentType,
    rewrite: RewriteFn,
}

/// Result of looking up a rule for an active pair.
pub struct MatchedRule<'a> {
    pub rewrite: &'a RewriteFn,
    /// True when the caller's `a` argument matched the stored `rhs`
    /// (or, for wildcards, matched the stored non-wildcard side).
    pub swapped: bool,
    /// True when the match came from the wildcard table. The reducer auto-
    /// ERAs the RHS arm peers before invoking the body.
    pub is_wildcard: bool,
}

/// Maps agent-type pairs to rewrite rules.
/// Built-in rules (ERA, DUP) are handled directly by the Reducer;
/// this table holds user-defined rules (specific + wildcard).
pub struct RuleTable {
    rules: Vec<UserEntry>,
    wildcards: Vec<WildcardEntry>,
}

impl RuleTable {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            wildcards: Vec::new(),
        }
    }

    /// Whether a specific rule is registered for the unordered pair `(a, b)`.
    pub fn has_pair(&self, a: AgentType, b: AgentType) -> bool {
        self.rules
            .iter()
            .any(|e| (e.lhs == a && e.rhs == b) || (e.lhs == b && e.rhs == a))
    }

    /// Whether a wildcard `@X >< _` is already registered for `lhs`.
    pub fn has_wildcard(&self, lhs: AgentType) -> bool {
        self.wildcards.iter().any(|e| e.lhs == lhs)
    }

    /// Register a specific rule for the pair `(lhs, rhs)`.
    pub fn add(
        &mut self,
        lhs: AgentType,
        rhs: AgentType,
        rewrite: impl Fn(&mut Web, &BloomCtx) + 'static,
    ) {
        self.rules.push(UserEntry {
            lhs,
            rhs,
            rewrite: Box::new(rewrite),
        });
    }

    /// Register a wildcard rule `@lhs >< _`.
    pub fn add_wildcard(
        &mut self,
        lhs: AgentType,
        rewrite: impl Fn(&mut Web, &BloomCtx) + 'static,
    ) {
        self.wildcards.push(WildcardEntry {
            lhs,
            rewrite: Box::new(rewrite),
        });
    }

    /// Look up a rule for the agent pair. Specific matches win over
    /// wildcards. Within each table the first registered entry wins
    /// (stdlib registers before user code), so pair ambiguity is
    /// resolved deterministically.
    pub fn lookup(&self, a: AgentType, b: AgentType) -> Option<MatchedRule<'_>> {
        for entry in &self.rules {
            if entry.lhs == a && entry.rhs == b {
                return Some(MatchedRule {
                    rewrite: &entry.rewrite,
                    swapped: false,
                    is_wildcard: false,
                });
            }
            if entry.lhs == b && entry.rhs == a {
                return Some(MatchedRule {
                    rewrite: &entry.rewrite,
                    swapped: true,
                    is_wildcard: false,
                });
            }
        }
        for entry in &self.wildcards {
            if entry.lhs == a {
                return Some(MatchedRule {
                    rewrite: &entry.rewrite,
                    swapped: false,
                    is_wildcard: true,
                });
            }
            if entry.lhs == b {
                return Some(MatchedRule {
                    rewrite: &entry.rewrite,
                    swapped: true,
                    is_wildcard: true,
                });
            }
        }
        None
    }
}

impl Default for RuleTable {
    fn default() -> Self {
        Self::new()
    }
}
