use serde::{de::DeserializeOwned, Deserialize};

/// Unwrap Sui's Move-JSON `{ fields: { ... } }` wrapper when present.
pub fn strip_fields_owned(value: serde_json::Value) -> serde_json::Value {
    let serde_json::Value::Object(mut object) = value else {
        return value;
    };

    match object.remove("fields") {
        Some(fields) if fields.is_object() => fields,
        Some(fields) => {
            object.insert("fields".to_string(), fields);
            serde_json::Value::Object(object)
        }
        None => serde_json::Value::Object(object),
    }
}

/// Deserialize a Move struct value, tolerating the `{ fields: ... }` wrapper.
#[derive(Clone, Debug)]
pub struct MoveFields<T>(pub T);

impl<'de, T> Deserialize<'de> for MoveFields<T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            return T::deserialize(deserializer).map(Self);
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        let value = strip_fields_owned(value);
        serde_json::from_value::<T>(value)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

/// Deserialize `0x1::option::Option<T>`.
///
/// accepted:
/// - `{ vec: [] | [T] }` (Move stdlib option layout),
/// - `{ some: T }` / `{ none: ... }`,
/// - `null`,
/// - and a best-effort fallback treating the value as `T`.
#[derive(Clone, Debug, Default)]
pub struct MoveOption<T>(pub Option<T>);

impl<'de, T> Deserialize<'de> for MoveOption<T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // In BCS, Move `Option<T>` is a single-field struct containing a `vector<T>`.
        if !deserializer.is_human_readable() {
            let mut vec = Vec::<T>::deserialize(deserializer)?;
            return Ok(Self(vec.drain(..).next()));
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        let value = strip_fields_owned(value);

        match value {
            serde_json::Value::Null => Ok(Self(None)),
            serde_json::Value::Array(mut vec) => Ok(Self(
                vec.drain(..)
                    .next()
                    .map(strip_fields_owned)
                    .map(serde_json::from_value::<T>)
                    .transpose()
                    .map_err(serde::de::Error::custom)?,
            )),
            serde_json::Value::Object(mut object) => {
                if let Some(vec) = object.remove("vec").or_else(|| object.remove("Vec")) {
                    let vec = strip_fields_owned(vec)
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    let mut vec = vec;
                    return Ok(Self(
                        vec.drain(..)
                            .next()
                            .map(strip_fields_owned)
                            .map(serde_json::from_value::<T>)
                            .transpose()
                            .map_err(serde::de::Error::custom)?,
                    ));
                }

                if let Some(inner) = object.remove("some").or_else(|| object.remove("Some")) {
                    let inner = serde_json::from_value::<T>(strip_fields_owned(inner))
                        .map_err(serde::de::Error::custom)?;
                    return Ok(Self(Some(inner)));
                }

                if object.contains_key("none") || object.contains_key("None") {
                    return Ok(Self(None));
                }

                // Fallback: treat as `T` directly.
                serde_json::from_value::<T>(serde_json::Value::Object(object))
                    .map(Some)
                    .map(Self)
                    .map_err(serde::de::Error::custom)
            }
            other => serde_json::from_value::<T>(other)
                .map(Some)
                .map(Self)
                .map_err(serde::de::Error::custom),
        }
    }
}
