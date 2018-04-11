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

    // Parse a term beginning with the given atom string.
    // If the next token is an open paren, parse the compound term. Otherwise,
    // just return an atomic term from that string.
    fn term_from_atom(&mut self, atom: String) -> OptRes<Term> {
        let next_token = self.tokens.peek();
        // the token after an atom that begins a term should be either:
        //  OpenParen - if the atom is a relation name
        //  Query - if the atom is a query by itself
        //  Comma - if the atom is in the parameters of a compound term
        // TODO - are there more tokens that can follow an atom that begins a term?
        // I'm pretty sure an atomic term can't be in the body list of a rule - that
        // doesn't make much sense.
        // Can an atom end with a dot? Single atom facts don't make sense to me.
        match next_token {
            Some(&Ok(Tok::OpenParen)) => {
                let params = try_get!(self.parse_atomic_term_list());
                OptRes::Good(Term::Compound(
                        CompoundTerm { relation: atom.to_string(), params: params }))
            },
            Some(&Ok(Tok::Query)) => OptRes::Good(Term::Atomic(
                    AtomicTerm::Atom(atom.to_string()))),
            None => OptRes::Done,
            Some(&Ok(ref x)) => OptRes::Bad(Error::Parser(
                    format!("Unexpected token following atom: {:?}", x))),
            // If we peek and see an error, we can just return the error by asking
            // for the next token. Can't just return the error, because peeking gives
            // us a reference.
            Some(&Err(_)) => OptRes::Bad(self.err("Error reading a token"))
        }
    }

    // Greedily parse a term (take the largest term we can parse)
    fn parse_term(&mut self) -> OptRes<Term> {
        let tok = try_get!(self.next_token());
        match tok {
        
            Tok::Atom(atom) => self.term_from_atom(atom),
            Tok::Variable(var) => OptRes::from(Some(Ok(
                        Term::Atomic(AtomicTerm::Variable(var))))),
            _ => OptRes::Bad(Error::Parser(
                    format!("Unexpected token at the start of a term: {:?}", tok)))
        }
    }

    // Parse the body of a rule - a list of terms forming a conjunction
    fn parse_term_list(&mut self) -> OptRes<Vec<Term>> {
        OptRes::Done
    }

    fn parse_atomic_term_list(&mut self) -> OptRes<Vec<AtomicTerm>> {
        // TODO - this doesn't need to be any different from parse_term_list,
        // but if any of the terms is a compound term, it's a syntax error.
        let list = try_get!(self.parse_term_list());
        let mut atomic_terms = Vec::new();
        for term in list {
            match term {
                Term::Atomic(at) => atomic_terms.push(at),
                Term::Compound(_) => { return OptRes::Bad(
                    self.err("Syntax Error: nested compound term.")); }
            }
        }
        OptRes::Good(atomic_terms)
    }

    fn err(&self, msg: &str) -> Error {
        Error::Parser(msg.to_string())
    }

    fn next_optres(&mut self) -> OptRes<Line> {
        let first_term = try_get!(self.parse_term());
        let next_token = try_get!(self.next_token());
        match next_token {
            Tok::Dot => OptRes::Good(
                Line::Rule(Rule { head: first_term, body: vec!() })),
            Tok::Query => OptRes::Good(
                Line::Query(first_term)),
            Tok::Means => {
                        
                let term_list = try_get!(self.parse_term_list());
                OptRes::Good(
                    Line::Rule(Rule { head: first_term, body: term_list }))
            },
            _ => OptRes::Bad(Error::Parser(format!(
                    "Unexpected token following a term. Token: {:?}", next_token)))
        }
    }
}

impl<I: Iterator<Item = Result<Tok>>> Iterator for Parser<I> {
    type Item = Result<Line>;

    fn next(&mut self) -> Option<Result<Line>> {
        // First, parse a term. Then, by examining the next token
        // we know what kind of line we're looking at.
        self.next_optres().into()
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
