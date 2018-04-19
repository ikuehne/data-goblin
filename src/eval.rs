/// Evaluator for dead-simple queries.
/// 
/// Intended mostly as a skeleton while we work out architecture.

use ast;
use error::*;
use storage;
use storage::Relation::*;

pub struct Evaluator {
    engine: storage::StorageEngine
}

pub enum QueryResult<'i> {
    TableFound {
        query: Vec<String>,
        scan: storage::TableScan<'i>
    },
    NoTableFound
}

impl<'i> Iterator for QueryResult<'i> {
    type Item = &'i storage::Tuple;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            QueryResult::TableFound { query, scan } =>
                match scan.next() {
                    None => None,
                    Some(t) => if *t == *query {
                        Some(t)
                    } else {
                        self.next()
                    }
                }
            QueryResult::NoTableFound => None
        }
    }
}

impl Evaluator {
    pub fn new(engine: storage::StorageEngine) -> Self {
        Evaluator { engine }
    }

    fn to_atom(t: ast::AtomicTerm) -> Result<String> {
        match t {
            ast::AtomicTerm::Atom(s) => Ok(s),
            ast::AtomicTerm::Variable(v) =>
                Err(Error::MalformedLine(format!("unexpected variable: {}", v)))
        }
    }

    fn deconstruct_term(t: ast::Term) -> Result<(String, Vec<String>)> {
        match t {
            ast::Term::Atomic(a) => Ok((Self::to_atom(a)?, Vec::new())),
            ast::Term::Compound(cterm) => {
                let mut rest = Vec::new();
                for param in cterm.params.into_iter().map(Self::to_atom) {
                    rest.push(param?);
                }
                Ok((cterm.relation, rest))
            }
        }
    }

    pub fn query(&self, query: ast::Term) -> Result<QueryResult> {
        let (head, rest) = Self::deconstruct_term(query)?;

        self.engine.get_table(&head).map(|r| match r {
            Extension(table) => Ok(QueryResult::TableFound {
                    query: rest,
                    scan: table.into_iter()
                }),
            Intension(_) => Err(Error::NotExtensional(head.clone()))
        }).unwrap_or(Ok(QueryResult::NoTableFound))
    }

    pub fn simple_assert(&mut self, fact: ast::Term) -> Result<()> {
        let (head, rest) = Self::deconstruct_term(fact)?;
        match self.engine.get_or_create_table(head.clone()) {
            Extension(t) => Ok(t.assert(rest)),
            Intension(_) => Err(Error::NotExtensional(head))
        }
    }

    pub fn assert(&mut self, fact: ast::Rule) -> Result<()> {
        if fact.body.len() == 0 {
            self.simple_assert(fact.head)
        } else {
            Ok(())
        }
    }
}
