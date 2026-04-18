use gugu_core::{AgentType, Web, AtomId};

use crate::reducer::{Reducer, RunResult, StepResult};
use crate::rule_table::RuleTable;

/// Count how many atoms of a given agent type exist.
fn count_agents(g: &Web, ty: AgentType) -> usize {
    g.atom_ids()
        .filter(|&nid| g.atom(nid).unwrap().agent == ty)
        .count()
}

/// Get all atom IDs with a given agent type.
fn find_agents(g: &Web, ty: AgentType) -> Vec<AtomId> {
    g.atom_ids()
        .filter(|&nid| g.atom(nid).unwrap().agent == ty)
        .collect()
}

/// Verify that port P is bonded to port Q.
fn assert_bonded(g: &Web, desc: &str, p: gugu_core::PortId, q: gugu_core::PortId) {
    assert_eq!(
        g.peer(p),
        Some(q),
        "{desc}: expected {p} bonded to {q}, got {:?}",
        g.peer(p)
    );
    assert_eq!(
        g.peer(q),
        Some(p),
        "{desc}: expected {q} bonded to {p}, got {:?}",
        g.peer(q)
    );
}

/// Verify that a port has no bond.
fn assert_free(g: &Web, desc: &str, p: gugu_core::PortId) {
    assert_eq!(
        g.peer(p),
        None,
        "{desc}: expected {p} to be free, but bonded to {:?}",
        g.peer(p)
    );
}

// ERA ANNIHILATION

mod era {
    use super::*;

    /// ERA >< ERA → both vanish, web is empty.
    #[test]
    fn era_era_both_vanish() {
        let mut g = Web::new();
        let a = g.add_atom(AgentType::ERA, 1);
        let b = g.add_atom(AgentType::ERA, 1);
        g.link(
            g.atom(a).unwrap().fuse(),
            g.atom(b).unwrap().fuse(),
        );

        let mut r = Reducer::new(g, RuleTable::new());
        assert_eq!(r.step(), StepResult::Bloomed);
        assert_eq!(r.web.atom_count(), 0);
        assert_eq!(r.web.bond_count(), 0);
        assert_eq!(r.step(), StepResult::Slag);
        assert_eq!(r.blooms(), 1);
    }

    /// ERA >< X (X has no arm ports, arity=1) → both vanish.
    #[test]
    fn era_vs_no_aux() {
        let zero = AgentType::user(0);
        let mut g = Web::new();
        let era = g.add_atom(AgentType::ERA, 1);
        let z = g.add_atom(zero, 1);
        g.link(
            g.atom(era).unwrap().fuse(),
            g.atom(z).unwrap().fuse(),
        );

        let mut r = Reducer::new(g, RuleTable::new());
        assert_eq!(r.step(), StepResult::Bloomed);
        assert_eq!(r.web.atom_count(), 0, "no atoms left");
        assert_eq!(r.web.bond_count(), 0, "no bonds left");
    }

    /// ERA >< X (X has 1 arm port bonded to Y) →
    /// X vanishes, a new ERA spawns bonded to Y.
    #[test]
    fn era_chain_single_aux() {
        let bit1 = AgentType::user(1);
        let zero = AgentType::user(0);

        let mut g = Web::new();
        let era = g.add_atom(AgentType::ERA, 1);
        let b = g.add_atom(bit1, 2); // fuse + 1 arm (/hi)
        let z = g.add_atom(zero, 1);

        // ERA.fuse -- BIT1.fuse  (spark)
        g.link(
            g.atom(era).unwrap().fuse(),
            g.atom(b).unwrap().fuse(),
        );
        // BIT1.arm[0] -- ZERO.fuse
        g.link(
            g.atom(b).unwrap().arms()[0],
            g.atom(z).unwrap().fuse(),
        );

        let mut r = Reducer::new(g, RuleTable::new());
        assert_eq!(r.step(), StepResult::Bloomed);

        // ERA and BIT1 gone. A new ERA spawned, bonded to ZERO.
        assert_eq!(count_agents(&r.web, bit1), 0, "BIT1 erased");
        assert_eq!(count_agents(&r.web, AgentType::ERA), 1, "one new ERA");
        assert_eq!(count_agents(&r.web, zero), 1, "ZERO still alive");

        // New ERA's fuse is bonded to ZERO's fuse.
        let new_eras = find_agents(&r.web, AgentType::ERA);
        let new_era_fuse = r.web.atom(new_eras[0]).unwrap().fuse();
        let z_nodes = find_agents(&r.web, zero);
        let z_fuse = r.web.atom(z_nodes[0]).unwrap().fuse();
        assert_bonded(&r.web, "new ERA -- ZERO", new_era_fuse, z_fuse);
    }

    /// ERA chain deletion: ERA erases an atom with 2 arm ports,
    /// spawning 2 new ERAs that each bond to a downstream peer.
    #[test]
    fn era_chain_two_aux() {
        let add = AgentType::user(2);
        let x = AgentType::user(10);
        let y = AgentType::user(11);

        let mut g = Web::new();
        let era = g.add_atom(AgentType::ERA, 1);
        let add_n = g.add_atom(add, 3); // fuse + /lft + /rgt
        let xn = g.add_atom(x, 1);
        let yn = g.add_atom(y, 1);

        g.link(
            g.atom(era).unwrap().fuse(),
            g.atom(add_n).unwrap().fuse(),
        );
        g.link(
            g.atom(add_n).unwrap().arms()[0],
            g.atom(xn).unwrap().fuse(),
        );
        g.link(
            g.atom(add_n).unwrap().arms()[1],
            g.atom(yn).unwrap().fuse(),
        );

        let mut r = Reducer::new(g, RuleTable::new());
        assert_eq!(r.step(), StepResult::Bloomed);

        assert_eq!(count_agents(&r.web, add), 0, "ADD erased");
        assert_eq!(count_agents(&r.web, AgentType::ERA), 2, "two new ERAs");
        assert_eq!(count_agents(&r.web, x), 1, "X still alive");
        assert_eq!(count_agents(&r.web, y), 1, "Y still alive");
        assert_eq!(r.web.bond_count(), 2, "two bonds: new_era1-X, new_era2-Y");
    }

    /// Full ERA chain: ERA erases a 3-deep chain. Multiple bloom steps.
    #[test]
    fn era_chain_multi_step() {
        let bit1 = AgentType::user(1);
        let zero = AgentType::user(0);

        let mut g = Web::new();
        let era = g.add_atom(AgentType::ERA, 1);
        let b1 = g.add_atom(bit1, 2);
        let b2 = g.add_atom(bit1, 2);
        let z = g.add_atom(zero, 1);

        // Chain: ERA.fuse -- b1.fuse,  b1.arm -- b2.fuse,  b2.arm -- z.fuse
        g.link(
            g.atom(era).unwrap().fuse(),
            g.atom(b1).unwrap().fuse(),
        );
        g.link(
            g.atom(b1).unwrap().arms()[0],
            g.atom(b2).unwrap().fuse(),
        );
        g.link(
            g.atom(b2).unwrap().arms()[0],
            g.atom(z).unwrap().fuse(),
        );

        let mut r = Reducer::new(g, RuleTable::new());
        let result = r.run(100);
        assert_eq!(result, RunResult::Slag(3), "3 blooms to erase the chain");
        assert_eq!(r.web.atom_count(), 0, "web fully erased");
        assert_eq!(r.web.bond_count(), 0, "no bonds left");
    }

    /// ERA >< X where arm port of X is unconnected → no new ERA for
    /// that port (nothing to chain-delete).
    #[test]
    fn era_vs_unconnected_aux() {
        let bit1 = AgentType::user(1);

        let mut g = Web::new();
        let era = g.add_atom(AgentType::ERA, 1);
        let b = g.add_atom(bit1, 2); // fuse + 1 arm, arm NOT bonded

        g.link(
            g.atom(era).unwrap().fuse(),
            g.atom(b).unwrap().fuse(),
        );

        let mut r = Reducer::new(g, RuleTable::new());
        assert_eq!(r.step(), StepResult::Bloomed);
        assert_eq!(
            r.web.atom_count(),
            0,
            "both vanish, no ERA spawned for unconnected arm"
        );
    }
}

// DUP CLONING

mod dup {
    use super::*;

    /// DUP >< DUP (annihilation): c1_a—c1_b, c2_a—c2_b.
    #[test]
    fn dup_dup_annihilation() {
        let x1 = AgentType::user(10);
        let x2 = AgentType::user(11);
        let y1 = AgentType::user(12);
        let y2 = AgentType::user(13);

        let mut g = Web::new();
        let da = g.add_atom(AgentType::DUP, 3);
        let db = g.add_atom(AgentType::DUP, 3);
        let xn1 = g.add_atom(x1, 1);
        let xn2 = g.add_atom(x2, 1);
        let yn1 = g.add_atom(y1, 1);
        let yn2 = g.add_atom(y2, 1);

        // da.fuse -- db.fuse
        g.link(
            g.atom(da).unwrap().fuse(),
            g.atom(db).unwrap().fuse(),
        );
        // da.c1 -- x1, da.c2 -- x2
        g.link(
            g.atom(da).unwrap().arms()[0],
            g.atom(xn1).unwrap().fuse(),
        );
        g.link(
            g.atom(da).unwrap().arms()[1],
            g.atom(xn2).unwrap().fuse(),
        );
        // db.c1 -- y1, db.c2 -- y2
        g.link(
            g.atom(db).unwrap().arms()[0],
            g.atom(yn1).unwrap().fuse(),
        );
        g.link(
            g.atom(db).unwrap().arms()[1],
            g.atom(yn2).unwrap().fuse(),
        );

        let mut r = Reducer::new(g, RuleTable::new());
        assert_eq!(r.step(), StepResult::Bloomed);

        assert_eq!(count_agents(&r.web, AgentType::DUP), 0, "both DUPs gone");
        assert_eq!(r.web.atom_count(), 4, "four leaf atoms remain");

        // c1_a(=x1) -- c1_b(=y1)
        let xn1_fuse = r.web.atom(xn1).unwrap().fuse();
        let yn1_fuse = r.web.atom(yn1).unwrap().fuse();
        assert_bonded(&r.web, "x1--y1", xn1_fuse, yn1_fuse);

        // c2_a(=x2) -- c2_b(=y2)
        let xn2_fuse = r.web.atom(xn2).unwrap().fuse();
        let yn2_fuse = r.web.atom(yn2).unwrap().fuse();
        assert_bonded(&r.web, "x2--y2", xn2_fuse, yn2_fuse);
    }

    /// DUP >< X (no arm on X): creates two copies of X, each bonded
    /// to the DUP's c1/c2 peers.
    #[test]
    fn dup_clone_no_aux() {
        let zero = AgentType::user(0);
        let sink1 = AgentType::user(10);
        let sink2 = AgentType::user(11);

        let mut g = Web::new();
        let d = g.add_atom(AgentType::DUP, 3);
        let z = g.add_atom(zero, 1);
        let s1 = g.add_atom(sink1, 1);
        let s2 = g.add_atom(sink2, 1);

        // DUP.fuse -- ZERO.fuse
        g.link(
            g.atom(d).unwrap().fuse(),
            g.atom(z).unwrap().fuse(),
        );
        // DUP.c1 -- sink1.fuse
        g.link(
            g.atom(d).unwrap().arms()[0],
            g.atom(s1).unwrap().fuse(),
        );
        // DUP.c2 -- sink2.fuse
        g.link(
            g.atom(d).unwrap().arms()[1],
            g.atom(s2).unwrap().fuse(),
        );

        let mut r = Reducer::new(g, RuleTable::new());
        assert_eq!(r.step(), StepResult::Bloomed);

        // DUP and original ZERO gone
        assert_eq!(count_agents(&r.web, AgentType::DUP), 0);
        assert!(r.web.atom(z).is_none(), "original ZERO removed");

        // Two new ZEROs created
        assert_eq!(count_agents(&r.web, zero), 2, "two ZERO copies");

        // Each copy's fuse is bonded to a sink
        let zeros = find_agents(&r.web, zero);
        let z0_fuse = r.web.atom(zeros[0]).unwrap().fuse();
        let z1_fuse = r.web.atom(zeros[1]).unwrap().fuse();

        // One of them bonded to sink1, the other to sink2
        let s1_fuse = r.web.atom(s1).unwrap().fuse();
        let s2_fuse = r.web.atom(s2).unwrap().fuse();

        let peers: Vec<_> = [z0_fuse, z1_fuse]
            .iter()
            .map(|&f| r.web.peer(f).unwrap())
            .collect();
        assert!(
            (peers[0] == s1_fuse && peers[1] == s2_fuse)
                || (peers[0] == s2_fuse && peers[1] == s1_fuse),
            "copies bonded to sink1 and sink2"
        );
    }

    /// DUP >< X (X has 1 arm bonded to Y):
    /// - Two copies X1, X2 created.
    /// - A new DUP_0 fans out Y to X1.arm and X2.arm.
    #[test]
    fn dup_clone_with_one_aux() {
        let bit1 = AgentType::user(1);
        let y_ty = AgentType::user(20);
        let sink1 = AgentType::user(10);
        let sink2 = AgentType::user(11);

        let mut g = Web::new();
        let d = g.add_atom(AgentType::DUP, 3);
        let b = g.add_atom(bit1, 2); // fuse + /hi
        let y = g.add_atom(y_ty, 1);
        let s1 = g.add_atom(sink1, 1);
        let s2 = g.add_atom(sink2, 1);

        // DUP.fuse -- BIT1.fuse
        g.link(
            g.atom(d).unwrap().fuse(),
            g.atom(b).unwrap().fuse(),
        );
        // BIT1.arm[0] -- Y.fuse
        g.link(
            g.atom(b).unwrap().arms()[0],
            g.atom(y).unwrap().fuse(),
        );
        // DUP.c1 -- sink1, DUP.c2 -- sink2
        g.link(
            g.atom(d).unwrap().arms()[0],
            g.atom(s1).unwrap().fuse(),
        );
        g.link(
            g.atom(d).unwrap().arms()[1],
            g.atom(s2).unwrap().fuse(),
        );

        let mut r = Reducer::new(g, RuleTable::new());
        assert_eq!(r.step(), StepResult::Bloomed);

        // Original DUP and BIT1 gone
        assert!(r.web.atom(d).is_none());
        assert!(r.web.atom(b).is_none());

        // Two new BIT1 copies
        assert_eq!(count_agents(&r.web, bit1), 2, "two BIT1 copies");

        // One new DUP (for the /hi fan-out to Y)
        assert_eq!(
            count_agents(&r.web, AgentType::DUP),
            1,
            "one DUP for arm fan-out"
        );

        // The new DUP's fuse is bonded to Y
        let new_dups = find_agents(&r.web, AgentType::DUP);
        let new_dup_fuse = r.web.atom(new_dups[0]).unwrap().fuse();
        let y_fuse = r.web.atom(y).unwrap().fuse();
        assert_bonded(&r.web, "new DUP -- Y", new_dup_fuse, y_fuse);

        // The new DUP's c1/c2 are bonded to the two BIT1 copies' arm[0]
        let new_dup_c1 = r.web.atom(new_dups[0]).unwrap().arms()[0];
        let new_dup_c2 = r.web.atom(new_dups[0]).unwrap().arms()[1];
        let bit1s = find_agents(&r.web, bit1);
        let b0_aux = r.web.atom(bit1s[0]).unwrap().arms()[0];
        let b1_aux = r.web.atom(bit1s[1]).unwrap().arms()[0];

        let c1_peer = r.web.peer(new_dup_c1).unwrap();
        let c2_peer = r.web.peer(new_dup_c2).unwrap();
        assert!(
            (c1_peer == b0_aux && c2_peer == b1_aux) || (c1_peer == b1_aux && c2_peer == b0_aux),
            "DUP.c1/c2 bonded to copies' arm"
        );
    }

    /// DUP >< ERA: both DUP.c1 and DUP.c2 peers get new ERAs.
    #[test]
    fn dup_vs_era() {
        let sink1 = AgentType::user(10);
        let sink2 = AgentType::user(11);

        let mut g = Web::new();
        let d = g.add_atom(AgentType::DUP, 3);
        let e = g.add_atom(AgentType::ERA, 1);
        let s1 = g.add_atom(sink1, 1);
        let s2 = g.add_atom(sink2, 1);

        g.link(
            g.atom(d).unwrap().fuse(),
            g.atom(e).unwrap().fuse(),
        );
        g.link(
            g.atom(d).unwrap().arms()[0],
            g.atom(s1).unwrap().fuse(),
        );
        g.link(
            g.atom(d).unwrap().arms()[1],
            g.atom(s2).unwrap().fuse(),
        );

        let mut r = Reducer::new(g, RuleTable::new());
        assert_eq!(r.step(), StepResult::Bloomed);

        // ERA wins: DUP is erased. ERA spawns new ERAs for DUP's arm peers.
        // ERA >< DUP: ERA erases DUP (which has 2 arm). So 2 new ERAs.
        assert_eq!(count_agents(&r.web, AgentType::ERA), 2, "two new ERAs");
        assert_eq!(count_agents(&r.web, AgentType::DUP), 0, "DUP gone");

        // Each new ERA bonded to a sink
        let eras = find_agents(&r.web, AgentType::ERA);
        let era_peers: Vec<_> = eras
            .iter()
            .map(|&nid| {
                r.web
                    .peer(r.web.atom(nid).unwrap().fuse())
                    .unwrap()
            })
            .collect();
        let s1_fuse = r.web.atom(s1).unwrap().fuse();
        let s2_fuse = r.web.atom(s2).unwrap().fuse();
        assert!(
            era_peers.contains(&s1_fuse) && era_peers.contains(&s2_fuse),
            "new ERAs bonded to sink1 and sink2"
        );
    }
}

// USER RULES

mod user_rules {
    use super::*;

    const ADD: u32 = 2;
    const ZERO: u32 = 0;
    const BIT1: u32 = 1;

    fn add_ty() -> AgentType {
        AgentType::user(ADD)
    }
    fn zero_ty() -> AgentType {
        AgentType::user(ZERO)
    }
    fn bit1_ty() -> AgentType {
        AgentType::user(BIT1)
    }

    fn rules_add_zero() -> RuleTable {
        let mut rt = RuleTable::new();
        // rule @ADD >< @ZERO : ~/lft -- ~/rgt
        // ADD has /lft(arm[0]), /rgt(arm[1]).  ZERO has no arm.
        // Rewrite: bond ADD's /lft peer to ADD's /rgt peer.
        rt.add(add_ty(), zero_ty(), |web, ctx| {
            if let (Some(lft), Some(rgt)) = (ctx.lhs_aux[0], ctx.lhs_aux[1]) {
                web.link(lft, rgt);
            }
        });
        rt
    }

    /// ADD >< ZERO: the two arm peers of ADD get bonded together.
    #[test]
    fn add_zero_basic() {
        let mut g = Web::new();
        let a = g.add_atom(add_ty(), 3); // fuse + /lft + /rgt
        let z = g.add_atom(zero_ty(), 1);
        let x = g.add_atom(AgentType::user(10), 1);
        let y = g.add_atom(AgentType::user(11), 1);

        // ADD.fuse -- ZERO.fuse (spark)
        g.link(
            g.atom(a).unwrap().fuse(),
            g.atom(z).unwrap().fuse(),
        );
        // ADD./lft -- X.fuse
        g.link(
            g.atom(a).unwrap().arms()[0],
            g.atom(x).unwrap().fuse(),
        );
        // ADD./rgt -- Y.fuse
        g.link(
            g.atom(a).unwrap().arms()[1],
            g.atom(y).unwrap().fuse(),
        );

        let mut r = Reducer::new(g, rules_add_zero());
        assert_eq!(r.step(), StepResult::Bloomed);

        // ADD and ZERO gone
        assert!(r.web.atom(a).is_none());
        assert!(r.web.atom(z).is_none());

        // X.fuse bonded to Y.fuse
        let x_fuse = r.web.atom(x).unwrap().fuse();
        let y_fuse = r.web.atom(y).unwrap().fuse();
        assert_bonded(&r.web, "X -- Y after ADD>< ZERO", x_fuse, y_fuse);

        assert_eq!(r.web.atom_count(), 2, "only X and Y remain");
        assert_eq!(r.web.bond_count(), 1, "one bond");
    }

    /// ADD >< ZERO where one arm is unconnected: the connected one
    /// becomes free.
    #[test]
    fn add_zero_one_aux_free() {
        let mut g = Web::new();
        let a = g.add_atom(add_ty(), 3);
        let z = g.add_atom(zero_ty(), 1);
        let x = g.add_atom(AgentType::user(10), 1);

        g.link(
            g.atom(a).unwrap().fuse(),
            g.atom(z).unwrap().fuse(),
        );
        g.link(
            g.atom(a).unwrap().arms()[0],
            g.atom(x).unwrap().fuse(),
        );
        // arm[1] (/rgt) left free — no bond

        let mut r = Reducer::new(g, rules_add_zero());
        assert_eq!(r.step(), StepResult::Bloomed);

        // ADD and ZERO gone, X remains but free
        assert_eq!(r.web.atom_count(), 1);
        let x_fuse = r.web.atom(x).unwrap().fuse();
        assert_free(&r.web, "X should be free", x_fuse);
    }

    /// No matching user rule → slag immediately (fuse-fuse pair
    /// without a rule is inert data, not a spark that can bloom).
    #[test]
    fn no_rule_is_slag() {
        let mut g = Web::new();
        let a = g.add_atom(AgentType::user(50), 1);
        let b = g.add_atom(AgentType::user(51), 1);
        g.link(
            g.atom(a).unwrap().fuse(),
            g.atom(b).unwrap().fuse(),
        );

        let mut r = Reducer::new(g, RuleTable::new());
        assert_eq!(r.step(), StepResult::Slag, "no rule → slag");
        assert_eq!(r.web.atom_count(), 2, "web untouched");
    }

    /// Multi-step bloom: adding 0 to a BIT1→ZERO number.
    ///    /// Initial web:
    ///   OUT -- ADD.fuse -- ZERO_a.fuse   (spark)
    ///   ADD./lft -- BIT1.fuse -- ZERO_b.fuse  (data, no rule)
    ///   ADD./rgt -- ZERO_c.fuse
    ///    /// Hm, ADD.fuse can only bond to one thing. Let me redo:
    ///   ADD.fuse -- ZERO_a.fuse  (spark: ADD >< ZERO)
    ///   ADD./lft -- BIT1.fuse    (BIT1 -- ZERO_b data chain)
    ///   BIT1.arm -- ZERO_b.fuse
    ///   ADD./rgt -- OUT.fuse     (result goes to OUT)
    ///    /// Step 1: ADD >< ZERO_a → bond /lft peer to /rgt peer
    ///   → BIT1.fuse -- OUT.fuse   (data pair, no rule → slag)
    ///   → BIT1.arm -- ZERO_b.fuse (data)
    #[test]
    fn add_zero_multi_step() {
        let out = AgentType::user(99);

        let mut g = Web::new();
        let add_n = g.add_atom(add_ty(), 3);
        let zero_a = g.add_atom(zero_ty(), 1);
        let bit1_n = g.add_atom(bit1_ty(), 2);
        let zero_b = g.add_atom(zero_ty(), 1);
        let out_n = g.add_atom(out, 1);

        // ADD.fuse -- ZERO_a.fuse (spark)
        g.link(
            g.atom(add_n).unwrap().fuse(),
            g.atom(zero_a).unwrap().fuse(),
        );
        // ADD./lft -- BIT1.fuse
        g.link(
            g.atom(add_n).unwrap().arms()[0],
            g.atom(bit1_n).unwrap().fuse(),
        );
        // BIT1./hi -- ZERO_b.fuse
        g.link(
            g.atom(bit1_n).unwrap().arms()[0],
            g.atom(zero_b).unwrap().fuse(),
        );
        // ADD./rgt -- OUT.fuse
        g.link(
            g.atom(add_n).unwrap().arms()[1],
            g.atom(out_n).unwrap().fuse(),
        );

        let mut r = Reducer::new(g, rules_add_zero());
        let result = r.run(100);

        assert_eq!(result, RunResult::Slag(1), "one bloom: ADD >< ZERO");
        assert_eq!(r.web.atom_count(), 3, "BIT1, ZERO_b, OUT remain");

        // BIT1.fuse -- OUT.fuse (ADD's /lft and /rgt peers got bonded)
        let bit1_fuse = r.web.atom(bit1_n).unwrap().fuse();
        let out_fuse = r.web.atom(out_n).unwrap().fuse();
        assert_bonded(&r.web, "BIT1 -- OUT", bit1_fuse, out_fuse);

        // BIT1.arm -- ZERO_b.fuse (untouched data)
        let bit1_aux = r.web.atom(bit1_n).unwrap().arms()[0];
        let zero_b_fuse = r.web.atom(zero_b).unwrap().fuse();
        assert_bonded(&r.web, "BIT1.hi -- ZERO_b", bit1_aux, zero_b_fuse);
    }

    /// Max-step safety: halt before infinite bloom.
    #[test]
    fn max_steps_halt() {
        // Create a self-regenerating spark: rule A >< B creates new A, B
        let a_ty = AgentType::user(40);
        let b_ty = AgentType::user(41);

        let mut rt = RuleTable::new();
        rt.add(a_ty, b_ty, move |web, _ctx| {
            let new_a = web.add_atom(a_ty, 1);
            let new_b = web.add_atom(b_ty, 1);
            web.link(
                web.atom(new_a).unwrap().fuse(),
                web.atom(new_b).unwrap().fuse(),
            );
        });

        let mut g = Web::new();
        let a = g.add_atom(a_ty, 1);
        let b = g.add_atom(b_ty, 1);
        g.link(
            g.atom(a).unwrap().fuse(),
            g.atom(b).unwrap().fuse(),
        );

        let mut r = Reducer::new(g, rt);
        let result = r.run(50);
        assert_eq!(result, RunResult::MaxSteps(50), "halted at 50");
    }

    /// Reversed agent order in spark: B >< A detected but rule is A >< B.
    /// The rule should still fire (lookup handles both orderings).
    #[test]
    fn reversed_spark_order() {
        let mut g = Web::new();
        // Intentionally create ZERO first (lower AtomId) so sparks
        // returns (ZERO, ADD) — the reverse of the rule definition.
        let z = g.add_atom(zero_ty(), 1);
        let a = g.add_atom(add_ty(), 3);
        let x = g.add_atom(AgentType::user(10), 1);
        let y = g.add_atom(AgentType::user(11), 1);

        g.link(
            g.atom(z).unwrap().fuse(),
            g.atom(a).unwrap().fuse(),
        );
        g.link(
            g.atom(a).unwrap().arms()[0],
            g.atom(x).unwrap().fuse(),
        );
        g.link(
            g.atom(a).unwrap().arms()[1],
            g.atom(y).unwrap().fuse(),
        );

        let mut r = Reducer::new(g, rules_add_zero());
        assert_eq!(r.step(), StepResult::Bloomed);

        // X and Y should be bonded
        let x_fuse = r.web.atom(x).unwrap().fuse();
        let y_fuse = r.web.atom(y).unwrap().fuse();
        assert_bonded(&r.web, "X -- Y", x_fuse, y_fuse);
    }

    /// Empty web is immediately slag with 0 blooms.
    #[test]
    fn empty_graph_is_slag() {
        let r_result = Reducer::new(Web::new(), RuleTable::new()).run(100);
        assert_eq!(r_result, RunResult::Slag(0));
    }

    /// Web with atoms but no bonds → slag.
    #[test]
    fn no_bonds_is_slag() {
        let mut g = Web::new();
        g.add_atom(AgentType::user(1), 2);
        g.add_atom(AgentType::user(2), 1);
        let r_result = Reducer::new(g, RuleTable::new()).run(100);
        assert_eq!(r_result, RunResult::Slag(0));
    }

    /// Bond on arm port only (no fuse-fuse) → slag.
    #[test]
    fn aux_only_bond_is_slag() {
        let mut g = Web::new();
        let a = g.add_atom(AgentType::user(1), 2);
        let b = g.add_atom(AgentType::user(2), 1);
        // a.arm[0] -- b.fuse  (not fuse-fuse)
        g.link(
            g.atom(a).unwrap().arms()[0],
            g.atom(b).unwrap().fuse(),
        );
        let r_result = Reducer::new(g, RuleTable::new()).run(100);
        assert_eq!(r_result, RunResult::Slag(0));
    }
}
