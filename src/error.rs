use std::io;
use std::result;

pub enum Error {
    EncodingError(io::CharsError),
    IOError(io::Error),
    LexerError(String)
}

pub type Result<T> = result::Result<T, Error>;
