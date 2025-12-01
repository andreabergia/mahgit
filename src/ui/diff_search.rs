#[derive(Debug, Clone, Default)]
pub struct DiffSearchFlags {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffSearchMode {
    #[default]
    Inactive,
    Editing,
    Viewing,
}

use crate::{
    config::expand_tabs_with_width,
    diff::{Diff, LineType},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Header,
    Line { line_index: usize },
    Separator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffSearchMatch {
    /// Index of the hunk the match belongs to
    pub hunk_index: usize,
    /// Line index within the hunk
    pub line_index: usize,
    /// Column offset within the line
    pub column: usize,
    /// Length of the match
    pub length: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffLineLocation {
    /// Index of the hunk this visual line maps to
    pub hunk_index: usize,
    pub kind: DiffLineKind,
}

impl Default for DiffLineLocation {
    fn default() -> Self {
        Self {
            hunk_index: 0,
            kind: DiffLineKind::Header,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiffSearchState {
    pub query: String,
    pub flags: DiffSearchFlags,
    pub matches: Vec<DiffSearchMatch>,
    pub active_match_index: Option<usize>,
    pub mode: DiffSearchMode,
    pub error: Option<String>,
    /// Indicates the query or flags changed and matches should be recomputed
    pub dirty: bool,
    /// Mapping from rendered lines to underlying diff positions
    pub line_locations: Vec<DiffLineLocation>,
}

impl Default for DiffSearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            flags: DiffSearchFlags::default(),
            matches: Vec::new(),
            active_match_index: None,
            mode: DiffSearchMode::Inactive,
            error: None,
            dirty: false,
            line_locations: Vec::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DiffSearchError {
    #[error("Invalid search pattern: {0}")]
    InvalidPattern(String),
}

#[derive(Debug, Clone)]
pub struct PreparedSearch {
    matcher: SearchMatcher,
}

#[derive(Debug, Clone)]
enum SearchMatcher {
    Literal {
        needle: String,
        case_sensitive: bool,
    },
    Regex(regex::Regex),
}

impl PreparedSearch {
    pub fn new(query: &str, flags: &DiffSearchFlags) -> Result<Self, DiffSearchError> {
        let matcher = if flags.regex || flags.whole_word {
            let pattern = build_regex_pattern(query, flags);
            let regex = regex::RegexBuilder::new(&pattern)
                .case_insensitive(!flags.case_sensitive)
                .build()
                .map_err(|e| DiffSearchError::InvalidPattern(e.to_string()))?;

            SearchMatcher::Regex(regex)
        } else {
            SearchMatcher::Literal {
                needle: query.to_string(),
                case_sensitive: flags.case_sensitive,
            }
        };

        Ok(Self { matcher })
    }

    pub fn find_in(&self, haystack: &str) -> Vec<(usize, usize)> {
        match &self.matcher {
            SearchMatcher::Literal {
                needle,
                case_sensitive,
            } => find_literal_matches(haystack, needle, *case_sensitive),
            SearchMatcher::Regex(regex) => regex
                .find_iter(haystack)
                .map(|m| {
                    let start = haystack[..m.start()].chars().count();
                    let len = m.as_str().chars().count();
                    (start, len)
                })
                .collect(),
        }
    }
}

fn build_regex_pattern(query: &str, flags: &DiffSearchFlags) -> String {
    if flags.regex {
        if flags.whole_word {
            format!(r"\b(?:{})\b", query)
        } else {
            query.to_string()
        }
    } else if flags.whole_word {
        format!(r"\b{}\b", regex::escape(query))
    } else {
        regex::escape(query)
    }
}

fn find_literal_matches(haystack: &str, needle: &str, case_sensitive: bool) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }

    if case_sensitive {
        haystack
            .match_indices(needle)
            .map(|(idx, m)| {
                let start = haystack[..idx].chars().count();
                let len = m.chars().count();
                (start, len)
            })
            .collect()
    } else {
        let needle_lower = needle.to_lowercase();
        let haystack_lower = haystack.to_lowercase();

        haystack_lower
            .match_indices(&needle_lower)
            .map(|(byte_idx, m)| {
                // Map byte offsets back to the original string to preserve correct column positions
                let start = haystack[..byte_idx].chars().count();
                let len = m.chars().count();
                (start, len)
            })
            .collect()
    }
}

pub fn find_matches_in_diff(
    diff: &Diff,
    query: &str,
    flags: &DiffSearchFlags,
    tab_width: usize,
) -> Result<Vec<DiffSearchMatch>, DiffSearchError> {
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let matcher = PreparedSearch::new(query, flags)?;
    let mut matches = Vec::new();

    for (hunk_index, hunk) in diff.hunks.iter().enumerate() {
        for (line_index, line) in hunk.lines.iter().enumerate() {
            let prefix = match line.line_type {
                LineType::Addition => "+",
                LineType::Deletion => "-",
                LineType::Context => " ",
                LineType::NoNewlineEOF => "\\",
            };
            let expanded = expand_tabs_with_width(&line.content, tab_width);
            let search_text = format!("{prefix}{expanded}");

            for (start, len) in matcher.find_in(&search_text) {
                matches.push(DiffSearchMatch {
                    hunk_index,
                    line_index,
                    column: start,
                    length: len,
                });
            }
        }
    }

    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::expand_tabs_with_width;
    use crate::diff::{Diff, DiffContext, DiffHunk, DiffLine, HunkHeader, LineRange, LineType};

    fn sample_diff() -> Diff {
        Diff {
            file_path: "file.txt".to_string(),
            context: DiffContext::WorkingTreeToIndex,
            binary: false,
            hunks: vec![DiffHunk {
                header: HunkHeader {
                    raw: "@@ -1,2 +1,2 @@".to_string(),
                    old_start: 1,
                    old_lines: 2,
                    new_start: 1,
                    new_lines: 2,
                },
                old_range: LineRange { start: 1, count: 2 },
                new_range: LineRange { start: 1, count: 2 },
                stageable: true,
                context_lines: 2,
                lines: vec![
                    DiffLine {
                        content: "Hello World".to_string(),
                        line_type: LineType::Context,
                        old_line_no: Some(1),
                        new_line_no: Some(1),
                        inline_diff: None,
                    },
                    DiffLine {
                        content: "hello\tplanet".to_string(),
                        line_type: LineType::Addition,
                        old_line_no: None,
                        new_line_no: Some(2),
                        inline_diff: None,
                    },
                ],
            }],
        }
    }

    #[test]
    fn literal_search_is_case_insensitive_by_default() {
        let diff = sample_diff();
        let flags = DiffSearchFlags::default();
        let matcher = PreparedSearch::new("hello", &flags).unwrap();

        let matches =
            diff.hunks
                .iter()
                .enumerate()
                .flat_map(|(hunk_idx, hunk)| {
                    let matcher = matcher.clone();
                    hunk.lines
                        .iter()
                        .enumerate()
                        .flat_map(move |(line_idx, line)| {
                            let matcher = matcher.clone();
                            let text = format!(
                                "{}{}",
                                match line.line_type {
                                    LineType::Addition => "+",
                                    LineType::Deletion => "-",
                                    LineType::Context => " ",
                                    LineType::NoNewlineEOF => "\\",
                                },
                                expand_tabs_with_width(&line.content, 4)
                            );
                            matcher.find_in(&text).into_iter().map(move |(col, len)| {
                                DiffSearchMatch {
                                    hunk_index: hunk_idx,
                                    line_index: line_idx,
                                    column: col,
                                    length: len,
                                }
                            })
                        })
                })
                .collect::<Vec<_>>();

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].column, 1); // Leading space prefix
        assert_eq!(matches[1].column, 1);
    }

    #[test]
    fn diff_search_uses_prefix_and_tab_width() {
        let diff = sample_diff();
        let flags = DiffSearchFlags::default();
        let matches = find_matches_in_diff(&diff, "planet", &flags, 4).unwrap();

        assert_eq!(matches.len(), 1);
        let m = &matches[0];
        assert_eq!(m.hunk_index, 0);
        assert_eq!(m.line_index, 1);
        // Leading "+" prefix and expanded tab contribute to offset
        assert_eq!(m.column, 9);
    }

    #[test]
    fn whole_word_wraps_pattern() {
        let flags = DiffSearchFlags {
            whole_word: true,
            ..Default::default()
        };
        let pattern = build_regex_pattern("foo", &flags);
        assert_eq!(pattern, r"\bfoo\b");
    }

    #[test]
    fn invalid_regex_returns_error() {
        let flags = DiffSearchFlags {
            regex: true,
            ..Default::default()
        };
        let err = PreparedSearch::new("[unclosed", &flags).unwrap_err();
        assert!(matches!(err, DiffSearchError::InvalidPattern(_)));
    }
}
