#[derive(Debug)]
pub enum ErrorCode {}

#[derive(Debug)]
pub enum Error {
    LexerError {
        msg: String,
        error_code: Option<ErrorCode>,
    },
    ParserError {
        msg: String,
        error_code: Option<ErrorCode>,
    },
    SemanticError {
        msg: String,
        error_code: Option<ErrorCode>,
    },
    InterpreterError {
        msg: String,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::LexerError { msg, error_code } => {
                write!(f, "Lexer Error {:?}: {}", error_code, msg)
            }
            Error::ParserError { msg, error_code } => {
                write!(f, "Parser Error {:?}: {}", error_code, msg)
            }
            Error::SemanticError { msg, error_code } => {
                write!(f, "Semantic Error {:?}: {}", error_code, msg)
            }
            Error::InterpreterError { msg } => {
                write!(f, "Interpreter Error: {}", msg)
            }
        }
    }
}
