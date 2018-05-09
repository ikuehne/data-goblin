/// Evaluator for dead-simple queries.

use ast;
use error::*;
use storage;
use storage::Relation::*;
use storage::Tuple;

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::collections::LinkedList;
use std::marker::PhantomData;

/// Plans are simply iterators that can be reset to the beginning.
pub trait Plan: Iterator {
    /// Sets the plan back to the beginning.
    /// 
    /// I.e., the next call to `next` should return the same thing as when the
    /// plan was first created.
    fn reset(&mut self);
}

//
// TuplePlans.
//

/// Plans that return tuples.
pub trait TuplePlan<'a>: Plan<Item = Tuple<'a>> {}
impl<'a, T: Plan<Item = Tuple<'a>>> TuplePlan<'a> for T {}

/// A (resetable) scan over an extensional relation.
struct ExtensionalScan<'a> {
    table: &'a storage::Table,
    scan: storage::TableScan<'a>
}

impl<'a> ExtensionalScan<'a> {
    /// Create a new ExtensionalScan staring at the beginning of this table.
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

impl<'a> Plan for ExtensionalScan<'a> {
    fn reset(&mut self) {
        self.scan = self.table.into_iter();
    }
}

/// A (resetable) scan over an intensional relation.
struct IntensionalScan<'a> {
    column_names: Vec<String>,
    scan: Frames<'a>
}

impl<'a> IntensionalScan<'a> {
    /// Create a new scan based on the given view definition, running against
    /// the given storage engine.
    fn new(engine: &'a storage::StorageEngine, view: &'a storage::View)
            -> Result<Self> {
        let joins = view.definition[0].clone().into_iter().collect();
        let scan = plan_joins(engine, joins)?;
        let column_names = to_variables(view.formals.clone())?;

        Ok(IntensionalScan {
            column_names,
            scan
        })
    }
}

impl<'a> Iterator for IntensionalScan<'a> {
    type Item = Tuple<'a>;

    fn next(&mut self) -> Option<Tuple<'a>> {
        self.scan.next().map(|frame| {
            (&self.column_names).into_iter().map(|v| {
                *frame.get(v).unwrap_or_else(|| {
                    panic!("frame in view plan missing a column")
                })
            }).collect()
        })
    }
}

impl<'a> Plan for IntensionalScan<'a> {
    fn reset(&mut self) {
        self.scan.reset()
    }
}

//
// FramePlans.
//

/// Plans that return frames.
pub trait FramePlan<'a>: Plan<Item = Frame<'a>> {}
impl<'a, T: Plan<Item = Frame<'a>>> FramePlan<'a> for T {}

// This type alias is convenient due to an annoying compiler bug (issue #23856).
// Just represents a trait object for a FramePlan with the given storage
// lifetime. The additional `+ 'a` is necessary because trait objects lose
// lifetime information.
pub type Frames<'a> = Box<FramePlan<'a, Item = Frame<'a>> + 'a>;

/// Takes tuples a `Scan` and matches them with the given pattern, returning the
/// assignment of any variables in the pattern to the contents of the tuples.
struct PatternMatch<'a, P: TuplePlan<'a>> {
    pattern: Pattern,
    child: P,
    _marker: PhantomData<&'a ()>
}

impl<'a, P: TuplePlan<'a>> PatternMatch<'a, P> {
    fn new(pattern: Pattern, child: P) -> Self {
        PatternMatch {
            pattern,
            child,
            _marker: PhantomData::default()
        }
    }
}

impl<'a, P: TuplePlan<'a>> Iterator for PatternMatch<'a, P> {
    type Item = Frame<'a>;

    fn next(&mut self) -> Option<Frame<'a>> {
        loop {
            let t = self.child.next()?;

            if let Some(f) = self.pattern.match_tuple(t) {
                return Some(f);
            }
        }
    }
}

impl<'a, P: TuplePlan<'a>> Plan for PatternMatch<'a, P> {
    fn reset(&mut self) {
        self.child.reset();
    }
}

/// Represents a cross join between two FramePlans.
struct Join<'a> {
    left: Frames<'a>,
    right: Frames<'a>,
    /// Where are we currently in the left scan? `None` if we haven't started.
    current_left: Option<Frame<'a>>
}

impl<'a> Join<'a> {
    fn new(left: Frames<'a>, right: Frames<'a>) -> Join<'a> {
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
                    let r = self.right.next()?;
                    let l = self.left.next()?;
                    self.current_left = Some(l.clone());
                    if let Some(result) = merge_frames(&l, &r) {
                        return Some(result);
                    };
                },
                Some(r) => {
                    if let Some(ref l) = self.current_left {
                        if let Some(result) = merge_frames(&l, &r) {
                            return Some(result);
                        }
                    } else {
                        // Left iterator hasn't been advanced
                        let l = self.left.next()?;
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

impl<'a> Plan for Join<'a> {
    fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
        self.current_left = None;
    }
}

struct BottomUp<'a> {
    all_frames: Vec<Frame<'a>>,
    index: usize
}

impl<'a> BottomUp<'a> {
    fn new(name: String, formals: Vec<String>,
           base_scans: Vec<Frames<'a>>, recursive_rules: Vec<Vec<ast::Term>>,
           engine: &'a storage::StorageEngine) -> Result<BottomUp<'a>> {
        let mut all_frames = HashSet::new();

        for scan in base_scans {
            for frame in scan {
                all_frames.insert(frame);
            }
        }

        // Now, repeatedly apply recursive rules.
        let mut new_tuple = false;
        while new_tuple {
            new_tuple = false;
            for rule in &recursive_rules {
                // Apply the given rule and see if we get any new tuples
                let scan = plan_recursive_rule(engine,
                                               &name,
                                               &rule,
                                               &formals,
                                               &all_frames)?;
                let mut new_frames = Vec::new();
                for frame in scan {
                    if !all_frames.contains(&frame) {
                        new_tuple = true;
                        new_frames.push(frame);
                    }
                }
                for frame in new_frames {
                    all_frames.insert(frame);
                }
            }
        }

        Ok(BottomUp { all_frames: all_frames.into_iter().collect(), index: 0 })
    }
}

impl<'a> Iterator for BottomUp<'a> {
    type Item = Frame<'a>;

    fn next(&mut self) -> Option<Frame<'a>> {
        let result = self.all_frames.get(self.index);
        self.index += 1;
        return result.map(|frame| frame.clone());
    }
}

impl<'a> Plan for BottomUp<'a> {
    fn reset(&mut self) {
        self.index = 0;
    }
}

//
// Frames and pattern matching.
//

pub type Frame<'a> = BTreeMap<String, &'a str>;

#[derive(Debug)]
struct Pattern {
    params: Vec<ast::AtomicTerm>
}

impl Pattern {
    fn new(params: Vec<ast::AtomicTerm>) -> Self {
        Pattern { params }
    }

    /// Match the tuple against this pattern, returning the variable bindings
    /// that make the match.
    /// 
    /// Return `None` if the given tuple does not match this pattern.
    fn match_tuple<'a>(&mut self, t: storage::Tuple<'a>) -> Option<Frame<'a>> {
        // Ensure each variable is bound to exactly one atom
        let mut variable_bindings = BTreeMap::new();

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
    let mut result = BTreeMap::new();
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

//
// Query planning.
//

/// Plan a cross join over arbitrarily many terms.
fn plan_joins<'a>(engine: &'a storage::StorageEngine,
                  mut joins: LinkedList<ast::Term>) -> Result<Frames<'a>> {
    let head = joins.pop_front();
    match head {
        None => Err(Error::MalformedLine("Empty Join list".to_string())),
        Some(term) => {
            let head_term: Frames<'a> = query(engine, term)?;
            if joins.len() == 0 {
                Ok(head_term)
            } else {
                let rest_term: Frames<'a> = plan_joins(engine, joins)?;
                Ok(Box::new(Join::new(head_term, rest_term)))
            }
        }
    }
}

fn plan_recursive_rule<'b>(
        engine: &'b storage::StorageEngine,
        name: &str,
        rule: &[ast::Term],
        formals: &[String],
        all_frames: &HashSet<Frame>) -> Result<Frames<'b>> {
    panic!("undefined")
}


/// Given a query, return all variable assignments over the database that
/// satisfy that query.
pub fn query<'a>(engine: &'a storage::StorageEngine,
                 query: ast::Term) -> Result<Frames<'a>> {
    let (head, rest) = deconstruct_term(query)?;

    let relation =
        engine.get_relation(head.as_str())
              .ok_or(Error::MalformedLine(format!("No relation found.")))?;
    match relation {
        Extension(ref table) => {
            let scan = ExtensionalScan::new(table);
            Ok(Box::new(PatternMatch::new(Pattern::new(rest), scan)))
        },
        Intension(view) => {
            let scan = IntensionalScan::new(engine, view)?;
            Ok(Box::new(PatternMatch::new(Pattern::new(rest), scan)))
        }
    }
}

//
// Modifying the database.
//

/// Add a simple fact (one with no variables) to the database.
fn simple_assert(engine: &mut storage::StorageEngine,
                 fact: ast::Term) -> Result<()> {
    let (head, rest) = deconstruct_term(fact)?;
    let tuple = to_atoms(rest)?;
    let arity = tuple.len();
    let relation = storage::Relation::Extension(storage::Table::new(arity));
    match *engine.get_or_create_relation(head.clone(), relation) {
        Extension(ref mut t) => t.assert(tuple),
        Intension(_) => Err(Error::NotExtensional(head))
    }
}

fn add_rule_to_view(engine: &mut storage::StorageEngine,
                    rule: ast::Rule) -> Result<()> {
    let (name, definition) = deconstruct_term(rule.head)?;
    let relation = storage::Relation::Intension(
        storage::View { formals: definition, definition: Vec::new() }
    );
    let mut rel_view = engine.get_or_create_relation(name.clone(), relation);
    match *rel_view {
        Extension(_) => Err(Error::NotIntensional(name)),
        Intension(ref mut view) => Ok(view.definition.push(rule.body))
    }
}

/// Add a fact or rule to the database.
pub fn assert(engine: &mut storage::StorageEngine,
              fact: ast::Rule) -> Result<()> {
    if fact.body.len() == 0 {
        simple_assert(engine, fact.head)
    } else {
        add_rule_to_view(engine, fact)
    }
}

//
// Processing queries.
//

/// Attempt to convert an AtomicTerm to an atom.
fn to_atom(t: ast::AtomicTerm) -> Result<String> {
    match t {
        ast::AtomicTerm::Atom(a) => Ok(a),
        ast::AtomicTerm::Variable(v) =>
            Err(Error::MalformedLine(format!("unexpected variable: {}", v)))
    }
}

/// Attempt to convert an AtomicTerm to a variable.
fn to_variable(t: ast::AtomicTerm) -> Result<String> {
    match t {
        ast::AtomicTerm::Atom(a) =>
            Err(Error::MalformedLine(format!("unexpected atom: {}", a))),
        ast::AtomicTerm::Variable(v) => Ok(v)
    }
}

/// Convert a vector of AtomicTerms to atoms, failing if any are variables.
fn to_atoms(v: Vec<ast::AtomicTerm>) -> Result<Vec<String>> {
    v.into_iter().map(to_atom).collect()
}

/// Convert a vector of AtomicTerms to variables, failing if any are atoms.
fn to_variables(v: Vec<ast::AtomicTerm>) -> Result<Vec<String>> {
    v.into_iter().map(to_variable).collect()
}

/// Deconstruct a term into a head and its parameters.
/// 
/// Fails if the term is not compound.
fn deconstruct_term(t: ast::Term) -> Result<(String, Vec<ast::AtomicTerm>)> {
    match t {
        ast::Term::Atomic(a) => Ok((to_atom(a)?, Vec::new())),
        ast::Term::Compound(cterm) => Ok((cterm.relation, cterm.params))
    }
}
