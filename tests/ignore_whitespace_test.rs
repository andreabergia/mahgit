use std::fs;
use std::process::Command;

mod common;

use common::create_test_repository;
use mahgit::diff::{DiffContext, DiffGenerator};

/// End-to-end check: with `ignore_whitespace = true`, a whitespace-only edit
/// in the working tree against HEAD must produce an empty diff; with `false`
/// it must produce a hunk.
#[test]
fn test_ignore_whitespace_end_to_end_workdir_against_head() {
    let test_repo = create_test_repository().expect("Failed to create test repository");
    let repo_path = test_repo.temp_dir.path();

    let file = repo_path.join("indent.rs");
    fs::write(&file, "fn main() {\n    println!(\"hi\");\n}\n").unwrap();

    Command::new("git")
        .args(["add", "indent.rs"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add indent.rs"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    fs::write(&file, "fn main() {\n        println!(\"hi\");\n}\n").unwrap();

    let git2_repo = test_repo.repository.git2_repo();
    let generator = DiffGenerator::new(git2_repo);

    let normal = generator
        .generate_diff_with_context("indent.rs", DiffContext::WorkingTreeToHead, None, false)
        .expect("normal diff");
    assert!(
        !normal.hunks.is_empty(),
        "without ignore_whitespace, the indentation change must produce a hunk"
    );

    let ignored = generator
        .generate_diff_with_context("indent.rs", DiffContext::WorkingTreeToHead, None, true)
        .expect("whitespace-ignoring diff");
    assert!(
        ignored.hunks.is_empty(),
        "with ignore_whitespace, the whitespace-only change must produce no hunks"
    );
}
