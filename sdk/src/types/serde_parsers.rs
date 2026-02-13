use {
    base64::{prelude::BASE64_STANDARD, Engine},
    serde::{de::Deserializer, ser::Serializer, Deserialize, Serialize},
    serde_json::Value,
};

/// Deserialize base 64 encoded bytes into a [reqwest::Url].
pub fn deserialize_bytes_to_url<'de, D>(deserializer: D) -> Result<reqwest::Url, D::Error>
where
    D: Deserializer<'de>,
{
    let bytes: Vec<u8> = deserialize_encoded_bytes(deserializer)?;
    let url = String::from_utf8(bytes).map_err(serde::de::Error::custom)?;

    reqwest::Url::parse(&url).map_err(serde::de::Error::custom)
}

/// Inverse of [deserialize_bytes_to_url].
pub fn serialize_url_to_bytes<S>(value: &reqwest::Url, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let url = value.to_string();
    let bytes = url.as_bytes();

    serialize_encoded_bytes(bytes, serializer)
}

/// Deserialize base 64 encoded bytes into a [serde_json::Value].
pub fn deserialize_bytes_to_json_value<'de, D>(
    deserializer: D,
) -> Result<serde_json::Value, D::Error>
where
    D: Deserializer<'de>,
{
    let bytes: Vec<u8> = deserialize_encoded_bytes(deserializer)?;
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
    let bytes = value.as_bytes();

    serialize_encoded_bytes(bytes, serializer)
}

/// Custom parser for deserializing to a [u64] from Sui Events. They wrap this
/// type as a string to avoid overflow.
///
/// See [sui_sdk::rpc_types::SuiMoveValue] for more information.
pub fn deserialize_sui_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return u64::deserialize(deserializer);
    }

    let value: Value = Deserialize::deserialize(deserializer)?;
    let value = match value {
        Value::String(value) => value.parse::<u64>().map_err(serde::de::Error::custom)?,
        Value::Number(num) => num
            .as_u64()
            .ok_or_else(|| serde::de::Error::custom("expected unsigned number"))?,
        other => {
            return Err(serde::de::Error::custom(format!(
                "unexpected value for u64: {other}"
            )))
        }
    };

    Ok(value)
}

/// Inverse of [deserialize_sui_u64] for indexing reasons.
pub fn serialize_sui_u64<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if serializer.is_human_readable() {
        value.to_string().serialize(serializer)
    } else {
        value.serialize(serializer)
    }
}

/// Deserialize an optional Sui `u64` value.
pub fn deserialize_sui_option_u64<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return Option::<u64>::deserialize(deserializer);
    }

    fn parse_value(value: Value) -> Result<Option<u64>, String> {
        match value {
            Value::Null => Ok(None),
            Value::String(str) => {
                if str.is_empty() {
                    Ok(None)
                } else {
                    str.parse::<u64>()
                        .map(Some)
                        .map_err(|e| format!("invalid number: {e}"))
                }
            }
            Value::Number(num) => num
                .as_u64()
                .map(Some)
                .ok_or_else(|| "expected unsigned number".to_string()),
            Value::Object(mut object) => {
                if let Some(inner) = object.remove("some").or_else(|| object.remove("Some")) {
                    parse_value(inner)
                } else if object.contains_key("none") || object.contains_key("None") {
                    Ok(None)
                } else {
                    Err("expected Option with `some` or `none` field".to_string())
                }
            }
            other => Err(format!("unexpected value for Option<u64>: {other}")),
        }
    }

    match parse_value(Deserialize::deserialize(deserializer)?) {
        Ok(value) => Ok(value),
        Err(err) => Err(serde::de::Error::custom(err)),
    }
}

/// Serialize an optional Sui `u64` value.
pub fn serialize_sui_option_u64<S>(value: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(inner) => Value::String(inner.to_string()).serialize(serializer),
        None => serializer.serialize_none(),
    }
}

/// Deserialize base 64 encoded bytes into a [String].
pub fn deserialize_bytes_to_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let bytes = deserialize_encoded_bytes(deserializer)?;
    String::from_utf8(bytes).map_err(serde::de::Error::custom)
}

/// Inverse of [deserialize_bytes_to_string].
pub fn serialize_string_to_bytes<S>(value: &String, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let bytes = value.as_bytes();
    serialize_encoded_bytes(bytes, serializer)
}

/// Deserialize a timestamp in milliseconds since epoch stored as a string
pub fn deserialize_sui_u64_to_datetime<'de, D>(
    deserializer: D,
) -> Result<chrono::DateTime<chrono::Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let timestamp = deserialize_sui_u64(deserializer)?;
    let datetime = chrono::DateTime::from_timestamp_millis(timestamp as i64);

    datetime.ok_or(serde::de::Error::custom("datetime out of range"))
}

/// Inverse of [deserialize_sui_u64_to_datetime].
pub fn serialize_datetime_to_sui_u64<S>(
    value: &chrono::DateTime<chrono::Utc>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let timestamp = value.timestamp_millis() as u64;

    serialize_sui_u64(&timestamp, serializer)
}

/// Deserialize a duration in milliseconds stored as a string.
pub fn deserialize_sui_u64_to_duration<'de, D>(
    deserializer: D,
) -> Result<chrono::Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let millis = deserialize_sui_u64(deserializer)?;

    Ok(chrono::Duration::milliseconds(millis as i64))
}

/// Inverse of [deserialize_sui_u64_to_duration].
pub fn serialize_duration_to_sui_u64<S>(
    value: &chrono::Duration,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let millis = value.num_milliseconds() as u64;

    serialize_sui_u64(&millis, serializer)
}

/// Deserialize a timestamp in milliseconds since epoch stored as a string
pub fn deserialize_option_sui_u64_to_datetime<'de, D>(
    deserializer: D,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let timestamp = deserialize_sui_option_u64(deserializer)?;

    let Some(timestamp) = timestamp else {
        return Ok(None);
    };

    Ok(chrono::DateTime::from_timestamp_millis(timestamp as i64))
}

/// Inverse of [deserialize_option_sui_u64_to_datetime].
pub fn serialize_option_datetime_to_sui_u64<S>(
    value: &Option<chrono::DateTime<chrono::Utc>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(datetime) => {
            let timestamp = datetime.timestamp_millis() as u64;
            serialize_sui_option_u64(&Some(timestamp), serializer)
        }
        None => serialize_sui_option_u64(&None, serializer),
    }
}

/// Deserialize a base64 encoded vector of bytest to a [`Vec<u8>`].
pub fn deserialize_encoded_bytes<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let encoded: String = Deserialize::deserialize(deserializer)?;

    BASE64_STANDARD
        .decode(&encoded)
        .map_err(serde::de::Error::custom)
}

/// Inverse of [deserialize_encoded_bytes].
pub fn serialize_encoded_bytes<S>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let encoded = BASE64_STANDARD.encode(value);

    encoded.serialize(serializer)
}

/// Same as [deserialize_encoded_bytes] but for a `Vec<Vec<u8>>`.
pub fn deserialize_encoded_bytes_vec<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    let encoded_vec: Vec<String> = Deserialize::deserialize(deserializer)?;

    encoded_vec
        .into_iter()
        .map(|encoded| {
            BASE64_STANDARD
                .decode(&encoded)
                .map_err(serde::de::Error::custom)
        })
        .collect()
}

/// Inverse of [deserialize_encoded_bytes_vec].
pub fn serialize_encoded_bytes_vec<S>(value: &[Vec<u8>], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let encoded_vec: Vec<String> = value
        .iter()
        .map(|bytes| BASE64_STANDARD.encode(bytes))
        .collect();

    encoded_vec.serialize(serializer)
}

#[cfg(test)]
mod tests {
    use {super::*, chrono::TimeZone, serde::Deserialize};

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

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct TestSuiOptionU64Struct {
        #[serde(
            deserialize_with = "deserialize_sui_option_u64",
            serialize_with = "serialize_sui_option_u64"
        )]
        value: Option<u64>,
    }

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct TestStringStruct {
        #[serde(
            deserialize_with = "deserialize_bytes_to_string",
            serialize_with = "serialize_string_to_bytes"
        )]
        value: String,
    }

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct TestDatetimeStruct {
        #[serde(
            deserialize_with = "deserialize_sui_u64_to_datetime",
            serialize_with = "serialize_datetime_to_sui_u64"
        )]
        value: chrono::DateTime<chrono::Utc>,
    }

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct TestDurationStruct {
        #[serde(
            deserialize_with = "deserialize_sui_u64_to_duration",
            serialize_with = "serialize_duration_to_sui_u64"
        )]
        value: chrono::Duration,
    }

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct TestOptionDatetimeStruct {
        #[serde(
            deserialize_with = "deserialize_option_sui_u64_to_datetime",
            serialize_with = "serialize_option_datetime_to_sui_u64"
        )]
        value: Option<chrono::DateTime<chrono::Utc>>,
    }

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct TestEncodedBytesStruct {
        #[serde(
            deserialize_with = "deserialize_encoded_bytes",
            serialize_with = "serialize_encoded_bytes"
        )]
        value: Vec<u8>,
    }

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct TestEncodedBytesVecStruct {
        #[serde(
            deserialize_with = "deserialize_encoded_bytes_vec",
            serialize_with = "serialize_encoded_bytes_vec"
        )]
        value: Vec<Vec<u8>>,
    }

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct TestJsonValueStruct {
        #[serde(
            deserialize_with = "deserialize_bytes_to_json_value",
            serialize_with = "serialize_json_value_to_bytes"
        )]
        value: serde_json::Value,
    }

    #[test]
    fn test_url_deser_ser() {
        let bytes = "aHR0cHM6Ly9leGFtcGxlLmNvbS8=";
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
    fn test_json_value_deser_ser() {
        let json = serde_json::json!({"foo": "bar"});
        let encoded =
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(&json).unwrap());
        let input = format!(
            r#"{{"value":{}}}"#,
            serde_json::to_string(&encoded).unwrap()
        );
        let result: TestJsonValueStruct = serde_json::from_str(&input).unwrap();
        assert_eq!(result.value, json);

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_sui_option_u64_deser_ser_some() {
        let input = r#"{"value":"456"}"#;
        let result: TestSuiOptionU64Struct = serde_json::from_str(input).unwrap();
        assert_eq!(result.value, Some(456));
        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_sui_option_u64_deser_ser_none() {
        let input = r#"{"value":null}"#;
        let result: TestSuiOptionU64Struct = serde_json::from_str(input).unwrap();
        assert_eq!(result.value, None);
        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_bytes_to_string_deser_ser() {
        let s = "hello world";
        let encoded = base64::engine::general_purpose::STANDARD.encode(s.as_bytes());
        let input = format!(
            r#"{{"value":{}}}"#,
            serde_json::to_string(&encoded).unwrap()
        );
        let result: TestStringStruct = serde_json::from_str(&input).unwrap();
        assert_eq!(result.value, s);

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_datetime_deser_ser() {
        let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_secs(1_600_000_000).unwrap();
        let input = format!(r#"{{"value":"1600000000000"}}"#);
        let result: TestDatetimeStruct = serde_json::from_str(&input).unwrap();
        assert_eq!(result.value, dt);

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_duration_deser_ser() {
        let dur = chrono::Duration::milliseconds(12345);
        let input = format!(r#"{{"value":"12345"}}"#);
        let result: TestDurationStruct = serde_json::from_str(&input).unwrap();
        assert_eq!(result.value, dur);

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_option_datetime_deser_ser_some() {
        let dt = chrono::Utc.timestamp_millis_opt(1_600_000_000_000).unwrap();
        let input = format!(r#"{{"value":"1600000000000"}}"#);
        let result: TestOptionDatetimeStruct = serde_json::from_str(&input).unwrap();
        assert_eq!(result.value, Some(dt));

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_option_datetime_deser_ser_none() {
        let input = r#"{"value":null}"#;
        let result: TestOptionDatetimeStruct = serde_json::from_str(input).unwrap();
        assert_eq!(result.value, None);

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_encoded_bytes_deser_ser() {
        let bytes = b"test bytes";
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        let input = format!(
            r#"{{"value":{}}}"#,
            serde_json::to_string(&encoded).unwrap()
        );
        let result: TestEncodedBytesStruct = serde_json::from_str(&input).unwrap();
        assert_eq!(result.value, bytes);

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_encoded_bytes_vec_deser_ser() {
        let vec = vec![b"foo".to_vec(), b"bar".to_vec()];
        let encoded_vec: Vec<String> = vec
            .iter()
            .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
            .collect();
        let input = format!(
            r#"{{"value":{}}}"#,
            serde_json::to_string(&encoded_vec).unwrap()
        );
        let result: TestEncodedBytesVecStruct = serde_json::from_str(&input).unwrap();
        assert_eq!(result.value, vec);

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }
}
