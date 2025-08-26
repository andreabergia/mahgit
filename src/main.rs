pub mod display;
pub mod repository;
pub mod status;

use display::display_status;
use repository::{Repository, RepositoryError};
use status::RepositoryStatus;
use std::process;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        process::exit(match e {
            RepositoryError::NotFound | RepositoryError::NotARepository => 128,
            RepositoryError::AccessDenied => 1,
            RepositoryError::Corrupted(_) => 1,
            RepositoryError::Other(_) => 1,
        });
    }
}

fn run() -> Result<(), RepositoryError> {
    let repo = Repository::discover(".")?;
    let status = RepositoryStatus::new(&repo)?;
    display_status(&status);
    Ok(())
}
