use error::*;
use tok::Tok;

use std::iter::Iterator;

pub struct Lexer<I: Iterator<Item = Result<char>>> {
    current: Option<char>,
    chars: I
}

impl<I: Iterator<Item = Result<char>>> Lexer<I> {
    pub fn new(chars: I) -> Self {
        Lexer { chars: chars, current: None }
    }

    fn peek(&mut self) -> OptRes<char> {
        self.current.map(OptRes::ok)
                    .unwrap_or_else(|| self.next_char())
    }

    fn next_char(&mut self) -> OptRes<char> {
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
        self.peek().and_then(|c|
            if c.is_whitespace() {
                self.next_char().and_then(|_| self.skip_whitespace())
            } else {
                OptRes::ok(())
            }
        )
    }

    fn append_ident(&mut self, result: &mut String) -> OptRes<()> {
        self.peek().and_then(|c| {
            if c.is_alphanumeric() || c == '_' {
                result.push(c);
                self.next_char().and_then_ok(|| self.append_ident(result))
            } else {
                OptRes::ok(())
            }
        })

    }

    fn lex_ident(&mut self) -> OptRes<String> {
        let mut result = String::new();

        self.append_ident(&mut result).and_then_ok(|| OptRes::ok(result))
    }

    fn next_unless_err<T>(&mut self, x: T) -> OptRes<T> {
        self.next_char().and_then_ok(|| OptRes::ok(x))
    }

    fn err(&self, msg: &str) -> Error {
        Error::LexerError(msg.to_string())
    }
}

impl<I: Iterator<Item = Result<char>>> Iterator for Lexer<I> {
    type Item = Result<Tok>;

    fn next(&mut self) -> Option<Result<Tok>> {
        self.skip_whitespace()
            .and_then(|()| self.peek().and_then(|c| match c {
                ',' => self.next_unless_err(Tok::Comma),
                '.' => self.next_unless_err(Tok::Dot),
                ':' => self.next_char().and_then(|c| match c {
                    '-' => self.next_unless_err(Tok::Means),
                    _ => OptRes::err(self.err("expected \"-\" in \":-\""))
                }),
                '?' => self.next_unless_err(Tok::Query),
                '(' => self.next_unless_err(Tok::OpenParen),
                ')' => self.next_unless_err(Tok::CloseParen),
                c if c.is_lowercase() => self.lex_ident().map(Tok::Atom),
                c if c.is_uppercase() => self.lex_ident().map(Tok::Variable),
                _ => OptRes::err(self.err("unrecognized character"))
            })).0
    }
}

// We'll be doing a lot of mixed error handling. This struct makes chaining
// operations on that much cleaner.
struct OptRes<T>(Option<Result<T>>);

impl<T> OptRes<T> {
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
            _ => op()
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
