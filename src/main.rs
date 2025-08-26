mod repository;
mod status;
mod display;

use std::process;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    println!("mahgit - Git repository status");
    Ok(())
}
