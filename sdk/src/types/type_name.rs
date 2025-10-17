//! Ubiqutously used name wrapper type. Useful to have this defined globally
//! so that we don't have to redefine it in every module that uses it.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeName {
    pub name: String,
}

impl TypeName {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl std::fmt::Display for TypeName {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_name_deser_display() {
        let name: TypeName = serde_json::from_str(r#"{"name":"test"}"#).unwrap();

        assert_eq!(name.to_string(), "test");
    }

    #[test]
    fn test_type_name_new() {
        let name = TypeName::new("example");
        assert_eq!(name.name, "example");
    }

    #[test]
    fn test_type_name_equality() {
        let name1 = TypeName::new("same");
        let name2 = TypeName::new("same");
        let name3 = TypeName::new("different");
        assert_eq!(name1, name2);
        assert_ne!(name1, name3);
    }

    #[test]
    fn test_type_name_serialize() {
        let name = TypeName::new("serialize");
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, r#"{"name":"serialize"}"#);
    }

    #[test]
    fn test_type_name_display() {
        let name = TypeName::new("display");
        assert_eq!(format!("{}", name), "display");
    }
}
