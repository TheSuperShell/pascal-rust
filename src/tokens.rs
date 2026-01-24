use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Token {
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
    Id(String),
    IntegerConst(i64),
    RealConst(f64),
    StringConst(String),
    CharConst(char),
    BooleanConst(bool),
    EOF,
}

impl PartialEq for Token {
    fn eq(&self, other: &Self) -> bool {
        core::mem::discriminant(self) == core::mem::discriminant(other)
    }
}

impl Token {
    pub fn is_compare_operator(&self) -> bool {
        matches!(
            self,
            Token::LessEqual
                | Token::LessThen
                | Token::Equal
                | Token::NotEqual
                | Token::GreaterEqual
                | Token::GreaterThen
        )
    }

    pub fn get_keywords() -> HashMap<String, Token> {
        let mut keywords = HashMap::new();
        keywords.insert("PROGRAM".to_string(), Token::Program);
        keywords.insert("INTEGER".to_string(), Token::Integer);
        keywords.insert("REAL".to_string(), Token::Real);
        keywords.insert("BOOLEAN".to_string(), Token::Boolean);
        keywords.insert("CHAR".to_string(), Token::Char);
        keywords.insert("STRING".to_string(), Token::String);
        keywords.insert("DIV".to_string(), Token::IntegerDiv);
        keywords.insert("BEGIN".to_string(), Token::Begin);
        keywords.insert("END".to_string(), Token::End);
        keywords.insert("VAR".to_string(), Token::Var);
        keywords.insert("PROCEDURE".to_string(), Token::Procedure);
        keywords.insert("FUNCTION".to_string(), Token::Function);
        keywords.insert("IF".to_string(), Token::If);
        keywords.insert("THEN".to_string(), Token::Then);
        keywords.insert("ELSE".to_string(), Token::Else);
        keywords.insert("WHILE".to_string(), Token::While);
        keywords.insert("DO".to_string(), Token::Do);
        keywords.insert("FOR".to_string(), Token::For);
        keywords.insert("TO".to_string(), Token::To);
        keywords.insert("CONTINUE".to_string(), Token::Continue);
        keywords.insert("BREAK".to_string(), Token::Break);
        keywords.insert("EXIT".to_string(), Token::Exit);
        keywords.insert("TYPE".to_string(), Token::Type);
        keywords.insert("CONST".to_string(), Token::Const);
        keywords.insert("ARRAY".to_string(), Token::Array);
        keywords.insert("OF".to_string(), Token::Of);
        keywords.insert("IN".to_string(), Token::In);
        keywords.insert("OUT".to_string(), Token::Out);
        keywords.insert("DIV".to_string(), Token::RealDiv);
        keywords.insert("AND".to_string(), Token::And);
        keywords.insert("OR".to_string(), Token::Or);
        keywords.insert("NOT".to_string(), Token::Not);
        keywords.insert("TRUE".to_string(), Token::BooleanConst(true));
        keywords.insert("FALSE".to_string(), Token::BooleanConst(false));
        keywords
    }

    pub fn get_char_tokens() -> HashMap<char, Token> {
        let mut char_tokens = HashMap::new();
        char_tokens.insert(',', Token::Comma);
        char_tokens.insert('=', Token::Equal);
        char_tokens.insert(';', Token::Semi);
        char_tokens.insert('.', Token::Dot);
        char_tokens.insert('(', Token::LParen);
        char_tokens.insert(')', Token::RParen);
        char_tokens.insert('[', Token::LBracket);
        char_tokens.insert(']', Token::RBracket);
        char_tokens.insert('+', Token::Plus);
        char_tokens.insert('-', Token::Minus);
        char_tokens.insert('/', Token::RealDiv);
        char_tokens.insert('*', Token::Mul);
        char_tokens
    }
}
