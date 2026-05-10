use crate::{
    command::error::OpenFileError,
    error::IndexError,
    layer::{LoadError, SaveError, rtex_from_image},
};
use clap::Args;
use raylib::prelude::*;
use std::{cell::RefCell, collections::TryReserveError, fs, path::PathBuf, rc::Rc, str::FromStr};
use thiserror::Error;

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

#[derive(Debug, Clone)]
pub enum RasterSrc {
    File(PathBuf),
    /// TODO: can't just be a position, layers can be reordered
    Layer(()),
}

impl RasterSrc {
    fn load(
        &self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
    ) -> Result<RenderTexture2D, OpenFileError> {
        match self {
            Self::File(path) => fs::read(path)
                .map_err(OpenFileError::Io)
                .and_then(|ref data| match path.extension() {
                    Some(ext) if ext.eq_ignore_ascii_case("png") => {
                        Image::load_image_from_mem(".png", data)
                            .and_then(|img| rtex_from_image(rl, thread, &img))
                            .map_err(OpenFileError::LoadImage)
                    }

                    _ => Err(OpenFileError::Unsupported),
                }),

            Self::Layer(()) => todo!(),
        }
    }
}

#[derive(Debug, Clone, Args)]
pub struct ShaderSrc {
    /// Path to the vertex shader code file
    #[arg(short, long = "vert")]
    pub vs_path: Option<PathBuf>,

    /// Path to the fragment shader code file
    #[arg(short, long = "frag")]
    pub fs_path: Option<PathBuf>,
}

impl ShaderSrc {
    fn load(&self, rl: &mut RaylibHandle, thread: &RaylibThread) -> Result<Shader, OpenFileError> {
        let (vs_code, fs_code);
        Ok(rl.load_shader_from_memory(
            thread,
            match &self.vs_path {
                Some(path) => {
                    vs_code = fs::read_to_string(path)?;
                    Some(vs_code.as_str())
                }
                None => None,
            },
            match &self.fs_path {
                Some(path) => {
                    fs_code = fs::read_to_string(path)?;
                    Some(fs_code.as_str())
                }
                None => None,
            },
        ))
    }
}

#[derive(Debug)]
enum AssetContent {
    Raster {
        src: RasterSrc,
        rtex: Rc<RefCell<RenderTexture2D>>,
    },
    Shader {
        src: ShaderSrc,
        shader: Rc<RefCell<Shader>>,
    },
}

#[derive(Debug)]
pub enum AssetRef<'a> {
    Raster(&'a Rc<RefCell<RenderTexture2D>>),
    Shader(&'a Rc<RefCell<Shader>>),
}

#[derive(Debug)]
pub struct Asset {
    pub name: String,
    data: AssetContent,
}

impl Asset {
    pub fn new_raster(name: String, src: RasterSrc, rtex: RenderTexture2D) -> Self {
        Self {
            name,
            data: AssetContent::Raster {
                src,
                rtex: Rc::new(RefCell::new(rtex)),
            },
        }
    }

    pub fn load_raster(
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        name: String,
        src: RasterSrc,
    ) -> Result<Self, OpenFileError> {
        src.load(rl, thread)
            .map(|rtex| Self::new_raster(name, src, rtex))
    }

    pub fn new_shader(name: String, src: ShaderSrc, shader: Shader) -> Self {
        Self {
            name,
            data: AssetContent::Shader {
                src,
                shader: Rc::new(RefCell::new(shader)),
            },
        }
    }

    pub fn load_shader(
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        name: String,
        src: ShaderSrc,
    ) -> Result<Self, OpenFileError> {
        src.load(rl, thread)
            .map(|shader| Self::new_shader(name, src, shader))
    }

    pub fn reload(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
    ) -> Result<(), OpenFileError> {
        match &mut self.data {
            AssetContent::Raster { src, rtex } => src
                .load(rl, thread)
                .map(|new_rtex| *rtex.borrow_mut() = new_rtex),

            AssetContent::Shader { src, shader } => src
                .load(rl, thread)
                .map(|new_shader| *shader.borrow_mut() = new_shader),
        }
    }

    pub const fn link_ref(&self) -> AssetRef<'_> {
        match &self.data {
            AssetContent::Raster { rtex, .. } => AssetRef::Raster(rtex),
            AssetContent::Shader { shader, .. } => AssetRef::Shader(shader),
        }
    }

    pub fn save<W: std::io::Write>(&self, dst: &mut W) -> Result<(), SaveError> {
        let Self { name, data } = self;
        dst.write_all(&u64::try_from(name.len())?.to_le_bytes())?;
        dst.write_all(name.as_bytes())?;
        match data {
            AssetContent::Raster { src, rtex: _ } => match src {
                RasterSrc::File(path) => {
                    dst.write_all(b"r")?;
                    let path = path.canonicalize()?; // defend against amyfx file getting moved (we cant deal with the resource moving)
                    let path = path.as_os_str().as_encoded_bytes();
                    dst.write_all(&path.len().to_le_bytes())?;
                    dst.write_all(path)?;
                }
                RasterSrc::Layer(_) => println!("not yet implemented"),
            },

            AssetContent::Shader {
                src: ShaderSrc { vs_path, fs_path },
                shader: _,
            } => {
                dst.write_all(&[match (vs_path.is_some(), fs_path.is_some()) {
                    (true, true) => b't',
                    (true, false) => b'v',
                    (false, true) => b'f',
                    (false, false) => b's',
                }])?;
                if let Some(path) = vs_path {
                    let path = path.canonicalize()?; // defend against amyfx file getting moved (we cant deal with the resource moving)
                    let path = path.as_os_str().as_encoded_bytes();
                    dst.write_all(&path.len().to_le_bytes())?;
                    dst.write_all(path)?;
                }
                if let Some(path) = fs_path {
                    let path = path.canonicalize()?; // defend against amyfx file getting moved (we cant deal with the resource moving)
                    let path = path.as_os_str().as_encoded_bytes();
                    dst.write_all(&path.len().to_le_bytes())?;
                    dst.write_all(path)?;
                }
            }
        }
        Ok(())
    }

    pub fn load<R: std::io::Read>(
        src: &mut R,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
    ) -> Result<Self, LoadError> {
        let mut name_len_bytes = [0; _];
        src.read_exact(&mut name_len_bytes)?;
        let name_len = u64::from_le_bytes(name_len_bytes).try_into()?;
        let mut name_bytes = vec![0; name_len];
        src.read_exact(&mut name_bytes)?;
        let name = String::from_utf8(name_bytes)?;
        let mut tag = 0;
        src.read_exact(std::slice::from_mut(&mut tag))?;
        // TODO: make this less gross
        match tag {
            b'r' => {
                let mut path_len_bytes = [0; _];
                src.read_exact(&mut path_len_bytes)?;
                let path_len = u64::from_le_bytes(path_len_bytes).try_into()?;
                let mut path_bytes = vec![0; path_len];
                src.read_exact(&mut path_bytes)?;
                // SAFETY: TBD
                let path = PathBuf::from(unsafe {
                    std::ffi::OsString::from_encoded_bytes_unchecked(path_bytes)
                });
                Self::load_raster(rl, thread, name, RasterSrc::File(path))
                    .map_err(LoadError::OpenFile)
            }

            b't' => {
                let mut path_len_bytes = [0; _];
                src.read_exact(&mut path_len_bytes)?;
                let path_len = u64::from_le_bytes(path_len_bytes).try_into()?;
                let mut path_bytes = vec![0; path_len];
                src.read_exact(&mut path_bytes)?;
                // SAFETY: TBD
                let vs_path = PathBuf::from(unsafe {
                    std::ffi::OsString::from_encoded_bytes_unchecked(path_bytes)
                });
                let mut path_len_bytes = [0; _];
                src.read_exact(&mut path_len_bytes)?;
                let path_len = u64::from_le_bytes(path_len_bytes).try_into()?;
                let mut path_bytes = vec![0; path_len];
                src.read_exact(&mut path_bytes)?;
                // SAFETY: TBD
                let fs_path = PathBuf::from(unsafe {
                    std::ffi::OsString::from_encoded_bytes_unchecked(path_bytes)
                });
                Self::load_shader(
                    rl,
                    thread,
                    name,
                    ShaderSrc {
                        vs_path: Some(vs_path),
                        fs_path: Some(fs_path),
                    },
                )
                .map_err(LoadError::OpenFile)
            }

            b'v' => {
                let mut path_len_bytes = [0; _];
                src.read_exact(&mut path_len_bytes)?;
                let path_len = u64::from_le_bytes(path_len_bytes).try_into()?;
                let mut path_bytes = vec![0; path_len];
                src.read_exact(&mut path_bytes)?;
                // SAFETY: TBD
                let path = PathBuf::from(unsafe {
                    std::ffi::OsString::from_encoded_bytes_unchecked(path_bytes)
                });
                Self::load_shader(
                    rl,
                    thread,
                    name,
                    ShaderSrc {
                        vs_path: Some(path),
                        fs_path: None,
                    },
                )
                .map_err(LoadError::OpenFile)
            }

            b'f' => {
                let mut path_len_bytes = [0; _];
                src.read_exact(&mut path_len_bytes)?;
                let path_len = u64::from_le_bytes(path_len_bytes).try_into()?;
                let mut path_bytes = vec![0; path_len];
                src.read_exact(&mut path_bytes)?;
                // SAFETY: TBD
                let path = PathBuf::from(unsafe {
                    std::ffi::OsString::from_encoded_bytes_unchecked(path_bytes)
                });
                Self::load_shader(
                    rl,
                    thread,
                    name,
                    ShaderSrc {
                        vs_path: None,
                        fs_path: Some(path),
                    },
                )
                .map_err(LoadError::OpenFile)
            }

            b's' => Self::load_shader(
                rl,
                thread,
                name,
                ShaderSrc {
                    vs_path: None,
                    fs_path: None,
                },
            )
            .map_err(LoadError::OpenFile),

            _ => todo!("tag: {tag:#X}"), // Err(LoadError::Invalid),
        }
    }
}

/// Uses [`AssetPos`] instead of [`usize`]
#[derive(Debug, Default)]
pub struct Assets {
    list: Vec<Asset>,
}

impl Assets {
    pub const fn new() -> Self {
        Self { list: Vec::new() }
    }

    pub fn save<W: std::io::Write>(&self, dst: &mut W) -> Result<(), SaveError> {
        dst.write_all(&u64::try_from(self.list.len())?.to_le_bytes())?;
        for asset in &self.list {
            asset.save(dst)?;
        }
        Ok(())
    }

    pub fn load<R: std::io::Read>(
        src: &mut R,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
    ) -> Result<Self, LoadError> {
        let mut list_len_bytes = [0; _];
        src.read_exact(&mut list_len_bytes)?;
        let list_len = u64::from_le_bytes(list_len_bytes).try_into()?;
        let list = std::iter::repeat_with(|| Asset::load(src, rl, thread))
            .take(list_len)
            .collect::<Result<_, _>>()?;
        Ok(Self { list })
    }

    pub const fn len(&self) -> usize {
        self.list.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Asset> {
        self.list.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Asset> {
        self.list.iter_mut()
    }

    /// # Panics
    /// This method may panic if [`AssetPos::select_asset_idx`] is incorrectly implemented
    pub fn get(&self, at: AssetPos) -> Result<&Asset, SelectAssetError> {
        at.select_asset_idx(self.list.len()).map(|idx| {
            self.list
                .get(idx)
                .expect("AssetPos should be a valid index in the non-error branch")
        })
    }

    /// # Panics
    /// This method may panic if [`AssetPos::select_asset_idx`] is incorrectly implemented
    pub fn get_mut(&mut self, at: AssetPos) -> Result<&mut Asset, SelectAssetError> {
        at.select_asset_idx(self.list.len()).map(|idx| {
            self.list
                .get_mut(idx)
                .expect("AssetPos should be a valid index in the non-error branch")
        })
    }

    pub fn push(&mut self, asset: Asset) -> Result<&mut Asset, TryReserveError> {
        if self.list.len() == self.list.capacity() {
            self.list.try_reserve(self.list.len().max(1))?;
        }
        Ok(self.list.push_mut(asset))
    }

    /// Returns [`None`] if the asset isn't in the list.
    /// This could be because the asset has been removed while an upgraded clone exists.
    pub fn raster_pos(&self, asset: &Rc<RefCell<RenderTexture2D>>) -> Option<AssetPos> {
        self.list
            .iter()
            .position(|x| {
                matches!(&x.data,
                AssetContent::Raster { rtex, .. } if Rc::ptr_eq(rtex, asset))
            })
            .map(AssetPos::Index)
    }

    /// Returns [`None`] if the asset isn't in the list.
    /// This could be because the asset has been removed while an upgraded clone exists.
    pub fn shader_pos(&self, asset: &Rc<RefCell<Shader>>) -> Option<AssetPos> {
        self.list
            .iter()
            .position(|x| {
                matches!(&x.data,
                AssetContent::Shader { shader, .. } if Rc::ptr_eq(shader, asset))
            })
            .map(AssetPos::Index)
    }
}
