/// Simple in-memory storage engine for Datalog.

use std::collections::HashMap;
use std::iter::IntoIterator;
use std::slice;

// Perhaps we want this to be generic in the future to allow swapping out
// storage engines, since we're likely to make several storage engines. For now,
// I think it's best to first write a simple storage engine so we can see what
// kind of interface works.

pub type Tuple = Vec<String>;

#[derive(Clone, Debug)]
pub struct Table {
    rows: Vec<Tuple>
}

impl Table {
    pub fn new() -> Self {
        Table {
            rows: Vec::new()
        }
    }

    pub fn assert(&mut self, fact: Tuple) {
        self.rows.push(fact)
    }
}

pub type TableScan<'i> = slice::Iter<'i, Tuple>;

impl<'i> IntoIterator for &'i Table {
    type Item = &'i Tuple;
    type IntoIter = TableScan<'i>;

    fn into_iter(self) -> TableScan<'i> {
        (&self.rows).into_iter()
    }
}

#[derive(Clone, Debug)]
pub struct StorageEngine {
    tables: HashMap<String, Table>
}

impl StorageEngine {
    pub fn new() -> Self {
        StorageEngine {
            tables: HashMap::new()
        }
    }

    pub fn get_table(&self, name: &str) -> Option<&Table> {
        self.tables.get(name)
    }

    pub fn get_table_mut(&mut self, name: &str) -> Option<&mut Table> {
        self.tables.get_mut(name)
    }

    pub fn get_or_create_table(&mut self, name: String) -> &mut Table {
        self.tables.entry(name).or_insert(Table::new())
    }
}
