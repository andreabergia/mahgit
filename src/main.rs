pub mod config;
pub mod diff;
pub mod operations;
pub mod repository;
pub mod status;
pub mod theme;
pub mod ui;

use config::Config;
use repository::{Repository, RepositoryError};
use status::RepositoryStatus;
use std::{env, process};
use ui::{App, console};

fn main() {
    let config = load_config();
    if let Err(e) = run(config) {
        eprintln!("Error: {}", e);
        process::exit(match e {
            RepositoryError::NotFound | RepositoryError::NotARepository => 128,
            RepositoryError::AccessDenied => 1,
            RepositoryError::Corrupted(_) => 1,
            RepositoryError::GitError(_) => 1,
            RepositoryError::IoError(_) => 1,
            RepositoryError::Other(_) => 1,
        });
    }
}

fn load_config() -> Config {
    match Config::load() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            process::exit(1);
        }
    }
}

fn run(config: Config) -> Result<(), RepositoryError> {
    let args: Vec<String> = env::args().collect();
    let use_console = args.iter().any(|arg| arg == "--console");

    let repo = Repository::discover(".")?;
    let status = RepositoryStatus::new(&repo)?;

    if use_console {
        console::display_status(&status);
    } else {
        let mut app = App::new(repo, status, config);
        if let Err(e) = app.run() {
            eprintln!("UI Error: {}", e);
            process::exit(1);
        }
    }

    Ok(())
}
