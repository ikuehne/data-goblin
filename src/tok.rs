#[derive(Debug, PartialEq)]
pub enum Tok {
    Atom(String),
    Comma,
    Dot,
    Means,
    Query,
    Variable(String)
}
