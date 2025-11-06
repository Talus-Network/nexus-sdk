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

/// Check if a string represents a large number (u128/u256 range).
/// Handles both positive and negative integers.
fn is_large_number(s: &str) -> bool {
    if s.starts_with('-') {
        s[1..].chars().all(|c| c.is_ascii_digit()) && s.len() > 21
    } else {
        s.chars().all(|c| c.is_ascii_digit()) && s.len() > 20
    }
}

/// Wrap large numbers as JSON strings to preserve precision for u128/u256.
fn wrap_large_numbers_as_string(value: &str) -> String {
    let trimmed = value.trim();
    if is_large_number(trimmed) {
        format!(r#""{}""#, trimmed)
    } else {
        trimmed.to_string()
    }
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

        // Wrap large numbers as strings to preserve precision.
        // We also trim the value to remove any leading or trailing whitespace.
        let adjusted_value = wrap_large_numbers_as_string(&value);

        let value = match serde_json::from_str(&adjusted_value) {
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

/// Deserialize a `Vec<u8>` into a [String].
pub fn deserialize_bytes_to_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let bytes: Vec<u8> = Deserialize::deserialize(deserializer)?;
    String::from_utf8(bytes).map_err(serde::de::Error::custom)
}

/// Inverse of [deserialize_bytes_to_string].
pub fn serialize_string_to_bytes<S>(value: &String, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let bytes = value.as_bytes();
    bytes.serialize(serializer)
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
    use {super::*, serde::Deserialize};

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
        #[serde(deserialize_with = "deserialize_bytes_to_lossy_utf8")]
        value: String,
    }

    #[test]
    fn test_lossy_utf8_deserialization_exact() {
        // The array [49, 50, 51] corresponds to a valid UTF-8 byte sequence,
        // which is the string "123".
        let input = r#"{"value":[49,50,51]}"#;
        let result: TestDescriptionStruct = serde_json::from_str(input).unwrap();
        assert_eq!(result.value, "123");
    }

    #[test]
    fn test_lossy_utf8_deserialization_lossy() {
        // The array [49, 50, 255, 48] does not correspond to a valid UTF-8 byte sequence.
        // "12\u{FFFD}0" is its lossy UTF-8 representation.
        let input = r#"{"value":[49,50,255,48]}"#;
        let result = serde_json::from_str::<TestDescriptionStruct>(input).unwrap();
        assert_eq!(result.value, "12\u{FFFD}0");
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

    #[test]
    fn test_large_number_precision_preserved() {
        // Test that large numbers (like u256) are converted to strings to preserve precision.
        let large_u256 =
            "105792089237316195563853351929625371316844592863025172891227567439681422591090";
        let input = format!(
            r#"{{"value":[{}]}}"#,
            serde_json::to_string(&large_u256).unwrap()
        );

        let result: TestStruct = serde_json::from_str(&input).unwrap();
        // The large number should be stored as a string to avoid precision loss.
        assert_eq!(
            result.value,
            serde_json::Value::String(large_u256.to_string())
        );
    }
}
