use serde::{de::Error as _, Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SuiBalance {
    pub value: u64,
}

impl<'de> Deserialize<'de> for SuiBalance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            struct RawBalance {
                value: u64,
            }

            return RawBalance::deserialize(deserializer).map(|balance| Self {
                value: balance.value,
            });
        }

        let value = super::strip_fields_owned(serde_json::Value::deserialize(deserializer)?);
        if let Some(parsed) = super::parse_u64_value(&value).map_err(D::Error::custom)? {
            return Ok(Self { value: parsed });
        }

        let parsed = value
            .as_object()
            .and_then(|object| object.get("value"))
            .and_then(|value| super::parse_u64_value(value).ok().flatten())
            .ok_or_else(|| D::Error::custom("missing SUI balance value"))?;

        Ok(Self { value: parsed })
    }
}
