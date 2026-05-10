use std::fmt;

/// Signed or unsigned integer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndexError {
    /// `checked_add`/`sub` returned [`None`]
    Overflow,
    Value(usize),
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => "[overflow]".fmt(f),
            Self::Value(n) => n.fmt(f),
        }
    }
}
