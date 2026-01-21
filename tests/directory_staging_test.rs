use std::fs;
use std::path::Path;

mod common;
use common::create_test_repository;

#[test]
fn test_add_regular_directory_to_index() {
    let test_repo = create_test_repository().expect("Failed to create test repository");
    let repo = &test_repo.repository;
    let repo_path = test_repo.temp_dir.path();

    // Create a regular directory
    let regular_dir = repo_path.join("regular_dir");
    fs::create_dir_all(&regular_dir).expect("Failed to create regular directory");

    // Create some test files in the directory
    fs::write(regular_dir.join("file1.txt"), "content1").expect("Failed to write file1");
    fs::write(regular_dir.join("file2.txt"), "content2").expect("Failed to write file2");

    // Test adding the regular directory to the index
    let result = repo.add_to_index("regular_dir");
    assert!(
        result.is_ok(),
        "Failed to add regular directory to index: {:?}",
        result.err()
    );

    // Verify that the files were added to the index
    let index = repo.get_index().expect("Failed to get index");

    // Check that all files are in the index
    assert!(
        index
            .get_path(Path::new("regular_dir/file1.txt"), 0)
            .is_some()
    );
    assert!(
        index
            .get_path(Path::new("regular_dir/file2.txt"), 0)
            .is_some()
    );
}

#[test]
fn test_add_directory_with_special_characters_to_index() {
    let test_repo = create_test_repository().expect("Failed to create test repository");
    let repo = &test_repo.repository;
    let repo_path = test_repo.temp_dir.path();

    let special_dir = repo_path.join("A/B C");
    fs::create_dir_all(&special_dir).expect("Failed to create directory with special characters");

    // Create some test files in the directory
    fs::write(special_dir.join("file1.txt"), "content1").expect("Failed to write file1");
    fs::write(special_dir.join("file2.txt"), "content2").expect("Failed to write file2");

    // Create a subdirectory with special characters
    let sub_dir = special_dir.join("Sub Directory");
    fs::create_dir_all(&sub_dir).expect("Failed to create subdirectory");
    fs::write(sub_dir.join("file3.txt"), "content3").expect("Failed to write file3");

    // Test adding the directory with special characters to the index
    let result = repo.add_to_index("A/B C");
    assert!(
        result.is_ok(),
        "Failed to add directory with special characters to index: {:?}",
        result.err()
    );

    // Verify that the files were added to the index
    let index = repo.get_index().expect("Failed to get index");

    // Check that all files are in the index
    assert!(index.get_path(Path::new("A/B C/file1.txt"), 0).is_some());
    assert!(index.get_path(Path::new("A/B C/file2.txt"), 0).is_some());
    assert!(
        index
            .get_path(Path::new("A/B C/Sub Directory/file3.txt"), 0)
            .is_some()
    );
}
