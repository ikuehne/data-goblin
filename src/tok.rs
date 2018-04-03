#[derive(Debug, PartialEq)]
pub enum Tok {
    Atom(String),
    Comma,
    CloseParen,
    Dot,
    Means,
    Query,
    OpenParen,
    Variable(String)
}
