/// Evaluator for dead-simple queries.
/// 
/// Intended mostly as a skeleton while we work out architecture.

use ast;
use error::*;
use std::collections::HashMap;
use std::collections::LinkedList;
use std::collections::HashSet;
use storage;
use storage::Relation::*;

#[derive(Debug)]
pub struct QueryParams {
    params: Vec<ast::AtomicTerm>
}

pub type Frame = HashMap<String, String>;

pub type OwnedTuple = Vec<String>;

impl QueryParams {
    
    /* Check if the given tuple can match with the query parameters, and
     * return a map of variable bindings.
     */
    fn match_tuple<'a>(&'a mut self, t: storage::Tuple<'a>)
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

#[derive(Debug)]
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

fn merge_frames<'i>(f1: &'i Frame, f2: &'i Frame) -> Option<Frame> {

    //println!("{:?} merging with {:?}", f1, f2);
    // TODO - don't copy these
    let mut result = HashMap::new();
    for (var, binding1) in f1 {
        match f2.get(var) {
            Some(binding2) => { 
                if binding1 != binding2 {
                    return None;
                } else {
                    result.insert(var.clone(), binding1.clone());
                }
            }
            None => {
                result.insert(var.clone(), binding1.clone());
            }
        };
    }

    for (var, binding2) in f2 {
        match f1.get(var) {
            Some(binding1) => { 
                if binding1 != binding2 {
                    return None;
                } else {
                    result.insert(var.clone(), binding2.clone());
                }
            }
            None => {
                result.insert(var.clone(), binding2.clone());
            }
        };
    }

    return Some(result);
   
}

impl<'i> Iterator for FrameScan<'i> {
    type Item = Frame;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            FrameScan::Binder { query, scan } => loop {
                let t = scan.next()?;
                match query.match_tuple(&t) {
                    None => (),
                    Some(frame) => { return Some(frame); }
                };
            },
            FrameScan::JoinScan { left, right, ref mut current_left } => loop {
                match right.next() {
                    None => {
                        //println!("resetting right iterator");
                        right.reset();
                        if let Some(r) = right.next() {
                            
                        match left.next() {
                            None => {
                                //println!("left iterator is over");
                                *current_left = None;
                                return None;
                            },
                            Some(l) => {
                                *current_left = Some(l.clone());
                                if let Some(result) = merge_frames(&l, &r) {
                                    return Some(result);
                                };
                            }
                        }
                        } else {
                            //println!("right iterator returns none after reset");
                            return None;
                        }
                    },
                    Some(r) => {
                        if let Some(l) = current_left {
                            if let Some(result) = merge_frames(l, &r) {
                                return Some(result);
                            }
                        } else {
                            // Left iterator hasn't been advanced
                            let l = left.next()?.clone();
                            *current_left = Some(l.clone());
                            if let Some(result) = merge_frames(&l, &r) {
                                return Some(result);
                            }
                        }
                    }
                }
            }
        }
    }

}

#[derive(Debug)]
pub enum RelationScan<'i> {
    Extensional {
        table: &'i storage::Table,
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
            RelationScan::Extensional { table, ref mut scan } => {
                *scan = table.into_iter();
            },
            RelationScan::Intensional { formals, scan } => {
                scan.reset();
            },
            RelationScan::NoTableFound => ()
        }
    }

    fn tuple_from_frame(formals: Vec<ast::AtomicTerm>, frame: Option<Frame>)
        -> Option<OwnedTuple> {
        if let Some(frame) = frame {
            let mut result = Vec::new();
            for f in formals {
                match f {
                    ast::AtomicTerm::Variable(v) => {
                        match frame.get(&v) {
                            Some(binding) => result.push(binding.clone()),
                            None => return None
                        };
                    }
                    ast::AtomicTerm::Atom(a) => {
                        result.push(a.clone());
                    }
                }
            }
            Some(result)
        }
        else {
            None
        }
    }

}

impl<'i> Iterator for RelationScan<'i> {
    type Item = OwnedTuple;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            RelationScan::Extensional { table, scan } =>
                scan.next().map(|t| t.to_vec()),
            RelationScan::Intensional { formals, scan } =>
                Self::tuple_from_frame(formals.clone(), scan.next()),
            RelationScan::NoTableFound => None
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

fn create_tuple(p: QueryParams) -> Result<OwnedTuple> {
    let mut result = Vec::new();
    for param in p.params {
        result.push(to_atom(param)?);
    }
    Ok(result)
}

fn scan_from_join_list(engine: &storage::StorageEngine,
                       mut joins: LinkedList<ast::Term>)
        -> Result<FrameScan> {

    let head = joins.pop_front();
    match head {
        None => Err(Error::MalformedLine("Empty Join list".to_string())),
        Some(term) => {
            let head_term = scan_from_term(engine, term)?;
            if joins.len() == 0 {
                Ok(head_term)
            } else {
                let rest_scan = scan_from_join_list(engine, joins)?;
                Ok(FrameScan::JoinScan {
                    left: Box::new(head_term),
                    right: Box::new(rest_scan),
                    current_left: None
                })
            }
        }
    }
}

fn scan_from_view<'i>(engine: &'i storage::StorageEngine,
                  v: &storage::View) -> Result<FrameScan<'i>> {
    let mut joins = LinkedList::new();
    // TODO - don't clone this whole list
    for term in &v.definition[0] {
        joins.push_back(term.clone());
    }
    scan_from_join_list(engine, joins)
    
}

pub fn scan_from_term(engine: &storage::StorageEngine,
                      query: ast::Term) -> Result<FrameScan> {
    let (head, rest) = deconstruct_term(query)?;

    let relation = engine.get_relation(head.as_str());
    let scan = match relation {
        Some(Extension(ref table)) => Some(RelationScan::Extensional {
                table: table,
                scan: table.into_iter()
            }),
        Some(Intension(view)) =>
            match scan_from_view(engine, &view) {
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

pub fn simple_assert(engine: &mut storage::StorageEngine,
                     fact: ast::Term) -> Result<()> {
    let (head, rest) = deconstruct_term(fact)?;
    let tuple = create_tuple(rest)?;
    let arity = tuple.len();
    let relation = storage::Relation::Extension(storage::Table::new(arity));
    match *engine.get_or_create_relation(head.clone(), relation) {
        Extension(ref mut t) => t.assert(tuple),
        Intension(_) => Err(Error::NotExtensional(head))
    }
}

pub fn add_rule_to_view(engine: &mut storage::StorageEngine,
                        rule: ast::Rule) -> Result<()> {
    let (name, definition) = deconstruct_term(rule.head)?;
    let relation = storage::Relation::Intension(
        storage::View { formals: definition.params, definition: Vec::new() }
    );
    let mut rel_view = engine.get_or_create_relation(name.clone(), relation);
    match *rel_view {
        Extension(_) => Err(Error::NotIntensional(name)),
        Intension(ref mut view) => Ok(view.definition.push(rule.body))
    }
}

pub fn assert(engine: &mut storage::StorageEngine,
              fact: ast::Rule) -> Result<()> {
    if fact.body.len() == 0 {
        simple_assert(engine, fact.head)
    } else {
        add_rule_to_view(engine, fact)
    }
}
