/// Stacking options and results.
/// 
/// Simplifies writing iterators over streams that might contain failures.

use error::*;

/// A combination of `Option` and `Result`.
/// 
/// Can be empty, an `Error`, or some value `T`.
pub enum OptRes<T> {
    Done,
    Bad(Error),
    Good(T)
}

impl<T> OptRes<T> {
    pub fn and_then<U, F: FnOnce(T) -> OptRes<U>> (self, op: F) -> OptRes<U> {
        match self {
            OptRes::Good(x) => op(x),
            OptRes::Bad(e) => OptRes::Bad(e),
            OptRes::Done => OptRes::Done
        }
    }

    pub fn unless_err<U, F: FnOnce() -> OptRes<U>>(self, op: F) -> OptRes<U> {
        match self {
            OptRes::Bad(e) => OptRes::Bad(e),
            _ => op()
        }
    }

    pub fn map<U, F: FnOnce(T) -> U> (self, op: F) -> OptRes<U> {
        self.and_then(|x| OptRes::Good(op(x)))
    }
}

impl<T> From<Option<Result<T>>> for OptRes<T> {
    fn from(o: Option<Result<T>>) -> Self {
        match o {
            None => OptRes::Done,
            Some(Err(e)) => OptRes::Bad(e),
            Some(Ok(x)) => OptRes::Good(x)
        }
    }
}

impl<T> Into<Option<Result<T>>> for OptRes<T> {
    fn into(self) -> Option<Result<T>> {
        match self {
            OptRes::Done => None,
            OptRes::Bad(e) => Some(Err(e)),
            OptRes::Good(x) => Some(Ok(x))
        }
    }
}
