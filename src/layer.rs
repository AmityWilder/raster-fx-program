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
// impl<T: AsRaylibHandle> AsRaylibHandle for &mut T {
//     fn as_rl(&mut self) -> &mut RaylibHandle {
//         (**self).as_rl()
//     }
// }

#[derive(Debug)]
pub struct Raster {
    rtex: RenderTexture2D,
    is_dirty: bool,
}

impl Raster {
    pub fn new(
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        width: u32,
        height: u32,
    ) -> Result<Self, raylib::error::Error> {
        rl.load_render_texture(thread, width, height)
            .map(|rtex| Self {
                rtex,
                is_dirty: true,
            })
    }

    pub fn from_image(
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        image: &Image,
    ) -> Result<Self, raylib::error::Error> {
        Self::new(
            rl,
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
        .and_then(|mut raster| {
            assert!(image.is_image_valid(), "image should be valid");
            assert!(
                !image.data.is_null() && image.data.is_aligned(),
                "image data pointer should be valid"
            );
            // SAFETY: this is the definition of image.data according to the Raylib source code.
            // The only reason it isn't safe in Raylib-rs is because nobody bothered to check.
            let pixels = unsafe {
                std::slice::from_raw_parts(image.data.cast(), image.get_pixel_data_size())
            };
            raster
                .rtex
                .update_texture(pixels)
                .or_else(|_| {
                    let texture = rl.load_texture_from_image(thread, image)?;
                    let mut d = rl.begin_texture_mode(thread, &mut raster.rtex);
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
                .map(|()| raster)
        })
    }

    fn draw<D>(&self, d: &mut D)
    where
        D: RaylibDraw,
    {
        d.draw_texture(&self.rtex, 0, 0, Color::WHITE);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Effect {}

#[derive(Debug)]
pub struct Group {
    buffer: RenderTexture2D,
    is_dirty: bool,
    pub children: Vec<Layer>,
}

impl Group {
    fn is_dirty(&self) -> bool {
        self.is_dirty || self.children.iter().any(|child| child.is_dirty())
    }

    fn draw<D>(&mut self, d: &mut D, thread: &RaylibThread)
    where
        D: RaylibDraw + AsRaylibHandle,
    {
        if self.is_dirty() {
            let mut d = d.as_rl().begin_texture_mode(thread, &mut self.buffer);
            for child in self.children.iter_mut().rev() {
                child.render_recursively(&mut d, thread);
            }
        }
    }
}

#[derive(Debug)]
pub enum LayerContent {
    Raster(Raster),
    Effect(Effect),
    Group(Group),
}

#[derive(Debug)]
pub struct Layer {
    pub name: String,
    pub content: LayerContent,
}

impl Layer {
    pub const fn with_name(name: String, content: LayerContent) -> Self {
        Self { name, content }
    }

    fn is_dirty(&self) -> bool {
        match &self.content {
            LayerContent::Raster(x) => x.is_dirty,
            LayerContent::Effect(_) => false,
            LayerContent::Group(x) => x.is_dirty(),
        }
    }

    pub fn render_recursively<D>(&mut self, d: &mut D, thread: &RaylibThread)
    where
        D: RaylibDraw + AsRaylibHandle,
    {
        match &mut self.content {
            LayerContent::Raster(raster) => {
                raster.draw(d);
            }

            LayerContent::Effect(_effect) => {
                // TODO
            }

            LayerContent::Group(group) => {
                group.draw(d, thread);
            }
        }
    }
}
