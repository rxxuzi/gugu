use gugu_lexer::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub pack: Option<PackDecl>,
    pub uses: Vec<UseDecl>,
    pub items: Vec<TopLevel>,
    /// `@GEN :` blocks — 0 / 1 / many, in declaration order.
    pub gens: Vec<GenBlock>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PackDecl {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UseDecl {
    pub module: String,
    pub alias: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TopLevel {
    Mod(ModDef),
    Agent(AgentDef),
    Rule(RuleDef),
    Alias(AliasDef),
    Frag(FragDef),
    Test(TestBlock),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModDef {
    pub name: String,
    pub items: Vec<ModItem>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModItem {
    Agent(AgentDef),
    Rule(RuleDef),
    Alias(AliasDef),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentDef {
    pub is_pub: bool,
    pub name: String,
    pub ports: Vec<PortDecl>,
    pub span: Span,
}

/// A port declaration in an agent definition.
#[derive(Debug, Clone, PartialEq)]
pub enum PortDecl {
    /// `/name` or `/name:Type`
    Arm {
        name: String,
        ty: Option<String>,
        span: Span,
    },
    /// `^name` — marks this position as the fuse (fuse)
    Fuse { name: String, span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AliasDef {
    pub agent: String,
    pub target: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleDef {
    pub modifier: Option<RuleModifier>,
    pub is_pub: bool,
    pub lhs: String,
    pub rhs: RuleTarget,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleModifier {
    Lazy,
    Inline,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuleTarget {
    /// Single agent or OR pattern: `@A | @B | @C`
    Agents(Vec<String>),
    /// `_` wildcard
    Wildcard,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FragDef {
    pub name: String,
    pub ports: Vec<PortDecl>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TestBlock {
    pub label: String,
    pub lhs: Expr,
    pub rhs: Expr,
    pub span: Span,
}

/// `@GEN : <stmts>` — a starting web. `@GEN` itself is a syntactic marker,
/// not a web atom.
#[derive(Debug, Clone, PartialEq)]
pub struct GenBlock {
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// expr `--` expr
    Bond { lhs: Expr, rhs: Expr, span: Span },
    /// Standalone expression (possibly containing `->`)
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// `@AGENT` with optional arm assigns / paren args / bare args
    Agent {
        name: String,
        args: Vec<AgentArg>,
        span: Span,
    },
    /// `expr -> expr` (directed bond to fuse)
    Connect {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    /// `/name` — arm name or bond label
    PortName(String, Span),
    /// `~/name` — self arm reference (inside rules)
    SelfPort(String, Span),
    /// `>>name` — anonymous bond
    AnonBond(String, Span),
    /// `>out`
    Output(Span),
    /// `$name` — web fragment reference
    Fragment(String, Span),
    /// `!expr` — force bloom
    Force(Box<Expr>, Span),
    /// `?expr` — web inspect
    Inspect(Box<Expr>, Span),
    /// Integer literal
    IntLit(i64, Span),
    /// Float literal
    FloatLit(f64, Span),
    /// String literal
    StrLit(String, Span),
    /// Char literal
    CharLit(char, Span),
    /// `true` / `false`
    BoolLit(bool, Span),
    /// `era`, `dup`, `err` — built-in atom sugar
    BuiltinAtom(BuiltinAtom, Span),
    /// `[expr, expr, ...]` list sugar
    List { items: Vec<Expr>, span: Span },
    /// `(expr)` — parenthesized expression
    Paren(Box<Expr>, Span),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinAtom {
    Era,
    Dup,
    Err,
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Self::Agent { span, .. }
            | Self::Connect { span, .. }
            | Self::PortName(_, span)
            | Self::SelfPort(_, span)
            | Self::AnonBond(_, span)
            | Self::Output(span)
            | Self::Fragment(_, span)
            | Self::Force(_, span)
            | Self::Inspect(_, span)
            | Self::IntLit(_, span)
            | Self::FloatLit(_, span)
            | Self::StrLit(_, span)
            | Self::CharLit(_, span)
            | Self::BoolLit(_, span)
            | Self::BuiltinAtom(_, span)
            | Self::List { span, .. }
            | Self::Paren(_, span) => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentArg {
    /// `/port = expr`
    Port(PortAssign),
    /// Positional argument (paren or arity consumption)
    Positional(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PortAssign {
    pub name: String,
    pub value: Expr,
    pub span: Span,
}
