#![feature(custom_attribute)]
#![feature(io)]
#![feature(option_filter)]
#![feature(trait_alias)]
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

// Integration tests go here.
#[cfg(test)]
mod tests {
    use ast;
    use storage::*;
    use eval;
    use lexer::Lexer;
    use parser::Parser;

    use std::collections::HashSet;

    #[test]
    fn simple_sentences() {
        let engine = StorageEngine::new("test_data/grammar".to_string())
            .unwrap();
        let query = "simple_sentence(SUBJECT, VERB, OBJECT)?";
        let lexer = Lexer::new(query.chars()).map(Result::unwrap);
        let parser = Parser::new(lexer).map(Result::unwrap);
        let sentences: HashSet<String> = parser.map(|line| {
            if let ast::Line::Query(t) = line {
                eval::query(&engine, t).unwrap()
            } else {
                panic!("parsed query as assertion");
            }
        }).next().unwrap().map(|frame| {
            let subject = frame.get("SUBJECT").unwrap();
            let verb = frame.get("VERB").unwrap();
            let object = frame.get("OBJECT").unwrap();
            format!("{} {} {}", subject, verb, object)
        }).collect();

        assert!(sentences.contains("he throws it"));
        assert!(sentences.contains("she eats him"));
        assert!(sentences.contains("i throw it"));

        assert!(!sentences.contains("him throws it"));
        assert!(!sentences.contains("she eat him"));
        assert!(!sentences.contains("i throws it"));
    }
}

