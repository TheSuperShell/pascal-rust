use crate::{error::Error, tokens::Token};

pub struct Lexer {
    source_code: String,
    index: usize,
    current_char: Option<char>,
    keywords: std::collections::HashMap<String, Token>,
    char_tokens: std::collections::HashMap<char, Token>,
}

impl Lexer {
    pub fn new(source_code: &str) -> Self {
        let first_char = source_code.chars().nth(0);
        Lexer {
            source_code: source_code.to_string(),
            index: 0,
            current_char: first_char,
            keywords: Token::get_keywords(),
            char_tokens: Token::get_char_tokens(),
        }
    }

    fn advance(&mut self) {
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
                self.trim_whitespace();
                break;
            }
            self.advance();
        }
    }

    pub fn peek(&self) -> Option<char> {
        self.source_code.chars().nth(self.index + 1)
    }

    pub fn next(&mut self) -> Result<Token, Error> {
        self.trim_whitespace();
        while let Some(c) = self.current_char
            && c == '{'
        {
            self.comment();
        }
        match self.current_char {
            None => Ok(Token::EOF),
            Some(c) if self.char_tokens.contains_key(&c) => {
                let token = self.char_tokens.get(&c).unwrap().clone();
                self.advance();
                Ok(token)
            }
            Some(c) if c == '>' => {
                self.advance();
                match self.current_char {
                    Some(c) if c == '=' => {
                        self.advance();
                        return Ok(Token::GreaterEqual);
                    }
                    _ => Ok(Token::GreaterThen),
                }
            }
            Some(c) if c == '<' => {
                self.advance();
                match self.current_char {
                    Some(c) if c == '=' => {
                        self.advance();
                        return Ok(Token::LessEqual);
                    }
                    Some(c) if c == '>' => {
                        self.advance();
                        return Ok(Token::NotEqual);
                    }
                    _ => Ok(Token::LessThen),
                }
            }
            Some(c) if c == ':' => {
                self.advance();
                match self.current_char {
                    Some(c) if c == '=' => {
                        self.advance();
                        return Ok(Token::Assign);
                    }
                    _ => Ok(Token::Colon),
                }
            }
            Some(c) if c.is_digit(10) => Ok(self.number()),
            Some(c) if c == '\'' => Ok(self.string()),
            Some(c) if c.is_alphanumeric() || c == '_' => Ok(self.id()),
            _ => Err(Error::LexerError {
                msg: "unkown char".to_string(),
                error_code: None,
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
        let word = &self.source_code[current_index..self.index].to_uppercase();
        self.keywords
            .get(word)
            .unwrap_or(&Token::Id(word.to_string()))
            .clone()
    }

    fn number(&mut self) -> Token {
        let current_index = self.index;
        while let Some(c) = self.current_char
            && c.is_digit(10)
        {
            self.advance();
        }
        if (self.current_char.is_some() && self.current_char.unwrap() != '.')
            || self.peek().is_none()
            || (self.peek().is_some() && !self.peek().unwrap().is_digit(10))
        {
            return Token::IntegerConst(
                self.source_code[current_index..self.index]
                    .parse()
                    .expect("integer parting error, should not happen!"),
            );
        }
        self.advance();
        while let Some(c) = self.current_char
            && c.is_digit(10)
        {
            self.advance();
        }
        Token::RealConst(
            self.source_code[current_index..self.index]
                .parse()
                .expect("float parsing error, should not happen!"),
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
            return Token::CharConst(
                self.source_code
                    .chars()
                    .nth(self.index)
                    .expect("there should be a char"),
            );
        }
        Token::StringConst(self.source_code[current_index..end_index].to_string())
    }

    pub fn current_char(&self) -> Option<char> {
        self.current_char
    }
}
