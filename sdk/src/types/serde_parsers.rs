use serde::{de::Deserializer, ser::Serializer, Deserialize, Serialize};

/// Deserialize a `Vec<u8>` into a [reqwest::Url].
pub fn deserialize_bytes_to_url<'de, D>(deserializer: D) -> Result<reqwest::Url, D::Error>
where
    D: Deserializer<'de>,
{
    let bytes: Vec<u8> = Deserialize::deserialize(deserializer)?;
    let url = String::from_utf8(bytes).map_err(serde::de::Error::custom)?;

    reqwest::Url::parse(&url).map_err(serde::de::Error::custom)
}

/// Inverse of [deserialize_bytes_to_url].
pub fn serialize_url_to_bytes<S>(value: &reqwest::Url, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let url = value.to_string();
    let bytes = url.into_bytes();

    bytes.serialize(serializer)
}

/// Deserialize a `Vec<u8>` into a [serde_json::Value].
pub fn deserialize_bytes_to_json_value<'de, D>(
    deserializer: D,
) -> Result<serde_json::Value, D::Error>
where
    D: Deserializer<'de>,
{
    let bytes: Vec<u8> = Deserialize::deserialize(deserializer)?;
    let value = String::from_utf8(bytes).map_err(serde::de::Error::custom)?;

    serde_json::from_str(&value).map_err(serde::de::Error::custom)
}

/// Inverse of [deserialize_bytes_to_json_value].
pub fn serialize_json_value_to_bytes<S>(
    value: &serde_json::Value,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let value = serde_json::to_string(value).map_err(serde::ser::Error::custom)?;
    let bytes = value.into_bytes();

    bytes.serialize(serializer)
}

/// Deserialize a `Vec<Vec<u8>>` into a `serde_json::Value`.
///
/// If the outer `Vec` is len 1, it will be deserialized as a single JSON value.
/// Otherwise it will be deserialized as a JSON array.
#[allow(dead_code)]
pub fn deserialize_array_of_bytes_to_json_value<'de, D>(
    deserializer: D,
) -> Result<serde_json::Value, D::Error>
where
    D: Deserializer<'de>,
{
    let array_of_bytes: Vec<Vec<u8>> = Deserialize::deserialize(deserializer)?;
    let mut result = Vec::with_capacity(array_of_bytes.len());

    for bytes in array_of_bytes {
        let value = String::from_utf8(bytes).map_err(serde::de::Error::custom)?;

        // TODO: This is temporarily added here to automatically fallback to
        // a JSON String if we can't parse the bytes as JSON. In the future,
        // this should fail the execution.
        //
        // TODO: <https://github.com/Talus-Network/nexus-next/issues/97>
        let value = match serde_json::from_str(&value) {
            Ok(value) => value,
            Err(_) => serde_json::Value::String(value),
        };

        result.push(value);
    }

    match result.len() {
        1 => Ok(result.pop().expect("Len is 1")),
        _ => Ok(serde_json::Value::Array(result)),
    }
}

/// Inverse of [deserialize_array_of_bytes_to_json_value].
#[allow(dead_code)]
pub fn serialize_json_value_to_array_of_bytes<S>(
    value: &serde_json::Value,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // The structure of the data here is TBD.
    //
    // TODO: <https://github.com/Talus-Network/nexus-next/issues/97>
    let array = match value {
        serde_json::Value::Array(array) => array,
        value => &vec![value.clone()],
    };

    let mut result = Vec::with_capacity(array.len());

    for value in array {
        let value = serde_json::to_string(value).map_err(serde::ser::Error::custom)?;
        let bytes = value.into_bytes();

        result.push(bytes);
    }

    result.serialize(serializer)
}

/// Custom parser for deserializing to a [u64] from Sui Events. They wrap this
/// type as a string to avoid overflow.
///
/// See [sui_sdk::rpc_types::SuiMoveValue] for more information.
pub fn deserialize_sui_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value: String = Deserialize::deserialize(deserializer)?;
    let value = value.parse::<u64>().map_err(serde::de::Error::custom)?;

    Ok(value)
}

/// Inverse of [deserialize_sui_u64] for indexing reasons.
pub fn serialize_sui_u64<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    value.to_string().serialize(serializer)
}

/// Deserialize a `Vec<u8>` into a `String` using lossy UTF-8 conversion.
pub fn deserialize_bytes_to_lossy_utf8<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let bytes: Vec<u8> = Deserialize::deserialize(deserializer)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub fn deserialize_string_to_datetime<'de, D>(
    deserializer: D,
) -> Result<chrono::DateTime<chrono::Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: String = Deserialize::deserialize(deserializer)?;
    let timestamp = value.parse::<i64>().map_err(serde::de::Error::custom)?;
    let datetime = chrono::DateTime::from_timestamp_millis(timestamp);

    datetime.ok_or(serde::de::Error::custom("datetime out of range"))
}

#[cfg(test)]
mod tests {
    use {super::*, serde::Deserialize, serde_json::json};

    #[derive(Deserialize, Serialize)]
    struct TestStruct {
        #[serde(
            deserialize_with = "deserialize_array_of_bytes_to_json_value",
            serialize_with = "serialize_json_value_to_array_of_bytes"
        )]
        value: serde_json::Value,
    }

    #[derive(Deserialize, Serialize)]
    struct TestUrlStruct {
        #[serde(
            deserialize_with = "deserialize_bytes_to_url",
            serialize_with = "serialize_url_to_bytes"
        )]
        url: reqwest::Url,
    }

    #[derive(Deserialize, Serialize)]
    struct TestSuiU64Struct {
        #[serde(
            deserialize_with = "deserialize_sui_u64",
            serialize_with = "serialize_sui_u64"
        )]
        value: u64,
    }

    #[derive(Deserialize, Serialize, Debug)]
    struct TestDescriptionStruct {
        #[serde(
            deserialize_with = "deserialize_bytes_to_lossy_utf8",
        )]
        value: String,
    }

    #[test]
    fn test_lossy_utf8_deserialization_exact() {
        // The inner array [49, 50, 51] corresponds to a valid UTF-8 byte sequence,
        // which is the string "123".
        let input = r#"{"value":[49,50,51]}"#;
        let result: TestDescriptionStruct = serde_json::from_str(input).unwrap();
        assert_eq!(result.value, "123");
    }

    #[test]
    fn test_lossy_utf8_deserialization_lossy() {
        // The inner array [49, 50, 255, 48] does not correspond to a valid UTF-8 byte sequence.
        // "12\u{FFFD}0" is its lossy UTF-8 representation.
        let input = r#"{"value":[49,50,255,48]}"#;
        let result = serde_json::from_str::<TestDescriptionStruct>(input).unwrap();
        assert_eq!(result.value, "12\u{FFFD}0");
    }

    #[test]
    fn test_single_valid_json_number() {
        // This test supplies a single element.
        // The inner array [49, 50, 51] corresponds to the UTF-8 string "123".
        // "123" is valid JSON and parses to the number 123.
        let input = r#"{"value":[[49,50,51]]}"#;
        let result: TestStruct = serde_json::from_str(input).unwrap();
        assert_eq!(result.value, json!(123));

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_multiple_valid_json() {
        // Two elements:
        // First element: [34,118,97,108,117,101,34] corresponds to "\"value\""
        //   which is valid JSON and becomes the string "value".
        // Second element: [49,50,51] corresponds to "123" and becomes the number 123.
        // Since there is more than one element, the deserializer returns a JSON array.
        let input = r#"{"value":[[34,118,97,108,117,101,34],[49,50,51]]}"#;
        let result: TestStruct = serde_json::from_str(input).unwrap();
        assert_eq!(result.value, json!(["value", 123]));

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_single_invalid_json_fallback() {
        // Single element with bytes for "hello": [104,101,108,108,111].
        // "hello" is not valid JSON (missing quotes), so the fallback
        // returns the string "hello" as a JSON string.
        let input = r#"{"value":[[104,101,108,108,111]]}"#;

        let result: TestStruct = serde_json::from_str(input).unwrap();
        assert_eq!(result.value, json!("hello"));

        let ser = serde_json::to_string(&result).unwrap();
        // Quotes are added when ser'd.
        assert_eq!(ser, r#"{"value":[[34,104,101,108,108,111,34]]}"#);
    }

    #[test]
    fn test_empty_array() {
        // An empty outer array should result in an empty JSON array.
        let input = r#"{"value":[]}"#;
        let result: TestStruct = serde_json::from_str(input).unwrap();
        assert_eq!(result.value, json!([]));

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_url_deser_ser() {
        let bytes = b"https://example.com/";
        let input = format!(r#"{{"url":{}}}"#, serde_json::to_string(&bytes).unwrap());

        let result: TestUrlStruct = serde_json::from_str(&input).unwrap();

        assert_eq!(
            result.url,
            reqwest::Url::parse("https://example.com").unwrap()
        );

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_sui_u64_deser_ser() {
        let input = r#"{"value":"123"}"#;
        let result: TestSuiU64Struct = serde_json::from_str(input).unwrap();
        assert_eq!(result.value, 123);

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }
}
