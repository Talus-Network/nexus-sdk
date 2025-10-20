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

impl From<&str> for TypeName {
    fn from(name: &str) -> Self {
        TypeName::new(name)
    }
}

impl From<String> for TypeName {
    fn from(name: String) -> Self {
        TypeName { name }
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

    #[test]
    fn test_type_name_from_str() {
        let name: TypeName = TypeName::from("from_str");
        assert_eq!(name.name, "from_str");
        assert_eq!(name, TypeName::new("from_str"));
    }

    #[test]
    fn test_type_name_from_str_trait() {
        let name: TypeName = "trait_from_str".into();
        assert_eq!(name.name, "trait_from_str");
    }

    #[test]
    fn test_type_name_from_empty_str() {
        let name: TypeName = "".into();
        assert_eq!(name.name, "");
    }

    #[test]
    fn test_type_name_from_string_type() {
        let s = String::from("from_string_type");
        let name: TypeName = TypeName::from(s.clone());
        assert_eq!(name.name, "from_string_type");
        // Ensure original string is not moved
        assert_eq!(s, "from_string_type");
    }

    #[test]
    fn test_type_name_from_string_into_trait() {
        let s = String::from("into_trait_string");
        let name: TypeName = s.clone().into();
        assert_eq!(name.name, "into_trait_string");
    }

    #[test]
    fn test_type_name_from_str_and_string_equality() {
        let name_from_str: TypeName = "same_name".into();
        let name_from_string: TypeName = String::from("same_name").into();
        assert_eq!(name_from_str, name_from_string);
    }
}
