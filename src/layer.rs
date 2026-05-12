use crate::{
    asset::{Asset, AssetPos, AssetRef, Assets, DeStrEnumError, SelectAssetError},
    command::error::{
        LinkError, NewLayerError, OpenFileError, RemoveLayerError, ReorderLayersError,
    },
    error::IndexError,
    message::{print_success_recursive, print_warning_recursive},
    rlgl::*,
    serde::{
        DeImageError, DeNonZeroError, DeStringError, DeUsizeError, Deserialize, DeserializeSlice,
        SerImageError, SerUsizeError, Serialize, SerializeSlice,
    },
    serde_pod,
};
use raylib::prelude::*;
use std::{
    cell::RefCell,
    collections::{BTreeSet, TryReserveError},
    rc::{Rc, Weak},
    str::FromStr,
};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LayerPos {
    /// `*`, `[h]ere` - the active target
    #[default]
    Current,
    /// `k`, `]`, `[n]ext` - the active target + 1
    Next,
    /// `j`, `[`, `[p]rev` - the active target &minus; 1
    Prev,
    /// `]]`, `[f]ront` - furthest in the `Next` direction
    Front,
    /// `[[`, `[b]ack` - furthest in the `Prev` direction
    Back,
    /// number - Specific position
    Index(usize),
    /// `+`/`-`number - Offset from current index
    Offset(isize),
}

impl From<usize> for LayerPos {
    fn from(value: usize) -> Self {
        Self::Index(value)
    }
}
impl From<isize> for LayerPos {
    fn from(value: isize) -> Self {
        Self::Offset(value)
    }
}

#[derive(Debug, Error)]
pub enum ParseLayerPosError {
    #[error("unrecognized position format: \"{}\"", .0)]
    Unknown(String),

    #[error("failed to parse position index")]
    IndexInt(#[source] std::num::ParseIntError),

    #[error("failed to parse offset amount")]
    OffsetInt(#[source] std::num::ParseIntError),
}

impl FromStr for LayerPos {
    type Err = ParseLayerPosError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use ParseLayerPosError::*;
        match s {
            "*" | "h" | "here" => Ok(Self::Current),
            "k" | "]" | "n" | "next" => Ok(Self::Next),
            "j" | "[" | "p" | "prev" => Ok(Self::Prev),
            "]]" | "f" | "front" => Ok(Self::Front),
            "[[" | "b" | "back" => Ok(Self::Back),
            _ if s.starts_with(['+', '-']) => s.parse().map(Self::Offset).map_err(OffsetInt),
            _ if s.starts_with(|ch: char| ch.is_ascii_digit()) => {
                s.parse().map(Self::Index).map_err(IndexInt)
            }
            _ => Err(Unknown(s.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InsertLayerError {
    #[error("cannot exceed {} layers on a {}-bit system", const { usize::MAX }, const { std::mem::size_of::<usize>() * 8 })]
    TooManyLayers,

    #[error("out of memory")]
    AllocError(#[from] TryReserveError),

    #[error("layer position {} is out of bounds, cannot exceed the number of layers", .0)]
    IndexOutOfBounds(IndexError),
}

// Distinct from #[from] because we dont want to imply #[source]
impl From<IndexError> for InsertLayerError {
    fn from(e: IndexError) -> Self {
        Self::IndexOutOfBounds(e)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum SelectLayerError {
    #[error("layer {} does not exist", .0)]
    IndexOutOfBounds(IndexError),
}

// Distinct from #[from] because we dont want to imply #[source]
impl From<IndexError> for SelectLayerError {
    fn from(e: IndexError) -> Self {
        Self::IndexOutOfBounds(e)
    }
}

impl LayerPos {
    /// Determine the layer index that may not exist yet so it can be created.
    ///
    /// Valid indices: `0..=layer_count`
    ///
    /// # Panics
    /// This method can panic if `curr_layer` is invalid
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

    /// Determine the layer index that must exist so it can be accessed.
    ///
    /// Valid indices: `0..layer_count`
    ///
    /// # Panics
    /// This method can panic if `curr_layer` is invalid
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

impl Serialize<&Assets> for Effect {
    type Error = SerUsizeError;

    fn serialize<W>(&self, dst: &mut W, assets: &&Assets) -> Result<(), Self::Error>
    where
        W: ?Sized + std::io::Write,
    {
        self.asset
            .upgrade()
            .and_then(|shader| assets.shader_pos(&shader))
            .unwrap_or_default()
            .serialize(dst, &())
    }
}

#[derive(Debug, Error)]
pub enum DeEffectError {
    #[error(transparent)]
    Conversion(#[from] std::num::TryFromIntError),

    #[error(transparent)]
    Read(#[from] std::io::Error),

    #[error(transparent)]
    SelectAsset(#[from] SelectAssetError),

    #[error("asset mismatch: expecing shader, found raster")]
    AssetMismatch,
}

impl From<DeUsizeError> for DeEffectError {
    fn from(e: DeUsizeError) -> Self {
        match e {
            DeUsizeError::Conversion(e) => Self::Conversion(e),
            DeUsizeError::Read(e) => Self::Read(e),
        }
    }
}

impl Deserialize<&Assets> for Effect {
    type Error = DeEffectError;

    fn deserialize<R>(src: &mut R, assets: &mut &Assets) -> Result<Self, Self::Error>
    where
        Self: Sized,
        R: ?Sized + std::io::Read,
    {
        let asset_pos = AssetPos::deserialize(src, &mut ())?;
        match assets.get(asset_pos)?.link_ref() {
            AssetRef::Shader(shader) => Ok(Effect {
                asset: Rc::downgrade(shader),
            }),
            AssetRef::Raster(_) => Err(DeEffectError::AssetMismatch),
        }
    }
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
enum LayerContent {
    Unique {
        buffer: RenderTexture2D,
    },
    Asset {
        buffer: Rc<RefCell<RenderTexture2D>>,
    },
    Group {
        buffer: RenderTexture2D,

        /// A group may be empty, but still distinct from a raster
        children: Vec<Layer>,
    },
}

impl Serialize<&Assets> for LayerContent {
    type Error = SerImageError;

    fn serialize<W>(&self, dst: &mut W, assets: &&Assets) -> Result<(), Self::Error>
    where
        W: ?Sized + std::io::Write,
    {
        match self {
            Self::Unique { buffer } => {
                b'u'.serialize(dst, &())?;
                buffer.serialize(dst, &())?;
            }

            Self::Asset { buffer } => {
                b'a'.serialize(dst, &())?;
                assets
                    .raster_pos(buffer)
                    .unwrap_or_default()
                    .serialize(dst, &())?;
            }

            Self::Group {
                buffer: _, // TODO: maybe save the width/height?
                children,
            } => {
                b'g'.serialize(dst, &())?;
                children.serialize_slice(dst, assets)?;
            }
        }
        Ok(())
    }
}

impl Deserialize<(&mut RaylibHandle, &RaylibThread, &Assets)> for LayerContent {
    type Error = DeLayerError;

    fn deserialize<R>(
        src: &mut R,
        (rl, thread, assets): &mut (&mut RaylibHandle, &RaylibThread, &Assets),
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
        R: ?Sized + std::io::Read,
    {
        match u8::deserialize(src, &mut ())? {
            b'u' => RenderTexture2D::deserialize(src, &mut (&mut **rl, &**thread))
                .map(|buffer| Self::Unique { buffer })
                .map_err(Into::into),

            b'a' => match assets.get(AssetPos::deserialize(src, &mut ())?)?.link_ref() {
                AssetRef::Raster(raster) => Ok(Self::Asset {
                    buffer: raster.clone(),
                }),
                AssetRef::Shader(_) => Err(DeLayerError::RasterMismatch),
            },

            b'g' => Ok(Self::Group {
                buffer: rl.load_render_texture(thread, 0, 0)?,
                children: Vec::deserialize_slice(src, &mut (&mut **rl, &**thread, &**assets))?,
            }),

            x => Err(DeLayerError::UnknownVariant(x)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blending {
    pub mode: BlendMode,
    pub tint: Color,
}

serde_pod!(Blending { mode, tint });

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

impl Serialize<&Assets> for Layer {
    type Error = SerImageError;

    fn serialize<W>(&self, dst: &mut W, assets: &&Assets) -> Result<(), Self::Error>
    where
        W: ?Sized + std::io::Write,
    {
        let Self {
            name,
            is_dirty: _, // will be dirty when loaded
            content,
            effects,
            blend,
            transform,
        } = self;
        name.serialize(dst, &())?;
        content.serialize(dst, assets)?;
        effects.serialize_slice(dst, assets)?;
        blend.serialize(dst, &())?;
        transform.serialize(dst, &())?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum DeLayerError {
    #[error(transparent)]
    Conversion(#[from] std::num::TryFromIntError),

    #[error(transparent)]
    Read(#[from] std::io::Error),

    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("nonzero cannot be zero")]
    Zero,

    #[error("unknown variant: {0} ({0:#X})")]
    UnknownVariant(u8),

    #[error(transparent)]
    Raylib(#[from] raylib::error::Error),

    #[error(transparent)]
    SelectAsset(#[from] SelectAssetError),

    #[error("asset mismatch: expecing raster, found shader")]
    RasterMismatch,

    #[error("asset mismatch: expecing shader, found raster")]
    EffectMismatch,
}

impl From<DeUsizeError> for DeLayerError {
    fn from(e: DeUsizeError) -> Self {
        match e {
            DeUsizeError::Conversion(e) => Self::Conversion(e),
            DeUsizeError::Read(e) => Self::Read(e),
        }
    }
}

impl From<DeNonZeroError> for DeLayerError {
    fn from(e: DeNonZeroError) -> Self {
        match e {
            DeNonZeroError::Zero => Self::Zero,
            DeNonZeroError::Conversion(e) => Self::Conversion(e),
            DeNonZeroError::Read(e) => Self::Read(e),
        }
    }
}

impl From<DeStringError> for DeLayerError {
    fn from(e: DeStringError) -> Self {
        match e {
            DeStringError::Conversion(e) => Self::Conversion(e),
            DeStringError::Read(e) => Self::Read(e),
            DeStringError::Utf8(e) => Self::Utf8(e),
        }
    }
}

impl From<DeStrEnumError> for DeLayerError {
    fn from(e: DeStrEnumError) -> Self {
        match e {
            DeStrEnumError::Conversion(e) => Self::Conversion(e),
            DeStrEnumError::Read(e) => Self::Read(e),
            DeStrEnumError::Utf8(e) => Self::Utf8(e),
            DeStrEnumError::UnknownVariant(x) => Self::UnknownVariant(x),
        }
    }
}

impl From<DeEffectError> for DeLayerError {
    fn from(e: DeEffectError) -> Self {
        match e {
            DeEffectError::Conversion(e) => Self::Conversion(e),
            DeEffectError::Read(e) => Self::Read(e),
            DeEffectError::SelectAsset(e) => Self::SelectAsset(e),
            DeEffectError::AssetMismatch => Self::EffectMismatch,
        }
    }
}

impl From<DeImageError> for DeLayerError {
    fn from(e: DeImageError) -> Self {
        match e {
            DeImageError::Conversion(e) => Self::Conversion(e),
            DeImageError::Read(e) => Self::Read(e),
            DeImageError::Raylib(e) => Self::Raylib(e),
        }
    }
}

impl Deserialize<(&mut RaylibHandle, &RaylibThread, &Assets)> for Layer {
    type Error = DeLayerError;

    fn deserialize<R>(
        src: &mut R,
        (rl, thread, assets): &mut (&mut RaylibHandle, &RaylibThread, &Assets),
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
        R: ?Sized + std::io::Read,
    {
        Ok(Self {
            name: String::deserialize(src, &mut ())?,
            is_dirty: true,
            content: LayerContent::deserialize(src, &mut (&mut **rl, &**thread, &**assets))?,
            effects: Vec::deserialize_slice(src, assets)?,
            blend: Blending::deserialize(src, &mut ())?,
            transform: Matrix::deserialize(src, &mut ())?,
        })
    }
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

    pub const fn new_raster_asset(name: String, buffer: Rc<RefCell<RenderTexture2D>>) -> Self {
        Self::new(name, LayerContent::Asset { buffer })
    }

    pub const fn new_raster(name: String, buffer: RenderTexture2D) -> Self {
        Self::new(name, LayerContent::Unique { buffer })
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
                LayerContent::Unique { buffer } | LayerContent::Group { buffer, .. } => buffer,
                LayerContent::Asset { buffer, .. } => {
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
                LayerContent::Asset { buffer } => *buffer = rtex.clone(),

                raster @ LayerContent::Unique { .. } => {
                    let buffer = match raster {
                        LayerContent::Unique { buffer } => buffer,
                        _ => unreachable!(),
                    };
                    if (buffer.width(), buffer.height()) != (0, 0) {
                        print_warning_recursive(&"replacing artwork layer with linked asset");
                    }
                    *raster = LayerContent::Asset {
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
    Serialization(#[from] SerImageError),

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
    Deserialize(#[from] DeLayerError),

    #[error(
        "save file appears to have been created on a system with a larger bit width and exceeded this system's memory limit without exceeding the limit on that system"
    )]
    Oversize(#[from] std::num::TryFromIntError),
}

impl Serialize<&Assets> for Layers {
    type Error = SerImageError;

    fn serialize<W>(&self, dst: &mut W, assets: &&Assets) -> Result<(), Self::Error>
    where
        W: ?Sized + std::io::Write,
    {
        let Self { list, curr } = self;
        list.serialize_slice(dst, assets)?;
        curr.serialize(dst, &())?;
        Ok(())
    }
}

impl Deserialize<(&mut RaylibHandle, &RaylibThread, &Assets)> for Layers {
    type Error = DeLayerError;

    fn deserialize<R>(
        src: &mut R,
        ctx: &mut (&mut RaylibHandle, &RaylibThread, &Assets),
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
        R: ?Sized + std::io::Read,
    {
        Ok(Self {
            list: if false {
                let len = usize::deserialize(src, &mut ())?;
                (0..len).map(|_| Layer::deserialize(src, ctx)).collect()
            } else {
                Vec::deserialize_slice(src, ctx)
            }?,
            curr: usize::deserialize(src, &mut ())?,
        })
    }
}

impl<'a> IntoIterator for &'a Layers {
    type Item = &'a Layer;
    type IntoIter = std::slice::Iter<'a, Layer>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Layers {
    type Item = &'a mut Layer;
    type IntoIter = std::slice::IterMut<'a, Layer>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl Layers {
    pub const fn new() -> Self {
        Self {
            list: Vec::new(),
            curr: 0,
        }
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
        print_success_recursive(&format!("created layer: \"{}\"", new_layer.name));
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
            Equal => print_warning_recursive(&"layer order unchanged"),
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
                print_warning_recursive(&"no layers removed");
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
        println!("positions: {positions:?}");
        let mut pos = positions.into_iter().peekable();
        self.list.retain(|layer| {
            // SAFETY: positions cannot be negative, duplicative, or exceed the maximum layer index.
            // there cannot be more positions than layers.
            let keep = pos
                .next_if_eq(&layer_index.next().expect("should never exceed"))
                .is_none();
            if !keep {
                print_success_recursive(&format!("removing layer: \"{}\"", layer.name));
            }
            keep
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
