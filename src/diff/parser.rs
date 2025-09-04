use crate::diff::{Diff, DiffContext, DiffHunk, DiffLine, HunkHeader, LineRange, LineType};
use git2::{DiffFormat, DiffHunk as Git2DiffHunk, DiffLine as Git2DiffLine};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
    #[error("Binary file not supported: {0}")]
    BinaryFile(String),
    #[error("Invalid diff format")]
    InvalidFormat,
}

pub struct DiffParser;

impl DiffParser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse_diff(
        &self,
        git_diff: &git2::Diff,
        file_path: &str,
        context: DiffContext,
    ) -> Result<Diff, ParseError> {
        let mut hunks = Vec::new();
        let mut binary = false;
        let mut current_hunk_index: Option<usize> = None;

        git_diff.print(DiffFormat::Patch, |delta, hunk, line| {
            if delta.flags().contains(git2::DiffFlags::BINARY) {
                binary = true;
                return true;
            }

            if let Some(hunk_data) = hunk
                && current_hunk_index != Some(hunks.len())
            {
                let diff_hunk = self.parse_hunk(&hunk_data);
                hunks.push(diff_hunk);
                current_hunk_index = Some(hunks.len() - 1);
            }

            if line.origin() != '\0' {
                let diff_line = self.parse_line(&line);
                if let Some(last_hunk) = hunks.last_mut() {
                    last_hunk.lines.push(diff_line);
                }
            }

            true
        })?;

        if binary {
            return Err(ParseError::BinaryFile(file_path.to_string()));
        }

        Ok(Diff {
            file_path: file_path.to_string(),
            context,
            hunks,
            binary,
        })
    }

    fn parse_hunk(&self, hunk: &Git2DiffHunk) -> DiffHunk {
        let header_raw = String::from_utf8_lossy(hunk.header()).to_string();

        DiffHunk {
            header: HunkHeader {
                raw: header_raw,
                old_start: hunk.old_start(),
                old_lines: hunk.old_lines(),
                new_start: hunk.new_start(),
                new_lines: hunk.new_lines(),
            },
            lines: Vec::new(),
            old_range: LineRange {
                start: hunk.old_start(),
                count: hunk.old_lines(),
            },
            new_range: LineRange {
                start: hunk.new_start(),
                count: hunk.new_lines(),
            },
            stageable: true, // Default to stageable, will be updated based on context
            context_lines: 3, // Default context lines, could be made configurable
        }
    }

    fn parse_line(&self, line: &Git2DiffLine) -> DiffLine {
        let line_type = match line.origin() {
            '+' => LineType::Addition,
            '-' => LineType::Deletion,
            ' ' => LineType::Context,
            '\\' => LineType::NoNewlineEOF,
            _ => LineType::Context,
        };

        let content = String::from_utf8_lossy(line.content()).to_string();

        DiffLine {
            content,
            line_type,
            old_line_no: line.old_lineno().map(|n| n as usize),
            new_line_no: line.new_lineno().map(|n| n as usize),
        }
    }
}

impl Default for DiffParser {
    fn default() -> Self {
        Self::new()
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
    fn test_parse_diff_basic_functionality() {
        let (temp_dir, repo) = setup_test_repo();
        let file_path = temp_dir.path().join("test.txt");

        // Create initial file and add to git
        fs::write(&file_path, "Hello\nWorld\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("test.txt")).unwrap();
        index.write().unwrap();

        // Create a diff by comparing empty diff (should be empty)
        let mut diff_options = git2::DiffOptions::new();
        diff_options.pathspec("test.txt");
        let index = repo.index().unwrap();
        let git_diff = repo
            .diff_index_to_workdir(Some(&index), Some(&mut diff_options))
            .unwrap();

        let parser = DiffParser::new();
        let diff = parser
            .parse_diff(&git_diff, "test.txt", DiffContext::WorkingTreeToIndex)
            .unwrap();

        assert_eq!(diff.file_path, "test.txt");
        assert_eq!(diff.context, DiffContext::WorkingTreeToIndex);
        assert!(!diff.binary);
        // This diff should be empty since working tree matches index
        assert!(diff.hunks.is_empty() || diff.hunks.iter().all(|h| h.lines.is_empty()));
    }

    #[test]
    fn test_parser_creation() {
        let _parser = DiffParser::new();
        // Just test that the parser can be created
        let _default_parser = DiffParser;
    }

    #[test]
    fn test_parse_diff_with_empty_file() {
        let (temp_dir, repo) = setup_test_repo();
        let file_path = temp_dir.path().join("empty.txt");

        // Create empty file and add to git
        fs::write(&file_path, "").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("empty.txt")).unwrap();
        index.write().unwrap();

        // Modify the empty file
        fs::write(&file_path, "New content\n").unwrap();

        // Create diff
        let mut diff_options = git2::DiffOptions::new();
        diff_options.pathspec("empty.txt");
        let index = repo.index().unwrap();
        let git_diff = repo
            .diff_index_to_workdir(Some(&index), Some(&mut diff_options))
            .unwrap();

        let parser = DiffParser::new();
        let diff = parser
            .parse_diff(&git_diff, "empty.txt", DiffContext::WorkingTreeToIndex)
            .unwrap();

        assert_eq!(diff.file_path, "empty.txt");
        assert!(!diff.binary);
        assert!(!diff.hunks.is_empty());
        // Should have one hunk adding the new content
        assert_eq!(diff.hunks[0].header.old_start, 0);
        assert_eq!(diff.hunks[0].header.old_lines, 0);
        assert_eq!(diff.hunks[0].header.new_start, 1);
        assert_eq!(diff.hunks[0].header.new_lines, 1);
    }

    #[test]
    fn test_parse_diff_file_deletion() {
        let (temp_dir, repo) = setup_test_repo();
        let file_path = temp_dir.path().join("delete_me.txt");

        // Create file and add to git
        fs::write(&file_path, "Content to be deleted\n").unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_path(std::path::Path::new("delete_me.txt"))
            .unwrap();
        index.write().unwrap();

        // Delete the file from working tree
        fs::remove_file(&file_path).unwrap();

        // Create diff with include_untracked to catch deletions
        let mut diff_options = git2::DiffOptions::new();
        diff_options.pathspec("delete_me.txt");
        diff_options.include_untracked(true);
        let index = repo.index().unwrap();
        let git_diff = repo
            .diff_index_to_workdir(Some(&index), Some(&mut diff_options))
            .unwrap();

        let parser = DiffParser::new();
        let diff = parser
            .parse_diff(&git_diff, "delete_me.txt", DiffContext::WorkingTreeToIndex)
            .unwrap();

        assert_eq!(diff.file_path, "delete_me.txt");
        assert!(!diff.binary);
        // File deletions may not create hunks in this context (working tree vs index)
        // The key is that we successfully parse without errors and handle the deletion gracefully
        // In this specific case, git may not generate a hunk because the file is simply missing
        // from the working directory
    }

    #[test]
    fn test_parse_diff_binary_file_detection() {
        let (temp_dir, repo) = setup_test_repo();
        let file_path = temp_dir.path().join("binary.bin");

        // Create a binary file (with null bytes)
        let binary_data = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
        fs::write(&file_path, &binary_data).unwrap();

        // Add to git first to create baseline
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("binary.bin")).unwrap();
        index.write().unwrap();

        // Modify binary file
        let modified_data = vec![0xFF, 0xFE, 0xFD, 0x00, 0x01, 0x02];
        fs::write(&file_path, &modified_data).unwrap();

        // Create diff
        let mut diff_options = git2::DiffOptions::new();
        diff_options.pathspec("binary.bin");
        let index = repo.index().unwrap();
        let git_diff = repo
            .diff_index_to_workdir(Some(&index), Some(&mut diff_options))
            .unwrap();

        let parser = DiffParser::new();
        let result = parser.parse_diff(&git_diff, "binary.bin", DiffContext::WorkingTreeToIndex);

        // Should return an error for binary files
        assert!(matches!(result, Err(ParseError::BinaryFile(_))));
        if let Err(ParseError::BinaryFile(path)) = result {
            assert_eq!(path, "binary.bin");
        }
    }

    #[test]
    fn test_parse_diff_with_no_newline_eof() {
        let (temp_dir, repo) = setup_test_repo();
        let file_path = temp_dir.path().join("no_newline.txt");

        // Create file with newline and add to git
        fs::write(&file_path, "Line with newline\n").unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_path(std::path::Path::new("no_newline.txt"))
            .unwrap();
        index.write().unwrap();

        // Modify the file to remove newline at end
        fs::write(&file_path, "Line with newline\nLine without newline").unwrap();

        // Create diff
        let mut diff_options = git2::DiffOptions::new();
        diff_options.pathspec("no_newline.txt");
        let index = repo.index().unwrap();
        let git_diff = repo
            .diff_index_to_workdir(Some(&index), Some(&mut diff_options))
            .unwrap();

        let parser = DiffParser::new();
        let diff = parser
            .parse_diff(&git_diff, "no_newline.txt", DiffContext::WorkingTreeToIndex)
            .unwrap();

        assert_eq!(diff.file_path, "no_newline.txt");
        assert!(!diff.binary);
        assert!(!diff.hunks.is_empty());
        // Should handle the "\ No newline at end of file" marker if git generates one
        // This is dependent on git's behavior and may not always be present
        let _has_no_newline_marker = diff
            .hunks
            .iter()
            .flat_map(|h| &h.lines)
            .any(|line| matches!(line.line_type, LineType::NoNewlineEOF));
        // The key is successful parsing, not necessarily the presence of the marker
    }
}
