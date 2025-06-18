use {
    super::traits::{BincodeCodec, EncryptionAlgo, EncryptionAlgoDefault, PlaintextCodec},
    base64::{engine::general_purpose, Engine as _},
    rand::{rngs::OsRng, RngCore},
    serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer},
    std::{
        marker::PhantomData,
        ops::{Deref, DerefMut},
    },
};

// -- External Keyed Encryption --

/// Wrapper that transparently encrypts / decrypts its inner value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenericSecret<
    T,
    E: EncryptionAlgo = EncryptionAlgoDefault,
    P: PlaintextCodec = BincodeCodec,
> {
    pub value: T,
    _enc: PhantomData<E>,
    _codec: PhantomData<P>,
}

impl<T, E, P> GenericSecret<T, E, P>
where
    E: EncryptionAlgo,
    P: PlaintextCodec,
{
    pub fn new(value: T) -> Self {
        Self {
            value,
            _enc: PhantomData,
            _codec: PhantomData,
        }
    }
}

impl<T: Default, E: EncryptionAlgo, P: PlaintextCodec> Default for GenericSecret<T, E, P> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T, E: EncryptionAlgo, P: PlaintextCodec> Deref for GenericSecret<T, E, P> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}
impl<T, E: EncryptionAlgo, P: PlaintextCodec> DerefMut for GenericSecret<T, E, P> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl<T, E, P> Serialize for GenericSecret<T, E, P>
where
    T: Serialize,
    E: EncryptionAlgo,
    P: PlaintextCodec,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let plain = P::encode(&self.value).map_err(serde::ser::Error::custom)?;
        let mut nonce = vec![0u8; E::NONCE_LEN];
        if E::NONCE_LEN > 0 {
            rand::rngs::OsRng.fill_bytes(&mut nonce);
        }
        let ct = E::encrypt(&nonce, &plain).map_err(serde::ser::Error::custom)?;
        let buf = if E::NONCE_LEN > 0 {
            let mut v = Vec::with_capacity(E::NONCE_LEN + ct.len());
            v.extend_from_slice(&nonce);
            v.extend_from_slice(&ct);
            v
        } else {
            ct
        };
        let encoded = general_purpose::STANDARD.encode(&buf);
        serializer.serialize_str(&encoded)
    }
}

impl<'de, T, E, P> Deserialize<'de> for GenericSecret<T, E, P>
where
    T: DeserializeOwned,
    E: EncryptionAlgo,
    P: PlaintextCodec,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        let decoded = general_purpose::STANDARD
            .decode(&encoded)
            .map_err(serde::de::Error::custom)?;
        let (nonce_bytes, ciphertext) = if E::NONCE_LEN > 0 {
            if decoded.len() < E::NONCE_LEN {
                return Err(serde::de::Error::custom("ciphertext too short"));
            }
            decoded.split_at(E::NONCE_LEN)
        } else {
            (&[][..], decoded.as_slice())
        };
        let mut nonce = vec![0u8; E::NONCE_LEN];
        if E::NONCE_LEN > 0 {
            nonce.copy_from_slice(nonce_bytes);
        }
        let plain = E::decrypt(&nonce, ciphertext).map_err(serde::de::Error::custom)?;
        let inner: T = P::decode(&plain).map_err(serde::de::Error::custom)?;
        Ok(GenericSecret::new(inner))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::secret_core::error::SecretStoreError,
        serde::{Deserialize, Serialize},
    };

    /// Very small encryption algorithm that just echoes the plaintext.
    /// Good enough for unit-tests that only care about the wrapping logic.
    #[derive(Clone, Debug, Default)]
    struct NoEncryption;

    impl EncryptionAlgo for NoEncryption {
        const NONCE_LEN: usize = 0;

        fn encrypt(_nonce: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, SecretStoreError> {
            Ok(plaintext.to_vec())
        }

        fn decrypt(_nonce: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, SecretStoreError> {
            Ok(ciphertext.to_vec())
        }
    }

    /// A simple payload type we can serialise with bincode/serde.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
    struct Foo {
        id: u32,
        label: String,
    }

    /// A convenient alias that uses our dummy crypto.
    type SecretFoo = GenericSecret<Foo, NoEncryption>;

    /// 1. Serialise -> deserialise round-trip through `serde_json`.
    #[test]
    fn roundtrip_json() {
        let secret = SecretFoo::new(Foo {
            id: 7,
            label: "hello".into(),
        });

        let json = serde_json::to_string(&secret).expect("serialize");
        let decoded: SecretFoo = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(
            *decoded,
            Foo {
                id: 7,
                label: "hello".into()
            }
        );
    }

    /// 2. Default should delegate to the inner type's `Default`.
    #[test]
    fn default_is_plain_default() {
        let secret: GenericSecret<Vec<u8>, NoEncryption> = Default::default();
        assert!(secret.is_empty());
    }

    /// 3. Verify `Deref` and `DerefMut` give ergonomic access.
    #[test]
    fn deref_and_deref_mut() {
        let mut secret = SecretFoo::new(Foo {
            id: 1,
            label: "x".into(),
        });

        // `Deref`
        assert_eq!(secret.id, 1);

        // `DerefMut`
        secret.id = 2;
        assert_eq!(secret.id, 2);
    }

    /// 4. Make sure the serialised representation really is base64.
    #[test]
    fn serialisation_is_base64() {
        let secret = SecretFoo::new(Foo {
            id: 42,
            label: "xyz".into(),
        });
        let encoded_json = serde_json::to_string(&secret).unwrap();

        // Strip the surrounding quotes added by JSON.
        let inner = encoded_json.trim_matches('"');
        assert!(
            base64::engine::general_purpose::STANDARD
                .decode(inner)
                .is_ok(),
            "ciphertext is not valid base64"
        );
    }
}
