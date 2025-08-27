use crate::operations::OperationResult;
use std::time::{Duration, Instant};

pub struct FeedbackManager {
    current_message: Option<OperationResult>,
    display_until: Option<Instant>,
}

impl FeedbackManager {
    pub fn new() -> Self {
        Self {
            current_message: None,
            display_until: None,
        }
    }

    pub fn show_result(&mut self, result: OperationResult) {
        self.current_message = Some(result);
        self.display_until = Some(Instant::now() + Duration::from_secs(3));
    }

    pub fn get_current_message(&self) -> Option<&OperationResult> {
        if let Some(until) = self.display_until
            && Instant::now() < until
        {
            return self.current_message.as_ref();
        }
        None
    }

    pub fn clear_message(&mut self) {
        self.current_message = None;
        self.display_until = None;
    }

    pub fn has_active_message(&self) -> bool {
        self.get_current_message().is_some()
    }
}

impl Default for FeedbackManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_manager_new() {
        let manager = FeedbackManager::new();
        assert!(manager.get_current_message().is_none());
        assert!(!manager.has_active_message());
    }

    #[test]
    fn test_show_result() {
        let mut manager = FeedbackManager::new();
        let result = OperationResult::new("Test message".to_string());

        manager.show_result(result);

        assert!(manager.has_active_message());
        let message = manager.get_current_message().unwrap();
        assert_eq!(message.message, "Test message");
    }

    #[test]
    fn test_message_timeout() {
        let mut manager = FeedbackManager::new();
        let result = OperationResult::new("Test message".to_string());

        manager.show_result(result);
        assert!(manager.has_active_message());

        // Manually expire the message by setting display_until to the past
        manager.display_until = Some(Instant::now() - Duration::from_secs(1));

        assert!(!manager.has_active_message());
        assert!(manager.get_current_message().is_none());
    }

    #[test]
    fn test_clear_message() {
        let mut manager = FeedbackManager::new();
        let result = OperationResult::new("Test message".to_string());

        manager.show_result(result);
        assert!(manager.has_active_message());

        manager.clear_message();
        assert!(!manager.has_active_message());
        assert!(manager.get_current_message().is_none());
    }

    #[test]
    fn test_message_replacement() {
        let mut manager = FeedbackManager::new();
        let result1 = OperationResult::new("First message".to_string());
        let result2 = OperationResult::new("Second message".to_string());

        manager.show_result(result1);
        assert_eq!(
            manager.get_current_message().unwrap().message,
            "First message"
        );

        manager.show_result(result2);
        assert_eq!(
            manager.get_current_message().unwrap().message,
            "Second message"
        );
    }
}
