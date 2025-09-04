use crate::diff::{Diff, DiffContext, DiffHunk, DiffLine, DiffLineType};
use git2::{DiffOptions, Repository};
use std::path::Path;

pub struct DiffGenerator<'repo> {
    repo: &'repo Repository,
}

#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Binary file not supported for diff: {0}")]
    BinaryFile(String),
    #[error("File too large ({0} bytes): {1}")]
    FileTooLarge(u64, String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Terminal compatibility issue: {0}")]
    TerminalCompatibility(String),
}

impl<'repo> DiffGenerator<'repo> {
    const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB limit
    const MAX_LINES_PER_DIFF: usize = 10000; // Limit lines for terminal performance

    pub fn new(repo: &'repo Repository) -> Self {
        Self { repo }
    }

    pub fn generate_diff(&self, file_path: &str, context: DiffContext) -> Result<Diff, DiffError> {
        // Pre-check file size and binary status
        self.check_file_constraints(file_path, &context)?;
        let mut diff_options = DiffOptions::new();
        diff_options.pathspec(file_path);

        let git_diff = match context {
            DiffContext::WorkingTreeToIndex => {
                let index = self.repo.index()?;
                self.repo
                    .diff_index_to_workdir(Some(&index), Some(&mut diff_options))?
            }
            DiffContext::IndexToHead => {
                let head = self.repo.head()?;
                let head_tree = head.peel_to_tree()?;
                let index = self.repo.index()?;
                self.repo.diff_tree_to_index(
                    Some(&head_tree),
                    Some(&index),
                    Some(&mut diff_options),
                )?
            }
            DiffContext::WorkingTreeToHead => {
                let head = self.repo.head()?;
                let head_tree = head.peel_to_tree()?;
                self.repo
                    .diff_tree_to_workdir(Some(&head_tree), Some(&mut diff_options))?
            }
        };

        let mut hunks = Vec::new();
        let mut binary = false;
        let mut line_count = 0;

        git_diff.print(git2::DiffFormat::Patch, |delta, hunk, line| {
            // Check for binary files using delta flags
            if delta.flags().contains(git2::DiffFlags::BINARY) {
                binary = true;
                return false; // Stop processing
            }
            if let Some(hunk_data) = hunk {
                let current_header = String::from_utf8_lossy(hunk_data.header()).to_string();

                // Check if this is a new hunk we haven't seen yet
                let is_new_hunk = hunks.is_empty()
                    || hunks.last().map(|h: &DiffHunk| &h.header) != Some(&current_header);

                if is_new_hunk {
                    let diff_hunk = DiffHunk {
                        header: current_header.clone(),
                        old_start: hunk_data.old_start(),
                        old_lines: hunk_data.old_lines(),
                        new_start: hunk_data.new_start(),
                        new_lines: hunk_data.new_lines(),
                        lines: Vec::new(),
                    };
                    hunks.push(diff_hunk);
                }
            }

            // Only process actual diff content lines (not headers or other metadata)
            match line.origin() {
                '+' | '-' | ' ' | '\\' => {
                    // Check line count limit for terminal performance
                    line_count += 1;
                    if line_count > Self::MAX_LINES_PER_DIFF {
                        return false; // Stop processing to prevent terminal overflow
                    }

                    let line_type = match line.origin() {
                        '+' => DiffLineType::Addition,
                        '-' => DiffLineType::Deletion,
                        ' ' => DiffLineType::Context,
                        '\\' => DiffLineType::NoNewlineWarning,
                        _ => unreachable!(),
                    };

                    let content = String::from_utf8_lossy(line.content()).to_string();

                    // Check for terminal compatibility issues (non-printable characters)
                    if Self::contains_problematic_chars(&content) {
                        return false;
                    }

                    let old_line_number = line.old_lineno();
                    let new_line_number = line.new_lineno();

                    let diff_line = DiffLine {
                        line_type,
                        content,
                        old_line_number,
                        new_line_number,
                    };

                    if let Some(last_hunk) = hunks.last_mut() {
                        last_hunk.lines.push(diff_line);
                    }
                }
                _ => {
                    // Ignore other line types (headers, metadata, etc.)
                }
            }

            true
        })?;

        if binary {
            return Err(DiffError::BinaryFile(file_path.to_string()));
        }

        // Check if diff was truncated due to line limits
        if line_count > Self::MAX_LINES_PER_DIFF {
            return Err(DiffError::TerminalCompatibility(format!(
                "Diff truncated - {} has too many changes (>{} lines)",
                file_path,
                Self::MAX_LINES_PER_DIFF
            )));
        }

        Ok(Diff {
            file_path: file_path.to_string(),
            context,
            hunks,
            binary,
        })
    }

    fn check_file_constraints(
        &self,
        file_path: &str,
        context: &DiffContext,
    ) -> Result<(), DiffError> {
        // For working tree comparisons, check actual file size
        if matches!(
            context,
            DiffContext::WorkingTreeToIndex | DiffContext::WorkingTreeToHead
        ) {
            let repo_workdir = self.repo.workdir().ok_or_else(|| {
                DiffError::Git(git2::Error::from_str("Repository has no working directory"))
            })?;
            let full_path = repo_workdir.join(file_path);

            if full_path.exists() {
                let metadata = std::fs::metadata(&full_path)?;
                let file_size = metadata.len();

                if file_size > Self::MAX_FILE_SIZE {
                    return Err(DiffError::FileTooLarge(file_size, file_path.to_string()));
                }

                // Check if file appears to be binary by examining first few bytes
                if self.is_likely_binary_file(&full_path)? {
                    return Err(DiffError::BinaryFile(file_path.to_string()));
                }
            }
        }

        Ok(())
    }

    fn is_likely_binary_file(&self, path: &Path) -> Result<bool, DiffError> {
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(path)?;
        let mut buffer = [0; 512]; // Check first 512 bytes
        let bytes_read = file.read(&mut buffer)?;

        // Check for null bytes or high percentage of non-ASCII characters
        let null_count = buffer[..bytes_read].iter().filter(|&&b| b == 0).count();
        let non_ascii_count = buffer[..bytes_read]
            .iter()
            .filter(|&&b| b > 127 || (b < 32 && b != 9 && b != 10 && b != 13))
            .count();

        // Consider binary if more than 1% null bytes or more than 30% non-ASCII
        Ok(null_count > bytes_read / 100 || non_ascii_count > bytes_read * 30 / 100)
    }

    fn contains_problematic_chars(content: &str) -> bool {
        content.chars().any(|c| {
            // Check for control characters that might cause terminal issues
            c.is_control() && c != '\t' && c != '\n' && c != '\r'
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_repo() -> (TempDir, Repository) {
        let temp_dir = TempDir::new().unwrap();
        let repo = Repository::init(temp_dir.path()).unwrap();

        let signature =
            git2::Signature::new("Test", "test@example.com", &git2::Time::new(0, 0)).unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        {
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                "Initial commit",
                &tree,
                &[],
            )
            .unwrap();
        }

        (temp_dir, repo)
    }

    #[test]
    fn test_diff_generator_creation() {
        let (_temp_dir, repo) = setup_test_repo();
        let generator = DiffGenerator::new(&repo);
        assert!(std::ptr::eq(generator.repo, &repo));
    }

    #[test]
    fn test_working_tree_to_index_diff() {
        let (temp_dir, repo) = setup_test_repo();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Hello\nWorld\n").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("test.txt")).unwrap();
        index.write().unwrap();

        fs::write(&file_path, "Hello\nWorld\nModified\n").unwrap();

        let generator = DiffGenerator::new(&repo);
        let diff = generator
            .generate_diff("test.txt", DiffContext::WorkingTreeToIndex)
            .unwrap();

        assert_eq!(diff.file_path, "test.txt");
        assert_eq!(diff.context, DiffContext::WorkingTreeToIndex);
        assert!(!diff.binary);
        assert!(!diff.hunks.is_empty());
    }

    #[test]
    fn test_binary_file_detection() {
        let (temp_dir, repo) = setup_test_repo();
        let file_path = temp_dir.path().join("binary.bin");

        // Create a binary file with null bytes
        let binary_data = vec![0u8, 1u8, 2u8, 0u8, 255u8];
        std::fs::write(&file_path, binary_data).unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("binary.bin")).unwrap();
        index.write().unwrap();

        let generator = DiffGenerator::new(&repo);
        let result = generator.generate_diff("binary.bin", DiffContext::WorkingTreeToIndex);

        match result {
            Err(DiffError::BinaryFile(_)) => {
                // Expected
            }
            _ => panic!("Expected binary file error"),
        }
    }

    #[test]
    fn test_file_size_limit() {
        let (temp_dir, repo) = setup_test_repo();
        let file_path = temp_dir.path().join("large.txt");

        // Create a large file exceeding the limit (but still text)
        let large_content = "a".repeat((DiffGenerator::MAX_FILE_SIZE + 1000) as usize);
        std::fs::write(&file_path, large_content).unwrap();

        let generator = DiffGenerator::new(&repo);
        let result = generator.generate_diff("large.txt", DiffContext::WorkingTreeToIndex);

        match result {
            Err(DiffError::FileTooLarge(size, _)) => {
                assert!(size > DiffGenerator::MAX_FILE_SIZE);
            }
            _ => panic!("Expected file too large error"),
        }
    }
}
