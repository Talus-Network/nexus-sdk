//! This module provides a location abstraction for both onchain and
//! offchain tools.

use {
    crate::sui,
    serde::{Deserialize, Serialize},
    std::str::FromStr,
};

/// Represents the location of a tool, either as an HTTP URL for offchain tools
/// or as a Sui module identifier for onchain tools.
///
/// # String Representations
///
/// - Offchain: `https://example.com/my-tool`
/// - Onchain: `0xabc123::my_module` (address::module format)
///
/// # Examples
///
/// ```
/// // Parse an offchain tool location.
/// let offchain: ToolLocation = "https://example.com/my-tool".parse().unwrap();
/// assert!(offchain.is_offchain());
///
/// // Parse an onchain tool location.
/// let onchain: ToolLocation = "0x1234::my_module".parse().unwrap();
/// assert!(onchain.is_onchain());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolLocation {
    /// HTTP(S) endpoint for offchain tools.
    Http(reqwest::Url),
    /// Sui module for onchain tools (package::module format).
    Sui {
        /// The package address containing the tool module.
        package: sui::types::Address,
        /// The module name within the package.
        module: sui::types::Identifier,
    },
}

impl ToolLocation {
    /// Creates a new offchain tool location from a URL.
    pub fn new_http(url: reqwest::Url) -> Self {
        Self::Http(url)
    }

    /// Creates a new onchain tool location from package address and module name.
    pub fn new_sui(package: &str, module: &str) -> Result<Self, anyhow::Error> {
        let package = package.parse()?;
        let module = sui::types::Identifier::new(module)?;
        Ok(Self::Sui { package, module })
    }

    /// Returns true if this is an offchain (HTTP) tool location.
    pub fn is_offchain(&self) -> bool {
        matches!(self, Self::Http(_))
    }

    /// Returns true if this is an onchain (Sui) tool location.
    pub fn is_onchain(&self) -> bool {
        matches!(self, Self::Sui { .. })
    }

    /// Returns the URL if this is an offchain tool location.
    pub fn url(&self) -> Result<&reqwest::Url, anyhow::Error> {
        match self {
            Self::Http(url) => Ok(url),
            Self::Sui { .. } => anyhow::bail!("URL is not available for onchain tools"),
        }
    }

    /// Returns the package address if this is an onchain tool location.
    pub fn package_address(&self) -> Result<sui::types::Address, anyhow::Error> {
        match self {
            Self::Http(_) => anyhow::bail!("Package address is not available for offchain tools"),
            Self::Sui { package, .. } => Ok(*package),
        }
    }

    /// Returns the module name if this is an onchain tool location.
    pub fn module_name(&self) -> Result<&sui::types::Identifier, anyhow::Error> {
        match self {
            Self::Http(_) => anyhow::bail!("Module name is not available for offchain tools"),
            Self::Sui { module, .. } => Ok(module),
        }
    }
}

impl FromStr for ToolLocation {
    type Err = anyhow::Error;

    /// Parses a string into a ToolLocation.
    ///
    /// The format is auto-detected:
    /// - If it starts with `http://` or `https://`, it's parsed as an HTTP URL.
    /// - Otherwise, it's parsed as a Sui module ID (`package_address::module_name`).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Check if it looks like an HTTP URL.
        if s.starts_with("http://") || s.starts_with("https://") {
            let url = reqwest::Url::parse(s)?;
            return Ok(Self::Http(url));
        }

        // Otherwise, try to parse as a Sui module ID (address::module).
        let parts: Vec<&str> = s.splitn(2, "::").collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid tool location format: expected 'address::module', got '{s}'");
        }

        let package = sui::types::Address::from_str(parts[0])
            .map_err(|e| anyhow::anyhow!("Invalid package address: {e}"))?;
        let module = sui::types::Identifier::from_str(parts[1])
            .map_err(|e| anyhow::anyhow!("Invalid module identifier: {e}"))?;

        Ok(Self::Sui { package, module })
    }
}

impl std::fmt::Display for ToolLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(url) => write!(f, "{}", url),
            Self::Sui { package, module } => write!(f, "{}::{}", package, module),
        }
    }
}

impl Serialize for ToolLocation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ToolLocation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let location = value
            .parse::<ToolLocation>()
            .map_err(serde::de::Error::custom)?;

        Ok(location)
    }
}

/// Conversion from reqwest::Url to ToolLocation.
impl From<reqwest::Url> for ToolLocation {
    fn from(url: reqwest::Url) -> Self {
        Self::Http(url)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, assert_matches::assert_matches};

    #[test]
    fn test_parse_http_url() {
        let location: ToolLocation = "https://example.com/my-tool".parse().unwrap();

        assert!(location.is_offchain());
        assert!(!location.is_onchain());
        assert_eq!(
            location.url().unwrap().as_str(),
            "https://example.com/my-tool"
        );
        assert!(location.package_address().is_err());
        assert!(location.module_name().is_err());
    }

    #[test]
    fn test_parse_http_url_with_port() {
        let location: ToolLocation = "http://localhost:8080/tool".parse().unwrap();

        assert!(location.is_offchain());
        assert_eq!(
            location.url().unwrap().as_str(),
            "http://localhost:8080/tool"
        );
    }

    #[test]
    fn test_parse_sui_module_id() {
        let location: ToolLocation =
            "0x0000000000000000000000000000000000000000000000000000000000001234::my_module"
                .parse()
                .unwrap();

        assert!(location.is_onchain());
        assert!(!location.is_offchain());
        assert!(location.url().is_err());
        assert!(location.package_address().is_ok());
        assert_eq!(
            location.module_name().unwrap(),
            &sui::types::Identifier::from_static("my_module")
        );
    }

    #[test]
    fn test_parse_short_sui_address() {
        // Short addresses are expanded by the Sui SDK.
        let location: ToolLocation = "0x1::module".parse().unwrap();

        assert!(location.is_onchain());
        assert_eq!(
            location.module_name().unwrap(),
            &sui::types::Identifier::from_static("module")
        );
    }

    #[test]
    fn test_display_http() {
        let location = ToolLocation::Http("https://example.com/tool".parse().unwrap());

        assert_eq!(location.to_string(), "https://example.com/tool");
    }

    #[test]
    fn test_display_sui() {
        let addr = sui::types::Address::from_str(
            "0x0000000000000000000000000000000000000000000000000000000000001234",
        )
        .unwrap();
        let location = ToolLocation::Sui {
            package: addr,
            module: sui::types::Identifier::from_static("my_module"),
        };

        assert_eq!(
            location.to_string(),
            "0x0000000000000000000000000000000000000000000000000000000000001234::my_module"
        );
    }

    #[test]
    fn test_serialize_deserialize_http() {
        let location = ToolLocation::Http("https://example.com/tool".parse().unwrap());

        let serialized = serde_json::to_string(&location).unwrap();
        assert_eq!(serialized, "\"https://example.com/tool\"");

        let deserialized: ToolLocation = serde_json::from_str(&serialized).unwrap();
        assert_eq!(location, deserialized);
    }

    #[test]
    fn test_serialize_deserialize_sui() {
        let addr = sui::types::Address::from_str(
            "0x0000000000000000000000000000000000000000000000000000000000001234",
        )
        .unwrap();
        let location = ToolLocation::Sui {
            package: addr,
            module: sui::types::Identifier::from_static("my_module"),
        };

        let serialized = serde_json::to_string(&location).unwrap();
        let deserialized: ToolLocation = serde_json::from_str(&serialized).unwrap();
        assert_eq!(location, deserialized);
    }

    #[test]
    fn test_invalid_location() {
        let result = "not_a_valid_location".parse::<ToolLocation>();

        assert_matches!(result, Err(e) if e.to_string().contains("Invalid tool location format"));
    }

    #[test]
    fn test_from_url() {
        let url: reqwest::Url = "https://example.com/tool".parse().unwrap();
        let location = ToolLocation::from(url.clone());

        assert!(location.is_offchain());
        assert_eq!(location.url().unwrap(), &url);
    }

    #[test]
    fn test_new_constructors() {
        let http_location = ToolLocation::new_http("https://example.com/tool".parse().unwrap());
        assert!(http_location.is_offchain());

        let sui_location = ToolLocation::new_sui(
            "0x0000000000000000000000000000000000000000000000000000000000001234",
            "my_module",
        )
        .unwrap();
        assert!(sui_location.is_onchain());
        assert_eq!(
            sui_location.module_name().unwrap(),
            &sui::types::Identifier::from_static("my_module")
        );
    }

    #[test]
    fn test_new_sui_success() {
        let location = ToolLocation::new_sui(
            "0x0000000000000000000000000000000000000000000000000000000000001234",
            "my_module",
        )
        .unwrap();

        assert!(location.is_onchain());
        assert_eq!(
            location.package_address().unwrap().to_string(),
            "0x0000000000000000000000000000000000000000000000000000000000001234"
        );
        assert_eq!(
            location.module_name().unwrap(),
            &sui::types::Identifier::from_static("my_module")
        );
    }

    #[test]
    fn test_new_sui_invalid_address() {
        let result = ToolLocation::new_sui("invalid_address", "my_module");
        assert!(result.is_err());
    }

    #[test]
    fn test_new_sui_invalid_module() {
        let result = ToolLocation::new_sui(
            "0x0000000000000000000000000000000000000000000000000000000000001234",
            "invalid-module-name",
        );
        assert!(result.is_err());
    }
}
