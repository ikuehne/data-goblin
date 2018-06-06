#![feature(custom_attribute)]
#![feature(io)]
#![feature(option_filter)]
#![feature(test)]
#![feature(trait_alias)]
#![feature(type_ascription)]

pub mod ast;
pub mod cache;
pub mod driver;
pub mod error;
pub mod eval;
pub mod lexer;
pub mod parser;
pub mod tok;
pub mod storage;

extern crate colored;
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
    use cache;
    use lexer::Lexer;
    use parser::Parser;

    use std::collections::HashSet;
    extern crate test;

    #[test]
    fn simple_sentences() {
        let engine = StorageEngine::new("test_data/grammar".to_string())
            .unwrap();
        let cache = cache::ViewCache::new();
        let query = "simple_sentence(SUBJECT, VERB, OBJECT)?";
        let lexer = Lexer::new(query.chars()).map(Result::unwrap);
        let parser = Parser::new(lexer).map(Result::unwrap);
        let sentences: HashSet<String> = parser.map(|line| {
            if let ast::Line::Query(t) = line {
                eval::query(&engine, &cache, t).unwrap()
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

    #[test]
    fn employee_hierarchy() {

        let engine = StorageEngine::new("test_data/hierarchy".to_string())
            .unwrap();
        let cache = cache::ViewCache::new();
        let query = "reports(EMP, MAN)?";
        let lexer = Lexer::new(query.chars()).map(Result::unwrap);
        let parser = Parser::new(lexer).map(Result::unwrap);
        let reports: HashSet<String> = parser.map(|line| {
            if let ast::Line::Query(t) = line {
                eval::query(&engine, &cache, t).unwrap()
            } else {
                panic!("parsed query as assertion");
            }
        }).next().unwrap().map(|frame| {
            let employee = frame.get("EMP").unwrap();
            let manager = frame.get("MAN").unwrap();
            format!("{}, {}", employee, manager)
        }).collect();

        assert!(reports.contains("id_10001, id_NULL"));
        assert!(reports.contains("id_10005, id_10002"));
        assert!(reports.contains("id_10006, id_10004"));

        assert!(!reports.contains("id_NULL, id_10001"));
        assert!(!reports.contains("id_10003, id_10002"));
        assert!(!reports.contains("id_10003, id_10007"));
    }

    #[test]
    fn test_recursive_query() {
        let engine = StorageEngine::new("test_data/hierarchy".to_string())
            .unwrap();
        let cache = cache::ViewCache::new();
        let query = "underling(UNDER, OVER)?";
        let lexer = Lexer::new(query.chars()).map(Result::unwrap);
        let parser = Parser::new(lexer).map(Result::unwrap);
        let underlings_bottom_up: HashSet<String> = parser.map(|line| {
            if let ast::Line::Query(t) = line {
                eval::query(&engine, &cache, t).unwrap()
            } else {
                panic!("parsed query as assertion");
            }
        }).next().unwrap().map(|frame| {
            let employee = frame.get("UNDER").unwrap();
            let manager = frame.get("OVER").unwrap();
            format!("{}, {}", employee, manager)
        }).collect();

        // TODO: improve these tests
        assert!(underlings_bottom_up.contains("id_10001, id_NULL"));
        assert!(underlings_bottom_up.contains("id_10005, id_10002"));
        assert!(underlings_bottom_up.contains("id_10006, id_10004"));

        assert!(!underlings_bottom_up.contains("id_NULL, id_10001"));
        assert!(!underlings_bottom_up.contains("id_10003, id_10002"));
        assert!(!underlings_bottom_up.contains("id_10003, id_10007"));
        
        let lexer_sn = Lexer::new(query.chars()).map(Result::unwrap);
        let parser_sn = Parser::new(lexer_sn).map(Result::unwrap);

        let underlings_semi_naive: HashSet<String> = parser_sn.map(|line| {
            if let ast::Line::Query(t) = line {
                eval::query_semi_naive(&engine, &cache, t).unwrap()
            } else {
                panic!("parsed query as assertion");
            }
        }).next().unwrap().map(|frame| {
            let employee = frame.get("UNDER").unwrap();
            let manager = frame.get("OVER").unwrap();
            format!("{}, {}", employee, manager)
        }).collect();

        assert!(underlings_bottom_up == underlings_semi_naive);

    }


    #[bench]
    fn simple_view_query(b: &mut test::Bencher) {
        let engine = StorageEngine::new("test_data/hierarchy".to_string())
            .unwrap();
        let cache = cache::ViewCache::new();
        b.iter(|| {
            // TODO: Find a way to move some of this setup outside the benchmark
            // iteration.
            let query = "reports(Emp, Man)?";
            let lexer = Lexer::new(query.chars()).map(Result::unwrap);
            let parser = Parser::new(lexer).map(Result::unwrap);
            for line in parser {
                if let ast::Line::Query(t) = line {
                    eval::query(&engine, &cache, t).unwrap();
                } else {
                    panic!("parsed query as assertion");
                }
            }
        });
    }

    #[bench]
    fn recursive_query(b: &mut test::Bencher) {
        let engine = StorageEngine::new("test_data/hierarchy".to_string())
              .unwrap();
        b.iter(|| {
            let cache = cache::ViewCache::new();
            let query = "underling(Under, Over)?";
            let lexer = Lexer::new(query.chars()).map(Result::unwrap);
            let parser = Parser::new(lexer).map(Result::unwrap);
            for line in parser {
                if let ast::Line::Query(t) = line {
                    eval::query(&engine, &cache, t).unwrap();
                } else {
                    panic!("parsed query as assertion");
                }
            }
        });
    }

    #[bench]
    fn recursive_query_semi_naive(b: &mut test::Bencher) {
        let engine = StorageEngine::new("test_data/hierarchy".to_string())
              .unwrap();
        b.iter(|| {
            let cache = cache::ViewCache::new();
            let query = "underling(Under, Over)?";
            let lexer = Lexer::new(query.chars()).map(Result::unwrap);
            let parser = Parser::new(lexer).map(Result::unwrap);
            for line in parser {
                if let ast::Line::Query(t) = line {
                    eval::query_semi_naive(&engine, &cache, t).unwrap();
                } else {
                    panic!("parsed query as assertion");
                }
            }
        });
    }
}

