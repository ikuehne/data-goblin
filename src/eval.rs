/// Evaluator for dead-simple queries.
/// 
/// Intended mostly as a skeleton while we work out architecture.

use ast;
use error::*;
use storage;
use storage::Relation::*;

pub struct QueryParams {
    params: Vec<ast::AtomicTerm>
}

impl QueryParams {
    fn match_tuple(&mut self, t: &storage::Tuple) -> bool {
        for i in 0..self.params.len() {
            match self.params[i] {
                ast::AtomicTerm::Atom(ref s) => {
                    if *s != t[i] {
                        return false;
                    }
                },
                _ => ()
            }
        }
        true
    }
}

pub enum QueryResult<'i> {
    TableFound {
        query: QueryParams,
        scan: storage::TableScan<'i>
    },
    NoTableFound
}

impl<'i> Iterator for QueryResult<'i> {
    type Item = &'i storage::Tuple;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            QueryResult::TableFound { query, scan } =>
                scan.filter(|t| query.match_tuple(t)).next(),
            QueryResult::NoTableFound => None
        }
    }
}

fn to_atom(t: ast::AtomicTerm) -> Result<String> {
    match t {
        ast::AtomicTerm::Atom(s) => Ok(s),
        ast::AtomicTerm::Variable(v) =>
            Err(Error::MalformedLine(format!("unexpected variable: {}", v)))
    }
}

fn deconstruct_term(t: ast::Term) -> Result<(String, QueryParams)> {
    match t {
        ast::Term::Atomic(a) => Ok((to_atom(a)?,
                                    QueryParams { params :Vec::new() })),
        ast::Term::Compound(cterm) => {
            let mut rest = Vec::new();
            for param in cterm.params.into_iter() {
                rest.push(param);
            }
            Ok((cterm.relation, QueryParams { params: rest }))
        }
    }
}

fn create_tuple(p: QueryParams) -> Result<storage::Tuple> {
    let mut result = Vec::new();
    for param in p.params {
        result.push(to_atom(param)?);
    }
    Ok(result)
}

pub fn query(engine: &storage::StorageEngine,
             query: ast::Term) -> Result<QueryResult> {
    let (head, rest) = deconstruct_term(query)?;

    engine.get_relation(head.as_str()).map(|r| match r {
        Extension(ref table) => Ok(QueryResult::TableFound {
                query: rest,
                scan: table.into_iter()
            }),
        Intension(_) => Err(Error::NotExtensional(head.clone()))
    }).unwrap_or(Ok(QueryResult::NoTableFound))
}

pub fn simple_assert(engine: &mut storage::StorageEngine,
                     fact: ast::Term) -> Result<()> {
    let (head, rest) = deconstruct_term(fact)?;
    let tuple = create_tuple(rest)?;
    match *engine.get_or_create_relation(head.clone()) {
        Extension(ref mut t) => Ok(t.assert(tuple)),
        Intension(_) => Err(Error::NotExtensional(head))
    }
}

pub fn assert(engine: &mut storage::StorageEngine,
              fact: ast::Rule) -> Result<()> {
    if fact.body.len() == 0 {
        simple_assert(engine, fact.head)
    } else {
        Ok(())
    }
}
