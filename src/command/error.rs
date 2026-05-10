use crate::{
    asset::SelectAssetError,
    command::ILLEGAL_LAYER_NAME_CHARS,
    layer::{InsertLayerError, LoadError, SaveError, SelectLayerError},
};
use std::{collections::TryReserveError, io};
use thiserror::Error;

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

#[derive(Debug, Error)]
pub enum NewLayerError {
    #[error("bad layer insertion")]
    InsertLayer(#[from] InsertLayerError),

    #[error("bad layer name")]
    LayerName(#[from] LayerNameError),

    #[error("could not create a new raster")]
    Raylib(#[from] raylib::error::Error),
}

#[derive(Debug, Error)]
pub enum AddEffectError {
    #[error("cannot apply effect to specified layer")]
    Select(#[from] SelectLayerError),

    #[error("failed to load shader from file")]
    Io(#[from] io::Error),
}

#[derive(Debug, Error)]
pub enum ReloadAssetError {
    #[error("cannot reload effects on specified layer")]
    Select(#[from] SelectAssetError),

    #[error("failed to reload shader from file")]
    Io(#[from] io::Error),
}

#[derive(Debug, Error)]
pub enum LinkError {
    #[error("cannot link asset")]
    SelectAsset(#[from] SelectAssetError),

    #[error("cannot link layer")]
    SelectLayer(#[from] SelectLayerError),

    #[error("cannot attach a raster to a group, put a layer inside it and link the asset to that")]
    OverrideGroup,
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

    #[error("out of memory")]
    NoMemory(#[from] TryReserveError),
}

#[derive(Debug, Error)]
pub enum RunCommandError {
    #[error("failed to create layer")]
    NewLayer(#[from] NewLayerError),

    #[error("invalid layer name")]
    LayerName(#[from] LayerNameError),

    #[error("failed to switch layers")]
    SwitchLayer(#[from] SwitchLayerError),

    #[error("failed to open file")]
    OpenFile(#[from] OpenFileError),

    #[error("failed to add effect")]
    AddEffect(#[from] AddEffectError),

    #[error("failed to reload effect")]
    ReloadEffect(#[from] ReloadAssetError),

    #[error("failed to link objects")]
    Link(#[from] LinkError),

    #[error("failed to reorder layers")]
    ReorderLayers(#[from] ReorderLayersError),

    #[error("failed to remove layers")]
    RemoveLayer(#[from] RemoveLayerError),

    #[error("failed to save")]
    Save(#[from] SaveError),

    #[error("failed to load")]
    Load(#[from] LoadError),
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("failed to parse command")]
    Parse(#[from] clap::Error),

    #[error("failed to execute command")]
    Run(#[from] RunCommandError),
}
