use mimalloc::MiMalloc;
use std::path::PathBuf;

#[global_allocator]
static ALLOCATOR: MiMalloc = MiMalloc;

fn main() {
    let path = std::env::args().nth(1).map_or_else(|| PathBuf::from("config.toml"), PathBuf::from);
    match multiogarii::server::run(&path) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            multiogarii::core::logger::error(&e);
            std::process::exit(1);
        }
    }
}
