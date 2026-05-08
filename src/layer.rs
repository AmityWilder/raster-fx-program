use raylib::prelude::*;

pub trait AsRaylibHandle {
    fn as_rl(&mut self) -> &mut RaylibHandle;
}

impl AsRaylibHandle for RaylibHandle {
    fn as_rl(&mut self) -> &mut RaylibHandle {
        self
    }
}
impl AsRaylibHandle for RaylibDrawHandle<'_> {
    fn as_rl(&mut self) -> &mut RaylibHandle {
        self
    }
}
impl<T: AsRaylibHandle> AsRaylibHandle for RaylibTextureMode<'_, T> {
    fn as_rl(&mut self) -> &mut RaylibHandle {
        (**self).as_rl()
    }
}

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
pub enum Effect {
    Shader(Shader),
    BlendMode(BlendMode),
}

trait EffectSlice {
    fn apply(&mut self, d: &mut impl RaylibDraw, texture: impl AsRef<ffi::Texture2D>);
}

impl EffectSlice for [Effect] {
    fn apply(&mut self, d: &mut impl RaylibDraw, texture: impl AsRef<ffi::Texture2D>) {
        if let [first, rest @ ..] = self {
            match first {
                Effect::Shader(shader) => rest.apply(&mut d.begin_shader_mode(shader), texture),
                Effect::BlendMode(mode) => rest.apply(&mut d.begin_blend_mode(*mode), texture),
            }
        } else {
            // empty
            d.draw_texture(texture, 0, 0, Color::WHITE);
        }
    }
}

#[derive(Debug)]
enum LayerContent {
    Raster { is_dirty: bool },
    Group { children: Vec<Layer> },
}

#[derive(Debug)]
pub struct Layer {
    pub name: String,
    /// private on groups, accessible on rasters
    buffer: RenderTexture2D,
    content: LayerContent,
    pub effects: Vec<Effect>,
}

impl Layer {
    pub const fn new_raster(name: String, buffer: RenderTexture2D) -> Self {
        Self {
            name,
            buffer,
            content: LayerContent::Raster { is_dirty: true },
            effects: Vec::new(),
        }
    }

    pub const fn new_group(name: String, buffer: RenderTexture2D) -> Self {
        Self {
            name,
            buffer,
            content: LayerContent::Group {
                children: Vec::new(),
            },
            effects: Vec::new(),
        }
    }

    /// Returns [`None`] for rasters
    pub fn children(&self) -> Option<&Vec<Layer>> {
        match &self.content {
            LayerContent::Raster { .. } => None,
            LayerContent::Group { children } => Some(children),
        }
    }

    /// Returns [`None`] for rasters
    pub fn children_mut(&mut self) -> Option<&mut Vec<Layer>> {
        match &mut self.content {
            LayerContent::Raster { .. } => None,
            LayerContent::Group { children } => Some(children),
        }
    }

    /// Returns [`None`] for groups
    pub fn buffer(&self) -> Option<&RenderTexture2D> {
        match &self.content {
            LayerContent::Raster { .. } => Some(&self.buffer),
            LayerContent::Group { .. } => None,
        }
    }

    /// Returns [`None`] for groups
    ///
    /// Assumes buffer will be modified and marks it as dirty
    pub fn buffer_mut(&mut self) -> Option<&mut RenderTexture2D> {
        match &mut self.content {
            LayerContent::Raster { is_dirty } => {
                *is_dirty = true;
                Some(&mut self.buffer)
            }
            LayerContent::Group { .. } => None,
        }
    }

    fn draw_buffered<D>(&mut self, d: &mut D)
    where
        D: RaylibDraw,
    {
        self.effects.as_mut_slice().apply(d, &self.buffer);
    }

    fn prep_buffer_recursively<D>(&mut self, d: &mut D, thread: &RaylibThread) -> bool
    where
        D: RaylibDraw + AsRaylibHandle,
    {
        match &mut self.content {
            LayerContent::Raster { is_dirty } => std::mem::take(is_dirty),

            LayerContent::Group { children } => {
                let mut any_updated = false;
                // cannot be represented as "any" because all must be visited
                for child in children.iter_mut() {
                    any_updated |= child.prep_buffer_recursively(d, thread);
                }
                if any_updated {
                    let mut d = d.as_rl().begin_texture_mode(thread, &mut self.buffer);
                    d.clear_background(Color::BLANK);
                    for child in children.iter_mut().rev() {
                        child.draw_buffered(&mut d);
                    }
                }
                any_updated
            }
        }
    }

    pub fn draw<D>(&mut self, d: &mut D, thread: &RaylibThread)
    where
        D: RaylibDraw + AsRaylibHandle,
    {
        if self.prep_buffer_recursively(d, thread) {}
        self.draw_buffered(d);
    }
}
