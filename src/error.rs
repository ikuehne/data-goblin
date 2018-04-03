use std::io;
use std::result;

pub enum Error {
    IOError(io::Error)   
}

pub type Result<T> = result::Result<T, Error>;
