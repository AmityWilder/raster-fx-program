use clap::Args;
use raylib::prelude::*;
use std::{fs, marker::PhantomData, path::PathBuf};

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

struct ShaderMode<'a>(PhantomData<&'a mut Shader>);

impl<'a> ShaderMode<'a> {
    fn begin(shader: &'a mut Shader) -> Self {
        // SAFETY: TBD
        unsafe {
            ffi::BeginShaderMode(*shader.as_ref());
        }
        Self(PhantomData)
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

struct BlendingMode<'a>(PhantomData<&'a mut BlendMode>);

impl<'a> BlendingMode<'a> {
    fn begin(mode: &'a mut BlendMode) -> Self {
        // SAFETY: TBD
        unsafe {
            ffi::BeginBlendMode(*mode as i32);
        }
        Self(PhantomData)
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

#[derive(Debug)]
pub struct Effect {
    builder: EffectBuilder,
    pub shader: Shader,
}

impl Effect {
    pub fn reload(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
    ) -> Result<(), std::io::Error> {
        self.shader = self.builder.load_shader(rl, thread)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Args)]
pub struct EffectBuilder {
    /// Path to the vertex shader code file
    #[arg(short, long = "vert")]
    vs_path: Option<PathBuf>,

    /// Path to the fragment shader code file
    #[arg(short, long = "frag")]
    fs_path: Option<PathBuf>,
}

impl EffectBuilder {
    fn load_shader(
        &self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
    ) -> Result<Shader, std::io::Error> {
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

    pub fn build(
        self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
    ) -> Result<Effect, std::io::Error> {
        self.load_shader(rl, thread).map(|shader| Effect {
            builder: self,
            shader,
        })
    }
}

trait EffectSlice {
    fn apply(
        &mut self,
        d: &mut impl RaylibDraw,
        texture: &impl RaylibTexture2D,
        tint: Color,
        transform: &Matrix,
    );
}

fn draw_texture_quad(
    _d: &mut impl RaylibDraw,
    texture: &impl RaylibTexture2D,
    mat: Matrix,
    tint: Color,
) {
    if !texture.is_texture_valid() {
        return;
    }
    let texture = texture.as_ref();
    let (width, height) = (texture.width as f32, texture.height as f32);
    let [tl, tr, bl, br] = [
        Vector3::new(0.0, 0.0, 0.0).transform_with(mat),
        Vector3::new(width, 0.0, 0.0).transform_with(mat),
        Vector3::new(0.0, height, 0.0).transform_with(mat),
        Vector3::new(width, height, 0.0).transform_with(mat),
    ]
    .map(|Vector3 { x, y, z: _ }| Vector2 { x, y });

    // SAFETY: based on DrawTexturePro
    #[allow(clippy::multiple_unsafe_ops_per_block)]
    unsafe {
        ffi::rlSetTexture(texture.id);
        ffi::rlBegin(ffi::RL_QUADS as i32);

        ffi::rlColor4ub(tint.r, tint.g, tint.b, tint.a);
        ffi::rlNormal3f(0.0, 0.0, 1.0); // Normal vector pointing towards viewer

        // Top-left corner for texture and quad
        ffi::rlTexCoord2f(0.0, 0.0);
        ffi::rlVertex2f(tl.x, tl.y);

        // Bottom-left corner for texture and quad
        ffi::rlTexCoord2f(0.0, 1.0);
        ffi::rlVertex2f(bl.x, bl.y);

        // Bottom-right corner for texture and quad
        ffi::rlTexCoord2f(1.0, 1.0);
        ffi::rlVertex2f(br.x, br.y);

        // Top-right corner for texture and quad
        ffi::rlTexCoord2f(1.0, 0.0);
        ffi::rlVertex2f(tr.x, tr.y);

        ffi::rlEnd();
        ffi::rlSetTexture(0);
    }
}

impl EffectSlice for [Effect] {
    fn apply(
        &mut self,
        d: &mut impl RaylibDraw,
        texture: &impl RaylibTexture2D,
        tint: Color,
        transform: &Matrix,
    ) {
        if let [first, rest @ ..] = self {
            let mut _mode = ShaderMode::begin(&mut first.shader);
            rest.apply(d, texture, tint, transform);
        } else {
            // empty
            draw_texture_quad(d, texture, *transform, tint)
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
    /// private on groups, accessible on rasters
    buffer: RenderTexture2D,
    /// rasters: the buffer has changed, making containing group buffers out of date
    /// groups: the number or order of children has changed, making the buffer out of date in a way the children cannot express
    is_dirty: bool,
    content: LayerContent,
    pub effects: Vec<Effect>,
    pub blend: Blending,
    pub transform: Matrix,
}

impl Layer {
    const fn new(name: String, buffer: RenderTexture2D, content: LayerContent) -> Self {
        Self {
            name,
            buffer,
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

    pub const fn new_raster(name: String, buffer: RenderTexture2D) -> Self {
        Self::new(name, buffer, LayerContent::Raster)
    }

    pub const fn new_group(name: String, buffer: RenderTexture2D) -> Self {
        Self::new(
            name,
            buffer,
            LayerContent::Group {
                children: Vec::new(),
            },
        )
    }

    /// Returns [`Some`] for groups and [`None`] otherwise
    pub const fn children(&self) -> Option<&Vec<Layer>> {
        match &self.content {
            LayerContent::Raster => None,
            LayerContent::Group { children } => Some(children),
        }
    }

    /// Returns [`Some`] for groups and [`None`] otherwise
    ///
    /// Assumes children will be modified and marks the group as dirty
    pub const fn children_mut(&mut self) -> Option<&mut Vec<Layer>> {
        match &mut self.content {
            LayerContent::Raster => None,
            LayerContent::Group { children } => {
                self.is_dirty = true;
                Some(children)
            }
        }
    }

    /// Returns [`Some`] for rasters and [`None`] otherwise
    pub const fn buffer(&self) -> Option<&RenderTexture2D> {
        match &self.content {
            LayerContent::Raster => Some(&self.buffer),
            LayerContent::Group { .. } => None,
        }
    }

    /// Returns [`Some`] for rasters and [`None`] otherwise
    ///
    /// Assumes buffer will be modified and marks the raster as dirty
    pub const fn buffer_mut(&mut self) -> Option<&mut RenderTexture2D> {
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
        let _mode = BlendingMode::begin(&mut self.blend.mode);
        self.effects
            .as_mut_slice()
            .apply(d, &self.buffer, self.blend.tint, &self.transform);
    }
}
