use {
    crate::ToolFqn,
    serde::{Deserialize, Serialize},
    std::time::Duration,
};

/// Useful struct holding Tool metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolMeta {
    pub fqn: ToolFqn,
    pub url: String,
    pub description: String,
    #[serde(
        deserialize_with = "duration::from_millis",
        serialize_with = "duration::to_millis"
    )]
    pub timeout: Duration,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
}

mod duration {
    use {
        super::*,
        serde::{Deserializer, Serializer},
    };

    pub fn from_millis<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }

    pub fn to_millis<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let millis = duration.as_millis() as u64;
        millis.serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::fqn, serde_json::json};

    #[test]
    fn test_duration_serialize() {
        let duration = Duration::from_millis(1500);
        let serialized = serde_json::to_string(
            &duration::to_millis(&duration, serde_json::value::Serializer).unwrap(),
        )
        .unwrap();
        assert_eq!(serialized, "1500");
    }

    #[test]
    fn test_duration_deserialize() {
        let millis_json = json!(2500);
        let duration: Duration = duration::from_millis(millis_json).unwrap();
        assert_eq!(duration, Duration::from_millis(2500));
    }

    #[test]
    fn test_tool_meta_serialize_deserialize() {
        let tool_meta = ToolMeta {
            fqn: fqn!("xyz.test.tool@1"),
            url: "http://example.com".to_string(),
            description: "A test tool".to_string(),
            timeout: Duration::from_millis(5000),
            input_schema: json!({"type": "object"}),
            output_schema: json!({"type": "object"}),
        };

        let serialized = serde_json::to_string(&tool_meta).unwrap();
        let deserialized: ToolMeta = serde_json::from_str(&serialized).unwrap();

        assert_eq!(tool_meta.fqn, deserialized.fqn);
        assert_eq!(tool_meta.url, deserialized.url);
        assert_eq!(tool_meta.description, deserialized.description);
        assert_eq!(tool_meta.timeout, deserialized.timeout);
        assert_eq!(tool_meta.input_schema, deserialized.input_schema);
        assert_eq!(tool_meta.output_schema, deserialized.output_schema);
    }
}
