#![feature(io)]

pub mod ast;
pub mod error;
pub mod lexer;
pub mod tok;

use std::io;
use std::io::Read;

fn main() {
    ast::AtomicTerm::Atom("hello".to_string());
    let stdin = io::BufReader::new(io::stdin());
    let lexer = lexer::Lexer::new(
        stdin.chars().map(|res| res.map_err(|e| match e {
            io::CharsError::NotUtf8 => error::Error::NotUtf8,
            io::CharsError::Other(e) => error::Error::Stream(e)
        })));
    let toks: Vec<tok::Tok> = lexer.map(error::Result::unwrap).collect();
    println!("Tokens read: {:?}", toks)
}
