use crate::diff::{Diff, InlineChange, InlineDiff, InlineDiffSegment, LineType};
use similar::{ChangeTag, TextDiff};

/// Compute inline word-level diffs for paired deletion/addition lines inside each hunk.
pub fn compute_inline_diffs(diff: &mut Diff) {
    for hunk in &mut diff.hunks {
        let mut idx = 0;
        while idx < hunk.lines.len() {
            if hunk.lines[idx].line_type == LineType::Addition {
                let addition_start = idx;
                while idx < hunk.lines.len() && hunk.lines[idx].line_type == LineType::Addition {
                    idx += 1;
                }

                let addition_count = idx.saturating_sub(addition_start);
                for offset in 0..addition_count {
                    let addition_line = &hunk.lines[addition_start + offset].content;
                    let (_, new_segments) = build_inline_segments("", addition_line);
                    hunk.lines[addition_start + offset].inline_diff = Some(InlineDiff {
                        segments: new_segments,
                    });
                }

                continue;
            }

            if hunk.lines[idx].line_type != LineType::Deletion {
                idx += 1;
                continue;
            }

            let deletion_start = idx;
            while idx < hunk.lines.len() && hunk.lines[idx].line_type == LineType::Deletion {
                idx += 1;
            }

            let addition_start = idx;
            while idx < hunk.lines.len() && hunk.lines[idx].line_type == LineType::Addition {
                idx += 1;
            }

            let deletion_count = addition_start.saturating_sub(deletion_start);
            let addition_count = idx.saturating_sub(addition_start);
            let pair_count = deletion_count.min(addition_count);

            for offset in 0..pair_count {
                let deletion_line = &hunk.lines[deletion_start + offset].content;
                let addition_line = &hunk.lines[addition_start + offset].content;
                let (old_segments, new_segments) =
                    build_inline_segments(deletion_line, addition_line);

                hunk.lines[deletion_start + offset].inline_diff = Some(InlineDiff {
                    segments: old_segments,
                });
                hunk.lines[addition_start + offset].inline_diff = Some(InlineDiff {
                    segments: new_segments,
                });
            }

            if deletion_count > pair_count {
                for offset in pair_count..deletion_count {
                    let deletion_line = &hunk.lines[deletion_start + offset].content;
                    let (old_segments, _) = build_inline_segments(deletion_line, "");
                    hunk.lines[deletion_start + offset].inline_diff = Some(InlineDiff {
                        segments: old_segments,
                    });
                }
            }

            if addition_count > pair_count {
                for offset in pair_count..addition_count {
                    let addition_line = &hunk.lines[addition_start + offset].content;
                    let (_, new_segments) = build_inline_segments("", addition_line);
                    hunk.lines[addition_start + offset].inline_diff = Some(InlineDiff {
                        segments: new_segments,
                    });
                }
            }
        }
    }
}

fn build_inline_segments(
    original: &str,
    updated: &str,
) -> (Vec<InlineDiffSegment>, Vec<InlineDiffSegment>) {
    let diff = TextDiff::configure().diff_words(original, updated);
    let mut original_segments = Vec::new();
    let mut updated_segments = Vec::new();

    for op in diff.ops() {
        for change in diff.iter_changes(op) {
            let value = change.value().to_string();
            match change.tag() {
                ChangeTag::Equal => {
                    original_segments.push(InlineDiffSegment {
                        content: value.clone(),
                        change: InlineChange::Unchanged,
                    });
                    updated_segments.push(InlineDiffSegment {
                        content: value,
                        change: InlineChange::Unchanged,
                    });
                }
                ChangeTag::Delete => original_segments.push(InlineDiffSegment {
                    content: value,
                    change: InlineChange::Removed,
                }),
                ChangeTag::Insert => updated_segments.push(InlineDiffSegment {
                    content: value,
                    change: InlineChange::Added,
                }),
            }
        }
    }

    (original_segments, updated_segments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{DiffContext, DiffHunk, HunkHeader, LineRange};

    fn build_diff(lines: Vec<(LineType, &str)>) -> Diff {
        let mut old_counter = 1usize;
        let mut new_counter = 1usize;
        let diff_lines = lines
            .into_iter()
            .map(|(line_type, content)| {
                let (old_line_no, new_line_no) = match line_type {
                    LineType::Deletion => {
                        let num = old_counter;
                        old_counter += 1;
                        (Some(num), None)
                    }
                    LineType::Addition => {
                        let num = new_counter;
                        new_counter += 1;
                        (None, Some(num))
                    }
                    _ => {
                        let old = old_counter;
                        let new = new_counter;
                        old_counter += 1;
                        new_counter += 1;
                        (Some(old), Some(new))
                    }
                };

                crate::diff::DiffLine {
                    content: content.to_string(),
                    line_type,
                    old_line_no,
                    new_line_no,
                    inline_diff: None,
                }
            })
            .collect();

        Diff {
            file_path: "file".to_string(),
            context: DiffContext::WorkingTreeToIndex,
            hunks: vec![DiffHunk {
                header: HunkHeader {
                    raw: "@@ -1,1 +1,1 @@".to_string(),
                    old_start: 1,
                    old_lines: old_counter as u32,
                    new_start: 1,
                    new_lines: new_counter as u32,
                },
                lines: diff_lines,
                old_range: LineRange {
                    start: 1,
                    count: old_counter as u32,
                },
                new_range: LineRange {
                    start: 1,
                    count: new_counter as u32,
                },
                stageable: true,
                context_lines: 3,
            }],
            binary: false,
        }
    }

    #[test]
    fn addition_only_block_gets_inline_highlights() {
        let mut diff = build_diff(vec![
            (LineType::Addition, "new line one"),
            (LineType::Addition, "new line two"),
        ]);

        compute_inline_diffs(&mut diff);
        let hunk = &diff.hunks[0];

        for line in &hunk.lines {
            let inline = line.inline_diff.as_ref().unwrap();
            assert!(
                inline
                    .segments
                    .iter()
                    .all(|s| s.change == InlineChange::Added)
            );
        }
    }

    #[test]
    fn marks_unpaired_lines_against_empty_counterpart() {
        let mut diff = build_diff(vec![
            (LineType::Deletion, "old only line"),
            (LineType::Deletion, "another old line"),
            (LineType::Addition, "new only line"),
        ]);

        compute_inline_diffs(&mut diff);
        let hunk = &diff.hunks[0];

        let old_only = hunk.lines[0].inline_diff.as_ref().unwrap();
        assert!(
            old_only
                .segments
                .iter()
                .any(|s| s.change == InlineChange::Removed)
        );

        let another_old = hunk.lines[1].inline_diff.as_ref().unwrap();
        assert!(
            another_old
                .segments
                .iter()
                .any(|s| s.change == InlineChange::Removed)
        );

        let new_only = hunk.lines[2].inline_diff.as_ref().unwrap();
        assert!(
            new_only
                .segments
                .iter()
                .any(|s| s.change == InlineChange::Added)
        );
    }
}
