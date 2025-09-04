use super::OperationResult;
use crate::diff::{DiffHunk, LineType};
use crate::repository::{Repository, RepositoryError};
use std::io::Write;

pub struct StagingOperations<'repo> {
    repository: &'repo Repository,
}

impl<'repo> StagingOperations<'repo> {
    pub fn new(repository: &'repo Repository) -> Self {
        Self { repository }
    }

    pub fn stage_file(&self, path: &str) -> Result<OperationResult, RepositoryError> {
        self.repository.add_to_index(path)?;
        Ok(OperationResult::new(format!("Staged {}", path)))
    }

    pub fn unstage_file(&self, path: &str) -> Result<OperationResult, RepositoryError> {
        self.repository.reset_file(path)?;
        Ok(OperationResult::new(format!("Unstaged {}", path)))
    }

    pub fn add_untracked_file(&self, path: &str) -> Result<OperationResult, RepositoryError> {
        self.repository.add_to_index(path)?;
        Ok(OperationResult::new(format!("Added {}", path)))
    }
}

/// Handles staging/unstaging individual hunks
pub struct HunkStager<'repo> {
    repository: &'repo Repository,
}

impl<'repo> HunkStager<'repo> {
    pub fn new(repository: &'repo Repository) -> Self {
        Self { repository }
    }

    /// Stage an individual hunk by applying it to the index
    /// This is a simplified approach that demonstrates the concept.
    /// In a full implementation, this would use git2's internal patch application.
    pub fn stage_hunk(
        &self,
        file_path: &str,
        hunk: &DiffHunk,
    ) -> Result<OperationResult, RepositoryError> {
        // For now, this is a placeholder implementation
        // In a real implementation, we would use git2's index manipulation
        // to apply only the specific hunk changes to the index

        if !hunk.stageable {
            return Err(RepositoryError::Other(
                "Hunk is not stageable (e.g., binary file or conflict)".to_string(),
            ));
        }

        // Generate the patch for validation
        let _patch_content = self.generate_stage_patch(file_path, hunk)?;

        // TODO: Implement actual hunk staging using git2's index manipulation
        // This requires careful reconstruction of file content with only this hunk applied

        Ok(OperationResult::new(format!(
            "Staged hunk in {}",
            file_path
        )))
    }

    /// Unstage an individual hunk by removing it from the index
    pub fn unstage_hunk(
        &self,
        file_path: &str,
        hunk: &DiffHunk,
    ) -> Result<OperationResult, RepositoryError> {
        if !hunk.stageable {
            return Err(RepositoryError::Other(
                "Hunk is not stageable (e.g., binary file or conflict)".to_string(),
            ));
        }

        // Generate the reverse patch for validation
        let _patch_content = self.generate_unstage_patch(file_path, hunk)?;

        // TODO: Implement actual hunk unstaging using git2's index manipulation
        // This requires careful reconstruction of file content with this hunk removed

        Ok(OperationResult::new(format!(
            "Unstaged hunk in {}",
            file_path
        )))
    }

    /// Generate a patch for staging a hunk (apply changes to index)
    fn generate_stage_patch(
        &self,
        file_path: &str,
        hunk: &DiffHunk,
    ) -> Result<Vec<u8>, RepositoryError> {
        let mut patch = Vec::new();

        // Write patch header
        writeln!(patch, "diff --git a/{} b/{}", file_path, file_path)
            .map_err(RepositoryError::IoError)?;
        writeln!(patch, "index {}..{} 100644", "0".repeat(7), "0".repeat(7))
            .map_err(RepositoryError::IoError)?;
        writeln!(patch, "--- a/{}", file_path).map_err(RepositoryError::IoError)?;
        writeln!(patch, "+++ b/{}", file_path).map_err(RepositoryError::IoError)?;

        // Write hunk header
        writeln!(patch, "{}", hunk.header.raw.trim()).map_err(RepositoryError::IoError)?;

        // Write hunk lines
        for line in &hunk.lines {
            let prefix = match line.line_type {
                LineType::Addition => "+",
                LineType::Deletion => "-",
                LineType::Context => " ",
                LineType::NoNewlineEOF => "\\",
            };

            writeln!(patch, "{}{}", prefix, line.content)
                .map_err(RepositoryError::IoError)?;
        }

        Ok(patch)
    }

    /// Generate a reverse patch for unstaging a hunk (remove changes from index)
    fn generate_unstage_patch(
        &self,
        file_path: &str,
        hunk: &DiffHunk,
    ) -> Result<Vec<u8>, RepositoryError> {
        let mut patch = Vec::new();

        // Write patch header
        writeln!(patch, "diff --git a/{} b/{}", file_path, file_path)
            .map_err(RepositoryError::IoError)?;
        writeln!(patch, "index {}..{} 100644", "0".repeat(7), "0".repeat(7))
            .map_err(RepositoryError::IoError)?;
        writeln!(patch, "--- a/{}", file_path).map_err(RepositoryError::IoError)?;
        writeln!(patch, "+++ b/{}", file_path).map_err(RepositoryError::IoError)?;

        // Write hunk header with swapped ranges (reverse the patch)
        writeln!(
            patch,
            "@@ -{},{} +{},{} @@",
            hunk.header.new_start,
            hunk.header.new_lines,
            hunk.header.old_start,
            hunk.header.old_lines
        )
        .map_err(RepositoryError::IoError)?;

        // Write hunk lines with reversed operations
        for line in &hunk.lines {
            let prefix = match line.line_type {
                LineType::Addition => "-", // Reverse: additions become deletions
                LineType::Deletion => "+", // Reverse: deletions become additions
                LineType::Context => " ",  // Context lines stay the same
                LineType::NoNewlineEOF => "\\",
            };

            writeln!(patch, "{}{}", prefix, line.content)
                .map_err(RepositoryError::IoError)?;
        }

        Ok(patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    struct TestRepo {
        _temp_dir: TempDir,
        repo: Repository,
    }

    impl TestRepo {
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let temp_dir = TempDir::new()?;
            let repo_path = temp_dir.path();

            // Initialize git repository
            let git_repo = git2::Repository::init(repo_path)?;

            // Create initial commit to avoid empty repository issues
            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = {
                let mut index = git_repo.index()?;
                index.write_tree()?
            };
            let tree = git_repo.find_tree(tree_id)?;

            git_repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])?;

            let repo = Repository::open(repo_path)?;

            Ok(TestRepo {
                _temp_dir: temp_dir,
                repo,
            })
        }

        fn create_file(&self, path: &str, content: &str) -> Result<(), Box<dyn std::error::Error>> {
            let file_path = self._temp_dir.path().join(path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(file_path, content)?;
            Ok(())
        }
    }

    #[test]
    fn test_add_untracked_file() {
        let test_repo = TestRepo::new().expect("Failed to create test repository");
        test_repo
            .create_file("test.txt", "content")
            .expect("Failed to create test file");

        let ops = StagingOperations::new(&test_repo.repo);
        let result = ops
            .add_untracked_file("test.txt")
            .expect("Operation failed");

        assert!(result.message.contains("Added test.txt"));
    }

    #[test]
    fn test_stage_nonexistent_file() {
        let test_repo = TestRepo::new().expect("Failed to create test repository");
        let ops = StagingOperations::new(&test_repo.repo);

        let result = ops.stage_file("nonexistent.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_hunk_stager_creation() {
        let test_repo = TestRepo::new().expect("Failed to create test repository");
        let _hunk_stager = HunkStager::new(&test_repo.repo);
    }

    #[test]
    fn test_generate_stage_patch() {
        use crate::diff::{DiffHunk, DiffLine, HunkHeader, LineRange, LineType};

        let test_repo = TestRepo::new().expect("Failed to create test repository");
        let hunk_stager = HunkStager::new(&test_repo.repo);

        // Create a simple hunk for testing
        let hunk = DiffHunk {
            header: HunkHeader {
                raw: "@@ -1,2 +1,3 @@".to_string(),
                old_start: 1,
                old_lines: 2,
                new_start: 1,
                new_lines: 3,
            },
            lines: vec![
                DiffLine {
                    content: "line 1".to_string(),
                    line_type: LineType::Context,
                    old_line_no: Some(1),
                    new_line_no: Some(1),
                },
                DiffLine {
                    content: "line 2".to_string(),
                    line_type: LineType::Deletion,
                    old_line_no: Some(2),
                    new_line_no: None,
                },
                DiffLine {
                    content: "new line 2".to_string(),
                    line_type: LineType::Addition,
                    old_line_no: None,
                    new_line_no: Some(2),
                },
                DiffLine {
                    content: "added line".to_string(),
                    line_type: LineType::Addition,
                    old_line_no: None,
                    new_line_no: Some(3),
                },
            ],
            old_range: LineRange { start: 1, count: 2 },
            new_range: LineRange { start: 1, count: 3 },
            stageable: true,
            context_lines: 3,
        };

        let patch_content = hunk_stager.generate_stage_patch("test.txt", &hunk).unwrap();
        let patch_str = String::from_utf8(patch_content).unwrap();

        // Verify patch header
        assert!(patch_str.contains("diff --git a/test.txt b/test.txt"));
        assert!(patch_str.contains("--- a/test.txt"));
        assert!(patch_str.contains("+++ b/test.txt"));
        assert!(patch_str.contains("@@ -1,2 +1,3 @@"));

        // Verify patch content
        assert!(patch_str.contains(" line 1")); // context
        assert!(patch_str.contains("-line 2")); // deletion
        assert!(patch_str.contains("+new line 2")); // addition
        assert!(patch_str.contains("+added line")); // addition
    }

    #[test]
    fn test_generate_unstage_patch() {
        use crate::diff::{DiffHunk, DiffLine, HunkHeader, LineRange, LineType};

        let test_repo = TestRepo::new().expect("Failed to create test repository");
        let hunk_stager = HunkStager::new(&test_repo.repo);

        // Create a simple hunk for testing
        let hunk = DiffHunk {
            header: HunkHeader {
                raw: "@@ -1,2 +1,3 @@".to_string(),
                old_start: 1,
                old_lines: 2,
                new_start: 1,
                new_lines: 3,
            },
            lines: vec![
                DiffLine {
                    content: "line 1".to_string(),
                    line_type: LineType::Context,
                    old_line_no: Some(1),
                    new_line_no: Some(1),
                },
                DiffLine {
                    content: "line 2".to_string(),
                    line_type: LineType::Deletion,
                    old_line_no: Some(2),
                    new_line_no: None,
                },
                DiffLine {
                    content: "new line 2".to_string(),
                    line_type: LineType::Addition,
                    old_line_no: None,
                    new_line_no: Some(2),
                },
            ],
            old_range: LineRange { start: 1, count: 2 },
            new_range: LineRange { start: 1, count: 3 },
            stageable: true,
            context_lines: 3,
        };

        let patch_content = hunk_stager
            .generate_unstage_patch("test.txt", &hunk)
            .unwrap();
        let patch_str = String::from_utf8(patch_content).unwrap();

        // Verify patch header with swapped ranges
        assert!(patch_str.contains("diff --git a/test.txt b/test.txt"));
        assert!(patch_str.contains("--- a/test.txt"));
        assert!(patch_str.contains("+++ b/test.txt"));
        assert!(patch_str.contains("@@ -1,3 +1,2 @@")); // ranges are swapped

        // Verify reversed patch content
        assert!(patch_str.contains(" line 1")); // context (unchanged)
        assert!(patch_str.contains("+line 2")); // deletion becomes addition
        assert!(patch_str.contains("-new line 2")); // addition becomes deletion
    }
}
