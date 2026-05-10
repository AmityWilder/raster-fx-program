use crate::{
    asset::{Asset, AssetRef, Assets},
    asset_pos::{AssetPos, SelectAssetError},
    command::error::{
        LinkError, NewLayerError, OpenFileError, RemoveLayerError, ReorderLayersError,
    },
    layer_pos::{InsertLayerError, LayerPos, SelectLayerError},
    rlgl::*,
};
use raylib::prelude::*;
use std::{
    cell::RefCell,
    collections::BTreeSet,
    rc::{Rc, Weak},
    string::FromUtf8Error,
};
use thiserror::Error;

/// # Panics
/// This method may panic if `image` is invalid
pub fn rtex_from_image(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    image: &Image,
) -> Result<RenderTexture2D, raylib::error::Error> {
    rl.load_render_texture(
        thread,
        image
            .width
            .try_into()
            .expect("image should not have negative width"),
        image
            .height
            .try_into()
            .expect("image should not have negative height"),
    )
    .and_then(|mut rtex| {
        assert!(image.is_image_valid(), "image should be valid");
        assert!(
            !image.data.is_null() && image.data.is_aligned(),
            "image data pointer should be valid"
        );
        // SAFETY: this is the definition of image.data according to the Raylib source code.
        // The only reason it isn't safe in Raylib-rs is because nobody bothered to check.
        let pixels =
            unsafe { std::slice::from_raw_parts(image.data.cast(), image.get_pixel_data_size()) };
        rtex.update_texture(pixels)
            // more expensive but reliable fallback
            .or_else(|_| {
                let texture = rl.load_texture_from_image(thread, image)?;
                let mut d = rl.begin_texture_mode(thread, &mut rtex);
                d.clear_background(Color::BLANK);
                d.draw_texture_pro(
                    &texture,
                    Rectangle::new(
                        0.0,
                        texture.height as f32,
                        texture.width as f32,
                        -(texture.height as f32),
                    ),
                    Rectangle::new(0.0, 0.0, texture.width as f32, texture.height as f32),
                    Vector2::zero(),
                    0.0,
                    Color::WHITE,
                );
                Ok(())
            })
            .map(|()| rtex)
    })
}

#[derive(Debug)]
pub struct Effect {
    pub asset: Weak<RefCell<Shader>>,
}

fn draw_texture_quad(
    d: &mut impl RaylibDraw,
    texture: &impl RaylibTexture2D,
    mat: Matrix,
    tint: Color,
) {
    if !texture.is_texture_valid() {
        return;
    }
    let (width, height) = (texture.width() as f32, texture.height() as f32);
    let [tl, tr, bl, br] = [
        Vector3::new(0.0, 0.0, 0.0).transform_with(mat),
        Vector3::new(width, 0.0, 0.0).transform_with(mat),
        Vector3::new(0.0, height, 0.0).transform_with(mat),
        Vector3::new(width, height, 0.0).transform_with(mat),
    ]
    .map(|Vector3 { x, y, z: _ }| Vector2 { x, y });

    // Based on DrawTexturePro

    let mut d = d.rl_set_texture(texture);
    let mut d = d.rl_begin_quads();
    d.rl_color(tint);
    d.rl_normal(Vector3::new(0.0, 0.0, 1.0)); // Normal vector pointing towards viewer
    d.quad([
        (Vector2::new(0.0, 0.0), tl), // Top-left corner for texture and quad
        (Vector2::new(0.0, 1.0), bl), // Bottom-left corner for texture and quad
        (Vector2::new(1.0, 1.0), br), // Bottom-right corner for texture and quad
        (Vector2::new(1.0, 0.0), tr), // Top-right corner for texture and quad
    ]);
}

#[derive(Debug, Error)]
pub enum ApplyEffectsError {
    #[error("too many shaders, stack overflow")]
    ShaderOverflow,

    #[error("effect asset deleted but still referenced on layer")]
    EffectDeleted,
}

trait EffectSlice {
    fn apply(
        &mut self,
        d: &mut impl RaylibDraw,
        texture: &impl RaylibTexture2D,
        tint: Color,
        transform: &Matrix,
    ) -> Result<(), ApplyEffectsError>;
}

impl EffectSlice for [Effect] {
    fn apply(
        &mut self,
        d: &mut impl RaylibDraw,
        texture: &impl RaylibTexture2D,
        tint: Color,
        transform: &Matrix,
    ) -> Result<(), ApplyEffectsError> {
        struct Guard {
            /// The number of shaders that have been begun so far.
            begun: usize,
        }
        impl Drop for Guard {
            fn drop(&mut self) {
                for _ in 0..self.begun {
                    // SAFETY: Guard guarantees that this many shaders have been begun and have not been ended
                    unsafe {
                        ffi::EndShaderMode();
                    }
                }
            }
        }
        impl Guard {
            /// # Safety
            /// Containing draw modifiers must not end before this guard is dropped
            unsafe fn begin(
                &mut self,
                shader: impl AsRef<ffi::Shader>,
            ) -> Result<(), ApplyEffectsError> {
                // if we can't track any more shaders, don't try beginning it
                self.begun = self
                    .begun
                    .checked_add(1)
                    .ok_or(ApplyEffectsError::ShaderOverflow)?;
                // SAFETY: Existence of RaylibDraw ensures safe to begin shader mode, mode will end when dropped
                unsafe {
                    ffi::BeginShaderMode(*shader.as_ref());
                }
                Ok(())
            }
        }

        let mut guard = Guard { begun: 0 };
        for effect in self {
            let shader = effect
                .asset
                .upgrade()
                .ok_or(ApplyEffectsError::EffectDeleted)?;
            let shader = shader.borrow();
            // SAFETY: RaylibDraw guarantees we are drawing, borrow ensures it cannot drop until the function ends
            unsafe {
                guard.begin(&*shader)?;
            }
        }
        draw_texture_quad(d, texture, *transform, tint);
        drop(guard);
        Ok(())
    }
}

#[derive(Debug)]
pub enum Raster {
    Unique {
        buffer: RenderTexture2D,
    },
    Asset {
        buffer: Rc<RefCell<RenderTexture2D>>,
    },
}

#[derive(Debug)]
enum LayerContent {
    Raster(Raster),
    Group {
        buffer: RenderTexture2D,

        /// A group may be empty, but still distinct from a raster
        children: Vec<Layer>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blending {
    pub mode: BlendMode,
    pub tint: Color,
}

impl Blending {
    pub const fn new() -> Self {
        Self {
            mode: BlendMode::BLEND_ALPHA,
            tint: Color::WHITE,
        }
    }
}

impl Default for Blending {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct Layer {
    pub name: String,
    /// rasters: the buffer has changed, making containing group buffers out of date
    /// groups: the number or order of children has changed, making the buffer out of date in a way the children cannot express
    is_dirty: bool,
    content: LayerContent,
    pub effects: Vec<Effect>,
    pub blend: Blending,
    pub transform: Matrix,
}

impl Layer {
    const fn new(name: String, content: LayerContent) -> Self {
        Self {
            name,
            is_dirty: true,
            content,
            effects: Vec::new(),
            blend: Blending::new(),
            transform: Matrix {
                m0: 1.0,
                m4: 0.0,
                m8: 0.0,
                m12: 0.0,
                m1: 0.0,
                m5: 1.0,
                m9: 0.0,
                m13: 0.0,
                m2: 0.0,
                m6: 0.0,
                m10: 1.0,
                m14: 0.0,
                m3: 0.0,
                m7: 0.0,
                m11: 0.0,
                m15: 1.0,
            },
        }
    }

    fn save<W: std::io::Write>(&self, dst: &mut W, assets: &Assets) -> Result<(), SaveError> {
        let Self {
            name,
            is_dirty: _, // will be dirty when loaded
            content,
            effects,
            blend,
            transform,
        } = self;
        dst.write_all(&u64::try_from(name.len())?.to_le_bytes())?;
        dst.write_all(name.as_bytes())?;
        match &content {
            LayerContent::Raster(raster) => match raster {
                Raster::Unique { buffer } => {
                    dst.write_all(b"u")?;
                    let img = buffer.load_image()?;
                    let data = img.export_image_to_memory(".png")?;
                    dst.write_all(&u64::try_from(data.len())?.to_le_bytes())?;
                    dst.write_all(data)?;
                }
                Raster::Asset { buffer } => {
                    dst.write_all(b"a")?;
                    match assets.raster_pos(buffer).unwrap_or_default() {
                        AssetPos::Basic => {
                            dst.write_all(const { &[b'*'; std::mem::size_of::<u64>()] })?
                        }
                        AssetPos::Index(idx) => {
                            dst.write_all(&u64::try_from(idx)?.to_le_bytes())?
                        }
                    }
                }
            },
            LayerContent::Group {
                buffer: _, // TODO: maybe store the size?
                children,
            } => {
                dst.write_all(b"g")?;
                dst.write_all(&u64::try_from(children.len())?.to_le_bytes())?;
                if let LayerContent::Group { children, .. } = &content {
                    for child in children {
                        child.save(dst, assets)?;
                    }
                }
            }
        }
        dst.write_all(&u64::try_from(effects.len())?.to_le_bytes())?;
        for Effect { asset } in effects {
            match asset
                .upgrade()
                .and_then(|shader| assets.shader_pos(&shader))
                .unwrap_or_default()
            {
                AssetPos::Basic => dst.write_all(b"*")?,
                AssetPos::Index(idx) => {
                    dst.write_all(b"#")?;
                    dst.write_all(&u64::try_from(idx)?.to_le_bytes())?;
                }
            }
        }
        {
            let Blending {
                mode,
                tint: Color { r, g, b, a },
            } = *blend;
            dst.write_all(&[mode as u8, r, g, b, a])?;
        }
        for cell in transform.to_array() {
            dst.write_all(&cell.to_bits().to_le_bytes())?;
        }
        Ok(())
    }

    fn load<R: std::io::Read>(
        src: &mut R,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        assets: &Assets,
    ) -> Result<Self, LoadError> {
        let mut name_len_bytes = [0; _];
        src.read_exact(&mut name_len_bytes)?;
        let name_len = u64::from_le_bytes(name_len_bytes).try_into()?;
        let mut name_bytes = vec![0; name_len];
        src.read_exact(&mut name_bytes)?;
        let name = String::from_utf8(name_bytes)?;
        let mut content_type = 0;
        src.read_exact(std::slice::from_mut(&mut content_type))?;
        let content = match content_type {
            b'u' => {
                let mut data_len_bytes = [0; _];
                src.read_exact(&mut data_len_bytes)?;
                let data_len = u64::from_le_bytes(data_len_bytes).try_into()?;
                let mut data = vec![0; data_len];
                src.read_exact(&mut data)?;
                LayerContent::Raster(Raster::Unique {
                    buffer: rtex_from_image(
                        rl,
                        thread,
                        &Image::load_image_from_mem(".png", &data)?,
                    )?,
                })
            }

            b'a' => {
                let mut is_default = 0;
                src.read_exact(std::slice::from_mut(&mut is_default))?;
                let asset_pos = match is_default {
                    b'*' => AssetPos::Basic,

                    b'#' => {
                        let mut pos_bytes = [0; _];
                        src.read_exact(&mut pos_bytes)?;
                        let pos = u64::from_le_bytes(pos_bytes).try_into()?;
                        AssetPos::Index(pos)
                    }

                    _ => return Err(LoadError::Invalid),
                };
                match assets.get(asset_pos)?.link_ref() {
                    AssetRef::Raster(raster) => LayerContent::Raster(Raster::Asset {
                        buffer: raster.clone(),
                    }),
                    AssetRef::Shader(_) => return Err(LoadError::AssetMismatch),
                }
            }

            b'g' => LayerContent::Group {
                buffer: rl.load_render_texture(thread, 0, 0)?,
                children: {
                    let mut children_len_bytes = [0; _];
                    src.read_exact(&mut children_len_bytes)?;
                    let children_len = u64::from_le_bytes(children_len_bytes).try_into()?;
                    std::iter::repeat_with(|| Layer::load(src, rl, thread, assets))
                        .take(children_len)
                        .collect::<Result<_, _>>()?
                },
            },

            _ => return Err(LoadError::Invalid),
        };

        let mut effects_len_bytes = [0; _];
        src.read_exact(&mut effects_len_bytes)?;
        let effects_len = u64::from_le_bytes(effects_len_bytes).try_into()?;
        let effects = std::iter::repeat_with(|| {
            let mut is_default = 0;
            src.read_exact(std::slice::from_mut(&mut is_default))?;
            let asset_pos = match is_default {
                b'*' => AssetPos::Basic,

                b'#' => {
                    let mut pos_bytes = [0; _];
                    src.read_exact(&mut pos_bytes)?;
                    let pos = u64::from_le_bytes(pos_bytes).try_into()?;
                    AssetPos::Index(pos)
                }

                _ => return Err(LoadError::Invalid),
            };
            match assets.get(asset_pos)?.link_ref() {
                AssetRef::Raster(_) => Err(LoadError::AssetMismatch),
                AssetRef::Shader(shader) => Ok(Effect {
                    asset: Rc::downgrade(shader),
                }),
            }
        })
        .take(effects_len)
        .collect::<Result<_, _>>()?;
        let blend = {
            let mut blend_bytes = [0; _];
            src.read_exact(&mut blend_bytes)?;
            let [mode, r, g, b, a] = blend_bytes;
            Blending {
                mode: match mode {
                    0 => BlendMode::BLEND_ALPHA,
                    1 => BlendMode::BLEND_ADDITIVE,
                    2 => BlendMode::BLEND_MULTIPLIED,
                    3 => BlendMode::BLEND_ADD_COLORS,
                    4 => BlendMode::BLEND_SUBTRACT_COLORS,
                    5 => BlendMode::BLEND_ALPHA_PREMULTIPLY,
                    6 => BlendMode::BLEND_CUSTOM,
                    7 => BlendMode::BLEND_CUSTOM_SEPARATE,
                    _ => return Err(LoadError::Invalid),
                },
                tint: Color::new(r, g, b, a),
            }
        };
        let mut matrix_bytes = [[0; _]; _];
        src.read_exact(matrix_bytes.as_flattened_mut())?;
        let [
            m0_bytes,
            m1_bytes,
            m2_bytes,
            m3_bytes,
            m4_bytes,
            m5_bytes,
            m6_bytes,
            m7_bytes,
            m8_bytes,
            m9_bytes,
            m10_bytes,
            m11_bytes,
            m12_bytes,
            m13_bytes,
            m14_bytes,
            m15_bytes,
        ] = matrix_bytes;

        let transform = Matrix {
            m0: f32::from_bits(u32::from_le_bytes(m0_bytes)),
            m4: f32::from_bits(u32::from_le_bytes(m4_bytes)),
            m8: f32::from_bits(u32::from_le_bytes(m8_bytes)),
            m12: f32::from_bits(u32::from_le_bytes(m12_bytes)),
            m1: f32::from_bits(u32::from_le_bytes(m1_bytes)),
            m5: f32::from_bits(u32::from_le_bytes(m5_bytes)),
            m9: f32::from_bits(u32::from_le_bytes(m9_bytes)),
            m13: f32::from_bits(u32::from_le_bytes(m13_bytes)),
            m2: f32::from_bits(u32::from_le_bytes(m2_bytes)),
            m6: f32::from_bits(u32::from_le_bytes(m6_bytes)),
            m10: f32::from_bits(u32::from_le_bytes(m10_bytes)),
            m14: f32::from_bits(u32::from_le_bytes(m14_bytes)),
            m3: f32::from_bits(u32::from_le_bytes(m3_bytes)),
            m7: f32::from_bits(u32::from_le_bytes(m7_bytes)),
            m11: f32::from_bits(u32::from_le_bytes(m11_bytes)),
            m15: f32::from_bits(u32::from_le_bytes(m15_bytes)),
        };

        Ok(Self {
            name,
            is_dirty: true,
            content,
            effects,
            blend,
            transform,
        })
    }

    pub const fn new_raster_asset(name: String, buffer: Rc<RefCell<RenderTexture2D>>) -> Self {
        Self::new(name, LayerContent::Raster(Raster::Asset { buffer }))
    }

    pub const fn new_raster(name: String, buffer: RenderTexture2D) -> Self {
        Self::new(name, LayerContent::Raster(Raster::Unique { buffer }))
    }

    pub const fn new_group(name: String, buffer: RenderTexture2D) -> Self {
        Self::new(
            name,
            LayerContent::Group {
                buffer,
                children: Vec::new(),
            },
        )
    }

    /// Pre-renders each child's buffer with DFS to ensure only one texture is ever drawn at a time
    pub fn prep_buffer_recursively(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
    ) -> Result<bool, ApplyEffectsError> {
        let mut err = None;
        let mut is_updated = std::mem::take(&mut self.is_dirty);
        if let LayerContent::Group { children, buffer } = &mut self.content {
            // cannot be represented as "any()" because all must be visited to prep them
            for child in children.iter_mut() {
                match child.prep_buffer_recursively(rl, thread) {
                    Ok(change) => is_updated |= change,
                    Err(e) => err = Some(e),
                }
            }
            if is_updated {
                let mut d = rl.begin_texture_mode(thread, buffer);
                d.clear_background(Color::BLANK);
                for child in children.iter_mut().rev() {
                    if let Err(e) = child.draw_buffer(&mut d, self.transform) {
                        err = Some(e);
                    }
                }
            }
        }
        match err {
            Some(e) => Err(e),
            None => Ok(is_updated),
        }
    }

    /// Draws the currently cached buffer with effects applied
    pub fn draw_buffer(
        &mut self,
        d: &mut impl RaylibDraw,
        transform: Matrix,
    ) -> Result<(), ApplyEffectsError> {
        let asset_ref;
        self.effects.apply(
            &mut d.begin_blend_mode(self.blend.mode),
            match &self.content {
                LayerContent::Raster(Raster::Unique { buffer })
                | LayerContent::Group { buffer, .. } => buffer,
                LayerContent::Raster(Raster::Asset { buffer, .. }) => {
                    asset_ref = buffer.borrow();
                    &*asset_ref
                }
            },
            self.blend.tint,
            #[allow(clippy::arithmetic_side_effects)]
            &(transform * self.transform),
        )
    }

    pub fn link(&mut self, asset: &Asset) -> Result<(), LinkError> {
        match asset.link_ref() {
            AssetRef::Raster(rtex) => match &mut self.content {
                LayerContent::Raster(Raster::Asset { buffer }) => *buffer = rtex.clone(),

                LayerContent::Raster(raster) => {
                    println!("\x1b[1;95mwarning:\x1b[0m replacing artwork layer with linked asset");
                    *raster = Raster::Asset {
                        buffer: rtex.clone(),
                    }
                }

                LayerContent::Group { .. } => return Err(LinkError::OverrideGroup),
            },

            AssetRef::Shader(shader) => self.effects.push(Effect {
                asset: Rc::downgrade(shader),
            }),
        }
        Ok(())
    }
}

/// Uses [`LayerPos`] instead of [`usize`]
#[derive(Debug, Default)]
pub struct Layers {
    list: Vec<Layer>,
    curr: usize,
}

#[derive(Debug, Error)]
pub enum SaveError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Raylib(#[from] raylib::error::Error),

    #[error(
        "cannot save this file; files are stored with 64-bit integers for standardization and your system has {} bits, which exceeds that. \
        this would be fine on its own, but you also managed to exceed {} instances of something, which is kind of absurd...",
        const { usize::BITS },
        const { u64::MAX }
    )]
    Oversize(#[from] std::num::TryFromIntError),
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error(transparent)]
    OpenFile(#[from] OpenFileError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Raylib(#[from] raylib::error::Error),

    #[error(transparent)]
    FromUtf8(#[from] FromUtf8Error),

    #[error(
        "save file appears to have been created on a system with a larger bit width and exceeded this system's memory limit without exceeding the limit on that system"
    )]
    Oversize(#[from] std::num::TryFromIntError),

    #[error("file is an incompatible format or corrupt")]
    Invalid,

    #[error("missing asset")]
    SelectAsset(#[from] SelectAssetError),

    #[error("asset is of wrong type")]
    AssetMismatch,
}

impl Layers {
    pub const fn new() -> Self {
        Self {
            list: Vec::new(),
            curr: 0,
        }
    }

    pub fn save<W: std::io::Write>(&self, dst: &mut W, assets: &Assets) -> Result<(), SaveError> {
        let Self { list, curr } = self;
        dst.write_all(&u64::try_from(list.len())?.to_le_bytes())?;
        for layer in list {
            layer.save(dst, assets)?;
        }
        dst.write_all(&u64::try_from(*curr)?.to_le_bytes())?;
        Ok(())
    }

    pub fn load<R: std::io::Read>(
        src: &mut R,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        assets: &Assets,
    ) -> Result<Self, LoadError> {
        let mut list_len_bytes = [0; _];
        src.read_exact(&mut list_len_bytes)?;
        let list_len = u64::from_le_bytes(list_len_bytes).try_into()?;
        let list = std::iter::repeat_with(|| Layer::load(src, rl, thread, assets))
            .take(list_len)
            .collect::<Result<_, _>>()?;
        let mut curr_bytes = [0; _];
        src.read_exact(&mut curr_bytes)?;
        let curr = u64::from_le_bytes(curr_bytes).try_into()?;
        Ok(Self { list, curr })
    }

    pub const fn len(&self) -> usize {
        self.list.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Layer> {
        self.list.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Layer> {
        self.list.iter_mut()
    }

    #[cfg(debug_assertions)]
    fn validate_curr(&self) {
        match self.list.as_slice() {
            [] => {
                assert_eq!(
                    self.curr, 0,
                    "layers.curr should always be zero if there are no layers"
                );
            }
            list => {
                assert!(
                    self.curr < list.len(),
                    "layers.curr should always be a valid index if there are layers"
                );
            }
        }
    }

    #[must_use]
    pub fn curr(&self) -> Option<usize> {
        #[cfg(debug_assertions)]
        self.validate_curr();
        (!self.list.is_empty()).then_some(self.curr)
    }

    pub fn insert_idx(&self, at: LayerPos) -> Result<usize, InsertLayerError> {
        at.insert_layer_idx(self.curr, self.list.len())
    }

    pub fn select_idx(&self, at: LayerPos) -> Result<usize, SelectLayerError> {
        at.select_layer_idx(self.curr, self.list.len())
    }

    pub fn insert(&mut self, at: LayerPos, layer: Layer) -> Result<&mut Layer, NewLayerError> {
        let pos = at.insert_layer_idx(self.curr, self.list.len())?;
        let new_layer = self.list.insert_mut(pos, layer);
        self.curr = pos;
        println!("\x1b[96mcreated layer\x1b[0m \"{}\"", new_layer.name);
        Ok(new_layer)
    }

    /// # Panics
    /// This method may panic if [`Layers::select_idx`] is implemented incorrectly
    pub fn get(&self, at: LayerPos) -> Result<&Layer, SelectLayerError> {
        self.select_idx(at).map(|idx| {
            self.list
                .get(idx)
                .expect("LayerPos should be a valid index in the non-error branch")
        })
    }

    /// # Panics
    /// This method may panic if [`Layers::select_idx`] is implemented incorrectly
    pub fn get_mut(&mut self, at: LayerPos) -> Result<&mut Layer, SelectLayerError> {
        self.select_idx(at).map(|idx| {
            self.list
                .get_mut(idx)
                .expect("LayerPos should be a valid index in the non-error branch")
        })
    }

    pub fn reorder(&mut self, from: LayerPos, to: LayerPos) -> Result<(), ReorderLayersError> {
        use ReorderLayersError::*;
        use std::cmp::Ordering::*;
        let from = self.select_idx(from).map_err(SrcIndexOutOfBounds)?;
        let to = self.select_idx(to).map_err(DstIndexOutOfBounds)?;
        match from.cmp(&to) {
            Less => self.list[from..=to].rotate_left(1),
            Equal => println!("\x1b[1;95mwarning:\x1b[0m layer order unchanged"),
            Greater => self.list[to..=from].rotate_right(1),
        }
        Ok(())
    }

    /// # Panics
    /// This method may panic if [`Layers::select_idx`] is implemented incorrectly
    pub fn remove(
        &mut self,
        positions: impl IntoIterator<Item = LayerPos>,
    ) -> Result<(), RemoveLayerError> {
        let positions = positions
            .into_iter()
            .map(|at| self.select_idx(at))
            .collect::<Result<BTreeSet<_>, _>>()?;
        match positions.last().copied() {
            Some(n) => {
                assert!(
                    n < self.list.len(),
                    "select_idx should have errored if an out of bounds index existed"
                );
            }

            None => {
                println!("\x1b[1;95mwarning:\x1b[0m no layers removed");
            }
        }
        let mut layer_index = 0..self.list.len();
        self.curr = self.curr.saturating_sub(
            positions
                .iter()
                .copied()
                .take_while(|&i| i <= self.curr)
                .count(),
        );
        let mut pos = positions.into_iter().peekable();
        self.list.retain(|layer| {
            // SAFETY: positions cannot be negative, duplicative, or exceed the maximum layer index.
            // there cannot be more positions than layers.
            pos.next_if_eq(&unsafe { layer_index.next().unwrap_unchecked() })
                .inspect(|_| println!("\x1b[96mremoving\x1b[0m {}", layer.name))
                .is_some()
        });
        Ok(())
    }

    pub fn set_target(&mut self, at: LayerPos) -> Result<(), SelectLayerError> {
        let idx = self.select_idx(at)?;
        self.curr = idx;
        #[cfg(debug_assertions)]
        self.validate_curr();
        Ok(())
    }
}
