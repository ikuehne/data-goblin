/// Converting character streams into token streams.

use error::*;
use optres::OptRes;
use tok::Tok;

use std::iter::Iterator;

/// Adapts an `Iterator` over `char`s to an iterator over `Tok`s.
/// 
/// More precisely, adapts an `Iterator` over `Result<char>`s to account for the
/// possibility that the underlying stream fails. This allows, for example,
/// lazily reading from an input stream.
pub struct Lexer<I: Iterator<Item = Result<char>>> {
    current: Option<char>,
    chars: I
}

impl<I: Iterator<Item = Result<char>>> Lexer<I> {
    pub fn new(chars: I) -> Self {
        Lexer { chars: chars, current: None }
    }

    fn peek(&mut self) -> OptRes<char> {
        self.current.map(OptRes::Good)
                    .unwrap_or_else(|| self.next_char())
    }

    fn next_char(&mut self) -> OptRes<char> {
        match self.chars.next() {
            Some(Ok(x)) => {
                self.current = Some(x);
                OptRes::Good(x)
            }
            other => {
                self.current = None;
                OptRes::from(other)
            }
        }
    }

    fn skip_whitespace(&mut self) -> OptRes<()> {
        self.peek().and_then(|c|
            if c.is_whitespace() {
                self.next_char().and_then(|_| self.skip_whitespace())
            } else {
                OptRes::Good(())
            }
        )
    }

    fn append_ident(&mut self, result: &mut String) -> OptRes<()> {
        self.peek().and_then(|c| {
            if c.is_alphanumeric() || c == '_' {
                result.push(c);
                self.next_char().unless_err(|| self.append_ident(result))
            } else {
                OptRes::Good(())
            }
        })

    }

    fn lex_ident(&mut self) -> OptRes<String> {
        let mut result = String::new();

        self.append_ident(&mut result).unless_err(|| OptRes::Good(result))
    }

    fn next_unless_err<T>(&mut self, x: T) -> OptRes<T> {
        self.next_char().unless_err(|| OptRes::Good(x))
    }

    fn err(&self, msg: &str) -> Error {
        Error::Lexer(msg.to_string())
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
                    _ => OptRes::Bad(self.err("expected \"-\" in \":-\""))
                }),
                '?' => self.next_unless_err(Tok::Query),
                '(' => self.next_unless_err(Tok::OpenParen),
                ')' => self.next_unless_err(Tok::CloseParen),
                c if c.is_lowercase() => self.lex_ident().map(Tok::Atom),
                c if c.is_uppercase() => self.lex_ident().map(Tok::Variable),
                c => OptRes::Bad(Error::Lexer(
                        format!("unrecognized character: {}", c)))
            })).into()
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
