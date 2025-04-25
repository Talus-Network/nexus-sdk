//! Module providing logic to encrypt and decrypt data using a Tool's keypair.
//!
//! TODO: <https://github.com/Talus-Network/nexus-sdk/issues/29>

use {
    schemars::{JsonSchema, Schema, SchemaGenerator},
    serde::{self, de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer},
    std::{
        borrow::Cow,
        marker::PhantomData,
        ops::{Deref, DerefMut},
    },
};

#[derive(Debug, PartialEq, Eq)]
// TODO: <https://github.com/Talus-Network/nexus-sdk/issues/29>
pub struct Secret<T, E: EncryptionStrategy = BestEncryptionEver>(T, PhantomData<E>);

impl<T, E: EncryptionStrategy> JsonSchema for Secret<T, E> {
    fn schema_name() -> Cow<'static, str> {
        String::schema_name()
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        String::json_schema(gen)
    }
}

/// Encrypt T before serializing.
impl<T, E> Serialize for Secret<T, E>
where
    T: Serialize,
    E: EncryptionStrategy,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // TODO: <https://github.com/Talus-Network/nexus-sdk/issues/29>
        let encrypted = serde_json::to_string(&self.0)
            .map(|s| E::try_encrypt(&s))
            .map_err(serde::ser::Error::custom)?
            .map_err(serde::ser::Error::custom)?;

        serializer.serialize_str(&encrypted)
    }
}

/// Decrypt T after deserializing.
impl<'de, T, E> Deserialize<'de> for Secret<T, E>
where
    T: DeserializeOwned,
    E: EncryptionStrategy,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encrypted = String::deserialize(deserializer)?;

        // TODO: <https://github.com/Talus-Network/nexus-sdk/issues/29>
        let decrypted = E::try_decrypt(&encrypted).map_err(serde::de::Error::custom)?;

        let t = serde_json::from_str(&decrypted).map_err(serde::de::Error::custom)?;

        Ok(Secret(t, PhantomData))
    }
}

impl<T> Deref for Secret<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Secret<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> Secret<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

/// Trait that defines how a string is encrypted and decrypted.
pub trait EncryptionStrategy {
    fn try_encrypt(s: &str) -> anyhow::Result<String>;
    fn try_decrypt(s: &str) -> anyhow::Result<String>;
}

/// Best encryption ever!
// TODO: figure out how to omit this completely.
#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub struct BestEncryptionEver;

impl EncryptionStrategy for BestEncryptionEver {
    fn try_encrypt(s: &str) -> anyhow::Result<String> {
        Ok(format!("best-encryption-ever-{}", s))
    }

    fn try_decrypt(s: &str) -> anyhow::Result<String> {
        s.strip_prefix("best-encryption-ever-")
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("Decryption failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
    struct Data {
        a: i32,
        b: String,
    }

    #[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
    struct SecretHolder {
        secret: Secret<Data>,
    }

    #[test]
    fn test_secret_encrypt_json() {
        let holder = SecretHolder {
            secret: Secret(
                Data {
                    a: 42,
                    b: "best".to_string(),
                },
                PhantomData,
            ),
        };

        let encrypted = serde_json::to_string(&holder).unwrap();

        assert_eq!(
            encrypted,
            r#"{"secret":"best-encryption-ever-{\"a\":42,\"b\":\"best\"}"}"#
        );
    }

    #[test]
    fn test_secret_decrypt_json() {
        let encrypted = r#"{"secret":"best-encryption-ever-{\"a\":42,\"b\":\"best\"}"}"#;
        let holder = serde_json::from_str::<SecretHolder>(encrypted).unwrap();

        assert_eq!(
            holder.secret.into_inner(),
            Data {
                a: 42,
                b: "best".to_string()
            }
        );
    }

    #[test]
    fn test_secret_decrypt_fail() {
        let encrypted = r#"{"secret":"worst-encryption-ever-{\"a\":42,\"b\":\"best\"}"}"#;
        let holder = serde_json::from_str::<SecretHolder>(encrypted);

        assert!(matches!(holder, Err(e) if e.to_string().contains("Decryption failed")));
    }

    #[test]
    fn test_secret_deser_fail() {
        let encrypted = r#"{"secret":"best-encryption-ever-{\"a\":42}"}"#;
        let holder = serde_json::from_str::<SecretHolder>(encrypted);

        assert!(matches!(holder, Err(e) if e.to_string().contains("missing field `b`")));
    }
}
