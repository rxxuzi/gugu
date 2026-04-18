/// Byte offset range in source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start: start as u32,
            end: end as u32,
        }
    }

    pub fn len(&self) -> usize {
        (self.end - self.start) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    AgentName(String), // @ADD, @ERA
    PortName(String),  // /lft, /a
    SelfPort(String),  // ~/lft
    AnonBond(String),  // >>new
    Output,            // >out
    Fragment(String),  // $one, $add_one

    ModName(String), // Nat, Str (uppercase start)
    Ident(String),   // foo, bar_2 (lowercase start)

    IntLit(i64),
    FloatLit(f64),
    StrLit(String),
    CharLit(char),

    Pack,
    Mod,
    Use,
    Agent,
    Rule,
    Gen, // @GEN — reserved marker, single token

    Pub,
    As,
    Alias,
    End,

    Lazy,
    Inline,
    Type,

    Test,
    Extern,

    Era,
    Dup,
    True,
    False,
    Err,

    Fire,  // ><
    Arrow, // ->
    Bond,  // --

    Bang,      // !
    Question,  // ?
    Pipe,      // |
    Caret,     // ^
    Percent,   // %
    Ampersand, // &
    EqEq,      // ==

    LParen,   // (
    RParen,   // )
    LBrace,   // {
    RBrace,   // }
    LBracket, // [
    RBracket, // ]
    Colon,    // :
    Equals,   // =
    Comma,    // ,
    Wildcard, // _
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}
