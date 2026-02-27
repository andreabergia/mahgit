pub mod config;
pub mod diff;
pub mod log;
pub mod operations;
pub mod repository;
pub mod status;
pub mod theme;
pub mod ui;

use config::{Config, ConfigError};
use repository::{Repository, RepositoryError};
use status::RepositoryStatus;
use std::{env, process};
use ui::{App, console};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return;
    }

    let use_console = args.iter().any(|arg| arg == "--console");
    let config = load_config();
    if let Err(e) = run(config, use_console) {
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

fn print_help() {
    let config_locations = match Config::config_paths() {
        Ok(paths) => paths
            .into_iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>(),
        Err(ConfigError::NoConfigDir) => {
            vec!["Unavailable (could not determine config directory)".to_string()]
        }
        Err(err) => vec![format!("Unavailable ({})", err)],
    };

    println!("mahgit - terminal Git interface inspired by Magit");
    println!();
    println!("Usage: mahgit [--console] [--help]");
    println!();
    println!("Options:");
    println!("  --help     Show this help message");
    println!("  --console  Print repository status to stdout instead of launching the UI");
    println!();
    println!("Configuration file paths:");
    for path in config_locations {
        println!("  {}", path);
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

fn run(config: Config, use_console: bool) -> Result<(), RepositoryError> {
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
