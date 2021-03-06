/// Converting token streams into ASTs.

use error::*;
use ast::*;
use tok::Tok;

use std::iter::Iterator;

// A useful macro for dealing with Option<Result>s.
macro_rules! try_get {
    ($expr:expr) => (match $expr {
        Some(Ok(val)) => val,
        Some(Err(e)) => return Some(Err(e)),
        None => return None
    })
}

pub struct Parser<I: Iterator<Item = Tok>> {
    tokens: I,
    current: Option<Tok>
}

impl<I: Iterator<Item = Tok>> Parser<I> {
    pub fn new(tokens: I) -> Self {
        Parser { tokens: tokens, current: None }
    }

    fn next_token(&mut self) -> Option<Tok> {
        self.tokens.next().map(|c| {
            self.current = Some(c.clone());
            c
        }).or_else(|| {
            self.current = None;
            None
        })
    }

    // Parse a term beginning with the given atom string.
    // If the next token is an open paren, parse the compound term. Otherwise,
    // just return an atomic term from that string.
    fn term_from_atom(&mut self, atom: String) -> Option<Result<Term>> {
        let next_token = self.next_token()?;
        // the token after an atom that begins a term should be either:
        //  OpenParen - if the atom is a relation name
        //  CloseParen - if the atom is at the end of the parameters list
        //  Query - if the atom is a query by itself
        //  Comma - if the atom is in the parameters of a compound term
        //  Dot - if the atom is its own rule with no body
        match next_token {
            Tok::OpenParen => {
                let params = try_get!(self.parse_atomic_term_list());
                // Advance past the final closing paren
                self.next_token()?;
                Some(Ok(Term::Compound(
                            CompoundTerm {
                                relation: atom.to_string(),
                                params: params
                            })))
            },
            Tok::Query | Tok::Dot | Tok::Comma | Tok::CloseParen
                => Some(Ok(Term::Atomic(AtomicTerm::Atom(atom.to_string())))),
            other => Some(Err(Error::Parser(
                    format!("Unexpected token after an atom: {:?}", other))))

        }
    }

    // Greedily parse a term (take the largest term we can parse)
    fn parse_term(&mut self) -> Option<Result<Term>> {
        let tok = self.next_token()?;
        match tok {
            Tok::Atom(atom) => self.term_from_atom(atom),
            Tok::Variable(var) => {
                // Since parse_term needs to get the next token after the term,
                // we need to advance the token iterator here
                self.next_token()?;
                Some(Ok(Term::Atomic(AtomicTerm::Variable(var))))
            },
            _ => Self::err(
                    format!("Unexpected token at the start of a term: {:?}",
                            tok))
        }
    }

    // Parse the body of a rule - a list of terms forming a conjunction
    // Assumes there will be at least one term.
    fn parse_term_list(&mut self) -> Option<Result<Vec<Term>>> {
        let mut terms = Vec::new();
        let next_term = try_get!(self.parse_term());
        terms.push(next_term);
        while let Some(Tok::Comma) = self.current {
            let next_term = try_get!(self.parse_term());
            terms.push(next_term);
        }
        Some(Ok(terms))
    }

    fn parse_atomic_term_list(&mut self) -> Option<Result<Vec<AtomicTerm>>> {
        let list = try_get!(self.parse_term_list());
        let mut atomic_terms = Vec::new();
        for term in list {
            match term {
                Term::Atomic(at) => atomic_terms.push(at),
                Term::Compound(_) => {
                    return Self::err(
                        "Syntax Error: nested compound term.".to_string());
                }
            }
        }
        Some(Ok(atomic_terms))
    }

    fn err<T>(msg: String) -> Option<Result<T>> {
        Some(Err(Error::Parser(msg)))
    }
}

impl<I: Iterator<Item = Tok>> Iterator for Parser<I> {
    type Item = Result<Line>;

    fn next(&mut self) -> Option<Result<Line>> {
        // First, parse a term. Then, by examining the next token
        // we know what kind of line we're looking at.
        let first_term = try_get!(self.parse_term());
        Some(Ok(match self.current {
            Some(Tok::Dot) => Line::Rule(Rule {
                head: first_term,
                body: vec!()
            }),
            Some(Tok::Query) => Line::Query(first_term),
            Some(Tok::Means) => {
                let term_list = try_get!(self.parse_term_list());
                Line::Rule(Rule { head: first_term, body: term_list })
            },
            Some(_) =>
                return Self::err(format!(
                    "Unexpected token following a term. Token: {:?}",
                    self.current)),
            None =>
                return Self::err(format!(
                    "Term found with no token following it: {:?}",
                    first_term))
        }))
    }
}

#[cfg(test)]
mod tests {
    use ast::*;
    use tok::Tok;
    use parser::Parser;
    use std::vec;

    fn parse_test(x: Vec<Tok>) -> Option<Vec<Line>> {
        let tokenstream : vec::IntoIter<Tok> = x.into_iter();
        let parser = Parser::new(tokenstream);
        parser.map(|opt| match opt {
                Ok(res) => Some(res),
                _ => None
            }).collect()
    }

    #[test]
    fn empty() {
        assert_eq!(parse_test(vec!()), Some(vec!()));
    }

    // Test some malformed lines (without ending punctuation, compound
    // terms inside of compound terms, etc)
    #[test]
    fn incomplete() {

    }

    #[test]
    fn nested_predicate() {

    }

    #[test]
    fn simplest_lines() {
        // We are not quite sure what these statements should do yet but they are
        // syntactically valid

        // > abraham.
        assert_eq!(parse_test(
                vec!(Tok::Atom("abraham".to_string()),
                     Tok::Dot)),
                Some(vec!(
                    Line::Rule( Rule {
                        head: Term::Atomic(AtomicTerm::Atom("abraham".to_string())),
                        body: vec!()
                    } )))
                );

        // > A.
        assert_eq!(parse_test(
                vec!(Tok::Variable("A".to_string()),
                     Tok::Dot)),
                Some(vec!(
                    Line::Rule( Rule {
                        head: Term::Atomic(AtomicTerm::Variable("A".to_string())),
                        body: vec!()
                    } )))
                );


        // > abraham?
        assert_eq!(parse_test(
                vec!(Tok::Atom("abraham".to_string()),
                     Tok::Query)),
                Some(vec!(
                        Line::Query(
                            Term::Atomic(AtomicTerm::Atom("abraham".to_string())))))
                );

        // > A?
        assert_eq!(parse_test(
                vec!(Tok::Variable("A".to_string()),
                     Tok::Query)),
                Some(vec!(
                        Line::Query(
                            Term::Atomic(AtomicTerm::Variable("A".to_string())))))
                );


    }

    #[test]
    fn simple_facts() {

        let head = Term::Compound(
            CompoundTerm { relation: "parent".to_string(),
                          params: vec!(
                            AtomicTerm::Atom("abraham".to_string()),
                            AtomicTerm::Atom("isaac".to_string())
                            ) });
        assert_eq!(parse_test(
                vec!(Tok::Atom("parent".to_string()),
                     Tok::OpenParen,
                     Tok::Atom("abraham".to_string()),
                     Tok::Comma,
                     Tok::Atom("isaac".to_string()),
                     Tok::CloseParen,
                     Tok::Dot)
                ),
                Some(vec!(
                        Line::Rule(
                            Rule {
                                head: head,
                                body: vec!()
                            })
                        )));

    }

    #[test]
    fn simple_rules() {

    }

}
