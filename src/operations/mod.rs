pub mod staging;
pub use staging::*;

#[derive(Debug, Clone)]
pub struct OperationResult {
    pub message: String,
}

impl OperationResult {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl std::fmt::Display for OperationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "✓ {}", self.message)
    }
}
