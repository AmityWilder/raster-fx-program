use crate::{error::*, layer::Layer};
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
    pub fn new_layer(self, curr_layer: usize, layer_count: usize) -> Result<usize, NewLayerError> {
        use NewLayerError::*;
        match self {
            LayerPos::Below => Ok(curr_layer.saturating_sub(1)),

            LayerPos::Above => {
                if layer_count == usize::MAX {
                    Err(TooManyLayers)
                } else {
                    // ensures
                    // 1. soundness
                    // 2. we don't need to .min(layer_count)
                    assert!(
                        curr_layer < layer_count,
                        "curr_layer should always be a valid index"
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

    pub fn switch_layer(
        self,
        curr_layer: usize,
        layer_count: usize,
    ) -> Result<usize, SwitchLayerError> {
        use SwitchLayerError::*;
        match self {
            LayerPos::Below => curr_layer.checked_sub(1).ok_or(IndexOutOfBounds(None)),

            LayerPos::Above => {
                assert!(
                    curr_layer < layer_count,
                    "curr_layer should always be a valid index"
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
        layers: &mut Vec<Layer>,
        curr_layer: &mut usize,
    ) -> Result<ControlFlow<()>, RunCommandError> {
        use RunCommandError::*;
        match self {
            Command::ListLayers => {
                println!("\x1b[96mlayers: {{");
                for (i, layer) in layers.iter().enumerate().rev() {
                    let (open, close) = if i == *curr_layer {
                        ('[', ']')
                    } else {
                        (' ', ' ')
                    };
                    println!("  {open}{i}{close}:\x1b[0m {}\x1b[96m", layer.name);
                }
                println!("}}\x1b[0m");
            }

            Command::NewLayer { at, names } => {
                for name in names {
                    name.map_err(LayerName).and_then(|name| {
                        at.new_layer(*curr_layer, layers.len())
                            .map(|pos| {
                                let new_layer = &*layers.insert_mut(pos, Layer::with_name(name));
                                *curr_layer = pos;
                                println!("\x1b[96mcreated layer\x1b[0m \"{}\"", new_layer.name);
                            })
                            .map_err(NewLayer)
                    })?;
                }
            }

            Command::SwitchLayer { to } => {
                to.switch_layer(*curr_layer, layers.len())
                    .map(|pos| *curr_layer = pos)
                    .map_err(SwitchLayer)?;
            }

            Command::Open { path } => {
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
                    .map_err(|e| OpenFile(OpenFileError::Io(e)))
                    .and_then(|_data| match path.extension() {
                        Some(ext) if ext.eq_ignore_ascii_case("amyfx") => {
                            println!("amyfx: not yet implemented");
                            Err(OpenFile(OpenFileError::Invalid))
                        }
                        Some(ext) if ext.eq_ignore_ascii_case("png") => {
                            println!("png: not yet implemented");
                            Err(OpenFile(OpenFileError::Invalid))
                        }
                        _ => Err(OpenFile(OpenFileError::Unsupported)),
                    })?;
            }

            Command::Quit => {
                return Ok(ControlFlow::Break(()));
            }
        }
        Ok(ControlFlow::Continue(()))
    }
}
