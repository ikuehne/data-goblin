/// Evaluator for dead-simple queries.
/// 
/// Intended mostly as a skeleton while we work out architecture.

use ast;
use error::*;
use std::collections::HashMap;
use storage;
use storage::Relation::*;

pub struct Evaluator {
    engine: storage::StorageEngine
}

pub struct QueryParams {
    params: Vec<ast::AtomicTerm>
}

impl QueryParams {
    
    /* Check if the given tuple can match with the query parameters, and
     * return a map of variable bindings.
     */
    fn match_tuple<'a>(&'a mut self, t: &'a storage::Tuple)
        -> Option<HashMap<&'a str, &'a str>> {

        // Ensure each variable is bound to exactly one atom
        let mut variable_bindings: HashMap<&str, &str> = HashMap::new();

        for i in 0..self.params.len() {
            match self.params[i] {
                ast::AtomicTerm::Atom(ref s) => {
                    if *s != t[i] {
                        return None;
                    }
                },
                ast::AtomicTerm::Variable(ref s) => {
                    let binding = variable_bindings.entry(s.as_str())
                        .or_insert(&t[i]);
                    if *binding != t[i] {
                        return None;
                    }
                }
            }
        }
        return Some(variable_bindings);
    }

    fn tuple_from_frame(&mut self, t: HashMap<&str, &str>) -> &storage::Tuple {
        // TODO
        Vec::new()
    }
}

pub enum FrameScan<'i> {
    TableScan {
        query: QueryParams,
        scan: storage::TableScan<'i>
    },
    JoinScan {
        left: FrameScan,
        right: FrameScan
    },
    ProjectScan {
        query: QueryParams,
        child: FrameScan
    }
}

impl<'i> Iterator for FrameScan<'i> {
    type Item = HashMap<&'i str, &'i str>;

    fn next(&mut self) -> Option<Self::Item> {
        // TODO
        None
    }

}

pub enum QueryResult<'i> {
    RelationFound {
        query: QueryParams,
        scan: FrameScan<'i>
    },
    NoTableFound
}

impl<'i> Iterator for QueryResult<'i> {
    type Item = &'i storage::Tuple;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            QueryResult::RelationFound { query, scan } =>
                scan.map(|f| query.tuple_from_frame(f)).next(),
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

    fn deconstruct_term(t: ast::Term) -> Result<(String, QueryParams)> {
        match t {
            ast::Term::Atomic(a) => Ok((Self::to_atom(a)?,
                                        QueryParams { params: Vec::new() })),
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
            result.push(Self::to_atom(param)?);
        }
        Ok(result)
    }

    fn scan_from_join_list(joins: Vec<ast::Term>) -> FrameScan {
        // TODO
    }

    fn scan_from_view(v: storage::View) -> FrameScan {
        FrameScan::ProjectScan {
            query: QueryParams { params: v.formals },
            child: scan_from_join_list(view.definition)
        }
    }

    fn scan_from_term(term: ast::Term) -> FrameScan {

    }

    pub fn query(&self, query: ast::Term) -> Result<QueryResult> {
        let (head, rest) = Self::deconstruct_term(query)?;

        self.engine.get_relation(head.as_str()).map(|r| match r {
            Extension(ref table) => Ok(QueryResult::TableFound {
                    query: rest,
                    scan: FrameScan::TableScan {
                        query: rest, scan: table.into_iter()
                    }
                }),
            Intension(view) => Ok(QueryResult::RelationFound {
                    query: rest,
                    scan: Self::scan_from_view(view)
                })
        }).unwrap_or(Ok(QueryResult::NoTableFound))
    }

    pub fn simple_assert(&mut self, fact: ast::Term) -> Result<()> {
        let (head, rest) = Self::deconstruct_term(fact)?;
        let tuple = Self::create_tuple(rest)?;
        match *self.engine.get_or_create_relation(head.clone()) {
            Extension(ref mut t) => Ok(t.assert(tuple)),
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
