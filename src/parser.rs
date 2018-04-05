/// Converting token streams into ASTs.

use error::*;
use ast::*;
use tok::Tok;
use std::iter::Iterator;

pub struct Parser<I: Iterator<Item = Result<Tok>>> {
    current: Option<Tok>,
    tokens: I
}

impl<I: Iterator<Item = Result<Tok>>> Parser<I> {
    pub fn new(tokens: I) -> Self {
        Parser { tokens: tokens, current: None }
    }
}

impl<I: Iterator<Item = Result<Tok>>> Iterator for Parser<I> {
    type Item = Result<Line>;

    fn next(&mut self) -> Option<Result<Line>> {
        None
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
        let mut parser = Parser::new(tokenstream.map(Ok));
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
