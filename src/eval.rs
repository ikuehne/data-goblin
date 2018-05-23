/// Evaluator for dead-simple queries.

use ast;
use error::*;
use storage;
use storage::Relation::*;
use storage::Tuple;

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::collections::hash_set;
use std::collections::LinkedList;

/// Plans are simply iterators that can be reset to the beginning.
pub trait Plan: Iterator {
    /// Sets the plan back to the beginning.
    /// 
    /// I.e., the next call to `next` should return the same thing as when the
    /// plan was first created.
    fn reset(&mut self);
}

//
// Views.
//

/// An `AstView` represents a view simply as the AST of each of its rules.
#[derive(Serialize, Deserialize)]
pub struct AstView {
    pub rules: Vec<(Vec<String>, Vec<ast::Term>)>
}

type Storage = storage::StorageEngine<AstView>;

//
// TuplePlans.
//

/// Plans that return tuples.
pub trait TuplePlan<'a>: Plan<Item = Tuple<'a>> {}
impl<'a, T: Plan<Item = Tuple<'a>>> TuplePlan<'a> for T {}

pub type Tuples<'s, 'a> = Box<TuplePlan<'s, Item = Tuple<'s>> + 'a>;

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
struct IntensionalScan<'s: 'a, 'a> {
    column_names: Vec<String>,
    scan: Frames<'s, 'a>
}

impl<'s: 'a, 'a> IntensionalScan<'s, 'a> {
    /// Create a new scan based on the given view definition, running against
    /// the given storage engine.
    fn from_view(name: &str,
                 engine: &'s Storage,
                 view: &'s AstView) -> Result<Tuples<'s, 's>> {
        let mut recursive = false;
        let mut base_scans: Vec<Tuples<'s, 's>> = Vec::new();
        let mut recursive_rules = Vec::new();
        for (params, rule) in &view.rules {
            if is_recursive(name, rule.to_vec())? {
                recursive = true;
                recursive_rules.push((params.clone(), rule.clone()));
            } else {
                let mut joins = LinkedList::new();
                for term in rule {
                    joins.push_back(query(engine, term.clone())?);
                }
                let join = plan_joins(joins);
                base_scans.push(Box::new(IntensionalScan::new(params.to_vec(),
                                                              join)));
            }
        }

        Ok(if recursive {
            Box::new(BottomUp::new(name, base_scans, recursive_rules, engine)?)
        } else {
            Box::new(Chain::new(base_scans))
        })
    }

    fn new(column_names: Vec<String>,
           scan: Frames<'s, 'a>) -> IntensionalScan<'s, 'a> {
        IntensionalScan { column_names, scan }
    }
}

impl<'s: 'a, 'a> Iterator for IntensionalScan<'s, 'a> {
    type Item = Tuple<'s>;

    fn next(&mut self) -> Option<Tuple<'s>> {
        self.scan.next().map(|frame| {
            (&self.column_names).into_iter().map(|v| {
                *frame.get(v).unwrap_or_else(|| {
                    panic!("frame in view plan missing a column")
                })
            }).collect()
        })
    }
}

impl<'s: 'a, 'a> Plan for IntensionalScan<'s, 'a> {
    fn reset(&mut self) {
        self.scan.reset()
    }
}

struct BottomUp<'s> {
    all_tuples: Vec<Tuple<'s>>,
    index: usize
}

impl<'s> BottomUp<'s> {
    fn new(name: &str, base_scans: Vec<Tuples<'s, 's>>,
           recursive_rules: Vec<(Vec<String>, Vec<ast::Term>)>,
           engine: &'s Storage) -> Result<BottomUp<'s>> {
        let mut all_tuples = HashSet::new();

        for scan in base_scans {
            for tuple in scan {
                all_tuples.insert(tuple);
            }
        }

        // Now, repeatedly apply recursive rules.
        let mut new_tuple = true;
        while new_tuple {
            new_tuple = false;
            for (formals, rule) in &recursive_rules {
                let mut new_tuples = Vec::new();
                {
                    // Apply the given rule and see if we get any new tuples
                    let scan = plan_recursive_rule(engine,
                                                   name,
                                                   &rule,
                                                   &formals,
                                                   &all_tuples)?;
                    for tuple in scan {
                        if !all_tuples.contains(&tuple) {
                            new_tuple = true;
                            new_tuples.push(tuple);
                        }
                    }
                }
                for tuple in new_tuples {
                    all_tuples.insert(tuple);
                }
            }
        }

        Ok(BottomUp { all_tuples: all_tuples.into_iter().collect(), index: 0 })
    }
}

impl<'s> Iterator for BottomUp<'s> {
    type Item = Tuple<'s>;

    fn next(&mut self) -> Option<Tuple<'s>> {
        let result = self.all_tuples.get(self.index);
        self.index += 1;
        return result.map(|t| t.clone());
    }
}

impl<'s> Plan for BottomUp<'s> {
    fn reset(&mut self) {
        self.index = 0;
    }
}

struct SetNode<'s: 'a, 'a> {
    tuples: &'a HashSet<Tuple<'s>>,
    iterator: hash_set::Iter<'a, Tuple<'s>>
}

impl<'s: 'a, 'a> SetNode<'s, 'a> {
    fn new(tuples: &'a HashSet<Tuple<'s>>) -> SetNode<'s, 'a> {
        SetNode { tuples: tuples, iterator: tuples.into_iter() }
    }
}

impl<'s, 'a> Iterator for SetNode<'s, 'a> {
    type Item = Tuple<'s>;

    fn next(&mut self) -> Option<Tuple<'s>> {
        self.iterator.next().map(|t| t.clone())
    }
}

impl<'s, 'a> Plan for SetNode<'s, 'a> {
    fn reset(&mut self) {
        self.iterator = self.tuples.into_iter();
    }
}

struct Chain<'s: 'a, 'a> {
    parts: Vec<Tuples<'s, 'a>>,
    current: usize
}

impl<'s: 'a, 'a> Chain<'s, 'a> {
    fn new(parts: Vec<Tuples<'s, 'a>>) -> Chain<'s, 'a> {
        Chain { parts, current: 0 }
    }
}

impl<'s: 'a, 'a> Iterator for Chain<'s, 'a> {
    type Item = Tuple<'s>;

    fn next(&mut self) -> Option<Tuple<'s>> {
        loop {
            if self.current == self.parts.len() {
                return None;
            }
            match self.parts[self.current].next() {
                None => { self.current += 1; },
                Some(t) => { return Some(t); }
            }
        }
    }
}

impl<'s: 'a, 'a> Plan for Chain<'s, 'a> {
    fn reset(&mut self) {
        for mut scan in &mut self.parts {
            scan.reset();
        }
        self.current = 0;
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
pub type Frames<'s, 'a> = Box<FramePlan<'s, Item = Frame<'s>> + 'a>;

/// Takes tuples a `Scan` and matches them with the given pattern, returning the
/// assignment of any variables in the pattern to the contents of the tuples.
struct PatternMatch<'s: 'a, 'a> {
    pattern: Pattern,
    child: Tuples<'s, 'a>,
}

impl<'s: 'a, 'a> PatternMatch<'s, 'a> {
    fn new(pattern: Pattern, child: Tuples<'s, 'a>) -> Self {
        PatternMatch {
            pattern,
            child,
        }
    }
}

impl<'s: 'a, 'a> Iterator for PatternMatch<'s, 'a> {
    type Item = Frame<'s>;

    fn next(&mut self) -> Option<Frame<'s>> {
        loop {
            let t = self.child.next()?;

            if let Some(f) = self.pattern.match_tuple(t) {
                return Some(f);
            }
        }
    }
}

impl<'s: 'a, 'a> Plan for PatternMatch<'s, 'a> {
    fn reset(&mut self) {
        self.child.reset();
    }
}

/// Represents a cross join between two FramePlans.
struct Join<'s: 'a, 'a> {
    left: Frames<'s, 'a>,
    right: Frames<'s, 'a>,
    /// Where are we currently in the left scan? `None` if we haven't started.
    current_left: Option<Frame<'s>>
}

impl<'s: 'a, 'a> Join<'s, 'a> {
    fn new(left: Frames<'s, 'a>, right: Frames<'s, 'a>) -> Join<'s, 'a> {
        Join {
            left,
            right,
            current_left: None
        }
    }
}

impl<'s: 'a, 'a> Iterator for Join<'s, 'a> {
    type Item = Frame<'s>;

    fn next(&mut self) -> Option<Frame<'s>> {
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

impl<'s: 'a, 'a> Plan for Join<'s, 'a> {
    fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
        self.current_left = None;
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
fn plan_joins<'s: 'a, 'a>(
        mut joins: LinkedList<Frames<'s, 'a>>) -> Frames<'s, 'a> {
    let head = joins.pop_front();
    match head {
        None => panic!("Empty Join list"),
        Some(term) => {
            if joins.len() == 0 {
                term
            } else {
                let rest: Frames<'s, 'a> = plan_joins(joins);
                Box::new(Join::new(term, rest))
            }
        }
    }
}

fn plan_recursive_rule<'s: 'a, 'a>(
        engine: &'s Storage,
        name: &str,
        rule: &[ast::Term],
        formals: &[String],
        all_tuples: &'a HashSet<Tuple<'s>>) -> Result<Tuples<'s, 'a>> {
    let mut joins: LinkedList<Frames<'s, 'a>> = LinkedList::new();
    for term in rule {
        let (relation_name, params) = deconstruct_term(term.clone())?;
        if relation_name == name {
            let tuples = Box::new(SetNode::new(all_tuples));
            let scan = PatternMatch::new(Pattern::new(params), tuples);
            joins.push_back(Box::new(scan));
        } else {
            joins.push_back(query(engine, term.clone())?);
        }
    }

    Ok(Box::new(IntensionalScan::new(formals.to_vec(), plan_joins(joins))))
}


/// Given a query, return all variable assignments over the database that
/// satisfy that query.
pub fn query<'s>(engine: &'s Storage,
                 query: ast::Term) -> Result<Frames<'s, 's>> {
    let (head, rest) = deconstruct_term(query)?;

    let relation =
        engine.get_relation(head.as_str())
              .ok_or(Error::MalformedLine(
                      format!("No relation \"{}\" found.", head.as_str())))?;
    let scan = match relation {
        Extension(ref table) => Box::new(ExtensionalScan::new(table)),
        Intension(view) => IntensionalScan::from_view(&head, engine, view)?
    };

    Ok(Box::new(PatternMatch::new(Pattern::new(rest), scan)))
}

//
// Modifying the database.
//

/// Add a simple fact (one with no variables) to the database.
fn simple_assert(engine: &mut Storage,
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

fn add_rule_to_view(engine: &mut Storage,
                    rule: ast::Rule) -> Result<()> {
    let (name, definition) = deconstruct_term(rule.head)?;
    let params = to_variables(definition)?;
    let relation = storage::Relation::Intension(
        AstView { rules: Vec::new() }
    );
    let mut rel_view = engine.get_or_create_relation(name.clone(), relation);
    match *rel_view {
        Extension(_) => Err(Error::NotIntensional(name)),
        Intension(ref mut view) => Ok(view.rules.push((params, rule.body)))
    }
}

/// Add a fact or rule to the database.
pub fn assert(engine: &mut Storage, fact: ast::Rule) -> Result<()> {
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

fn is_recursive(name: &str, rule: Vec<ast::Term>) -> Result<bool> {
    for term in rule {
        let (relation_name, _) = deconstruct_term(term)?;
        if relation_name == name {
            return Ok(true);
        }
    }

    Ok(false)
}
