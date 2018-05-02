#![feature(custom_attribute)]
#![feature(io)]
#![feature(option_filter)]
#![feature(type_ascription)]

pub mod ast;
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

use error::*;

use std::collections::HashMap;
use std::io;
use std::io::stdout;
use std::io::Read;
use std::io::Write;
use std::fmt::Display;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;
use std::sync::TryLockError::WouldBlock;
use std::time::Duration;

const DEFAULT_DATA_DIR: &'static str = "./data/";

fn abort<T: Display>(e: T) -> ! {
    eprintln!("Error: {}", e);
    std::process::exit(1)
}

fn handle_line(engine: &RwLock<storage::StorageEngine>, line: ast::Line)
        -> Result<()> {
    Ok(match line {
        ast::Line::Query(t) => {
            for tuple in eval::scan_from_term(&engine.read().unwrap(), t)? {
                println!("{:?}", tuple);
            }
        },
        ast::Line::Rule(r) => eval::assert(&mut engine.write().unwrap(), r)?
    })
}

fn write_back(engine: Arc<RwLock<storage::StorageEngine>>,
              done: Arc<AtomicBool>)
        -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while !done.load(Ordering::Relaxed) {
            match engine.try_read() {
                Ok(guard) => guard.write_back(),
                Err(WouldBlock) => (),
                Err(_) => panic!("poisoned engine lock")
            };
            std::thread::sleep(Duration::from_millis(250));
        }
    })
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
    let engine = storage::StorageEngine::new(DEFAULT_DATA_DIR.to_string())
        .unwrap_or_else(|e| abort(e));

    let locked_engine = Arc::new(RwLock::new(engine));
    let done = Arc::new(AtomicBool::default());

    let write_back_handle = write_back(locked_engine.clone(), done.clone());

    for line in lines {
        handle_line(&locked_engine, line).unwrap_or_else(|e| {
            eprintln!("Error: {}", e);
        });
        print!("{}", prompt);
        stdout().flush().unwrap();
    }

    done.store(true, Ordering::Relaxed);

    write_back_handle.join().unwrap();
}
