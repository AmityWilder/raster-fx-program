use crate::{
    asset::{Asset, AssetPos, Assets, RasterSrc, ShaderSrc},
    layer::{Layer, LayerPos, Layers, LoadError, SaveError},
    message::{print_success_recursive, print_warning_recursive},
    serde::{Deserialize, DeserializeArr, Serialize},
};
use clap::Parser;
use raylib::prelude::*;
use std::{io::Write, ops::ControlFlow, path::PathBuf};

pub mod error;
use error::*;

pub const ILLEGAL_LAYER_NAME_CHARS: [char; 6] = ['\n', '\r', '\t', '\\', '"', ';'];

fn valid_layer_name(s: &str) -> Result<String, LayerNameError> {
    use LayerNameError::*;
    let s = s.trim();
    if s.is_empty() {
        Err(Empty)
    } else if s.contains(ILLEGAL_LAYER_NAME_CHARS) {
        Err(Illegal)
    } else {
        Ok(s.to_string())
    }
}

#[derive(Parser)]
#[command(version)]
pub enum Command {
    /// List the current layers in the open editor
    #[command(visible_alias = "ls")]
    List {
        /// List assets instead of layers
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        assets: bool,

        /// Verbose debugging on layer list
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        dbg: bool,
    },

    /// Create one or more new layers
    #[command(name = "make", visible_alias = "mk")]
    Create {
        /// Create a group instead of a raster
        #[arg(short = 'g', long = "group", action = clap::ArgAction::SetTrue)]
        is_group: bool,

        /// The name of the layer to create
        #[arg(value_parser = valid_layer_name)]
        name: String,

        /// Where to put the layer
        #[arg(default_value = "]")]
        at: LayerPos,
    },

    /// Link items together
    #[command(visible_alias = "l")]
    Link {
        /// The asset to link
        from: AssetPos,

        /// The target to link with
        #[arg(default_value = "*")]
        to: LayerPos,
    },

    /// Reload an asset
    #[command(visible_alias = "re")]
    Reload {
        /// Which asset to reload
        what: AssetPos,
    },

    /// Change the order of layers
    #[command(name = "move", visible_alias = "mv")]
    Reorder {
        /// The layer to move
        from: LayerPos,

        /// Where to put it
        to: LayerPos,
    },

    /// Remove one or more layers
    #[command(visible_alias = "rm")]
    Remove {
        /// List of layer indices to remove
        ///
        /// Empty implies current
        positions: Vec<LayerPos>,
    },

    /// Change which layer is currently being targeted
    #[command(name = "switch", visible_alias = "cd")]
    Target {
        /// Which layer to switch focus to
        to: LayerPos,
    },

    /// Open a file
    #[command(visible_alias = "o")]
    Open {
        /// The name to give the asset, otherwise the filename will be used
        #[arg(short, long)]
        name: Option<String>,

        /// Path to the file to open
        ///
        /// Sensitive to file extension:
        ///
        /// ".png" will be loaded as a raster asset
        ///
        /// ".vs"/".vert" will be loaded as a vertex shader asset
        ///
        /// ".fs"/".frag" will be loaded as a fragment shader asset
        ///
        /// ".amyfx" will be loaded as an amyfx document
        ///
        /// A shader may contain both a vertex and fragment shader
        #[arg(num_args = 1..=2)]
        paths: Vec<PathBuf>,
    },

    #[command(visible_alias = "ex")]
    Export {
        /// Destination file to export to
        path: PathBuf,
    },

    /// Save and close the application
    #[command(visible_alias = "q")]
    Quit,

    /// Save and close the application, overwriting the save file without warning
    #[command(visible_alias = "wq")]
    SaveAndQuit,

    /// Close the application without saving
    #[command(visible_alias = "q!")]
    ForceQuit,
}

impl Command {
    pub fn run(
        self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        assets: &mut Assets,
        layers: &mut Layers,
        file: &mut Option<PathBuf>,
        canvas_size: &mut [std::num::NonZeroU32; 2],
    ) -> Result<ControlFlow<()>, RunCommandError> {
        match self {
            Self::List {
                assets: list_assets,
                dbg,
            } => {
                if list_assets {
                    println!("\x1b[96massets: {{\x1b[0m");
                    for (i, asset) in assets.iter().enumerate().rev() {
                        print!("  \x1b[92m{i}:\x1b[0m ");
                        if dbg {
                            println!("{asset:#?}");
                        } else {
                            println!("{}", asset.name);
                        }
                    }
                    println!("\x1b[96m}}\x1b[0m");
                } else {
                    println!("\x1b[96mlayers: {{\x1b[0m");
                    for (i, layer) in layers.iter().enumerate().rev() {
                        let (color, open, close) = if i
                            == layers
                                .curr()
                                .expect("should be Some if layers is non-empty")
                        {
                            (95, '[', ']')
                        } else {
                            (92, ' ', ' ')
                        };
                        print!("  \x1b[{color}m{open}{i}{close}:\x1b[0m ");
                        if dbg {
                            println!("{layer:#?}");
                        } else {
                            println!("{}", layer.name);
                        }
                    }
                    println!("\x1b[96m}}\x1b[0m");
                }
            }

            Self::Create {
                mut at,
                name,
                is_group,
            } => {
                let [w, h] = *canvas_size;
                rl.load_render_texture(thread, w.get(), h.get())
                    .map_err(NewLayerError::Raylib)
                    .and_then(|buffer| {
                        layers.insert(
                            at,
                            if is_group {
                                Layer::new_group(name, buffer)
                            } else {
                                Layer::new_raster(name, buffer)
                            },
                        )?;
                        at = LayerPos::Next;
                        Ok(())
                    })?;
            }

            Self::Link { from, to } => {
                let asset = assets.get_mut(from).map_err(LinkError::from)?;
                let layer = layers.get_mut(to).map_err(LinkError::from)?;
                layer.link(asset)?;
                print_success_recursive(&format!(
                    "linked asset \"{}\" to layer \"{}\"",
                    asset.name, layer.name
                ));
            }

            Self::Reload { what } => {
                let asset = assets.get_mut(what).map_err(ReloadAssetError::from)?;
                asset.reload(rl, thread)?;
                print_success_recursive(&format!("asset \"{}\" reloaded", asset.name));
            }

            Self::Reorder { from, to } => layers.reorder(from, to)?,

            Self::Remove { positions } => layers.remove(positions)?,

            Self::Target { to } => layers.set_target(to).map_err(SwitchLayerError::Select)?,

            Self::Open { name, paths } => {
                if let [path] = paths.as_slice()
                    && let ext = path.extension().unwrap_or_default()
                    && ext.eq_ignore_ascii_case("amyfx")
                {
                    let contents = std::fs::read(path)
                        .map_err(Into::into)
                        .map_err(LoadError::Deserialize)?;
                    let mut data = contents.as_slice();
                    *canvas_size = DeserializeArr::deserialize_arr(&mut data, &mut ())
                        .map_err(Into::into)
                        .map_err(LoadError::Deserialize)?;
                    *assets = Assets::deserialize(&mut data, &mut (rl, thread))
                        .map_err(Into::into)
                        .map_err(LoadError::Deserialize)?;
                    *layers = Layers::deserialize(&mut data, &mut (rl, thread, assets))
                        .map_err(LoadError::Deserialize)?;
                    print_success_recursive(&format!("amyfx loaded: \"{}\"", path.display()));
                    *file = Some(
                        paths
                            .into_iter()
                            .next()
                            .expect("should be guaranteed by branch"),
                    );
                } else if let [path] = paths.as_slice()
                    && let Some(ext) = path.extension()
                    && ext.eq_ignore_ascii_case("png")
                {
                    let asset = assets
                        .push(Asset::load_raster(
                            rl,
                            thread,
                            name.or_else(|| {
                                path.file_name()
                                    .map(|filename| filename.to_string_lossy().to_string())
                            })
                            .unwrap_or_else(|| format!("asset {}", assets.len())),
                            RasterSrc::File(
                                paths
                                    .into_iter()
                                    .next()
                                    .expect("should be guaranteed by branch"),
                            ),
                        )?)
                        .map_err(OpenFileError::NoMemory)?;
                    print_success_recursive(&format!("raster loaded: \"{}\"", asset.name));
                } else {
                    let mut vs_path = None;
                    let mut fs_path = None;
                    for path in paths {
                        let ext = path.extension().unwrap_or_default();
                        if ext.eq_ignore_ascii_case("vs") || ext.eq_ignore_ascii_case("vert") {
                            if vs_path.is_some() {
                                print_warning_recursive(
                                    &"multiple vertex shaders, using the last one",
                                );
                            }
                            vs_path = Some(path);
                        } else if ext.eq_ignore_ascii_case("fs") || ext.eq_ignore_ascii_case("frag")
                        {
                            if fs_path.is_some() {
                                print_warning_recursive(
                                    &"multiple fragment shaders, using the last one",
                                );
                            }
                            fs_path = Some(path);
                        }
                    }
                    if fs_path.is_none() && vs_path.is_none() {
                        print_warning_recursive(&"no files to open");
                    } else {
                        let asset = assets
                            .push(Asset::load_shader(
                                rl,
                                thread,
                                name.or_else(|| {
                                    let fs_name = fs_path
                                        .as_ref()
                                        .and_then(|path| path.file_name())
                                        .map(|fname| fname.to_string_lossy());
                                    let vs_name = vs_path
                                        .as_ref()
                                        .and_then(|path| path.file_name())
                                        .map(|fname| fname.to_string_lossy());
                                    match (fs_name, vs_name) {
                                        (Some(fs), Some(vs)) => Some(format!("{fs}_{vs}")),
                                        (Some(x), None) | (None, Some(x)) => Some(x.to_string()),
                                        (None, None) => None,
                                    }
                                })
                                .unwrap_or_else(|| format!("asset {}", assets.len())),
                                ShaderSrc { fs_path, vs_path },
                            )?)
                            .map_err(OpenFileError::NoMemory)?;
                        print_success_recursive(&format!("shader loaded: \"{}\"", asset.name));
                    }
                }
            }

            Self::Export { path: _ } => {
                print_warning_recursive(&"not yet implemented");
            }

            Self::Quit | Self::SaveAndQuit => {
                let mut contents = Vec::new();
                assets
                    .serialize(&mut contents, &())
                    .map_err(Into::into)
                    .map_err(SaveError::Serialization)?;
                layers
                    .serialize(&mut contents, &&*assets)
                    .map_err(SaveError::Serialization)?;
                if let Some(save_path) = file {
                    let mut open_opts = std::fs::OpenOptions::new();
                    match self {
                        Self::Quit => open_opts.create_new(true).write(true),
                        Self::SaveAndQuit => open_opts.create(true).truncate(true),
                        _ => unreachable!(),
                    };
                    open_opts
                        .write(true)
                        .open(save_path)
                        .and_then(|mut file| file.write_all(&contents))
                        .map_err(Into::into)
                        .map_err(SaveError::Serialization)?;
                    return Ok(ControlFlow::Break(()));
                } else {
                    print_warning_recursive(&"no location to save");
                }
            }

            Self::ForceQuit => {
                return Ok(ControlFlow::Break(()));
            }
        }
        Ok(ControlFlow::Continue(()))
    }
}
