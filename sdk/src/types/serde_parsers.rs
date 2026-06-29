use {
    super::{
        move_json::{parse_byte_vector_value, parse_string_value, parse_u64_value},
        parse_address_value,
        ExecutionPaymentFinalState,
        MoveOption,
        ScheduledOccurrenceFinalState,
        VertexExecutionPaymentSettlementKind,
    },
    crate::{sui, types::strip_fields_owned},
    base64::{prelude::BASE64_STANDARD, Engine},
    serde::{
        de::{DeserializeOwned, Deserializer},
        ser::Serializer,
        Deserialize,
        Serialize,
    },
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

/// Deserialize a Move `Option<T>` field into a Rust `Option<T>`.
pub fn deserialize_move_option_field<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    MoveOption::<T>::deserialize(deserializer).map(|value| value.0)
}

/// Deserialize a Move `Option<u64>` whose human-readable Sui JSON may encode `u64` as a string.
pub fn deserialize_move_option_sui_u64<'de, D>(deserializer: D) -> Result<MoveOption<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return MoveOption::<u64>::deserialize(deserializer);
    }

    fn parse_value(value: Value) -> Result<Option<u64>, String> {
        let value = strip_fields_owned(value);
        match value {
            Value::Null => Ok(None),
            Value::String(value) => value
                .parse::<u64>()
                .map(Some)
                .map_err(|e| format!("invalid number: {e}")),
            Value::Number(value) => value
                .as_u64()
                .map(Some)
                .ok_or_else(|| "expected unsigned number".to_string()),
            Value::Array(mut values) => values
                .drain(..)
                .next()
                .map(parse_value)
                .transpose()
                .map(|value| value.flatten()),
            Value::Object(mut object) => {
                if let Some(vec) = object.remove("vec").or_else(|| object.remove("Vec")) {
                    return parse_value(vec);
                }
                if let Some(inner) = object.remove("some").or_else(|| object.remove("Some")) {
                    return parse_value(inner);
                }
                if object.contains_key("none") || object.contains_key("None") {
                    return Ok(None);
                }
                Err("expected Move Option<u64>".to_string())
            }
            other => Err(format!("unexpected value for Option<u64>: {other}")),
        }
    }

    parse_value(Deserialize::deserialize(deserializer)?)
        .map(MoveOption)
        .map_err(serde::de::Error::custom)
}

/// Deserialize a Move `Option<u64>` field into a Rust `Option<u64>`.
pub fn deserialize_move_option_sui_u64_field<'de, D>(
    deserializer: D,
) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_move_option_sui_u64(deserializer).map(|value| value.0)
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
    if deserializer.is_human_readable() {
        match Value::deserialize(deserializer)? {
            Value::String(value) => match BASE64_STANDARD.decode(&value) {
                Ok(bytes) => String::from_utf8(bytes).map_err(serde::de::Error::custom),
                Err(_) => Ok(value),
            },
            value => {
                let bytes = deserialize_encoded_bytes(value).map_err(serde::de::Error::custom)?;
                String::from_utf8(bytes).map_err(serde::de::Error::custom)
            }
        }
    } else {
        let bytes = deserialize_encoded_bytes(deserializer)?;
        String::from_utf8(bytes).map_err(serde::de::Error::custom)
    }
}

/// Inverse of [deserialize_bytes_to_string].
pub fn serialize_string_to_bytes<S>(value: &String, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let bytes = value.as_bytes();
    serialize_encoded_bytes(bytes, serializer)
}

/// Deserialize Move `std::ascii::String` into a Rust `String`.
pub fn deserialize_move_ascii_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct MoveAsciiString {
        bytes: Vec<u8>,
    }

    if !deserializer.is_human_readable() {
        let value = MoveAsciiString::deserialize(deserializer)?;
        return String::from_utf8(value.bytes).map_err(serde::de::Error::custom);
    }

    let value = Value::deserialize(deserializer)?;
    let Some(parsed) = parse_string_value(&value).map_err(serde::de::Error::custom)? else {
        return Err(serde::de::Error::custom(
            "could not parse Move ascii string",
        ));
    };

    Ok(parsed)
}

/// Deserialize a Move `address` field from either a string or a Move-JSON
/// wrapper.
pub fn deserialize_tap_address_value<'de, D>(
    deserializer: D,
) -> Result<sui::types::Address, D::Error>
where
    D: Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return sui::types::Address::deserialize(deserializer);
    }

    let value = serde_json::Value::deserialize(deserializer)?;
    parse_address_value(&value)
        .map_err(serde::de::Error::custom)?
        .ok_or_else(|| serde::de::Error::custom("missing TAP address value"))
}

/// Deserialize a Move `u64` field from string or numeric JSON representations.
pub fn deserialize_tap_u64_value<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return u64::deserialize(deserializer);
    }

    let value = serde_json::Value::deserialize(deserializer)?;
    parse_u64_value(&value)
        .map_err(serde::de::Error::custom)?
        .ok_or_else(|| serde::de::Error::custom("missing TAP u64 value"))
}

/// Deserialize a `MoveOption`-encoded optional address.
pub fn deserialize_move_option_tap_address<'de, D>(
    deserializer: D,
) -> Result<Option<sui::types::Address>, D::Error>
where
    D: Deserializer<'de>,
{
    MoveOption::<sui::types::Address>::deserialize(deserializer).map(|value| value.0)
}

/// Deserialize a Move byte vector as either a byte array or hex/base64/UTF-8
/// string.
pub fn deserialize_tap_byte_vector<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return Vec::<u8>::deserialize(deserializer);
    }

    let value = serde_json::Value::deserialize(deserializer)?;
    parse_byte_vector_value(&value)
        .map_err(serde::de::Error::custom)?
        .ok_or_else(|| serde::de::Error::custom("missing TAP byte-vector value"))
}

/// Deserialize a vector of Move byte vectors from arrays or encoded strings.
pub fn deserialize_tap_byte_vector_vec<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return Vec::<Vec<u8>>::deserialize(deserializer);
    }

    let value = Value::deserialize(deserializer)?;
    let value = strip_fields_owned(value);
    let Value::Array(values) = value else {
        return Err(serde::de::Error::custom(
            "expected array of TAP byte-vector values",
        ));
    };

    values
        .into_iter()
        .map(|value| {
            parse_byte_vector_value(&value)
                .map_err(serde::de::Error::custom)?
                .ok_or_else(|| serde::de::Error::custom("missing TAP byte-vector value"))
        })
        .collect()
}

pub fn deserialize_vertex_execution_payment_settlement_kind_value(
    value: &serde_json::Value,
) -> Option<VertexExecutionPaymentSettlementKind> {
    fn from_text(text: &str) -> Option<VertexExecutionPaymentSettlementKind> {
        match text {
            "free" | "Free" => Some(VertexExecutionPaymentSettlementKind::Free),
            "ticket" | "Ticket" => Some(VertexExecutionPaymentSettlementKind::Ticket),
            "paid" | "Paid" => Some(VertexExecutionPaymentSettlementKind::Paid),
            _ => None,
        }
    }

    match value {
        serde_json::Value::String(text) => from_text(text),
        serde_json::Value::Object(object) => {
            for key in ["@variant", "variant", "type"] {
                if let Some(serde_json::Value::String(text)) = object.get(key) {
                    if let Some(kind) = from_text(text) {
                        return Some(kind);
                    }
                }
            }

            if let Some(fields) = object.get("fields") {
                if let Some(kind) =
                    deserialize_vertex_execution_payment_settlement_kind_value(fields)
                {
                    return Some(kind);
                }
            }

            object.keys().find_map(|key| from_text(key))
        }
        _ => None,
    }
}

pub fn deserialize_tap_execution_payment_final_state_value(
    value: &serde_json::Value,
) -> Option<ExecutionPaymentFinalState> {
    fn from_text(text: &str) -> Option<ExecutionPaymentFinalState> {
        match text {
            "pending" | "Pending" => Some(ExecutionPaymentFinalState::Pending),
            "accomplished" | "Accomplished" => Some(ExecutionPaymentFinalState::Accomplished),
            "refunded" | "Refunded" => Some(ExecutionPaymentFinalState::Refunded),
            _ => None,
        }
    }

    match value {
        serde_json::Value::String(text) => from_text(text),
        serde_json::Value::Object(object) => {
            for key in ["@variant", "variant", "type"] {
                if let Some(serde_json::Value::String(text)) = object.get(key) {
                    if let Some(state) = from_text(text) {
                        return Some(state);
                    }
                }
            }

            if let Some(fields) = object.get("fields") {
                if let Some(state) = deserialize_tap_execution_payment_final_state_value(fields) {
                    return Some(state);
                }
            }

            object.keys().find_map(|key| from_text(key))
        }
        _ => None,
    }
}

pub fn deserialize_tap_scheduled_occurrence_final_state_value(
    value: &serde_json::Value,
) -> Option<ScheduledOccurrenceFinalState> {
    fn from_text(text: &str) -> Option<ScheduledOccurrenceFinalState> {
        match text {
            "in_flight" | "inFlight" | "InFlight" => Some(ScheduledOccurrenceFinalState::InFlight),
            "accomplished" | "Accomplished" => Some(ScheduledOccurrenceFinalState::Accomplished),
            "refunded" | "Refunded" => Some(ScheduledOccurrenceFinalState::Refunded),
            _ => None,
        }
    }

    match value {
        serde_json::Value::String(text) => from_text(text),
        serde_json::Value::Object(object) => {
            for key in ["@variant", "variant", "type"] {
                if let Some(serde_json::Value::String(text)) = object.get(key) {
                    if let Some(state) = from_text(text) {
                        return Some(state);
                    }
                }
            }

            if let Some(fields) = object.get("fields") {
                if let Some(state) = deserialize_tap_scheduled_occurrence_final_state_value(fields)
                {
                    return Some(state);
                }
            }

            object.keys().find_map(|key| from_text(key))
        }
        _ => None,
    }
}

/// Deserialize authorization vertex strings from either UTF-8 bytes, base16 bytes,
/// raw UTF-8, or plain text.
pub fn deserialize_vertex_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        return String::from_utf8(bytes).map_err(serde::de::Error::custom);
    }

    let value = serde_json::Value::deserialize(deserializer)?;
    if let Some(bytes) = parse_byte_vector_value(&value).map_err(serde::de::Error::custom)? {
        return String::from_utf8(bytes).map_err(serde::de::Error::custom);
    }

    let text = parse_string_value(&value)
        .map_err(serde::de::Error::custom)?
        .ok_or_else(|| serde::de::Error::custom("missing authorization vertex value"))?;

    if let Some(hex) = text.strip_prefix("0x") {
        if hex.len() % 2 == 0 && hex.as_bytes().iter().all(u8::is_ascii_hexdigit) {
            if let Ok(bytes) = hex::decode(hex) {
                if let Ok(decoded) = String::from_utf8(bytes) {
                    return Ok(decoded);
                }
            }
        }
    }

    Ok(text)
}

/// Serialize a Rust `String` into Move `std::ascii::String`.
pub fn serialize_move_ascii_string<S>(value: &String, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    #[derive(Serialize)]
    struct MoveAsciiString<'a> {
        bytes: &'a [u8],
    }

    if serializer.is_human_readable() {
        return value.serialize(serializer);
    }

    MoveAsciiString {
        bytes: value.as_bytes(),
    }
    .serialize(serializer)
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
    // Accommodate for BCS.
    if !deserializer.is_human_readable() {
        return Vec::<u8>::deserialize(deserializer);
    }

    let encoded = String::deserialize(deserializer)?;

    BASE64_STANDARD
        .decode(&encoded)
        .map_err(serde::de::Error::custom)
}

/// Inverse of [deserialize_encoded_bytes].
pub fn serialize_encoded_bytes<S>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if !serializer.is_human_readable() {
        return value.serialize(serializer);
    }

    let encoded = BASE64_STANDARD.encode(value);

    encoded.serialize(serializer)
}

/// Same as [deserialize_encoded_bytes] but for a `Vec<Vec<u8>>`.
pub fn deserialize_encoded_bytes_vec<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    // Accommodate for BCS.
    if !deserializer.is_human_readable() {
        return Vec::<Vec<u8>>::deserialize(deserializer);
    }

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
    if !serializer.is_human_readable() {
        return value.serialize(serializer);
    }

    let encoded_vec: Vec<String> = value
        .iter()
        .map(|bytes| BASE64_STANDARD.encode(bytes))
        .collect();

    encoded_vec.serialize(serializer)
}

/// Deserialize a timestamp in milliseconds since epoch stored as a string
pub fn deserialize_u64_to_datetime<'de, D>(
    deserializer: D,
) -> Result<chrono::DateTime<chrono::Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let timestamp = u64::deserialize(deserializer)?;
    let datetime = chrono::DateTime::from_timestamp_millis(timestamp as i64);

    datetime.ok_or(serde::de::Error::custom("datetime out of range"))
}

/// Inverse of [deserialize_u64_to_datetime].
pub fn serialize_datetime_to_u64<S>(
    value: &chrono::DateTime<chrono::Utc>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_u64(value.timestamp_millis() as u64)
}

/// Deserialize a duration in milliseconds stored as a string.
pub fn deserialize_u64_to_duration<'de, D>(deserializer: D) -> Result<chrono::Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let millis = u64::deserialize(deserializer)?;

    Ok(chrono::Duration::milliseconds(millis as i64))
}

/// Inverse of [deserialize_u64_to_duration].
pub fn serialize_duration_to_u64<S>(
    value: &chrono::Duration,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_u64(value.num_milliseconds() as u64)
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
    struct TestSuiDatetimeStruct {
        #[serde(
            deserialize_with = "deserialize_sui_u64_to_datetime",
            serialize_with = "serialize_datetime_to_sui_u64"
        )]
        value: chrono::DateTime<chrono::Utc>,
    }

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct TestSuiDurationStruct {
        #[serde(
            deserialize_with = "deserialize_sui_u64_to_duration",
            serialize_with = "serialize_duration_to_sui_u64"
        )]
        value: chrono::Duration,
    }

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct TestSuiOptionDatetimeStruct {
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

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct TestDatetimeStruct {
        #[serde(
            deserialize_with = "deserialize_u64_to_datetime",
            serialize_with = "serialize_datetime_to_u64"
        )]
        value: chrono::DateTime<chrono::Utc>,
    }

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct TestDurationStruct {
        #[serde(
            deserialize_with = "deserialize_u64_to_duration",
            serialize_with = "serialize_duration_to_u64"
        )]
        value: chrono::Duration,
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
    fn test_bytes_to_string_accepts_plain_json_string() {
        let result: TestStringStruct = serde_json::from_str(r#"{"value":"hello world"}"#).unwrap();
        assert_eq!(result.value, "hello world");

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, r#"{"value":"aGVsbG8gd29ybGQ="}"#);
    }

    #[test]
    fn test_sui_datetime_deser_ser() {
        let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(1_600_000_000, 0).unwrap();
        let input = r#"{"value":"1600000000000"}"#.to_string();
        let result: TestSuiDatetimeStruct = serde_json::from_str(&input).unwrap();
        assert_eq!(result.value, dt);

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_sui_duration_deser_ser() {
        let dur = chrono::Duration::milliseconds(12345);
        let input = r#"{"value":"12345"}"#.to_string();
        let result: TestSuiDurationStruct = serde_json::from_str(&input).unwrap();
        assert_eq!(result.value, dur);

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_option_datetime_deser_ser_some() {
        let dt = chrono::Utc.timestamp_millis_opt(1_600_000_000_000).unwrap();
        let input = r#"{"value":"1600000000000"}"#.to_string();
        let result: TestSuiOptionDatetimeStruct = serde_json::from_str(&input).unwrap();
        assert_eq!(result.value, Some(dt));

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_option_datetime_deser_ser_none() {
        let input = r#"{"value":null}"#;
        let result: TestSuiOptionDatetimeStruct = serde_json::from_str(input).unwrap();
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

    #[test]
    fn test_encoded_bytes_bcs_ser_deser() {
        let value = TestEncodedBytesStruct {
            value: b"test bytes".to_vec(),
        };

        let bytes = bcs::to_bytes(&value).unwrap();
        assert_eq!(bytes, bcs::to_bytes(&value.value).unwrap());

        let round_trip: TestEncodedBytesStruct = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(round_trip, value);
    }

    #[test]
    fn test_encoded_bytes_vec_bcs_ser_deser() {
        let value = TestEncodedBytesVecStruct {
            value: vec![b"foo".to_vec(), b"bar".to_vec()],
        };

        let bytes = bcs::to_bytes(&value).unwrap();
        assert_eq!(bytes, bcs::to_bytes(&value.value).unwrap());

        let round_trip: TestEncodedBytesVecStruct = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(round_trip, value);
    }

    #[test]
    fn test_datetime_deser_ser() {
        let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(1_600_000_000, 0).unwrap();
        let input = r#"{"value":1600000000000}"#.to_string();
        let result: TestDatetimeStruct = serde_json::from_str(&input).unwrap();
        assert_eq!(result.value, dt);

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }

    #[test]
    fn test_duration_deser_ser() {
        let dur = chrono::Duration::milliseconds(12345);
        let input = r#"{"value":12345}"#.to_string();
        let result: TestDurationStruct = serde_json::from_str(&input).unwrap();
        assert_eq!(result.value, dur);

        let ser = serde_json::to_string(&result).unwrap();
        assert_eq!(ser, input);
    }
}
