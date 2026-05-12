pub trait Success: std::fmt::Debug + std::fmt::Display {
    fn source(&self) -> Option<&dyn Success> {
        None
    }
}

impl Success for String {}
impl Success for &str {}
impl Success for std::fmt::Arguments<'_> {}

pub fn print_success_recursive(mut s: &dyn Success) {
    print!("\x1b[1;96m{s}");
    while let Some(src) = s.source() {
        print!(": {src}");
        s = src;
    }
    println!("\x1b[0m");
}

pub trait Warning: std::fmt::Debug + std::fmt::Display {
    fn source(&self) -> Option<&dyn Warning> {
        None
    }
}

impl Warning for String {}
impl Warning for &str {}
impl Warning for std::fmt::Arguments<'_> {}

pub fn print_warning_recursive(mut w: &dyn Warning) {
    print!("\x1b[1;93mwarning:\x1b[0m {w}");
    while let Some(src) = w.source() {
        print!(": {src}");
        w = src;
    }
    println!();
}

use std::error::Error;

pub fn print_err_recursive(mut e: &(dyn Error + 'static)) {
    eprint!("\x1b[1;91merror:\x1b[0m {e}");
    while let Some(src) = e.source() {
        eprint!(": {src}");
        e = src;
    }
    eprintln!();
}
