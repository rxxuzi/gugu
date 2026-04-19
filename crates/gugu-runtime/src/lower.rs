//! AST → Web lowering.
//!
//! Walks the parsed Program, registers agent definitions, builds the
//! rule table, and constructs the initial web from the main block.

use std::collections::HashMap;
use std::sync::Arc;

use gugu_core::{AgentDef, AgentType, AtomId, PortId, Web};
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

/// Visibility metadata for user-declared agents. Built-ins have no entry
/// and are treated as globally visible.
#[derive(Debug, Clone)]
pub struct AgentMeta {
    pub mod_name: Option<String>,
    pub is_pub: bool,
}

/// Maps agent names to their types and definitions.
pub struct AgentRegistry {
    by_name: HashMap<String, AgentType>,
    defs: HashMap<AgentType, AgentDef>,
    meta: HashMap<AgentType, AgentMeta>,
    next_user: u32,
}

impl AgentRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            by_name: HashMap::new(),
            defs: HashMap::new(),
            meta: HashMap::new(),
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
        self.register_with_meta(name, ports, None, true)
    }

    pub fn register_with_meta(
        &mut self,
        name: &str,
        ports: Vec<String>,
        mod_name: Option<String>,
        is_pub: bool,
    ) -> AgentType {
        if let Some(&ty) = self.by_name.get(name) {
            return ty;
        }
        let ty = AgentType::user(self.next_user);
        self.next_user += 1;
        self.by_name.insert(name.into(), ty);
        self.defs.insert(ty, AgentDef::new(ty, name, ports));
        self.meta.insert(ty, AgentMeta { mod_name, is_pub });
        ty
    }

    pub fn lookup(&self, name: &str) -> Option<AgentType> {
        self.by_name.get(name).copied()
    }

    pub fn def(&self, ty: AgentType) -> Option<&AgentDef> {
        self.defs.get(&ty)
    }

    pub fn meta(&self, ty: AgentType) -> Option<&AgentMeta> {
        self.meta.get(&ty)
    }

    pub fn arity(&self, ty: AgentType) -> u32 {
        self.defs.get(&ty).map(|d| d.arity() as u32).unwrap_or(1)
    }

    /// Can an agent declared in `agent_mod` (with visibility `is_pub`) be
    /// referenced from the scope `from_mod`?  Global-scope agents are
    /// always visible; mod-scoped ones need either the `pub` keyword or
    /// a same-mod reference.
    pub fn is_visible_from(&self, ty: AgentType, from_mod: Option<&str>) -> bool {
        match self.meta.get(&ty) {
            None => true, // builtin
            Some(m) => match &m.mod_name {
                None => true,
                Some(owner) => m.is_pub || Some(owner.as_str()) == from_mod,
            },
        }
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

    // Phase 2: collect fragments. Stdlib currently declares none, but keep
    // the scan symmetric so future stdlib fragments get picked up too.
    let mut frag_defs: HashMap<String, ast::FragDef> = HashMap::new();
    for prog in &stdlib_programs {
        for item in &prog.items {
            register_fragment(item, &mut frag_defs)?;
        }
    }
    for item in &program.items {
        register_fragment(item, &mut frag_defs)?;
    }
    let fragments: FragMap = Arc::new(frag_defs);

    // Phase 3: register rules. Stdlib rules register first and win `lookup`
    // ordering; user redefinitions of identical pairs are shadowed.
    for prog in &stdlib_programs {
        for item in &prog.items {
            if let ast::TopLevel::Rule(rule_def) = item {
                register_rule(rule_def, &reg, &mut rules, &fragments, None)?;
            }
        }
    }
    for item in &program.items {
        if let ast::TopLevel::Rule(rule_def) = item {
            register_rule(rule_def, &reg, &mut rules, &fragments, None)?;
        }
        if let ast::TopLevel::Mod(mod_def) = item {
            for mi in &mod_def.items {
                if let ast::ModItem::Rule(rule_def) = mi {
                    register_rule(rule_def, &reg, &mut rules, &fragments, Some(&mod_def.name))?;
                }
            }
        }
    }

    // Phase 4: lower each @GEN block into the web as independent components
    let mut output_ports = Vec::with_capacity(program.gens.len());
    for block in &program.gens {
        output_ports.push(lower_gen(block, &reg, &fragments, &mut web)?);
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
        ast::TopLevel::Agent(a) => register_one_agent(a, reg, None)?,
        ast::TopLevel::Mod(m) => {
            for mi in &m.items {
                if let ast::ModItem::Agent(a) = mi {
                    register_one_agent(a, reg, Some(m.name.as_str()))?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn register_fragment(
    item: &ast::TopLevel,
    frags: &mut HashMap<String, ast::FragDef>,
) -> Result<(), LowerError> {
    let ast::TopLevel::Frag(f) = item else {
        return Ok(());
    };
    if !f.ports.is_empty() {
        return Err(err(format!(
            "parameterized fragment ${} not yet supported",
            f.name
        )));
    }
    if let Some(bad) = find_self_port(&f.value) {
        return Err(err(format!(
            "fragment ${}: ~/{bad} has no LHS/RHS context to resolve against",
            f.name
        )));
    }
    if frags.contains_key(&f.name) {
        return Err(err(format!("duplicate fragment ${}", f.name)));
    }
    frags.insert(f.name.clone(), f.clone());
    Ok(())
}

fn find_self_port(e: &ast::Expr) -> Option<String> {
    match e {
        ast::Expr::SelfPort(n, _) => Some(n.clone()),
        ast::Expr::Agent { args, .. } => args.iter().find_map(|a| match a {
            ast::AgentArg::Port(pa) => find_self_port(&pa.value),
            ast::AgentArg::Positional(e) => find_self_port(e),
        }),
        ast::Expr::Connect { lhs, rhs, .. } => {
            find_self_port(lhs).or_else(|| find_self_port(rhs))
        }
        ast::Expr::Paren(i, _) | ast::Expr::Force(i, _) | ast::Expr::Inspect(i, _) => {
            find_self_port(i)
        }
        ast::Expr::List { items, .. } => items.iter().find_map(find_self_port),
        _ => None,
    }
}

fn register_one_agent(
    a: &ast::AgentDef,
    reg: &mut AgentRegistry,
    mod_name: Option<&str>,
) -> Result<(), LowerError> {
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
    reg.register_with_meta(&a.name, ports, mod_name.map(String::from), a.is_pub);
    Ok(())
}

fn register_rule(
    rule_def: &ast::RuleDef,
    reg: &AgentRegistry,
    rules: &mut RuleTable,
    fragments: &FragMap,
    current_mod: Option<&str>,
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
                let rpm = build_rule_port_map(
                    reg,
                    &rule_def.lhs,
                    rhs_name,
                    fragments.clone(),
                    current_mod,
                );
                check_self_ports(&rule_def.body, &rule_def.lhs, rhs_name, &rpm)?;
                check_agent_visibility(&rule_def.body, reg, current_mod)?;
                let body = rule_def.body.clone();
                rules.add(lhs_ty, rhs_ty, move |web, ctx| {
                    let mut env = build_rule_env(ctx, &rpm);
                    for stmt in &body {
                        exec_stmt(stmt, web, &mut env, &rpm);
                    }
                    flatten_bridges(&mut env, web);
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
            let rpm = build_wildcard_port_map(reg, &rule_def.lhs, fragments.clone(), current_mod);
            check_self_ports(&rule_def.body, &rule_def.lhs, "_", &rpm)?;
            check_agent_visibility(&rule_def.body, reg, current_mod)?;
            let body = rule_def.body.clone();
            rules.add_wildcard(lhs_ty, move |web, ctx| {
                let mut env = build_rule_env(ctx, &rpm);
                for stmt in &body {
                    exec_stmt(stmt, web, &mut env, &rpm);
                }
                flatten_bridges(&mut env, web);
            });
        }
    }
    Ok(())
}

fn build_wildcard_port_map(
    reg: &AgentRegistry,
    lhs_name: &str,
    fragments: FragMap,
    current_mod: Option<&str>,
) -> RulePortMap {
    let lhs_ty = reg.lookup(lhs_name).unwrap();
    RulePortMap {
        lhs_ports: reg.def(lhs_ty).map(|d| d.ports.clone()).unwrap_or_default(),
        rhs_ports: Vec::new(),
        agents: agent_info_map(reg),
        fragments,
        current_mod: current_mod.map(String::from),
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

    let mut counts: HashMap<String, u32> = HashMap::new();
    for s in body {
        walk_stmt(s, &mut |name| {
            *counts.entry(name.to_string()).or_insert(0) += 1;
        });
    }
    // Every `~/name` used in the body must actually be an arm somewhere.
    for name in counts.keys() {
        let found = rpm.lhs_ports.iter().any(|p| p == name)
            || rpm.rhs_ports.iter().any(|p| p == name);
        if !found {
            return Err(err(format!(
                "rule @{lhs_name} >< @{rhs_name}: ~/{name} is not an arm of either agent"
            )));
        }
    }
    // Linearity: each arm (LHS + RHS) must be referenced exactly once in
    // the body. Missing → the peer is left dangling after bloom. Repeated →
    // the same peer gets bonded multiple times (prior bonds silently lost).
    for name in rpm.lhs_ports.iter().chain(rpm.rhs_ports.iter()) {
        let n = counts.get(name).copied().unwrap_or(0);
        if n != 1 {
            return Err(err(format!(
                "rule @{lhs_name} >< @{rhs_name}: ~/{name} referenced {n} times — must be exactly 1"
            )));
        }
    }
    Ok(())
}

fn check_agent_visibility(
    body: &[ast::Stmt],
    reg: &AgentRegistry,
    current_mod: Option<&str>,
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
            ast::Expr::Agent { name, args, .. } => {
                visit(name);
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
            ast::Expr::Paren(i, _) | ast::Expr::Force(i, _) | ast::Expr::Inspect(i, _) => {
                walk_expr(i, visit);
            }
            ast::Expr::List { items, .. } => {
                for it in items {
                    walk_expr(it, visit);
                }
            }
            _ => {}
        }
    }

    let mut bad: Option<(String, String)> = None;
    for s in body {
        walk_stmt(s, &mut |name| {
            if bad.is_some() {
                return;
            }
            if let Some(ty) = reg.lookup(name)
                && !reg.is_visible_from(ty, current_mod)
                && let Some(owner) = reg.meta(ty).and_then(|m| m.mod_name.clone())
            {
                bad = Some((name.to_string(), owner));
            }
        });
        if bad.is_some() {
            break;
        }
    }
    if let Some((name, owner)) = bad {
        return Err(err(format!(
            "agent @{name} is private in mod {owner} — declare it `pub` to reference it from outside"
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
    mod_name: Option<String>,
    is_pub: bool,
}

type FragMap = Arc<HashMap<String, ast::FragDef>>;

#[derive(Clone)]
struct RulePortMap {
    lhs_ports: Vec<String>,
    rhs_ports: Vec<String>,
    agents: HashMap<String, AgentInfo>,
    fragments: FragMap,
    /// Mod in which the rule body / GEN block was declared. `None` for
    /// top-level code. Used to gate access to non-`pub` agents declared
    /// in other mods.
    current_mod: Option<String>,
}

fn build_rule_port_map(
    reg: &AgentRegistry,
    lhs_name: &str,
    rhs_name: &str,
    fragments: FragMap,
    current_mod: Option<&str>,
) -> RulePortMap {
    let lhs_ty = reg.lookup(lhs_name).unwrap();
    let rhs_ty = reg.lookup(rhs_name).unwrap();
    let lhs_def = reg.def(lhs_ty);
    let rhs_def = reg.def(rhs_ty);

    RulePortMap {
        lhs_ports: lhs_def.map(|d| d.ports.clone()).unwrap_or_default(),
        rhs_ports: rhs_def.map(|d| d.ports.clone()).unwrap_or_default(),
        agents: agent_info_map(reg),
        fragments,
        current_mod: current_mod.map(String::from),
    }
}

fn agent_info_map(reg: &AgentRegistry) -> HashMap<String, AgentInfo> {
    let mut agents = HashMap::new();
    for (name, &ty) in &reg.by_name {
        let ports = reg.def(ty).map(|d| d.ports.clone()).unwrap_or_default();
        let (mod_name, is_pub) = reg
            .meta(ty)
            .map(|m| (m.mod_name.clone(), m.is_pub))
            .unwrap_or((None, true));
        agents.insert(
            name.clone(),
            AgentInfo {
                ty,
                arity: reg.arity(ty),
                ports,
                mod_name,
                is_pub,
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
    /// Nesting depth of `$fragment` expansions; caps at 64 to catch cycles.
    frag_depth: u32,
}

impl ExecEnv {
    fn new() -> Self {
        Self {
            bonds: HashMap::new(),
            anon_bridges: HashMap::new(),
            anon_refs: HashMap::new(),
            pending_flatten: Vec::new(),
            errors: Vec::new(),
            frag_depth: 0,
        }
    }

    /// Fresh scope for a `$fragment` body; labels and anon bonds are
    /// isolated from the caller so the same fragment can be inlined many
    /// times without collisions.
    fn for_fragment(parent_depth: u32) -> Self {
        let mut env = Self::new();
        env.frag_depth = parent_depth + 1;
        env
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
        ast::Expr::PortName(name, _) => eval_bridge_ref(env, web, &format!("/{name}")),
        ast::Expr::SelfPort(name, _) => env.bonds.get(&format!("~/{name}")).copied(),
        ast::Expr::AnonBond(name, _) => eval_bridge_ref(env, web, &format!(">>{name}")),
        ast::Expr::Output(_) => eval_output(env, web),
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
        ast::Expr::Force(inner, _) => eval_mark(inner, web, env, rpm, Web::force),
        ast::Expr::Inspect(inner, _) => eval_mark(inner, web, env, rpm, Web::inspect),
        ast::Expr::IntLit(n, _) => Some(lower_int(*n, web)),
        ast::Expr::CharLit(c, _) => Some(lower_char(*c, web)),
        ast::Expr::StrLit(s, _) => Some(lower_str(s, web)),
        ast::Expr::List { items, .. } => Some(lower_list(items, web, env, rpm)),
        ast::Expr::Fragment(name, _) => eval_fragment(name, web, env, rpm),
        _ => None, // other literals: later sub-phases
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
    if let Some(owner) = &info.mod_name {
        let same_mod = rpm.current_mod.as_deref() == Some(owner.as_str());
        if !info.is_pub && !same_mod {
            env.errors.push(format!(
                "agent @{name} is private in mod {owner} — declare it `pub` to reference it from outside"
            ));
            return None;
        }
    }
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

/// `>out` is the GEN block's output port. The runtime reads from here
/// after reducing to slag, so it must be referenced exactly once in the
/// user's web — a second reference would either silently overwrite the
/// first bond (linearity break) or leave the reader unreachable.
fn eval_output(env: &mut ExecEnv, web: &mut Web) -> Option<PortId> {
    if env.bonds.contains_key(">out") {
        env.errors
            .push(">out referenced more than once in @GEN body".into());
        return None;
    }
    let nid = web.add_atom(AgentType::NIL, 1);
    let pid = web.atom(nid).unwrap().fuse();
    env.bonds.insert(">out".into(), pid);
    Some(pid)
}

/// A label (`/name` or `>>name`) is a single bond between exactly two
/// textual occurrences. We don't know either endpoint's port at the
/// moment each reference is evaluated, so we bridge through a 2-port NIL
/// atom:
///   1st ref returns NIL.arm[0]; the caller bonds its port there.
///   2nd ref returns NIL.fuse; the second caller bonds its port there.
///   Scope end: `flatten_bridges` splices the two peers directly and
///   removes the NIL.
/// Three references would break linearity (one port, one bond) and
/// surface as a lowering error.
fn eval_bridge_ref(env: &mut ExecEnv, web: &mut Web, key: &str) -> Option<PortId> {
    let count = env.anon_refs.entry(key.to_string()).or_insert(0);
    *count += 1;
    match *count {
        1 => {
            let nid = web.add_atom(AgentType::NIL, 2);
            let arm = web.atom(nid).unwrap().arms()[0];
            env.anon_bridges.insert(key.to_string(), nid);
            Some(arm)
        }
        2 => {
            let nid = env.anon_bridges.remove(key)?;
            env.pending_flatten.push(nid);
            web.atom(nid).map(|n| n.fuse())
        }
        n => {
            env.errors
                .push(format!("{key} referenced {n} times — must be exactly 2"));
            None
        }
    }
}

/// Short-circuit every bridge NIL created by `eval_bridge_ref`: take its
/// two endpoints' peers, remove the bridge, and bond the peers directly.
/// A bridge that never saw its second reference is a linearity error.
fn flatten_bridges(env: &mut ExecEnv, web: &mut Web) {
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
    for (key, nid) in env.anon_bridges.drain() {
        web.remove_atom(nid);
        env.errors
            .push(format!("{key} referenced once — must be exactly 2"));
    }
    env.anon_refs.clear();
}

/// `!expr` and `?expr` — evaluate the inner expression, then tag the atom
/// it resolved to. `!` (priority) or `?` (inspection) differ only in
/// which setter we call on `Web`.
fn eval_mark(
    inner: &ast::Expr,
    web: &mut Web,
    env: &mut ExecEnv,
    rpm: &RulePortMap,
    mark: fn(&mut Web, AtomId),
) -> Option<PortId> {
    let pid = eval_expr(inner, web, env, rpm)?;
    let atom_id = web.port(pid)?.atom;
    mark(web, atom_id);
    Some(pid)
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

const FRAG_DEPTH_LIMIT: u32 = 64;

/// Inline a fresh copy of `$name`'s body. Each call gets its own ExecEnv
/// so labels and anon bonds in the body never collide with the caller or
/// with another expansion of the same fragment.
fn eval_fragment(
    name: &str,
    web: &mut Web,
    env: &mut ExecEnv,
    rpm: &RulePortMap,
) -> Option<PortId> {
    if env.frag_depth >= FRAG_DEPTH_LIMIT {
        env.errors.push(format!(
            "fragment ${name} exceeded depth {FRAG_DEPTH_LIMIT} — cycle?"
        ));
        return None;
    }
    let Some(frag) = rpm.fragments.get(name) else {
        env.errors.push(format!("unknown fragment ${name}"));
        return None;
    };
    let mut sub = ExecEnv::for_fragment(env.frag_depth);
    let out = eval_expr(&frag.value, web, &mut sub, rpm);
    flatten_bridges(&mut sub, web);
    env.errors.extend(sub.errors);
    out
}

fn lower_gen(
    block: &ast::GenBlock,
    reg: &AgentRegistry,
    fragments: &FragMap,
    web: &mut Web,
) -> Result<PortId, LowerError> {
    let rpm = RulePortMap {
        lhs_ports: vec![],
        rhs_ports: vec![],
        agents: agent_info_map(reg),
        fragments: fragments.clone(),
        current_mod: None,
    };

    // Each @GEN is an independent component → fresh bond env so >out doesn't leak between blocks.
    let mut env = ExecEnv::new();

    for stmt in &block.body {
        exec_stmt(stmt, web, &mut env, &rpm);
    }
    flatten_bridges(&mut env, web);

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
