use crate::diff::{Diff, DiffContext, DiffHunk, DiffLine, LineType};
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
                    || hunks.last().map(|h: &DiffHunk| &h.header.raw) != Some(&current_header);

                if is_new_hunk {
                    use crate::diff::{HunkHeader, LineRange};
                    let diff_hunk = DiffHunk {
                        header: HunkHeader {
                            raw: current_header.clone(),
                            old_start: hunk_data.old_start(),
                            old_lines: hunk_data.old_lines(),
                            new_start: hunk_data.new_start(),
                            new_lines: hunk_data.new_lines(),
                        },
                        old_range: LineRange {
                            start: hunk_data.old_start(),
                            count: hunk_data.old_lines(),
                        },
                        new_range: LineRange {
                            start: hunk_data.new_start(),
                            count: hunk_data.new_lines(),
                        },
                        stageable: true,
                        context_lines: 3,
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
                        '+' => LineType::Addition,
                        '-' => LineType::Deletion,
                        ' ' => LineType::Context,
                        '\\' => LineType::NoNewlineEOF,
                        _ => unreachable!(),
                    };

                    let content = String::from_utf8_lossy(line.content()).to_string();

                    // Check for terminal compatibility issues (non-printable characters)
                    if Self::contains_problematic_chars(&content) {
                        return false;
                    }

                    let old_line_no = line.old_lineno().map(|n| n as usize);
                    let new_line_no = line.new_lineno().map(|n| n as usize);

                    let diff_line = DiffLine {
                        content,
                        line_type,
                        old_line_no,
                        new_line_no,
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

        // If no hunks were generated and this is a working tree comparison,
        // check if it's an untracked file and generate synthetic diff
        if hunks.is_empty()
            && matches!(context, DiffContext::WorkingTreeToIndex)
            && let Ok(synthetic_diff) =
                self.generate_untracked_file_diff(file_path, context.clone())
        {
            return Ok(synthetic_diff);
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

    fn generate_untracked_file_diff(
        &self,
        file_path: &str,
        context: DiffContext,
    ) -> Result<Diff, DiffError> {
        use crate::diff::{HunkHeader, LineRange};
        use std::fs;
        use std::path::Path;

        let repo_workdir = self.repo.workdir().ok_or_else(|| {
            DiffError::Git(git2::Error::from_str("Repository has no working directory"))
        })?;
        let full_path = repo_workdir.join(file_path);

        let index = self.repo.index()?;
        let file_exists_in_working_tree = full_path.exists();
        let file_exists_in_index = index.get_path(Path::new(file_path), 0).is_some();

        match (file_exists_in_working_tree, file_exists_in_index) {
            (true, false) => {
                // File exists in working tree but not in index (untracked)
                // Continue with untracked file diff generation below
            }
            (false, true) => {
                // File deleted from working tree but still in index - generate deletion diff
                return self.generate_deleted_file_diff(file_path, context);
            }
            (false, false) => {
                // File doesn't exist anywhere
                return Err(DiffError::FileNotFound(file_path.to_string()));
            }
            (true, true) => {
                // File exists in both - this should be handled by regular git diff,
                // not untracked file diff
                return Err(DiffError::FileNotFound(format!(
                    "File {} exists in both working tree and index",
                    file_path
                )));
            }
        }

        // Read file content
        let file_content = fs::read_to_string(&full_path)?;

        // Check for problematic characters
        if Self::contains_problematic_chars(&file_content) {
            return Err(DiffError::TerminalCompatibility(
                "File contains problematic characters that may cause terminal issues".to_string(),
            ));
        }

        let lines: Vec<&str> = file_content.lines().collect();
        let line_count = lines.len();

        // Check line count limit
        if line_count > Self::MAX_LINES_PER_DIFF {
            return Err(DiffError::TerminalCompatibility(format!(
                "File too large ({} lines) - cannot display diff",
                line_count
            )));
        }

        // Create synthetic hunk showing entire file as additions
        let header = HunkHeader {
            raw: format!("@@ -0,0 +1,{} @@", line_count),
            old_start: 0,
            old_lines: 0,
            new_start: 1,
            new_lines: line_count as u32,
        };

        let mut diff_lines = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            diff_lines.push(crate::diff::DiffLine {
                content: line.to_string(),
                line_type: crate::diff::LineType::Addition,
                old_line_no: None,
                new_line_no: Some(i + 1),
            });
        }

        let hunk = crate::diff::DiffHunk {
            header,
            lines: diff_lines,
            old_range: LineRange { start: 0, count: 0 },
            new_range: LineRange {
                start: 1,
                count: line_count as u32,
            },
            stageable: true,
            context_lines: 3,
        };

        Ok(Diff {
            file_path: file_path.to_string(),
            context,
            hunks: vec![hunk],
            binary: false,
        })
    }

    fn generate_deleted_file_diff(
        &self,
        file_path: &str,
        context: DiffContext,
    ) -> Result<Diff, DiffError> {
        use crate::diff::{HunkHeader, LineRange};
        use std::path::Path;

        // Get the file content from the index
        let index = self.repo.index()?;
        let entry = index.get_path(Path::new(file_path), 0).ok_or_else(|| {
            DiffError::FileNotFound(format!("File {} not found in index", file_path))
        })?;

        // Get the blob content from the index
        let blob = self.repo.find_blob(entry.id)?;
        let content = std::str::from_utf8(blob.content())
            .map_err(|_| DiffError::BinaryFile(file_path.to_string()))?;

        // Check for problematic characters
        if Self::contains_problematic_chars(content) {
            return Err(DiffError::TerminalCompatibility(
                "File contains problematic characters that may cause terminal issues".to_string(),
            ));
        }

        let lines: Vec<&str> = content.lines().collect();
        let line_count = lines.len();

        // Check line count limit
        if line_count > Self::MAX_LINES_PER_DIFF {
            return Err(DiffError::TerminalCompatibility(format!(
                "File too large ({} lines) - cannot display diff",
                line_count
            )));
        }

        // Create synthetic hunk showing entire file as deletions
        let header = HunkHeader {
            raw: format!("@@ -1,{} +0,0 @@", line_count),
            old_start: 1,
            old_lines: line_count as u32,
            new_start: 0,
            new_lines: 0,
        };

        let mut diff_lines = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            diff_lines.push(crate::diff::DiffLine {
                content: line.to_string(),
                line_type: crate::diff::LineType::Deletion,
                old_line_no: Some(i + 1),
                new_line_no: None,
            });
        }

        let hunk = crate::diff::DiffHunk {
            header,
            lines: diff_lines,
            old_range: LineRange {
                start: 1,
                count: line_count as u32,
            },
            new_range: LineRange { start: 0, count: 0 },
            stageable: true,
            context_lines: 3,
        };

        Ok(Diff {
            file_path: file_path.to_string(),
            context,
            hunks: vec![hunk],
            binary: false,
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
    fn test_index_to_head_diff() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize repository
        let repo = git2::Repository::init(repo_path).unwrap();

        // Set up initial commit
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        // Create a test file and stage it
        let test_file_path = repo_path.join("test.txt");
        fs::write(&test_file_path, "staged content\n").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("test.txt")).unwrap();
        index.write().unwrap();

        // Test IndexToHead diff generation
        let generator = DiffGenerator::new(&repo);
        let result = generator.generate_diff("test.txt", DiffContext::IndexToHead);

        match result {
            Ok(diff) => {
                assert!(!diff.hunks.is_empty(), "IndexToHead diff should have hunks");
            }
            Err(e) => {
                panic!("IndexToHead diff generation failed: {}", e);
            }
        }
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

    #[test]
    fn test_untracked_file_diff() {
        let (temp_dir, repo) = setup_test_repo();
        let file_path = temp_dir.path().join("untracked.txt");

        // Create an untracked file (don't add to index)
        std::fs::write(&file_path, "New file line 1\nNew file line 2\n").unwrap();

        let generator = DiffGenerator::new(&repo);
        let diff = generator
            .generate_diff("untracked.txt", DiffContext::WorkingTreeToIndex)
            .unwrap();

        assert_eq!(diff.file_path, "untracked.txt");
        assert_eq!(diff.context, DiffContext::WorkingTreeToIndex);
        assert!(!diff.binary);
        assert_eq!(diff.hunks.len(), 1);

        let hunk = &diff.hunks[0];
        assert_eq!(hunk.header.old_start, 0);
        assert_eq!(hunk.header.old_lines, 0);
        assert_eq!(hunk.header.new_start, 1);
        assert_eq!(hunk.header.new_lines, 2);
        assert_eq!(hunk.lines.len(), 2);

        // Check that all lines are additions
        for line in &hunk.lines {
            assert_eq!(line.line_type, crate::diff::LineType::Addition);
            assert_eq!(line.old_line_no, None);
            assert!(line.new_line_no.is_some());
        }

        assert_eq!(hunk.lines[0].content, "New file line 1");
        assert_eq!(hunk.lines[1].content, "New file line 2");
        assert_eq!(hunk.lines[0].new_line_no, Some(1));
        assert_eq!(hunk.lines[1].new_line_no, Some(2));
    }

    #[test]
    fn test_deleted_file_diff() {
        let (temp_dir, repo) = setup_test_repo();
        let file_path = temp_dir.path().join("to_delete.txt");

        // Create a file, add it to index, then delete it from working tree
        std::fs::write(&file_path, "Line to be deleted\nAnother line\n").unwrap();

        let mut index = repo.index().unwrap();
        index
            .add_path(std::path::Path::new("to_delete.txt"))
            .unwrap();
        index.write().unwrap();

        // Delete the file from working tree
        std::fs::remove_file(&file_path).unwrap();

        let generator = DiffGenerator::new(&repo);
        let diff = generator
            .generate_diff("to_delete.txt", DiffContext::WorkingTreeToIndex)
            .unwrap();

        assert_eq!(diff.file_path, "to_delete.txt");
        assert_eq!(diff.context, DiffContext::WorkingTreeToIndex);
        assert!(!diff.binary);
        assert_eq!(diff.hunks.len(), 1);

        let hunk = &diff.hunks[0];
        assert_eq!(hunk.header.old_start, 1);
        assert_eq!(hunk.header.old_lines, 2);
        assert_eq!(hunk.header.new_start, 0);
        assert_eq!(hunk.header.new_lines, 0);
        assert_eq!(hunk.lines.len(), 2);

        // Check that all lines are deletions
        for line in &hunk.lines {
            assert_eq!(line.line_type, crate::diff::LineType::Deletion);
            assert!(line.old_line_no.is_some());
            assert_eq!(line.new_line_no, None);
        }

        // Git2's native diff output includes newlines in content
        assert_eq!(hunk.lines[0].content, "Line to be deleted\n");
        assert_eq!(hunk.lines[1].content, "Another line\n");
        assert_eq!(hunk.lines[0].old_line_no, Some(1));
        assert_eq!(hunk.lines[1].old_line_no, Some(2));
    }
}
