use std::{collections::TryReserveError, fmt, str::FromStr};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LayerPos {
    /// `*`, `[h]ere` - the active target
    #[default]
    Current,
    /// `k`, `]`, `[n]ext` - the active target + 1
    Next,
    /// `j`, `[`, `[p]rev` - the active target &minus; 1
    Prev,
    /// `]]`, `[f]ront` - furthest in the `Next` direction
    Front,
    /// `[[`, `[b]ack` - furthest in the `Prev` direction
    Back,
    /// number - Specific position
    Index(usize),
    /// `+`/`-`number - Offset from current index
    Offset(isize),
}

impl From<usize> for LayerPos {
    fn from(value: usize) -> Self {
        Self::Index(value)
    }
}
impl From<isize> for LayerPos {
    fn from(value: isize) -> Self {
        Self::Offset(value)
    }
}

#[derive(Debug, Error)]
pub enum ParseLayerPosError {
    #[error("unrecognized position format: {}", .0)]
    Unknown(String),

    #[error("failed to parse position index")]
    IndexInt(#[source] std::num::ParseIntError),

    #[error("failed to parse offset amount")]
    OffsetInt(#[source] std::num::ParseIntError),
}

impl FromStr for LayerPos {
    type Err = ParseLayerPosError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use ParseLayerPosError::*;
        match s {
            "*" | "h" | "here" => Ok(Self::Current),
            "k" | "]" | "n" | "next" => Ok(Self::Next),
            "j" | "[" | "p" | "prev" => Ok(Self::Prev),
            "]]" | "f" | "front" => Ok(Self::Front),
            "[[" | "b" | "back" => Ok(Self::Back),
            _ if s.starts_with(['+', '-']) => s.parse().map(Self::Offset).map_err(OffsetInt),
            _ if s.starts_with(|ch: char| ch.is_ascii_digit()) => {
                s.parse().map(Self::Index).map_err(IndexInt)
            }
            _ => Err(Unknown(s.to_string())),
        }
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InsertLayerError {
    #[error("cannot exceed {} layers on a {}-bit system", const { usize::MAX }, const { std::mem::size_of::<usize>() * 8 })]
    TooManyLayers,

    #[error("out of memory")]
    AllocError(#[from] TryReserveError),

    #[error("layer position {} is out of bounds, cannot exceed the number of layers", .0)]
    IndexOutOfBounds(IndexError),
}

// Distinct from #[from] because we dont want to imply #[source]
impl From<IndexError> for InsertLayerError {
    fn from(e: IndexError) -> Self {
        Self::IndexOutOfBounds(e)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum SelectLayerError {
    #[error("layer {} does not exist", .0)]
    IndexOutOfBounds(IndexError),
}

// Distinct from #[from] because we dont want to imply #[source]
impl From<IndexError> for SelectLayerError {
    fn from(e: IndexError) -> Self {
        Self::IndexOutOfBounds(e)
    }
}

impl LayerPos {
    /// Determine the layer index that may not exist yet so it can be created.
    ///
    /// Valid indices: `0..=layer_count`
    pub fn insert_layer_idx(
        self,
        curr_layer: usize,
        layer_count: usize,
    ) -> Result<usize, InsertLayerError> {
        use InsertLayerError::*;
        match self {
            Self::Current => {
                if layer_count == 0 {
                    assert_eq!(
                        curr_layer, 0,
                        "curr_layer should always be zero if there are no layers"
                    );
                    Ok(0)
                } else {
                    // ensures
                    // 1. soundness
                    // 2. we don't need to .min(layer_count)
                    assert!(
                        curr_layer < layer_count,
                        "curr_layer should always be a valid index if there are layers"
                    );
                    // SAFETY: curr_layer is a usize and smaller than layer_count,
                    // therefore adding 1 is guaranteed to be at most layer_count,
                    // which is still a valid usize and will not overflow.
                    Ok(curr_layer)
                }
            }

            Self::Next => {
                if layer_count == usize::MAX {
                    Err(TooManyLayers)
                } else if layer_count == 0 {
                    assert_eq!(
                        curr_layer, 0,
                        "curr_layer should always be zero if there are no layers"
                    );
                    Ok(0)
                } else {
                    // ensures
                    // 1. soundness
                    // 2. we don't need to .min(layer_count)
                    assert!(
                        curr_layer < layer_count,
                        "curr_layer should always be a valid index if there are layers"
                    );
                    // SAFETY: curr_layer is a usize and smaller than layer_count,
                    // therefore adding 1 is guaranteed to be at most layer_count,
                    // which is still a valid usize and will not overflow.
                    Ok(unsafe { curr_layer.unchecked_add(1) })
                }
            }

            Self::Prev => Ok(curr_layer.saturating_sub(1)),

            Self::Front => Ok(layer_count.saturating_sub(1)),

            Self::Back => Ok(0),

            Self::Index(idx) => {
                if idx <= layer_count {
                    Ok(idx)
                } else {
                    Err(IndexOutOfBounds(IndexError::Value(idx)))
                }
            }

            Self::Offset(amnt) => curr_layer
                .checked_add_signed(amnt)
                .ok_or(IndexOutOfBounds(IndexError::Overflow))
                .and_then(|idx| {
                    if idx <= layer_count {
                        Ok(idx)
                    } else {
                        Err(IndexOutOfBounds(IndexError::Value(idx)))
                    }
                }),
        }
    }

    /// Determine the layer index that must exist so it can be accessed.
    ///
    /// Valid indices: `0..layer_count`
    pub fn select_layer_idx(
        self,
        curr_layer: usize,
        layer_count: usize,
    ) -> Result<usize, SelectLayerError> {
        use SelectLayerError::*;
        match self {
            Self::Current => {
                if layer_count == 0 {
                    assert_eq!(
                        curr_layer, 0,
                        "curr_layer should always be zero if there are no layers"
                    );
                    Err(IndexOutOfBounds(IndexError::Value(0)))
                } else {
                    assert!(
                        curr_layer < layer_count,
                        "curr_layer should always be a valid index if there are layers"
                    );
                    Ok(curr_layer)
                }
            }

            Self::Next => {
                if layer_count == 0 {
                    assert_eq!(
                        curr_layer, 0,
                        "curr_layer should always be zero if there are no layers"
                    );
                    Err(IndexOutOfBounds(IndexError::Value(1)))
                } else {
                    assert!(
                        curr_layer < layer_count,
                        "curr_layer should always be a valid index if there are layers"
                    );
                    // SAFETY: curr_layer is a usize and smaller than layer_count,
                    // therefore adding 1 is guaranteed to be at most layer_count,
                    // which is still a valid usize and will not overflow.
                    let idx = unsafe { curr_layer.unchecked_add(1) };
                    if idx < layer_count {
                        Ok(idx)
                    } else {
                        Err(IndexOutOfBounds(IndexError::Value(idx)))
                    }
                }
            }

            Self::Prev => curr_layer
                .checked_sub(1)
                .ok_or(IndexOutOfBounds(IndexError::Overflow)),

            Self::Front => layer_count
                .checked_sub(1)
                .ok_or(IndexOutOfBounds(IndexError::Overflow)),

            Self::Back => {
                if layer_count > 0 {
                    Ok(0)
                } else {
                    Err(IndexOutOfBounds(IndexError::Value(0)))
                }
            }

            Self::Index(idx) => {
                if idx < layer_count {
                    Ok(idx)
                } else {
                    Err(IndexOutOfBounds(IndexError::Value(idx)))
                }
            }

            Self::Offset(amnt) => curr_layer
                .checked_add_signed(amnt)
                .ok_or(IndexOutOfBounds(IndexError::Overflow))
                .and_then(|idx| {
                    if idx < layer_count {
                        Ok(idx)
                    } else {
                        Err(IndexOutOfBounds(IndexError::Value(idx)))
                    }
                }),
        }
    }
}
