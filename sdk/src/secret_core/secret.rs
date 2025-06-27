use {
    super::{
        error::SecretStoreError,
        traits::{
            BincodeCodec,
            EncryptionAlgo,
            EncryptionAlgoDefault,
            KeyProvider,
            KeyedEncryptionAlgo,
            PlaintextCodec,
        },
    },
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

// -- Internal Keyed Encryption --

#[derive(PartialEq, Eq)]
pub struct GenericSecretKeyed<
    T,
    A: KeyedEncryptionAlgo,
    C: PlaintextCodec,
    K: KeyProvider<Key = A::Key>,
> {
    // Either ciphertext or plaintext is present – never both.
    cipher: Option<(Vec<u8>, Vec<u8>)>, // (nonce, ct)
    plain: Option<T>,                   // decrypted value

    provider: Option<K>, // key provider

    _algo: PhantomData<A>,
    _codec: PhantomData<C>,
}

impl<T, A, C, K> GenericSecretKeyed<T, A, C, K>
where
    A: KeyedEncryptionAlgo,
    C: PlaintextCodec,
    K: KeyProvider<Key = A::Key>,
{
    /// Construct an encrypted secret without provider (used by `Deserialize`).
    pub fn new_encrypted(nonce: Vec<u8>, ct: Vec<u8>) -> Self {
        Self {
            cipher: Some((nonce, ct)),
            plain: None,
            provider: None,
            _algo: PhantomData,
            _codec: PhantomData,
        }
    }

    /// Construct a ready‑to‑use secret (happy path).
    pub fn with_provider(value: T, provider: K) -> Self {
        Self {
            cipher: None,
            plain: Some(value),
            provider: Some(provider),
            _algo: PhantomData,
            _codec: PhantomData,
        }
    }

    /// Attach or replace a provider (needed right after deserialisation).
    pub fn attach_provider(&mut self, provider: K) {
        self.provider = Some(provider)
    }

    /// Temporarily expose the decrypted value.  Decrypts lazily, zeroises on drop.
    pub fn expose<F, R>(&mut self, f: F) -> Result<R, SecretStoreError>
    where
        F: FnOnce(&T) -> R,
        T: DeserializeOwned,
    {
        // Decrypt on first access.
        if self.plain.is_none() {
            let (nonce, ct) = self.cipher.take().ok_or_else(|| {
                SecretStoreError::Crypto(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "missing ciphertext",
                )))
            })?;

            let provider = self.require_provider()?;
            let key = provider.key()?;

            let pt_bytes = A::decrypt_with_key(&key, &nonce, &ct)?;
            let value =
                C::decode(&pt_bytes).map_err(|e| SecretStoreError::Codec(e.to_string().into()))?;

            self.plain = Some(value);
        }

        Ok(f(self.plain.as_ref().unwrap()))
    }

    // Helper function to require a provider
    fn require_provider(&self) -> Result<&K, SecretStoreError> {
        self.provider
            .as_ref()
            .ok_or_else(|| SecretStoreError::Provider("no key provider attached".into()))
    }
}

// Default

impl<T: Default, A, C, K> Default for GenericSecretKeyed<T, A, C, K>
where
    A: KeyedEncryptionAlgo,
    C: PlaintextCodec,
    K: KeyProvider<Key = A::Key>,
{
    fn default() -> Self {
        Self {
            cipher: None, // not encrypted yet
            plain: Some(T::default()),
            provider: None, // must be attached later
            _algo: PhantomData,
            _codec: PhantomData,
        }
    }
}

// Serde impls

impl<T, A, C, K> Serialize for GenericSecretKeyed<T, A, C, K>
where
    T: Serialize,
    A: KeyedEncryptionAlgo,
    C: PlaintextCodec,
    K: KeyProvider<Key = A::Key>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let plain_ref = self.plain.as_ref().ok_or_else(|| {
            serde::ser::Error::custom("secret locked – provider missing or not decrypted")
        })?;

        // Encode plaintext -> bytes (buffer is zeroised on drop)
        let pt_buf = C::encode(plain_ref).map_err(|e| serde::ser::Error::custom(e.to_string()))?;

        // Fresh nonce
        let mut nonce = vec![0u8; A::NONCE_LEN];
        if A::NONCE_LEN > 0 {
            OsRng.fill_bytes(&mut nonce);
        }

        // Encrypt
        let key = self
            .require_provider()
            .map_err(|e| serde::ser::Error::custom(e.to_string()))?
            .key()
            .map_err(|e| serde::ser::Error::custom(e.to_string()))?;

        let ct = A::encrypt_with_key(&key, &nonce, &pt_buf).map_err(serde::ser::Error::custom)?;

        // nonce || ct -> Base64 string
        let mut buf = Vec::with_capacity(A::NONCE_LEN + ct.len());
        if A::NONCE_LEN > 0 {
            buf.extend_from_slice(&nonce);
        }
        buf.extend_from_slice(&ct);

        serializer.serialize_str(&general_purpose::STANDARD.encode(buf))
    }
}

impl<'de, T, A, C, K> Deserialize<'de> for GenericSecretKeyed<T, A, C, K>
where
    T: DeserializeOwned,
    A: KeyedEncryptionAlgo,
    C: PlaintextCodec,
    K: KeyProvider<Key = A::Key>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        let decoded = general_purpose::STANDARD
            .decode(&encoded)
            .map_err(serde::de::Error::custom)?;

        if decoded.len() < A::NONCE_LEN {
            return Err(serde::de::Error::custom("ciphertext too short"));
        }

        let (nonce, ct) = decoded.split_at(A::NONCE_LEN);

        Ok(Self::new_encrypted(nonce.to_vec(), ct.to_vec()))
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

    /// "Do-nothing" keyed cipher - just echoes the plaintext.
    #[derive(Clone, Debug, Default)]
    struct NoEncryptionKeyed;

    impl KeyedEncryptionAlgo for NoEncryptionKeyed {
        type Key = ();

        // no real key material
        const NONCE_LEN: usize = 0;

        // deterministic / nonce-less

        fn encrypt_with_key(
            _key: &Self::Key,
            _nonce: &[u8],
            pt: &[u8],
        ) -> Result<Vec<u8>, SecretStoreError> {
            Ok(pt.to_vec())
        }

        fn decrypt_with_key(
            _key: &Self::Key,
            _nonce: &[u8],
            ct: &[u8],
        ) -> Result<Vec<u8>, SecretStoreError> {
            Ok(ct.to_vec())
        }
    }

    /// Provider that yields an empty key (because our cipher ignores it).
    #[derive(Default, Debug, Clone, Copy)]
    struct DummyProvider;

    impl KeyProvider for DummyProvider {
        type Key = ();

        fn key(&self) -> Result<Self::Key, SecretStoreError> {
            Ok(())
        }
    }

    type SecretFooKeyed = GenericSecretKeyed<Foo, NoEncryptionKeyed, BincodeCodec, DummyProvider>;

    //  Serialise -> deserialise round-trip
    #[test]
    fn keyed_roundtrip_json() {
        let secret = SecretFooKeyed::with_provider(
            Foo {
                id: 9,
                label: "abc".into(),
            },
            DummyProvider,
        );

        let json = serde_json::to_string(&secret).expect("serialize");

        // After deserialisation the provider is missing -> must be re-attached.
        let mut decoded: SecretFooKeyed = serde_json::from_str(&json).expect("deserialize");
        decoded.attach_provider(DummyProvider);

        decoded
            .expose(|v| {
                assert_eq!(v.id, 9);
                assert_eq!(v.label, "abc");
            })
            .expect("expose");
    }

    // `Default` should yield the inner type's default.
    #[test]
    fn keyed_default_is_plain_default() {
        let mut secret: SecretFooKeyed = Default::default();
        secret.attach_provider(DummyProvider);

        secret
            .expose(|v| {
                assert_eq!(
                    *v,
                    Foo {
                        id: 0,
                        label: String::new()
                    }
                )
            })
            .unwrap();
    }

    // Serialised ciphertext must be valid Base64.
    #[test]
    fn keyed_serialisation_is_base64() {
        let secret = SecretFooKeyed::with_provider(
            Foo {
                id: 42,
                label: "xyz".into(),
            },
            DummyProvider,
        );
        let encoded_json = serde_json::to_string(&secret).unwrap();
        let inner = encoded_json.trim_matches('"'); // strip JSON quotes

        assert!(
            base64::engine::general_purpose::STANDARD
                .decode(inner)
                .is_ok(),
            "ciphertext is not valid base64"
        );
    }

    /// Access without attaching a provider should raise an error.
    #[test]
    fn keyed_missing_provider_fails() {
        // Construct by serialising with provider, then deserialising without re-attachment.
        let original = SecretFooKeyed::with_provider(
            Foo {
                id: 1,
                label: "fail".into(),
            },
            DummyProvider,
        );
        let json = serde_json::to_string(&original).unwrap();
        let mut decoded: SecretFooKeyed = serde_json::from_str(&json).unwrap();

        let err = decoded
            .expose(|_| ())
            .expect_err("expose must fail without provider");

        assert!(matches!(err, SecretStoreError::Provider(_)));
    }
}
