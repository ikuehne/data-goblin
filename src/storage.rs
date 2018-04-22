/// Simple JSON-based. storage engine for Datalog.

use ast;
use error::*;
use error::Error::StorageError;

use serde_json;

use std;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::iter::IntoIterator;
use std::path::Path;
use std::slice;

// Perhaps we want this to be generic in the future to allow swapping out
// storage engines, since we're likely to make several storage engines. For now,
// I think it's best to first write a simple storage engine so we can see what
// kind of interface works.

pub type Tuple = Vec<String>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Table {
    rows: Vec<Tuple>
}

#[derive(Serialize, Deserialize)]
pub struct View {
    formals: Vec<String>,
    definition: Vec<ast::Term>
}

#[derive(Serialize, Deserialize)]
pub enum Relation {
    Extension(Table),
    Intension(View)
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

pub struct StorageEngine {
    data_dir: String,
    tables: HashMap<String, Relation>
}

fn err<E: std::error::Error + 'static>(err: E) -> Error {
    StorageError(Box::new(err))
}

impl StorageEngine {
    pub fn new(data_dir: String) -> Result<Self> {
        let mut tables = HashMap::new();

        match fs::read_dir(data_dir.clone()) {
            Err(e) =>
                match e.kind() {
                    io::ErrorKind::NotFound => {
                        fs::create_dir(data_dir.as_str());
                        Ok(StorageEngine {
                            data_dir,
                            tables
                        })
                    },
                    other => Err(err(e))
                },
            Ok(files)  => {
                for res_entry in files {
                    let entry = res_entry.map_err(err)?;
                    let fname = entry.path();
                    let reader = fs::File::open(fname).map_err(err)?;
                    let buffered = io::BufReader::new(reader);
                    let table: Relation =
                        serde_json::from_reader(buffered).map_err(err)?;
                    let name = entry.file_name().into_string().map_err(|e|
                        Error::BadFilename(e)
                    )?;
                    tables.insert(name, table);
                }
                Ok(StorageEngine {
                    data_dir,
                    tables
                })
            }
        }
    }

    pub fn get_table(&self, name: &str) -> Option<&Relation> {
        self.tables.get(name)
    }

    pub fn get_table_mut(&mut self, name: &str) -> Option<&mut Relation> {
        self.tables.get_mut(name)
    }

    pub fn get_or_create_table(&mut self, name: String) -> &mut Relation {
        self.tables.entry(name).or_insert(Relation::Extension(Table::new()))
    }
}

impl Drop for StorageEngine {
    fn drop(&mut self) {
        for (fname, table) in &self.tables {
            let path_buf = Path::new(self.data_dir.as_str()).join(fname);
            let path = path_buf.as_path();
            let out = io::BufWriter::new(fs::File::create(path).unwrap());
            serde_json::to_writer(out, table).unwrap();
        }
    }
}
