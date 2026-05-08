use crate::command::ILLEGAL_LAYER_NAME_CHARS;
use std::{error, fmt, io};
use thiserror::Error;

/// Signed or unsigned integer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndexError {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Error)]
pub enum LayerNameError {
    #[error("layer name cannot be entirely empty or whitespace")]
    Empty,

    #[error(
        "layer names cannot contain any of the following characters: {:?}",
        ILLEGAL_LAYER_NAME_CHARS
    )]
    Illegal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum InsertLayerError {
    #[error("cannot exceed {} layers on a {}-bit system", const { usize::MAX }, const { std::mem::size_of::<usize>() * 8 })]
    TooManyLayers,

    #[error("layer position {} is out of bounds, cannot exceed the number of layers", .0)]
    IndexOutOfBounds(IndexError),
}

#[derive(Debug, Error)]
pub enum NewLayerError {
    #[error("bad layer insertion")]
    InsertLayer(#[from] InsertLayerError),

    #[error("bad layer name")]
    LayerName(#[from] LayerNameError),

    #[error("could not create a new raster")]
    Raylib(#[from] raylib::error::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum SelectLayerError {
    #[error("layer {} does not exist", .0)]
    IndexOutOfBounds(IndexError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum SwitchLayerError {
    #[error("cannot switch to layer")]
    Select(#[from] SelectLayerError),
}

#[derive(Debug, Error)]
pub enum OpenFileError {
    #[error("invalid or corrupted file")]
    Invalid,

    #[error("unsupported file format")]
    Unsupported,

    #[error("failed to load image file")]
    LoadImage(#[from] raylib::error::Error),

    #[error("could not create a new layer to insert the image")]
    NewLayer(#[from] NewLayerError),

    #[error("IO system error")]
    Io(#[from] io::Error),
}

#[derive(Debug, Error)]
pub enum AddEffectError {
    #[error("cannot apply effect to specified layer")]
    Select(#[from] SelectLayerError),

    #[error("failed to load shader from file")]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum ReorderLayersError {
    #[error("cannot move layer")]
    SrcIndexOutOfBounds(SelectLayerError),

    #[error("cannot replace layer")]
    DstIndexOutOfBounds(SelectLayerError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum RemoveLayerError {
    #[error("cannot remove layer")]
    Select(#[from] SelectLayerError),
}

#[derive(Debug, Error)]
pub enum RunCommandError {
    #[error("failed to create layer")]
    NewLayer(#[from] NewLayerError),

    #[error("invalid layer name")]
    LayerName(#[from] LayerNameError),

    #[error("failed to switch layers")]
    SwitchLayer(#[from] SelectLayerError),

    #[error("failed to open file")]
    OpenFile(#[from] OpenFileError),

    #[error("failed to add effect")]
    AddEffect(#[from] AddEffectError),

    #[error("failed to reorder layers")]
    ReorderLayers(#[from] ReorderLayersError),

    #[error("failed to remove layers")]
    RemoveLayer(#[from] RemoveLayerError),
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("failed to parse command")]
    Parse(#[from] clap::Error),

    #[error("failed to execute command")]
    Run(#[from] RunCommandError),
}

pub fn print_err_recursive(mut e: &dyn error::Error) {
    loop {
        eprint!("{e}");
        if let Some(src) = e.source() {
            eprint!(": ");
            e = src;
        } else {
            break;
        }
    }
    eprintln!();
}
