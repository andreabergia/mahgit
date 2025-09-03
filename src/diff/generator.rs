use crate::diff::{Diff, DiffContext, DiffHunk, DiffLine, DiffLineType};
use git2::{DiffOptions, Repository};

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
    #[error("File too large: {0}")]
    FileTooLarge(String),
}

impl<'repo> DiffGenerator<'repo> {
    pub fn new(repo: &'repo Repository) -> Self {
        Self { repo }
    }

    pub fn generate_diff(&self, file_path: &str, context: DiffContext) -> Result<Diff, DiffError> {
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
        let binary = false;

        git_diff.print(git2::DiffFormat::Patch, |_delta, hunk, line| {
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
                    let line_type = match line.origin() {
                        '+' => DiffLineType::Addition,
                        '-' => DiffLineType::Deletion,
                        ' ' => DiffLineType::Context,
                        '\\' => DiffLineType::NoNewlineWarning,
                        _ => unreachable!(),
                    };

                    let content = String::from_utf8_lossy(line.content()).to_string();
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

        Ok(Diff {
            file_path: file_path.to_string(),
            context,
            hunks,
            binary,
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
}
