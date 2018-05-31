/// Simple JSON-based storage engine for Datalog.
/// 
/// Uses the `serde_json` library for deserialization; note that all types that
/// own durable data are `Serialize` and `Deserialize`.

use error::*;
use error::Error::StorageError;

use serde::{Serialize, Deserialize};
use serde_json;

use std;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::iter::IntoIterator;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

// Perhaps we want this to be generic in the future to allow swapping out
// storage engines, since we're likely to make several storage engines. For now,
// I think it's best to first write a simple storage engine so we can see what
// kind of interface works.

/// A `Tuple` is simply an ordered collection of atoms.
pub type Tuple<'a> = Vec<&'a str>;

/// A `Table` is an extensional relation in the database.
#[derive(Debug, Serialize, Deserialize)]
pub struct Table {
    contents: Vec<String>,
    arity: usize
}

impl Table {
    pub fn new(arity: usize) -> Self {
        Table {
            contents: Vec::new(),
            arity
        }
    }

    /// Add a fact to this relation.
    pub fn assert(&mut self, mut fact: Vec<String>) -> Result<()> {
        if fact.len() != self.arity {
            Err(Error::ArityMismatch {
                expected: self.arity,
                got: fact.len()
            })
        } else {
            self.contents.append(&mut fact);
            Ok(())
        }
    }
}

/// A TableScan is an iterator over all of the tuples in an extensional
/// relation.
#[derive(Debug)]
pub struct TableScan<'a> {
    table: &'a Table,
    index: usize
}

impl<'a> Iterator for TableScan<'a> {
    type Item = Tuple<'a>;

    fn next(&mut self) -> Option<Tuple<'a>> {
        if self.index >= self.table.contents.len() {
            return None;
        }

        let slice = self.index..self.index + self.table.arity;
        let result: Vec<_> =
            self.table.contents[slice].into_iter().map(|s| s.as_str()).collect();

        self.index += self.table.arity;

        Some(result)
    }
}

/// Immutable views on tables can be converted to TableScans.
impl<'i> IntoIterator for &'i Table {
    type Item = Tuple<'i>;
    type IntoIter = TableScan<'i>;

    fn into_iter(self) -> TableScan<'i> {
        TableScan {
            table: self,
            index: 0
        }
    }
}

pub trait View<'de>: Serialize + Deserialize<'de> {}

impl<'de, T: Serialize + Deserialize<'de>> View<'de> for T {}

/// A `Relation` is either an extensional or an intensional relation.
#[derive(Serialize, Deserialize)]
pub enum Relation<V> {
    Extension(Table),
    Intension(V)
}

impl<'de, V: View<'de>> Relation<V> {
    pub fn write_back(&self, path: &str) {
        let out = io::BufWriter::new(fs::File::create(path).unwrap());
        serde_json::to_writer(out, self).unwrap();
    }
}

#[derive(Serialize, Deserialize)]
struct TaggedRelation<V> {
    contents: Relation<V>,
    path: String,
    #[serde(default, skip)]
    dirty: AtomicBool
}

impl<'de, V: View<'de>> TaggedRelation<V> {
    /// Set the "dirty" flag, and return the previous dirty state.
    fn dirty(&self) -> bool {
        self.dirty.swap(true, Ordering::SeqCst)
    }

    /// Unset the "dirty" flag, and return the previous dirty state.
    fn clean(&self) -> bool {
        self.dirty.swap(false, Ordering::SeqCst)
    }

    // On dropping the `RelViewMut`, any changes are written back.
    fn write_back(&self) {
        if self.clean() {
            let out =
                io::BufWriter::new(fs::File::create(self.path.as_str())
                                       .unwrap());
            serde_json::to_writer(out, self).unwrap();
        }
    }
}

/// A StorageEngine manages all of the relations in a database.
/// 
/// In particular, it can create new relations, provide views on existing
/// relations, and ensure that modifications to relations are durable.
pub struct StorageEngine<V> {
    data_dir: String,
    relations: HashMap<String, TaggedRelation<V>>
}

/// A mutable view on a `Relation`.
/// 
/// Ensures that any changes to the `Relation` are written back to disk.
pub struct RelViewMut<'i, 'de, V: View<'de> + 'i> {
    referand: &'i mut TaggedRelation<V>,
    _phantom: PhantomData<&'de ()>
}

impl<'i, 'de, V: View<'de>> RelViewMut<'i, 'de, V> {
    fn new(referand: &'i mut TaggedRelation<V>) -> Self {
        RelViewMut {
            referand,
            _phantom: PhantomData::default()
        }
    }
}

impl<'i, 'de, V: View<'de>> Drop for RelViewMut<'i, 'de, V> {
    // On dropping the `RelViewMut`, any changes are written back.
    fn drop(&mut self) {
        self.referand.dirty();
    }
}

impl<'i, 'de, V: View<'de>> Deref for RelViewMut<'i, 'de, V> {
    type Target = Relation<V>;

    fn deref(&self) -> &Self::Target {
        &self.referand.contents
    }
}

impl<'i, 'de, V: View<'de>> DerefMut for RelViewMut<'i, 'de, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.referand.contents
    }
}

// Lift some error into an `error::Error`.
fn err<E: std::error::Error + 'static>(err: E) -> Error {
    StorageError(Box::new(err))
}

impl<V> StorageEngine<V> where for<'de> V: View<'de> {
    /// Create a new StorageEngine.
    /// 
    /// Tables are stored in/retrieved from `data_dir`. If that directory does
    /// not exist, it will be created; if it does, its contents will be read
    /// into the new `StorageEngine`.
    pub fn new(data_dir: String) -> Result<Self> {
        let mut relations = HashMap::new();

        match fs::read_dir(data_dir.clone()) {
            Err(e) =>
                match e.kind() {
                    io::ErrorKind::NotFound => {
                        fs::create_dir(data_dir.as_str()).map_err(err)?;
                        Ok(StorageEngine {
                            data_dir,
                            relations
                        })
                    },
                    _ => Err(err(e))
                },
            Ok(files)  => {
                for res_entry in files {
                    let entry = res_entry.map_err(err)?;
                    let fname = entry.path();
                    let reader = fs::File::open(fname).map_err(err)?;
                    let buffered = io::BufReader::new(reader);
                    let table: TaggedRelation<V> =
                        serde_json::from_reader(buffered).map_err(err)?;
                    let name = entry.file_name().into_string().map_err(|e|
                        Error::BadFilename(e)
                    )?;
                    relations.insert(name, table);
                }
                Ok(StorageEngine {
                    data_dir,
                    relations
                })
            }
        }
    }

    // From the name of a table, get the path to that table.
    fn path_of_table_name(&self, table_name: &str) -> String {
        let path_buf = Path::new(self.data_dir.as_str()).join(table_name);
        path_buf.as_path().as_os_str().to_str().unwrap().to_owned()
    }

    /// Get an immutable view on the named relation.
    /// 
    /// Returns `None` if it is not in the database.
    pub fn get_relation(&self, name: &str) -> Option<&Relation<V>> {
        self.relations.get(name).map(|r| &r.contents)
    }

    /// Get a mutable view on the named relation.
    /// 
    /// Returns `None` if it is not in the database. See also `RelViewMut`.
    pub fn get_relation_mut(&mut self, name: &str)
            -> Option<RelViewMut<V>> {
        self.relations.get_mut(name).map(RelViewMut::new)
    }

    /// Retrieve the given relation, or create it if it doesn't exist.
    /// 
    /// Must take ownership of the table name, because it needs to be stored in
    /// the database if it is not already there. See also `RelViewMut`.
    pub fn get_or_create_relation(
            &mut self,
            name: String,
            rel: Relation<V>) -> RelViewMut<V> {
        let path = self.path_of_table_name(name.as_str());
        let tagged = TaggedRelation { contents: rel,
                                      path, dirty: AtomicBool::new(true) };
        RelViewMut::new(self.relations.entry(name).or_insert(tagged))
    }

    pub fn write_back(&self) {
        for (_, relation) in &self.relations {
            (&relation).write_back();
        }
    }

    pub fn get_relations<'a>(&'a self) -> Vec<&'a str> {
        let mut result = Vec::new();
        for (k, _) in self.relations.iter() {
            result.push(k.as_str());
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use storage::*;

    static TEST_DIR: &'static str = "_test_dir";

    fn test_table(v: &[Vec<&str>]) -> Table {
        let mut t = Table::new(v[0].len());
        for tuple in v {
            t.assert(tuple.into_iter().map(|r| r.to_string()).collect())
             .unwrap();
        }
        t
    }

    fn table_as_vec(t: &Table) -> Vec<Tuple> {
        t.into_iter().collect()
    }

    #[test]
    fn empty_table() {
        let t = Table::new(10);
        let expected: Vec<Tuple> = vec!();
        assert_eq!(table_as_vec(&t), expected);
    }

    #[test]
    fn table_scan() {
        let expected_contents = vec!(vec!("a", "b", "c"),
                                     vec!("d", "e", "f"));
        let t = test_table(&expected_contents);
        let mut expected: Vec<&[&str]> = Vec::new();
        
        for tuple in &expected_contents {
            expected.push(tuple)
        }

        assert_eq!(table_as_vec(&t), expected);
    }

    fn clear_test_dir() {
        if std::fs::read_dir(TEST_DIR).is_ok() {
            std::fs::remove_dir_all(TEST_DIR).unwrap();
        }
    }

    fn test_engine() -> StorageEngine<()> {
        clear_test_dir();
        StorageEngine::new(TEST_DIR.to_string()).unwrap()
    }

    fn cleanup(engine: StorageEngine<()>) {
        std::mem::drop(engine);
        clear_test_dir();
    }

    #[test]
    fn initially_empty() {
        let engine = test_engine();
        assert!(engine.get_relation("test relation").is_none());
        cleanup(engine);
    }
}
