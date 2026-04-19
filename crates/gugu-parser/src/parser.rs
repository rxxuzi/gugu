use gugu_lexer::{Span, Token, TokenKind};

use crate::ast::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub msg: String,
    pub span: Span,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "parse error at {}..{}: {}",
            self.span.start, self.span.end, self.msg
        )
    }
}

impl std::error::Error for ParseError {}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&TokenKind> {
        self.tokens.get(self.pos).map(|t| &t.kind)
    }

    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn span_here(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|t| t.span)
            .unwrap_or_else(|| {
                self.tokens
                    .last()
                    .map(|t| Span::new(t.span.end as usize, t.span.end as usize))
                    .unwrap_or(Span::new(0, 0))
            })
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos];
        self.pos += 1;
        tok
    }

    fn expect(&mut self, expected: &TokenKind) -> Result<Span, ParseError> {
        match self.peek() {
            Some(k) if k == expected => Ok(self.advance().span),
            _ => Err(self.err(format!("expected {expected:?}"))),
        }
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.peek() == Some(kind) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn err(&self, msg: impl Into<String>) -> ParseError {
        ParseError {
            msg: msg.into(),
            span: self.span_here(),
        }
    }

    fn err_at(&self, span: Span, msg: impl Into<String>) -> ParseError {
        ParseError {
            msg: msg.into(),
            span,
        }
    }

    fn span_from(&self, start: Span) -> Span {
        let end = if self.pos > 0 {
            self.tokens[self.pos - 1].span.end
        } else {
            start.end
        };
        Span::new(start.start as usize, end as usize)
    }

    fn at_top_keyword(&self) -> bool {
        matches!(
            self.peek(),
            Some(
                TokenKind::Agent
                    | TokenKind::Rule
                    | TokenKind::Alias
                    | TokenKind::Mod
                    | TokenKind::Gen
                    | TokenKind::End
                    | TokenKind::Use
                    | TokenKind::Pack
                    | TokenKind::Pub
                    | TokenKind::Lazy
                    | TokenKind::Inline
                    | TokenKind::Test
                    | TokenKind::Extern
            )
        )
    }

    fn at_mod_item_start(&self) -> bool {
        matches!(
            self.peek(),
            Some(
                TokenKind::Agent
                    | TokenKind::Rule
                    | TokenKind::Alias
                    | TokenKind::Pub
                    | TokenKind::Lazy
                    | TokenKind::Inline
            )
        )
    }

    fn can_start_stmt(&self) -> bool {
        !self.at_end()
            && !self.at_top_keyword()
            && !matches!(self.peek(), Some(TokenKind::RBrace | TokenKind::EqEq))
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let pack = if self.peek() == Some(&TokenKind::Pack) {
            Some(self.parse_pack()?)
        } else {
            None
        };
        let mut uses = Vec::new();
        while self.peek() == Some(&TokenKind::Use) {
            uses.push(self.parse_use()?);
        }
        let items = self.parse_top_items()?;
        let mut gens = Vec::new();
        while self.peek() == Some(&TokenKind::Gen) {
            gens.push(self.parse_gen()?);
        }
        if !self.at_end() {
            return Err(self.err("unexpected token at top level"));
        }
        Ok(Program {
            pack,
            uses,
            items,
            gens,
        })
    }

    fn parse_top_items(&mut self) -> Result<Vec<TopLevel>, ParseError> {
        let mut items = Vec::new();
        loop {
            match self.peek() {
                Some(TokenKind::Agent) => items.push(TopLevel::Agent(self.parse_agent_def(false)?)),
                Some(TokenKind::Rule) => {
                    items.push(TopLevel::Rule(self.parse_rule_def(false, None)?))
                }
                Some(TokenKind::Alias) => items.push(TopLevel::Alias(self.parse_alias_def()?)),
                Some(TokenKind::Mod) => items.push(TopLevel::Mod(self.parse_mod_def()?)),
                Some(TokenKind::Test) => items.push(TopLevel::Test(self.parse_test_block()?)),
                Some(TokenKind::Pub) => items.push(self.parse_pub_item()?),
                Some(TokenKind::Lazy) | Some(TokenKind::Inline) => {
                    items.push(TopLevel::Rule(self.parse_modified_rule(false)?))
                }
                Some(TokenKind::Fragment(_)) => items.push(TopLevel::Frag(self.parse_frag_def()?)),
                _ => break,
            }
        }
        Ok(items)
    }

    fn parse_pub_item(&mut self) -> Result<TopLevel, ParseError> {
        let start = self.advance().span;
        match self.peek() {
            Some(TokenKind::Agent) => Ok(TopLevel::Agent(self.parse_agent_def(true)?)),
            Some(TokenKind::Rule) => Ok(TopLevel::Rule(self.parse_rule_def(true, None)?)),
            Some(TokenKind::Lazy) | Some(TokenKind::Inline) => {
                Ok(TopLevel::Rule(self.parse_modified_rule(true)?))
            }
            _ => Err(self.err_at(start, "expected agent or rule after 'pub'")),
        }
    }

    fn parse_pack(&mut self) -> Result<PackDecl, ParseError> {
        let start = self.advance().span;
        let name = self.expect_mod_name()?;
        self.expect(&TokenKind::Colon)?;
        Ok(PackDecl {
            name,
            span: self.span_from(start),
        })
    }

    fn parse_use(&mut self) -> Result<UseDecl, ParseError> {
        let start = self.advance().span;
        let module = self.expect_mod_name()?;
        let alias = if self.eat(&TokenKind::As) {
            Some(self.expect_mod_name()?)
        } else {
            None
        };
        Ok(UseDecl {
            module,
            alias,
            span: self.span_from(start),
        })
    }

    fn parse_agent_def(&mut self, is_pub: bool) -> Result<AgentDef, ParseError> {
        let start = self.advance().span; // consume 'agent'
        let name = self.expect_agent_name()?;
        let ports = self.parse_port_decls()?;
        Ok(AgentDef {
            is_pub,
            name,
            ports,
            span: self.span_from(start),
        })
    }

    fn parse_port_decls(&mut self) -> Result<Vec<PortDecl>, ParseError> {
        let mut ports = Vec::new();
        loop {
            match self.peek() {
                Some(TokenKind::PortName(_)) => {
                    let tok = self.advance();
                    let pstart = tok.span;
                    let name = match &tok.kind {
                        TokenKind::PortName(n) => n.clone(),
                        _ => unreachable!(),
                    };
                    let ty = if self.eat(&TokenKind::Colon) {
                        Some(self.expect_mod_name()?)
                    } else {
                        None
                    };
                    ports.push(PortDecl::Arm {
                        name,
                        ty,
                        span: self.span_from(pstart),
                    });
                }
                Some(TokenKind::Caret) => {
                    let pstart = self.advance().span;
                    let name = self.expect_ident()?;
                    ports.push(PortDecl::Fuse {
                        name,
                        span: self.span_from(pstart),
                    });
                }
                _ => break,
            }
        }
        Ok(ports)
    }

    fn parse_alias_def(&mut self) -> Result<AliasDef, ParseError> {
        let start = self.advance().span;
        let agent = self.expect_agent_name()?;
        match self.peek() {
            Some(TokenKind::Equals) | Some(TokenKind::As) => {
                self.advance();
            }
            _ => return Err(self.err("expected '=' or 'as' in alias")),
        }
        let target = self.expect_ident()?;
        Ok(AliasDef {
            agent,
            target,
            span: self.span_from(start),
        })
    }

    fn parse_modified_rule(&mut self, is_pub: bool) -> Result<RuleDef, ParseError> {
        let modifier = match self.peek() {
            Some(TokenKind::Lazy) => {
                self.advance();
                Some(RuleModifier::Lazy)
            }
            Some(TokenKind::Inline) => {
                self.advance();
                Some(RuleModifier::Inline)
            }
            _ => None,
        };
        self.parse_rule_def(is_pub, modifier)
    }

    fn parse_rule_def(
        &mut self,
        is_pub: bool,
        modifier: Option<RuleModifier>,
    ) -> Result<RuleDef, ParseError> {
        let start = self.advance().span; // consume 'rule'
        let lhs = self.expect_agent_name()?;
        self.expect(&TokenKind::Fire)?;
        let rhs = self.parse_rule_target()?;
        self.expect(&TokenKind::Colon)?;
        let body = self.parse_stmts()?;
        if body.is_empty() {
            if self.peek() == Some(&TokenKind::Gen) {
                return Err(self.err(
                    "@GEN cannot appear inside rule body — it is a top-level marker only",
                ));
            }
            return Err(self.err_at(start, "rule body must have at least one statement"));
        }
        Ok(RuleDef {
            modifier,
            is_pub,
            lhs,
            rhs,
            body,
            span: self.span_from(start),
        })
    }

    fn parse_rule_target(&mut self) -> Result<RuleTarget, ParseError> {
        if self.eat(&TokenKind::Wildcard) {
            return Ok(RuleTarget::Wildcard);
        }
        let first = self.expect_agent_name()?;
        let mut agents = vec![first];
        while self.eat(&TokenKind::Pipe) {
            agents.push(self.expect_agent_name()?);
        }
        Ok(RuleTarget::Agents(agents))
    }

    fn parse_mod_def(&mut self) -> Result<ModDef, ParseError> {
        let start = self.advance().span;
        let name = self.expect_mod_name()?;
        let brace = self.expect_block_open()?;
        let items = self.parse_mod_items()?;
        if brace {
            self.expect(&TokenKind::RBrace)?;
        } else {
            self.eat(&TokenKind::End);
        }
        Ok(ModDef {
            name,
            items,
            span: self.span_from(start),
        })
    }

    fn expect_block_open(&mut self) -> Result<bool, ParseError> {
        match self.peek() {
            Some(TokenKind::Colon) => {
                self.advance();
                Ok(false)
            }
            Some(TokenKind::LBrace) => {
                self.advance();
                Ok(true)
            }
            _ => Err(self.err("expected ':' or '{'")),
        }
    }

    fn parse_mod_items(&mut self) -> Result<Vec<ModItem>, ParseError> {
        let mut items = Vec::new();
        while self.at_mod_item_start() {
            items.push(self.parse_mod_item()?);
        }
        Ok(items)
    }

    fn parse_mod_item(&mut self) -> Result<ModItem, ParseError> {
        let is_pub = self.eat(&TokenKind::Pub);
        let modifier = self.eat_rule_modifier();
        match self.peek() {
            Some(TokenKind::Agent) => Ok(ModItem::Agent(self.parse_agent_def(is_pub)?)),
            Some(TokenKind::Rule) => Ok(ModItem::Rule(self.parse_rule_def(is_pub, modifier)?)),
            Some(TokenKind::Alias) => Ok(ModItem::Alias(self.parse_alias_def()?)),
            _ => Err(self.err("expected agent, rule, or alias in mod")),
        }
    }

    fn eat_rule_modifier(&mut self) -> Option<RuleModifier> {
        match self.peek() {
            Some(TokenKind::Lazy) => {
                self.advance();
                Some(RuleModifier::Lazy)
            }
            Some(TokenKind::Inline) => {
                self.advance();
                Some(RuleModifier::Inline)
            }
            _ => None,
        }
    }

    fn parse_frag_def(&mut self) -> Result<FragDef, ParseError> {
        let tok = self.advance();
        let start = tok.span;
        let name = match &tok.kind {
            TokenKind::Fragment(n) => n.clone(),
            _ => unreachable!(),
        };
        let ports = self.parse_port_decls()?;
        self.expect(&TokenKind::Equals)?;
        let value = self.parse_connect()?;
        Ok(FragDef {
            name,
            ports,
            value,
            span: self.span_from(start),
        })
    }

    fn parse_test_block(&mut self) -> Result<TestBlock, ParseError> {
        let start = self.advance().span; // consume 'test'
        let label = self.expect_str_lit()?;
        self.expect(&TokenKind::Colon)?;
        let lhs = self.parse_connect()?;
        self.expect(&TokenKind::EqEq)?;
        let rhs = self.parse_connect()?;
        Ok(TestBlock {
            label,
            lhs,
            rhs,
            span: self.span_from(start),
        })
    }

    fn parse_gen(&mut self) -> Result<GenBlock, ParseError> {
        let start = self.advance().span; // consume @GEN
        self.expect(&TokenKind::Colon)?;
        let body = self.parse_stmts()?;
        if body.is_empty() {
            if self.peek() == Some(&TokenKind::Gen) {
                return Err(self.err(
                    "@GEN cannot appear inside expression — it is a top-level marker only",
                ));
            }
            return Err(self.err_at(start, "@GEN block must have at least one statement"));
        }
        Ok(GenBlock {
            body,
            span: self.span_from(start),
        })
    }

    //    // Precedence (high -> low): () > arm-assign > -> > --

    fn parse_stmts(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut stmts = Vec::new();
        while self.can_start_stmt() {
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let lhs = self.parse_connect()?;
        if self.eat(&TokenKind::Bond) {
            let rhs = self.parse_connect()?;
            let span = Span::new(lhs.span().start as usize, rhs.span().end as usize);
            Ok(Stmt::Bond { lhs, rhs, span })
        } else {
            Ok(Stmt::Expr(lhs))
        }
    }

    fn parse_connect(&mut self) -> Result<Expr, ParseError> {
        let lhs = self.parse_atom()?;
        if self.eat(&TokenKind::Arrow) {
            let rhs = self.parse_atom()?;
            let span = Span::new(lhs.span().start as usize, rhs.span().end as usize);
            Ok(Expr::Connect {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            })
        } else {
            Ok(lhs)
        }
    }

    fn parse_atom(&mut self) -> Result<Expr, ParseError> {
        self.parse_atom_with(true)
    }

    /// Bare variant: agent references do NOT consume subsequent `/port=expr`
    /// or positional args. Used for port-assign values so that
    /// `@ADD /lft=@FOO /x=Y` parses `/x=Y` as belonging to ADD, not FOO.
    /// To put args on a nested agent, users must wrap it in parens:
    /// `/lft=(@FOO /x=1)`.
    fn parse_atom_bare(&mut self) -> Result<Expr, ParseError> {
        self.parse_atom_with(false)
    }

    fn parse_atom_with(&mut self, consume_agent_args: bool) -> Result<Expr, ParseError> {
        match self.peek() {
            Some(TokenKind::AgentName(_)) => self.parse_agent_expr(consume_agent_args),
            Some(TokenKind::Bang) => {
                self.parse_prefix_with(consume_agent_args, |e, sp| Expr::Force(Box::new(e), sp))
            }
            Some(TokenKind::Question) => {
                self.parse_prefix_with(consume_agent_args, |e, sp| Expr::Inspect(Box::new(e), sp))
            }
            Some(TokenKind::PortName(_)) => self.advance_name(Expr::PortName),
            Some(TokenKind::SelfPort(_)) => self.advance_name(Expr::SelfPort),
            Some(TokenKind::AnonBond(_)) => self.advance_name(Expr::AnonBond),
            Some(TokenKind::Fragment(_)) => self.advance_name(Expr::Fragment),
            Some(TokenKind::Output) => Ok(Expr::Output(self.advance().span)),
            Some(TokenKind::IntLit(_)) => self.advance_lit(),
            Some(TokenKind::FloatLit(_)) => self.advance_lit(),
            Some(TokenKind::StrLit(_)) => self.advance_lit(),
            Some(TokenKind::CharLit(_)) => self.advance_lit(),
            Some(TokenKind::True) => Ok(Expr::BoolLit(true, self.advance().span)),
            Some(TokenKind::False) => Ok(Expr::BoolLit(false, self.advance().span)),
            Some(TokenKind::Era) => Ok(Expr::BuiltinAtom(BuiltinAtom::Era, self.advance().span)),
            Some(TokenKind::Dup) => Ok(Expr::BuiltinAtom(BuiltinAtom::Dup, self.advance().span)),
            Some(TokenKind::Err) => Ok(Expr::BuiltinAtom(BuiltinAtom::Err, self.advance().span)),
            Some(TokenKind::LBracket) => self.parse_list(),
            Some(TokenKind::LParen) => self.parse_paren(),
            Some(TokenKind::Gen) => Err(self.err(
                "@GEN cannot appear inside rule body / expression — it is a top-level marker only",
            )),
            _ => Err(self.err("expected expression")),
        }
    }

    fn parse_prefix_with(
        &mut self,
        consume_agent_args: bool,
        mk: fn(Expr, Span) -> Expr,
    ) -> Result<Expr, ParseError> {
        let start = self.advance().span;
        let inner = self.parse_atom_with(consume_agent_args)?;
        Ok(mk(inner, self.span_from(start)))
    }

    /// Advance a token that carries a String name (PortName, SelfPort, etc.)
    fn advance_name(&mut self, mk: fn(String, Span) -> Expr) -> Result<Expr, ParseError> {
        let tok = self.advance();
        let sp = tok.span;
        let name = match &tok.kind {
            TokenKind::PortName(n)
            | TokenKind::SelfPort(n)
            | TokenKind::AnonBond(n)
            | TokenKind::Fragment(n) => n.clone(),
            _ => unreachable!(),
        };
        Ok(mk(name, sp))
    }

    /// Advance a literal token.
    fn advance_lit(&mut self) -> Result<Expr, ParseError> {
        let tok = self.advance();
        let sp = tok.span;
        Ok(match &tok.kind {
            TokenKind::IntLit(v) => Expr::IntLit(*v, sp),
            TokenKind::FloatLit(v) => Expr::FloatLit(*v, sp),
            TokenKind::StrLit(s) => Expr::StrLit(s.clone(), sp),
            TokenKind::CharLit(c) => Expr::CharLit(*c, sp),
            _ => unreachable!(),
        })
    }

    //    // Bare atoms (arity consumption): @AGENT, literals, (paren), !, ?
    // Stops at: ->, --, ), ], }, keywords, /name without =

    fn parse_agent_expr(&mut self, consume_args: bool) -> Result<Expr, ParseError> {
        let tok = self.advance();
        let start = tok.span;
        let name = match &tok.kind {
            TokenKind::AgentName(n) => n.clone(),
            _ => unreachable!(),
        };

        let mut args = Vec::new();
        if consume_args {
            loop {
                match self.peek() {
                    // /port = expr  ->  arm assign
                    Some(TokenKind::PortName(_)) if self.is_port_assign() => {
                        args.push(AgentArg::Port(self.parse_port_assign()?));
                    }
                    // (expr)  ->  positional arg via explicit paren.
                    // `$frag` as bare positional is disallowed so a sequence
                    // of frag defs like `$a = @X(...) $b = ...` doesn't get
                    // its next `$b` swallowed by `@X`; wrap in parens if you
                    // really want `@X($b)`.
                    Some(TokenKind::LParen) => {
                        let paren = self.parse_paren()?;
                        args.push(AgentArg::Positional(paren));
                    }
                    _ => break,
                }
            }
        }

        let span = self.span_from(start);
        Ok(Expr::Agent { name, args, span })
    }

    fn is_port_assign(&self) -> bool {
        matches!(self.peek(), Some(TokenKind::PortName(_)))
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|t| t.kind == TokenKind::Equals)
    }

    fn parse_port_assign(&mut self) -> Result<PortAssign, ParseError> {
        let tok = self.advance();
        let start = tok.span;
        let name = match &tok.kind {
            TokenKind::PortName(n) => n.clone(),
            _ => return Err(self.err_at(start, "expected port name")),
        };
        self.expect(&TokenKind::Equals)?;
        // Bare: a nested agent's args must be in parens so they don't get
        // greedily absorbed from the outer agent's argument list.
        let value = self.parse_atom_bare()?;
        Ok(PortAssign {
            name,
            value,
            span: self.span_from(start),
        })
    }

    fn parse_list(&mut self) -> Result<Expr, ParseError> {
        let start = self.advance().span;
        let mut items = Vec::new();
        if self.peek() != Some(&TokenKind::RBracket) {
            items.push(self.parse_connect()?);
            while self.eat(&TokenKind::Comma) {
                items.push(self.parse_connect()?);
            }
        }
        self.expect(&TokenKind::RBracket)?;
        Ok(Expr::List {
            items,
            span: self.span_from(start),
        })
    }

    fn parse_paren(&mut self) -> Result<Expr, ParseError> {
        let start = self.advance().span;
        let mut inner = self.parse_connect()?;
        // Greedy positional inside parens — `(@BIT1 @ZERO)` means @BIT1
        // with @ZERO as a positional arm. Outside parens the stmt-level
        // parser stays conservative so adjacent atoms form separate stmts.
        while self.peek() != Some(&TokenKind::RParen) && !self.at_end() {
            let extra = self.parse_connect()?;
            inner = attach_positional(inner, extra)?;
        }
        self.expect(&TokenKind::RParen)?;
        Ok(Expr::Paren(Box::new(inner), self.span_from(start)))
    }

    fn expect_agent_name(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            Some(TokenKind::AgentName(_)) => {
                let tok = self.advance();
                match &tok.kind {
                    TokenKind::AgentName(n) => Ok(n.clone()),
                    _ => unreachable!(),
                }
            }
            Some(TokenKind::Gen) => Err(self.err(
                "@GEN is a reserved marker — cannot define as agent",
            )),
            _ => Err(self.err("expected agent name (@NAME)")),
        }
    }

    fn expect_mod_name(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            Some(TokenKind::ModName(_)) => {
                let tok = self.advance();
                match &tok.kind {
                    TokenKind::ModName(n) => Ok(n.clone()),
                    _ => unreachable!(),
                }
            }
            _ => Err(self.err("expected module name (uppercase)")),
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            Some(TokenKind::Ident(_)) => {
                let tok = self.advance();
                match &tok.kind {
                    TokenKind::Ident(n) => Ok(n.clone()),
                    _ => unreachable!(),
                }
            }
            _ => Err(self.err("expected identifier")),
        }
    }

    fn expect_str_lit(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            Some(TokenKind::StrLit(_)) => {
                let tok = self.advance();
                match &tok.kind {
                    TokenKind::StrLit(s) => Ok(s.clone()),
                    _ => unreachable!(),
                }
            }
            _ => Err(self.err("expected string literal")),
        }
    }
}

/// Treat `extra` as a positional arg of `outer`. Only meaningful when
/// `outer` is an `@AGENT` expr; otherwise the trailing atom is a parse
/// error.
fn attach_positional(outer: Expr, extra: Expr) -> Result<Expr, ParseError> {
    match outer {
        Expr::Agent {
            name,
            mut args,
            span,
        } => {
            args.push(AgentArg::Positional(extra));
            Ok(Expr::Agent { name, args, span })
        }
        _ => Err(ParseError {
            msg: "positional arg must follow an agent".into(),
            span: extra.span(),
        }),
    }
}

/// Convenience: parse source text into a Program.
pub fn parse(src: &str) -> Result<Program, ParseError> {
    let tokens = gugu_lexer::lex(src).map_err(|e| ParseError {
        msg: e.msg,
        span: e.span,
    })?;
    Parser::new(tokens).parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(src: &str) -> Program {
        parse(src).unwrap_or_else(|e| panic!("parse failed: {e}"))
    }

    #[test]
    fn agent_def_no_ports() {
        let p = parse_ok("agent @ZERO @GEN : @ZERO -> >out");
        if let TopLevel::Agent(a) = &p.items[0] {
            assert_eq!(a.name, "ZERO");
            assert!(!a.is_pub);
            assert!(a.ports.is_empty());
        } else {
            panic!("expected agent");
        }
    }

    #[test]
    fn agent_def_with_ports() {
        let p = parse_ok("agent @ADD /lft /rgt @GEN : @ADD /lft=/a -> >out");
        if let TopLevel::Agent(a) = &p.items[0] {
            assert_eq!(a.name, "ADD");
            assert_eq!(a.ports.len(), 2);
        } else {
            panic!("expected agent");
        }
    }

    #[test]
    fn pub_agent_def() {
        let p = parse_ok("pub agent @ADD /lft /rgt @GEN : @ZERO -> >out");
        if let TopLevel::Agent(a) = &p.items[0] {
            assert!(a.is_pub);
            assert_eq!(a.name, "ADD");
        } else {
            panic!("expected pub agent");
        }
    }

    #[test]
    fn agent_with_type_annotations() {
        let p = parse_ok("agent @ADD /lft:Nat /rgt:Nat @GEN : @ZERO -> >out");
        if let TopLevel::Agent(a) = &p.items[0] {
            if let PortDecl::Arm { name, ty, .. } = &a.ports[0] {
                assert_eq!(name, "lft");
                assert_eq!(ty.as_deref(), Some("Nat"));
            } else {
                panic!("expected arm");
            }
        } else {
            panic!("expected agent");
        }
    }

    #[test]
    fn agent_with_fuse_decl() {
        let p = parse_ok("agent @FOO /a ^pri /b @GEN : @ZERO -> >out");
        if let TopLevel::Agent(a) = &p.items[0] {
            assert_eq!(a.ports.len(), 3);
            assert!(matches!(&a.ports[1], PortDecl::Fuse { name, .. } if name == "pri"));
        } else {
            panic!("expected agent");
        }
    }

    #[test]
    fn use_decl() {
        let p = parse_ok("use Nat @GEN : @ZERO -> >out");
        assert_eq!(p.uses[0].module, "Nat");
        assert!(p.uses[0].alias.is_none());
    }

    #[test]
    fn use_as() {
        let p = parse_ok("use Nat as N @GEN : @ZERO -> >out");
        assert_eq!(p.uses[0].alias.as_deref(), Some("N"));
    }

    #[test]
    fn alias_def() {
        let p = parse_ok("alias @ZERO = z @GEN : @ZERO -> >out");
        if let TopLevel::Alias(a) = &p.items[0] {
            assert_eq!(a.agent, "ZERO");
            assert_eq!(a.target, "z");
        } else {
            panic!("expected alias");
        }
    }

    #[test]
    fn rule_basic() {
        let p = parse_ok("rule @ADD >< @ZERO : ~/result -- ~/lft @GEN : @ZERO -> >out");
        if let TopLevel::Rule(r) = &p.items[0] {
            assert_eq!(r.lhs, "ADD");
            assert_eq!(r.rhs, RuleTarget::Agents(vec!["ZERO".into()]));
            assert_eq!(r.body.len(), 1);
            assert!(!r.is_pub);
            assert!(r.modifier.is_none());
        } else {
            panic!("expected rule");
        }
    }

    #[test]
    fn rule_wildcard() {
        let p = parse_ok("rule @ERR >< _ : ~/result -- @ERA @GEN : @ZERO -> >out");
        if let TopLevel::Rule(r) = &p.items[0] {
            assert_eq!(r.rhs, RuleTarget::Wildcard);
        } else {
            panic!("expected rule");
        }
    }

    #[test]
    fn rule_or_pattern() {
        let p = parse_ok("rule @ERA >< @BIT0 | @BIT1 : ~/hi -- @ERA @GEN : @ZERO -> >out");
        if let TopLevel::Rule(r) = &p.items[0] {
            assert_eq!(
                r.rhs,
                RuleTarget::Agents(vec!["BIT0".into(), "BIT1".into()])
            );
        } else {
            panic!("expected rule");
        }
    }

    #[test]
    fn rule_multi_stmt() {
        let src = "rule @AND >< @FALSE : ~/result -- @FALSE ~/rgt -- @ERA @GEN : @ZERO -> >out";
        let p = parse_ok(src);
        if let TopLevel::Rule(r) = &p.items[0] {
            assert_eq!(r.body.len(), 2);
        } else {
            panic!("expected rule");
        }
    }

    #[test]
    fn lazy_rule() {
        let p = parse_ok("lazy rule @FIB >< @SUCC : ~/result -- ~/val @GEN : @ZERO -> >out");
        if let TopLevel::Rule(r) = &p.items[0] {
            assert_eq!(r.modifier, Some(RuleModifier::Lazy));
        } else {
            panic!("expected rule");
        }
    }

    #[test]
    fn inline_rule() {
        let p = parse_ok("inline rule @ADD >< @ZERO : ~/result -- ~/lft @GEN : @ZERO -> >out");
        if let TopLevel::Rule(r) = &p.items[0] {
            assert_eq!(r.modifier, Some(RuleModifier::Inline));
        } else {
            panic!("expected rule");
        }
    }

    #[test]
    fn pub_lazy_rule() {
        let p = parse_ok("pub lazy rule @FIB >< @SUCC : ~/result -- ~/val @GEN : @ZERO -> >out");
        if let TopLevel::Rule(r) = &p.items[0] {
            assert!(r.is_pub);
            assert_eq!(r.modifier, Some(RuleModifier::Lazy));
        } else {
            panic!("expected rule");
        }
    }

    #[test]
    fn mod_colon_end() {
        let src = "mod Nat : agent @ZERO agent @ADD /lft /rgt end @GEN : @ZERO -> >out";
        let p = parse_ok(src);
        if let TopLevel::Mod(m) = &p.items[0] {
            assert_eq!(m.name, "Nat");
            assert_eq!(m.items.len(), 2);
        } else {
            panic!("expected mod");
        }
    }

    #[test]
    fn mod_brace() {
        let src = "mod Nat { agent @ZERO agent @ADD /lft /rgt } @GEN : @ZERO -> >out";
        let p = parse_ok(src);
        if let TopLevel::Mod(m) = &p.items[0] {
            assert_eq!(m.items.len(), 2);
        } else {
            panic!("expected mod");
        }
    }

    #[test]
    fn frag_def() {
        let p = parse_ok("$one = @BIT1 -> @ZERO @GEN : @ADD($one)($one) -> >out");
        if let TopLevel::Frag(f) = &p.items[0] {
            assert_eq!(f.name, "one");
            assert!(matches!(&f.value, Expr::Connect { .. }));
        } else {
            panic!("expected frag, got {:?}", p.items[0]);
        }
    }

    #[test]
    fn frag_with_ports() {
        let p = parse_ok("$add_one /n = @ADD /lft=/n /rgt=1 @GEN : @ZERO -> >out");
        if let TopLevel::Frag(f) = &p.items[0] {
            assert_eq!(f.name, "add_one");
            assert_eq!(f.ports.len(), 1);
        } else {
            panic!("expected frag");
        }
    }

    #[test]
    fn test_block() {
        let p = parse_ok(r#"test "1+1=2" : @ADD(1)(1) == 2"#);
        if let TopLevel::Test(t) = &p.items[0] {
            assert_eq!(t.label, "1+1=2");
        } else {
            panic!("expected test");
        }
    }

    #[test]
    fn gen_simple() {
        let p = parse_ok("@GEN : @ZERO -> >out");
        assert_eq!(p.gens.len(), 1);
        assert_eq!(p.gens[0].body.len(), 1);
    }

    #[test]
    fn gen_multiple() {
        let p = parse_ok(
            "agent @ZERO\n\
             @GEN : @ZERO -> >out\n\
             @GEN : @ZERO -> >out\n",
        );
        assert_eq!(p.gens.len(), 2);
    }

    #[test]
    fn gen_zero_is_library() {
        let p = parse_ok("agent @ZERO agent @ADD /lft /rgt");
        assert!(p.gens.is_empty());
        assert_eq!(p.items.len(), 2);
    }

    #[test]
    fn gen_as_agent_name_rejected() {
        let err = parse("agent @GEN /x").unwrap_err();
        assert!(err.msg.contains("reserved marker"), "msg = {}", err.msg);
    }

    #[test]
    fn gen_in_rule_body_rejected() {
        let err = parse("rule @ADD >< @ZERO : @GEN : @ZERO -- >out").unwrap_err();
        assert!(
            err.msg.contains("@GEN cannot appear inside rule body"),
            "msg = {}",
            err.msg
        );
    }

    #[test]
    fn gen_in_expr_rejected() {
        let err = parse("@GEN : @GEN : @ZERO -> >out").unwrap_err();
        assert!(
            err.msg.contains("@GEN cannot appear inside expression"),
            "msg = {}",
            err.msg
        );
    }

    #[test]
    fn agent_with_paren_args() {
        // Phase 1: inside parens, use -> for directed bond
        let p = parse_ok("@GEN : @ADD(@BIT1 -> @ZERO)(@BIT1 -> @ZERO) -> >out");
        let main = p.gens[0].clone();
        if let Stmt::Expr(Expr::Connect { lhs, rhs, .. }) = &main.body[0] {
            if let Expr::Agent { name, args, .. } = lhs.as_ref() {
                assert_eq!(name, "ADD");
                assert_eq!(args.len(), 2);
            } else {
                panic!("expected agent");
            }
            assert!(matches!(rhs.as_ref(), Expr::Output(_)));
        } else {
            panic!("expected connect");
        }
    }

    #[test]
    fn agent_with_port_assigns() {
        let p = parse_ok("@GEN : @ADD /lft=/a /rgt=/b -> >out");
        let main = p.gens[0].clone();
        if let Stmt::Expr(Expr::Connect { lhs, .. }) = &main.body[0] {
            if let Expr::Agent { args, .. } = lhs.as_ref() {
                assert_eq!(args.len(), 2);
                assert!(matches!(&args[0], AgentArg::Port(pa) if pa.name == "lft"));
            } else {
                panic!("expected agent");
            }
        } else {
            panic!("expected connect");
        }
    }

    #[test]
    fn port_assign_value_is_bare() {
        // `/lft=@FOO /x=Y` — /x=Y belongs to ADD, not FOO (bare value doesn't consume args)
        let p = parse_ok("@GEN : @ADD /lft=@FOO /x=/y -> >out");
        let block = p.gens[0].clone();
        if let Stmt::Expr(Expr::Connect { lhs, .. }) = &block.body[0] {
            if let Expr::Agent { name, args, .. } = lhs.as_ref() {
                assert_eq!(name, "ADD");
                assert_eq!(args.len(), 2, "both /lft and /x should be on ADD");
                if let AgentArg::Port(pa) = &args[0] {
                    assert_eq!(pa.name, "lft");
                    if let Expr::Agent { name: foo_name, args: foo_args, .. } = &pa.value {
                        assert_eq!(foo_name, "FOO");
                        assert!(foo_args.is_empty(), "bare @FOO should have no args");
                    } else {
                        panic!("expected /lft value to be bare agent");
                    }
                } else {
                    panic!("expected /lft port assign");
                }
                assert!(matches!(&args[1], AgentArg::Port(pa) if pa.name == "x"));
            } else {
                panic!("expected agent");
            }
        } else {
            panic!("expected connect");
        }
    }

    #[test]
    fn port_assign_value_with_parens_consumes_args() {
        // `/lft=(@FOO /x=Y)` — parens allow FOO to have its own /x arg
        let p = parse_ok("@GEN : @ADD /lft=(@FOO /x=/y) -> >out");
        let block = p.gens[0].clone();
        if let Stmt::Expr(Expr::Connect { lhs, .. }) = &block.body[0] {
            if let Expr::Agent { args, .. } = lhs.as_ref() {
                assert_eq!(args.len(), 1, "only /lft on ADD");
                if let AgentArg::Port(pa) = &args[0] {
                    // value is Paren(Agent { name: FOO, args: [/x=/y] })
                    if let Expr::Paren(inner, _) = &pa.value {
                        if let Expr::Agent { name, args: foo_args, .. } = inner.as_ref() {
                            assert_eq!(name, "FOO");
                            assert_eq!(foo_args.len(), 1, "/x=/y on FOO via parens");
                        } else {
                            panic!("expected paren-wrapped agent");
                        }
                    } else {
                        panic!("expected paren");
                    }
                }
            }
        }
    }

    #[test]
    fn arity_via_parens() {
        // Arity consumption requires explicit parens at Phase 1
        // @BIT1(@ZERO) — paren arg consumed
        let p = parse_ok("@GEN : @BIT1(@ZERO) -> >out");
        let main = p.gens[0].clone();
        if let Stmt::Expr(Expr::Connect { lhs, .. }) = &main.body[0] {
            if let Expr::Agent { name, args, .. } = lhs.as_ref() {
                assert_eq!(name, "BIT1");
                assert_eq!(args.len(), 1);
                assert!(matches!(&args[0], AgentArg::Positional(_)));
            } else {
                panic!("expected agent");
            }
        } else {
            panic!("expected connect");
        }
    }

    #[test]
    fn wire_with_connect_rhs() {
        let p = parse_ok("@GEN : /a -- @BIT1 -> @ZERO");
        let main = p.gens[0].clone();
        if let Stmt::Bond { lhs, rhs, .. } = &main.body[0] {
            assert!(matches!(lhs, Expr::PortName(n, _) if n == "a"));
            assert!(matches!(rhs, Expr::Connect { .. }));
        } else {
            panic!("expected bond, got {:?}", main.body[0]);
        }
    }

    #[test]
    fn force_bloom() {
        let p = parse_ok("@GEN : !@PRINT -> >out");
        let main = p.gens[0].clone();
        if let Stmt::Expr(Expr::Connect { lhs, .. }) = &main.body[0] {
            assert!(matches!(lhs.as_ref(), Expr::Force(_, _)));
        } else {
            panic!("expected force connect");
        }
    }

    #[test]
    fn inspect() {
        let p = parse_ok("@GEN : ?@ADD(1)(2) -> >out");
        let main = p.gens[0].clone();
        if let Stmt::Expr(Expr::Connect { lhs, .. }) = &main.body[0] {
            assert!(matches!(lhs.as_ref(), Expr::Inspect(_, _)));
        } else {
            panic!("expected inspect");
        }
    }

    #[test]
    fn builtin_atoms() {
        let p = parse_ok("@GEN : era -- >out");
        let main = p.gens[0].clone();
        if let Stmt::Bond { lhs, .. } = &main.body[0] {
            assert!(matches!(lhs, Expr::BuiltinAtom(BuiltinAtom::Era, _)));
        } else {
            panic!("expected bond");
        }
    }

    #[test]
    fn fragment_in_expr() {
        let p = parse_ok("$one = @BIT1 -> @ZERO @GEN : @ADD($one)($one) -> >out");
        let main = p.gens[0].clone();
        if let Stmt::Expr(Expr::Connect { lhs, .. }) = &main.body[0] {
            if let Expr::Agent { args, .. } = lhs.as_ref() {
                assert_eq!(args.len(), 2);
            } else {
                panic!("expected agent");
            }
        } else {
            panic!("expected connect");
        }
    }

    #[test]
    fn list_literal() {
        let p = parse_ok("@GEN : [1, 2, 3] -> >out");
        let main = p.gens[0].clone();
        if let Stmt::Expr(Expr::Connect { lhs, .. }) = &main.body[0] {
            if let Expr::List { items, .. } = lhs.as_ref() {
                assert_eq!(items.len(), 3);
            } else {
                panic!("expected list");
            }
        } else {
            panic!("expected connect");
        }
    }

    #[test]
    fn error_missing_colon() {
        // @GEN without ':' — expect a Colon, got AgentName
        assert!(parse("@GEN @ZERO -> >out").is_err());
    }

    #[test]
    fn error_empty_rule_body() {
        // rule body empty (the `@GEN :` after would be a sibling top-level form)
        assert!(parse("rule @ADD >< @ZERO :").is_err());
    }

    #[test]
    fn full_nat_add() {
        let src = "\
            agent @ZERO\n\
            agent @BIT1\n\
            agent @ADD /lft /rgt\n\
            rule @ADD >< @ZERO :\n\
              ~/result -- ~/lft\n\
            @GEN :\n\
              /a -- @BIT1 -> @ZERO\n\
              @ADD /lft=/a /rgt=(@BIT1 -> @ZERO) -> >out\n\
        ";
        let p = parse_ok(src);
        assert_eq!(p.items.len(), 4); // 3 agents + 1 rule
        let main = p.gens[0].clone();
        assert_eq!(main.body.len(), 2);
    }

    #[test]
    fn bool_module() {
        let src = "\
            mod Bool :\n\
              agent @TRUE\n\
              agent @FALSE\n\
              agent @AND /rgt\n\
              rule @AND >< @TRUE : ~/result -- ~/rgt\n\
              rule @AND >< @FALSE : ~/result -- @FALSE ~/rgt -- @ERA\n\
            end\n\
        ";
        let p = parse_ok(src);
        if let TopLevel::Mod(m) = &p.items[0] {
            assert_eq!(m.name, "Bool");
            assert_eq!(m.items.len(), 5);
        } else {
            panic!("expected mod");
        }
    }
}
