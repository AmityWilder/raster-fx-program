use raylib::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Effect {}

#[derive(Debug)]
pub enum LayerContent {
    Effect(Effect),
    Raster(RenderTexture2D),
    Group(Vec<Layer>),
}

impl PartialEq for LayerContent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Effect(a), Self::Effect(b)) => a == b,
            (Self::Raster(a), Self::Raster(b)) => a.texture.id == b.texture.id,
            (Self::Group(a), Self::Group(b)) => a == b,
            _ => false,
        }
    }
}

impl std::hash::Hash for LayerContent {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Effect(x) => x.hash(state),
            Self::Raster(x) => x.id.hash(state),
            Self::Group(x) => x.hash(state),
        }
    }
}

impl Default for LayerContent {
    fn default() -> Self {
        Self::Group(Vec::new())
    }
}

#[derive(Debug, PartialEq, Hash)]
pub struct Layer {
    pub name: String,
    pub content: LayerContent,
}

impl Default for Layer {
    fn default() -> Self {
        Self::new()
    }
}

impl Layer {
    pub const fn new() -> Self {
        Self {
            name: String::new(),
            content: LayerContent::Group(Vec::new()),
        }
    }

    pub const fn with_name(name: String) -> Self {
        Self {
            name,
            content: LayerContent::Group(Vec::new()),
        }
    }
}
