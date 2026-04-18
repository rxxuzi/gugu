use gugu_core::{AgentType, Web, AtomId, PortId};

use crate::rule_table::{BloomCtx, RuleTable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepResult {
    /// A bloom occurred.
    Bloomed,
    /// No spark has a matching rule — computation finished.
    Slag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunResult {
    /// Reached slag after N blooms.
    Slag(u64),
    /// Hit the step limit after N blooms.
    MaxSteps(u64),
}

pub struct Reducer {
    pub web: Web,
    rules: RuleTable,
    blooms: u64,
}

impl Reducer {
    pub fn new(web: Web, rules: RuleTable) -> Self {
        Self {
            web,
            rules,
            blooms: 0,
        }
    }

    pub fn blooms(&self) -> u64 {
        self.blooms
    }

    /// Perform one bloom. Returns `Slag` if no rule can fire.
    pub fn step(&mut self) -> StepResult {
        let pairs = self.web.sparks();
        for (a, b) in pairs {
            let agent_a = self.web.atom(a).unwrap().agent;
            let agent_b = self.web.atom(b).unwrap().agent;
            if self.can_bloom(agent_a, agent_b) {
                self.bloom(a, b, agent_a, agent_b);
                self.blooms += 1;
                return StepResult::Bloomed;
            }
        }
        StepResult::Slag
    }

    /// Run until slag or `max_steps` reached.
    pub fn run(&mut self, max_steps: u64) -> RunResult {
        for _ in 0..max_steps {
            if self.step() == StepResult::Slag {
                return RunResult::Slag(self.blooms);
            }
        }
        RunResult::MaxSteps(self.blooms)
    }

    fn can_bloom(&self, a: AgentType, b: AgentType) -> bool {
        // ERA fires with anything
        if a == AgentType::ERA || b == AgentType::ERA {
            return true;
        }
        // DUP fires with anything
        if a == AgentType::DUP || b == AgentType::DUP {
            return true;
        }
        // User rule
        self.rules.lookup(a, b).is_some()
    }

    fn bloom(&mut self, a: AtomId, b: AtomId, agent_a: AgentType, agent_b: AgentType) {
        // ERA >< ERA
        if agent_a == AgentType::ERA && agent_b == AgentType::ERA {
            self.bloom_era_era(a, b);
            return;
        }
        // ERA >< X
        if agent_a == AgentType::ERA {
            self.bloom_era(a, b);
            return;
        }
        if agent_b == AgentType::ERA {
            self.bloom_era(b, a);
            return;
        }
        // DUP >< DUP
        if agent_a == AgentType::DUP && agent_b == AgentType::DUP {
            self.bloom_dup_dup(a, b);
            return;
        }
        // DUP >< X
        if agent_a == AgentType::DUP {
            self.bloom_dup(a, b);
            return;
        }
        if agent_b == AgentType::DUP {
            self.bloom_dup(b, a);
            return;
        }
        // User rule
        self.bloom_user(a, b, agent_a, agent_b);
    }

    /// ERA >< ERA: both vanish. Neither has arm ports.
    fn bloom_era_era(&mut self, era1: AtomId, era2: AtomId) {
        self.web.remove_atom(era1);
        self.web.remove_atom(era2);
    }

    /// ERA >< X: erase X. For each of X's arm-port peers, spawn a
    /// fresh ERA bonded to that peer (chain deletion).
    fn bloom_era(&mut self, era: AtomId, target: AtomId) {
        let target_aux_peers = self.save_aux_peers(target);
        self.web.remove_atom(era);
        self.web.remove_atom(target);

        for peer in target_aux_peers.into_iter().flatten() {
            let new_era = self.web.add_atom(AgentType::ERA, 1);
            let fuse = self.web.atom(new_era).unwrap().fuse();
            self.web.link(fuse, peer);
        }
    }

    /// DUP >< DUP (same label, annihilation):
    /// bond c1_a—c1_b and c2_a—c2_b directly.
    fn bloom_dup_dup(&mut self, dup_a: AtomId, dup_b: AtomId) {
        let a_aux = self.save_aux_peers(dup_a);
        let b_aux = self.save_aux_peers(dup_b);
        self.web.remove_atom(dup_a);
        self.web.remove_atom(dup_b);

        // c1_a -- c1_b
        if let (Some(pa), Some(pb)) = (a_aux[0], b_aux[0]) {
            self.web.link(pa, pb);
        }
        // c2_a -- c2_b
        if let (Some(pa), Some(pb)) = (a_aux[1], b_aux[1]) {
            self.web.link(pa, pb);
        }
    }

    /// DUP >< X (X is not ERA/DUP): clone X.
    ///    /// DUP has arm [c1, c2] with peers P1, P2.
    /// X has arm [x0, x1, …] with peers Q0, Q1, ….
    ///    /// After bloom:
    ///   Create X1, X2 (copies of X).
    ///   X1.fuse — P1,  X2.fuse — P2.
    ///   For each i:
    ///     Create DUP_i.
    ///     DUP_i.fuse — Qi.
    ///     DUP_i.c1 — X1.arm[i].
    ///     DUP_i.c2 — X2.arm[i].
    fn bloom_dup(&mut self, dup: AtomId, target: AtomId) {
        let dup_aux = self.save_aux_peers(dup);
        let target_agent = self.web.atom(target).unwrap().agent;
        let target_arity = self.web.atom(target).unwrap().ports.len() as u32;
        let target_aux = self.save_aux_peers(target);

        self.web.remove_atom(dup);
        self.web.remove_atom(target);

        // Create copies
        let x1 = self.web.add_atom(target_agent, target_arity);
        let x2 = self.web.add_atom(target_agent, target_arity);

        // X1.fuse — P1 (DUP.c1 peer)
        if let Some(p1) = dup_aux[0] {
            let fuse1 = self.web.atom(x1).unwrap().fuse();
            self.web.link(fuse1, p1);
        }
        // X2.fuse — P2 (DUP.c2 peer)
        if let Some(p2) = dup_aux[1] {
            let fuse2 = self.web.atom(x2).unwrap().fuse();
            self.web.link(fuse2, p2);
        }

        // For each arm port of X: create DUP_i to fan out
        for (i, qi) in target_aux.into_iter().enumerate() {
            let di = self.web.add_atom(AgentType::DUP, 3);
            let di_fuse = self.web.atom(di).unwrap().fuse();
            let di_c1 = self.web.atom(di).unwrap().arms()[0];
            let di_c2 = self.web.atom(di).unwrap().arms()[1];

            if let Some(qi) = qi {
                self.web.link(di_fuse, qi);
            }
            let x1_aux_i = self.web.atom(x1).unwrap().arms()[i];
            let x2_aux_i = self.web.atom(x2).unwrap().arms()[i];
            self.web.link(di_c1, x1_aux_i);
            self.web.link(di_c2, x2_aux_i);
        }
    }

    fn bloom_user(&mut self, a: AtomId, b: AtomId, agent_a: AgentType, agent_b: AgentType) {
        let (swapped, is_wildcard) = {
            let m = self
                .rules
                .lookup(agent_a, agent_b)
                .expect("bloom_user called without matching rule");
            (m.swapped, m.is_wildcard)
        };

        let (lhs_nid, rhs_nid) = if swapped { (b, a) } else { (a, b) };

        let lhs_aux = self.save_aux_peers(lhs_nid);
        let rhs_aux = self.save_aux_peers(rhs_nid);

        self.web.remove_atom(lhs_nid);
        self.web.remove_atom(rhs_nid);

        let ctx = if is_wildcard {
            // Wildcard rule bodies can't name the RHS agent's arm ports;
            // auto-ERA each peer so the structure it held chain-deletes.
            for peer in rhs_aux.iter().copied().flatten() {
                let era = self.web.add_atom(AgentType::ERA, 1);
                let ep = self.web.atom(era).unwrap().fuse();
                self.web.link(ep, peer);
            }
            BloomCtx {
                lhs_aux,
                rhs_aux: Vec::new(),
            }
        } else {
            BloomCtx { lhs_aux, rhs_aux }
        };

        // Borrow fields separately: rules (shared) + web (exclusive).
        let m2 = self.rules.lookup(agent_a, agent_b).unwrap();
        (m2.rewrite)(&mut self.web, &ctx);
    }

    /// Save the peers of an atom's arms (before removal).
    fn save_aux_peers(&self, nid: AtomId) -> Vec<Option<PortId>> {
        let atom = self.web.atom(nid).unwrap();
        atom.arms()
            .iter()
            .map(|&pid| self.web.peer(pid))
            .collect()
    }
}
