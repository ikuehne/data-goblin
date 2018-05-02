/// Simple JSON-based storage engine for Datalog.
/// 
/// Uses the `serde_json` library for deserialization; note that all types that
/// own durable data are `Serialize` and `Deserialize`.

use ast;
use error::*;
use error::Error::StorageError;

use serde_json;

use std;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::iter::IntoIterator;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::slice;
use std::sync::atomic::{AtomicBool, Ordering};

// Perhaps we want this to be generic in the future to allow swapping out
// storage engines, since we're likely to make several storage engines. For now,
// I think it's best to first write a simple storage engine so we can see what
// kind of interface works.

/// A `Tuple` is simply an ordered collection of atoms.
pub type Tuple = Vec<String>;

/// A `Table` is an extensional relation in the database.
#[derive(Debug, Serialize, Deserialize)]
pub struct Table {
    rows: Vec<Tuple>,
}

/// A `View` is an intensional relation in the database.
#[derive(Serialize, Deserialize)]
pub struct View {
    pub formals: Vec<ast::AtomicTerm>,
    pub definition: Vec<Vec<ast::Term>>
}

/// A `Relation` is either an extensional or an intensional relation.
#[derive(Serialize, Deserialize)]
pub enum Relation {
    Extension(Table),
    Intension(View)
}

impl Relation {

    pub fn write_back(&self, path: &str) {
        let out =
            io::BufWriter::new(fs::File::create(path).unwrap());
        serde_json::to_writer(out, self).unwrap();
    }
}

#[derive(Serialize, Deserialize)]
struct TaggedRelation {
    contents: Relation,
    path: String,
    #[serde(default, skip)]
    dirty: AtomicBool
}

impl Table {
    pub fn new() -> Self {
        Table {
            rows: Vec::new()
        }
    }

    /// Add a fact to this relation.
    pub fn assert(&mut self, fact: Tuple) {
        self.rows.push(fact)
    }
}

impl TaggedRelation {
    fn new(path: String, contents: Relation) -> Self {
        TaggedRelation {
            path,
            contents,
            dirty: AtomicBool::default()
        }
    }

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

/// A TableScan is an iterator over all of the tuples in an extensional
/// relation.
pub type TableScan<'i> = slice::Iter<'i, Tuple>;

/// Immutable views on tables can be converted to TableScans.
impl<'i> IntoIterator for &'i Table {
    type Item = &'i Tuple;
    type IntoIter = TableScan<'i>;

    fn into_iter(self) -> TableScan<'i> {
        (&self.rows).into_iter()
    }
}

/// A StorageEngine manages all of the relations in a database.
/// 
/// In particular, it can create new relations, provide views on existing
/// relations, and ensure that modifications to relations are durable.
pub struct StorageEngine {
    data_dir: String,
    relations: HashMap<String, TaggedRelation>
}

/// A mutable view on a `Relation`.
/// 
/// Ensures that any changes to the `Relation` are written back to disk.
pub struct RelViewMut<'i>(&'i mut TaggedRelation);

impl<'i> Drop for RelViewMut<'i> {
    // On dropping the `RelViewMut`, any changes are written back.
    fn drop(&mut self) {
        self.0.dirty();
    }
}

impl<'i> Deref for RelViewMut<'i> {
    type Target = Relation;

    fn deref(&self) -> &Self::Target {
        &self.0.contents
    }
}

impl<'i> DerefMut for RelViewMut<'i> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0.contents
    }
}

// Lift some error into an `error::Error`.
fn err<E: std::error::Error + 'static>(err: E) -> Error {
    StorageError(Box::new(err))
}

impl StorageEngine {
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
                    let table: TaggedRelation =
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
    pub fn get_relation(&self, name: &str) -> Option<&Relation> {
        self.relations.get(name).map(|r| &r.contents)
    }

    /// Get a mutable view on the named relation.
    /// 
    /// Returns `None` if it is not in the database. See also `RelViewMut`.
    pub fn get_relation_mut(&mut self, name: &str) -> Option<RelViewMut> {
        self.relations.get_mut(name).map(RelViewMut)
    }

    /// Retrieve the given relation, or create it if it doesn't exist.
    /// 
    /// Must take ownership of the table name, because it needs to be stored in
    /// the database if it is not already there. See also `RelViewMut`.
    pub fn get_or_create_relation(&mut self, name: String, rel: Relation)
            -> RelViewMut {
        let path = self.path_of_table_name(name.as_str());
        let tagged = TaggedRelation { contents: rel,
                                      path, dirty: AtomicBool::new(true) };
        RelViewMut(self.relations.entry(name).or_insert(tagged))
    }

    pub fn write_back(&self) {
        for (_, relation) in &self.relations {
            (&relation).write_back();
        }
    }
}

impl Drop for StorageEngine {
    fn drop(&mut self) {
        self.write_back();
    }
}

#[cfg(test)]
mod tests {
    use storage::*;

    fn test_table(v: &Vec<Vec<&str>>) -> Table {
        let mut t = Table::new();
        for tuple in v {
            t.assert(tuple.into_iter().map(|r| r.to_string()).collect());
        }
        t
    }

    fn table_as_vec(t: &Table) -> Vec<&Tuple> {
        t.into_iter().collect()
    }

    #[test]
    fn empty_table() {
        let t = Table::new();
        let expected: Vec<&Tuple> = vec!();
        assert_eq!(table_as_vec(&t), expected);
    }

    #[test]
    fn table_scan() {
        let expected_contents = vec!(vec!("a", "b", "c"),
                                     vec!("d", "e", "f"));
        let t = test_table(&expected_contents);
        let mut expected: Vec<&Vec<&str>> = Vec::new();
        
        for tuple in &expected_contents {
            expected.push(tuple)
        }

        assert_eq!(table_as_vec(&t), expected);
    }
}
