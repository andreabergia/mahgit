use git2::Oid;

/// A single commit entry in the log
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub oid: Oid,
    pub short_hash: String,
    pub summary: String,
    pub author_name: String,
    pub author_email: String,
    pub time: git2::Time,
}

/// State for the log view's data
#[derive(Debug)]
pub struct LogData {
    pub entries: Vec<LogEntry>,
    pub branch_name: String,
    pub has_more: bool,
    pub total_loaded: usize,
}

impl LogData {
    pub fn new(branch_name: String) -> Self {
        Self {
            entries: Vec::new(),
            branch_name,
            has_more: true,
            total_loaded: 0,
        }
    }
}
