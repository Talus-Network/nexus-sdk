//! Defines the `StorageKind` enum which specifies the storage method for data in
//! the DAG.

use {
    serde::{Deserialize, Serialize},
    std::str::FromStr,
};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageKind {
    Inline,
    Walrus,
}

impl FromStr for StorageKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "inline" => Ok(StorageKind::Inline),
            "walrus" => Ok(StorageKind::Walrus),
            _ => Err(format!("Invalid storage kind: {}", s)),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str_inline_lowercase() {
        assert_eq!(StorageKind::from_str("inline"), Ok(StorageKind::Inline));
    }

    #[test]
    fn test_from_str_inline_uppercase() {
        assert_eq!(StorageKind::from_str("INLINE"), Ok(StorageKind::Inline));
    }

    #[test]
    fn test_from_str_inline_mixedcase() {
        assert_eq!(StorageKind::from_str("InLiNe"), Ok(StorageKind::Inline));
    }

    #[test]
    fn test_from_str_walrus_lowercase() {
        assert_eq!(StorageKind::from_str("walrus"), Ok(StorageKind::Walrus));
    }

    #[test]
    fn test_from_str_walrus_uppercase() {
        assert_eq!(StorageKind::from_str("WALRUS"), Ok(StorageKind::Walrus));
    }

    #[test]
    fn test_from_str_walrus_mixedcase() {
        assert_eq!(StorageKind::from_str("WaLrUs"), Ok(StorageKind::Walrus));
    }

    #[test]
    fn test_from_str_invalid() {
        let err = StorageKind::from_str("unknown").unwrap_err();
        assert_eq!(err, "Invalid storage kind: unknown");
    }

    #[test]
    fn test_from_str_empty() {
        let err = StorageKind::from_str("").unwrap_err();
        assert_eq!(err, "Invalid storage kind: ");
    }

    #[test]
    fn test_from_str_whitespace() {
        let err = StorageKind::from_str(" walrus ").unwrap_err();
        assert_eq!(err, "Invalid storage kind:  walrus ");
    }
}
