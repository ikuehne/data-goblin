#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum AtomicTerm {
    Atom(String),
    Variable(String)
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct CompoundTerm {
    pub relation: String,
    pub params: Vec<AtomicTerm>
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Term {
    Atomic(AtomicTerm),
    Compound(CompoundTerm)
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub head: Term,
    pub body: Vec<Term>
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Line {
    Query(Term),
    Rule(Rule)
}
