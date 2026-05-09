use std::{collections::TryReserveError, str::FromStr};
use thiserror::Error;

use crate::layer_pos::IndexError;

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

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PushAssetError {
    #[error("cannot exceed {} assets on a {}-bit system", const { usize::MAX }, const { std::mem::size_of::<usize>() * 8 })]
    TooManyAssets,

    #[error("out of memory")]
    AllocError(#[from] TryReserveError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum SelectAssetError {
    #[error("asset {} does not exist", .0)]
    IndexOutOfBounds(IndexError),
}

// Distinct from #[from] because we dont want to imply #[source]
impl From<IndexError> for SelectAssetError {
    fn from(e: IndexError) -> Self {
        Self::IndexOutOfBounds(e)
    }
}

impl AssetPos {
    /// Determine the asset index that must exist so it can be accessed.
    ///
    /// Valid indices: `0..asset_count`
    pub fn select_asset_idx(self, asset_count: usize) -> Result<usize, SelectAssetError> {
        match self {
            AssetPos::Basic => todo!(),

            AssetPos::Index(idx) => {
                if idx < asset_count {
                    Ok(idx)
                } else {
                    Err(SelectAssetError::IndexOutOfBounds(IndexError::Value(idx)))
                }
            }
        }
    }
}
