#![deny(clippy::undocumented_unsafe_blocks)]
#![warn(clippy::multiple_unsafe_ops_per_block)]
#![warn(clippy::unwrap_used, clippy::panic, clippy::arithmetic_side_effects)]

use crate::{
    command::{Command, CommandError},
    layer::Layer,
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

mod command;
mod layer;

pub fn print_err_recursive(mut e: &dyn std::error::Error) {
    loop {
        eprint!("{e}");
        if let Some(src) = e.source() {
            eprint!(": ");
            e = src;
        } else {
            break;
        }
    }
    eprintln!();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ArgsIter<'a> {
    line: &'a str,
}

impl<'a> ArgsIter<'a> {
    fn new(line: &'a str) -> Self {
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

fn main() {
    let stdin_channel = {
        let (tx, rx) = mpsc::channel::<String>();
        thread::Builder::new()
            .name("input".to_string())
            .spawn(move || {
                loop {
                    let mut buffer = String::new();
                    if let Err(e) = stdin().read_line(&mut buffer) {
                        eprintln!("\x1b[1;91invalid input:\x1b[0m {e}");
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
    let (mut rl, thread) = init().title("Amity FX").size(1280, 720).resizable().build();
    rl.set_trace_log(TraceLogLevel::LOG_ERROR);

    rl.set_target_fps(60);

    let mut history: VecDeque<String> = VecDeque::new();
    let mut layers: Vec<Layer> = Vec::new();
    let mut curr_layer: usize = 0;

    'mainloop: while !rl.window_should_close() {
        match stdin_channel.try_recv() {
            Ok(input) => {
                'pipeline: for input in history.push_back_mut(input).split(';') {
                    match Command::try_parse_from(std::iter::once("").chain(ArgsIter::new(input)))
                        .map_err(CommandError::Parse)
                        .and_then(|cmd| {
                            cmd.run(&mut rl, &thread, &mut layers, &mut curr_layer)
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
                                    eprint!("\x1b[1;91merror:\x1b[0m ");
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
            layer.prep_buffer_recursively(&mut rl, &thread);
        }
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        for layer in layers.iter_mut() {
            layer.draw_buffer(&mut d);
        }
    }
}
