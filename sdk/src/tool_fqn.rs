use {
    lazy_regex::{lazy_regex, regex, Lazy},
    regex::Regex,
    serde::{Deserialize, Serialize},
    std::str::FromStr,
};

/// Struct that holds a structured tool FQN. This FQN consists of the tool
/// creator domain, the tool name and the tool version.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ToolFqn {
    domain: String,
    name: String,
    version: u32,
}

impl ToolFqn {
    /// Returns the tool creator domain.
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Returns the tool name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the tool version.
    pub fn version(&self) -> u32 {
        self.version
    }
}

/// Helper macro that creates a Tool FQN from a string literal. This DOES NOT
/// perform a compile-time check on the FQN format.
///
/// # Examples
///
/// ```
/// use nexus_sdk::{fqn, ToolFqn};
///
/// let fqn = fqn!("xyz.taluslabs.example@1");
///
/// assert_eq!(fqn.domain(), "xyz.taluslabs");
/// assert_eq!(fqn.name(), "example");
/// assert_eq!(fqn.version(), 1);
/// ```
///
/// ```should_panic
/// use nexus_sdk::{fqn, ToolFqn};
///
/// let _ = fqn!("xyz.taluslabs.example@");
/// ```
#[macro_export]
macro_rules! fqn {
    ($fqn:expr) => {{
        ($fqn as &'static str).parse::<ToolFqn>().unwrap()
    }};
}

/// Regex to match all the rules highlighted in the [FromStr] implementation.
static FQN_REGEX: Lazy<Regex> = lazy_regex!(
    r"(?x)                                               # Enable verbose mode
    ^                                                    # Start of string
    (?P<domain>[a-z][a-z0-9_-]+(?:\.[a-z][a-z0-9_-]+)+)+ # Tool domain
    \.                                                   # '.' literal
    (?P<name>[a-z][a-z0-9_-]+)                           # Tool name
    @                                                    # '@' literal
    (?P<version>[0-9]+)                                  # Tool version
    $                                                    # End of string
    "
);

impl FromStr for ToolFqn {
    type Err = anyhow::Error;

    /// This [FromStr] implementation expects a string with the following
    /// format:
    ///
    /// `xyz.taluslabs.example@1`
    ///
    /// Where:
    /// - `xyz.taluslabs` is the tool creator domain
    /// - `example` is the tool name
    /// - `1` is the tool version
    ///
    /// Constraints:
    /// 1. Splitting by `.` must yield at least 3 parts.
    /// 2. Each part must satisfy the `[a-z0-9_-]{2,}` regex.
    /// 3. Each part must not start with a digit, an underscore or a hyphen.
    /// 4. First N-1 parts when joined by `.` make the domain.
    /// 5. N-th part is the tool name and its version separated by `@`.
    /// 6. The version must be a positive u32 integer.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some((domain, name, version)) = FQN_REGEX.captures(s).map(|captures| {
            (
                captures["domain"].to_string(),
                captures["name"].to_string(),
                captures["version"].to_string(),
            )
        }) else {
            anyhow::bail!("Invalid tool FQN format");
        };

        // This only fails if u32 overflows as the format is already validated.
        // If this happens, kudos to the tool devs.
        let Ok(version) = version.parse::<u32>() else {
            anyhow::bail!("Tool version too large");
        };

        Ok(Self {
            domain,
            name,
            version,
        })
    }
}

impl std::fmt::Display for ToolFqn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}@{}", self.domain, self.name, self.version)
    }
}

impl Serialize for ToolFqn {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ToolFqn {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let fqn = value.parse::<ToolFqn>().map_err(serde::de::Error::custom)?;

        Ok(fqn)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, assert_matches::assert_matches};

    #[test]
    fn test_tool_fqn_from_str() {
        let ok_fqn = "xyz.taluslabs.example@1".parse::<ToolFqn>().unwrap();

        assert_eq!(ok_fqn.domain, "xyz.taluslabs");
        assert_eq!(ok_fqn.name, "example");
        assert_eq!(ok_fqn.version, 1);

        assert_eq!(ok_fqn.to_string(), "xyz.taluslabs.example@1");

        let ok_with_special = "xyz123.talus_labs.example-1@1".parse::<ToolFqn>().unwrap();

        assert_eq!(ok_with_special.domain, "xyz123.talus_labs");
        assert_eq!(ok_with_special.name, "example-1");
        assert_eq!(ok_with_special.version, 1);

        assert_eq!(ok_with_special.to_string(), "xyz123.talus_labs.example-1@1");

        let ok_long_domain = "xyz.talus.labs.tool.llm.example@1"
            .parse::<ToolFqn>()
            .unwrap();

        assert_eq!(ok_long_domain.domain, "xyz.talus.labs.tool.llm");
        assert_eq!(ok_long_domain.name, "example");
        assert_eq!(ok_long_domain.version, 1);

        assert_eq!(
            ok_long_domain.to_string(),
            "xyz.talus.labs.tool.llm.example@1"
        );

        let missing_version = "xyz.taluslabs.example@".parse::<ToolFqn>();

        assert_matches!(missing_version, Err(e) if e.to_string().contains("Invalid tool FQN format"));

        let short_domain1 = "xyz.tool@1".parse::<ToolFqn>();
        let short_domain2 = "x.taluslabs.tool@1".parse::<ToolFqn>();

        assert_matches!(short_domain1, Err(e) if e.to_string().contains("Invalid tool FQN format"));
        assert_matches!(short_domain2, Err(e) if e.to_string().contains("Invalid tool FQN format"));

        let short_name = "xyz.taluslabs.t@1".parse::<ToolFqn>();

        assert_matches!(short_name, Err(e) if e.to_string().contains("Invalid tool FQN format"));

        let starts_with_digit1 = "xyz.taluslabs.1tool@1".parse::<ToolFqn>();
        let starts_with_digit2 = "1xyz.taluslabs.tool@1".parse::<ToolFqn>();
        let starts_with_digit3 = "xyz.1taluslabs.example@1".parse::<ToolFqn>();

        assert_matches!(starts_with_digit1, Err(e) if e.to_string().contains("Invalid tool FQN format"));
        assert_matches!(starts_with_digit2, Err(e) if e.to_string().contains("Invalid tool FQN format"));
        assert_matches!(starts_with_digit3, Err(e) if e.to_string().contains("Invalid tool FQN format"));

        let starts_with_underscore1 = "xyz.taluslabs._tool@1".parse::<ToolFqn>();
        let starts_with_underscore2 = "_xyz.taluslabs.tool@1".parse::<ToolFqn>();
        let starts_with_underscore3 = "xyz._taluslabs.example@1".parse::<ToolFqn>();

        assert_matches!(starts_with_underscore1, Err(e) if e.to_string().contains("Invalid tool FQN format"));
        assert_matches!(starts_with_underscore2, Err(e) if e.to_string().contains("Invalid tool FQN format"));
        assert_matches!(starts_with_underscore3, Err(e) if e.to_string().contains("Invalid tool FQN format"));

        let starts_with_hyphen1 = "xyz.taluslabs.-tool@1".parse::<ToolFqn>();
        let starts_with_hyphen2 = "-xyz.taluslabs.tool@1".parse::<ToolFqn>();
        let starts_with_hyphen3 = "xyz.-taluslabs.example@1".parse::<ToolFqn>();

        assert_matches!(starts_with_hyphen1, Err(e) if e.to_string().contains("Invalid tool FQN format"));
        assert_matches!(starts_with_hyphen2, Err(e) if e.to_string().contains("Invalid tool FQN format"));
        assert_matches!(starts_with_hyphen3, Err(e) if e.to_string().contains("Invalid tool FQN format"));

        let invalid_version1 = "xyz.taluslabs.example@a".parse::<ToolFqn>();
        let invalid_version2 = "xyz.taluslabs.example@-1".parse::<ToolFqn>();
        let invalid_version3 = "xyz.taluslabs.example@1.1".parse::<ToolFqn>();

        assert_matches!(invalid_version1, Err(e) if e.to_string().contains("Invalid tool FQN format"));
        assert_matches!(invalid_version2, Err(e) if e.to_string().contains("Invalid tool FQN format"));
        assert_matches!(invalid_version3, Err(e) if e.to_string().contains("Invalid tool FQN format"));

        let invalid_characters1 = "xyz.ta!u$labs.example@1".parse::<ToolFqn>();
        let invalid_characters2 = "XYZ.taluslabs.example@1".parse::<ToolFqn>();

        assert_matches!(invalid_characters1, Err(e) if e.to_string().contains("Invalid tool FQN format"));
        assert_matches!(invalid_characters2, Err(e) if e.to_string().contains("Invalid tool FQN format"));

        let version_too_large: u64 = u32::MAX as u64 + 1;

        let version_too_large =
            format!("xyz.taluslabs.example@{}", version_too_large).parse::<ToolFqn>();

        assert_matches!(version_too_large, Err(e) if e.to_string().contains("Tool version too large"));
    }
}
