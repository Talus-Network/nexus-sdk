//! This module provides a reference abstraction for both onchain and
//! offchain tools.

use {
    crate::sui,
    serde::{Deserialize, Serialize},
    std::str::FromStr,
};

/// Represents the reference of a tool, either as an HTTP URL for offchain tools
/// or as a Sui module identifier for onchain tools.
///
/// # String Representations
///
/// - Offchain: `https://example.com/my-tool`
/// - Onchain: `0xabc123::my_module@0xwitness` (address::module@witness format)
///
/// # Examples
///
/// ```
/// // Parse an offchain tool reference.
/// let offchain: ToolRef = "https://example.com/my-tool".parse().unwrap();
/// assert!(offchain.is_offchain());
///
/// // Parse an onchain tool reference.
/// let onchain: ToolRef = "0x1234::my_module@0x5678".parse().unwrap();
/// assert!(onchain.is_onchain());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolRef {
    /// HTTP(S) endpoint for offchain tools.
    Http(reqwest::Url),
    /// Sui module for onchain tools (package::module@witness format).
    Sui {
        /// The package address containing the tool module.
        package: sui::types::Address,
        /// The module name within the package.
        module: sui::types::Identifier,
        /// The witness ID for the onchain tool.
        witness_id: sui::types::Address,
    },
}

impl ToolRef {
    /// Creates a new offchain tool reference from a URL.
    pub fn new_http(url: reqwest::Url) -> Self {
        Self::Http(url)
    }

    /// Creates a new onchain tool reference from package address, module name, and witness ID.
    pub fn new_sui(package: &str, module: &str, witness_id: &str) -> Result<Self, anyhow::Error> {
        let package = package.parse()?;
        let module = sui::types::Identifier::new(module)?;
        let witness_id = witness_id.parse()?;
        Ok(Self::Sui {
            package,
            module,
            witness_id,
        })
    }

    /// Returns true if this is an offchain (HTTP) tool reference.
    pub fn is_offchain(&self) -> bool {
        matches!(self, Self::Http(_))
    }

    /// Returns true if this is an onchain (Sui) tool reference.
    pub fn is_onchain(&self) -> bool {
        matches!(self, Self::Sui { .. })
    }

    /// Returns the URL if this is an offchain tool reference.
    pub fn url(&self) -> Result<&reqwest::Url, anyhow::Error> {
        match self {
            Self::Http(url) => Ok(url),
            Self::Sui { .. } => anyhow::bail!("URL is not available for onchain tools"),
        }
    }

    /// Returns the package address if this is an onchain tool reference.
    pub fn package_address(&self) -> Result<sui::types::Address, anyhow::Error> {
        match self {
            Self::Http(_) => anyhow::bail!("Package address is not available for offchain tools"),
            Self::Sui { package, .. } => Ok(*package),
        }
    }

    /// Returns the module name if this is an onchain tool reference.
    pub fn module_name(&self) -> Result<&sui::types::Identifier, anyhow::Error> {
        match self {
            Self::Http(_) => anyhow::bail!("Module name is not available for offchain tools"),
            Self::Sui { module, .. } => Ok(module),
        }
    }

    /// Returns the witness ID if this is an onchain tool reference.
    pub fn witness_id(&self) -> Result<sui::types::Address, anyhow::Error> {
        match self {
            Self::Http(_) => anyhow::bail!("Witness ID is not available for offchain tools"),
            Self::Sui { witness_id, .. } => Ok(*witness_id),
        }
    }
}

impl FromStr for ToolRef {
    type Err = anyhow::Error;

    /// Parses a string into a ToolRef.
    ///
    /// The format is auto-detected:
    /// - If it starts with `http://` or `https://`, it's parsed as an HTTP URL.
    /// - Otherwise, it's parsed as a Sui module ID (`package_address::module_name@witness_id`).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Check if it looks like an HTTP URL.
        if s.starts_with("http://") || s.starts_with("https://") {
            let url = reqwest::Url::parse(s)?;
            return Ok(Self::Http(url));
        }

        // Otherwise, try to parse as a Sui module ID (address::module@witness).
        let parts: Vec<&str> = s.splitn(2, "::").collect();
        if parts.len() != 2 {
            anyhow::bail!(
                "Invalid tool reference format: expected 'address::module@witness', got '{s}'"
            );
        }

        let package = sui::types::Address::from_str(parts[0])
            .map_err(|e| anyhow::anyhow!("Invalid package address: {e}"))?;

        // Split the module part to extract module name and witness ID.
        let module_witness: Vec<&str> = parts[1].splitn(2, '@').collect();
        if module_witness.len() != 2 {
            anyhow::bail!(
                "Invalid tool reference format: expected 'address::module@witness', got '{s}'"
            );
        }

        let module = sui::types::Identifier::from_str(module_witness[0])
            .map_err(|e| anyhow::anyhow!("Invalid module identifier: {e}"))?;
        let witness_id = sui::types::Address::from_str(module_witness[1])
            .map_err(|e| anyhow::anyhow!("Invalid witness ID: {e}"))?;

        Ok(Self::Sui {
            package,
            module,
            witness_id,
        })
    }
}

impl std::fmt::Display for ToolRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(url) => write!(f, "{}", url),
            Self::Sui {
                package,
                module,
                witness_id,
            } => write!(f, "{}::{}@{}", package, module, witness_id),
        }
    }
}

impl Serialize for ToolRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ToolRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let reference = value.parse::<ToolRef>().map_err(serde::de::Error::custom)?;

        Ok(reference)
    }
}

/// Conversion from reqwest::Url to ToolRef.
impl From<reqwest::Url> for ToolRef {
    fn from(url: reqwest::Url) -> Self {
        Self::Http(url)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, assert_matches::assert_matches};

    #[test]
    fn test_parse_http_url() {
        let reference: ToolRef = "https://example.com/my-tool".parse().unwrap();

        assert!(reference.is_offchain());
        assert!(!reference.is_onchain());
        assert_eq!(
            reference.url().unwrap().as_str(),
            "https://example.com/my-tool"
        );
        assert!(reference.package_address().is_err());
        assert!(reference.module_name().is_err());
        assert!(reference.witness_id().is_err());
    }

    #[test]
    fn test_parse_http_url_with_port() {
        let reference: ToolRef = "http://localhost:8080/tool".parse().unwrap();

        assert!(reference.is_offchain());
        assert_eq!(
            reference.url().unwrap().as_str(),
            "http://localhost:8080/tool"
        );
    }

    #[test]
    fn test_parse_sui_module_id() {
        let reference: ToolRef =
            "0x0000000000000000000000000000000000000000000000000000000000001234::my_module@0x0000000000000000000000000000000000000000000000000000000000005678"
                .parse()
                .unwrap();

        assert!(reference.is_onchain());
        assert!(!reference.is_offchain());
        assert!(reference.url().is_err());
        assert!(reference.package_address().is_ok());
        assert_eq!(
            reference.module_name().unwrap(),
            &sui::types::Identifier::from_static("my_module")
        );
        assert_eq!(
            reference.witness_id().unwrap().to_string(),
            "0x0000000000000000000000000000000000000000000000000000000000005678"
        );
    }

    #[test]
    fn test_parse_short_sui_address() {
        // Short addresses are expanded by the Sui SDK.
        let reference: ToolRef = "0x1::module@0x2".parse().unwrap();

        assert!(reference.is_onchain());
        assert_eq!(
            reference.module_name().unwrap(),
            &sui::types::Identifier::from_static("module")
        );
        assert!(reference.witness_id().is_ok());
    }

    #[test]
    fn test_display_http() {
        let reference = ToolRef::Http("https://example.com/tool".parse().unwrap());

        assert_eq!(reference.to_string(), "https://example.com/tool");
    }

    #[test]
    fn test_display_sui() {
        let package = sui::types::Address::from_str(
            "0x0000000000000000000000000000000000000000000000000000000000001234",
        )
        .unwrap();
        let witness_id = sui::types::Address::from_str(
            "0x0000000000000000000000000000000000000000000000000000000000005678",
        )
        .unwrap();
        let reference = ToolRef::Sui {
            package,
            module: sui::types::Identifier::from_static("my_module"),
            witness_id,
        };

        assert_eq!(
            reference.to_string(),
            "0x0000000000000000000000000000000000000000000000000000000000001234::my_module@0x0000000000000000000000000000000000000000000000000000000000005678"
        );
    }

    #[test]
    fn test_serialize_deserialize_http() {
        let reference = ToolRef::Http("https://example.com/tool".parse().unwrap());

        let serialized = serde_json::to_string(&reference).unwrap();
        assert_eq!(serialized, "\"https://example.com/tool\"");

        let deserialized: ToolRef = serde_json::from_str(&serialized).unwrap();
        assert_eq!(reference, deserialized);
    }

    #[test]
    fn test_serialize_deserialize_sui() {
        let package = sui::types::Address::from_str(
            "0x0000000000000000000000000000000000000000000000000000000000001234",
        )
        .unwrap();
        let witness_id = sui::types::Address::from_str(
            "0x0000000000000000000000000000000000000000000000000000000000005678",
        )
        .unwrap();
        let reference = ToolRef::Sui {
            package,
            module: sui::types::Identifier::from_static("my_module"),
            witness_id,
        };

        let serialized = serde_json::to_string(&reference).unwrap();
        let deserialized: ToolRef = serde_json::from_str(&serialized).unwrap();
        assert_eq!(reference, deserialized);
    }

    #[test]
    fn test_invalid_reference() {
        let result = "not_a_valid_reference".parse::<ToolRef>();

        assert_matches!(result, Err(e) if e.to_string().contains("Invalid tool reference format"));
    }

    #[test]
    fn test_from_url() {
        let url: reqwest::Url = "https://example.com/tool".parse().unwrap();
        let reference = ToolRef::from(url.clone());

        assert!(reference.is_offchain());
        assert_eq!(reference.url().unwrap(), &url);
    }

    #[test]
    fn test_new_constructors() {
        let http_reference = ToolRef::new_http("https://example.com/tool".parse().unwrap());
        assert!(http_reference.is_offchain());

        let sui_reference = ToolRef::new_sui(
            "0x0000000000000000000000000000000000000000000000000000000000001234",
            "my_module",
            "0x0000000000000000000000000000000000000000000000000000000000005678",
        )
        .unwrap();
        assert!(sui_reference.is_onchain());
        assert_eq!(
            sui_reference.module_name().unwrap(),
            &sui::types::Identifier::from_static("my_module")
        );
        assert!(sui_reference.witness_id().is_ok());
    }

    #[test]
    fn test_new_sui_success() {
        let reference = ToolRef::new_sui(
            "0x0000000000000000000000000000000000000000000000000000000000001234",
            "my_module",
            "0x0000000000000000000000000000000000000000000000000000000000005678",
        )
        .unwrap();

        assert!(reference.is_onchain());
        assert_eq!(
            reference.package_address().unwrap().to_string(),
            "0x0000000000000000000000000000000000000000000000000000000000001234"
        );
        assert_eq!(
            reference.module_name().unwrap(),
            &sui::types::Identifier::from_static("my_module")
        );
        assert_eq!(
            reference.witness_id().unwrap().to_string(),
            "0x0000000000000000000000000000000000000000000000000000000000005678"
        );
    }

    #[test]
    fn test_new_sui_invalid_address() {
        let result = ToolRef::new_sui(
            "invalid_address",
            "my_module",
            "0x0000000000000000000000000000000000000000000000000000000000005678",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_new_sui_invalid_module() {
        let result = ToolRef::new_sui(
            "0x0000000000000000000000000000000000000000000000000000000000001234",
            "invalid-module-name",
            "0x0000000000000000000000000000000000000000000000000000000000005678",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_new_sui_invalid_witness() {
        let result = ToolRef::new_sui(
            "0x0000000000000000000000000000000000000000000000000000000000001234",
            "my_module",
            "invalid_witness",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_sui_missing_witness() {
        // Should fail because witness_id is missing.
        let result =
            "0x0000000000000000000000000000000000000000000000000000000000001234::my_module"
                .parse::<ToolRef>();
        assert_matches!(result, Err(e) if e.to_string().contains("Invalid tool reference format"));
    }
}
