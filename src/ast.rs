
#[derive(Debug, PartialEq)]
pub enum AtomicTerm {
    Atom(String),
    Variable(String)
}

#[derive(Debug, PartialEq)]
pub struct CompoundTerm {
    pub relation: String,
    pub params: Vec<AtomicTerm>
}

#[derive(Debug, PartialEq)]
pub enum Term {
    Atomic(AtomicTerm),
    Compound(CompoundTerm)
}

#[derive(Debug, PartialEq)]
pub struct Rule {
    pub head: Term,
    pub body: Vec<Term>
}

#[derive(Debug, PartialEq)]
pub enum Line {
    Query(Term),
    Rule(Rule)
}
