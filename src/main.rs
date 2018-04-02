mod ast;

fn main() {
    let x = ast::AtomicTerm::Atom("hello".to_string());
    println!("Hello, world!");
}
