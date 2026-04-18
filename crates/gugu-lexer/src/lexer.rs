use crate::token::{Span, Token, TokenKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub msg: String,
    pub span: Span,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "lex error at {}..{}: {}",
            self.span.start, self.span.end, self.msg
        )
    }
}

impl std::error::Error for LexError {}

pub struct Lexer<'src> {
    src: &'src [u8],
    pos: usize,
}

impl<'src> Lexer<'src> {
    pub fn new(src: &'src str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: 0,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.src.len() {
                break;
            }
            tokens.push(self.next_token()?);
        }
        Ok(tokens)
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.src.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> u8 {
        let b = self.src[self.pos];
        self.pos += 1;
        b
    }

    fn skip_whitespace_and_comments(&mut self) {
        while self.pos < self.src.len() {
            match self.src[self.pos] {
                b' ' | b'\t' | b'\r' | b'\n' => self.pos += 1,
                b'#' => {
                    while self.pos < self.src.len() && self.src[self.pos] != b'\n' {
                        self.pos += 1;
                    }
                }
                _ => break,
            }
        }
    }

    fn err(&self, start: usize, msg: impl Into<String>) -> LexError {
        LexError {
            msg: msg.into(),
            span: Span::new(start, self.pos.max(start + 1)),
        }
    }

    fn tok(&self, kind: TokenKind, start: usize) -> Token {
        Token::new(kind, Span::new(start, self.pos))
    }

    fn next_token(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        let b = self.advance();

        match b {
            b'@' => self.lex_agent_name(start),
            b'~' => self.lex_self_port(start),
            b'/' => self.lex_port_name(start),
            b'$' => self.lex_fragment(start),
            b'>' => self.lex_gt(start),
            b'-' => self.lex_dash(start),
            b'=' => self.lex_eq(start),
            b'"' => self.lex_string(start),
            b'\'' => self.lex_char(start),
            b'(' => Ok(self.tok(TokenKind::LParen, start)),
            b')' => Ok(self.tok(TokenKind::RParen, start)),
            b'{' => Ok(self.tok(TokenKind::LBrace, start)),
            b'}' => Ok(self.tok(TokenKind::RBrace, start)),
            b'[' => Ok(self.tok(TokenKind::LBracket, start)),
            b']' => Ok(self.tok(TokenKind::RBracket, start)),
            b':' => Ok(self.tok(TokenKind::Colon, start)),
            b',' => Ok(self.tok(TokenKind::Comma, start)),
            b'!' => Ok(self.tok(TokenKind::Bang, start)),
            b'?' => Ok(self.tok(TokenKind::Question, start)),
            b'|' => Ok(self.tok(TokenKind::Pipe, start)),
            b'^' => Ok(self.tok(TokenKind::Caret, start)),
            b'%' => Ok(self.tok(TokenKind::Percent, start)),
            b'&' => Ok(self.tok(TokenKind::Ampersand, start)),
            b'_' if !self.is_ident_continue() => Ok(self.tok(TokenKind::Wildcard, start)),
            b if b.is_ascii_digit() => self.lex_number(start),
            b if b.is_ascii_uppercase() => self.lex_upper_word(start),
            b if b.is_ascii_lowercase() || b == b'_' => self.lex_lower_word(start),
            _ => Err(self.err(start, format!("unexpected character '{}'", b as char))),
        }
    }

    fn is_ident_continue(&self) -> bool {
        self.is_ident_at(0)
    }

    fn is_ident_at(&self, offset: usize) -> bool {
        matches!(self.peek_at(offset), Some(b) if b.is_ascii_alphanumeric() || b == b'_')
    }

    // @NAME — uppercase start
    fn lex_agent_name(&mut self, start: usize) -> Result<Token, LexError> {
        match self.peek() {
            Some(b) if b.is_ascii_uppercase() => {}
            _ => return Err(self.err(start, "expected uppercase letter after '@'")),
        }
        let name_start = self.pos;
        while let Some(b) = self.peek() {
            if b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let name = self.slice_str(name_start, self.pos);
        let kind = if name == "GEN" {
            TokenKind::Gen
        } else {
            TokenKind::AgentName(name)
        };
        Ok(Token::new(kind, Span::new(start, self.pos)))
    }

    // ~/name
    fn lex_self_port(&mut self, start: usize) -> Result<Token, LexError> {
        if self.peek() != Some(b'/') {
            return Err(self.err(start, "expected '/' after '~'"));
        }
        self.pos += 1;
        let name = self.read_lower_ident()?;
        if name.is_empty() {
            return Err(self.err(start, "expected port name after '~/'"));
        }
        Ok(Token::new(
            TokenKind::SelfPort(name),
            Span::new(start, self.pos),
        ))
    }

    // /name
    fn lex_port_name(&mut self, start: usize) -> Result<Token, LexError> {
        let name = self.read_lower_ident()?;
        if name.is_empty() {
            return Err(self.err(start, "expected name after '/'"));
        }
        Ok(Token::new(
            TokenKind::PortName(name),
            Span::new(start, self.pos),
        ))
    }

    // $name
    fn lex_fragment(&mut self, start: usize) -> Result<Token, LexError> {
        let name = self.read_lower_ident()?;
        if name.is_empty() {
            return Err(self.err(start, "expected name after '$'"));
        }
        Ok(Token::new(
            TokenKind::Fragment(name),
            Span::new(start, self.pos),
        ))
    }

    // > can start: >out, ><, >>name
    fn lex_gt(&mut self, start: usize) -> Result<Token, LexError> {
        match self.peek() {
            Some(b'<') => {
                self.pos += 1;
                Ok(self.tok(TokenKind::Fire, start))
            }
            Some(b'>') => {
                self.pos += 1;
                let name = self.read_lower_ident()?;
                if name.is_empty() {
                    return Err(self.err(start, "expected name after '>>'"));
                }
                Ok(Token::new(
                    TokenKind::AnonBond(name),
                    Span::new(start, self.pos),
                ))
            }
            Some(b'o')
                if self.peek_at(1) == Some(b'u')
                    && self.peek_at(2) == Some(b't')
                    && !self.is_ident_at(3) =>
            {
                self.pos += 3;
                Ok(self.tok(TokenKind::Output, start))
            }
            _ => Err(self.err(start, "unexpected '>' (expected ><, >>, or >out)")),
        }
    }

    // - can start: ->, --, or negative number
    fn lex_dash(&mut self, start: usize) -> Result<Token, LexError> {
        match self.peek() {
            Some(b'>') => {
                self.pos += 1;
                Ok(self.tok(TokenKind::Arrow, start))
            }
            Some(b'-') => {
                self.pos += 1;
                Ok(self.tok(TokenKind::Bond, start))
            }
            Some(b) if b.is_ascii_digit() => self.lex_number(start),
            _ => Err(self.err(
                start,
                "unexpected '-' (expected ->, --, or negative number)",
            )),
        }
    }

    // = or ==
    fn lex_eq(&mut self, start: usize) -> Result<Token, LexError> {
        if self.peek() == Some(b'=') {
            self.pos += 1;
            Ok(self.tok(TokenKind::EqEq, start))
        } else {
            Ok(self.tok(TokenKind::Equals, start))
        }
    }

    fn lex_number(&mut self, start: usize) -> Result<Token, LexError> {
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.peek() == Some(b'.') && matches!(self.peek_at(1), Some(b) if b.is_ascii_digit()) {
            self.pos += 1;
            while let Some(b) = self.peek() {
                if b.is_ascii_digit() {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            let text = self.slice_str(start, self.pos);
            let val: f64 = text.parse().map_err(|_| self.err(start, "invalid float"))?;
            return Ok(Token::new(
                TokenKind::FloatLit(val),
                Span::new(start, self.pos),
            ));
        }
        let text = self.slice_str(start, self.pos);
        let val: i64 = text
            .parse()
            .map_err(|_| self.err(start, "invalid integer"))?;
        Ok(Token::new(
            TokenKind::IntLit(val),
            Span::new(start, self.pos),
        ))
    }

    /// Decode one Unicode scalar starting at `self.pos` and advance past it.
    /// The source came from a `&str` so the remaining slice is guaranteed to
    /// be valid UTF-8.
    fn advance_scalar(&mut self) -> Option<char> {
        // SAFETY: src originates from &str, which enforces UTF-8.
        let s = std::str::from_utf8(&self.src[self.pos..]).ok()?;
        let c = s.chars().next()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn lex_string(&mut self, start: usize) -> Result<Token, LexError> {
        let mut buf = String::new();
        loop {
            match self.peek() {
                None => return Err(self.err(start, "unterminated string")),
                Some(b'"') => {
                    self.pos += 1;
                    break;
                }
                Some(b'\\') => {
                    self.pos += 1;
                    match self.peek() {
                        Some(b'n') => {
                            buf.push('\n');
                            self.pos += 1;
                        }
                        Some(b't') => {
                            buf.push('\t');
                            self.pos += 1;
                        }
                        Some(b'\\') => {
                            buf.push('\\');
                            self.pos += 1;
                        }
                        Some(b'"') => {
                            buf.push('"');
                            self.pos += 1;
                        }
                        _ => return Err(self.err(self.pos, "invalid escape")),
                    }
                }
                Some(_) => {
                    let c = self
                        .advance_scalar()
                        .ok_or_else(|| self.err(start, "bad UTF-8 in string"))?;
                    buf.push(c);
                }
            }
        }
        Ok(Token::new(
            TokenKind::StrLit(buf),
            Span::new(start, self.pos),
        ))
    }

    fn lex_char(&mut self, start: usize) -> Result<Token, LexError> {
        let ch = match self.peek() {
            Some(b'\\') => {
                self.pos += 1;
                match self.peek() {
                    Some(b'n') => {
                        self.pos += 1;
                        '\n'
                    }
                    Some(b't') => {
                        self.pos += 1;
                        '\t'
                    }
                    Some(b'\\') => {
                        self.pos += 1;
                        '\\'
                    }
                    Some(b'\'') => {
                        self.pos += 1;
                        '\''
                    }
                    _ => return Err(self.err(self.pos, "invalid char escape")),
                }
            }
            Some(_) => self
                .advance_scalar()
                .ok_or_else(|| self.err(start, "bad UTF-8 in char literal"))?,
            None => return Err(self.err(start, "unterminated char literal")),
        };
        if self.peek() != Some(b'\'') {
            return Err(self.err(start, "unterminated char literal"));
        }
        self.pos += 1;
        Ok(Token::new(
            TokenKind::CharLit(ch),
            Span::new(start, self.pos),
        ))
    }

    // Uppercase-start word -> MOD_NAME
    fn lex_upper_word(&mut self, start: usize) -> Result<Token, LexError> {
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let word = self.slice_str(start, self.pos);
        Ok(Token::new(
            TokenKind::ModName(word),
            Span::new(start, self.pos),
        ))
    }

    // Lowercase-start word -> keyword or IDENT
    fn lex_lower_word(&mut self, start: usize) -> Result<Token, LexError> {
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let word = self.slice_str(start, self.pos);
        let kind = match word.as_str() {
            // Structure
            "pack" => TokenKind::Pack,
            "mod" => TokenKind::Mod,
            "use" => TokenKind::Use,
            "agent" => TokenKind::Agent,
            "rule" => TokenKind::Rule,
            // Modifier
            "pub" => TokenKind::Pub,
            "as" => TokenKind::As,
            "alias" => TokenKind::Alias,
            "end" => TokenKind::End,
            // Evaluation hints
            "lazy" => TokenKind::Lazy,
            "inline" => TokenKind::Inline,
            "type" => TokenKind::Type,
            // Special
            "test" => TokenKind::Test,
            "extern" => TokenKind::Extern,
            // Built-in atoms
            "era" => TokenKind::Era,
            "dup" => TokenKind::Dup,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "err" => TokenKind::Err,
            _ => TokenKind::Ident(word),
        };
        Ok(Token::new(kind, Span::new(start, self.pos)))
    }

    fn read_lower_ident(&mut self) -> Result<String, LexError> {
        let s = self.pos;
        if !matches!(self.peek(), Some(b) if b.is_ascii_lowercase()) {
            return Ok(String::new());
        }
        self.pos += 1;
        while let Some(b) = self.peek() {
            if b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        Ok(self.slice_str(s, self.pos))
    }

    fn slice_str(&self, start: usize, end: usize) -> String {
        std::str::from_utf8(&self.src[start..end]).unwrap().into()
    }
}

/// Convenience function: tokenize a source string.
pub fn lex(src: &str) -> Result<Vec<Token>, LexError> {
    Lexer::new(src).tokenize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::TokenKind::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        lex(src).unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn agent_names() {
        assert_eq!(
            kinds("@ADD @ERA @BIT1"),
            vec![
                AgentName("ADD".into()),
                AgentName("ERA".into()),
                AgentName("BIT1".into()),
            ]
        );
    }

    #[test]
    fn port_and_self_port() {
        assert_eq!(
            kinds("/lft ~/result"),
            vec![PortName("lft".into()), SelfPort("result".into())]
        );
    }

    #[test]
    fn anon_wire_and_output() {
        assert_eq!(kinds(">>new >out"), vec![AnonBond("new".into()), Output]);
    }

    #[test]
    fn fragment() {
        assert_eq!(
            kinds("$one $add_one"),
            vec![Fragment("one".into()), Fragment("add_one".into())]
        );
    }

    #[test]
    fn connection_symbols() {
        assert_eq!(kinds(">< -> --"), vec![Fire, Arrow, Bond]);
    }

    #[test]
    fn control_symbols() {
        assert_eq!(
            kinds("! ? | ^ % &"),
            vec![Bang, Question, Pipe, Caret, Percent, Ampersand]
        );
    }

    #[test]
    fn eq_and_eqeq() {
        assert_eq!(kinds("= =="), vec![Equals, EqEq]);
    }

    #[test]
    fn brackets_and_delimiters() {
        assert_eq!(
            kinds("( ) { } [ ] : , _"),
            vec![
                LParen, RParen, LBrace, RBrace, LBracket, RBracket, Colon, Comma, Wildcard,
            ]
        );
    }

    #[test]
    fn structure_keywords() {
        assert_eq!(
            kinds("pack mod use agent rule"),
            vec![Pack, Mod, Use, Agent, Rule]
        );
    }

    #[test]
    fn gen_is_single_token() {
        assert_eq!(kinds("@GEN"), vec![Gen]);
        assert_eq!(
            kinds("@GEN @FOO"),
            vec![Gen, AgentName("FOO".into())]
        );
    }

    #[test]
    fn modifier_keywords() {
        assert_eq!(kinds("pub as alias end"), vec![Pub, As, Alias, End]);
    }

    #[test]
    fn eval_keywords() {
        assert_eq!(kinds("lazy inline type"), vec![Lazy, Inline, Type]);
    }

    #[test]
    fn special_keywords() {
        assert_eq!(kinds("test extern"), vec![Test, Extern]);
    }

    #[test]
    fn builtin_atom_keywords() {
        assert_eq!(
            kinds("era dup true false err"),
            vec![Era, Dup, True, False, Err]
        );
    }

    #[test]
    fn ident_and_mod_name() {
        assert_eq!(
            kinds("foo Nat bar_2 Bool3"),
            vec![
                Ident("foo".into()),
                ModName("Nat".into()),
                Ident("bar_2".into()),
                ModName("Bool3".into()),
            ]
        );
    }

    #[test]
    fn numbers() {
        assert_eq!(
            kinds("42 -7 3.14"),
            vec![IntLit(42), IntLit(-7), FloatLit(3.14)]
        );
    }

    #[test]
    fn string_and_char() {
        assert_eq!(
            kinds(r#""hello" 'x'"#),
            vec![StrLit("hello".into()), CharLit('x')]
        );
    }

    #[test]
    fn string_escapes() {
        assert_eq!(
            kinds(r#""\n\t\\\"" '\n'"#),
            vec![StrLit("\n\t\\\"".into()), CharLit('\n')]
        );
    }

    #[test]
    fn string_and_char_utf8() {
        assert_eq!(
            kinds(r#""こんにちは" 'あ'"#),
            vec![StrLit("こんにちは".into()), CharLit('あ')]
        );
    }

    #[test]
    fn comments_skipped() {
        assert_eq!(
            kinds("@ADD # comment\n@ERA"),
            vec![AgentName("ADD".into()), AgentName("ERA".into())]
        );
    }

    #[test]
    fn full_rule() {
        let src = "rule @ADD >< @ZERO :\n  ~/result -- ~/lft";
        assert_eq!(
            kinds(src),
            vec![
                Rule,
                AgentName("ADD".into()),
                Fire,
                AgentName("ZERO".into()),
                Colon,
                SelfPort("result".into()),
                Bond,
                SelfPort("lft".into()),
            ]
        );
    }

    #[test]
    fn gen_block() {
        let src = "@GEN : @ADD /lft=/a -> >out";
        assert_eq!(
            kinds(src),
            vec![
                Gen,
                Colon,
                AgentName("ADD".into()),
                PortName("lft".into()),
                Equals,
                PortName("a".into()),
                Arrow,
                Output,
            ]
        );
    }

    #[test]
    fn main_is_plain_ident_now() {
        assert_eq!(kinds("main"), vec![Ident("main".into())]);
    }

    #[test]
    fn pub_agent() {
        assert_eq!(
            kinds("pub agent @ADD /lft /rgt"),
            vec![
                Pub,
                Agent,
                AgentName("ADD".into()),
                PortName("lft".into()),
                PortName("rgt".into()),
            ]
        );
    }

    #[test]
    fn lazy_rule() {
        assert_eq!(
            kinds("lazy rule @FIB >< @SUCC :"),
            vec![
                Lazy,
                Rule,
                AgentName("FIB".into()),
                Fire,
                AgentName("SUCC".into()),
                Colon
            ]
        );
    }

    #[test]
    fn test_block() {
        let src = r#"test "1+1=2" : @ADD(1)(1) == 2"#;
        assert_eq!(
            kinds(src),
            vec![
                Test,
                StrLit("1+1=2".into()),
                Colon,
                AgentName("ADD".into()),
                LParen,
                IntLit(1),
                RParen,
                LParen,
                IntLit(1),
                RParen,
                EqEq,
                IntLit(2),
            ]
        );
    }

    #[test]
    fn rule_or_pattern() {
        assert_eq!(
            kinds("rule @ERA >< @BIT0 | @BIT1 :"),
            vec![
                Rule,
                AgentName("ERA".into()),
                Fire,
                AgentName("BIT0".into()),
                Pipe,
                AgentName("BIT1".into()),
                Colon,
            ]
        );
    }

    #[test]
    fn fragment_def() {
        assert_eq!(
            kinds("$one = @BIT1 -> @ZERO"),
            vec![
                Fragment("one".into()),
                Equals,
                AgentName("BIT1".into()),
                Arrow,
                AgentName("ZERO".into()),
            ]
        );
    }

    #[test]
    fn force_and_inspect() {
        assert_eq!(
            kinds("!@PRINT ?@ADD"),
            vec![
                Bang,
                AgentName("PRINT".into()),
                Question,
                AgentName("ADD".into()),
            ]
        );
    }

    #[test]
    fn self_mod_ref() {
        // % is its own token; module path resolution is a parser concern
        assert_eq!(
            kinds("% @HELPER"),
            vec![Percent, AgentName("HELPER".into())]
        );
    }

    #[test]
    fn wildcard_vs_ident() {
        assert_eq!(kinds("_ _foo"), vec![Wildcard, Ident("_foo".into())]);
    }

    #[test]
    fn empty_input() {
        assert_eq!(lex("").unwrap(), vec![]);
        assert_eq!(lex("  # only comment\n  ").unwrap(), vec![]);
    }

    #[test]
    fn unterminated_string() {
        assert!(lex(r#""oops"#).is_err());
    }

    #[test]
    fn bad_agent_name() {
        assert!(lex("@foo").is_err());
    }

    #[test]
    fn spans_are_correct() {
        let tokens = lex("@ADD /lft").unwrap();
        assert_eq!(tokens[0].span, Span::new(0, 4));
        assert_eq!(tokens[1].span, Span::new(5, 9));
    }
}
