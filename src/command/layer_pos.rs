use crate::layer::Layer;
use std::{collections::TryReserveError, fmt, str::FromStr};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AssetPos {
    #[default]
    Basic,
    Index(usize),
}

impl From<usize> for AssetPos {
    fn from(value: usize) -> Self {
        Self::Index(value)
    }
}

#[derive(Debug, Error)]
pub enum ParseAssetPosError {
    #[error("unrecognized position format: {}", .0)]
    Unknown(String),

    #[error("failed to parse position index")]
    IndexInt(#[source] std::num::ParseIntError),
}

impl FromStr for AssetPos {
    type Err = ParseAssetPosError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use ParseAssetPosError::*;
        match s {
            _ if s.starts_with(|ch: char| ch.is_ascii_digit()) => {
                s.parse().map(Self::Index).map_err(IndexInt)
            }
            _ => Err(Unknown(s.to_string())),
        }
    }
}

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

pub trait LayerRange: std::ops::RangeBounds<LayerPos> {
    type Output: std::ops::RangeBounds<usize> + std::slice::SliceIndex<[Layer]>;

    fn select_layer_range(
        self,
        curr_layer: usize,
        layer_count: usize,
    ) -> Result<Self::Output, SelectLayerError>;
}

impl LayerRange for std::ops::Range<LayerPos> {
    type Output = std::ops::Range<usize>;

    fn select_layer_range(
        self,
        curr_layer: usize,
        layer_count: usize,
    ) -> Result<Self::Output, SelectLayerError> {
        Ok(self.start.select_layer_idx(curr_layer, layer_count)?
            ..self.end.select_layer_idx(curr_layer, layer_count)?)
    }
}

impl LayerRange for std::ops::RangeInclusive<LayerPos> {
    type Output = std::ops::RangeInclusive<usize>;

    fn select_layer_range(
        self,
        curr_layer: usize,
        layer_count: usize,
    ) -> Result<Self::Output, SelectLayerError> {
        Ok(self.start().select_layer_idx(curr_layer, layer_count)?
            ..=self.end().select_layer_idx(curr_layer, layer_count)?)
    }
}

impl LayerRange for std::ops::RangeFrom<LayerPos> {
    type Output = std::ops::RangeFrom<usize>;

    fn select_layer_range(
        self,
        curr_layer: usize,
        layer_count: usize,
    ) -> Result<Self::Output, SelectLayerError> {
        Ok(self.start.select_layer_idx(curr_layer, layer_count)?..)
    }
}

impl LayerRange for std::ops::RangeTo<LayerPos> {
    type Output = std::ops::RangeTo<usize>;

    fn select_layer_range(
        self,
        curr_layer: usize,
        layer_count: usize,
    ) -> Result<Self::Output, SelectLayerError> {
        Ok(..self.end.select_layer_idx(curr_layer, layer_count)?)
    }
}

impl LayerRange for std::ops::RangeToInclusive<LayerPos> {
    type Output = std::ops::RangeToInclusive<usize>;

    fn select_layer_range(
        self,
        curr_layer: usize,
        layer_count: usize,
    ) -> Result<Self::Output, SelectLayerError> {
        Ok(..=self.end.select_layer_idx(curr_layer, layer_count)?)
    }
}

impl LayerRange for std::ops::RangeFull {
    type Output = std::ops::RangeFull;

    fn select_layer_range(
        self,
        _curr_layer: usize,
        _layer_count: usize,
    ) -> Result<Self::Output, SelectLayerError> {
        Ok(..)
    }
}

pub trait LayerIndex<T> {
    type Output: ?Sized;

    fn select(&self, curr_layer: usize, index: T) -> Result<&Self::Output, SelectLayerError>;
}

impl LayerIndex<LayerPos> for [Layer] {
    type Output = Layer;

    fn select(
        &self,
        curr_layer: usize,
        index: LayerPos,
    ) -> Result<&Self::Output, SelectLayerError> {
        index
            .select_layer_idx(curr_layer, self.len())
            .map(|idx| self.get(idx)
            .expect("select_layer_idx should always product a valid index into a slice with len layer_count in the non-err branch"))
    }
}

impl<B: LayerRange> LayerIndex<B> for [Layer] {
    type Output = <<B as LayerRange>::Output as std::slice::SliceIndex<[Layer]>>::Output;

    fn select(&self, curr_layer: usize, index: B) -> Result<&Self::Output, SelectLayerError> {
        index
            .select_layer_range(curr_layer, self.len())
            .map(|range| self.get(range)
            .expect("select_layer_range should always product a valid range into a slice with len layer_count in the non-err branch"))
    }
}

pub trait LayerIndexMut<T>: LayerIndex<T> {
    fn select_mut(
        &mut self,
        curr_layer: usize,
        index: T,
    ) -> Result<&mut Self::Output, SelectLayerError>;
}

impl LayerIndexMut<LayerPos> for [Layer] {
    fn select_mut(
        &mut self,
        curr_layer: usize,
        index: LayerPos,
    ) -> Result<&mut Self::Output, SelectLayerError> {
        index
            .select_layer_idx(curr_layer, self.len())
            .map(|idx| self.get_mut(idx)
            .expect("select_layer_idx should always product a valid index into a slice with len layer_count in the non-err branch"))
    }
}

impl<B: LayerRange> LayerIndexMut<B> for [Layer] {
    fn select_mut(
        &mut self,
        curr_layer: usize,
        index: B,
    ) -> Result<&mut Self::Output, SelectLayerError> {
        index
            .select_layer_range(curr_layer, self.len())
            .map(|range| self.get_mut(range)
            .expect("select_layer_range should always product a valid range into a slice with len layer_count in the non-err branch"))
    }
}

pub trait LayerInsert<T> {
    fn insert_layer(
        &mut self,
        curr_layer: usize,
        index: T,
        layer: Layer,
    ) -> Result<(usize, &mut Layer), InsertLayerError>;
}

impl LayerInsert<LayerPos> for Vec<Layer> {
    fn insert_layer(
        &mut self,
        curr_layer: usize,
        index: LayerPos,
        layer: Layer,
    ) -> Result<(usize, &mut Layer), InsertLayerError> {
        let idx = index.insert_layer_idx(curr_layer, self.len())?;
        // manually reserve so that we can error instead of panicking
        if self.len() == self.capacity() {
            self.try_reserve(self.len().max(1))?; // double size
        }
        Ok((idx, self.insert_mut(idx, layer)))
    }
}
