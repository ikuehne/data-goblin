/// Converting token streams into ASTs.

use error::*;
use ast::*;
use optres::OptRes;
use tok::Tok;

use std::iter::Iterator;
use std::iter::Peekable;


pub struct Parser<I: Iterator<Item = Result<Tok>>> {
    tokens: Peekable<I>
}

impl<I: Iterator<Item = Result<Tok>>> Parser<I> {
    pub fn new(tokens: I) -> Self {
        Parser { tokens: tokens.peekable() }
    }

    fn next_token(&mut self) -> OptRes<Tok> {
        match self.tokens.next() {
            Some(Ok(x)) => {
                OptRes::Good(x)
            }
            other => {
                OptRes::from(other)
            }
        }
    }

    // Greedily parse a term (take the largest term we can parse)
    fn parse_term(&mut self) -> OptRes<Term> {
        self.next_token()
            .and_then(|tok| {
            match tok {
                Tok::Atom(atom) => {
            match self.tokens.peek() {
                Some(Ok(Tok::OpenParen)) => {
                    self.parse_atomic_term_list()
                        .and_then(|inner|
                            OptRes::from(Some(Ok(
                                Term::Compound(
                                CompoundTerm { relation: atom.to_string(), params: inner })))))
                },
                // TODO - figure out what next tokens are possible in the grammar
                Some(Ok(Tok::Atom(atom))) => OptRes::from(Some(Ok(
                            Term::Atomic(AtomicTerm::Atom(atom.to_string()))))),
                Some(Ok(Tok::Variable(var))) => OptRes::from(Some(Ok(
                            Term::Atomic(AtomicTerm::Variable(var.to_string()))))),
                Some(Ok(x)) => OptRes::Bad(Error::Parser(
                        format!("Unexpected token after atom: {:?}", x))),
                // TODO - pass along the error even though peek returns a reference
                Some(Err(_)) => OptRes::Bad(self.err("Error reading next token")),
                None => OptRes::Done
            }
            },
                _ => OptRes::from(Some(Err(self.err("Expected atomic term."))))
            }
            })
    }

    // Parse the body of a rule - a list of terms forming a conjunction
    fn parse_term_list(&mut self) -> OptRes<Vec<Term>> {
        OptRes::Done
    }

    fn parse_atomic_term_list(&mut self) -> OptRes<Vec<AtomicTerm>> {
        OptRes::Done
    }

    fn err(&self, msg: &str) -> Error {
        Error::Parser(msg.to_string())
    }
}

impl<I: Iterator<Item = Result<Tok>>> Iterator for Parser<I> {
    type Item = Result<Line>;

    fn next(&mut self) -> Option<Result<Line>> {
        // First, parse a term. Then, by examining the next token
        // we know what kind of line we're looking at.
        self.parse_term()
            .and_then(|term| {
        self.next_token()
            .and_then(|token|
            match token {
                Tok::Dot => OptRes::Good(Line::Rule(Rule { head: term, body: vec!() })),
                Tok::Query => OptRes::Good(Line::Query(term)),
                Tok::Means => {
                    self.parse_term_list()
                        .and_then(|next_terms|
                            OptRes::Good(
                                Line::Rule(Rule { head: term, body: next_terms })))
                },
                _ => OptRes::Bad(Error::Parser(
                        format!("Unexpected token after term: {:?}", token)))
            })
        }).into()
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
        let parser = Parser::new(tokenstream.map(Ok));
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
