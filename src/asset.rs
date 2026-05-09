use raylib::prelude::*;

#[derive(Debug)]
pub enum AssetContent {
    Raster(RenderTexture2D),
    Shader(Shader),
}

#[derive(Debug)]
pub struct Asset {
    pub name: String,
    pub data: AssetContent,
}
