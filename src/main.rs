#![deny(clippy::undocumented_unsafe_blocks)]

use rand::prelude::*;
use raylib::prelude::*;
use std::{
    collections::VecDeque,
    error, fmt, fs,
    io::{self, stdin},
    path::{Path, PathBuf},
    str::FromStr,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum NewLayerError {
    TooManyLayers,
    IndexOutOfBounds(usize),
}

impl fmt::Display for NewLayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyLayers => write!(
                f,
                "cannot exceed {} layers on a {}-bit system",
                const { usize::MAX },
                const { std::mem::size_of::<usize>() * 8 }
            ),
            Self::IndexOutOfBounds(idx) => write!(f, "{idx} is out of bounds"),
        }
    }
}

impl error::Error for NewLayerError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SwitchLayerError {
    IndexOutOfBounds(Option<usize>),
}

impl fmt::Display for SwitchLayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IndexOutOfBounds(idx) => {
                match idx {
                    Some(idx) => idx.fmt(f),
                    None => (-1).fmt(f),
                }?;
                write!(f, " is out of bounds")
            }
        }
    }
}

impl error::Error for SwitchLayerError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
enum LayerPos {
    #[default]
    Below,
    Above,
    Index(usize),
}

impl LayerPos {
    fn new_layer(self, curr_layer: usize, layer_count: usize) -> Result<usize, NewLayerError> {
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

    fn switch_layer(
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

const ILLEGAL_LAYER_NAME_CHARS: [char; 4] = ['\\', '"', '\n', '\r'];

#[derive(Debug, Clone)]
enum LayerNameError {
    Empty,
    Illegal(String),
}

impl fmt::Display for LayerNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("layer name cannot be entirely empty or whitespace"),
            Self::Illegal(name) => write!(
                f,
                "the name {name:?} contains illegal characters; layers cannot contain {ILLEGAL_LAYER_NAME_CHARS:?}"
            ),
        }
    }
}

impl error::Error for LayerNameError {}

#[derive(Debug, Clone)]
enum Command {
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

#[derive(Debug, Clone)]
enum ParseCommandError {
    UnknownCommand { cmd: String },
    BadArgCount { actual: usize, expect: &'static str },
    ParseInt(std::num::ParseIntError),
}

impl From<std::num::ParseIntError> for ParseCommandError {
    fn from(e: std::num::ParseIntError) -> Self {
        Self::ParseInt(e)
    }
}

impl fmt::Display for ParseCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCommand { cmd } => write!(f, "no such command: `{cmd}`"),
            Self::BadArgCount { actual, expect } => {
                write!(f, "bad arg count: expected {expect}, got {actual}")
            }
            Self::ParseInt(_) => write!(f, "failed to parse integer"),
        }
    }
}

impl error::Error for ParseCommandError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::ParseInt(e) => Some(e),
            _ => None,
        }
    }
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
                            return Err(Empty);
                        }
                        let name = arg.to_string();
                        if name.contains(ILLEGAL_LAYER_NAME_CHARS) {
                            return Err(Illegal(name));
                        }
                        Ok(name)
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

#[derive(Debug, Clone, PartialEq, Default)]
struct Layer {
    name: String,
}

fn main() {
    let stdin_channel = {
        let (tx, rx) = mpsc::channel::<String>();
        thread::spawn(move || {
            loop {
                let mut buffer = String::new();
                stdin().read_line(&mut buffer).unwrap();
                if buffer.ends_with('\n') {
                    buffer.pop();
                    #[cfg(windows)]
                    if buffer.ends_with('\r') {
                        buffer.pop();
                    }
                }
                tx.send(buffer).unwrap();
            }
        });
        rx
    };
    let (mut rl, thread) = init().title("Amity FX").size(1280, 720).resizable().build();

    rl.set_target_fps(60);

    let mut history: VecDeque<String> = VecDeque::new();
    let mut layers: Vec<Layer> = Vec::new();
    let mut curr_layer: usize = 0;

    'mainloop: while !rl.window_should_close() {
        match stdin_channel.try_recv() {
            Ok(input) => {
                if false {
                    println!("Received `{input}`");
                }
                let input = &*history.push_back_mut(input);
                match input.parse() {
                    Ok(cmd) => match cmd {
                        Command::ListLayers => {
                            println!("layers: {{");
                            for (i, Layer { name, .. }) in layers.iter().enumerate().rev() {
                                let (open, close) = if i == curr_layer {
                                    ('[', ']')
                                } else {
                                    (' ', ' ')
                                };
                                println!("  {open}{i}{close}: {name}");
                            }
                            println!("}}");
                        }

                        Command::NewLayer { at, names } => {
                            for name in names {
                                match name {
                                    Ok(name) => match at.new_layer(curr_layer, layers.len()) {
                                        Ok(pos) => {
                                            let new_layer =
                                                &*layers.insert_mut(pos, Layer { name });
                                            curr_layer = pos;
                                            println!("created layer \"{}\"", new_layer.name);
                                        }
                                        Err(e) => println!("error creating layer: {e}"),
                                    },

                                    Err(e) => println!("layer name error: {e}"),
                                }
                            }
                        }

                        Command::SwitchLayer { to } => {
                            match to.switch_layer(curr_layer, layers.len()) {
                                Ok(pos) => curr_layer = pos,
                                Err(e) => println!("error switching layers: {e}"),
                            }
                        }

                        Command::Open { path } => match fs::read(path) {
                            Ok(_) => todo!(),
                            Err(e) => println!("error opening file: {e}"),
                        },

                        Command::Quit => {
                            break 'mainloop;
                        }
                    },

                    Err(e) => println!("error parsing command: {e}"),
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                if false {
                    println!("Channel disconnected");
                }
                break 'mainloop;
            }
        }
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
    }
}
