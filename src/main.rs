#![feature(custom_attribute)]
#![feature(io)]
#![feature(option_filter)]
#![feature(type_ascription)]

pub mod ast;
pub mod driver;
pub mod error;
pub mod eval;
pub mod lexer;
pub mod parser;
pub mod tok;
pub mod storage;

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

const DEFAULT_DATA_DIR: &'static str = "./data/";

fn main() {
    driver::Driver::from_stdin(DEFAULT_DATA_DIR.to_string()).run()
}
