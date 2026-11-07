use std::io::Write;
use std::sync::atomic::{AtomicI32, Ordering};

static VERBOSITY: AtomicI32 = AtomicI32::new(4);

pub fn set_verbosity(level: i32) {
    VERBOSITY.store(level, Ordering::Relaxed);
}

fn line(text: std::fmt::Arguments) {
    let _ = writeln!(std::io::stdout(), "{text}");
}

fn enabled(level: i32) -> bool {
    VERBOSITY.load(Ordering::Relaxed) >= level
}

pub fn info(text: &str) {
    if enabled(4) {
        line(format_args!("[\x1b[34mInfo\x1b[0m] {text}"));
    }
}

pub fn warn(text: &str) {
    if enabled(3) {
        line(format_args!("[\x1b[33mWarning\x1b[0m] {text}"));
    }
}

pub fn error(text: &str) {
    if enabled(2) {
        line(format_args!("[\x1b[31mError\x1b[0m] {text}"));
    }
}

pub fn print(text: &str) {
    line(format_args!("{text}"));
}
