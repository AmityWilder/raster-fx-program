#![deny(clippy::undocumented_unsafe_blocks)]
#![warn(clippy::multiple_unsafe_ops_per_block)]

use crate::{
    command::Command,
    error::{CommandError, print_err_recursive},
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
mod error;
mod layer;

fn main() {
    let stdin_channel = {
        let (tx, rx) = mpsc::channel::<String>();
        thread::spawn(move || {
            loop {
                let mut buffer = String::new();
                stdin().read_line(&mut buffer).unwrap();
                if buffer.ends_with('\n') {
                    buffer.pop();
                    #[cfg(windows)]
                    if buffer.ends_with('\r') {
                        buffer.pop();
                    }
                }
                tx.send(buffer).unwrap();
            }
        });
        rx
    };
    let (mut rl, thread) = init().title("Amity FX").size(1280, 720).resizable().build();

    rl.set_target_fps(60);

    let mut history: VecDeque<String> = VecDeque::new();
    let mut layers: Vec<Layer> = Vec::new();
    let mut curr_layer: usize = 0;

    'mainloop: while !rl.window_should_close() {
        match stdin_channel.try_recv() {
            Ok(input) => {
                let input = &*history.push_back_mut(input);
                // todo: make this more advanced so layer names can have spaces
                match Command::try_parse_from(std::iter::once("").chain(input.split_whitespace()))
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
                    }
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                break 'mainloop;
            }
        }

        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        for layer in layers.iter_mut().rev() {
            layer.render_recursively(&mut d, &thread);
        }
    }
}
