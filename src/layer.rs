use raylib::prelude::*;

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

struct ShaderMode<'a>(&'a mut Shader);

impl<'a> ShaderMode<'a> {
    fn begin(shader: &'a mut Shader) -> Self {
        // SAFETY: TBD
        unsafe {
            ffi::BeginShaderMode(*shader.as_ref());
        }
        Self(shader)
    }
}

impl Drop for ShaderMode<'_> {
    fn drop(&mut self) {
        // SAFETY: TBD
        unsafe {
            ffi::EndShaderMode();
        }
    }
}

struct BlendingMode<'a>(&'a mut BlendMode);

impl<'a> BlendingMode<'a> {
    fn begin(mode: &'a mut BlendMode) -> Self {
        // SAFETY: TBD
        unsafe {
            ffi::BeginBlendMode(*mode as i32);
        }
        Self(mode)
    }
}

impl Drop for BlendingMode<'_> {
    fn drop(&mut self) {
        // SAFETY: TBD
        unsafe {
            ffi::EndBlendMode();
        }
    }
}

impl EffectSlice for [Effect] {
    fn apply(&mut self, d: &mut impl RaylibDraw, texture: impl AsRef<ffi::Texture2D>) {
        if let [first, rest @ ..] = self {
            match first {
                Effect::Shader(shader) => {
                    let mut _mode = ShaderMode::begin(shader);
                    rest.apply(d, texture);
                }
                Effect::BlendMode(mode) => {
                    let mut _mode = BlendingMode::begin(mode);
                    rest.apply(d, texture);
                }
            }
        } else {
            // empty
            d.draw_texture(texture, 0, 0, Color::WHITE);
        }
    }
}

#[derive(Debug)]
enum LayerContent {
    Raster,
    Group {
        /// A group may be empty, but still distinct from a raster
        children: Vec<Layer>,
    },
}

#[derive(Debug)]
pub struct Layer {
    pub name: String,
    /// private on groups, accessible on rasters
    buffer: RenderTexture2D,
    /// rasters: the buffer has changed, making containing group buffers out of date
    /// groups: the number or order of children has changed, making the buffer out of date in a way the children cannot express
    is_dirty: bool,
    content: LayerContent,
    pub effects: Vec<Effect>,
}

impl Layer {
    pub const fn new_raster(name: String, buffer: RenderTexture2D) -> Self {
        Self {
            name,
            buffer,
            is_dirty: true,
            content: LayerContent::Raster,
            effects: Vec::new(),
        }
    }

    pub const fn new_group(name: String, buffer: RenderTexture2D) -> Self {
        Self {
            name,
            buffer,
            is_dirty: true,
            content: LayerContent::Group {
                children: Vec::new(),
            },
            effects: Vec::new(),
        }
    }

    /// Returns [`Some`] for groups and [`None`] otherwise
    pub fn children(&self) -> Option<&Vec<Layer>> {
        match &self.content {
            LayerContent::Raster => None,
            LayerContent::Group { children } => Some(children),
        }
    }

    /// Returns [`Some`] for groups and [`None`] otherwise
    ///
    /// Assumes children will be modified and marks the group as dirty
    pub fn children_mut(&mut self) -> Option<&mut Vec<Layer>> {
        match &mut self.content {
            LayerContent::Raster => None,
            LayerContent::Group { children } => {
                self.is_dirty = true;
                Some(children)
            }
        }
    }

    /// Returns [`Some`] for rasters and [`None`] otherwise
    pub fn buffer(&self) -> Option<&RenderTexture2D> {
        match &self.content {
            LayerContent::Raster => Some(&self.buffer),
            LayerContent::Group { .. } => None,
        }
    }

    /// Returns [`Some`] for rasters and [`None`] otherwise
    ///
    /// Assumes buffer will be modified and marks the raster as dirty
    pub fn buffer_mut(&mut self) -> Option<&mut RenderTexture2D> {
        match &mut self.content {
            LayerContent::Raster => {
                self.is_dirty = true;
                Some(&mut self.buffer)
            }
            LayerContent::Group { .. } => None,
        }
    }

    pub fn prep_buffer_recursively(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
    ) -> bool {
        let mut is_updated = std::mem::take(&mut self.is_dirty);
        if let LayerContent::Group { children } = &mut self.content {
            // cannot be represented as "any()" because all must be visited to prep them
            for child in children.iter_mut() {
                is_updated |= child.prep_buffer_recursively(rl, thread);
            }
            if is_updated {
                let mut d = rl.begin_texture_mode(thread, &mut self.buffer);
                d.clear_background(Color::BLANK);
                for child in children.iter_mut().rev() {
                    child.draw_buffer(&mut d);
                }
            }
        }
        is_updated
    }

    pub fn draw_buffer(&mut self, d: &mut impl RaylibDraw) {
        self.effects.as_mut_slice().apply(d, &self.buffer);
    }
}
