use std::collections::HashMap;

use crate::utils::Pos;

#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: u32,
    pub len: u32,
}

impl Span {
    pub fn lexem<'a>(&self, src: &'a str) -> &'a str {
        let s = self.start as usize;
        let e = s + self.len as usize;
        &src[s..e]
    }
}

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
            span: Span { start, len },
            pos,
        }
    }

    pub fn token_type(&self) -> &TokenType {
        &self.token_type
    }

    pub fn lexem<'a>(&self, src: &'a str) -> &'a str {
        self.span.lexem(src)
    }
}

impl PartialEq for Token {
    fn eq(&self, other: &Self) -> bool {
        self.token_type == other.token_type
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TokenType {
    Program,
    Integer,
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
    IntegerConst(i64),
    RealConst(f64),
    StringConst,
    CharConst(char),
    BooleanConst(bool),
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

    pub fn get_keywords() -> HashMap<String, TokenType> {
        let mut keywords = HashMap::new();
        keywords.insert("PROGRAM".to_string(), TokenType::Program);
        keywords.insert("INTEGER".to_string(), TokenType::Integer);
        keywords.insert("REAL".to_string(), TokenType::Real);
        keywords.insert("BOOLEAN".to_string(), TokenType::Boolean);
        keywords.insert("CHAR".to_string(), TokenType::Char);
        keywords.insert("STRING".to_string(), TokenType::String);
        keywords.insert("DIV".to_string(), TokenType::IntegerDiv);
        keywords.insert("BEGIN".to_string(), TokenType::Begin);
        keywords.insert("END".to_string(), TokenType::End);
        keywords.insert("VAR".to_string(), TokenType::Var);
        keywords.insert("PROCEDURE".to_string(), TokenType::Procedure);
        keywords.insert("FUNCTION".to_string(), TokenType::Function);
        keywords.insert("IF".to_string(), TokenType::If);
        keywords.insert("THEN".to_string(), TokenType::Then);
        keywords.insert("ELSE".to_string(), TokenType::Else);
        keywords.insert("WHILE".to_string(), TokenType::While);
        keywords.insert("DO".to_string(), TokenType::Do);
        keywords.insert("FOR".to_string(), TokenType::For);
        keywords.insert("TO".to_string(), TokenType::To);
        keywords.insert("CONTINUE".to_string(), TokenType::Continue);
        keywords.insert("BREAK".to_string(), TokenType::Break);
        keywords.insert("EXIT".to_string(), TokenType::Exit);
        keywords.insert("TYPE".to_string(), TokenType::Type);
        keywords.insert("CONST".to_string(), TokenType::Const);
        keywords.insert("ARRAY".to_string(), TokenType::Array);
        keywords.insert("OF".to_string(), TokenType::Of);
        keywords.insert("IN".to_string(), TokenType::In);
        keywords.insert("OUT".to_string(), TokenType::Out);
        keywords.insert("DIV".to_string(), TokenType::RealDiv);
        keywords.insert("AND".to_string(), TokenType::And);
        keywords.insert("OR".to_string(), TokenType::Or);
        keywords.insert("NOT".to_string(), TokenType::Not);
        keywords.insert("TRUE".to_string(), TokenType::BooleanConst(true));
        keywords.insert("FALSE".to_string(), TokenType::BooleanConst(false));
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
        char_tokens.insert('-', TokenType::Minus);
        char_tokens.insert('/', TokenType::RealDiv);
        char_tokens.insert('*', TokenType::Mul);
        char_tokens
    }
}
