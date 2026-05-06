use crate::command::ILLEGAL_LAYER_NAME_CHARS;
use std::{error, fmt, io};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NewLayerError {
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
pub enum SwitchLayerError {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LayerNameError {
    Empty,
    Illegal,
}

impl fmt::Display for LayerNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("layer name cannot be entirely empty or whitespace"),
            Self::Illegal => {
                write!(
                    f,
                    "layer names cannot contain any of the following characters: "
                )?;
                let mut has_prev = false;
                for ch in ILLEGAL_LAYER_NAME_CHARS {
                    if has_prev {
                        ' '.fmt(f)?;
                    }
                    has_prev = true;
                    ch.fmt(f)?;
                }
                Ok(())
            }
        }
    }
}

impl error::Error for LayerNameError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseCommandError {
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

#[derive(Debug)]
pub enum OpenFileError {
    Invalid,
    Unsupported,
    LoadImage(raylib::error::Error),
    Io(io::Error),
}

impl From<io::Error> for OpenFileError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl fmt::Display for OpenFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid => "invalid or corrupted file",
            Self::Unsupported => "unsupported file format",
            Self::LoadImage(_) => "failed to load image file",
            Self::Io(_) => "IO system error",
        }
        .fmt(f)
    }
}

impl error::Error for OpenFileError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::LoadImage(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum RunCommandError {
    NewLayer(NewLayerError),
    LayerName(LayerNameError),
    SwitchLayer(SwitchLayerError),
    OpenFile(OpenFileError),
}

impl From<NewLayerError> for RunCommandError {
    fn from(e: NewLayerError) -> Self {
        Self::NewLayer(e)
    }
}
impl From<LayerNameError> for RunCommandError {
    fn from(e: LayerNameError) -> Self {
        Self::LayerName(e)
    }
}
impl From<SwitchLayerError> for RunCommandError {
    fn from(e: SwitchLayerError) -> Self {
        Self::SwitchLayer(e)
    }
}
impl From<OpenFileError> for RunCommandError {
    fn from(e: OpenFileError) -> Self {
        Self::OpenFile(e)
    }
}

impl fmt::Display for RunCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NewLayer(_) => "failed to create layer",
            Self::LayerName(_) => "invalid layer name",
            Self::SwitchLayer(_) => "failed to switch layers",
            Self::OpenFile(_) => "failed to open file",
        }
        .fmt(f)
    }
}

impl error::Error for RunCommandError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(match self {
            Self::NewLayer(e) => e,
            Self::LayerName(e) => e,
            Self::SwitchLayer(e) => e,
            Self::OpenFile(e) => e,
        })
    }
}

#[derive(Debug)]
pub enum CommandError {
    Parse(ParseCommandError),
    Run(RunCommandError),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(_) => "failed to parse command",
            Self::Run(_) => "failed to run command",
        }
        .fmt(f)
    }
}

impl error::Error for CommandError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(match self {
            Self::Parse(e) => e,
            Self::Run(e) => e,
        })
    }
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
