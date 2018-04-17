#![feature(io)]

#[macro_use]
pub mod optres;

pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod tok;
pub mod storage;

use std::io;
use std::io::stdout;
use std::io::Read;
use std::io::Write;

fn main() {
    let stdin = io::BufReader::new(io::stdin());
    let lexer = lexer::Lexer::new(
        stdin.chars().map(|res| res.map_err(|e| match e {
            io::CharsError::NotUtf8 => error::Error::NotUtf8,
            io::CharsError::Other(e) => error::Error::Stream(e)
        })));
    let toks = lexer.map(|r|
                         r.unwrap_or_else(|err| {
                             println!("{}.", err);
                             std::process::exit(1)
                         }));

    let parser = parser::Parser::new(toks.map(Ok));
    let lines = parser.map(|l| l.unwrap_or_else(|err| {
        println!("{}.", err);
        std::process::exit(1)
    }));

    let prompt = "data-goblin> ";
    print!("{}", prompt);
    stdout().flush().unwrap();
    for line in lines {
        println!("Read line: {:?}", line);
        print!("{}", prompt);
        stdout().flush().unwrap();
    }
}
