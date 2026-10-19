use judas_ast::Span;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    // Keywords
    Module,
    Enum,
    Input,
    Output,
    Register,
    Connect,
    On,
    Rising,
    Falling,
    If,
    Else,
    Match,
    Let,
    Assert,
    Struct,
    Interface,
    Master,
    Slave,

    // Literals
    Ident(String),
    IntLiteral(i128),
    BoolLiteral(bool),
    StringLiteral(String),

    // Delimiters
    LParen,    // (
    RParen,    // )
    LBrace,    // {
    RBrace,    // }
    LBracket,  // [
    RBracket,  // ]
    Comma,     // ,
    Colon,     // :
    Semicolon, // ;
    Dot,       // .
    FatArrow,  // =>

    // Operators
    Eq,       // =
    EqEq,     // ==
    NotEq,    // !=
    Lt,       // <
    LtEq,     // <=
    Gt,       // >
    GtEq,     // >=
    Plus,     // +
    Minus,    // -
    Star,     // *
    Slash,    // /
    Percent,  // %
    AmpAmp,   // &&
    PipePipe, // ||
    Amp,      // &
    Pipe,     // |
    Caret,    // ^
    Shl,      // <<
    Shr,      // >>
    Bang,     // !
    Tilde,    // ~

    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Module => write!(f, "module"),
            TokenKind::Enum => write!(f, "enum"),
            TokenKind::Input => write!(f, "input"),
            TokenKind::Output => write!(f, "output"),
            TokenKind::Register => write!(f, "register"),
            TokenKind::Connect => write!(f, "connect"),
            TokenKind::On => write!(f, "on"),
            TokenKind::Rising => write!(f, "rising"),
            TokenKind::Falling => write!(f, "falling"),
            TokenKind::If => write!(f, "if"),
            TokenKind::Else => write!(f, "else"),
            TokenKind::Match => write!(f, "match"),
            TokenKind::Let => write!(f, "let"),
            TokenKind::Assert => write!(f, "assert"),
            TokenKind::Struct => write!(f, "struct"),
            TokenKind::Interface => write!(f, "interface"),
            TokenKind::Master => write!(f, "master"),
            TokenKind::Slave => write!(f, "slave"),
            TokenKind::Ident(s) => write!(f, "identifier `{}`", s),
            TokenKind::IntLiteral(n) => write!(f, "integer `{}`", n),
            TokenKind::BoolLiteral(b) => write!(f, "boolean `{}`", b),
            TokenKind::StringLiteral(s) => write!(f, "string `\"{}\"`", s),
            TokenKind::LParen => write!(f, "`(`"),
            TokenKind::RParen => write!(f, "`)`"),
            TokenKind::LBrace => write!(f, "`{{`"),
            TokenKind::RBrace => write!(f, "`}}`"),
            TokenKind::LBracket => write!(f, "`[`"),
            TokenKind::RBracket => write!(f, "`]`"),
            TokenKind::Comma => write!(f, "`,`"),
            TokenKind::Colon => write!(f, "`:`"),
            TokenKind::Semicolon => write!(f, "`;`"),
            TokenKind::Dot => write!(f, "`.`"),
            TokenKind::FatArrow => write!(f, "`=>`"),
            TokenKind::Eq => write!(f, "`=`"),
            TokenKind::EqEq => write!(f, "`==`"),
            TokenKind::NotEq => write!(f, "`!=`"),
            TokenKind::Lt => write!(f, "`<`"),
            TokenKind::LtEq => write!(f, "`<=`"),
            TokenKind::Gt => write!(f, "`>`"),
            TokenKind::GtEq => write!(f, "`>=`"),
            TokenKind::Plus => write!(f, "`+`"),
            TokenKind::Minus => write!(f, "`-`"),
            TokenKind::Star => write!(f, "`*`"),
            TokenKind::Slash => write!(f, "`/`"),
            TokenKind::Percent => write!(f, "`%`"),
            TokenKind::AmpAmp => write!(f, "`&&`"),
            TokenKind::PipePipe => write!(f, "`||`"),
            TokenKind::Amp => write!(f, "`&`"),
            TokenKind::Pipe => write!(f, "`|`"),
            TokenKind::Caret => write!(f, "`^`"),
            TokenKind::Shl => write!(f, "`<<`"),
            TokenKind::Shr => write!(f, "`>>`"),
            TokenKind::Bang => write!(f, "`!`"),
            TokenKind::Tilde => write!(f, "`~`"),
            TokenKind::Eof => write!(f, "<EOF>"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}
