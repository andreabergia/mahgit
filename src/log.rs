use git2::Oid;

/// A single commit entry in the log
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub oid: Oid,
    pub short_hash: String,
    pub summary: String,
    /// Full commit message (body included), not just the summary line.
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    /// Author time.
    pub time: git2::Time,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_time: git2::Time,
}

impl LogEntry {
    /// True when the committer differs from the author (name, email, or time),
    /// as happens for rebased / cherry-picked / amended commits.
    pub fn has_distinct_committer(&self) -> bool {
        self.committer_name != self.author_name
            || self.committer_email != self.author_email
            || self.committer_time != self.time
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> LogEntry {
        LogEntry {
            oid: Oid::zero(),
            short_hash: "0000000".to_string(),
            summary: "commit".to_string(),
            message: "commit".to_string(),
            author_name: "Alice".to_string(),
            author_email: "alice@example.com".to_string(),
            time: git2::Time::new(1000, 0),
            committer_name: "Alice".to_string(),
            committer_email: "alice@example.com".to_string(),
            committer_time: git2::Time::new(1000, 0),
        }
    }

    #[test]
    fn has_distinct_committer_false_when_identical() {
        assert!(!entry().has_distinct_committer());
    }

    #[test]
    fn has_distinct_committer_true_when_name_differs() {
        let mut e = entry();
        e.committer_name = "Bob".to_string();
        assert!(e.has_distinct_committer());
    }

    #[test]
    fn has_distinct_committer_true_when_email_differs() {
        let mut e = entry();
        e.committer_email = "bob@example.com".to_string();
        assert!(e.has_distinct_committer());
    }

    #[test]
    fn has_distinct_committer_true_when_time_differs() {
        let mut e = entry();
        e.committer_time = git2::Time::new(2000, 0);
        assert!(e.has_distinct_committer());
    }
}
