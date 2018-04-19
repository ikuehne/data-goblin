/// Evaluator for dead-simple queries.
/// 
/// Intended mostly as a skeleton while we work out architecture.

use ast;
use storage;

pub struct Evaluator {
    engine: storage::StorageEngine
}

pub enum QueryResult<'i> {
    TableFound {
        query: Vec<String>,
        scan: storage::TableScan<'i>
    },
    NoTableFound
}

impl<'i> Iterator for QueryResult<'i> {
    type Item = &'i storage::Tuple;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            QueryResult::TableFound { query, scan } =>
                match scan.next() {
                    None => None,
                    Some(t) => if *t == *query {
                        Some(t)
                    } else {
                        self.next()
                    }
                }
            QueryResult::NoTableFound => None
        }
    }
}

impl Evaluator {
    pub fn new(engine: storage::StorageEngine) -> Self {
        Evaluator { engine }
    }

    fn to_atom(t: ast::AtomicTerm) -> String {
        match t {
            ast::AtomicTerm::Atom(s) => s,
            _ => panic!("Can't handle variables yet!")
        }
    }

    fn deconstruct_term(t: ast::Term) -> (String, Vec<String>) {
        match t {
            ast::Term::Atomic(a) => (Self::to_atom(a), Vec::new()),
            ast::Term::Compound(cterm) =>
                (cterm.relation,
                 cterm.params.into_iter().map(Self::to_atom).collect())
        }
    }

    pub fn query<'i>(&'i self, query: ast::Term) -> QueryResult<'i> {
        let (head, rest) = Self::deconstruct_term(query);

        let table = self.engine.get_table(&head);
        table.map(|t| QueryResult::TableFound {
            query: rest,
            scan: t.into_iter()
        }).unwrap_or(QueryResult::NoTableFound)
    }

    pub fn assert(&mut self, fact: ast::Rule) {
        let (head, rest) = Self::deconstruct_term(fact.head);
        self.engine.get_or_create_table(head).assert(rest)
    }
}
