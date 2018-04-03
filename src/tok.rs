#[derive(Debug, PartialEq)]

/// Datalog lexical tokens.
pub enum Tok {
    Atom(String),
    Comma,
    CloseParen,
    /// "."
    Dot,
    /// ":-"
    Means,
    /// "?"
    Query,
    OpenParen,
    Variable(String)
}
