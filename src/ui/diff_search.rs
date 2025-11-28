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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiffLineLocation {
    /// Index of the hunk this visual line maps to
    pub hunk_index: usize,
    /// Line index within the hunk (excluding header)
    pub line_index: usize,
}

#[derive(Debug, Clone)]
pub struct DiffSearchState {
    pub query: String,
    pub flags: DiffSearchFlags,
    pub matches: Vec<DiffSearchMatch>,
    pub active_match_index: Option<usize>,
    pub mode: DiffSearchMode,
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
            line_locations: Vec::new(),
        }
    }
}
