use std::{collections::HashMap, hash::Hash};

use crate::utils::{Pos, Span};

#[derive(Debug, Clone, Copy)]
pub struct Token {
    token_type: TokenType,
    span: Span,
    pos: Pos,
}

impl Token {
    pub fn new(token_type: TokenType, start: u32, len: u32, pos: Pos) -> Self {
        Self {
            token_type,
            span: Span::new(start, len),
            pos,
        }
    }

    pub fn token_type(&self) -> &TokenType {
        &self.token_type
    }

    pub fn lexem<'a>(&self, src: &'a str) -> &'a str {
        self.span.lexem(src)
    }

    pub fn pos(&self) -> Pos {
        self.pos
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

impl PartialEq for Token {
    fn eq(&self, other: &Self) -> bool {
        self.token_type == other.token_type
    }
}

impl Eq for Token {}

impl Hash for Token {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.span.hash(state);
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TokenType {
    IntegerConst(i32),
    Int64Const(i64),
    RealConst(f32),
    CharConst(char),
    BooleanConst(bool),
    Program,
    Integer,
    Int64,
    Real,
    Boolean,
    Char,
    String,
    IntegerDiv,
    RealDiv,
    Plus,
    Minus,
    Mul,
    And,
    Or,
    Not,
    Begin,
    End,
    Var,
    Procedure,
    Function,
    If,
    Then,
    Else,
    While,
    Do,
    For,
    To,
    Continue,
    Break,
    Exit,
    Type,
    Const,
    Array,
    Of,
    In,
    Out,
    Comma,
    Equal,
    NotEqual,
    LessThen,
    LessEqual,
    GreaterThen,
    GreaterEqual,
    Assign,
    Semi,
    Colon,
    Dot,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Id,
    StringConst,
    EOF,
}

impl PartialEq for TokenType {
    fn eq(&self, other: &Self) -> bool {
        core::mem::discriminant(self) == core::mem::discriminant(other)
    }
}

impl TokenType {
    pub fn is_compare_operator(&self) -> bool {
        matches!(
            self,
            TokenType::LessEqual
                | TokenType::LessThen
                | TokenType::Equal
                | TokenType::NotEqual
                | TokenType::GreaterEqual
                | TokenType::GreaterThen
        )
    }

    pub fn get_keywords() -> HashMap<&'static str, TokenType> {
        let mut keywords = HashMap::new();
        keywords.insert("PROGRAM", TokenType::Program);
        keywords.insert("INTEGER", TokenType::Integer);
        keywords.insert("INT64", TokenType::Int64);
        keywords.insert("REAL", TokenType::Real);
        keywords.insert("BOOLEAN", TokenType::Boolean);
        keywords.insert("CHAR", TokenType::Char);
        keywords.insert("STRING", TokenType::String);
        keywords.insert("BEGIN", TokenType::Begin);
        keywords.insert("END", TokenType::End);
        keywords.insert("VAR", TokenType::Var);
        keywords.insert("PROCEDURE", TokenType::Procedure);
        keywords.insert("FUNCTION", TokenType::Function);
        keywords.insert("IF", TokenType::If);
        keywords.insert("THEN", TokenType::Then);
        keywords.insert("ELSE", TokenType::Else);
        keywords.insert("WHILE", TokenType::While);
        keywords.insert("DO", TokenType::Do);
        keywords.insert("FOR", TokenType::For);
        keywords.insert("TO", TokenType::To);
        keywords.insert("CONTINUE", TokenType::Continue);
        keywords.insert("BREAK", TokenType::Break);
        keywords.insert("EXIT", TokenType::Exit);
        keywords.insert("TYPE", TokenType::Type);
        keywords.insert("CONST", TokenType::Const);
        keywords.insert("ARRAY", TokenType::Array);
        keywords.insert("OF", TokenType::Of);
        keywords.insert("IN", TokenType::In);
        keywords.insert("OUT", TokenType::Out);
        keywords.insert("DIV", TokenType::RealDiv);
        keywords.insert("AND", TokenType::And);
        keywords.insert("OR", TokenType::Or);
        keywords.insert("NOT", TokenType::Not);
        keywords.insert("TRUE", TokenType::BooleanConst(true));
        keywords.insert("FALSE", TokenType::BooleanConst(false));
        keywords
    }

    pub fn get_char_tokens() -> HashMap<char, TokenType> {
        let mut char_tokens = HashMap::new();
        char_tokens.insert(',', TokenType::Comma);
        char_tokens.insert('=', TokenType::Equal);
        char_tokens.insert(';', TokenType::Semi);
        char_tokens.insert('.', TokenType::Dot);
        char_tokens.insert('(', TokenType::LParen);
        char_tokens.insert(')', TokenType::RParen);
        char_tokens.insert('[', TokenType::LBracket);
        char_tokens.insert(']', TokenType::RBracket);
        char_tokens.insert('+', TokenType::Plus);
        char_tokens.insert('/', TokenType::IntegerDiv);
        char_tokens.insert('*', TokenType::Mul);
        char_tokens
    }
}
