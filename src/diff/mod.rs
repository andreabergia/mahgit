pub mod generator;
pub mod inline;
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
    pub inline_diff: Option<InlineDiff>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineType {
    Context,
    Addition,
    Deletion,
    NoNewlineEOF,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InlineDiff {
    pub segments: Vec<InlineDiffSegment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InlineDiffSegment {
    pub content: String,
    pub change: InlineChange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InlineChange {
    Unchanged,
    Added,
    Removed,
}

#[derive(Debug, PartialEq)]
pub struct HunkPosition {
    pub start_line: usize,
    pub end_line: usize,
    pub screen_y: usize,
}

pub fn merge_adjacent_hunks(diff: Diff) -> Diff {
    if diff.hunks.is_empty() {
        return diff;
    }

    let mut merged_hunks = Vec::new();
    let mut current_hunk: Option<DiffHunk> = None;

    for hunk in diff.hunks {
        if let Some(ref mut curr) = current_hunk {
            if hunks_are_mergeable(curr, &hunk) {
                merge_hunks(curr, hunk);
                continue;
            } else {
                merged_hunks.push(current_hunk.take().unwrap());
            }
        }
        current_hunk = Some(hunk);
    }

    if let Some(hunk) = current_hunk {
        merged_hunks.push(hunk);
    }

    Diff {
        hunks: merged_hunks,
        ..diff
    }
}

fn hunks_are_mergeable(hunk1: &DiffHunk, hunk2: &DiffHunk) -> bool {
    let old_end = hunk1.old_range.start + hunk1.old_range.count;
    let new_end = hunk1.new_range.start + hunk1.new_range.count;

    let old_contiguous = old_end >= hunk2.old_range.start;
    let new_contiguous = new_end >= hunk2.new_range.start;

    old_contiguous && new_contiguous
}

fn merge_hunks(hunk1: &mut DiffHunk, hunk2: DiffHunk) {
    hunk1.old_range.count = (hunk2.old_range.start + hunk2.old_range.count) - hunk1.old_range.start;
    hunk1.new_range.count = (hunk2.new_range.start + hunk2.new_range.count) - hunk1.new_range.start;

    hunk1.lines.extend(hunk2.lines);

    hunk1.header = generate_hunk_header(&hunk1.old_range, &hunk1.new_range);

    hunk1.stageable = hunk1.stageable && hunk2.stageable;
}

fn generate_hunk_header(old_range: &LineRange, new_range: &LineRange) -> HunkHeader {
    let raw = format!(
        "@@ -{},{} +{},{} @@",
        old_range.start, old_range.count, new_range.start, new_range.count
    );

    HunkHeader {
        raw,
        old_start: old_range.start,
        old_lines: old_range.count,
        new_start: new_range.start,
        new_lines: new_range.count,
    }
}
