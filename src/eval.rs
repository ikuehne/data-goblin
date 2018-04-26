/// Evaluator for dead-simple queries.
/// 
/// Intended mostly as a skeleton while we work out architecture.

use ast;
use error::*;
use std::collections::HashMap;
use std::collections::LinkedList;
use storage;
use storage::Relation::*;

pub struct Evaluator {
    engine: storage::StorageEngine
}

pub struct QueryParams {
    params: Vec<ast::AtomicTerm>
}

pub type Frame = HashMap<String, String>;

impl QueryParams {
    
    /* Check if the given tuple can match with the query parameters, and
     * return a map of variable bindings.
     */
    fn match_tuple<'a>(&'a mut self, t: &'a storage::Tuple)
        -> Option<Frame> {

        // Ensure each variable is bound to exactly one atom
        let mut variable_bindings: HashMap<String, String> = HashMap::new();

        for i in 0..self.params.len() {
            match self.params[i] {
                ast::AtomicTerm::Atom(ref s) => {
                    if *s != t[i] {
                        return None;
                    }
                },
                ast::AtomicTerm::Variable(ref s) => {
                    let binding = variable_bindings.entry(s.to_string())
                        .or_insert(t[i].clone());
                    if *binding != t[i] {
                        return None;
                    }
                }
            }
        }
        return Some(variable_bindings);
    }
}

/// A `FrameScan` is a struct that produces frames.
pub enum FrameScan<'i> {
    /// A `Binder` takes tuples from a QueryResult and binds them according
    /// to the given query, producing frames.
    Binder {
        query: QueryParams,
        scan: Box<RelationScan<'i>>
    },
    /// A `JoinScan` performs a cross join on its two child FrameScans.
    /// For each Frame on the left, it looks at each Frame on the right, and if
    /// the variable bindings match, it produces a combined Frame.
    JoinScan {
        left: Box<FrameScan<'i>>,
        right: Box<FrameScan<'i>>,
        current_left: Option<Frame>
    }
}

impl<'i> FrameScan<'i> {

    fn reset(&mut self) {
        match self {
            FrameScan::Binder { query, scan } => scan.reset(),
            FrameScan::JoinScan { left, right, ref mut current_left } => {
                left.reset();
                right.reset();
                *current_left = None;
            }
        }
    }

}

fn merge_frames<'i>(f1: &'i Frame, f2: &'i Frame) -> Frame {
    HashMap::new()
}

impl<'i> Iterator for FrameScan<'i> {
    type Item = Frame;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            FrameScan::Binder { query, scan } => scan.next()
                .and_then(|t| query.match_tuple(t)),
            FrameScan::JoinScan { left, right, ref mut current_left } => match right.next() {
                None => {
                    right.reset();
                    let r = right.next();
                    let result = r.and_then(|right|
                        match left.next() {
                            None => {
                                *current_left = None;
                                None
                            },
                            Some(l) => {
                                *current_left = Some(l.clone());
                                Some(merge_frames(&l, &right))
                            }
                        });
                    return result;
                    },
                Some(r) => match current_left {
                    Some(l) => Some(merge_frames(&l, &r)),
                    None => None
                }
            }
        }
    }

}

pub enum RelationScan<'i> {
    Extensional {
        scan: storage::TableScan<'i>
    },
    Intensional {
        formals: Vec<ast::AtomicTerm>,
        scan: FrameScan<'i>
    },
    NoTableFound
}

impl<'i> RelationScan<'i> {

    fn reset(&mut self) {

        match self {
            RelationScan::Extensional { scan } => {
                scan.into_iter();
            },
            RelationScan::Intensional { formals, scan } => {
                scan.reset();
            },
            RelationScan::NoTableFound => ()
        }
    }

    fn tuple_from_frame(formals: Vec<ast::AtomicTerm>, frame: Option<Frame>)
        -> Option<&'i storage::Tuple> {
        // TODO
        None
    }

}

impl<'i> Iterator for RelationScan<'i> {
    type Item = &'i storage::Tuple;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            RelationScan::Extensional { scan } =>
                scan.next(),
            RelationScan::Intensional { formals, scan } =>
                Self::tuple_from_frame(formals.clone(), scan.next()),
            RelationScan::NoTableFound => None
        }
    }
}

impl<'i> Evaluator {
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

    fn scan_from_join_list(&self, mut joins: LinkedList<ast::Term>) -> Result<FrameScan<'i>> {

        let head = joins.pop_front();
        match head {
            None => Err(Error::MalformedLine("Empty Join list".to_string())),
            Some(term) => {
                let head_term = self.scan_from_term(term)?;
                if joins.len() == 0 {
                    Ok(head_term)
                } else {
                    let rest_scan = self.scan_from_join_list(joins)?;
                    Ok(FrameScan::JoinScan {
                        left: Box::new(head_term),
                        right: Box::new(rest_scan),
                        current_left: None
                    })
                }
            }
        }
    }

    fn scan_from_view(&self, v: & storage::View) -> Result<FrameScan<'i>> {
        let mut joins = LinkedList::new();
        // TODO - don't clone this whole list
        for term in &v.definition {
            joins.push_back(term.clone());
        }
        self.scan_from_join_list(joins)
        
    }

    pub fn scan_from_term(&self, query: ast::Term) -> Result<FrameScan<'i>> {
        let (head, rest) = Self::deconstruct_term(query)?;

        let relation = self.engine.get_relation(head.as_str());
        let scan = match relation {
            Some(Extension(ref table)) => Some(RelationScan::Extensional {
                    scan: table.into_iter()
                }),
            Some(Intension(view)) =>
                match self.scan_from_view(&view) {
                    Err(_) => None,
                    Ok(s) => Some(RelationScan::Intensional {
                        formals: view.formals.clone(),
                        scan: s
                    })
                },
            None => None
        };

        match scan {
            None => Err(Error::MalformedLine(format!("No relation found."))),
            Some(scan) => {
                Ok(FrameScan::Binder {
                    query: rest,
                    scan: Box::new(scan)
                })
            }
        }
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
