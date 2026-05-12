#![deny(
    clippy::undocumented_unsafe_blocks,
    clippy::missing_safety_doc,
    reason = "do not be unsound"
)]
#![warn(
    clippy::multiple_unsafe_ops_per_block,
    reason = "avoid large chunks of unsafe code"
)]
#![warn(
    clippy::missing_panics_doc,
    clippy::unwrap_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    reason = "avoid panics at all costs"
)]
#![warn(clippy::missing_const_for_fn, reason = "hygene")]
#![feature(array_try_from_fn)]

use crate::{
    asset::Assets,
    command::{Command, error::CommandError},
    layer::Layers,
    message::print_err_recursive,
};
use clap::Parser;
use raylib::prelude::*;
use std::{
    collections::VecDeque,
    io::stdin,
    ops::ControlFlow,
    sync::mpsc::{self, TryRecvError},
    thread,
};

mod asset;
mod command;
mod error;
mod layer;
mod message;
pub mod rlgl;
mod serde;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ArgsIter<'a> {
    line: &'a str,
}

impl<'a> ArgsIter<'a> {
    const fn new(line: &'a str) -> Self {
        Self { line }
    }
}

impl<'a> Iterator for ArgsIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let mut res;
        let s = self.line.trim_start();
        (res, self.line) = if let Some(s) = s.strip_prefix('"') {
            s.split_once('"').unwrap_or((s, ""))
        } else {
            s.split_at(s.find(char::is_whitespace).unwrap_or(s.len()))
        };
        res = res.trim_end();
        (!res.is_empty()).then_some(res)
    }
}

impl std::iter::FusedIterator for ArgsIter<'_> {}

#[derive(Debug, thiserror::Error)]
#[error("invalid input")]
struct InvalidInputError(#[from] std::io::Error);

fn main() {
    let stdin_channel = {
        let (tx, rx) = mpsc::channel::<String>();
        thread::Builder::new()
            .name("input".to_string())
            .spawn(move || {
                loop {
                    let mut buffer = String::new();
                    if let Err(e) = stdin().read_line(&mut buffer) {
                        print_err_recursive(&InvalidInputError(e));
                        continue;
                    }
                    buffer.truncate(buffer.trim_end().len());
                    if tx.send(buffer).is_err() {
                        break;
                    }
                }
            })
            .expect("failed to spawn input thread");
        rx
    };
    let (mut rl, thread) = init()
        .log_level(TraceLogLevel::LOG_WARNING)
        .title("Amity FX")
        .size(1280, 720)
        .resizable()
        .build();

    rl.set_target_fps(30);

    let mut history: VecDeque<String> = VecDeque::new();
    let mut assets = Assets::new();
    let mut layers = Layers::new();
    let mut save_file = None;
    // SAFETY: these literals are not 0
    let mut canvas_size = [1024, 720].map(|n| unsafe { std::num::NonZeroU32::new_unchecked(n) });

    'mainloop: while !rl.window_should_close() {
        match stdin_channel.try_recv() {
            Ok(input) => {
                'pipeline: for input in history.push_back_mut(input).split(';') {
                    match Command::try_parse_from(std::iter::once("").chain(ArgsIter::new(input)))
                        .map_err(CommandError::Parse)
                        .and_then(|cmd| {
                            cmd.run(
                                &mut rl,
                                &thread,
                                &mut assets,
                                &mut layers,
                                &mut save_file,
                                &mut canvas_size,
                            )
                            .map_err(CommandError::Run)
                        }) {
                        Ok(ControlFlow::Continue(())) => {}

                        Ok(ControlFlow::Break(())) => break 'mainloop,

                        Err(e) => {
                            use clap::error::ErrorKind::*;
                            match e {
                                CommandError::Parse(e)
                                    if matches!(e.kind(), DisplayHelp | DisplayVersion) =>
                                {
                                    println!("{}", e.render());
                                }

                                CommandError::Parse(e) => {
                                    println!("\x1b[1;91merror:\x1b[0m {}", e.render());
                                }

                                _ => {
                                    print_err_recursive(&e);
                                }
                            }

                            break 'pipeline;
                        }
                    }
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                break 'mainloop;
            }
        }

        for layer in layers.iter_mut() {
            if let Err(e) = layer.prep_buffer_recursively(&mut rl, &thread) {
                #[cfg(debug_assertions)]
                print_err_recursive(&e);
            }
        }
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        for layer in layers.iter_mut() {
            if let Err(e) = layer.draw_buffer(&mut d, Matrix::identity()) {
                #[cfg(debug_assertions)]
                print_err_recursive(&e);
            }
        }
    }
}
