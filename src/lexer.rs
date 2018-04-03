use error::*;
use tok::Tok;

use std::io;
use std::iter::Iterator;
use std::result;

type CharsResult = result::Result<char, io::CharsError>;

pub struct Lexer<I: Iterator<Item = Result<char>>> {
    current: Option<char>,
    chars: I
}

impl<I: Iterator<Item = Result<char>>> Lexer<I> {
    pub fn new(chars: I) -> Self {
        Lexer { chars: chars, current: None }
    }

    fn peek(&mut self) -> OptRes<char> {
        println!("In peek");
        self.current.map(OptRes::ok)
                    .unwrap_or_else(|| self.next_char())
    }

    fn next_char(&mut self) -> OptRes<char> {
        println!("In next_char");
        match self.chars.next() {
            Some(Ok(x)) => {
                self.current = Some(x);
                OptRes::ok(x)
            }
            other => {
                self.current = None;
                OptRes(other)
            }
        }
    }

    fn skip_whitespace(&mut self) -> OptRes<()> {
        println!("In skip_whitespace");
        self.peek().and_then(|c|
            if c.is_whitespace() {
                self.next_char().and_then(|_| self.skip_whitespace())
            } else {
                OptRes::ok(())
            }
        )
    }

    fn err(&self, msg: &str) -> Error {
        Error::LexerError(msg.to_string())
    }
}

impl<I: Iterator<Item = Result<char>>> Iterator for Lexer<I> {
    type Item = Result<Tok>;

    fn next(&mut self) -> Option<Result<Tok>> {
        println!("called \"next\"");

        self.skip_whitespace()
            .and_then(|()| self.peek().and_then(|c| match c {
                ',' => 
                    self.next_char().and_then_ok(||
                        OptRes::ok(Tok::Comma)),
                '.' =>
                    self.next_char().and_then_ok(||
                        OptRes::ok(Tok::Dot)),
                ':' => self.next_char().and_then(|c| match c {
                    '-' =>
                        self.next_char().and_then_ok(||
                            OptRes::ok(Tok::Means)),
                    _ => OptRes::err(self.err("expected \"-\" in \":-\""))
                }),
                '?' =>
                    self.next_char().and_then_ok(||
                        OptRes::ok(Tok::Query)),
                _ => OptRes::err(self.err("unrecognized character"))
            })).0
    }
}

// We'll be doing a lot of mixed error handling. This struct makes chaining
// operations on that much cleaner.
struct OptRes<T>(Option<Result<T>>);

impl<T> OptRes<T> {
    fn none() -> Self {
        OptRes(None)
    }

    fn opt(x: Option<T>) -> OptRes<T> {
        OptRes(x.map(Ok))
    }

    fn err(x: Error) -> Self {
        OptRes(Some(Err(x)))
    }

    fn ok(x: T) -> Self {
        OptRes(Some(Ok(x)))
    }

    fn and_then<U, F: FnOnce(T) -> OptRes<U>> (self, op: F) -> OptRes<U> {
        match self.0 {
            Some(Ok(x)) => op(x),
            Some(Err(e)) => OptRes(Some(Err(e))),
            None => OptRes(None)
        }
    }

    fn and_then_ok<U, F: FnOnce() -> OptRes<U>>(self, op: F) -> OptRes<U> {
        match self.0 {
            Some(Err(e)) => OptRes::err(e),
            other => op()
        }
    }

    fn map<U, F: FnOnce(T) -> U> (self, op: F) -> OptRes<U> {
        self.and_then(|x| OptRes::ok(op(x)))
    }
}

#[cfg(test)]
mod tests {
    use tok::Tok;
    use lexer::Lexer;

    fn lex_test(x: &str) -> Option<Vec<Tok>> {
        Lexer::new(x.chars().map(Ok))
            .map(|opt| match opt {
                Ok(res) => Some(res),
                _ => None
            }).collect()
    }

    #[test]
    fn symbols() {
        assert_eq!(lex_test("?"), Some(vec!(Tok::Query)));
        assert_eq!(lex_test("."), Some(vec!(Tok::Dot)));
        assert_eq!(lex_test(","), Some(vec!(Tok::Comma)));
        assert_eq!(lex_test(":-"), Some(vec!(Tok::Means)));
        assert_eq!(lex_test(" ? , . :-"),
                   Some(vec!(Tok::Query, Tok::Comma, Tok::Dot, Tok::Means)));
    }

    #[test]
    fn empty() {
        assert_eq!(lex_test(""), Some(vec!()));
        assert_eq!(lex_test(""), Some(vec!()));
    }
}
