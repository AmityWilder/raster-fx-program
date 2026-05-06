#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub name: String,
    pub children: Vec<Layer>,
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
            children: Vec::new(),
        }
    }

    pub const fn with_name(name: String) -> Self {
        Self {
            name,
            children: Vec::new(),
        }
    }
}
