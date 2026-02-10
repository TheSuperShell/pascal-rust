use std::iter::once;

use err_code::ErrorCode;
use itertools::Itertools;

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
    UnsupportedBinaryOperation,
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
    Errors(Vec<Box<Error>>),
}

pub struct Errors(Vec<Error>);

impl From<Vec<Error>> for Errors {
    fn from(value: Vec<Error>) -> Self {
        Errors(value)
    }
}

#[cfg(test)]
impl From<Error> for Errors {
    fn from(value: Error) -> Self {
        match value {
            Error::Errors(errs) => Self(errs.into_iter().map(|v| *v).collect()),
            _ => Self(vec![value]),
        }
    }
}

impl Errors {
    #[cfg(test)]
    pub fn iter(&self) -> impl Iterator<Item = &Error> {
        self.0.iter()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn add(self, other: Result<(), Error>) -> Errors {
        match other {
            Err(Error::Errors(errs)) => Errors(
                self.0
                    .into_iter()
                    .chain(errs.into_iter().map(|v| *v))
                    .collect(),
            ),
            Err(e) => Errors(self.0.into_iter().chain(once(e)).collect()),
            Ok(()) => self,
        }
    }

    pub fn result<R>(self, res: R) -> Result<R, Error> {
        match self.0.len() {
            0 => Ok(res),
            1 => Err(self.0.into_iter().last().unwrap()),
            _ => Err(Error::Errors(self.0.into_iter().map(Box::new).collect())),
        }
    }
}

impl Into<Result<(), Error>> for Errors {
    fn into(self) -> Result<(), Error> {
        match self.0.len() {
            0 => Ok(()),
            1 => Err(self.0.into_iter().last().unwrap()),
            _ => Err(Error::Errors(self.0.into_iter().map(Box::new).collect())),
        }
    }
}

impl Error {
    fn to_string(&self) -> String {
        match self {
            Error::LexerError {
                msg,
                pos,
                error_code,
            } => {
                format!(
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
                format!(
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
                format!(
                    "Semantic Error at row {} col {} ({}: {:?}): {}",
                    pos.row,
                    pos.col,
                    error_code.error_code(),
                    error_code,
                    msg
                )
            }
            Error::RuntimeError { msg, pos } => {
                format!("Runtime Error at row {} col {}: {}", pos.row, pos.col, msg)
            }
            Error::BuiltinFunctionError { function_name, msg } => {
                format!("Builtin function {function_name} error: {msg}")
            }
            Error::IoError(e) => e.to_string(),
            Error::Errors(errs) => {
                format!(
                    "Errors:\n{}",
                    errs.iter().map(|v| v.to_string()).join(",\n")
                )
            }
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value)
    }
}
