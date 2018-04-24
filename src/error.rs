use std;
use std::error;
use std::fmt;
use std::result;

/// Errors data-goblin can encounter.
#[derive(Debug)]
pub enum Error {
    /// The lexer failed for the given reason.
    Lexer(String),
    /// The parser failed for the given reason.
    Parser(String),
    /// An operation could not be performed because the named relation is not
    /// extensional.
    NotExtensional(String),
    /// A query or assertion was malformed for the given reason.
    MalformedLine(String),
    StorageError(Box<std::error::Error>),
    BadFilename(std::ffi::OsString)
}

/// Custom result type for data-goblin.
pub type Result<T> = result::Result<T, Error>;

impl error::Error for Error {
    fn description(&self) -> &str {
        match self {
            Error::Lexer(_) => "lexer error",
            Error::Parser(_) => "parser error",
            Error::NotExtensional(_) | Error::MalformedLine(_) =>
                "evaluation error",
            Error::StorageError(_) => "storage error",
            Error::BadFilename(_) => "bad filename for table file"
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            Error::Lexer(_) => None,
            Error::Parser(_) => None,
            Error::NotExtensional(_) => None,
            Error::MalformedLine(_) => None,
            Error::StorageError(e) => e.cause(),
            Error::BadFilename(_) => None
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Lexer(s) => write!(f, "lexer error: {}", s),
            Error::Parser(s) => write!(f, "parser error: {}", s),
            Error::NotExtensional(s) =>
                write!(f, "not an extensional relation: {}", s),
            Error::MalformedLine(s) =>
                write!(f, "malformed query/assertion: {}", s),
            Error::StorageError(e) => write!(f, "storage error: {}", e),
            Error::BadFilename(s) =>
                write!(f, "bad filename for table file: {:?}", s)
        }
    }
}
