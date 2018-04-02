pub enum AtomicTerm {
    Atom(String),
    Variable(String)
}

pub struct CompoundTerm {
    pub relation: String,
    pub params: Vec<AtomicTerm>
}

pub enum Term {
    Atomic(AtomicTerm),
    Compound(CompoundTerm)
}

pub struct Rule {
    pub head: Term,
    pub body: Vec<Term>
}

pub enum Line {
    Query(Term),
    Rule(Rule)
}
