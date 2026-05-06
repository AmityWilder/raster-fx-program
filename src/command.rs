use crate::{
    error::*,
    layer::{Layer, LayerContent, Raster},
};
use clap::Parser;
use raylib::prelude::*;
use std::{fs, ops::ControlFlow, path::PathBuf, str::FromStr};

/// A string that is not allowed to be empty or contain illegal characters
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct LayerName(String);

impl LayerName {
    pub const ILLEGAL_CHARS: [char; 6] = ['\n', '\r', '\t', '\\', '"', '\''];
}

impl FromStr for LayerName {
    type Err = LayerNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use LayerNameError::*;
        let s = s.trim();
        if s.is_empty() {
            Err(Empty)
        } else if s.contains(LayerName::ILLEGAL_CHARS) {
            Err(Illegal)
        } else {
            Ok(Self(s.to_string()))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerPos {
    Below,
    Above,
    Index(usize),
}

impl FromStr for LayerPos {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "k" => Ok(Self::Above),
            "j" => Ok(Self::Below),
            _ => s.parse().map(Self::Index),
        }
    }
}

impl LayerPos {
    pub fn new_layer_idx(
        self,
        curr_layer: usize,
        layer_count: usize,
    ) -> Result<usize, NewLayerError> {
        use NewLayerError::*;
        match self {
            LayerPos::Below => Ok(curr_layer.saturating_sub(1)),

            LayerPos::Above => {
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

            LayerPos::Index(idx) => {
                if idx <= layer_count {
                    Ok(idx)
                } else {
                    Err(IndexOutOfBounds(idx))
                }
            }
        }
    }

    pub fn switch_layer_idx(
        self,
        curr_layer: usize,
        layer_count: usize,
    ) -> Result<usize, SwitchLayerError> {
        use SwitchLayerError::*;
        match self {
            LayerPos::Below => curr_layer.checked_sub(1).ok_or(IndexOutOfBounds(None)),

            LayerPos::Above => {
                if layer_count == 0 {
                    assert_eq!(
                        curr_layer, 0,
                        "curr_layer should always be zero if there are no layers"
                    );
                    Err(IndexOutOfBounds(Some(0)))
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
                        Err(IndexOutOfBounds(Some(idx)))
                    }
                }
            }

            LayerPos::Index(idx) => {
                if idx < layer_count {
                    Ok(idx)
                } else {
                    Err(IndexOutOfBounds(Some(idx)))
                }
            }
        }
    }
}

/// Commands
#[derive(Debug, Clone, Parser)]
#[command(version)]
pub enum Command {
    #[command(name = "ls")]
    ListLayers {},

    #[command(name = "mk")]
    NewLayers { at: LayerPos, names: Vec<LayerName> },

    #[command(name = "rm")]
    RemoveLayers {
        /// empty implies current
        position: Vec<usize>,
    },

    #[command(name = "cd")]
    SwitchLayer { to: LayerPos },

    #[command(name = "open")]
    Open { path: PathBuf },

    #[command(name = "quit")]
    Quit {},
}

impl Command {
    pub fn run(
        self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        layers: &mut Vec<Layer>,
        curr_layer: &mut usize,
    ) -> Result<ControlFlow<()>, RunCommandError> {
        use RunCommandError::*;
        match self {
            Self::ListLayers {} => list_layers(layers, *curr_layer),
            Self::NewLayers { at, names } => new_layers(rl, thread, layers, curr_layer, at, names)?,
            Self::RemoveLayers { position } => {
                remove_layers(layers, curr_layer, position).map_err(RemoveLayer)?
            }
            Self::SwitchLayer { to } => {
                switch_layer(layers, curr_layer, to).map_err(SwitchLayer)?
            }
            Self::Open { path } => open(rl, thread, layers, curr_layer, path).map_err(OpenFile)?,
            Self::Quit {} => return Ok(ControlFlow::Break(())),
        }
        Ok(ControlFlow::Continue(()))
    }
}

fn list_layers(layers: &[Layer], curr_layer: usize) {
    println!("\x1b[96mlayers: {{");
    for (i, layer) in layers.iter().enumerate().rev() {
        let (open, close) = if i == curr_layer {
            ('[', ']')
        } else {
            (' ', ' ')
        };
        println!("  {open}{i}{close}:\x1b[0m {}\x1b[96m", layer.name);
    }
    println!("}}\x1b[0m");
}

fn new_layers(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    layers: &mut Vec<Layer>,
    curr_layer: &mut usize,
    at: LayerPos,
    names: Vec<LayerName>,
) -> Result<(), NewLayerError> {
    names.into_iter().try_for_each(|name| {
        Raster::new(rl, thread, 0, 0)
            .map_err(NewLayerError::Raylib)
            .and_then(|content| {
                new_layer(
                    layers,
                    curr_layer,
                    at,
                    name.0,
                    LayerContent::Raster(content),
                )
            })
            .map(|_| ())
    })
}

fn new_layer<'a>(
    layers: &'a mut Vec<Layer>,
    curr_layer: &mut usize,
    at: LayerPos,
    name: String,
    content: LayerContent,
) -> Result<&'a mut Layer, NewLayerError> {
    at.new_layer_idx(*curr_layer, layers.len()).map(|pos| {
        let new_layer = layers.insert_mut(pos, Layer::with_name(name, content));
        *curr_layer = pos;
        println!("\x1b[96mcreated layer\x1b[0m \"{}\"", new_layer.name);
        new_layer
    })
}

fn remove_layers(
    layers: &mut Vec<Layer>,
    curr_layer: &mut usize,
    mut positions: Vec<usize>,
) -> Result<(), RemoveLayerError> {
    use RemoveLayerError::*;
    let mut it = 0..;
    positions.sort();
    positions.dedup();
    match positions.last().copied() {
        Some(n) => {
            if n < layers.len() {
                *curr_layer -= positions
                    .iter()
                    .copied()
                    .take_while(|i| i <= curr_layer)
                    .count();
                let mut pos = positions.into_iter().peekable();
                layers.retain(|_| pos.next_if_eq(&it.next().unwrap()).is_some());
                Ok(())
            } else {
                Err(IndexOutOfBounds(n))
            }
        }
        None => {
            if layers.is_empty() {
                assert_eq!(
                    *curr_layer, 0,
                    "curr_layer should always be zero if there are no layers"
                );
                Err(IndexOutOfBounds(0))
            } else {
                assert!(
                    *curr_layer < layers.len(),
                    "curr_layer should always be a valid index if there are layers"
                );
                layers.remove(*curr_layer);
                *curr_layer = (*curr_layer).min(layers.len().saturating_sub(1));
                Ok(())
            }
        }
    }
}

fn switch_layer(
    layers: &[Layer],
    curr_layer: &mut usize,
    to: LayerPos,
) -> Result<(), SwitchLayerError> {
    to.switch_layer_idx(*curr_layer, layers.len())
        .map(|pos| *curr_layer = pos)
}

fn open(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    layers: &mut Vec<Layer>,
    curr_layer: &mut usize,
    path: PathBuf,
) -> Result<(), OpenFileError> {
    use OpenFileError::*;
    {
        let display_path;
        println!(
            "loading {:?}",
            match path.canonicalize() {
                Ok(canon_path) => {
                    display_path = canon_path;
                    &display_path
                }
                Err(_) => &path,
            }
            .display()
        );
    }

    fs::read(&path)
        .map_err(Io)
        .and_then(|data| match path.extension() {
            Some(ext) if ext.eq_ignore_ascii_case("amyfx") => open_amyfx(),
            Some(ext) if ext.eq_ignore_ascii_case("png") => {
                open_png(rl, thread, layers, curr_layer, path, &data)
            }
            _ => Err(Unsupported),
        })
}

fn open_amyfx() -> Result<(), OpenFileError> {
    use OpenFileError::*;
    println!("amyfx: not yet implemented");
    Err(Invalid)
}

fn open_png(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    layers: &mut Vec<Layer>,
    curr_layer: &mut usize,
    path: PathBuf,
    data: &[u8],
) -> Result<(), OpenFileError> {
    use OpenFileError::*;

    let raster = Image::load_image_from_mem(".png", data)
        .and_then(|img| Raster::from_image(rl, thread, &img))
        .map_err(LoadImage)?;

    new_layer(
        layers,
        curr_layer,
        LayerPos::Above,
        path.file_name()
            .expect("file should have file name")
            .to_string_lossy()
            .to_string(),
        LayerContent::Raster(raster),
    )
    .map_err(NewLayer)
    .map(|_| ())
}
