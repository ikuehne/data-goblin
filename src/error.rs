use std::io;
use std::result;

/// Errors data-goblin can encounter.
#[derive(Debug)]
pub enum Error {
    NotUtf8,
    /// An IO operation failed.
    Stream(io::Error),
    Lexer(String)
}

pub type Result<T> = result::Result<T, Error>;
