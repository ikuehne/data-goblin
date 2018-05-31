use error::*;

use ast;
use cache::ViewCache;
use eval;
use lexer::Lexer;
use storage;
use parser::Parser;

use colored::Colorize;

use std;
use std::fmt::Display;
use std::io;
use std::io::stdout;
use std::io::Read;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;
use std::sync::TryLockError::WouldBlock;
use std::time::Duration;

fn abort<T: Display>(e: T) -> ! {
    eprintln!("Error: {}", e);
    std::process::exit(1)
}

fn unwrap_or_abort<T, E: Display>(res: std::result::Result<T, E>) -> T {
    res.unwrap_or_else(|e| abort(e))
}

#[derive(Copy, Clone)]
#[allow(dead_code)]
enum DriverMode {
    Interactive,
    Quiet
}

static PROMPT: &'static str = "data-goblin> ";

pub struct Driver {
    lines: Box<Iterator<Item = ast::Line>>,
    storage: Arc<RwLock<storage::StorageEngine<eval::AstView>>>,
    writer: std::thread::JoinHandle<()>,
    done: Arc<AtomicBool>,
    mode: DriverMode
}

impl Driver {
    pub fn from_stdin(data_dir: String) -> Driver {
        Self::from_reader(io::stdin(), data_dir, DriverMode::Interactive)
    }

    pub fn run(self) {
        print!("{}", PROMPT.bright_blue());

        // TODO: Initially populate cache.
        let mut cache = ViewCache::new();

        eval::initialize_view_cache(&self.storage.read().unwrap(), &mut cache);

        stdout().flush().unwrap();
        for line in self.lines {
            Self::handle_line(self.storage.clone(), &mut cache, self.mode, line)
                .unwrap_or_else(|e| {
                    eprintln!("{} {}", "Error:".bright_red(), e)
                });
            match self.mode {
                DriverMode::Quiet => continue,
                DriverMode::Interactive => {
                    print!("{}", PROMPT.bright_blue());
                    stdout().flush().unwrap();
                }
            }
        }

        self.done.store(true, Ordering::Relaxed);

        self.writer.join().unwrap();

        self.storage.write().unwrap().write_back();
    }

    fn make_writer(engine: Arc<RwLock<storage::StorageEngine<eval::AstView>>>,
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

    fn from_reader<'a, Reader: io::Read + 'static>(
            reader: Reader, data_dir: String, mode: DriverMode)
                -> Driver {
        let buffered = io::BufReader::new(reader);
        let chars = buffered.chars().map(unwrap_or_abort);

        let lexer = Lexer::new(chars);
        let toks = lexer.map(unwrap_or_abort);

        let parser = Parser::new(toks);
        let lines = Box::new(parser.map(unwrap_or_abort));

        let unlocked_storage = unwrap_or_abort(
            storage::StorageEngine::new(data_dir));
        let storage = Arc::new(RwLock::new(unlocked_storage));

        let done = Arc::new(AtomicBool::new(false));

        let writer = Self::make_writer(storage.clone(), done.clone());

        Driver { lines, storage, writer, done, mode }
    }

    fn handle_line(storage: Arc<RwLock<storage::StorageEngine<eval::AstView>>>,
                   cache: &mut ViewCache,
                   mode: DriverMode,
                   line: ast::Line) -> Result<()> {
        Ok(match line {
            ast::Line::Query(t) => {
                match mode {
                    DriverMode::Quiet => (),
                    DriverMode::Interactive => {
                        let engine = &storage.read().unwrap();
                        for frame in eval::query(engine, cache, t)? {
                            let l = frame.len();
                            for (i, (var, val)) in frame.iter().enumerate() {
                                print!("{}{:} {}", var.bright_black(),
                                                   ":".bright_black(),
                                                   val);
                                unwrap_or_abort(stdout().flush());
                                if i != l - 1 {
                                    println!("");
                                }
                            }

                            let mut buf = String::new();
                            unwrap_or_abort(io::stdin().read_line(&mut buf));
                            println!("");
                            match buf.as_str() {
                                ";\n" => continue,
                                _ => break
                            }
                        }
                    }
                }
            },
            ast::Line::Rule(r) => {
                eval::assert(&mut storage.write().unwrap(), cache, r)?
            }
        })
    }
}
