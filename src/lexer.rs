use crate::{
    error::{Error, ErrorCode},
    tokens::{Token, TokenType},
    utils::Pos,
};

pub struct Lexer<'a> {
    char_tokens: std::collections::HashMap<char, TokenType>,
    keywords: std::collections::HashMap<&'static str, TokenType>,
    source_code: &'a str,
    index: usize,
    pos: Pos,
    current_char: Option<char>,
}

impl<'a> Lexer<'a> {
    pub fn new(source_code: &'a str) -> Self {
        let first_char = source_code.chars().nth(0);
        Lexer {
            source_code,
            index: 0,
            pos: Pos { row: 1, col: 1 },
            current_char: first_char,
            keywords: TokenType::get_keywords(),
            char_tokens: TokenType::get_char_tokens(),
        }
    }

    fn advance(&mut self) {
        if let Some('\n') = self.current_char {
            self.pos = Pos {
                row: self.pos.row + 1,
                col: 1,
            }
        } else {
            self.pos = Pos {
                col: self.pos.col + 1,
                row: self.pos.row,
            }
        }
        self.index += 1;
        self.current_char = self.source_code.chars().nth(self.index);
    }

    fn trim_whitespace(&mut self) {
        while let Some(c) = self.current_char {
            if !c.is_whitespace() {
                break;
            }
            self.advance();
        }
    }

    fn comment(&mut self) {
        while let Some(c) = self.current_char {
            if c == '}' {
                self.advance();
                break;
            }
            self.advance();
        }
    }

    fn single_line_comment(&mut self) {
        while let Some(c) = self.current_char
            && c != '\n'
        {
            self.advance();
        }
        self.advance();
    }

    pub fn peek(&self) -> Option<char> {
        self.source_code.chars().nth(self.index + 1)
    }
    fn previous_index(&self) -> u32 {
        (self.index - 1) as u32
    }

    pub fn next(&mut self) -> Result<Token, Error> {
        self.trim_whitespace();
        loop {
            match (self.current_char, self.peek()) {
                (Some('{'), Some(_)) => self.comment(),
                (Some('\\'), Some('\\')) => self.single_line_comment(),
                _ => {
                    self.trim_whitespace();
                    break;
                }
            }
            self.trim_whitespace();
        }
        match self.current_char {
            None => Ok(Token::new(
                TokenType::Eof,
                self.previous_index(),
                0,
                self.pos.shift(1),
            )),
            Some(c) if self.char_tokens.contains_key(&c) => {
                let token = *self.char_tokens.get(&c).unwrap();
                self.advance();
                Ok(Token::new(token, self.previous_index(), 1, self.pos))
            }
            Some('>') => {
                self.advance();
                match self.current_char {
                    Some('=') => {
                        self.advance();
                        Ok(Token::new(
                            TokenType::GreaterEqual,
                            self.previous_index() - 1,
                            2,
                            self.pos.shift(2),
                        ))
                    }
                    _ => Ok(Token::new(
                        TokenType::GreaterThen,
                        self.previous_index(),
                        1,
                        self.pos.shift(1),
                    )),
                }
            }
            Some('<') => {
                self.advance();
                match self.current_char {
                    Some('=') => {
                        self.advance();
                        Ok(Token::new(
                            TokenType::LessEqual,
                            self.previous_index() - 1,
                            2,
                            self.pos.shift(2),
                        ))
                    }
                    Some('>') => {
                        self.advance();
                        Ok(Token::new(
                            TokenType::NotEqual,
                            self.previous_index() - 1,
                            2,
                            self.pos.shift(2),
                        ))
                    }
                    _ => Ok(Token::new(
                        TokenType::LessThen,
                        self.previous_index(),
                        1,
                        self.pos.shift(1),
                    )),
                }
            }
            Some(':') => {
                self.advance();
                match self.current_char {
                    Some('=') => {
                        self.advance();
                        Ok(Token::new(
                            TokenType::Assign,
                            self.previous_index() - 1,
                            2,
                            self.pos.shift(2),
                        ))
                    }
                    _ => Ok(Token::new(
                        TokenType::Colon,
                        self.previous_index(),
                        1,
                        self.pos.shift(1),
                    )),
                }
            }
            Some('-') => match self.peek() {
                Some(c) if c.is_ascii_digit() => Ok(self.number()),
                _ => {
                    self.advance();
                    Ok(Token::new(
                        TokenType::Minus,
                        self.previous_index(),
                        1,
                        self.pos.shift(1),
                    ))
                }
            },
            Some(c) if c.is_numeric() => Ok(self.number()),
            Some('\'') => Ok(self.string()),
            Some(c) if c.is_alphanumeric() || c == '_' => Ok(self.id()),
            _ => Err(Error::LexerError {
                msg: format!("unexpected character {:?}", self.current_char),
                pos: self.pos,
                error_code: ErrorCode::UnknownCharacter,
            }),
        }
    }

    fn id(&mut self) -> Token {
        let current_index = self.index;
        while let Some(c) = self.current_char
            && (c.is_alphanumeric() || c == '_')
        {
            self.advance();
        }
        let word = &self.source_code[current_index..self.index];
        Token::new(
            *self
                .keywords
                .get(word.to_uppercase().as_str())
                .unwrap_or(&TokenType::Id),
            current_index as u32,
            word.len() as u32,
            self.pos.shift(word.len() as u32),
        )
    }

    fn number(&mut self) -> Token {
        let current_index = self.index;
        if let Some(c) = self.current_char
            && c == '-'
        {
            self.advance();
        }
        while let Some(c) = self.current_char
            && c.is_ascii_digit()
        {
            self.advance();
        }
        if (self.current_char.is_some() && self.current_char.unwrap() != '.')
            || self.peek().is_none()
            || (self.peek().is_some() && !self.peek().unwrap().is_ascii_digit())
        {
            let len = (self.index - current_index) as u32;
            let int_value: i64 = self.source_code[current_index..self.index]
                .parse()
                .expect("integer parsing error");
            let token_type = if int_value > i32::MAX as i64 || int_value < i32::MIN as i64 {
                TokenType::Int64Const(int_value)
            } else {
                TokenType::IntegerConst(int_value as i32)
            };
            return Token::new(token_type, current_index as u32, len, self.pos.shift(len));
        }
        self.advance();
        while let Some(c) = self.current_char
            && c.is_ascii_digit()
        {
            self.advance();
        }
        let len = (self.index - current_index) as u32;
        Token::new(
            TokenType::RealConst(
                self.source_code[current_index..self.index]
                    .parse()
                    .expect("float parsing error, should not happen!"),
            ),
            current_index as u32,
            len,
            self.pos.shift(len),
        )
    }
    fn string(&mut self) -> Token {
        self.advance();
        let current_index = self.index;
        while let (Some(c), Some(nc)) = (self.current_char, self.peek())
            && !(nc == '\'' && c != '\\')
        {
            self.advance();
        }
        self.advance();
        let end_index = self.index;
        self.advance();
        if end_index - current_index == 1 {
            return Token::new(
                TokenType::CharConst(
                    self.source_code
                        .chars()
                        .nth(current_index)
                        .expect("there should be a char"),
                ),
                current_index as u32,
                1,
                self.pos.shift(1),
            );
        }
        let len = (end_index - current_index) as u32;
        Token::new(
            TokenType::StringConst,
            current_index as u32,
            len,
            self.pos.shift(len),
        )
    }

    pub fn current_char(&self) -> Option<char> {
        self.current_char
    }

    pub fn source_code(&self) -> &'a str {
        self.source_code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer() {
        const SOURCE_CODE: &str = "PROGRAM IN FOR 1.3212; -10 - { some comment }{another one}  'c' \\\\ other type of comments FOR i := 10\n 'hello'\n;;. >= : :=";
        let expected = [
            TokenType::Program,
            TokenType::In,
            TokenType::For,
            TokenType::RealConst(1.3212),
            TokenType::Semi,
            TokenType::IntegerConst(-10),
            TokenType::Minus,
            TokenType::CharConst('c'),
            TokenType::StringConst,
            TokenType::Semi,
            TokenType::Semi,
            TokenType::Dot,
            TokenType::GreaterEqual,
            TokenType::Colon,
            TokenType::Assign,
        ];
        let mut lexer = Lexer::new(SOURCE_CODE);
        for e in expected {
            let result = lexer.next();
            assert!(result.is_ok());
            let token_type = result.unwrap();
            assert_eq!(token_type.token_type(), &e);
        }
    }

    #[test]
    fn test_string_token() {
        const SOURCE_CODE: &str = "a := 'Hello, World!'";
        let mut result = Lexer::new(SOURCE_CODE);
        result.next().unwrap();
        result.next().unwrap();
        let str_token = result.next();
        assert!(str_token.is_ok());
        let str_token = str_token.unwrap();
        assert_eq!(
            str_token,
            Token::new(TokenType::StringConst, 6, 13, Pos { row: 1, col: 7 })
        );
        assert_eq!(str_token.lexem(SOURCE_CODE), "Hello, World!")
    }

    #[test]
    fn test_unexpected_token() {
        const SOURCE_CODE: &str = "@";
        let result = Lexer::new(SOURCE_CODE).next();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            Error::LexerError {
                error_code: ErrorCode::UnknownCharacter,
                ..
            }
        ))
    }
}
