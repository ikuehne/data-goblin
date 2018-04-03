use error::*;
use tok::Tok;
use pos::Pos;
use pos::Range;
use owning_chars::OwningChars;

use std::io;
use std::iter::Iterator;

pub struct Lexer<R> {
    reader: R,
    pos: Pos,
    current_line: OwningChars
}

impl<R: io::BufRead> Lexer<R> {
    pub fn new(reader: R) -> Self {
        let current_line = OwningChars::new(String::new());
        let pos = Pos { chr: 0, line: -1 };
        Lexer { reader, pos: pos, current_line }
    }

    // Forget the old line, read a new one.
    fn next_line(&mut self) -> Option<Result<()>> {
        self.pos.next_line();
        let mut new_line = String::new();
        (match self.reader.read_line(&mut new_line) {
            Ok(0) => None,
            Ok(other) => Some(Ok(other)),
            Err(err) => Some(Err(Error::IOError(err)))
        }).map(|err| {
            err.map(|_| {
                self.current_line = OwningChars::new(new_line);
            })
        })
    }

    fn next_char(&mut self) -> Option<Result<char>> {
        loop {
            self.pos.next_char();
            match self.current_line.next() {
                None => {
                    match self.next_line() {
                        None => return None,
                        Some(Err(e)) => return Some(Err(e)),
                        Some(Ok(_)) => continue
                    }
                },
                Some(c) => return Some(Ok(c))
            }
        }
    }
}

impl<R: io::BufRead> Iterator for Lexer<R> {
    type Item = Result<(Range, Tok)>;

    fn next(&mut self) -> Option<Result<(Range, Tok)>> {
        let start = self.pos;

        self.next_char().map(|res| res.and_then(|c| match c {
            _ => Ok((Range { start: start, end: self.pos }, Tok::Dot))
        }))
    }
}
