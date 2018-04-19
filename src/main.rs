#![feature(io)]
#![feature(option_filter)]

pub mod ast;
pub mod error;
pub mod eval;
pub mod lexer;
pub mod parser;
pub mod tok;
pub mod storage;

use std::io;
use std::io::stdout;
use std::io::Read;
use std::io::Write;
use std::fmt::Display;

fn abort<T: Display>(e: T) -> ! {
    eprintln!("Error: {}", e);
    std::process::exit(1)
}

fn main() {
    let stdin = io::BufReader::new(io::stdin());
    let chars = stdin.chars().map(|r| r.unwrap_or_else(|e| abort(e)));

    let lexer = lexer::Lexer::new(chars);
    let toks = lexer.map(|r| r.unwrap_or_else(|e| abort(e)));

    let parser = parser::Parser::new(toks);
    let lines = parser.map(|l| l.unwrap_or_else(|e| abort(e)));

    let prompt = "data-goblin> ";
    print!("{}", prompt);
    stdout().flush().unwrap();
    let engine = storage::StorageEngine::new();
    let mut evaluator = eval::Evaluator::new(engine);
    for line in lines {
        match line {
            ast::Line::Query(t) => {
                if evaluator.query(t).next().is_some() {
                    println!("True.")
                } else {
                    println!("False.")
                }
            },
            ast::Line::Rule(r) => evaluator.assert(r)
        }
        print!("{}", prompt);
        stdout().flush().unwrap();
    }
}
