use crate::{
    asset_pos::{AssetPos, SelectAssetError},
    command::error::OpenFileError,
    layer::rtex_from_image,
};
use clap::Args;
use raylib::prelude::*;
use std::{cell::RefCell, collections::TryReserveError, fs, path::PathBuf, rc::Rc};

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
