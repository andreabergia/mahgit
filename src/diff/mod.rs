pub mod generator;

pub use generator::DiffGenerator;

#[derive(Debug, Clone)]
pub struct Diff {
    pub file_path: String,
    pub context: DiffContext,
    pub hunks: Vec<DiffHunk>,
    pub binary: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiffContext {
    WorkingTreeToIndex, // Unstaged changes
    IndexToHead,        // Staged changes
    WorkingTreeToHead,  // All changes
}

#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub line_type: DiffLineType,
    pub content: String,
    pub old_line_number: Option<u32>,
    pub new_line_number: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiffLineType {
    Context,
    Addition,
    Deletion,
    NoNewlineWarning,
}
