use crate::{
    command::error::OpenFileError,
    error::IndexError,
    layer::rtex_from_image,
    serde::{
        DeStringError, DeUsizeError, Deserialize, DeserializeSlice, SerUsizeError, Serialize,
        SerializeSlice,
    },
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
    type Error = SerUsizeError;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + std::io::Write,
    {
        match self {
            AssetPos::Basic => {
                b'*'.serialize(dst, &())?;
            }
            AssetPos::Index(x) => {
                b'#'.serialize(dst, &())?;
                x.serialize(dst, &())?;
            }
        }
        Ok(())
    }
}

impl Deserialize for AssetPos {
    type Error = DeUsizeError;

    fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
    where
        Self: Sized,
        R: ?Sized + std::io::Read,
    {
        match u8::deserialize(src, &mut ())? {
            b'*' => Ok(Self::Basic),
            b'#' => usize::deserialize(src, &mut ()).map(Self::Index),
            _ => Err(DeUsizeError::Read(std::io::Error::other(
                OpenFileError::Invalid,
            ))),
        }
    }
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
    type Error = SerUsizeError;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + std::io::Write,
    {
        match self {
            Self::File(path) => {
                b'f'.serialize(dst, &())?;
                path.serialize(dst, &())?;
            }
            Self::Layer(()) => {
                b'l'.serialize(dst, &())?;
                todo!("layer assets don't exist yet")
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum DeEnumError {
    #[error(transparent)]
    Conversion(#[from] std::num::TryFromIntError),

    #[error(transparent)]
    Read(#[from] std::io::Error),

    #[error("unknown variant: {0} ({0:#X})")]
    UnknownVariant(u8),
}

impl From<DeUsizeError> for DeEnumError {
    fn from(e: DeUsizeError) -> Self {
        match e {
            DeUsizeError::Conversion(e) => Self::Conversion(e),
            DeUsizeError::Read(e) => Self::Read(e),
        }
    }
}

impl Deserialize for RasterSrc {
    type Error = DeEnumError;

    fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
    where
        Self: Sized,
        R: ?Sized + std::io::Read,
    {
        match u8::deserialize(src, &mut ())? {
            b'f' => PathBuf::deserialize(src, &mut ())
                .map(Self::File)
                .map_err(Into::into),
            b'l' => todo!("layer assets don't exist yet"),
            x => Err(DeEnumError::UnknownVariant(x)),
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
    type Error = SerUsizeError;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + std::io::Write,
    {
        let flags = ((self.vs_path.is_some() as u8) << 1) | (self.fs_path.is_some() as u8);
        flags.serialize(dst, &())?;
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
    type Error = DeEnumError;

    fn deserialize<R>(src: &mut R, _: &mut ()) -> Result<Self, Self::Error>
    where
        Self: Sized,
        R: ?Sized + std::io::Read,
    {
        let flags = u8::deserialize(src, &mut ())?;
        if (flags & !3) != 0 {
            return Err(DeEnumError::UnknownVariant(flags));
        }
        let (has_vs, has_fs) = ((flags & 2) != 0, (flags & 1) != 0);
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
    type Error = SerUsizeError;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + std::io::Write,
    {
        match self {
            Self::Raster { src, rtex: _ } => {
                b'r'.serialize(dst, &())?;
                src.serialize(dst, &())?;
            }
            Self::Shader { src, shader: _ } => {
                b's'.serialize(dst, &())?;
                src.serialize(dst, &())?;
            }
        }
        Ok(())
    }
}

impl Deserialize<(&mut RaylibHandle, &RaylibThread)> for AssetContent {
    type Error = DeEnumError;

    fn deserialize<R>(
        src: &mut R,
        (rl, thread): &mut (&mut RaylibHandle, &RaylibThread),
    ) -> Result<Self, Self::Error>
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

            x => Err(DeEnumError::UnknownVariant(x)),
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
    type Error = SerUsizeError;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + std::io::Write,
    {
        let Self { name, data } = self;
        name.serialize(dst, &())?;
        data.serialize(dst, &())?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum DeStrEnumError {
    #[error(transparent)]
    Conversion(#[from] std::num::TryFromIntError),

    #[error(transparent)]
    Read(#[from] std::io::Error),

    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("unknown variant: {0} ({0:#X})")]
    UnknownVariant(u8),
}

impl From<DeUsizeError> for DeStrEnumError {
    fn from(e: DeUsizeError) -> Self {
        match e {
            DeUsizeError::Conversion(e) => Self::Conversion(e),
            DeUsizeError::Read(e) => Self::Read(e),
        }
    }
}

impl From<DeStringError> for DeStrEnumError {
    fn from(e: DeStringError) -> Self {
        match e {
            DeStringError::Conversion(e) => Self::Conversion(e),
            DeStringError::Read(e) => Self::Read(e),
            DeStringError::Utf8(e) => Self::Utf8(e),
        }
    }
}

impl From<DeEnumError> for DeStrEnumError {
    fn from(e: DeEnumError) -> Self {
        match e {
            DeEnumError::Conversion(e) => Self::Conversion(e),
            DeEnumError::Read(e) => Self::Read(e),
            DeEnumError::UnknownVariant(x) => Self::UnknownVariant(x),
        }
    }
}

impl Deserialize<(&mut RaylibHandle, &RaylibThread)> for Asset {
    type Error = DeStrEnumError;

    fn deserialize<R>(
        src: &mut R,
        ctx: &mut (&mut RaylibHandle, &RaylibThread),
    ) -> Result<Self, Self::Error>
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
    type Error = SerUsizeError;

    fn serialize<W>(&self, dst: &mut W, _: &()) -> Result<(), Self::Error>
    where
        W: ?Sized + std::io::Write,
    {
        let Self { list } = self;
        list.serialize_slice(dst, &())?;
        Ok(())
    }
}

impl Deserialize<(&mut RaylibHandle, &RaylibThread)> for Assets {
    type Error = DeStrEnumError;

    fn deserialize<R>(
        src: &mut R,
        ctx: &mut (&mut RaylibHandle, &RaylibThread),
    ) -> Result<Self, Self::Error>
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
