//! String wrapper for type safety around our secret strings. Also implements
//! custom display that avoids leaking the secret in logs.

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretValue(String);

impl From<String> for SecretValue {
    fn from(s: String) -> Self {
        SecretValue(s)
    }
}

impl From<&str> for SecretValue {
    fn from(s: &str) -> Self {
        SecretValue(s.to_string())
    }
}

impl SecretValue {
    pub fn peek(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecretValue([Redacted: *****])",)
    }
}

impl std::fmt::Display for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[Redacted: *****]")
    }
}

#[cfg(test)]
mod tests {
    use super::SecretValue;

    #[test]
    fn test_secret_value_from_string_and_str() {
        let s = "supersecret";
        let sv1 = SecretValue::from(s.to_string());
        let sv2 = SecretValue::from(s);

        assert_eq!(sv1.peek(), s);
        assert_eq!(sv2.peek(), s);
        assert_eq!(sv1, sv2);
    }

    #[test]
    fn test_secret_value_debug_and_display_redacted() {
        let secret = "averylongsupersecretvalue";
        let sv = SecretValue::from(secret);
        let debug_str = format!("{sv:?}");
        let display_str = format!("{sv}");

        assert_eq!(debug_str, "SecretValue([Redacted: *****])");
        assert_eq!(display_str, "[Redacted: *****]");
    }

    #[test]
    fn test_secret_value_debug_and_display_short_secret() {
        let secret = "tiny";
        let sv = SecretValue::from(secret);
        let debug_str = format!("{sv:?}");
        let display_str = format!("{sv}");

        assert_eq!(debug_str, "SecretValue([Redacted: *****])");
        assert_eq!(display_str, "[Redacted: *****]");
    }
}
