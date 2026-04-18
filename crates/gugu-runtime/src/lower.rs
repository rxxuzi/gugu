//! AST → Web lowering.
//!
//! Walks the parsed Program, registers agent definitions, builds the
//! rule table, and constructs the initial web (web) from the main block.

use std::collections::HashMap;

use gugu_core::{AgentDef, AgentType, Web, AtomId, PortId};
use gugu_parser::ast;
use gugu_reducer::RuleTable;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LowerError {
    pub msg: String,
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "lower error: {}", self.msg)
    }
}

impl std::error::Error for LowerError {}

fn err(msg: impl Into<String>) -> LowerError {
    LowerError { msg: msg.into() }
}

/// Maps agent names to their types and definitions.
pub struct AgentRegistry {
    by_name: HashMap<String, AgentType>,
    defs: HashMap<AgentType, AgentDef>,
    next_user: u32,
}

impl AgentRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            by_name: HashMap::new(),
            defs: HashMap::new(),
            next_user: 0,
        };
        // Built-in agents
        reg.register_builtin("ERA", AgentType::ERA, vec![]);
        reg.register_builtin("DUP", AgentType::DUP, vec!["c1".into(), "c2".into()]);
        reg.register_builtin("TRUE", AgentType::TRUE, vec![]);
        reg.register_builtin("FALSE", AgentType::FALSE, vec![]);
        reg.register_builtin("ERR", AgentType::ERR, vec!["msg".into()]);
        reg.register_builtin("NIL", AgentType::NIL, vec![]);
        reg.register_builtin("EQ", AgentType::EQ, vec![]);
        reg.register_builtin("NEQ", AgentType::NEQ, vec![]);
        reg.register_builtin("LT", AgentType::LT, vec![]);
        reg.register_builtin("GT", AgentType::GT, vec![]);
        reg.register_builtin("BIT0", AgentType::BIT0, vec!["hi".into()]);
        reg.register_builtin("BIT1", AgentType::BIT1, vec!["hi".into()]);
        reg.register_builtin("ZERO", AgentType::ZERO, vec![]);
        reg.register_builtin("NEG", AgentType::NEG, vec!["val".into()]);
        reg.register_builtin("CHAR", AgentType::CHAR, vec!["code".into()]);
        reg.register_builtin("CONS", AgentType::CONS, vec!["head".into(), "tail".into()]);
        reg
    }

    fn register_builtin(&mut self, name: &str, ty: AgentType, ports: Vec<String>) {
        self.by_name.insert(name.into(), ty);
        self.defs.insert(ty, AgentDef::new(ty, name, ports));
    }

    pub fn register(&mut self, name: &str, ports: Vec<String>) -> AgentType {
        if let Some(&ty) = self.by_name.get(name) {
            return ty;
        }
        let ty = AgentType::user(self.next_user);
        self.next_user += 1;
        self.by_name.insert(name.into(), ty);
        self.defs.insert(ty, AgentDef::new(ty, name, ports.clone()));
        ty
    }

    pub fn lookup(&self, name: &str) -> Option<AgentType> {
        self.by_name.get(name).copied()
    }

    pub fn def(&self, ty: AgentType) -> Option<&AgentDef> {
        self.defs.get(&ty)
    }

    pub fn arity(&self, ty: AgentType) -> u32 {
        self.defs.get(&ty).map(|d| d.arity() as u32).unwrap_or(1)
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LowerResult {
    pub web: Web,
    pub rules: RuleTable,
    pub registry: AgentRegistry,
    /// One `>out` port per `@GEN` block, in declaration order.
    pub output_ports: Vec<PortId>,
}

/// Standard library sources, prepended to every user program during
/// lowering. Order matters only for cosmetic reasons (error messages
/// mention the module name first seen); all modules share one namespace.
const STDLIB_SOURCES: &[(&str, &str)] = &[
    ("std/nat.gu", include_str!("std/nat.gu")),
    ("std/bool.gu", include_str!("std/bool.gu")),
    ("std/list.gu", include_str!("std/list.gu")),
];

pub fn lower(program: &ast::Program) -> Result<LowerResult, LowerError> {
    let mut reg = AgentRegistry::new();
    let mut web = Web::new();
    let mut rules = RuleTable::new();

    let stdlib_programs: Vec<ast::Program> = STDLIB_SOURCES
        .iter()
        .map(|(name, src)| {
            gugu_parser::parse(src)
                .map_err(|e| err(format!("stdlib {name} parse error: {e}")))
        })
        .collect::<Result<_, _>>()?;

    // Phase 1: register agent definitions (stdlib first, so user `agent`
    // decls for the same name are no-ops via `register`'s name cache)
    for prog in &stdlib_programs {
        for item in &prog.items {
            register_agents(item, &mut reg)?;
        }
    }
    for item in &program.items {
        register_agents(item, &mut reg)?;
    }

    // Phase 2: register rules. Stdlib rules register first and win `lookup`
    // ordering; user redefinitions of identical pairs are shadowed.
    for prog in &stdlib_programs {
        for item in &prog.items {
            if let ast::TopLevel::Rule(rule_def) = item {
                register_rule(rule_def, &reg, &mut rules)?;
            }
        }
    }
    for item in &program.items {
        if let ast::TopLevel::Rule(rule_def) = item {
            register_rule(rule_def, &reg, &mut rules)?;
        }
        if let ast::TopLevel::Mod(mod_def) = item {
            for mi in &mod_def.items {
                if let ast::ModItem::Rule(rule_def) = mi {
                    register_rule(rule_def, &reg, &mut rules)?;
                }
            }
        }
    }

    // Phase 3: lower each @GEN block into the web as independent components
    let mut output_ports = Vec::with_capacity(program.gens.len());
    for block in &program.gens {
        output_ports.push(lower_gen(block, &reg, &mut web)?);
    }

    Ok(LowerResult {
        web,
        rules,
        registry: reg,
        output_ports,
    })
}

fn register_agents(item: &ast::TopLevel, reg: &mut AgentRegistry) -> Result<(), LowerError> {
    match item {
        ast::TopLevel::Agent(a) => register_one_agent(a, reg)?,
        ast::TopLevel::Mod(m) => {
            for mi in &m.items {
                if let ast::ModItem::Agent(a) = mi {
                    register_one_agent(a, reg)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn register_one_agent(a: &ast::AgentDef, reg: &mut AgentRegistry) -> Result<(), LowerError> {
    let ports: Vec<String> = a
        .ports
        .iter()
        .filter_map(|p| match p {
            ast::PortDecl::Arm { name, .. } => Some(name.clone()),
            ast::PortDecl::Fuse { .. } => None,
        })
        .collect();
    // Linearity: each arm port has a unique name, so rule bodies can refer
    // to `~/name` without ambiguity.
    for i in 0..ports.len() {
        for j in (i + 1)..ports.len() {
            if ports[i] == ports[j] {
                return Err(err(format!(
                    "agent @{} declares port /{} twice",
                    a.name, ports[i]
                )));
            }
        }
    }
    reg.register(&a.name, ports);
    Ok(())
}

fn register_rule(
    rule_def: &ast::RuleDef,
    reg: &AgentRegistry,
    rules: &mut RuleTable,
) -> Result<(), LowerError> {
    let lhs_ty = reg
        .lookup(&rule_def.lhs)
        .ok_or_else(|| err(format!("unknown agent @{}", rule_def.lhs)))?;
    match &rule_def.rhs {
        ast::RuleTarget::Agents(names) => {
            for rhs_name in names {
                let rhs_ty = reg
                    .lookup(rhs_name)
                    .ok_or_else(|| err(format!("unknown agent @{}", rhs_name)))?;
                if rules.has_pair(lhs_ty, rhs_ty) {
                    return Err(err(format!(
                        "duplicate rule for @{} >< @{} — a bloom must have exactly one rewrite",
                        rule_def.lhs, rhs_name
                    )));
                }
                let rpm = build_rule_port_map(reg, &rule_def.lhs, rhs_name);
                check_self_ports(&rule_def.body, &rule_def.lhs, rhs_name, &rpm)?;
                let body = rule_def.body.clone();
                rules.add(lhs_ty, rhs_ty, move |web, ctx| {
                    let mut env = build_rule_env(ctx, &rpm);
                    for stmt in &body {
                        exec_stmt(stmt, web, &mut env, &rpm);
                    }
                    flatten_anon_bridges(&mut env, web);
                });
            }
        }
        ast::RuleTarget::Wildcard => {
            if rules.has_wildcard(lhs_ty) {
                return Err(err(format!(
                    "duplicate wildcard rule for @{} — a bloom must have exactly one rewrite",
                    rule_def.lhs
                )));
            }
            let rpm = build_wildcard_port_map(reg, &rule_def.lhs);
            check_self_ports(&rule_def.body, &rule_def.lhs, "_", &rpm)?;
            let body = rule_def.body.clone();
            rules.add_wildcard(lhs_ty, move |web, ctx| {
                let mut env = build_rule_env(ctx, &rpm);
                for stmt in &body {
                    exec_stmt(stmt, web, &mut env, &rpm);
                }
                flatten_anon_bridges(&mut env, web);
            });
        }
    }
    Ok(())
}

fn build_wildcard_port_map(reg: &AgentRegistry, lhs_name: &str) -> RulePortMap {
    let lhs_ty = reg.lookup(lhs_name).unwrap();
    let lhs_ports = reg.def(lhs_ty).map(|d| d.ports.clone()).unwrap_or_default();
    RulePortMap {
        lhs_ports,
        rhs_ports: Vec::new(),
        agents: agent_info_map(reg),
    }
}

/// Walk a rule body and reject any `~/name` that is not an arm of either
/// the LHS or RHS agent. Without this, typos silently resolve to `None`
/// and the intended bond just never forms.
fn check_self_ports(
    body: &[ast::Stmt],
    lhs_name: &str,
    rhs_name: &str,
    rpm: &RulePortMap,
) -> Result<(), LowerError> {
    fn walk_stmt(s: &ast::Stmt, visit: &mut impl FnMut(&str)) {
        match s {
            ast::Stmt::Bond { lhs, rhs, .. } => {
                walk_expr(lhs, visit);
                walk_expr(rhs, visit);
            }
            ast::Stmt::Expr(e) => walk_expr(e, visit),
        }
    }
    fn walk_expr(e: &ast::Expr, visit: &mut impl FnMut(&str)) {
        match e {
            ast::Expr::SelfPort(name, _) => visit(name),
            ast::Expr::Agent { args, .. } => {
                for a in args {
                    match a {
                        ast::AgentArg::Port(pa) => walk_expr(&pa.value, visit),
                        ast::AgentArg::Positional(e) => walk_expr(e, visit),
                    }
                }
            }
            ast::Expr::Connect { lhs, rhs, .. } => {
                walk_expr(lhs, visit);
                walk_expr(rhs, visit);
            }
            ast::Expr::Paren(inner, _)
            | ast::Expr::Force(inner, _)
            | ast::Expr::Inspect(inner, _) => walk_expr(inner, visit),
            ast::Expr::List { items, .. } => {
                for it in items {
                    walk_expr(it, visit);
                }
            }
            _ => {}
        }
    }

    let mut bad: Option<String> = None;
    for s in body {
        walk_stmt(s, &mut |name| {
            if bad.is_some() {
                return;
            }
            let found = rpm.lhs_ports.iter().any(|p| p == name)
                || rpm.rhs_ports.iter().any(|p| p == name);
            if !found {
                bad = Some(name.to_string());
            }
        });
        if bad.is_some() {
            break;
        }
    }
    if let Some(name) = bad {
        return Err(err(format!(
            "rule @{lhs_name} >< @{rhs_name}: ~/{name} is not an arm of either agent"
        )));
    }
    Ok(())
}

/// Populate bond env from the saved arm-port peers.
////// Rule bodies reference arm ports as `~/name`. Both LHS and RHS arm peers
/// are inserted under that single namespace, because there is no surface
/// syntax for `~rhs/name`. If a name appears on both sides (a collision),
/// the LHS binding wins — no stdlib rule currently has that situation.
fn build_rule_env(ctx: &gugu_reducer::BloomCtx, rpm: &RulePortMap) -> ExecEnv {
    let mut env = ExecEnv::new();
    for (i, name) in rpm.rhs_ports.iter().enumerate() {
        if let Some(peer) = ctx.rhs_aux.get(i).copied().flatten() {
            env.bonds.insert(format!("~/{name}"), peer);
        }
    }
    // Insert LHS second so colliding names resolve to the LHS peer.
    for (i, name) in rpm.lhs_ports.iter().enumerate() {
        if let Some(peer) = ctx.lhs_aux.get(i).copied().flatten() {
            env.bonds.insert(format!("~/{name}"), peer);
        }
    }
    env
}

#[derive(Clone)]
struct AgentInfo {
    ty: AgentType,
    arity: u32,
    ports: Vec<String>, // arm port names, index order (fuse not included)
}

#[derive(Clone)]
struct RulePortMap {
    lhs_ports: Vec<String>,
    rhs_ports: Vec<String>,
    agents: HashMap<String, AgentInfo>,
}

fn build_rule_port_map(reg: &AgentRegistry, lhs_name: &str, rhs_name: &str) -> RulePortMap {
    let lhs_ty = reg.lookup(lhs_name).unwrap();
    let rhs_ty = reg.lookup(rhs_name).unwrap();
    let lhs_def = reg.def(lhs_ty);
    let rhs_def = reg.def(rhs_ty);

    let lhs_ports = lhs_def.map(|d| d.ports.clone()).unwrap_or_default();
    let rhs_ports = rhs_def.map(|d| d.ports.clone()).unwrap_or_default();

    let agents = agent_info_map(reg);

    RulePortMap {
        lhs_ports,
        rhs_ports,
        agents,
    }
}

fn agent_info_map(reg: &AgentRegistry) -> HashMap<String, AgentInfo> {
    let mut agents = HashMap::new();
    for (name, &ty) in &reg.by_name {
        let ports = reg.def(ty).map(|d| d.ports.clone()).unwrap_or_default();
        agents.insert(
            name.clone(),
            AgentInfo {
                ty,
                arity: reg.arity(ty),
                ports,
            },
        );
    }
    agents
}

struct ExecEnv {
    bonds: HashMap<String, PortId>,
    /// Bridge atom for each `>>name` that has had exactly one reference so
    /// far. Moved out on the second reference.
    anon_bridges: HashMap<String, AtomId>,
    /// How many times each `>>name` has been referenced in the current
    /// scope. Linearity requires exactly 2.
    anon_refs: HashMap<String, u32>,
    /// Bridges that hit 2 refs and are ready to flatten at end of scope.
    pending_flatten: Vec<AtomId>,
    /// Errors collected during `@GEN` lowering. Rule-body exec is best-effort
    /// (Phase 4 will propagate those properly).
    errors: Vec<String>,
}

impl ExecEnv {
    fn new() -> Self {
        Self {
            bonds: HashMap::new(),
            anon_bridges: HashMap::new(),
            anon_refs: HashMap::new(),
            pending_flatten: Vec::new(),
            errors: Vec::new(),
        }
    }
}

fn exec_stmt(stmt: &ast::Stmt, web: &mut Web, env: &mut ExecEnv, rpm: &RulePortMap) {
    match stmt {
        ast::Stmt::Bond { lhs, rhs, .. } => {
            let a = eval_expr(lhs, web, env, rpm);
            let b = eval_expr(rhs, web, env, rpm);
            if let (Some(a), Some(b)) = (a, b) {
                web.link(a, b);
            }
        }
        ast::Stmt::Expr(expr) => {
            eval_expr(expr, web, env, rpm);
        }
    }
}

fn eval_expr(
    expr: &ast::Expr,
    web: &mut Web,
    env: &mut ExecEnv,
    rpm: &RulePortMap,
) -> Option<PortId> {
    match expr {
        ast::Expr::Agent { name, args, .. } => eval_agent(name, args, web, env, rpm),
        ast::Expr::Connect { lhs, rhs, .. } => eval_connect(lhs, rhs, web, env, rpm),
        ast::Expr::PortName(name, _) => eval_label(env, web, &format!("/{name}")),
        ast::Expr::SelfPort(name, _) => env.bonds.get(&format!("~/{name}")).copied(),
        ast::Expr::AnonBond(name, _) => eval_anon(env, web, name),
        ast::Expr::Output(_) => eval_label(env, web, ">out"),
        ast::Expr::BoolLit(v, _) => {
            let ty = if *v {
                AgentType::TRUE
            } else {
                AgentType::FALSE
            };
            let nid = web.add_atom(ty, 1);
            Some(web.atom(nid).unwrap().fuse())
        }
        ast::Expr::BuiltinAtom(atom, _) => eval_builtin(*atom, web),
        ast::Expr::Paren(inner, _) => eval_expr(inner, web, env, rpm),
        ast::Expr::IntLit(n, _) => Some(lower_int(*n, web)),
        ast::Expr::CharLit(c, _) => Some(lower_char(*c, web)),
        ast::Expr::StrLit(s, _) => Some(lower_str(s, web)),
        ast::Expr::List { items, .. } => Some(lower_list(items, web, env, rpm)),
        _ => None, // other literals, fragments, etc: later sub-phases
    }
}

/// `[a, b, c]` → `CONS(a, CONS(b, CONS(c, NIL)))`. Empty list is bare @NIL.
/// Items are evaluated left-to-right but linked right-to-left, so order
/// in the resulting chain matches source order.
fn lower_list(
    items: &[ast::Expr],
    web: &mut Web,
    env: &mut ExecEnv,
    rpm: &RulePortMap,
) -> PortId {
    let item_fuses: Vec<PortId> = items
        .iter()
        .map(|e| eval_expr(e, web, env, rpm).unwrap_or_else(|| placeholder_nil(web)))
        .collect();
    let nil = web.add_atom(AgentType::NIL, 1);
    let mut tail_fuse = web.atom(nil).unwrap().fuse();
    for item_fuse in item_fuses.into_iter().rev() {
        let cons = web.add_atom(AgentType::CONS, 3);
        let head_aux = web.atom(cons).unwrap().arms()[0];
        let tail_aux = web.atom(cons).unwrap().arms()[1];
        web.link(head_aux, item_fuse);
        web.link(tail_aux, tail_fuse);
        tail_fuse = web.atom(cons).unwrap().fuse();
    }
    tail_fuse
}

fn placeholder_nil(web: &mut Web) -> PortId {
    let nid = web.add_atom(AgentType::NIL, 1);
    web.atom(nid).unwrap().fuse()
}

/// `@CHAR /code` where `/code` bonds to a BIT chain encoding the Unicode
/// scalar value as u32.
fn lower_char(c: char, web: &mut Web) -> PortId {
    let nid = web.add_atom(AgentType::CHAR, 2);
    let code_aux = web.atom(nid).unwrap().arms()[0];
    let code_fuse = lower_nat(c as u64, web);
    web.link(code_aux, code_fuse);
    web.atom(nid).unwrap().fuse()
}

/// `"abc"` → `CONS(CHAR('a'), CONS(CHAR('b'), CONS(CHAR('c'), NIL)))`.
/// Empty string lowers to a bare `@NIL` atom.
fn lower_str(s: &str, web: &mut Web) -> PortId {
    let nil = web.add_atom(AgentType::NIL, 1);
    let mut tail_fuse = web.atom(nil).unwrap().fuse();
    for ch in s.chars().rev() {
        let cons = web.add_atom(AgentType::CONS, 3);
        let head_aux = web.atom(cons).unwrap().arms()[0];
        let tail_aux = web.atom(cons).unwrap().arms()[1];
        let char_fuse = lower_char(ch, web);
        web.link(head_aux, char_fuse);
        web.link(tail_aux, tail_fuse);
        tail_fuse = web.atom(cons).unwrap().fuse();
    }
    tail_fuse
}

/// Build a LSB-first BIT chain terminated in `@ZERO` and return the LSB's fuse.
////// Layout (for n = 5, binary 0b101):
//////   returned ↓
///     BIT1 ─/hi─ BIT0 ─/hi─ BIT1 ─/hi─ ZERO
///     (bit 0)    (bit 1)    (bit 2)
////// Negative values are wrapped in an `@NEG /val` atom:
//////   returned ↓
///     NEG ─/val─ <BIT chain for |n|>
////// `lower_int(0)` returns a bare `@ZERO` atom's fuse (no BIT atoms).
fn lower_int(n: i64, web: &mut Web) -> PortId {
    if n < 0 {
        let abs_fuse = lower_nat(n.unsigned_abs(), web);
        let neg = web.add_atom(AgentType::NEG, 2);
        let val = web.atom(neg).unwrap().arms()[0];
        web.link(val, abs_fuse);
        return web.atom(neg).unwrap().fuse();
    }
    lower_nat(n as u64, web)
}

fn lower_nat(v: u64, web: &mut Web) -> PortId {
    let zero = web.add_atom(AgentType::ZERO, 1);
    let mut tail = web.atom(zero).unwrap().fuse();
    if v == 0 {
        return tail;
    }
    // Iterate MSB → LSB so the last atom created is the LSB,
    // and its fuse is what gets returned (fuse side == low order).
    let n_bits = 64 - v.leading_zeros();
    for i in (0..n_bits).rev() {
        let bit = if (v >> i) & 1 == 1 {
            AgentType::BIT1
        } else {
            AgentType::BIT0
        };
        let nid = web.add_atom(bit, 2);
        let hi = web.atom(nid).unwrap().arms()[0];
        web.link(hi, tail);
        tail = web.atom(nid).unwrap().fuse();
    }
    tail
}

fn eval_agent(
    name: &str,
    args: &[ast::AgentArg],
    web: &mut Web,
    env: &mut ExecEnv,
    rpm: &RulePortMap,
) -> Option<PortId> {
    let info = rpm.agents.get(name)?.clone();
    let nid = web.add_atom(info.ty, info.arity);
    let fuse = web.atom(nid).unwrap().fuse();
    let arm: Vec<PortId> = web.atom(nid).unwrap().arms().to_vec();
    let mut assigned = vec![false; arm.len()];

    for arg in args {
        match arg {
            ast::AgentArg::Port(pa) => {
                let Some(idx) = info.ports.iter().position(|p| p == &pa.name) else {
                    env.errors
                        .push(format!("unknown port /{} on @{}", pa.name, name));
                    // still evaluate the value so we don't swallow nested side-effects,
                    // but drop the resulting port.
                    let _ = eval_expr(&pa.value, web, env, rpm);
                    continue;
                };
                if assigned[idx] {
                    env.errors
                        .push(format!("port /{} of @{} assigned twice", pa.name, name));
                    let _ = eval_expr(&pa.value, web, env, rpm);
                    continue;
                }
                if let Some(v) = eval_expr(&pa.value, web, env, rpm) {
                    web.link(arm[idx], v);
                    assigned[idx] = true;
                }
            }
            ast::AgentArg::Positional(e) => {
                let val = eval_expr(e, web, env, rpm);
                if let Some(v) = val
                    && let Some(idx) = assigned.iter().position(|a| !a)
                {
                    web.link(arm[idx], v);
                    assigned[idx] = true;
                }
            }
        }
    }
    Some(fuse)
}

/// `->` chains data: LHS's first free arm → RHS fuse.
/// Falls back to fuse-fuse when no arm is free.
fn eval_connect(
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    web: &mut Web,
    env: &mut ExecEnv,
    rpm: &RulePortMap,
) -> Option<PortId> {
    let l = eval_expr(lhs, web, env, rpm);
    let r = eval_expr(rhs, web, env, rpm);
    if let (Some(lf), Some(rf)) = (l, r) {
        let linked = web.port(lf).and_then(|p| {
            let arm = web.atom(p.atom)?.arms().to_vec();
            arm.into_iter().find(|&pid| web.peer(pid).is_none())
        });
        web.link(linked.unwrap_or(lf), rf);
    }
    l
}

fn eval_label(env: &mut ExecEnv, web: &mut Web, key: &str) -> Option<PortId> {
    if let Some(&pid) = env.bonds.get(key) {
        return Some(pid);
    }
    let n = web.add_atom(AgentType::NIL, 1);
    let pid = web.atom(n).unwrap().fuse();
    env.bonds.insert(key.to_string(), pid);
    Some(pid)
}

/// `>>name` is an anonymous through-bond shared by exactly two references.
/// The two references must end up connected directly — but we don't know
/// either caller's port when each reference is evaluated, so we bridge
/// through a 2-port NIL atom:
///   - 1st ref returns NIL.arm[0]; caller A bonds its port to arm[0].
///   - 2nd ref returns NIL.fuse; caller B bonds its port to fuse.
///   - Scope end calls `flatten_anon_bridges` to splice A↔B directly and
///     remove the NIL.
fn eval_anon(env: &mut ExecEnv, web: &mut Web, name: &str) -> Option<PortId> {
    let key = format!(">>{name}");
    let count = env.anon_refs.entry(key.clone()).or_insert(0);
    *count += 1;
    match *count {
        1 => {
            // First reference: allocate the bridge, return its arm[0].
            let nid = web.add_atom(AgentType::NIL, 2);
            let arm = web.atom(nid).unwrap().arms()[0];
            env.anon_bridges.insert(key, nid);
            Some(arm)
        }
        2 => {
            // Second reference: return bridge's fuse; mark for flatten.
            let nid = env.anon_bridges.remove(&key)?;
            env.pending_flatten.push(nid);
            web.atom(nid).map(|n| n.fuse())
        }
        _ => {
            // A third reference would break linearity (one port, one bond).
            env.errors.push(format!(
                "anonymous bond {key} referenced {} times — must be exactly 2",
                count
            ));
            None
        }
    }
}

/// Short-circuit each bridge NIL created by `eval_anon`: take its two
/// endpoints' peers, remove the bridge, and bond the peers together.
/// Any bridge that never received its second reference becomes a
/// linearity error.
fn flatten_anon_bridges(env: &mut ExecEnv, web: &mut Web) {
    for nid in env.pending_flatten.drain(..) {
        let Some(atom) = web.atom(nid) else {
            continue;
        };
        let fuse = atom.fuse();
        let aux0 = atom.arms()[0];
        let pa = web.peer(fuse);
        let pb = web.peer(aux0);
        web.remove_atom(nid);
        if let (Some(pa), Some(pb)) = (pa, pb) {
            web.link(pa, pb);
        }
    }
    for (name, nid) in env.anon_bridges.drain() {
        web.remove_atom(nid);
        env.errors
            .push(format!("anonymous bond {name} referenced once — must be exactly 2"));
    }
    env.anon_refs.clear();
}

fn eval_builtin(atom: ast::BuiltinAtom, web: &mut Web) -> Option<PortId> {
    let (ty, arity) = match atom {
        ast::BuiltinAtom::Era => (AgentType::ERA, 1),
        ast::BuiltinAtom::Dup => (AgentType::DUP, 3),
        ast::BuiltinAtom::Err => (AgentType::ERR, 2),
    };
    let nid = web.add_atom(ty, arity);
    Some(web.atom(nid).unwrap().fuse())
}

fn lower_gen(
    block: &ast::GenBlock,
    reg: &AgentRegistry,
    web: &mut Web,
) -> Result<PortId, LowerError> {
    let rpm = RulePortMap {
        lhs_ports: vec![],
        rhs_ports: vec![],
        agents: agent_info_map(reg),
    };

    // Each @GEN is an independent component → fresh bond env so >out doesn't leak between blocks.
    let mut env = ExecEnv::new();

    for stmt in &block.body {
        exec_stmt(stmt, web, &mut env, &rpm);
    }
    flatten_anon_bridges(&mut env, web);

    if let Some(msg) = env.errors.into_iter().next() {
        return Err(err(msg));
    }

    let out_port = env
        .bonds
        .get(">out")
        .copied()
        .ok_or_else(|| err("@GEN block has no >out"))?;
    Ok(out_port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_simple_program() {
        // @ADD and its rules come from stdlib now — no need to redefine.
        let src = "\
            @GEN :\n\
              @ZERO -> >out\n\
        ";
        let program = gugu_parser::parse(src).unwrap();
        let result = lower(&program).unwrap();
        assert_eq!(result.output_ports.len(), 1);
        assert!(result.web.atom_count() >= 2);
    }

    #[test]
    fn lower_multiple_gens() {
        let src = "\
            agent @ZERO\n\
            @GEN : @ZERO -> >out\n\
            @GEN : @ZERO -> >out\n\
        ";
        let program = gugu_parser::parse(src).unwrap();
        let result = lower(&program).unwrap();
        assert_eq!(result.output_ports.len(), 2);
    }

    #[test]
    fn lower_zero_gens_is_library() {
        let src = "agent @ZERO agent @ADD /lft /rgt";
        let program = gugu_parser::parse(src).unwrap();
        let result = lower(&program).unwrap();
        assert!(result.output_ports.is_empty());
    }

    #[test]
    fn lower_registers_agents() {
        let src = "agent @ZERO agent @BIT1 /hi agent @ADD /lft /rgt";
        let program = gugu_parser::parse(src).unwrap();
        let result = lower(&program).unwrap();
        assert!(result.registry.lookup("ZERO").is_some());
        assert!(result.registry.lookup("BIT1").is_some());
        assert!(result.registry.lookup("ADD").is_some());
        assert_eq!(
            result
                .registry
                .arity(result.registry.lookup("ADD").unwrap()),
            3
        );
    }

    #[test]
    fn int_zero_is_bare_zero_node() {
        let mut g = Web::new();
        let _pid = lower_int(0, &mut g);
        assert_eq!(g.atom_count(), 1);
    }

    #[test]
    fn int_five_is_three_bits_plus_zero() {
        // 5 = 0b101 → BIT1, BIT0, BIT1, ZERO (4 atoms total)
        let mut g = Web::new();
        let _pid = lower_int(5, &mut g);
        assert_eq!(g.atom_count(), 4);
    }
}
