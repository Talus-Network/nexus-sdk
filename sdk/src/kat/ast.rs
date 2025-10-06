use std::{borrow::Borrow, fmt, ops::Deref, sync::Arc};

/// Half-open byte range in the source string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub(crate) const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// Shared symbol value used for actions and tests.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(Arc<str>);

impl Symbol {
    pub fn new<S: AsRef<str>>(s: S) -> Self {
        Self(Arc::from(s.as_ref()))
    }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl Deref for Symbol {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for Symbol {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Symbol {
    fn from(value: &str) -> Self {
        Symbol::new(value)
    }
}

impl From<String> for Symbol {
    fn from(value: String) -> Self {
        Symbol::new(value)
    }
}

/// Syntax tree for KAT expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KatExpr {
    Zero,
    One,
    Action(Symbol),
    Test(TestExpr),
    Sequence(Box<KatExpr>, Box<KatExpr>),
    Choice(Box<KatExpr>, Box<KatExpr>),
    Star(Box<KatExpr>),
}

/// Syntax tree for Boolean test expressions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TestExpr {
    Zero,
    One,
    Atom(Symbol),
    Not(Box<TestExpr>),
    And(Box<TestExpr>, Box<TestExpr>),
    Or(Box<TestExpr>, Box<TestExpr>),
}
