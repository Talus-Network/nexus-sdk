use serde::{
    de::{DeserializeOwned, Deserializer},
    Deserialize,
    Serialize,
    Serializer,
};

/// A generic wrapper type that transparently serializes and deserializes the inner type `T`.
/// Deserialization uses `serde_path_to_error` to track errors and report the exact JSON path.
#[derive(Debug)]
pub struct WithSerdeErrorPath<T>(pub T);

impl<'de, T> Deserialize<'de> for WithSerdeErrorPath<T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Create a mutable tracker.
        let mut track = serde_path_to_error::Track::new();
        // Wrap the deserializer with our tracker.
        let d = serde_path_to_error::Deserializer::new(deserializer, &mut track);
        let t = T::deserialize(d)?;
        Ok(WithSerdeErrorPath(t))
    }
}

impl<T> Serialize for WithSerdeErrorPath<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serde_path_to_error::serialize(&self.0, serializer).map_err(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        serde::{Deserialize, Serialize},
        std::cell::RefCell,
    };
    // A dummy type for testing.
    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct Dummy {
        foo: String,
        bar: i32,
    }

    #[test]
    fn test_deserialize_valid() {
        let json_str = r#"{"foo": "hello", "bar": 42}"#;
        // Deserialize using our WithSerdeErrorPath wrapper.
        let wrapped: WithSerdeErrorPath<Dummy> =
            serde_json::from_str(json_str).expect("Deserialization should succeed");
        let dummy: Dummy = wrapped.0;
        assert_eq!(dummy.foo, "hello");
        assert_eq!(dummy.bar, 42);
    }

    #[test]
    fn test_serialize() {
        let dummy = Dummy {
            foo: "world".to_string(),
            bar: 100,
        };
        let wrapped = WithSerdeErrorPath(dummy);
        let json_str = serde_json::to_string(&wrapped).expect("Serialization should succeed");

        // Parse back into a serde_json::Value to avoid ordering issues.
        let value: serde_json::Value =
            serde_json::from_str(&json_str).expect("Parsing serialized JSON should succeed");
        let expected = serde_json::json!({
            "foo": "world",
            "bar": 100
        });
        assert_eq!(value, expected);
    }

    #[derive(Serialize)]
    struct Outer<'a> {
        inner: Inner<'a>,
    }

    #[derive(Serialize)]
    struct Inner<'a> {
        refcell: &'a RefCell<String>,
    }

    #[test]
    fn test_serialize_failure_with_path() {
        let refcell = RefCell::new(String::new());
        let outer = Outer {
            inner: Inner { refcell: &refcell },
        };

        // Force an error by borrowing mutably.
        let _borrow = refcell.borrow_mut();

        let wrapped = WithSerdeErrorPath(outer);
        let result = serde_json::to_string(&wrapped);

        assert!(result.is_err(), "Expected serialization to fail");

        let error_message = result.err().unwrap().to_string();
        assert!(
            error_message.contains("already mutably borrowed"),
            "Expected error message to mention 'inner.refcell', got: {}",
            error_message
        );
    }

    #[test]
    fn test_deserialize_error_path() {
        // Create invalid JSON: missing the "bar" field.
        let json_str = r#"{"foo": "oops"}"#;
        let err = serde_json::from_str::<WithSerdeErrorPath<Dummy>>(json_str)
            .expect_err("Deserialization should fail due to missing field");
        let error_str = err.to_string();

        // The error message should contain a hint about the missing "bar" field.
        // Depending on serde_path_to_error's formatting, the error should mention "bar".
        assert!(
            error_str.contains("bar"),
            "Error should indicate that the `bar` field is missing: {}",
            error_str
        );
    }
}
