use err_code::ErrorCode;

use crate::utils::Pos;

#[derive(ErrorCode, Debug, PartialEq)]
pub enum ErrorCode {
    #[error_code(100)]
    UnknownCharacter,

    #[error_code(201)]
    UnexpectedToken,
    #[error_code(202)]
    UnkownLiteral,

    #[error_code(301)]
    AssignmentError,
    #[error_code(302)]
    BreakOutsideLoop,
    #[error_code(303)]
    ContinueOutsideLoop,
    #[error_code(304)]
    ConditionNotBoolean,
    #[error_code(305)]
    UnkownVariable,
    #[error_code(306)]
    ExpectedVar,
    #[error_code(307)]
    IncompatibleTypes,
    #[error_code(308)]
    UnsupportedBinaryOperator,
    #[error_code(309)]
    UnsupportedUnaryOperator,
    #[error_code(310)]
    IncorrectUseOfProcedure,
    #[error_code(311)]
    IncorrectIndexType,
    #[error_code(312)]
    IncorrectBaseType,
    #[error_code(313)]
    UnkownType,
    #[error_code(314)]
    RangeLimitsNotOrdinal,
    #[error_code(315)]
    ExpectedLiteral,
    #[error_code(316)]
    FunctionMayNotReturn,
    #[error_code(317)]
    DuplicateTypeDefinition,
    #[error_code(318)]
    DuplicateVarDefinition,
    #[error_code(319)]
    IncorrectType,
    #[error_code(320)]
    UnkownCallable,
    #[error_code(321)]
    IncorrectNumberOfArguments,
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
        pos: Pos,
        error_code: ErrorCode,
    },
    RuntimeError {
        msg: String,
        pos: Pos,
    },
    IoError(std::io::Error),
    BuiltinFunctionError {
        function_name: &'static str,
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
            Error::SemanticError {
                msg,
                pos,
                error_code,
            } => {
                write!(
                    f,
                    "Semantic Error at row {} col {} ({}: {:?}): {}",
                    pos.row,
                    pos.col,
                    error_code.error_code(),
                    error_code,
                    msg
                )
            }
            Error::RuntimeError { msg, pos } => {
                write!(
                    f,
                    "Runtime Error at row {} col {}: {}",
                    pos.row, pos.col, msg
                )
            }
            Error::BuiltinFunctionError { function_name, msg } => {
                write!(f, "Builtin function {function_name} error: {msg}")
            }
            Error::IoError(e) => e.fmt(f),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value)
    }
}
