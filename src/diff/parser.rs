use crate::diff::{Diff, DiffContext, DiffHunk, DiffLine, DiffLineType};
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
        DiffHunk {
            header: String::from_utf8_lossy(hunk.header()).to_string(),
            old_start: hunk.old_start(),
            old_lines: hunk.old_lines(),
            new_start: hunk.new_start(),
            new_lines: hunk.new_lines(),
            lines: Vec::new(),
        }
    }

    fn parse_line(&self, line: &Git2DiffLine) -> DiffLine {
        let line_type = match line.origin() {
            '+' => DiffLineType::Addition,
            '-' => DiffLineType::Deletion,
            ' ' => DiffLineType::Context,
            '\\' => DiffLineType::NoNewlineWarning,
            _ => DiffLineType::Context,
        };

        let content = String::from_utf8_lossy(line.content()).to_string();

        DiffLine {
            line_type,
            content,
            old_line_number: line.old_lineno(),
            new_line_number: line.new_lineno(),
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
}
