/// Simple in-memory storage engine for Datalog.

use std::collections::HashMap;
use std::iter::IntoIterator;
use std::slice;

// Perhaps we want this to be generic in the future to allow swapping out
// storage engines, since we're likely to make several storage engines. For now,
// I think it's best to first write a simple storage engine so we can see what
// kind of interface works.

pub type Tuple<'a> = Vec<&'a str>;

#[derive(Clone, Debug)]
pub struct Table<'a> {
    name: &'a str,
    rows: Vec<Tuple<'a>>
}

impl<'a> Table<'a> {
    pub fn new(name: &'a str) -> Self {
        Table {
            name,
            rows: Vec::new()
        }
    }
}

impl<'a> IntoIterator for &'a mut Table<'a> {
    type Item = &'a mut Tuple<'a>;
    type IntoIter = slice::IterMut<'a, Tuple<'a>>;

    fn into_iter(self) -> slice::IterMut<'a, Tuple<'a>> {
        (&mut self.rows).into_iter()
    }
}

#[derive(Clone, Debug)]
pub struct StorageEngine<'a> {
    tables: HashMap<&'a str, Table<'a>>
}

impl<'a> StorageEngine<'a> {
    pub fn new() -> Self {
        StorageEngine {
            tables: HashMap::new()
        }
    }

    pub fn get_table(&mut self, name: &'a str) -> &Table {
        self.get_table_mut(name)
    }

    pub fn get_table_mut(&mut self, name: &'a str) -> &mut Table<'a> {
        self.tables.entry(name).or_insert(Table::new(name))
    }
}
