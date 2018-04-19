use std::error;
use std::fmt;
use std::result;

/// Errors data-goblin can encounter.
#[derive(Debug)]
pub enum Error {
    /// The lexer failed for the given reason.
    Lexer(String),
    /// The parser failed for the given reason.
    Parser(String)
}

/// Custom result type for data-goblin.
pub type Result<T> = result::Result<T, Error>;

impl error::Error for Error {
    fn description(&self) -> &str {
        match self {
            Error::Lexer(_) => "lexer error",
            Error::Parser(_) => "parser error"
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            Error::Lexer(_) => None,
            Error::Parser(_) => None
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Lexer(s) => write!(f, "lexer error: {}", s),
            Error::Parser(s) => write!(f, "parser error: {}", s)
        }
    }
}
