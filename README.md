# Data Goblin

A Datalog implementation in Rust.

## Introduction

Data Goblin is Ian Kuehne and Noah Nelson's Caltech CS 123 project, under Donnie
Pinkston's mentorship. The project aims to be a robust, extensible, minimal
implementation of the Datalog query language. In particular we want it to serve
as an educational testbed for implementing evaluation algorithms and
optimizations.

## Compiling

Data Goblin is written as a Cargo crate, so
```
cargo build --release
```
will compile it.
```
cargo test
```
will run suites of unit and integration tests.
```
cargo bench
```
will run benchmarks - these take quite a while to run.

## Usage

```
cargo run --release
```
will open the Datalog REPL. Datalog stores the database in `data/`, which it
will create if it does not already exist.

## Datalog

Datalog is a logical query language related to the Prolog programming language.
A Datalog database consists of a set of facts and rules for making inferences
over those facts. For example, one might express a simple family tree using a
`parent` relation:
```prolog
parent(helen, mary).
parent(mary, isaac).
parent(isaac, james).
parent(isaac, robert).
```
The `parent` relation thus corresponds to a table in SQL, or an "extensional
relation". Through the use of variables, it is also possible to create views, or
extensional relations:
```prolog
sibling(X, Y) :- parent(Z, X), parent(Z, Y).
```
The `,` operator signifies logical conjunction. Views can also have multiple
definitions, to acheive logically disjunction:
```prolog
cousin_once_removed(X, Y) :- parent(A, X), parent (B, A), parent(B, Y).
cousin_once_removed(X, Y) :- parent(A, Y), parent (B, A), parent(B, X).
```
Recursive definitions are also allowed:
```prolog
ancestor(X, Y) :- parent(X, Y).
ancestor(X, Y) :- parent(X, Z), ancestor(Z, Y).
```
To query a database, in Data Goblin the user enters a term followed by `?`, and
Data Goblin returns all assignments to the variables in that term that
correspond to facts deducible from the database. So, for instance, if we wanted
to ask who `isaac`'s children are, we would query:
```prolog
parent(isaac, X)?
```
Datalog might return
```
X: james
```
Datalog returns once assignment as a time, as they are computed; to tell Data
Goblin to get the next assignment, the user must enter `;`. Entering any other
key will terminate the query.

Because Datalog includes recursion, it is computationally more powerful than the
relational algebra; specifically Datalog is P-complete. That also means that
datalog queries cannot in general be evaluated in less than exponential time.

## Data Goblin Internals

### Architecture

Evaluation starts with the `driver::Driver`. The `Driver` creates a new
`storage::StorageEngine`, which deals with the data and ensures it is stored to
disk. It also creates a `cache::Cache`, which maintains a cache of the contents
of the views in the database. Finally, it starts up a write-back thread which
periodically writes dirty tables to disk (`driver::make_writer`).  The driver
then reads input from a character stream and passes it through the rest of the
system. It first lexes it (`lexer::Lexer`) into a stream of tokens (`tok::Tok`),
which are then parsed (`parser::Parser`) into a stream of ASTs (`ast::Line`),
each of which represents either a query or a new entry in the database.

Once syntax analysis is completed, the driver passes new entries `eval::assert`,
which stores the entry in the storage engine. Queries are passed to
`eval::query`, which returns a plan (`eval::Frames`) for producing the matching
frames (assignments to the variables in the query). The driver then iterates
over the frames in that plan, pretty-printing them to the console.

### Storage Engine

Because of the limited time we had to spend on this project, Data Goblin has a
very primitive "storage engine". Each table is simply serialized to JSON using
`serde` and written to a file on disk. The storage engine takes advantage of
Rust's affine types to ensure that changes are always written back: the only
interface through which the storage engine allows modifying tables is by
returning `RelViewMut`, which behaves like a mutable reference to a relation
except that it sets the table's dirty bit when it is `Drop`ed. Thus, it is
impossible to make changes to the database without setting the corresponding
tables' dirty bits.

### Evaluation

Data Goblin uses a plan node evaluation structure. Complex intensional rules
are recursively planned. Recursive rules are the most interesting problem in
evaluating Datalog, and Data Goblin has two main ways of evaluating them. Consider
the above recursive view:
```prolog
ancestor(X, Y) :- parent(X, Y).
ancestor(X, Y) :- parent(X, Z), ancestor(Z, Y).
```

#### Bottom Up Evaluation

The basic way of getting tuples from a recursively-defined view is to first
evaluate all rules that aren't recursive in the usual way to get the first level
of `ancestor` tuples, and then to apply the recursive rules to the current
collection of tuples until no new tuples are generated. This is bottom up
evaluation, and is fairly slow. It is implemented by the `eval::BottomUp`
plan node, which applies the bottom up process to generate all of its tuples
immediately upon construction.

#### Semi-Naïve Evaluation

Semi-naïve evaluation makes use of the insight that not every
previously-generated tuple from a recursive view is necessary at every step
of the bottom up process. So, for some rules, the new set of tuples avaible
depends on the _last_ set of tuples generated, not the full set of tuples
in the relation. This is especially true for linear recursive rules, which
only have one occurrence of the recursive view in the join body of the rule.

The `ancestor` view above is a good example of a linear recursive rule. Consider
again the above extensional dataset:
```prolog
parent(helen, mary).
parent(mary, isaac).
parent(isaac, james).
parent(isaac, robert).
```
When evaluating the `ancestor` view's contents with the bottom up method, first
the four tuples in the `parent` table are added to the set of results, so our
results so far are the tuples:
```prolog
ancestor(helen, mary).
ancestor(mary, isaac).
ancestor(isaac, james).
ancestor(isaac, robert).
```
Then, these results are joined against the `parent` table, so
`ancestor(helen, mary)` is joined against `parent(mary, isaac)` according to the
second rule, and `ancestor(helen, isaac)` is produced. Now, we notice that
in the next loop of the bottom up evaluation, `ancestor(helen, mary)` will again
be joined against `parent(mary, isaac)`, even though this doesn't yield any new
information. So, in semi-naive evaluation, we would only join the newest
`ancestor` tuples against the parent relation.

Currently, Data Goblin only supports semi-naive evaluation for linear
recursive rules. The benchmarks included should show
that semi-naive evaluation is more than five times faster than bottom up in a
test data set of a 100-person employee hierarchy.
