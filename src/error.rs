use std::{error, fmt, io};
use thiserror::Error;

use crate::command::LayerName;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Error)]
pub enum LayerNameError {
    #[error("layer name cannot be entirely empty or whitespace")]
    Empty,

    #[error(
        "layer names cannot contain any of the following characters: {:?}",
        LayerName::ILLEGAL_CHARS
    )]
    Illegal,
}

#[derive(Debug, Error)]
pub enum NewLayerError {
    #[error("cannot exceed {} layers on a {}-bit system", const { usize::MAX }, const { std::mem::size_of::<usize>() * 8 })]
    TooManyLayers,

    #[error(
        "layer {} is out of bounds, must be at most equal to the number of layers",
        .0
    )]
    IndexOutOfBounds(usize),

    #[error("bad layer name")]
    LayerName(#[from] LayerNameError),

    #[error("could not create a new raster")]
    Raylib(#[from] raylib::error::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum SwitchLayerError {
    #[error("layer {} is out of bounds", match .0.as_ref() { Some(idx) => idx as &dyn fmt::Display, None => const { &-1 } })]
    IndexOutOfBounds(Option<usize>),
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

#[derive(Debug, Clone, Error)]
pub enum RemoveLayerError {
    #[error("layer {} cannot be removed, because it does not exist", .0)]
    IndexOutOfBounds(usize),
}

#[derive(Debug, Error)]
pub enum RunCommandError {
    #[error("failed to create layer")]
    NewLayer(#[from] NewLayerError),

    #[error("invalid layer name")]
    LayerName(#[from] LayerNameError),

    #[error("failed to switch layers")]
    SwitchLayer(#[from] SwitchLayerError),

    #[error("failed to remove layers")]
    OpenFile(#[from] OpenFileError),

    #[error("failed to open file")]
    RemoveLayer(#[from] RemoveLayerError),
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("failed to parse command")]
    Parse(#[from] clap::Error),

    #[error("failed to run command")]
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
