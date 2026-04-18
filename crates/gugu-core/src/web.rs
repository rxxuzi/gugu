use std::collections::{HashMap, HashSet};

use crate::{AgentType, AtomId, PortId, Bond};

/// A port belongs to an atom at a given index.
/// Index 0 = fuse, >= 1 = arm.
#[derive(Debug, Clone, Copy)]
pub struct Port {
    pub id: PortId,
    pub atom: AtomId,
    pub index: u32,
}

impl Port {
    pub fn is_fuse(&self) -> bool {
        self.index == 0
    }
}

/// An interaction net atom.
#[derive(Debug, Clone)]
pub struct Atom {
    pub id: AtomId,
    pub agent: AgentType,
    pub ports: Vec<PortId>,
}

impl Atom {
    pub fn fuse(&self) -> PortId {
        self.ports[0]
    }

    pub fn arms(&self) -> &[PortId] {
        &self.ports[1..]
    }
}

/// The interaction net web.
pub struct Web {
    atoms: HashMap<AtomId, Atom>,
    ports: HashMap<PortId, Port>,
    bonds: HashSet<Bond>,
    port_to_bond: HashMap<PortId, Bond>,
    next_atom: u32,
    next_port: u32,
}

impl Web {
    pub fn new() -> Self {
        Self {
            atoms: HashMap::new(),
            ports: HashMap::new(),
            bonds: HashSet::new(),
            port_to_bond: HashMap::new(),
            next_atom: 0,
            next_port: 0,
        }
    }

    fn alloc_atom_id(&mut self) -> AtomId {
        let id = AtomId::new(self.next_atom);
        self.next_atom += 1;
        id
    }

    fn alloc_port_id(&mut self) -> PortId {
        let id = PortId::new(self.next_port);
        self.next_port += 1;
        id
    }

    /// Create an atom with the given agent type and arity (total port count).
    pub fn add_atom(&mut self, agent: AgentType, arity: u32) -> AtomId {
        let nid = self.alloc_atom_id();
        let mut port_ids = Vec::with_capacity(arity as usize);
        for idx in 0..arity {
            let pid = self.alloc_port_id();
            self.ports.insert(
                pid,
                Port {
                    id: pid,
                    atom: nid,
                    index: idx,
                },
            );
            port_ids.push(pid);
        }
        self.atoms.insert(
            nid,
            Atom {
                id: nid,
                agent,
                ports: port_ids,
            },
        );
        nid
    }

    /// Connect two ports with a bond. Disconnects any existing bond on either port.
    pub fn link(&mut self, a: PortId, b: PortId) {
        self.unlink_port(a);
        self.unlink_port(b);
        let w = Bond::new(a, b);
        self.bonds.insert(w);
        self.port_to_bond.insert(a, w);
        self.port_to_bond.insert(b, w);
    }

    /// Remove the bond attached to a port, if any.
    pub fn unlink_port(&mut self, p: PortId) {
        if let Some(w) = self.port_to_bond.remove(&p) {
            let other = w.other(p);
            self.port_to_bond.remove(&other);
            self.bonds.remove(&w);
        }
    }

    /// Remove an atom and all its bonds from the web.
    pub fn remove_atom(&mut self, nid: AtomId) {
        if let Some(atom) = self.atoms.remove(&nid) {
            for &pid in &atom.ports {
                self.unlink_port(pid);
                self.ports.remove(&pid);
            }
        }
    }

    /// Find all active pairs: two atoms whose fuse are wired together.
    pub fn sparks(&self) -> Vec<(AtomId, AtomId)> {
        let mut pairs = Vec::new();
        for &w in &self.bonds {
            let (pa, pb) = w.ports();
            let (Some(a), Some(b)) = (self.ports.get(&pa), self.ports.get(&pb)) else {
                continue;
            };
            if a.is_fuse() && b.is_fuse() {
                let (na, nb) = (a.atom, b.atom);
                if na.raw() < nb.raw() {
                    pairs.push((na, nb));
                } else {
                    pairs.push((nb, na));
                }
            }
        }
        pairs
    }

    pub fn atom(&self, id: AtomId) -> Option<&Atom> {
        self.atoms.get(&id)
    }

    pub fn port(&self, id: PortId) -> Option<&Port> {
        self.ports.get(&id)
    }

    pub fn bond_of(&self, p: PortId) -> Option<Bond> {
        self.port_to_bond.get(&p).copied()
    }

    /// The port connected to `p` via a bond, if any.
    pub fn peer(&self, p: PortId) -> Option<PortId> {
        self.port_to_bond.get(&p).map(|w| w.other(p))
    }

    pub fn atom_count(&self) -> usize {
        self.atoms.len()
    }

    pub fn bond_count(&self) -> usize {
        self.bonds.len()
    }

    /// Iterate over all atom IDs in the web.
    pub fn atom_ids(&self) -> impl Iterator<Item = AtomId> + '_ {
        self.atoms.keys().copied()
    }
}

impl Default for Web {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_link() {
        let mut g = Web::new();
        let a = g.add_atom(AgentType::ERA, 1);
        let b = g.add_atom(AgentType::ERA, 1);
        let pa = g.atom(a).unwrap().fuse();
        let pb = g.atom(b).unwrap().fuse();
        g.link(pa, pb);
        assert_eq!(g.bond_count(), 1);
        assert_eq!(g.peer(pa), Some(pb));
        assert_eq!(g.peer(pb), Some(pa));
    }

    #[test]
    fn active_pairs_detected() {
        let mut g = Web::new();
        let a = g.add_atom(AgentType::ERA, 1);
        let b = g.add_atom(AgentType::ERA, 1);
        let pa = g.atom(a).unwrap().fuse();
        let pb = g.atom(b).unwrap().fuse();
        g.link(pa, pb);
        let pairs = g.sparks();
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn aux_wire_not_active() {
        let mut g = Web::new();
        let a = g.add_atom(AgentType::DUP, 3); // fuse + 2 arm
        let b = g.add_atom(AgentType::ERA, 1);
        let arm = g.atom(a).unwrap().arms()[0];
        let pb = g.atom(b).unwrap().fuse();
        g.link(arm, pb);
        assert!(g.sparks().is_empty());
    }

    #[test]
    fn remove_node_cleans_wires() {
        let mut g = Web::new();
        let a = g.add_atom(AgentType::ERA, 1);
        let b = g.add_atom(AgentType::ERA, 1);
        let pa = g.atom(a).unwrap().fuse();
        let pb = g.atom(b).unwrap().fuse();
        g.link(pa, pb);
        g.remove_atom(a);
        assert_eq!(g.atom_count(), 1);
        assert_eq!(g.bond_count(), 0);
        assert_eq!(g.peer(pb), None);
    }

    #[test]
    fn relink_disconnects_old() {
        let mut g = Web::new();
        let a = g.add_atom(AgentType::ERA, 1);
        let b = g.add_atom(AgentType::ERA, 1);
        let c = g.add_atom(AgentType::ERA, 1);
        let pa = g.atom(a).unwrap().fuse();
        let pb = g.atom(b).unwrap().fuse();
        let pc = g.atom(c).unwrap().fuse();
        g.link(pa, pb);
        g.link(pa, pc); // should disconnect pa--pb
        assert_eq!(g.peer(pa), Some(pc));
        assert_eq!(g.peer(pb), None);
        assert_eq!(g.bond_count(), 1);
    }
}
