pub mod generator;
pub mod navigator;
pub mod parser;

pub use generator::DiffGenerator;
pub use navigator::HunkNavigator;
pub use parser::DiffParser;

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
    pub header: HunkHeader,
    pub lines: Vec<DiffLine>,
    pub old_range: LineRange,
    pub new_range: LineRange,
    pub stageable: bool,
    pub context_lines: usize,
}

#[derive(Debug, Clone)]
pub struct HunkHeader {
    pub raw: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
}

#[derive(Debug, Clone)]
pub struct LineRange {
    pub start: u32,
    pub count: u32,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub content: String,
    pub line_type: LineType,
    pub old_line_no: Option<usize>,
    pub new_line_no: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineType {
    Context,
    Addition,
    Deletion,
    NoNewlineEOF,
}

#[derive(Debug, PartialEq)]
pub struct HunkPosition {
    pub start_line: usize,
    pub end_line: usize,
    pub screen_y: usize,
}
