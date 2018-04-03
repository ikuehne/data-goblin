mod ast;
mod error;
mod lexer;
mod pos;
mod tok;
mod owning_chars;

use std::io;

fn main() {
    let x = ast::AtomicTerm::Atom("hello".to_string());
    let stdin = io::BufReader::new(io::stdin());
    let lexer = lexer::Lexer::new(stdin);
    println!("Hello, world!");
}
