use crate::layer::{Effect, EffectBuilder, Layer, rtex_from_image};
use clap::{Parser, ValueHint};
use raylib::prelude::*;
use std::{fmt, fs, io, ops::ControlFlow, path::PathBuf, str::FromStr};
use thiserror::Error;

pub const ILLEGAL_LAYER_NAME_CHARS: [char; 6] = ['\n', '\r', '\t', '\\', '"', ';'];

fn valid_layer_name(s: &str) -> Result<String, LayerNameError> {
    use LayerNameError::*;
    let s = s.trim();
    if s.is_empty() {
        Err(Empty)
    } else if s.contains(ILLEGAL_LAYER_NAME_CHARS) {
        Err(Illegal)
    } else {
        Ok(s.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LayerPos {
    #[default]
    Current,
    Next,
    Prev,
    Front,
    Back,
    Index(usize),
    Offset(isize),
}

impl FromStr for LayerPos {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "*" | "h" | "here" => Ok(Self::Current),
            "k" | "]" | "n" | "next" => Ok(Self::Next),
            "j" | "[" | "p" | "prev" => Ok(Self::Prev),
            "]]" | "f" | "front" => Ok(Self::Front),
            "[[" | "b" | "back" => Ok(Self::Back),
            _ if s.starts_with(['+', '-']) => s.parse().map(Self::Offset),
            _ => s.parse().map(Self::Index),
        }
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum InsertLayerError {
    #[error("cannot exceed {} layers on a {}-bit system", const { usize::MAX }, const { std::mem::size_of::<usize>() * 8 })]
    TooManyLayers,

    #[error("layer position {} is out of bounds, cannot exceed the number of layers", .0)]
    IndexOutOfBounds(IndexError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum SelectLayerError {
    #[error("layer {} does not exist", .0)]
    IndexOutOfBounds(IndexError),
}

impl LayerPos {
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

#[derive(Parser)]
#[command(version)]
pub enum Command {
    /// List the current layers in the open editor
    #[command(name = "layers", visible_alias = "ls")]
    ListLayers {
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        dbg: bool,
    },

    /// Create one or more new layers
    #[command(name = "make", visible_alias = "mk")]
    NewLayer {
        /// Where to put the layer
        at: LayerPos,

        /// The name of the layer to create
        #[arg(value_parser = valid_layer_name)]
        name: String,

        #[arg(short = 'g', long = "group", action = clap::ArgAction::SetTrue)]
        is_group: bool,
    },

    /// Apply an effect to a layer
    #[command(name = "effect", visible_alias = "fx")]
    AddEffect {
        /// Which layer to apply the effect to
        to: LayerPos,

        /// The DNA of the effect to add
        #[command(flatten)]
        effect: EffectBuilder,
    },

    /// Reload all effects on a layer
    #[command(name = "reload", visible_alias = "re")]
    ReloadEffects {
        /// Which layer to reload the effects on
        on: LayerPos,
    },

    /// Create one or more new layers
    #[command(name = "move", visible_alias = "mv")]
    ReorderLayer {
        /// The layer to move
        from: LayerPos,

        /// Where to put it
        to: LayerPos,
    },

    /// Remove one or more layers
    #[command(name = "remove", visible_alias = "rm")]
    RemoveLayers {
        /// List of layer indices to remove
        ///
        /// Empty implies current
        position: Vec<usize>,
    },

    /// Change which layer is currently being targeted
    #[command(name = "switch", visible_alias = "cd")]
    SwitchLayer { to: LayerPos },

    /// Open a file
    ///
    /// If the file is a PNG, it will be inserted into a new layer above the current one
    ///
    /// If it is an AmyFX file, the current file will be closed and the provided one will open
    #[command(name = "open", visible_alias = "o")]
    Open {
        /// Path to the file to open
        #[arg(value_hint = ValueHint::FilePath)]
        path: PathBuf,
    },

    /// Close the application
    #[command(name = "quit", visible_alias = "q")]
    Quit,
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
    ReloadEffect(#[from] ReloadEffectError),

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

impl Command {
    pub fn run(
        self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        layers: &mut Vec<Layer>,
        curr_layer: &mut usize,
    ) -> Result<ControlFlow<()>, RunCommandError> {
        match self {
            Self::ListLayers { dbg } => list_layers(layers, *curr_layer, dbg),
            Self::NewLayer { at, name, is_group } => {
                new_layer(rl, thread, layers, curr_layer, at, name, is_group)?
            }
            Self::AddEffect { to, effect } => {
                add_effect(rl, thread, layers, *curr_layer, to, effect).map(|_| ())?
            }
            Self::ReloadEffects { on } => reload_effects(rl, thread, layers, *curr_layer, on)?,
            Self::ReorderLayer { from, to } => reorder_layers(layers, curr_layer, from, to)?,
            Self::RemoveLayers { position } => remove_layers(layers, curr_layer, position)?,
            Self::SwitchLayer { to } => switch_layer(layers, curr_layer, to)?,
            Self::Open { path } => open(rl, thread, layers, curr_layer, path)?,
            Self::Quit {} => return Ok(ControlFlow::Break(())),
        }
        Ok(ControlFlow::Continue(()))
    }
}

fn list_layers(layers: &[Layer], curr_layer: usize, debug: bool) {
    println!("\x1b[96mlayers: {{\x1b[0m");
    for (i, layer) in layers.iter().enumerate().rev() {
        let (color, open, close) = if i == curr_layer {
            (95, '[', ']')
        } else {
            (92, ' ', ' ')
        };
        print!("  \x1b[{color}m{open}{i}{close}:\x1b[0m ");
        if debug {
            println!("{layer:#?}");
        } else {
            println!("{}", layer.name);
        }
    }
    println!("\x1b[96m}}\x1b[0m");
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

#[derive(Debug, Error)]
pub enum NewLayerError {
    #[error("bad layer insertion")]
    InsertLayer(#[from] InsertLayerError),

    #[error("bad layer name")]
    LayerName(#[from] LayerNameError),

    #[error("could not create a new raster")]
    Raylib(#[from] raylib::error::Error),
}

fn new_layer(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    layers: &mut Vec<Layer>,
    curr_layer: &mut usize,
    mut at: LayerPos,
    name: String,
    is_group: bool,
) -> Result<(), NewLayerError> {
    rl.load_render_texture(thread, 0, 0)
        .map_err(NewLayerError::Raylib)
        .and_then(|buffer| {
            insert_layer(
                layers,
                curr_layer,
                at,
                if is_group {
                    Layer::new_group(name, buffer)
                } else {
                    Layer::new_raster(name, buffer)
                },
            )?;
            at = LayerPos::Next;
            Ok(())
        })
}

fn insert_layer<'a>(
    layers: &'a mut Vec<Layer>,
    curr_layer: &mut usize,
    at: LayerPos,
    layer: Layer,
) -> Result<&'a mut Layer, NewLayerError> {
    at.insert_layer_idx(*curr_layer, layers.len())
        .map(|pos| {
            let new_layer = layers.insert_mut(pos, layer);
            *curr_layer = pos;
            println!("\x1b[96mcreated layer\x1b[0m \"{}\"", new_layer.name);
            new_layer
        })
        .map_err(NewLayerError::InsertLayer)
}

#[derive(Debug, Error)]
pub enum AddEffectError {
    #[error("cannot apply effect to specified layer")]
    Select(#[from] SelectLayerError),

    #[error("failed to load shader from file")]
    Io(#[from] io::Error),
}

fn add_effect<'a>(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    layers: &'a mut [Layer],
    curr_layer: usize,
    to: LayerPos,
    effect: EffectBuilder,
) -> Result<&'a mut Effect, AddEffectError> {
    let idx = to.select_layer_idx(curr_layer, layers.len())?;
    let n = layers[idx].effects.len();
    Ok(layers[idx].effects.push_mut(
        effect
            .build(rl, thread)
            .inspect(|_| println!("\x1b[96meffect {n} added to layer {idx}\x1b[0m"))?,
    ))
}

#[derive(Debug, Error)]
pub enum ReloadEffectError {
    #[error("cannot reload effects on specified layer")]
    Select(#[from] SelectLayerError),

    #[error("failed to reload shader from file")]
    Io(#[from] io::Error),
}

fn reload_effects(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    layers: &mut [Layer],
    curr_layer: usize,
    on: LayerPos,
) -> Result<(), ReloadEffectError> {
    let idx = on.select_layer_idx(curr_layer, layers.len())?;
    let layer = &mut layers[idx];
    if layer.effects.is_empty() {
        println!("\x1b[1;95mwarning:\x1b[0m layer {idx} has no effects")
    } else {
        for (i, effect) in layer.effects.iter_mut().enumerate() {
            effect.reload(rl, thread)?;
            println!("\x1b[96meffect {i} reloaded\x1b[0m");
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum ReorderLayersError {
    #[error("cannot move layer")]
    SrcIndexOutOfBounds(SelectLayerError),

    #[error("cannot replace layer")]
    DstIndexOutOfBounds(SelectLayerError),
}

fn reorder_layers(
    layers: &mut [Layer],
    curr_layer: &mut usize,
    from: LayerPos,
    to: LayerPos,
) -> Result<(), ReorderLayersError> {
    use ReorderLayersError::*;
    use std::cmp::Ordering::*;
    let from = from
        .select_layer_idx(*curr_layer, layers.len())
        .map_err(SrcIndexOutOfBounds)?;
    let to = to
        .select_layer_idx(*curr_layer, layers.len())
        .map_err(DstIndexOutOfBounds)?;
    match from.cmp(&to) {
        Less => layers[from..=to].rotate_left(1),
        Equal => {
            println!("\x1b[1;95mwarning:\x1b[0m layer order unchanged");
        }
        Greater => layers[to..=from].rotate_right(1),
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum RemoveLayerError {
    #[error("cannot remove layer")]
    Select(#[from] SelectLayerError),
}

fn remove_layers(
    layers: &mut Vec<Layer>,
    curr_layer: &mut usize,
    mut positions: Vec<usize>,
) -> Result<(), RemoveLayerError> {
    use RemoveLayerError::*;
    positions.sort();
    positions.dedup();
    match positions.last().copied() {
        Some(n) => {
            if n < layers.len() {
                let mut layer_index = 0..layers.len();
                *curr_layer = (*curr_layer).saturating_sub(
                    positions
                        .iter()
                        .copied()
                        .take_while(|i| i <= curr_layer)
                        .count(),
                );
                let mut pos = positions.into_iter().peekable();
                layers.retain(|layer| {
                    // SAFETY: positions cannot be negative, duplicative, or exceed the maximum layer index.
                    // there cannot be more positions than layers.
                    pos.next_if_eq(&unsafe { layer_index.next().unwrap_unchecked() })
                        .inspect(|_| println!("\x1b[96mremoving\x1b[0m {}", layer.name))
                        .is_some()
                });
                Ok(())
            } else {
                Err(Select(SelectLayerError::IndexOutOfBounds(
                    IndexError::Value(n),
                )))
            }
        }
        None => {
            if layers.is_empty() {
                assert_eq!(
                    *curr_layer, 0,
                    "curr_layer should always be zero if there are no layers"
                );
                Err(Select(SelectLayerError::IndexOutOfBounds(
                    IndexError::Value(0),
                )))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum SwitchLayerError {
    #[error("cannot switch to layer")]
    Select(#[from] SelectLayerError),
}

fn switch_layer(
    layers: &[Layer],
    curr_layer: &mut usize,
    to: LayerPos,
) -> Result<(), SwitchLayerError> {
    to.select_layer_idx(*curr_layer, layers.len())
        .map(|pos| *curr_layer = pos)
        .map_err(SwitchLayerError::Select)
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
            "path resolves to: {:?}",
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

    let buffer = Image::load_image_from_mem(".png", data)
        .and_then(|img| rtex_from_image(rl, thread, &img))
        .map_err(LoadImage)?;

    insert_layer(
        layers,
        curr_layer,
        LayerPos::Next,
        Layer::new_raster(
            path.file_name()
                .expect("file should have file name")
                .to_string_lossy()
                .to_string(),
            buffer,
        ),
    )
    .map_err(NewLayer)
    .map(|_| ())
}
