/// Converting character streams into token streams.

use error::*;
use tok::Tok;

use std::iter::Iterator;

#[derive(Debug)]
enum Buffer {
    Uninitialized,
    EOF,
    Lexing(char)
}

/// Adapts an `Iterator` over `char`s to an iterator over `Tok`s.
pub struct Lexer<I: Iterator<Item = char>> {
    current: Buffer,
    chars: I
}

impl<I: Iterator<Item = char>> Lexer<I> {
    pub fn new(chars: I) -> Self {
        Lexer { chars: chars, current: Buffer::Uninitialized }
    }

    fn peek(&mut self) -> Option<char> {
        match self.current {
            Buffer::Uninitialized => self.next_char(),
            Buffer::EOF => None,
            Buffer::Lexing(c) => Some(c)
        }
    }

    fn next_char(&mut self) -> Option<char> {
        self.chars.next().map(|c| {
            self.current = Buffer::Lexing(c);
            c
        }).or_else(|| {
            self.current = Buffer::EOF;
            None
        })
    }

    fn skip_whitespace(&mut self) {
        while self.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
            self.next_char();
        }
    }

    fn append_ident(&mut self, result: &mut String) {
        loop {
            match self.peek()
                      .filter(|c| c.is_alphanumeric() || *c == '_')
                      .and_then(|c| {
                          result.push(c);
                          self.next_char()
                      }) {
                Some(_) => continue,
                None => return
            }
        }
    }

    fn lex_ident(&mut self) -> String {
        let mut result = String::new();

        self.append_ident(&mut result);
        result
    }

    fn unrecognized(c: char) -> Error {
        Error::Lexer(format!("unrecognized character: {}", c))
    }

    fn unexpected(c: char) -> Error {
        Error::Lexer(format!("unexpected character: {}", c))
    }
}

impl<I: Iterator<Item = char>> Iterator for Lexer<I> {
    type Item = Result<Tok>;

    fn next(&mut self) -> Option<Result<Tok>> {
        self.skip_whitespace();
        let c = self.peek()?;
        match c {
            ',' => {
                self.next_char();
                Some(Ok(Tok::Comma))
            },
            '.' => {
                self.next_char();
                Some(Ok(Tok::Dot))
            },
            ':' => {
                let c = self.next_char()?;
                match c {
                    '-' => {
                        self.next_char();
                        Some(Ok(Tok::Means))
                    }
                    c => Some(Err(Self::unexpected(c)))
                }
            },
            '?' => {
                self.next_char();
                Some(Ok(Tok::Query))
            },
            '(' => {
                self.next_char();
                Some(Ok(Tok::OpenParen))
            },
            ')' => {
                self.next_char();
                Some(Ok(Tok::CloseParen))
            },
            c if c.is_lowercase() => Some(Ok(Tok::Atom(self.lex_ident()))),
            c if c.is_uppercase() => Some(Ok(Tok::Variable(self.lex_ident()))),
            c => Some(Err(Self::unrecognized(c)))
        }
    }
}

#[cfg(test)]
mod tests {
    use tok::Tok;
    use lexer::Lexer;

    fn lex_test(x: &str) -> Option<Vec<Tok>> {
        Lexer::new(x.chars())
            .map(Result::ok)
            .collect()
    }

    #[test]
    fn symbols() {
        assert_eq!(lex_test("("), Some(vec!(Tok::OpenParen)));
        assert_eq!(lex_test(")"), Some(vec!(Tok::CloseParen)));
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
        assert_eq!(lex_test("    "), Some(vec!()));
        assert_eq!(lex_test(" \n\r\t"), Some(vec!()));
    }

    #[test]
    fn atoms() {
        assert_eq!(lex_test("a"), Some(vec!(Tok::Atom("a".to_string()))));
        assert_eq!(lex_test("a_t_o_m"),
                   Some(vec!(Tok::Atom("a_t_o_m".to_string()))));
        assert_eq!(lex_test("aTOM"),
                   Some(vec!(Tok::Atom("aTOM".to_string()))));
        assert_eq!(lex_test(" atom1 atom_2 aTOM3"),
                   Some(vec!(Tok::Atom("atom1".to_string()),
                             Tok::Atom("atom_2".to_string()),
                             Tok::Atom("aTOM3".to_string()))));
    }

    #[test]
    fn vars() {
        assert_eq!(lex_test("V"), Some(vec!(Tok::Variable("V".to_string()))));
        assert_eq!(lex_test("V_A_R"),
                   Some(vec!(Tok::Variable("V_A_R".to_string()))));
        assert_eq!(lex_test("Var"),
                   Some(vec!(Tok::Variable("Var".to_string()))));
        assert_eq!(lex_test(" VAR1 VAR_2 Var3"),
                   Some(vec!(Tok::Variable("VAR1".to_string()),
                             Tok::Variable("VAR_2".to_string()),
                             Tok::Variable("Var3".to_string()))));
    }

    #[test]
    fn combined() {
         assert_eq!(lex_test("rule(Var, atom) :- first(atom, Var),
                                                 second(atom, atom)."),
                    Some(vec!(Tok::Atom("rule".to_string()),
                              Tok::OpenParen,
                              Tok::Variable("Var".to_string()),
                              Tok::Comma,
                              Tok::Atom("atom".to_string()),
                              Tok::CloseParen,
                              Tok::Means,
                              Tok::Atom("first".to_string()),
                              Tok::OpenParen,
                              Tok::Atom("atom".to_string()),
                              Tok::Comma,
                              Tok::Variable("Var".to_string()),
                              Tok::CloseParen,
                              Tok::Comma,
                              Tok::Atom("second".to_string()),
                              Tok::OpenParen,
                              Tok::Atom("atom".to_string()),
                              Tok::Comma,
                              Tok::Atom("atom".to_string()),
                              Tok::CloseParen,
                              Tok::Dot)));
    }
}
