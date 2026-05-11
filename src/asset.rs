use crate::{
    command::error::OpenFileError,
    error::IndexError,
    layer::{LoadError, SaveError, rtex_from_image},
    serde::{Deserialize, DeserializeSlice, Serialize, SerializeSlice},
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

impl Serialize for AssetPos {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        match self {
            AssetPos::Basic => b'*'.serialize(dst, &()),
            AssetPos::Index(x) => b'#'
                .serialize(dst, &())
                .and_then(|()| x.serialize(dst, &())),
        }
    }
}

impl Deserialize for AssetPos {
    fn deserialize<R>(src: &mut R, _: &mut ()) -> std::io::Result<Self>
    where
        Self: Sized,
        R: ?Sized + std::io::Read,
    {
        match u8::deserialize(src, &mut ())? {
            b'*' => Ok(Self::Basic),
            b'#' => Deserialize::deserialize(src, &mut ()).map(Self::Index),
            _ => Err(std::io::Error::other(OpenFileError::Invalid)),
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

impl Serialize for RasterSrc {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        match self {
            Self::File(path) => b'f'
                .serialize(dst, &())
                .and_then(|()| path.serialize(dst, &())),
            Self::Layer(()) => b'l'.serialize(dst, &()).and_then(|()| todo!()),
        }
    }
}

impl Deserialize for RasterSrc {
    fn deserialize<R>(src: &mut R, _: &mut ()) -> std::io::Result<Self>
    where
        Self: Sized,
        R: ?Sized + std::io::Read,
    {
        match u8::deserialize(src, &mut ())? {
            b'f' => PathBuf::deserialize(src, &mut ()).map(Self::File),

            b'l' => todo!(),

            x => Err(std::io::Error::other(format!(
                "unknown variant: {x} ({x:#X})"
            ))),
        }
    }
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

impl Serialize for ShaderSrc {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        match (self.vs_path.is_some(), self.fs_path.is_some()) {
            (true, true) => b't',
            (true, false) => b'v',
            (false, true) => b'f',
            (false, false) => b's',
        }
        .serialize(dst, &())?;
        if let Some(path) = &self.vs_path {
            path.serialize(dst, &())?;
        }
        if let Some(path) = &self.fs_path {
            path.serialize(dst, &())?;
        }
        Ok(())
    }
}

impl Deserialize for ShaderSrc {
    fn deserialize<R>(src: &mut R, _: &mut ()) -> std::io::Result<Self>
    where
        Self: Sized,
        R: ?Sized + std::io::Read,
    {
        let (has_vs, has_fs) = match u8::deserialize(src, &mut ())? {
            b't' => (true, true),
            b'v' => (true, false),
            b'f' => (false, true),
            b's' => (false, false),

            x => {
                return Err(std::io::Error::other(format!(
                    "unknown variant: {x} ({x:#X})"
                )));
            }
        };
        Ok(Self {
            vs_path: has_vs
                .then(|| PathBuf::deserialize(src, &mut ()))
                .transpose()?,
            fs_path: has_fs
                .then(|| PathBuf::deserialize(src, &mut ()))
                .transpose()?,
        })
    }
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

impl Serialize for AssetContent {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        match self {
            Self::Raster { src, rtex: _ } => b'r'
                .serialize(dst, &())
                .and_then(|()| src.serialize(dst, &())),

            Self::Shader { src, shader: _ } => b's'
                .serialize(dst, &())
                .and_then(|()| src.serialize(dst, &())),
        }
    }
}

impl Deserialize<(&mut RaylibHandle, &RaylibThread)> for AssetContent {
    fn deserialize<R>(
        src: &mut R,
        (rl, thread): &mut (&mut RaylibHandle, &RaylibThread),
    ) -> std::io::Result<Self>
    where
        Self: Sized,
        R: ?Sized + std::io::Read,
    {
        match u8::deserialize(src, &mut ())? {
            b'r' => {
                let src = RasterSrc::deserialize(src, &mut ())?;
                Ok(Self::Raster {
                    rtex: Rc::new(RefCell::new(
                        src.load(rl, thread).map_err(std::io::Error::other)?,
                    )),
                    src,
                })
            }

            b's' => {
                let src = ShaderSrc::deserialize(src, &mut ())?;
                Ok(Self::Shader {
                    shader: Rc::new(RefCell::new(
                        src.load(rl, thread).map_err(std::io::Error::other)?,
                    )),
                    src,
                })
            }

            x => Err(std::io::Error::other(format!(
                "unknown variant: {x} ({x:#X})"
            ))),
        }
    }
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

impl Serialize for Asset {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        let Self { name, data } = self;
        name.serialize(dst, &())?;
        data.serialize(dst, &())?;
        Ok(())
    }
}

impl Deserialize<(&mut RaylibHandle, &RaylibThread)> for Asset {
    fn deserialize<R>(
        src: &mut R,
        ctx: &mut (&mut RaylibHandle, &RaylibThread),
    ) -> std::io::Result<Self>
    where
        Self: Sized,
        R: ?Sized + std::io::Read,
    {
        Ok(Self {
            name: String::deserialize(src, &mut ())?,
            data: AssetContent::deserialize(src, ctx)?,
        })
    }
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
}

/// Uses [`AssetPos`] instead of [`usize`]
#[derive(Debug, Default)]
pub struct Assets {
    list: Vec<Asset>,
}

impl Serialize for Assets {
    fn serialize<W>(&self, dst: &mut W, _: &()) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        let Self { list } = self;
        list.serialize_slice(dst, &())
    }
}

impl Deserialize<(&mut RaylibHandle, &RaylibThread)> for Assets {
    fn deserialize<R>(
        src: &mut R,
        ctx: &mut (&mut RaylibHandle, &RaylibThread),
    ) -> std::io::Result<Self>
    where
        Self: Sized,
        R: ?Sized + std::io::Read,
    {
        Vec::deserialize_slice(src, ctx).map(|list| Self { list })
    }
}

impl Assets {
    pub const fn new() -> Self {
        Self { list: Vec::new() }
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
