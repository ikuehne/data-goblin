use std::cell::{RefCell};
use std::collections::{HashMap, HashSet};

struct DependencyGraph {
    /// Maps relations to the relations *that depend on them*.
    dependents: HashMap<String, Vec<String>>
}

impl DependencyGraph {
    fn new() -> Self {
        DependencyGraph { dependents: HashMap::new() }
    }

    fn add_dependency(&mut self, relation: String, dependent: String) {
        self.dependents.entry(relation)
                       .or_insert(Vec::new())
                       .push(dependent)
    }

    fn get_dependents(&self, relation: &str) -> &[String] {
        self.dependents.get(relation).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

pub struct ViewCache {
    dependencies: DependencyGraph,
    contents: RefCell<HashMap<String, HashSet<Vec<String>>>>
}

impl ViewCache {
    pub fn new() -> Self {
        ViewCache {
            dependencies: DependencyGraph::new(),
            contents: RefCell::new(HashMap::new())
        }
    }

    pub fn add_dependency(&mut self, relation: String, dependent: String) {
        self.dependencies.add_dependency(relation, dependent);
    }

    fn invalidate_helper<'a>(
            contents: &mut HashMap<String, HashSet<Vec<String>>>,
            dependencies: &'a DependencyGraph,
            relation: &str,
            visited: &mut HashSet<&'a str>) {
        contents.remove(relation);

        for dependency in dependencies.get_dependents(relation) {
            if visited.insert(dependency) {
                Self::invalidate_helper(contents,
                                        dependencies,
                                        dependency,
                                        visited);
            }
        }
    }

    pub fn invalidate(&mut self, relation: &str) {
        let mut visited: HashSet<&'_ str> = HashSet::new();

        Self::invalidate_helper(&mut self.contents.borrow_mut(),
                                &self.dependencies,
                                relation,
                                &mut visited);
    }

    pub fn add_tuple(&self, relation: String, tuple: Vec<String>) {
        let mut lock = self.contents.borrow_mut();
        let set = lock.entry(relation).or_insert(HashSet::new());
        set.insert(tuple);
    }

    pub fn read_cache<'s>(&'s self, relation: &str)
            -> Option<Vec<Vec<String>>> {
        self.contents.borrow().get(relation).map(|set| {
            set.iter().map(Vec::clone).collect()
        })
    }
}
