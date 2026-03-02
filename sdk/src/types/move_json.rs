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

#[cfg(test)]
mod tests {
    use {
        super::*,
        serde::{Deserialize, Serialize},
        serde_json::json,
    };

    #[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
    struct Foo {
        a: u64,
    }

    #[test]
    fn strip_fields_owned_behaviour() {
        let value = json!("hello");
        assert_eq!(strip_fields_owned(value.clone()), value);

        let value = json!({"a": 1});
        assert_eq!(strip_fields_owned(value.clone()), value);

        let value = json!({"fields": {"a": 1}});
        assert_eq!(strip_fields_owned(value), json!({"a": 1}));

        let value = json!({"fields": 1, "x": 2});
        assert_eq!(strip_fields_owned(value.clone()), value);
    }

    #[test]
    fn move_fields_deserializes_json_with_or_without_wrapper() {
        let wrapped: MoveFields<Foo> =
            serde_json::from_value(json!({"fields": {"a": 1}})).expect("should unwrap fields");
        assert_eq!(wrapped.0, Foo { a: 1 });

        let plain: MoveFields<Foo> =
            serde_json::from_value(json!({"a": 2})).expect("should deserialize plain struct");
        assert_eq!(plain.0, Foo { a: 2 });
    }

    #[cfg(feature = "bcs")]
    #[test]
    fn move_fields_deserializes_bcs_without_wrapper() {
        let bytes = bcs::to_bytes(&Foo { a: 3 }).expect("BCS serialization should succeed");
        let decoded: MoveFields<Foo> =
            bcs::from_bytes(&bytes).expect("BCS deserialization should succeed");
        assert_eq!(decoded.0, Foo { a: 3 });
    }

    #[test]
    fn move_option_deserializes_common_json_forms() {
        let none: MoveOption<u64> = serde_json::from_value(json!(null)).expect("null is accepted");
        assert_eq!(none.0, None);

        let array_some: MoveOption<u64> =
            serde_json::from_value(json!([5])).expect("array form is accepted");
        assert_eq!(array_some.0, Some(5));

        let vec_some: MoveOption<u64> =
            serde_json::from_value(json!({"vec": [7]})).expect("vec form is accepted");
        assert_eq!(vec_some.0, Some(7));

        let vec_cap: MoveOption<u64> =
            serde_json::from_value(json!({"Vec": [8]})).expect("Vec form is accepted");
        assert_eq!(vec_cap.0, Some(8));
    }

    #[test]
    fn move_option_deserializes_some_none_and_fallback_forms() {
        let some: MoveOption<u64> =
            serde_json::from_value(json!({"some": 9})).expect("some form is accepted");
        assert_eq!(some.0, Some(9));

        let none: MoveOption<u64> =
            serde_json::from_value(json!({"none": true})).expect("none form is accepted");
        assert_eq!(none.0, None);

        let none_cap: MoveOption<u64> =
            serde_json::from_value(json!({"None": {}})).expect("None form is accepted");
        assert_eq!(none_cap.0, None);

        let number: MoveOption<u64> =
            serde_json::from_value(json!(12)).expect("plain value form is accepted");
        assert_eq!(number.0, Some(12));

        let fallback: MoveOption<Foo> = serde_json::from_value(json!({"a": 42}))
            .expect("fallback object form should deserialize as T");
        assert_eq!(fallback.0, Some(Foo { a: 42 }));

        let some_wrapped: MoveOption<Foo> = serde_json::from_value(json!({
            "some": { "fields": { "a": 13 } }
        }))
        .expect("some inner fields wrapper should be tolerated");
        assert_eq!(some_wrapped.0, Some(Foo { a: 13 }));
    }

    #[cfg(feature = "bcs")]
    #[test]
    fn move_option_deserializes_bcs_vec_layout() {
        let bytes = bcs::to_bytes(&vec![1u64]).expect("BCS serialization should succeed");
        let decoded: MoveOption<u64> =
            bcs::from_bytes(&bytes).expect("BCS deserialization should succeed");
        assert_eq!(decoded.0, Some(1));

        let bytes = bcs::to_bytes(&Vec::<u64>::new()).expect("BCS serialization should succeed");
        let decoded: MoveOption<u64> =
            bcs::from_bytes(&bytes).expect("BCS deserialization should succeed");
        assert_eq!(decoded.0, None);
    }
}
