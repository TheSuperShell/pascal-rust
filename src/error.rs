use err_code::ErrorCode;

use crate::utils::Pos;

#[derive(ErrorCode, Debug)]
pub enum ErrorCode {
    #[error_code(100)]
    UnkownCharacter,

    #[error_code(201)]
    UnexpectedToken,
    #[error_code(202)]
    UnkownLiteral,
}

#[derive(Debug)]
pub enum Error {
    LexerError {
        msg: String,
        pos: Pos,
        error_code: ErrorCode,
    },
    ParserError {
        msg: String,
        pos: Pos,
        error_code: ErrorCode,
    },
    SemanticError {
        msg: String,
        error_code: Option<ErrorCode>,
    },
    RuntimeError {
        msg: String,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::LexerError {
                msg,
                pos,
                error_code,
            } => {
                write!(
                    f,
                    "Lexer Error at row {} col {} ({}: {:?}): {}",
                    pos.row,
                    pos.col,
                    error_code.error_code(),
                    error_code,
                    msg
                )
            }
            Error::ParserError {
                msg,
                pos,
                error_code,
            } => {
                write!(
                    f,
                    "Parser Error at row {} col {} ({}: {:?}): {}",
                    pos.row,
                    pos.col,
                    error_code.error_code(),
                    error_code,
                    msg
                )
            }
            Error::SemanticError { msg, error_code } => {
                write!(f, "Semantic Error {:?}: {}", error_code, msg)
            }
            Error::RuntimeError { msg } => {
                write!(f, "Runtime Error: {}", msg)
            }
        }
    }
}
