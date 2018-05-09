/// Evaluator for dead-simple queries.
/// 
/// Intended mostly as a skeleton while we work out architecture.

use ast;
use error::*;
use storage;
use storage::Relation::*;
use storage::Tuple;

use std::collections::HashMap;
use std::collections::LinkedList;
use std::marker::PhantomData;

//
// Resettable iterators. Necessary for plan nodes.
//

pub trait ResettableIterator: Iterator {
    fn reset(&mut self);
}

//
// Plan data structures.
//

pub trait PlanNode<'a>: ResettableIterator<Item = Frame<'a>> {}
impl<'a, T: ResettableIterator<Item = Frame<'a>>> PlanNode<'a> for T {}

pub trait Plan<'a>: ResettableIterator<Item = Tuple<'a>> {}
impl<'a, T: ResettableIterator<Item = Tuple<'a>>> Plan<'a> for T {}

// This type alias is convenient due to an annoying compiler bug (issue #23856).
pub type Assignments<'a> = Box<PlanNode<'a, Item = Frame<'a>> + 'a>;

// Simple scans over tables and views.

struct ExtensionalScan<'a> {
    table: &'a storage::Table,
    scan: storage::TableScan<'a>
}

impl<'a> ExtensionalScan<'a> {
    fn new(table: &'a storage::Table) -> Self {
        ExtensionalScan {
            table,
            scan: table.into_iter()
        }
    }
}

impl<'a> Iterator for ExtensionalScan<'a> {
    type Item = Tuple<'a>;

    fn next(&mut self) -> Option<Tuple<'a>> {
        self.scan.next()
    }
}

impl<'a> ResettableIterator for ExtensionalScan<'a> {
    fn reset(&mut self) {
        self.scan = self.table.into_iter();
    }
}

struct IntensionalScan<'a> {
    formals: Vec<String>,
    scan: Assignments<'a>
}

impl<'a> IntensionalScan<'a> {
    fn new(formals: Vec<String>, scan: Assignments<'a>) -> Self {
        IntensionalScan {
            formals,
            scan
        }
    }
}

impl<'a> Iterator for IntensionalScan<'a> {
    type Item = Tuple<'a>;

    fn next(&mut self) -> Option<Tuple<'a>> {
        self.scan.next().map(|frame| {
            let mut result: Tuple = Vec::new();
            for f in &self.formals {
                match frame.get(f) {
                    Some(binding) => result.push(binding),
                    None => panic!("frame in view does not match schema")
                };
            }
            result
        })
    }
}

impl<'a> ResettableIterator for IntensionalScan<'a> {
    fn reset(&mut self) {
        self.scan.reset()
    }
}

/// Takes tuples a `Scan` and matches them with the given pattern, returning the
/// assignment of any variables in the pattern to the contents of the tuples.
struct PatternMatch<'a, P: Plan<'a>> {
    pattern: Pattern,
    plan: P,
    _marker: PhantomData<&'a ()>
}

impl<'a, P: Plan<'a>> PatternMatch<'a, P> {
    fn new(pattern: Pattern, plan: P) -> Self {
        PatternMatch {
            pattern,
            plan,
            _marker: PhantomData::default()
        }
    }
}

impl<'a, P: Plan<'a>> Iterator for PatternMatch<'a, P> {
    type Item = Frame<'a>;

    fn next(&mut self) -> Option<Frame<'a>> {
        self.plan.next().and_then(|t| self.pattern.match_tuple(t))
    }
}

impl<'a, P: Plan<'a>> ResettableIterator for PatternMatch<'a, P> {
    fn reset(&mut self) {
        self.plan.reset();
    }
}

struct Join<'a> {
    left: Assignments<'a>,
    right: Assignments<'a>,
    current_left: Option<Frame<'a>>
}

impl<'a> Join<'a> {
    fn new(left: Assignments<'a>, right: Assignments<'a>) -> Join<'a> {
        Join {
            left,
            right,
            current_left: None
        }
    }
}

impl<'a> Iterator for Join<'a> {
    type Item = Frame<'a>;

    fn next(&mut self) -> Option<Frame<'a>> {
        loop {
            match self.right.next() {
                None => {
                    self.right.reset();
                    if let Some(r) = self.right.next() {

                    match self.left.next() {
                        None => {
                            self.current_left = None;
                            return None;
                        },
                        Some(l) => {
                            self.current_left = Some(l.clone());
                            if let Some(result) = merge_frames(&l, &r) {
                                return Some(result);
                            };
                        }
                    }
                    } else {
                        return None;
                    }
                },
                Some(r) => {
                    if let Some(ref l) = self.current_left {
                        if let Some(result) = merge_frames(&l, &r) {
                            return Some(result);
                        }
                    } else {
                        // Left iterator hasn't been advanced
                        let l = self.left.next()?.clone();
                        self.current_left = Some(l.clone());
                        if let Some(result) = merge_frames(&l, &r) {
                            return Some(result);
                        }
                    }
                }
            }
        }
    }
}

impl<'a> ResettableIterator for Join<'a> {
    fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
        self.current_left = None;
    }
}

//
// Frames and pattern matching.
//

pub type Frame<'a> = HashMap<String, &'a str>;

#[derive(Debug)]
pub struct Pattern {
    params: Vec<ast::AtomicTerm>
}

impl Pattern {
    /* Check if the given tuple can match with the query parameters, and
     * return a map of variable bindings.
     */
    fn match_tuple<'a>(&mut self, t: storage::Tuple<'a>) -> Option<Frame<'a>> {
        // Ensure each variable is bound to exactly one atom
        let mut variable_bindings: HashMap<String, &str> = HashMap::new();

        for i in 0..self.params.len() {
            match self.params[i] {
                ast::AtomicTerm::Atom(ref s) => {
                    if *s != t[i] {
                        return None;
                    }
                },
                ast::AtomicTerm::Variable(ref s) => {
                    let binding = variable_bindings.entry(s.to_string())
                        .or_insert(t[i]);
                    if *binding != t[i] {
                        return None;
                    }
                }
            }
        }
        return Some(variable_bindings);
    }
}

fn merge_frames<'a>(f1: &Frame<'a>, f2: &Frame<'a>) -> Option<Frame<'a>> {
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


fn to_atom(t: ast::AtomicTerm) -> Result<String> {
    match t {
        ast::AtomicTerm::Atom(s) => Ok(s),
        ast::AtomicTerm::Variable(v) =>
            Err(Error::MalformedLine(format!("unexpected variable: {}", v)))
    }
}


fn deconstruct_term(t: ast::Term) -> Result<(String, Pattern)> {
    match t {
        ast::Term::Atomic(a) => Ok((to_atom(a)?,
                                    Pattern { params :Vec::new() })),
        ast::Term::Compound(cterm) => {
            let mut rest = Vec::new();
            for param in cterm.params.into_iter() {
                rest.push(param);
            }
            Ok((cterm.relation, Pattern { params: rest }))
        }
    }
}

fn create_fact<'a>(p: Pattern) -> Result<Vec<String>> {
    let mut result = Vec::new();
    for param in p.params {
        result.push(to_atom(param)?);
    }
    Ok(result)
}

fn plan_joins<'a>(engine: &'a storage::StorageEngine,
                  mut joins: LinkedList<ast::Term>) -> Result<Assignments<'a>> {
    let head = joins.pop_front();
    match head {
        None => Err(Error::MalformedLine("Empty Join list".to_string())),
        Some(term) => {
            let head_term: Assignments<'a> = scan_from_term(engine, term)?;
            if joins.len() == 0 {
                Ok(head_term)
            } else {
                let rest_term: Assignments<'a> = plan_joins(engine, joins)?;
                Ok(Box::new(Join::new(head_term, rest_term)))
            }
        }
    }
}

fn plan_view<'a>(engine: &'a storage::StorageEngine,
                 v: &storage::View) -> Result<Assignments<'a>> {
    let mut joins = LinkedList::new();
    // TODO - don't clone this whole list
    for term in &v.definition[0] {
        joins.push_back(term.clone());
    }
    plan_joins(engine, joins)
}

fn to_variables(terms: Vec<ast::AtomicTerm>) -> Result<Vec<String>> {
    let err_msg = "atom appeared as view parameter";
    terms.into_iter().map(|t| match t {
        ast::AtomicTerm::Atom(_) =>
            Err(Error::MalformedLine(err_msg.to_string())),
        ast::AtomicTerm::Variable(v) => Ok(v)
    }).collect()
}

pub fn scan_from_term<'a>(engine: &'a storage::StorageEngine,
                          query: ast::Term) -> Result<Assignments<'a>> {
    let (head, rest) = deconstruct_term(query)?;

    let relation =
        engine.get_relation(head.as_str())
              .ok_or(Error::MalformedLine(format!("No relation found.")))?;
    match relation {
        Extension(ref table) => {
            let scan = ExtensionalScan::new(table);
            Ok(Box::new(PatternMatch::new(rest, scan)))
        },
        Intension(view) => {
            let formals = to_variables(view.formals.clone())?;
            let view_plan = plan_view(engine, &view)?;
            let scan = IntensionalScan::new(formals, view_plan);
            Ok(Box::new(PatternMatch::new(rest, scan)))
        }
    }
}

pub fn simple_assert(engine: &mut storage::StorageEngine,
                     fact: ast::Term) -> Result<()> {
    let (head, rest) = deconstruct_term(fact)?;
    let tuple = create_fact(rest)?;
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
