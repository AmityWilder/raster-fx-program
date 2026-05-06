use crate::{
    error::*,
    layer::{Layer, LayerContent},
};
use raylib::prelude::*;
use std::{fs, ops::ControlFlow, path::PathBuf, str::FromStr};

pub const ILLEGAL_LAYER_NAME_CHARS: [char; 6] = ['\n', '\r', '\t', '\\', '"', '\''];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LayerPos {
    #[default]
    Below,
    Above,
    Index(usize),
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

#[derive(Debug, Clone)]
pub enum Command {
    ListLayers,
    NewLayer {
        at: LayerPos,
        names: Vec<Result<String, LayerNameError>>,
    },
    SwitchLayer {
        to: LayerPos,
    },
    Open {
        path: PathBuf,
    },
    Quit,
}

impl FromStr for Command {
    type Err = ParseCommandError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use ParseCommandError::*;
        let (cmd, args) = s.split_once(' ').unwrap_or((s, ""));
        let mut args = args.split(','); // todo: make this more advanced
        match cmd {
            "ls" => Ok(Self::ListLayers),

            "open" => {
                #[allow(non_upper_case_globals)]
                const expect: &str = "1";
                let path = args.next().ok_or(BadArgCount { actual: 0, expect })?;
                match args.count() {
                    0 => Ok(Self::Open { path: path.into() }),
                    remaining => Err(BadArgCount {
                        actual: remaining + 1,
                        expect,
                    }),
                }
            }

            "k" => Ok(Self::SwitchLayer {
                to: LayerPos::Above,
            }),

            "j" => Ok(Self::SwitchLayer {
                to: LayerPos::Below,
            }),

            _ if let Some(n) = cmd.strip_suffix('G').or_else(|| cmd.strip_suffix("gg")) => n
                .parse()
                .map(|idx| Self::SwitchLayer {
                    to: LayerPos::Index(idx),
                })
                .map_err(ParseInt),

            "o" | "O" => Ok(Self::NewLayer {
                at: match cmd {
                    "o" => LayerPos::Below,
                    "O" => LayerPos::Above,
                    _ => unreachable!(),
                },
                names: args
                    .map(|mut arg| {
                        use LayerNameError::*;
                        arg = arg.trim();
                        if arg.is_empty() {
                            Err(Empty)
                        } else if arg.contains(ILLEGAL_LAYER_NAME_CHARS) {
                            Err(Illegal)
                        } else {
                            Ok(arg.to_string())
                        }
                    })
                    .collect(),
            }),

            "q" => Ok(Self::Quit),

            _ => Err(UnknownCommand {
                cmd: cmd.to_string(),
            }),
        }
    }
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
            Self::ListLayers => list_layers(layers, *curr_layer),
            Self::NewLayer { at, names } => new_layers(layers, curr_layer, at, names)?,
            Self::SwitchLayer { to } => switch_layer(layers, curr_layer, to)?,
            Self::Open { path } => open(rl, thread, layers, curr_layer, path)?,
            Self::Quit => return Ok(ControlFlow::Break(())),
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
    layers: &mut Vec<Layer>,
    curr_layer: &mut usize,
    at: LayerPos,
    names: Vec<Result<String, LayerNameError>>,
) -> Result<(), RunCommandError> {
    use RunCommandError::*;
    names.into_iter().try_for_each(|name| {
        name.map_err(LayerName)
            .and_then(|name| new_layer(layers, curr_layer, at, name).map(|_| ()))
    })
}

fn new_layer<'a>(
    layers: &'a mut Vec<Layer>,
    curr_layer: &mut usize,
    at: LayerPos,
    name: String,
) -> Result<&'a mut Layer, RunCommandError> {
    use RunCommandError::*;
    at.new_layer_idx(*curr_layer, layers.len())
        .map(|pos| {
            let new_layer = layers.insert_mut(pos, Layer::with_name(name));
            *curr_layer = pos;
            println!("\x1b[96mcreated layer\x1b[0m \"{}\"", new_layer.name);
            new_layer
        })
        .map_err(NewLayer)
}

fn switch_layer(
    layers: &[Layer],
    curr_layer: &mut usize,
    to: LayerPos,
) -> Result<(), RunCommandError> {
    use RunCommandError::*;
    to.switch_layer_idx(*curr_layer, layers.len())
        .map(|pos| *curr_layer = pos)
        .map_err(SwitchLayer)
}

fn open(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    layers: &mut Vec<Layer>,
    curr_layer: &mut usize,
    path: PathBuf,
) -> Result<(), RunCommandError> {
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
        .map_err(|e| RunCommandError::OpenFile(OpenFileError::Io(e)))
        .and_then(|data| match path.extension() {
            Some(ext) if ext.eq_ignore_ascii_case("amyfx") => open_amyfx(),
            Some(ext) if ext.eq_ignore_ascii_case("png") => {
                open_png(rl, thread, layers, curr_layer, path, &data)
            }
            _ => Err(RunCommandError::OpenFile(OpenFileError::Unsupported)),
        })
}

fn open_amyfx() -> Result<(), RunCommandError> {
    println!("amyfx: not yet implemented");
    Err(RunCommandError::OpenFile(OpenFileError::Invalid))
}

fn open_png(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    layers: &mut Vec<Layer>,
    curr_layer: &mut usize,
    path: PathBuf,
    data: &[u8],
) -> Result<(), RunCommandError> {
    let rtex = Image::load_image_from_mem(".png", data)
        .and_then(|img| {
            let mut rtex = rl.load_render_texture(
                thread,
                img.width
                    .try_into()
                    .expect("image should not have negative width"),
                img.height
                    .try_into()
                    .expect("image should not have negative height"),
            )?;
            assert!(img.is_image_valid(), "img should be valid");
            assert!(
                !img.data.is_null() && img.data.is_aligned(),
                "img data pointer should be valid"
            );
            // SAFETY: this is the definition of img.data according to the Raylib source code.
            // The only reason it isn't safe in Raylib-rs is because nobody bothered to check.
            let pixels =
                unsafe { std::slice::from_raw_parts(img.data.cast(), img.get_pixel_data_size()) };
            rtex.update_texture(pixels)?;
            Ok(rtex)
        })
        .map_err(OpenFileError::LoadImage)?;

    new_layer(
        layers,
        curr_layer,
        LayerPos::Above,
        path.file_name()
            .expect("file should have file name")
            .to_string_lossy()
            .to_string(),
    )
    .map(|layer| {
        layer.content = LayerContent::Raster(rtex);
    })
}
