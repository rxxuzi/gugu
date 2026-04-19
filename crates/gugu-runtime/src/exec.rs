//! Execution: lower + reduce + extract result.

use gugu_core::{AgentType, AtomId, PortId, Web};
use gugu_parser::ast::Program;
use gugu_reducer::{BloomInfo, Reducer, RunResult};

use crate::lower::{self, AgentRegistry, LowerError, LowerResult};

/// Per-bloom info surfaced to `exec_traced` callbacks.
#[derive(Debug, Clone)]
pub struct Trace<'a> {
    pub count: u64,
    pub lhs: &'a str,
    pub rhs: &'a str,
    pub inspected: bool,
}

const MAX_STEPS: u64 = 1_000_000;

#[derive(Debug)]
pub enum ExecError {
    Lower(LowerError),
    MaxSteps(u64),
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lower(e) => write!(f, "{e}"),
            Self::MaxSteps(n) => write!(f, "hit {n} step limit (infinite bloom?)"),
        }
    }
}

impl std::error::Error for ExecError {}

/// The outcome of running a .gu program.
pub struct ExecResult {
    pub web: Web,
    /// One `>out` port per `@GEN` block, in declaration order. Empty if no `@GEN`.
    pub output_ports: Vec<PortId>,
    pub blooms: u64,
    pub registry: lower::AgentRegistry,
}

/// Run a parsed program to slag and return the result.
pub fn exec(program: &Program) -> Result<ExecResult, ExecError> {
    exec_traced(program, |_| {})
}

/// Like `exec`, but `on_bloom(Trace { .. })` is called before every bloom
/// with resolved agent names and the `?`-inspection flag. Backs
/// `gugu run --trace` and `?expr` routing.
pub fn exec_traced(
    program: &Program,
    mut on_bloom: impl FnMut(Trace<'_>),
) -> Result<ExecResult, ExecError> {
    let LowerResult {
        web,
        rules,
        registry,
        output_ports,
    } = lower::lower(program).map_err(ExecError::Lower)?;

    let mut reducer = Reducer::new(web, rules);
    let result = reducer.run_traced(MAX_STEPS, |info: BloomInfo| {
        on_bloom(Trace {
            count: info.count,
            lhs: &agent_name_of(&registry, info.a),
            rhs: &agent_name_of(&registry, info.b),
            inspected: info.inspected,
        });
    });
    match result {
        RunResult::Slag(blooms) => Ok(ExecResult {
            web: reducer.web,
            output_ports,
            blooms,
            registry,
        }),
        RunResult::MaxSteps(n) => Err(ExecError::MaxSteps(n)),
    }
}

fn agent_name_of(reg: &AgentRegistry, ty: AgentType) -> String {
    reg.def(ty)
        .map(|d| d.name.clone())
        .unwrap_or_else(|| format!("?{}", ty.raw()))
}

/// Read the results from each `@GEN`'s `>out`, in declaration order.
pub fn read_outputs(result: &ExecResult) -> Vec<String> {
    result
        .output_ports
        .iter()
        .map(|&p| read_one(result, p))
        .collect()
}

fn read_one(result: &ExecResult, out: PortId) -> String {
    let g = &result.web;
    let reg = &result.registry;

    let Some(peer) = g.peer(out) else {
        return "<no output>".into();
    };

    let Some(port) = g.port(peer) else {
        return "<dangling>".into();
    };

    format_node(g, reg, port.atom, 0)
}

fn format_node(g: &Web, reg: &lower::AgentRegistry, nid: AtomId, depth: u32) -> String {
    if depth > 100 {
        return "...".into();
    }
    let Some(atom) = g.atom(nid) else {
        return "<removed>".into();
    };

    if let Some(n) = try_format_int(g, nid) {
        return n;
    }
    if let Some(s) = try_format_string(g, nid) {
        return s;
    }
    if let Some(c) = try_format_char(g, nid) {
        return c;
    }
    if let Some(l) = try_format_list(g, reg, nid, depth) {
        return l;
    }

    let name = agent_name(reg, atom.agent);

    if atom.arms().is_empty() {
        return name;
    }

    let mut parts = vec![name];
    for &aux_pid in atom.arms() {
        if let Some(peer) = g.peer(aux_pid) {
            if let Some(port) = g.port(peer) {
                if port.is_fuse() {
                    parts.push(format_node(g, reg, port.atom, depth + 1));
                } else {
                    parts.push(format!("p{}", peer.raw()));
                }
            } else {
                parts.push("?".into());
            }
        } else {
            parts.push("_".into());
        }
    }

    if parts.len() == 2 {
        format!("{}({})", parts[0], parts[1])
    } else {
        format!("{}({})", parts[0], parts[1..].join(", "))
    }
}

fn agent_name(reg: &lower::AgentRegistry, ty: AgentType) -> String {
    if let Some(def) = reg.def(ty) {
        def.name.clone()
    } else {
        format!("?{}", ty.raw())
    }
}

/// Decode a LSB-first BIT chain terminated by `@ZERO` into its decimal string.
/// `@NEG /val=<chain>` is decoded as `-<chain>`. Returns None if the atom is
/// not a BIT/ZERO/NEG chain, or the chain is malformed.
fn try_format_int(g: &Web, nid: AtomId) -> Option<String> {
    let atom = g.atom(nid)?;
    if atom.agent == AgentType::NEG {
        let val = *atom.arms().first()?;
        let peer = g.peer(val)?;
        let peer_port = g.port(peer)?;
        if !peer_port.is_fuse() {
            return None;
        }
        let inner = decode_nat_chain(g, peer_port.atom)?;
        // Don't print "-0" — a NEG wrapping zero is just zero.
        if inner == "0" {
            return Some(inner);
        }
        return Some(format!("-{inner}"));
    }
    decode_nat_chain(g, nid)
}

/// Decode a `@CHAR /code=<BIT chain>` into its `'x'` form. Returns None
/// if the atom isn't CHAR or the /code bond doesn't land on a BIT/ZERO
/// chain that fits in a Unicode scalar.
fn try_format_char(g: &Web, nid: AtomId) -> Option<String> {
    let ch = decode_char(g, nid)?;
    Some(format!("'{}'", escape_char_lit(ch)))
}

/// If `nid` is the head of a `CONS(CHAR, ...)` chain ending at `@NIL`,
/// render it as a `"..."` literal (escape-aware). Lists containing
/// non-CHAR heads return None so the general CONS(...) printer handles them.
fn try_format_string(g: &Web, nid: AtomId) -> Option<String> {
    let mut out = String::new();
    let mut current = nid;
    loop {
        let atom = g.atom(current)?;
        if atom.agent == AgentType::NIL {
            // Bare NIL is ambiguous between "" and [] — require at least one
            // CHAR so empty lists fall through to the list formatter as [].
            if out.is_empty() {
                return None;
            }
            return Some(format!("\"{out}\""));
        }
        if atom.agent != AgentType::CONS {
            return None;
        }
        let head_aux = *atom.arms().first()?;
        let tail_aux = *atom.arms().get(1)?;

        let head_peer = g.peer(head_aux)?;
        let head_port = g.port(head_peer)?;
        if !head_port.is_fuse() {
            return None;
        }
        let ch = decode_char(g, head_port.atom)?;
        out.push_str(&escape_str_char(ch));

        let tail_peer = g.peer(tail_aux)?;
        let tail_port = g.port(tail_peer)?;
        if !tail_port.is_fuse() {
            return None;
        }
        current = tail_port.atom;
    }
}

/// `[a, b, c]` rendering for a CONS chain whose heads are not all CHAR
/// (those take the string path earlier). Falls back to None if the chain
/// is malformed, so the generic `AGENT(...)` printer can take over.
fn try_format_list(
    g: &Web,
    reg: &lower::AgentRegistry,
    nid: AtomId,
    depth: u32,
) -> Option<String> {
    let atom = g.atom(nid)?;
    if atom.agent != AgentType::CONS && atom.agent != AgentType::NIL {
        return None;
    }
    let mut items = Vec::new();
    let mut current = nid;
    loop {
        let atom = g.atom(current)?;
        if atom.agent == AgentType::NIL {
            return Some(format!("[{}]", items.join(", ")));
        }
        if atom.agent != AgentType::CONS {
            return None;
        }
        let head_aux = *atom.arms().first()?;
        let tail_aux = *atom.arms().get(1)?;

        let head_peer = g.peer(head_aux)?;
        let head_port = g.port(head_peer)?;
        if !head_port.is_fuse() {
            return None;
        }
        items.push(format_node(g, reg, head_port.atom, depth + 1));

        let tail_peer = g.peer(tail_aux)?;
        let tail_port = g.port(tail_peer)?;
        if !tail_port.is_fuse() {
            return None;
        }
        current = tail_port.atom;
    }
}

fn decode_char(g: &Web, nid: AtomId) -> Option<char> {
    let atom = g.atom(nid)?;
    if atom.agent != AgentType::CHAR {
        return None;
    }
    let code_aux = *atom.arms().first()?;
    let code_peer = g.peer(code_aux)?;
    let code_port = g.port(code_peer)?;
    if !code_port.is_fuse() {
        return None;
    }
    let code_str = decode_nat_chain(g, code_port.atom)?;
    let code: u32 = code_str.parse().ok()?;
    char::from_u32(code)
}

fn escape_char_lit(c: char) -> String {
    match c {
        '\\' => "\\\\".into(),
        '\'' => "\\'".into(),
        '\n' => "\\n".into(),
        '\t' => "\\t".into(),
        c if (c as u32) < 0x20 => format!("\\u{{{:x}}}", c as u32),
        c => c.to_string(),
    }
}

fn escape_str_char(c: char) -> String {
    match c {
        '\\' => "\\\\".into(),
        '"' => "\\\"".into(),
        '\n' => "\\n".into(),
        '\t' => "\\t".into(),
        c if (c as u32) < 0x20 => format!("\\u{{{:x}}}", c as u32),
        c => c.to_string(),
    }
}

fn decode_nat_chain(g: &Web, nid: AtomId) -> Option<String> {
    let mut current = nid;
    let mut value: u64 = 0;
    let mut bit: u32 = 0;
    loop {
        let atom = g.atom(current)?;
        if atom.agent == AgentType::ZERO {
            return Some(value.to_string());
        }
        let digit: u64 = if atom.agent == AgentType::BIT1 {
            1
        } else if atom.agent == AgentType::BIT0 {
            0
        } else {
            return None;
        };
        if bit >= 64 {
            return None;
        }
        value |= digit << bit;
        bit += 1;
        let hi = *atom.arms().first()?;
        let peer = g.peer(hi)?;
        let peer_port = g.port(peer)?;
        if !peer_port.is_fuse() {
            return None;
        }
        current = peer_port.atom;
    }
}
